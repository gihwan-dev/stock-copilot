use super::{Scope, test_packages};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetPlatform {
    All,
    Linux,
    Macos,
}

impl TargetPlatform {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "all" => Ok(Self::All),
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::Macos),
            _ => Err(format!(
                "변이 대상 플랫폼은 all, linux, macos 중 하나여야 한다: {value}"
            )),
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Linux => "linux",
            Self::Macos => "macos",
        }
    }
}

pub(super) fn filter_platform(
    root: &Path,
    files: BTreeSet<String>,
    platform: TargetPlatform,
) -> Result<BTreeSet<String>, String> {
    if platform == TargetPlatform::All {
        for file in &files {
            let _package = test_packages::package_of_file(file)?;
        }
        return Ok(files);
    }
    let excluded = crate::test_diff::linux_excluded_crates(root)?;
    let mut selected = BTreeSet::new();
    for file in files {
        let package = test_packages::package_of_file(&file)?;
        let keep = match platform {
            TargetPlatform::Linux => !excluded.contains(package),
            TargetPlatform::Macos => excluded.contains(package),
            TargetPlatform::All => true,
        };
        if keep {
            selected.insert(file);
        }
    }
    Ok(selected)
}

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
