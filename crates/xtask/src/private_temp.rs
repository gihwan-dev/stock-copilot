#[cfg(test)]
mod tests;

use std::fs::{self, DirBuilder, Metadata};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const PRIVATE_MODE: u32 = 0o700;

const PERMISSION_BITS: u32 = 0o777;

const ATTEMPTS: usize = 16;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct PrivateDirectory {
    root: PathBuf,
    owner: u32,
}

fn unpredictable_name(prefix: &str) -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u64(SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let first = hasher.finish();
    let mut second = RandomState::new().build_hasher();
    second.write_u64(first);
    format!("{prefix}-{first:016x}{:016x}", second.finish())
}

fn plain_directory(path: &Path) -> Result<Metadata, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{}: 심볼릭 링크다", path.display()));
    }
    if !metadata.is_dir() {
        return Err(format!("{}: 폴더가 아니다", path.display()));
    }
    Ok(metadata)
}

fn private_directory(path: &Path, owner: u32) -> Result<(), String> {
    let metadata = plain_directory(path)?;
    if metadata.uid() != owner {
        return Err(format!("{}: 현재 사용자 소유가 아니다", path.display()));
    }
    if metadata.permissions().mode() & PERMISSION_BITS != PRIVATE_MODE {
        return Err(format!("{}: 권한이 0700이 아니다", path.display()));
    }
    Ok(())
}

fn create_private(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(PRIVATE_MODE).create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_MODE))
}

impl PrivateDirectory {
    pub(crate) fn create(prefix: &str) -> Result<Self, String> {
        Self::create_in(&std::env::temp_dir(), prefix)
    }

    pub(crate) fn create_in(base: &Path, prefix: &str) -> Result<Self, String> {
        let parent =
            fs::canonicalize(base).map_err(|error| format!("{}: {error}", base.display()))?;
        plain_directory(&parent)?;
        for _ in 0..ATTEMPTS {
            let root = parent.join(unpredictable_name(prefix));
            match create_private(&root) {
                Ok(()) => {
                    let owner = plain_directory(&root)?.uid();
                    private_directory(&root, owner)?;
                    return Ok(Self { root, owner });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(format!("{}: {error}", root.display())),
            }
        }
        Err(format!(
            "{}: 고유한 임시 폴더 이름을 만들지 못했다",
            parent.display()
        ))
    }

    pub(crate) fn child(&self, name: &str) -> Result<PathBuf, String> {
        private_directory(&self.root, self.owner)?;
        let child = self.root.join(name);
        create_private(&child).map_err(|error| format!("{}: {error}", child.display()))?;
        private_directory(&child, self.owner)?;
        Ok(child)
    }

    pub(crate) fn remove(self) -> Result<(), String> {
        plain_directory(&self.root)?;
        fs::remove_dir_all(&self.root).map_err(|error| format!("{}: {error}", self.root.display()))
    }
}
