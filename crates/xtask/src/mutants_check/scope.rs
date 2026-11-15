use super::Scope;
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn matches_target(glob: &str, file: &str) -> bool {
    glob.strip_suffix("/**/*.rs").map_or_else(
        || glob == file,
        |prefix| {
            file.starts_with(&format!("{prefix}/"))
                && Path::new(file).extension().is_some_and(|ext| ext == "rs")
        },
    )
}

pub(super) fn diff_scope(
    root: &Path,
    _base: &str,
    globs: &[String],
    changed: &BTreeSet<String>,
) -> Scope {
    Scope::Files(
        changed
            .iter()
            .filter(|file| {
                root.join(file).is_file() && globs.iter().any(|glob| matches_target(glob, file))
            })
            .cloned()
            .collect(),
    )
}

pub(super) fn jobs_for(cores: usize, override_value: Option<&str>) -> Result<usize, String> {
    let maximum = (cores.checked_div(2).unwrap_or(1)).clamp(1, 6);
    match override_value {
        None => Ok(maximum),
        Some(value) => value
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then_some(value)
            .and_then(|digits| digits.parse::<usize>().ok())
            .filter(|jobs| (1..=maximum).contains(jobs))
            .ok_or_else(|| {
                format!("STOCK_COPILOT_MUTANTS_JOBS는 ASCII 정수 1..={maximum}이어야 한다: {value}")
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::{diff_scope, jobs_for};
    use crate::harness_workspace_tests::{fresh_directory, repository_root};
    use crate::mutants_check::Scope;
    use std::collections::BTreeSet;
    #[test]
    fn changed_src_files_are_selected_without_reference_expansion() {
        let root = fresh_directory(&repository_root(), "h46-scope");
        std::fs::create_dir_all(root.join("crates/domain/src")).expect("directory");
        for name in ["a.rs", "b.rs", "lib.rs"] {
            std::fs::write(root.join("crates/domain/src").join(name), "").expect("source");
        }
        let paths = [
            "crates/domain/src/a.rs",
            "crates/domain/src/lib.rs",
            "crates/domain/tests/test.rs",
            "crates/domain/src/deleted.rs",
            "Cargo.toml",
        ]
        .map(String::from)
        .into();
        assert_eq!(
            diff_scope(
                &root,
                "base",
                &[String::from("crates/domain/src/**/*.rs")],
                &paths
            ),
            Scope::Files(
                ["crates/domain/src/a.rs", "crates/domain/src/lib.rs"]
                    .map(String::from)
                    .into()
            )
        );
        assert_eq!(
            diff_scope(
                &root,
                "base",
                &[String::from("crates/domain/src/**/*.rs")],
                &BTreeSet::from([String::from("crates/domain/tests/a.rs")])
            ),
            Scope::Files(BTreeSet::new())
        );
    }
    #[test]
    fn bounded_parallelism_rejects_injected_or_excessive_values() {
        assert_eq!(jobs_for(1, None), Ok(1));
        assert_eq!(jobs_for(64, None), Ok(6));
        assert_eq!(jobs_for(4, Some("2")), Ok(2));
        for value in ["0", "3", "-1", " 1", "1;true"] {
            assert!(
                jobs_for(4, Some(value))
                    .expect_err("reject invalid jobs")
                    .contains("ASCII")
            );
        }
    }
}
