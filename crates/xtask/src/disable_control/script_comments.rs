use std::collections::BTreeSet;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) line: usize,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Previous {
    Operand,
    Operator,
}

struct Lexer<'source> {
    chars: Peekable<Chars<'source>>,
    keywords: &'source BTreeSet<String>,
    line: usize,
    previous: Previous,
    braces: Vec<usize>,
    found: Vec<Comment>,
}

fn starts_word(next: char) -> bool {
    next.is_alphanumeric() || next == '_' || next == '$'
}

fn escaped_on_line(probe: &mut Peekable<Chars<'_>>) -> Option<()> {
    (probe.next()? != '\n').then_some(())
}

fn regex_end(probe: &mut Peekable<Chars<'_>>) -> Option<()> {
    let mut class = false;
    loop {
        match probe.next()? {
            '\n' => return None,
            '\\' => escaped_on_line(probe)?,
            '[' => class = true,
            ']' => class = false,
            '/' if !class => return Some(()),
            _ => {}
        }
    }
}

impl Lexer<'_> {
    fn advance(&mut self) -> Option<char> {
        let next = self.chars.next();
        if next == Some('\n') {
            self.line = self.line.saturating_add(1);
        }
        next
    }

    fn line_comment(&mut self) {
        let mut text = String::new();
        while let Some(next) = self.chars.peek().copied() {
            if next == '\n' {
                break;
            }
            text.push(next);
            self.advance();
        }
        self.found.push(Comment {
            line: self.line,
            text,
        });
    }

    fn block_comment(&mut self) {
        let line = self.line;
        let mut text = String::new();
        while let Some(next) = self.advance() {
            if next == '*' && self.chars.peek() == Some(&'/') {
                self.advance();
                break;
            }
            text.push(next);
        }
        self.found.push(Comment { line, text });
    }

    fn quoted(&mut self, quote: char) {
        while let Some(next) = self.chars.peek().copied() {
            if next == '\n' {
                break;
            }
            self.advance();
            if next == '\\' {
                self.advance();
            } else if next == quote {
                break;
            }
        }
        self.previous = Previous::Operand;
    }

    fn template(&mut self) {
        while let Some(next) = self.advance() {
            match next {
                '\\' => {
                    self.advance();
                }
                '`' => {
                    self.previous = Previous::Operand;
                    return;
                }
                '$' if self.chars.peek() == Some(&'{') => {
                    self.advance();
                    self.braces.push(0);
                    self.previous = Previous::Operator;
                    return;
                }
                _ => {}
            }
        }
    }

    fn regex(&mut self) -> Option<()> {
        let mut probe = self.chars.clone();
        regex_end(&mut probe)?;
        while probe.peek().copied().is_some_and(starts_word) {
            probe.next();
        }
        self.chars = probe;
        self.previous = Previous::Operand;
        Some(())
    }

    fn open_brace(&mut self) {
        if let Some(depth) = self.braces.last_mut() {
            *depth = depth.saturating_add(1);
        }
        self.previous = Previous::Operator;
    }

    fn slash(&mut self) {
        match self.chars.peek() {
            Some('/') => {
                self.advance();
                self.line_comment();
            }
            Some('*') => {
                self.advance();
                self.block_comment();
            }
            Some(_) | None => {
                let divides = self.previous == Previous::Operand || self.regex().is_none();
                if divides {
                    self.previous = Previous::Operator;
                }
            }
        }
    }

    fn word(&mut self, first: char) {
        let mut word = String::from(first);
        while let Some(next) = self.chars.peek().copied() {
            if !starts_word(next) {
                break;
            }
            word.push(next);
            self.advance();
        }
        self.previous = if self.keywords.contains(&word) {
            Previous::Operator
        } else {
            Previous::Operand
        };
    }

    fn close_brace(&mut self) {
        match self.braces.last_mut() {
            Some(0) => {
                self.braces.pop();
                self.template();
            }
            Some(depth) => {
                *depth = depth.saturating_sub(1);
                self.previous = Previous::Operator;
            }
            None => self.previous = Previous::Operator,
        }
    }

    fn run(&mut self) {
        while let Some(next) = self.advance() {
            match next {
                '/' => self.slash(),
                '\'' | '"' => self.quoted(next),
                '`' => self.template(),
                '{' => self.open_brace(),
                '}' => self.close_brace(),
                ')' | ']' => self.previous = Previous::Operand,
                _ if next.is_whitespace() => {}
                _ if starts_word(next) => self.word(next),
                _ => self.previous = Previous::Operator,
            }
        }
    }
}

pub(crate) fn comments<'source>(
    source: &'source str,
    keywords: &'source BTreeSet<String>,
) -> Vec<Comment> {
    let mut lexer = Lexer {
        chars: source.chars().peekable(),
        keywords,
        line: 1,
        previous: Previous::Operator,
        braces: Vec::new(),
        found: Vec::new(),
    };
    lexer.run();
    lexer.found
}
