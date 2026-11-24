use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions, Permissions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path};

use crate::input::AppStopConfirmation;

pub const APP_DATA_DIR_NAME: &str = "dev.stockcopilot.desktop";
pub const LOCK_FILE_NAME: &str = "toss-token.lock";
pub const DESKTOP_STOPPED_CONTENT: &str = "version=1\nowner=desktop-app\nstate=stopped\n";
pub const SMOKE_RUNNING_CONTENT: &str = "version=1\nowner=toss-smoke\nstate=running\n";
pub const SMOKE_STOPPED_CONTENT: &str = "version=1\nowner=toss-smoke\nstate=stopped\n";

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;
const MAX_LOCK_CONTENT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    DesktopVerifiedStopped,
    SmokeConfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockError {
    MissingDefaultPath,
    UnsafePath,
    AppRunning,
    UnconfirmedIndeterminate,
    IoError,
}

impl LockError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingDefaultPath => {
                "기본 앱 데이터 경로를 찾을 수 없습니다. `--lock-file <경로>`로 잠금 파일 경로를 지정하세요."
            }
            Self::UnsafePath => {
                "잠금 파일 또는 상위 폴더가 심볼릭 링크이거나 안전한 일반 파일 경로가 아닙니다."
            }
            Self::AppRunning => {
                "앱 또는 다른 연결 확인 프로세스가 토큰 잠금을 보유 중입니다. 토스 계정은 토큰이 하나뿐이므로 앱을 완전히 종료한 뒤 다시 실행하세요."
            }
            Self::UnconfirmedIndeterminate => {
                "잠금 파일로 앱 종료 상태를 증명할 수 없어 닫힌 실패로 중단했습니다. 앱을 완전히 끈 뒤 표준 입력 3줄에 `앱을 껐습니다`를 함께 전달하세요."
            }
            Self::IoError => "잠금 파일을 열거나 잠그는 중 입출력 오류가 발생했습니다.",
        }
    }
}

pub struct LockGuard {
    file: File,
    mode: LockMode,
}

impl fmt::Debug for LockGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockGuard")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl LockGuard {
    #[must_use]
    pub const fn mode(&self) -> LockMode {
        self.mode
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        match self.mode {
            LockMode::SmokeConfirmed => match rewrite_marker(&mut self.file, SMOKE_STOPPED_CONTENT)
            {
                Ok(()) | Err(_) => {}
            },
            LockMode::DesktopVerifiedStopped => {}
        }
    }
}

fn rewrite_marker(file: &mut File, content: &str) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(content.as_bytes())?;
    file.sync_data()
}

fn validate_lock_path_shape(path: &Path) -> Result<&Path, LockError> {
    if path.as_os_str().is_empty() {
        return Err(LockError::UnsafePath);
    }
    let last_is_normal = path
        .components()
        .next_back()
        .is_some_and(|component| match component {
            Component::Normal(_) => true,
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => false,
        });
    if !last_is_normal {
        return Err(LockError::UnsafePath);
    }
    let parent = path.parent().ok_or(LockError::UnsafePath)?;
    if parent.as_os_str().is_empty() {
        return Err(LockError::UnsafePath);
    }
    Ok(parent)
}

fn check_existing_parent(parent: &Path) -> Result<ParentStatus, LockError> {
    match fs::symlink_metadata(parent) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(LockError::UnsafePath);
            }
            Ok(ParentStatus::Exists)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ParentStatus::Missing),
        Err(_) => Err(LockError::IoError),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentStatus {
    Exists,
    Missing,
}

fn ensure_private_parent(parent: &Path) -> Result<(), LockError> {
    match DirBuilder::new().mode(DIR_MODE).create(parent) {
        Ok(()) => {
            fs::set_permissions(parent, Permissions::from_mode(DIR_MODE))
                .map_err(|_error| LockError::IoError)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            match check_existing_parent(parent)? {
                ParentStatus::Exists => Ok(()),
                ParentStatus::Missing => Err(LockError::IoError),
            }
        }
        Err(_) => Err(LockError::IoError),
    }
}

fn try_exclusive_lock(file: &File) -> Result<(), LockError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(LockError::AppRunning),
        Err(TryLockError::Error(_)) => Err(LockError::IoError),
    }
}

const MAX_LOCK_READ_LIMIT: u64 = 257;

fn read_lock_content(file: &mut File) -> Result<String, LockError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_error| LockError::IoError)?;
    let mut raw = Vec::new();
    file.take(MAX_LOCK_READ_LIMIT)
        .read_to_end(&mut raw)
        .map_err(|_error| LockError::IoError)?;
    if raw.len() > MAX_LOCK_CONTENT_BYTES {
        return Ok(String::new());
    }
    match String::from_utf8(raw) {
        Ok(text) => Ok(text),
        Err(_) => Ok(String::new()),
    }
}

fn open_and_verify_existing(path: &Path) -> Result<File, LockError> {
    let link_meta = fs::symlink_metadata(path).map_err(|_error| LockError::IoError)?;
    if link_meta.file_type().is_symlink() || link_meta.is_dir() || !link_meta.is_file() {
        return Err(LockError::UnsafePath);
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_error| LockError::IoError)?;
    let open_meta = file.metadata().map_err(|_error| LockError::IoError)?;
    if open_meta.is_dir()
        || !open_meta.is_file()
        || open_meta.ino() != link_meta.ino()
        || open_meta.dev() != link_meta.dev()
    {
        return Err(LockError::UnsafePath);
    }
    Ok(file)
}

fn lock_existing(path: &Path, confirmation: AppStopConfirmation) -> Result<LockGuard, LockError> {
    let mut file = open_and_verify_existing(path)?;
    try_exclusive_lock(&file)?;
    let content = read_lock_content(&mut file)?;
    if content == DESKTOP_STOPPED_CONTENT {
        return Ok(LockGuard {
            file,
            mode: LockMode::DesktopVerifiedStopped,
        });
    }
    match confirmation {
        AppStopConfirmation::NotProvided => return Err(LockError::UnconfirmedIndeterminate),
        AppStopConfirmation::Confirmed => {}
    }
    file.set_permissions(Permissions::from_mode(FILE_MODE))
        .map_err(|_error| LockError::IoError)?;
    rewrite_marker(&mut file, SMOKE_RUNNING_CONTENT).map_err(|_error| LockError::IoError)?;
    Ok(LockGuard {
        file,
        mode: LockMode::SmokeConfirmed,
    })
}

fn create_and_lock_new(
    path: &Path,
    confirmation: AppStopConfirmation,
) -> Result<LockGuard, LockError> {
    match confirmation {
        AppStopConfirmation::NotProvided => return Err(LockError::UnconfirmedIndeterminate),
        AppStopConfirmation::Confirmed => {}
    }
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(path)
    {
        Ok(mut file) => {
            try_exclusive_lock(&file)?;
            file.set_permissions(Permissions::from_mode(FILE_MODE))
                .map_err(|_error| LockError::IoError)?;
            rewrite_marker(&mut file, SMOKE_RUNNING_CONTENT)
                .map_err(|_error| LockError::IoError)?;
            Ok(LockGuard {
                file,
                mode: LockMode::SmokeConfirmed,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            lock_existing(path, confirmation)
        }
        Err(_) => Err(LockError::IoError),
    }
}

pub fn acquire_token_lock(
    path: &Path,
    confirmation: AppStopConfirmation,
) -> Result<LockGuard, LockError> {
    let parent = validate_lock_path_shape(path)?;
    match check_existing_parent(parent)? {
        ParentStatus::Missing => {
            match confirmation {
                AppStopConfirmation::NotProvided => {
                    return Err(LockError::UnconfirmedIndeterminate);
                }
                AppStopConfirmation::Confirmed => {}
            }
            ensure_private_parent(parent)?;
            create_and_lock_new(path, confirmation)
        }
        ParentStatus::Exists => match fs::symlink_metadata(path) {
            Ok(_) => lock_existing(path, confirmation),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                create_and_lock_new(path, confirmation)
            }
            Err(_) => Err(LockError::IoError),
        },
    }
}
