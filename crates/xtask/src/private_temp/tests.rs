use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

use super::PrivateDirectory;

fn scratch() -> PrivateDirectory {
    PrivateDirectory::create("stock-copilot-private-test").expect("create private directory")
}

#[test]
fn private_directories_are_unique_owned_and_closed_to_others() {
    let first = scratch();
    let second = scratch();
    assert_ne!(first.root.as_path(), second.root.as_path(), "names differ");
    for directory in [&first, &second] {
        let metadata = fs::symlink_metadata(directory.root.as_path()).expect("metadata");
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
        let child = directory.child("git").expect("child");
        let child_metadata = fs::symlink_metadata(&child).expect("child metadata");
        assert_eq!(child_metadata.permissions().mode() & 0o777, 0o700);
        assert_eq!(child_metadata.uid(), metadata.uid());
    }
    let root = first.root.as_path().to_path_buf();
    first.remove().expect("remove first");
    second.remove().expect("remove second");
    fs::symlink_metadata(&root).expect_err("removed");
}

#[test]
fn a_planted_child_link_fails_and_its_target_is_untouched() {
    let directory = scratch();
    let outside = scratch();
    fs::write(outside.root.as_path().join("kept.txt"), "kept\n").expect("write outside file");
    symlink(outside.root.as_path(), directory.root.as_path().join("git")).expect("plant link");
    directory.child("git").expect_err("planted link");
    directory.remove().expect("remove directory");
    assert_eq!(
        fs::read_to_string(outside.root.as_path().join("kept.txt")).expect("outside file survives"),
        "kept\n"
    );
    outside.remove().expect("remove outside");
}

#[test]
fn a_root_swapped_for_a_link_or_opened_to_others_fails() {
    let directory = scratch();
    let outside = scratch();
    let moved = outside.root.as_path().join("moved");
    fs::rename(directory.root.as_path(), &moved).expect("move root away");
    symlink(outside.root.as_path(), directory.root.as_path()).expect("plant link at root");
    directory.child("git").expect_err("root is a link");
    PrivateDirectory {
        root: directory.root.clone(),
        owner: directory.owner,
    }
    .remove()
    .expect_err("removing through a link");
    fs::symlink_metadata(&outside.root).expect("link target kept");
    fs::remove_file(directory.root.as_path()).expect("remove planted link");
    fs::rename(&moved, directory.root.as_path()).expect("move root back");
    fs::set_permissions(directory.root.as_path(), fs::Permissions::from_mode(0o755))
        .expect("open root");
    directory.child("git").expect_err("root open to others");
    directory.remove().expect("remove directory");
    outside.remove().expect("remove outside");
}

#[test]
fn a_base_that_is_not_a_directory_fails() {
    let directory = scratch();
    let file = directory.root.as_path().join("file");
    fs::write(&file, "x\n").expect("write file");
    PrivateDirectory::create_in(&file, "probe").expect_err("base is a file");
    directory.remove().expect("remove directory");
}

fn age(path: &std::path::Path, by: std::time::Duration) {
    let modified = fs::symlink_metadata(path)
        .and_then(|metadata| metadata.modified())
        .expect("modified time");
    fs::File::open(path)
        .and_then(|file| file.set_modified(modified.checked_sub(by).expect("earlier time")))
        .expect("age directory");
}

#[test]
fn creating_removes_only_stale_private_folders_of_the_same_prefix() {
    let prefix = "stock-copilot-private-stale-test";
    let two_hours = std::time::Duration::from_hours(2);
    let stale = PrivateDirectory::create(prefix).expect("stale");
    age(&stale.root, two_hours);
    let recent = PrivateDirectory::create(prefix).expect("recent");
    let outside = scratch();
    fs::write(outside.root.join("kept.txt"), "kept\n").expect("write outside file");
    let parent = stale.root.parent().expect("parent").to_path_buf();
    let linked = parent.join(format!("{prefix}-planted-link"));
    symlink(&outside.root, &linked).expect("plant link");
    let open = parent.join(format!("{prefix}-open-folder"));
    fs::create_dir_all(&open).expect("open folder");
    fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).expect("open mode");
    age(&open, two_hours);
    let fresh = PrivateDirectory::create(prefix).expect("fresh");
    fs::symlink_metadata(&stale.root).expect_err("stale folder removed");
    fs::symlink_metadata(&recent.root).expect("recent folder kept");
    fs::symlink_metadata(&linked).expect("planted link kept");
    assert_eq!(
        fs::read_to_string(outside.root.join("kept.txt")).expect("link target kept"),
        "kept\n"
    );
    fs::symlink_metadata(&open).expect("open folder kept");
    fs::remove_file(&linked).expect("remove link");
    fs::remove_dir(&open).expect("remove open folder");
    recent.remove().expect("remove recent");
    fresh.remove().expect("remove fresh");
    outside.remove().expect("remove outside");
}
