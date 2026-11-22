use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

use crate::private_temp::PrivateDirectory;
use crate::private_temp::fixture_support_tests::{
    create_owned_in, create_owned_temporary, remove_if_present,
};

thread_local! {
    static TEST_DIRECTORIES: Cell<TestDirectories> = const { Cell::new(TestDirectories(Vec::new())) };
}

struct TestDirectories(Vec<TestDirectory>);

impl Drop for TestDirectories {
    fn drop(&mut self) {
        while let Some(directory) = self.0.pop() {
            let _owner = directory;
        }
    }
}

struct TestDirectory(Option<PrivateDirectory>);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Some(directory) = self.0.take()
            && let Err(error) = remove_if_present(directory)
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
        let mut entries = directories.replace(TestDirectories(Vec::new()));
        entries.0.push(owner);
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
    unique_directory(root, name)
}

pub(crate) fn unique_directory(root: &Path, name: &str) -> PathBuf {
    let parent = root.join("target").join("harness");
    fs::create_dir_all(&parent).expect("create harness parent");
    let prefix = name.replace('/', "-");
    let directory = create_owned_in(&parent, &prefix).expect("unique harness directory");
    tracked_child(directory, "fixture")
}

pub(crate) fn outside_git_directory() -> PathBuf {
    let directory =
        create_owned_temporary("stock-copilot-harness-git").expect("create private git directory");
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
fn fresh_fixtures_with_the_same_name_are_independent_and_cleaned() {
    let paths = std::thread::spawn(|| {
        let root = repository_root();
        let name = format!("h53-fresh-isolation-{}", std::process::id());
        let first = fresh_directory(&root, &name);
        fs::write(first.join("owned"), "first").expect("first fixture");
        let second = fresh_directory(&root, &name);
        assert_ne!(first, second);
        assert_eq!(
            fs::read_to_string(first.join("owned")).expect("first survives"),
            "first"
        );
        assert!(second.is_dir());
        (first, second)
    })
    .join()
    .expect("fixture thread");
    assert!(!paths.0.exists());
    assert!(!paths.1.exists());
}

#[test]
fn fixture_cleanup_handles_nested_and_already_removed_paths() {
    let paths = std::thread::spawn(|| {
        let outer = unique_directory(&repository_root(), "h53-outer");
        let inner = fresh_directory(&outer, "h53-inner");
        let removed = fresh_directory(&outer, "h53-removed");
        fs::remove_dir_all(removed.parent().expect("fixture owner")).expect("remove owned fixture");
        assert!(inner.is_dir());
        (outer, inner)
    })
    .join()
    .expect("nested fixture cleanup");
    assert!(!paths.0.exists());
    assert!(!paths.1.exists());
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
