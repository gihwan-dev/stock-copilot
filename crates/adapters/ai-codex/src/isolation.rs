use crate::error::{FieldName, SchemaFailure, StreamDecodeError};
use application::{IsolationCheck, IsolationFailure, RunKind};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexIsolationExpectation {
    run_kind: RunKind,
    expected_tools: BTreeSet<String>,
    expected_mcp_server: Option<String>,
    allowed_cli_versions: Option<BTreeSet<String>>,
}

impl CodexIsolationExpectation {
    #[must_use]
    pub fn new(
        run_kind: RunKind,
        expected_tools: BTreeSet<String>,
        expected_mcp_server: Option<String>,
        allowed_cli_versions: Option<BTreeSet<String>>,
    ) -> Self {
        Self {
            run_kind,
            expected_tools,
            expected_mcp_server,
            allowed_cli_versions,
        }
    }

    #[must_use]
    pub const fn run_kind(&self) -> RunKind {
        self.run_kind
    }

    #[must_use]
    pub const fn expected_tools(&self) -> &BTreeSet<String> {
        &self.expected_tools
    }

    #[must_use]
    pub fn expected_mcp_server(&self) -> Option<&str> {
        self.expected_mcp_server.as_deref()
    }

    #[must_use]
    pub const fn allowed_cli_versions(&self) -> Option<&BTreeSet<String>> {
        self.allowed_cli_versions.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MultiAgentState {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkAccessState {
    Blocked,
    Allowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThreadStartSnapshot {
    pub(crate) sandbox_type: String,
    pub(crate) network_access: NetworkAccessState,
    pub(crate) instruction_sources: Vec<String>,
    pub(crate) multi_agent: MultiAgentState,
    pub(crate) cli_version: Option<String>,
    pub(crate) model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpServerEntry {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpStatusListSnapshot {
    pub(crate) servers: Vec<McpServerEntry>,
}

fn required_string(
    object: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<String, StreamDecodeError> {
    let value = object
        .get(key)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            field,
        )))?;
    value
        .as_str()
        .map(String::from)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            field,
        )))
}

fn parse_string_list(value: &Value, field: FieldName) -> Result<Vec<String>, StreamDecodeError> {
    let items =
        value
            .as_array()
            .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                field,
            )))?;
    items
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(String::from)
                .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    field,
                )))
        })
        .collect()
}

pub(crate) fn parse_thread_start_snapshot(
    result: &Map<String, Value>,
) -> Result<ThreadStartSnapshot, StreamDecodeError> {
    let sandbox = result
        .get("sandbox")
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            FieldName::Sandbox,
        )))?
        .as_object()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            FieldName::Sandbox,
        )))?;
    let sandbox_type = required_string(sandbox, "type", FieldName::Sandbox)?;
    let network_access = match sandbox.get("networkAccess") {
        None | Some(Value::Null | Value::Bool(false)) => NetworkAccessState::Blocked,
        Some(Value::Bool(true)) => NetworkAccessState::Allowed,
        Some(Value::Number(_) | Value::String(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Sandbox,
            )));
        }
    };
    let sources_value = result
        .get("instructionSources")
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            FieldName::InstructionSources,
        )))?;
    let instruction_sources = parse_string_list(sources_value, FieldName::InstructionSources)?;
    let multi_agent = match result.get("multiAgentMode") {
        None | Some(Value::Null | Value::Bool(false)) => MultiAgentState::Disabled,
        Some(Value::String(text)) if text == "disabled" || text == "off" => {
            MultiAgentState::Disabled
        }
        Some(Value::Bool(true) | Value::String(_)) => MultiAgentState::Enabled,
        Some(Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::MultiAgentMode,
            )));
        }
    };
    let cli_version = optional_string(result, "cliVersion", FieldName::CliVersion)?;
    let model = optional_string(result, "model", FieldName::Model)?;
    Ok(ThreadStartSnapshot {
        sandbox_type,
        network_access,
        instruction_sources,
        multi_agent,
        cli_version,
        model,
    })
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<Option<String>, StreamDecodeError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(field)),
        ),
    }
}

fn parse_mcp_tools(entry: &Map<String, Value>) -> Result<Vec<String>, StreamDecodeError> {
    let Some(tools_value) = entry.get("tools") else {
        return Ok(Vec::new());
    };
    match tools_value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                let name = match item {
                    Value::String(text) => Some(text.as_str()),
                    Value::Object(obj) => obj.get("name").and_then(Value::as_str),
                    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => None,
                };
                name.map(String::from).ok_or(StreamDecodeError::Schema(
                    SchemaFailure::InvalidFieldType(FieldName::Tools),
                ))
            })
            .collect(),
        Value::Object(map) => Ok(map.keys().cloned().collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(FieldName::Tools)),
        ),
    }
}

pub(crate) fn parse_mcp_status_list(
    result: &Map<String, Value>,
) -> Result<McpStatusListSnapshot, StreamDecodeError> {
    let data = result
        .get("data")
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            FieldName::Data,
        )))?
        .as_array()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            FieldName::Data,
        )))?;
    let mut servers = Vec::new();
    for entry in data {
        let item =
            entry
                .as_object()
                .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                    FieldName::Data,
                )))?;
        let name = required_string(item, "name", FieldName::Name)?;
        let runtime_status = optional_string(item, "runtimeStatus", FieldName::Status)?;
        let status = match runtime_status {
            Some(found) => found,
            None => match optional_string(item, "status", FieldName::Status)? {
                Some(found) => found,
                None => {
                    return Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                        FieldName::Status,
                    )));
                }
            },
        };
        let tools = parse_mcp_tools(item)?;
        servers.push(McpServerEntry {
            name,
            status,
            tools,
        });
    }
    Ok(McpStatusListSnapshot { servers })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchOutcome {
    Matched,
    Mismatched,
}

fn version_matches(
    snapshot: &ThreadStartSnapshot,
    expectation: &CodexIsolationExpectation,
) -> MatchOutcome {
    let Some(allowed) = &expectation.allowed_cli_versions else {
        return MatchOutcome::Matched;
    };
    match &snapshot.cli_version {
        Some(version) if !version.trim().is_empty() && allowed.contains(version) => {
            MatchOutcome::Matched
        }
        Some(_) | None => MatchOutcome::Mismatched,
    }
}

fn evaluate_thread_start(
    snapshot: &ThreadStartSnapshot,
    expectation: &CodexIsolationExpectation,
) -> IsolationCheck {
    if snapshot.sandbox_type != "readOnly" {
        return IsolationCheck::Rejected(IsolationFailure::PermissionMode);
    }
    if expectation.run_kind == RunKind::Analysis
        && snapshot.network_access == NetworkAccessState::Allowed
    {
        return IsolationCheck::Rejected(IsolationFailure::PermissionMode);
    }
    if !snapshot.instruction_sources.is_empty() {
        return IsolationCheck::Rejected(IsolationFailure::Skills);
    }
    if snapshot.multi_agent == MultiAgentState::Enabled {
        return IsolationCheck::Rejected(IsolationFailure::Plugins);
    }
    if version_matches(snapshot, expectation) == MatchOutcome::Mismatched {
        return IsolationCheck::Rejected(IsolationFailure::CliVersion);
    }
    IsolationCheck::Passed
}

fn evaluate_mcp_list(
    mcp: &McpStatusListSnapshot,
    expectation: &CodexIsolationExpectation,
) -> IsolationCheck {
    match (expectation.run_kind, expectation.expected_mcp_server()) {
        (RunKind::Collection, _) => {
            if mcp.servers.is_empty() {
                IsolationCheck::Passed
            } else {
                IsolationCheck::Rejected(IsolationFailure::McpServers)
            }
        }
        (RunKind::Analysis, None) => IsolationCheck::Rejected(IsolationFailure::McpServers),
        (RunKind::Analysis, Some(expected_name)) => {
            if mcp.servers.len() != 1_usize {
                return IsolationCheck::Rejected(IsolationFailure::McpServers);
            }
            let Some(server) = mcp.servers.first() else {
                return IsolationCheck::Rejected(IsolationFailure::McpServers);
            };
            let healthy = server.status == "ready" || server.status == "connected";
            if server.name != expected_name || !healthy {
                return IsolationCheck::Rejected(IsolationFailure::McpServers);
            }
            let unique: BTreeSet<String> = server.tools.iter().cloned().collect();
            if unique.len() == server.tools.len() && unique == expectation.expected_tools {
                IsolationCheck::Passed
            } else {
                IsolationCheck::Rejected(IsolationFailure::Tools)
            }
        }
    }
}

pub(crate) fn evaluate_codex_isolation(
    thread: &ThreadStartSnapshot,
    mcp: &McpStatusListSnapshot,
    expectation: &CodexIsolationExpectation,
) -> IsolationCheck {
    match evaluate_thread_start(thread, expectation) {
        IsolationCheck::Rejected(failure) => IsolationCheck::Rejected(failure),
        IsolationCheck::Passed => evaluate_mcp_list(mcp, expectation),
    }
}
