use super::{
    AttachableAi, McpAttachedAi, McpRunLink, McpServerHost, McpServerHostParts, RunMcpHost,
};
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AnalysisPrompt, ChatHistory,
    CollectionQuery, ExecutablePath, InformationError, IsolationCheck, ModelSelection,
    PeerProcessInfo, ProcessId, ReasoningEffort, ResetTime, RunInput, RunKind, RunPurpose, RunSpec,
};
use domain::UntrustedText;
use mcp_server::{McpServerConfig, PeerInspectorMode, ToolLedgerMode, ToolRegistry};
use serde_json::Value;
use std::future::{Future, ready};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;
use testkit_ports::chat_fakes::ScriptedAiEventStream;
use testkit_ports::system_fakes::{MemoryToolCallLedger, ScriptedPeerProcessInspector};

static COUNTER: AtomicU64 = AtomicU64::new(1_u64);
const BRIDGE: &str = "/Applications/Stock Copilot.app/Contents/MacOS/mcp-bridge";

struct TempBase(PathBuf);

impl TempBase {
    fn new() -> Self {
        let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
        Self(PathBuf::from("/private/tmp").join(format!("sc-fd-{}-{seq}", std::process::id())))
    }
}

impl Drop for TempBase {
    fn drop(&mut self) {
        match std::fs::remove_dir_all(&self.0) {
            Ok(()) | Err(_) => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Run(RunKind),
    Resume(RunKind),
    RunWithMcp,
    ResumeWithMcp,
    VerifyWithMcp,
    Verify(RunKind),
    Interrupt,
    Usage,
}

struct RecordingAi {
    calls: mpsc::Sender<Call>,
    links: mpsc::Sender<McpRunLink>,
}

impl RecordingAi {
    fn record(&self, call: Call) {
        let _sent = self.calls.send(call);
    }

    fn keep(&self, call: Call, link: McpRunLink) {
        self.record(call);
        let _sent = self.links.send(link);
    }
}

fn stream() -> ScriptedAiEventStream {
    ScriptedAiEventStream::from_events(vec![
        AiEvent::IsolationChecked(IsolationCheck::Passed),
        AiEvent::TextDelta(UntrustedText::try_new("ok").expect("text")),
    ])
}

impl AiProvider for RecordingAi {
    type Stream = ScriptedAiEventStream;

    fn run(
        &self,
        spec: RunSpec,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        self.record(Call::Run(spec.kind()));
        ready(Ok(stream()))
    }

    fn resume(
        &self,
        spec: RunSpec,
        _history: ChatHistory,
        _message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        self.record(Call::Resume(spec.kind()));
        ready(Ok(stream()))
    }

    fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
        self.record(Call::Usage);
        ready(Ok(AiUsage::Unknown))
    }

    fn interrupt(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<(), InformationError>> + Send {
        self.record(Call::Interrupt);
        ready(Ok(()))
    }

    fn verify_isolation(
        &self,
        kind: RunKind,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
        self.record(Call::Verify(kind));
        ready(Ok(IsolationCheck::Passed))
    }
}

impl AttachableAi for RecordingAi {
    fn run_with_mcp(
        &self,
        _spec: RunSpec,
        link: McpRunLink,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        self.keep(Call::RunWithMcp, link);
        ready(Ok(stream()))
    }

    fn resume_with_mcp(
        &self,
        _spec: RunSpec,
        _history: ChatHistory,
        _message: UntrustedText,
        link: McpRunLink,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        self.keep(Call::ResumeWithMcp, link);
        ready(Ok(stream()))
    }

    fn verify_with_mcp(
        &self,
        link: McpRunLink,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
        let check = if link.socket_path.exists() {
            IsolationCheck::Passed
        } else {
            IsolationCheck::Rejected(application::IsolationFailure::McpServers)
        };
        self.keep(Call::VerifyWithMcp, link);
        ready(Ok(check))
    }
}

fn clock() -> Arc<testkit_ports::SteppedClock> {
    Arc::new(testkit_ports::SteppedClock::new(
        domain::Timestamp::from_parts(
            domain::UnixSeconds::new(1_790_000_000_i64),
            domain::Nanosecond::new(0_u32),
            domain::OffsetSeconds::new(0_i32),
        )
        .expect("시각"),
    ))
}

fn parent_pid() -> ProcessId {
    ProcessId::try_new(4242_u32).expect("pid")
}

fn host(
    base: &Path,
    inspector: PeerInspectorMode<ScriptedPeerProcessInspector>,
) -> McpServerHost<testkit_ports::SteppedClock, ScriptedPeerProcessInspector, MemoryToolCallLedger>
{
    McpServerHost::new(McpServerHostParts {
        clock: clock(),
        inspector,
        config: McpServerConfig::default_config().expect("설정"),
        registry: Arc::new(ToolRegistry::new()),
        ledger: ToolLedgerMode::Configured(Arc::new(MemoryToolCallLedger::recording())),
        base_dir: base.join("m"),
        bridge: ExecutablePath::try_new(BRIDGE).expect("브리지"),
    })
}

fn matching_inspector() -> PeerInspectorMode<ScriptedPeerProcessInspector> {
    PeerInspectorMode::Configured(Arc::new(ScriptedPeerProcessInspector::new([Ok(
        PeerProcessInfo::new(
            parent_pid(),
            ExecutablePath::try_new(BRIDGE).expect("브리지"),
        ),
    )])))
}

fn analysis(id: &str) -> RunSpec {
    RunSpec::new(
        AiRunId::try_new(id).expect("id"),
        RunInput::Analysis(AnalysisPrompt::try_new("보유 종목 확인").expect("prompt")),
        ModelSelection::Default,
        ReasoningEffort::Default,
        RunPurpose::Chat,
    )
}

fn collection(id: &str) -> RunSpec {
    RunSpec::new(
        AiRunId::try_new(id).expect("id"),
        RunInput::Collection(CollectionQuery::CodeTopics {
            instruments: Vec::new(),
            topics: Vec::new(),
        }),
        ModelSelection::Fast,
        ReasoningEffort::Low,
        RunPurpose::WebCollection,
    )
}

fn read_line(reader: &mut BufReader<UnixStream>) -> Option<Value> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0_usize) | Err(_) => None,
        Ok(_) => serde_json::from_str(line.trim_end()).ok(),
    }
}

fn list_tools_over(socket: PathBuf, token: Vec<u8>) -> Option<Value> {
    std::thread::spawn(move || {
        let stream = UnixStream::connect(&socket).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .ok()?;
        let mut writer = stream.try_clone().ok()?;
        let mut reader = BufReader::new(stream);
        let mut payload = b"AUTH ".to_vec();
        payload.extend_from_slice(&token);
        payload.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t\",\"version\":\"0\"}}}\n");
        writer.write_all(&payload).ok()?;
        let _init = read_line(&mut reader)?;
        writer
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n")
            .ok()?;
        read_line(&mut reader)
    })
    .join()
    .ok()
    .flatten()
}

fn wrapper(
    base: &Path,
    inspector: PeerInspectorMode<ScriptedPeerProcessInspector>,
) -> (
    McpAttachedAi<
        RecordingAi,
        McpServerHost<
            testkit_ports::SteppedClock,
            ScriptedPeerProcessInspector,
            MemoryToolCallLedger,
        >,
    >,
    Recorded,
) {
    let (calls, call_rx) = mpsc::channel();
    let (links, link_rx) = mpsc::channel();
    (
        McpAttachedAi::new(
            Arc::new(RecordingAi { calls, links }),
            Arc::new(host(base, inspector)),
        ),
        Recorded {
            calls: call_rx,
            links: link_rx,
        },
    )
}

struct Recorded {
    calls: mpsc::Receiver<Call>,
    links: mpsc::Receiver<McpRunLink>,
}

impl Recorded {
    fn next_call(&self) -> Option<Call> {
        self.calls.try_recv().ok()
    }

    fn take_link(&self, expected: Call) -> McpRunLink {
        assert_eq!(self.next_call(), Some(expected));
        self.links.try_recv().expect("첨부")
    }
}

async fn drain<S: AiEventStream>(stream: &mut S) -> usize {
    let mut count = 0_usize;
    while let Ok(Some(_event)) = stream.next().await {
        count = count.saturating_add(1);
    }
    count
}

#[test]
fn analysis_run_serves_its_own_mcp_server_and_closes_it_when_the_stream_ends() {
    tauri::async_runtime::block_on(async {
        let base = TempBase::new();
        let (ai, calls) = wrapper(&base.0, matching_inspector());
        let mut stream = ai.run(analysis("chat-run-1")).await.expect("실행");
        let link = calls.take_link(Call::RunWithMcp);
        assert!(link.socket_path.starts_with(base.0.join("m")));
        assert!(link.socket_path.exists(), "실행 중에는 소켓이 있다");
        link.binder.bind(Some(parent_pid()));
        let tools = list_tools_over(
            link.socket_path.clone(),
            link.token.as_exposed_secret().to_vec(),
        )
        .expect("tools/list 응답");
        assert_eq!(
            tools.pointer("/result/tools"),
            Some(&Value::Array(Vec::new()))
        );
        assert_eq!(drain(&mut stream).await, 2_usize);
        assert!(!link.socket_path.exists(), "스트림이 끝나면 소켓을 지운다");
        assert!(
            list_tools_over(
                link.socket_path.clone(),
                link.token.as_exposed_secret().to_vec()
            )
            .is_none(),
            "끝난 실행의 토큰으로는 연결할 수 없다"
        );
    });
}

#[test]
fn each_analysis_run_gets_a_distinct_socket_and_token() {
    tauri::async_runtime::block_on(async {
        let base = TempBase::new();
        let (ai, calls) = wrapper(&base.0, matching_inspector());
        let _first = ai.run(analysis("chat-run-a")).await.expect("첫 실행");
        let _second = ai
            .resume(
                analysis("chat-run-b"),
                ChatHistory::new(Vec::new()),
                UntrustedText::try_new("다음").expect("text"),
            )
            .await
            .expect("둘째 실행");
        let first = calls.take_link(Call::RunWithMcp);
        let second = calls.take_link(Call::ResumeWithMcp);
        assert_ne!(first.socket_path, second.socket_path);
        assert_ne!(
            first.token.as_exposed_secret(),
            second.token.as_exposed_secret()
        );
    });
}

async fn tools_reachable(
    inspector: PeerInspectorMode<ScriptedPeerProcessInspector>,
    bound: Option<ProcessId>,
) -> bool {
    let base = TempBase::new();
    let (ai, calls) = wrapper(&base.0, inspector);
    let _stream = ai.run(analysis("chat-run-peer")).await.expect("실행");
    let link = calls.take_link(Call::RunWithMcp);
    if bound.is_some() {
        link.binder.bind(bound);
    }
    list_tools_over(
        link.socket_path.clone(),
        link.token.as_exposed_secret().to_vec(),
    )
    .is_some()
}

#[test]
fn unbound_or_mismatched_peer_cannot_reach_the_run_server() {
    tauri::async_runtime::block_on(async {
        assert!(tools_reachable(matching_inspector(), Some(parent_pid())).await);
        assert!(
            !tools_reachable(matching_inspector(), None).await,
            "CLI PID를 묶기 전에는 거부"
        );
        assert!(
            !tools_reachable(
                matching_inspector(),
                Some(ProcessId::try_new(7_u32).expect("pid"))
            )
            .await,
            "다른 부모 PID는 거부"
        );
        assert!(
            !tools_reachable(PeerInspectorMode::Unconfigured, Some(parent_pid())).await,
            "상대 확인 수단이 없으면 거부"
        );
    });
}

#[test]
fn collection_runs_never_open_an_mcp_server() {
    tauri::async_runtime::block_on(async {
        let base = TempBase::new();
        let (ai, calls) = wrapper(&base.0, matching_inspector());
        let mut stream = ai.run(collection("collect-1")).await.expect("수집");
        let _resumed = ai
            .resume(
                collection("collect-2"),
                ChatHistory::new(Vec::new()),
                UntrustedText::try_new("x").expect("text"),
            )
            .await
            .expect("수집 재개");
        assert_eq!(drain(&mut stream).await, 2_usize);
        assert_eq!(calls.next_call(), Some(Call::Run(RunKind::Collection)));
        assert_eq!(calls.next_call(), Some(Call::Resume(RunKind::Collection)));
        assert!(
            !base.0.join("m").exists(),
            "수집 실행은 소켓 폴더도 만들지 않는다"
        );
    });
}

#[test]
fn isolation_probe_for_analysis_uses_a_live_run_server_and_delegates_the_rest() {
    tauri::async_runtime::block_on(async {
        let base = TempBase::new();
        let (ai, calls) = wrapper(&base.0, matching_inspector());
        assert_eq!(
            ai.verify_isolation(RunKind::Analysis).await,
            Ok(IsolationCheck::Passed)
        );
        assert_eq!(calls.next_call(), Some(Call::VerifyWithMcp));
        assert_eq!(
            ai.verify_isolation(RunKind::Collection).await,
            Ok(IsolationCheck::Passed)
        );
        assert_eq!(calls.next_call(), Some(Call::Verify(RunKind::Collection)));
        assert_eq!(ai.usage().await, Ok(AiUsage::Unknown));
        assert_eq!(calls.next_call(), Some(Call::Usage));
        assert_eq!(
            ai.interrupt(&AiRunId::try_new("chat-run-1").expect("id"))
                .await,
            Ok(())
        );
        assert_eq!(calls.next_call(), Some(Call::Interrupt));
    });
}

#[test]
fn analysis_run_fails_closed_without_calling_the_cli_when_the_server_cannot_start() {
    tauri::async_runtime::block_on(async {
        let base = TempBase::new();
        let long = base.0.join("x".repeat(120_usize));
        let (sender, calls) = mpsc::channel();
        let (links, _link_rx) = mpsc::channel();
        let ai = McpAttachedAi::new(
            Arc::new(RecordingAi {
                calls: sender,
                links,
            }),
            Arc::new(host(&long, matching_inspector())),
        );
        assert_eq!(
            ai.run(analysis("chat-run-long")).await.err(),
            Some(InformationError::Unavailable(ResetTime::NotProvided))
        );
        assert!(calls.try_recv().is_err(), "CLI를 띄우지 않는다");
    });
}

#[test]
fn host_reports_the_registry_it_serves() {
    let base = TempBase::new();
    let served = host(&base.0, matching_inspector());
    assert!(served.registry().is_empty());
    let opened = served.open(&AiRunId::try_new("outside-runtime").expect("id"));
    assert_eq!(
        opened.err(),
        Some(InformationError::Unavailable(ResetTime::NotProvided)),
        "런타임 밖에서는 서버를 띄우지 않는다"
    );
}
