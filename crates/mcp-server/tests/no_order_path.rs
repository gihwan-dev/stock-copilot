#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use application::{AiRunId, ToolName};
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        AnalysisCatalogPorts, ToolErrorCode, ToolInvocation, ToolRegistry, build_analysis_catalog,
        make_cursor_store, validate_arguments_against_schema,
    };
    use proptest::prelude::{BoxedStrategy, Just, Strategy, any, prop, prop_oneof};
    use proptest::test_runner::{Config, TestCaseError, TestRunner};
    use serde_json::{Map, Value};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::{
        FixedMarketToolReads, FixedNewsToolReads, FixedPortfolioToolReads, MarketReadCall,
        NewsReadCall, PortfolioReadCall,
    };

    const STATE_CHANGING_ALLOWED: [&str; 0] = [];

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
        };
        let registry =
            build_analysis_catalog(&ports, &make_cursor_store(clock)).expect("카탈로그 조립");
        Catalog {
            registry,
            portfolio: ports.portfolio,
            market: ports.market,
            news: ports.news,
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
    ) -> Result<(), TestCaseError> {
        let (spec, tool) = registry
            .get(name)
            .ok_or_else(|| TestCaseError::fail("등록된 도구"))?;
        if validate_arguments_against_schema(args, spec.input_schema()).is_err() {
            return Ok(());
        }
        let run_id = AiRunId::try_new("run-m05").map_err(|_err| TestCaseError::fail("실행 ID"))?;
        let outcome = rt.block_on(tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: args,
        }));
        match outcome {
            Ok(value) => {
                if value.is_object() {
                    Ok(())
                } else {
                    Err(TestCaseError::fail("성공 결과는 객체여야 한다"))
                }
            }
            Err(error) => match error.code() {
                ToolErrorCode::ForbiddenOperation | ToolErrorCode::InternalError => Err(
                    TestCaseError::fail("읽기 도구가 금지·내부 오류를 내면 안 된다"),
                ),
                ToolErrorCode::UnknownTool
                | ToolErrorCode::InvalidInput
                | ToolErrorCode::InvalidCursor
                | ToolErrorCode::CursorExpired
                | ToolErrorCode::ResponseTooLarge
                | ToolErrorCode::LedgerUnavailable
                | ToolErrorCode::DataUnavailable
                | ToolErrorCode::UpstreamUnavailable => Ok(()),
            },
        }
    }

    #[test]
    fn every_registered_tool_survives_random_arguments_with_read_only_port_calls() {
        let catalog = catalog();
        let rt = runtime();
        let names = catalog.registry.tool_names();
        assert!(!names.is_empty(), "카탈로그가 비어 있으면 안 된다");
        for name in &names {
            let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
            runner
                .run(&arguments(), |args| {
                    invoke_once(&rt, &catalog.registry, name, &args)
                })
                .expect("무작위 인자 호출");
        }
        assert!(
            !catalog.portfolio.calls().is_empty() && !catalog.market.calls().is_empty(),
            "무작위 인자 일부는 검증을 지나 도구 본문까지 닿아야 한다"
        );
        for call in catalog.portfolio.calls() {
            match call {
                PortfolioReadCall::Portfolio
                | PortfolioReadCall::Allocation
                | PortfolioReadCall::Transactions
                | PortfolioReadCall::Performance(_) => {}
            }
        }
        assert!(
            !catalog.news.calls().is_empty(),
            "뉴스 도구 본문까지 닿아야 한다"
        );
        for call in catalog.news.calls() {
            match call {
                NewsReadCall::SearchNews(_) | NewsReadCall::Disclosures(_) => {}
            }
        }
        for call in catalog.market.calls() {
            match call {
                MarketReadCall::Quote(..)
                | MarketReadCall::Candles(..)
                | MarketReadCall::Overview
                | MarketReadCall::Rankings(..)
                | MarketReadCall::Sectors(..)
                | MarketReadCall::InvestorFlow(..) => {}
            }
        }
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
