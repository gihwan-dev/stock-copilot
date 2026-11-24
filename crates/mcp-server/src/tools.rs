use std::borrow::Cow;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use application::{
    AiRunId, Clock, RecordOutcome, RetryDelay, SystemFailure, SystemPort, SystemPortError,
    ToolCallLedger, ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolName,
    ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
};
use rmcp::model::{CallToolResult, JsonObject, Tool, ToolAnnotations};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::pagination::{CursorStore, MAX_TOOL_RESPONSE_CHARS};
use crate::token::RunAuthBinding;

pub const MAX_TOOL_ERROR_CHARS: usize = 4_000_usize;

const FORBIDDEN_SCHEMA_COMBINATORS: [&str; 3] = ["anyOf", "oneOf", "allOf"];
const MAX_SCHEMA_DEPTH: usize = 8_usize;
const MAX_LEDGER_PAYLOAD_CHARS: usize = 16_384_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    UnknownTool,
    InvalidInput,
    InvalidCursor,
    CursorExpired,
    ResponseTooLarge,
    LedgerUnavailable,
    DataUnavailable,
    UpstreamUnavailable,
    ForbiddenOperation,
    RateLimited,
    InternalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    Retryable,
    NonRetryable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolErrorPayload {
    code: ToolErrorCode,
    message: String,
    retryable: RetryPolicy,
    field: Option<String>,
}

impl ToolErrorPayload {
    #[must_use]
    pub fn invalid_input(field: Option<&str>, message: &'static str) -> Self {
        Self::non_retryable(ToolErrorCode::InvalidInput, field, message)
    }

    #[must_use]
    pub fn response_too_large() -> Self {
        Self::non_retryable(
            ToolErrorCode::ResponseTooLarge,
            None,
            "tool response exceeded 30000 character limit; narrow the query or limit",
        )
    }

    #[must_use]
    pub fn ledger_unavailable() -> Self {
        Self::retryable(
            ToolErrorCode::LedgerUnavailable,
            None,
            "tool call ledger could not record this invocation",
        )
    }

    #[must_use]
    pub fn non_retryable(code: ToolErrorCode, field: Option<&str>, message: &'static str) -> Self {
        Self::build(code, RetryPolicy::NonRetryable, field, message)
    }

    #[must_use]
    pub fn retryable(code: ToolErrorCode, field: Option<&str>, message: &'static str) -> Self {
        Self::build(code, RetryPolicy::Retryable, field, message)
    }

    fn build(
        code: ToolErrorCode,
        retryable: RetryPolicy,
        field: Option<&str>,
        message: &'static str,
    ) -> Self {
        let safe_message = sanitize_fixed_message(message);
        let safe_field = field
            .filter(|name| ToolName::try_new(name).is_ok())
            .map(str::to_owned);
        Self {
            code,
            message: safe_message,
            retryable,
            field: safe_field,
        }
    }

    #[must_use]
    pub const fn code(&self) -> ToolErrorCode {
        self.code
    }

    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut obj = Map::new();
        let code_value = match serde_json::to_value(self.code) {
            Ok(encoded) => encoded,
            Err(_error) => Value::String(String::from("internal_error")),
        };
        obj.insert(String::from("code"), code_value);
        obj.insert(String::from("message"), Value::String(self.message.clone()));
        let retry_bool = match self.retryable {
            RetryPolicy::Retryable => true,
            RetryPolicy::NonRetryable => false,
        };
        obj.insert(String::from("retryable"), Value::Bool(retry_bool));
        if let Some(field) = &self.field {
            obj.insert(String::from("field"), Value::String(field.clone()));
        }
        Value::Object(obj)
    }

    #[must_use]
    pub fn to_call_tool_result(&self) -> (CallToolResult, String) {
        let value = self.to_json_value();
        let text = match serde_json::to_string(&value) {
            Ok(rendered) => rendered,
            Err(_error) => String::from(
                "{\"code\":\"internal_error\",\"message\":\"tool error\",\"retryable\":false}",
            ),
        };
        (CallToolResult::structured_error(value), text)
    }
}

fn sanitize_fixed_message(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TOOL_ERROR_CHARS)
        .collect();
    if cleaned.is_empty() {
        String::from("tool invocation failed")
    } else {
        cleaned
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolAccessKind {
    ReadOnly,
    StateChanging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRegistryError {
    DuplicateToolName,
    InvalidDescription,
    InvalidInputSchema,
    InvalidOutputSchema,
    ForbiddenToolName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultRetention {
    RecordInLedger,
    OmitFromLedger,
}

const OMITTED_RESULT_JSON: &str = "{\"omitted\":\"tool result is delivered once and not stored\"}";

#[derive(Debug, Clone)]
pub struct ToolSpec {
    name: ToolName,
    description: String,
    access: ToolAccessKind,
    input_schema: JsonObject,
    output_schema: Option<JsonObject>,
    retention: ResultRetention,
}

impl ToolSpec {
    pub fn try_new(
        name: ToolName,
        description: &'static str,
        access: ToolAccessKind,
        input_schema: JsonObject,
        output_schema: Option<JsonObject>,
    ) -> Result<Self, ToolRegistryError> {
        if description.is_empty() || description.chars().any(char::is_control) {
            return Err(ToolRegistryError::InvalidDescription);
        }
        if is_forbidden_tool_name(name.as_str()) {
            return Err(ToolRegistryError::ForbiddenToolName);
        }
        validate_input_schema_object(&input_schema)?;
        if let Some(output) = &output_schema {
            validate_output_schema_object(output)?;
        }
        Ok(Self {
            name,
            description: description.to_owned(),
            access,
            input_schema,
            output_schema,
            retention: ResultRetention::RecordInLedger,
        })
    }

    #[must_use]
    pub const fn with_result_omitted_from_ledger(mut self) -> Self {
        self.retention = ResultRetention::OmitFromLedger;
        self
    }

    #[must_use]
    pub const fn retention(&self) -> ResultRetention {
        self.retention
    }

    #[must_use]
    pub const fn name(&self) -> &ToolName {
        &self.name
    }

    #[must_use]
    pub const fn access(&self) -> ToolAccessKind {
        self.access
    }

    #[must_use]
    pub const fn input_schema(&self) -> &JsonObject {
        &self.input_schema
    }

    #[must_use]
    pub fn to_rmcp_tool(&self) -> Tool {
        let annotations = match self.access {
            ToolAccessKind::ReadOnly => ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true)
                .open_world(false),
            ToolAccessKind::StateChanging => ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(false)
                .open_world(false),
        };
        let mut tool = Tool::new(
            Cow::Owned(self.name.as_str().to_owned()),
            Cow::Owned(self.description.clone()),
            Arc::new(self.input_schema.clone()),
        )
        .with_annotations(annotations);
        if let Some(output) = &self.output_schema {
            tool.output_schema = Some(Arc::new(output.clone()));
        }
        tool
    }
}

fn is_forbidden_tool_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    for forbidden_fragment in [
        "order",
        "trade",
        "approve",
        "approval",
        "confirm_order",
        "cancel_order",
        "setting",
        "secret",
        "keychain",
        "api_key",
        "credential",
        "token",
        "execute_buy",
        "execute_sell",
    ] {
        if lower.contains(forbidden_fragment) {
            return true;
        }
    }
    false
}

fn validate_schema_node(schema: &JsonObject, depth: usize) -> Result<(), ToolRegistryError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(ToolRegistryError::InvalidInputSchema);
    }
    for banned in FORBIDDEN_SCHEMA_COMBINATORS {
        if schema.contains_key(banned) {
            return Err(ToolRegistryError::InvalidInputSchema);
        }
    }
    let Some(schema_type) = schema.get("type").and_then(Value::as_str) else {
        return Err(ToolRegistryError::InvalidInputSchema);
    };
    match schema_type {
        "string" | "integer" | "number" | "boolean" => Ok(()),
        "array" => {
            if let Some(items) = schema.get("items") {
                let Some(items_obj) = items.as_object() else {
                    return Err(ToolRegistryError::InvalidInputSchema);
                };
                validate_schema_node(items_obj, depth.saturating_add(1_usize))?;
            }
            Ok(())
        }
        "object" => validate_object_schema_node(schema, depth),
        _other => Err(ToolRegistryError::InvalidInputSchema),
    }
}

fn validate_object_schema_node(schema: &JsonObject, depth: usize) -> Result<(), ToolRegistryError> {
    if schema.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
        return Err(ToolRegistryError::InvalidInputSchema);
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Err(ToolRegistryError::InvalidInputSchema);
    };
    for (prop_name, prop_value) in properties {
        if ToolName::try_new(prop_name).is_err() {
            return Err(ToolRegistryError::InvalidInputSchema);
        }
        let Some(prop_obj) = prop_value.as_object() else {
            return Err(ToolRegistryError::InvalidInputSchema);
        };
        validate_schema_node(prop_obj, depth.saturating_add(1_usize))?;
    }
    if let Some(required) = schema.get("required") {
        let Some(items) = required.as_array() else {
            return Err(ToolRegistryError::InvalidInputSchema);
        };
        for item in items {
            let Some(key) = item.as_str() else {
                return Err(ToolRegistryError::InvalidInputSchema);
            };
            if !properties.contains_key(key) {
                return Err(ToolRegistryError::InvalidInputSchema);
            }
        }
    }
    Ok(())
}

fn validate_input_schema_object(schema: &JsonObject) -> Result<(), ToolRegistryError> {
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ToolRegistryError::InvalidInputSchema);
    }
    validate_schema_node(schema, 0_usize)
}

fn validate_output_schema_object(schema: &JsonObject) -> Result<(), ToolRegistryError> {
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ToolRegistryError::InvalidOutputSchema);
    }
    Ok(())
}

fn validate_string_constraints(
    key: &str,
    text: &str,
    schema_obj: &Map<String, Value>,
) -> Result<(), ToolErrorPayload> {
    let char_len = u64::try_from(text.chars().count()).map_err(|_overflow| {
        ToolErrorPayload::invalid_input(Some(key), "string argument length overflowed")
    })?;
    if let Some(min_len) = schema_obj.get("minLength").and_then(Value::as_u64)
        && char_len < min_len
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "string argument is shorter than minLength",
        ));
    }
    if let Some(max_len) = schema_obj.get("maxLength").and_then(Value::as_u64)
        && char_len > max_len
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "string argument exceeds maxLength",
        ));
    }
    Ok(())
}

fn extract_json_i128(value: &Value) -> Option<i128> {
    match (value.as_i64(), value.as_u64()) {
        (Some(signed), _) => Some(i128::from(signed)),
        (None, Some(unsigned)) => Some(i128::from(unsigned)),
        (None, None) => None,
    }
}

fn validate_integer_constraints(
    key: &str,
    num: i128,
    schema_obj: &Map<String, Value>,
) -> Result<(), ToolErrorPayload> {
    if let Some(min_val) = schema_obj.get("minimum").and_then(extract_json_i128)
        && num < min_val
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "integer argument is below minimum",
        ));
    }
    if let Some(max_val) = schema_obj.get("maximum").and_then(extract_json_i128)
        && num > max_val
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "integer argument exceeds maximum",
        ));
    }
    Ok(())
}

fn validate_array_constraints(
    key: &str,
    items: &[Value],
    schema_obj: &Map<String, Value>,
    depth: usize,
) -> Result<(), ToolErrorPayload> {
    let item_count = u64::try_from(items.len()).map_err(|_overflow| {
        ToolErrorPayload::invalid_input(Some(key), "array argument length overflowed")
    })?;
    if let Some(min_items) = schema_obj.get("minItems").and_then(Value::as_u64)
        && item_count < min_items
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "array argument has fewer items than minItems",
        ));
    }
    if let Some(max_items) = schema_obj.get("maxItems").and_then(Value::as_u64)
        && item_count > max_items
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "array argument exceeds maxItems",
        ));
    }
    if let Some(item_schema) = schema_obj.get("items") {
        for element in items {
            validate_property_value(key, element, item_schema, depth.saturating_add(1_usize))?;
        }
    }
    Ok(())
}

fn validate_property_value(
    key: &str,
    value: &Value,
    prop_schema: &Value,
    depth: usize,
) -> Result<(), ToolErrorPayload> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "argument nesting exceeds maximum depth",
        ));
    }
    let Some(schema_obj) = prop_schema.as_object() else {
        return Err(ToolErrorPayload::non_retryable(
            ToolErrorCode::InternalError,
            None,
            "tool schema property definition is invalid",
        ));
    };
    let Some(expected_type) = schema_obj.get("type").and_then(Value::as_str) else {
        return Err(ToolErrorPayload::non_retryable(
            ToolErrorCode::InternalError,
            None,
            "tool schema property missing type",
        ));
    };
    let type_matches = match expected_type {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _other => false,
    };
    if !type_matches {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "argument field has invalid JSON type",
        ));
    }
    if let Some(allowed_values) = schema_obj.get("enum").and_then(Value::as_array)
        && !allowed_values.contains(value)
    {
        return Err(ToolErrorPayload::invalid_input(
            Some(key),
            "argument field is outside allowed enum values",
        ));
    }
    if let Some(text) = value.as_str() {
        validate_string_constraints(key, text, schema_obj)?;
    }
    if expected_type == "integer"
        && let Some(num) = extract_json_i128(value)
    {
        validate_integer_constraints(key, num, schema_obj)?;
    }
    if let Some(items) = value.as_array() {
        validate_array_constraints(key, items, schema_obj, depth)?;
    }
    if let Some(nested_obj) = value.as_object() {
        validate_object_arguments(nested_obj, schema_obj, depth.saturating_add(1_usize))?;
    }
    Ok(())
}

fn validate_object_arguments(
    arguments: &JsonObject,
    schema: &JsonObject,
    depth: usize,
) -> Result<(), ToolErrorPayload> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(ToolErrorPayload::invalid_input(
            None,
            "argument nesting exceeds maximum depth",
        ));
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Err(ToolErrorPayload::non_retryable(
            ToolErrorCode::InternalError,
            None,
            "tool schema missing properties map",
        ));
    };
    for (key, value) in arguments {
        let Some(prop_schema) = properties.get(key) else {
            return Err(ToolErrorPayload::invalid_input(
                None,
                "unknown argument field is not allowed",
            ));
        };
        validate_property_value(key, value, prop_schema, depth)?;
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for req in required {
            if let Some(key) = req.as_str()
                && !arguments.contains_key(key)
            {
                return Err(ToolErrorPayload::invalid_input(
                    Some(key),
                    "required argument field is missing",
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_arguments_against_schema(
    arguments: &JsonObject,
    schema: &JsonObject,
) -> Result<(), ToolErrorPayload> {
    validate_object_arguments(arguments, schema, 0_usize)
}

#[derive(Debug)]
pub struct ToolInvocation<'a> {
    pub run_id: &'a AiRunId,
    pub arguments: &'a JsonObject,
}

pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Value, ToolErrorPayload>> + Send + 'a>>;

pub trait McpTool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<ToolName, (ToolSpec, Arc<dyn McpTool>)>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.tools.keys().map(ToolName::as_str).collect();
        f.debug_struct("ToolRegistry")
            .field("tools", &names)
            .finish()
    }
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn McpTool>) -> Result<(), ToolRegistryError> {
        let spec = tool.spec();
        if self.tools.contains_key(spec.name()) {
            return Err(ToolRegistryError::DuplicateToolName);
        }
        let key = spec.name().clone();
        self.tools.insert(key, (spec, tool));
        Ok(())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    #[must_use]
    pub fn tool_names(&self) -> Vec<ToolName> {
        self.tools.keys().cloned().collect()
    }

    #[must_use]
    pub fn list_rmcp_tools(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|(spec, _tool)| spec.to_rmcp_tool())
            .collect()
    }

    #[must_use]
    pub fn get(&self, name: &ToolName) -> Option<(&ToolSpec, &Arc<dyn McpTool>)> {
        self.tools.get(name).map(|(spec, tool)| (spec, tool))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnconfiguredToolLedger;

impl ToolCallLedger for UnconfiguredToolLedger {
    fn record_call(
        &self,
        _: &ToolCallRecord,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}

#[derive(Debug)]
pub enum ToolLedgerMode<L> {
    Configured(Arc<L>),
    Unconfigured,
}

impl<L> Clone for ToolLedgerMode<L> {
    fn clone(&self) -> Self {
        match self {
            Self::Configured(ledger) => Self::Configured(Arc::clone(ledger)),
            Self::Unconfigured => Self::Unconfigured,
        }
    }
}

pub(crate) struct ToolRuntime<L> {
    pub(crate) registry: Arc<ToolRegistry>,
    pub(crate) ledger: ToolLedgerMode<L>,
}

impl<L> Clone for ToolRuntime<L> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            ledger: self.ledger.clone(),
        }
    }
}

fn encode_tool_success(value: Value) -> Result<(CallToolResult, String), ToolErrorPayload> {
    let text = serde_json::to_string(&value).map_err(|_error| {
        ToolErrorPayload::retryable(
            ToolErrorCode::InternalError,
            None,
            "failed to serialize tool response",
        )
    })?;
    if text.chars().count() > MAX_TOOL_RESPONSE_CHARS {
        return Err(ToolErrorPayload::response_too_large());
    }
    Ok((CallToolResult::structured(value), text))
}

fn bound_ledger_payload(raw: &str) -> Result<ToolPayloadJson, ToolErrorPayload> {
    if let Ok(valid) = ToolPayloadJson::try_new(raw) {
        return Ok(valid);
    }
    let clipped: String = raw
        .chars()
        .filter(|ch| *ch != '\0')
        .take(MAX_LEDGER_PAYLOAD_CHARS)
        .collect();
    if let Ok(valid) = ToolPayloadJson::try_new(&clipped) {
        return Ok(valid);
    }
    ToolPayloadJson::try_new("{}").map_err(|_err| ToolErrorPayload::ledger_unavailable())
}

fn sanitize_request_id(raw: &str) -> Result<ToolRequestId, ToolErrorPayload> {
    if let Ok(valid) = ToolRequestId::try_new(raw) {
        return Ok(valid);
    }
    let cleaned: String = raw
        .bytes()
        .filter(|byte| (0x20_u8..=0x7e_u8).contains(byte))
        .take(128_usize)
        .map(char::from)
        .collect();
    if let Ok(valid) = ToolRequestId::try_new(&cleaned) {
        return Ok(valid);
    }
    ToolRequestId::try_new("unknown-request-id")
        .map_err(|_err| ToolErrorPayload::ledger_unavailable())
}

fn sanitize_protocol_version(raw: &str) -> Result<ToolProtocolVersion, ToolErrorPayload> {
    if let Ok(valid) = ToolProtocolVersion::try_new(raw) {
        return Ok(valid);
    }
    ToolProtocolVersion::try_new("2026-07-28")
        .map_err(|_err| ToolErrorPayload::ledger_unavailable())
}

fn fallback_tool_name() -> Result<ToolName, ToolErrorPayload> {
    ToolName::try_new("unknown_tool").map_err(|_err| ToolErrorPayload::ledger_unavailable())
}

async fn execute_registered_tool(
    registry: &ToolRegistry,
    run_id: &AiRunId,
    raw_name: &str,
    arguments: &JsonObject,
) -> (
    CallToolResult,
    ToolCallOutcome,
    String,
    Result<ToolName, ToolErrorPayload>,
) {
    let Ok(tool_name) = ToolName::try_new(raw_name) else {
        let (result, text) = ToolErrorPayload::non_retryable(
            ToolErrorCode::UnknownTool,
            Some("name"),
            "requested tool name is invalid",
        )
        .to_call_tool_result();
        return (
            result,
            ToolCallOutcome::Rejected,
            text,
            fallback_tool_name(),
        );
    };
    let Some((spec, tool)) = registry.get(&tool_name) else {
        let (result, text) = ToolErrorPayload::non_retryable(
            ToolErrorCode::UnknownTool,
            Some("name"),
            "requested tool is not registered",
        )
        .to_call_tool_result();
        return (result, ToolCallOutcome::Rejected, text, Ok(tool_name));
    };
    if let Err(err_payload) = validate_arguments_against_schema(arguments, spec.input_schema()) {
        let (result, text) = err_payload.to_call_tool_result();
        return (result, ToolCallOutcome::Rejected, text, Ok(tool_name));
    }
    let invocation = ToolInvocation { run_id, arguments };
    match tool.invoke(invocation).await.and_then(encode_tool_success) {
        Ok((result, text)) => (result, ToolCallOutcome::Succeeded, text, Ok(tool_name)),
        Err(err_payload) => {
            let (result, text) = err_payload.to_call_tool_result();
            (result, ToolCallOutcome::ToolError, text, Ok(tool_name))
        }
    }
}

pub(crate) struct DispatchCallInput<'a> {
    pub(crate) request_id_raw: &'a str,
    pub(crate) protocol_version_raw: &'a str,
    pub(crate) raw_tool_name: &'a str,
    pub(crate) arguments: &'a JsonObject,
}

fn ledger_result_text(
    registry: &ToolRegistry,
    recorded_name: &Result<ToolName, ToolErrorPayload>,
    outcome: ToolCallOutcome,
    result_text: String,
) -> String {
    let retention = recorded_name
        .as_ref()
        .ok()
        .and_then(|name| registry.get(name))
        .map_or(ResultRetention::RecordInLedger, |(spec, _tool)| {
            spec.retention()
        });
    match (retention, outcome) {
        (ResultRetention::OmitFromLedger, ToolCallOutcome::Succeeded) => {
            String::from(OMITTED_RESULT_JSON)
        }
        (
            ResultRetention::OmitFromLedger,
            ToolCallOutcome::ToolError | ToolCallOutcome::Rejected,
        )
        | (ResultRetention::RecordInLedger, _) => result_text,
    }
}

pub(crate) async fn dispatch_tool_call<C: Clock, L: ToolCallLedger>(
    runtime: &ToolRuntime<L>,
    auth: &RunAuthBinding<C>,
    input: DispatchCallInput<'_>,
) -> CallToolResult {
    let started_at = auth.clock().now();
    let (result, outcome, result_text, recorded_name_result) = execute_registered_tool(
        &runtime.registry,
        auth.run_id(),
        input.raw_tool_name,
        input.arguments,
    )
    .await;
    let finished_at = auth.clock().now();

    let args_text = match serde_json::to_string(input.arguments) {
        Ok(encoded) => auth.redact_token(&encoded),
        Err(_error) => String::from("{}"),
    };
    let ledger_result = ledger_result_text(
        &runtime.registry,
        &recorded_name_result,
        outcome,
        result_text,
    );
    let redacted_result = auth.redact_token(&ledger_result);
    let record_build = (|| -> Result<ToolCallRecord, ToolErrorPayload> {
        Ok(ToolCallRecord::new(ToolCallRecordInput {
            run_id: auth.run_id().clone(),
            request_id: sanitize_request_id(input.request_id_raw)?,
            tool_name: recorded_name_result?,
            arguments_json: bound_ledger_payload(&args_text)?,
            result_json: bound_ledger_payload(&redacted_result)?,
            outcome,
            started_at,
            finished_at,
            protocol_version: sanitize_protocol_version(input.protocol_version_raw)?,
        }))
    })();
    let record = match record_build {
        Ok(built) => built,
        Err(err_payload) => return err_payload.to_call_tool_result().0,
    };

    match &runtime.ledger {
        ToolLedgerMode::Configured(ledger) => match ledger.record_call(&record).await {
            Ok(_recorded) => result,
            Err(_error) => {
                ToolErrorPayload::ledger_unavailable()
                    .to_call_tool_result()
                    .0
            }
        },
        ToolLedgerMode::Unconfigured => {
            ToolErrorPayload::ledger_unavailable()
                .to_call_tool_result()
                .0
        }
    }
}

#[must_use]
pub fn build_object_schema(properties: Map<String, Value>, required: &[&str]) -> JsonObject {
    let mut schema = JsonObject::new();
    schema.insert(String::from("type"), Value::String(String::from("object")));
    schema.insert(String::from("properties"), Value::Object(properties));
    if !required.is_empty() {
        let req_values: Vec<Value> = required
            .iter()
            .map(|name| Value::String((*name).to_owned()))
            .collect();
        schema.insert(String::from("required"), Value::Array(req_values));
    }
    schema.insert(String::from("additionalProperties"), Value::Bool(false));
    schema
}

#[must_use]
pub fn make_cursor_store<C: Clock>(clock: Arc<C>) -> Arc<CursorStore<C>> {
    let store = match CursorStore::try_new(
        Arc::clone(&clock),
        crate::pagination::DEFAULT_CURSOR_TTL_SECS,
    ) {
        Ok(created) => created,
        Err(_err) => CursorStore::with_default_ttl(clock),
    };
    Arc::new(store)
}
