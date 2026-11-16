use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::git_base::{self, BaseUse};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Full,
    Fast(BTreeSet<String>, Frontend),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Frontend {
    Changed,
    Unchanged,
}

pub(crate) fn plan(
    paths: &BTreeSet<String>,
    members: &BTreeMap<String, String>,
    dependencies: &BTreeMap<String, BTreeSet<String>>,
) -> Plan {
    let mut crates = BTreeSet::new();
    let mut frontend = Frontend::Unchanged;
    for path in paths {
        if path == "crates/testkit/src/lib.rs"
            || path.starts_with("crates/testkit-ports/src/")
            || path.starts_with("docs/")
        {
            return Plan::Full;
        }
        if Path::new(path).file_name().is_some_and(|file| {
            file == "Cargo.toml" || file == "package.json" || file == "pnpm-lock.yaml"
        }) {
            return Plan::Full;
        }
        if let Some((_, name)) = members.iter().find(|(directory, _)| {
            path.starts_with(&format!("{directory}/src/"))
                || path.starts_with(&format!("{directory}/tests/"))
        }) {
            if name == "xtask" {
                return Plan::Full;
            }
            crates.insert(name.clone());
        } else if path.starts_with("apps/desktop/src/")
            || path.starts_with("apps/desktop/tests/")
            || path.starts_with("apps/desktop/public/")
        {
            frontend = Frontend::Changed;
        } else if !path.starts_with("docs/") {
            return Plan::Full;
        }
    }
    loop {
        let previous = crates.clone();
        for (name, deps) in dependencies {
            if !deps.is_disjoint(&crates) {
                crates.insert(name.clone());
            }
        }
        if previous == crates {
            return Plan::Fast(crates, frontend);
        }
    }
}

fn execute(root: &Path, executable: &str, args: &[String]) -> Result<(), String> {
    let status = Command::new(executable)
        .current_dir(root)
        .args(args)
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{executable}: {status}"))
    }
}

type Members = BTreeMap<String, String>;
type Dependencies = BTreeMap<String, BTreeSet<String>>;

fn workspace_inputs(root: &Path) -> Result<(Members, Dependencies), String> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .map_err(|error| error.to_string())?;
    let mut members = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    for package in metadata.workspace_packages() {
        let directory = package
            .manifest_path
            .parent()
            .ok_or("manifest parent")?
            .strip_prefix(&metadata.workspace_root)
            .map_err(|error| error.to_string())?;
        members.insert(directory.to_string(), package.name.to_string());
        dependencies.insert(
            package.name.to_string(),
            package
                .dependencies
                .iter()
                .map(|dep| dep.name.clone())
                .collect(),
        );
    }
    Ok((members, dependencies))
}

fn generated_stage(crates: &BTreeSet<String>, frontend: &Frontend) -> Option<&'static str> {
    (*frontend == Frontend::Changed || crates.contains("src-tauri"))
        .then_some("check-fast-generated")
}

fn check_rust(root: &Path, crates: &BTreeSet<String>) -> Result<(), String> {
    if !crates.is_empty() {
        let packages: Vec<String> = crates
            .iter()
            .flat_map(|name| [String::from("-p"), name.clone()])
            .collect();
        let mut clippy = vec![
            String::from("clippy"),
            String::from("--locked"),
            String::from("--all-targets"),
        ];
        clippy.extend(packages.clone());
        clippy.extend([
            String::from("--"),
            String::from("-D"),
            String::from("warnings"),
        ]);
        execute(root, "cargo", &clippy)?;
        let mut tests = vec![
            String::from("nextest"),
            String::from("run"),
            String::from("--locked"),
        ];
        tests.extend(packages);
        execute(root, "cargo", &tests)?;
    }
    Ok(())
}

fn run(root: &Path, requested_base: &str, base_use: BaseUse, just: &str) -> Result<(), String> {
    let base = git_base::resolve_base_for(root, requested_base, base_use)?;
    let (members, dependencies) = workspace_inputs(root)?;
    let selected = plan(
        &git_base::changed_paths(root, &base)?,
        &members,
        &dependencies,
    );
    let just_step =
        |recipe: &str| execute(root, just, &[format!("base={base}"), String::from(recipe)]);
    just_step("commit-guard")?;
    if crate::git_hooks::commit_base::fast_branch(root)?
        != crate::git_hooks::commit_base::WorkBranch::NamedWork
    {
        return just_step("check");
    }
    let Plan::Fast(crates, frontend) = selected else {
        return just_step("check");
    };
    just_step("check-fast-static")?;
    if frontend == Frontend::Changed {
        just_step("check-ts")?;
    }
    if let Some(recipe) = generated_stage(&crates, &frontend) {
        just_step(recipe)?;
    }
    check_rust(root, &crates)?;
    let mut test_diff = vec![format!("base={base}"), String::from("test-diff-fast")];
    test_diff.extend(crates);
    execute(root, just, &test_diff)?;
    for recipe in ["disable-report", "disable-report-fast", "hooks-installed"] {
        just_step(recipe)?;
    }
    Ok(())
}

pub(crate) fn command(base: &str, base_use: BaseUse, just: &str) -> ExitCode {
    match crate::check_support::workspace_root().and_then(|root| run(&root, base, base_use, just)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => match writeln!(std::io::stderr().lock(), "check-fast: {failure}") {
            Ok(()) | Err(_) => ExitCode::FAILURE,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Frontend, Plan, plan};
    use std::collections::{BTreeMap, BTreeSet};

    fn fixture() -> (super::Members, super::Dependencies) {
        let members = BTreeMap::from([
            (String::from("crates/domain"), String::from("domain")),
            (String::from("crates/xtask"), String::from("xtask")),
        ]);
        let deps = BTreeMap::from([
            (
                String::from("application"),
                BTreeSet::from([String::from("domain")]),
            ),
            (
                String::from("desktop"),
                BTreeSet::from([String::from("application")]),
            ),
        ]);
        (members, deps)
    }

    #[test]
    fn generated_bindings_build_only_for_affected_app_or_frontend() {
        assert_eq!(
            super::generated_stage(
                &BTreeSet::from([String::from("domain")]),
                &Frontend::Unchanged
            ),
            None
        );
        assert_eq!(
            super::generated_stage(
                &BTreeSet::from([String::from("src-tauri")]),
                &Frontend::Unchanged
            ),
            Some("check-fast-generated")
        );
        assert_eq!(
            super::generated_stage(&BTreeSet::new(), &Frontend::Changed),
            Some("check-fast-generated")
        );
    }

    #[test]
    fn nested_rust_members_are_not_misclassified_as_frontend() {
        let (mut members, deps) = fixture();
        members.insert(
            String::from("apps/desktop/e2e-app"),
            String::from("e2e-app"),
        );
        assert_eq!(
            plan(
                &BTreeSet::from([String::from("apps/desktop/e2e-app/src/lib.rs")]),
                &members,
                &deps
            ),
            Plan::Fast(
                BTreeSet::from([String::from("e2e-app")]),
                Frontend::Unchanged
            )
        );
        for path in [
            "apps/desktop/eslint.config.mjs",
            "apps/desktop/tsconfig.json",
            "apps/desktop/package.json",
            "apps/desktop/yarn.lock",
        ] {
            assert_eq!(
                plan(&BTreeSet::from([String::from(path)]), &members, &deps),
                Plan::Full
            );
        }
        assert_eq!(
            plan(
                &BTreeSet::from([
                    String::from("apps/desktop/src/app.ts"),
                    String::from("crates/domain/src/value.rs")
                ]),
                &members,
                &deps
            ),
            Plan::Fast(
                ["domain", "application", "desktop"]
                    .map(String::from)
                    .into(),
                Frontend::Changed
            )
        );
    }

    #[test]
    fn path_table_and_reverse_dependencies() {
        let (members, deps) = fixture();
        for path in [
            "harness/a",
            "justfile",
            "lefthook.yml",
            ".github/workflows/a",
            "Cargo.toml",
            "Cargo.lock",
            "package.json",
            "pnpm-lock.yaml",
            "deny.toml",
            "unknown",
            "crates/domain/Cargo.toml",
            "crates/xtask/src/a.rs",
        ] {
            assert_eq!(
                plan(&BTreeSet::from([String::from(path)]), &members, &deps),
                Plan::Full,
                "{path}"
            );
        }
        assert_eq!(
            plan(
                &BTreeSet::from([String::from("crates/domain/src/a.rs")]),
                &members,
                &deps
            ),
            Plan::Fast(
                ["domain", "application", "desktop"]
                    .map(String::from)
                    .into(),
                Frontend::Unchanged
            )
        );
        assert_eq!(
            plan(
                &BTreeSet::from([String::from("apps/desktop/src/a.ts")]),
                &members,
                &deps
            ),
            Plan::Fast(BTreeSet::new(), Frontend::Changed)
        );
        assert_eq!(
            plan(
                &BTreeSet::from([String::from("docs/a.md")]),
                &members,
                &deps
            ),
            Plan::Full
        );
    }
}

#[cfg(test)]
mod execution_tests {
    use crate::harness_workspace_tests::{
        fresh_directory, generate_lockfile, place_git_directory_outside, repository_root,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    const RUST_TRACE: [&str; 6] = [
        "commit-guard",
        "check-fast-static",
        "test-diff-fast",
        "disable-report",
        "disable-report-fast",
        "hooks-installed",
    ];
    const FRONTEND_TRACE: [&str; 8] = [
        "commit-guard",
        "check-fast-static",
        "check-ts",
        "check-fast-generated",
        "test-diff-fast",
        "disable-report",
        "disable-report-fast",
        "hooks-installed",
    ];

    fn git(root: &Path, args: &[&str]) {
        place_git_directory_outside(root, args);
        let status = crate::git_base::git(root)
            .args([
                "-c",
                "user.name=harness",
                "-c",
                "user.email=harness@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .status()
            .expect("fixture git");
        assert!(status.success(), "{args:?}");
    }

    fn trace(root: &Path, base: &str) -> Vec<String> {
        std::fs::write(root.join("target/steps.log"), "").expect("clear trace");
        super::run(
            root,
            base,
            crate::git_base::BaseUse::Ancestor,
            root.join("target/just-stub").to_str().expect("stub path"),
        )
        .expect("fast execution");
        std::fs::read_to_string(root.join("target/steps.log"))
            .expect("trace")
            .lines()
            .map(|line| line.split_whitespace().nth(1).expect("recipe").to_owned())
            .collect()
    }

    fn fixture() -> std::path::PathBuf {
        let root = fresh_directory(&repository_root(), "h46-fast-execution");
        std::fs::create_dir_all(root.join("crates/domain/src")).expect("crate directory");
        std::fs::create_dir_all(root.join("target")).expect("target");
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/domain\"]\nresolver=\"3\"\n",
        )
        .expect("workspace");
        std::fs::write(
            root.join("crates/domain/Cargo.toml"),
            "[package]\nname=\"domain\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .expect("manifest");
        std::fs::write(root.join("crates/domain/src/lib.rs"), "pub fn value() -> u32 { 1 }\n#[cfg(test)] mod tests { #[test] fn kept() { assert_eq!(super::value(), 1); } }\n").expect("source");
        std::fs::write(root.join(".gitignore"), "/target/\n").expect("ignore");
        std::fs::write(
            root.join("target/just-stub"),
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> target/steps.log\n",
        )
        .expect("stub");
        std::fs::set_permissions(
            root.join("target/just-stub"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("executable");
        generate_lockfile(&root.join("Cargo.toml"));
        git(&root, &["init", "--initial-branch=master"]);
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "baseline"]);
        git(&root, &["switch", "-c", "work"]);
        root
    }

    #[test]
    fn execution_keeps_all_guards_and_selects_frontend_or_full_without_dropping_checks() {
        let root = fixture();
        let base = crate::git_base::resolve_base(&root, "HEAD").expect("baseline");
        let library = root.join("crates/domain/src/lib.rs");
        let source = std::fs::read_to_string(&library).expect("source");
        std::fs::write(library, format!("pub fn added() -> u32 {{ 2 }}\n{source}"))
            .expect("addition");
        git(&root, &["add", "."]);
        assert_eq!(trace(&root, &base), RUST_TRACE);
        assert!(
            std::fs::read_to_string(root.join("target/steps.log"))
                .expect("trace")
                .lines()
                .any(|line| line == format!("base={base} test-diff-fast domain"))
        );
        git(&root, &["commit", "-m", "addition"]);
        let current = crate::git_base::resolve_base(&root, "HEAD").expect("current baseline");
        std::fs::create_dir_all(root.join("apps/desktop/src")).expect("frontend directory");
        std::fs::write(root.join("apps/desktop/src/app.ts"), "").expect("frontend");
        git(&root, &["add", "."]);
        assert_eq!(trace(&root, &current), FRONTEND_TRACE);
        std::fs::create_dir_all(root.join("harness")).expect("harness directory");
        std::fs::write(root.join("harness/rules.json"), "{}").expect("harness change");
        git(&root, &["add", "."]);
        assert_eq!(trace(&root, &current), ["commit-guard", "check"]);
        git(&root, &["commit", "-m", "harness fixture"]);
        git(&root, &["switch", "master"]);
        let master_base = crate::git_base::resolve_base(&root, "HEAD").expect("master baseline");
        assert_eq!(trace(&root, &master_base), ["commit-guard", "check"]);
    }
}

#[cfg(test)]
mod review_tests {
    #[test]
    fn documents_and_property_helper_promote_full() {
        for path in [
            "docs/architecture.md",
            "docs/design.md",
            "docs/harness.md",
            "docs/unknown.md",
            "crates/testkit/src/lib.rs",
        ] {
            assert_eq!(
                super::plan(
                    &[String::from(path)].into(),
                    &super::Members::from([(
                        String::from("crates/testkit"),
                        String::from("testkit")
                    )]),
                    &super::Dependencies::new()
                ),
                super::Plan::Full
            );
        }
    }

    #[test]
    fn metadata_keeps_dev_and_build_reverse_dependencies() {
        let root = crate::check_support::workspace_root().expect("root");
        let (_, dependencies) = super::workspace_inputs(&root).expect("inputs");
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(root.join("Cargo.toml"))
            .no_deps()
            .exec()
            .expect("metadata");
        let mut kinds = std::collections::BTreeSet::new();
        for package in metadata.workspace_packages() {
            for dependency in package
                .dependencies
                .iter()
                .filter(|dependency| dependency.kind != cargo_metadata::DependencyKind::Normal)
            {
                assert!(dependencies[package.name.as_str()].contains(&dependency.name));
                kinds.insert(format!("{:?}", dependency.kind));
            }
        }
        assert!(kinds.contains("Development"));
        assert!(kinds.contains("Build"));
    }
    #[test]
    fn port_fake_sources_keep_the_full_shared_support_check() {
        let members = super::Members::from([(
            String::from("crates/testkit-ports"),
            String::from("testkit-ports"),
        )]);
        for path in [
            "crates/testkit-ports/src/lib.rs",
            "crates/testkit-ports/src/scripted_presence.rs",
            "crates/testkit-ports/src/memory.rs",
        ] {
            assert_eq!(
                super::plan(
                    &[String::from(path)].into(),
                    &members,
                    &super::Dependencies::new()
                ),
                super::Plan::Full,
                "{path}"
            );
        }
    }
}
