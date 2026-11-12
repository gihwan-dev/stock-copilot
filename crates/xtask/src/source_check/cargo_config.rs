use std::collections::BTreeSet;
use std::fmt;

pub(crate) const MUTANTS_FILE: &str = "mutants.toml";

pub(crate) const NEXTEST_FILE: &str = ".config/nextest.toml";

pub(crate) const SANDBOX_SHELL: &str = "/bin/sh";

pub(crate) const SANDBOX_SCRIPT: &str = "p=\"$1\"; shift; if /usr/bin/sandbox-exec -p \"$p\" /usr/bin/true 2>/dev/null; then exec /usr/bin/sandbox-exec -p \"$p\" \"$@\"; fi; if [ \"${SANDBOX_RUNTIME:-}\" = 1 ]; then exec \"$@\"; fi; echo \"network-sandbox: sandbox-exec를 적용할 수 없고 바깥 샌드박스(SANDBOX_RUNTIME=1)도 없다\" >&2; exit 1";

pub(crate) const SANDBOX_PROFILE: &str = "(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip \"localhost:*\"))(allow network-outbound (remote unix-socket))(deny network-outbound (remote unix-socket (path-literal \"/private/var/run/mDNSResponder\")))";

pub(crate) const SANDBOX_WRAPPER: &str = "network-sandbox";

pub(crate) const MUTANTS_TEST_TOOL: (&str, &str) = ("test_tool", "nextest");

pub(crate) const MUTANTS_CAP_LINTS: &str = "cap_lints";

pub(crate) const MUTANTS_EXAMINE_GLOBS: &str = "examine_globs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConfigFile {
    Nextest,
    Mutants,
}

impl ConfigFile {
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            NEXTEST_FILE => Some(Self::Nextest),
            MUTANTS_FILE => Some(Self::Mutants),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Nextest => "nextest-config",
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

pub(crate) fn expected_nextest() -> toml::Table {
    let command = toml::Value::Array(
        [
            SANDBOX_SHELL,
            "-c",
            SANDBOX_SCRIPT,
            SANDBOX_WRAPPER,
            SANDBOX_PROFILE,
        ]
        .into_iter()
        .map(|part| toml::Value::String(String::from(part)))
        .collect(),
    );
    let mut wrapper = toml::Table::new();
    wrapper.insert(String::from("command"), command);
    wrapper.insert(String::from("target-runner"), toml::Value::from("ignore"));
    let mut wrappers = toml::Table::new();
    wrappers.insert(String::from(SANDBOX_WRAPPER), toml::Value::Table(wrapper));
    let mut scripts = toml::Table::new();
    scripts.insert(String::from("wrapper"), toml::Value::Table(wrappers));
    let mut rule = toml::Table::new();
    rule.insert(String::from("filter"), toml::Value::from("all()"));
    rule.insert(
        String::from("platform"),
        toml::Value::from("cfg(target_os = \"macos\")"),
    );
    rule.insert(
        String::from("run-wrapper"),
        toml::Value::from(SANDBOX_WRAPPER),
    );
    let mut default = toml::Table::new();
    default.insert(
        String::from("scripts"),
        toml::Value::Array(vec![toml::Value::Table(rule)]),
    );
    let mut profiles = toml::Table::new();
    profiles.insert(String::from("default"), toml::Value::Table(default));
    let mut table = toml::Table::new();
    table.insert(
        String::from("experimental"),
        toml::Value::Array(vec![toml::Value::from("wrapper-scripts")]),
    );
    table.insert(String::from("scripts"), toml::Value::Table(scripts));
    table.insert(String::from("profile"), toml::Value::Table(profiles));
    table
}

fn joined_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        String::from(key)
    } else {
        format!("{prefix}.{key}")
    }
}

fn table_differences(
    actual: &toml::Table,
    expected: &toml::Table,
    prefix: &str,
    problems: &mut Vec<ConfigProblem>,
) {
    let file = ConfigFile::Nextest;
    for (key, value) in actual {
        let path = joined_path(prefix, key);
        match (value, expected.get(key)) {
            (_, None) => problems.push(problem(file, ProblemKind::Key(path))),
            (toml::Value::Table(inner), Some(toml::Value::Table(wanted))) => {
                table_differences(inner, wanted, &path, problems);
            }
            (_, Some(wanted)) if wanted != value => {
                problems.push(problem(file, ProblemKind::Value(path)));
            }
            (_, Some(_)) => {}
        }
    }
    for key in expected.keys().filter(|key| !actual.contains_key(*key)) {
        problems.push(problem(
            file,
            ProblemKind::Missing(joined_path(prefix, key)),
        ));
    }
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
        ConfigFile::Nextest => {
            let mut found = Vec::new();
            table_differences(&table, &expected_nextest(), "", &mut found);
            found
        }
        ConfigFile::Mutants => mutants_table_problems(&table),
    };
    problems.into_iter().collect()
}

pub(crate) fn examine_globs(text: &str) -> Option<Vec<String>> {
    let table = text.parse::<toml::Table>().ok()?;
    string_list(table.get(MUTANTS_EXAMINE_GLOBS)?)
        .map(|items| items.into_iter().map(String::from).collect())
}
