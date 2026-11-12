use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Word,
    QuotedName,
    Number,
    Text,
    Parameter,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: Kind,
    pub(crate) text: String,
}

impl Token {
    pub(crate) fn is_symbol(&self, symbol: &str) -> bool {
        self.kind == Kind::Symbol && self.text == symbol
    }

    pub(crate) fn is_word(&self, word: &str) -> bool {
        self.kind == Kind::Word && self.text.eq_ignore_ascii_case(word)
    }

    pub(crate) fn lowered(&self) -> String {
        self.text.to_lowercase()
    }
}

type Source<'a> = Peekable<Chars<'a>>;

const TWO_CHARACTER_SYMBOLS: [&str; 9] = ["<=", ">=", "<>", "!=", "==", "||", "<<", ">>", "->"];

fn second(rest: &Source<'_>) -> Option<char> {
    rest.clone().nth(1)
}

fn skip_line(rest: &mut Source<'_>) {
    for next in rest.by_ref() {
        if next == '\n' {
            break;
        }
    }
}

fn skip_block(rest: &mut Source<'_>) {
    rest.next();
    rest.next();
    let mut previous = ' ';
    for next in rest.by_ref() {
        if previous == '*' && next == '/' {
            break;
        }
        previous = next;
    }
}

fn quoted(rest: &mut Source<'_>, close: char, kind: Kind) -> Token {
    rest.next();
    let mut text = String::new();
    while let Some(next) = rest.next() {
        if next != close {
            text.push(next);
        } else if close != ']' && rest.peek() == Some(&close) {
            rest.next();
            text.push(close);
        } else {
            break;
        }
    }
    Token { kind, text }
}

fn consume_while(rest: &mut Source<'_>, keep: impl Fn(char) -> bool) -> String {
    let mut text = String::new();
    while let Some(next) = rest.next_if(|candidate| keep(*candidate)) {
        text.push(next);
    }
    text
}

fn parameter(rest: &mut Source<'_>) -> Token {
    let mut text = String::new();
    if let Some(first) = rest.next() {
        text.push(first);
    }
    text.push_str(&consume_while(rest, |next| {
        next.is_alphanumeric() || next == '_'
    }));
    Token {
        kind: Kind::Parameter,
        text,
    }
}

fn number(rest: &mut Source<'_>) -> Token {
    let mut text = String::new();
    let mut previous = ' ';
    while let Some(next) = rest.next_if(|candidate| {
        candidate.is_ascii_alphanumeric()
            || *candidate == '.'
            || *candidate == '_'
            || ((*candidate == '+' || *candidate == '-') && matches_exponent(previous))
    }) {
        text.push(next);
        previous = next;
    }
    Token {
        kind: Kind::Number,
        text,
    }
}

fn matches_exponent(previous: char) -> bool {
    previous == 'e' || previous == 'E'
}

fn word(rest: &mut Source<'_>) -> Token {
    Token {
        kind: Kind::Word,
        text: consume_while(rest, |next| {
            next.is_alphanumeric() || next == '_' || next == '$'
        }),
    }
}

fn symbol(first: char, rest: &mut Source<'_>) -> Token {
    rest.next();
    let mut text = String::from(first);
    if let Some(next) = rest.peek().copied() {
        let pair = format!("{first}{next}");
        if TWO_CHARACTER_SYMBOLS.contains(&pair.as_str()) {
            rest.next();
            text = pair;
        }
    }
    if text == "->" && rest.next_if_eq(&'>').is_some() {
        text.push('>');
    }
    Token {
        kind: Kind::Symbol,
        text,
    }
}

fn starts_number(first: char, rest: &Source<'_>) -> bool {
    first.is_ascii_digit()
        || (first == '.' && second(rest).is_some_and(|next| next.is_ascii_digit()))
}

fn next_token(first: char, rest: &mut Source<'_>) -> Option<Token> {
    match first {
        '-' if second(rest) == Some('-') => {
            skip_line(rest);
            None
        }
        '/' if second(rest) == Some('*') => {
            skip_block(rest);
            None
        }
        '\'' => Some(quoted(rest, '\'', Kind::Text)),
        '"' | '`' => Some(quoted(rest, first, Kind::QuotedName)),
        '[' => Some(quoted(rest, ']', Kind::QuotedName)),
        '?' | ':' | '@' | '$' => Some(parameter(rest)),
        _ if starts_number(first, rest) => Some(number(rest)),
        _ if first.is_alphabetic() || first == '_' => Some(word(rest)),
        _ => Some(symbol(first, rest)),
    }
}

pub(crate) fn tokenize(sql: &str) -> Vec<Token> {
    let mut rest = sql.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(first) = rest.peek().copied() {
        if first.is_whitespace() {
            rest.next();
        } else if let Some(token) = next_token(first, &mut rest) {
            tokens.push(token);
        }
    }
    tokens
}
