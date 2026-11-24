use crate::error::{FieldName, SchemaFailure, StreamDecodeError};
use application::{IsolationCheck, IsolationFailure, RunKind};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationExpectation {
    run_kind: RunKind,
    expected_tools: BTreeSet<String>,
    expected_mcp_server: Option<String>,
    allowed_builtin_plugins: BTreeSet<String>,
    allowed_cli_versions: Option<BTreeSet<String>>,
}

impl IsolationExpectation {
    #[must_use]
    pub fn new(
        run_kind: RunKind,
        expected_tools: BTreeSet<String>,
        expected_mcp_server: Option<String>,
        allowed_builtin_plugins: BTreeSet<String>,
        allowed_cli_versions: Option<BTreeSet<String>>,
    ) -> Self {
        Self {
            run_kind,
            expected_tools,
            expected_mcp_server,
            allowed_builtin_plugins,
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
    pub const fn allowed_builtin_plugins(&self) -> &BTreeSet<String> {
        &self.allowed_builtin_plugins
    }

    #[must_use]
    pub const fn allowed_cli_versions(&self) -> Option<&BTreeSet<String>> {
        self.allowed_cli_versions.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitMcpServer {
    pub(crate) name: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitSnapshot {
    pub(crate) api_key_source: String,
    pub(crate) permission_mode: String,
    pub(crate) skills: Vec<String>,
    pub(crate) slash_commands: Vec<String>,
    pub(crate) plugins: Vec<String>,
    pub(crate) plugin_errors: usize,
    pub(crate) mcp_servers: Vec<InitMcpServer>,
    pub(crate) mcp_server_errors: usize,
    pub(crate) tools: Vec<String>,
    pub(crate) cli_version: String,
    pub(crate) model: Option<String>,
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

fn required_string_array(
    object: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<Vec<String>, StreamDecodeError> {
    let entries = object
        .get(key)
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            field,
        )))?
        .as_array()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            field,
        )))?;
    entries
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

fn parse_plugins(object: &Map<String, Value>) -> Result<Vec<String>, StreamDecodeError> {
    let entries = object
        .get("plugins")
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            FieldName::Plugins,
        )))?
        .as_array()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            FieldName::Plugins,
        )))?;
    entries
        .iter()
        .map(|entry| {
            let name = match entry {
                Value::String(text) => Some(text.as_str()),
                Value::Object(item) => item.get("name").and_then(Value::as_str),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => None,
            };
            name.map(String::from).ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::Plugins),
            ))
        })
        .collect()
}

fn parse_mcp_servers(object: &Map<String, Value>) -> Result<Vec<InitMcpServer>, StreamDecodeError> {
    let entries = object
        .get("mcp_servers")
        .ok_or(StreamDecodeError::Schema(SchemaFailure::MissingField(
            FieldName::McpServers,
        )))?
        .as_array()
        .ok_or(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
            FieldName::McpServers,
        )))?;
    entries
        .iter()
        .map(|entry| {
            let item = entry.as_object().ok_or(StreamDecodeError::Schema(
                SchemaFailure::InvalidFieldType(FieldName::McpServers),
            ))?;
            let name = required_string(item, "name", FieldName::McpServers)?;
            let status = required_string(item, "status", FieldName::McpServers)?;
            Ok(InitMcpServer { name, status })
        })
        .collect()
}

fn optional_array_len(
    object: &Map<String, Value>,
    key: &str,
    field: FieldName,
) -> Result<usize, StreamDecodeError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(0_usize),
        Some(Value::Array(items)) => Ok(items.len()),
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_)) => Err(
            StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(field)),
        ),
    }
}

pub(crate) fn parse_init_snapshot(
    object: &Map<String, Value>,
) -> Result<InitSnapshot, StreamDecodeError> {
    let model = match object.get("model") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(StreamDecodeError::Schema(SchemaFailure::InvalidFieldType(
                FieldName::Model,
            )));
        }
    };
    Ok(InitSnapshot {
        api_key_source: required_string(object, "apiKeySource", FieldName::ApiKeySource)?,
        permission_mode: required_string(object, "permissionMode", FieldName::PermissionMode)?,
        skills: required_string_array(object, "skills", FieldName::Skills)?,
        slash_commands: required_string_array(object, "slash_commands", FieldName::SlashCommands)?,
        plugins: parse_plugins(object)?,
        plugin_errors: optional_array_len(object, "plugin_errors", FieldName::Plugins)?,
        mcp_servers: parse_mcp_servers(object)?,
        mcp_server_errors: optional_array_len(object, "mcp_server_errors", FieldName::McpServers)?,
        tools: required_string_array(object, "tools", FieldName::Tools)?,
        cli_version: required_string(object, "claude_code_version", FieldName::ClaudeCodeVersion)?,
        model,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchOutcome {
    Matched,
    Mismatched,
}

fn mcp_servers_match(snapshot: &InitSnapshot, expectation: &IsolationExpectation) -> MatchOutcome {
    if snapshot.mcp_server_errors > 0_usize {
        return MatchOutcome::Mismatched;
    }
    let matched = match (expectation.run_kind, expectation.expected_mcp_server()) {
        (RunKind::Collection, _) => snapshot.mcp_servers.is_empty(),
        (RunKind::Analysis, None) => false,
        (RunKind::Analysis, Some(expected_name)) => {
            snapshot.mcp_servers.len() == 1_usize
                && snapshot.mcp_servers.first().is_some_and(|server| {
                    server.name == expected_name && server.status == "connected"
                })
        }
    };
    if matched {
        MatchOutcome::Matched
    } else {
        MatchOutcome::Mismatched
    }
}

fn tools_match(snapshot: &InitSnapshot, expectation: &IsolationExpectation) -> MatchOutcome {
    let unique: BTreeSet<String> = snapshot.tools.iter().cloned().collect();
    if unique.len() == snapshot.tools.len() && unique == expectation.expected_tools {
        MatchOutcome::Matched
    } else {
        MatchOutcome::Mismatched
    }
}

fn version_matches(snapshot: &InitSnapshot, expectation: &IsolationExpectation) -> MatchOutcome {
    if snapshot.cli_version.trim().is_empty() {
        return MatchOutcome::Mismatched;
    }
    let matched = match &expectation.allowed_cli_versions {
        None => true,
        Some(allowed) => allowed.contains(&snapshot.cli_version),
    };
    if matched {
        MatchOutcome::Matched
    } else {
        MatchOutcome::Mismatched
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveRecheckOutcome {
    Passed,
    ExpectedMcpDown,
    Regressed(IsolationFailure),
}

fn evaluate_non_mcp_non_tool_isolation(
    snapshot: &InitSnapshot,
    expectation: &IsolationExpectation,
) -> Option<IsolationFailure> {
    if snapshot.api_key_source != "none" {
        return Some(IsolationFailure::ApiKeySource);
    }
    if snapshot.permission_mode != "dontAsk" {
        return Some(IsolationFailure::PermissionMode);
    }
    if !snapshot.skills.is_empty() {
        return Some(IsolationFailure::Skills);
    }
    if !snapshot.slash_commands.is_empty() {
        return Some(IsolationFailure::SlashCommands);
    }
    if snapshot.plugin_errors > 0_usize
        || !snapshot
            .plugins
            .iter()
            .all(|plugin| expectation.allowed_builtin_plugins.contains(plugin))
    {
        return Some(IsolationFailure::Plugins);
    }
    if version_matches(snapshot, expectation) == MatchOutcome::Mismatched {
        return Some(IsolationFailure::CliVersion);
    }
    None
}

pub(crate) fn evaluate_init_isolation(
    snapshot: &InitSnapshot,
    expectation: &IsolationExpectation,
) -> IsolationCheck {
    if let Some(failure) = evaluate_non_mcp_non_tool_isolation(snapshot, expectation)
        && failure != IsolationFailure::CliVersion
    {
        return IsolationCheck::Rejected(failure);
    }
    if mcp_servers_match(snapshot, expectation) == MatchOutcome::Mismatched {
        return IsolationCheck::Rejected(IsolationFailure::McpServers);
    }
    if tools_match(snapshot, expectation) == MatchOutcome::Mismatched {
        return IsolationCheck::Rejected(IsolationFailure::Tools);
    }
    if version_matches(snapshot, expectation) == MatchOutcome::Mismatched {
        return IsolationCheck::Rejected(IsolationFailure::CliVersion);
    }
    IsolationCheck::Passed
}

pub(crate) fn evaluate_active_init_recheck(
    snapshot: &InitSnapshot,
    expectation: &IsolationExpectation,
) -> ActiveRecheckOutcome {
    if let Some(failure) = evaluate_non_mcp_non_tool_isolation(snapshot, expectation) {
        return ActiveRecheckOutcome::Regressed(failure);
    }
    let unique_tools: BTreeSet<String> = snapshot.tools.iter().cloned().collect();
    if unique_tools.len() != snapshot.tools.len()
        || !unique_tools.is_subset(&expectation.expected_tools)
    {
        return ActiveRecheckOutcome::Regressed(IsolationFailure::Tools);
    }
    match (expectation.run_kind, expectation.expected_mcp_server()) {
        (RunKind::Collection, _) => {
            if snapshot.mcp_server_errors > 0_usize || !snapshot.mcp_servers.is_empty() {
                return ActiveRecheckOutcome::Regressed(IsolationFailure::McpServers);
            }
            if unique_tools == expectation.expected_tools {
                ActiveRecheckOutcome::Passed
            } else {
                ActiveRecheckOutcome::Regressed(IsolationFailure::Tools)
            }
        }
        (RunKind::Analysis, None) => ActiveRecheckOutcome::Regressed(IsolationFailure::McpServers),
        (RunKind::Analysis, Some(expected_name)) => {
            let has_unexpected_server = snapshot.mcp_servers.len() > 1_usize
                || snapshot
                    .mcp_servers
                    .iter()
                    .any(|server| server.name != expected_name);
            if has_unexpected_server {
                return ActiveRecheckOutcome::Regressed(IsolationFailure::McpServers);
            }
            let expected_connected = snapshot.mcp_server_errors == 0_usize
                && snapshot
                    .mcp_servers
                    .first()
                    .is_some_and(|server| server.status == "connected");
            if expected_connected {
                if unique_tools == expectation.expected_tools {
                    ActiveRecheckOutcome::Passed
                } else {
                    ActiveRecheckOutcome::Regressed(IsolationFailure::Tools)
                }
            } else {
                ActiveRecheckOutcome::ExpectedMcpDown
            }
        }
    }
}
