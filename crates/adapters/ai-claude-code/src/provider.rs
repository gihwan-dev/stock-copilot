use crate::command::{
    CommandBuildError, CommandPlan, CommandPlanInput, HostEnvironment, McpBridgeConfig,
    McpConfigSource, RunWorkspace, build_command_plan, prepare_run_workspace,
};
use crate::decoder::ClaudeEventDecoder;
use crate::isolation::IsolationExpectation;
use crate::process::{
    ActiveRunRegistry, ChildCancelGuard, ForceKill, SharedChild, SpawnSpec, map_line_read_failure,
    read_bounded_line, spawn_isolated_child, write_user_message_line,
};
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, ChatHistory, ChatRole, CollectionQuery,
    CollectionTopic, InformationError, IsolationCheck, ModelName, ModelSelection, ReasoningEffort,
    ResetTime, ResponseMismatch, RunInput, RunKind, RunSpec, SecretValue,
};
use domain::UntrustedText;
use std::collections::BTreeSet;
use std::fmt;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

const EVENT_CHANNEL_CAPACITY: usize = 64_usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDefaults {
    highest: ModelName,
    fast: ModelName,
}

impl ModelDefaults {
    #[must_use]
    pub fn new(highest: ModelName, fast: ModelName) -> Self {
        Self { highest, fast }
    }

    #[must_use]
    pub fn resolve(&self, selection: &ModelSelection) -> ModelName {
        match selection {
            ModelSelection::Default | ModelSelection::Highest => self.highest.clone(),
            ModelSelection::Fast => self.fast.clone(),
            ModelSelection::Named(model) => model.clone(),
        }
    }
}

pub struct ClaudeCodeConfig {
    executable_path: PathBuf,
    work_dir: PathBuf,
    expected_mcp_server: String,
    analysis_tools: BTreeSet<String>,
    allowed_builtin_plugins: BTreeSet<String>,
    allowed_cli_versions: Option<BTreeSet<String>>,
    host_env: Option<HostEnvironment>,
    mcp_bridge: Option<McpBridgeConfig>,
    mcp_config_path: Option<PathBuf>,
    mcp_token: Option<SecretValue>,
    collection_json_schema: Option<String>,
    model_defaults: ModelDefaults,
}

impl fmt::Debug for ClaudeCodeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeCodeConfig")
            .field("executable_path", &self.executable_path)
            .field("work_dir", &self.work_dir)
            .field("expected_mcp_server", &self.expected_mcp_server)
            .field("analysis_tool_count", &self.analysis_tools.len())
            .field("plugin_count", &self.allowed_builtin_plugins.len())
            .field("has_host_env", &self.host_env.is_some())
            .field("has_mcp_bridge", &self.mcp_bridge.is_some())
            .field("has_mcp_config", &self.mcp_config_path.is_some())
            .field("has_mcp_token", &self.mcp_token.is_some())
            .field(
                "has_collection_schema",
                &self.collection_json_schema.is_some(),
            )
            .field("model_defaults", &self.model_defaults)
            .finish_non_exhaustive()
    }
}

impl ClaudeCodeConfig {
    #[must_use]
    pub fn new(
        executable_path: PathBuf,
        work_dir: PathBuf,
        expected_mcp_server: String,
        analysis_tools: BTreeSet<String>,
        allowed_builtin_plugins: BTreeSet<String>,
    ) -> Option<Self> {
        let highest = ModelName::try_new("claude-opus-5-5").ok()?;
        let fast = ModelName::try_new("claude-haiku-5-5").ok()?;
        Some(Self {
            executable_path,
            work_dir,
            expected_mcp_server,
            analysis_tools,
            allowed_builtin_plugins,
            allowed_cli_versions: None,
            host_env: None,
            mcp_bridge: None,
            mcp_config_path: None,
            mcp_token: None,
            collection_json_schema: None,
            model_defaults: ModelDefaults::new(highest, fast),
        })
    }

    #[must_use]
    pub fn with_allowed_cli_versions(mut self, versions: BTreeSet<String>) -> Self {
        self.allowed_cli_versions = Some(versions);
        self
    }

    #[must_use]
    pub fn with_host_environment(mut self, host_env: HostEnvironment) -> Self {
        self.host_env = Some(host_env);
        self
    }

    #[must_use]
    pub fn with_mcp_bridge(mut self, bridge: McpBridgeConfig, token: SecretValue) -> Self {
        self.mcp_bridge = Some(bridge);
        self.mcp_config_path = None;
        self.mcp_token = Some(token);
        self
    }

    #[must_use]
    pub fn with_mcp_config(mut self, config_path: PathBuf, token: SecretValue) -> Self {
        self.mcp_config_path = Some(config_path);
        self.mcp_bridge = None;
        self.mcp_token = Some(token);
        self
    }

    #[must_use]
    pub fn with_collection_json_schema(mut self, schema: String) -> Self {
        self.collection_json_schema = Some(schema);
        self
    }

    #[must_use]
    pub fn with_model_defaults(mut self, defaults: ModelDefaults) -> Self {
        self.model_defaults = defaults;
        self
    }

    #[must_use]
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    #[must_use]
    pub fn expectation_for(&self, kind: RunKind) -> IsolationExpectation {
        match kind {
            RunKind::Analysis => IsolationExpectation::new(
                RunKind::Analysis,
                self.analysis_tools.clone(),
                Some(self.expected_mcp_server.clone()),
                self.allowed_builtin_plugins.clone(),
                self.allowed_cli_versions.clone(),
            ),
            RunKind::Collection => IsolationExpectation::new(
                RunKind::Collection,
                BTreeSet::from([String::from("WebSearch")]),
                None,
                self.allowed_builtin_plugins.clone(),
                self.allowed_cli_versions.clone(),
            ),
        }
    }

    pub fn preview_command_plan(
        &self,
        spec: &RunSpec,
        work_dir: &Path,
        mcp_config_path: &Path,
    ) -> Result<CommandPlan, CommandBuildError> {
        let model = self.model_defaults.resolve(spec.model());
        let input = CommandPlanInput {
            executable: &self.executable_path,
            work_dir,
            mcp_config_path,
            kind: spec.kind(),
            model: &model,
            effort: spec.effort(),
            expected_mcp_server: &self.expected_mcp_server,
            analysis_tools: &self.analysis_tools,
            collection_json_schema: self.collection_json_schema.as_deref(),
            host_env: self.host_env.as_ref(),
            mcp_token: self.mcp_token.as_ref(),
        };
        build_command_plan(&input)
    }

    fn mcp_source_for(&self, kind: RunKind) -> Result<McpConfigSource<'_>, CommandBuildError> {
        if kind == RunKind::Collection {
            return Ok(McpConfigSource::Empty);
        }
        if let Some(bridge) = self.mcp_bridge.as_ref() {
            if bridge.server_name() != self.expected_mcp_server {
                return Err(CommandBuildError::InvalidMcpServerName);
            }
            return Ok(McpConfigSource::Bridge(bridge));
        }
        if let Some(path) = self.mcp_config_path.as_deref() {
            return Ok(McpConfigSource::ExistingFile(path));
        }
        Err(CommandBuildError::MissingMcpConfigForAnalysis)
    }
}

pub struct ClaudeCodeProvider {
    config: ClaudeCodeConfig,
    active_runs: Arc<ActiveRunRegistry>,
    last_usage: Arc<Mutex<AiUsage>>,
}

impl fmt::Debug for ClaudeCodeProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeCodeProvider")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ClaudeCodeProvider {
    #[must_use]
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self {
            config,
            active_runs: ActiveRunRegistry::new(),
            last_usage: Arc::new(Mutex::new(AiUsage::Unknown)),
        }
    }

    #[must_use]
    pub const fn config(&self) -> &ClaudeCodeConfig {
        &self.config
    }

    fn prepare_spawn(
        &self,
        spec: &RunSpec,
    ) -> Result<(RunWorkspace, SpawnSpec), CommandBuildError> {
        let mcp_source = self.config.mcp_source_for(spec.kind())?;
        let token_for_workspace = match spec.kind() {
            RunKind::Analysis => self.config.mcp_token.as_ref(),
            RunKind::Collection => None,
        };
        let workspace = prepare_run_workspace(
            &self.config.work_dir,
            spec.id(),
            mcp_source,
            token_for_workspace,
        )?;
        let plan = self.config.preview_command_plan(
            spec,
            workspace.work_dir(),
            workspace.mcp_config_path(),
        )?;
        let (executable, args, work_dir, env) = plan.into_parts();
        let spawn_spec = SpawnSpec {
            executable,
            args,
            work_dir,
            env,
        };
        Ok((workspace, spawn_spec))
    }

    async fn spawn_stream(
        &self,
        spec: &RunSpec,
        prompt_text: String,
    ) -> Result<ClaudeEventStream, InformationError> {
        let (workspace, spawn_spec) = self
            .prepare_spawn(spec)
            .map_err(CommandBuildError::to_information_error)?;
        let spawned = spawn_isolated_child(&spawn_spec)
            .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
        let run_id_key = String::from(spec.id().as_str());
        self.active_runs
            .register(run_id_key.clone(), Arc::clone(&spawned.shared))
            .await;
        let expectation = self.config.expectation_for(spec.kind());
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let cancel_guard = ChildCancelGuard::new(Arc::clone(&spawned.shared));
        let reader_ctx = ReaderContext {
            run_id: run_id_key,
            shared: spawned.shared,
            stdin: spawned.stdin,
            stdout: spawned.stdout,
            prompt_text,
            expectation,
            active_runs: Arc::clone(&self.active_runs),
            last_usage: Arc::clone(&self.last_usage),
            tx,
            workspace,
        };
        let spawned_thread = std::thread::Builder::new()
            .name(String::from("claude-stream-reader"))
            .spawn(move || run_reader_loop(reader_ctx));
        if spawned_thread.is_err() {
            self.active_runs.unregister_blocking(spec.id().as_str());
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        }
        Ok(ClaudeEventStream {
            rx,
            finished: StreamState::Open,
            _cancel_guard: cancel_guard,
        })
    }
}

impl AiProvider for ClaudeCodeProvider {
    type Stream = ClaudeEventStream;

    async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
        let prompt_text = format_run_input(spec.input());
        self.spawn_stream(&spec, prompt_text).await
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        let prompt_text = format_resumed_prompt(&history, &message);
        self.spawn_stream(&spec, prompt_text).await
    }

    async fn usage(&self) -> Result<AiUsage, InformationError> {
        let guard = self.last_usage.lock().await;
        Ok(*guard)
    }

    async fn interrupt(&self, run: &AiRunId) -> Result<(), InformationError> {
        let _signaled = self.active_runs.interrupt(run.as_str()).await;
        Ok(())
    }

    async fn verify_isolation(&self, kind: RunKind) -> Result<IsolationCheck, InformationError> {
        let probe_id = AiRunId::try_new("isolation-probe")
            .map_err(|_error| InformationError::InvalidResponse(ResponseMismatch::Validation))?;
        let prompt = application::AnalysisPrompt::try_new("/usage")
            .map_err(|_error| InformationError::InvalidResponse(ResponseMismatch::Validation))?;
        let input = match kind {
            RunKind::Analysis => RunInput::Analysis(prompt),
            RunKind::Collection => RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: Vec::new(),
                topics: Vec::new(),
            }),
        };
        let probe_spec = RunSpec::new(
            probe_id,
            input,
            ModelSelection::Default,
            ReasoningEffort::Default,
            application::RunPurpose::WebCollection,
        );
        let mut stream = self
            .spawn_stream(&probe_spec, String::from("/usage"))
            .await?;
        match stream.next().await? {
            Some(AiEvent::IsolationChecked(check)) => Ok(check),
            Some(_) | None => Err(InformationError::InvalidResponse(
                ResponseMismatch::Validation,
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamItem {
    End,
    Error(InformationError),
}

#[derive(Debug)]
enum ChannelMessage {
    Event(AiEvent),
    Control(StreamItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamState {
    Open,
    Closed,
}

pub struct ClaudeEventStream {
    rx: mpsc::Receiver<ChannelMessage>,
    finished: StreamState,
    _cancel_guard: ChildCancelGuard,
}

impl fmt::Debug for ClaudeEventStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeEventStream")
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl AiEventStream for ClaudeEventStream {
    async fn next(&mut self) -> Result<Option<AiEvent>, InformationError> {
        if self.finished == StreamState::Closed {
            return Ok(None);
        }
        match self.rx.recv().await {
            Some(ChannelMessage::Event(event)) => {
                if matches_rejected(&event) {
                    self.finished = StreamState::Closed;
                }
                Ok(Some(event))
            }
            Some(ChannelMessage::Control(StreamItem::End)) | None => {
                self.finished = StreamState::Closed;
                Ok(None)
            }
            Some(ChannelMessage::Control(StreamItem::Error(error))) => {
                self.finished = StreamState::Closed;
                Err(error)
            }
        }
    }
}

const fn matches_rejected(event: &AiEvent) -> bool {
    match event {
        AiEvent::IsolationChecked(IsolationCheck::Rejected(_)) => true,
        AiEvent::IsolationChecked(IsolationCheck::Passed)
        | AiEvent::TextDelta(_)
        | AiEvent::ToolCall { .. }
        | AiEvent::WebSearch { .. }
        | AiEvent::TokenUsage(_)
        | AiEvent::Quota(_)
        | AiEvent::RateLimited { .. }
        | AiEvent::ModelRerouted(_)
        | AiEvent::ActualModel(_)
        | AiEvent::McpUnavailable
        | AiEvent::Unknown(_) => false,
    }
}

struct ReaderContext {
    run_id: String,
    shared: Arc<SharedChild>,
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    prompt_text: String,
    expectation: IsolationExpectation,
    active_runs: Arc<ActiveRunRegistry>,
    last_usage: Arc<Mutex<AiUsage>>,
    tx: mpsc::Sender<ChannelMessage>,
    workspace: RunWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RateLimitSeen {
    No,
    Yes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    NaturalEof,
    Rejected,
    DecodeError(InformationError),
    ReceiverClosed,
}

fn run_reader_loop(ctx: ReaderContext) {
    let ReaderContext {
        run_id,
        shared,
        mut stdin,
        stdout,
        prompt_text,
        expectation,
        active_runs,
        last_usage,
        tx,
        workspace,
    } = ctx;
    let write_outcome = write_user_message_line(&mut stdin, &prompt_text);
    let stdin_failed = match write_outcome {
        Ok(()) => false,
        Err(error) => error.kind() != std::io::ErrorKind::BrokenPipe,
    };
    {
        let _closed_stdin = stdin;
    }
    let mut decoder = ClaudeEventDecoder::new(expectation);
    let mut reader = BufReader::new(stdout);
    let mut rate_limit_seen = RateLimitSeen::No;
    let stop = if stdin_failed {
        StopReason::DecodeError(InformationError::Unavailable(ResetTime::NotProvided))
    } else {
        decode_stdout_loop(
            &mut reader,
            &mut decoder,
            &last_usage,
            &tx,
            &mut rate_limit_seen,
        )
    };
    {
        let _closed_reader = reader;
    }
    let force_kill = match stop {
        StopReason::NaturalEof => ForceKill::No,
        StopReason::Rejected | StopReason::DecodeError(_) | StopReason::ReceiverClosed => {
            ForceKill::Yes
        }
    };
    let exit_status = shared.reap_blocking(force_kill);
    active_runs.unregister_blocking(&run_id);
    workspace.cleanup();
    finalize_stream(stop, decoder, exit_status, rate_limit_seen, &tx);
}

fn decode_stdout_loop(
    reader: &mut BufReader<std::process::ChildStdout>,
    decoder: &mut ClaudeEventDecoder,
    last_usage: &Mutex<AiUsage>,
    tx: &mpsc::Sender<ChannelMessage>,
    rate_limit_seen: &mut RateLimitSeen,
) -> StopReason {
    loop {
        let maybe_line = match map_line_read_failure(read_bounded_line(reader)) {
            Ok(line) => line,
            Err(error) => return StopReason::DecodeError(error.to_information_error()),
        };
        let Some(line) = maybe_line else {
            return StopReason::NaturalEof;
        };
        let events = match decoder.decode_line(&line) {
            Ok(items) => items,
            Err(error) => return StopReason::DecodeError(error.to_information_error()),
        };
        for event in events {
            record_usage_observation(&event, last_usage, rate_limit_seen);
            let is_rejected = matches_rejected(&event);
            if tx.blocking_send(ChannelMessage::Event(event)).is_err() {
                return StopReason::ReceiverClosed;
            }
            if is_rejected {
                return StopReason::Rejected;
            }
        }
    }
}

fn record_usage_observation(
    event: &AiEvent,
    last_usage: &Mutex<AiUsage>,
    rate_limit_seen: &mut RateLimitSeen,
) {
    match event {
        AiEvent::Quota(usage) => {
            let mut guard = last_usage.blocking_lock();
            if let (
                AiUsage::Waiting(ResetTime::Known(existing)),
                AiUsage::Waiting(ResetTime::NotProvided),
            ) = (*guard, *usage)
            {
                *guard = AiUsage::Waiting(ResetTime::Known(existing));
            } else {
                *guard = *usage;
            }
            if let AiUsage::Waiting(_) = usage {
                *rate_limit_seen = RateLimitSeen::Yes;
            }
        }
        AiEvent::RateLimited { resets_at } => {
            *rate_limit_seen = RateLimitSeen::Yes;
            let mut guard = last_usage.blocking_lock();
            match (*guard, *resets_at) {
                (AiUsage::Waiting(ResetTime::Known(_)), ResetTime::NotProvided) => {}
                (_, next_reset) => {
                    *guard = AiUsage::Waiting(next_reset);
                }
            }
        }
        AiEvent::IsolationChecked(_)
        | AiEvent::TextDelta(_)
        | AiEvent::ToolCall { .. }
        | AiEvent::WebSearch { .. }
        | AiEvent::TokenUsage(_)
        | AiEvent::ModelRerouted(_)
        | AiEvent::ActualModel(_)
        | AiEvent::McpUnavailable
        | AiEvent::Unknown(_) => {}
    }
}

fn finalize_stream(
    stop: StopReason,
    decoder: ClaudeEventDecoder,
    exit_status: Option<std::process::ExitStatus>,
    rate_limit_seen: RateLimitSeen,
    tx: &mpsc::Sender<ChannelMessage>,
) {
    let final_message = match stop {
        StopReason::ReceiverClosed => return,
        StopReason::Rejected => ChannelMessage::Control(StreamItem::End),
        StopReason::DecodeError(error) => ChannelMessage::Control(StreamItem::Error(error)),
        StopReason::NaturalEof => finalize_natural_eof(decoder, exit_status, rate_limit_seen),
    };
    match tx.blocking_send(final_message) {
        Ok(()) | Err(_) => {}
    }
}

fn finalize_natural_eof(
    decoder: ClaudeEventDecoder,
    exit_status: Option<std::process::ExitStatus>,
    rate_limit_seen: RateLimitSeen,
) -> ChannelMessage {
    if let Err(error) = decoder.finish() {
        return ChannelMessage::Control(StreamItem::Error(error.to_information_error()));
    }
    let exit_ok = exit_status.is_some_and(|status| status.success());
    if exit_ok || rate_limit_seen == RateLimitSeen::Yes {
        ChannelMessage::Control(StreamItem::End)
    } else {
        ChannelMessage::Control(StreamItem::Error(InformationError::Unavailable(
            ResetTime::NotProvided,
        )))
    }
}

fn format_run_input(input: &RunInput) -> String {
    match input {
        RunInput::Analysis(prompt) => String::from(prompt.as_str()),
        RunInput::Collection(query) => format_collection_query(query),
    }
}

fn format_collection_query(query: &CollectionQuery) -> String {
    match query {
        CollectionQuery::CodeTopics {
            instruments,
            topics,
        } => {
            let symbols: Vec<&str> = instruments.iter().map(domain::Symbol::as_str).collect();
            let labels: Vec<&str> = topics.iter().map(|topic| topic_label(*topic)).collect();
            format!(
                "Search public web sources for symbols [{}] covering topics [{}]. Return structured JSON.",
                symbols.join(", "),
                labels.join(", ")
            )
        }
        CollectionQuery::UserMessage(stored) => String::from(stored.text().as_untrusted_str()),
    }
}

const fn topic_label(topic: CollectionTopic) -> &'static str {
    match topic {
        CollectionTopic::CompanyNews => "company_news",
        CollectionTopic::Earnings => "earnings",
        CollectionTopic::Disclosures => "disclosures",
    }
}

fn format_resumed_prompt(history: &ChatHistory, message: &UntrustedText) -> String {
    if history.messages().is_empty() {
        return String::from(message.as_untrusted_str());
    }
    let mut combined = String::from("<conversation_history>\n");
    for entry in history.messages() {
        let role_label = match entry.role() {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        };
        combined.push_str(role_label);
        combined.push_str(": ");
        combined.push_str(entry.text().as_untrusted_str());
        combined.push('\n');
    }
    combined.push_str("</conversation_history>\n");
    combined.push_str(message.as_untrusted_str());
    combined
}
