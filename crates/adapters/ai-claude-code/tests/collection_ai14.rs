#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, ClaudeCodeProvider, ClaudeWebCollectionRunner, HostEnvironment,
        WEB_COLLECTION_JSON_SCHEMA, parse_web_findings,
    };
    use application::{
        CollectionQuery, CollectionTopic, EventBusBuilder, EventSubscriber, RunStockWebCollection,
        StockWebCollectionOutcome, SubscriberFailure, SystemPortError, WebCollectionFailure,
        WebCollectionReservation, WebCollectionRunner, WebCollectionStore, WriteCompleted,
        plan_stock_web_collection,
    };
    use domain::{
        CollectedItem, DomainEvent, DomainEventKind, InformationSource, Nanosecond, OffsetSeconds,
        Symbol, Timestamp, UnixSeconds,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, mpsc};
    use testkit::fake_ai_cli::{
        EnvPresence, FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn,
        TurnStop,
    };
    use testkit_ports::SteppedClock;

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai14-{}-{seq}-{label}", std::process::id()));
            fs::create_dir_all(&path).expect("create dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn run_async<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(future)
    }

    fn init_line(mcp_servers: &Value) -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["WebSearch"],
            "mcp_servers": mcp_servers,
            "model": "claude-haiku-5-5",
            "permissionMode": "dontAsk",
            "slash_commands": [],
            "skills": [],
            "plugins": ["cc-plugin-privacy"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.322"
        })
        .to_string()
    }

    fn turn(lines: Vec<String>) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: lines,
            stderr_lines: Vec::new(),
            stop_after: TurnStop::Continue,
        }
    }

    fn scenario(init: String, answer: Vec<String>) -> FakeAiScenario {
        let zero =
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0_i32})
                .to_string();
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns: vec![turn(vec![init, zero]), turn(answer)],
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    fn result_with(output: Option<Value>) -> String {
        let mut line =
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1_i32});
        if let (Some(value), Some(object)) = (output, line.as_object_mut()) {
            object.insert(String::from("structured_output"), value);
        }
        line.to_string()
    }

    fn good_output() -> Value {
        json!({"items": [
            {"url": "https://news.example.com/a", "title": "실적 발표", "summary": "요약"},
            {"url": "not a url", "title": "버림"},
            {"url": "https://news.example.com/b", "title": "   "},
            {"url": "https://news.example.com/c", "title": "공시 정리"}
        ]})
    }

    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }

    fn provider(prepared: &PreparedFakeAiCli, root: &Path) -> ClaudeCodeProvider {
        let config = ClaudeCodeConfig::new(
            prepared.executable_path().to_path_buf(),
            root.join("work"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_allowed_cli_versions(BTreeSet::from([String::from("2.1.322")]))
        .with_host_environment(HostEnvironment::new_clean(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        ))
        .with_collection_json_schema(String::from(WEB_COLLECTION_JSON_SCHEMA))
        .with_cli_projects_dir(root.join("projects"))
        .with_preflight_reap_grace_ms(10_000_u64);
        ClaudeCodeProvider::new(config)
    }

    fn runner(prepared: &PreparedFakeAiCli, root: &Path) -> ClaudeWebCollectionRunner {
        ClaudeWebCollectionRunner::new(
            Arc::new(provider(prepared, root)),
            Arc::new(SteppedClock::new(at(1_700_000_000))),
        )
    }

    fn query() -> CollectionQuery {
        CollectionQuery::CodeTopics {
            instruments: vec![Symbol::try_new("005930").expect("symbol")],
            topics: vec![CollectionTopic::CompanyNews, CollectionTopic::Earnings],
        }
    }

    fn sent_prompts(prepared: &PreparedFakeAiCli) -> Vec<String> {
        let records = prepared.read_invocations().expect("invocations");
        let record = records.first().expect("one invocation");
        assert_eq!(
            record.env_observation.mcp_token,
            EnvPresence::Absent,
            "수집 실행에 토큰 없음"
        );
        record
            .stdin_lines
            .iter()
            .map(|line| {
                let value: Value = serde_json::from_str(line).expect("json line");
                String::from(value["message"]["content"].as_str().expect("content"))
            })
            .collect()
    }

    fn invocation_argv(prepared: &PreparedFakeAiCli) -> Vec<String> {
        prepared
            .read_invocations()
            .expect("invocations")
            .first()
            .expect("one")
            .argv
            .clone()
    }

    #[test]
    fn market_outlook_stream_carries_only_dated_tool_results_for_the_public_query() {
        use application::{
            AiEventStream, AiProvider, AiRunId, ModelSelection, ReasoningEffort, RunInput,
            RunPurpose, RunSpec,
        };
        use domain::{
            WebSearchCandidateQuery, WebSearchInspection, WebSearchSensitiveContext,
            inspect_web_search_query,
        };
        run_async(async {
            let dir = TempDirGuard::new("outlook");
            let candidate = WebSearchCandidateQuery::new("Korean stock market KOSPI week");
            let WebSearchInspection::Approved(approved) =
                inspect_web_search_query(&candidate, &WebSearchSensitiveContext::new())
            else {
                panic!("공개 질의")
            };
            let links = r#"Links: [{"title":"Index close","url":"https://news.example/close","observed_at":"2026-09-20"}]"#;
            let answer = vec![
                json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"web","name":"WebSearch","input":{"query":candidate.as_str()}}]}}).to_string(),
                json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"web","content":[{"type":"text","text":links}]}]}}).to_string(),
                result_with(Some(good_output())),
            ];
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-outlook",
                &scenario(init_line(&json!([])), answer),
            )
            .expect("가상 CLI");
            let spec = RunSpec::new(
                AiRunId::try_new("outlook").expect("실행"),
                RunInput::Collection(CollectionQuery::MarketOutlook(approved)),
                ModelSelection::Fast,
                ReasoningEffort::Low,
                RunPurpose::WebCollection,
            );
            let mut stream = provider(&prepared, dir.path())
                .run(spec)
                .await
                .expect("수집");
            while stream.next().await.expect("스트림").is_some() {}
            let [source] = stream.search_observations() else {
                panic!("성공한 도구 결과 하나")
            };
            assert_eq!(source.query.as_untrusted_str(), candidate.as_str());
            assert_eq!(source.url.as_str(), "https://news.example/close");
            assert_eq!(source.title.as_untrusted_str(), "Index close");
            assert_eq!(source.date_kind, application::SourceDateKind::Observed);
            assert!(
                sent_prompts(&prepared)
                    .last()
                    .expect("수집 프롬프트")
                    .ends_with(candidate.as_str())
            );
        });
    }

    #[test]
    fn collection_run_uses_web_search_only_and_returns_untrusted_findings() {
        run_async(async {
            let dir = TempDirGuard::new("ok");
            let answer = vec![result_with(Some(good_output()))];
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-ok",
                &scenario(init_line(&json!([])), answer),
            )
            .expect("prepare");
            let findings = runner(&prepared, dir.path())
                .collect(&query())
                .await
                .expect("findings");
            let urls: Vec<String> = findings
                .iter()
                .map(|finding| String::from(finding.collected().url().as_str()))
                .collect();
            assert_eq!(
                urls,
                vec!["https://news.example.com/a", "https://news.example.com/c"]
            );
            let argv = invocation_argv(&prepared);
            let joined = argv.join(" ");
            assert!(joined.contains("--tools WebSearch"), "{joined}");
            assert_eq!(
                argv.iter()
                    .position(|arg| arg == "--json-schema")
                    .and_then(|index| argv.get(index + 1))
                    .map(String::as_str),
                Some(WEB_COLLECTION_JSON_SCHEMA)
            );
            assert!(argv.iter().any(|arg| arg == "--strict-mcp-config"));
            assert_eq!(
                sent_prompts(&prepared),
                vec![
                    String::from("/usage"),
                    String::from(
                        "Search public web sources for symbols [005930] covering topics [company_news, earnings]. Return structured JSON."
                    ),
                ],
                "프롬프트에는 종목 코드와 주제 항목만"
            );
        });
    }

    #[test]
    fn rejected_isolation_sends_no_prompt() {
        run_async(async {
            let dir = TempDirGuard::new("reject");
            let mcp = json!([{"name": "app", "status": "connected"}]);
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-reject",
                &scenario(init_line(&mcp), vec![result_with(Some(good_output()))]),
            )
            .expect("prepare");
            assert_eq!(
                runner(&prepared, dir.path()).collect(&query()).await,
                Err(WebCollectionFailure::Rejected)
            );
            assert_eq!(sent_prompts(&prepared), vec![String::from("/usage")]);
        });
    }

    #[test]
    fn missing_or_malformed_structured_output_is_unavailable() {
        run_async(async {
            for (label, output) in [
                ("none", None),
                ("shape", Some(json!({"items": "nope"}))),
                (
                    "array",
                    Some(json!([{"url": "https://a.example.com", "title": "t"}])),
                ),
            ] {
                let dir = TempDirGuard::new(label);
                let prepared = PreparedFakeAiCli::prepare(
                    dir.path(),
                    "fake-bad",
                    &scenario(init_line(&json!([])), vec![result_with(output)]),
                )
                .expect("prepare");
                assert_eq!(
                    runner(&prepared, dir.path()).collect(&query()).await,
                    Err(WebCollectionFailure::Unavailable),
                    "{label}"
                );
            }
        });
    }

    #[test]
    fn parser_caps_findings_and_drops_non_string_summary() {
        let many: Vec<Value> = (0_i32..30_i32)
            .map(|index| json!({"url": format!("https://e.example.com/{index}"), "title": "t"}))
            .collect();
        assert_eq!(
            parse_web_findings(&json!({"items": many}))
                .expect("ok")
                .len(),
            20
        );
        let odd =
            json!({"items": [{"url": "https://e.example.com/x", "title": "t", "summary": 3_i32}]});
        assert_eq!(parse_web_findings(&odd).expect("ok").len(), 0);
        assert_eq!(parse_web_findings(&json!({"other": []})), None);
    }

    struct SavedItems(mpsc::Sender<(Symbol, Vec<CollectedItem>)>);

    impl WebCollectionStore for SavedItems {
        fn save(
            &self,
            instrument: &Symbol,
            items: &[CollectedItem],
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.0
                .send((instrument.clone(), items.to_vec()))
                .expect("saved channel");
            std::future::ready(Ok(WriteCompleted))
        }
    }

    struct Seen(mpsc::Sender<DomainEvent>);

    impl EventSubscriber for Seen {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }

    #[test]
    fn stock_collection_saves_web_items_and_announces_by_source_kind() {
        run_async(async {
            let dir = TempDirGuard::new("e2e");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-e2e",
                &scenario(
                    init_line(&json!([])),
                    vec![result_with(Some(good_output()))],
                ),
            )
            .expect("prepare");
            let runner = runner(&prepared, dir.path());
            let (saved_tx, saved_rx) = mpsc::channel();
            let store = SavedItems(saved_tx);
            let (sender, received) = mpsc::channel();
            let mut builder = EventBusBuilder::default();
            builder.subscribe(
                BTreeSet::from([DomainEventKind::CollectionFinished]),
                Seen(sender),
            );
            let events = builder.build();
            let plan: Vec<WebCollectionReservation> =
                plan_stock_web_collection(&[Symbol::try_new("005930").expect("symbol")]);
            let report = RunStockWebCollection {
                runner: &runner,
                store: &store,
                events: &events,
            }
            .run(&plan)
            .await;
            let saved: Vec<(Symbol, Vec<CollectedItem>)> = saved_rx.try_iter().collect();
            assert_eq!(saved.len(), 1);
            assert!(
                saved
                    .iter()
                    .flat_map(|(_, items)| items)
                    .all(|item| item.source() == &InformationSource::Web)
            );
            assert_eq!(
                received.try_recv(),
                Ok(DomainEvent::CollectionFinished(InformationSource::Web))
            );
            let outcome = report.results.first().expect("result").outcome().clone();
            assert_eq!(
                outcome,
                StockWebCollectionOutcome::Saved(
                    saved.into_iter().flat_map(|(_, items)| items).collect()
                )
            );
        });
    }

    #[test]
    fn error_result_structured_output_is_not_accepted() {
        run_async(async {
            let dir = TempDirGuard::new("error-result");
            let error_result = json!({
                "type": "result",
                "subtype": "error_max_turns",
                "is_error": true,
                "num_turns": 3_i32,
                "structured_output": good_output()
            })
            .to_string();
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-error",
                &scenario(init_line(&json!([])), vec![error_result]),
            )
            .expect("prepare");
            assert_eq!(
                runner(&prepared, dir.path()).collect(&query()).await,
                Err(WebCollectionFailure::Unavailable)
            );
        });
    }
}
