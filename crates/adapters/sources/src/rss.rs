use application::{InformationError, PublicationTime, ResponseMismatch};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use quick_xml::Reader;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use time::format_description::well_known::{Rfc2822, Rfc3339};
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

const MAX_FEED_ITEMS: usize = 2000;
const MAX_SUMMARY_CHARS: usize = 1000;
const KST_OFFSET_SECONDS: i32 = 32_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFeedItem {
    pub title: String,
    pub link: String,
    pub summary: Option<String>,
    pub published_at: PublicationTime,
    pub author: Option<String>,
    pub category: Option<String>,
    pub guid: Option<String>,
    pub source_name: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedFlavor {
    Unknown,
    Rss,
    Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveField {
    None,
    Title,
    Link,
    Summary,
    Published,
    Author,
    Category,
    Guid,
    SourceName,
}

#[derive(Default)]
struct ItemBuilder {
    title: String,
    link: String,
    summary: String,
    published: String,
    author: String,
    category: String,
    guid: String,
    source_name: String,
    source_url: Option<String>,
    in_author_block: bool,
    has_alternate_link: bool,
}

impl ItemBuilder {
    fn append(&mut self, field: ActiveField, text: &str) {
        match field {
            ActiveField::None => {}
            ActiveField::Title => self.title.push_str(text),
            ActiveField::Link => self.link.push_str(text),
            ActiveField::Summary => self.summary.push_str(text),
            ActiveField::Published => self.published.push_str(text),
            ActiveField::Author => self.author.push_str(text),
            ActiveField::Category => self.category.push_str(text),
            ActiveField::Guid => self.guid.push_str(text),
            ActiveField::SourceName => self.source_name.push_str(text),
        }
    }

    fn finish(self) -> ParsedFeedItem {
        let title = collapse_whitespace(&strip_html_tags(&self.title));
        let link = self.link.trim().to_owned();
        let cleaned_summary = truncate_chars(
            &collapse_whitespace(&strip_html_tags(&self.summary)),
            MAX_SUMMARY_CHARS,
        );
        let summary = if cleaned_summary.is_empty() {
            None
        } else {
            Some(cleaned_summary)
        };
        let published_at = parse_feed_timestamp(self.published.trim());
        let author = non_empty_trimmed(&self.author);
        let category = non_empty_trimmed(&self.category);
        let guid = non_empty_trimmed(&self.guid);
        let source_name = non_empty_trimmed(&self.source_name);
        let source_url = self.source_url.and_then(|value| non_empty_trimmed(&value));
        ParsedFeedItem {
            title,
            link,
            summary,
            published_at,
            author,
            category,
            guid,
            source_name,
            source_url,
        }
    }
}

struct ParseState {
    flavor: FeedFlavor,
    items: Vec<ParsedFeedItem>,
    current: Option<ItemBuilder>,
    field: ActiveField,
}

impl ParseState {
    fn new() -> Self {
        Self {
            flavor: FeedFlavor::Unknown,
            items: Vec::new(),
            current: None,
            field: ActiveField::None,
        }
    }

    fn on_start(&mut self, start: &BytesStart<'_>) -> Result<(), InformationError> {
        handle_start_tag(
            start,
            &mut self.flavor,
            &mut self.current,
            &mut self.field,
            self.items.len(),
        )
    }

    fn on_empty(&mut self, start: &BytesStart<'_>) -> Result<(), InformationError> {
        if self.flavor == FeedFlavor::Unknown {
            self.flavor = flavor_from_root(start.name().as_ref())?;
            return Ok(());
        }
        handle_empty_tag(start, self.flavor, &mut self.current)
    }

    fn on_end(&mut self, end: &BytesEnd<'_>) {
        let name = local_name_lower(end.name().as_ref());
        let is_item_end = (self.flavor == FeedFlavor::Rss && name == "item")
            || (self.flavor == FeedFlavor::Atom && name == "entry");
        if is_item_end {
            if let Some(builder) = self.current.take() {
                self.items.push(builder.finish());
            }
            self.field = ActiveField::None;
            return;
        }
        if self.flavor == FeedFlavor::Atom
            && name == "author"
            && let Some(builder) = self.current.as_mut()
        {
            builder.in_author_block = false;
        }
        self.field = ActiveField::None;
    }

    fn on_text(&mut self, text: &str) {
        if self.field != ActiveField::None
            && let Some(builder) = self.current.as_mut()
        {
            builder.append(self.field, text);
        }
    }

    fn on_entity(&mut self, raw: &str) {
        if self.field == ActiveField::None {
            return;
        }
        if let (Some(builder), Some(resolved)) = (self.current.as_mut(), resolve_entity_ref(raw)) {
            builder.append(self.field, &resolved);
        }
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = collapse_whitespace(value);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

fn local_name_lower(raw: &str) -> String {
    let slice = raw.rsplit_once(':').map_or(raw, |(_prefix, local)| local);
    slice.to_ascii_lowercase()
}

fn flavor_from_root(raw: &str) -> Result<FeedFlavor, InformationError> {
    match local_name_lower(raw).as_str() {
        "rss" | "rdf" => Ok(FeedFlavor::Rss),
        "feed" => Ok(FeedFlavor::Atom),
        _ => Err(schema_error()),
    }
}

fn read_attribute(
    start: &BytesStart<'_>,
    target: &str,
) -> Result<Option<String>, InformationError> {
    for attr_result in start.attributes() {
        let attr = attr_result.map_err(|_error| schema_error())?;
        if local_name_lower(attr.key.as_ref()) == target {
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|_error| schema_error())?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn resolve_entity_ref(raw: &str) -> Option<String> {
    if let Some(hex_or_dec) = raw.strip_prefix('#') {
        let code = if let Some(hex) = hex_or_dec
            .strip_prefix('x')
            .or_else(|| hex_or_dec.strip_prefix('X'))
        {
            u32::from_str_radix(hex, 16_u32).ok()?
        } else {
            hex_or_dec.parse::<u32>().ok()?
        };
        return char::from_u32(code).map(|ch| ch.to_string());
    }
    let resolved = match raw {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "middot" => "·",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "hellip" => "\u{2026}",
        "ndash" => "\u{2013}",
        "mdash" => "\u{2014}",
        _ => return None,
    };
    Some(resolved.to_owned())
}

fn try_decode_html_entity(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let mut entity = String::new();
    let mut lookahead = chars.clone();
    for _ in 0_usize..12_usize {
        let next = lookahead.next()?;
        if next == ';' {
            let decoded = resolve_entity_ref(&entity)?;
            *chars = lookahead;
            return Some(decoded);
        }
        if !next.is_ascii_alphanumeric() && next != '#' {
            return None;
        }
        entity.push(next);
    }
    None
}

fn is_block_html_tag(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut name = String::new();
    for ch in chars.clone() {
        if ch == '/' && name.is_empty() {
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
            if name.len() >= 12_usize {
                break;
            }
        } else {
            break;
        }
    }
    matches_block_tag(&name)
}

fn matches_block_tag(name: &str) -> bool {
    [
        "p",
        "div",
        "br",
        "li",
        "ul",
        "ol",
        "tr",
        "td",
        "th",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "hr",
        "section",
        "article",
        "header",
        "footer",
        "pre",
        "table",
    ]
    .contains(&name)
}

#[must_use]
pub fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            if is_block_html_tag(&chars) {
                out.push(' ');
            }
            in_tag = true;
            continue;
        }
        if ch == '>' && in_tag {
            in_tag = false;
            continue;
        }
        if in_tag {
            continue;
        }
        if ch == '&'
            && let Some(decoded) = try_decode_html_entity(&mut chars)
        {
            out.push_str(&decoded);
            continue;
        }
        out.push(ch);
    }
    out
}

#[must_use]
pub fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = true;
    for ch in input.chars() {
        if ch.is_whitespace() || ch.is_control() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        let _trailing = out.pop();
    }
    out
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

fn offset_datetime_to_timestamp(dt: OffsetDateTime) -> PublicationTime {
    Timestamp::from_parts(
        UnixSeconds::new(dt.unix_timestamp()),
        Nanosecond::new(dt.nanosecond()),
        OffsetSeconds::new(dt.offset().whole_seconds()),
    )
    .map_or(PublicationTime::NotProvided, PublicationTime::Known)
}

fn normalize_rfc2822(raw: &str) -> String {
    let collapsed = collapse_whitespace(raw);
    if let Some(prefix) = collapsed.strip_suffix(" KST") {
        format!("{prefix} +0900")
    } else if let Some(prefix) = collapsed.strip_suffix(" UTC") {
        format!("{prefix} +0000")
    } else if let Some(prefix) = collapsed.strip_suffix(" GMT") {
        format!("{prefix} +0000")
    } else if let Some(prefix) = collapsed.strip_suffix(" EST") {
        format!("{prefix} -0500")
    } else if let Some(prefix) = collapsed.strip_suffix(" EDT") {
        format!("{prefix} -0400")
    } else {
        collapsed
    }
}

fn parse_naive_date_time(raw: &str) -> Option<OffsetDateTime> {
    let trimmed = raw.trim();
    let (date_part, rest) = trimmed.split_once([' ', 'T'])?;
    let mut date_iter = date_part.split(['-', '.', '/']);
    let year = date_iter.next()?.parse::<i32>().ok()?;
    let month_num = date_iter.next()?.parse::<u8>().ok()?;
    let day = date_iter.next()?.parse::<u8>().ok()?;
    if date_iter.next().is_some() {
        return None;
    }
    let month = Month::try_from(month_num).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;

    let (time_part, offset_seconds) = if let Some(stripped) = rest.strip_suffix('Z') {
        (stripped, 0_i32)
    } else if let Some(stripped) = rest.strip_suffix("+09:00") {
        (stripped, KST_OFFSET_SECONDS)
    } else if let Some(stripped) = rest.strip_suffix("+0900") {
        (stripped, KST_OFFSET_SECONDS)
    } else if let Some(stripped) = rest.strip_suffix("+00:00") {
        (stripped, 0_i32)
    } else {
        (rest, KST_OFFSET_SECONDS)
    };
    let clean_time = time_part
        .split_once('.')
        .map_or(time_part, |(head, _)| head)
        .trim();
    let mut time_iter = clean_time.split(':');
    let hour = time_iter.next()?.parse::<u8>().ok()?;
    let minute = time_iter.next()?.parse::<u8>().ok()?;
    let second = match time_iter.next() {
        Some(sec) => sec.parse::<u8>().ok()?,
        None => 0_u8,
    };
    if time_iter.next().is_some() {
        return None;
    }
    let time = Time::from_hms(hour, minute, second).ok()?;
    let offset = UtcOffset::from_whole_seconds(offset_seconds).ok()?;
    Some(PrimitiveDateTime::new(date, time).assume_offset(offset))
}

#[must_use]
pub fn parse_feed_timestamp(raw: &str) -> PublicationTime {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PublicationTime::NotProvided;
    }
    if let Ok(dt) = OffsetDateTime::parse(trimmed, &Rfc3339) {
        return offset_datetime_to_timestamp(dt);
    }
    let normalized = normalize_rfc2822(trimmed);
    if let Ok(dt) = OffsetDateTime::parse(&normalized, &Rfc2822) {
        return offset_datetime_to_timestamp(dt);
    }
    if let Some(dt) = parse_naive_date_time(&normalized) {
        return offset_datetime_to_timestamp(dt);
    }
    PublicationTime::NotProvided
}

fn assign_atom_link(
    builder: &mut ItemBuilder,
    start: &BytesStart<'_>,
) -> Result<(), InformationError> {
    let rel = read_attribute(start, "rel")?;
    let Some(url) = read_attribute(start, "href")? else {
        return Ok(());
    };
    let is_alternate = rel
        .as_deref()
        .is_none_or(|value| value.eq_ignore_ascii_case("alternate"));
    if is_alternate && (!builder.has_alternate_link || builder.link.is_empty()) {
        builder.link = url;
        builder.has_alternate_link = true;
    }
    Ok(())
}

fn handle_start_tag(
    start: &BytesStart<'_>,
    flavor: &mut FeedFlavor,
    current: &mut Option<ItemBuilder>,
    field: &mut ActiveField,
    items_len: usize,
) -> Result<(), InformationError> {
    let name = local_name_lower(start.name().as_ref());
    if *flavor == FeedFlavor::Unknown {
        *flavor = flavor_from_root(&name)?;
        return Ok(());
    }
    if current.is_none() {
        let is_entry = (*flavor == FeedFlavor::Rss && name == "item")
            || (*flavor == FeedFlavor::Atom && name == "entry");
        if is_entry {
            if items_len >= MAX_FEED_ITEMS {
                return Err(schema_error());
            }
            *current = Some(ItemBuilder::default());
        }
        return Ok(());
    }
    let Some(builder) = current.as_mut() else {
        return Ok(());
    };
    match (*flavor, name.as_str()) {
        (_, "title") => *field = ActiveField::Title,
        (FeedFlavor::Rss, "link") => *field = ActiveField::Link,
        (FeedFlavor::Atom, "link") => assign_atom_link(builder, start)?,
        (FeedFlavor::Rss, "description" | "encoded")
        | (FeedFlavor::Atom, "summary" | "content") => {
            if builder.summary.is_empty() {
                *field = ActiveField::Summary;
            }
        }
        (FeedFlavor::Rss, "pubdate" | "date") | (FeedFlavor::Atom, "updated" | "published") => {
            if builder.published.is_empty() {
                *field = ActiveField::Published;
            }
        }
        (FeedFlavor::Rss, "author" | "creator") => *field = ActiveField::Author,
        (FeedFlavor::Atom, "author") => builder.in_author_block = true,
        (FeedFlavor::Atom, "name") if builder.in_author_block => *field = ActiveField::Author,
        (FeedFlavor::Rss, "category") => *field = ActiveField::Category,
        (FeedFlavor::Atom, "category") => {
            if builder.category.is_empty()
                && let Some(term) = read_attribute(start, "term")?
            {
                builder.category = term;
            }
        }
        (FeedFlavor::Rss, "guid") | (FeedFlavor::Atom, "id") => *field = ActiveField::Guid,
        (FeedFlavor::Rss, "source") => {
            builder.source_url = read_attribute(start, "url")?;
            *field = ActiveField::SourceName;
        }
        _ => {}
    }
    Ok(())
}

fn handle_empty_tag(
    start: &BytesStart<'_>,
    flavor: FeedFlavor,
    current: &mut Option<ItemBuilder>,
) -> Result<(), InformationError> {
    let Some(builder) = current.as_mut() else {
        return Ok(());
    };
    let name = local_name_lower(start.name().as_ref());
    if flavor == FeedFlavor::Atom && name == "link" {
        assign_atom_link(builder, start)?;
    } else if flavor == FeedFlavor::Atom && name == "category" && builder.category.is_empty() {
        if let Some(term) = read_attribute(start, "term")? {
            builder.category = term;
        }
    } else if flavor == FeedFlavor::Rss && name == "source" {
        builder.source_url = read_attribute(start, "url")?;
    }
    Ok(())
}

pub fn parse_feed(bytes: &[u8]) -> Result<Vec<ParsedFeedItem>, InformationError> {
    if bytes.is_empty() {
        return Err(schema_error());
    }
    let raw_text = std::str::from_utf8(bytes).map_err(|_error| schema_error())?;
    let trimmed = raw_text.trim_start_matches('\u{feff}').trim_start();
    if trimmed.is_empty() {
        return Err(schema_error());
    }
    let lower_head = trimmed
        .chars()
        .take(32_usize)
        .collect::<String>()
        .to_ascii_lowercase();
    if lower_head.starts_with("<!doctype html") || lower_head.starts_with("<html") {
        return Err(schema_error());
    }
    let mut reader = Reader::from_str(trimmed);
    reader.config_mut().check_end_names = true;
    let mut state = ParseState::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => state.on_start(&start)?,
            Ok(Event::Empty(start)) => state.on_empty(&start)?,
            Ok(Event::End(end)) => state.on_end(&end),
            Ok(Event::Text(text)) => state.on_text(text.as_ref()),
            Ok(Event::CData(cdata)) => state.on_text(cdata.as_ref()),
            Ok(Event::GeneralRef(entity)) => state.on_entity(entity.as_ref()),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_error) => return Err(schema_error()),
        }
    }
    if state.flavor == FeedFlavor::Unknown || state.current.is_some() {
        return Err(schema_error());
    }
    Ok(state.items)
}
