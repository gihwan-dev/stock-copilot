use std::fs::{self, DirBuilder, Permissions};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use tokio::net::UnixListener;

use crate::peer::UserId;

pub const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103_usize;
pub const DEFAULT_SOCKET_FILE_NAME: &str = "m.sock";
pub const DIRECTORY_MODE: u32 = 0o700_u32;
pub const SOCKET_MODE: u32 = 0o600_u32;

const RANDOM_DIR_BYTES: usize = 8_usize;
const MODE_PERMISSION_MASK: u32 = 0o7777_u32;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketSetupError {
    EntropyUnavailable,
    InvalidSocketName,
    InvalidRunDirectory,
    MissingParentDirectory,
    InvalidBaseDirectory,
    SymlinkNotAllowed,
    PathTooLong,
    LeftoverPathRefused,
    DirectoryCreateFailed,
    DirectoryPermissionsInvalid,
    BindFailed,
    SocketPermissionsInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSocketSpec {
    run_dir: PathBuf,
    socket_path: PathBuf,
}

impl RunSocketSpec {
    pub fn generate_under(base_dir: &Path) -> Result<Self, SocketSetupError> {
        let mut raw = [0_u8; RANDOM_DIR_BYTES];
        getrandom::fill(&mut raw).map_err(|_error| SocketSetupError::EntropyUnavailable)?;
        let suffix = encode_dir_suffix(&raw)?;
        let run_dir = base_dir.join(format!("r{suffix}"));
        Self::explicit(run_dir, DEFAULT_SOCKET_FILE_NAME)
    }

    pub fn explicit(run_dir: PathBuf, socket_file_name: &str) -> Result<Self, SocketSetupError> {
        validate_socket_file_name(socket_file_name)?;
        validate_run_dir_path(&run_dir)?;
        let socket_path = run_dir.join(socket_file_name);
        validate_socket_path_length(&socket_path)?;
        Ok(Self {
            run_dir,
            socket_path,
        })
    }

    #[must_use]
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

fn validate_run_dir_path(run_dir: &Path) -> Result<(), SocketSetupError> {
    if !run_dir.is_absolute() {
        return Err(SocketSetupError::InvalidRunDirectory);
    }
    if run_dir.parent().is_none() || run_dir.file_name().is_none() {
        return Err(SocketSetupError::MissingParentDirectory);
    }
    if run_dir
        .as_os_str()
        .as_encoded_bytes()
        .split(|&byte| byte == b'/')
        .any(|segment| segment == b"." || segment == b"..")
    {
        return Err(SocketSetupError::InvalidRunDirectory);
    }
    for component in run_dir.components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(SocketSetupError::InvalidRunDirectory);
            }
        }
    }
    Ok(())
}

fn encode_dir_suffix(bytes: &[u8]) -> Result<String, SocketSetupError> {
    let mut text = String::with_capacity(bytes.len().saturating_mul(2_usize));
    for &byte in bytes {
        let high = usize::from(byte.wrapping_shr(4_u32));
        let low = usize::from(byte & 0x0f_u8);
        let Some(&high_char) = HEX_DIGITS.get(high) else {
            return Err(SocketSetupError::EntropyUnavailable);
        };
        let Some(&low_char) = HEX_DIGITS.get(low) else {
            return Err(SocketSetupError::EntropyUnavailable);
        };
        text.push(char::from(high_char));
        text.push(char::from(low_char));
    }
    Ok(text)
}

fn validate_socket_file_name(name: &str) -> Result<(), SocketSetupError> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(SocketSetupError::InvalidSocketName);
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err(SocketSetupError::InvalidSocketName);
    }
    Ok(())
}

pub fn validate_socket_path_length(path: &Path) -> Result<(), SocketSetupError> {
    let byte_len = path.as_os_str().as_encoded_bytes().len();
    if byte_len == 0_usize || byte_len > MAX_UNIX_SOCKET_PATH_BYTES {
        return Err(SocketSetupError::PathTooLong);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupState {
    Pending,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SocketBoundState {
    NotBound,
    Bound,
}

#[derive(Debug)]
pub struct SocketCleanupGuard {
    run_dir: PathBuf,
    socket_path: PathBuf,
    socket_state: SocketBoundState,
    state: CleanupState,
}

impl SocketCleanupGuard {
    fn new(run_dir: PathBuf, socket_path: PathBuf) -> Self {
        Self {
            run_dir,
            socket_path,
            socket_state: SocketBoundState::NotBound,
            state: CleanupState::Pending,
        }
    }

    fn mark_socket_bound(&mut self) {
        self.socket_state = SocketBoundState::Bound;
    }

    pub fn cleanup(&mut self) {
        match self.state {
            CleanupState::Done => return,
            CleanupState::Pending => {}
        }
        self.state = CleanupState::Done;
        match self.socket_state {
            SocketBoundState::Bound => remove_owned_socket(&self.socket_path),
            SocketBoundState::NotBound => {}
        }
        remove_owned_dir(&self.run_dir);
    }
}

impl Drop for SocketCleanupGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn remove_owned_socket(socket_path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(socket_path)
        && !meta.file_type().is_symlink()
        && meta.file_type().is_socket()
    {
        match fs::remove_file(socket_path) {
            Ok(()) => {}
            Err(_error) => {}
        }
    }
}

fn remove_owned_dir(run_dir: &Path) {
    if let Ok(dir_meta) = fs::symlink_metadata(run_dir)
        && !dir_meta.file_type().is_symlink()
        && dir_meta.is_dir()
    {
        match fs::remove_dir(run_dir) {
            Ok(()) => {}
            Err(_error) => {}
        }
    }
}

fn validate_base_dir(run_dir: &Path) -> Result<(), SocketSetupError> {
    let Some(base_dir) = run_dir.parent() else {
        return Err(SocketSetupError::MissingParentDirectory);
    };
    let base_meta =
        fs::symlink_metadata(base_dir).map_err(|_error| SocketSetupError::InvalidBaseDirectory)?;
    if base_meta.file_type().is_symlink() {
        return Err(SocketSetupError::SymlinkNotAllowed);
    }
    if !base_meta.is_dir() {
        return Err(SocketSetupError::InvalidBaseDirectory);
    }
    Ok(())
}

fn create_run_dir(run_dir: &Path) -> Result<(), SocketSetupError> {
    match DirBuilder::new().mode(DIRECTORY_MODE).create(run_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            Err(SocketSetupError::LeftoverPathRefused)
        }
        Err(_error) => Err(SocketSetupError::DirectoryCreateFailed),
    }
}

fn verify_run_dir_0700(run_dir: &Path) -> Result<UserId, SocketSetupError> {
    let dir_meta = fs::symlink_metadata(run_dir)
        .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
    if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
        return Err(SocketSetupError::SymlinkNotAllowed);
    }
    let owner_uid = dir_meta.uid();
    if (dir_meta.mode() & MODE_PERMISSION_MASK) != DIRECTORY_MODE {
        fs::set_permissions(run_dir, Permissions::from_mode(DIRECTORY_MODE))
            .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
        let rechecked = fs::symlink_metadata(run_dir)
            .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
        if rechecked.file_type().is_symlink()
            || !rechecked.is_dir()
            || rechecked.uid() != owner_uid
            || (rechecked.mode() & MODE_PERMISSION_MASK) != DIRECTORY_MODE
        {
            return Err(SocketSetupError::DirectoryPermissionsInvalid);
        }
    }
    Ok(UserId::new(owner_uid))
}

fn ensure_socket_absent(socket_path: &Path) -> Result<(), SocketSetupError> {
    match fs::symlink_metadata(socket_path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_existing) => Err(SocketSetupError::LeftoverPathRefused),
        Err(_error) => Err(SocketSetupError::LeftoverPathRefused),
    }
}

fn bind_and_verify_socket(
    socket_path: &Path,
    owner_uid: UserId,
    cleanup: &mut SocketCleanupGuard,
) -> Result<UnixListener, SocketSetupError> {
    let listener =
        UnixListener::bind(socket_path).map_err(|_error| SocketSetupError::BindFailed)?;
    cleanup.mark_socket_bound();
    fs::set_permissions(socket_path, Permissions::from_mode(SOCKET_MODE))
        .map_err(|_error| SocketSetupError::SocketPermissionsInvalid)?;
    let socket_meta = fs::symlink_metadata(socket_path)
        .map_err(|_error| SocketSetupError::SocketPermissionsInvalid)?;
    if socket_meta.file_type().is_symlink()
        || !socket_meta.file_type().is_socket()
        || (socket_meta.mode() & MODE_PERMISSION_MASK) != SOCKET_MODE
        || socket_meta.uid() != owner_uid.as_u32()
    {
        return Err(SocketSetupError::SocketPermissionsInvalid);
    }
    Ok(listener)
}

#[derive(Debug)]
pub struct BoundRunSocket {
    listener: UnixListener,
    owner_uid: UserId,
    run_dir: PathBuf,
    socket_path: PathBuf,
    cleanup: SocketCleanupGuard,
}

impl BoundRunSocket {
    pub fn bind(spec: &RunSocketSpec) -> Result<Self, SocketSetupError> {
        validate_socket_path_length(spec.socket_path())?;
        validate_base_dir(spec.run_dir())?;
        create_run_dir(spec.run_dir())?;

        let mut cleanup = SocketCleanupGuard::new(
            spec.run_dir().to_path_buf(),
            spec.socket_path().to_path_buf(),
        );
        let owner_uid = verify_run_dir_0700(spec.run_dir())?;
        ensure_socket_absent(spec.socket_path())?;
        let listener = bind_and_verify_socket(spec.socket_path(), owner_uid, &mut cleanup)?;

        Ok(Self {
            listener,
            owner_uid,
            run_dir: spec.run_dir().to_path_buf(),
            socket_path: spec.socket_path().to_path_buf(),
            cleanup,
        })
    }

    #[must_use]
    pub const fn owner_uid(&self) -> UserId {
        self.owner_uid
    }

    #[must_use]
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    #[must_use]
    pub fn into_parts(self) -> (UnixListener, UserId, PathBuf, PathBuf, SocketCleanupGuard) {
        (
            self.listener,
            self.owner_uid,
            self.run_dir,
            self.socket_path,
            self.cleanup,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        MAX_UNIX_SOCKET_PATH_BYTES, RunSocketSpec, SocketSetupError, validate_socket_path_length,
    };

    #[test]
    fn socket_path_length_enforces_103_byte_sun_path_boundary() {
        let exact = PathBuf::from(format!(
            "/{}",
            "a".repeat(MAX_UNIX_SOCKET_PATH_BYTES.saturating_sub(1_usize))
        ));
        assert_eq!(
            exact.as_os_str().as_encoded_bytes().len(),
            103_usize,
            "103바이트 경로 준비"
        );
        assert_eq!(
            validate_socket_path_length(&exact),
            Ok(()),
            "103바이트 경로는 NUL 포함 104바이트 이내로 허용한다"
        );

        let too_long = PathBuf::from(format!("/{}", "a".repeat(MAX_UNIX_SOCKET_PATH_BYTES)));
        assert_eq!(
            too_long.as_os_str().as_encoded_bytes().len(),
            104_usize,
            "104바이트 경로 준비"
        );
        assert_eq!(
            validate_socket_path_length(&too_long),
            Err(SocketSetupError::PathTooLong),
            "104바이트 경로는 NUL 포함 105바이트이므로 거부한다"
        );
        assert_eq!(
            validate_socket_path_length(Path::new("")),
            Err(SocketSetupError::PathTooLong),
            "빈 경로는 거부한다"
        );
    }

    #[test]
    fn socket_spec_rejects_bad_names_relative_and_traversal_paths() {
        for invalid in ["", ".", "..", "a/b.sock", "m.sock\0", "bad name.sock"] {
            assert_eq!(
                RunSocketSpec::explicit(PathBuf::from("/tmp/r1"), invalid),
                Err(SocketSetupError::InvalidSocketName),
                "잘못된 소켓 파일 이름 거부: {invalid:?}"
            );
        }
        assert_eq!(
            RunSocketSpec::explicit(PathBuf::from("/"), "m.sock"),
            Err(SocketSetupError::MissingParentDirectory),
            "부모 폴더 없는 루트 경로 거부"
        );
        for bad_dir in ["relative/r1", "/tmp/a/../r1", "/tmp/./r1"] {
            assert_eq!(
                RunSocketSpec::explicit(PathBuf::from(bad_dir), "m.sock"),
                Err(SocketSetupError::InvalidRunDirectory),
                "상대·상위 이동 경로 거부: {bad_dir}"
            );
        }
        let valid = RunSocketSpec::explicit(PathBuf::from("/tmp/r1"), "m.sock").expect("유효 스펙");
        assert_eq!(valid.run_dir(), Path::new("/tmp/r1"), "run_dir 보존");
        assert_eq!(
            valid.socket_path(),
            Path::new("/tmp/r1/m.sock"),
            "socket_path 보존"
        );
    }
}
