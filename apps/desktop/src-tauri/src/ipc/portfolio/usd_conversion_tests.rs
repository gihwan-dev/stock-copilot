use super::{UiCurrency, UiTotalCoverage, applied_usd_rate, to_portfolio_view};
use crate::holdings_link::InstrumentDirectory;
use domain::{
    BuyingPower, Currency, CurrencyAmounts, FxPair, FxRate, FxRateKind, FxSource, Money,
    Nanosecond, NumericInput, OffsetSeconds, Portfolio, ReportedPortfolio, TimeInterval, Timestamp,
    UnixSeconds,
};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn number(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("숫자")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&number(text), currency).expect("금액")
}

fn rate(price: &str, pair: FxPair, kind: FxRateKind) -> FxRate {
    FxRate::parse(
        &number(price),
        pair,
        kind,
        TimeInterval::try_new(at(60), at(180)).expect("구간"),
        FxSource::TossOpenApi,
    )
    .expect("환율")
}

fn portfolio(cash: &str) -> Portfolio {
    let amounts = |krw, usd| {
        CurrencyAmounts::new(money(krw, Currency::Krw), Some(money(usd, Currency::Usd)))
            .expect("통화별 금액")
    };
    Portfolio::new(
        vec![],
        BuyingPower::new(money("770", Currency::Krw), at(120)),
        BuyingPower::new(money(cash, Currency::Usd), at(120)),
    )
    .expect("잔고")
    .with_reported(ReportedPortfolio::new(
        amounts("1230", "2.504"),
        amounts("1000", "2"),
        amounts("230", "0.504"),
        at(120),
    ))
}

#[test]
fn converts_unrounded_native_amounts_at_the_applied_fractional_mid_rate() {
    let mid = rate("1345.67", FxPair::UsdKrw, FxRateKind::Mid);
    let valued = portfolio("3").evaluate(Some(mid));
    let view = to_portfolio_view(at(120), &valued, &InstrumentDirectory::default()).expect("화면");
    let converted = view.usd_conversion.expect("환산");
    assert_eq!(converted.krw_per_usd.amount, "1345.67");
    assert_eq!(converted.holdings_krw.amount, "3370");
    assert_eq!(converted.buying_power_krw.amount, "4037");
    for value in [
        converted.krw_per_usd,
        converted.holdings_krw,
        converted.buying_power_krw,
    ] {
        assert_eq!(value.currency, UiCurrency::Krw);
    }
    assert_eq!(view.total.amount, "9407");
    assert_eq!(view.currencies[1].holdings.amount, "2.50");
    assert_eq!(
        view.currencies[1]
            .buying_power
            .as_ref()
            .expect("달러")
            .amount,
        "3.00"
    );
    assert_eq!(
        view.exchange_rate_at.as_deref(),
        Some("1970-01-01T09:01:00+09:00")
    );
    assert_eq!(view.recorded_at, "1970-01-01T09:02:00+09:00");
}

#[test]
fn omits_conversion_for_missing_or_partial_assets_even_with_an_applied_rate() {
    let mid = rate("1345.67", FxPair::UsdKrw, FxRateKind::Mid);
    for valued in [
        portfolio("3").evaluate(None),
        portfolio("-1").evaluate(Some(mid)),
    ] {
        let view =
            to_portfolio_view(at(120), &valued, &InstrumentDirectory::default()).expect("화면");
        assert_eq!(view.total_coverage, UiTotalCoverage::Partial);
        assert_eq!(view.usd_conversion, None);
        assert_eq!(view.exchange_rate_at, None);
        assert_eq!(view.currencies[1].holdings.amount, "2.50");
    }
}

#[test]
fn accepts_only_one_applied_usd_krw_mid_rate() {
    let mid = rate("1345.67", FxPair::UsdKrw, FxRateKind::Mid);
    assert_eq!(
        applied_usd_rate(&[mid], UiTotalCoverage::Complete),
        Some(mid)
    );
    for rates in [
        vec![],
        vec![mid, rate("1345", FxPair::UsdKrw, FxRateKind::Mid)],
        vec![rate("1345", FxPair::UsdKrw, FxRateKind::Buy)],
        vec![rate("0.001", FxPair::KrwUsd, FxRateKind::Mid)],
    ] {
        assert_eq!(applied_usd_rate(&rates, UiTotalCoverage::Complete), None);
    }
}
