use std::collections::BTreeSet;
use std::path::Path;

use super::lexer::{Kind, Token, tokenize};
use super::rules::{
    ARITHMETIC_OPERATORS, BETWEEN, COMPARISON_OPERATORS, MONEY_FUNCTIONS,
    NON_MONEY_RESULT_FUNCTIONS, ORDER_BY_END, Rule,
};
use crate::check_support::read_text;

pub(crate) const MONEY_COLUMNS_FILE: &str = "harness/sql-check/money-columns.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Money {
    Money,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Lower,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Start,
    Inside,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoneyNames {
    segments: BTreeSet<String>,
}

fn name_segments(name: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut previous = Case::Other;
    for next in name.chars() {
        let boundary = !next.is_alphanumeric() || (next.is_uppercase() && previous == Case::Lower);
        if boundary && !current.is_empty() {
            segments.push(current.to_lowercase());
            current = String::new();
        }
        if next.is_alphanumeric() {
            current.push(next);
        }
        previous = if next.is_lowercase() || next.is_ascii_digit() {
            Case::Lower
        } else {
            Case::Other
        };
    }
    if !current.is_empty() {
        segments.push(current.to_lowercase());
    }
    segments
}

impl MoneyNames {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let table: toml::Table = read_text(&root.join(MONEY_COLUMNS_FILE))?
            .parse()
            .map_err(|error| format!("{MONEY_COLUMNS_FILE}: {error}"))?;
        let segments = table
            .get("segments")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{MONEY_COLUMNS_FILE}: segments 목록이 없다"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|text| {
                        !text.is_empty()
                            && text
                                .chars()
                                .all(|next| next.is_ascii_lowercase() || next.is_ascii_digit())
                    })
                    .map(String::from)
                    .ok_or_else(|| {
                        format!("{MONEY_COLUMNS_FILE}: segments는 소문자 영숫자 문자열이다")
                    })
            })
            .collect::<Result<BTreeSet<String>, String>>()?;
        let reason = table.get("reason").and_then(toml::Value::as_str);
        if segments.is_empty() || reason.is_none_or(|text| text.trim().is_empty()) {
            return Err(format!(
                "{MONEY_COLUMNS_FILE}: segments와 reason이 비어 있으면 안 된다"
            ));
        }
        Ok(Self { segments })
    }

    fn classify(&self, name: &str) -> Money {
        if name_segments(name)
            .iter()
            .any(|segment| self.segments.contains(segment))
        {
            Money::Money
        } else {
            Money::Plain
        }
    }
}

struct Statement {
    tokens: Vec<Token>,
    partners: Vec<Option<usize>>,
    depths: Vec<usize>,
    money: Vec<Money>,
}

fn next_index(index: usize) -> Option<usize> {
    index.checked_add(1)
}

fn pair_parentheses(tokens: &[Token]) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut partners = vec![None; tokens.len()];
    let mut depths = Vec::with_capacity(tokens.len());
    let mut open = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.is_symbol(")")
            && let Some(start) = open.pop()
        {
            if let Some(slot) = partners.get_mut(start) {
                *slot = Some(index);
            }
            if let Some(slot) = partners.get_mut(index) {
                *slot = Some(start);
            }
        }
        depths.push(open.len());
        if token.is_symbol("(") {
            open.push(index);
        }
    }
    (partners, depths)
}

impl Statement {
    fn new(sql: &str, names: &MoneyNames) -> Self {
        let tokens = tokenize(sql);
        let (partners, depths) = pair_parentheses(&tokens);
        let mut statement = Self {
            money: vec![Money::Plain; tokens.len()],
            tokens,
            partners,
            depths,
        };
        statement.mark_columns(names, &BTreeSet::new());
        let aliases = statement.aliases();
        statement.mark_columns(names, &aliases);
        statement
    }

    fn token(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    fn partner(&self, index: usize) -> Option<usize> {
        self.partners.get(index).copied().flatten()
    }

    fn depth(&self, index: usize) -> usize {
        self.depths.get(index).copied().unwrap_or(0)
    }

    fn is_qualifier(&self, index: usize) -> bool {
        next_index(index)
            .and_then(|next| self.token(next))
            .is_some_and(|token| token.is_symbol("."))
    }

    fn mark_columns(&mut self, names: &MoneyNames, aliases: &BTreeSet<String>) {
        let marks: Vec<Money> = self
            .tokens
            .iter()
            .enumerate()
            .map(|(index, token)| {
                if matches_name(token)
                    && !self.is_qualifier(index)
                    && (names.classify(&token.text) == Money::Money
                        || aliases.contains(&token.lowered()))
                {
                    Money::Money
                } else {
                    Money::Plain
                }
            })
            .collect();
        self.money = marks;
    }

    fn aliases(&self) -> BTreeSet<String> {
        self.tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.is_word("as"))
            .filter_map(|(index, _)| {
                let alias = next_index(index).and_then(|next| self.token(next))?;
                let operand = self.left_operand(index)?;
                (matches_name(alias) && self.operand_money(operand) == Money::Money)
                    .then(|| alias.lowered())
            })
            .collect()
    }

    fn function_start(&self, open: usize) -> usize {
        open.checked_sub(1)
            .filter(|before| {
                self.token(*before)
                    .is_some_and(|token| token.kind == Kind::Word)
            })
            .unwrap_or(open)
    }

    fn left_operand(&self, operator: usize) -> Option<(usize, usize)> {
        let end = operator.checked_sub(1)?;
        let token = self.token(end)?;
        match token.kind {
            Kind::Symbol if token.text == ")" => self
                .partner(end)
                .map(|open| (self.function_start(open), end)),
            Kind::Word | Kind::QuotedName | Kind::Number | Kind::Text | Kind::Parameter => {
                Some((end, end))
            }
            Kind::Symbol => None,
        }
    }

    fn qualified_end(&self, start: usize) -> usize {
        let mut end = start;
        while self.is_qualifier(end) {
            match next_index(end).and_then(next_index) {
                Some(next) if self.token(next).is_some_and(matches_name) => end = next,
                Some(_) | None => break,
            }
        }
        end
    }

    fn right_operand(&self, operator: usize) -> Option<(usize, usize)> {
        let start = next_index(operator)?;
        let token = self.token(start)?;
        let opens_call = next_index(start)
            .and_then(|next| self.token(next))
            .is_some_and(|next| next.is_symbol("("));
        match token.kind {
            Kind::Symbol if token.text == "(" => self.partner(start).map(|end| (start, end)),
            Kind::Word if opens_call => next_index(start)
                .and_then(|open| self.partner(open))
                .map(|end| (start, end)),
            Kind::Word | Kind::QuotedName => Some((start, self.qualified_end(start))),
            Kind::Number | Kind::Text | Kind::Parameter => Some((start, start)),
            Kind::Symbol => None,
        }
    }

    fn operand_money(&self, (start, end): (usize, usize)) -> Money {
        let call = self.token(start).is_some_and(|token| {
            token.kind == Kind::Word
                && NON_MONEY_RESULT_FUNCTIONS.contains(&token.lowered().as_str())
        }) && next_index(start)
            .and_then(|next| self.token(next))
            .is_some_and(|token| token.is_symbol("("));
        let inside = self
            .money
            .get(start..=end)
            .is_some_and(|marks| marks.contains(&Money::Money));
        if inside && !call {
            Money::Money
        } else {
            Money::Plain
        }
    }

    fn side_money(&self, side: Option<(usize, usize)>) -> Money {
        side.map_or(Money::Plain, |operand| self.operand_money(operand))
    }

    fn call_arguments(&self, index: usize) -> Option<(usize, usize)> {
        let open = next_index(index)
            .filter(|open| self.token(*open).is_some_and(|next| next.is_symbol("(")))?;
        self.partner(open).map(|close| (open, close))
    }

    fn function_rules(&self, rules: &mut BTreeSet<Rule>) {
        for (index, token) in self.tokens.iter().enumerate() {
            let name = token.lowered();
            let flagged = token.kind == Kind::Word
                && MONEY_FUNCTIONS.contains(&name.as_str())
                && self
                    .call_arguments(index)
                    .is_some_and(|range| self.operand_money(range) == Money::Money);
            if flagged {
                rules.insert(Rule::MoneyFunction(name));
            }
        }
    }

    fn operator_rules(&self, rules: &mut BTreeSet<Rule>) {
        for (index, token) in self.tokens.iter().enumerate() {
            let left = self.left_operand(index);
            let right = self.right_operand(index);
            let touches =
                self.side_money(left) == Money::Money || self.side_money(right) == Money::Money;
            if token.kind == Kind::Symbol
                && ARITHMETIC_OPERATORS.contains(&token.text.as_str())
                && touches
                && !(token.text == "*" && left.is_none())
            {
                rules.insert(Rule::MoneyArithmetic(token.text.clone()));
            }
            if token.kind == Kind::Symbol
                && COMPARISON_OPERATORS.contains(&token.text.as_str())
                && touches
            {
                rules.insert(Rule::MoneyComparison(token.text.clone()));
            }
            if token.is_word(BETWEEN) && self.between_money(index) == Money::Money {
                rules.insert(Rule::MoneyComparison(String::from(BETWEEN)));
            }
        }
    }

    fn between_money(&self, index: usize) -> Money {
        let negated = index
            .checked_sub(1)
            .filter(|before| {
                self.token(*before)
                    .is_some_and(|token| token.is_word("not"))
            })
            .unwrap_or(index);
        if self.side_money(self.left_operand(negated)) == Money::Money
            || self.side_money(self.right_operand(index)) == Money::Money
        {
            Money::Money
        } else {
            Money::Plain
        }
    }

    fn order_by_rules(&self, rules: &mut BTreeSet<Rule>) {
        for (index, token) in self.tokens.iter().enumerate() {
            let following = next_index(index).filter(|next| {
                self.token(*next)
                    .is_some_and(|candidate| candidate.is_word("by"))
            });
            if let Some(by) = following
                && token.is_word("order")
            {
                self.order_by_clause(index, by, rules);
            }
        }
    }

    fn ends_clause(&self, index: usize, depth: usize) -> bool {
        self.depth(index) < depth
            || self.token(index).is_some_and(|token| {
                token.is_symbol(";")
                    || (self.depth(index) == depth
                        && ORDER_BY_END.iter().any(|word| token.is_word(word)))
            })
    }

    fn order_by_clause(&self, order: usize, by: usize, rules: &mut BTreeSet<Rule>) {
        let depth = self.depth(order);
        let mut item = Item::Start;
        let start = next_index(by).unwrap_or(by);
        for index in start..self.tokens.len() {
            if self.ends_clause(index, depth) {
                break;
            }
            let Some(token) = self.token(index) else {
                break;
            };
            if self.depth(index) == depth && token.is_symbol(",") {
                item = Item::Start;
                continue;
            }
            if item == Item::Start && token.kind == Kind::Number {
                rules.insert(Rule::OrderByPosition);
            }
            item = Item::Inside;
            if self.money.get(index) == Some(&Money::Money) {
                rules.insert(Rule::MoneyOrderBy);
            }
        }
    }
}

fn matches_name(token: &Token) -> bool {
    token.kind == Kind::Word || token.kind == Kind::QuotedName
}

pub(crate) fn money_rules(sql: &str, names: &MoneyNames) -> BTreeSet<Rule> {
    let statement = Statement::new(sql, names);
    let mut rules = BTreeSet::new();
    statement.function_rules(&mut rules);
    statement.operator_rules(&mut rules);
    statement.order_by_rules(&mut rules);
    rules
}
