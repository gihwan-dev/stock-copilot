use crate::command::{CommandBuildError, RunWorkspace, TokenEnvInclusion};
use crate::decoder::{ClaudeEventDecoder, DecodedLineKind};
use crate::isolation::IsolationExpectation;
use crate::process::{
    ChildCancelGuard, SharedChild, map_line_read_failure, read_bounded_line, spawn_isolated_child,
    write_user_message_line,
};
use crate::provider::{ClaudeCodeProvider, PREFLIGHT_MESSAGE, format_resumed_prompt};
use application::{
    AiEvent, AiRunId, AnalysisPrompt, ChatHistory, InformationError, IsolationCheck, ModelName,
    ModelSelection, ReasoningEffort, ResetTime, ResponseMismatch, RunInput, RunKind, RunPurpose,
    RunSpec, TokenUsage,
};
use domain::UntrustedText;
use std::fmt;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use tokio::sync::{Mutex, oneshot};

const SHUTDOWN_REAP_GRACE_MS: u64 = 250_u64;

static CHAT_RUN_SEQ: AtomicU64 = AtomicU64::new(1_u64);

#[must_use]
pub fn encode_claude_project_dir_name(work_dir: &Path) -> String {
    work_dir
        .to_string_lossy()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliRecordCleanup {
    Removed,
    NotPresent,
    RejectedUnsafePath,
}

#[must_use]
pub fn cleanup_cli_project_records(
    projects_root: &Path,
    work_dirs: &[PathBuf],
) -> CliRecordCleanup {
    let Ok(root_meta) = fs::symlink_metadata(projects_root) else {
        return CliRecordCleanup::NotPresent;
    };
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return CliRecordCleanup::RejectedUnsafePath;
    }
    let mut removed_any = false;
    for work_dir in work_dirs {
        let encoded = encode_claude_project_dir_name(work_dir);
        if !is_safe_single_dir_name(&encoded) {
            return CliRecordCleanup::RejectedUnsafePath;
        }
        let candidate = projects_root.join(&encoded);
        let Ok(meta) = fs::symlink_metadata(&candidate) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            return CliRecordCleanup::RejectedUnsafePath;
        }
        if meta.is_dir() && fs::remove_dir_all(&candidate).is_ok() {
            removed_any = true;
        }
    }
    if removed_any {
        CliRecordCleanup::Removed
    } else {
        CliRecordCleanup::NotPresent
    }
}

fn is_safe_single_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurnConfig {
    pub model: ModelSelection,
    pub effort: ReasoningEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatProcessReuse {
    ReusedLiveProcess,
    StartedFreshWithHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurnOutput {
    pub run_id: AiRunId,
    pub reuse: ChatProcessReuse,
    pub events: Vec<AiEvent>,
    pub actual_model: Option<ModelName>,
    pub rerouted_models: Vec<ModelName>,
    pub token_usage: Option<TokenUsage>,
    pub rate_limited: Option<ResetTime>,
}

enum WorkerCommand {
    SendTurn {
        prompt: String,
        reply: oneshot::Sender<TurnWorkerOutcome>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

enum TurnWorkerOutcome {
    Completed(TurnPayload),
    ProcessExitedBeforeResult,
    Failed(InformationError),
}

struct TurnPayload {
    events: Vec<AiEvent>,
    actual_model: Option<ModelName>,
    rerouted_models: Vec<ModelName>,
    token_usage: Option<TokenUsage>,
    rate_limited: Option<ResetTime>,
}

struct LiveChatProcess {
    run_id: AiRunId,
    config: ChatTurnConfig,
    shared: Arc<SharedChild>,
    cmd_tx: std_mpsc::Sender<WorkerCommand>,
    _cancel_guard: ChildCancelGuard,
}

impl LiveChatProcess {
    async fn shutdown(self) {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(WorkerCommand::Shutdown { reply: tx })
            .is_ok()
        {
            match rx.await {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

pub struct PersistentChatSession {
    provider: Arc<ClaudeCodeProvider>,
    cli_projects_dir: Option<PathBuf>,
    live: Mutex<Option<LiveChatProcess>>,
}

impl fmt::Debug for PersistentChatSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistentChatSession")
            .field("has_cli_projects_dir", &self.cli_projects_dir.is_some())
            .finish_non_exhaustive()
    }
}

impl PersistentChatSession {
    #[must_use]
    pub fn new(provider: Arc<ClaudeCodeProvider>) -> Self {
        let default_projects = provider
            .config()
            .host_env_home()
            .map(|home| PathBuf::from(home).join(".claude").join("projects"));
        Self {
            provider,
            cli_projects_dir: default_projects,
            live: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn with_cli_projects_dir(mut self, dir: PathBuf) -> Self {
        self.cli_projects_dir = Some(dir);
        self
    }

    pub async fn send_turn(
        &self,
        history: &ChatHistory,
        message: &UntrustedText,
        config: ChatTurnConfig,
    ) -> Result<ChatTurnOutput, InformationError> {
        let mut guard = self.live.lock().await;
        if let Some(existing) = guard.as_ref()
            && existing.config == config
        {
            let run_id = existing.run_id.clone();
            let worker_outcome =
                dispatch_turn_to_worker(&existing.cmd_tx, message.as_untrusted_str()).await;
            match worker_outcome {
                TurnWorkerOutcome::Completed(payload) => {
                    return Ok(build_turn_output(
                        run_id,
                        ChatProcessReuse::ReusedLiveProcess,
                        payload,
                    ));
                }
                TurnWorkerOutcome::ProcessExitedBeforeResult => {
                    shutdown_slot(&mut guard).await;
                }
                TurnWorkerOutcome::Failed(error) => {
                    shutdown_slot(&mut guard).await;
                    return Err(error);
                }
            }
        } else {
            shutdown_slot(&mut guard).await;
        }
        let fresh = self.spawn_live_process(config).await?;
        let run_id = fresh.run_id.clone();
        let resumed_prompt = format_resumed_prompt(history, message);
        let outcome = dispatch_turn_to_worker(&fresh.cmd_tx, &resumed_prompt).await;
        match outcome {
            TurnWorkerOutcome::Completed(payload) => {
                *guard = Some(fresh);
                Ok(build_turn_output(
                    run_id,
                    ChatProcessReuse::StartedFreshWithHistory,
                    payload,
                ))
            }
            TurnWorkerOutcome::ProcessExitedBeforeResult => {
                fresh.shutdown().await;
                Err(InformationError::Unavailable(ResetTime::NotProvided))
            }
            TurnWorkerOutcome::Failed(error) => {
                fresh.shutdown().await;
                Err(error)
            }
        }
    }

    pub async fn interrupt(&self) {
        let target = {
            let guard = self.live.lock().await;
            guard.as_ref().map(|proc_ref| Arc::clone(&proc_ref.shared))
        };
        if let Some(shared) = target {
            let _signaled = shared.send_interrupt().await;
        }
    }

    pub async fn close(&self) {
        let existing = {
            let mut guard = self.live.lock().await;
            guard.take()
        };
        if let Some(proc_handle) = existing {
            proc_handle.shutdown().await;
        }
    }

    async fn spawn_live_process(
        &self,
        config: ChatTurnConfig,
    ) -> Result<LiveChatProcess, InformationError> {
        let seq = CHAT_RUN_SEQ.fetch_add(1_u64, Ordering::Relaxed);
        let run_id = AiRunId::try_new(&format!("chat-proc-{seq}"))
            .map_err(|_error| InformationError::InvalidResponse(ResponseMismatch::Validation))?;
        let prompt = AnalysisPrompt::try_new(PREFLIGHT_MESSAGE)
            .map_err(|_error| InformationError::InvalidResponse(ResponseMismatch::Validation))?;
        let spec = RunSpec::new(
            run_id.clone(),
            RunInput::Analysis(prompt),
            config.model.clone(),
            config.effort,
            RunPurpose::Chat,
        );
        let (workspace, spawn_spec, token_inclusion) = self
            .provider
            .prepare_spawn(&spec)
            .map_err(CommandBuildError::to_information_error)?;
        let token_for_child = match token_inclusion {
            TokenEnvInclusion::Included => self.provider.config().mcp_token(),
            TokenEnvInclusion::Omitted => None,
        };
        let spawned = spawn_isolated_child(&spawn_spec, token_for_child)
            .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
        let expectation = self.provider.config().expectation_for(RunKind::Analysis);
        let work_dirs_for_cleanup = collect_work_dir_variants(workspace.work_dir());
        let (cmd_tx, cmd_rx) = std_mpsc::channel::<WorkerCommand>();
        let (preflight_tx, preflight_rx) = oneshot::channel::<Result<(), InformationError>>();
        let shared_for_fallback = Arc::clone(&spawned.shared);
        let cancel_guard = ChildCancelGuard::new(Arc::clone(&spawned.shared));
        let worker_ctx = ChatWorkerContext {
            shared: Arc::clone(&spawned.shared),
            stdin: spawned.stdin,
            stdout: spawned.stdout,
            expectation,
            workspace,
            cli_projects_dir: self.cli_projects_dir.clone(),
            work_dirs_for_cleanup,
            cmd_rx,
            preflight_tx,
        };
        let spawned_thread = std::thread::Builder::new()
            .name(String::from("claude-chat-session"))
            .spawn(move || run_chat_worker(worker_ctx));
        if spawned_thread.is_err() {
            let _reaped = shared_for_fallback.force_kill_and_reap().await;
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        }
        let preflight = preflight_rx
            .await
            .map_err(|_closed| InformationError::Unavailable(ResetTime::NotProvided))?;
        preflight?;
        Ok(LiveChatProcess {
            run_id,
            config,
            shared: spawned.shared,
            cmd_tx,
            _cancel_guard: cancel_guard,
        })
    }
}

async fn shutdown_slot(slot: &mut Option<LiveChatProcess>) {
    if let Some(existing) = slot.take() {
        existing.shutdown().await;
    }
}

fn collect_work_dir_variants(work_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![work_dir.to_path_buf()];
    if let Ok(canonical) = fs::canonicalize(work_dir)
        && canonical != work_dir
    {
        dirs.push(canonical);
    }
    dirs
}

async fn dispatch_turn_to_worker(
    cmd_tx: &std_mpsc::Sender<WorkerCommand>,
    prompt: &str,
) -> TurnWorkerOutcome {
    let (reply_tx, reply_rx) = oneshot::channel();
    if cmd_tx
        .send(WorkerCommand::SendTurn {
            prompt: String::from(prompt),
            reply: reply_tx,
        })
        .is_err()
    {
        return TurnWorkerOutcome::ProcessExitedBeforeResult;
    }
    match reply_rx.await {
        Ok(outcome) => outcome,
        Err(_) => TurnWorkerOutcome::ProcessExitedBeforeResult,
    }
}

fn build_turn_output(
    run_id: AiRunId,
    reuse: ChatProcessReuse,
    payload: TurnPayload,
) -> ChatTurnOutput {
    ChatTurnOutput {
        run_id,
        reuse,
        events: payload.events,
        actual_model: payload.actual_model,
        rerouted_models: payload.rerouted_models,
        token_usage: payload.token_usage,
        rate_limited: payload.rate_limited,
    }
}

struct ChatWorkerContext {
    shared: Arc<SharedChild>,
    stdin: ChildStdin,
    stdout: ChildStdout,
    expectation: IsolationExpectation,
    workspace: RunWorkspace,
    cli_projects_dir: Option<PathBuf>,
    work_dirs_for_cleanup: Vec<PathBuf>,
    cmd_rx: std_mpsc::Receiver<WorkerCommand>,
    preflight_tx: oneshot::Sender<Result<(), InformationError>>,
}

fn run_chat_worker(mut ctx: ChatWorkerContext) {
    let mut decoder = ClaudeEventDecoder::new(ctx.expectation);
    let mut reader = BufReader::new(ctx.stdout);
    let preflight_result = execute_chat_preflight(&mut ctx.stdin, &mut reader, &mut decoder);
    let preflight_ok = preflight_result.is_ok();
    match ctx.preflight_tx.send(preflight_result) {
        Ok(()) | Err(_) => {}
    }
    let shutdown_reply = if preflight_ok {
        serve_chat_turns(&mut ctx.stdin, &mut reader, &mut decoder, &ctx.cmd_rx)
    } else {
        None
    };
    {
        let _closed_stdin = ctx.stdin;
        let _closed_reader = reader;
    }
    let _reaped = ctx.shared.reap_with_grace_blocking(SHUTDOWN_REAP_GRACE_MS);
    ctx.workspace.cleanup();
    if let Some(projects_dir) = &ctx.cli_projects_dir {
        let _cleaned = cleanup_cli_project_records(projects_dir, &ctx.work_dirs_for_cleanup);
    }
    if let Some(reply) = shutdown_reply {
        match reply.send(()) {
            Ok(()) | Err(()) => {}
        }
    }
}

fn execute_chat_preflight(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    decoder: &mut ClaudeEventDecoder,
) -> Result<(), InformationError> {
    write_user_message_line(stdin, PREFLIGHT_MESSAGE)
        .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
    let init_line = map_line_read_failure(read_bounded_line(reader))
        .map_err(|error| error.to_information_error())?
        .ok_or(InformationError::InvalidResponse(
            ResponseMismatch::Validation,
        ))?;
    let init_events = decoder
        .decode_line(&init_line)
        .map_err(|error| error.to_information_error())?;
    let Some(AiEvent::IsolationChecked(IsolationCheck::Passed)) = init_events.first() else {
        return Err(InformationError::InvalidResponse(
            ResponseMismatch::Validation,
        ));
    };
    drain_preflight_zero_turn_result(reader, decoder)
}

fn drain_preflight_zero_turn_result(
    reader: &mut BufReader<ChildStdout>,
    decoder: &mut ClaudeEventDecoder,
) -> Result<(), InformationError> {
    for _step in 0_usize..16_usize {
        let line = map_line_read_failure(read_bounded_line(reader))
            .map_err(|error| error.to_information_error())?
            .ok_or(InformationError::InvalidResponse(
                ResponseMismatch::Validation,
            ))?;
        let _events = decoder
            .decode_line(&line)
            .map_err(|error| error.to_information_error())?;
        if decoder.last_line_kind() == DecodedLineKind::Result {
            return if decoder.last_result_num_turns() == Some(0_u64) {
                Ok(())
            } else {
                Err(InformationError::InvalidResponse(
                    ResponseMismatch::Validation,
                ))
            };
        }
    }
    Err(InformationError::InvalidResponse(
        ResponseMismatch::Validation,
    ))
}

fn serve_chat_turns(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    decoder: &mut ClaudeEventDecoder,
    cmd_rx: &std_mpsc::Receiver<WorkerCommand>,
) -> Option<oneshot::Sender<()>> {
    while let Ok(command) = cmd_rx.recv() {
        match command {
            WorkerCommand::Shutdown { reply } => {
                return Some(reply);
            }
            WorkerCommand::SendTurn { prompt, reply } => {
                let outcome = execute_single_chat_turn(stdin, reader, decoder, &prompt);
                let should_stop = !matches_completed(&outcome);
                match reply.send(outcome) {
                    Ok(()) | Err(_) => {}
                }
                if should_stop {
                    return None;
                }
            }
        }
    }
    None
}

const fn matches_completed(outcome: &TurnWorkerOutcome) -> bool {
    match outcome {
        TurnWorkerOutcome::Completed(_) => true,
        TurnWorkerOutcome::ProcessExitedBeforeResult | TurnWorkerOutcome::Failed(_) => false,
    }
}

fn execute_single_chat_turn(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    decoder: &mut ClaudeEventDecoder,
    prompt: &str,
) -> TurnWorkerOutcome {
    if write_user_message_line(stdin, prompt).is_err() {
        return TurnWorkerOutcome::ProcessExitedBeforeResult;
    }
    let mut events = Vec::new();
    let mut actual_model = None;
    let mut rerouted_models = Vec::new();
    let mut token_usage = None;
    let mut rate_limited = None;
    loop {
        let maybe_line = match map_line_read_failure(read_bounded_line(reader)) {
            Ok(line) => line,
            Err(error) => return TurnWorkerOutcome::Failed(error.to_information_error()),
        };
        let Some(line) = maybe_line else {
            return TurnWorkerOutcome::ProcessExitedBeforeResult;
        };
        let line_events = match decoder.decode_line(&line) {
            Ok(items) => items,
            Err(error) => return TurnWorkerOutcome::Failed(error.to_information_error()),
        };
        for event in line_events {
            record_turn_metadata(
                &event,
                &mut actual_model,
                &mut rerouted_models,
                &mut token_usage,
                &mut rate_limited,
            );
            events.push(event);
        }
        if decoder.last_line_kind() == DecodedLineKind::Result {
            return TurnWorkerOutcome::Completed(TurnPayload {
                events,
                actual_model,
                rerouted_models,
                token_usage,
                rate_limited,
            });
        }
    }
}

fn record_turn_metadata(
    event: &AiEvent,
    actual_model: &mut Option<ModelName>,
    rerouted_models: &mut Vec<ModelName>,
    token_usage: &mut Option<TokenUsage>,
    rate_limited: &mut Option<ResetTime>,
) {
    match event {
        AiEvent::ActualModel(model) => *actual_model = Some(model.clone()),
        AiEvent::ModelRerouted(model) => rerouted_models.push(model.clone()),
        AiEvent::TokenUsage(usage) => *token_usage = Some(usage.clone()),
        AiEvent::RateLimited { resets_at } => *rate_limited = Some(*resets_at),
        AiEvent::IsolationChecked(_)
        | AiEvent::TextDelta(_)
        | AiEvent::ToolCall { .. }
        | AiEvent::WebSearch { .. }
        | AiEvent::Quota(_)
        | AiEvent::McpUnavailable
        | AiEvent::Unknown(_) => {}
    }
}
