use std::fs::{self, DirBuilder, Metadata, Permissions};
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
const GROUP_OR_OTHER_WRITE_MASK: u32 = 0o022_u32;
const STICKY_BIT_MASK: u32 = 0o1000_u32;
const ROOT_UID: u32 = 0_u32;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketSetupError {
    EntropyUnavailable,
    InvalidSocketName,
    InvalidRunDirectory,
    MissingParentDirectory,
    InvalidBaseDirectory,
    BaseDirectoryPermissionsInvalid,
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
    if run_dir.parent().is_none() {
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
enum ObservedFileKind {
    Directory,
    Socket,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservedFileMeta {
    kind: ObservedFileKind,
    uid: UserId,
    mode: u32,
}

impl ObservedFileMeta {
    fn from_metadata(meta: &Metadata) -> Self {
        let file_type = meta.file_type();
        let kind = if file_type.is_symlink() {
            ObservedFileKind::Symlink
        } else if file_type.is_dir() {
            ObservedFileKind::Directory
        } else if file_type.is_socket() {
            ObservedFileKind::Socket
        } else {
            ObservedFileKind::Other
        };
        Self {
            kind,
            uid: UserId::new(meta.uid()),
            mode: meta.mode(),
        }
    }
}

fn check_ancestor_dir_precreate(meta: ObservedFileMeta) -> Result<(), SocketSetupError> {
    match meta.kind {
        ObservedFileKind::Symlink => return Err(SocketSetupError::SymlinkNotAllowed),
        ObservedFileKind::Directory => {}
        ObservedFileKind::Socket | ObservedFileKind::Other => {
            return Err(SocketSetupError::InvalidBaseDirectory);
        }
    }
    let perm = meta.mode & MODE_PERMISSION_MASK;
    let other_writable = (perm & GROUP_OR_OTHER_WRITE_MASK) != 0_u32;
    let sticky = (perm & STICKY_BIT_MASK) != 0_u32;
    if other_writable && !sticky {
        return Err(SocketSetupError::BaseDirectoryPermissionsInvalid);
    }
    Ok(())
}

fn check_ancestor_dir_ownership(
    meta: ObservedFileMeta,
    owner_uid: UserId,
) -> Result<(), SocketSetupError> {
    check_ancestor_dir_precreate(meta)?;
    let dir_uid = meta.uid.as_u32();
    if dir_uid != owner_uid.as_u32() && dir_uid != ROOT_UID {
        return Err(SocketSetupError::BaseDirectoryPermissionsInvalid);
    }
    Ok(())
}

fn check_run_dir_metadata(meta: ObservedFileMeta) -> Result<UserId, SocketSetupError> {
    match meta.kind {
        ObservedFileKind::Symlink => return Err(SocketSetupError::SymlinkNotAllowed),
        ObservedFileKind::Directory => {}
        ObservedFileKind::Socket | ObservedFileKind::Other => {
            return Err(SocketSetupError::DirectoryPermissionsInvalid);
        }
    }
    if (meta.mode & MODE_PERMISSION_MASK) != DIRECTORY_MODE {
        return Err(SocketSetupError::DirectoryPermissionsInvalid);
    }
    Ok(meta.uid)
}

fn check_socket_metadata(
    meta: ObservedFileMeta,
    owner_uid: UserId,
) -> Result<(), SocketSetupError> {
    match meta.kind {
        ObservedFileKind::Socket => {}
        ObservedFileKind::Symlink | ObservedFileKind::Directory | ObservedFileKind::Other => {
            return Err(SocketSetupError::SocketPermissionsInvalid);
        }
    }
    if (meta.mode & MODE_PERMISSION_MASK) != SOCKET_MODE {
        return Err(SocketSetupError::SocketPermissionsInvalid);
    }
    if meta.uid != owner_uid {
        return Err(SocketSetupError::SocketPermissionsInvalid);
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
pub(crate) struct SocketCleanupGuard {
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

    pub(crate) fn cleanup(&mut self) {
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

fn collect_ancestor_prefixes(base_dir: &Path) -> Vec<PathBuf> {
    let mut prefix = PathBuf::new();
    let mut prefixes = Vec::new();
    for component in base_dir.components() {
        prefix.push(component.as_os_str());
        prefixes.push(prefix.clone());
    }
    prefixes
}

fn validate_base_dir_precreate(run_dir: &Path) -> Result<(), SocketSetupError> {
    let Some(base_dir) = run_dir.parent() else {
        return Err(SocketSetupError::MissingParentDirectory);
    };
    for ancestor in collect_ancestor_prefixes(base_dir) {
        let meta = fs::symlink_metadata(&ancestor)
            .map_err(|_error| SocketSetupError::InvalidBaseDirectory)?;
        check_ancestor_dir_precreate(ObservedFileMeta::from_metadata(&meta))?;
    }
    Ok(())
}

fn verify_base_dir_ownership(run_dir: &Path, owner_uid: UserId) -> Result<(), SocketSetupError> {
    let Some(base_dir) = run_dir.parent() else {
        return Err(SocketSetupError::MissingParentDirectory);
    };
    for ancestor in collect_ancestor_prefixes(base_dir) {
        let meta = fs::symlink_metadata(&ancestor)
            .map_err(|_error| SocketSetupError::InvalidBaseDirectory)?;
        check_ancestor_dir_ownership(ObservedFileMeta::from_metadata(&meta), owner_uid)?;
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
    let pre_meta = fs::symlink_metadata(run_dir)
        .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
    if pre_meta.file_type().is_symlink() {
        return Err(SocketSetupError::SymlinkNotAllowed);
    }
    fs::set_permissions(run_dir, Permissions::from_mode(DIRECTORY_MODE))
        .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
    let dir_meta = fs::symlink_metadata(run_dir)
        .map_err(|_error| SocketSetupError::DirectoryPermissionsInvalid)?;
    check_run_dir_metadata(ObservedFileMeta::from_metadata(&dir_meta))
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
    check_socket_metadata(ObservedFileMeta::from_metadata(&socket_meta), owner_uid)?;
    Ok(listener)
}

#[derive(Debug)]
pub(crate) struct BoundRunSocket {
    listener: UnixListener,
    owner_uid: UserId,
    run_dir: PathBuf,
    socket_path: PathBuf,
    cleanup: SocketCleanupGuard,
}

impl BoundRunSocket {
    pub(crate) fn bind(spec: &RunSocketSpec) -> Result<Self, SocketSetupError> {
        validate_socket_path_length(spec.socket_path())?;
        validate_base_dir_precreate(spec.run_dir())?;
        create_run_dir(spec.run_dir())?;

        let mut cleanup = SocketCleanupGuard::new(
            spec.run_dir().to_path_buf(),
            spec.socket_path().to_path_buf(),
        );
        let owner_uid = verify_run_dir_0700(spec.run_dir())?;
        verify_base_dir_ownership(spec.run_dir(), owner_uid)?;
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
    pub(crate) fn into_parts(self) -> (UnixListener, UserId, PathBuf, PathBuf, SocketCleanupGuard) {
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

    use crate::peer::UserId;

    use super::{
        MAX_UNIX_SOCKET_PATH_BYTES, ObservedFileKind, ObservedFileMeta, RunSocketSpec,
        SocketSetupError, check_ancestor_dir_ownership, check_run_dir_metadata,
        check_socket_metadata, create_run_dir, validate_socket_path_length,
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
                RunSocketSpec::explicit(PathBuf::from("/private/tmp/r1"), invalid),
                Err(SocketSetupError::InvalidSocketName),
                "잘못된 소켓 파일 이름 거부: {invalid:?}"
            );
        }
        assert_eq!(
            RunSocketSpec::explicit(PathBuf::from("/"), "m.sock"),
            Err(SocketSetupError::MissingParentDirectory),
            "부모 폴더 없는 루트 경로 거부"
        );
        for bad_dir in ["relative/r1", "/private/tmp/a/../r1", "/private/tmp/./r1"] {
            assert_eq!(
                RunSocketSpec::explicit(PathBuf::from(bad_dir), "m.sock"),
                Err(SocketSetupError::InvalidRunDirectory),
                "상대·상위 이동 경로 거부: {bad_dir}"
            );
        }
        let valid =
            RunSocketSpec::explicit(PathBuf::from("/private/tmp/r1"), "m.sock").expect("유효 스펙");
        assert_eq!(
            valid.run_dir(),
            Path::new("/private/tmp/r1"),
            "run_dir 보존"
        );
        assert_eq!(
            valid.socket_path(),
            Path::new("/private/tmp/r1/m.sock"),
            "socket_path 보존"
        );
    }

    #[test]
    fn ancestor_dir_metadata_checks_enforce_owner_and_permissions() {
        let self_uid = UserId::new(501_u32);
        let root_uid = UserId::new(0_u32);
        let other_uid = UserId::new(502_u32);

        let cases = [
            (
                ObservedFileKind::Directory,
                self_uid,
                0o700_u32,
                Ok(()),
                "본인 0700 허용",
            ),
            (
                ObservedFileKind::Directory,
                root_uid,
                0o1777_u32,
                Ok(()),
                "루트 sticky 허용",
            ),
            (
                ObservedFileKind::Directory,
                self_uid,
                0o777_u32,
                Err(SocketSetupError::BaseDirectoryPermissionsInvalid),
                "sticky 없는 0777 거부",
            ),
            (
                ObservedFileKind::Directory,
                self_uid,
                0o775_u32,
                Err(SocketSetupError::BaseDirectoryPermissionsInvalid),
                "sticky 없는 0775 거부",
            ),
            (
                ObservedFileKind::Directory,
                other_uid,
                0o755_u32,
                Err(SocketSetupError::BaseDirectoryPermissionsInvalid),
                "타 사용자 소유 거부",
            ),
            (
                ObservedFileKind::Symlink,
                self_uid,
                0o777_u32,
                Err(SocketSetupError::SymlinkNotAllowed),
                "심볼릭 링크 거부",
            ),
        ];
        for (kind, uid, mode, expected, msg) in cases {
            assert_eq!(
                check_ancestor_dir_ownership(ObservedFileMeta { kind, uid, mode }, self_uid),
                expected,
                "{msg}"
            );
        }
    }

    #[test]
    fn run_dir_metadata_checks_enforce_mode_and_type() {
        let self_uid = UserId::new(501_u32);
        assert_eq!(
            check_run_dir_metadata(ObservedFileMeta {
                kind: ObservedFileKind::Directory,
                uid: self_uid,
                mode: 0o700_u32,
            }),
            Ok(self_uid),
            "0700 실행 폴더 허용"
        );
        assert_eq!(
            check_run_dir_metadata(ObservedFileMeta {
                kind: ObservedFileKind::Directory,
                uid: self_uid,
                mode: 0o755_u32,
            }),
            Err(SocketSetupError::DirectoryPermissionsInvalid),
            "0755 실행 폴더 거부"
        );
        assert_eq!(
            check_run_dir_metadata(ObservedFileMeta {
                kind: ObservedFileKind::Symlink,
                uid: self_uid,
                mode: 0o700_u32,
            }),
            Err(SocketSetupError::SymlinkNotAllowed),
            "심볼릭 링크 실행 폴더 거부"
        );
        assert_eq!(
            create_run_dir(Path::new("/nonexistent-stock-copilot-parent/r1")),
            Err(SocketSetupError::DirectoryCreateFailed),
            "부모 부재 오류는 DirectoryCreateFailed"
        );
    }

    #[test]
    fn socket_metadata_checks_enforce_mode_type_and_owner() {
        let self_uid = UserId::new(501_u32);
        let other_uid = UserId::new(502_u32);
        assert_eq!(
            check_socket_metadata(
                ObservedFileMeta {
                    kind: ObservedFileKind::Socket,
                    uid: self_uid,
                    mode: 0o600_u32,
                },
                self_uid,
            ),
            Ok(()),
            "본인 소유 0600 소켓 허용"
        );
        for (kind, uid, mode, msg) in [
            (ObservedFileKind::Other, self_uid, 0o600_u32, "비소켓 거부"),
            (ObservedFileKind::Socket, self_uid, 0o660_u32, "0660 거부"),
            (
                ObservedFileKind::Socket,
                other_uid,
                0o600_u32,
                "타 소유자 거부",
            ),
        ] {
            assert_eq!(
                check_socket_metadata(ObservedFileMeta { kind, uid, mode }, self_uid),
                Err(SocketSetupError::SocketPermissionsInvalid),
                "{msg}"
            );
        }
    }
}
