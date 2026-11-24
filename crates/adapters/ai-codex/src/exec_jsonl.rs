use crate::app_server::split_ndjson_lines;
use crate::error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError};
use crate::parse::{
    parse_model_name, parse_reset_time, parse_token_usage_snake, parse_tool_status,
    parse_web_search_action, render_mcp_tool_result, required_object, required_str,
    serialize_untrusted, untrusted,
};
use application::{AiEvent, IsolationCheck, ToolStatus};
use domain::UntrustedText;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmissionState {
    PendingInitial,
    Active,
    TerminatedAfterRejection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingTool {
    server: UntrustedText,
    tool: UntrustedText,
    args: UntrustedText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexExecDecoder {
    initial_isolation: IsolationCheck,
    state: EmissionState,
    pending_tools: BTreeMap<String, PendingTool>,
}

impl CodexExecDecoder {
    #[must_use]
    pub fn new(initial_isolation: IsolationCheck) -> Self {
        Self {
            initial_isolation,
            state: EmissionState::PendingInitial,
            pending_tools: BTreeMap::new(),
        }
    }

    pub fn take_initial_event(&mut self) -> Option<AiEvent> {
        if self.state != EmissionState::PendingInitial {
            return None;
        }
        self.state = match self.initial_isolation {
            IsolationCheck::Passed => EmissionState::Active,
            IsolationCheck::Rejected(_) => EmissionState::TerminatedAfterRejection,
        };
        Some(AiEvent::IsolationChecked(self.initial_isolation))
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
        let mut prefix = Vec::new();
        if self.state == EmissionState::PendingInitial {
            let first = self.take_initial_event();
            if self.state == EmissionState::TerminatedAfterRejection {
                return Ok(first.into_iter().collect());
            }
            if let Some(event) = first {
                prefix.push(event);
            }
        } else if self.state == EmissionState::TerminatedAfterRejection {
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
        prefix.extend(self.dispatch_exec(kind, object, trimmed)?);
        Ok(prefix)
    }

    pub fn finish(self) -> Result<(), StreamDecodeError> {
        match self.state {
            EmissionState::PendingInitial => Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck,
            )),
            EmissionState::TerminatedAfterRejection => Ok(()),
            EmissionState::Active => {
                if self.pending_tools.is_empty() {
                    Ok(())
                } else {
                    Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall))
                }
            }
        }
    }

    fn dispatch_exec(
        &mut self,
        kind: &str,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        match kind {
            "item.started" => self.handle_item_started(object, raw_line),
            "item.completed" => self.handle_item_completed(object, raw_line),
            "turn.completed" => self.handle_turn_completed(object),
            "turn.failed" => self.handle_turn_failed(object, raw_line),
            "error" => handle_exec_error(object, raw_line),
            _ => Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)]),
        }
    }

    fn handle_item_started(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let item = required_object(object, "item", FieldName::Item)?;
        let item_type = required_str(item, "type", FieldName::ItemType)?;
        match item_type {
            "mcp_tool_call" => self.start_mcp_tool(item),
            "web_search" => handle_web_search(item),
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
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let item = required_object(object, "item", FieldName::Item)?;
        let item_type = required_str(item, "type", FieldName::ItemType)?;
        match item_type {
            "mcp_tool_call" => self.complete_mcp_tool(item),
            "web_search" => handle_web_search(item),
            "agent_message" => {
                let text = required_str(item, "text", FieldName::Text)?;
                Ok(vec![AiEvent::TextDelta(untrusted(text)?)])
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
        &self,
        object: &Map<String, Value>,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        if !self.pending_tools.is_empty() {
            return Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall));
        }
        let mut events = Vec::new();
        if let Some(usage_value) = object.get("usage")
            && !usage_value.is_null()
        {
            let usage = usage_value.as_object().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Usage),
            ))?;
            events.push(AiEvent::TokenUsage(parse_token_usage_snake(usage)?));
        }
        if let Some(model_value) = object.get("model")
            && !model_value.is_null()
        {
            let raw_model = model_value.as_str().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Model),
            ))?;
            events.push(AiEvent::ActualModel(parse_model_name(raw_model)?));
        }
        Ok(events)
    }

    fn handle_turn_failed(
        &mut self,
        object: &Map<String, Value>,
        raw_line: &str,
    ) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let error_obj = required_object(object, "error", FieldName::Error)?;
        let mut events = self.drain_failed_pending_tools()?;
        events.extend(handle_exec_error(error_obj, raw_line)?);
        Ok(events)
    }

    fn drain_failed_pending_tools(&mut self) -> Result<Vec<AiEvent>, StreamDecodeError> {
        let mut events = Vec::new();
        while let Some((_id, tool)) = self.pending_tools.pop_first() {
            events.push(AiEvent::ToolCall {
                server: tool.server,
                tool: tool.tool,
                args: tool.args,
                result: untrusted("turn_failed")?,
                status: ToolStatus::Failed,
            });
        }
        Ok(events)
    }
}

fn handle_web_search(item: &Map<String, Value>) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let query = untrusted(required_str(item, "query", FieldName::Query)?)?;
    let action = parse_web_search_action(item.get("action"))?;
    Ok(vec![AiEvent::WebSearch { query, action }])
}

fn handle_exec_error(
    object: &Map<String, Value>,
    raw_line: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let code = match object.get("code") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.as_str()),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::CodexErrorInfo,
            )));
        }
    };
    if matches_exec_limit_code(code) == ExecLimitMatch::RateLimited {
        let resets_at = parse_reset_time(object.get("resetsAt"), FieldName::ResetsAt)?;
        return Ok(vec![AiEvent::RateLimited { resets_at }]);
    }
    Ok(vec![AiEvent::Unknown(untrusted(raw_line)?)])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecLimitMatch {
    RateLimited,
    Other,
}

fn matches_exec_limit_code(code: Option<&str>) -> ExecLimitMatch {
    match code {
        Some("usage_limit_reached" | "usageLimitExceeded" | "rateLimitExceeded") => {
            ExecLimitMatch::RateLimited
        }
        Some(_) | None => ExecLimitMatch::Other,
    }
}

pub fn decode_exec_lines<'lines, I>(
    initial_isolation: IsolationCheck,
    lines: I,
) -> Result<Vec<AiEvent>, StreamDecodeError>
where
    I: IntoIterator<Item = &'lines str>,
{
    let mut decoder = CodexExecDecoder::new(initial_isolation);
    let mut collected = Vec::new();
    if let Some(first) = decoder.take_initial_event() {
        collected.push(first);
    }
    for line in lines {
        collected.extend(decoder.decode_line(line)?);
    }
    decoder.finish()?;
    Ok(collected)
}

pub fn decode_exec_text(
    initial_isolation: IsolationCheck,
    text: &str,
) -> Result<Vec<AiEvent>, StreamDecodeError> {
    let lines = split_ndjson_lines(text)?;
    decode_exec_lines(initial_isolation, lines)
}
