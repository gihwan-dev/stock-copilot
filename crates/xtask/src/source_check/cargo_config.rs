use std::collections::BTreeSet;
use std::fmt;

pub(crate) const CONFIG_FILE: &str = "config.toml";

pub(crate) const MUTANTS_FILE: &str = "mutants.toml";

pub(crate) const RUNNER_TRIPLE: &str = "aarch64-apple-darwin";

pub(crate) const RUNNER_KEY: &str = "runner";

pub(crate) const EXPECTED_RUNNER: [&str; 3] = [
    "/usr/bin/sandbox-exec",
    "-p",
    "(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip \"localhost:*\"))(allow network-outbound (remote unix-socket))",
];

pub(crate) const MUTANTS_TEST_TOOL: (&str, &str) = ("test_tool", "nextest");

pub(crate) const MUTANTS_CAP_LINTS: &str = "cap_lints";

pub(crate) const MUTANTS_EXAMINE_GLOBS: &str = "examine_globs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConfigFile {
    Config,
    Mutants,
}

impl ConfigFile {
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            CONFIG_FILE => Some(Self::Config),
            MUTANTS_FILE => Some(Self::Mutants),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Config => "cargo-config",
            Self::Mutants => "cargo-mutants",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProblemKind {
    Unparsable,
    Key(String),
    Missing(String),
    Value(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ConfigProblem {
    pub(crate) file: ConfigFile,
    pub(crate) kind: ProblemKind,
}

impl fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.file.label();
        match &self.kind {
            ProblemKind::Unparsable => write!(f, "{label}:unparsable"),
            ProblemKind::Key(key) => write!(f, "{label}:key:{key}"),
            ProblemKind::Missing(key) => write!(f, "{label}:missing:{key}"),
            ProblemKind::Value(key) => write!(f, "{label}:value:{key}"),
        }
    }
}

fn problem(file: ConfigFile, kind: ProblemKind) -> ConfigProblem {
    ConfigProblem { file, kind }
}

fn expected_runner() -> toml::Value {
    toml::Value::Array(
        EXPECTED_RUNNER
            .into_iter()
            .map(|part| toml::Value::String(String::from(part)))
            .collect(),
    )
}

fn runner_problems(triple: &toml::Value, path: &str) -> Vec<ConfigProblem> {
    let file = ConfigFile::Config;
    let Some(table) = triple.as_table() else {
        return vec![problem(file, ProblemKind::Value(String::from(path)))];
    };
    let mut problems: Vec<ConfigProblem> = table
        .keys()
        .filter(|key| key.as_str() != RUNNER_KEY)
        .map(|key| problem(file, ProblemKind::Key(format!("{path}.{key}"))))
        .collect();
    let runner_path = format!("{path}.{RUNNER_KEY}");
    match table.get(RUNNER_KEY) {
        None => problems.push(problem(file, ProblemKind::Missing(runner_path))),
        Some(value) if *value != expected_runner() => {
            problems.push(problem(file, ProblemKind::Value(runner_path)));
        }
        Some(_) => {}
    }
    problems
}

fn config_table_problems(table: &toml::Table) -> Vec<ConfigProblem> {
    let file = ConfigFile::Config;
    let triple_path = format!("target.{RUNNER_TRIPLE}");
    let mut problems: Vec<ConfigProblem> = table
        .keys()
        .filter(|key| key.as_str() != "target")
        .map(|key| problem(file, ProblemKind::Key(key.clone())))
        .collect();
    match table.get("target").map(toml::Value::as_table) {
        None => problems.push(problem(
            file,
            ProblemKind::Missing(format!("{triple_path}.{RUNNER_KEY}")),
        )),
        Some(None) => problems.push(problem(file, ProblemKind::Value(String::from("target")))),
        Some(Some(targets)) => {
            problems.extend(
                targets
                    .keys()
                    .filter(|key| key.as_str() != RUNNER_TRIPLE)
                    .map(|key| problem(file, ProblemKind::Key(format!("target.{key}")))),
            );
            match targets.get(RUNNER_TRIPLE) {
                None => problems.push(problem(
                    file,
                    ProblemKind::Missing(format!("{triple_path}.{RUNNER_KEY}")),
                )),
                Some(triple) => problems.extend(runner_problems(triple, &triple_path)),
            }
        }
    }
    problems
}

fn string_list(value: &toml::Value) -> Option<Vec<&str>> {
    value
        .as_array()?
        .iter()
        .map(toml::Value::as_str)
        .collect::<Option<Vec<&str>>>()
        .filter(|items| !items.is_empty() && items.iter().all(|item| !item.trim().is_empty()))
}

fn mutants_table_problems(table: &toml::Table) -> Vec<ConfigProblem> {
    let file = ConfigFile::Mutants;
    let (tool_key, tool_value) = MUTANTS_TEST_TOOL;
    let known: BTreeSet<&str> = [tool_key, MUTANTS_CAP_LINTS, MUTANTS_EXAMINE_GLOBS]
        .into_iter()
        .collect();
    let mut problems: Vec<ConfigProblem> = table
        .keys()
        .filter(|key| !known.contains(key.as_str()))
        .map(|key| problem(file, ProblemKind::Key(key.clone())))
        .collect();
    let accepted = |key: &str, value: &toml::Value| match key {
        MUTANTS_CAP_LINTS => value.as_bool() == Some(true),
        MUTANTS_EXAMINE_GLOBS => string_list(value).is_some(),
        _ => value.as_str() == Some(tool_value),
    };
    for key in [tool_key, MUTANTS_CAP_LINTS, MUTANTS_EXAMINE_GLOBS] {
        match table.get(key) {
            None => problems.push(problem(file, ProblemKind::Missing(String::from(key)))),
            Some(value) if !accepted(key, value) => {
                problems.push(problem(file, ProblemKind::Value(String::from(key))));
            }
            Some(_) => {}
        }
    }
    problems
}

pub(crate) fn config_problems(file: ConfigFile, text: &str) -> BTreeSet<ConfigProblem> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return BTreeSet::from([problem(file, ProblemKind::Unparsable)]);
    };
    let problems = match file {
        ConfigFile::Config => config_table_problems(&table),
        ConfigFile::Mutants => mutants_table_problems(&table),
    };
    problems.into_iter().collect()
}
