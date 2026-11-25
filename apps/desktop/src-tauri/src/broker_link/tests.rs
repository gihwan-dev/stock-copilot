use std::sync::Arc;

use application::{
    EventBus, HttpStatus, MarketToolReads, PortError, PortErrorKind, QuoteObservation,
    ReadEndpoint, RetryDelay, UnavailableCause,
};
use domain::{
    AccountId, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
    BrokerAccountTypeCode, Currency, KnownMarket, NumericInput, Price, Quote, Symbol, Timestamp,
};
use testkit_ports::scripted::ScriptedBrokerRead;

use super::{AccountSelector, AccountSlot, BrokerMarketTools, SelectionChange};

fn at() -> Timestamp {
    Timestamp::from_parts(
        domain::UnixSeconds::new(1_795_564_800),
        domain::Nanosecond::new(0),
        domain::OffsetSeconds::new(0),
    )
    .expect("time")
}

fn row(id: i64, kind: &str, number: &str) -> BrokerAccountRow {
    BrokerAccountRow::from_broker_account_result(
        AccountId::new(id),
        BrokerAccountTypeCode::new(kind),
        BrokerAccountNumberInput::new(number.to_owned()),
        at(),
    )
    .expect("row")
}

fn rows(list: Vec<BrokerAccountRow>) -> BrokerAccountRows {
    BrokerAccountRows::try_new(list).expect("rows")
}

fn unavailable(endpoint: ReadEndpoint) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::ServerError,
        PortErrorKind::Unavailable {
            cause: UnavailableCause::ServerError,
            retry: RetryDelay::Unknown,
        },
    )
}

fn selector(
    broker: &Arc<ScriptedBrokerRead>,
) -> (AccountSelector<ScriptedBrokerRead>, AccountSlot) {
    let slot = AccountSlot::default();
    (
        AccountSelector::new(
            Arc::clone(broker),
            slot.clone(),
            Arc::new(EventBus::default()),
        ),
        slot,
    )
}

#[test]
fn first_brokerage_account_is_selected_automatically_and_only_once() {
    let broker = Arc::new(ScriptedBrokerRead::default());
    let listed = rows(vec![
        row(27, "PENSION_SAVINGS", "00000000419"),
        row(413, "BROKERAGE", "00000000731"),
        row(500, "BROKERAGE", "00000000999"),
    ]);
    broker.script_accounts().push(Ok(listed.clone()));
    broker.script_accounts().push(Ok(listed));
    let (selector, slot) = selector(&broker);
    tauri::async_runtime::block_on(async {
        assert_eq!(selector.select().await, SelectionChange::Changed);
        assert_eq!(slot.current(), vec![AccountId::new(413)]);
        assert_eq!(selector.select().await, SelectionChange::Unchanged);
    });
}

#[test]
fn a_chosen_brokerage_account_survives_reconnects_and_other_kinds_are_refused() {
    let broker = Arc::new(ScriptedBrokerRead::default());
    let listed = rows(vec![
        row(27, "PENSION_SAVINGS", "00000000419"),
        row(413, "BROKERAGE", "00000000731"),
        row(500, "BROKERAGE", "00000000999"),
    ]);
    for _ in 0_u8..5_u8 {
        broker.script_accounts().push(Ok(listed.clone()));
    }
    let (selector, slot) = selector(&broker);
    tauri::async_runtime::block_on(async {
        let brokerage = selector.brokerage_accounts().await.expect("계좌 목록");
        assert_eq!(
            brokerage
                .iter()
                .map(|(account, _)| account.clone())
                .collect::<Vec<_>>(),
            vec![AccountId::new(413), AccountId::new(500)]
        );
        assert_eq!(
            selector.choose(&AccountId::new(500)).await.expect("선택"),
            super::AccountChoice::Chosen(SelectionChange::Changed)
        );
        assert_eq!(selector.select().await, SelectionChange::Unchanged);
        assert_eq!(slot.current(), vec![AccountId::new(500)]);
        assert_eq!(
            selector
                .choose(&AccountId::new(27))
                .await
                .expect("연금 계좌"),
            super::AccountChoice::NotAvailable
        );
        assert_eq!(slot.current(), vec![AccountId::new(500)]);
    });
}

#[test]
fn no_brokerage_account_leaves_no_selection_and_a_failed_read_keeps_the_last_one() {
    let broker = Arc::new(ScriptedBrokerRead::default());
    broker
        .script_accounts()
        .push(Ok(rows(vec![row(413, "BROKERAGE", "00000000731")])));
    broker
        .script_accounts()
        .push(Err(unavailable(ReadEndpoint::Accounts)));
    broker
        .script_accounts()
        .push(Ok(rows(vec![row(27, "PENSION_SAVINGS", "00000000419")])));
    let (selector, slot) = selector(&broker);
    tauri::async_runtime::block_on(async {
        let _first = selector.select().await;
        assert_eq!(selector.select().await, SelectionChange::Unchanged);
        assert_eq!(slot.current(), vec![AccountId::new(413)]);
        assert_eq!(selector.select().await, SelectionChange::Changed);
        assert_eq!(slot.current(), Vec::<AccountId>::new());
    });
}

fn quote_observation(symbol: &Symbol) -> QuoteObservation {
    let instrument = super::tool_instrument(symbol, KnownMarket::Kr).expect("instrument");
    let price = Price::parse(
        &NumericInput::try_new("71500").expect("number"),
        Currency::Krw,
    )
    .expect("price");
    QuoteObservation {
        instrument,
        result: Ok(
            Quote::try_new(symbol.clone(), Currency::Krw, Some(price), None, at()).expect("quote"),
        ),
    }
}

#[test]
fn quote_tool_reads_the_live_broker_price_for_the_symbol() {
    let broker = Arc::new(ScriptedBrokerRead::default());
    let symbol = Symbol::try_new("005930").expect("symbol");
    broker
        .script_quotes()
        .push(Ok(vec![quote_observation(&symbol)]));
    let tools = BrokerMarketTools::connected(Arc::<ScriptedBrokerRead>::clone(&broker));
    let view = tauri::async_runtime::block_on(tools.quote(&symbol, KnownMarket::Kr))
        .expect("view")
        .expect("quote");
    assert_eq!(view.symbol, symbol);
    assert_eq!(view.market, KnownMarket::Kr);
    assert_eq!(view.collected_at, at());
    assert_eq!(
        Some(view.price),
        Price::parse(&NumericInput::try_new("71500").expect("n"), Currency::Krw)
            .ok()
            .map(|price| price.as_money())
    );
}

#[test]
fn quote_tool_fails_closed_when_unconnected_or_when_the_broker_fails() {
    let symbol = Symbol::try_new("005930").expect("symbol");
    let _failure = tauri::async_runtime::block_on(
        BrokerMarketTools::unconnected().quote(&symbol, KnownMarket::Kr),
    )
    .expect_err("fails");
    let broker = Arc::new(ScriptedBrokerRead::default());
    broker
        .script_quotes()
        .push(Err(unavailable(ReadEndpoint::Quotes)));
    let tools = BrokerMarketTools::connected(broker);
    let _broker_failure =
        tauri::async_runtime::block_on(tools.quote(&symbol, KnownMarket::Kr)).expect_err("fails");
}

#[test]
fn candles_tool_reads_recent_daily_candles_and_keeps_only_the_requested_days() {
    let broker = Arc::new(ScriptedBrokerRead::default());
    let symbol = Symbol::try_new("005930").expect("symbol");
    let candles = testkit_ports::estimate_fixtures::sample_daily_candles(10).expect("일봉");
    let last_close = candles.last().expect("마지막").values().close().as_money();
    broker.script_candles().push(Ok(candles));
    let now = testkit_ports::estimate_fixtures::sample_day(11).expect("시각");
    let tools = BrokerMarketTools::connected(Arc::<ScriptedBrokerRead>::clone(&broker))
        .with_clock(Arc::new(testkit_ports::SteppedClock::new(now)));
    let window = application::CandleWindow::try_new(3).expect("창");
    let view = tauri::async_runtime::block_on(tools.candles(&symbol, KnownMarket::Kr, window))
        .expect("view")
        .expect("candles");
    assert_eq!(view.lines.len(), 3);
    assert_eq!(view.lines.last().expect("마지막 줄").close, last_close);
    assert_eq!(view.collected_at, now);
    let Some(testkit_ports::scripted::BrokerReadCall::Candles(_, interval, period, _)) =
        broker.calls().into_iter().next()
    else {
        panic!("일봉 조회");
    };
    assert_eq!(interval.end(), now);
    assert_eq!(period, domain::CandlePeriod::Daily);
    let without_clock = BrokerMarketTools::connected(broker);
    let _unavailable = tauri::async_runtime::block_on(without_clock.candles(
        &symbol,
        KnownMarket::Kr,
        application::CandleWindow::try_new(3).expect("창"),
    ))
    .expect_err("시계 없으면 닫힘");
}
