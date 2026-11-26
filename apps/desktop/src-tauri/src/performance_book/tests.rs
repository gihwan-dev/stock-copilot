use std::future::ready;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{
    AccessDenial, Clock, HttpStatus, PortError, PortErrorKind, ReadEndpoint, SettledRowsSource,
    SystemPortError, TransactionRead,
};
use domain::{
    AccountId, AssetClass, CalendarDate, Cancellation, Candle, Currency, FxRate, KnownMarket,
    MarketCalendar, Money, Nanosecond, NumericInput, OffsetSeconds, PerformanceSpan, Quantity,
    QuoteInstrument, SettledRowSet, Settlement, SettlementWindow, StoredTransaction, Symbol,
    TimeInterval, Timestamp, Transaction, TransactionBatch, TransactionData, TransactionKind,
    UnixSeconds,
};

use super::{FeedHistory, PerformanceBook, PerformanceFeed, SPANS, traded_stocks};
use crate::ipc::state::BoxFuture;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
}

fn row(kind: TransactionKind, currency: Currency, symbol: Option<&str>) -> Transaction {
    Transaction::try_new(TransactionData {
        kind,
        market: match currency {
            Currency::Krw => domain::Market::Kr,
            Currency::Usd => domain::Market::Us,
        },
        currency,
        symbol: symbol.map(|text| Symbol::try_new(text).expect("심볼")),
        asset_class: symbol.map(|_text| AssetClass::Stock),
        isin: None,
        quantity: symbol.map(|_text| {
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량")
        }),
        filled_price: None,
        amount: Some(money("10", currency)),
        commission: money("0", currency),
        tax: money("0", currency),
        net_amount: Some(money("10", currency)),
        exchange_rate: None,
        settlement_date: at(86_400).offset_calendar_date(),
        settlement: Settlement::Settled,
        transaction_time: Some(at(86_400)),
        cancellation: Cancellation::Regular,
    })
    .expect("행")
}

fn rows() -> SettledRowSet {
    let history = [
        row(TransactionKind::Deposit, Currency::Krw, None),
        row(TransactionKind::Buy, Currency::Krw, Some("005930")),
        row(TransactionKind::Sell, Currency::Krw, Some("005930")),
        row(TransactionKind::Buy, Currency::Usd, Some("AAPL")),
    ]
    .into_iter()
    .map(|item| StoredTransaction::try_restore(item, at(86_400), None).expect("저장 행"))
    .collect();
    SettledRowSet::try_restore(
        AccountId::new(7),
        history,
        TransactionBatch::try_new(Vec::new()).expect("목록"),
        Some(at(172_800)),
    )
    .expect("행 집합")
}

#[test]
fn every_traded_stock_gets_one_history_and_cash_rows_none() {
    let stocks = traded_stocks(&rows());
    assert_eq!(
        stocks,
        vec![
            (KnownMarket::Kr, Symbol::try_new("005930").expect("심볼")),
            (KnownMarket::Us, Symbol::try_new("AAPL").expect("심볼")),
        ]
    );
}

fn denied(endpoint: ReadEndpoint) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::NotReceived,
        PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
    )
}

#[derive(Default)]
struct DownFeed {
    calendars: AtomicUsize,
}

impl PerformanceFeed for DownFeed {
    fn transactions<'a>(
        &'a self,
        _account: &'a AccountId,
        _window: SettlementWindow,
    ) -> BoxFuture<'a, Result<TransactionRead, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Transactions))))
    }

    fn calendar_on(
        &self,
        _market: KnownMarket,
        _date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        let _seen = self.calendars.fetch_add(1, Ordering::SeqCst);
        Box::pin(ready(Err(denied(ReadEndpoint::Calendar))))
    }

    fn fx_at(&self, _at: Timestamp) -> BoxFuture<'_, Result<FxRate, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::FxRate))))
    }

    fn daily_candles<'a>(
        &'a self,
        _instrument: &'a QuoteInstrument,
        _window: TimeInterval,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Candles))))
    }
}

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        at(1_790_000_000)
    }
}

struct Rows;
impl SettledRowsSource for Rows {
    fn settled_rows(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send {
        ready(Ok(rows()))
    }
}

#[test]
fn an_unreachable_feed_leaves_every_period_without_a_number() {
    let feed = Arc::new(DownFeed::default());
    let shared: Arc<dyn PerformanceFeed> = Arc::<DownFeed>::clone(&feed);
    let clock: Arc<dyn Clock> = Arc::new(Fixed);
    let book = PerformanceBook::new(
        Arc::new(FeedHistory::new(shared, Arc::clone(&clock))),
        clock,
    );
    tauri::async_runtime::block_on(book.refresh(&Rows, &AccountId::new(7)));
    for span in SPANS {
        assert_eq!(book.summary(span), None, "{span:?}");
    }
    assert_eq!(
        feed.calendars.load(Ordering::SeqCst),
        SPANS.len(),
        "기간마다 첫 날짜에서 멈추고 반복 호출하지 않는다"
    );
    assert_eq!(book.summary(PerformanceSpan::OneYear), None);
}
