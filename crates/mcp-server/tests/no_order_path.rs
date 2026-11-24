#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use application::{AiRunId, ToolName};
    use application::{
        ArticleBody, ArticleLocation, CollectedItemHandle, ThesisRunMetadata,
        WebResearchToolOutcome,
    };
    use application::{EventBusBuilder, RetryDelay, SystemFailure, SystemPort, SystemPortError};
    use domain::EstimateAssignments;
    use domain::{
        InformationSource, InstrumentId, Nanosecond, NewsSource, NormalizedUrl, OffsetSeconds,
        Symbol, Timestamp, UnixSeconds, UntrustedText,
    };
    use mcp_server::{
        AnalysisCatalogPorts, EstimateToolPorts, ToolErrorCode, ToolInvocation, ToolRegistry,
        build_analysis_catalog, make_cursor_store, validate_arguments_against_schema,
    };
    use proptest::prelude::{BoxedStrategy, Just, Strategy, any, prop, prop_oneof};
    use proptest::test_runner::{Config, TestCaseError, TestRunner};
    use serde_json::{Map, Value};
    use testkit_ports::SteppedClock;
    use testkit_ports::memory::MemoryEstimateLedger;
    use testkit_ports::system_fakes::MemoryToolCallLedger;
    use testkit_ports::tool_read_fakes::{
        ArticleCall, FixedArticleToolPort, FixedEstimateAssignments, FixedEstimateRunContext,
        FixedMarketToolReads, FixedNewsToolReads, FixedPortfolioToolReads, FixedProfileSource,
        FixedScorecardObservations, FixedThesisToolPort, FixedWebResearchToolPort,
        LatestThesisRepository,
    };

    const STATE_CHANGING_ALLOWED: [&str; 3] =
        ["request_web_research", "submit_estimate", "update_thesis"];

    const FORBIDDEN_FRAGMENTS: [&str; 16] = [
        "order",
        "trade",
        "buy",
        "sell",
        "approve",
        "approval",
        "confirm",
        "cancel",
        "setting",
        "config",
        "secret",
        "keychain",
        "key",
        "token",
        "credential",
        "policy",
    ];

    const PROBE_KEYS: [&str; 12] = [
        "side", "quantity", "price", "order", "account", "approve", "token", "setting", "symbol",
        "market", "kind", "period",
    ];

    struct Catalog {
        registry: ToolRegistry,
        portfolio: Arc<FixedPortfolioToolReads>,
        market: Arc<FixedMarketToolReads>,
        news: Arc<FixedNewsToolReads>,
        article: Arc<FixedArticleToolPort>,
    }

    fn research_fake() -> FixedWebResearchToolPort {
        let known: Vec<Symbol> = ["005930", "000660", "AAPL", "035420"]
            .into_iter()
            .map(|code| Symbol::try_new(code).expect("종목"))
            .collect();
        FixedWebResearchToolPort::new(
            Ok(known),
            Ok(WebResearchToolOutcome {
                summary: UntrustedText::try_new("조사 요약").expect("요약"),
                collected_at: Timestamp::from_parts(
                    UnixSeconds::new(1_764_000_000_i64),
                    Nanosecond::new(0_u32),
                    OffsetSeconds::new(0_i32),
                )
                .expect("시각"),
            }),
        )
    }

    type TestEstimatePorts = EstimateToolPorts<
        FixedEstimateAssignments,
        MemoryEstimateLedger,
        MemoryToolCallLedger,
        FixedEstimateRunContext,
        FixedProfileSource,
        FixedScorecardObservations,
        SteppedClock,
    >;

    fn estimate_ports(clock: &Arc<SteppedClock>) -> Arc<TestEstimatePorts> {
        Arc::new(EstimateToolPorts {
            assignments: Arc::new(FixedEstimateAssignments::new(EstimateAssignments::default())),
            submissions: Arc::new(MemoryEstimateLedger::new()),
            history: Arc::new(MemoryToolCallLedger::recording()),
            context: Arc::new(FixedEstimateRunContext(Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            )))),
            settings: Arc::new(FixedProfileSource(Ok(None))),
            observations: Arc::new(FixedScorecardObservations(Ok(Vec::new()))),
            clock: Arc::clone(clock),
            events: Arc::new(EventBusBuilder::default().build()),
        })
    }

    fn catalog() -> Catalog {
        let clock = Arc::new(SteppedClock::new(
            Timestamp::from_parts(
                UnixSeconds::new(1_764_000_000_i64),
                Nanosecond::new(0_u32),
                OffsetSeconds::new(0_i32),
            )
            .expect("시각"),
        ));
        let ports = AnalysisCatalogPorts {
            portfolio: Arc::new(FixedPortfolioToolReads::default()),
            market: Arc::new(FixedMarketToolReads::default()),
            news: Arc::new(FixedNewsToolReads::default()),
            research: Arc::new(research_fake()),
            article: Arc::new(FixedArticleToolPort::new(
                vec![(
                    CollectedItemHandle::try_new("press-ok").expect("핸들"),
                    ArticleLocation {
                        source: InformationSource::News(NewsSource::Hankyung),
                        url: NormalizedUrl::try_new("https://www.hankyung.com/a").expect("주소"),
                    },
                )],
                vec![NewsSource::Hankyung],
                Ok(ArticleBody {
                    text: UntrustedText::try_new("본문").expect("본문"),
                    fetched_at: Timestamp::from_parts(
                        UnixSeconds::new(1_764_000_000_i64),
                        Nanosecond::new(0_u32),
                        OffsetSeconds::new(0_i32),
                    )
                    .expect("시각"),
                }),
            )),
            thesis: Arc::new(FixedThesisToolPort::new(
                vec![(
                    Symbol::try_new("005930").expect("종목"),
                    InstrumentId::try_new("KR7005930003").expect("종목 ID"),
                )],
                Vec::new(),
                ThesisRunMetadata {
                    model: UntrustedText::try_new("model-x").expect("모델"),
                    prompt_version: UntrustedText::try_new("p1").expect("프롬프트"),
                },
            )),
            thesis_repository: Arc::new(LatestThesisRepository::default()),
            estimates: estimate_ports(&clock),
        };
        let registry =
            build_analysis_catalog(&ports, &clock, &make_cursor_store(Arc::clone(&clock)))
                .expect("카탈로그 조립");
        Catalog {
            registry,
            portfolio: ports.portfolio,
            market: ports.market,
            news: ports.news,
            article: ports.article,
        }
    }

    fn leaf_value() -> BoxedStrategy<Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::from),
            any::<u64>().prop_map(Value::from),
            (-1.0e12_f64..1.0e12_f64).prop_map(Value::from),
            "[ -~가-힣]{0,40}".prop_map(Value::String),
            prop::sample::select(vec![
                "KR",
                "US",
                "005930",
                "AAPL",
                "all",
                "trades",
                "1y",
                "3m",
                "gainers",
                "buy",
                "sell",
                "place_order",
            ])
            .prop_map(|text| Value::String(text.to_owned())),
        ]
        .boxed()
    }

    fn any_value() -> BoxedStrategy<Value> {
        leaf_value()
            .prop_recursive(3_u32, 24_u32, 4_u32, |inner| {
                prop_oneof![
                    prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
                    prop::collection::btree_map("[a-z_]{1,8}", inner, 0..4)
                        .prop_map(|map| Value::Object(map.into_iter().collect())),
                ]
            })
            .boxed()
    }

    fn arguments() -> BoxedStrategy<Map<String, Value>> {
        let key = prop_oneof![
            prop::sample::select(PROBE_KEYS.to_vec()).prop_map(str::to_owned),
            "[a-z_]{1,10}",
            Just(String::from("days")),
            Just(String::from("limit")),
            Just(String::from("cursor")),
        ];
        prop::collection::btree_map(key, any_value(), 0..5)
            .prop_map(|map| map.into_iter().collect())
            .boxed()
    }

    fn value_for_schema(schema: &Value) -> BoxedStrategy<Value> {
        if let Some(values) = schema.get("enum").and_then(Value::as_array) {
            return prop::sample::select(values.clone()).boxed();
        }
        match schema.get("type").and_then(Value::as_str) {
            Some("string") => prop_oneof![
                3 => prop::sample::select(vec!["005930", "000660", "AAPL", "035420", "press-ok"])
                    .prop_map(|text| Value::String(text.to_owned())),
                1 => "[ -~]{0,16}".prop_map(Value::String),
            ]
            .boxed(),
            Some("integer") => {
                let low = schema
                    .get("minimum")
                    .and_then(Value::as_i64)
                    .unwrap_or(0_i64);
                let high = schema
                    .get("maximum")
                    .and_then(Value::as_i64)
                    .unwrap_or(1_000_i64);
                prop_oneof![
                    4 => (low..=high).prop_map(Value::from),
                    1 => any::<i64>().prop_map(Value::from),
                ]
                .boxed()
            }
            Some("array") => {
                let item = schema.get("items").cloned().unwrap_or(Value::Null);
                let low = schema
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0_usize);
                let high = schema
                    .get("maxItems")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(4_usize);
                prop::collection::vec(value_for_schema(&item), low..=high)
                    .prop_map(Value::Array)
                    .boxed()
            }
            Some("object") => match schema.as_object() {
                Some(nested) => schema_arguments(nested).prop_map(Value::Object).boxed(),
                None => any_value(),
            },
            Some(_) | None => any_value(),
        }
    }

    fn schema_arguments(schema: &Map<String, Value>) -> BoxedStrategy<Map<String, Value>> {
        let required: Vec<String> = schema
            .get("required")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            });
        let properties: Vec<(String, Value)> = schema
            .get("properties")
            .and_then(Value::as_object)
            .map_or_else(Vec::new, |props| {
                props
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            });
        let fields: Vec<BoxedStrategy<Option<(String, Value)>>> = properties
            .into_iter()
            .map(|(key, prop_schema)| {
                let must = required.contains(&key);
                (any::<bool>(), value_for_schema(&prop_schema))
                    .prop_map(move |(include, value)| {
                        (must || include).then(|| (key.clone(), value))
                    })
                    .boxed()
            })
            .collect();
        (
            fields,
            prop::option::weighted(
                0.1_f64,
                (prop::sample::select(PROBE_KEYS.to_vec()), any_value()),
            ),
        )
            .prop_map(|(entries, probe)| {
                let mut map: Map<String, Value> = entries.into_iter().flatten().collect();
                if let Some((key, value)) = probe {
                    map.insert(key.to_owned(), value);
                }
                map
            })
            .boxed()
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn invoke_once(
        rt: &tokio::runtime::Runtime,
        registry: &ToolRegistry,
        name: &ToolName,
        args: &Map<String, Value>,
    ) -> Result<Reach, TestCaseError> {
        let (spec, tool) = registry
            .get(name)
            .ok_or_else(|| TestCaseError::fail("등록된 도구"))?;
        if validate_arguments_against_schema(args, spec.input_schema()).is_err() {
            return Ok(Reach::RejectedBySchema);
        }
        let run_id = AiRunId::try_new("run-m05").map_err(|_err| TestCaseError::fail("실행 ID"))?;
        let outcome = rt.block_on(tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: args,
        }));
        match outcome {
            Ok(value) => {
                if value.is_object() {
                    Ok(Reach::Body)
                } else {
                    Err(TestCaseError::fail("성공 결과는 객체여야 한다"))
                }
            }
            Err(error) => match error.code() {
                ToolErrorCode::InternalError => {
                    Err(TestCaseError::fail("도구가 내부 오류를 내면 안 된다"))
                }
                ToolErrorCode::UnknownTool
                | ToolErrorCode::InvalidInput
                | ToolErrorCode::InvalidCursor
                | ToolErrorCode::CursorExpired
                | ToolErrorCode::ResponseTooLarge
                | ToolErrorCode::LedgerUnavailable
                | ToolErrorCode::DataUnavailable
                | ToolErrorCode::UpstreamUnavailable
                | ToolErrorCode::ForbiddenOperation
                | ToolErrorCode::RateLimited => Ok(Reach::Body),
            },
        }
    }

    const fn matches_fetch(call: &ArticleCall) -> bool {
        match call {
            ArticleCall::FetchBody(_) => true,
            ArticleCall::Locate(_) | ArticleCall::BodyReading(_) => false,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Reach {
        RejectedBySchema,
        Body,
    }

    #[test]
    fn every_registered_tool_survives_random_arguments_and_reaches_its_body() {
        let catalog = catalog();
        let rt = runtime();
        let names = catalog.registry.tool_names();
        assert!(!names.is_empty(), "카탈로그가 비어 있으면 안 된다");
        for name in &names {
            let (spec, _tool) = catalog.registry.get(name).expect("등록된 도구");
            let strategy = prop_oneof![
                1 => arguments(),
                3 => schema_arguments(spec.input_schema()),
            ];
            let reached = std::cell::Cell::new(0_u32);
            let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
            runner
                .run(&strategy, |args| {
                    let reach = invoke_once(&rt, &catalog.registry, name, &args)?;
                    reached.set(
                        reached
                            .get()
                            .saturating_add(u32::from(reach == Reach::Body)),
                    );
                    Ok(())
                })
                .expect("무작위 인자 호출");
            assert!(
                reached.get() > 0_u32,
                "{}: 무작위 인자 일부는 검증을 지나 도구 본문까지 닿아야 한다",
                name.as_str()
            );
        }
        let port_calls = catalog
            .portfolio
            .calls()
            .len()
            .saturating_add(catalog.market.calls().len())
            .saturating_add(catalog.news.calls().len());
        assert!(port_calls > 0_usize, "읽기 포트까지 호출이 닿아야 한다");
        assert!(
            catalog.article.calls().iter().any(matches_fetch),
            "read_article 본문 경로도 무작위 인자로 돌아야 한다"
        );
    }

    #[test]
    fn tools_list_has_no_order_approval_setting_or_key_tools() {
        let tools = catalog().registry.list_rmcp_tools();
        for tool in &tools {
            let name = tool.name.to_ascii_lowercase();
            for fragment in FORBIDDEN_FRAGMENTS {
                assert!(
                    !name.contains(fragment),
                    "{name}: 주문·승인·설정·키 도구는 MCP에 없다 ({fragment})"
                );
            }
            let annotations = tool.annotations.as_ref().expect("주석");
            let read_only = annotations.read_only_hint == Some(true);
            assert!(
                read_only || STATE_CHANGING_ALLOWED.contains(&name.as_str()),
                "{name}: 상태를 바꾸는 도구는 고정 허용 목록에만 있다"
            );
            assert_eq!(annotations.destructive_hint, Some(false), "{name}");
        }
    }

    fn workspace_root() -> PathBuf {
        let exe = testkit::test_executable().expect("test executable");
        exe.ancestors()
            .find(|dir| {
                dir.join("crates")
                    .join("mcp-server")
                    .join("Cargo.toml")
                    .is_file()
            })
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn mcp_server_manifest_has_no_order_crate_dependency() {
        let manifest = fs::read_to_string(
            workspace_root()
                .join("crates")
                .join("mcp-server")
                .join("Cargo.toml"),
        )
        .expect("매니페스트");
        for forbidden in [
            "order-execution",
            "broker-toss-trade",
            "approval-command",
            "trade-fakes",
            "broker-toss",
            "secrets-keychain",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "mcp-server는 {forbidden}에 의존하지 않는다"
            );
        }
    }
}
