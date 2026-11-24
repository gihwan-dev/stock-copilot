use crate::error::{
    FieldName, SchemaFailure, StreamDecodeError, StreamOrderError, ValidationFailure,
};
use crate::isolation::{
    ActiveCodexRecheck, CodexIsolationExpectation, McpStatusListSnapshot, ThreadStartSnapshot,
    evaluate_active_codex_recheck, evaluate_codex_isolation, evaluate_thread_start,
    parse_mcp_status_list, parse_thread_start_snapshot,
};
use crate::parse::{
    parse_model_name, parse_reset_time, parse_token_usage_camel, parse_tool_status,
    parse_used_percent, parse_web_search_action, render_mcp_tool_result, required_object,
    required_str, serialize_untrusted, untrusted,
};
use application::{
    AiEvent, AiUsage, IsolationCheck, IsolationFailure, ModelName, ResetTime, RunKind, ToolStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerExpectation {
    Expected,
    Unexpected,
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
    last_actual_model: Option<ModelName>,
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
            last_actual_model: None,
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
            return self.handle_thread_start_result(result);
        }
        if self.lifecycle == Lifecycle::AwaitingIsolation && result.contains_key("data") {
            if self.mcp_snapshot.is_some() {
                return Err(StreamDecodeError::Order(
                    StreamOrderError::DuplicateIsolationSnapshot,
                ));
            }
            let snapshot = parse_mcp_status_list(result)?;
            self.mcp_snapshot = Some(snapshot);
            return Ok(self.try_complete_isolation());
        }
        if self.lifecycle == Lifecycle::Active && is_mcp_status_list_result(result) {
            let snapshot = parse_mcp_status_list(result)?;
            return self.recheck_active_mcp_list(snapshot);
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

    fn handle_thread_start_result(
        &mut self,
        result: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let snapshot = parse_thread_start_snapshot(result)?;
        match self.lifecycle {
            Lifecycle::AwaitingIsolation => {
                if self.thread_snapshot.is_some() {
                    return Err(StreamDecodeError::Order(
                        StreamOrderError::DuplicateIsolationSnapshot,
                    ));
                }
                if let Some(raw_model) = &snapshot.model {
                    self.configured_model = Some(parse_model_name(raw_model)?);
                }
                self.thread_snapshot = Some(snapshot);
                Ok(self.try_complete_isolation())
            }
            Lifecycle::Active => match evaluate_thread_start(&snapshot, &self.expectation) {
                IsolationCheck::Passed => {
                    if let Some(raw_model) = &snapshot.model {
                        self.configured_model = Some(parse_model_name(raw_model)?);
                    }
                    self.thread_snapshot = Some(snapshot);
                    Ok(Vec::new())
                }
                IsolationCheck::Rejected(failure) => {
                    self.lifecycle = Lifecycle::TerminatedAfterRejection;
                    Err(StreamDecodeError::Order(
                        StreamOrderError::IsolationRegressed(failure),
                    ))
                }
            },
            Lifecycle::TerminatedAfterRejection => Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            )),
        }
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

    fn recheck_active_mcp_list(
        &mut self,
        mcp: McpStatusListSnapshot,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let Some(thread) = &self.thread_snapshot else {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        };
        match evaluate_active_codex_recheck(thread, &mcp, &self.expectation) {
            ActiveCodexRecheck::Passed => {
                self.mcp_snapshot = Some(mcp);
                Ok(Vec::new())
            }
            ActiveCodexRecheck::ExpectedMcpDown => {
                self.mcp_snapshot = Some(mcp);
                Ok(vec![AiEvent::McpUnavailable])
            }
            ActiveCodexRecheck::Regressed(failure) => {
                self.lifecycle = Lifecycle::TerminatedAfterRejection;
                Err(StreamDecodeError::Order(
                    StreamOrderError::IsolationRegressed(failure),
                ))
            }
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
            "model/rerouted" => self.handle_model_rerouted(params),
            "turn/completed" => self.handle_turn_completed(params, raw_line),
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
        let server_name = optional_str(params, "name", FieldName::Name)?;
        if self.lifecycle == Lifecycle::TerminatedAfterRejection {
            return Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            ));
        }
        if self.lifecycle == Lifecycle::Active
            && self.check_expected_server(server_name) == ServerExpectation::Unexpected
        {
            self.lifecycle = Lifecycle::TerminatedAfterRejection;
            return Err(StreamDecodeError::Order(
                StreamOrderError::IsolationRegressed(IsolationFailure::McpServers),
            ));
        }
        if status == "failed" || status == "error" {
            return self.handle_mcp_startup_failure();
        }
        match self.lifecycle {
            Lifecycle::AwaitingIsolation => match status {
                "starting" | "connecting" | "ready" | "connected" => Ok(Vec::new()),
                _ => Err(StreamDecodeError::Validation(
                    ValidationFailure::InvalidToolStatus,
                )),
            },
            Lifecycle::Active => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
            Lifecycle::TerminatedAfterRejection => Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            )),
        }
    }

    fn check_expected_server(&self, server_name: Option<&str>) -> ServerExpectation {
        if self.expectation.run_kind() == RunKind::Collection {
            return ServerExpectation::Unexpected;
        }
        match server_name {
            Some(name) if self.expectation.expected_mcp_server() != Some(name) => {
                ServerExpectation::Unexpected
            }
            Some(_) | None => ServerExpectation::Expected,
        }
    }

    fn handle_mcp_startup_failure(&mut self) -> Result<Vec<AiEvent>, StreamDecodeError> {
        match self.lifecycle {
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
        let raw_server = required_str(item, "server", FieldName::Server)?;
        let raw_tool = required_str(item, "tool", FieldName::Tool)?;
        let tool_allowed = self.expectation.run_kind() == RunKind::Analysis
            && self.expectation.expected_mcp_server() == Some(raw_server)
            && self.expectation.expected_tools().contains(raw_tool);
        if !tool_allowed {
            self.lifecycle = Lifecycle::TerminatedAfterRejection;
            return Err(StreamDecodeError::Order(
                StreamOrderError::IsolationRegressed(IsolationFailure::Tools),
            ));
        }
        let server = untrusted(raw_server)?;
        let tool = untrusted(raw_tool)?;
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
                let emitted = if self.turn_delta_state == TurnDeltaState::NoneYet {
                    vec![AiEvent::TextDelta(untrusted(text)?)]
                } else {
                    Vec::new()
                };
                self.turn_delta_state = TurnDeltaState::NoneYet;
                Ok(emitted)
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

    fn handle_model_rerouted(
        &mut self,
        params: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let raw_model = match params.get("toModel").or_else(|| params.get("model")) {
            Some(Value::String(text)) => text.as_str(),
            Some(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_),
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
        let mut events = Vec::new();
        if self.last_actual_model.as_ref() != Some(&model) {
            events.push(AiEvent::ModelRerouted(model.clone()));
        }
        self.last_actual_model = Some(model.clone());
        events.push(AiEvent::ActualModel(model));
        Ok(events)
    }

    fn record_turn_model(&mut self, actual: ModelName, events: &mut Vec<AiEvent>) {
        if let Some(configured) = &self.configured_model
            && configured != &actual
            && self.last_actual_model.as_ref() != Some(&actual)
        {
            events.push(AiEvent::ModelRerouted(actual.clone()));
        }
        self.last_actual_model = Some(actual.clone());
        events.push(AiEvent::ActualModel(actual));
    }

    fn drain_failed_pending_tools(
        &mut self,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        while let Some((_id, tool)) = self.pending_tools.pop_first() {
            events.push(AiEvent::ToolCall {
                server: tool.server,
                tool: tool.tool,
                args: tool.args,
                result: untrusted("turn_failed")?,
                status: ToolStatus::Failed,
            });
        }
        Ok(())
    }

    fn handle_turn_completed(
        &mut self,
        params: &Map<String, Value>,
        raw_line: &str,
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
        if status == "failed" {
            self.drain_failed_pending_tools(&mut events)?;
        }
        if let Some(model_value) = params.get("model")
            && !model_value.is_null()
        {
            let raw_model = model_value.as_str().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Model),
            ))?;
            let actual = parse_model_name(raw_model)?;
            self.record_turn_model(actual, &mut events);
        }
        if status == "failed" {
            match extract_turn_failure_rate_limit(params.get("error"))? {
                Some(event) => events.push(event),
                None => events.push(AiEvent::Unknown(untrusted(raw_line)?)),
            }
        } else if status != "completed" {
            events.push(AiEvent::Unknown(untrusted(raw_line)?));
        }
        self.turn_delta_state = TurnDeltaState::NoneYet;
        Ok(events)
    }
}

fn is_mcp_status_list_result(result: &Map<String, Value>) -> bool {
    let Some(data) = result.get("data") else {
        return false;
    };
    let Some(items) = data.as_array() else {
        return true;
    };
    if items.is_empty() {
        return true;
    }
    items.iter().any(|entry| match entry.as_object() {
        None => true,
        Some(obj) => {
            obj.contains_key("name")
                || obj.contains_key("runtimeStatus")
                || obj.contains_key("status")
                || obj.contains_key("tools")
        }
    })
}

fn optional_str<'value>(
    object: &'value Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<Option<&'value str>, StreamDecodeError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(field)),
        ),
    }
}

fn optional_object<'value>(
    object: &'value Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<Option<&'value Map<String, Value>>, StreamDecodeError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(field)),
        ),
    }
}

fn extract_turn_failure_rate_limit(
    error_value: Option<&Value>,
) -> Result<Option<AiEvent>, StreamDecodeError> {
    let error_obj = match error_value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(obj)) => obj,
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Error,
            )));
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LimitReached {
    Yes,
    No,
}

fn parse_limit_reached(
    rate_limits: &Map<String, Value>,
) -> Result<LimitReached, StreamDecodeError> {
    match rate_limits.get("rateLimitReachedType") {
        None | Some(Value::Null) => Ok(LimitReached::No),
        Some(Value::String(text)) if !text.is_empty() => Ok(LimitReached::Yes),
        Some(
            Value::String(_)
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Array(_)
            | Value::Object(_),
        ) => Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            FieldName::RateLimitReachedType,
        ))),
    }
}

fn extract_window_utilization(
    window: Option<&Map<String, Value>>,
) -> Result<Option<application::Utilization>, StreamDecodeError> {
    let Some(map) = window else {
        return Ok(None);
    };
    let Some(used_percent) = map.get("usedPercent") else {
        return Ok(None);
    };
    if used_percent.is_null() {
        return Ok(None);
    }
    parse_used_percent(used_percent, FieldName::UsedPercent).map(Some)
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
    let primary = optional_object(rate_limits, "primary", FieldName::RateLimits)?;
    let secondary = optional_object(rate_limits, "secondary", FieldName::RateLimits)?;
    let primary_reset = match primary {
        Some(window) => parse_reset_time(window.get("resetsAt"), FieldName::ResetsAt)?,
        None => ResetTime::NotProvided,
    };
    let secondary_reset = match secondary {
        Some(window) => parse_reset_time(window.get("resetsAt"), FieldName::ResetsAt)?,
        None => ResetTime::NotProvided,
    };
    let resets_at = match primary_reset {
        ResetTime::Known(ts) => ResetTime::Known(ts),
        ResetTime::NotProvided => secondary_reset,
    };
    let limit_reached = parse_limit_reached(rate_limits)?;
    if ordinary == OrdinaryUsage::Disallowed || limit_reached == LimitReached::Yes {
        return Ok(vec![
            AiEvent::Quota(AiUsage::Waiting(resets_at)),
            AiEvent::RateLimited { resets_at },
        ]);
    }
    let primary_util = extract_window_utilization(primary)?;
    let secondary_util = extract_window_utilization(secondary)?;
    let max_util = match (primary_util, secondary_util) {
        (Some(left), Some(right)) => {
            if right.as_probability() > left.as_probability() {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    };
    match max_util {
        Some(utilization) => Ok(vec![AiEvent::Quota(AiUsage::Available(utilization))]),
        None => Ok(vec![AiEvent::Quota(AiUsage::Unknown)]),
    }
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
