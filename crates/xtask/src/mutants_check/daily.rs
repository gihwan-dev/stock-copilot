use super::{IgnoreFiles, Outcomes, Scope};
use std::io::Write;
use std::path::Path;

pub(super) fn catalog(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let output = root.join(super::OUTPUT_DIRECTORY);
    std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    std::fs::write(output.join(super::catalog::FILE), bytes).map_err(|error| error.to_string())
}

fn base(root: &Path, value: &str, usage: crate::git_base::BaseUse) -> Result<String, String> {
    if value != "initial" {
        return crate::git_base::resolve_base_for(root, value, usage);
    }
    let mut command = crate::git_base::git(root);
    command.args(["hash-object", "-w", "-t", "tree", "--stdin"]);
    command.stdin(std::process::Stdio::null());
    let output = crate::git_base::output(command, "empty tree")?;
    String::from_utf8(output)
        .map(|text| text.trim().to_owned())
        .map_err(|error| error.to_string())
}

fn empty(root: &Path) -> Result<Outcomes, String> {
    let output = root.join(super::OUTPUT_DIRECTORY).join("mutants.out");
    std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    for name in ["missed.txt", "timeout.txt", "caught.txt", "unviable.txt"] {
        std::fs::write(output.join(name), "").map_err(|error| error.to_string())?;
    }
    std::fs::write(output.join("mutants.json"), "[]").map_err(|error| error.to_string())?;
    writeln!(std::io::stdout().lock(), "mutants: 변경 없음").map_err(|error| error.to_string())?;
    Ok(Outcomes::default())
}

#[derive(PartialEq, Eq)]
enum Execution {
    Plan,
    Whole,
    Sharded(String),
}

type Findings = std::collections::BTreeSet<crate::check_support::Finding<super::Rule>>;

pub(super) fn valid_shard(value: &str) -> bool {
    (1_usize..=8).any(|parts| (0..parts).any(|index| value == format!("{index}/{parts}")))
}

pub(super) fn dispatch(
    root: &Path,
    arguments: &[String],
    usage: crate::git_base::BaseUse,
) -> Result<Findings, String> {
    match arguments {
        [flag, base] if flag == "--daily" => check(root, base, usage),
        [flag, base] if flag == "--daily-plan" => run(root, base, usage, &Execution::Plan),
        [flag, base, option, shard]
            if flag == "--daily" && option == "--shard" && valid_shard(shard) =>
        {
            run(root, base, usage, &Execution::Sharded(shard.clone()))
        }
        _ => Err(String::from(
            "일일 변이 사용법: --daily[-plan] <기준> [--shard <0/1..7/8>]",
        )),
    }
}

pub(super) fn check(
    root: &Path,
    value: &str,
    usage: crate::git_base::BaseUse,
) -> Result<Findings, String> {
    run(root, value, usage, &Execution::Whole)
}

fn run(
    root: &Path,
    value: &str,
    usage: crate::git_base::BaseUse,
    execution: &Execution,
) -> Result<std::collections::BTreeSet<crate::check_support::Finding<super::Rule>>, String> {
    let commit = base(root, value, usage)?;
    let changed = crate::git_base::changed_paths(root, &commit)?;
    let Scope::Files(files) =
        super::diff_scope(root, &commit, &super::target_globs(root)?, &changed)
    else {
        return Err(String::from("일일 변이 파일 선택 실패"));
    };
    let selected_shard = match execution {
        Execution::Sharded(shard) => shard.as_str(),
        Execution::Plan | Execution::Whole => "0/1",
    };
    let scope = match execution {
        Execution::Sharded(shard) => Scope::DailySharded(files, shard.clone()),
        Execution::Plan | Execution::Whole => Scope::DailyFiles(files),
    };
    let output = super::prepare_output(root, &scope, IgnoreFiles::Respected, &[])?;
    for name in ["daily.json", super::catalog::FILE] {
        let path = output.join(name);
        if path.exists() {
            std::fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    let patch = super::diff_patch(root, &commit)?;
    let selected = super::list_changed_mutants(root, &scope, &patch)?;
    let mut head_command = crate::git_base::git(root);
    head_command.args(["rev-parse", "HEAD"]);
    let head = String::from_utf8(crate::git_base::output(head_command, "HEAD")?)
        .map_err(|error| error.to_string())?;
    let receipt = serde_json::json!({"base": if value == "initial" { value } else { &commit }, "head": head.trim(), "shard": selected_shard});
    std::fs::write(output.join("daily.json"), receipt.to_string())
        .map_err(|error| error.to_string())?;
    if execution == &Execution::Plan {
        return Ok(std::collections::BTreeSet::new());
    }
    let outcomes = if selected.is_empty() {
        empty(root)?
    } else {
        super::run_with_diff(root, &scope, IgnoreFiles::Respected, Some(&patch))?
    };
    super::evaluate(
        &outcomes,
        &super::load_survivors(root, super::SURVIVORS_FILE)?,
    )
}

#[cfg(test)]
mod tests {
    fn fixture() -> std::path::PathBuf {
        let root = crate::harness_workspace_tests::unique_directory(
            &crate::harness_workspace_tests::repository_root(),
            "h47-daily",
        );
        for (file, text) in [
            (
                "Cargo.toml",
                "[workspace]\nmembers=['crates/domain']\nresolver='3'\n",
            ),
            (
                "crates/domain/Cargo.toml",
                "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n",
            ),
            (
                "crates/domain/src/lib.rs",
                "pub fn yes() -> bool { true } #[test] fn catches() { assert!(yes()); }",
            ),
            (
                ".cargo/mutants.toml",
                "examine_globs=['crates/domain/src/**/*.rs']\n",
            ),
            ("harness/mutants/survivors.toml", "survivor=[]\n"),
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
            std::fs::write(path, text).expect("fixture");
        }
        crate::harness_workspace_tests::generate_lockfile(&root.join("Cargo.toml"));
        for args in [
            vec!["init"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--no-verify",
                "-m",
                "initial",
            ],
        ] {
            let mut command = crate::git_base::git(&root);
            command.args(args);
            crate::git_base::output(command, "fixture git").expect("git");
        }
        root
    }

    #[test]
    fn daily_quiet_window_records_empty_selection_and_clears_stale_results() {
        let root = fixture();
        let old = root
            .join(super::super::OUTPUT_DIRECTORY)
            .join("mutants.out");
        std::fs::create_dir_all(&old).expect("old directory");
        std::fs::write(old.join("outcomes.json"), "stale").expect("old result");
        assert!(
            super::check(&root, "HEAD", crate::git_base::BaseUse::Ancestor)
                .expect("quiet window")
                .is_empty()
        );
        assert!(!old.join("outcomes.json").exists());
        assert_eq!(
            std::fs::read_to_string(old.join("mutants.json")).expect("empty plan"),
            "[]"
        );
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("target/mutants/daily.json")).expect("receipt"),
        )
        .expect("json");
        assert_eq!(receipt["base"], receipt["head"]);
    }

    #[test]
    fn daily_initial_history_tests_root_sources_with_default_profile() {
        let root = fixture();
        assert!(
            super::check(&root, "initial", crate::git_base::BaseUse::Ancestor)
                .expect("initial mutation run")
                .is_empty()
        );
        let listed: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("target/mutants/catalog.json")).expect("catalog"),
        )
        .expect("json");
        assert_ne!(listed.as_array().expect("array").len(), 0);
        assert!(
            root.join("target/mutants/mutants.out/outcomes.json")
                .is_file()
        );
    }
    #[test]
    fn daily_reports_surviving_mutants_as_findings() {
        let root = fixture();
        std::fs::write(
            root.join("crates/domain/src/lib.rs"),
            "pub fn yes() -> bool { true } #[test] fn weak() { assert!(true); }",
        )
        .expect("weak fixture");
        let findings = super::check(&root, "initial", crate::git_base::BaseUse::Ancestor)
            .expect("completed mutation run");
        assert!(
            findings.iter().any(|finding| match finding.rule {
                super::super::Rule::Missed(_) => true,
                super::super::Rule::Timeout(_) => false,
            }),
            "missed mutants must fail evaluation"
        );
    }
    #[test]
    fn daily_shards_are_canonical_and_never_exceed_eight() {
        for total in 1_usize..=8 {
            for index in 0..total {
                assert!(super::valid_shard(&format!("{index}/{total}")));
            }
        }
        for shard in [
            "0/0", "0/9", "8/8", "00/8", "0/08", "-1/8", "0/128", "0/8;true", " 0/8",
        ] {
            assert!(!super::valid_shard(shard), "{shard}");
        }
    }

    #[test]
    fn daily_plan_never_runs_tests_and_shards_cover_the_actual_catalog() {
        let root = fixture();
        std::fs::write(root.join("crates/domain/src/lib.rs"), "pub fn yes() -> bool { true } #[test] fn catches() { std::fs::write(\"ran.txt\", \"test\").unwrap(); assert!(yes()); }").expect("marked fixture");
        let usage = crate::git_base::BaseUse::Ancestor;
        super::dispatch(
            &root,
            &[String::from("--daily-plan"), String::from("initial")],
            usage,
        )
        .expect("plan");
        assert!(!root.join("crates/domain/ran.txt").exists());
        let catalog: Vec<serde_json::Value> = serde_json::from_slice(
            &std::fs::read(root.join("target/mutants/catalog.json")).expect("catalog"),
        )
        .expect("array");
        assert_ne!(catalog.len(), 0);
        let mut union = Vec::new();
        for index in 0..2 {
            let args = [
                String::from("--daily"),
                String::from("initial"),
                String::from("--shard"),
                format!("{index}/2"),
            ];
            let findings = super::dispatch(&root, &args, usage).expect("shard run");
            assert!(findings.is_empty(), "{findings:?}");
            let selected: Vec<serde_json::Value> = serde_json::from_slice(
                &std::fs::read(root.join("target/mutants/mutants.out/mutants.json"))
                    .expect("planned"),
            )
            .expect("array");
            let start = index * catalog.len().div_ceil(2);
            let end = (start + catalog.len().div_ceil(2)).min(catalog.len());
            assert_eq!(selected, catalog[start.min(catalog.len())..end]);
            union.extend(selected);
        }
        assert_eq!(union, catalog);
    }
}
