use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::git_base::{BaseUse, git, output};
use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

use super::scope::{TargetPlatform, filter_platform};
use super::{Scope, checked_scope, run_command, scope_for};

fn run_git(root: &Path, args: &[&str]) -> String {
    let mut command = git(root);
    command.args([
        "-c",
        "user.name=Harness",
        "-c",
        "user.email=harness@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ]);
    command.args(args);
    String::from_utf8(output(command, "h63 fixture git").expect("git"))
        .expect("utf8")
        .trim()
        .to_owned()
}

fn write_file(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().expect("parent")).expect("directory");
    fs::write(full, text).expect("write fixture");
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn owned_args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| String::from(*value)).collect()
}

fn read_json_value(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).expect("read json")).expect("parse json")
}

fn read_json_vec(path: &Path) -> Vec<serde_json::Value> {
    serde_json::from_slice(&fs::read(path).expect("read json")).expect("parse json array")
}

fn dual_platform_fixture(name: &str) -> (PathBuf, String) {
    let root = fresh_directory(&repository_root(), name);
    for (file, text) in [
        (
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"crates/domain\", \"crates/adapters/store-sqlite\"]\n",
        ),
        (
            "crates/domain/Cargo.toml",
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        (
            "crates/domain/src/lib.rs",
            "pub fn positive(value: i32) -> bool { value > 0 }\n#[cfg(test)] mod tests { #[test] fn domain_test() { assert!(super::positive(1)); assert!(!super::positive(0)); } }\n",
        ),
        (
            "crates/adapters/store-sqlite/Cargo.toml",
            "[package]\nname = \"store-sqlite\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        (
            "crates/adapters/store-sqlite/src/lib.rs",
            "pub fn non_zero(value: i32) -> bool { value != 0 }\n#[cfg(test)] mod tests { #[test] fn store_test() { assert!(super::non_zero(1)); assert!(!super::non_zero(0)); } }\n",
        ),
        (
            ".cargo/mutants.toml",
            "examine_globs = [\"crates/domain/src/**/*.rs\", \"crates/adapters/store-sqlite/src/**/*.rs\"]\n",
        ),
        ("harness/mutants/survivors.toml", "survivor = []\n"),
        (
            "harness/ci/linux/exclusions.json",
            "{\"crates\":[{\"name\":\"store-sqlite\",\"reason\":\"macOS only\"}],\"tests\":[]}\n",
        ),
    ] {
        write_file(&root, file, text);
    }
    generate_lockfile(&root.join("Cargo.toml"));
    run_git(&root, &["init", "--quiet", "-b", "master"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "--quiet", "-m", "baseline"]);
    let base = run_git(&root, &["rev-parse", "HEAD"]);
    (root, base)
}

#[test]
fn h63_platform_filter_routes_neutral_crates_to_linux_and_excluded_to_macos() {
    let (root, _) = dual_platform_fixture("h63-filter");
    let files = BTreeSet::from([
        String::from("crates/domain/src/lib.rs"),
        String::from("crates/application/src/lib.rs"),
        String::from("crates/order-execution/src/lib.rs"),
        String::from("crates/adapters/store-sqlite/src/lib.rs"),
    ]);
    assert_eq!(
        filter_platform(&root, files.clone(), TargetPlatform::Linux).expect("linux"),
        BTreeSet::from([
            String::from("crates/domain/src/lib.rs"),
            String::from("crates/application/src/lib.rs"),
            String::from("crates/order-execution/src/lib.rs"),
        ])
    );
    assert_eq!(
        filter_platform(&root, files.clone(), TargetPlatform::Macos).expect("macos"),
        BTreeSet::from([String::from("crates/adapters/store-sqlite/src/lib.rs")])
    );
    assert_eq!(
        filter_platform(&root, files, TargetPlatform::All).expect("all"),
        BTreeSet::from([
            String::from("crates/domain/src/lib.rs"),
            String::from("crates/application/src/lib.rs"),
            String::from("crates/order-execution/src/lib.rs"),
            String::from("crates/adapters/store-sqlite/src/lib.rs"),
        ])
    );
    filter_platform(
        &root,
        BTreeSet::from([String::from("crates/unknown/src/lib.rs")]),
        TargetPlatform::Linux,
    )
    .expect_err("unknown crate must fail closed");
    fs::remove_file(root.join("harness/ci/linux/exclusions.json")).expect("remove exclusions");
    filter_platform(
        &root,
        BTreeSet::from([String::from("crates/domain/src/lib.rs")]),
        TargetPlatform::Linux,
    )
    .expect_err("missing exclusions file must fail closed");
}

#[test]
fn h63_platform_cli_accepts_canonical_plan_and_shards_and_rejects_invalid() {
    let (root, base) = dual_platform_fixture("h63-cli");
    for (platform, expected) in [
        ("all", TargetPlatform::All),
        ("linux", TargetPlatform::Linux),
        ("macos", TargetPlatform::Macos),
    ] {
        for args in [
            owned_args(&["--in-diff-plan", &base, "--platform", platform]),
            owned_args(&[
                "--in-diff-plan",
                &base,
                "--platform",
                platform,
                "--linux-exclusions",
            ]),
        ] {
            assert_eq!(
                scope_for(&root, &args, BaseUse::Ancestor).expect("plan"),
                Scope::DiffPlatformPlan(BTreeSet::new(), expected)
            );
        }
    }
    for (platform, expected) in [
        ("linux", TargetPlatform::Linux),
        ("macos", TargetPlatform::Macos),
    ] {
        for shard in ["0/1", "0/2", "1/2", "7/8"] {
            let args = owned_args(&["--in-diff", &base, "--platform", platform, "--shard", shard]);
            assert_eq!(
                scope_for(&root, &args, BaseUse::Ancestor).expect("shard"),
                Scope::DiffPlatformSharded(BTreeSet::new(), expected, String::from(shard))
            );
        }
    }
    for invalid in [
        &["--in-diff", &base, "--platform", "all", "--shard", "0/1"][..],
        &[
            "--in-diff",
            &base,
            "--platform",
            "windows",
            "--shard",
            "0/1",
        ][..],
        &["--in-diff", &base, "--platform", "linux", "--shard", "0/9"][..],
        &["--in-diff", &base, "--platform", "linux", "--shard", "8/8"][..],
        &["--in-diff", &base, "--platform", "linux", "--shard", "00/2"][..],
        &[
            "--in-diff",
            &base,
            "--platform",
            "linux",
            "--shard",
            "0/128",
        ][..],
        &["--in-diff", &base, "--linux-exclusions"][..],
    ] {
        scope_for(&root, &owned_args(invalid), BaseUse::Ancestor)
            .expect_err("invalid platform args");
    }
}

fn setup_h62_bundle_merge(root: &Path, base: &str, after: &str) -> String {
    let before = fs::read_to_string(root.join("crates/adapters/store-sqlite/src/lib.rs"))
        .expect("before store");
    run_git(root, &["checkout", "--quiet", "-b", "feature", base]);
    write_file(root, "crates/adapters/store-sqlite/src/lib.rs", after);
    run_git(root, &["add", "crates/adapters/store-sqlite/src/lib.rs"]);
    run_git(root, &["commit", "--quiet", "-m", "feature store test"]);
    let feature_head = run_git(root, &["rev-parse", "HEAD"]);
    run_git(root, &["checkout", "--quiet", "master"]);
    let bundle_toml = format!(
        "task = \"H-63\"\nbase_commit = \"{base}\"\nmerge_head = \"{feature_head}\"\nreason = \"loop assertion review\"\nfindings = [\n    \"test-diff:modified:store-sqlite tests::store_test\",\n]\n\n[[file]]\npath = \"crates/adapters/store-sqlite/src/lib.rs\"\nbefore_sha256 = \"{}\"\nafter_sha256 = \"{}\"\n",
        sha256_hex(&before),
        sha256_hex(after),
    );
    write_file(root, "harness/test-diff/bundles/H-63.toml", &bundle_toml);
    run_git(root, &["add", "harness/test-diff/bundles/H-63.toml"]);
    run_git(root, &["commit", "--quiet", "-m", "add H-63 bundle"]);
    let bundle_commit = run_git(root, &["rev-parse", "HEAD"]);
    run_git(
        root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    let message = "Merge H-63\n\nTest-Diff-Bundle: harness/test-diff/bundles/H-63.toml\nTest-Diff-Findings: 1";
    run_git(
        root,
        &["merge", "--quiet", "--no-ff", "-m", message, &feature_head],
    );
    run_git(
        root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    bundle_commit
}

#[test]
fn h63_linux_precondition_accepts_h62_bundle_and_rejects_unapproved_change() {
    let (root, base) = dual_platform_fixture("h63-bundle-precondition");
    let after = "pub fn non_zero(value: i32) -> bool { value != 0 }\n#[cfg(test)] mod tests { #[test] fn store_test() { for sample in [1, 2] { assert!(super::non_zero(sample)); } assert!(!super::non_zero(0)); } }\n";
    let bundle_commit = setup_h62_bundle_merge(&root, &base, after);
    for args in [
        owned_args(&[
            "--in-diff-plan",
            &bundle_commit,
            "--platform",
            "all",
            "--linux-exclusions",
        ]),
        owned_args(&[
            "--in-diff",
            &bundle_commit,
            "--platform",
            "linux",
            "--shard",
            "0/1",
            "--linux-exclusions",
        ]),
        owned_args(&[
            "--in-diff",
            &bundle_commit,
            "--platform",
            "macos",
            "--shard",
            "0/1",
        ]),
    ] {
        checked_scope(&root, &args, BaseUse::Ancestor)
            .expect("H-62 bundle passes H-63 precondition");
    }
    write_file(
        &root,
        "crates/adapters/store-sqlite/src/lib.rs",
        &after.replace("[1, 2]", "[1, 3]"),
    );
    let linux_plan = owned_args(&[
        "--in-diff-plan",
        &bundle_commit,
        "--platform",
        "all",
        "--linux-exclusions",
    ]);
    let error = checked_scope(&root, &linux_plan, BaseUse::Ancestor)
        .expect_err("tampered excluded crate must still fail AST check on Linux");
    assert!(error.contains("test-diff:modified"), "{error}");
}

fn run_platform_shard_and_collect(
    root: &Path,
    base: &str,
    head: &str,
    platform: &str,
    shard: &str,
) -> Vec<serde_json::Value> {
    let args = if platform == "linux" {
        owned_args(&[
            "--in-diff",
            base,
            "--platform",
            platform,
            "--shard",
            shard,
            "--linux-exclusions",
        ])
    } else {
        owned_args(&["--in-diff", base, "--platform", platform, "--shard", shard])
    };
    let findings = run_command(root, &args, BaseUse::Ancestor).expect("platform shard");
    assert!(findings.is_empty(), "{findings:?}");
    let receipt = read_json_value(&root.join("target/mutants/diff-shard.json"));
    assert_eq!(receipt["base"], base);
    assert_eq!(receipt["head"], head);
    assert_eq!(receipt["platform"], platform);
    assert_eq!(receipt["shard"], shard);
    read_json_vec(&root.join("target/mutants/mutants.out/mutants.json"))
}

fn mutant_identities(items: &[serde_json::Value]) -> BTreeSet<(String, String)> {
    items
        .iter()
        .map(|entry| {
            (
                String::from(entry["file"].as_str().expect("file")),
                String::from(entry["name"].as_str().expect("name")),
            )
        })
        .collect()
}

#[test]
fn h63_platform_shards_partition_live_catalog_and_write_receipts() {
    let (root, base) = dual_platform_fixture("h63-live-partition");
    write_file(
        &root,
        "crates/domain/src/lib.rs",
        "pub fn positive(value: i32) -> bool { value > 0 && value < 10 }\n#[cfg(test)] mod tests { #[test] fn domain_test() { assert!(super::positive(1)); assert!(!super::positive(0)); } #[test] fn upper_bound() { assert!(!super::positive(10)); } }\n",
    );
    write_file(
        &root,
        "crates/adapters/store-sqlite/src/lib.rs",
        "pub fn non_zero(value: i32) -> bool { value != 0 && value != -1 }\n#[cfg(test)] mod tests { #[test] fn store_test() { assert!(super::non_zero(1)); assert!(!super::non_zero(0)); } #[test] fn minus_one() { assert!(!super::non_zero(-1)); } }\n",
    );
    run_git(&root, &["add", "."]);
    run_git(
        &root,
        &["commit", "--quiet", "-m", "both platforms changed"],
    );
    let head = run_git(&root, &["rev-parse", "HEAD"]);
    let plan_args = owned_args(&[
        "--in-diff-plan",
        &base,
        "--platform",
        "all",
        "--linux-exclusions",
    ]);
    assert!(
        run_command(&root, &plan_args, BaseUse::Ancestor)
            .expect("all plan")
            .is_empty()
    );
    let all_catalog = read_json_vec(&root.join("target/mutants/catalog.json"));
    assert!(all_catalog.len() >= 2, "{all_catalog:?}");
    let mut union = Vec::new();
    for shard in ["0/2", "1/2"] {
        union.extend(run_platform_shard_and_collect(
            &root, &base, &head, "linux", shard,
        ));
    }
    let macos_planned = run_platform_shard_and_collect(&root, &base, &head, "macos", "0/1");
    assert_ne!(macos_planned, Vec::<serde_json::Value>::new());
    union.extend(macos_planned);
    assert_eq!(mutant_identities(&union), mutant_identities(&all_catalog));
    assert_eq!(union.len(), all_catalog.len());
}
