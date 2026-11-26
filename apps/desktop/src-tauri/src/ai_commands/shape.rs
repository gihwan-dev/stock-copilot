use application::Clock;
use mcp_server::{
    NormalizedUntrustedText, PageRequest, PaginationContext, ToolErrorPayload, UntrustedTextLimit,
    paginate_items,
};
use serde_json::{Map, Value};

const RESULT_FIELD: &str = "result";
const ITEMS_FIELD: &str = "items_field";
const SUMMARY_DEPTH: usize = 3_usize;
const PLAIN_VALUE_CHARS: usize = 32_usize;
const REDACTED_KEYS: [&str; 6] = [
    "secret",
    "key",
    "token",
    "password",
    "contact",
    "credential",
];

#[must_use]
pub(super) fn normalized(value: Value) -> Value {
    match value {
        Value::String(text) => {
            NormalizedUntrustedText::from_raw(&text, UntrustedTextLimit::default()).into_value()
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalized).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(field, inner)| (field, normalized(inner)))
                .collect(),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) => value,
    }
}

#[must_use]
pub(super) fn object(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            let mut wrapped = Map::new();
            let _previous = wrapped.insert(String::from(RESULT_FIELD), value);
            Value::Object(wrapped)
        }
    }
}

fn largest_list(fields: &Map<String, Value>) -> Option<String> {
    fields
        .iter()
        .filter_map(|(field, inner)| inner.as_array().map(|items| (items.len(), field)))
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(left.1)))
        .map(|(_count, field)| field.clone())
}

pub(super) fn paged<C: Clock>(
    value: Value,
    request: &PageRequest,
    context: &PaginationContext<'_, C>,
) -> Result<Value, ToolErrorPayload> {
    let Value::Object(mut fields) = object(value) else {
        return Ok(Value::Object(Map::new()));
    };
    let Some(list_field) = largest_list(&fields) else {
        return Ok(Value::Object(fields));
    };
    let items = match fields.remove(&list_field) {
        Some(Value::Array(items)) => items,
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_),
        )
        | None => Vec::new(),
    };
    let _previous = fields.insert(String::from(ITEMS_FIELD), Value::String(list_field));
    paginate_items(&items, request, context, &fields)
}

fn redacted_key(field: &str) -> bool {
    let lower = field.to_ascii_lowercase();
    REDACTED_KEYS
        .iter()
        .any(|fragment| lower.contains(fragment))
}

fn plain(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= PLAIN_VALUE_CHARS
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.:".contains(character))
}

fn describe(value: &Value, depth: usize) -> String {
    match value {
        Value::Null => String::from("null"),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) if plain(text) => text.clone(),
        Value::String(_) => String::from("<text>"),
        Value::Array(items) => format!("[{} items]", items.len()),
        Value::Object(fields) if depth >= SUMMARY_DEPTH => {
            format!("{{{} fields}}", fields.len())
        }
        Value::Object(fields) => {
            format!("{{{}}}", describe_fields(fields, depth.saturating_add(1)))
        }
    }
}

fn describe_fields(fields: &Map<String, Value>, depth: usize) -> String {
    fields
        .iter()
        .map(|(field, inner)| {
            if redacted_key(field) {
                format!("{field}=<redacted>")
            } else {
                format!("{field}={}", describe(inner, depth))
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[must_use]
pub(super) fn summarize(arguments: &Map<String, Value>) -> String {
    describe_fields(arguments, 0_usize)
}
