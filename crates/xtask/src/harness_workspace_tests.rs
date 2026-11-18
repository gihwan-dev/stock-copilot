use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

use crate::private_temp::PrivateDirectory;

thread_local! {
    static TEST_DIRECTORIES: Cell<Vec<TestDirectory>> = const { Cell::new(Vec::new()) };
}

struct TestDirectory(Option<PrivateDirectory>);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Some(directory) = self.0.take()
            && let Err(error) = directory.remove()
        {
            panic!("temporary fixture cleanup failed: {error}");
        }
    }
}

fn tracked_child(directory: PrivateDirectory, name: &str) -> PathBuf {
    let owner = TestDirectory(Some(directory));
    let child = owner
        .0
        .as_ref()
        .expect("directory")
        .child(name)
        .expect("fixture child");
    TEST_DIRECTORIES.with(|directories| {
        let mut entries = directories.take();
        entries.push(owner);
        directories.set(entries);
    });
    child
}

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

pub(crate) fn unique_directory(root: &Path, name: &str) -> PathBuf {
    let parent = root.join("target").join("harness");
    fs::create_dir_all(&parent).expect("create harness parent");
    let directory = PrivateDirectory::create_in(&parent, name).expect("unique harness directory");
    tracked_child(directory, "fixture")
}

pub(crate) fn outside_git_directory() -> PathBuf {
    let directory = PrivateDirectory::create("stock-copilot-harness-git")
        .expect("create private git directory");
    tracked_child(directory, "git")
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

#[test]
fn outside_git_fixture_is_removed_when_its_test_thread_finishes() {
    let parent = std::thread::spawn(|| {
        let path = outside_git_directory();
        assert!(path.is_dir());
        path.parent().expect("parent").to_path_buf()
    })
    .join()
    .expect("fixture thread");
    let remains = parent.exists();
    if remains {
        fs::remove_dir_all(&parent).expect("remove test parent");
    }
    assert!(!remains, "temporary git parent remains");
}

#[test]
fn outside_git_fixture_is_removed_when_its_test_thread_panics() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let path = outside_git_directory();
        sender
            .send(path.parent().expect("parent").to_path_buf())
            .expect("send parent");
        panic!("fixture failure");
    });
    let parent = receiver.recv().expect("fixture parent");
    assert!(thread.join().is_err());
    let remains = parent.exists();
    if remains {
        fs::remove_dir_all(&parent).expect("remove test parent");
    }
    assert!(!remains, "temporary git parent remains after panic");
}
