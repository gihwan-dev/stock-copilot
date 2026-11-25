mod attachment;
mod chat_session;
mod collection;
mod command;
mod config_dir;
mod decoder;
mod discovery;
mod error;
mod isolation;
mod parse;
mod process;
mod provider;
mod queue;
mod quota;
mod sandbox;

pub use attachment::{RunMcpAttachment, SpawnedPidNotice};
pub use chat_session::{
    ChatProcessReuse, ChatTurnConfig, ChatTurnOutput, CliRecordCleanup, PersistentChatSession,
    cleanup_cli_project_records, encode_claude_project_dir_name,
};
pub use config_dir::{CONFIG_DIR_ENV_KEY, ConfigDirSlot};

pub use collection::{
    ClaudeWebCollectionRunner, MAX_WEB_FINDINGS, WEB_COLLECTION_JSON_SCHEMA, parse_web_findings,
};
pub use command::{
    ALLOWED_HOST_ENV_KEYS, ApiKeyEnvState, CommandBuildError, CommandPlan, CommandPlanInput,
    FORBIDDEN_API_KEY_ENV_KEYS, HostEnvironment, MCP_TOKEN_ENV_KEY, McpBridgeConfig,
    McpConfigSource, NONESSENTIAL_TRAFFIC_ENV_KEY, RequiredEnvKey, RunWorkspace,
    build_command_plan, prepare_run_workspace,
};
pub use decoder::{ClaudeEventDecoder, decode_stream_lines, decode_stream_text};
pub use discovery::{
    ClaudeStartupReport, DEFAULT_PROBE_TIMEOUT_MS, DiscoveryConfig, DiscoveryOutcome,
    DiscoverySource, LoginCheckOutcome, LoginShellPathResolver, StaticPathResolver,
    SystemLoginShellPathResolver, check_login_status, check_login_status_with_timeout,
    default_known_locations, extract_marked_shell_path, find_claude_executable,
    run_claude_startup_check, run_claude_startup_check_with_timeout,
};
pub use error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError, ValidationFailure};
pub use isolation::IsolationExpectation;
pub use process::{MAX_STREAM_LINE_BYTES, ProcessSignal, send_signal_to_process_group};
pub use provider::{
    ClaudeCodeConfig, ClaudeCodeProvider, ClaudeEventStream, ModelDefaults, PREFLIGHT_MESSAGE,
};
pub use queue::{
    AiRunnerQueue, ChatSlotAdmission, ChatSlotGrant, QueueSnapshot, RunCredentials,
    ScheduledReleaseOutcome, ScheduledResumeKind, ScheduledSlotAdmission, ScheduledSlotGrant,
    generate_run_token, issue_unique_run_id,
};
pub use quota::{
    BudgetSummaryKind, DEFAULT_SCHEDULED_CAP_PERCENT, DEFAULT_UNKNOWN_RESET_COOLDOWN_SECONDS,
    QuotaConfigError, QuotaDecision, QuotaPolicy, QuotaState, QuotaTracker, QuotaTransition,
    ResetSource, WorkCategory, utilization_from_percent,
};
pub use sandbox::{
    ProbeExit, SYSTEM_SANDBOX_EXEC, SandboxLayer, SandboxProbeOutcome, SandboxProfile,
    SandboxProfileError, classify_sandbox_probe, probe_sandbox_layer,
};
