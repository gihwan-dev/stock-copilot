use crate::command::{ApiKeyEnvState, HostEnvironment, McpConfigSource, prepare_run_workspace};
use crate::process::{ProcessSignal, SharedChild, send_signal_to_process_group};
use application::{
    AiCheckObservation, AiEnvironmentProblem, AiRunId, BugDiagnostic, CheckState,
    ClaudeInstallProblem, ClaudeLoginProblem, IsolationFailure, ResponseMismatch, RunKind,
    SelfCheckInput, isolation_check_observation,
};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_PROBE_STDOUT_BYTES: usize = 16_384_usize;
const PROBE_REAP_GRACE_MS: u64 = 1_500_u64;

static DISCOVERY_PROBE_SEQ: AtomicU64 = AtomicU64::new(1_u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    ExplicitOverride,
    KnownLocation,
    ProcessPath,
    LoginShellPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryOutcome {
    Found {
        path: PathBuf,
        source: DiscoverySource,
    },
    NotInstalled,
}

pub trait LoginShellPathResolver: Send + Sync {
    fn resolve_login_shell_path(&self, host_env: &HostEnvironment) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticPathResolver {
    path_value: Option<String>,
}

impl StaticPathResolver {
    #[must_use]
    pub const fn new(path_value: Option<String>) -> Self {
        Self { path_value }
    }
}

impl LoginShellPathResolver for StaticPathResolver {
    fn resolve_login_shell_path(&self, _host_env: &HostEnvironment) -> Option<String> {
        self.path_value.clone()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLoginShellPathResolver;

impl LoginShellPathResolver for SystemLoginShellPathResolver {
    fn resolve_login_shell_path(&self, host_env: &HostEnvironment) -> Option<String> {
        let env = host_env.build_child_env(RunKind::Collection, None).ok()?;
        let mut command = Command::new("/bin/zsh");
        command
            .args(["-l", "-c", "printf '%s' \"$PATH\""])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0_i32);
        for (key, value) in env {
            command.env(key, value);
        }
        let outcome = run_short_probe_command(&mut command)?;
        if outcome.exit != ProbeExit::Succeeded || outcome.stdout.trim().is_empty() {
            return None;
        }
        Some(outcome.stdout)
    }
}

#[must_use]
pub fn default_known_locations(home: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(raw_home) = home
        && !raw_home.trim().is_empty()
    {
        let home_dir = PathBuf::from(raw_home);
        candidates.push(home_dir.join(".local").join("bin").join("claude"));
        candidates.push(home_dir.join(".claude").join("local").join("claude"));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
    candidates.push(PathBuf::from("/usr/local/bin/claude"));
    candidates
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    explicit_override: Option<PathBuf>,
    known_locations: Vec<PathBuf>,
    host_env: HostEnvironment,
}

impl DiscoveryConfig {
    #[must_use]
    pub fn new(host_env: HostEnvironment) -> Self {
        let known_locations = default_known_locations(host_env.home());
        Self {
            explicit_override: None,
            known_locations,
            host_env,
        }
    }

    #[must_use]
    pub fn with_explicit_override(mut self, path: PathBuf) -> Self {
        self.explicit_override = Some(path);
        self
    }

    #[must_use]
    pub fn with_known_locations(mut self, locations: Vec<PathBuf>) -> Self {
        self.known_locations = locations;
        self
    }

    #[must_use]
    pub const fn host_env(&self) -> &HostEnvironment {
        &self.host_env
    }
}

#[must_use]
pub fn find_claude_executable<R: LoginShellPathResolver>(
    config: &DiscoveryConfig,
    login_shell: &R,
) -> DiscoveryOutcome {
    if let Some(explicit) = &config.explicit_override {
        return if is_executable_file(explicit) {
            DiscoveryOutcome::Found {
                path: explicit.clone(),
                source: DiscoverySource::ExplicitOverride,
            }
        } else {
            DiscoveryOutcome::NotInstalled
        };
    }
    for candidate in &config.known_locations {
        if is_executable_file(candidate) {
            return DiscoveryOutcome::Found {
                path: candidate.clone(),
                source: DiscoverySource::KnownLocation,
            };
        }
    }
    if let Some(raw_path) = config.host_env.path()
        && let Some(found) = find_in_path_list(raw_path)
    {
        return DiscoveryOutcome::Found {
            path: found,
            source: DiscoverySource::ProcessPath,
        };
    }
    if let Some(shell_path) = login_shell.resolve_login_shell_path(&config.host_env)
        && let Some(found) = find_in_path_list(&shell_path)
    {
        return DiscoveryOutcome::Found {
            path: found,
            source: DiscoverySource::LoginShellPath,
        };
    }
    DiscoveryOutcome::NotInstalled
}

fn find_in_path_list(path_list: &str) -> Option<PathBuf> {
    for raw_dir in path_list.split(':') {
        if raw_dir.is_empty() {
            continue;
        }
        let dir = Path::new(raw_dir);
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join("claude");
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && (meta.permissions().mode() & 0o111_u32) != 0_u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginCheckOutcome {
    LoggedIn,
    LoginRequired,
    EnvironmentRejected(IsolationFailure),
    CheckUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeStartupReport {
    pub executable: Option<PathBuf>,
    pub discovery_source: Option<DiscoverySource>,
    pub claude_install: CheckState<ClaudeInstallProblem>,
    pub claude_login: CheckState<ClaudeLoginProblem>,
    pub ai_environment: CheckState<AiEnvironmentProblem>,
}

impl ClaudeStartupReport {
    pub fn apply_to_self_check(&self, input: &mut SelfCheckInput) {
        input.claude_install = self.claude_install;
        input.claude_login = self.claude_login;
        input.ai_environment = self.ai_environment;
    }
}

#[must_use]
pub fn check_login_status(
    executable: &Path,
    work_root: &Path,
    host_env: &HostEnvironment,
) -> LoginCheckOutcome {
    if host_env.api_key_state() != ApiKeyEnvState::Absent {
        return LoginCheckOutcome::EnvironmentRejected(IsolationFailure::ApiKeySource);
    }
    let Ok(env_entries) = host_env.build_child_env(RunKind::Collection, None) else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    let seq = DISCOVERY_PROBE_SEQ.fetch_add(1_u64, Ordering::Relaxed);
    let Ok(probe_id) = AiRunId::try_new(&format!("auth-status-{seq}")) else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    let Ok(workspace) = prepare_run_workspace(work_root, &probe_id, McpConfigSource::Empty, None)
    else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    let mut command = Command::new(executable);
    command
        .args(["auth", "status"])
        .current_dir(workspace.work_dir())
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0_i32);
    for (key, value) in env_entries {
        command.env(key, value);
    }
    let probe_result = run_short_probe_command(&mut command);
    workspace.cleanup();
    let Some(output) = probe_result else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    parse_auth_status_output(&output.stdout, output.exit)
}

#[must_use]
pub fn run_claude_startup_check<R: LoginShellPathResolver>(
    config: &DiscoveryConfig,
    login_shell: &R,
    work_root: &Path,
) -> ClaudeStartupReport {
    match find_claude_executable(config, login_shell) {
        DiscoveryOutcome::NotInstalled => ClaudeStartupReport {
            executable: None,
            discovery_source: None,
            claude_install: CheckState::Problem(ClaudeInstallProblem::NotInstalled),
            claude_login: CheckState::Healthy,
            ai_environment: CheckState::Healthy,
        },
        DiscoveryOutcome::Found { path, source } => {
            let login_outcome = check_login_status(&path, work_root, config.host_env());
            report_from_login_outcome(path, source, login_outcome)
        }
    }
}

fn report_from_login_outcome(
    path: PathBuf,
    source: DiscoverySource,
    login_outcome: LoginCheckOutcome,
) -> ClaudeStartupReport {
    let (claude_install, claude_login, ai_environment) = match login_outcome {
        LoginCheckOutcome::LoggedIn => (
            CheckState::Healthy,
            CheckState::Healthy,
            CheckState::Healthy,
        ),
        LoginCheckOutcome::LoginRequired => (
            CheckState::Healthy,
            CheckState::Problem(ClaudeLoginProblem::LoginRequired),
            CheckState::Healthy,
        ),
        LoginCheckOutcome::EnvironmentRejected(failure) => {
            match isolation_check_observation(failure) {
                AiCheckObservation::Install(problem) => (
                    CheckState::Problem(problem),
                    CheckState::Healthy,
                    CheckState::Healthy,
                ),
                AiCheckObservation::Login(problem) => (
                    CheckState::Healthy,
                    CheckState::Problem(problem),
                    CheckState::Healthy,
                ),
                AiCheckObservation::Environment(problem) => (
                    CheckState::Healthy,
                    CheckState::Healthy,
                    CheckState::Problem(problem),
                ),
            }
        }
        LoginCheckOutcome::CheckUnavailable => (
            CheckState::Healthy,
            CheckState::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
            CheckState::Healthy,
        ),
    };
    ClaudeStartupReport {
        executable: Some(path),
        discovery_source: Some(source),
        claude_install,
        claude_login,
        ai_environment,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeExit {
    Succeeded,
    NonZero,
}

fn parse_auth_status_output(raw: &str, exit: ProbeExit) -> LoginCheckOutcome {
    let Ok(parsed) = serde_json::from_str::<Value>(raw.trim()) else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    let Some(object) = parsed.as_object() else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    if let Some(api_source) = object.get("apiKeySource")
        && !api_source.is_null()
        && api_source.as_str() != Some("none")
    {
        return LoginCheckOutcome::EnvironmentRejected(IsolationFailure::ApiKeySource);
    }
    let Some(logged_in) = object.get("loggedIn").and_then(Value::as_bool) else {
        return LoginCheckOutcome::CheckUnavailable;
    };
    if !logged_in {
        return LoginCheckOutcome::LoginRequired;
    }
    match exit {
        ProbeExit::Succeeded => LoginCheckOutcome::LoggedIn,
        ProbeExit::NonZero => LoginCheckOutcome::CheckUnavailable,
    }
}

struct ProbeCommandOutput {
    stdout: String,
    exit: ProbeExit,
}

fn run_short_probe_command(command: &mut Command) -> Option<ProbeCommandOutput> {
    let mut child = command.spawn().ok()?;
    let pgid = child.id();
    let stdout = child.stdout.take()?;
    let shared = SharedChild::new(child);
    let Ok(reader_handle) = std::thread::Builder::new()
        .name(String::from("claude-probe-reader"))
        .spawn(move || read_bounded_stdout(stdout))
    else {
        let _reaped = shared.force_kill_and_reap_blocking();
        return None;
    };
    let exit_status = shared.reap_with_grace_blocking(PROBE_REAP_GRACE_MS);
    if exit_status.is_none() {
        send_signal_to_process_group(pgid, ProcessSignal::Kill);
    }
    let raw_bytes = reader_handle.join().ok()??;
    let text = String::from_utf8(raw_bytes).ok()?;
    let exit = if exit_status.is_some_and(|status| status.success()) {
        ProbeExit::Succeeded
    } else {
        ProbeExit::NonZero
    };
    Some(ProbeCommandOutput { stdout: text, exit })
}

fn read_bounded_stdout(stdout: std::process::ChildStdout) -> Option<Vec<u8>> {
    let limit = u64::try_from(MAX_PROBE_STDOUT_BYTES.saturating_add(1_usize)).ok()?;
    let mut buffer = Vec::new();
    stdout.take(limit).read_to_end(&mut buffer).ok()?;
    if buffer.len() > MAX_PROBE_STDOUT_BYTES {
        return None;
    }
    Some(buffer)
}
