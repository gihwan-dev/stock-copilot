use std::collections::BTreeSet;
use std::fs;

use crate::harness_workspace_tests::repository_root;

const PRETTIER_IGNORE: &str = ".prettierignore";

const EXPECTED_IGNORED: [&str; 1] = ["harness/arch-check/vendor/"];

#[test]
fn prettier_ignores_only_the_vendored_crate_stand_ins() {
    let text =
        fs::read_to_string(repository_root().join(PRETTIER_IGNORE)).expect("read .prettierignore");
    let entries: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let unique: BTreeSet<&str> = entries.iter().copied().collect();
    assert_eq!(
        entries.len(),
        unique.len(),
        "duplicate entries: {entries:?}"
    );
    assert_eq!(
        unique,
        BTreeSet::from(EXPECTED_IGNORED),
        "{PRETTIER_IGNORE} must list exactly the expected paths"
    );
}
