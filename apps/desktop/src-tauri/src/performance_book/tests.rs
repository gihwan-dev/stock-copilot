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

    fn buying_power<'a>(
        &'a self,
        _account: &'a AccountId,
        _currency: Currency,
    ) -> BoxFuture<'a, Result<domain::BuyingPower, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::BuyingPower))))
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

fn feb(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        domain::CalendarYear::new(1970),
        domain::MonthNumber::new(2),
        domain::DayNumber::new(day),
    )
    .expect("날짜")
}

fn trading_session(day: u8, market: KnownMarket) -> domain::MarketSession {
    let start = i64::from(day)
        .checked_add(30)
        .and_then(|days| days.checked_mul(86_400))
        .expect("초");
    let regular = Some(
        domain::SessionData::try_new(
            TimeInterval::try_new(at(start), at(start.checked_add(3_600).expect("끝")))
                .expect("구간"),
            None,
        )
        .expect("세션"),
    );
    domain::MarketSession::try_new(
        feb(day),
        match market {
            KnownMarket::Kr => domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: regular,
                after_market: None,
            },
            KnownMarket::Us => domain::SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: regular,
                after_market: None,
            },
        },
    )
    .expect("세션")
}

#[derive(Default)]
struct WeekFeed {
    calendars: AtomicUsize,
}

impl PerformanceFeed for WeekFeed {
    fn transactions<'a>(
        &'a self,
        _account: &'a AccountId,
        _window: SettlementWindow,
    ) -> BoxFuture<'a, Result<TransactionRead, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Transactions))))
    }

    fn calendar_on(
        &self,
        market: KnownMarket,
        _date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        let _seen = self.calendars.fetch_add(1, Ordering::SeqCst);
        Box::pin(ready(Ok(MarketCalendar::try_new(
            trading_session(5, market),
            trading_session(6, market),
            trading_session(9, market),
        )
        .expect("달력"))))
    }

    fn fx_at(&self, _at: Timestamp) -> BoxFuture<'_, Result<FxRate, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::FxRate))))
    }

    fn buying_power<'a>(
        &'a self,
        _account: &'a AccountId,
        _currency: Currency,
    ) -> BoxFuture<'a, Result<domain::BuyingPower, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::BuyingPower))))
    }

    fn daily_candles<'a>(
        &'a self,
        _instrument: &'a QuoteInstrument,
        _window: TimeInterval,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Candles))))
    }
}

#[test]
fn the_weekend_after_a_fetched_friday_is_known_without_more_calendar_calls() {
    use application::PerformanceHistorySource as _;
    let feed = Arc::new(WeekFeed::default());
    let shared: Arc<dyn PerformanceFeed> = Arc::<WeekFeed>::clone(&feed);
    let history = FeedHistory::new(shared, Arc::new(Fixed));
    let friday = tauri::async_runtime::block_on(history.window_day(feb(6))).expect("금요일");
    assert!(friday.is_some(), "영업일은 평가일이다");
    for weekend in [7, 8] {
        let day = tauri::async_runtime::block_on(history.window_day(feb(weekend))).expect("주말");
        assert_eq!(day, None, "두 시장이 모두 쉬면 평가하지 않는다");
    }
    assert_eq!(
        feed.calendars.load(Ordering::SeqCst),
        2,
        "시장마다 한 번만 묻는다"
    );
}

struct GappedFx {
    fetched: AtomicUsize,
}

impl PerformanceFeed for GappedFx {
    fn transactions<'a>(
        &'a self,
        _account: &'a AccountId,
        _window: SettlementWindow,
    ) -> BoxFuture<'a, Result<TransactionRead, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Transactions))))
    }

    fn calendar_on(
        &self,
        market: KnownMarket,
        _date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        Box::pin(ready(Ok(MarketCalendar::try_new(
            trading_session(5, market),
            trading_session(6, market),
            trading_session(9, market),
        )
        .expect("달력"))))
    }

    fn fx_at(&self, at: Timestamp) -> BoxFuture<'_, Result<FxRate, PortError>> {
        let first = self.fetched.fetch_add(1, Ordering::SeqCst) == 0;
        let rate = first.then(|| {
            FxRate::parse(
                &NumericInput::try_new("1300").expect("입력"),
                domain::FxPair::UsdKrw,
                domain::FxRateKind::Mid,
                TimeInterval::try_new(at, at.checked_add(domain::TimeSpan::DAY).expect("끝"))
                    .expect("구간"),
                domain::FxSource::TossOpenApi,
            )
            .expect("환율")
        });
        Box::pin(ready(rate.ok_or_else(|| denied(ReadEndpoint::FxRate))))
    }

    fn buying_power<'a>(
        &'a self,
        _account: &'a AccountId,
        _currency: Currency,
    ) -> BoxFuture<'a, Result<domain::BuyingPower, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::BuyingPower))))
    }

    fn daily_candles<'a>(
        &'a self,
        _instrument: &'a QuoteInstrument,
        _window: TimeInterval,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>> {
        Box::pin(ready(Err(denied(ReadEndpoint::Candles))))
    }
}

#[test]
fn a_fetched_rate_is_cached_and_not_counted_as_a_gap() {
    use application::PerformanceHistorySource as _;
    let feed: Arc<dyn PerformanceFeed> = Arc::new(GappedFx {
        fetched: AtomicUsize::new(0),
    });
    let history = FeedHistory::new(feed, Arc::new(Fixed));
    let friday = tauri::async_runtime::block_on(history.window_day(feb(6)))
        .expect("금요일")
        .expect("평가일");
    let friday_rate = friday.fx.expect("받은 환율");
    let later = tauri::async_runtime::block_on(history.window_day(feb(6)));
    assert_eq!(
        later.expect("다시").map(|day| day.fx),
        Some(Some(friday_rate))
    );
    let (carried, missing) = tauri::async_runtime::block_on(history.take_rate_gaps());
    assert_eq!((carried, missing), (0, 0), "받은 날은 캐시에서 다시 쓴다");
}

#[test]
fn a_finished_period_failure_stays_distinct_from_a_live_or_unstarted_calculation() {
    let clock: Arc<dyn Clock> = Arc::new(Fixed);
    let book = PerformanceBook::new(
        Arc::new(FeedHistory::new(
            Arc::new(DownFeed::default()),
            Arc::clone(&clock),
        )),
        clock,
    );
    let span = PerformanceSpan::SixMonths;
    assert_eq!(book.reading(span), super::PeriodReading::Waiting);
    {
        let _permit = book.running.try_acquire().expect("실행");
        assert_eq!(book.reading(span), super::PeriodReading::Calculating);
    }
    tauri::async_runtime::block_on(book.refresh(&Rows, &AccountId::new(7)));
    assert_eq!(
        book.reading(span),
        super::PeriodReading::Finished(Err(application::PerformanceRunError::Source(denied(
            ReadEndpoint::Calendar
        ))))
    );
}
