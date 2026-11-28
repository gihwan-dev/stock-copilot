use super::{ComputePerformance, PerformanceHistorySource, PerformanceRunError, SettledRowsSource};
use crate::{Clock, PortError, SystemPortError};
use domain::{
    AccountId, CalendarDate, CalendarYear, Cancellation, CompleteTransactionResponse,
    CompletionAttestation, Currency, DayNumber, FxPair, FxRate, FxRateKind, FxSource,
    MarketCalendar, MarketSession, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds,
    PerformanceSpan, SessionData, SessionInformation, SettledRowSet, Settlement, SettlementWindow,
    SnapshotCalendar, SnapshotInstrumentHistory, StoredTransaction, TimeInterval, Timestamp,
    Transaction, TransactionBatch, TransactionData, TransactionFilter, TransactionKind,
    UnixSeconds, WindowDay, valuation_time,
};
use std::future::{Future, ready};
use std::task::{Context, Poll, Waker};

fn run<F: Future>(future: F) -> F::Output {
    let mut pinned = std::pin::pin!(future);
    match pinned
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("즉시 완료"),
    }
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(1970),
        MonthNumber::new(2),
        DayNumber::new(day),
    )
    .expect("날짜")
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn day_start(day: u8) -> i64 {
    i64::from(day)
        .checked_add(30)
        .and_then(|days| days.checked_mul(86_400))
        .expect("초")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
}

fn session(day: u8, market: &domain::Market) -> MarketSession {
    let regular_market = Some(
        SessionData::try_new(
            TimeInterval::try_new(
                at(day_start(day)),
                at(day_start(day).checked_add(3_600).expect("끝")),
            )
            .expect("구간"),
            None,
        )
        .expect("세션"),
    );
    MarketSession::try_new(
        date(day),
        match market {
            domain::Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market,
                after_market: None,
            },
            domain::Market::Us | domain::Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market,
                after_market: None,
            },
        },
    )
    .expect("세션")
}

fn snapshot_calendar(day: u8) -> SnapshotCalendar {
    let calendar = |market: &domain::Market| {
        MarketCalendar::try_new(
            session(day.checked_sub(1).expect("전일"), market),
            session(day, market),
            session(day.checked_add(1).expect("익일"), market),
        )
        .expect("달력")
    };
    SnapshotCalendar::new(
        &calendar(&domain::Market::Kr),
        &calendar(&domain::Market::Us),
    )
    .expect("평가 달력")
}

fn fx(day: u8) -> FxRate {
    fx_at_rate(day, "1300")
}

fn fx_at_rate(day: u8, rate: &str) -> FxRate {
    let valued = valuation_time(date(day));
    FxRate::parse(
        &NumericInput::try_new(rate).expect("입력"),
        FxPair::UsdKrw,
        FxRateKind::Mid,
        TimeInterval::try_new(
            valued,
            valued
                .checked_add(
                    domain::TimeSpan::try_new(domain::SpanSeconds::new(3_600), Nanosecond::new(0))
                        .expect("기간"),
                )
                .expect("끝"),
        )
        .expect("구간"),
        FxSource::TossOpenApi,
    )
    .expect("환율")
}

fn account() -> AccountId {
    AccountId::new(7)
}

fn now() -> Timestamp {
    valuation_time(date(20))
}

fn deposit_in(currency: Currency, day: u8, amount: &str) -> Transaction {
    Transaction::try_new(TransactionData {
        kind: TransactionKind::Deposit,
        market: match currency {
            Currency::Krw => domain::Market::Kr,
            Currency::Usd => domain::Market::Us,
        },
        currency,
        symbol: None,
        asset_class: None,
        isin: None,
        quantity: None,
        filled_price: None,
        amount: Some(money(amount, currency)),
        commission: money("0", currency),
        tax: money("0", currency),
        net_amount: Some(money(amount, currency)),
        exchange_rate: None,
        settlement_date: date(day),
        settlement: Settlement::Settled,
        transaction_time: Some(at(day_start(day))),
        cancellation: Cancellation::Regular,
    })
    .expect("입금")
}

fn deposit(day: u8, amount: &str) -> Transaction {
    deposit_in(Currency::Krw, day, amount)
}

fn transactions() -> Vec<Transaction> {
    vec![
        deposit(1, "1000"),
        deposit_in(Currency::Usd, 1, "5"),
        deposit(15, "500"),
    ]
}

struct Rows;
impl SettledRowsSource for Rows {
    fn settled_rows(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send {
        let history = transactions()
            .into_iter()
            .map(|row| StoredTransaction::try_restore(row, at(day_start(1)), None).expect("행"))
            .collect();
        ready(Ok(SettledRowSet::try_restore(
            account(),
            history,
            TransactionBatch::try_new(Vec::new()).expect("목록"),
            Some(now()),
        )
        .expect("행 집합")))
    }
}

#[derive(Clone, Copy)]
enum Days {
    Weekdays,
    FirstWithoutFx,
    MovingFx,
    None,
}

struct Source(Days);
impl PerformanceHistorySource for Source {
    fn reported_cash(
        &self,
        _account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Option<domain::BuyingPower>> + Send {
        let amount = match currency {
            Currency::Krw => "1500",
            Currency::Usd => "5",
        };
        ready(Some(domain::BuyingPower::new(
            money(amount, currency),
            now(),
        )))
    }

    fn complete_transactions(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<CompleteTransactionResponse, PortError>> + Send {
        ready(Ok(
            CompleteTransactionResponse::after_last_page_without_quarantine(
                account(),
                SettlementWindow::try_new(date(1), date(25)).expect("기간"),
                TransactionFilter::all(),
                TransactionBatch::try_new(transactions()).expect("목록"),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("완전 응답"),
        ))
    }

    fn window_day(
        &self,
        date: CalendarDate,
    ) -> impl Future<Output = Result<Option<WindowDay>, PortError>> + Send {
        let found = (10_u8..=20_u8)
            .filter(|day| day % 7 != 0)
            .find(|day| self::date(*day) == date);
        ready(Ok(match (self.0, found) {
            (Days::Weekdays, Some(day)) => Some(WindowDay {
                calendar: snapshot_calendar(day),
                fx: Some(fx(day)),
            }),
            (Days::MovingFx, Some(day)) => Some(WindowDay {
                calendar: snapshot_calendar(day),
                fx: Some(fx_at_rate(day, if day < 15 { "1100" } else { "1300" })),
            }),
            (Days::FirstWithoutFx, Some(day)) => Some(WindowDay {
                calendar: snapshot_calendar(day),
                fx: (day != 10).then(|| fx(day)),
            }),
            (Days::Weekdays | Days::FirstWithoutFx | Days::MovingFx | Days::None, _) => None,
        }))
    }

    fn histories(
        &self,
        _rows: &SettledRowSet,
        _window: SettlementWindow,
    ) -> impl Future<Output = Result<Vec<SnapshotInstrumentHistory>, PortError>> + Send {
        ready(Ok(Vec::new()))
    }
}

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        now()
    }
}

#[test]
fn a_deposit_only_month_has_a_flat_return_and_starts_on_the_first_valued_day() {
    let summary = run(ComputePerformance {
        rows: &Rows,
        source: &Source(Days::Weekdays),
        clock: &Fixed,
    }
    .execute(&account(), PerformanceSpan::OneMonth))
    .expect("성과");
    assert_eq!(summary.performance.start(), date(10));
    assert_eq!(summary.performance.end(), date(20));
    assert_eq!(summary.performance.twr().to_string(), "0.000000");
    assert_eq!(summary.skipped, Vec::new());
    assert_eq!(summary.span, PerformanceSpan::OneMonth);
}

#[test]
fn no_valued_day_in_the_window_is_reported_instead_of_a_zero_return() {
    assert_eq!(
        run(ComputePerformance {
            rows: &Rows,
            source: &Source(Days::None),
            clock: &Fixed,
        }
        .execute(&account(), PerformanceSpan::OneMonth))
        .map(|summary| summary.span),
        Err(PerformanceRunError::NoValuedDays)
    );
}

#[test]
fn a_failed_first_day_narrows_the_window_and_is_listed_as_skipped() {
    let summary = run(ComputePerformance {
        rows: &Rows,
        source: &Source(Days::FirstWithoutFx),
        clock: &Fixed,
    }
    .execute(&account(), PerformanceSpan::OneMonth))
    .expect("성과");
    assert_eq!(summary.performance.start(), date(11));
    assert_eq!(summary.performance.end(), date(20));
    assert_eq!(
        summary
            .skipped
            .iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>(),
        vec![date(10)]
    );
    assert_eq!(summary.performance.twr().to_string(), "0.000000");
}

#[test]
fn one_day_is_recomputed_from_closes_and_cash_for_the_daily_record() {
    let compute = ComputePerformance {
        rows: &Rows,
        source: &Source(Days::Weekdays),
        clock: &Fixed,
    };
    let snapshot = run(compute.recompute_day(&account(), date(16))).expect("하루");
    assert_eq!(snapshot.settlement_date(), date(16));
    assert_eq!(
        run(compute.recompute_day(&account(), date(14))).map(|found| found.settlement_date()),
        Err(PerformanceRunError::NoValuedDays),
        "평가하지 않는 날은 기록하지 않는다"
    );
}

#[test]
fn reported_cash_that_matches_the_rows_is_marked_as_matching() {
    let summary = run(ComputePerformance {
        rows: &Rows,
        source: &Source(Days::Weekdays),
        clock: &Fixed,
    }
    .execute(&account(), PerformanceSpan::OneMonth))
    .expect("성과");
    assert_eq!(
        summary.cash,
        vec![
            (Currency::Krw, domain::CashAnchor::ReportedMatches),
            (Currency::Usd, domain::CashAnchor::ReportedMatches),
        ]
    );
}

struct AsOf(CalendarDate);
impl Clock for AsOf {
    fn now(&self) -> Timestamp {
        valuation_time(self.0)
    }
}

#[test]
fn a_wider_read_recovers_six_months_with_its_own_ledger_and_actual_first_session() {
    let clock = AsOf(
        CalendarDate::from_parts(
            CalendarYear::new(1970),
            MonthNumber::new(8),
            DayNumber::new(15),
        )
        .expect("평가일"),
    );
    let calculate = ComputePerformance {
        rows: &Rows,
        source: &Source(Days::MovingFx),
        clock: &clock,
    };
    let six = PerformanceSpan::SixMonths;
    let interrupted = ComputePerformance {
        rows: &Rows,
        source: &Source(Days::None),
        clock: &clock,
    };
    assert_eq!(
        run(interrupted.execute(&account(), six)),
        Err(PerformanceRunError::NoValuedDays)
    );
    let independent = run(calculate.execute(&account(), six)).expect("독립 6개월");
    let (year, recovered) =
        run(calculate.execute_with_recovery(&account(), PerformanceSpan::OneYear, &[six]))
            .expect("1년");
    assert_eq!(year.performance.start(), date(10));
    assert_eq!(
        independent.performance.start(),
        date(15),
        "2월 14일 휴장 뒤 첫 평가일"
    );
    assert_ne!(year.performance.twr(), independent.performance.twr());
    assert_eq!(recovered, [independent]);
}

#[test]
fn an_annual_result_does_not_fill_a_period_without_valuation_days() {
    let clock = AsOf(
        CalendarDate::from_parts(
            CalendarYear::new(1970),
            MonthNumber::new(9),
            DayNumber::new(15),
        )
        .expect("평가일"),
    );
    let calculate = ComputePerformance {
        rows: &Rows,
        source: &Source(Days::Weekdays),
        clock: &clock,
    };
    let (year, recovered) = run(calculate.execute_with_recovery(
        &account(),
        PerformanceSpan::OneYear,
        &[PerformanceSpan::SixMonths],
    ))
    .expect("1년");
    assert_eq!(year.performance.start(), date(10));
    assert_eq!(recovered, Vec::new());
}
