use crate::error::{FieldName, SchemaFailure, StreamDecodeError, ValidationFailure};
use application::{ModelName, ResetTime, TokenCount, TokenUsage, Utilization};
use domain::{
    Amount, AmountInput, Nanosecond, OffsetSeconds, Probability, Timestamp, UnixSeconds,
    UntrustedText,
};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedToolName {
    WebSearch,
    Mcp { server: String, tool: String },
    Builtin { tool: String },
}

pub(crate) fn untrusted(text: &str) -> Result<UntrustedText, StreamDecodeError> {
    UntrustedText::try_new(text)
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::FieldTooLong))
}

pub(crate) fn serialize_untrusted(value: &Value) -> Result<UntrustedText, StreamDecodeError> {
    let encoded = serde_json::to_string(value)
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::FieldTooLong))?;
    untrusted(&encoded)
}

pub(crate) fn required_str<'value>(
    object: &'value Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<&'value str, StreamDecodeError> {
    let value = object
        .get(key)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            field,
        )))?;
    value
        .as_str()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            field,
        )))
}

pub(crate) fn required_object<'value>(
    object: &'value Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<&'value Map<String, Value>, StreamDecodeError> {
    let value = object
        .get(key)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            field,
        )))?;
    value
        .as_object()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            field,
        )))
}

pub(crate) fn parse_model_name(raw: &str) -> Result<ModelName, StreamDecodeError> {
    ModelName::try_new(raw)
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidModelName))
}

pub(crate) fn parse_reset_time(
    value: Option<&Value>,
    field: FieldName,
) -> Result<ResetTime, StreamDecodeError> {
    match value {
        None | Some(Value::Null) => Ok(ResetTime::NotProvided),
        Some(Value::Number(number)) => {
            let seconds = number.as_i64().ok_or(StreamDecodeError::Validation(
                ValidationFailure::InvalidTimestamp,
            ))?;
            let timestamp = Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(0_u32),
                OffsetSeconds::new(0_i32),
            )
            .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidTimestamp))?;
            Ok(ResetTime::Known(timestamp))
        }
        Some(Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(field)),
        ),
    }
}

pub(crate) fn parse_utilization(
    value: &Value,
    field: FieldName,
) -> Result<Utilization, StreamDecodeError> {
    let number = match value {
        Value::Number(raw) => raw,
        Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                field,
            )));
        }
    };
    let text = number.to_string();
    let input = AmountInput::new(&text);
    let amount = Amount::parse(&input)
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidUtilization))?;
    let probability = Probability::try_new(amount.as_decimal())
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidUtilization))?;
    Ok(Utilization::new(probability))
}

fn token_field_required(
    usage: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<TokenCount, StreamDecodeError> {
    let value = usage
        .get(key)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            field,
        )))?;
    parse_token_count(value, field)
}

fn token_field_optional(
    usage: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<TokenCount, StreamDecodeError> {
    match usage.get(key) {
        None | Some(Value::Null) => Ok(TokenCount::new(0_u64)),
        Some(value) => parse_token_count(value, field),
    }
}

fn parse_token_count(value: &Value, field: FieldName) -> Result<TokenCount, StreamDecodeError> {
    match value {
        Value::Number(number) => {
            number
                .as_u64()
                .map(TokenCount::new)
                .ok_or(StreamDecodeError::Validation(
                    ValidationFailure::InvalidTokenCount,
                ))
        }
        Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
            Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                field,
            )))
        }
    }
}

pub(crate) fn parse_token_usage(
    usage: &Map<String, Value>,
) -> Result<TokenUsage, StreamDecodeError> {
    let input = token_field_required(usage, "input_tokens", FieldName::InputTokens)?;
    let output = token_field_required(usage, "output_tokens", FieldName::OutputTokens)?;
    let cache_read = token_field_optional(
        usage,
        "cache_read_input_tokens",
        FieldName::CacheReadInputTokens,
    )?;
    let cache_creation = token_field_optional(
        usage,
        "cache_creation_input_tokens",
        FieldName::CacheCreationInputTokens,
    )?;
    Ok(TokenUsage::new(input, output, cache_read, cache_creation))
}

pub(crate) fn parse_tool_name(name: &str) -> Result<ParsedToolName, StreamDecodeError> {
    if name.trim().is_empty() {
        return Err(StreamDecodeError::Validation(
            ValidationFailure::InvalidToolName,
        ));
    }
    if name == "WebSearch" {
        return Ok(ParsedToolName::WebSearch);
    }
    if let Some(rest) = name.strip_prefix("mcp__") {
        let Some((server, tool)) = rest.split_once("__") else {
            return Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidToolName,
            ));
        };
        if server.is_empty() || tool.is_empty() {
            return Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidToolName,
            ));
        }
        return Ok(ParsedToolName::Mcp {
            server: String::from(server),
            tool: String::from(tool),
        });
    }
    Ok(ParsedToolName::Builtin {
        tool: String::from(name),
    })
}

pub(crate) fn parse_web_search_fields(
    input: &Value,
) -> Result<(UntrustedText, UntrustedText), StreamDecodeError> {
    let object =
        input
            .as_object()
            .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Input,
            )))?;
    let query_text = required_str(object, "query", FieldName::Query)?;
    let action_text = match object.get("action") {
        None | Some(Value::Null) => "search",
        Some(Value::String(action)) => action.as_str(),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Action,
            )));
        }
    };
    Ok((untrusted(query_text)?, untrusted(action_text)?))
}

pub(crate) fn render_tool_result_content(
    value: Option<&Value>,
) -> Result<UntrustedText, StreamDecodeError> {
    match value {
        None | Some(Value::Null) => untrusted(""),
        Some(Value::String(text)) => untrusted(text),
        Some(Value::Array(items)) => {
            if let [Value::Object(only)] = items.as_slice()
                && only.get("type").and_then(Value::as_str) == Some("text")
                && let Some(text) = only.get("text").and_then(Value::as_str)
            {
                return untrusted(text);
            }
            serialize_untrusted(&Value::Array(items.clone()))
        }
        Some(other) => serialize_untrusted(other),
    }
}
