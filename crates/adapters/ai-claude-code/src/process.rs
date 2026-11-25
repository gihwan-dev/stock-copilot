use crate::error::StreamDecodeError;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

pub const MAX_STREAM_LINE_BYTES: usize = 65_536_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSignal {
    Interrupt,
    Terminate,
    Kill,
}

impl ProcessSignal {
    const fn kill_arg(self) -> &'static str {
        match self {
            Self::Interrupt => "INT",
            Self::Terminate => "TERM",
            Self::Kill => "KILL",
        }
    }
}

pub fn send_signal_to_process_group(pgid: u32, signal: ProcessSignal) {
    if pgid == 0_u32 {
        return;
    }
    let target = format!("-{pgid}");
    let outcome = Command::new("/bin/kill")
        .env_clear()
        .args(["-s", signal.kill_arg(), "--", &target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match outcome {
        Ok(_) | Err(_) => {}
    }
}

#[derive(Debug)]
enum ChildState {
    Running { child: Child, pgid: u32 },
    Reaped(ExitStatus),
    ReapFailed,
}

pub(crate) struct SharedChild {
    cancel_requested: AtomicBool,
    interrupted: AtomicBool,
    state: Mutex<ChildState>,
}

impl fmt::Debug for SharedChild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedChild")
            .field(
                "cancel_requested",
                &self.cancel_requested.load(Ordering::Acquire),
            )
            .field("interrupted", &self.interrupted.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl SharedChild {
    pub(crate) fn new(child: Child) -> Arc<Self> {
        let pgid = child.id();
        Arc::new(Self {
            cancel_requested: AtomicBool::new(false),
            interrupted: AtomicBool::new(false),
            state: Mutex::new(ChildState::Running { child, pgid }),
        })
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel_requested.load(Ordering::Acquire)
    }

    pub(crate) fn was_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Acquire)
    }

    pub(crate) async fn send_interrupt(&self) -> bool {
        let guard = self.state.lock().await;
        match &*guard {
            ChildState::Running { pgid, .. } => {
                self.interrupted.store(true, Ordering::Release);
                send_signal_to_process_group(*pgid, ProcessSignal::Interrupt);
                true
            }
            ChildState::Reaped(_) | ChildState::ReapFailed => false,
        }
    }

    pub(crate) fn cancel_and_kill_nonblocking(&self) {
        self.cancel_requested.store(true, Ordering::Release);
        if let Ok(mut guard) = self.state.try_lock()
            && let ChildState::Running { child, pgid } = &mut *guard
        {
            send_signal_to_process_group(*pgid, ProcessSignal::Kill);
            match child.kill() {
                Ok(()) | Err(_) => {}
            }
        }
    }

    pub(crate) fn reap_with_grace_blocking(&self, grace_ms: u64) -> Option<ExitStatus> {
        let mut waited_ms = 0_u64;
        let step_ms = 10_u64;
        let (_wait_tx, wait_rx) = std::sync::mpsc::channel::<()>();
        loop {
            match self.try_reap_if_exited() {
                TryReapOutcome::StillRunning => {}
                TryReapOutcome::Exited(status) => return Some(status),
                TryReapOutcome::Failed => return None,
            }
            if waited_ms >= grace_ms || self.is_cancelled() {
                return self.force_kill_and_reap_blocking();
            }
            match wait_rx.recv_timeout(std::time::Duration::from_millis(step_ms)) {
                Ok(()) | Err(_) => {}
            }
            waited_ms = waited_ms.saturating_add(step_ms);
        }
    }

    fn try_reap_if_exited(&self) -> TryReapOutcome {
        let mut guard = self.state.blocking_lock();
        match &mut *guard {
            ChildState::Running { child, .. } => match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = ChildState::Reaped(status);
                    TryReapOutcome::Exited(status)
                }
                Ok(None) => TryReapOutcome::StillRunning,
                Err(_) => {
                    *guard = ChildState::ReapFailed;
                    TryReapOutcome::Failed
                }
            },
            ChildState::Reaped(status) => TryReapOutcome::Exited(*status),
            ChildState::ReapFailed => TryReapOutcome::Failed,
        }
    }

    pub(crate) fn force_kill_and_reap_blocking(&self) -> Option<ExitStatus> {
        let mut guard = self.state.blocking_lock();
        reap_guard_forced(&mut guard)
    }

    pub(crate) async fn force_kill_and_reap(&self) -> Option<ExitStatus> {
        let mut guard = self.state.lock().await;
        reap_guard_forced(&mut guard)
    }
}

fn reap_guard_forced(guard: &mut ChildState) -> Option<ExitStatus> {
    match guard {
        ChildState::Running { child, pgid } => {
            let target_pgid = *pgid;
            terminate_running_child(child, target_pgid);
            let waited = child.wait();
            send_signal_to_process_group(target_pgid, ProcessSignal::Kill);
            if let Ok(status) = waited {
                *guard = ChildState::Reaped(status);
                Some(status)
            } else {
                *guard = ChildState::ReapFailed;
                None
            }
        }
        ChildState::Reaped(status) => Some(*status),
        ChildState::ReapFailed => None,
    }
}

fn terminate_running_child(child: &mut Child, target_pgid: u32) {
    send_signal_to_process_group(target_pgid, ProcessSignal::Terminate);
    send_signal_to_process_group(target_pgid, ProcessSignal::Kill);
    match child.kill() {
        Ok(()) | Err(_) => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TryReapOutcome {
    StillRunning,
    Exited(ExitStatus),
    Failed,
}

pub(crate) struct ChildCancelGuard {
    child: Arc<SharedChild>,
}

impl fmt::Debug for ChildCancelGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildCancelGuard").finish_non_exhaustive()
    }
}

impl ChildCancelGuard {
    pub(crate) fn new(child: Arc<SharedChild>) -> Self {
        Self { child }
    }
}

impl Drop for ChildCancelGuard {
    fn drop(&mut self) {
        self.child.cancel_and_kill_nonblocking();
    }
}

pub(crate) struct SpawnSpec {
    pub(crate) executable: std::path::PathBuf,
    pub(crate) args: Vec<String>,
    pub(crate) work_dir: std::path::PathBuf,
    pub(crate) env: Vec<(String, String)>,
}

impl fmt::Debug for SpawnSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpawnSpec")
            .field("executable", &self.executable)
            .field("work_dir", &self.work_dir)
            .field("arg_count", &self.args.len())
            .field("env_count", &self.env.len())
            .finish()
    }
}

pub(crate) struct SpawnedChild {
    pub(crate) pid: Option<application::ProcessId>,
    pub(crate) shared: Arc<SharedChild>,
    pub(crate) stdin: ChildStdin,
    pub(crate) stdout: ChildStdout,
}

pub(crate) fn spawn_isolated_child(
    spec: &SpawnSpec,
    mcp_token: Option<&application::SecretValue>,
) -> io::Result<SpawnedChild> {
    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.args)
        .current_dir(&spec.work_dir)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0_i32);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    if let Some(secret) = mcp_token {
        let token_str = std::str::from_utf8(secret.as_exposed_secret())
            .map_err(|_error| io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 mcp token"))?;
        command.env(crate::command::MCP_TOKEN_ENV_KEY, token_str);
    }
    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing child stdout"))?;
    let pid = application::ProcessId::try_new(child.id()).ok();
    Ok(SpawnedChild {
        pid,
        shared: SharedChild::new(child),
        stdin,
        stdout,
    })
}

pub(crate) fn write_user_message_line(stdin: &mut ChildStdin, content: &str) -> io::Result<()> {
    let payload = serde_json::json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": content,
        },
        "parent_tool_use_id": serde_json::Value::Null,
    });
    let encoded = serde_json::to_string(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    writeln!(stdin, "{encoded}")?;
    stdin.flush()
}

pub(crate) enum LineReadOutcome {
    Line(String),
    Eof,
    TooLong { byte_len: usize },
    InvalidUtf8,
    IoError,
}

pub(crate) fn read_bounded_line(reader: &mut BufReader<ChildStdout>) -> LineReadOutcome {
    let limit = MAX_STREAM_LINE_BYTES.saturating_add(2_usize);
    let mut buffer = Vec::new();
    let mut saw_newline = false;
    loop {
        let Ok(available) = reader.fill_buf() else {
            return LineReadOutcome::IoError;
        };
        if available.is_empty() {
            break;
        }
        let newline_pos = available.iter().position(|byte| *byte == b'\n');
        let take_len = match newline_pos {
            Some(pos) => pos.saturating_add(1_usize),
            None => available.len(),
        };
        let remaining = limit.saturating_sub(buffer.len());
        if take_len > remaining {
            let overflow_len = buffer.len().saturating_add(take_len);
            reader.consume(take_len);
            return LineReadOutcome::TooLong {
                byte_len: overflow_len,
            };
        }
        if let Some(chunk) = available.get(..take_len) {
            buffer.extend_from_slice(chunk);
        }
        reader.consume(take_len);
        if newline_pos.is_some() {
            saw_newline = true;
            break;
        }
    }
    if buffer.is_empty() && !saw_newline {
        return LineReadOutcome::Eof;
    }
    let Ok(text) = String::from_utf8(buffer) else {
        return LineReadOutcome::InvalidUtf8;
    };
    let trimmed_len = text.trim_end_matches(['\r', '\n']).len();
    if trimmed_len > MAX_STREAM_LINE_BYTES {
        return LineReadOutcome::TooLong {
            byte_len: trimmed_len,
        };
    }
    LineReadOutcome::Line(text)
}

pub(crate) fn map_line_read_failure(
    outcome: LineReadOutcome,
) -> Result<Option<String>, StreamDecodeError> {
    match outcome {
        LineReadOutcome::Line(line) => Ok(Some(line)),
        LineReadOutcome::Eof => Ok(None),
        LineReadOutcome::TooLong { byte_len } => Err(StreamDecodeError::LineTooLong { byte_len }),
        LineReadOutcome::InvalidUtf8 | LineReadOutcome::IoError => Err(StreamDecodeError::Schema(
            crate::error::SchemaFailure::ExpectedObject,
        )),
    }
}

pub(crate) struct ActiveRunRegistry {
    runs: Mutex<BTreeMap<String, Arc<SharedChild>>>,
}

impl fmt::Debug for ActiveRunRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveRunRegistry").finish_non_exhaustive()
    }
}

impl ActiveRunRegistry {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            runs: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) async fn register(&self, run_id: String, child: Arc<SharedChild>) {
        let mut guard = self.runs.lock().await;
        guard.insert(run_id, child);
    }

    pub(crate) fn unregister_blocking(&self, run_id: &str) {
        let mut guard = self.runs.blocking_lock();
        guard.remove(run_id);
    }

    pub(crate) async fn unregister(&self, run_id: &str) {
        let mut guard = self.runs.lock().await;
        guard.remove(run_id);
    }

    pub(crate) async fn interrupt(&self, run_id: &str) -> bool {
        let target = {
            let guard = self.runs.lock().await;
            guard.get(run_id).cloned()
        };
        match target {
            Some(child) => child.send_interrupt().await,
            None => false,
        }
    }
}
