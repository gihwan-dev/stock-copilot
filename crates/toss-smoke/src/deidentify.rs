use std::collections::BTreeMap;

use serde_json::{Map, Value};

pub const DEIDENTIFIED_ORIGIN: &str = "deidentified";
pub const SYNTHETIC_ORIGIN: &str = "synthetic";
const ACCOUNT_PREFIX: &str = "synthetic-account-";
const SYNTHETIC_ZERO_PREFIX: &str = "000000";
const KEPT_KEYS: [&str; 12] = [
    "symbol",
    "code",
    "isinCode",
    "orderId",
    "requestId",
    "nextCursor",
    "currency",
    "baseCurrency",
    "quoteCurrency",
    "marketCountry",
    "exchange",
    "status",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeidentifyError {
    NotRecording,
    UnsupportedFormat,
    LeakRemains,
}

impl DeidentifyError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NotRecording => "toss-smoke 녹화 파일 형식이 아닙니다.",
            Self::UnsupportedFormat => "지원하지 않는 녹화 형식 버전입니다.",
            Self::LeakRemains => {
                "비식별화 뒤에도 계좌번호 형태 문자열이 남아 픽스처를 만들지 않았습니다."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Default)]
struct Pseudonyms {
    accounts: BTreeMap<String, String>,
    sequences: BTreeMap<String, u64>,
    numbers: u64,
}

impl Pseudonyms {
    fn account(&mut self, original: &str) -> String {
        let next = self.accounts.len().saturating_add(101);
        self.accounts
            .entry(original.to_owned())
            .or_insert_with(|| format!("{ACCOUNT_PREFIX}{next}"))
            .clone()
    }
    fn sequence(&mut self, original: &str) -> u64 {
        let next = match u64::try_from(self.sequences.len()) {
            Ok(count) => count.saturating_add(1),
            Err(_error) => u64::MAX,
        };
        *self.sequences.entry(original.to_owned()).or_insert(next)
    }
    fn number(&mut self) -> u64 {
        self.numbers = self.numbers.saturating_add(1);
        self.numbers
            .saturating_mul(37)
            .checked_rem(900)
            .unwrap_or(0)
            .saturating_add(100)
    }
}

fn is_account_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower == "account" || lower.contains("accountno") || lower.contains("accountnumber")
}

fn is_sequence_key(key: &str) -> bool {
    key.to_ascii_lowercase().ends_with("seq")
}

fn numeric_shape(text: &str) -> Option<(bool, Option<usize>)> {
    let (negative, digits) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits, None),
    };
    let all_digits =
        |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !all_digits(whole) || fraction.is_some_and(|part| !all_digits(part)) {
        return None;
    }
    Some((negative, fraction.map(str::len)))
}

fn is_zero(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte == b'0' || byte == b'.' || byte == b'-')
}

fn synthetic_number(text: &str, pseudonyms: &mut Pseudonyms) -> Option<String> {
    let (negative, scale) = numeric_shape(text)?;
    if is_zero(text) {
        return Some(text.to_owned());
    }
    let whole = pseudonyms.number();
    let sign = if negative { "-" } else { "" };
    Some(match scale {
        Some(width) => format!("{sign}{whole}.{}", "7".repeat(width)),
        None => format!("{sign}{whole}"),
    })
}

fn scrub(key: Option<&str>, value: Value, pseudonyms: &mut Pseudonyms) -> Value {
    match value {
        Value::Null | Value::Bool(_) => value,
        Value::Number(number) => match key {
            Some(name) if is_sequence_key(name) || is_account_key(name) => {
                Value::from(pseudonyms.sequence(&number.to_string()))
            }
            Some(name) if KEPT_KEYS.contains(&name) => Value::Number(number),
            Some(_) | None => synthetic_number(&number.to_string(), pseudonyms)
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or(Value::Number(number)),
        },
        Value::String(text) => match key {
            Some(name) if is_account_key(name) => Value::String(pseudonyms.account(&text)),
            Some(name) if KEPT_KEYS.contains(&name) => Value::String(text),
            Some(_) | None => Value::String(synthetic_number(&text, pseudonyms).unwrap_or(text)),
        },
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub(key, item, pseudonyms))
                .collect(),
        ),
        Value::Object(entries) => {
            let mut scrubbed = Map::new();
            for (name, entry) in entries {
                let safe = scrub(Some(&name), entry, pseudonyms);
                scrubbed.insert(name, safe);
            }
            Value::Object(scrubbed)
        }
    }
}

fn pointer_segment(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}

fn file_stem(step: &str, status: &str) -> String {
    let safe: String = step
        .chars()
        .map(|item| {
            if item.is_ascii_alphanumeric() {
                item.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("deidentified-{safe}-{status}.json")
}

pub fn deidentify_recording(recording: &Value) -> Result<Vec<Fixture>, DeidentifyError> {
    let root = recording.as_object().ok_or(DeidentifyError::NotRecording)?;
    if root.get("formatVersion").and_then(Value::as_u64) != Some(1) {
        return Err(DeidentifyError::UnsupportedFormat);
    }
    let steps = root
        .get("steps")
        .and_then(Value::as_array)
        .ok_or(DeidentifyError::NotRecording)?;
    let mut pseudonyms = Pseudonyms::default();
    let mut fixtures = Vec::new();
    for step in steps {
        let (Some(name), Some(path), Some(status), Some(body)) = (
            step.get("step").and_then(Value::as_str),
            step.get("path").and_then(Value::as_str),
            step.get("status").and_then(Value::as_u64),
            step.get("body").filter(|body| !body.is_null()),
        ) else {
            continue;
        };
        let status_text = status.to_string();
        let mut fixture = Map::new();
        fixture.insert("origin".to_owned(), Value::from(DEIDENTIFIED_ORIGIN));
        fixture.insert("path".to_owned(), Value::from(path));
        fixture.insert("method".to_owned(), Value::from("get"));
        fixture.insert("status".to_owned(), Value::from(status_text.clone()));
        fixture.insert(
            "schema".to_owned(),
            Value::from(format!(
                "#/paths/{}/get/responses/{status_text}/content/application~1json/schema",
                pointer_segment(path)
            )),
        );
        fixture.insert(
            "body".to_owned(),
            scrub(None, body.clone(), &mut pseudonyms),
        );
        let value = Value::Object(fixture);
        if !account_like_strings(&value).is_empty() {
            return Err(DeidentifyError::LeakRemains);
        }
        fixtures.push(Fixture {
            name: file_stem(name, &status_text),
            value,
        });
    }
    Ok(fixtures)
}

fn is_date_shape(groups: &[&str]) -> bool {
    match groups {
        [year, month, day] => year.len() == 4 && month.len() == 2 && day.len() == 2,
        _ => false,
    }
}

fn token_is_account_like(token: &str) -> bool {
    let groups: Vec<&str> = token.split('-').filter(|part| !part.is_empty()).collect();
    let digits: usize = groups.iter().map(|part| part.len()).sum();
    if groups.len() >= 3 {
        return !is_date_shape(&groups) && digits >= 9;
    }
    false
}

fn text_has_account_like(text: &str) -> bool {
    text.split(|item: char| !(item.is_ascii_digit() || item == '-'))
        .any(token_is_account_like)
}

#[must_use]
pub fn account_like_strings(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    collect_account_like(value, &mut found);
    found
}

fn collect_account_like(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
        Value::String(text) => {
            if text_has_account_like(text) {
                found.push(text.clone());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_account_like(item, found);
            }
        }
        Value::Object(entries) => {
            for (key, entry) in entries {
                if is_account_key(key)
                    && let Some(text) = entry.as_str()
                    && !text.starts_with(ACCOUNT_PREFIX)
                    && !text.starts_with(SYNTHETIC_ZERO_PREFIX)
                {
                    found.push(text.to_owned());
                }
                if text_has_account_like(key) {
                    found.push(key.clone());
                }
                collect_account_like(entry, found);
            }
        }
    }
}

#[must_use]
pub fn has_allowed_origin(fixture: &Value) -> bool {
    match fixture.get("origin").and_then(Value::as_str) {
        Some(origin) => origin == DEIDENTIFIED_ORIGIN || origin == SYNTHETIC_ORIGIN,
        None => false,
    }
}

pub const DEIDENTIFY_USAGE: &str =
    "사용법: toss-smoke deidentify <저장소_밖_녹화_파일> <픽스처_폴더>";

pub fn deidentify_files(
    input: &std::path::Path,
    output: &std::path::Path,
) -> Result<usize, &'static str> {
    use std::io::Write;
    let text = std::fs::read_to_string(input).map_err(|_error| "녹화 파일을 읽지 못했습니다.")?;
    let recording: Value =
        serde_json::from_str(&text).map_err(|_error| "녹화 파일이 JSON이 아닙니다.")?;
    let fixtures = deidentify_recording(&recording).map_err(DeidentifyError::message)?;
    if !output.is_dir() {
        return Err("픽스처 폴더가 없습니다.");
    }
    for fixture in &fixtures {
        let mut bytes = serde_json::to_vec_pretty(&fixture.value)
            .map_err(|_error| "픽스처를 직렬화하지 못했습니다.")?;
        bytes.push(b'\n');
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(&fixture.name))
            .map_err(|_error| {
                "픽스처 파일을 새로 만들지 못했습니다(같은 이름이 있으면 덮어쓰지 않습니다)."
            })?;
        file.write_all(&bytes)
            .map_err(|_error| "픽스처 파일을 쓰지 못했습니다.")?;
    }
    Ok(fixtures.len())
}
