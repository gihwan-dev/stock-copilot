use std::sync::Arc;

use application::{
    BrokerCapability, CandleWindow, DiagnosticEvent, HttpStatus, MarketToolError, MarketToolName,
    MarketToolReads, OverviewItem, OverviewUnit, PortError, PortErrorKind, QuoteObservation,
    RankingToolKind, ReadEndpoint, RetryDelay, ToolReadDiagnosis, ToolReadResult, UnavailableCause,
    ValueProvenance,
};
use domain::{
    Currency, DisplayInstrumentName, FxPair, FxRate, FxRateKind, FxSource, InvestorFlow,
    InvestorGroupRow, InvestorTradingRankings, KnownMarket, MarketIndicatorQuote, MasterCode,
    Money, NetTradingRow, NumericInput, Price, Quote, RankingCautionFilter, RankingDuration,
    RankingKind, RankingLimit, RankingOrdinal, RankingResponseInput, RankingRow, Ratio, SectorId,
    SectorRanking, SectorRankingRow, StockFlowRecordInput, StockFlowRow, StockRanking, Symbol,
    TimeInterval, Timestamp,
};
use testkit_ports::scripted::{BrokerReadCall, ScriptedBrokerRead, ScriptedMarketInfo};

use super::BrokerMarketTools;

fn now() -> Timestamp {
    testkit_ports::estimate_fixtures::sample_day(11).expect("now")
}
fn number(value: &str) -> NumericInput {
    NumericInput::try_new(value).expect("number")
}
fn code(value: &str) -> MasterCode {
    MasterCode::try_new(value).expect("code")
}
fn symbol(value: &str) -> Symbol {
    Symbol::try_new(value).expect("symbol")
}
fn krw(value: &str) -> Money {
    Money::parse(&number(value), Currency::Krw).expect("krw")
}
fn ratio(value: &str) -> Ratio {
    Ratio::parse(&number(value)).expect("ratio")
}
fn server_error(endpoint: ReadEndpoint) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::ServerError,
        PortErrorKind::Unavailable {
            cause: UnavailableCause::ServerError,
            retry: RetryDelay::Unknown,
        },
    )
}

struct Fixture {
    broker: Arc<ScriptedBrokerRead>,
    market: Arc<ScriptedMarketInfo>,
    tools: BrokerMarketTools,
}

fn fixture() -> Fixture {
    let broker = Arc::new(ScriptedBrokerRead::default());
    let market = Arc::new(ScriptedMarketInfo::default());
    let tools = BrokerMarketTools::connected(Arc::<ScriptedBrokerRead>::clone(&broker))
        .with_market(Arc::<ScriptedMarketInfo>::clone(&market))
        .with_clock(Arc::new(testkit_ports::SteppedClock::new(now())));
    Fixture {
        broker,
        market,
        tools,
    }
}

fn run<T>(future: impl Future<Output = T>) -> T {
    tauri::async_runtime::block_on(future)
}

fn spy_quote() -> QuoteObservation {
    let ticker = symbol("SPY");
    let instrument = crate::broker_link::tool_instrument(&ticker, KnownMarket::Us).expect("spy");
    let price = Price::parse(&number("601.25"), Currency::Usd).expect("price");
    QuoteObservation {
        instrument,
        result: Ok(Quote::try_new(ticker, Currency::Usd, Some(price), None, now()).expect("quote")),
    }
}

fn usd_krw() -> FxRate {
    let validity = TimeInterval::try_new(
        now().checked_sub(domain::TimeSpan::DAY).expect("start"),
        now(),
    )
    .expect("validity");
    FxRate::parse(
        &number("1385.20"),
        FxPair::UsdKrw,
        FxRateKind::Mid,
        validity,
        FxSource::TossOpenApi,
    )
    .expect("fx")
}

type OverviewSummary = (OverviewItem, OverviewUnit, Ratio, ValueProvenance);

fn expected_overview() -> Vec<OverviewSummary> {
    vec![
        (
            OverviewItem::Kospi,
            OverviewUnit::IndexPoints,
            ratio("2512.34"),
            ValueProvenance::Official,
        ),
        (
            OverviewItem::KrBond10Y,
            OverviewUnit::YieldPercent,
            ratio("3.105"),
            ValueProvenance::Official,
        ),
        (
            OverviewItem::UsdKrw,
            OverviewUnit::KrwPerUsd,
            ratio("1385.20"),
            ValueProvenance::Official,
        ),
        (
            OverviewItem::Sp500,
            OverviewUnit::UsdPrice,
            ratio("601.25"),
            ValueProvenance::Approximate,
        ),
    ]
}

#[test]
fn overview_reads_indices_bonds_fx_and_etf_proxies_and_skips_failed_parts() {
    let fixture = fixture();
    fixture.market.script_indicator_quotes().push(Ok(vec![
        MarketIndicatorQuote::from_response(code("KOSPI"), &number("2512.34"), None, now())
            .expect("kospi"),
        MarketIndicatorQuote::from_response(code("KR_BOND_10Y"), &number("3.105"), None, now())
            .expect("bond"),
    ]));
    fixture.market.script_fx_rate().push(Ok(usd_krw()));
    fixture.broker.script_quotes().push(Ok(vec![spy_quote()]));
    fixture
        .market
        .script_calendar()
        .push(Err(server_error(ReadEndpoint::Calendar)));
    fixture
        .market
        .script_calendar()
        .push(Err(server_error(ReadEndpoint::Calendar)));
    let view = run(fixture.tools.market_overview())
        .expect("overview")
        .expect("view");
    let summary: Vec<_> = view
        .lines
        .iter()
        .map(|line| (line.item, line.unit, line.value, line.provenance))
        .collect();
    assert_eq!(summary, expected_overview());
    assert_eq!(view.sessions.len(), 0);
    let Some(BrokerReadCall::Quotes(proxies)) = fixture.broker.calls().into_iter().next() else {
        panic!("etf quotes");
    };
    let tickers: Vec<_> = proxies
        .iter()
        .map(|instrument| instrument.symbol().as_str().to_owned())
        .collect();
    assert_eq!(tickers, vec!["SPY", "QQQ", "GLD", "USO", "UUP"]);
}

#[test]
fn overview_fails_with_the_first_cause_only_when_every_part_failed() {
    let fixture = fixture();
    fixture
        .market
        .script_indicator_quotes()
        .push(Err(PortError::new(
            ReadEndpoint::IndicatorQuotes,
            HttpStatus::RateLimited,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::RateLimited,
                retry: RetryDelay::Unknown,
            },
        )));
    fixture
        .market
        .script_fx_rate()
        .push(Err(server_error(ReadEndpoint::FxRate)));
    fixture
        .broker
        .script_quotes()
        .push(Err(server_error(ReadEndpoint::Quotes)));
    for _market in 0_u8..2_u8 {
        fixture
            .market
            .script_calendar()
            .push(Err(server_error(ReadEndpoint::Calendar)));
    }
    assert_eq!(
        run(fixture.tools.market_overview()),
        Err(MarketToolError::RateLimited)
    );
}

fn trading_ranking() -> StockRanking {
    StockRanking::from_response(RankingResponseInput {
        kind: code("MARKET_TRADING_AMOUNT"),
        market: KnownMarket::Kr,
        duration: code("1d"),
        limit: RankingLimit::try_new(100).expect("limit"),
        rows: vec![
            RankingRow {
                rank: RankingOrdinal::new(1),
                symbol: symbol("000660"),
                currency: Currency::Krw,
                volume: number("10"),
                amount: number("5000"),
                change_rate: Some(number("0.012")),
            },
            RankingRow {
                rank: RankingOrdinal::new(2),
                symbol: symbol("005930"),
                currency: Currency::Krw,
                volume: number("10"),
                amount: number("3000"),
                change_rate: None,
            },
        ],
        ranked_at: None,
        collected: now(),
        caution_filter: RankingCautionFilter::Included,
    })
    .expect("ranking")
}

#[test]
fn trading_amount_rankings_carry_amount_change_and_symbol_names() {
    let fixture = fixture();
    let ranking = trading_ranking();
    fixture.market.script_rankings().push(Ok(ranking));
    let view = run(fixture
        .tools
        .rankings(RankingToolKind::TradingAmount, KnownMarket::Kr))
    .expect("rankings")
    .expect("view");
    assert_eq!(view.kind, RankingToolKind::TradingAmount);
    assert_eq!(view.collected_at, now());
    let lines: Vec<_> = view
        .lines
        .iter()
        .map(|line| {
            (
                line.symbol.as_str().to_owned(),
                line.name.as_str().to_owned(),
                line.metric,
                line.change_rate,
            )
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            (
                String::from("000660"),
                String::from("000660"),
                Some(krw("5000")),
                Some(ratio("0.012"))
            ),
            (
                String::from("005930"),
                String::from("005930"),
                Some(krw("3000")),
                None
            ),
        ]
    );
    let Some(testkit_ports::scripted::MarketInfoCall::Rankings(query)) =
        fixture.market.calls().into_iter().next()
    else {
        panic!("rankings call");
    };
    assert_eq!(query.kind(), RankingKind::MarketTradingAmount);
    assert_eq!(query.duration(), RankingDuration::Day);
    assert_eq!(*query.limit().as_usize(), 100);
}

fn group(rows: Vec<(&str, &str)>) -> InvestorGroupRow {
    InvestorGroupRow {
        ranked_at: None,
        net_buy: rows
            .into_iter()
            .zip(1_usize..)
            .map(|((ticker, amount), rank)| NetTradingRow {
                rank: RankingOrdinal::new(rank),
                symbol: symbol(ticker),
                currency: Currency::Krw,
                amount: number(amount),
            })
            .collect(),
        net_sell: Vec::new(),
    }
}

#[test]
fn net_buy_rankings_use_the_requested_investor_and_are_domestic_only() {
    let fixture = fixture();
    let rankings = InvestorTradingRankings::from_response(
        group(vec![("000660", "900")]),
        group(vec![("005930", "700"), ("035420", "100")]),
        group(Vec::new()),
        RankingLimit::try_new(100).expect("limit"),
        now(),
    )
    .expect("investor rankings");
    fixture
        .market
        .script_investor_trading_rankings()
        .push(Ok(rankings));
    let view = run(fixture
        .tools
        .rankings(RankingToolKind::InstitutionNetBuy, KnownMarket::Kr))
    .expect("rankings")
    .expect("view");
    let lines: Vec<_> = view
        .lines
        .iter()
        .map(|line| (line.symbol.as_str().to_owned(), line.metric))
        .collect();
    assert_eq!(
        lines,
        vec![
            (String::from("005930"), Some(krw("700"))),
            (String::from("035420"), Some(krw("100"))),
        ]
    );
    assert_eq!(
        run(fixture
            .tools
            .rankings(RankingToolKind::ForeignNetBuy, KnownMarket::Us)),
        Err(MarketToolError::NotOffered)
    );
}

#[test]
fn sectors_read_todays_sector_ranking() {
    let fixture = fixture();
    let ranking = SectorRanking::from_response(
        RankingKind::TopGainers,
        KnownMarket::Kr,
        RankingDuration::Day,
        vec![SectorRankingRow {
            rank: RankingOrdinal::new(1),
            sector: SectorId::try_new("G1").expect("sector"),
            name: DisplayInstrumentName::try_new("반도체").expect("name"),
            change_rate: Some(number("0.031")),
            trading_amount: Some(number("1000000")),
            market_capitalization: None,
            currency: Currency::Krw,
        }],
        now(),
    )
    .expect("sectors");
    fixture.market.script_sector_rankings().push(Ok(ranking));
    let view = run(fixture.tools.sectors(KnownMarket::Kr))
        .expect("sectors")
        .expect("view");
    assert_eq!(view.lines.len(), 1);
    let line = &view.lines[0];
    assert_eq!(line.label.as_str(), "반도체");
    assert_eq!(line.change_rate, Some(ratio("0.031")));
    assert_eq!(line.trading_amount, Some(krw("1000000")));
    let Some(testkit_ports::scripted::MarketInfoCall::SectorRankings(query)) =
        fixture.market.calls().into_iter().next()
    else {
        panic!("sector call");
    };
    assert_eq!(query.kind(), RankingKind::TopGainers);
    assert_eq!(query.duration(), RankingDuration::Day);
}

#[test]
fn investor_flow_values_net_shares_at_the_days_close() {
    let fixture = fixture();
    let candles = testkit_ports::estimate_fixtures::sample_daily_candles(10).expect("candles");
    let last = candles.last().expect("last").clone();
    let flow = InvestorFlow::from_stock_response(
        symbol("000660"),
        StockFlowRecordInput {
            date: last.time().date(),
            updated: now(),
            rows: vec![
                StockFlowRow {
                    investor: code("foreigner"),
                    buy: number("30"),
                    sell: number("10"),
                    reported_net: number("20"),
                },
                StockFlowRow {
                    investor: code("individual"),
                    buy: number("5"),
                    sell: number("25"),
                    reported_net: number("-20"),
                },
            ],
            collected: now(),
        },
    )
    .expect("flow");
    fixture
        .market
        .script_stock_investor_flow()
        .push(Ok(vec![flow]));
    fixture.broker.script_candles().push(Ok(candles));
    let view = run(fixture
        .tools
        .investor_flow(&symbol("000660"), KnownMarket::Kr))
    .expect("flow")
    .expect("view");
    assert_eq!(view.lines.len(), 1);
    let line = &view.lines[0];
    let close = last.values().close().as_money();
    assert_eq!(close, krw("102"));
    assert_eq!(line.foreign_net, Some(krw("2040")));
    assert_eq!(line.individual_net, Some(krw("-2040")));
    assert_eq!(line.institution_net, None);
    assert_eq!(line.trading_date.offset_calendar_date(), last.time().date());
    assert_eq!(
        run(fixture
            .tools
            .investor_flow(&symbol("AAPL"), KnownMarket::Us)),
        Err(MarketToolError::NotOffered)
    );
}

#[test]
fn investor_flow_keeps_rows_without_values_when_closes_fail() {
    let fixture = fixture();
    let flow = InvestorFlow::from_stock_response(
        symbol("000660"),
        StockFlowRecordInput {
            date: now().offset_calendar_date(),
            updated: now(),
            rows: vec![StockFlowRow {
                investor: code("foreigner"),
                buy: number("3"),
                sell: number("1"),
                reported_net: number("2"),
            }],
            collected: now(),
        },
    )
    .expect("flow");
    fixture
        .market
        .script_stock_investor_flow()
        .push(Ok(vec![flow]));
    fixture
        .broker
        .script_candles()
        .push(Err(server_error(ReadEndpoint::Candles)));
    let view = run(fixture
        .tools
        .investor_flow(&symbol("000660"), KnownMarket::Kr))
    .expect("flow")
    .expect("view");
    assert_eq!(view.lines.len(), 1);
    assert_eq!(view.lines[0].foreign_net, None);
}

#[test]
fn candles_request_covers_holidays_and_report_a_missing_capability() {
    let fixture = fixture();
    fixture.broker.script_candles().push(Err(PortError::new(
        ReadEndpoint::Candles,
        HttpStatus::Success,
        PortErrorKind::Unsupported(BrokerCapability::DailyCandles),
    )));
    let window = CandleWindow::try_new(3).expect("window");
    assert_eq!(
        run(fixture
            .tools
            .candles(&symbol("000660"), KnownMarket::Kr, window)),
        Err(MarketToolError::NotOffered)
    );
    let Some(BrokerReadCall::Candles(_, interval, _, _)) =
        fixture.broker.calls().into_iter().next()
    else {
        panic!("candles call");
    };
    let span = interval
        .start()
        .elapsed_until(interval.end())
        .expect("span");
    assert_eq!(*span.as_parts().0.as_u64(), (3 * 2 + 14) * 86_400);
}

#[test]
fn every_tool_reports_not_connected_without_a_brokerage() {
    let tools = BrokerMarketTools::unconnected();
    let window = CandleWindow::try_new(5).expect("window");
    let ticker = symbol("000660");
    assert_eq!(
        run(tools.quote(&ticker, KnownMarket::Kr)),
        Err(MarketToolError::NotConnected)
    );
    assert_eq!(
        run(tools.candles(&ticker, KnownMarket::Kr, window)),
        Err(MarketToolError::NotConnected)
    );
    assert_eq!(
        run(tools.market_overview()),
        Err(MarketToolError::NotConnected)
    );
    assert_eq!(
        run(tools.rankings(RankingToolKind::Gainers, KnownMarket::Kr)),
        Err(MarketToolError::NotConnected)
    );
    assert_eq!(
        run(tools.sectors(KnownMarket::Kr)),
        Err(MarketToolError::NotConnected)
    );
    assert_eq!(
        run(tools.investor_flow(&ticker, KnownMarket::Kr)),
        Err(MarketToolError::NotConnected)
    );
}

#[test]
fn tool_read_lines_name_the_tool_endpoint_and_kind_only() {
    let failed = crate::diagnostics_log::format_line(
        now(),
        &DiagnosticEvent::ToolRead(ToolReadDiagnosis {
            tool: MarketToolName::Candles,
            result: ToolReadResult::Failed(PortError::new(
                ReadEndpoint::Candles,
                HttpStatus::Success,
                PortErrorKind::Unsupported(BrokerCapability::DailyCandles),
            )),
        }),
    );
    assert!(
        failed.ends_with(
            "tool-read tool=get_candles result=failed endpoint=Candles status=Success kind=Unsupported(DailyCandles)"
        ),
        "{failed}"
    );
    let unconnected = crate::diagnostics_log::format_line(
        now(),
        &DiagnosticEvent::ToolRead(ToolReadDiagnosis {
            tool: MarketToolName::Overview,
            result: ToolReadResult::NotConnected,
        }),
    );
    assert!(
        unconnected.ends_with("tool-read tool=get_market_overview result=not-connected"),
        "{unconnected}"
    );
}

fn candle_adjustments(broker: &ScriptedBrokerRead) -> Vec<domain::PriceAdjustment> {
    broker
        .calls()
        .into_iter()
        .filter_map(|call| {
            if let BrokerReadCall::Candles(_, _, _, adjustment) = call {
                Some(adjustment)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn adjusted_history_uses_the_registered_master_identity() {
    let fixture = fixture();
    let master = testkit_ports::estimate_fixtures::sample_master().expect("종목");
    let directory = crate::holdings_link::InstrumentDirectory::default();
    directory.replace(std::slice::from_ref(&master));
    let tools = fixture.tools.with_names(directory);
    let candles = testkit_ports::estimate_fixtures::sample_daily_candles(10).expect("일봉");
    fixture.broker.script_candles().push(Ok(candles));
    let expected = master.quote_instrument().expect("식별자");
    let window = CandleWindow::try_new(3).expect("조회 기간");
    let result = run(tools.priced_candles(expected.symbol(), *expected.market(), window))
        .expect("일봉")
        .expect("결과");
    assert_eq!(result.basis, application::CandlePriceBasis::Adjusted);
    let calls = fixture.broker.calls();
    let [BrokerReadCall::Candles(actual, _, _, adjustment)] = calls.as_slice() else {
        panic!("수정주가 조회 한 번을 기대함");
    };
    assert_eq!(actual, &expected);
    assert_eq!(
        *adjustment,
        domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(now()))
    );
}

#[test]
fn candles_tool_reads_the_adjusted_path_and_says_so() {
    let fixture = fixture();
    let candles = testkit_ports::estimate_fixtures::sample_daily_candles(10).expect("candles");
    fixture.broker.script_candles().push(Ok(candles));
    let window = CandleWindow::try_new(3).expect("window");
    let priced = run(fixture
        .tools
        .priced_candles(&symbol("000660"), KnownMarket::Kr, window))
    .expect("candles")
    .expect("view");
    assert_eq!(priced.basis, application::CandlePriceBasis::Adjusted);
    assert_eq!(priced.view.lines.len(), 3);
    let adjustments = candle_adjustments(&fixture.broker);
    assert_eq!(adjustments.len(), 1);
    assert_eq!(
        adjustments[0],
        domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(now()))
    );
}

#[test]
fn candles_tool_falls_back_to_raw_prices_when_adjusted_history_is_missing() {
    let fixture = fixture();
    fixture.broker.script_candles().push(Err(PortError::new(
        ReadEndpoint::Candles,
        HttpStatus::Success,
        PortErrorKind::Unsupported(BrokerCapability::AdjustedHistory),
    )));
    let candles = testkit_ports::estimate_fixtures::sample_daily_candles(10).expect("candles");
    fixture.broker.script_candles().push(Ok(candles));
    let window = CandleWindow::try_new(4).expect("window");
    let priced = run(fixture
        .tools
        .priced_candles(&symbol("000660"), KnownMarket::Kr, window))
    .expect("candles")
    .expect("view");
    assert_eq!(priced.basis, application::CandlePriceBasis::Unadjusted);
    assert_eq!(priced.view.lines.len(), 4);
    let adjustments = candle_adjustments(&fixture.broker);
    assert_eq!(adjustments.len(), 2);
    assert_eq!(adjustments[1], domain::PriceAdjustment::Raw);
}

#[test]
fn candles_tool_reports_other_adjusted_failures_without_a_raw_retry() {
    let fixture = fixture();
    fixture
        .broker
        .script_candles()
        .push(Err(server_error(ReadEndpoint::Candles)));
    let window = CandleWindow::try_new(3).expect("window");
    assert_eq!(
        run(fixture
            .tools
            .priced_candles(&symbol("000660"), KnownMarket::Kr, window)),
        Err(MarketToolError::Unavailable)
    );
    assert_eq!(candle_adjustments(&fixture.broker).len(), 1);
}
