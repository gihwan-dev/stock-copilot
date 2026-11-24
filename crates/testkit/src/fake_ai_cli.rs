use crate::fake_ai_cli_replay::{append_invocation_record, run_replay_mode, scrub_optional_token};
use crate::fake_mcp_client::{run_scripted_mcp_mode, run_stdio_unix_bridge};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub const SCENARIO_FLAG: &str = "--fake-ai-scenario";
pub const BRIDGE_FLAG: &str = "--fake-ai-bridge";
pub const MCP_TOKEN_ENV: &str = "STOCK_MCP_TOKEN";

pub struct RedactedToken(String);

impl RedactedToken {
    #[must_use]
    pub fn new(raw: String) -> Self {
        Self(raw)
    }

    pub(crate) fn expose_for_auth(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for RedactedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedactedToken([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvPresence {
    Present,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonessentialTrafficFlag {
    Disabled,
    Other,
    Unset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvObservation {
    pub mcp_token: EnvPresence,
    pub anthropic_api_key: EnvPresence,
    pub openai_api_key: EnvPresence,
    pub user: EnvPresence,
    pub disable_nonessential_traffic: NonessentialTrafficFlag,
}

#[derive(Debug)]
pub struct ProcessEnvironment {
    pub mcp_token: Option<RedactedToken>,
    pub observation: EnvObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnStop {
    Continue,
    Terminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayTurn {
    pub stdout_lines: Vec<String>,
    pub stderr_lines: Vec<String>,
    pub stop_after: TurnStop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayScenario {
    pub immediate_lines: Vec<String>,
    pub turns: Vec<ReplayTurn>,
    pub final_stderr_lines: Vec<String>,
    pub trailing_without_newline: Option<String>,
    pub exit_code: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpProtocolGeneration {
    V2026_07_28,
    V2025_06_18,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CliWireFormat {
    ClaudeStreamJson,
    CodexAppServerJsonRpc,
    CodexExecJsonl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpTransportSpec {
    FromClaudeMcpConfig,
    FromCodexConfigFlags,
    DirectUnixSocket { socket_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptedToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptedTokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpFailureBehavior {
    ContinueWithFailedInit,
    ExitWithCode(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightBehavior {
    Immediate,
    WaitForStdinTurns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodexMultiAgentOverride {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodexNetworkAccessOverride {
    Blocked,
    Allowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InitOverride {
    pub model: Option<String>,
    pub actual_model: Option<String>,
    pub cli_version: Option<String>,
    pub permission_mode: Option<String>,
    pub api_key_source: Option<String>,
    pub skills: Option<Vec<String>>,
    pub slash_commands: Option<Vec<String>>,
    pub plugins: Option<Vec<String>>,
    pub extra_tools: Vec<String>,
    pub sandbox_type: Option<String>,
    pub network_access: Option<CodexNetworkAccessOverride>,
    pub instruction_sources: Option<Vec<String>>,
    pub multi_agent: Option<CodexMultiAgentOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpClientScenario {
    pub protocol: McpProtocolGeneration,
    pub wire_format: CliWireFormat,
    pub transport: McpTransportSpec,
    pub server_name: String,
    pub tool_calls: Vec<ScriptedToolCall>,
    pub final_text: String,
    pub token_usage: ScriptedTokenUsage,
    pub init_override: InitOverride,
    pub on_mcp_failure: McpFailureBehavior,
    pub preflight: PreflightBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FakeAiMode {
    Replay(ReplayScenario),
    ScriptedMcpClient(Box<McpClientScenario>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeAiScenario {
    pub mode: FakeAiMode,
    pub capture_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationRecord {
    pub argv: Vec<String>,
    pub stdin_lines: Vec<String>,
    pub env_observation: EnvObservation,
    pub mcp_tools_discovered: Vec<String>,
    pub mcp_calls_executed: usize,
    pub exit_code: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFakeAiCli {
    executable: PathBuf,
    scenario: PathBuf,
    capture: PathBuf,
}

impl PreparedFakeAiCli {
    pub fn prepare(dir: &Path, binary_name: &str, scenario: &FakeAiScenario) -> io::Result<Self> {
        let source_bin = resolve_fake_ai_cli_binary()?;
        Self::prepare_with_binary(&source_bin, dir, binary_name, scenario)
    }

    pub fn prepare_with_binary(
        source_bin: &Path,
        dir: &Path,
        binary_name: &str,
        scenario: &FakeAiScenario,
    ) -> io::Result<Self> {
        if binary_name.is_empty()
            || binary_name.contains('/')
            || binary_name.contains('\\')
            || binary_name == "."
            || binary_name == ".."
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "binary_name must be a single file name",
            ));
        }
        fs::create_dir_all(dir)?;
        let executable = dir.join(binary_name);
        if executable.exists() {
            fs::remove_file(&executable)?;
        }
        fs::copy(source_bin, &executable)?;
        let capture = match &scenario.capture_path {
            Some(path) => path.clone(),
            None => dir.join(format!("{binary_name}.invocations.jsonl")),
        };
        let mut materialized = scenario.clone();
        materialized.capture_path = Some(capture.clone());
        let scenario_file = sidecar_scenario_path(&executable);
        let serialized = serde_json::to_vec_pretty(&materialized)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(&scenario_file, serialized)?;
        Ok(Self {
            executable,
            scenario: scenario_file,
            capture,
        })
    }

    #[must_use]
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn scenario_path(&self) -> &Path {
        &self.scenario
    }

    #[must_use]
    pub fn capture_path(&self) -> &Path {
        &self.capture
    }

    pub fn read_invocations(&self) -> io::Result<Vec<InvocationRecord>> {
        if !self.capture.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.capture)?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<InvocationRecord>(line)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
            })
            .collect()
    }
}

pub fn sidecar_scenario_path(executable: &Path) -> PathBuf {
    let mut file_name = executable
        .file_name()
        .map_or_else(|| OsString::from("fake_ai_cli"), OsString::from);
    file_name.push(".fake-ai.json");
    executable.with_file_name(file_name)
}

pub fn resolve_fake_ai_cli_binary() -> io::Result<PathBuf> {
    let current = crate::test_executable()?;
    let bin_name = format!("fake_ai_cli{}", std::env::consts::EXE_SUFFIX);
    let workspace_root = find_workspace_root(&current);
    if let Some(found) = find_prebuilt_binary(&current, &bin_name)
        && is_binary_fresh(&found, workspace_root.as_deref()) == BinaryFreshness::Fresh
    {
        return Ok(found);
    }
    let root = workspace_root.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cannot locate workspace root for fake_ai_cli binary",
        )
    })?;
    build_fake_ai_cli_offline(&current, &root, &bin_name)
}

fn find_workspace_root(current: &Path) -> Option<PathBuf> {
    current
        .ancestors()
        .find(|dir| {
            dir.join("Cargo.toml").exists()
                && dir
                    .join("crates")
                    .join("testkit")
                    .join("Cargo.toml")
                    .exists()
        })
        .map(Path::to_path_buf)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryFreshness {
    Fresh,
    Stale,
}

fn is_binary_fresh(binary: &Path, workspace_root: Option<&Path>) -> BinaryFreshness {
    if fs::metadata(binary)
        .and_then(|meta| meta.modified())
        .is_err()
    {
        return BinaryFreshness::Stale;
    }
    let Some(root) = workspace_root else {
        return BinaryFreshness::Fresh;
    };
    let testkit_dir = root.join("crates").join("testkit");
    if is_path_newer_than(&testkit_dir.join("Cargo.toml"), binary) == BinaryFreshness::Stale {
        return BinaryFreshness::Stale;
    }
    directory_freshness(&testkit_dir.join("src"), binary)
}

fn is_path_newer_than(candidate: &Path, reference: &Path) -> BinaryFreshness {
    let Ok(cand_time) = fs::metadata(candidate).and_then(|meta| meta.modified()) else {
        return BinaryFreshness::Fresh;
    };
    let Ok(ref_time) = fs::metadata(reference).and_then(|meta| meta.modified()) else {
        return BinaryFreshness::Stale;
    };
    if cand_time > ref_time {
        BinaryFreshness::Stale
    } else {
        BinaryFreshness::Fresh
    }
}

fn directory_freshness(dir: &Path, binary: &Path) -> BinaryFreshness {
    let Ok(entries) = fs::read_dir(dir) else {
        return BinaryFreshness::Fresh;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if directory_freshness(&path, binary) == BinaryFreshness::Stale {
                return BinaryFreshness::Stale;
            }
            continue;
        }
        if is_path_newer_than(&path, binary) == BinaryFreshness::Stale {
            return BinaryFreshness::Stale;
        }
    }
    BinaryFreshness::Fresh
}

fn find_prebuilt_binary(current: &Path, bin_name: &str) -> Option<PathBuf> {
    let parent = current.parent()?;
    let direct = parent.join(bin_name);
    if direct.exists() {
        return Some(direct);
    }
    if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
        let profile_dir = parent.parent()?;
        let candidate = profile_dir.join(bin_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

struct TargetLayout {
    target_dir: PathBuf,
    profile_name: String,
}

fn derive_target_layout(current: &Path, workspace_root: &Path) -> TargetLayout {
    let derived = current
        .parent()
        .filter(|parent| parent.file_name().and_then(|name| name.to_str()) == Some("deps"))
        .and_then(Path::parent)
        .and_then(|profile_dir| {
            let profile_name = profile_dir.file_name()?.to_str()?.to_owned();
            let target_dir = profile_dir.parent()?.to_path_buf();
            Some(TargetLayout {
                target_dir,
                profile_name,
            })
        });
    derived.unwrap_or_else(|| TargetLayout {
        target_dir: workspace_root.join("target"),
        profile_name: String::from("debug"),
    })
}

fn build_fake_ai_cli_offline(
    current: &Path,
    workspace_root: &Path,
    bin_name: &str,
) -> io::Result<PathBuf> {
    let layout = derive_target_layout(current, workspace_root);
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "--offline",
            "--locked",
            "--quiet",
            "--manifest-path",
        ])
        .arg(workspace_root.join("Cargo.toml"))
        .args([
            "--package",
            "testkit",
            "--bin",
            "fake_ai_cli",
            "--target-dir",
        ])
        .arg(&layout.target_dir);
    if layout.profile_name == "release" {
        command.arg("--release");
    } else if layout.profile_name != "debug" {
        command.args(["--profile", &layout.profile_name]);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(io::Error::other("failed to build fake_ai_cli offline"));
    }
    let built = layout.target_dir.join(&layout.profile_name).join(bin_name);
    if built.exists() {
        Ok(built)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "fake_ai_cli binary missing after offline build",
        ))
    }
}

#[must_use]
pub fn run_from_process(
    executable: &Path,
    raw_args: &[OsString],
    env: &ProcessEnvironment,
) -> ExitCode {
    if let Some(socket_path) = extract_flag_path(raw_args, BRIDGE_FLAG) {
        return match run_stdio_unix_bridge(&socket_path, env.mcp_token.as_ref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                write_stderr_line(&format!("fake_ai_cli bridge error: {}", error.kind()));
                ExitCode::from(2_u8)
            }
        };
    }
    let scenario_path = match extract_flag_path(raw_args, SCENARIO_FLAG) {
        Some(path) => path,
        None => sidecar_scenario_path(executable),
    };
    let scenario = match load_scenario(&scenario_path) {
        Ok(loaded) => loaded,
        Err(error) => {
            write_stderr_line(&format!("fake_ai_cli scenario error: {}", error.kind()));
            return ExitCode::from(2_u8);
        }
    };
    let clean_args = sanitize_argv(raw_args, env.mcp_token.as_ref());
    let outcome = match &scenario.mode {
        FakeAiMode::Replay(replay) => run_replay_mode(replay, env.mcp_token.as_ref()),
        FakeAiMode::ScriptedMcpClient(mcp) => run_scripted_mcp_mode(mcp, raw_args, env),
    };
    match outcome {
        Ok(result) => {
            if let Some(capture) = &scenario.capture_path {
                let record = InvocationRecord {
                    argv: clean_args,
                    stdin_lines: result.stdin_lines,
                    env_observation: env.observation,
                    mcp_tools_discovered: result.mcp_tools_discovered,
                    mcp_calls_executed: result.mcp_calls_executed,
                    exit_code: result.exit_code,
                };
                if let Err(error) = append_invocation_record(capture, &record) {
                    write_stderr_line(&format!("fake_ai_cli capture error: {}", error.kind()));
                    return ExitCode::from(2_u8);
                }
            }
            ExitCode::from(result.exit_code)
        }
        Err(error) => {
            write_stderr_line(&format!("fake_ai_cli runtime error: {}", error.kind()));
            ExitCode::from(2_u8)
        }
    }
}

pub(crate) struct RunOutcome {
    pub(crate) stdin_lines: Vec<String>,
    pub(crate) mcp_tools_discovered: Vec<String>,
    pub(crate) mcp_calls_executed: usize,
    pub(crate) exit_code: u8,
}

fn load_scenario(path: &Path) -> io::Result<FakeAiScenario> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn extract_flag_path(raw_args: &[OsString], flag: &str) -> Option<PathBuf> {
    let mut index = 1_usize;
    while let Some(arg) = raw_args.get(index) {
        if arg.to_str() == Some(flag) {
            return raw_args
                .get(index.saturating_add(1_usize))
                .map(PathBuf::from);
        }
        index = index.saturating_add(1_usize);
    }
    None
}

fn sanitize_argv(raw_args: &[OsString], token: Option<&RedactedToken>) -> Vec<String> {
    raw_args
        .iter()
        .skip(1_usize)
        .map(|arg| scrub_optional_token(&arg.to_string_lossy(), token))
        .collect()
}

fn write_stderr_line(message: &str) {
    let mut stderr = io::stderr().lock();
    match writeln!(stderr, "{message}") {
        Ok(()) | Err(_) => {}
    }
}
