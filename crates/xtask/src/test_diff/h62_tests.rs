use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::git_base::BaseUse;
use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

use super::bundle::command::{DraftMode, check_bundle_file, draft_bundle, message_trailers};
use super::{Files, approval, base_files, compare_files};

struct CaseSpec {
    task: &'static str,
    fixture_dir: &'static str,
    expected_count: usize,
    members: &'static [&'static str],
    extra_files: &'static [(&'static str, &'static str)],
    files: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

const D55_CASE: CaseSpec = CaseSpec {
    task: "D-55",
    fixture_dir: "h62-d55",
    expected_count: 30,
    members: &[
        "crates/adapters/broker-toss",
        "crates/domain",
        "crates/testkit-ports",
    ],
    extra_files: &[],
    files: &[
        (
            "crates/adapters/broker-toss/tests/t21_instruments.rs",
            "broker_toss_t21_instruments.rs",
            "560a4ea939f671ee49f0f2381e54c62efcd0b1190d2f0e65860dd33e48c33068",
            "3fc0def2f3cca21ea9e42ec4fa86ff85c457567fc2b62c3e4550e52e6368f26a",
        ),
        (
            "crates/domain/tests/instrument_mapping.rs",
            "domain_instrument_mapping.rs",
            "c5174afbd1b7a02a58ec8488d465aacb5ef8ec92993ba557c2f43415b525d792",
            "e7f43fcc8c2c59cc7175a24169cf653faea65086ff178db0c0b2f56e8d68c893",
        ),
        (
            "crates/domain/tests/instrument_mapping_review.rs",
            "domain_instrument_mapping_review.rs",
            "4939f791d128f65061dccba47610384c340375bee9e80cf9c290487c8c64fb44",
            "2d150c00cb908a9f06cb32c7b16829d4699d995b838f830fb6b9b04891fa2e12",
        ),
        (
            "crates/testkit-ports/tests/instrument_mapping.rs",
            "testkit_instrument_mapping.rs",
            "7d5ac0e0022c06992c76aa4a3e3338767f17ddae8738672d5c47592c4d3fe430",
            "19715bcf8ee91efe7094acd8c6ec3346fb8ba5a36eace75bf3c58286970ae205",
        ),
        (
            "crates/testkit-ports/tests/instrument_mapping_review.rs",
            "testkit_instrument_mapping_review.rs",
            "ee6fdb694be65b70dd03956508e5c472fe6bfca38c8aae2ba795a0718b1d1aeb",
            "84a85fa20066f574b76cc06bf01c738732216f728df338aca1b3bf34d6492628",
        ),
    ],
};

const D56_CASE: CaseSpec = CaseSpec {
    task: "D-56",
    fixture_dir: "h62-d56",
    expected_count: 76,
    members: &[
        "crates/application",
        "crates/domain",
        "crates/testkit-ports",
    ],
    extra_files: &[("crates/domain/src/lib.rs", "pub mod proposal;\n")],
    files: &[
        (
            "crates/application/tests/ports_a02_contract.rs",
            "ports_a02_contract.rs",
            "2930209068aa1ee13e699131551c6a11cb6d7f14ce1be9cb0ea8b4e8525c4345",
            "b75da7ccfbf0bcdfc12137b5f855e5a4e56953e830c81bf6d93ecf7560ef6667",
        ),
        (
            "crates/domain/src/proposal.rs",
            "proposal.rs",
            "32f3e944adc478aae9b9340e6098297649f08d05da93f411efe81be70a31ad96",
            "d3dcc2339cce2d5272c1c0f1b2ef26409e36e1d341c2808d7593cd7db52938dd",
        ),
        (
            "crates/testkit-ports/tests/a27_signal_ports.rs",
            "a27_signal_ports.rs",
            "c9ff4e39f554e62787a8c5f0c11a85a7d1081fc613242e982df6171d0abd2708",
            "ea7f9aa8d7da4531396691704744d08b134781273fab303f976b5b0b8293b448",
        ),
        (
            "crates/testkit-ports/tests/a27_submission_and_rejection.rs",
            "a27_submission_and_rejection.rs",
            "c1bdf3853c300fbf28fbd0e37b83bf9b9822ff02cc525217dc7b06849c499b85",
            "3443f6f84b3ee70e0a795032fa3b1e3d2b14535cabf0a298a170ec6a3d15c324",
        ),
        (
            "crates/testkit-ports/tests/draft_proposal.rs",
            "draft_proposal.rs",
            "2c70288c5e734220321769dedbff6795500814c64aaeb0f903bc45268a674b2b",
            "555c8666d9d8560605d472a5f6d54c338ab7b835feb4c11f523ae6a5fe50e739",
        ),
        (
            "crates/testkit-ports/tests/evaluate_signals.rs",
            "evaluate_signals.rs",
            "d31641321e4a8d86ec1d66cc067c309d25e973277831833c2171bfc9e16c1143",
            "43135e19698377661493cd835c9a91dc52b02e9de0b84e27b9a7af5769a7b2bd",
        ),
        (
            "crates/testkit-ports/tests/memory.rs",
            "memory.rs",
            "f178e0286aa072e38696fa3e8f284dc914748c645c66a20b909716c7f98052c9",
            "d04e45238b0b6a7a0a387ff19917c72ff41ab315165b7c1a1af87a0941f64fbc",
        ),
        (
            "crates/testkit-ports/tests/review_regressions.rs",
            "review_regressions.rs",
            "86cb3559a22f8ae0d5a91df101f68182c40c626e80f5741043fd1a5e8b9e2b01",
            "93ab282f894defbaaa6a1d9939c9c3f4f1789899846c430307d677939db2b4d9",
        ),
    ],
};

const S21_CASE: CaseSpec = CaseSpec {
    task: "S-21",
    fixture_dir: "h62-s21",
    expected_count: 2,
    members: &["crates/adapters/store-sqlite"],
    extra_files: &[(
        "crates/adapters/store-sqlite/src/lib.rs",
        "#[cfg(test)] mod migration_tests;\n#[cfg(test)] mod tests;\n",
    )],
    files: &[
        (
            "crates/adapters/store-sqlite/src/migration_tests.rs",
            "migration_tests.rs",
            "dbf33466db87285fb773ecd6cd8d096ef45f9544d6305ea9b935eb5e4fca99e1",
            "d62952637a9c9e99a314ba9e8d229b89295351668e5c26d8004fd29f7160df8f",
        ),
        (
            "crates/adapters/store-sqlite/src/tests.rs",
            "tests.rs",
            "d29a3e513e90295682cc083ba69ba288b4c5d8c450e549e54df5a48633801e28",
            "1f82f383ed3a04d89cb8059535fcf6676146399c89db050429dea9fdf1ba30e0",
        ),
    ],
};

const D57_CASE: CaseSpec = CaseSpec {
    task: "D-57",
    fixture_dir: "h62-d57",
    expected_count: 5,
    members: &["crates/testkit-ports"],
    extra_files: &[],
    files: &[
        (
            "crates/testkit-ports/tests/d56_signal_restore.rs",
            "d56_signal_restore.rs",
            "92f9e58f82417e21df4daa3440868511834c7e86a348d668e11c66195003fbbb",
            "e36e3654c3a6cc7997cce8896f83098121f95ce2597f8f281359d14692cb50e5",
        ),
        (
            "crates/testkit-ports/tests/draft_proposal.rs",
            "draft_proposal.rs",
            "555c8666d9d8560605d472a5f6d54c338ab7b835feb4c11f523ae6a5fe50e739",
            "cff35ee501a98390db81b21b3ee2832ac1bd75fa6149cf17458d029ec72cdc16",
        ),
    ],
};

fn git(root: &Path, args: &[&str]) -> String {
    let result = crate::git_base::git(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .expect("git");
    assert!(
        result.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn load_case_files(spec: &CaseSpec, side: &str) -> Files {
    let mut members_toml = String::from("[workspace]\nmembers = [");
    for (index, member) in spec.members.iter().enumerate() {
        if index > 0 {
            members_toml.push_str(", ");
        }
        members_toml.push('"');
        members_toml.push_str(member);
        members_toml.push('"');
    }
    members_toml.push_str("]\n");
    let mut files = BTreeMap::from([(String::from("Cargo.toml"), members_toml.into_bytes())]);
    for member in spec.members {
        let name = member.rsplit('/').next().expect("crate name");
        files.insert(
            format!("{member}/Cargo.toml"),
            format!("[package]\nname = \"{name}\"\n").into_bytes(),
        );
    }
    for (path, body) in spec.extra_files {
        files.insert(String::from(*path), body.as_bytes().to_vec());
    }
    for (repo_path, flat_name, before_hash, after_hash) in spec.files {
        let source = repository_root().join(format!(
            "harness/test-diff/fixtures/{}/{side}/{flat_name}.txt",
            spec.fixture_dir
        ));
        let bytes = fs::read(&source).expect("fixture file");
        let expected_hash = if side == "before" {
            *before_hash
        } else {
            *after_hash
        };
        assert_eq!(sha256_hex(&bytes), expected_hash, "{repo_path} ({side})");
        files.insert(String::from(*repo_path), bytes);
    }
    files
}

fn write_files(root: &Path, files: &Files) {
    for (relative, bytes) in files {
        let full = root.join(relative);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(full, bytes).expect("write");
    }
}

struct LiveMergeRepo {
    root: PathBuf,
    pre_bundle: String,
    bundle_commit: String,
    feature: String,
    merge: String,
    bundle_path: String,
    before: Files,
    after: Files,
}

fn setup_live_merge(spec: &CaseSpec, label: &str) -> LiveMergeRepo {
    let root = fresh_directory(&repository_root(), label);
    let before = load_case_files(spec, "before");
    let after = load_case_files(spec, "after");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let pre_bundle = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, &after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let bundle_toml = draft_bundle(
        &root,
        spec.task,
        &pre_bundle,
        &feature,
        DraftMode::MergeHead,
    )
    .expect("draft bundle");
    let bundle_path = format!("harness/test-diff/bundles/{}.toml", spec.task);
    assert!(
        !root.join(&bundle_path).exists(),
        "draft must not write files"
    );
    let full = root.join(&bundle_path);
    fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    fs::write(&full, &bundle_toml).expect("write bundle");
    git(&root, &["add", &bundle_path]);
    git(&root, &["commit", "--quiet", "-m", "main bundle"]);
    let bundle_commit = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &bundle_commit],
    );
    git(&root, &["checkout", "--quiet", "-b", "merge-check"]);
    let trailers = message_trailers(&root, Path::new(&bundle_path)).expect("trailers");
    let message = format!("Merge {}: approved bundle\n\n{trailers}", spec.task);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "-m", &message, "feature"],
    );
    let merge = git(&root, &["rev-parse", "HEAD"]);
    LiveMergeRepo {
        root,
        pre_bundle,
        bundle_commit,
        feature,
        merge,
        bundle_path,
        before,
        after,
    }
}

fn evaluate(root: &Path, base: &str, before: &Files, after: &Files) -> (usize, String) {
    let (findings, _) = compare_files(before, after);
    let mut output = Vec::new();
    let remaining = approval::apply_for_base_to(
        (root, base),
        (before, after),
        findings,
        BaseUse::Ancestor,
        &mut output,
    )
    .expect("evaluate");
    (
        remaining.len(),
        String::from_utf8(output).expect("utf8 diagnostics"),
    )
}

fn assert_case_reproduction(spec: &CaseSpec) {
    let repo = setup_live_merge(spec, &format!("h62-real-{}", spec.task));
    let (raw_findings, _) = compare_files(&repo.before, &repo.after);
    assert_eq!(raw_findings.len(), spec.expected_count, "{}", spec.task);

    let (without_bundle, _) = evaluate(&repo.root, &repo.pre_bundle, &repo.before, &repo.after);
    assert_eq!(without_bundle, 0, "{} combined push A+M passes", spec.task);

    let (branch_ci, diag) = evaluate(&repo.root, &repo.bundle_commit, &repo.before, &repo.after);
    assert_eq!(branch_ci, 0, "{} merge-check CI: {diag}", spec.task);

    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/master", &repo.merge],
    );
    git(&repo.root, &["checkout", "--quiet", "master"]);
    git(
        &repo.root,
        &["merge", "--quiet", "--ff-only", "merge-check"],
    );
    let (master_ci, master_diag) =
        evaluate(&repo.root, &repo.bundle_commit, &repo.before, &repo.after);
    assert_eq!(master_ci, 0, "{} master CI: {master_diag}", spec.task);

    let problems = check_bundle_file(
        &repo.root,
        Path::new(&repo.bundle_path),
        &repo.bundle_commit,
        &repo.merge,
    )
    .expect("check bundle");
    assert!(problems.is_empty(), "{} check: {problems:?}", spec.task);

    git(&repo.root, &["checkout", "--quiet", "feature"]);
    let (on_feature, feature_diag) =
        evaluate(&repo.root, &repo.bundle_commit, &repo.before, &repo.after);
    assert_eq!(on_feature, spec.expected_count, "{} on feature", spec.task);
    assert!(
        feature_diag.contains("병합 커밋이 아닌 커밋"),
        "{} feature diag: {feature_diag}",
        spec.task
    );

    git(&repo.root, &["checkout", "--quiet", "master"]);
    fs::write(repo.root.join("after.txt"), "next\n").expect("next file");
    git(&repo.root, &["add", "after.txt"]);
    git(&repo.root, &["commit", "--quiet", "-m", "after merge"]);
    let after_merge = git(&repo.root, &["rev-parse", "HEAD"]);
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/master", &after_merge],
    );
    let (reused, _) = evaluate(&repo.root, &repo.bundle_commit, &repo.before, &repo.after);
    assert_eq!(reused, spec.expected_count, "{} cannot reuse", spec.task);
}

#[test]
fn h62_four_real_historical_cases_reproduce_exact_counts_and_pass_only_with_exact_bundle() {
    for spec in [&D55_CASE, &D56_CASE, &S21_CASE, &D57_CASE] {
        assert_case_reproduction(spec);
    }
}

#[test]
fn h62_without_bundle_all_four_real_cases_keep_their_exact_finding_counts() {
    for spec in [&D55_CASE, &D56_CASE, &S21_CASE, &D57_CASE] {
        let root = fresh_directory(&repository_root(), &format!("h62-plain-{}", spec.task));
        let before = load_case_files(spec, "before");
        let after = load_case_files(spec, "after");
        git(&root, &["init", "--quiet", "-b", "master"]);
        write_files(&root, &before);
        git(&root, &["add", "."]);
        git(&root, &["commit", "--quiet", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["update-ref", "refs/remotes/origin/master", &base]);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        write_files(&root, &after);
        git(&root, &["add", "."]);
        git(&root, &["commit", "--quiet", "-m", "feature"]);
        git(&root, &["checkout", "--quiet", "master"]);
        git(
            &root,
            &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
        );
        let (remaining, _) = evaluate(&root, &base, &before, &after);
        assert_eq!(remaining, spec.expected_count, "{}", spec.task);
    }
}

fn replace_bundle_and_remerge(
    spec: &CaseSpec,
    label: &str,
    mutate: impl FnOnce(&str) -> String,
) -> (usize, String) {
    let root = fresh_directory(&repository_root(), label);
    let before = load_case_files(spec, "before");
    let after = load_case_files(spec, "after");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let pre_bundle = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, &after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let drafted = draft_bundle(
        &root,
        spec.task,
        &pre_bundle,
        &feature,
        DraftMode::MergeHead,
    )
    .expect("draft");
    let mutated = mutate(&drafted);
    let bundle_path = format!("harness/test-diff/bundles/{}.toml", spec.task);
    let full = root.join(&bundle_path);
    fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    fs::write(&full, mutated).expect("write mutated");
    git(&root, &["add", &bundle_path]);
    git(&root, &["commit", "--quiet", "-m", "mutated bundle"]);
    let bundle_commit = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &bundle_commit],
    );
    git(&root, &["checkout", "--quiet", "-b", "merge-check"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    evaluate(&root, &bundle_commit, &before, &after)
}

#[test]
fn h62_missing_finding_extra_finding_and_wrong_hash_reject_the_entire_bundle() {
    for spec in [&D55_CASE, &D56_CASE, &S21_CASE, &D57_CASE] {
        let (missing_count, missing_diag) =
            replace_bundle_and_remerge(spec, &format!("h62-missing-{}", spec.task), |toml_text| {
                let lines: Vec<&str> = toml_text.lines().collect();
                let pos = lines
                    .iter()
                    .position(|line| line.trim_start().starts_with("\"test-diff:"))
                    .expect("finding line");
                let filtered: Vec<&str> = lines
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| *index != pos)
                    .map(|(_, line)| line)
                    .collect();
                format!("{}\n", filtered.join("\n"))
            });
        assert_eq!(missing_count, spec.expected_count, "{}", spec.task);
        assert!(
            missing_diag.contains("묶음에 없는 추가 지적"),
            "{}: {missing_diag}",
            spec.task
        );

        let (extra_count, extra_diag) =
            replace_bundle_and_remerge(spec, &format!("h62-extra-{}", spec.task), |toml_text| {
                toml_text.replacen(
                    "findings = [\n",
                    "findings = [\n  \"test-diff:modified:domain tests::extra_not_present\",\n",
                    1,
                )
            });
        assert_eq!(extra_count, spec.expected_count, "{}", spec.task);
        assert!(
            extra_diag.contains("발생하지 않은 묶음 지적"),
            "{}: {extra_diag}",
            spec.task
        );

        let (_, _, _, actual_after_hash) = spec.files[0];
        let altered_hash = format!(
            "{}{}",
            if actual_after_hash.starts_with('0') {
                '1'
            } else {
                '0'
            },
            actual_after_hash.get(1..).expect("tail")
        );
        let (hash_count, hash_diag) =
            replace_bundle_and_remerge(spec, &format!("h62-hash-{}", spec.task), |toml_text| {
                toml_text.replacen(actual_after_hash, &altered_hash, 1)
            });
        assert_eq!(hash_count, spec.expected_count, "{}", spec.task);
        assert!(
            hash_diag.contains("파일 해시 불일치"),
            "{}: {hash_diag}",
            spec.task
        );
    }
}

fn commit_s21_approval_rows(repo: &LiveMergeRepo) -> String {
    let mut approval_rows = String::new();
    let (raw_findings, _) = compare_files(&repo.before, &repo.after);
    let finding_list: Vec<String> = raw_findings
        .iter()
        .map(|item| item.rule.to_string())
        .collect();
    for ((path, _, before_hash, after_hash), finding) in S21_CASE.files.iter().zip(&finding_list) {
        writeln!(
            approval_rows,
            "[[approval]]\ntask = \"S-21\"\npath = \"{path}\"\nbefore_sha256 = \"{before_hash}\"\nafter_sha256 = \"{after_hash}\"\nfinding = \"{finding}\"\nassertion_change = true\ndecision = \"approved\"\nreason = \"fixed count\""
        )
        .expect("format row");
    }
    git(&repo.root, &["checkout", "--quiet", "master"]);
    git(
        &repo.root,
        &["reset", "--quiet", "--hard", &repo.pre_bundle],
    );
    let approvals_path = repo.root.join("harness/test-diff/approvals.toml");
    fs::create_dir_all(approvals_path.parent().expect("parent")).expect("mkdir");
    fs::write(&approvals_path, &approval_rows).expect("write approvals");
    git(&repo.root, &["add", "harness/test-diff/approvals.toml"]);
    git(
        &repo.root,
        &["commit", "--quiet", "-m", "s21 approvals row"],
    );
    let authority = git(&repo.root, &["rev-parse", "HEAD"]);
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    authority
}

#[test]
fn h62_s21_structural_checker_is_not_widened_and_bundle_resolves_only_the_approved_interval() {
    let repo = setup_live_merge(&S21_CASE, "h62-s21-structural");
    let approval_authority = commit_s21_approval_rows(&repo);
    git(&repo.root, &["checkout", "--quiet", "-b", "s21-no-bundle"]);
    git(
        &repo.root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let (remaining, diag) = evaluate(&repo.root, &approval_authority, &repo.before, &repo.after);
    assert_eq!(remaining, 2);
    assert!(diag.contains("클로저"), "{diag}");

    git(&repo.root, &["checkout", "--quiet", "master"]);
    let bundle_toml = draft_bundle(
        &repo.root,
        "S-21",
        &approval_authority,
        &repo.feature,
        DraftMode::MergeHead,
    )
    .expect("s21 draft");
    let bundle_path = "harness/test-diff/bundles/S-21.toml";
    fs::create_dir_all(repo.root.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(repo.root.join(bundle_path), bundle_toml).expect("write s21 bundle");
    git(&repo.root, &["add", bundle_path]);
    git(&repo.root, &["commit", "--quiet", "-m", "s21 bundle"]);
    let bundle_commit = git(&repo.root, &["rev-parse", "HEAD"]);
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/master", &bundle_commit],
    );
    git(
        &repo.root,
        &["checkout", "--quiet", "-b", "s21-with-bundle"],
    );
    git(
        &repo.root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let (with_bundle, pass_diag) = evaluate(&repo.root, &bundle_commit, &repo.before, &repo.after);
    assert_eq!(with_bundle, 0, "{pass_diag}");
    assert!(pass_diag.is_empty(), "{pass_diag}");
}

#[test]
fn h62_historical_target_commit_and_target_tree_modes_match_only_their_exact_merge_interval() {
    for mode in [DraftMode::TargetCommit, DraftMode::TargetTree] {
        let label = match mode {
            DraftMode::TargetCommit => "h62-mode-commit",
            DraftMode::TargetTree => "h62-mode-tree",
            DraftMode::Auto | DraftMode::MergeHead => "h62-mode-other",
        };
        let root = fresh_directory(&repository_root(), label);
        let before = load_case_files(&D57_CASE, "before");
        let after = load_case_files(&D57_CASE, "after");
        git(&root, &["init", "--quiet", "-b", "master"]);
        write_files(&root, &before);
        git(&root, &["add", "."]);
        git(&root, &["commit", "--quiet", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        write_files(&root, &after);
        git(&root, &["add", "."]);
        git(&root, &["commit", "--quiet", "-m", "feature"]);
        git(&root, &["checkout", "--quiet", "master"]);
        git(
            &root,
            &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
        );
        let merge = git(&root, &["rev-parse", "HEAD"]);
        let drafted = draft_bundle(&root, "D-57", &base, &merge, mode).expect("draft");
        let bundle_path = "harness/test-diff/bundles/D-57.toml";
        fs::create_dir_all(root.join(bundle_path).parent().expect("parent")).expect("mkdir");
        fs::write(root.join(bundle_path), drafted).expect("write");
        git(&root, &["add", bundle_path]);
        git(&root, &["commit", "--quiet", "-m", "historical bundle"]);
        let head_master = git(&root, &["rev-parse", "HEAD"]);
        git(
            &root,
            &["update-ref", "refs/remotes/origin/master", &head_master],
        );
        git(&root, &["checkout", "--quiet", "--detach", &merge]);
        let (remaining, diag) = evaluate(&root, &base, &before, &after);
        assert_eq!(remaining, 0, "{label}: {diag}");

        let (wrong_base, _) = evaluate(&root, &head_master, &before, &after);
        assert_eq!(wrong_base, 5, "{label} wrong base");

        let mut dirty_after = after.clone();
        dirty_after.insert(
            String::from("crates/testkit-ports/tests/extra.rs"),
            b"#[test] fn added() {}\n".to_vec(),
        );
        let (dirty, dirty_diag) = evaluate(&root, &base, &before, &dirty_after);
        assert_eq!(dirty, 5, "{label} dirty worktree");
        assert!(dirty_diag.contains("작업 사본"), "{dirty_diag}");
    }
}

#[test]
fn h62_pending_master_merge_and_master_advance_invalidation_work_strictly() {
    let root = fresh_directory(&repository_root(), "h62-pending-merge");
    let before = load_case_files(&D57_CASE, "before");
    let after = load_case_files(&D57_CASE, "after");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let pre_bundle = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, &after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let bundle_toml =
        draft_bundle(&root, "D-57", &pre_bundle, &feature, DraftMode::MergeHead).expect("draft");
    let bundle_path = "harness/test-diff/bundles/D-57.toml";
    fs::create_dir_all(root.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(root.join(bundle_path), bundle_toml).expect("write");
    git(&root, &["add", bundle_path]);
    git(&root, &["commit", "--quiet", "-m", "bundle"]);
    let bundle_commit = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &bundle_commit],
    );
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-commit", "feature"],
    );
    let (pending_ok, diag) = evaluate(&root, &bundle_commit, &before, &after);
    assert_eq!(pending_ok, 0, "pending merge: {diag}");

    git(&root, &["merge", "--abort"]);
    fs::write(root.join("intervening.txt"), "master advanced\n").expect("advance");
    git(&root, &["add", "intervening.txt"]);
    git(&root, &["commit", "--quiet", "-m", "intervening commit"]);
    let advanced = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &advanced],
    );
    git(&root, &["checkout", "--quiet", "-b", "merge-after-advance"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let (after_advance, _) = evaluate(&root, &advanced, &before, &after);
    assert_eq!(after_advance, 5, "bundle must expire once master advances");
}

#[test]
fn h62_untrusted_branch_bundle_is_rejected() {
    let before = load_case_files(&D57_CASE, "before");
    let after = load_case_files(&D57_CASE, "after");
    let bundle_path = "harness/test-diff/bundles/D-57.toml";
    let branch_root = fresh_directory(&repository_root(), "h62-branch-bundle");
    git(&branch_root, &["init", "--quiet", "-b", "master"]);
    write_files(&branch_root, &before);
    git(&branch_root, &["add", "."]);
    git(&branch_root, &["commit", "--quiet", "-m", "base"]);
    let base = git(&branch_root, &["rev-parse", "HEAD"]);
    git(
        &branch_root,
        &["update-ref", "refs/remotes/origin/master", &base],
    );
    git(&branch_root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&branch_root, &after);
    git(&branch_root, &["add", "."]);
    git(&branch_root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&branch_root, &["rev-parse", "HEAD"]);
    let drafted =
        draft_bundle(&branch_root, "D-57", &base, &feature, DraftMode::MergeHead).expect("draft");
    fs::create_dir_all(branch_root.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(branch_root.join(bundle_path), &drafted).expect("write");
    git(&branch_root, &["add", bundle_path]);
    git(&branch_root, &["commit", "--quiet", "-m", "forged bundle"]);
    let (branch_remaining, branch_diag) = evaluate(&branch_root, &base, &before, &after);
    assert_eq!(branch_remaining, 5);
    assert!(branch_diag.contains("신뢰 실패"), "{branch_diag}");
}

#[test]
fn h62_merge_introduced_bundle_is_rejected() {
    let before = load_case_files(&D57_CASE, "before");
    let after = load_case_files(&D57_CASE, "after");
    let bundle_path = "harness/test-diff/bundles/D-57.toml";
    let merge_intro = fresh_directory(&repository_root(), "h62-merge-intro");
    git(&merge_intro, &["init", "--quiet", "-b", "master"]);
    write_files(&merge_intro, &before);
    git(&merge_intro, &["add", "."]);
    git(&merge_intro, &["commit", "--quiet", "-m", "base"]);
    let m_base = git(&merge_intro, &["rev-parse", "HEAD"]);
    git(&merge_intro, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&merge_intro, &after);
    git(&merge_intro, &["add", "."]);
    git(&merge_intro, &["commit", "--quiet", "-m", "feature"]);
    let m_feature = git(&merge_intro, &["rev-parse", "HEAD"]);
    let m_draft = draft_bundle(
        &merge_intro,
        "D-57",
        &m_base,
        &m_feature,
        DraftMode::MergeHead,
    )
    .expect("draft");
    git(
        &merge_intro,
        &["checkout", "--quiet", "-b", "side", &m_base],
    );
    fs::create_dir_all(merge_intro.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(merge_intro.join(bundle_path), &m_draft).expect("write");
    git(&merge_intro, &["add", bundle_path]);
    git(&merge_intro, &["commit", "--quiet", "-m", "side bundle"]);
    git(&merge_intro, &["checkout", "--quiet", "master"]);
    git(
        &merge_intro,
        &["merge", "--quiet", "--no-ff", "--no-edit", "side"],
    );
    let m_auth = git(&merge_intro, &["rev-parse", "HEAD"]);
    git(
        &merge_intro,
        &["update-ref", "refs/remotes/origin/master", &m_auth],
    );
    git(&merge_intro, &["checkout", "--quiet", "-b", "merge-check"]);
    git(
        &merge_intro,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let (merge_intro_remaining, merge_intro_diag) =
        evaluate(&merge_intro, &m_auth, &before, &after);
    assert_eq!(merge_intro_remaining, 5);
    assert!(merge_intro_diag.contains("신뢰 실패"), "{merge_intro_diag}");
}

#[test]
fn h62_shallow_repository_rejects_bundle() {
    let shallow = setup_live_merge(&D57_CASE, "h62-shallow");
    fs::write(
        shallow.root.join(".git/shallow"),
        format!("{}\n", shallow.bundle_commit),
    )
    .expect("shallow");
    let (shallow_remaining, shallow_diag) = evaluate(
        &shallow.root,
        &shallow.bundle_commit,
        &shallow.before,
        &shallow.after,
    );
    assert_eq!(shallow_remaining, 5);
    assert!(shallow_diag.contains("신뢰 실패"), "{shallow_diag}");
}

#[test]
fn h62_intro_commit_with_extra_file_is_rejected() {
    let before = load_case_files(&D57_CASE, "before");
    let after = load_case_files(&D57_CASE, "after");
    let root = fresh_directory(&repository_root(), "h62-extra-intro");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let pre = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, &after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let drafted = draft_bundle(&root, "D-57", &pre, &feature, DraftMode::MergeHead).expect("draft");
    let bundle_path = "harness/test-diff/bundles/D-57.toml";
    fs::create_dir_all(root.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(root.join(bundle_path), &drafted).expect("write");
    fs::write(root.join("unrelated.txt"), "extra\n").expect("extra");
    git(&root, &["add", bundle_path, "unrelated.txt"]);
    git(&root, &["commit", "--quiet", "-m", "mixed bundle commit"]);
    let auth = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["update-ref", "refs/remotes/origin/master", &auth]);
    git(&root, &["checkout", "--quiet", "-b", "merge-check"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let (mixed_remaining, mixed_diag) = evaluate(&root, &auth, &before, &after);
    assert_eq!(mixed_remaining, 5);
    assert!(mixed_diag.contains("묶음 외 파일"), "{mixed_diag}");
}

#[test]
fn h62_mismatched_merge_commit_trailers_are_rejected() {
    let bad_trailer = setup_live_merge(&D57_CASE, "h62-bad-trailer");
    git(
        &bad_trailer.root,
        &[
            "checkout",
            "--quiet",
            "-b",
            "bad-msg",
            &bad_trailer.bundle_commit,
        ],
    );
    git(
        &bad_trailer.root,
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "-m",
            "Merge D-57\n\nTest-Diff-Bundle: harness/test-diff/bundles/D-57.toml\nTest-Diff-Findings: 99\n",
            "feature",
        ],
    );
    let (trailer_rem, trailer_diag) = evaluate(
        &bad_trailer.root,
        &bad_trailer.bundle_commit,
        &bad_trailer.before,
        &bad_trailer.after,
    );
    assert_eq!(trailer_rem, 5);
    assert!(
        trailer_diag.contains("병합 커밋 메시지 불일치"),
        "{trailer_diag}"
    );
}

#[test]
fn h62_duplicate_matching_bundles_fail_closed() {
    let before = load_case_files(&D57_CASE, "before");
    let after = load_case_files(&D57_CASE, "after");
    let dup = fresh_directory(&repository_root(), "h62-duplicate-bundles");
    git(&dup, &["init", "--quiet", "-b", "master"]);
    write_files(&dup, &before);
    git(&dup, &["add", "."]);
    git(&dup, &["commit", "--quiet", "-m", "base"]);
    let d_base = git(&dup, &["rev-parse", "HEAD"]);
    git(&dup, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&dup, &after);
    git(&dup, &["add", "."]);
    git(&dup, &["commit", "--quiet", "-m", "feature"]);
    git(&dup, &["checkout", "--quiet", "master"]);
    git(
        &dup,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    let d_merge = git(&dup, &["rev-parse", "HEAD"]);
    let b1 = draft_bundle(&dup, "D-57", &d_base, &d_merge, DraftMode::TargetCommit).expect("b1");
    let b2 = draft_bundle(&dup, "D-58", &d_base, &d_merge, DraftMode::TargetCommit).expect("b2");
    fs::create_dir_all(dup.join("harness/test-diff/bundles")).expect("mkdir");
    fs::write(dup.join("harness/test-diff/bundles/D-57.toml"), b1).expect("w1");
    fs::write(dup.join("harness/test-diff/bundles/D-58.toml"), b2).expect("w2");
    git(&dup, &["add", "harness/test-diff/bundles"]);
    git(&dup, &["commit", "--quiet", "-m", "two bundles"]);
    let d_auth = git(&dup, &["rev-parse", "HEAD"]);
    git(&dup, &["update-ref", "refs/remotes/origin/master", &d_auth]);
    git(&dup, &["checkout", "--quiet", "--detach", &d_merge]);
    let (dup_rem, dup_diag) = evaluate(&dup, &d_base, &before, &after);
    assert_eq!(dup_rem, 5);
    assert!(dup_diag.contains("중복 일치"), "{dup_diag}");
}

fn setup_removed_test_repo() -> (PathBuf, String, String, String, String) {
    let root = fresh_directory(&repository_root(), "h62-removed-test");
    let before = BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/sample\"]\n".to_vec(),
        ),
        (
            String::from("crates/sample/Cargo.toml"),
            b"[package]\nname = \"sample\"\n".to_vec(),
        ),
        (
            String::from("crates/sample/src/lib.rs"),
            b"#[cfg(test)] mod tests { #[test] fn a() { assert_eq!(1, 1); } #[test] fn b() { assert_eq!(2, 2); } }\n".to_vec(),
        ),
    ]);
    let after = BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/sample\"]\n".to_vec(),
        ),
        (
            String::from("crates/sample/Cargo.toml"),
            b"[package]\nname = \"sample\"\n".to_vec(),
        ),
        (
            String::from("crates/sample/src/lib.rs"),
            b"#[cfg(test)] mod tests { #[test] fn b() { assert_eq!(2, 2); } }\n".to_vec(),
        ),
    ]);
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let base = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, &after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "remove test"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    let old_h = sha256_hex(before.get("crates/sample/src/lib.rs").expect("before"));
    let new_h = sha256_hex(after.get("crates/sample/src/lib.rs").expect("after"));
    (root, base, feature, old_h, new_h)
}

#[test]
fn h62_malformed_wildcard_and_removed_finding_bundles_fail_closed() {
    let (root, base, feature, old_h, new_h) = setup_removed_test_repo();
    let draft_err = draft_bundle(&root, "H-62", &base, &feature, DraftMode::MergeHead)
        .expect_err("removed test cannot be drafted");
    assert!(draft_err.contains("승인할 수 없는 지적"), "{draft_err}");
    for (name, body) in [
        (
            "removed",
            format!(
                "task = \"H-62\"\nbase_commit = \"{base}\"\nmerge_head = \"{feature}\"\nreason = \"bad\"\nfindings = [\"test-diff:removed:sample tests::a\"]\n[[file]]\npath = \"crates/sample/src/lib.rs\"\nbefore_sha256 = \"{old_h}\"\nafter_sha256 = \"{new_h}\"\n"
            ),
        ),
        (
            "wildcard",
            format!(
                "task = \"H-62\"\nbase_commit = \"{base}\"\nmerge_head = \"{feature}\"\nreason = \"bad\"\nfindings = [\"test-diff:modified:sample *\"]\n[[file]]\npath = \"crates/sample/src/lib.rs\"\nbefore_sha256 = \"{old_h}\"\nafter_sha256 = \"{new_h}\"\n"
            ),
        ),
        (
            "two-targets",
            format!(
                "task = \"H-62\"\nbase_commit = \"{base}\"\nmerge_head = \"{feature}\"\ntarget_commit = \"{feature}\"\nreason = \"bad\"\nfindings = [\"test-diff:modified:sample tests::a\"]\n[[file]]\npath = \"crates/sample/src/lib.rs\"\nbefore_sha256 = \"{old_h}\"\nafter_sha256 = \"{new_h}\"\n"
            ),
        ),
        (
            "mismatched-task-name",
            format!(
                "task = \"H-99\"\nbase_commit = \"{base}\"\nmerge_head = \"{feature}\"\nreason = \"bad\"\nfindings = [\"test-diff:modified:sample tests::a\"]\n[[file]]\npath = \"crates/sample/src/lib.rs\"\nbefore_sha256 = \"{old_h}\"\nafter_sha256 = \"{new_h}\"\n"
            ),
        ),
    ] {
        let probe = root.join(format!("target-{name}/H-62.toml"));
        fs::create_dir_all(probe.parent().expect("parent")).expect("mkdir");
        fs::write(&probe, body).expect("write");
        check_bundle_file(&root, &probe, &base, &feature)
            .expect_err(&format!("{name} must fail closed"));
    }
}

fn setup_mutants_bundle_repo() -> (PathBuf, String, &'static str, &'static str) {
    let root = fresh_directory(&repository_root(), "h62-mutants-bundle");
    let path = "crates/domain/src/sample.rs";
    let before = "pub fn value() -> usize { 1 }\n#[cfg(test)] mod tests { #[test] fn stable() { for count in [7_usize] { assert_eq!(super::value() + 6, count); } } }\n";
    let after = "pub fn value() -> usize { 2 }\n#[cfg(test)] mod tests { #[test] fn stable() { for count in [8_usize] { assert_eq!(super::value() + 6, count); } } }\n";
    for (file, text) in [
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/domain\"]\nresolver = \"3\"\n",
        ),
        (
            "crates/domain/Cargo.toml",
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        (
            ".cargo/mutants.toml",
            "examine_globs = [\"crates/domain/src/**/*.rs\"]\n",
        ),
        ("crates/domain/src/lib.rs", "pub mod sample;\n"),
        (path, before),
    ] {
        let full = root.join(file);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(full, text).expect("write");
    }
    generate_lockfile(&root.join("Cargo.toml"));
    git(&root, &["init", "--quiet", "-b", "master"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let pre_bundle = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    fs::write(root.join(path), after).expect("write after");
    git(&root, &["add", path]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    let feature = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let drafted =
        draft_bundle(&root, "H-62", &pre_bundle, &feature, DraftMode::MergeHead).expect("draft");
    let bundle_path = "harness/test-diff/bundles/H-62.toml";
    fs::create_dir_all(root.join(bundle_path).parent().expect("parent")).expect("mkdir");
    fs::write(root.join(bundle_path), drafted).expect("write bundle");
    git(&root, &["add", bundle_path]);
    git(&root, &["commit", "--quiet", "-m", "bundle"]);
    let bundle_commit = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &bundle_commit],
    );
    git(&root, &["checkout", "--quiet", "-b", "merge-check"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature"],
    );
    (root, bundle_commit, path, after)
}

#[test]
fn h62_mutants_diff_precondition_accepts_valid_bundle_and_rejects_tampered_file() {
    let (root, bundle_commit, path, after) = setup_mutants_bundle_repo();
    let diff_args = [String::from("--in-diff"), bundle_commit.clone()];
    let plan_args = [String::from("--in-diff-plan"), bundle_commit.clone()];
    let shard_args = [
        String::from("--in-diff"),
        bundle_commit.clone(),
        String::from("--shard"),
        String::from("0/4"),
    ];
    for candidate in [&diff_args[..], &plan_args[..], &shard_args[..]] {
        crate::mutants_check::checked_scope(&root, candidate, BaseUse::Ancestor)
            .expect("valid bundle passes mutants precondition");
    }
    fs::write(root.join(path), after.replace("8_usize", "9_usize")).expect("tamper");
    for candidate in [&diff_args[..], &plan_args[..], &shard_args[..]] {
        let error = crate::mutants_check::checked_scope(&root, candidate, BaseUse::Ancestor)
            .expect_err("tampered file must fail mutants precondition");
        assert!(error.contains("test-diff 실패"), "{error}");
    }
    let committed_before = base_files(&root, &bundle_commit).expect("before files");
    assert!(committed_before.contains_key("harness/test-diff/bundles/H-62.toml"));
}

#[test]
fn h62_draft_bundle_auto_mode_and_unlisted_file_coverage_are_strict() {
    let repo = setup_live_merge(&D57_CASE, "h62-auto-and-coverage");
    let auto_merge = draft_bundle(
        &repo.root,
        "D-57",
        &repo.bundle_commit,
        &repo.merge,
        DraftMode::Auto,
    )
    .expect("auto commit mode");
    assert!(auto_merge.contains("target_commit = "), "{auto_merge}");
    let auto_branch = draft_bundle(
        &repo.root,
        "D-57",
        &repo.pre_bundle,
        &repo.feature,
        DraftMode::Auto,
    )
    .expect("auto merge-head mode");
    assert!(auto_branch.contains("merge_head = "), "{auto_branch}");

    let (without_one_file_count, without_one_file_diag) =
        replace_bundle_and_remerge(&D57_CASE, "h62-missing-file-entry", |toml_text| {
            let cut = toml_text.rfind("\n[[file]]").expect("second file entry");
            format!("{}\n", toml_text.get(..cut).expect("prefix"))
        });
    assert_eq!(without_one_file_count, 5);
    assert!(
        without_one_file_diag.contains("해시가 없는 파일"),
        "{without_one_file_diag}"
    );
}
