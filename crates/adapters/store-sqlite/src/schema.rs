const MONEY_SEGMENTS: &[&str] = &[
    "amount",
    "balance",
    "cash",
    "cost",
    "deposit",
    "dividend",
    "exposure",
    "fee",
    "gain",
    "loss",
    "notional",
    "pct",
    "percent",
    "pnl",
    "price",
    "principal",
    "probability",
    "proceeds",
    "profit",
    "qty",
    "quantity",
    "rate",
    "ratio",
    "return",
    "shares",
    "tax",
    "threshold",
    "valuation",
    "value",
    "weight",
    "withdrawal",
];

#[derive(Clone, Copy)]
enum ColumnKind {
    Plain,
    Monetary,
    Scalar,
}

#[derive(Clone, Copy)]
enum CurrencyColumn {
    Present,
    Missing,
}

const SCALAR_SEGMENTS: &[&str] = &[
    "pct",
    "percent",
    "probability",
    "qty",
    "quantity",
    "rate",
    "ratio",
    "shares",
    "threshold",
    "weight",
];

pub(crate) fn validate(ddl: &str) -> rusqlite::Result<()> {
    let clauses = columns(table_body(ddl)?)?;
    let currency = currency_column(&clauses);
    for body in &clauses {
        let name = body.split_whitespace().next().unwrap_or("");
        if name.starts_with(['[', '\'', '"', '`']) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        match column_kind(name) {
            ColumnKind::Plain => continue,
            ColumnKind::Monetary => match currency {
                CurrencyColumn::Missing => return Err(rusqlite::Error::InvalidQuery),
                CurrencyColumn::Present => {}
            },
            ColumnKind::Scalar => {}
        }
        validate_decimal(name, body)?;
    }
    Ok(())
}

fn table_body(ddl: &str) -> rusqlite::Result<&str> {
    let (header, after_open) = ddl.split_once('(').ok_or(rusqlite::Error::InvalidQuery)?;
    if !header.is_ascii() || header.split_ascii_whitespace().count() != 3 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let table = header
        .split_whitespace()
        .nth(2)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let identifier = table
        .strip_prefix('"')
        .and_then(|name| name.strip_suffix('"'))
        .unwrap_or(table);
    if identifier.is_empty()
        || !identifier
            .chars()
            .all(|symbol| symbol.is_ascii_alphanumeric() || symbol == '_' || symbol == '$')
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    after_open
        .rsplit_once(')')
        .map(|(body, _)| body)
        .ok_or(rusqlite::Error::InvalidQuery)
}

#[derive(Clone, Copy)]
enum TextContext {
    Syntax,
    Literal,
}

fn columns(body: &str) -> rusqlite::Result<Vec<String>> {
    let mut clauses = Vec::new();
    let mut depth = 0_u32;
    let mut context = TextContext::Syntax;
    let mut current = String::new();
    let mut remaining = body.chars().peekable();
    while let Some(symbol) = remaining.next() {
        match context {
            TextContext::Literal => {
                current.push(symbol);
                literal_symbol(symbol, &mut remaining, &mut context, &mut current);
                continue;
            }
            TextContext::Syntax => match symbol {
                non_ascii if !non_ascii.is_ascii() => return Err(rusqlite::Error::InvalidQuery),
                '\'' => context = TextContext::Literal,
                '"' | '`' | '[' => return Err(rusqlite::Error::InvalidQuery),
                '-' if remaining.peek() == Some(&'-') => return Err(rusqlite::Error::InvalidQuery),
                '/' if remaining.peek() == Some(&'*') => return Err(rusqlite::Error::InvalidQuery),
                '(' => depth = depth.saturating_add(1),
                ')' => depth = depth.checked_sub(1).ok_or(rusqlite::Error::InvalidQuery)?,
                ',' if depth == 0 => {
                    clauses.push(current.clone());
                    current.clear();
                    continue;
                }
                _ => {}
            },
        }
        current.push(symbol);
    }
    if depth != 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    match context {
        TextContext::Literal => Err(rusqlite::Error::InvalidQuery),
        TextContext::Syntax => {
            clauses.push(current);
            Ok(clauses)
        }
    }
}

fn literal_symbol(
    symbol: char,
    remaining: &mut std::iter::Peekable<std::str::Chars<'_>>,
    context: &mut TextContext,
    current: &mut String,
) {
    if symbol == '\'' {
        match remaining.next_if_eq(&'\'') {
            Some(quote) => current.push(quote),
            None => *context = TextContext::Syntax,
        }
    }
}

fn segments(name: &str) -> Vec<String> {
    let mut normalized = String::new();
    let mut previous = ' ';
    for symbol in name.chars() {
        if (previous.is_lowercase() || previous.is_ascii_digit()) && symbol.is_uppercase() {
            normalized.push('_');
        }
        normalized.extend(symbol.to_lowercase());
        previous = symbol;
    }
    normalized
        .split(|symbol: char| !symbol.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect()
}

fn column_kind(name: &str) -> ColumnKind {
    let parts = segments(name);
    match parts
        .iter()
        .rev()
        .find(|part| MONEY_SEGMENTS.contains(&part.as_str()))
    {
        None => ColumnKind::Plain,
        Some(unit) => {
            if SCALAR_SEGMENTS.contains(&unit.as_str()) {
                ColumnKind::Scalar
            } else {
                ColumnKind::Monetary
            }
        }
    }
}

fn currency_column(clauses: &[String]) -> CurrencyColumn {
    if clauses.iter().any(|clause| {
        clause
            .split_whitespace()
            .take(4)
            .map(str::to_ascii_lowercase)
            .eq(["currency", "text", "not", "null"])
    }) {
        CurrencyColumn::Present
    } else {
        CurrencyColumn::Missing
    }
}

fn validate_decimal(name: &str, body: &str) -> rusqlite::Result<()> {
    let lowered = body.to_ascii_lowercase();
    let words = lowered.split_whitespace().collect::<Vec<_>>();
    let expected = decimal_check(&name.to_ascii_lowercase());
    let mut compact = String::new();
    let mut inside_text = false;
    for symbol in lowered.chars() {
        if symbol == '\'' {
            inside_text = !inside_text;
        }
        if inside_text || !symbol.is_whitespace() {
            compact.push(symbol);
        }
    }
    if words.get(1) != Some(&"text") || !compact.ends_with(&expected) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn decimal_check(name: &str) -> String {
    [
        "check(instr(",
        name,
        ",char(0))=0and",
        name,
        "notglob'*[^0-9.-]*'and",
        name,
        "glob'*[0-9]*'and",
        name,
        "notglob'?*-*'and",
        name,
        "notglob'*.*.*'and",
        name,
        "notglob'.*'and",
        name,
        "notglob'-.*'and",
        name,
        "notglob'*.')",
    ]
    .join("")
}

#[cfg(test)]
mod tests {
    #[test]
    fn decimal_schema_requires_type_currency_and_exact_check() {
        let good = crate::sql::tests::GOOD_MONEY;
        super::validate(good).expect("canonical decimal check");
        for bad in [
            good.replace("TEXT NOT NULL CHECK", "INTEGER NOT NULL CHECK"),
            good.replace("currency TEXT NOT NULL", "currency TEXT"),
            good.replace("CHECK (", "CHECK (1 OR "),
            good.replace("amount NOT GLOB", "payload NOT GLOB"),
            good.replace("amount", "unitPrice")
                .replace("unitPrice NOT GLOB", "payload NOT GLOB"),
            good.replace("amount", "[amount]"),
            good.replace("[^0-9.-]", "[^0-9. -]"),
            good.replace(
                "amount TEXT NOT NULL CHECK",
                "amount$tag TEXT NOT NULL CHECK",
            ),
        ] {
            assert!(super::validate(&bad).is_err(), "{bad}");
        }
        super::validate(crate::sql::tests::BAD_MONEY).expect_err("missing check");
    }

    #[test]
    fn decimal_check_accepts_only_documented_decimal_grammar() {
        let connection = rusqlite::Connection::open_in_memory().expect("memory");
        connection
            .execute_batch(crate::sql::tests::GOOD_MONEY)
            .expect("fixture");
        for value in ["0", "13", "-47", "123.456", "-0.75", "00.10"] {
            connection
                .execute(crate::sql::tests::INSERT_MONEY, [value])
                .expect("decimal");
        }
        for value in [
            "", "-", ".", "-.", "+1", "--1", "1-2", ".1", "-.1", "1.", "1.2.3", "1e2", " 1", "１",
            "a1",
        ] {
            connection
                .execute(crate::sql::tests::INSERT_MONEY, [value])
                .expect_err("not decimal");
        }
    }
    #[test]
    fn numeric_name_policy_matches_protected_harness_segments() {
        let policy = std::fs::read_to_string("../../../harness/sql-check/money-columns.toml")
            .expect("protected name policy");
        let names = policy
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix('"')
                    .and_then(|after| after.split_once('"'))
                    .map(|(name, _)| name)
            })
            .collect::<Vec<_>>();
        assert_eq!(names, super::MONEY_SEGMENTS);
    }
    #[test]
    fn malformed_column_boundaries_fail_closed() {
        for body in ["a INTEGER)", "a INTEGER(", "a TEXT DEFAULT '"] {
            super::columns(body).expect_err("malformed boundary");
        }
        super::columns("a INTEGER DEFAULT (17), b TEXT DEFAULT '한글''가격'")
            .expect("balanced supported body");
    }
}
