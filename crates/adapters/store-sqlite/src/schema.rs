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

pub(crate) fn validate(ddl: &str) -> rusqlite::Result<()> {
    if ddl.contains("--") || ddl.contains("/*") || ddl.contains('"') || ddl.contains('`') {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let mut normalized = String::new();
    let mut previous = ' ';
    for symbol in ddl.chars() {
        if (previous.is_lowercase() || previous.is_ascii_digit()) && symbol.is_uppercase() {
            normalized.push('_');
        }
        normalized.extend(symbol.to_lowercase());
        previous = symbol;
    }
    let (_, after_open) = normalized
        .split_once('(')
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let (columns_body, _) = after_open
        .rsplit_once(')')
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let mut clauses = Vec::new();
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut current = String::new();
    for symbol in columns_body.chars() {
        if symbol == '\'' {
            quoted = !quoted;
        }
        if !quoted {
            match symbol {
                '(' => {
                    depth = depth.saturating_add(1);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                }
                ',' if depth == 0 => {
                    clauses.push(current.clone());
                    current.clear();
                    continue;
                }
                _ => {}
            }
        }
        current.push(symbol);
    }
    clauses.push(current);
    let mut money = Vec::new();
    for clause in &clauses {
        let body = clause.as_str();
        let name = body.split_whitespace().next().unwrap_or("");
        if name.starts_with('[') || name.starts_with('\'') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if name
            .split(|symbol: char| !symbol.is_alphanumeric())
            .any(|part| MONEY_SEGMENTS.contains(&part))
        {
            money.push((name, body));
        }
    }
    if money.is_empty() {
        return Ok(());
    }
    if ddl.contains("--") || ddl.contains("/*") || ddl.contains('"') || ddl.contains('`') {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let currency = clauses.iter().any(|clause| {
        clause
            .split_whitespace()
            .take(4)
            .eq(["currency", "text", "not", "null"])
    });
    for (name, body) in money {
        let words = body.split_whitespace().collect::<Vec<_>>();
        let expected = decimal_check(name);
        let mut compact = String::new();
        let mut inside_text = false;
        for symbol in body.chars() {
            if symbol == '\'' {
                inside_text = !inside_text;
            }
            if inside_text || !symbol.is_whitespace() {
                compact.push(symbol);
            }
        }
        if words.get(1) != Some(&"text") || !currency || !compact.ends_with(&expected) {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

pub(crate) fn decimal_check(name: &str) -> String {
    [
        "check(",
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
}
