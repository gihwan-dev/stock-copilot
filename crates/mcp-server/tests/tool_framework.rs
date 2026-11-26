#[cfg(test)]
mod tests {
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::DirBuilderExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use application::{
        AiRunId, ExecutablePath, PeerProcessInfo, ProcessId, RetryDelay, RunKind, SystemFailure,
        SystemPort, SystemPortError, ToolCallOutcome, ToolCallRecord, ToolName,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        CursorStore, MAX_TOOL_RESPONSE_CHARS, McpServerConfig, McpServerHandle, McpTool,
        PageRequest, PaginationContext, PeerInspectorMode, PeerProcessTarget, RunIdentity,
        RunServerDeps, RunServerRequest, RunSocketSpec, ToolAccessKind, ToolErrorCode,
        ToolErrorPayload, ToolFuture, ToolInvocation, ToolLedgerMode, ToolRegistry,
        ToolRegistryError, ToolSpec, UnconfiguredToolLedger, build_object_schema,
        make_cursor_store, paginate_items,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::{MemoryToolCallLedger, ScriptedPeerProcessInspector};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    const META_2026: &str = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}"#;

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

    fn expected_parent_pid() -> ProcessId {
        ProcessId::try_new(8888_u32).expect("부모 PID")
    }

    fn expected_exe_path() -> ExecutablePath {
        ExecutablePath::try_new("/Applications/StockCopilot.app/Contents/MacOS/mcp-bridge")
            .expect("실행 경로")
    }

    fn ignore_io_result(result: std::io::Result<()>) {
        match result {
            Ok(()) => {}
            Err(_error) => {}
        }
    }

    struct TempBaseDir {
        path: PathBuf,
    }

    impl TempBaseDir {
        fn new(tag: &str) -> Self {
            let mut raw = [0_u8; 4];
            getrandom::fill(&mut raw).expect("임시 폴더 난수");
            let hex = format!("{:08x}", u32::from_be_bytes(raw));
            let canonical_tmp = testkit::canonical_temp_dir().expect("임시 루트 정규화");
            let path = canonical_tmp.join(format!("m02-{tag}-{hex}"));
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&path)
                .expect("테스트 기본 폴더 생성");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempBaseDir {
        fn drop(&mut self) {
            if self.path.exists() {
                ignore_io_result(fs::remove_dir_all(&self.path));
            }
        }
    }

    struct EchoReadTool {
        spec: ToolSpec,
    }

    impl EchoReadTool {
        fn new(name: &str, access: ToolAccessKind) -> Self {
            let mut props = Map::new();
            props.insert(
                String::from("symbol"),
                json!({ "type": "string", "minLength": 1_u64, "maxLength": 16_u64 }),
            );
            props.insert(
                String::from("count"),
                json!({ "type": "integer", "minimum": 1_i64, "maximum": 100_i64 }),
            );
            props.insert(
                String::from("market"),
                json!({ "type": "string", "enum": ["KR", "US"] }),
            );
            let schema = build_object_schema(props, &["symbol"]);
            let spec = ToolSpec::try_new(
                ToolName::try_new(name).expect("도구 이름"),
                "Deterministic echo tool for M-02 verification.",
                access,
                schema,
                None,
            )
            .expect("도구 사양");
            Self { spec }
        }
    }

    impl McpTool for EchoReadTool {
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
                Ok(json!({
                    "run_id": invocation.run_id.as_str(),
                    "symbol": symbol,
                    "ok": true
                }))
            })
        }
    }

    struct PagedListTool {
        spec: ToolSpec,
        cursors: Arc<CursorStore<SteppedClock>>,
        items: Vec<String>,
    }

    impl PagedListTool {
        fn new(cursors: Arc<CursorStore<SteppedClock>>, items: Vec<String>) -> Self {
            let mut props = Map::new();
            props.insert(
                String::from("limit"),
                json!({ "type": "integer", "minimum": 1_i64, "maximum": 200_i64 }),
            );
            props.insert(
                String::from("cursor"),
                json!({ "type": "string", "minLength": 32_u64, "maxLength": 32_u64 }),
            );
            let schema = build_object_schema(props, &[]);
            let spec = ToolSpec::try_new(
                ToolName::try_new("list_demo_items").expect("도구 이름"),
                "Paginated list tool for 30k character cap tests.",
                ToolAccessKind::ReadOnly,
                schema,
                None,
            )
            .expect("도구 사양");
            Self {
                spec,
                cursors,
                items,
            }
        }
    }

    impl McpTool for PagedListTool {
        fn spec(&self) -> ToolSpec {
            self.spec.clone()
        }

        fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
            Box::pin(async move {
                let raw_limit = invocation.arguments.get("limit").and_then(Value::as_u64);
                let raw_cursor = invocation.arguments.get("cursor").and_then(Value::as_str);
                let page_req = PageRequest::from_raw(raw_limit, raw_cursor)?;
                let context = PaginationContext {
                    cursors: &self.cursors,
                    run_id: invocation.run_id,
                    tool_name: self.spec.name(),
                    query_fingerprint: "demo-list",
                };
                paginate_items(&self.items, &page_req, &context, &Map::new())
            })
        }
    }

    struct OversizedTool {
        spec: ToolSpec,
    }

    impl OversizedTool {
        fn new() -> Self {
            let schema = build_object_schema(Map::new(), &[]);
            let spec = ToolSpec::try_new(
                ToolName::try_new("get_oversized_blob").expect("도구 이름"),
                "Returns a payload above 30000 chars to verify hard cap.",
                ToolAccessKind::ReadOnly,
                schema,
                None,
            )
            .expect("도구 사양");
            Self { spec }
        }
    }

    impl McpTool for OversizedTool {
        fn spec(&self) -> ToolSpec {
            self.spec.clone()
        }

        fn invoke<'a>(&'a self, _invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
            Box::pin(async move {
                Ok(json!({
                    "blob": "z".repeat(MAX_TOOL_RESPONSE_CHARS.saturating_add(500_usize))
                }))
            })
        }
    }

    async fn connect_and_auth(
        handle: &McpServerHandle<SteppedClock>,
    ) -> (
        BufReader<tokio::net::unix::OwnedReadHalf>,
        tokio::net::unix::OwnedWriteHalf,
    ) {
        let stream = UnixStream::connect(handle.socket_path())
            .await
            .expect("소켓 연결");
        let (read_half, mut write_half) = stream.into_split();
        let mut auth_line = b"AUTH ".to_vec();
        auth_line.extend_from_slice(handle.token().as_exposed_bytes());
        auth_line.push(b'\n');
        write_half.write_all(&auth_line).await.expect("인증 전송");
        (BufReader::new(read_half), write_half)
    }

    async fn send_rpc(
        reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: &mut tokio::net::unix::OwnedWriteHalf,
        request_json: &str,
    ) -> Value {
        writer
            .write_all(format!("{request_json}\n").as_bytes())
            .await
            .expect("JSON-RPC 전송");
        let mut line = String::new();
        let count = tokio::time::timeout(Duration::from_secs(3_u64), reader.read_line(&mut line))
            .await
            .expect("응답 시간 초과")
            .expect("응답 읽기");
        assert!(count > 0_usize, "응답 줄이 비어 있으면 안 된다");
        serde_json::from_str(&line).expect("응답 JSON 파싱")
    }

    fn start_tool_server(
        base: &TempBaseDir,
        run_id: &str,
        clock: Arc<SteppedClock>,
        registry: Arc<ToolRegistry>,
        ledger: ToolLedgerMode<MemoryToolCallLedger>,
    ) -> McpServerHandle<SteppedClock> {
        let inspector = Arc::new(ScriptedPeerProcessInspector::new(vec![Ok(
            PeerProcessInfo::new(expected_parent_pid(), expected_exe_path()),
        )]));
        let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 사양");
        let deps = RunServerDeps::new(
            clock,
            PeerInspectorMode::Configured(inspector),
            McpServerConfig::default_config().expect("기본 설정"),
        )
        .with_tools(registry, ledger);
        let request = RunServerRequest::new(
            RunIdentity::new(AiRunId::try_new(run_id).expect("run id"), RunKind::Analysis),
            spec,
            PeerProcessTarget::with_parent(expected_parent_pid(), expected_exe_path()),
            deps,
        );
        McpServerHandle::start(request).expect("도구 서버 시작")
    }

    fn call_tool_json(id: u64, name: &str, args_json: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{{META_2026},"name":"{name}","arguments":{args_json}}}}}"#
        )
    }

    #[test]
    fn tool_spec_leaves_naming_to_command_policy_and_rejects_unsafe_schemas() {
        let empty_schema = build_object_schema(Map::new(), &[]);
        for policy_named in [
            "place_order",
            "submit_trade",
            "approve_proposal",
            "update_settings",
            "read_keychain",
            "get_secret",
            "issue_token",
        ] {
            let spec = ToolSpec::try_new(
                ToolName::try_new(policy_named).expect("문법 유효 이름"),
                "Policy decides exposure",
                ToolAccessKind::ReadOnly,
                empty_schema.clone(),
                None,
            );
            assert!(
                spec.is_ok(),
                "이름 조각 금지는 명령별 AI 정책으로 대체됐다: {policy_named}"
            );
        }

        let mut open_schema = Map::new();
        open_schema.insert(String::from("type"), json!("object"));
        open_schema.insert(String::from("properties"), json!({}));
        assert_eq!(
            ToolSpec::try_new(
                ToolName::try_new("get_demo").expect("이름"),
                "Missing additionalProperties false",
                ToolAccessKind::ReadOnly,
                open_schema,
                None,
            )
            .expect_err("additionalProperties false 누락 거부"),
            ToolRegistryError::InvalidInputSchema,
            "additionalProperties=false 필수"
        );

        let mut combinator_schema = build_object_schema(Map::new(), &[]);
        combinator_schema.insert(String::from("anyOf"), json!([]));
        assert_eq!(
            ToolSpec::try_new(
                ToolName::try_new("get_demo").expect("이름"),
                "Combinator schema",
                ToolAccessKind::ReadOnly,
                combinator_schema,
                None,
            )
            .expect_err("anyOf 스키마 거부"),
            ToolRegistryError::InvalidInputSchema,
            "anyOf/oneOf/allOf 금지"
        );
    }

    fn build_sorted_registry() -> Arc<ToolRegistry> {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(EchoReadTool::new(
                "zeta_tool",
                ToolAccessKind::StateChanging,
            )))
            .expect("zeta 등록");
        registry
            .register(Arc::new(EchoReadTool::new(
                "alpha_tool",
                ToolAccessKind::ReadOnly,
            )))
            .expect("alpha 등록");
        registry
            .register(Arc::new(EchoReadTool::new(
                "beta_tool",
                ToolAccessKind::ReadOnly,
            )))
            .expect("beta 등록");
        Arc::new(registry)
    }

    #[test]
    fn tools_list_is_deterministic_and_preserves_readonly_annotations() {
        runtime().block_on(async {
            let base = TempBaseDir::new("list");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let ledger = Arc::new(MemoryToolCallLedger::recording());
            let handle = start_tool_server(
                &base,
                "run-list-1",
                clock,
                build_sorted_registry(),
                ToolLedgerMode::Configured(ledger),
            );
            let (mut reader, mut writer) = connect_and_auth(&handle).await;
            let list_req = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{{{META_2026}}}}}"#
            );
            let response = send_rpc(&mut reader, &mut writer, &list_req).await;
            let tools = response
                .get("result")
                .and_then(|res| res.get("tools"))
                .and_then(Value::as_array)
                .expect("tools 배열");
            let names: Vec<&str> = tools
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .collect();
            assert_eq!(
                names,
                vec!["alpha_tool", "beta_tool", "zeta_tool"],
                "결정적 사전순"
            );
            assert_eq!(
                tools
                    .first()
                    .and_then(|item| item.get("annotations"))
                    .and_then(|ann| ann.get("readOnlyHint"))
                    .and_then(Value::as_bool),
                Some(true),
                "읽기 도구 readOnlyHint=true"
            );
            assert_eq!(
                tools
                    .get(2_usize)
                    .and_then(|item| item.get("annotations"))
                    .and_then(|ann| ann.get("readOnlyHint"))
                    .and_then(Value::as_bool),
                Some(false),
                "상태 변경 도구 readOnlyHint=false"
            );
            let _events = handle.shutdown().await;
        });
    }

    fn assert_invalid_structured_response(response: &Value, raw_token: &str) {
        let serialized = serde_json::to_string(response).expect("응답 직렬화");
        assert!(
            !serialized.contains(raw_token),
            "오류 응답에 원문 토큰이 남아서는 안 된다"
        );
        let inner = response.get("result").expect("입력 오류도 result로 반환");
        assert_eq!(
            inner.get("isError").and_then(Value::as_bool),
            Some(true),
            "스키마 오류는 isError=true: {response}"
        );
        assert_eq!(
            inner
                .get("structuredContent")
                .and_then(|sc| sc.get("code"))
                .and_then(Value::as_str),
            Some("invalid_input"),
            "구조화 오류 코드 invalid_input"
        );
    }

    #[test]
    fn valid_tool_call_returns_matching_structured_and_text_content() {
        runtime().block_on(async {
            let base = TempBaseDir::new("ok");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(EchoReadTool::new(
                    "echo_read",
                    ToolAccessKind::ReadOnly,
                )))
                .expect("등록");
            let ledger = Arc::new(MemoryToolCallLedger::recording());
            let handle = start_tool_server(
                &base,
                "run-ok-1",
                clock,
                Arc::new(registry),
                ToolLedgerMode::Configured(Arc::clone(&ledger)),
            );
            let (mut reader, mut writer) = connect_and_auth(&handle).await;
            let request = call_tool_json(
                1_u64,
                "echo_read",
                r#"{"symbol":"005930","count":10,"market":"KR"}"#,
            );
            let response = send_rpc(&mut reader, &mut writer, &request).await;
            let result_obj = response.get("result").expect("성공 result");
            assert_ne!(
                result_obj.get("isError").and_then(Value::as_bool),
                Some(true),
                "정상 호출은 isError=true가 아니다"
            );
            let text_block = result_obj
                .get("content")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(Value::as_str)
                .expect("text 블록");
            let parsed: Value = serde_json::from_str(text_block).expect("text JSON");
            assert_eq!(
                Some(&parsed),
                result_obj.get("structuredContent"),
                "structuredContent와 text 블록 일치"
            );
            assert_eq!(
                ledger.records().first().map(ToolCallRecord::outcome),
                Some(ToolCallOutcome::Succeeded),
                "성공 호출 원장 기록"
            );
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn schema_validation_errors_are_structured_recorded_and_redact_secrets() {
        runtime().block_on(async {
            let base = TempBaseDir::new("val");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(EchoReadTool::new(
                    "echo_read",
                    ToolAccessKind::ReadOnly,
                )))
                .expect("등록");
            let ledger = Arc::new(MemoryToolCallLedger::recording());
            let handle = start_tool_server(
                &base,
                "run-val-1",
                clock,
                Arc::new(registry),
                ToolLedgerMode::Configured(Arc::clone(&ledger)),
            );
            let raw_token = std::str::from_utf8(handle.token().as_exposed_bytes())
                .expect("토큰 UTF-8")
                .to_owned();
            let (mut reader, mut writer) = connect_and_auth(&handle).await;

            let invalid_args = [
                format!(r#"{{"symbol":"005930","{raw_token}":1}}"#),
                String::from(r#"{"count":5}"#),
                String::from(r#"{"symbol":"005930","count":18446744073709551615}"#),
                String::from(r#"{"symbol":"005930","count":null}"#),
            ];
            for (index, args) in invalid_args.iter().enumerate() {
                let req_id = u64::try_from(index.saturating_add(1_usize)).expect("요청 번호");
                let request = call_tool_json(req_id, "echo_read", args);
                let response = send_rpc(&mut reader, &mut writer, &request).await;
                assert_invalid_structured_response(&response, &raw_token);
            }

            let missing_request = call_tool_json(9_u64, "missing_tool", "{}");
            let missing_response = send_rpc(&mut reader, &mut writer, &missing_request).await;
            assert_eq!(
                missing_response
                    .get("result")
                    .and_then(|res| res.get("structuredContent"))
                    .and_then(|sc| sc.get("code"))
                    .and_then(Value::as_str),
                Some("unknown_tool"),
                "미등록 도구는 unknown_tool"
            );

            let records = ledger.records();
            assert_eq!(records.len(), 5_usize, "거부된 호출도 모두 원장에 기록");
            for rec in &records {
                assert!(
                    !rec.arguments_json().as_str().contains(&raw_token)
                        && !rec.result_json().as_str().contains(&raw_token),
                    "원장에 원문 토큰이 남아서는 안 된다"
                );
            }
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn pagination_enforces_30k_cap_across_pages() {
        runtime().block_on(async {
            let base = TempBaseDir::new("page");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cursors = make_cursor_store(Arc::clone(&clock));
            let items: Vec<String> = (0_usize..4_usize)
                .map(|idx| format!("row-{idx}-{}", "a".repeat(12_000_usize)))
                .collect();
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(PagedListTool::new(Arc::clone(&cursors), items)))
                .expect("목록 도구 등록");
            let handle = start_tool_server(
                &base,
                "run-page-1",
                clock,
                Arc::new(registry),
                ToolLedgerMode::Configured(Arc::new(MemoryToolCallLedger::recording())),
            );
            let (mut reader, mut writer) = connect_and_auth(&handle).await;

            let page1 = send_rpc(
                &mut reader,
                &mut writer,
                &call_tool_json(1_u64, "list_demo_items", r#"{"limit":4}"#),
            )
            .await;
            let text1 = page1
                .get("result")
                .and_then(|res| res.get("content"))
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(Value::as_str)
                .expect("1페이지 text");
            assert!(
                text1.chars().count() <= MAX_TOOL_RESPONSE_CHARS,
                "3만 자 상한"
            );
            let next_cursor = page1
                .get("result")
                .and_then(|res| res.get("structuredContent"))
                .and_then(|sc| sc.get("next_cursor"))
                .and_then(Value::as_str)
                .expect("커서");

            let page2_args = format!(r#"{{"limit":4,"cursor":"{next_cursor}"}}"#);
            let page2 = send_rpc(
                &mut reader,
                &mut writer,
                &call_tool_json(2_u64, "list_demo_items", &page2_args),
            )
            .await;
            assert_eq!(
                page2
                    .get("result")
                    .and_then(|res| res.get("structuredContent"))
                    .and_then(|sc| sc.get("next_cursor")),
                Some(&Value::Null),
                "마지막 페이지 next_cursor는 null"
            );
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn oversized_tool_response_fails_closed() {
        runtime().block_on(async {
            let base = TempBaseDir::new("huge");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(OversizedTool::new()))
                .expect("초과 도구 등록");
            let handle = start_tool_server(
                &base,
                "run-huge-1",
                clock,
                Arc::new(registry),
                ToolLedgerMode::Configured(Arc::new(MemoryToolCallLedger::recording())),
            );
            let (mut reader, mut writer) = connect_and_auth(&handle).await;
            let huge = send_rpc(
                &mut reader,
                &mut writer,
                &call_tool_json(3_u64, "get_oversized_blob", "{}"),
            )
            .await;
            assert_eq!(
                huge.get("result")
                    .and_then(|res| res.get("isError"))
                    .and_then(Value::as_bool),
                Some(true),
                "초과 응답은 isError=true"
            );
            assert_eq!(
                huge.get("result")
                    .and_then(|res| res.get("structuredContent"))
                    .and_then(|sc| sc.get("code"))
                    .and_then(Value::as_str),
                Some("response_too_large"),
                "초과 응답은 response_too_large"
            );
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn ledger_failure_fails_closed_with_ledger_unavailable() {
        runtime().block_on(async {
            let base = TempBaseDir::new("ledger-fail");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(EchoReadTool::new(
                    "echo_read",
                    ToolAccessKind::ReadOnly,
                )))
                .expect("등록");
            let failing_ledger = Arc::new(MemoryToolCallLedger::default());
            failing_ledger
                .script_record()
                .push(MemoryReply::Return(Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::Unavailable(RetryDelay::Unknown),
                ))));
            let handle = start_tool_server(
                &base,
                "run-ledger-fail",
                clock,
                Arc::new(registry),
                ToolLedgerMode::Configured(failing_ledger),
            );
            let (mut reader, mut writer) = connect_and_auth(&handle).await;
            let request = call_tool_json(1_u64, "echo_read", r#"{"symbol":"AAPL"}"#);
            let response = send_rpc(&mut reader, &mut writer, &request).await;
            assert_eq!(
                response
                    .get("result")
                    .and_then(|res| res.get("isError"))
                    .and_then(Value::as_bool),
                Some(true),
                "원장 기록 실패 시 닫힌 실패"
            );
            assert_eq!(
                response
                    .get("result")
                    .and_then(|res| res.get("structuredContent"))
                    .and_then(|sc| sc.get("code"))
                    .and_then(Value::as_str),
                Some("ledger_unavailable"),
                "원장 오류 코드 ledger_unavailable"
            );
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn unconfigured_ledger_fails_closed_with_ledger_unavailable() {
        runtime().block_on(async {
            let base = TempBaseDir::new("ledger-none");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(EchoReadTool::new(
                    "echo_read",
                    ToolAccessKind::ReadOnly,
                )))
                .expect("등록");
            let inspector = Arc::new(ScriptedPeerProcessInspector::new(vec![Ok(
                PeerProcessInfo::new(expected_parent_pid(), expected_exe_path()),
            )]));
            let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 사양");
            let deps = RunServerDeps::new(
                clock,
                PeerInspectorMode::Configured(inspector),
                McpServerConfig::default_config().expect("기본 설정"),
            )
            .with_tools(
                Arc::new(registry),
                ToolLedgerMode::<UnconfiguredToolLedger>::Unconfigured,
            );
            let handle = McpServerHandle::start(RunServerRequest::new(
                RunIdentity::new(
                    AiRunId::try_new("run-ledger-none").expect("run id"),
                    RunKind::Analysis,
                ),
                spec,
                PeerProcessTarget::with_parent(expected_parent_pid(), expected_exe_path()),
                deps,
            ))
            .expect("미구성 원장 서버 시작");
            let (mut reader, mut writer) = connect_and_auth(&handle).await;
            let request = call_tool_json(1_u64, "echo_read", r#"{"symbol":"AAPL"}"#);
            let response = send_rpc(&mut reader, &mut writer, &request).await;
            assert_eq!(
                response
                    .get("result")
                    .and_then(|res| res.get("structuredContent"))
                    .and_then(|sc| sc.get("code"))
                    .and_then(Value::as_str),
                Some("ledger_unavailable"),
                "미구성 원장은 닫힌 실패로 거부"
            );
            let _events = handle.shutdown().await;
        });
    }

    #[test]
    fn error_payload_sanitizes_control_chars() {
        let payload = ToolErrorPayload::non_retryable(
            ToolErrorCode::InvalidInput,
            Some("bad field"),
            "line1\n\u{0000}line2",
        );
        let value = payload.to_json_value();
        assert_eq!(value.get("field"), None, "잘못된 필드 이름은 제거한다");
        assert_eq!(
            value.get("message").and_then(Value::as_str),
            Some("line1line2"),
            "제어 문자를 제거한 고정 오류 메시지"
        );
    }
}
