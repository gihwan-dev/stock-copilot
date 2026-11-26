use super::{
    WindowDay, day_price_evidence, derived_window_ledger, performance_max_drawdown,
    recompute_window_day, settled_cash_through,
};
use crate::cash_ledger::tests::{evidence, set};
use crate::valuation_snapshot::tests::{at, calendar, date, fx, money, tx};
use crate::{
    AssetClass, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, Currency,
    InstrumentId, LotIdentity, Market, NumericInput, Ohlcv, Performance, PerformanceCurrency,
    Price, PriceAdjustment, Quantity, RiskMetric, SettlementWindow, SnapshotError,
    SnapshotInstrumentHistory, Symbol, Transaction, TransactionKind, ValuationPriceEvidence,
};

fn buy(day: i64, quantity: &str, amount: &str) -> Transaction {
    let mut data = tx(TransactionKind::Buy, Currency::Krw, day, 1, amount)
        .data()
        .clone();
    data.symbol = Some(Symbol::try_new("A").expect("심볼"));
    data.asset_class = Some(AssetClass::Stock);
    data.quantity =
        Some(Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"));
    Transaction::try_new(data).expect("매수")
}

fn rows() -> Vec<Transaction> {
    vec![
        tx(TransactionKind::Deposit, Currency::Krw, 2, 1, "1000"),
        buy(3, "10", "500"),
    ]
}

fn history(closes: &[(i64, &str)]) -> SnapshotInstrumentHistory {
    let identity = LotIdentity::new(
        InstrumentId::try_new("A").expect("ID"),
        Symbol::try_new("A").expect("심볼"),
        Market::Kr,
        Currency::Krw,
    );
    let candles = closes
        .iter()
        .map(|(day, value)| {
            let item = Price::try_new(money(value, Currency::Krw)).expect("가격");
            Candle::try_new(
                InstrumentId::try_new("A").expect("ID"),
                CandleTime::try_new(at(*day, 0), CandlePeriod::Daily).expect("봉 시각"),
                PriceAdjustment::Raw,
                Ohlcv::try_new(
                    item,
                    item,
                    item,
                    item,
                    Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("거래량"),
                )
                .expect("OHLC"),
                CandleObservation::new(at(20, 0), CandleStatus::Finalized),
            )
            .expect("봉")
        })
        .collect();
    SnapshotInstrumentHistory::new(identity, Vec::new(), candles).expect("이력")
}

fn window(from: i64, to: i64) -> SettlementWindow {
    SettlementWindow::try_new(date(from), date(to)).expect("기간")
}

#[test]
fn settled_cash_adds_every_settled_row_up_to_the_day_from_zero() {
    let set = set(rows());
    for (day, expected) in [(1, "0"), (2, "1000"), (3, "500"), (9, "500")] {
        assert_eq!(
            settled_cash_through(&set, Currency::Krw, date(day)),
            Ok(money(expected, Currency::Krw)),
            "{day}"
        );
    }
    assert_eq!(
        settled_cash_through(&set, Currency::Usd, date(9)),
        Ok(money("0", Currency::Usd))
    );
}

#[test]
fn a_window_ledger_opens_with_the_cash_before_its_first_day_and_confirms_its_flows() {
    let set = set(rows());
    let whole = window(2, 6);
    let ledger =
        derived_window_ledger(&set, whole, Currency::Krw, evidence(&set, whole), at(20, 0))
            .expect("원장");
    assert_eq!(ledger.period().window(), whole);
    assert_eq!(
        ledger.period().flows().len(),
        1,
        "입금 하나만 외부 흐름이다"
    );
    let later = window(4, 6);
    let quiet = derived_window_ledger(&set, later, Currency::Krw, evidence(&set, later), at(20, 0))
        .expect("원장");
    assert_eq!(quiet.period().flows().len(), 0);
    assert!(
        derived_window_ledger(
            &set,
            later,
            Currency::Krw,
            evidence(&set, window(5, 6)),
            at(20, 0)
        )
        .is_err(),
        "거래 응답이 기간을 덮지 않으면 확정하지 않는다"
    );
}

#[test]
fn price_evidence_uses_the_session_close_of_the_price_date_or_reports_it_missing() {
    let full = history(&[(4, "50"), (5, "55")]);
    assert_eq!(
        day_price_evidence(calendar(5), &full, at(5, 7)),
        Ok(ValuationPriceEvidence::SessionClose {
            price: Price::try_new(money("55", Currency::Krw)).expect("가격"),
            date: date(5),
        })
    );
    assert_eq!(
        day_price_evidence(calendar(6), &full, at(6, 7)),
        Err(SnapshotError::MissingPrice)
    );
}

fn series() -> (Vec<crate::ValuationSnapshot>, Performance) {
    let set = set(rows());
    let span = window(4, 6);
    let kr = derived_window_ledger(&set, span, Currency::Krw, evidence(&set, span), at(20, 0))
        .expect("원화");
    let us = derived_window_ledger(&set, span, Currency::Usd, evidence(&set, span), at(20, 0))
        .expect("달러");
    let histories = [history(&[(4, "50"), (5, "55"), (6, "45")])];
    let snapshots: Vec<_> = (4..=6)
        .map(|day| {
            recompute_window_day(
                &WindowDay {
                    calendar: calendar(day),
                    fx: Some(fx(at(day, 7), "1300")),
                },
                &set,
                &histories,
                (&kr, &us),
                at(20, 0),
            )
            .expect("재계산")
        })
        .collect();
    let performance = Performance::calculate(
        &snapshots,
        (&kr.flow_period(), &us.flow_period()),
        &[],
        PerformanceCurrency::ConvertedKrw,
    )
    .expect("성과");
    (snapshots, performance)
}

#[test]
fn recomputed_days_value_cash_plus_shares_at_each_close() {
    let (snapshots, _performance) = series();
    let values: Vec<String> = snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .account_krw_value()
                .expect("평가")
                .clone()
                .into_parts()
                .0
                .amount()
                .as_decimal()
                .normalize()
                .to_string()
        })
        .collect();
    assert_eq!(values, ["1000", "1050", "950"]);
}

#[test]
fn return_and_drawdown_follow_the_hand_computed_series() {
    let (_snapshots, performance) = series();
    assert_eq!(performance.twr().to_string(), "-0.050000");
    match performance_max_drawdown(&performance) {
        RiskMetric::Value(value) => assert_eq!(value.to_string(), "-0.095238"),
        other @ (RiskMetric::InsufficientSamples | RiskMetric::Unavailable(_)) => {
            panic!("낙폭: {other:?}")
        }
    }
}

fn performance_of(
    transactions: Vec<Transaction>,
    histories: &[SnapshotInstrumentHistory],
    days: std::ops::RangeInclusive<i64>,
    rate: &str,
) -> (Vec<crate::ValuationSnapshot>, Performance) {
    let set = set(transactions);
    let span = window(*days.start(), *days.end());
    let kr = derived_window_ledger(&set, span, Currency::Krw, evidence(&set, span), at(20, 0))
        .expect("원화");
    let us = derived_window_ledger(&set, span, Currency::Usd, evidence(&set, span), at(20, 0))
        .expect("달러");
    let snapshots: Vec<_> = days
        .map(|day| {
            recompute_window_day(
                &WindowDay {
                    calendar: calendar(day),
                    fx: Some(fx(at(day, 7), rate)),
                },
                &set,
                histories,
                (&kr, &us),
                at(20, 0),
            )
            .expect("재계산")
        })
        .collect();
    let performance = Performance::calculate(
        &snapshots,
        (&kr.flow_period(), &us.flow_period()),
        &[],
        PerformanceCurrency::ConvertedKrw,
    )
    .expect("성과");
    (snapshots, performance)
}

#[test]
fn a_deposit_inside_the_window_is_a_flow_and_not_a_gain() {
    let mut transactions = rows();
    transactions.push(tx(TransactionKind::Deposit, Currency::Krw, 5, 1, "500"));
    let (_snapshots, performance) = performance_of(
        transactions,
        &[history(&[(4, "50"), (5, "50"), (6, "50")])],
        4..=6,
        "1300",
    );
    assert_eq!(performance.twr().to_string(), "0.000000");
    match performance_max_drawdown(&performance) {
        RiskMetric::Value(value) => assert_eq!(value.to_string(), "0.000000"),
        other @ (RiskMetric::InsufficientSamples | RiskMetric::Unavailable(_)) => {
            panic!("낙폭: {other:?}")
        }
    }
}

#[test]
fn dollar_cash_is_converted_at_each_days_rate() {
    let transactions = vec![tx(TransactionKind::Deposit, Currency::Usd, 2, 1, "10")];
    let (snapshots, performance) = performance_of(transactions, &[], 4..=5, "1300");
    let values: Vec<String> = snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .account_krw_value()
                .expect("평가")
                .clone()
                .into_parts()
                .0
                .amount()
                .as_decimal()
                .normalize()
                .to_string()
        })
        .collect();
    assert_eq!(values, ["13000", "13000"]);
    assert_eq!(performance.twr().to_string(), "0.000000");
}

#[test]
fn the_window_ends_on_the_last_valued_day_and_spans_the_requested_days() {
    use super::{PerformanceSpan, last_valuation_date, performance_window, window_dates};
    assert_eq!(last_valuation_date(at(40, 7)), Ok(date(40)));
    assert_eq!(last_valuation_date(at(40, 6)), Ok(date(39)));
    let month = performance_window(at(40, 8), PerformanceSpan::OneMonth).expect("기간");
    assert_eq!((month.from(), month.to()), (date(10), date(40)));
    let dates = window_dates(month);
    assert_eq!(dates.len(), 31);
    assert_eq!(dates.first(), Some(&date(10)));
    assert_eq!(dates.last(), Some(&date(40)));
    let year = performance_window(at(400, 8), PerformanceSpan::OneYear).expect("기간");
    assert_eq!(year.from(), date(35));
}

#[test]
fn reported_cash_anchors_the_ledger_when_transactions_miss_some_flows() {
    use super::{CashAnchor, anchored_window_ledger};
    let mut dollar_buy = tx(TransactionKind::Buy, Currency::Usd, 3, 1, "10")
        .data()
        .clone();
    dollar_buy.symbol = Some(Symbol::try_new("AAPL").expect("심볼"));
    dollar_buy.asset_class = Some(AssetClass::Stock);
    dollar_buy.quantity =
        Some(Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"));
    let mut transactions = rows();
    transactions.push(Transaction::try_new(dollar_buy).expect("달러 매수"));
    let set = set(transactions);
    let span = window(4, 6);
    assert!(
        derived_window_ledger(&set, span, Currency::Usd, evidence(&set, span), at(20, 0)).is_err(),
        "거래내역만으로는 달러 현금이 음수라 맞출 수 없다"
    );
    let reported = crate::BuyingPower::new(money("0.12", Currency::Usd), at(20, 0));
    let (dollars, anchor) =
        anchored_window_ledger(&set, span, Currency::Usd, evidence(&set, span), reported)
            .expect("보고 잔고 기준");
    assert_eq!(anchor, CashAnchor::ReportedWithOtherFlows);
    assert_eq!(dollars.period().window(), span);
    let won = crate::BuyingPower::new(money("500", Currency::Krw), at(20, 0));
    let (_won_ledger, won_anchor) =
        anchored_window_ledger(&set, span, Currency::Krw, evidence(&set, span), won).expect("원화");
    assert_eq!(won_anchor, CashAnchor::ReportedMatches);
}
