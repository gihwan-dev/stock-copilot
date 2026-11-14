use std::path::Path;
use std::process::Output;

use crate::git_base;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TreeMatch {
    Equal,
    Different,
    Unavailable,
}

pub(super) fn compare_outputs(merged: &Output, indexed: &Output) -> TreeMatch {
    if merged.status.code() == Some(1_i32) {
        return TreeMatch::Different;
    }
    if !merged.status.success() || !indexed.status.success() {
        return TreeMatch::Unavailable;
    }
    let tree = String::from_utf8_lossy(&merged.stdout);
    let index = String::from_utf8_lossy(&indexed.stdout);
    if !git_base::is_commit_id(tree.trim()) || !git_base::is_commit_id(index.trim()) {
        return TreeMatch::Unavailable;
    }
    if tree.trim() == index.trim() {
        TreeMatch::Equal
    } else {
        TreeMatch::Different
    }
}

pub(super) fn tree_match(root: &Path, tip: &str) -> Result<TreeMatch, String> {
    let mut merge = git_base::git(root);
    merge.args(["merge-tree", "--write-tree", "HEAD", tip]);
    let merged = merge
        .output()
        .map_err(|error| format!("merge-tree: {error}"))?;
    let mut index = git_base::git(root);
    index.arg("write-tree");
    let indexed = index
        .output()
        .map_err(|error| format!("write-tree: {error}"))?;
    Ok(compare_outputs(&merged, &indexed))
}

#[cfg(test)]
mod tests {
    use super::{TreeMatch, compare_outputs};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    const FIRST: &str = "1111111111111111111111111111111111111111\n";
    const SECOND: &str = "2222222222222222222222222222222222222222\n";

    fn result(code: i32, text: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8_i32),
            stdout: text.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn h44_review_tree_match_requires_successful_conflict_free_equal_trees() {
        assert_eq!(
            compare_outputs(&result(0_i32, FIRST), &result(0_i32, FIRST)),
            TreeMatch::Equal
        );
        assert_eq!(
            compare_outputs(&result(0_i32, FIRST), &result(0_i32, SECOND)),
            TreeMatch::Different
        );
        assert_eq!(
            compare_outputs(&result(1_i32, FIRST), &result(0_i32, FIRST)),
            TreeMatch::Different
        );
        assert_eq!(
            compare_outputs(&result(129_i32, FIRST), &result(0_i32, FIRST)),
            TreeMatch::Unavailable
        );
        assert_eq!(
            compare_outputs(&result(128_i32, FIRST), &result(0_i32, FIRST)),
            TreeMatch::Unavailable
        );
        assert_eq!(
            compare_outputs(&result(0_i32, FIRST), &result(128_i32, FIRST)),
            TreeMatch::Unavailable
        );
        assert_eq!(
            compare_outputs(&result(0_i32, ""), &result(0_i32, FIRST)),
            TreeMatch::Unavailable
        );
        assert_eq!(
            compare_outputs(&result(0_i32, FIRST), &result(0_i32, "")),
            TreeMatch::Unavailable
        );
        assert_eq!(
            compare_outputs(&result(0_i32, "not-a-tree"), &result(0_i32, "not-a-tree")),
            TreeMatch::Unavailable
        );
    }
}
