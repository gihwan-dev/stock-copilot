use std::collections::BTreeMap;
use std::iter::Peekable;
use std::str::Chars;

const MAXIMUM_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Json {
    Null,
    True,
    False,
    Number(String),
    Text(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl Json {
    pub(crate) fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(members) => members.get(key),
            Self::Null
            | Self::True
            | Self::False
            | Self::Number(_)
            | Self::Text(_)
            | Self::Array(_) => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            Self::Null
            | Self::True
            | Self::False
            | Self::Number(_)
            | Self::Text(_)
            | Self::Object(_) => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Null
            | Self::True
            | Self::False
            | Self::Number(_)
            | Self::Array(_)
            | Self::Object(_) => None,
        }
    }
}

type Source<'a> = Peekable<Chars<'a>>;

fn skip_space(rest: &mut Source<'_>) {
    while rest
        .next_if(|next| [' ', '\t', '\n', '\r'].contains(next))
        .is_some()
    {}
}

fn keyword(rest: &mut Source<'_>, word: &str, value: Json) -> Option<Json> {
    for expected in word.chars() {
        rest.next_if_eq(&expected)?;
    }
    Some(value)
}

fn number(rest: &mut Source<'_>) -> Option<Json> {
    let mut text = String::new();
    while let Some(next) =
        rest.next_if(|next| next.is_ascii_digit() || ['-', '+', '.', 'e', 'E'].contains(next))
    {
        text.push(next);
    }
    let digits = text.trim_start_matches('-');
    digits
        .starts_with(|first: char| first.is_ascii_digit())
        .then_some(Json::Number(text))
}

fn hex_unit(rest: &mut Source<'_>) -> Option<u32> {
    let mut unit: u32 = 0;
    for _ in 0_u8..4 {
        let digit = rest.next()?.to_digit(16)?;
        unit = unit.checked_mul(16)?.checked_add(digit)?;
    }
    Some(unit)
}

fn unicode_escape(rest: &mut Source<'_>) -> Option<char> {
    let high = hex_unit(rest)?;
    if !(0xD800..0xDC00).contains(&high) {
        return char::from_u32(high);
    }
    rest.next_if_eq(&'\\')?;
    rest.next_if_eq(&'u')?;
    let low = hex_unit(rest)?;
    if !(0xDC00..0xE000).contains(&low) {
        return None;
    }
    let offset = high
        .checked_sub(0xD800)?
        .checked_shl(10)?
        .checked_add(low.checked_sub(0xDC00)?)?;
    char::from_u32(offset.checked_add(0x1_0000)?)
}

fn escape(rest: &mut Source<'_>) -> Option<char> {
    match rest.next()? {
        '"' => Some('"'),
        '\\' => Some('\\'),
        '/' => Some('/'),
        'b' => Some('\u{8}'),
        'f' => Some('\u{c}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'u' => unicode_escape(rest),
        _ => None,
    }
}

fn string(rest: &mut Source<'_>) -> Option<String> {
    rest.next_if_eq(&'"')?;
    let mut text = String::new();
    loop {
        match rest.next()? {
            '"' => return Some(text),
            '\\' => text.push(escape(rest)?),
            control if control < ' ' => return None,
            other => text.push(other),
        }
    }
}

fn separated<T>(
    rest: &mut Source<'_>,
    close: char,
    mut item: impl FnMut(&mut Source<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let mut items = Vec::new();
    skip_space(rest);
    if rest.next_if_eq(&close).is_some() {
        return Some(items);
    }
    loop {
        items.push(item(rest)?);
        skip_space(rest);
        match rest.next()? {
            ',' => {}
            next if next == close => return Some(items),
            _ => return None,
        }
    }
}

fn member(rest: &mut Source<'_>, depth: usize) -> Option<(String, Json)> {
    skip_space(rest);
    let key = string(rest)?;
    skip_space(rest);
    rest.next_if_eq(&':')?;
    Some((key, value(rest, depth)?))
}

fn object(rest: &mut Source<'_>, depth: usize) -> Option<Json> {
    rest.next();
    let members = separated(rest, '}', |inner| member(inner, depth))?;
    let mut object = BTreeMap::new();
    for (key, entry) in members {
        if object.insert(key, entry).is_some() {
            return None;
        }
    }
    Some(Json::Object(object))
}

fn value(rest: &mut Source<'_>, depth: usize) -> Option<Json> {
    let inner = depth.checked_add(1).filter(|next| *next <= MAXIMUM_DEPTH)?;
    skip_space(rest);
    match *rest.peek()? {
        '{' => object(rest, inner),
        '[' => {
            rest.next();
            separated(rest, ']', |item| value(item, inner)).map(Json::Array)
        }
        '"' => string(rest).map(Json::Text),
        't' => keyword(rest, "true", Json::True),
        'f' => keyword(rest, "false", Json::False),
        'n' => keyword(rest, "null", Json::Null),
        _ => number(rest),
    }
}

pub(crate) fn parse(text: &str) -> Option<Json> {
    let mut rest = text.chars().peekable();
    let parsed = value(&mut rest, 0)?;
    skip_space(&mut rest);
    rest.peek().is_none().then_some(parsed)
}
