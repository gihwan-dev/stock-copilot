use crate::app_server::CodexAppServerDecoder;
use crate::isolation::CodexIsolationExpectation;
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, ChatHistory, ChatRole, CollectionQuery,
    InformationError, IsolationCheck, ModelName, ResetTime, ResponseMismatch, RunInput, RunKind,
    RunSpec,
};
use domain::UntrustedText;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, DirBuilder};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

pub const CODEX_ALLOWED_ENV_KEYS: [&str; 3] = ["HOME", "PATH", "USER"];
pub const CODEX_FORBIDDEN_ENV_KEYS: [&str; 2] = ["OPENAI_API_KEY", "CODEX_API_KEY"];
const MAX_LINE_BYTES: usize = 65_536_usize;
const STOP_GRACE_STEPS: u32 = 200_u32;

static WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(1_u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexActivation {
    LockedUntilS09,
    S09Passed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexApiKeyEnv {
    Absent,
    Present,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CodexHostEnvironment {
    home: String,
    path: String,
    user: String,
    api_key: CodexApiKeyEnv,
}

impl fmt::Debug for CodexHostEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodexHostEnvironment")
            .field("api_key", &self.api_key)
            .finish_non_exhaustive()
    }
}

impl CodexHostEnvironment {
    #[must_use]
    pub const fn new(home: String, path: String, user: String, api_key: CodexApiKeyEnv) -> Self {
        Self {
            home,
            path,
            user,
            api_key,
        }
    }

    #[must_use]
    pub fn from_pairs(pairs: &[(String, String)]) -> Option<Self> {
        let lookup = |key: &str| {
            pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        };
        let api_key = if CODEX_FORBIDDEN_ENV_KEYS
            .iter()
            .any(|key| lookup(key).is_some())
        {
            CodexApiKeyEnv::Present
        } else {
            CodexApiKeyEnv::Absent
        };
        Some(Self::new(
            lookup("HOME")?,
            lookup("PATH")?,
            lookup("USER")?,
            api_key,
        ))
    }

    fn child_env(&self) -> [(&'static str, String); 3] {
        [
            ("HOME", self.home.clone()),
            ("PATH", self.path.clone()),
            ("USER", self.user.clone()),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAnalysisProfile {
    pub expected_mcp_server: String,
    pub analysis_tools: BTreeSet<String>,
    pub mcp_server_config: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    executable: PathBuf,
    work_root: PathBuf,
    host_env: CodexHostEnvironment,
    activation: CodexActivation,
    expected_mcp_server: String,
    analysis_tools: BTreeSet<String>,
    mcp_server_config: Value,
    model: ModelName,
    allowed_cli_versions: Option<BTreeSet<String>>,
}

impl CodexProviderConfig {
    #[must_use]
    pub fn new(
        executable: PathBuf,
        work_root: PathBuf,
        host_env: CodexHostEnvironment,
        analysis: CodexAnalysisProfile,
        model: ModelName,
    ) -> Self {
        Self {
            executable,
            work_root,
            host_env,
            activation: CodexActivation::LockedUntilS09,
            expected_mcp_server: analysis.expected_mcp_server,
            analysis_tools: analysis.analysis_tools,
            mcp_server_config: analysis.mcp_server_config,
            model,
            allowed_cli_versions: None,
        }
    }

    #[must_use]
    pub const fn with_activation(mut self, activation: CodexActivation) -> Self {
        self.activation = activation;
        self
    }

    #[must_use]
    pub fn with_allowed_cli_versions(mut self, versions: BTreeSet<String>) -> Self {
        self.allowed_cli_versions = Some(versions);
        self
    }

    #[must_use]
    pub fn expectation_for(&self, kind: RunKind) -> CodexIsolationExpectation {
        match kind {
            RunKind::Analysis => CodexIsolationExpectation::new(
                RunKind::Analysis,
                self.analysis_tools.clone(),
                Some(self.expected_mcp_server.clone()),
                self.allowed_cli_versions.clone(),
            ),
            RunKind::Collection => CodexIsolationExpectation::new(
                RunKind::Collection,
                BTreeSet::new(),
                None,
                self.allowed_cli_versions.clone(),
            ),
        }
    }

    #[must_use]
    pub fn app_server_args(kind: RunKind) -> Vec<String> {
        let web_search = match kind {
            RunKind::Analysis => "web_search=\"disabled\"",
            RunKind::Collection => "web_search=\"cached\"",
        };
        [
            "app-server",
            "--disable",
            "shell_snapshot",
            "--disable",
            "hooks",
            "--disable",
            "multi_agent",
            "-c",
            "agents.enabled=false",
            "-c",
            "skills.include_instructions=false",
            "-c",
            "project_doc_max_bytes=0",
            "-c",
            "sandbox_mode=\"read-only\"",
            "-c",
            "approval_policy=\"never\"",
            "-c",
            web_search,
        ]
        .iter()
        .map(|arg| String::from(*arg))
        .collect()
    }

    fn thread_start_request(&self, kind: RunKind, work_dir: &Path) -> Value {
        let mcp_servers = match kind {
            RunKind::Analysis => {
                let mut servers = serde_json::Map::new();
                servers.insert(
                    self.expected_mcp_server.clone(),
                    self.mcp_server_config.clone(),
                );
                Value::Object(servers)
            }
            RunKind::Collection => json!({}),
        };
        json!({
            "jsonrpc": "2.0",
            "id": 1_i32,
            "method": "thread/start",
            "params": {
                "cwd": work_dir.to_string_lossy(),
                "model": self.model.as_str(),
                "ephemeral": true,
                "config": {"mcp_servers": mcp_servers}
            }
        })
    }
}

#[derive(Debug)]
pub struct CodexProvider {
    config: CodexProviderConfig,
}

impl CodexProvider {
    #[must_use]
    pub const fn new(config: CodexProviderConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> &CodexProviderConfig {
        &self.config
    }

    fn verify_now(&self, kind: RunKind) -> Result<IsolationCheck, InformationError> {
        let mut stream = self.start(kind, None)?;
        let first = stream.next_now()?;
        while stream.next_now()?.is_some() {}
        match first {
            Some(AiEvent::IsolationChecked(check)) => Ok(check),
            Some(_) | None => Err(invalid()),
        }
    }

    fn start(
        &self,
        kind: RunKind,
        prompt: Option<String>,
    ) -> Result<CodexEventStream, InformationError> {
        if self.config.activation == CodexActivation::LockedUntilS09
            || self.config.host_env.api_key == CodexApiKeyEnv::Present
        {
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        }
        let work_dir = create_work_dir(&self.config.work_root)
            .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
        let mut command = Command::new(&self.config.executable);
        command
            .args(CodexProviderConfig::app_server_args(kind))
            .current_dir(&work_dir)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0_i32);
        for (key, value) in self.config.host_env.child_env() {
            command.env(key, value);
        }
        let spawned = command.spawn();
        let Ok(mut child) = spawned else {
            remove_dir(&work_dir);
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        };
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            kill_group(&mut child);
            remove_dir(&work_dir);
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        };
        let pgid = child.id();
        let (tx, rx) = mpsc::sync_channel(64_usize);
        let session = ReaderSession {
            decoder: CodexAppServerDecoder::new(self.config.expectation_for(kind)),
            thread_start: self.config.thread_start_request(kind, &work_dir),
            prompt,
            model: self.config.model.clone(),
            tx,
        };
        let thread = std::thread::Builder::new()
            .name(String::from("codex-app-server-reader"))
            .spawn(move || finish_session(session, stdin, stdout, &mut child, &work_dir));
        if thread.is_err() {
            send_signal(pgid, "KILL");
            return Err(InformationError::Unavailable(ResetTime::NotProvided));
        }
        Ok(CodexEventStream {
            rx,
            pgid,
            closed: StreamClosed::No,
        })
    }
}

impl AiProvider for CodexProvider {
    type Stream = CodexEventStream;

    fn run(
        &self,
        spec: RunSpec,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        let prompt = match spec.input() {
            RunInput::Analysis(prompt) => String::from(prompt.as_str()),
            RunInput::Collection(query) => collection_prompt(query),
        };
        std::future::ready(self.start(spec.kind(), Some(prompt)))
    }

    fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        std::future::ready(self.start(spec.kind(), Some(resumed_prompt(&history, &message))))
    }

    fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
        std::future::ready(Ok(AiUsage::Unknown))
    }

    fn interrupt(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<(), InformationError>> + Send {
        std::future::ready(Ok(()))
    }

    fn verify_isolation(
        &self,
        kind: RunKind,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
        std::future::ready(self.verify_now(kind))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamClosed {
    No,
    Yes,
}

#[derive(Debug)]
enum ReaderMessage {
    Event(AiEvent),
    End,
    Failed(InformationError),
}

pub struct CodexEventStream {
    rx: mpsc::Receiver<ReaderMessage>,
    pgid: u32,
    closed: StreamClosed,
}

impl fmt::Debug for CodexEventStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodexEventStream")
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl Drop for CodexEventStream {
    fn drop(&mut self) {
        if self.closed == StreamClosed::No {
            send_signal(self.pgid, "KILL");
        }
    }
}

impl AiEventStream for CodexEventStream {
    fn next(&mut self) -> impl Future<Output = Result<Option<AiEvent>, InformationError>> + Send {
        std::future::ready(self.next_now())
    }
}

impl CodexEventStream {
    fn next_now(&mut self) -> Result<Option<AiEvent>, InformationError> {
        if self.closed == StreamClosed::Yes {
            return Ok(None);
        }
        match self.rx.recv() {
            Ok(ReaderMessage::Event(event)) => Ok(Some(event)),
            Ok(ReaderMessage::End) => {
                self.closed = StreamClosed::Yes;
                Ok(None)
            }
            Ok(ReaderMessage::Failed(error)) => {
                self.closed = StreamClosed::Yes;
                Err(error)
            }
            Err(_disconnected) => {
                self.closed = StreamClosed::Yes;
                Err(InformationError::Unavailable(ResetTime::NotProvided))
            }
        }
    }
}

struct ReaderSession {
    decoder: CodexAppServerDecoder,
    thread_start: Value,
    prompt: Option<String>,
    model: ModelName,
    tx: mpsc::SyncSender<ReaderMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitingIsolation,
    Streaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineStep {
    Continue,
    Stop(Option<InformationError>),
    ReceiverGone,
}

const fn isolation_of(event: &AiEvent) -> Option<IsolationCheck> {
    match event {
        AiEvent::IsolationChecked(check) => Some(*check),
        AiEvent::TextDelta(_)
        | AiEvent::ToolCall { .. }
        | AiEvent::WebSearch { .. }
        | AiEvent::TokenUsage(_)
        | AiEvent::Quota(_)
        | AiEvent::RateLimited { .. }
        | AiEvent::ModelRerouted(_)
        | AiEvent::ActualModel(_)
        | AiEvent::McpUnavailable
        | AiEvent::Unknown(_) => None,
    }
}

const fn invalid() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Validation)
}

const fn unavailable() -> InformationError {
    InformationError::Unavailable(ResetTime::NotProvided)
}

impl ReaderSession {
    fn run(
        mut self,
        stdin: ChildStdin,
        stdout: std::process::ChildStdout,
    ) -> (mpsc::SyncSender<ReaderMessage>, Option<ReaderMessage>) {
        let mut held = Some(stdin);
        let opening = [
            self.thread_start.clone(),
            json!({"jsonrpc": "2.0", "id": 2_i32, "method": "mcpServerStatus/list", "params": {}}),
        ];
        if opening
            .iter()
            .any(|request| write_request(held.as_mut(), request).is_err())
        {
            return (self.tx, Some(ReaderMessage::Failed(unavailable())));
        }
        let mut phase = Phase::AwaitingIsolation;
        let mut reader = BufReader::new(stdout);
        loop {
            let step = self.read_one(&mut reader, &mut phase, &mut held);
            match step {
                LineStep::Continue => {}
                LineStep::Stop(failure) => {
                    let message = match failure {
                        Some(error) => ReaderMessage::Failed(error),
                        None => ReaderMessage::End,
                    };
                    return (self.tx, Some(message));
                }
                LineStep::ReceiverGone => return (self.tx, None),
            }
        }
    }

    fn read_one(
        &mut self,
        reader: &mut BufReader<std::process::ChildStdout>,
        phase: &mut Phase,
        held: &mut Option<ChildStdin>,
    ) -> LineStep {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0_usize) => {
                return LineStep::Stop(self.decoder.clone().finish().err().map(|_e| invalid()));
            }
            Ok(_) if line.len() > MAX_LINE_BYTES => return LineStep::Stop(Some(invalid())),
            Ok(_) => {}
            Err(_error) => return LineStep::Stop(Some(unavailable())),
        }
        let Ok(events) = self.decoder.decode_line(&line) else {
            return LineStep::Stop(Some(invalid()));
        };
        for event in events {
            let step = self.forward(event, phase, held);
            if step != LineStep::Continue {
                return step;
            }
        }
        if is_turn_completed(&line) {
            *held = None;
        }
        LineStep::Continue
    }

    fn forward(
        &mut self,
        event: AiEvent,
        phase: &mut Phase,
        held: &mut Option<ChildStdin>,
    ) -> LineStep {
        let isolation = isolation_of(&event);
        if self.tx.send(ReaderMessage::Event(event)).is_err() {
            return LineStep::ReceiverGone;
        }
        match (*phase, isolation) {
            (Phase::AwaitingIsolation, Some(IsolationCheck::Passed)) => {
                *phase = Phase::Streaming;
                if self.send_turn(held) {
                    LineStep::Continue
                } else {
                    LineStep::Stop(None)
                }
            }
            (_, Some(IsolationCheck::Rejected(_))) => LineStep::Stop(None),
            (Phase::AwaitingIsolation | Phase::Streaming, _) => LineStep::Continue,
        }
    }

    fn send_turn(&mut self, held: &mut Option<ChildStdin>) -> bool {
        let Some(prompt) = self.prompt.take() else {
            return false;
        };
        let request = json!({
            "jsonrpc": "2.0",
            "id": 3_i32,
            "method": "turn/start",
            "params": {
                "input": [{"type": "text", "text": prompt}],
                "model": self.model.as_str()
            }
        });
        write_request(held.as_mut(), &request).is_ok()
    }
}

fn is_turn_completed(line: &str) -> bool {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(Value::as_str)
                .map(|method| method == "turn/completed")
        })
        .is_some_and(|completed| completed)
}

fn write_request(stdin: Option<&mut ChildStdin>, request: &Value) -> std::io::Result<()> {
    let Some(pipe) = stdin else {
        return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    };
    let encoded = serde_json::to_string(request)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    writeln!(pipe, "{encoded}")?;
    pipe.flush()
}

fn create_work_dir(root: &Path) -> std::io::Result<PathBuf> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700_u32)
        .create(root)?;
    let seq = WORKSPACE_SEQUENCE.fetch_add(1_u64, Ordering::Relaxed);
    let dir = root.join(format!("codex-work-{}-{seq}", std::process::id()));
    DirBuilder::new().mode(0o700_u32).create(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700_u32))?;
    Ok(dir)
}

fn remove_dir(dir: &Path) {
    let is_real_dir =
        fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink());
    if is_real_dir {
        match fs::remove_dir_all(dir) {
            Ok(()) | Err(_) => {}
        }
    }
}

fn finish_session(
    session: ReaderSession,
    stdin: ChildStdin,
    stdout: std::process::ChildStdout,
    child: &mut Child,
    work_dir: &Path,
) {
    let (sender, last) = session.run(stdin, stdout);
    stop_child(child);
    remove_dir(work_dir);
    if let Some(message) = last {
        match sender.send(message) {
            Ok(()) | Err(_) => {}
        }
    }
}

fn stop_child(child: &mut Child) {
    let (_keep, pause) = mpsc::channel::<()>();
    for _attempt in 0_u32..STOP_GRACE_STEPS {
        match child.try_wait() {
            Ok(Some(_status)) => {
                send_signal(child.id(), "KILL");
                return;
            }
            Ok(None) => match pause.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(()) | Err(_) => {}
            },
            Err(_error) => break,
        }
    }
    kill_group(child);
}

fn kill_group(child: &mut Child) {
    send_signal(child.id(), "KILL");
    match child.kill() {
        Ok(()) | Err(_) => {}
    }
    match child.wait() {
        Ok(_) | Err(_) => {}
    }
}

fn send_signal(pgid: u32, signal: &str) {
    if pgid == 0_u32 {
        return;
    }
    let target = format!("-{pgid}");
    let outcome = Command::new("/bin/kill")
        .env_clear()
        .args(["-s", signal, "--", &target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match outcome {
        Ok(_) | Err(_) => {}
    }
}

fn collection_prompt(query: &CollectionQuery) -> String {
    match query {
        CollectionQuery::CodeTopics {
            instruments,
            topics,
        } => {
            let symbols: Vec<&str> = instruments.iter().map(domain::Symbol::as_str).collect();
            let labels: Vec<String> = topics.iter().map(|topic| format!("{topic:?}")).collect();
            format!(
                "Search public web sources for symbols [{}] covering topics [{}]. Return structured JSON.",
                symbols.join(", "),
                labels.join(", ")
            )
        }
        CollectionQuery::UserMessage(stored) => String::from(stored.text().as_untrusted_str()),
    }
}

fn resumed_prompt(history: &ChatHistory, message: &UntrustedText) -> String {
    if history.messages().is_empty() {
        return String::from(message.as_untrusted_str());
    }
    let mut combined = String::from("<conversation_history>\n");
    for entry in history.messages() {
        combined.push_str(match entry.role() {
            ChatRole::User => "user: ",
            ChatRole::Assistant => "assistant: ",
        });
        combined.push_str(entry.text().as_untrusted_str());
        combined.push('\n');
    }
    combined.push_str("</conversation_history>\n");
    combined.push_str(message.as_untrusted_str());
    combined
}
