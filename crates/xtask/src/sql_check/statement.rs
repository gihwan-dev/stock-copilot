use std::collections::BTreeSet;

use sqlparser::ast::{
    AssignmentTarget, BinaryOperator, ColumnOption, Expr, Insert, ObjectName, OnConflictAction,
    OnInsert, Query, SelectItem, SetExpr, Statement, TableConstraint, Value,
};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::keywords::Keyword;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer};

pub(crate) const DDL_STARTS: [&str; 3] = ["create", "alter", "drop"];

pub(crate) const DECIMAL_CHECK: &str = "NOT LIKE '*[^0-9.-]*'";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Migration,
    Query,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Problem {
    Unparsable,
    StatementCount(usize),
    DdlOutsideMigration,
    MoneyPlacement(String),
}

fn glob_as_like(tokens: Vec<Token>) -> Vec<Token> {
    let mut previous_not = false;
    tokens
        .into_iter()
        .map(|token| {
            let replaced = if previous_not && is_keyword(&token, Keyword::GLOB) {
                Token::make_keyword("LIKE")
            } else {
                token
            };
            if !is_space(&replaced) {
                previous_not = is_keyword(&replaced, Keyword::NOT);
            }
            replaced
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Piece {
    Parsed(Box<Statement>),
    Virtual(String),
}

fn is_keyword(token: &Token, keyword: Keyword) -> bool {
    if let Token::Word(word) = token {
        word.keyword == keyword
    } else {
        false
    }
}

fn is_space(token: &Token) -> bool {
    let Token::Whitespace(_) = token else {
        return false;
    };
    true
}

fn meaningful(tokens: &[Token]) -> Vec<&Token> {
    tokens.iter().filter(|token| !is_space(token)).collect()
}

fn split(tokens: Vec<Token>) -> Vec<Vec<Token>> {
    let mut groups = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut in_trigger = false;
    let mut cases = 0_usize;
    for token in tokens {
        let trigger = meaningful(&current)
            .iter()
            .take(4)
            .any(|word| is_keyword(word, Keyword::TRIGGER));
        if trigger && is_keyword(&token, Keyword::BEGIN) {
            in_trigger = true;
        }
        if is_keyword(&token, Keyword::CASE) {
            cases = cases.saturating_add(1);
        } else if is_keyword(&token, Keyword::END) {
            if cases > 0 {
                cases = cases.saturating_sub(1);
            } else {
                in_trigger = false;
            }
        }
        if token == Token::SemiColon && !in_trigger {
            groups.push(current);
            current = Vec::new();
        } else {
            current.push(token);
        }
    }
    groups.push(current);
    groups
        .into_iter()
        .filter(|group| !meaningful(group).is_empty())
        .collect()
}

fn pragma_value(group: Vec<Token>) -> Vec<Token> {
    let pragma = meaningful(&group)
        .first()
        .is_some_and(|word| is_keyword(word, Keyword::PRAGMA));
    if !pragma {
        return group;
    }
    let mut after_equals = false;
    group
        .into_iter()
        .map(|token| {
            if token == Token::Eq {
                after_equals = true;
            }
            if let Token::Word(word) = &token
                && after_equals
            {
                Token::SingleQuotedString(word.value.clone())
            } else {
                token
            }
        })
        .collect()
}

fn is_virtual(group: &[Token]) -> bool {
    let words = meaningful(group);
    words.len() > 2
        && words
            .first()
            .is_some_and(|word| is_keyword(word, Keyword::CREATE))
        && words
            .get(1)
            .is_some_and(|word| is_keyword(word, Keyword::VIRTUAL))
}

pub(crate) fn parse_text(text: &str) -> Result<Vec<Piece>, String> {
    let dialect = SQLiteDialect {};
    let tokens = Tokenizer::new(&dialect, text)
        .tokenize()
        .map_err(|error| error.to_string())?;
    let mut pieces = Vec::new();
    for group in split(glob_as_like(tokens)).into_iter().map(pragma_value) {
        let parsed = Parser::new(&dialect)
            .with_tokens(group.clone())
            .parse_statements();
        match parsed {
            Ok(statements) => {
                pieces.extend(
                    statements
                        .into_iter()
                        .map(|statement| Piece::Parsed(Box::new(statement))),
                );
            }
            Err(error) => {
                if is_virtual(&group) {
                    pieces.push(Piece::Virtual(
                        group.iter().map(ToString::to_string).collect::<String>(),
                    ));
                } else {
                    return Err(error.to_string());
                }
            }
        }
    }
    Ok(pieces)
}

pub(crate) fn parse(text: &str) -> Result<Vec<Piece>, Problem> {
    parse_text(text).map_err(|_error| Problem::Unparsable)
}

fn is_ddl(piece: &Piece) -> bool {
    let Piece::Parsed(statement) = piece else {
        return true;
    };
    let words = words(&statement.to_string());
    words
        .first()
        .is_some_and(|word| DDL_STARTS.contains(&word.as_str()))
}

fn words(text: &str) -> Vec<String> {
    Tokenizer::new(&SQLiteDialect {}, text)
        .tokenize()
        .map_or_else(
            |_error| Vec::new(),
            |tokens| {
                tokens
                    .into_iter()
                    .filter_map(|token| {
                        if let Token::Word(word) = token {
                            Some(word.value.to_lowercase())
                        } else {
                            None
                        }
                    })
                    .collect()
            },
        )
}

fn money_count(text: &str, money: &BTreeSet<String>) -> usize {
    words(text)
        .iter()
        .filter(|word| money.contains(word.as_str()))
        .count()
}

fn column_name(expr: &Expr) -> Option<String> {
    if let Expr::Identifier(ident) = expr {
        Some(ident.value.to_lowercase())
    } else if let Expr::CompoundIdentifier(parts) = expr {
        parts.last().map(|ident| ident.value.to_lowercase())
    } else {
        None
    }
}

fn object_column(name: &ObjectName) -> Option<String> {
    name.0
        .last()
        .and_then(|part| part.as_ident())
        .map(|ident| ident.value.to_lowercase())
}

fn is_placeholder(expr: &Expr) -> bool {
    if let Expr::Value(value) = expr
        && let Value::Placeholder(_) = value.value
    {
        true
    } else {
        false
    }
}

fn is_money(name: Option<String>, money: &BTreeSet<String>) -> bool {
    name.is_some_and(|column| money.contains(&column))
}

fn equality_count(expr: &Expr, money: &BTreeSet<String>) -> usize {
    if let Expr::Nested(inner) = expr {
        return equality_count(inner, money);
    }
    let Expr::BinaryOp { left, op, right } = expr else {
        return 0;
    };
    if *op == BinaryOperator::And || *op == BinaryOperator::Or {
        equality_count(left, money).saturating_add(equality_count(right, money))
    } else if *op == BinaryOperator::Eq {
        usize::from(
            (is_money(column_name(left), money) && is_placeholder(right))
                || (is_placeholder(left) && is_money(column_name(right), money)),
        )
    } else {
        0
    }
}

fn selection_count(selection: Option<&Expr>, money: &BTreeSet<String>) -> usize {
    selection.map_or(0, |expr| equality_count(expr, money))
}

fn projection_count(query: &Query, money: &BTreeSet<String>) -> usize {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return 0;
    };
    select
        .projection
        .iter()
        .filter(|item| {
            if let SelectItem::UnnamedExpr(expr) = item {
                is_money(column_name(expr), money)
            } else {
                false
            }
        })
        .count()
        .saturating_add(selection_count(select.selection.as_ref(), money))
}

fn assignment_count(assignments: &[sqlparser::ast::Assignment], money: &BTreeSet<String>) -> usize {
    assignments
        .iter()
        .filter(|assignment| match &assignment.target {
            AssignmentTarget::ColumnName(name) => {
                is_money(object_column(name), money) && is_placeholder(&assignment.value)
            }
            AssignmentTarget::Tuple(_) => false,
        })
        .count()
}

fn excluded_count(assignments: &[sqlparser::ast::Assignment], money: &BTreeSet<String>) -> usize {
    assignments
        .iter()
        .filter(|assignment| {
            if let AssignmentTarget::ColumnName(name) = &assignment.target
                && let Expr::CompoundIdentifier(parts) = &assignment.value
            {
                let target = object_column(name);
                parts.len() == 2
                    && parts
                        .first()
                        .is_some_and(|part| part.value.eq_ignore_ascii_case("excluded"))
                    && is_money(target.clone(), money)
                    && target == column_name(&assignment.value)
            } else {
                false
            }
        })
        .count()
        .saturating_mul(2)
}

fn insert_count(insert: &Insert, money: &BTreeSet<String>) -> usize {
    let columns = insert
        .columns
        .iter()
        .filter(|column| is_money(object_column(column), money))
        .count();
    let source = insert
        .source
        .as_deref()
        .map_or(0, |query| projection_count(query, money));
    let conflict = if let Some(OnInsert::OnConflict(conflict)) = &insert.on
        && let OnConflictAction::DoUpdate(update) = &conflict.action
    {
        assignment_count(&update.assignments, money)
            .saturating_add(excluded_count(&update.assignments, money))
    } else {
        0
    };
    columns.saturating_add(source).saturating_add(conflict)
}

fn table_count(statement: &Statement, money: &BTreeSet<String>) -> usize {
    let Statement::CreateTable(table) = statement else {
        return 0;
    };
    let mut allowed = 0_usize;
    for column in &table.columns {
        if money.contains(&column.name.value.to_lowercase()) {
            allowed = allowed.saturating_add(1);
        }
        for option in &column.options {
            if let ColumnOption::Check(check) = &option.option {
                allowed = allowed.saturating_add(money_count(&check.expr.to_string(), money));
            }
        }
    }
    for constraint in &table.constraints {
        if let TableConstraint::Check(check) = constraint {
            allowed = allowed.saturating_add(money_count(&check.expr.to_string(), money));
        }
    }
    allowed
}

fn allowed_count(statement: &Statement, money: &BTreeSet<String>) -> usize {
    if let Statement::Query(query) = statement {
        projection_count(query, money)
    } else if let Statement::Insert(insert) = statement {
        insert_count(insert, money)
    } else if let Statement::Update(update) = statement {
        assignment_count(&update.assignments, money)
            .saturating_add(selection_count(update.selection.as_ref(), money))
    } else if let Statement::Delete(delete) = statement {
        selection_count(delete.selection.as_ref(), money)
    } else {
        table_count(statement, money)
    }
}

fn placement(piece: &Piece, money: &BTreeSet<String>) -> Option<Problem> {
    let (text, allowed) = match piece {
        Piece::Parsed(statement) => (statement.to_string(), allowed_count(statement, money)),
        Piece::Virtual(text) => (text.clone(), 0),
    };
    let found = words(&text)
        .into_iter()
        .filter(|word| money.contains(word.as_str()))
        .collect::<Vec<String>>();
    if found.len() > allowed {
        found.into_iter().next().map(Problem::MoneyPlacement)
    } else {
        None
    }
}

pub(crate) fn check(text: &str, source: Source, money: &BTreeSet<String>) -> Vec<Problem> {
    let statements = match parse(text) {
        Ok(statements) => statements,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    match source {
        Source::Query => {
            if statements.len() != 1 {
                problems.push(Problem::StatementCount(statements.len()));
            }
            if statements.iter().any(is_ddl) {
                problems.push(Problem::DdlOutsideMigration);
            }
        }
        Source::Migration => {}
    }
    problems.extend(
        statements
            .iter()
            .filter_map(|piece| placement(piece, money)),
    );
    problems
}

fn decimal_checked(name: &str, check: &str) -> bool {
    let normalized = format!(
        " {}",
        check
            .replace('(', " ( ")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    );
    normalized.contains(&format!(" {name} {DECIMAL_CHECK}"))
}

fn table_checks(table: &sqlparser::ast::CreateTable) -> Vec<String> {
    table
        .constraints
        .iter()
        .filter_map(|constraint| {
            if let TableConstraint::Check(check) = constraint {
                Some(check.expr.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn column_checked(column: &sqlparser::ast::ColumnDef, checks: &[String]) -> bool {
    let column_name = &column.name.value;
    column.options.iter().any(|option| {
        if let ColumnOption::Check(check) = &option.option {
            decimal_checked(column_name, &check.expr.to_string())
        } else {
            false
        }
    }) || checks
        .iter()
        .any(|check| decimal_checked(column_name, check))
}

pub(crate) fn money_columns(migrations: &[(String, String)]) -> Result<BTreeSet<String>, String> {
    let mut columns = BTreeSet::new();
    for (name, text) in migrations {
        let statements = parse_text(text)
            .map_err(|error| format!("{name}: 마이그레이션을 SQL 파서로 읽지 못했다: {error}"))?;
        for statement in statements {
            let Piece::Parsed(parsed) = statement else {
                continue;
            };
            let Statement::CreateTable(table) = *parsed else {
                continue;
            };
            let checks = table_checks(&table);
            columns.extend(
                table
                    .columns
                    .iter()
                    .filter(|column| column_checked(column, &checks))
                    .map(|column| column.name.value.to_lowercase()),
            );
        }
    }
    Ok(columns)
}
