use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

pub(crate) fn repository_root() -> PathBuf {
    MetadataCommand::new()
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into_std_path_buf()
}

pub(crate) fn fresh_directory(root: &Path, name: &str) -> PathBuf {
    let directory = root.join("target").join("harness").join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("remove previous harness directory");
    }
    fs::create_dir_all(&directory).expect("create harness directory");
    directory
}

pub(crate) fn generate_lockfile(manifest: &Path) {
    let status = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(manifest)
        .status()
        .expect("cargo generate-lockfile");
    assert!(
        status.success(),
        "cargo generate-lockfile failed for {}",
        manifest.display()
    );
}
