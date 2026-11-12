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
