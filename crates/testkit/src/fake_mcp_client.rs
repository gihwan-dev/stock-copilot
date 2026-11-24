use crate::fake_ai_cli::{
    CliWireFormat, CodexMultiAgentOverride, CodexNetworkAccessOverride, EnvPresence,
    McpClientScenario, McpFailureBehavior, McpProtocolGeneration, McpTransportSpec,
    PreflightBehavior, ProcessEnvironment, RedactedToken, RunOutcome, ScriptedToolCall,
};
use crate::fake_ai_cli_replay::{scrub_optional_token, write_stdout_lines};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread;

#[derive(Debug)]
struct ExecutedToolCall {
    call_id: String,
    tool_name: String,
    arguments: Value,
    content_text: String,
    outcome: ToolCallOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolCallOutcome {
    Succeeded,
    Failed,
}

#[derive(Debug)]
enum McpSessionState {
    Connected {
        tools: Vec<String>,
        executed: Vec<ExecutedToolCall>,
    },
    Failed,
}

enum JsonRpcChannel {
    StdioChild {
        child: Child,
        stdin: Option<ChildStdin>,
        reader: BufReader<ChildStdout>,
    },
    Unix {
        writer: UnixStream,
        reader: BufReader<UnixStream>,
    },
}

impl JsonRpcChannel {
    fn send_json(&mut self, payload: &Value) -> io::Result<()> {
        let encoded = serde_json::to_string(payload)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        match self {
            Self::StdioChild { stdin, .. } => {
                let pipe = stdin.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "stdio child stdin closed")
                })?;
                writeln!(pipe, "{encoded}")?;
                pipe.flush()
            }
            Self::Unix { writer, .. } => {
                writeln!(writer, "{encoded}")?;
                writer.flush()
            }
        }
    }

    fn read_response(&mut self) -> io::Result<Value> {
        let mut line = String::new();
        let count = match self {
            Self::StdioChild { reader, .. } => reader.read_line(&mut line)?,
            Self::Unix { reader, .. } => reader.read_line(&mut line)?,
        };
        if count == 0_usize {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mcp peer closed connection before response",
            ));
        }
        serde_json::from_str(line.trim_end_matches(['\r', '\n']))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn close(mut self) {
        match &mut self {
            Self::StdioChild { child, stdin, .. } => {
                *stdin = None;
                match child.wait() {
                    Ok(_) | Err(_) => {}
                }
            }
            Self::Unix { writer, .. } => match writer.shutdown(std::net::Shutdown::Both) {
                Ok(()) | Err(_) => {}
            },
        }
    }
}

#[derive(Debug)]
struct StdioServerConfig {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

pub(crate) fn run_stdio_unix_bridge(
    socket_path: &Path,
    token: Option<&RedactedToken>,
) -> io::Result<()> {
    let mut writer = UnixStream::connect(socket_path)?;
    if let Some(secret) = token {
        let auth_line = format!("AUTH {}\n", secret.expose_for_auth());
        writer.write_all(auth_line.as_bytes())?;
        writer.flush()?;
    }
    let mut reader = writer.try_clone()?;
    let shutdown_handle = writer.try_clone()?;
    let _up = thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let copy_res = io::copy(&mut stdin, &mut writer);
        match writer.shutdown(std::net::Shutdown::Write) {
            Ok(()) | Err(_) => {}
        }
        copy_res
    });
    let down_result = {
        let mut stdout = io::stdout().lock();
        let copy_res = io::copy(&mut reader, &mut stdout);
        let flush_res = stdout.flush();
        copy_res.and(flush_res)
    };
    match shutdown_handle.shutdown(std::net::Shutdown::Both) {
        Ok(()) | Err(_) => {}
    }
    down_result
}

enum McpConnection {
    Connected {
        channel: JsonRpcChannel,
        tools: Vec<String>,
    },
    Failed,
}

impl McpConnection {
    fn close(self) {
        match self {
            Self::Connected { channel, .. } => channel.close(),
            Self::Failed => {}
        }
    }

    fn init_session_view(&self) -> McpSessionState {
        match self {
            Self::Connected { tools, .. } => McpSessionState::Connected {
                tools: tools.clone(),
                executed: Vec::new(),
            },
            Self::Failed => McpSessionState::Failed,
        }
    }
}

pub(crate) fn run_scripted_mcp_mode(
    scenario: &McpClientScenario,
    raw_args: &[OsString],
    env: &ProcessEnvironment,
) -> io::Result<RunOutcome> {
    let connection = match connect_and_list_mcp(scenario, raw_args, env.mcp_token.as_ref()) {
        Ok((channel, tools)) => McpConnection::Connected { channel, tools },
        Err(_error) => McpConnection::Failed,
    };
    if matches_failed_connection(&connection)
        && let McpFailureBehavior::ExitWithCode(code) = scenario.on_mcp_failure
    {
        return Ok(RunOutcome {
            stdin_lines: Vec::new(),
            mcp_tools_discovered: Vec::new(),
            mcp_calls_executed: 0_usize,
            exit_code: code,
        });
    }
    match scenario.preflight {
        PreflightBehavior::Immediate => run_immediate_mcp(scenario, connection, env),
        PreflightBehavior::WaitForStdinTurns => run_preflight_mcp(scenario, connection, env),
    }
}

fn matches_failed_connection(connection: &McpConnection) -> bool {
    match connection {
        McpConnection::Failed => true,
        McpConnection::Connected { .. } => false,
    }
}

fn connect_and_list_mcp(
    scenario: &McpClientScenario,
    raw_args: &[OsString],
    token: Option<&RedactedToken>,
) -> io::Result<(JsonRpcChannel, Vec<String>)> {
    let mut channel = open_channel(&scenario.transport, &scenario.server_name, raw_args, token)?;
    if let Err(error) = perform_handshake(&mut channel, scenario.protocol) {
        channel.close();
        return Err(error);
    }
    let tools = match list_tools(&mut channel) {
        Ok(found) => found,
        Err(error) => {
            channel.close();
            return Err(error);
        }
    };
    Ok((channel, tools))
}

fn finalize_connection(
    connection: McpConnection,
    tool_calls: &[ScriptedToolCall],
) -> io::Result<McpSessionState> {
    match connection {
        McpConnection::Failed => Ok(McpSessionState::Failed),
        McpConnection::Connected { mut channel, tools } => {
            let mut executed = Vec::new();
            let mut next_id = 3_u64;
            for (index, call) in tool_calls.iter().enumerate() {
                match execute_single_tool_call(&mut channel, next_id, index, call) {
                    Ok(record) => {
                        executed.push(record);
                        next_id = next_id.saturating_add(1_u64);
                    }
                    Err(error) => {
                        channel.close();
                        return Err(error);
                    }
                }
            }
            channel.close();
            Ok(McpSessionState::Connected { tools, executed })
        }
    }
}

fn run_immediate_mcp(
    scenario: &McpClientScenario,
    connection: McpConnection,
    env: &ProcessEnvironment,
) -> io::Result<RunOutcome> {
    let session = finalize_connection(connection, &scenario.tool_calls)?;
    let lines = build_full_session_lines(scenario, &session, env)?;
    write_stdout_lines(&lines)?;
    let (tools, calls) = session_summary(session);
    Ok(RunOutcome {
        stdin_lines: Vec::new(),
        mcp_tools_discovered: tools,
        mcp_calls_executed: calls,
        exit_code: 0_u8,
    })
}

fn run_preflight_mcp(
    scenario: &McpClientScenario,
    connection: McpConnection,
    env: &ProcessEnvironment,
) -> io::Result<RunOutcome> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdin_lines = Vec::new();
    let mut first_buf = String::new();
    if reader.read_line(&mut first_buf)? == 0_usize {
        let init_view = connection.init_session_view();
        connection.close();
        let (tools, calls) = session_summary(init_view);
        return Ok(RunOutcome {
            stdin_lines,
            mcp_tools_discovered: tools,
            mcp_calls_executed: calls,
            exit_code: 0_u8,
        });
    }
    stdin_lines.push(scrub_optional_token(
        first_buf.trim_end_matches(['\r', '\n']),
        env.mcp_token.as_ref(),
    ));
    let init_view = connection.init_session_view();
    let init_lines = build_init_only_lines(scenario, &init_view, env)?;
    write_stdout_lines(&init_lines)?;
    let mut second_buf = String::new();
    if reader.read_line(&mut second_buf)? == 0_usize {
        connection.close();
        let (tools, calls) = session_summary(init_view);
        return Ok(RunOutcome {
            stdin_lines,
            mcp_tools_discovered: tools,
            mcp_calls_executed: calls,
            exit_code: 0_u8,
        });
    }
    stdin_lines.push(scrub_optional_token(
        second_buf.trim_end_matches(['\r', '\n']),
        env.mcp_token.as_ref(),
    ));
    let session = finalize_connection(connection, &scenario.tool_calls)?;
    let body_lines = build_body_only_lines(scenario, &session)?;
    write_stdout_lines(&body_lines)?;
    let (tools, calls) = session_summary(session);
    Ok(RunOutcome {
        stdin_lines,
        mcp_tools_discovered: tools,
        mcp_calls_executed: calls,
        exit_code: 0_u8,
    })
}

fn session_summary(session: McpSessionState) -> (Vec<String>, usize) {
    match session {
        McpSessionState::Connected { tools, executed } => (tools, executed.len()),
        McpSessionState::Failed => (Vec::new(), 0_usize),
    }
}

fn open_channel(
    transport: &McpTransportSpec,
    server_name: &str,
    raw_args: &[OsString],
    token: Option<&RedactedToken>,
) -> io::Result<JsonRpcChannel> {
    match transport {
        McpTransportSpec::FromClaudeMcpConfig => {
            let config = parse_claude_mcp_config(raw_args, server_name, token)?;
            spawn_stdio_server(&config)
        }
        McpTransportSpec::FromCodexConfigFlags => {
            let config = parse_codex_mcp_flags(raw_args, server_name, token)?;
            spawn_stdio_server(&config)
        }
        McpTransportSpec::DirectUnixSocket { socket_path } => {
            let mut writer = UnixStream::connect(socket_path)?;
            if let Some(secret) = token {
                let auth_line = format!("AUTH {}\n", secret.expose_for_auth());
                writer.write_all(auth_line.as_bytes())?;
                writer.flush()?;
            }
            let reader_stream = writer.try_clone()?;
            Ok(JsonRpcChannel::Unix {
                writer,
                reader: BufReader::new(reader_stream),
            })
        }
    }
}

fn spawn_stdio_server(config: &StdioServerConfig) -> io::Result<JsonRpcChannel> {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .env_clear()
        .envs(&config.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdin pipe"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdout pipe"))?;
    Ok(JsonRpcChannel::StdioChild {
        child,
        stdin: Some(stdin),
        reader: BufReader::new(stdout),
    })
}

fn perform_handshake(
    channel: &mut JsonRpcChannel,
    protocol: McpProtocolGeneration,
) -> io::Result<()> {
    match protocol {
        McpProtocolGeneration::V2026_07_28 => {
            let discover = json!({
                "jsonrpc": "2.0",
                "id": 1_u64,
                "method": "server/discover",
                "params": {
                    "protocolVersion": "2026-07-28",
                    "clientInfo": { "name": "fake-claude-code", "version": "2.1.322" }
                }
            });
            channel.send_json(&discover)?;
            let reply = channel.read_response()?;
            require_rpc_result(&reply)
        }
        McpProtocolGeneration::V2025_06_18 => {
            let initialize = json!({
                "jsonrpc": "2.0",
                "id": 1_u64,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "fake-codex-mcp-client", "version": "0.167.1" }
                }
            });
            channel.send_json(&initialize)?;
            let reply = channel.read_response()?;
            require_rpc_result(&reply)?;
            let initialized = json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            channel.send_json(&initialized)
        }
    }
}

fn list_tools(channel: &mut JsonRpcChannel) -> io::Result<Vec<String>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2_u64,
        "method": "tools/list",
        "params": {}
    });
    channel.send_json(&request)?;
    let reply = channel.read_response()?;
    require_rpc_result(&reply)?;
    let tools_array = reply
        .get("result")
        .and_then(|res| res.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing result.tools"))?;
    let mut names = Vec::new();
    for entry in tools_array {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing tool name"))?;
        names.push(String::from(name));
    }
    Ok(names)
}

fn execute_single_tool_call(
    channel: &mut JsonRpcChannel,
    request_id: u64,
    index: usize,
    call: &ScriptedToolCall,
) -> io::Result<ExecutedToolCall> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": call.name,
            "arguments": call.arguments,
        }
    });
    channel.send_json(&request)?;
    let reply = channel.read_response()?;
    let call_id = format!("toolu_fake_{}", index.saturating_add(1_usize));
    if let Some(error_obj) = reply.get("error") {
        let encoded = serde_json::to_string(error_obj)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        return Ok(ExecutedToolCall {
            call_id,
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
            content_text: encoded,
            outcome: ToolCallOutcome::Failed,
        });
    }
    let result_obj = reply
        .get("result")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing tools/call result"))?;
    let outcome = if result_obj.get("isError").and_then(Value::as_bool) == Some(true) {
        ToolCallOutcome::Failed
    } else {
        ToolCallOutcome::Succeeded
    };
    let content_text = extract_mcp_result_text(result_obj)?;
    Ok(ExecutedToolCall {
        call_id,
        tool_name: call.name.clone(),
        arguments: call.arguments.clone(),
        content_text,
        outcome,
    })
}

fn extract_mcp_result_text(result_obj: &Value) -> io::Result<String> {
    if let Some(blocks) = result_obj.get("content").and_then(Value::as_array)
        && let [Value::Object(only)] = blocks.as_slice()
        && only.get("type").and_then(Value::as_str) == Some("text")
        && let Some(text) = only.get("text").and_then(Value::as_str)
    {
        return Ok(String::from(text));
    }
    serde_json::to_string(result_obj)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn require_rpc_result(reply: &Value) -> io::Result<()> {
    if reply.get("error").is_some() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "mcp server returned rpc error",
        ));
    }
    if reply.get("result").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mcp server reply missing result",
        ));
    }
    Ok(())
}

fn parse_claude_mcp_config(
    raw_args: &[OsString],
    server_name: &str,
    token: Option<&RedactedToken>,
) -> io::Result<StdioServerConfig> {
    let config_path = find_arg_value(raw_args, "--mcp-config").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing --mcp-config argument")
    })?;
    let raw = fs::read_to_string(config_path)?;
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let server = parsed
        .get("mcpServers")
        .and_then(|map| map.get(server_name))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "server entry missing in mcp-config",
            )
        })?;
    let command = server
        .get("command")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing server.command"))?;
    let args = parse_json_string_array(server.get("args"))?;
    let env = expand_claude_env_map(server.get("env"), token)?;
    Ok(StdioServerConfig { command, args, env })
}

fn expand_claude_env_map(
    value: Option<&Value>,
    token: Option<&RedactedToken>,
) -> io::Result<BTreeMap<String, String>> {
    let Some(raw_env) = value else {
        return Ok(BTreeMap::new());
    };
    let object = raw_env
        .as_object()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "server.env must be object"))?;
    let mut expanded = BTreeMap::new();
    for (key, entry) in object {
        let raw_text = entry
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "env value must be text"))?;
        if raw_text == "${STOCK_MCP_TOKEN}" {
            if let Some(secret) = token {
                expanded.insert(key.clone(), String::from(secret.expose_for_auth()));
            }
        } else {
            expanded.insert(key.clone(), String::from(raw_text));
        }
    }
    Ok(expanded)
}

fn parse_codex_mcp_flags(
    raw_args: &[OsString],
    server_name: &str,
    token: Option<&RedactedToken>,
) -> io::Result<StdioServerConfig> {
    let prefix = format!("mcp_servers.{server_name}.");
    let mut command = None;
    let mut args = Vec::new();
    let mut env_vars = Vec::new();
    let mut index = 1_usize;
    while let Some(arg) = raw_args.get(index) {
        if arg.to_str() == Some("-c")
            && let Some(setting) = raw_args.get(index.saturating_add(1_usize))
            && let Some(text) = setting.to_str()
            && let Some(rest) = text.strip_prefix(&prefix)
            && let Some((key, raw_val)) = rest.split_once('=')
        {
            apply_codex_config_entry(key, raw_val, &mut command, &mut args, &mut env_vars)?;
            index = index.saturating_add(2_usize);
            continue;
        }
        index = index.saturating_add(1_usize);
    }
    let resolved_command = command.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing -c mcp_servers.<name>.command",
        )
    })?;
    let mut env = BTreeMap::new();
    if env_vars.iter().any(|key| key == "STOCK_MCP_TOKEN")
        && let Some(secret) = token
    {
        env.insert(
            String::from("STOCK_MCP_TOKEN"),
            String::from(secret.expose_for_auth()),
        );
    }
    Ok(StdioServerConfig {
        command: resolved_command,
        args,
        env,
    })
}

fn apply_codex_config_entry(
    key: &str,
    raw_val: &str,
    command: &mut Option<String>,
    args: &mut Vec<String>,
    env_vars: &mut Vec<String>,
) -> io::Result<()> {
    match key {
        "command" => {
            let parsed: Value = serde_json::from_str(raw_val)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let text = parsed
                .as_str()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "command must be str"))?;
            *command = Some(String::from(text));
        }
        "args" => {
            let parsed: Value = serde_json::from_str(raw_val)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            *args = parse_json_string_array(Some(&parsed))?;
        }
        "env_vars" => {
            let parsed: Value = serde_json::from_str(raw_val)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            *env_vars = parse_json_string_array(Some(&parsed))?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_json_string_array(value: Option<&Value>) -> io::Result<Vec<String>> {
    let Some(raw) = value else {
        return Ok(Vec::new());
    };
    let items = raw
        .as_array()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "expected string array"))?;
    let mut output = Vec::new();
    for item in items {
        let text = item
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "array item must be str"))?;
        output.push(String::from(text));
    }
    Ok(output)
}

fn find_arg_value(raw_args: &[OsString], flag: &str) -> Option<String> {
    let mut index = 1_usize;
    while let Some(arg) = raw_args.get(index) {
        if arg.to_str() == Some(flag) {
            return raw_args
                .get(index.saturating_add(1_usize))
                .and_then(|value| value.to_str())
                .map(String::from);
        }
        index = index.saturating_add(1_usize);
    }
    None
}

fn build_full_session_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
    env: &ProcessEnvironment,
) -> io::Result<Vec<String>> {
    let mut lines = build_init_only_lines(scenario, session, env)?;
    lines.extend(build_body_only_lines(scenario, session)?);
    Ok(lines)
}

fn build_init_only_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
    env: &ProcessEnvironment,
) -> io::Result<Vec<String>> {
    match scenario.wire_format {
        CliWireFormat::ClaudeStreamJson => Ok(vec![encode_json(&build_claude_init(
            scenario, session, env,
        ))?]),
        CliWireFormat::CodexAppServerJsonRpc => {
            let (thread_start, mcp_list) = build_codex_startup_lines(scenario, session);
            Ok(vec![encode_json(&thread_start)?, encode_json(&mcp_list)?])
        }
        CliWireFormat::CodexExecJsonl => Ok(vec![
            encode_json(&json!({"type": "thread.started", "thread_id": "thread-fake-001"}))?,
            encode_json(&json!({"type": "turn.started"}))?,
        ]),
    }
}

fn build_body_only_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
) -> io::Result<Vec<String>> {
    match scenario.wire_format {
        CliWireFormat::ClaudeStreamJson => build_claude_body_lines(scenario, session),
        CliWireFormat::CodexAppServerJsonRpc => {
            build_codex_app_server_body_lines(scenario, session)
        }
        CliWireFormat::CodexExecJsonl => build_codex_exec_body_lines(scenario, session),
    }
}

fn build_claude_init(
    scenario: &McpClientScenario,
    session: &McpSessionState,
    env: &ProcessEnvironment,
) -> Value {
    let overrides = &scenario.init_override;
    let model = overrides
        .model
        .as_deref()
        .map_or("claude-opus-5-5", |value| value);
    let cli_version = overrides
        .cli_version
        .as_deref()
        .map_or("2.1.322", |value| value);
    let permission_mode = overrides
        .permission_mode
        .as_deref()
        .map_or("dontAsk", |value| value);
    let api_key_source = match overrides.api_key_source.as_deref() {
        Some(custom) => custom,
        None => match env.observation.anthropic_api_key {
            EnvPresence::Present => "env",
            EnvPresence::Absent => "none",
        },
    };
    let (mcp_status, mut tools) = match session {
        McpSessionState::Connected { tools, .. } => (
            "connected",
            tools
                .iter()
                .map(|tool| format!("mcp__{}__{tool}", scenario.server_name))
                .collect::<Vec<_>>(),
        ),
        McpSessionState::Failed => ("failed", Vec::new()),
    };
    tools.extend(overrides.extra_tools.iter().cloned());
    let skills = match &overrides.skills {
        Some(items) => items.clone(),
        None => Vec::new(),
    };
    let slash_commands = match &overrides.slash_commands {
        Some(items) => items.clone(),
        None => Vec::new(),
    };
    let plugins = match &overrides.plugins {
        Some(items) => items.clone(),
        None => vec![String::from("cc-plugin-privacy")],
    };
    json!({
        "type": "system",
        "subtype": "init",
        "tools": tools,
        "mcp_servers": [{"name": scenario.server_name, "status": mcp_status}],
        "model": model,
        "permissionMode": permission_mode,
        "slash_commands": slash_commands,
        "skills": skills,
        "plugins": plugins,
        "apiKeySource": api_key_source,
        "claude_code_version": cli_version
    })
}

fn push_claude_tool_events(
    server_name: &str,
    actual_model: &str,
    executed: &[ExecutedToolCall],
    lines: &mut Vec<String>,
) -> io::Result<()> {
    for call in executed {
        let full_name = format!("mcp__{server_name}__{}", call.tool_name);
        let tool_use = json!({
            "type": "assistant",
            "message": {
                "model": actual_model,
                "content": [{
                    "type": "tool_use",
                    "id": call.call_id,
                    "name": full_name,
                    "input": call.arguments
                }]
            }
        });
        let is_error = call.outcome == ToolCallOutcome::Failed;
        let tool_result = json!({
            "type": "user",
            "message": {
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": call.call_id,
                    "content": call.content_text,
                    "is_error": is_error
                }]
            }
        });
        lines.push(encode_json(&tool_use)?);
        lines.push(encode_json(&tool_result)?);
    }
    Ok(())
}

fn build_claude_body_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
) -> io::Result<Vec<String>> {
    let mut lines = Vec::new();
    let actual_model = scenario
        .init_override
        .actual_model
        .as_deref()
        .or(scenario.init_override.model.as_deref())
        .map_or("claude-opus-5-5", |value| value);
    if let McpSessionState::Connected { executed, .. } = session {
        push_claude_tool_events(&scenario.server_name, actual_model, executed, &mut lines)?;
    }
    let delta = json!({
        "type": "stream_event",
        "event": {
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": scenario.final_text }
        }
    });
    let usage = scenario.token_usage;
    let mut model_usage = Map::new();
    model_usage.insert(
        String::from(actual_model),
        json!({ "inputTokens": usage.input }),
    );
    let result = json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "usage": {
            "input_tokens": usage.input,
            "output_tokens": usage.output,
            "cache_read_input_tokens": usage.cache_read,
            "cache_creation_input_tokens": usage.cache_creation
        },
        "modelUsage": model_usage
    });
    lines.push(encode_json(&delta)?);
    lines.push(encode_json(&result)?);
    Ok(lines)
}

fn build_codex_startup_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
) -> (Value, Value) {
    let overrides = &scenario.init_override;
    let sandbox_type = overrides
        .sandbox_type
        .as_deref()
        .map_or("readOnly", |value| value);
    let network_access = overrides.network_access == Some(CodexNetworkAccessOverride::Allowed);
    let instruction_sources = match &overrides.instruction_sources {
        Some(items) => items.clone(),
        None => Vec::new(),
    };
    let multi_agent = overrides.multi_agent == Some(CodexMultiAgentOverride::Enabled);
    let cli_version = overrides
        .cli_version
        .as_deref()
        .map_or("0.167.1", |value| value);
    let model = overrides
        .model
        .as_deref()
        .map_or("gpt-6.1-sol", |value| value);
    let thread_start = json!({
        "id": 1_u64,
        "result": {
            "sandbox": { "type": sandbox_type, "networkAccess": network_access },
            "instructionSources": instruction_sources,
            "multiAgentMode": multi_agent,
            "cliVersion": cli_version,
            "model": model
        }
    });
    let (runtime_status, mut tools) = match session {
        McpSessionState::Connected { tools, .. } => ("connected", tools.clone()),
        McpSessionState::Failed => ("failed", Vec::new()),
    };
    tools.extend(overrides.extra_tools.iter().cloned());
    let mcp_list = json!({
        "id": 2_u64,
        "result": {
            "data": [{
                "name": scenario.server_name,
                "runtimeStatus": runtime_status,
                "tools": tools
            }]
        }
    });
    (thread_start, mcp_list)
}

fn codex_tool_result_fields(call: &ExecutedToolCall) -> (&'static str, Value, Value) {
    match call.outcome {
        ToolCallOutcome::Succeeded => (
            "completed",
            json!({ "content": [{ "type": "text", "text": call.content_text }] }),
            Value::Null,
        ),
        ToolCallOutcome::Failed => (
            "failed",
            Value::Null,
            Value::String(call.content_text.clone()),
        ),
    }
}

fn push_codex_app_server_tool_events(
    server_name: &str,
    executed: &[ExecutedToolCall],
    lines: &mut Vec<String>,
) -> io::Result<()> {
    for call in executed {
        let started = json!({
            "method": "item/started",
            "params": {
                "item": {
                    "id": call.call_id,
                    "type": "mcpToolCall",
                    "server": server_name,
                    "tool": call.tool_name,
                    "arguments": call.arguments,
                    "status": "inProgress"
                }
            }
        });
        let (status, result_val, error_val) = codex_tool_result_fields(call);
        let completed = json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": call.call_id,
                    "type": "mcpToolCall",
                    "server": server_name,
                    "tool": call.tool_name,
                    "arguments": call.arguments,
                    "result": result_val,
                    "error": error_val,
                    "status": status
                }
            }
        });
        lines.push(encode_json(&started)?);
        lines.push(encode_json(&completed)?);
    }
    Ok(())
}

fn build_codex_app_server_body_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
) -> io::Result<Vec<String>> {
    let mut lines = Vec::new();
    let actual_model = scenario
        .init_override
        .actual_model
        .as_deref()
        .or(scenario.init_override.model.as_deref())
        .map_or("gpt-6.1-sol", |value| value);
    if let McpSessionState::Connected { executed, .. } = session {
        push_codex_app_server_tool_events(&scenario.server_name, executed, &mut lines)?;
    }
    let delta = json!({
        "method": "item/agentMessage/delta",
        "params": { "delta": scenario.final_text }
    });
    let usage = scenario.token_usage;
    let token_usage = json!({
        "method": "thread/tokenUsage/updated",
        "params": {
            "last": {
                "inputTokens": usage.input,
                "outputTokens": usage.output,
                "cachedInputTokens": usage.cache_read,
                "cacheWriteInputTokens": usage.cache_creation
            }
        }
    });
    let turn_completed = json!({
        "method": "turn/completed",
        "params": { "status": "completed", "model": actual_model }
    });
    lines.push(encode_json(&delta)?);
    lines.push(encode_json(&token_usage)?);
    lines.push(encode_json(&turn_completed)?);
    Ok(lines)
}

fn push_codex_exec_tool_events(
    server_name: &str,
    executed: &[ExecutedToolCall],
    lines: &mut Vec<String>,
) -> io::Result<()> {
    for call in executed {
        let started = json!({
            "type": "item.started",
            "item": {
                "id": call.call_id,
                "type": "mcp_tool_call",
                "server": server_name,
                "tool": call.tool_name,
                "arguments": call.arguments,
                "status": "in_progress"
            }
        });
        let (status, result_val, error_val) = codex_tool_result_fields(call);
        let completed = json!({
            "type": "item.completed",
            "item": {
                "id": call.call_id,
                "type": "mcp_tool_call",
                "server": server_name,
                "tool": call.tool_name,
                "arguments": call.arguments,
                "result": result_val,
                "error": error_val,
                "status": status
            }
        });
        lines.push(encode_json(&started)?);
        lines.push(encode_json(&completed)?);
    }
    Ok(())
}

fn build_codex_exec_body_lines(
    scenario: &McpClientScenario,
    session: &McpSessionState,
) -> io::Result<Vec<String>> {
    let mut lines = Vec::new();
    let actual_model = scenario
        .init_override
        .actual_model
        .as_deref()
        .or(scenario.init_override.model.as_deref())
        .map_or("gpt-6.1-sol", |value| value);
    if let McpSessionState::Connected { executed, .. } = session {
        push_codex_exec_tool_events(&scenario.server_name, executed, &mut lines)?;
    }
    let message = json!({
        "type": "item.completed",
        "item": {
            "id": "item_msg_fake_1",
            "type": "agent_message",
            "text": scenario.final_text
        }
    });
    let usage = scenario.token_usage;
    let turn_completed = json!({
        "type": "turn.completed",
        "usage": {
            "input_tokens": usage.input,
            "output_tokens": usage.output,
            "cached_input_tokens": usage.cache_read,
            "cache_write_input_tokens": usage.cache_creation
        },
        "model": actual_model
    });
    lines.push(encode_json(&message)?);
    lines.push(encode_json(&turn_completed)?);
    Ok(lines)
}

fn encode_json(value: &Value) -> io::Result<String> {
    serde_json::to_string(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
