use crate::error::{FieldName, SchemaFailure, StreamDecodeError, ValidationFailure};
use application::{ModelName, ResetTime, TokenCount, TokenUsage, ToolStatus, Utilization};
use domain::{
    Amount, AmountInput, Nanosecond, OffsetSeconds, Probability, Timestamp, UnixSeconds,
    UntrustedText,
};
use serde_json::{Map, Value};

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

fn shift_percent_decimal(raw: &str) -> Result<String, StreamDecodeError> {
    if raw.starts_with('-') {
        return Err(StreamDecodeError::Validation(
            ValidationFailure::InvalidUtilization,
        ));
    }
    let (integer, fraction) = raw
        .split_once('.')
        .map_or((raw, ""), |(whole, frac)| (whole, frac));
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(StreamDecodeError::Validation(
            ValidationFailure::InvalidUtilization,
        ));
    }
    let shifted = match integer.len() {
        1_usize => format!("0.0{integer}{fraction}"),
        2_usize => format!("0.{integer}{fraction}"),
        len => {
            let split_at = len.saturating_sub(2_usize);
            let Some((head, tail)) = integer.split_at_checked(split_at) else {
                return Err(StreamDecodeError::Validation(
                    ValidationFailure::InvalidUtilization,
                ));
            };
            format!("{head}.{tail}{fraction}")
        }
    };
    Ok(shifted)
}

pub(crate) fn parse_used_percent(
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
    let raw_text = number.to_string();
    let ratio_text = shift_percent_decimal(&raw_text)?;
    let input = AmountInput::new(&ratio_text);
    let amount = Amount::parse(&input)
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidUtilization))?;
    let probability = Probability::try_new(amount.as_decimal())
        .map_err(|_error| StreamDecodeError::Validation(ValidationFailure::InvalidUtilization))?;
    Ok(Utilization::new(probability))
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

fn required_token(
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

fn optional_token(
    usage: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<TokenCount, StreamDecodeError> {
    match usage.get(key) {
        None | Some(Value::Null) => Ok(TokenCount::new(0_u64)),
        Some(value) => parse_token_count(value, field),
    }
}

pub(crate) fn parse_token_usage_camel(
    usage: &Map<String, Value>,
) -> Result<TokenUsage, StreamDecodeError> {
    let input = required_token(usage, "inputTokens", FieldName::InputTokens)?;
    let output = required_token(usage, "outputTokens", FieldName::OutputTokens)?;
    let cache_read = optional_token(usage, "cachedInputTokens", FieldName::CachedInputTokens)?;
    let cache_creation = optional_token(
        usage,
        "cacheWriteInputTokens",
        FieldName::CacheWriteInputTokens,
    )?;
    Ok(TokenUsage::new(input, output, cache_read, cache_creation))
}

pub(crate) fn parse_token_usage_snake(
    usage: &Map<String, Value>,
) -> Result<TokenUsage, StreamDecodeError> {
    let input = required_token(usage, "input_tokens", FieldName::InputTokens)?;
    let output = required_token(usage, "output_tokens", FieldName::OutputTokens)?;
    let cache_read = optional_token(usage, "cached_input_tokens", FieldName::CachedInputTokens)?;
    let cache_creation = optional_token(
        usage,
        "cache_write_input_tokens",
        FieldName::CacheWriteInputTokens,
    )?;
    Ok(TokenUsage::new(input, output, cache_read, cache_creation))
}

pub(crate) fn parse_tool_status(raw: &str) -> Result<ToolStatus, StreamDecodeError> {
    match raw {
        "in_progress" | "inProgress" | "running" => Ok(ToolStatus::Running),
        "completed" | "succeeded" => Ok(ToolStatus::Succeeded),
        "failed" | "error" => Ok(ToolStatus::Failed),
        _ => Err(StreamDecodeError::Validation(
            ValidationFailure::InvalidToolStatus,
        )),
    }
}

pub(crate) fn render_mcp_tool_result(
    item: &Map<String, Value>,
) -> Result<UntrustedText, StreamDecodeError> {
    if let Some(error_value) = item.get("error")
        && !error_value.is_null()
    {
        return match error_value {
            Value::String(text) => untrusted(text),
            other @ (Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Array(_)
            | Value::Object(_)) => serialize_untrusted(other),
        };
    }
    let Some(result_value) = item.get("result") else {
        return untrusted("");
    };
    if result_value.is_null() {
        return untrusted("");
    }
    if let Some(result_obj) = result_value.as_object()
        && let Some(Value::Array(blocks)) = result_obj.get("content")
        && let [Value::Object(only)] = blocks.as_slice()
        && only.get("type").and_then(Value::as_str) == Some("text")
        && let Some(text) = only.get("text").and_then(Value::as_str)
    {
        return untrusted(text);
    }
    serialize_untrusted(result_value)
}

pub(crate) fn parse_web_search_action(
    value: Option<&Value>,
) -> Result<UntrustedText, StreamDecodeError> {
    match value {
        None | Some(Value::Null) => untrusted(""),
        Some(Value::String(text)) => untrusted(text),
        Some(Value::Object(object)) => {
            if let Some(kind) = object.get("type").and_then(Value::as_str) {
                return untrusted(kind);
            }
            serialize_untrusted(&Value::Object(object.clone()))
        }
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(FieldName::Action)),
        ),
    }
}
