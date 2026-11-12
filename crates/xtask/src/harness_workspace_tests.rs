use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

use crate::private_temp::PrivateDirectory;

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

pub(crate) fn outside_git_directory() -> PathBuf {
    PrivateDirectory::create("stock-copilot-harness-git")
        .and_then(|private| private.child("git"))
        .expect("create private git directory")
}

pub(crate) fn place_git_directory_outside(directory: &Path, arguments: &[&str]) {
    if arguments.first() != Some(&"init") {
        return;
    }
    let named = arguments
        .iter()
        .skip(1)
        .rfind(|argument| !argument.starts_with('-'));
    let work = named.map_or_else(|| directory.to_path_buf(), |name| directory.join(name));
    let link = if arguments.contains(&"--bare") {
        work
    } else {
        work.join(".git")
    };
    let target = outside_git_directory();
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).expect("create repository parent");
    }
    if fs::symlink_metadata(&link).is_ok() {
        fs::remove_file(&link).expect("remove previous git link");
    }
    std::os::unix::fs::symlink(&target, &link).expect("link git directory");
}

pub(crate) fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create directory");
    for listed in fs::read_dir(source).expect("read directory") {
        let entry = listed.expect("directory entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
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
