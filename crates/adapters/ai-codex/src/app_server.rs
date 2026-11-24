use crate::error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError};
use crate::isolation::{
    CodexIsolationExpectation, McpStatusListSnapshot, ThreadStartSnapshot,
    evaluate_codex_isolation, parse_mcp_status_list, parse_thread_start_snapshot,
};
use crate::parse::{
    parse_model_name, parse_reset_time, parse_token_usage_camel, parse_tool_status,
    parse_used_percent, parse_web_search_action, render_mcp_tool_result, required_object,
    required_str, serialize_untrusted, untrusted,
};
use application::{
    AiEvent, AiUsage, IsolationCheck, IsolationFailure, ModelName, ResetTime, ToolStatus,
};
use domain::UntrustedText;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    AwaitingIsolation,
    Active,
    TerminatedAfterRejection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnDeltaState {
    NoneYet,
    Streamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingTool {
    server: UntrustedText,
    tool: UntrustedText,
    args: UntrustedText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAppServerDecoder {
    expectation: CodexIsolationExpectation,
    lifecycle: Lifecycle,
    thread_snapshot: Option<ThreadStartSnapshot>,
    mcp_snapshot: Option<McpStatusListSnapshot>,
    configured_model: Option<ModelName>,
    turn_delta_state: TurnDeltaState,
    pending_tools: BTreeMap<String, PendingTool>,
}

impl CodexAppServerDecoder {
    #[must_use]
    pub fn new(expectation: CodexIsolationExpectation) -> Self {
        Self {
            expectation,
            lifecycle: Lifecycle::AwaitingIsolation,
            thread_snapshot: None,
            mcp_snapshot: None,
            configured_model: None,
            turn_delta_state: TurnDeltaState::NoneYet,
            pending_tools: BTreeMap::new(),
        }
    }

    pub fn decode_line(&mut self, line: &str) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.trim().is_empty() {
            return Err(StreamDecodeError::EmptyLine);
        }
        if trimmed.len() > 65_536_usize {
            return Err(StreamDecodeError::LineTooLong {
                byte_len: trimmed.len(),
            });
        }
        if self.lifecycle == Lifecycle::TerminatedAfterRejection {
            return Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            ));
        }
        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_error) => {
                return Err(StreamDecodeError::MalformedJson {
                    raw: untrusted(trimmed)?,
                });
            }
        };
        let Some(object) = parsed.as_object() else {
            return Err(StreamDecodeError::Schema(SchemaFailure::ExpectedObject));
        };
        self.dispatch_envelope(object, trimmed)
    }

    pub fn finish(self) -> Result<(), StreamDecodeError> {
        match self.lifecycle {
            Lifecycle::AwaitingIsolation => Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            )),
            Lifecycle::TerminatedAfterRejection => Ok(()),
            Lifecycle::Active => {
                if self.pending_tools.is_empty() {
                    Ok(())
                } else {
                    Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall))
                }
            }
        }
    }

    fn dispatch_envelope(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        if let Some(method_value) = object.get("method") {
            let method = method_value.as_str().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Method),
            ))?;
            if method.is_empty() {
                return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::Method,
                )));
            }
            return self.handle_notification(method, object, raw_line);
        }
        if let Some(result_value) = object.get("result") {
            let result = result_value.as_object().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Result),
            ))?;
            return self.handle_rpc_result(result, raw_line);
        }
        if let Some(error_value) = object.get("error") {
            let error_obj = error_value.as_object().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Error),
            ))?;
            return self.handle_rpc_error(error_obj, raw_line);
        }
        Err(StreamDecodeError::Schema(
            SchemaFailure::UnrecognizedEnvelope,
        ))
    }

    fn handle_rpc_result(
        &mut self,
        result: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        if result.contains_key("sandbox") && result.contains_key("instructionSources") {
            let snapshot = parse_thread_start_snapshot(result)?;
            if let Some(raw_model) = &snapshot.model {
                self.configured_model = Some(parse_model_name(raw_model)?);
            }
            self.thread_snapshot = Some(snapshot);
            return Ok(self.try_complete_isolation());
        }
        if result.contains_key("data") && self.lifecycle == Lifecycle::AwaitingIsolation {
            let snapshot = parse_mcp_status_list(result)?;
            self.mcp_snapshot = Some(snapshot);
            return Ok(self.try_complete_isolation());
        }
        if result.contains_key("data") && self.lifecycle == Lifecycle::Active {
            let snapshot = parse_mcp_status_list(result)?;
            self.mcp_snapshot = Some(snapshot);
            return self.recheck_active_isolation();
        }
        if self.lifecycle == Lifecycle::AwaitingIsolation {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        }
        if result.contains_key("rateLimits") || result.contains_key("ordinaryUsageAllowed") {
            return handle_rate_limits_payload(result);
        }
        Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
    }

    fn try_complete_isolation(&mut self) -> Vec<AiEvent> {
        let (Some(thread), Some(mcp)) = (&self.thread_snapshot, &self.mcp_snapshot) else {
            return Vec::new();
        };
        let check = evaluate_codex_isolation(thread, mcp, &self.expectation);
        match check {
            IsolationCheck::Passed => self.lifecycle = Lifecycle::Active,
            IsolationCheck::Rejected(_) => self.lifecycle = Lifecycle::TerminatedAfterRejection,
        }
        vec![AiEvent::IsolationChecked(check)]
    }

    fn recheck_active_isolation(&self) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let (Some(thread), Some(mcp)) = (&self.thread_snapshot, &self.mcp_snapshot) else {
            return Ok(Vec::new());
        };
        match evaluate_codex_isolation(thread, mcp, &self.expectation) {
            IsolationCheck::Passed => Ok(Vec::new()),
            IsolationCheck::Rejected(IsolationFailure::McpServers) => {
                Ok(vec![AiEvent::McpUnavailable])
            }
            IsolationCheck::Rejected(failure) => Err(StreamDecodeError::Order(
                StreamOrderError::IsolationRegressed(failure),
            )),
        }
    }

    fn handle_rpc_error(
        &mut self,
        error_obj: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let info = extract_codex_error_info(error_obj)?;
        if info == Some("unauthorized") && self.lifecycle == Lifecycle::AwaitingIsolation {
            self.lifecycle = Lifecycle::TerminatedAfterRejection;
            return Ok(vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::LoginRequired,
            ))]);
        }
        if self.lifecycle == Lifecycle::AwaitingIsolation {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        }
        if matches_rate_limit_code(info) == LimitCodeMatch::RateLimited {
            let resets_at = parse_reset_time(error_obj.get("resetsAt"), FieldName::ResetsAt)?;
            return Ok(vec![AiEvent::RateLimited { resets_at }]);
        }
        Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
    }

    fn handle_notification(
        &mut self,
        method: &str,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        if method == "mcpServer/startupStatus/updated" {
            return self.handle_mcp_startup_status(object, raw_line);
        }
        if self.lifecycle == Lifecycle::AwaitingIsolation {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        }
        let params = required_object(object, "params", FieldName::Params)?;
        match method {
            "item/agentMessage/delta" => self.handle_text_delta(params),
            "item/started" => self.handle_item_started(params, raw_line),
            "item/completed" => self.handle_item_completed(params, raw_line),
            "thread/tokenUsage/updated" => handle_token_usage(params),
            "account/rateLimits/updated" => handle_rate_limits_payload(params),
            "model/rerouted" => handle_model_rerouted(params),
            "turn/completed" => self.handle_turn_completed(params),
            "error" => handle_error_notification(params, raw_line),
            _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
        }
    }

    fn handle_mcp_startup_status(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let params = required_object(object, "params", FieldName::Params)?;
        let status = required_str(params, "status", FieldName::Status)?;
        if status == "failed" || status == "error" {
            return match self.lifecycle {
                Lifecycle::AwaitingIsolation => {
                    self.lifecycle = Lifecycle::TerminatedAfterRejection;
                    Ok(vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                        IsolationFailure::McpServers,
                    ))])
                }
                Lifecycle::Active => Ok(vec![AiEvent::McpUnavailable]),
                Lifecycle::TerminatedAfterRejection => Err(StreamDecodeError::Order(
                    StreamOrderError::EventAfterRejection,
                )),
            };
        }
        match self.lifecycle {
            Lifecycle::AwaitingIsolation => Ok(Vec::new()),
            Lifecycle::Active => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
            Lifecycle::TerminatedAfterRejection => Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            )),
        }
    }

    fn handle_text_delta(
        &mut self,
        params: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let delta = required_str(params, "delta", FieldName::Delta)?;
        self.turn_delta_state = TurnDeltaState::Streamed;
        Ok(vec![AiEvent::TextDelta(untrusted(delta)?)])
    }

    fn handle_item_started(
        &mut self,
        params: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let item = required_object(params, "item", FieldName::Item)?;
        let item_type = required_str(item, "type", FieldName::ItemType)?;
        match item_type {
            "mcpToolCall" => self.start_mcp_tool(item),
            "webSearch" => handle_web_search_item(item),
            _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
        }
    }

    fn start_mcp_tool(
        &mut self,
        item: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let id = required_str(item, "id", FieldName::Id)?;
        if id.is_empty() {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Id,
            )));
        }
        if self.pending_tools.contains_key(id) {
            return Err(StreamDecodeError::Order(
                StreamOrderError::DuplicateToolCallId,
            ));
        }
        let server = untrusted(required_str(item, "server", FieldName::Server)?)?;
        let tool = untrusted(required_str(item, "tool", FieldName::Tool)?)?;
        let arguments =
            item.get("arguments")
                .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
                    FieldName::Arguments,
                )))?;
        let args = serialize_untrusted(arguments)?;
        let result = untrusted("")?;
        self.pending_tools.insert(
            String::from(id),
            PendingTool {
                server: server.clone(),
                tool: tool.clone(),
                args: args.clone(),
            },
        );
        Ok(vec![AiEvent::ToolCall {
            server,
            tool,
            args,
            result,
            status: ToolStatus::Running,
        }])
    }

    fn handle_item_completed(
        &mut self,
        params: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let item = required_object(params, "item", FieldName::Item)?;
        let item_type = required_str(item, "type", FieldName::ItemType)?;
        match item_type {
            "mcpToolCall" => self.complete_mcp_tool(item),
            "webSearch" => handle_web_search_item(item),
            "agentMessage" => {
                let text = required_str(item, "text", FieldName::Text)?;
                if self.turn_delta_state == TurnDeltaState::NoneYet {
                    Ok(vec![AiEvent::TextDelta(untrusted(text)?)])
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
        }
    }

    fn complete_mcp_tool(
        &mut self,
        item: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let id = required_str(item, "id", FieldName::Id)?;
        let Some(pending) = self.pending_tools.remove(id) else {
            return Err(StreamDecodeError::Order(
                StreamOrderError::UnexpectedToolCompletion,
            ));
        };
        let status = parse_tool_status(required_str(item, "status", FieldName::Status)?)?;
        let result = render_mcp_tool_result(item)?;
        Ok(vec![AiEvent::ToolCall {
            server: pending.server,
            tool: pending.tool,
            args: pending.args,
            result,
            status,
        }])
    }

    fn handle_turn_completed(
        &mut self,
        params: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let status = match params.get("status") {
            None | Some(Value::Null) => "completed",
            Some(Value::String(text)) => text.as_str(),
            Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
                return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::Status,
                )));
            }
        };
        if status != "failed" && !self.pending_tools.is_empty() {
            return Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall));
        }
        let mut events = Vec::new();
        if let Some(model_value) = params.get("model")
            && !model_value.is_null()
        {
            let raw_model = model_value.as_str().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Model),
            ))?;
            let actual = parse_model_name(raw_model)?;
            if let Some(configured) = &self.configured_model
                && configured != &actual
            {
                events.push(AiEvent::ModelRerouted(actual.clone()));
            }
            events.push(AiEvent::ActualModel(actual));
        }
        if status == "failed" {
            self.pending_tools.clear();
            if let Some(event) = extract_turn_failure_rate_limit(params.get("error"))? {
                events.push(event);
            }
        }
        self.turn_delta_state = TurnDeltaState::NoneYet;
        Ok(events)
    }
}

fn extract_turn_failure_rate_limit(
    error_value: Option<&Value>,
) -> Result<Option<AiEvent>, StreamDecodeError> {
    let Some(Value::Object(error_obj)) = error_value else {
        return Ok(None);
    };
    let info = extract_codex_error_info(error_obj)?;
    if matches_rate_limit_code(info) != LimitCodeMatch::RateLimited {
        return Ok(None);
    }
    let resets_at = parse_reset_time(error_obj.get("resetsAt"), FieldName::ResetsAt)?;
    Ok(Some(AiEvent::RateLimited { resets_at }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LimitCodeMatch {
    RateLimited,
    Other,
}

fn matches_rate_limit_code(code: Option<&str>) -> LimitCodeMatch {
    match code {
        Some("usageLimitExceeded" | "rateLimitExceeded" | "usage_limit_reached") => {
            LimitCodeMatch::RateLimited
        }
        Some(_) | None => LimitCodeMatch::Other,
    }
}

fn extract_codex_error_info(
    error_obj: &Map<String, Value>,
) -> Result<Option<&str>, StreamDecodeError> {
    match error_obj.get("codexErrorInfo") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(code)) => Ok(Some(code.as_str())),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(FieldName::CodexErrorInfo)),
        ),
    }
}

fn handle_web_search_item(item: &Map<String, Value>) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let query = untrusted(required_str(item, "query", FieldName::Query)?)?;
    let action = parse_web_search_action(item.get("action"))?;
    Ok(vec![AiEvent::WebSearch { query, action }])
}

fn handle_token_usage(params: &Map<String, Value>) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let usage = match params.get("last").or_else(|| params.get("total")) {
        Some(Value::Object(obj)) => obj,
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_),
        ) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Usage,
            )));
        }
        None => {
            return Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Usage,
            )));
        }
    };
    Ok(vec![AiEvent::TokenUsage(parse_token_usage_camel(usage)?)])
}

fn handle_model_rerouted(params: &Map<String, Value>) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let raw_model = match params.get("toModel").or_else(|| params.get("model")) {
        Some(Value::String(text)) => text.as_str(),
        Some(
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_),
        ) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Model,
            )));
        }
        None => {
            return Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Model,
            )));
        }
    };
    let model = parse_model_name(raw_model)?;
    Ok(vec![
        AiEvent::ModelRerouted(model.clone()),
        AiEvent::ActualModel(model),
    ])
}

fn handle_error_notification(
    params: &Map<String, Value>,
    raw_line: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let info = extract_codex_error_info(params)?;
    if matches_rate_limit_code(info) == LimitCodeMatch::RateLimited {
        let resets_at = parse_reset_time(params.get("resetsAt"), FieldName::ResetsAt)?;
        return Ok(vec![AiEvent::RateLimited { resets_at }]);
    }
    Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
}

fn handle_rate_limits_payload(
    payload: &Map<String, Value>,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let ordinary = match payload.get("ordinaryUsageAllowed") {
        None | Some(Value::Null) => OrdinaryUsage::Unknown,
        Some(Value::Bool(true)) => OrdinaryUsage::Allowed,
        Some(Value::Bool(false)) => OrdinaryUsage::Disallowed,
        Some(Value::Number(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::OrdinaryUsageAllowed,
            )));
        }
    };
    if ordinary == OrdinaryUsage::Unknown {
        return Ok(vec![AiEvent::Quota(AiUsage::Unknown)]);
    }
    let rate_limits = required_object(payload, "rateLimits", FieldName::RateLimits)?;
    let primary = rate_limits.get("primary").and_then(Value::as_object);
    let resets_at = match primary {
        Some(window) => parse_reset_time(window.get("resetsAt"), FieldName::ResetsAt)?,
        None => ResetTime::NotProvided,
    };
    let limit_reached = rate_limits
        .get("rateLimitReachedType")
        .is_some_and(|value| !value.is_null());
    if ordinary == OrdinaryUsage::Disallowed || limit_reached {
        return Ok(vec![
            AiEvent::Quota(AiUsage::Waiting(resets_at)),
            AiEvent::RateLimited { resets_at },
        ]);
    }
    let Some(window) = primary else {
        return Ok(vec![AiEvent::Quota(AiUsage::Unknown)]);
    };
    let Some(used_percent) = window.get("usedPercent") else {
        return Ok(vec![AiEvent::Quota(AiUsage::Unknown)]);
    };
    if used_percent.is_null() {
        return Ok(vec![AiEvent::Quota(AiUsage::Unknown)]);
    }
    let utilization = parse_used_percent(used_percent, FieldName::UsedPercent)?;
    Ok(vec![AiEvent::Quota(AiUsage::Available(utilization))])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrdinaryUsage {
    Allowed,
    Disallowed,
    Unknown,
}

pub fn decode_app_server_lines<'lines, I>(
    expectation: CodexIsolationExpectation,
    lines: I,
) -> Result<Vec<AiEvent>, StreamDecodeError>
where
    I: IntoIterator<Item = &'lines str>,
{
    let mut decoder = CodexAppServerDecoder::new(expectation);
    let mut collected = Vec::new();
    for line in lines {
        collected.extend(decoder.decode_line(line)?);
    }
    decoder.finish()?;
    Ok(collected)
}

pub fn decode_app_server_text(
    expectation: CodexIsolationExpectation,
    text: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let lines = split_ndjson_lines(text)?;
    decode_app_server_lines(expectation, lines)
}

pub(crate) fn split_ndjson_lines(text: &str) -> Result<Vec<&str>, StreamDecodeError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let without_trailing_newline = text.strip_suffix('\n').unwrap_or(text);
    let lines: Vec<&str> = without_trailing_newline.split('\n').collect();
    if lines.iter().any(|entry| entry.trim().is_empty()) {
        return Err(StreamDecodeError::EmptyLine);
    }
    Ok(lines)
}
