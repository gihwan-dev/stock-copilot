use std::sync::Arc;
use std::time::Duration;

use application::{AiRunId, RunKind, ToolCallOutcome, ToolCallRecord, ToolName};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use serde_json::{Map, Value, json};
use testkit_ports::SteppedClock;
use testkit_ports::system_fakes::MemoryToolCallLedger;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, duplex};
use tokio::sync::watch;

use crate::handler::authenticate_and_serve_stream;
use crate::handler::{ConnectionEventLog, ShutdownSignal, StreamAuthParams};
use crate::token::{AuthLineLimit, RunAuthBinding, TokenTtl};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorCode, ToolErrorPayload, ToolFuture, ToolInvocation,
    ToolLedgerMode, ToolRegistry, ToolRuntime, ToolSpec, build_object_schema,
};

const SNAPSHOT_2026: &str = "contract_2026_07_28.jsonl";
const SNAPSHOT_2025: &str = "contract_2025_06_18.jsonl";

const ENVELOPE_ONLY_2026: [&str; 3] = ["resultType", "cacheScope", "ttlMs"];

const META_2026: &str = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}"#;

struct FixtureQuoteTool {
    spec: ToolSpec,
}

impl FixtureQuoteTool {
    fn new() -> Self {
        let mut props = Map::new();
        props.insert(
            String::from("symbol"),
            json!({ "type": "string", "minLength": 1_u64, "maxLength": 12_u64 }),
        );
        let schema = build_object_schema(props, &["symbol"]);
        let spec = ToolSpec::try_new(
            ToolName::try_new("get_fixture_quote").expect("도구 이름"),
            "Contract fixture returning a fixed quote.",
            ToolAccessKind::ReadOnly,
            schema,
            None,
        )
        .expect("도구 사양");
        Self { spec }
    }
}

impl McpTool for FixtureQuoteTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move {
            let symbol = invocation
                .arguments
                .get("symbol")
                .and_then(Value::as_str)
                .map_or("", |val| val);
            if symbol == "MISSING" {
                return Err(ToolErrorPayload::non_retryable(
                    ToolErrorCode::DataUnavailable,
                    Some("symbol"),
                    "no data for the requested symbol",
                ));
            }
            Ok(
                json!({ "symbol": symbol, "price": "71200", "fetched_at": "2026-11-25T09:00:00+09:00" }),
            )
        })
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("테스트 런타임")
}

fn timestamp_at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0_u32),
        OffsetSeconds::new(0_i32),
    )
    .expect("유효한 시각")
}

enum Step {
    Request(String),
    Notification(String),
}

struct Session {
    reader: BufReader<tokio::io::ReadHalf<DuplexStream>>,
    writer: tokio::io::WriteHalf<DuplexStream>,
    transcript: Vec<Value>,
}

impl Session {
    async fn run(&mut self, step: Step) {
        let (line, expects_reply) = match step {
            Step::Request(line) => (line, true),
            Step::Notification(line) => (line, false),
        };
        let sent: Value = serde_json::from_str(&line).expect("요청 JSON");
        self.transcript.push(json!({ "send": sent }));
        self.writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("요청 전송");
        if expects_reply {
            let mut reply = String::new();
            let count = tokio::time::timeout(
                Duration::from_secs(3_u64),
                self.reader.read_line(&mut reply),
            )
            .await
            .expect("응답 시간 초과")
            .expect("응답 읽기");
            assert!(count > 0_usize, "응답 줄이 비어 있으면 안 된다");
            let received: Value = serde_json::from_str(&reply).expect("응답 JSON");
            self.transcript.push(json!({ "recv": received }));
        }
    }
}

struct Outcome {
    transcript: Vec<Value>,
    records: Vec<ToolCallRecord>,
    raw_token: String,
}

fn issue_binding(run_label: &str) -> Arc<RunAuthBinding<SteppedClock>> {
    let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
    Arc::new(
        RunAuthBinding::issue(
            AiRunId::try_new(run_label).expect("실행 ID"),
            RunKind::Analysis,
            clock,
            TokenTtl::try_from_seconds(600_u64).expect("TTL"),
        )
        .expect("발급"),
    )
}

fn fixture_runtime() -> (ToolRuntime<MemoryToolCallLedger>, Arc<MemoryToolCallLedger>) {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(FixtureQuoteTool::new()))
        .expect("등록");
    let ledger = Arc::new(MemoryToolCallLedger::recording());
    let tools = ToolRuntime {
        registry: Arc::new(registry),
        ledger: ToolLedgerMode::Configured(Arc::clone(&ledger)),
    };
    (tools, ledger)
}

async fn run_transcript(run_label: &str, steps: Vec<Step>) -> Outcome {
    let binding = issue_binding(run_label);
    let raw_token = std::str::from_utf8(binding.token().as_exposed_bytes())
        .expect("UTF-8")
        .to_owned();
    let (tools, ledger) = fixture_runtime();
    let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
    let (client, server) = duplex(65_536_usize);
    let (server_read, server_write) = tokio::io::split(server);
    let params = StreamAuthParams {
        auth: Arc::clone(&binding),
        auth_line_limit: AuthLineLimit::default_limit(),
        auth_timeout: Duration::from_secs(2_u64),
        shutdown_rx,
        events: Arc::new(ConnectionEventLog::new()),
        tools: Some(tools),
    };
    let task = tokio::spawn(authenticate_and_serve_stream(
        server_read,
        server_write,
        params,
    ));
    let (client_read, mut client_write) = tokio::io::split(client);
    let mut auth_line = b"AUTH ".to_vec();
    auth_line.extend_from_slice(binding.token().as_exposed_bytes());
    auth_line.push(b'\n');
    client_write.write_all(&auth_line).await.expect("인증 전송");
    let transcript = {
        let mut session = Session {
            reader: BufReader::new(client_read),
            writer: client_write,
            transcript: Vec::new(),
        };
        for step in steps {
            session.run(step).await;
        }
        session.transcript
    };
    let _stopped = shutdown_tx.send(ShutdownSignal::Stopped).is_err();
    task.await.expect("서버 태스크 종료");
    Outcome {
        transcript,
        records: ledger.records(),
        raw_token,
    }
}

fn steps_2026() -> Vec<Step> {
    vec![
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{{{META_2026}}}}}"#
        )),
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{{META_2026}}}}}"#
        )),
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{{META_2026},"name":"get_fixture_quote","arguments":{{"symbol":"005930"}}}}}}"#
        )),
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{{META_2026},"name":"get_fixture_quote","arguments":{{"symbol":"005930","side":"buy"}}}}}}"#
        )),
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{{{META_2026},"name":"get_fixture_quote","arguments":{{"symbol":"MISSING"}}}}}}"#
        )),
        Step::Request(format!(
            r#"{{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{{{META_2026},"name":"place_order","arguments":{{}}}}}}"#
        )),
    ]
}

fn steps_2025() -> Vec<Step> {
    vec![
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"contract-client","version":"0.0.0"}}}"#,
        )),
        Step::Notification(String::from(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )),
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        )),
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_fixture_quote","arguments":{"symbol":"005930"}}}"#,
        )),
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_fixture_quote","arguments":{"symbol":"005930","side":"buy"}}}"#,
        )),
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"get_fixture_quote","arguments":{"symbol":"MISSING"}}}"#,
        )),
        Step::Request(String::from(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"place_order","arguments":{}}}"#,
        )),
        Step::Request(String::from(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)),
    ]
}

fn fixture_text(name: &str) -> String {
    let exe = testkit::test_executable().expect("test executable");
    let root = exe
        .ancestors()
        .find(|dir| dir.join("fixtures").join("mcp").is_dir())
        .expect("workspace root with fixtures/mcp");
    std::fs::read_to_string(root.join("fixtures").join("mcp").join(name)).expect("스냅샷 파일 읽기")
}

fn parse_snapshot(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("스냅샷 줄 JSON"))
        .collect()
}

fn render_transcript(transcript: &[Value]) -> String {
    transcript
        .iter()
        .map(|entry| serde_json::to_string(entry).expect("스냅샷 직렬화"))
        .collect::<Vec<String>>()
        .join("\n")
}

fn assert_matches_snapshot(name: &str, file_name: &str, outcome: &Outcome) {
    let expected = parse_snapshot(&fixture_text(file_name));
    let rendered = render_transcript(&outcome.transcript);
    for record in &outcome.records {
        assert!(
            !record
                .arguments_json()
                .as_str()
                .contains(&outcome.raw_token)
                && !record.result_json().as_str().contains(&outcome.raw_token),
            "{name} 원장 기록에 실행 토큰이 없어야 한다"
        );
    }
    assert_eq!(
        outcome.transcript, expected,
        "{name} JSON 줄 스냅샷이 달라졌다. 실제 기록:\n{rendered}\n"
    );
}

fn recorded_versions(records: &[ToolCallRecord]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record.protocol_version().as_str())
        .collect()
}

fn recorded_outcomes(records: &[ToolCallRecord]) -> Vec<ToolCallOutcome> {
    records.iter().map(ToolCallRecord::outcome).collect()
}

#[test]
fn generation_2026_07_28_matches_json_line_snapshot() {
    runtime().block_on(async {
        let outcome = run_transcript("run-contract-2026", steps_2026()).await;
        assert_matches_snapshot("2026-07-28", SNAPSHOT_2026, &outcome);
        assert_eq!(
            recorded_versions(&outcome.records),
            vec!["2026-07-28"; 4],
            "2026-07-28 세대 호출은 요청 _meta의 버전으로 원장에 남는다"
        );
        assert_eq!(
            recorded_outcomes(&outcome.records),
            vec![
                ToolCallOutcome::Succeeded,
                ToolCallOutcome::Rejected,
                ToolCallOutcome::ToolError,
                ToolCallOutcome::Rejected,
            ],
            "성공·스키마 거부·도구 오류·미등록 도구가 차례로 원장에 남는다"
        );
    });
}

#[test]
fn generation_2025_06_18_matches_json_line_snapshot() {
    runtime().block_on(async {
        let outcome = run_transcript("run-contract-2025", steps_2025()).await;
        assert_matches_snapshot("2025-06-18", SNAPSHOT_2025, &outcome);
        assert_eq!(
            recorded_versions(&outcome.records),
            vec!["2025-06-18"; 4],
            "2025-06-18 세대 호출은 협상한 버전으로 원장에 남는다"
        );
        assert_eq!(
            recorded_outcomes(&outcome.records),
            vec![
                ToolCallOutcome::Succeeded,
                ToolCallOutcome::Rejected,
                ToolCallOutcome::ToolError,
                ToolCallOutcome::Rejected,
            ],
            "두 세대의 원장 결과가 같다"
        );
    });
}

fn tool_results(transcript: &[Value]) -> Vec<Value> {
    transcript
        .iter()
        .filter_map(|entry| entry.get("recv"))
        .filter(|reply| {
            reply
                .get("result")
                .and_then(|res| res.get("content").or_else(|| res.get("tools")))
                .is_some()
        })
        .filter_map(|reply| reply.get("result").and_then(Value::as_object))
        .map(|result| {
            let kept: Map<String, Value> = result
                .iter()
                .filter(|(key, _value)| !ENVELOPE_ONLY_2026.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            Value::Object(kept)
        })
        .collect()
}

#[test]
fn both_generations_return_identical_tool_payloads() {
    runtime().block_on(async {
        let newer = run_transcript("run-contract-a", steps_2026()).await;
        let older = run_transcript("run-contract-b", steps_2025()).await;
        assert_eq!(
            tool_results(&newer.transcript),
            tool_results(&older.transcript),
            "도구 목록과 도구 호출 결과는 프로토콜 세대와 무관하게 같다"
        );
    });
}
