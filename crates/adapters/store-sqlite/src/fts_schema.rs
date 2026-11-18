use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
enum Mode {
    Syntax,
    Single,
    Double,
    Backtick,
    Bracket,
    Line,
    Block,
}
struct Scanner {
    mode: Mode,
    current: String,
    items: Vec<String>,
}
impl Scanner {
    fn syntax(&mut self, symbol: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        match symbol {
            '\'' => {
                self.mode = Mode::Single;
                self.current.push(symbol);
            }
            '"' => {
                self.mode = Mode::Double;
                self.current.push(symbol);
            }
            '`' => {
                self.mode = Mode::Backtick;
                self.current.push(symbol);
            }
            '[' => {
                self.mode = Mode::Bracket;
                self.current.push(symbol);
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                self.mode = Mode::Line;
                self.current.push(' ');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                self.mode = Mode::Block;
                self.current.push(' ');
            }
            ';' => {
                self.items.push(self.current.clone());
                self.current.clear();
            }
            _ => self.current.push(symbol),
        }
    }
    fn quoted(
        &mut self,
        symbol: char,
        quote: char,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) {
        self.current.push(symbol);
        if symbol != quote {
            return;
        }
        if chars.next_if_eq(&quote).is_some() {
            self.current.push(quote);
        } else {
            self.mode = Mode::Syntax;
        }
    }
    fn advance(&mut self, symbol: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        match self.mode {
            Mode::Syntax => self.syntax(symbol, chars),
            Mode::Single => self.quoted(symbol, '\'', chars),
            Mode::Double => self.quoted(symbol, '"', chars),
            Mode::Backtick => {
                self.current.push(symbol);
                if symbol == '`' {
                    self.mode = Mode::Syntax;
                }
            }
            Mode::Bracket => {
                self.current.push(symbol);
                if symbol == ']' {
                    self.mode = Mode::Syntax;
                }
            }
            Mode::Line => {
                if symbol == '\n' {
                    self.current.push(' ');
                    self.mode = Mode::Syntax;
                }
            }
            Mode::Block => {
                if symbol == '*' && chars.next_if_eq(&'/').is_some() {
                    self.current.push(' ');
                    self.mode = Mode::Syntax;
                }
            }
        }
    }
    fn finish(mut self) -> rusqlite::Result<Vec<String>> {
        match self.mode {
            Mode::Syntax | Mode::Line => {
                self.items.push(self.current);
                Ok(self.items)
            }
            Mode::Single | Mode::Double | Mode::Backtick | Mode::Bracket | Mode::Block => {
                Err(rusqlite::Error::InvalidQuery)
            }
        }
    }
}
fn statements(source: &str) -> rusqlite::Result<Vec<String>> {
    let mut scanner = Scanner {
        mode: Mode::Syntax,
        current: String::new(),
        items: Vec::new(),
    };
    let mut chars = source.chars().peekable();
    while let Some(symbol) = chars.next() {
        scanner.advance(symbol, &mut chars);
    }
    scanner.finish()
}
fn virtual_name(statement: &str) -> Option<String> {
    let mut words = statement.split_ascii_whitespace();
    if !words.next()?.eq_ignore_ascii_case("create")
        || !words.next()?.eq_ignore_ascii_case("virtual")
        || !words.next()?.eq_ignore_ascii_case("table")
    {
        return None;
    }
    let mut spelling = words.next()?;
    if spelling.eq_ignore_ascii_case("if") {
        if !words.next()?.eq_ignore_ascii_case("not")
            || !words.next()?.eq_ignore_ascii_case("exists")
        {
            return None;
        }
        spelling = words.next()?;
    }
    let name = spelling
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(spelling);
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    if !words.next()?.eq_ignore_ascii_case("using") {
        return None;
    }
    let module = words.next()?.split('(').next()?;
    if !module.eq_ignore_ascii_case("fts5") {
        return None;
    }
    Some(name.to_owned())
}
fn declared(migrations: &[(&str, &str)]) -> rusqlite::Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for (_, source) in migrations {
        for statement in statements(source)? {
            if let Some(name) = virtual_name(&statement) {
                names.insert(name);
            }
        }
    }
    Ok(names)
}
fn ordinary_table_name(statement: &str) -> Option<&str> {
    let mut words = statement
        .split(|symbol: char| !symbol.is_alphanumeric() && symbol != '_')
        .filter(|word| !word.is_empty());
    if !words.next()?.eq_ignore_ascii_case("create") {
        return None;
    }
    let mut word = words.next()?;
    if word.eq_ignore_ascii_case("temp") || word.eq_ignore_ascii_case("temporary") {
        word = words.next()?;
    }
    if !word.eq_ignore_ascii_case("table") {
        return None;
    }
    word = words.next()?;
    if word.eq_ignore_ascii_case("if") {
        if !words.next()?.eq_ignore_ascii_case("not")
            || !words.next()?.eq_ignore_ascii_case("exists")
        {
            return None;
        }
        word = words.next()?;
    }
    if word.eq_ignore_ascii_case("main") || word.eq_ignore_ascii_case("temp") {
        word = words.next()?;
    }
    Some(word)
}
fn reject_declared_shadow_tables(
    migrations: &[(&str, &str)],
    virtuals: &BTreeSet<String>,
) -> rusqlite::Result<()> {
    for (_, source) in migrations {
        for statement in statements(source)? {
            let Some(name) = ordinary_table_name(&statement) else {
                continue;
            };
            if virtuals.iter().any(|parent| shadow_name(name, parent)) {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
    }
    Ok(())
}
fn shadow_name(name: &str, parent: &str) -> bool {
    ["_data", "_idx", "_content", "_docsize", "_config"]
        .iter()
        .any(|suffix| name.eq_ignore_ascii_case(&[parent, suffix].concat()))
}
struct Table {
    kind: String,
    strict: i64,
    columns: i64,
}
fn content_columns(body: &str, columns: i64, parent_columns: i64) -> bool {
    if columns.checked_add(1) != Some(parent_columns) {
        return false;
    }
    let Some(values) = body.strip_suffix(')') else {
        return false;
    };
    let mut fields = values.split(", ");
    if fields.next() != Some("id INTEGER PRIMARY KEY") {
        return false;
    }
    fields
        .enumerate()
        .all(|(index, field)| field == ["c", &index.to_string()].concat())
}
fn shadow_ddl(name: &str, parent: &str, ddl: &str, columns: i64, parent_columns: i64) -> bool {
    let Some(suffix) = name.strip_prefix(parent) else {
        return false;
    };
    let Some((header, body)) = ddl.split_once('(') else {
        return false;
    };
    let mut head = header.split_ascii_whitespace();
    if head.next() != Some("CREATE") || head.next() != Some("TABLE") {
        return false;
    }
    let expected_name = head
        .next()
        .and_then(|value| value.strip_prefix('\''))
        .and_then(|value| value.strip_suffix('\''));
    if expected_name != Some(name) || head.next().is_some() {
        return false;
    }
    match suffix {
        "_data" => body == "id INTEGER PRIMARY KEY, block BLOB)",
        "_idx" => body == "segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID",
        "_config" => body == "k PRIMARY KEY, v) WITHOUT ROWID",
        "_docsize" => {
            body == "id INTEGER PRIMARY KEY, sz BLOB)"
                || body == "id INTEGER PRIMARY KEY, sz BLOB, origin INTEGER)"
        }
        "_content" => content_columns(body, columns, parent_columns),
        _ => false,
    }
}
fn tables(connection: &Connection) -> rusqlite::Result<BTreeMap<String, Table>> {
    let mut objects = BTreeMap::new();
    for row in connection
        .prepare(crate::sql::migration::TABLES)?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Table {
                    strict: row.get(1)?,
                    kind: row.get(2)?,
                    columns: row.get(3)?,
                },
            ))
        })?
    {
        let (name, table) = row?;
        objects.insert(name, table);
    }
    Ok(objects)
}
fn definitions(connection: &Connection) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut ddls = BTreeMap::new();
    for row in connection
        .prepare(crate::sql::migration::SCHEMA)?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
    {
        let (name, ddl, kind) = row?;
        match kind.as_str() {
            "table" => {
                ddls.insert(name, ddl);
            }
            "index" | "view" | "trigger" => {}
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
    }
    Ok(ddls)
}
fn authorized_virtuals(
    names: BTreeSet<String>,
    objects: &BTreeMap<String, Table>,
    ddls: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    names
        .into_iter()
        .filter(|name| {
            objects
                .get(name)
                .zip(ddls.get(name))
                .is_some_and(|(table, ddl)| {
                    table.kind == "virtual" && virtual_name(ddl).as_deref() == Some(name.as_str())
                })
        })
        .collect()
}
fn validate_objects(
    objects: &BTreeMap<String, Table>,
    ddls: &BTreeMap<String, String>,
    virtuals: &BTreeSet<String>,
) -> rusqlite::Result<()> {
    for (name, table) in objects {
        let ddl = ddls.get(name).ok_or(rusqlite::Error::InvalidQuery)?;
        match (table.kind.as_str(), table.strict) {
            ("table", 1) => crate::schema::validate(ddl)?,
            ("virtual", 0) if virtuals.contains(name) => {}
            ("shadow", 0)
                if virtuals.iter().any(|parent| {
                    objects.get(parent).is_some_and(|value| {
                        shadow_ddl(name, parent, ddl, table.columns, value.columns)
                    })
                }) => {}
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
    }
    Ok(())
}
pub(crate) fn validate(
    connection: &Connection,
    migrations: &[(&str, &str)],
) -> rusqlite::Result<()> {
    let objects = tables(connection)?;
    let ddls = definitions(connection)?;
    if objects.len() != ddls.len() || ddls.keys().any(|name| !objects.contains_key(name)) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let names = declared(migrations)?;
    reject_declared_shadow_tables(migrations, &names)?;
    let virtuals = authorized_virtuals(names, &objects, &ddls);
    validate_objects(&objects, &ddls, &virtuals)?;
    if !virtuals.is_empty() {
        let outcome: String =
            connection.query_row(crate::sql::migration::QUICK_CHECK, [], |row| row.get(0))?;
        if outcome != "ok" {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}
