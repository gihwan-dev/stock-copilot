mod command;
mod decoder;
mod discovery;
mod error;
mod isolation;
mod parse;
mod process;
mod provider;

pub use command::{
    ALLOWED_HOST_ENV_KEYS, ApiKeyEnvState, CommandBuildError, CommandPlan, CommandPlanInput,
    FORBIDDEN_API_KEY_ENV_KEYS, HostEnvironment, MCP_TOKEN_ENV_KEY, McpBridgeConfig,
    McpConfigSource, NONESSENTIAL_TRAFFIC_ENV_KEY, RequiredEnvKey, RunWorkspace,
    build_command_plan, prepare_run_workspace,
};
pub use decoder::{ClaudeEventDecoder, decode_stream_lines, decode_stream_text};
pub use discovery::{
    ClaudeStartupReport, DiscoveryConfig, DiscoveryOutcome, DiscoverySource, LoginCheckOutcome,
    LoginShellPathResolver, StaticPathResolver, SystemLoginShellPathResolver, check_login_status,
    default_known_locations, find_claude_executable, run_claude_startup_check,
};
pub use error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError, ValidationFailure};
pub use isolation::IsolationExpectation;
pub use process::{MAX_STREAM_LINE_BYTES, ProcessSignal, send_signal_to_process_group};
pub use provider::{
    ClaudeCodeConfig, ClaudeCodeProvider, ClaudeEventStream, ModelDefaults, PREFLIGHT_MESSAGE,
};
