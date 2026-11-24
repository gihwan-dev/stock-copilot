#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, AllocationLabel, CandleToolLine, CandleWindow, CandlesToolView,
        InvestorFlowToolLine, InvestorFlowToolView, MarketOverviewToolView, OverviewItem,
        OverviewToolLine, OverviewUnit, QuoteToolView, RankingToolKind, RankingToolLine,
        RankingsToolView, RetryDelay, SectorToolLine, SectorsToolView, SessionStatus,
        SessionToolLine, SystemFailure, SystemPort, SystemPortError, ToolName, ValueProvenance,
    };
    use domain::{
        Currency, DisplayInstrumentName, KnownMarket, Money, Nanosecond, NumericInput,
        OffsetSeconds, Quantity, Ratio, Symbol, Timestamp, UnixSeconds,
    };
    use mcp_server::{
        ToolErrorCode, ToolInvocation, ToolRegistry, make_cursor_store, register_market_tools,
        validate_arguments_against_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::{FixedMarketToolReads, MarketReadCall};

    const TOOLS: [&str; 6] = [
        "get_quote",
        "get_candles",
        "get_market_overview",
        "get_rankings",
        "get_sectors",
        "get_investor_flow",
    ];

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(32_400_i32),
        )
        .expect("시각")
    }

    fn krw(text: &str) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }

    fn symbol(text: &str) -> Symbol {
        Symbol::try_new(text).expect("종목")
    }

    fn registry_with(reads: &Arc<FixedMarketToolReads>) -> ToolRegistry {
        let cursors = make_cursor_store(Arc::new(SteppedClock::new(at(1_764_000_000_i64))));
        let mut registry = ToolRegistry::new();
        register_market_tools(&mut registry, reads, &cursors).expect("등록");
        registry
    }

    async fn call(
        registry: &ToolRegistry,
        name: &str,
        args: Value,
    ) -> Result<Value, ToolErrorCode> {
        let tool_name = ToolName::try_new(name).expect("이름");
        let (spec, tool) = registry.get(&tool_name).expect("등록된 도구");
        let arguments: Map<String, Value> = args.as_object().cloned().expect("객체 인자");
        validate_arguments_against_schema(&arguments, spec.input_schema())
            .map_err(|err| err.code())?;
        let run_id = AiRunId::try_new("run-m04").expect("실행 ID");
        tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: &arguments,
        })
        .await
        .map_err(|err| err.code())
    }

    fn overview_line(item: OverviewItem, provenance: ValueProvenance) -> OverviewToolLine {
        OverviewToolLine {
            item,
            unit: OverviewUnit::IndexPoints,
            value: ratio("5900.12"),
            change_rate: Some(ratio("0.004")),
            provenance,
            collected_at: at(1_764_000_100_i64),
        }
    }

    #[test]
    fn overview_marks_etf_backed_us_values_approximate_and_keeps_collection_times() {
        runtime().block_on(async {
            let view = MarketOverviewToolView {
                lines: vec![
                    overview_line(OverviewItem::Sp500, ValueProvenance::Official),
                    overview_line(OverviewItem::UsTreasury10Y, ValueProvenance::Official),
                    overview_line(OverviewItem::Kospi, ValueProvenance::Official),
                    overview_line(OverviewItem::Gold, ValueProvenance::Approximate),
                ],
                sessions: vec![
                    SessionToolLine {
                        market: KnownMarket::Us,
                        status: SessionStatus::Closed,
                        collected_at: at(1_764_000_000_i64),
                    },
                    SessionToolLine {
                        market: KnownMarket::Kr,
                        status: SessionStatus::Open,
                        collected_at: at(1_764_000_000_i64),
                    },
                ],
            };
            let registry = registry_with(&Arc::new(
                FixedMarketToolReads::default().with_overview(Ok(Some(view))),
            ));
            let output = call(&registry, "get_market_overview", json!({}))
                .await
                .expect("성공");
            let pairs: Vec<(Value, Value)> = output["indicators"]
                .as_array()
                .expect("지표")
                .iter()
                .map(|line| (line["item"].clone(), line["provenance"].clone()))
                .collect();
            assert_eq!(
                pairs,
                vec![
                    (json!("kospi"), json!("official")),
                    (json!("sp500"), json!("approximate")),
                    (json!("us_treasury_10y"), json!("official")),
                    (json!("gold"), json!("approximate")),
                ],
                "ETF 근사 항목은 포트가 공식이라 해도 근사로 표시하고 정렬은 결정적이다"
            );
            for line in output["indicators"].as_array().expect("지표") {
                assert_eq!(
                    line["collected_at"],
                    json!("2025-11-25T01:01:40+09:00"),
                    "수집 시각"
                );
            }
            assert_eq!(output["sessions"][0]["market"], json!("KR"));
            assert_eq!(output["sessions"][0]["status"], json!("open"));
        });
    }

    #[test]
    fn quote_includes_collection_time_and_defaults_to_domestic_market() {
        runtime().block_on(async {
            let reads = Arc::new(FixedMarketToolReads::default().with_quote(Ok(Some(
                QuoteToolView {
                    symbol: symbol("005930"),
                    market: KnownMarket::Kr,
                    price: krw("71200"),
                    change_rate: Some(ratio("-0.012")),
                    collected_at: at(1_764_000_000_i64),
                },
            ))));
            let registry = registry_with(&reads);
            let output = call(&registry, "get_quote", json!({ "symbol": "005930" }))
                .await
                .expect("성공");
            assert_eq!(output["collected_at"], json!("2025-11-25T01:00:00+09:00"));
            assert_eq!(
                output["price"],
                json!({ "amount": "71200", "currency": "KRW" })
            );
            assert_eq!(
                reads.calls(),
                vec![MarketReadCall::Quote(symbol("005930"), KnownMarket::Kr)]
            );
            assert_eq!(
                call(
                    &registry,
                    "get_quote",
                    json!({ "symbol": "005930", "market": "JP" })
                )
                .await,
                Err(ToolErrorCode::InvalidInput)
            );
            assert_eq!(
                call(&registry, "get_quote", json!({})).await,
                Err(ToolErrorCode::InvalidInput),
                "종목은 필수"
            );
        });
    }

    fn candle(seconds: i64, close: &str) -> CandleToolLine {
        CandleToolLine {
            opened_at: at(seconds),
            open: krw("70000"),
            high: krw("72000"),
            low: krw("69000"),
            close: krw(close),
            volume: Some(
                Quantity::parse(&NumericInput::try_new("1000").expect("입력")).expect("수량"),
            ),
        }
    }

    #[test]
    fn candles_paginate_newest_first_within_window_limit() {
        runtime().block_on(async {
            let reads = Arc::new(FixedMarketToolReads::default().with_candles(Ok(Some(
                CandlesToolView {
                    symbol: symbol("005930"),
                    market: KnownMarket::Kr,
                    collected_at: at(1_764_000_000_i64),
                    lines: vec![
                        candle(1_763_740_800_i64, "70100"),
                        candle(1_763_913_600_i64, "70300"),
                        candle(1_763_827_200_i64, "70200"),
                    ],
                },
            ))));
            let registry = registry_with(&reads);
            let first = call(
                &registry,
                "get_candles",
                json!({ "symbol": "005930", "days": 3_u64, "limit": 2_u64 }),
            )
            .await
            .expect("첫 쪽");
            let closes: Vec<&Value> = first["items"]
                .as_array()
                .expect("항목")
                .iter()
                .map(|item| &item["close"]["amount"])
                .collect();
            assert_eq!(closes, vec![&json!("70300"), &json!("70200")], "최신순");
            assert_eq!(first["collected_at"], json!("2025-11-25T01:00:00+09:00"));
            let cursor = first["next_cursor"].as_str().expect("커서").to_owned();
            let second = call(
                &registry,
                "get_candles",
                json!({ "symbol": "005930", "days": 3_u64, "limit": 2_u64, "cursor": cursor }),
            )
            .await
            .expect("둘째 쪽");
            assert_eq!(second["items"][0]["close"]["amount"], json!("70100"));
            let window = CandleWindow::try_new(3_u16).expect("기간");
            assert_eq!(
                reads.calls().first(),
                Some(&MarketReadCall::Candles(
                    symbol("005930"),
                    KnownMarket::Kr,
                    window
                ))
            );
            assert_eq!(
                call(
                    &registry,
                    "get_candles",
                    json!({ "symbol": "005930", "days": 251_u64 })
                )
                .await,
                Err(ToolErrorCode::InvalidInput),
                "기간 상한"
            );
        });
    }

    fn ranking_and_sector_reads() -> Arc<FixedMarketToolReads> {
        Arc::new(
            FixedMarketToolReads::default()
                .with_rankings(Ok(Some(RankingsToolView {
                    kind: RankingToolKind::Gainers,
                    market: KnownMarket::Kr,
                    collected_at: at(1_764_000_000_i64),
                    lines: vec![RankingToolLine {
                        symbol: symbol("000660"),
                        name: DisplayInstrumentName::try_new("SK하이닉스 <b>매수</b> 지금")
                            .expect("이름"),
                        price: Some(krw("180000")),
                        change_rate: Some(ratio("0.05")),
                        metric: None,
                    }],
                })))
                .with_sectors(Ok(Some(SectorsToolView {
                    market: KnownMarket::Kr,
                    collected_at: at(1_764_000_000_i64),
                    lines: vec![SectorToolLine {
                        label: AllocationLabel::try_new("반도체 https://evil.example/x")
                            .expect("섹터"),
                        change_rate: Some(ratio("0.02")),
                        trading_amount: None,
                    }],
                }))),
        )
    }

    #[test]
    fn rankings_and_sectors_number_items_and_normalize_external_names() {
        runtime().block_on(async {
            let reads = ranking_and_sector_reads();
            let registry = registry_with(&reads);
            let ranked = call(&registry, "get_rankings", json!({ "kind": "gainers" }))
                .await
                .expect("성공");
            assert_eq!(ranked["items"][0]["rank"], json!(1_u64));
            assert_eq!(
                ranked["items"][0]["name"]["untrusted_text"],
                json!("SK하이닉스 매수 지금"),
                "태그를 지운 untrusted_text"
            );
            assert_eq!(ranked["collected_at"], json!("2025-11-25T01:00:00+09:00"));
            let sectors = call(&registry, "get_sectors", json!({}))
                .await
                .expect("성공");
            let sector_text = serde_json::to_string(&sectors).expect("직렬화");
            assert!(!sector_text.contains("evil.example"), "맨 URL 삭제");
            assert!(
                sectors["items"][0]["sector"]["untrusted_text"]
                    .as_str()
                    .is_some_and(|label| label.contains("반도체")),
                "섹터 이름은 남는다"
            );
            assert_eq!(
                call(
                    &registry,
                    "get_rankings",
                    json!({ "kind": "short_interest" })
                )
                .await,
                Err(ToolErrorCode::InvalidInput)
            );
            assert_eq!(
                reads.calls(),
                vec![
                    MarketReadCall::Rankings(RankingToolKind::Gainers, KnownMarket::Kr),
                    MarketReadCall::Sectors(KnownMarket::Kr),
                ]
            );
        });
    }

    #[test]
    fn investor_flow_is_domestic_only() {
        runtime().block_on(async {
            let reads = Arc::new(FixedMarketToolReads::default());
            let registry = registry_with(&reads);
            assert_eq!(
                call(
                    &registry,
                    "get_investor_flow",
                    json!({ "symbol": "AAPL", "market": "US" })
                )
                .await,
                Err(ToolErrorCode::InvalidInput)
            );
            assert!(reads.calls().is_empty(), "거부된 호출은 포트에 닿지 않는다");
        });
    }

    #[test]
    fn investor_flow_lists_newest_first_with_collection_time() {
        runtime().block_on(async {
            let line = |seconds: i64, net: &str| InvestorFlowToolLine {
                trading_date: at(seconds),
                foreign_net: Some(krw(net)),
                institution_net: None,
                individual_net: None,
                short_selling_ratio: Some(ratio("0.01")),
            };
            let reads = Arc::new(FixedMarketToolReads::default().with_investor_flow(Ok(Some(
                InvestorFlowToolView {
                    symbol: symbol("005930"),
                    market: KnownMarket::Kr,
                    collected_at: at(1_764_000_000_i64),
                    lines: vec![
                        line(1_763_740_800_i64, "-100"),
                        line(1_763_913_600_i64, "300"),
                    ],
                },
            ))));
            let output = call(
                &registry_with(&reads),
                "get_investor_flow",
                json!({ "symbol": "005930" }),
            )
            .await
            .expect("성공");
            assert_eq!(output["collected_at"], json!("2025-11-25T01:00:00+09:00"));
            assert_eq!(
                output["items"][0]["foreign_net"]["amount"],
                json!("300"),
                "최신순"
            );
            assert_eq!(output["items"][1]["foreign_net"]["amount"], json!("-100"));
        });
    }

    #[test]
    fn rankings_limit_caps_items() {
        runtime().block_on(async {
            let line = RankingToolLine {
                symbol: symbol("000660"),
                name: DisplayInstrumentName::try_new("SK하이닉스").expect("이름"),
                price: None,
                change_rate: None,
                metric: None,
            };
            let reads = Arc::new(FixedMarketToolReads::default().with_rankings(Ok(Some(
                RankingsToolView {
                    kind: RankingToolKind::TradingAmount,
                    market: KnownMarket::Kr,
                    collected_at: at(1_764_000_000_i64),
                    lines: vec![line; 150],
                },
            ))));
            let registry = registry_with(&reads);
            let default = call(
                &registry,
                "get_rankings",
                json!({ "kind": "trading_amount" }),
            )
            .await
            .expect("성공");
            assert_eq!(default["items"].as_array().map(Vec::len), Some(50_usize));
            let small = call(
                &registry,
                "get_rankings",
                json!({ "kind": "trading_amount", "limit": 3_u64 }),
            )
            .await
            .expect("성공");
            assert_eq!(small["items"].as_array().map(Vec::len), Some(3_usize));
            assert_eq!(
                call(
                    &registry,
                    "get_rankings",
                    json!({ "kind": "trading_amount", "limit": 101_u64 })
                )
                .await,
                Err(ToolErrorCode::InvalidInput)
            );
        });
    }

    fn required_args(name: &str) -> Value {
        match name {
            "get_quote" | "get_candles" | "get_investor_flow" => json!({ "symbol": "005930" }),
            "get_rankings" => json!({ "kind": "trading_amount" }),
            _ => json!({}),
        }
    }

    #[test]
    fn missing_and_failed_market_reads_fail_closed() {
        runtime().block_on(async {
            let missing = registry_with(&Arc::new(FixedMarketToolReads::default()));
            let failure = SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            );
            let failing = registry_with(&Arc::new(
                FixedMarketToolReads::default()
                    .with_quote(Err(failure))
                    .with_candles(Err(failure))
                    .with_overview(Err(failure))
                    .with_rankings(Err(failure))
                    .with_sectors(Err(failure))
                    .with_investor_flow(Err(failure)),
            ));
            for name in TOOLS {
                assert_eq!(
                    call(&missing, name, required_args(name)).await,
                    Err(ToolErrorCode::DataUnavailable),
                    "{name}: 자료 없음"
                );
                assert_eq!(
                    call(&failing, name, required_args(name)).await,
                    Err(ToolErrorCode::UpstreamUnavailable),
                    "{name}: 저장소 실패"
                );
            }
        });
    }
}
