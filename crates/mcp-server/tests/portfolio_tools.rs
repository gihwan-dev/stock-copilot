#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, BenchmarkReturnLine, BuyingPowerLine, HoldingToolLine, PerformanceBenchmark,
        PerformancePeriod, PerformanceToolView, PortfolioToolView, RetryDelay, RiskMetrics,
        SampleSufficiency, SystemFailure, SystemPort, SystemPortError, ToolName,
        TransactionToolKind, TransactionToolLine, TransactionsToolView,
    };
    use domain::{
        Currency, DisplayInstrumentName, MaskedAccountNumber, Money, Nanosecond, NumericInput,
        OffsetSeconds, Quantity, Ratio, Symbol, Timestamp, TotalCompleteness, UnixSeconds,
    };
    use mcp_server::{
        ToolErrorCode, ToolInvocation, ToolRegistry, make_cursor_store, register_portfolio_tools,
        validate_arguments_against_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::{FixedPortfolioToolReads, PortfolioReadCall};

    const FULL_ACCOUNT: &str = "12345678901";

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

    fn usd(text: &str) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Usd).expect("금액")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }

    fn qty(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }

    fn account() -> MaskedAccountNumber {
        MaskedAccountNumber::try_new(FULL_ACCOUNT).expect("계좌")
    }

    fn holding(symbol: &str, name: &str) -> HoldingToolLine {
        HoldingToolLine {
            symbol: Symbol::try_new(symbol).expect("종목"),
            name: DisplayInstrumentName::try_new(name).expect("이름"),
            quantity: qty("3"),
            valuation: Some(krw("213600")),
            purchase: Some(krw("200000")),
            profit: Some(krw("13600")),
            weight: Some(ratio("0.4")),
        }
    }

    fn portfolio_view() -> PortfolioToolView {
        PortfolioToolView {
            account: account(),
            recorded_at: at(1_764_000_000_i64),
            completeness: TotalCompleteness::Incomplete,
            holdings: vec![
                holding("035420", "NAVER"),
                holding("005930", "삼성전자 [공식](https://evil.example) 보기"),
            ],
            buying_power: vec![
                BuyingPowerLine {
                    currency: Currency::Usd,
                    amount: Some(usd("120.5")),
                },
                BuyingPowerLine {
                    currency: Currency::Krw,
                    amount: Some(krw("500000")),
                },
            ],
            approximate_total_assets: Some(krw("1200000")),
            fx_used: None,
        }
    }

    fn registry_with(reads: &Arc<FixedPortfolioToolReads>) -> ToolRegistry {
        let clock = Arc::new(SteppedClock::new(at(1_764_000_000_i64)));
        let cursors = make_cursor_store(clock);
        let mut registry = ToolRegistry::new();
        register_portfolio_tools(&mut registry, reads, &cursors).expect("등록");
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
        let run_id = AiRunId::try_new("run-m03").expect("실행 ID");
        tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: &arguments,
        })
        .await
        .map_err(|err| err.code())
    }

    #[test]
    fn get_portfolio_shows_only_account_suffix_and_buying_power_cash() {
        runtime().block_on(async {
            let reads = Arc::new(
                FixedPortfolioToolReads::default().with_portfolio(Ok(Some(portfolio_view()))),
            );
            let registry = registry_with(&reads);
            let output = call(&registry, "get_portfolio", json!({}))
                .await
                .expect("성공");
            let text = serde_json::to_string(&output).expect("직렬화");
            assert_eq!(
                output["account_suffix"],
                json!("8901"),
                "계좌번호 뒤 4자리만"
            );
            assert!(!text.contains(FULL_ACCOUNT), "전체 계좌번호가 없어야 한다");
            assert!(!text.contains("12345678"), "앞자리도 없어야 한다");
            assert_eq!(
                output["cash_basis"],
                json!("buying_power"),
                "현금 자리는 매수가능금액"
            );
            assert_eq!(
                output["buying_power"],
                json!([
                    { "currency": "KRW", "amount": { "amount": "500000", "currency": "KRW" } },
                    { "currency": "USD", "amount": { "amount": "120.5", "currency": "USD" } }
                ]),
                "통화별 매수가능금액을 결정적 순서로 준다"
            );
            assert_eq!(
                output["total_assets"]["accuracy"],
                json!("approximate"),
                "총자산은 근사"
            );
            assert_eq!(output["completeness"], json!("incomplete"));
            assert_eq!(output["recorded_at"], json!("2025-11-25T01:00:00+09:00"));
            let symbols: Vec<&Value> = output["holdings"]
                .as_array()
                .expect("보유 목록")
                .iter()
                .map(|line| &line["symbol"])
                .collect();
            assert_eq!(
                symbols,
                vec![&json!("005930"), &json!("035420")],
                "종목 코드순 정렬"
            );
            let name = &output["holdings"][0]["name"]["untrusted_text"];
            assert_eq!(
                name,
                &json!("삼성전자 공식 보기"),
                "외부 이름은 정규화된 untrusted_text"
            );
            assert!(!text.contains("evil.example"), "링크 주소가 남지 않는다");
            assert_eq!(reads.calls(), vec![PortfolioReadCall::Portfolio]);
        });
    }

    #[test]
    fn missing_and_failed_reads_fail_closed_with_structured_codes() {
        runtime().block_on(async {
            let missing = registry_with(&Arc::new(FixedPortfolioToolReads::default()));
            for name in [
                "get_portfolio",
                "get_allocation",
                "get_transactions",
                "get_performance",
            ] {
                assert_eq!(
                    call(&missing, name, json!({})).await,
                    Err(ToolErrorCode::DataUnavailable),
                    "{name}: 저장된 자료가 없으면 data_unavailable"
                );
            }
            let failure = SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            );
            let failing = registry_with(&Arc::new(
                FixedPortfolioToolReads::default()
                    .with_portfolio(Err(failure))
                    .with_allocation(Err(failure))
                    .with_transactions(Err(failure))
                    .with_performance(Err(failure)),
            ));
            for name in [
                "get_portfolio",
                "get_allocation",
                "get_transactions",
                "get_performance",
            ] {
                assert_eq!(
                    call(&failing, name, json!({})).await,
                    Err(ToolErrorCode::UpstreamUnavailable),
                    "{name}: 저장소 실패는 upstream_unavailable"
                );
            }
        });
    }

    fn transaction(seconds: i64, kind: TransactionToolKind, amount: &str) -> TransactionToolLine {
        TransactionToolLine {
            occurred_at: at(seconds),
            kind,
            symbol: Some(Symbol::try_new("005930").expect("종목")),
            quantity: Some(qty("1")),
            amount: krw(amount),
        }
    }

    fn transactions_view() -> TransactionsToolView {
        TransactionsToolView {
            account: account(),
            recorded_at: at(1_764_000_000_i64),
            lines: vec![
                transaction(1_763_000_000_i64, TransactionToolKind::Buy, "71000"),
                transaction(1_763_500_000_i64, TransactionToolKind::Dividend, "361"),
                transaction(1_763_900_000_i64, TransactionToolKind::Sell, "72000"),
            ],
        }
    }

    #[test]
    fn get_transactions_filters_sorts_newest_first_and_paginates() {
        runtime().block_on(async {
            let registry = registry_with(&Arc::new(
                FixedPortfolioToolReads::default().with_transactions(Ok(Some(transactions_view()))),
            ));
            let trades = call(&registry, "get_transactions", json!({ "kind": "trades" }))
                .await
                .expect("성공");
            let kinds: Vec<&Value> = trades["items"]
                .as_array()
                .expect("항목")
                .iter()
                .map(|item| &item["kind"])
                .collect();
            assert_eq!(kinds, vec![&json!("sell"), &json!("buy")], "매매만, 최신순");
            assert_eq!(trades["account_suffix"], json!("8901"));
            let first = call(&registry, "get_transactions", json!({ "limit": 1_u64 }))
                .await
                .expect("첫 쪽");
            let cursor = first["next_cursor"].as_str().expect("다음 커서").to_owned();
            assert_eq!(first["items"][0]["kind"], json!("sell"));
            let second = call(
                &registry,
                "get_transactions",
                json!({ "limit": 1_u64, "cursor": cursor }),
            )
            .await
            .expect("둘째 쪽");
            assert_eq!(second["items"][0]["kind"], json!("dividend"));
            assert_eq!(
                call(&registry, "get_transactions", json!({ "kind": "orders" })).await,
                Err(ToolErrorCode::InvalidInput),
                "정한 종류 밖은 거부"
            );
        });
    }

    fn performance_view(sample: SampleSufficiency) -> PerformanceToolView {
        PerformanceToolView {
            account: account(),
            period: PerformancePeriod::ThreeMonths,
            computed_at: at(1_764_000_000_i64),
            sample,
            time_weighted_return: Some(ratio("0.052")),
            money_weighted_return: Some(ratio("0.047")),
            benchmarks: vec![
                BenchmarkReturnLine {
                    benchmark: PerformanceBenchmark::Sp500,
                    period_return: ratio("0.03"),
                },
                BenchmarkReturnLine {
                    benchmark: PerformanceBenchmark::Kospi,
                    period_return: ratio("0.01"),
                },
            ],
            risk: RiskMetrics {
                annualized_volatility: Some(ratio("0.21")),
                max_drawdown: Some(ratio("-0.08")),
            },
        }
    }

    #[test]
    fn get_performance_withholds_risk_metrics_when_sample_is_insufficient() {
        runtime().block_on(async {
            let reads = Arc::new(
                FixedPortfolioToolReads::default()
                    .with_performance(Ok(Some(performance_view(SampleSufficiency::Insufficient)))),
            );
            let registry = registry_with(&reads);
            let output = call(&registry, "get_performance", json!({ "period": "3m" }))
                .await
                .expect("성공");
            assert_eq!(output["sample"], json!("insufficient"), "표본 부족 표시");
            assert_eq!(
                output["risk"],
                json!({ "annualized_volatility": null, "max_drawdown": null }),
                "표본이 부족하면 위험 지표를 주지 않는다"
            );
            assert_eq!(
                output["benchmarks"][0]["benchmark"],
                json!("KOSPI"),
                "결정적 정렬"
            );
            assert_eq!(
                reads.calls(),
                vec![PortfolioReadCall::Performance(
                    PerformancePeriod::ThreeMonths
                )]
            );
            let enough = registry_with(&Arc::new(
                FixedPortfolioToolReads::default()
                    .with_performance(Ok(Some(performance_view(SampleSufficiency::Sufficient)))),
            ));
            assert_eq!(
                call(&enough, "get_performance", json!({})).await,
                Err(ToolErrorCode::UpstreamUnavailable),
                "요청한 기간과 다른 저장 결과는 내보내지 않는다"
            );
            let full = call(&enough, "get_performance", json!({ "period": "3m" }))
                .await
                .expect("성공");
            assert_eq!(full["period"], json!("3m"));
            assert_eq!(full["sample"], json!("sufficient"));
            assert_eq!(full["risk"]["max_drawdown"], json!("-0.08"));
            assert_eq!(
                call(&enough, "get_performance", json!({ "period": "10y" })).await,
                Err(ToolErrorCode::InvalidInput),
                "정한 기간 밖은 거부"
            );
        });
    }

    #[test]
    fn portfolio_tools_reject_unknown_arguments() {
        runtime().block_on(async {
            let registry = registry_with(&Arc::new(
                FixedPortfolioToolReads::default().with_portfolio(Ok(Some(portfolio_view()))),
            ));
            assert_eq!(
                call(
                    &registry,
                    "get_portfolio",
                    json!({ "account": FULL_ACCOUNT })
                )
                .await,
                Err(ToolErrorCode::InvalidInput),
                "계좌를 인자로 받지 않는다"
            );
        });
    }
}
