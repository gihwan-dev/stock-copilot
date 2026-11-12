use std::fmt;

pub(crate) const MONEY_FUNCTIONS: [&str; 8] =
    ["sum", "avg", "total", "min", "max", "abs", "round", "cast"];

pub(crate) const ARITHMETIC_OPERATORS: [&str; 5] = ["+", "-", "*", "/", "%"];

pub(crate) const COMPARISON_OPERATORS: [&str; 4] = ["<", ">", "<=", ">="];

pub(crate) const BETWEEN: &str = "between";

pub(crate) const NON_MONEY_RESULT_FUNCTIONS: [&str; 3] = ["length", "typeof", "count"];

pub(crate) const ORDER_BY_END: [&str; 1] = ["limit"];

pub(crate) const ASSEMBLY_MACROS: [&str; 8] = [
    "format",
    "format_args",
    "concat",
    "write",
    "writeln",
    "include_str",
    "include_bytes",
    "stringify",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follow {
    Any,
    Words(&'static [&'static str]),
    Contains(&'static str),
}

pub(crate) const STATEMENT_STARTS: [(&str, Follow); 22] = [
    ("select", Follow::Any),
    ("insert", Follow::Words(&["into", "or"])),
    ("replace", Follow::Words(&["into"])),
    ("update", Follow::Contains(" set ")),
    ("delete", Follow::Words(&["from"])),
    (
        "create",
        Follow::Words(&[
            "table",
            "index",
            "unique",
            "view",
            "trigger",
            "virtual",
            "temp",
            "temporary",
        ]),
    ),
    ("alter", Follow::Words(&["table"])),
    (
        "drop",
        Follow::Words(&["table", "index", "view", "trigger"]),
    ),
    ("with", Follow::Contains(" as (")),
    ("pragma", Follow::Any),
    ("vacuum", Follow::Any),
    ("attach", Follow::Contains(" as ")),
    ("detach", Follow::Any),
    (
        "begin",
        Follow::Words(&["transaction", "immediate", "deferred", "exclusive"]),
    ),
    ("commit", Follow::Words(&["transaction"])),
    ("end", Follow::Words(&["transaction"])),
    ("rollback", Follow::Words(&["transaction", "to"])),
    ("savepoint", Follow::Any),
    ("release", Follow::Words(&["savepoint"])),
    ("analyze", Follow::Any),
    ("reindex", Follow::Any),
    ("explain", Follow::Any),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    MoneyFunction(String),
    MoneyArithmetic(String),
    MoneyComparison(String),
    MoneyOrderBy,
    OrderByPosition,
    NotConstant,
    OutsideStore,
    Assembly(String),
    Unparsable,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MoneyFunction(name) => write!(f, "sql-money:function:{name}"),
            Self::MoneyArithmetic(operator) => write!(f, "sql-money:arithmetic:{operator}"),
            Self::MoneyComparison(operator) => write!(f, "sql-money:comparison:{operator}"),
            Self::MoneyOrderBy => f.write_str("sql-money:order-by"),
            Self::OrderByPosition => f.write_str("sql:order-by-position"),
            Self::NotConstant => f.write_str("sql:not-constant"),
            Self::OutsideStore => f.write_str("sql:outside-store"),
            Self::Assembly(name) => write!(f, "sql:assembly:{name}"),
            Self::Unparsable => f.write_str("sql:unparsable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recognition {
    Sql,
    Text,
}

fn follows(follow: Follow, second: &str, lowered: &str) -> bool {
    match follow {
        Follow::Any => true,
        Follow::Words(words) => words.contains(&second),
        Follow::Contains(fragment) => lowered.contains(fragment),
    }
}

fn strip_leading(text: &str) -> &str {
    let mut rest = text;
    loop {
        let trimmed = rest.trim_start();
        let next = if let Some(inside) = trimmed.strip_prefix("/*") {
            inside.split_once("*/").map_or("", |(_, after)| after)
        } else if let Some(inside) = trimmed.strip_prefix("--") {
            inside.split_once('\n').map_or("", |(_, after)| after)
        } else if let Some(inside) = trimmed.strip_prefix('(') {
            inside
        } else {
            return trimmed;
        };
        rest = next;
    }
}

pub(crate) fn recognize(text: &str) -> Recognition {
    let lowered = strip_leading(text)
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase();
    let statement = lowered.trim_end_matches(';').trim_end();
    let mut words = statement.split([' ', '(']);
    let first = words.next().unwrap_or("");
    let second = words.next();
    let started = second.is_some_and(|word| {
        STATEMENT_STARTS
            .iter()
            .any(|(keyword, follow)| *keyword == first && follows(*follow, word, statement))
    });
    if started {
        Recognition::Sql
    } else {
        Recognition::Text
    }
}
