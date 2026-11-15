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

fn workspace_inputs() -> Result<(Members, Dependencies), String> {
    let metadata = cargo_metadata::MetadataCommand::new()
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

fn run(requested_base: &str, base_use: BaseUse, just: &str) -> Result<(), String> {
    let root = crate::check_support::workspace_root()?;
    let base = git_base::resolve_base_for(&root, requested_base, base_use)?;
    let (members, dependencies) = workspace_inputs()?;
    let selected = plan(
        &git_base::changed_paths(&root, &base)?,
        &members,
        &dependencies,
    );
    let just_step =
        |recipe: &str| execute(&root, just, &[format!("base={base}"), String::from(recipe)]);
    just_step("commit-guard")?;
    if crate::git_hooks::commit_base::work_branch(&root)?
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
        execute(&root, "cargo", &clippy)?;
        let mut tests = vec![
            String::from("nextest"),
            String::from("run"),
            String::from("--locked"),
        ];
        tests.extend(packages);
        execute(&root, "cargo", &tests)?;
    }
    let mut test_diff = vec![format!("base={base}"), String::from("test-diff-fast")];
    test_diff.extend(crates);
    execute(&root, just, &test_diff)?;
    for recipe in ["disable-report", "disable-report-fast", "hooks-installed"] {
        just_step(recipe)?;
    }
    Ok(())
}

pub(crate) fn command(base: &str, base_use: BaseUse, just: &str) -> ExitCode {
    match run(base, base_use, just) {
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
            Plan::Fast(BTreeSet::new(), Frontend::Unchanged)
        );
    }
}
