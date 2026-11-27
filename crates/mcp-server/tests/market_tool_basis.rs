#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, CandlePriceBasis, CandleToolLine, CandlesToolView, MarketOverviewToolView,
        OverviewItem, OverviewToolLine, OverviewUnit, ToolName, ValueProvenance,
    };
    use domain::{
        Currency, KnownMarket, Money, Nanosecond, NumericInput, OffsetSeconds, Quantity, Ratio,
        Symbol, Timestamp, UnixSeconds,
    };
    use mcp_server::{ToolInvocation, ToolRegistry, make_cursor_store, register_market_tools};
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::FixedMarketToolReads;

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

    fn call(fixed: FixedMarketToolReads, name: &str, args: &Value) -> Value {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("런타임")
            .block_on(async {
                let reads = Arc::new(fixed);
                let cursors = make_cursor_store(Arc::new(SteppedClock::new(at(1_764_000_000_i64))));
                let mut registry = ToolRegistry::new();
                register_market_tools(&mut registry, &reads, &cursors).expect("등록");
                let tool_name = ToolName::try_new(name).expect("이름");
                let (_spec, tool) = registry.get(&tool_name).expect("도구");
                let arguments: Map<String, Value> = args.as_object().cloned().expect("인자");
                let run_id = AiRunId::try_new("run-basis").expect("실행");
                tool.invoke(ToolInvocation {
                    run_id: &run_id,
                    arguments: &arguments,
                })
                .await
                .expect("성공")
            })
    }

    fn candles() -> CandlesToolView {
        CandlesToolView {
            symbol: Symbol::try_new("000660").expect("종목"),
            market: KnownMarket::Kr,
            collected_at: at(1_764_000_000_i64),
            lines: vec![CandleToolLine {
                opened_at: at(1_763_913_600_i64),
                open: krw("10200"),
                high: krw("10300"),
                low: krw("10100"),
                close: krw("10300"),
                volume: Some(
                    Quantity::parse(&NumericInput::try_new("10").expect("입력")).expect("수량"),
                ),
            }],
        }
    }

    #[test]
    fn candles_state_whether_prices_are_adjusted() {
        let args = json!({ "symbol": "000660", "days": 5_u64 });
        let adjusted = call(
            FixedMarketToolReads::default()
                .with_candles(Ok(Some(candles())))
                .with_candle_basis(CandlePriceBasis::Adjusted),
            "get_candles",
            &args,
        );
        assert_eq!(adjusted["adjusted"], json!(true));
        let raw = call(
            FixedMarketToolReads::default().with_candles(Ok(Some(candles()))),
            "get_candles",
            &args,
        );
        assert_eq!(raw["adjusted"], json!(false));
        assert_eq!(raw["items"][0]["close"]["amount"], json!("10300"));
    }

    #[test]
    fn candle_cursor_rejects_a_change_in_price_basis() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("런타임");
        let cursors = make_cursor_store(Arc::new(SteppedClock::new(at(1_764_000_000_i64))));
        let run_id = AiRunId::try_new("candle-pages").expect("실행");
        let name = ToolName::try_new("get_candles").expect("도구");
        let read = |basis, args: Value| {
            let mut view = candles();
            let mut earlier = view.lines[0].clone();
            earlier.opened_at = at(1_763_827_200_i64);
            view.lines.push(earlier);
            let reads = Arc::new(
                FixedMarketToolReads::default()
                    .with_candles(Ok(Some(view)))
                    .with_candle_basis(basis),
            );
            let mut registry = ToolRegistry::new();
            register_market_tools(&mut registry, &reads, &cursors).expect("등록");
            let (_, tool) = registry.get(&name).expect("도구");
            runtime.block_on(tool.invoke(ToolInvocation {
                run_id: &run_id,
                arguments: args.as_object().expect("인자"),
            }))
        };
        let query = json!({ "symbol":"000660", "days":5, "limit":1 });
        let first = read(CandlePriceBasis::Unadjusted, query.clone()).expect("첫 페이지");
        let mut next = query;
        next["cursor"] = first["next_cursor"].clone();
        assert!(next["cursor"].is_string());
        assert_eq!(
            read(CandlePriceBasis::Adjusted, next).unwrap_err().code(),
            mcp_server::ToolErrorCode::InvalidCursor
        );
    }

    #[test]
    fn overview_lists_us_treasury_yields_as_not_provided() {
        let view = MarketOverviewToolView {
            lines: vec![OverviewToolLine {
                item: OverviewItem::Kospi,
                unit: OverviewUnit::IndexPoints,
                value: Ratio::parse(&NumericInput::try_new("2800.5").expect("입력")).expect("값"),
                change_rate: None,
                provenance: ValueProvenance::Official,
                collected_at: at(1_764_000_100_i64),
            }],
            sessions: Vec::new(),
        };
        let output = call(
            FixedMarketToolReads::default().with_overview(Ok(Some(view))),
            "get_market_overview",
            &json!({}),
        );
        assert_eq!(
            output["not_provided"],
            json!([
                { "item": "us_treasury_2y", "reason": "no_source" },
                { "item": "us_treasury_10y", "reason": "no_source" }
            ])
        );
        let items: Vec<&Value> = output["indicators"]
            .as_array()
            .expect("지표")
            .iter()
            .map(|line| &line["item"])
            .collect();
        assert_eq!(items, vec![&json!("kospi")]);
    }
}
