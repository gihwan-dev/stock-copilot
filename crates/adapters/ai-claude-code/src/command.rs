use application::{
    AiRunId, InformationError, ModelName, ReasoningEffort, ResponseMismatch, RunKind, SecretValue,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MCP_TOKEN_ENV_KEY: &str = "STOCK_MCP_TOKEN";
pub const NONESSENTIAL_TRAFFIC_ENV_KEY: &str = "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC";
pub const ALLOWED_HOST_ENV_KEYS: [&str; 5] = ["HOME", "PATH", "USER", "LOGNAME", "TMPDIR"];
pub const FORBIDDEN_API_KEY_ENV_KEYS: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

static WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(1_u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyEnvState {
    Absent,
    AnthropicApiKey,
    AnthropicAuthToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredEnvKey {
    Home,
    Path,
    User,
    TmpDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBuildError {
    MissingHostEnvironment,
    ForbiddenApiKeyEnv(ApiKeyEnvState),
    MissingRequiredEnv(RequiredEnvKey),
    InvalidEnvValue(RequiredEnvKey),
    InvalidLogname,
    InvalidAnalysisTools,
    MissingMcpConfigForAnalysis,
    InvalidMcpServerName,
    McpAttachedToCollection,
    InvalidMcpConfigFile,
    SecretInConfig,
    InvalidToken,
    InvalidWorkRoot,
    WorkDirIo,
    InvalidJsonSchema,
}

impl CommandBuildError {
    #[must_use]
    pub const fn to_information_error(self) -> InformationError {
        match self {
            Self::WorkDirIo => InformationError::Unavailable(application::ResetTime::NotProvided),
            Self::MissingHostEnvironment
            | Self::ForbiddenApiKeyEnv(_)
            | Self::MissingRequiredEnv(_)
            | Self::InvalidEnvValue(_)
            | Self::InvalidLogname
            | Self::InvalidAnalysisTools
            | Self::MissingMcpConfigForAnalysis
            | Self::InvalidMcpServerName
            | Self::McpAttachedToCollection
            | Self::InvalidMcpConfigFile
            | Self::SecretInConfig
            | Self::InvalidToken
            | Self::InvalidWorkRoot
            | Self::InvalidJsonSchema => {
                InformationError::InvalidResponse(ResponseMismatch::Validation)
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HostEnvironment {
    home: Option<String>,
    path: Option<String>,
    user: Option<String>,
    logname: Option<String>,
    tmpdir: Option<String>,
    api_key_state: ApiKeyEnvState,
}

impl fmt::Debug for HostEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostEnvironment")
            .field("has_home", &self.home.is_some())
            .field("has_path", &self.path.is_some())
            .field("has_user", &self.user.is_some())
            .field("has_logname", &self.logname.is_some())
            .field("has_tmpdir", &self.tmpdir.is_some())
            .field("api_key_state", &self.api_key_state)
            .finish()
    }
}

impl HostEnvironment {
    #[must_use]
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<String>,
    {
        let mut env = Self {
            home: None,
            path: None,
            user: None,
            logname: None,
            tmpdir: None,
            api_key_state: ApiKeyEnvState::Absent,
        };
        for (raw_key, raw_value) in pairs {
            match raw_key.as_ref() {
                "HOME" => env.home = Some(raw_value.into()),
                "PATH" => env.path = Some(raw_value.into()),
                "USER" => env.user = Some(raw_value.into()),
                "LOGNAME" => env.logname = Some(raw_value.into()),
                "TMPDIR" => env.tmpdir = Some(raw_value.into()),
                "ANTHROPIC_API_KEY" => env.api_key_state = ApiKeyEnvState::AnthropicApiKey,
                "ANTHROPIC_AUTH_TOKEN" if env.api_key_state == ApiKeyEnvState::Absent => {
                    env.api_key_state = ApiKeyEnvState::AnthropicAuthToken;
                }
                _ => {}
            }
        }
        env
    }

    #[must_use]
    pub fn new_clean(
        home: String,
        path: String,
        user: String,
        logname: Option<String>,
        tmpdir: String,
    ) -> Self {
        Self {
            home: Some(home),
            path: Some(path),
            user: Some(user),
            logname,
            tmpdir: Some(tmpdir),
            api_key_state: ApiKeyEnvState::Absent,
        }
    }

    #[must_use]
    pub const fn api_key_state(&self) -> ApiKeyEnvState {
        self.api_key_state
    }

    #[must_use]
    pub fn home(&self) -> Option<&str> {
        self.home.as_deref()
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    #[must_use]
    pub fn tmpdir(&self) -> Option<&str> {
        self.tmpdir.as_deref()
    }

    pub fn build_child_env(
        &self,
        kind: RunKind,
        mcp_token: Option<&SecretValue>,
    ) -> Result<Vec<(String, String)>, CommandBuildError> {
        if self.api_key_state != ApiKeyEnvState::Absent {
            return Err(CommandBuildError::ForbiddenApiKeyEnv(self.api_key_state));
        }
        let home = validate_required_env(self.home.as_deref(), RequiredEnvKey::Home)?;
        let path = validate_required_env(self.path.as_deref(), RequiredEnvKey::Path)?;
        let user = validate_required_env(self.user.as_deref(), RequiredEnvKey::User)?;
        let tmpdir = validate_required_env(self.tmpdir.as_deref(), RequiredEnvKey::TmpDir)?;
        let mut entries = vec![
            (String::from("HOME"), home),
            (String::from("PATH"), path),
            (String::from("USER"), user),
            (String::from("TMPDIR"), tmpdir),
            (
                String::from(NONESSENTIAL_TRAFFIC_ENV_KEY),
                String::from("1"),
            ),
        ];
        if let Some(raw_logname) = self.logname.as_deref() {
            if raw_logname.trim().is_empty() || raw_logname.contains('\0') {
                return Err(CommandBuildError::InvalidLogname);
            }
            entries.push((String::from("LOGNAME"), String::from(raw_logname)));
        }
        if kind == RunKind::Analysis
            && let Some(secret) = mcp_token
        {
            let _validated = validate_token_str(secret)?;
        }
        Ok(entries)
    }
}

fn validate_required_env(
    value: Option<&str>,
    key: RequiredEnvKey,
) -> Result<String, CommandBuildError> {
    let Some(raw) = value else {
        return Err(CommandBuildError::MissingRequiredEnv(key));
    };
    if raw.trim().is_empty() {
        return Err(CommandBuildError::MissingRequiredEnv(key));
    }
    if raw.contains('\0') {
        return Err(CommandBuildError::InvalidEnvValue(key));
    }
    Ok(String::from(raw))
}

pub(crate) fn validate_token_str(secret: &SecretValue) -> Result<&str, CommandBuildError> {
    let Ok(text) = std::str::from_utf8(secret.as_exposed_secret()) else {
        return Err(CommandBuildError::InvalidToken);
    };
    if text.is_empty() || text.contains(['\0', '\r', '\n']) {
        return Err(CommandBuildError::InvalidToken);
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpBridgeConfig {
    server_name: String,
    bridge_command: PathBuf,
    bridge_args: Vec<String>,
}

impl McpBridgeConfig {
    pub fn try_new(
        server_name: String,
        bridge_command: PathBuf,
        bridge_args: Vec<String>,
    ) -> Result<Self, CommandBuildError> {
        if !is_valid_server_name(&server_name) || bridge_command.as_os_str().is_empty() {
            return Err(CommandBuildError::InvalidMcpServerName);
        }
        Ok(Self {
            server_name,
            bridge_command,
            bridge_args,
        })
    }

    #[must_use]
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    #[must_use]
    pub fn bridge_command(&self) -> &Path {
        &self.bridge_command
    }

    #[must_use]
    pub fn bridge_args(&self) -> &[String] {
        &self.bridge_args
    }

    pub fn render_mcp_json(
        &self,
        secret: Option<&SecretValue>,
    ) -> Result<String, CommandBuildError> {
        let placeholder = format!("${{{MCP_TOKEN_ENV_KEY}}}");
        let payload = json!({
            "mcpServers": {
                &self.server_name: {
                    "type": "stdio",
                    "command": self.bridge_command.to_string_lossy(),
                    "args": self.bridge_args,
                    "env": {
                        MCP_TOKEN_ENV_KEY: placeholder,
                    }
                }
            }
        });
        let rendered =
            serde_json::to_string(&payload).map_err(|_error| CommandBuildError::WorkDirIo)?;
        verify_no_secret_in_text(&rendered, secret)?;
        Ok(rendered)
    }
}

fn is_valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64_usize
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn verify_no_secret_in_text(
    text: &str,
    secret: Option<&SecretValue>,
) -> Result<(), CommandBuildError> {
    if let Some(secret_value) = secret {
        let token = validate_token_str(secret_value)?;
        if text.contains(token) {
            return Err(CommandBuildError::SecretInConfig);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigSource<'a> {
    Empty,
    Bridge(&'a McpBridgeConfig),
    ExistingFile(&'a Path),
}

pub struct RunWorkspace {
    work_dir: PathBuf,
    config_dir: PathBuf,
    mcp_config_path: PathBuf,
}

impl fmt::Debug for RunWorkspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunWorkspace")
            .field("work_dir", &self.work_dir)
            .field("config_dir", &self.config_dir)
            .field("mcp_config_path", &self.mcp_config_path)
            .finish()
    }
}

impl RunWorkspace {
    #[must_use]
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    #[must_use]
    pub fn mcp_config_path(&self) -> &Path {
        &self.mcp_config_path
    }

    pub fn cleanup(&self) {
        remove_dir_if_not_symlink(&self.work_dir);
        remove_dir_if_not_symlink(&self.config_dir);
    }
}

impl Drop for RunWorkspace {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn remove_dir_if_not_symlink(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.file_type().is_symlink() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) | Err(_) => {}
    }
}

pub fn prepare_run_workspace(
    base_work_dir: &Path,
    run_id: &AiRunId,
    mcp_source: McpConfigSource<'_>,
    secret: Option<&SecretValue>,
) -> Result<RunWorkspace, CommandBuildError> {
    ensure_private_root(base_work_dir)?;
    let seq = WORKSPACE_SEQUENCE.fetch_add(1_u64, Ordering::Relaxed);
    let work_dir = base_work_dir.join(format!("work-{}-{seq}", run_id.as_str()));
    let config_dir = base_work_dir.join(format!("cfg-{}-{seq}", run_id.as_str()));
    create_private_empty_dir(&work_dir)?;
    if let Err(error) = create_private_empty_dir(&config_dir) {
        remove_dir_if_not_symlink(&work_dir);
        return Err(error);
    }
    let mcp_result = materialize_mcp_config(&config_dir, mcp_source, secret);
    match mcp_result {
        Ok(mcp_config_path) => Ok(RunWorkspace {
            work_dir,
            config_dir,
            mcp_config_path,
        }),
        Err(error) => {
            remove_dir_if_not_symlink(&work_dir);
            remove_dir_if_not_symlink(&config_dir);
            Err(error)
        }
    }
}

fn ensure_private_root(root: &Path) -> Result<(), CommandBuildError> {
    if root.as_os_str().is_empty() {
        return Err(CommandBuildError::InvalidWorkRoot);
    }
    match fs::symlink_metadata(root) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(CommandBuildError::InvalidWorkRoot);
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DirBuilder::new()
            .recursive(true)
            .mode(0o700_u32)
            .create(root)
            .map_err(|_io_err| CommandBuildError::WorkDirIo),
        Err(_) => Err(CommandBuildError::WorkDirIo),
    }
}

fn create_private_empty_dir(path: &Path) -> Result<(), CommandBuildError> {
    DirBuilder::new()
        .mode(0o700_u32)
        .create(path)
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    fs::set_permissions(path, Permissions::from_mode(0o700_u32))
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    let meta = fs::symlink_metadata(path).map_err(|_error| CommandBuildError::WorkDirIo)?;
    if meta.file_type().is_symlink()
        || !meta.is_dir()
        || (meta.permissions().mode() & 0o777_u32) != 0o700_u32
    {
        return Err(CommandBuildError::InvalidWorkRoot);
    }
    let mut entries = fs::read_dir(path).map_err(|_error| CommandBuildError::WorkDirIo)?;
    if entries.next().is_some() {
        return Err(CommandBuildError::InvalidWorkRoot);
    }
    Ok(())
}

fn materialize_mcp_config(
    config_dir: &Path,
    mcp_source: McpConfigSource<'_>,
    secret: Option<&SecretValue>,
) -> Result<PathBuf, CommandBuildError> {
    let target = config_dir.join("mcp.json");
    match mcp_source {
        McpConfigSource::Empty => {
            write_private_file(&target, "{\"mcpServers\":{}}\n")?;
            Ok(target)
        }
        McpConfigSource::Bridge(bridge) => {
            let rendered = bridge.render_mcp_json(secret)?;
            write_private_file(&target, &rendered)?;
            Ok(target)
        }
        McpConfigSource::ExistingFile(existing) => {
            let meta = fs::symlink_metadata(existing)
                .map_err(|_error| CommandBuildError::InvalidMcpConfigFile)?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(CommandBuildError::InvalidMcpConfigFile);
            }
            let content = fs::read_to_string(existing)
                .map_err(|_error| CommandBuildError::InvalidMcpConfigFile)?;
            verify_no_secret_in_text(&content, secret)?;
            Ok(existing.to_path_buf())
        }
    }
}

fn write_private_file(path: &Path, content: &str) -> Result<(), CommandBuildError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600_u32)
        .open(path)
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    file.write_all(content.as_bytes())
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    file.flush()
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    fs::set_permissions(path, Permissions::from_mode(0o600_u32))
        .map_err(|_error| CommandBuildError::WorkDirIo)?;
    let meta = fs::symlink_metadata(path).map_err(|_error| CommandBuildError::WorkDirIo)?;
    if meta.file_type().is_symlink() || (meta.permissions().mode() & 0o777_u32) != 0o600_u32 {
        return Err(CommandBuildError::InvalidMcpConfigFile);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenEnvInclusion {
    Included,
    Omitted,
}

pub struct CommandPlan {
    executable: PathBuf,
    args: Vec<String>,
    work_dir: PathBuf,
    env: Vec<(String, String)>,
    mcp_token_env: TokenEnvInclusion,
}

impl fmt::Debug for CommandPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env_keys = self.env_keys();
        f.debug_struct("CommandPlan")
            .field("executable", &self.executable)
            .field("args", &self.args)
            .field("work_dir", &self.work_dir)
            .field("env_keys", &env_keys)
            .finish_non_exhaustive()
    }
}

impl CommandPlan {
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    #[must_use]
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    #[must_use]
    pub fn env_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.env.iter().map(|(key, _)| key.as_str()).collect();
        if self.mcp_token_env == TokenEnvInclusion::Included {
            keys.push(MCP_TOKEN_ENV_KEY);
        }
        keys
    }

    #[must_use]
    pub fn wrapped_in_sandbox(mut self, layer: &crate::sandbox::SandboxLayer) -> Self {
        let (executable, args) = layer.wrap(&self.executable, &self.args);
        self.executable = executable;
        self.args = args;
        self
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PathBuf,
        Vec<String>,
        PathBuf,
        Vec<(String, String)>,
        TokenEnvInclusion,
    ) {
        (
            self.executable,
            self.args,
            self.work_dir,
            self.env,
            self.mcp_token_env,
        )
    }
}

pub struct CommandPlanInput<'a> {
    pub executable: &'a Path,
    pub work_dir: &'a Path,
    pub mcp_config_path: &'a Path,
    pub kind: RunKind,
    pub model: &'a ModelName,
    pub effort: ReasoningEffort,
    pub expected_mcp_server: &'a str,
    pub analysis_tools: &'a BTreeSet<String>,
    pub collection_json_schema: Option<&'a str>,
    pub host_env: Option<&'a HostEnvironment>,
    pub mcp_token: Option<&'a SecretValue>,
}

impl fmt::Debug for CommandPlanInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandPlanInput")
            .field("executable", &self.executable)
            .field("work_dir", &self.work_dir)
            .field("mcp_config_path", &self.mcp_config_path)
            .field("kind", &self.kind)
            .field("model", &self.model)
            .field("effort", &self.effort)
            .field("has_token", &self.mcp_token.is_some())
            .finish_non_exhaustive()
    }
}

pub fn build_command_plan(input: &CommandPlanInput<'_>) -> Result<CommandPlan, CommandBuildError> {
    let Some(host_env) = input.host_env else {
        return Err(CommandBuildError::MissingHostEnvironment);
    };
    let env = host_env.build_child_env(input.kind, input.mcp_token)?;
    let args = build_profile_args(input)?;
    verify_args_contain_no_secret(&args, input.mcp_token)?;
    let mcp_token_env = if input.kind == RunKind::Analysis && input.mcp_token.is_some() {
        TokenEnvInclusion::Included
    } else {
        TokenEnvInclusion::Omitted
    };
    Ok(CommandPlan {
        executable: input.executable.to_path_buf(),
        args,
        work_dir: input.work_dir.to_path_buf(),
        env,
        mcp_token_env,
    })
}

fn build_profile_args(input: &CommandPlanInput<'_>) -> Result<Vec<String>, CommandBuildError> {
    let mut args = vec![
        String::from("-p"),
        String::from("--verbose"),
        String::from("--input-format"),
        String::from("stream-json"),
        String::from("--output-format"),
        String::from("stream-json"),
        String::from("--include-partial-messages"),
        String::from("--restricted"),
        String::from("--disable-slash-commands"),
    ];
    append_tool_args(&mut args, input)?;
    args.push(String::from("--permission-mode"));
    args.push(String::from("dontAsk"));
    args.push(String::from("--permission-prompts"));
    args.push(String::from("none"));
    args.push(String::from("--mcp-config"));
    args.push(input.mcp_config_path.to_string_lossy().into_owned());
    args.push(String::from("--strict-mcp-config"));
    args.push(String::from("--no-session-persistence"));
    args.push(String::from("--model"));
    args.push(String::from(input.model.as_str()));
    if let Some(effort_arg) = effort_flag_value(input.effort) {
        args.push(String::from("--effort"));
        args.push(String::from(effort_arg));
    }
    if input.kind == RunKind::Collection
        && let Some(schema) = input.collection_json_schema
    {
        if schema.trim().is_empty() || serde_json::from_str::<serde_json::Value>(schema).is_err() {
            return Err(CommandBuildError::InvalidJsonSchema);
        }
        args.push(String::from("--json-schema"));
        args.push(String::from(schema));
    }
    Ok(args)
}

fn append_tool_args(
    args: &mut Vec<String>,
    input: &CommandPlanInput<'_>,
) -> Result<(), CommandBuildError> {
    match input.kind {
        RunKind::Analysis => {
            validate_analysis_tools(input.expected_mcp_server, input.analysis_tools)?;
            args.push(String::from("--tools"));
            args.push(String::new());
            args.push(String::from("--allowedTools"));
            args.extend(input.analysis_tools.iter().cloned());
        }
        RunKind::Collection => {
            args.push(String::from("--tools"));
            args.push(String::from("WebSearch"));
            args.push(String::from("--allowedTools"));
            args.push(String::from("WebSearch"));
        }
    }
    Ok(())
}

fn validate_analysis_tools(
    expected_mcp_server: &str,
    analysis_tools: &BTreeSet<String>,
) -> Result<(), CommandBuildError> {
    if !is_valid_server_name(expected_mcp_server) {
        return Err(CommandBuildError::InvalidMcpServerName);
    }
    if analysis_tools.is_empty() {
        return Err(CommandBuildError::InvalidAnalysisTools);
    }
    let prefix = format!("mcp__{expected_mcp_server}__");
    for tool in analysis_tools {
        let Some(suffix) = tool.strip_prefix(&prefix) else {
            return Err(CommandBuildError::InvalidAnalysisTools);
        };
        if suffix.is_empty() {
            return Err(CommandBuildError::InvalidAnalysisTools);
        }
    }
    Ok(())
}

fn verify_args_contain_no_secret(
    args: &[String],
    secret: Option<&SecretValue>,
) -> Result<(), CommandBuildError> {
    if let Some(secret_value) = secret {
        let token = validate_token_str(secret_value)?;
        if args.iter().any(|arg| arg.contains(token)) {
            return Err(CommandBuildError::SecretInConfig);
        }
    }
    Ok(())
}

const fn effort_flag_value(effort: ReasoningEffort) -> Option<&'static str> {
    match effort {
        ReasoningEffort::Default => None,
        ReasoningEffort::Low => Some("low"),
        ReasoningEffort::Medium => Some("medium"),
        ReasoningEffort::High => Some("high"),
        ReasoningEffort::XHigh => Some("xhigh"),
        ReasoningEffort::Max => Some("max"),
    }
}
