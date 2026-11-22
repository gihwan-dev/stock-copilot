use std::fs;
use std::io;
use std::path::Path;

use super::{PrivateDirectory, temporary_base};

pub(crate) fn create_owned_in(base: &Path, prefix: &str) -> Result<PrivateDirectory, String> {
    PrivateDirectory::create_owned(base, prefix)
}

pub(crate) fn create_owned_temporary(prefix: &str) -> Result<PrivateDirectory, String> {
    PrivateDirectory::create_owned(&temporary_base(), prefix)
}

pub(crate) fn remove_if_present(directory: PrivateDirectory) -> Result<(), String> {
    match fs::symlink_metadata(&directory.root) {
        Ok(_) => directory.remove(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", directory.root.display())),
    }
}
