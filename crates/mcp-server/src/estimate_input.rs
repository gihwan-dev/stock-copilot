use application::{CollectedItemHandle, EstimateSource, EstimateToolArguments};
use domain::{AiProbabilityInput, CalibrationEstimateId, ToolReference, UntrustedText};
use rmcp::model::JsonObject;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::tools::{ToolErrorPayload, build_object_schema};

pub const SUBMIT_ESTIMATE_TOOL: &str = "submit_estimate";
const MAX_ESTIMATE_SOURCES: usize = 8_usize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitEstimateDto {
    task_id: String,
    #[serde(rename = "p")]
    probability: u64,
    rationale: String,
    counterargument: String,
    sources: Vec<String>,
}

#[must_use]
pub fn submit_estimate_schema() -> JsonObject {
    let mut properties = Map::new();
    properties.insert(
        String::from("task_id"),
        json!({"type": "string", "minLength": 1_u64, "maxLength": 128_u64}),
    );
    properties.insert(
        String::from("p"),
        json!({"type": "integer", "minimum": 1_u64, "maximum": 99_u64}),
    );
    properties.insert(
        String::from("rationale"),
        json!({"type": "string", "minLength": 1_u64, "maxLength": 4000_u64}),
    );
    properties.insert(
        String::from("counterargument"),
        json!({"type": "string", "minLength": 1_u64, "maxLength": 4000_u64}),
    );
    properties.insert(
        String::from("sources"),
        json!({
            "type": "array",
            "maxItems": MAX_ESTIMATE_SOURCES,
            "items": {
                "type": "string",
                "pattern": "^(tool:.+|[nd]-[0-9]+)$",
                "maxLength": 128_u64
            }
        }),
    );
    build_object_schema(
        properties,
        &["task_id", "p", "rationale", "counterargument", "sources"],
    )
}

pub fn parse_submit_estimate_arguments(
    arguments: &JsonObject,
) -> Result<EstimateToolArguments<EstimateSource>, ToolErrorPayload> {
    let dto: SubmitEstimateDto =
        serde_json::from_value(Value::Object(arguments.clone())).map_err(|_error| {
            ToolErrorPayload::invalid_input(
                None,
                "submit_estimate accepts only task_id, p, rationale, counterargument and sources",
            )
        })?;
    let task_id = CalibrationEstimateId::try_new(&dto.task_id)
        .map_err(|_error| ToolErrorPayload::invalid_input(Some("task_id"), "unknown task_id"))?;
    if !(1_u64..=99_u64).contains(&dto.probability) {
        return Err(ToolErrorPayload::invalid_input(
            Some("p"),
            "p must be an integer from 1 to 99",
        ));
    }
    let probability =
        AiProbabilityInput::try_new(&dto.probability.to_string()).map_err(|_error| {
            ToolErrorPayload::invalid_input(Some("p"), "p must be an integer from 1 to 99")
        })?;
    let rationale = narrative(&dto.rationale, "rationale")?;
    let counterargument = narrative(&dto.counterargument, "counterargument")?;
    if dto.sources.len() > MAX_ESTIMATE_SOURCES {
        return Err(ToolErrorPayload::invalid_input(
            Some("sources"),
            "at most 8 sources are allowed",
        ));
    }
    let sources = dto
        .sources
        .iter()
        .map(|source| {
            if collected_handle(source) {
                CollectedItemHandle::try_new(source).map(EstimateSource::Collected).ok()
            } else {
                ToolReference::try_new(source).map(EstimateSource::Tool).ok()
            }
            .ok_or_else(|| ToolErrorPayload::invalid_input(
                Some("sources"),
                "sources must be handles from search_news or get_disclosures or tool: references from this run",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EstimateToolArguments {
        task_id,
        probability,
        rationale,
        counterargument,
        sources,
    })
}

fn collected_handle(source: &str) -> bool {
    source
        .strip_prefix("n-")
        .or_else(|| source.strip_prefix("d-"))
        .is_some_and(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn narrative(value: &str, field: &'static str) -> Result<UntrustedText, ToolErrorPayload> {
    if value.trim().is_empty() || value.chars().count() > 4_000_usize {
        return Err(ToolErrorPayload::invalid_input(
            Some(field),
            "text must be 1 to 4000 characters",
        ));
    }
    UntrustedText::try_new(value).map_err(|_error| {
        ToolErrorPayload::invalid_input(Some(field), "text must be 1 to 4000 characters")
    })
}

#[cfg(test)]
mod tests {
    use rmcp::model::JsonObject;
    use serde_json::{Value, json};

    use super::{parse_submit_estimate_arguments, submit_estimate_schema};
    use crate::tools::{ToolErrorCode, validate_arguments_against_schema};

    fn object(value: Value) -> JsonObject {
        match value {
            Value::Object(map) => map,
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_) => JsonObject::new(),
        }
    }

    fn valid() -> Value {
        json!({
            "task_id": "task-1",
            "p": 61_u64,
            "rationale": "실적 개선",
            "counterargument": "환율 부담",
            "sources": ["tool:q1"]
        })
    }

    #[test]
    fn five_fields_are_parsed() {
        let parsed = parse_submit_estimate_arguments(&object(valid())).expect("입력");
        assert_eq!(parsed.task_id.as_str(), "task-1");
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(
            validate_arguments_against_schema(&object(valid()), &submit_estimate_schema()),
            Ok(())
        );
    }

    #[test]
    fn extra_order_fields_are_rejected() {
        for extra in ["side", "quantity", "price", "direction", "order"] {
            let mut value = object(valid());
            value.insert(String::from(extra), json!("1"));
            assert_eq!(
                parse_submit_estimate_arguments(&value).map(|_args| ()),
                Err(crate::tools::ToolErrorPayload::invalid_input(
                    None,
                    "submit_estimate accepts only task_id, p, rationale, counterargument and sources",
                ))
            );
            assert_eq!(
                validate_arguments_against_schema(&value, &submit_estimate_schema())
                    .map_err(|payload| payload.code()),
                Err(ToolErrorCode::InvalidInput)
            );
        }
    }

    #[test]
    fn bad_values_are_rejected() {
        for (field, bad) in [
            ("p", json!(0_u64)),
            ("p", json!(100_u64)),
            ("p", json!(55.5_f64)),
            ("p", json!("55")),
            ("rationale", json!(" ")),
            ("counterargument", json!("")),
            ("sources", json!(["https://example.com"])),
            (
                "sources",
                json!([
                    "tool:a", "tool:b", "tool:c", "tool:d", "tool:e", "tool:f", "tool:g", "tool:h",
                    "tool:i"
                ]),
            ),
            ("task_id", json!("")),
        ] {
            let mut value = object(valid());
            value.insert(String::from(field), bad);
            let error = parse_submit_estimate_arguments(&value).map(|_args| ());
            assert_eq!(
                error.map_err(|payload| payload.code()),
                Err(ToolErrorCode::InvalidInput),
                "{field}"
            );
        }
        let mut missing = object(valid());
        missing.remove("sources");
        assert_eq!(
            parse_submit_estimate_arguments(&missing)
                .map(|_args| ())
                .map_err(|payload| payload.code()),
            Err(ToolErrorCode::InvalidInput)
        );
    }

    #[test]
    fn empty_sources_are_accepted() {
        let mut value = object(valid());
        value.insert(String::from("sources"), json!([]));
        let parsed = parse_submit_estimate_arguments(&value).expect("입력");
        assert_eq!(parsed.sources, Vec::new());
    }
}
