use crate::error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError};
use crate::isolation::{IsolationExpectation, evaluate_init_isolation, parse_init_snapshot};
use crate::parse::{
    ParsedToolName, parse_model_name, parse_reset_time, parse_token_usage, parse_tool_name,
    parse_utilization, parse_web_search_fields, render_tool_result_content, required_object,
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
    AwaitingInit,
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
struct PendingWebSearch {
    query: UntrustedText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeEventDecoder {
    expectation: IsolationExpectation,
    lifecycle: Lifecycle,
    configured_model: Option<ModelName>,
    last_actual_model: Option<ModelName>,
    turn_delta_state: TurnDeltaState,
    pending_tools: BTreeMap<String, PendingTool>,
    pending_web_searches: BTreeMap<String, PendingWebSearch>,
}

impl ClaudeEventDecoder {
    #[must_use]
    pub fn new(expectation: IsolationExpectation) -> Self {
        Self {
            expectation,
            lifecycle: Lifecycle::AwaitingInit,
            configured_model: None,
            last_actual_model: None,
            turn_delta_state: TurnDeltaState::NoneYet,
            pending_tools: BTreeMap::new(),
            pending_web_searches: BTreeMap::new(),
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
        let kind = required_str(object, "type", FieldName::Type)?;
        if kind.is_empty() {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Type,
            )));
        }
        self.dispatch_object(kind, object, trimmed)
    }

    pub fn finish(self) -> Result<(), StreamDecodeError> {
        match self.lifecycle {
            Lifecycle::AwaitingInit => Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            )),
            Lifecycle::TerminatedAfterRejection => Ok(()),
            Lifecycle::Active => {
                if self.pending_tools.is_empty() && self.pending_web_searches.is_empty() {
                    Ok(())
                } else {
                    Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall))
                }
            }
        }
    }

    fn dispatch_object(
        &mut self,
        kind: &str,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        if kind == "system" {
            return self.handle_system(object, raw_line);
        }
        if self.lifecycle == Lifecycle::AwaitingInit {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        }
        match kind {
            "stream_event" => self.handle_stream_event(object, raw_line),
            "assistant" => self.handle_assistant(object, raw_line),
            "user" => self.handle_user(object),
            "rate_limit_event" => handle_rate_limit(object, raw_line),
            "result" => self.handle_result(object),
            _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
        }
    }

    fn handle_system(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let subtype = required_str(object, "subtype", FieldName::Subtype)?;
        if subtype == "init" {
            return self.handle_init(object);
        }
        if self.lifecycle == Lifecycle::AwaitingInit {
            return Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            ));
        }
        Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
    }

    fn handle_init(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let snapshot = parse_init_snapshot(object)?;
        let check = evaluate_init_isolation(&snapshot, &self.expectation);
        match self.lifecycle {
            Lifecycle::AwaitingInit => self.apply_initial_init(check, snapshot.model.as_deref()),
            Lifecycle::Active => match check {
                IsolationCheck::Passed => {
                    self.turn_delta_state = TurnDeltaState::NoneYet;
                    Ok(Vec::new())
                }
                IsolationCheck::Rejected(IsolationFailure::McpServers) => {
                    Ok(vec![AiEvent::McpUnavailable])
                }
                IsolationCheck::Rejected(failure) => Err(StreamDecodeError::Order(
                    StreamOrderError::IsolationRegressed(failure),
                )),
            },
            Lifecycle::TerminatedAfterRejection => Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection,
            )),
        }
    }

    fn apply_initial_init(
        &mut self,
        check: IsolationCheck,
        raw_model: Option<&str>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        match check {
            IsolationCheck::Passed => {
                self.lifecycle = Lifecycle::Active;
                if let Some(model) = raw_model {
                    self.configured_model = Some(parse_model_name(model)?);
                }
            }
            IsolationCheck::Rejected(_) => {
                self.lifecycle = Lifecycle::TerminatedAfterRejection;
            }
        }
        Ok(vec![AiEvent::IsolationChecked(check)])
    }

    fn handle_stream_event(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let event = required_object(object, "event", FieldName::Event)?;
        let event_type = required_str(event, "type", FieldName::EventType)?;
        if event_type == "content_block_delta" {
            let delta = required_object(event, "delta", FieldName::Delta)?;
            let delta_type = required_str(delta, "type", FieldName::DeltaType)?;
            if delta_type == "text_delta" {
                let text = required_str(delta, "text", FieldName::Text)?;
                self.turn_delta_state = TurnDeltaState::Streamed;
                return Ok(vec![AiEvent::TextDelta(untrusted(text)?)]);
            }
        }
        Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
    }

    fn handle_assistant(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let message = required_object(object, "message", FieldName::Message)?;
        let mut events = Vec::new();
        self.collect_assistant_model(message, object, raw_line, &mut events)?;
        let content = message
            .get("content")
            .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Content,
            )))?
            .as_array()
            .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Content,
            )))?;
        for block in content {
            self.handle_assistant_block(block, &mut events)?;
        }
        self.turn_delta_state = TurnDeltaState::NoneYet;
        Ok(events)
    }

    fn collect_assistant_model(
        &mut self,
        message: &Map<String, Value>,
        outer: &Map<String, Value>,
        raw_line: &str,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        let Some(model_value) = message.get("model") else {
            return Ok(());
        };
        if model_value.is_null() {
            return Ok(());
        }
        let raw_model = model_value.as_str().ok_or(StreamDecodeError::Schema(
            SchemaFailure::InvalidFieldType(FieldName::Model),
        ))?;
        if raw_model == "<synthetic>" {
            let error_kind = outer
                .get("error")
                .or_else(|| message.get("error"))
                .and_then(Value::as_str);
            if error_kind == Some("rate_limit") {
                events.push(AiEvent::RateLimited {
                    resets_at: ResetTime::NotProvided,
                });
            } else {
                events.push(AiEvent::Unknown(untrusted(raw_line)?));
            }
            return Ok(());
        }
        let actual = parse_model_name(raw_model)?;
        self.record_actual_model(actual, events);
        Ok(())
    }

    fn record_actual_model(&mut self, actual: ModelName, events: &mut Vec<AiEvent>) {
        if let Some(configured) = &self.configured_model
            && configured != &actual
            && self.last_actual_model.as_ref() != Some(&actual)
        {
            events.push(AiEvent::ModelRerouted(actual.clone()));
        }
        self.last_actual_model = Some(actual.clone());
        events.push(AiEvent::ActualModel(actual));
    }

    fn handle_assistant_block(
        &mut self,
        block: &Value,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        let item =
            block
                .as_object()
                .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::Content,
                )))?;
        let block_type = required_str(item, "type", FieldName::BlockType)?;
        match block_type {
            "text" => {
                let text = required_str(item, "text", FieldName::Text)?;
                if self.turn_delta_state == TurnDeltaState::NoneYet {
                    events.push(AiEvent::TextDelta(untrusted(text)?));
                }
                Ok(())
            }
            "tool_use" => self.handle_tool_use(item, events),
            _ => {
                events.push(AiEvent::Unknown(serialize_untrusted(block)?));
                Ok(())
            }
        }
    }

    fn handle_tool_use(
        &mut self,
        item: &Map<String, Value>,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        let id = required_str(item, "id", FieldName::Id)?;
        if id.is_empty() {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Id,
            )));
        }
        if self.pending_tools.contains_key(id) || self.pending_web_searches.contains_key(id) {
            return Err(StreamDecodeError::Order(
                StreamOrderError::DuplicateToolCallId,
            ));
        }
        let name = required_str(item, "name", FieldName::Name)?;
        let input =
            item.get("input")
                .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
                    FieldName::Input,
                )))?;
        let event = match parse_tool_name(name)? {
            ParsedToolName::WebSearch => {
                let (query, action) = parse_web_search_fields(input)?;
                self.pending_web_searches.insert(
                    String::from(id),
                    PendingWebSearch {
                        query: query.clone(),
                    },
                );
                AiEvent::WebSearch { query, action }
            }
            ParsedToolName::Mcp { server, tool } => {
                self.register_pending_tool(id, &server, &tool, input)?
            }
            ParsedToolName::Builtin { tool } => {
                self.register_pending_tool(id, "builtin", &tool, input)?
            }
        };
        events.push(event);
        Ok(())
    }

    fn register_pending_tool(
        &mut self,
        id: &str,
        server_name: &str,
        tool_name: &str,
        input: &Value,
    ) -> Result<AiEvent, StreamDecodeError> {
        let server = untrusted(server_name)?;
        let tool = untrusted(tool_name)?;
        let args = serialize_untrusted(input)?;
        let result = untrusted("")?;
        self.pending_tools.insert(
            String::from(id),
            PendingTool {
                server: server.clone(),
                tool: tool.clone(),
                args: args.clone(),
            },
        );
        Ok(AiEvent::ToolCall {
            server,
            tool,
            args,
            result,
            status: ToolStatus::Running,
        })
    }

    fn handle_user(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let message = required_object(object, "message", FieldName::Message)?;
        let content = message
            .get("content")
            .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Content,
            )))?
            .as_array()
            .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Content,
            )))?;
        let mut events = Vec::new();
        for block in content {
            self.handle_user_block(block, &mut events)?;
        }
        Ok(events)
    }

    fn handle_user_block(
        &mut self,
        block: &Value,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        let item =
            block
                .as_object()
                .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::Content,
                )))?;
        let block_type = required_str(item, "type", FieldName::BlockType)?;
        if block_type != "tool_result" {
            events.push(AiEvent::Unknown(serialize_untrusted(block)?));
            return Ok(());
        }
        let tool_use_id = required_str(item, "tool_use_id", FieldName::ToolUseId)?;
        let status = match item.get("is_error") {
            None | Some(Value::Null | Value::Bool(false)) => ToolStatus::Succeeded,
            Some(Value::Bool(true)) => ToolStatus::Failed,
            Some(Value::Number(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => {
                return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::IsError,
                )));
            }
        };
        let result = render_tool_result_content(item.get("content"))?;
        if let Some(pending) = self.pending_tools.remove(tool_use_id) {
            events.push(AiEvent::ToolCall {
                server: pending.server,
                tool: pending.tool,
                args: pending.args,
                result,
                status,
            });
            return Ok(());
        }
        if let Some(pending_search) = self.pending_web_searches.remove(tool_use_id) {
            let action = match status {
                ToolStatus::Succeeded | ToolStatus::Running => untrusted("completed")?,
                ToolStatus::Failed => untrusted("failed")?,
            };
            events.push(AiEvent::WebSearch {
                query: pending_search.query,
                action,
            });
            return Ok(());
        }
        Err(StreamDecodeError::Order(
            StreamOrderError::UnexpectedToolResult,
        ))
    }

    fn handle_result(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let _subtype = required_str(object, "subtype", FieldName::Subtype)?;
        let is_error = match object.get("is_error") {
            None | Some(Value::Null | Value::Bool(false)) => ResultOutcome::Ok,
            Some(Value::Bool(true)) => ResultOutcome::Error,
            Some(Value::Number(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => {
                return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::IsError,
                )));
            }
        };
        if is_error == ResultOutcome::Ok
            && (!self.pending_tools.is_empty() || !self.pending_web_searches.is_empty())
        {
            return Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall));
        }
        let mut events = Vec::new();
        collect_result_usage(object, &mut events)?;
        self.collect_result_models(object, &mut events)?;
        collect_result_rate_limit(object, is_error, &mut events)?;
        self.turn_delta_state = TurnDeltaState::NoneYet;
        if is_error == ResultOutcome::Error {
            self.pending_tools.clear();
            self.pending_web_searches.clear();
        }
        Ok(events)
    }

    fn collect_result_models(
        &mut self,
        object: &Map<String, Value>,
        events: &mut Vec<AiEvent>,
    ) -> Result<(), StreamDecodeError> {
        let Some(model_usage_value) = object.get("modelUsage") else {
            return Ok(());
        };
        if model_usage_value.is_null() {
            return Ok(());
        }
        let model_usage = model_usage_value
            .as_object()
            .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::ModelUsage,
            )))?;
        for key in model_usage.keys() {
            if key == "<synthetic>" {
                continue;
            }
            let model = parse_model_name(key)?;
            if self.last_actual_model.as_ref() != Some(&model) {
                self.record_actual_model(model, events);
            }
        }
        Ok(())
    }
}

fn collect_result_usage(
    object: &Map<String, Value>,
    events: &mut Vec<AiEvent>,
) -> Result<(), StreamDecodeError> {
    let Some(usage_value) = object.get("usage") else {
        return Ok(());
    };
    if usage_value.is_null() {
        return Ok(());
    }
    let usage = usage_value.as_object().ok_or(StreamDecodeError::Schema(
        SchemaFailure::InvalidFieldType(FieldName::Usage),
    ))?;
    events.push(AiEvent::TokenUsage(parse_token_usage(usage)?));
    Ok(())
}

fn collect_result_rate_limit(
    object: &Map<String, Value>,
    outcome: ResultOutcome,
    events: &mut Vec<AiEvent>,
) -> Result<(), StreamDecodeError> {
    if outcome != ResultOutcome::Error {
        return Ok(());
    }
    let api_error = match object.get("api_error") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.as_str()),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::ApiError,
            )));
        }
    };
    if api_error == Some("usage_limit_reached") {
        let resets_at = parse_reset_time(object.get("resetsAt"), FieldName::ResetsAt)?;
        events.push(AiEvent::RateLimited { resets_at });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultOutcome {
    Ok,
    Error,
}

fn handle_rate_limit(
    object: &Map<String, Value>,
    raw_line: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let info = required_object(object, "rate_limit_info", FieldName::RateLimitInfo)?;
    let status = required_str(info, "status", FieldName::Status)?;
    let resets_at = parse_reset_time(info.get("resetsAt"), FieldName::ResetsAt)?;
    match status {
        "rejected" => Ok(vec![
            AiEvent::Quota(AiUsage::Waiting(resets_at)),
            AiEvent::RateLimited { resets_at },
        ]),
        "allowed" | "allowed_warning" => {
            let utilization = extract_rate_limit_utilization(info)?;
            let usage = match utilization {
                Some(parsed) => AiUsage::Available(parsed),
                None => AiUsage::Unknown,
            };
            Ok(vec![AiEvent::Quota(usage)])
        }
        _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
    }
}

fn extract_rate_limit_utilization(
    info: &Map<String, Value>,
) -> Result<Option<application::Utilization>, StreamDecodeError> {
    if let Some(direct) = info.get("utilization")
        && !direct.is_null()
    {
        return parse_utilization(direct, FieldName::Utilization).map(Some);
    }
    let Some(Value::Object(windows)) = info.get("unifiedWindows") else {
        return Ok(None);
    };
    let mut best: Option<application::Utilization> = None;
    for entry in windows.values() {
        let Some(window) = entry.as_object() else {
            continue;
        };
        let Some(value) = window.get("utilization") else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let parsed = parse_utilization(value, FieldName::Utilization)?;
        best = match best {
            None => Some(parsed),
            Some(current) if parsed.as_probability() > current.as_probability() => Some(parsed),
            Some(current) => Some(current),
        };
    }
    Ok(best)
}

pub fn decode_stream_lines<'lines, I>(
    expectation: IsolationExpectation,
    lines: I,
) -> Result<Vec<AiEvent>, StreamDecodeError>
where
    I: IntoIterator<Item = &'lines str>,
{
    let mut decoder = ClaudeEventDecoder::new(expectation);
    let mut collected = Vec::new();
    for line in lines {
        collected.extend(decoder.decode_line(line)?);
    }
    decoder.finish()?;
    Ok(collected)
}

pub fn decode_stream_text(
    expectation: IsolationExpectation,
    text: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let lines = split_ndjson_lines(text)?;
    decode_stream_lines(expectation, lines)
}

fn split_ndjson_lines(text: &str) -> Result<Vec<&str>, StreamDecodeError> {
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
