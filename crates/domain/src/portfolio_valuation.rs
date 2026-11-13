use crate::{
    Currency, EvaluationError, FxRate, HoldingValuation, KrwValuation, KrwValuationComponent,
    Money, Portfolio, ResolvedOrderInstrument, ValuationAccuracy,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotalCompleteness {
    Complete,
    Incomplete,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Total<T> {
    Complete(CompleteTotal<T>),
    Incomplete(IncompleteTotal<T>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTotal<T>(T);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteTotal<T>(T);
impl<T> CompleteTotal<T> {
    #[must_use]
    pub fn value(&self) -> &T {
        &self.0
    }
}
impl<T> IncompleteTotal<T> {
    #[must_use]
    pub fn subtotal(&self) -> &T {
        &self.0
    }
}
impl<T> Total<T> {
    #[must_use]
    pub fn completeness(&self) -> TotalCompleteness {
        match self {
            Self::Complete(_) => TotalCompleteness::Complete,
            Self::Incomplete(_) => TotalCompleteness::Incomplete,
        }
    }

    fn new(value: T) -> Self {
        Self::Complete(CompleteTotal(value))
    }
    fn current(&self) -> &T {
        match self {
            Self::Complete(value) => &value.0,
            Self::Incomplete(value) => &value.0,
        }
    }
    fn update(&mut self, value: T) {
        *self = match self {
            Self::Complete(_) => Self::Complete(CompleteTotal(value)),
            Self::Incomplete(_) => Self::Incomplete(IncompleteTotal(value)),
        };
    }
    fn exclude(&mut self)
    where
        T: Clone,
    {
        *self = Self::Incomplete(IncompleteTotal(self.current().clone()));
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuedHolding {
    instrument: ResolvedOrderInstrument,
    valuation: HoldingValuation,
    currency_sum: Result<(), EvaluationError>,
    conversion: Result<(), EvaluationError>,
}
impl ValuedHolding {
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn valuation(&self) -> HoldingValuation {
        self.valuation
    }
    pub const fn currency_sum(&self) -> Result<(), EvaluationError> {
        self.currency_sum
    }
    pub const fn conversion(&self) -> Result<(), EvaluationError> {
        self.conversion
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetEstimate {
    valuation: KrwValuation,
    accuracy: ValuationAccuracy,
}
impl AssetEstimate {
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.accuracy
    }
    #[must_use]
    pub fn into_parts(self) -> (KrwValuation, ValuationAccuracy) {
        (self.valuation, self.accuracy)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioValuation {
    holdings: Vec<ValuedHolding>,
    krw: Total<Money>,
    usd: Total<Money>,
    valuation: Total<KrwValuation>,
    assets: Total<AssetEstimate>,
    assets_accuracy: ValuationAccuracy,
    buying_power_errors: Vec<(Currency, EvaluationError)>,
}
impl PortfolioValuation {
    #[must_use]
    pub fn holdings(&self) -> &[ValuedHolding] {
        &self.holdings
    }
    #[must_use]
    pub fn currency_total(&self, currency: Currency) -> &Total<Money> {
        match currency {
            Currency::Krw => &self.krw,
            Currency::Usd => &self.usd,
        }
    }
    #[must_use]
    pub fn valuation(&self) -> &Total<KrwValuation> {
        &self.valuation
    }
    #[must_use]
    pub fn assets(&self) -> &Total<AssetEstimate> {
        &self.assets
    }
    #[must_use]
    pub const fn assets_accuracy(&self) -> ValuationAccuracy {
        self.assets_accuracy
    }
    #[must_use]
    pub fn buying_power_errors(&self) -> &[(Currency, EvaluationError)] {
        &self.buying_power_errors
    }

    pub(crate) fn evaluate(portfolio: &Portfolio, rate: Option<FxRate>) -> Self {
        let mut krw = Total::new(crate::holding::zero(Currency::Krw));
        let mut usd = Total::new(crate::holding::zero(Currency::Usd));
        let mut valuation = Total::new(KrwValuation::zero());
        let mut holdings = Vec::new();
        for holding in portfolio.holdings() {
            let evaluated = holding.evaluate();
            let sum = match holding.currency() {
                Currency::Krw => &mut krw,
                Currency::Usd => &mut usd,
            };
            let currency_sum = match evaluated.selected().and_then(|value| {
                sum.current()
                    .checked_add(&value)
                    .map_err(EvaluationError::Arithmetic)
            }) {
                Ok(value) => {
                    sum.update(value);
                    Ok(())
                }
                Err(error) => {
                    sum.exclude();
                    Err(error)
                }
            };
            let conversion = append(&mut valuation, evaluated.selected(), rate);
            holdings.push(ValuedHolding {
                instrument: holding.instrument().clone(),
                valuation: evaluated,
                currency_sum,
                conversion,
            });
        }
        let mut assets = valuation.clone();
        let mut buying_power_errors = Vec::new();
        for currency in [Currency::Krw, Currency::Usd] {
            if let Err(error) = append(
                &mut assets,
                Ok(portfolio.buying_power(currency).money()),
                rate,
            ) {
                buying_power_errors.push((currency, error));
            }
        }
        Self {
            holdings,
            krw,
            usd,
            valuation,
            assets: estimate_assets(assets),
            assets_accuracy: ValuationAccuracy::Approximate,
            buying_power_errors,
        }
    }
}

fn estimate_assets(assets: Total<KrwValuation>) -> Total<AssetEstimate> {
    match assets {
        Total::Complete(value) => Total::Complete(CompleteTotal(AssetEstimate {
            valuation: value.0,
            accuracy: ValuationAccuracy::Approximate,
        })),
        Total::Incomplete(value) => Total::Incomplete(IncompleteTotal(AssetEstimate {
            valuation: value.0,
            accuracy: ValuationAccuracy::Approximate,
        })),
    }
}

fn convert(value: Money, rate: Option<FxRate>) -> Result<KrwValuation, EvaluationError> {
    if value.amount().as_decimal().is_zero() {
        return Ok(KrwValuation::zero());
    }
    if value.currency() == Currency::Krw {
        return KrwValuation::from_native_krw(value).map_err(EvaluationError::Conversion);
    }
    rate.ok_or(EvaluationError::MissingFx)?
        .convert_for_aggregation(value)
        .map(|converted| KrwValuation::from_component(KrwValuationComponent::Converted(converted)))
        .map_err(EvaluationError::Conversion)
}
fn append(
    total: &mut Total<KrwValuation>,
    value: Result<Money, EvaluationError>,
    rate: Option<FxRate>,
) -> Result<(), EvaluationError> {
    let result = value
        .and_then(|money| convert(money, rate))
        .and_then(|converted| {
            total
                .current()
                .checked_add(&converted)
                .map_err(EvaluationError::Conversion)
        });
    match result {
        Ok(combined) => {
            total.update(combined);
            Ok(())
        }
        Err(error) => {
            total.exclude();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, BuyingPower, Comparison, Currency, EvaluationError, FxConversionError,
        FxPair, FxRate, FxRateKind, FxSource, Holding, HoldingError, HoldingObservation,
        HoldingReturn, KrwValuation, Market, Money, MoneyError, Nanosecond, NumericError,
        NumericInput, OffsetSeconds, Portfolio, PortfolioError, Price, Quantity, Ratio,
        ReportedHolding, ResolvedOrderInstrument, Symbol, TimeInterval, Timestamp, Total,
        UnixSeconds, ValuationAccuracy, ValuationSource,
    };
    use proptest::prop_assert_eq;
    use proptest::test_runner::{Config, TestRunner};

    fn input(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("입력")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&input(value), currency).expect("금액")
    }
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn rate(value: &str, kind: FxRateKind) -> FxRate {
        FxRate::parse(
            &input(value),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(
                time(),
                Timestamp::from_parts(
                    UnixSeconds::new(1060),
                    Nanosecond::new(0),
                    OffsetSeconds::new(0),
                )
                .expect("끝"),
            )
            .expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn holding(
        symbol: &str,
        currency: Currency,
        quantity: &str,
        average: &str,
        current: Option<&str>,
    ) -> Holding {
        let market = match currency {
            Currency::Krw => Market::Kr,
            Currency::Usd => Market::Us,
        };
        Holding::new(
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(symbol).expect("심볼"),
                market,
            )
            .expect("종목"),
            Quantity::parse(&input(quantity)).expect("수량"),
            AcquisitionPrice::try_new(money(average, currency)).expect("취득가"),
            HoldingObservation::new(
                current.map(|value| Price::parse(&input(value), currency).expect("현재가")),
                None,
                time(),
            ),
        )
        .expect("보유")
    }
    fn portfolio(holdings: Vec<Holding>, krw: &str, usd: &str) -> Portfolio {
        Portfolio::new(
            holdings,
            BuyingPower::new(money(krw, Currency::Krw), time()).expect("원화"),
            BuyingPower::new(money(usd, Currency::Usd), time()).expect("달러"),
        )
        .expect("포트폴리오")
    }
    fn complete<T: Clone>(total: &Total<T>) -> T {
        assert_eq!(total.completeness(), crate::TotalCompleteness::Complete);
        total.current().clone()
    }
    fn partial<T: Clone>(total: &Total<T>) -> T {
        assert_eq!(total.completeness(), crate::TotalCompleteness::Incomplete);
        total.current().clone()
    }
    fn asset_amount(total: crate::AssetEstimate) -> Money {
        let (valuation, accuracy) = total.into_parts();
        assert_eq!(accuracy, ValuationAccuracy::Approximate);
        amount(valuation)
    }
    fn amount(total: KrwValuation) -> Money {
        total.into_parts().0
    }

    #[test]
    fn r11_acquisition_snapshots() {
        for (quantity, average, expected) in [
            ("10", "100", "1000"),
            ("20", "110", "2200"),
            ("15", "110", "1650"),
            ("20", "110", "2200"),
            ("10", "110", "1100"),
            ("10", "115", "1150"),
        ] {
            let item = holding("A", Currency::Usd, quantity, average, Some(average));
            assert_eq!(
                item.evaluate().computed().expect("평가").purchase(),
                money(expected, Currency::Usd)
            );
        }
    }
    #[test]
    fn r11_products() {
        for (currency, quantity, average, current, expected, purchase, profit) in [
            (
                Currency::Krw,
                "100",
                "72000",
                "72000",
                "7200000",
                "7200000",
                "0",
            ),
            (Currency::Usd, "50", "150", "150", "7500", "7500", "0"),
            (Currency::Usd, "50", "180", "180", "9000", "9000", "0"),
            (Currency::Usd, "60", "220", "220", "13200", "13200", "0"),
            (Currency::Usd, "100", "70", "65", "6500", "7000", "-500"),
        ] {
            let evaluated = holding("A", currency, quantity, average, Some(current)).evaluate();
            let computed = evaluated.computed().expect("평가");
            assert_eq!(computed.valuation(), money(expected, currency));
            assert_eq!(computed.purchase(), money(purchase, currency));
            assert_eq!(computed.profit(), money(profit, currency));
            assert_eq!(evaluated.selected(), Ok(money(expected, currency)));
            assert_eq!(evaluated.source(), ValuationSource::Calculated);
            assert_eq!(evaluated.comparison(), Comparison::NotProvided);
        }
    }
    #[test]
    fn r11_mixed_total() {
        let items = vec![
            holding("KR", Currency::Krw, "100", "100000", Some("100000")),
            holding("US", Currency::Usd, "50", "100", Some("100")),
        ];
        let book = portfolio(items, "200", "3");
        let fx = rate("1375", FxRateKind::Mid);
        let result = book.evaluate(Some(fx));
        assert_eq!(
            complete(result.currency_total(Currency::Krw)),
            money("10000000", Currency::Krw)
        );
        assert_eq!(
            complete(result.currency_total(Currency::Usd)),
            money("5000", Currency::Usd)
        );
        assert_eq!(
            complete(result.valuation()).into_parts(),
            (
                money("16875000", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
        assert_eq!(
            asset_amount(complete(result.assets())),
            money("16879325", Currency::Krw)
        );
        assert_eq!(result.assets_accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(result.buying_power_errors(), []);
        assert_eq!(book.buying_power(Currency::Krw).observed_at(), time());
        for row in result.holdings() {
            assert_eq!(row.currency_sum(), Ok(()));
            assert_eq!(row.conversion(), Ok(()));
            assert!(
                book.holdings()
                    .iter()
                    .any(|item| item.instrument() == row.instrument())
            );
            assert_eq!(row.valuation().source(), ValuationSource::Calculated);
        }
    }
    #[test]
    fn reported_values_are_authoritative_and_discrepancies_are_visible() {
        let item = holding("A", Currency::Usd, "2", "3", Some("4"));
        for (valuation, profit, rate_text, comparison) in [
            (
                "8",
                "2",
                "0.3333333333333333333333333333",
                Comparison::Matches,
            ),
            (
                "9",
                "2",
                "0.3333333333333333333333333333",
                Comparison::Different,
            ),
            (
                "8",
                "3",
                "0.3333333333333333333333333333",
                Comparison::Different,
            ),
            ("8", "2", "0.5", Comparison::Different),
        ] {
            let report = ReportedHolding::new(
                money(valuation, Currency::Usd),
                money(profit, Currency::Usd),
                Ratio::parse(&input(rate_text)).expect("비율"),
            )
            .expect("토스");
            assert_eq!(report.profit(), money(profit, Currency::Usd));
            let reported_item = Holding::new(
                item.instrument().clone(),
                item.quantity(),
                item.acquisition_price(),
                HoldingObservation::new(item.current_price(), Some(report), time()),
            )
            .expect("보유");
            assert_eq!(reported_item.observed_at(), time());
            let result = reported_item.evaluate();
            assert_eq!(result.source(), ValuationSource::TossOpenApi);
            assert_eq!(result.comparison(), comparison);
            assert_eq!(result.selected(), Ok(money(valuation, Currency::Usd)));
            assert_eq!(
                amount(complete(
                    portfolio(vec![reported_item], "0", "0")
                        .evaluate(Some(rate("1", FxRateKind::Mid)))
                        .valuation()
                )),
                money(valuation, Currency::Krw)
            );
        }
    }
    #[test]
    fn reported_exact_and_missing_price_comparison() {
        let exact = holding("B", Currency::Krw, "2", "2", Some("3"));
        let report = ReportedHolding::new(
            money("6", Currency::Krw),
            money("2", Currency::Krw),
            Ratio::parse(&input("0.5")).expect("비율"),
        )
        .expect("토스");
        let exact_reported = Holding::new(
            exact.instrument().clone(),
            exact.quantity(),
            exact.acquisition_price(),
            HoldingObservation::new(exact.current_price(), Some(report), time()),
        )
        .expect("보유");
        assert_eq!(exact_reported.evaluate().comparison(), Comparison::Matches);
        let missing = Holding::new(
            exact.instrument().clone(),
            exact.quantity(),
            exact.acquisition_price(),
            HoldingObservation::new(None, Some(report), time()),
        )
        .expect("시세 없음");
        assert_eq!(missing.evaluate().comparison(), Comparison::Unavailable);
        assert_eq!(
            missing.evaluate().selected(),
            Err(EvaluationError::MissingPrice)
        );
    }
    #[test]
    fn free_acquisition_zero_quantity_and_approximate_return_are_distinct() {
        let free = holding("A", Currency::Krw, "3", "0", Some("5"))
            .evaluate()
            .computed()
            .expect("무상");
        assert_eq!(free.profit(), money("15", Currency::Krw));
        assert_eq!(free.return_rate(), Ok(HoldingReturn::ZeroCost));
        let empty = holding("A", Currency::Usd, "0", "100", None)
            .evaluate()
            .computed()
            .expect("0주");
        assert_eq!(empty.valuation(), money("0", Currency::Usd));
        assert_eq!(empty.return_rate(), Ok(HoldingReturn::ZeroCost));
        let repeating = holding("A", Currency::Usd, "1", "3", Some("4"))
            .evaluate()
            .computed()
            .expect("평가");
        assert_eq!(
            repeating.return_rate(),
            money("1", Currency::Usd)
                .checked_ratio(&money("3", Currency::Usd))
                .map(HoldingReturn::Defined)
        );
        let zero_report = ReportedHolding::new(
            money("15", Currency::Krw),
            money("15", Currency::Krw),
            Ratio::parse(&input("0")).expect("비율"),
        )
        .expect("보고");
        let original = holding("A", Currency::Krw, "3", "0", Some("5"));
        let reported = Holding::new(
            original.instrument().clone(),
            original.quantity(),
            original.acquisition_price(),
            HoldingObservation::new(original.current_price(), Some(zero_report), time()),
        )
        .expect("무상");
        assert_eq!(reported.evaluate().comparison(), Comparison::Different);
    }
    #[test]
    fn invalid_inputs_and_resource_limits_are_rejected_without_echoing_inputs() {
        assert_eq!(
            AcquisitionPrice::try_new(money("-1", Currency::Krw)),
            Err(HoldingError::NegativeAcquisitionPrice)
        );
        assert_eq!(
            AcquisitionPrice::try_new(money("-0", Currency::Krw))
                .expect("음수 0")
                .money(),
            money("0", Currency::Krw)
        );
        assert_eq!(
            Quantity::parse(&input("-1")),
            Err(NumericError::NegativeQuantity)
        );
        assert_eq!(
            ReportedHolding::new(
                money("-1", Currency::Krw),
                money("0", Currency::Krw),
                Ratio::parse(&input("0")).expect("비율")
            ),
            Err(HoldingError::NegativeValuation)
        );
        assert_eq!(
            ReportedHolding::new(
                money("1", Currency::Krw),
                money("0", Currency::Usd),
                Ratio::parse(&input("0")).expect("비율")
            ),
            Err(HoldingError::CurrencyMismatch)
        );
    }
    #[test]
    fn holding_currency_boundaries() {
        let item = holding("A", Currency::Krw, "1", "1", Some("1"));
        for observation in [
            HoldingObservation::new(
                Some(Price::parse(&input("1"), Currency::Usd).expect("가격")),
                None,
                time(),
            ),
            HoldingObservation::new(
                None,
                Some(
                    ReportedHolding::new(
                        money("1", Currency::Usd),
                        money("0", Currency::Usd),
                        Ratio::parse(&input("0")).expect("비율"),
                    )
                    .expect("보고"),
                ),
                time(),
            ),
        ] {
            assert_eq!(
                Holding::new(
                    item.instrument().clone(),
                    item.quantity(),
                    item.acquisition_price(),
                    observation
                ),
                Err(HoldingError::CurrencyMismatch)
            );
        }
        assert_eq!(
            Holding::new(
                item.instrument().clone(),
                item.quantity(),
                AcquisitionPrice::try_new(money("1", Currency::Usd)).expect("단가"),
                HoldingObservation::new(None, None, time())
            ),
            Err(HoldingError::CurrencyMismatch)
        );
    }
    #[test]
    fn portfolio_limits_and_cash_boundaries() {
        let item = holding("A", Currency::Krw, "1", "1", Some("1"));
        let krw = BuyingPower::new(money("0", Currency::Krw), time()).expect("원화");
        let usd = BuyingPower::new(money("0", Currency::Usd), time()).expect("달러");
        assert_eq!(
            BuyingPower::new(money("-1", Currency::Krw), time()),
            Err(PortfolioError::NegativeBuyingPower)
        );
        assert_eq!(
            Portfolio::new(vec![], usd, krw),
            Err(PortfolioError::CurrencyMismatch)
        );
        assert_eq!(
            Portfolio::new(vec![item.clone(), item.clone()], krw, usd),
            Err(PortfolioError::DuplicateInstrument)
        );
        assert_eq!(
            Portfolio::new(vec![item; 4097], krw, usd),
            Err(PortfolioError::TooManyHoldings)
        );
        let at_limit = (0_u32..4096)
            .map(|index| holding(&format!("S{index}"), Currency::Krw, "0", "0", None))
            .collect();
        assert_eq!(
            Portfolio::new(at_limit, krw, usd)
                .expect("상한")
                .holdings()
                .len(),
            4096
        );
    }
    #[test]
    fn failures_exclude_only_affected_holdings_and_mark_subtotals() {
        let good = holding("A", Currency::Krw, "1", "2", Some("3"));
        let scenarios = [
            holding("B", Currency::Usd, "1", "1", None),
            holding(
                "B",
                Currency::Usd,
                "79228162514264337593543950335",
                "1",
                Some("2"),
            ),
            holding(
                "B",
                Currency::Usd,
                "0.0000000000000000000000000001",
                "1",
                Some("1.1"),
            ),
        ];
        for (bad, error) in scenarios.into_iter().zip([
            EvaluationError::MissingPrice,
            EvaluationError::Arithmetic(MoneyError::Overflow),
            EvaluationError::Arithmetic(MoneyError::PrecisionLoss),
        ]) {
            let evaluated = portfolio(vec![bad, good.clone()], "0", "0").evaluate(None);
            assert_eq!(
                amount(partial(evaluated.valuation())),
                money("3", Currency::Krw)
            );
            assert_eq!(
                partial(evaluated.currency_total(Currency::Usd)),
                money("0", Currency::Usd)
            );
            let row = evaluated.holdings().last().expect("실패 행");
            assert_eq!(row.valuation().computed(), Err(error));
            assert_eq!(row.currency_sum(), Err(error));
            assert_eq!(row.conversion(), Err(error));
        }
    }
    #[test]
    fn missing_and_invalid_fx_keep_native_totals() {
        let good = holding("A", Currency::Krw, "1", "2", Some("3"));
        let missing_fx = portfolio(
            vec![holding("B", Currency::Usd, "1", "1", Some("2")), good],
            "0",
            "1",
        )
        .evaluate(None);
        assert_eq!(
            amount(partial(missing_fx.valuation())),
            money("3", Currency::Krw)
        );
        assert_eq!(
            complete(missing_fx.currency_total(Currency::Usd)),
            money("2", Currency::Usd)
        );
        assert_eq!(
            missing_fx.buying_power_errors(),
            &[(Currency::Usd, EvaluationError::MissingFx)]
        );
        assert_eq!(
            asset_amount(partial(missing_fx.assets())),
            money("3", Currency::Krw)
        );
        for (fx, expected) in [
            (rate("2", FxRateKind::Buy), FxConversionError::MidRequired),
            (
                rate("79228162514264337593543950335", FxRateKind::Mid),
                FxConversionError::Arithmetic(MoneyError::Overflow),
            ),
        ] {
            let result = portfolio(
                vec![holding("A", Currency::Usd, "2", "1", Some("1"))],
                "0",
                "0",
            )
            .evaluate(Some(fx));
            assert_eq!(
                result.holdings().first().expect("행").conversion(),
                Err(EvaluationError::Conversion(expected))
            );
            assert_eq!(
                amount(partial(result.valuation())),
                money("0", Currency::Krw)
            );
        }
    }
    #[test]
    fn empty_cash_only_and_sum_overflow_remain_explicit() {
        let empty = portfolio(vec![], "0", "0").evaluate(None);
        assert_eq!(
            complete(empty.valuation()).accuracy(),
            ValuationAccuracy::Exact
        );
        assert_eq!(
            asset_amount(complete(empty.assets())),
            money("0", Currency::Krw)
        );
        assert_eq!(empty.assets_accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(
            complete(empty.assets()).accuracy(),
            ValuationAccuracy::Approximate
        );
        let cash = portfolio(vec![], "10", "2").evaluate(Some(rate("1375", FxRateKind::Mid)));
        assert_eq!(
            asset_amount(complete(cash.assets())),
            money("2760", Currency::Krw)
        );
        let overflow = portfolio(
            vec![
                holding(
                    "A",
                    Currency::Krw,
                    "1",
                    "1",
                    Some("79228162514264337593543950335"),
                ),
                holding("B", Currency::Krw, "1", "1", Some("1")),
            ],
            "1",
            "0",
        )
        .evaluate(None);
        assert_eq!(
            partial(overflow.currency_total(Currency::Krw)),
            money("79228162514264337593543950335", Currency::Krw)
        );
        assert_eq!(
            amount(partial(overflow.valuation())),
            money("79228162514264337593543950335", Currency::Krw)
        );
        assert_eq!(
            overflow.buying_power_errors(),
            &[(
                Currency::Krw,
                EvaluationError::Conversion(FxConversionError::Arithmetic(MoneyError::Overflow))
            )]
        );
    }
    fn integer_holding(symbol: &str, currency: Currency, quantity: u32, price: u32) -> Holding {
        holding(
            symbol,
            currency,
            &quantity.to_string(),
            "1",
            Some(&price.to_string()),
        )
    }
    #[test]
    fn independent_integer_totals_are_permutation_invariant_and_partial_is_a_filtered_total() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_u32..10000, 1_u32..1000, 0_u32..10000, 1_u32..1000),
                |(kr_quantity, kr_price, us_quantity, us_price)| {
                    let kr_expected = u64::from(kr_quantity)
                        .checked_mul(u64::from(kr_price))
                        .expect("정수 곱");
                    let us_expected = u64::from(us_quantity)
                        .checked_mul(u64::from(us_price))
                        .expect("정수 곱");
                    let expected = kr_expected
                        .checked_add(us_expected.checked_mul(1375).expect("환산"))
                        .expect("합");
                    let kr = integer_holding("KR", Currency::Krw, kr_quantity, kr_price);
                    let us = integer_holding("US", Currency::Usd, us_quantity, us_price);
                    let fx = Some(rate("1375", FxRateKind::Mid));
                    let forward = portfolio(vec![kr.clone(), us.clone()], "0", "0").evaluate(fx);
                    let reverse = portfolio(vec![us.clone(), kr.clone()], "0", "0").evaluate(fx);
                    prop_assert_eq!(&forward, &reverse);
                    prop_assert_eq!(
                        complete(forward.currency_total(Currency::Krw)),
                        money(&kr_expected.to_string(), Currency::Krw)
                    );
                    prop_assert_eq!(
                        complete(forward.currency_total(Currency::Usd)),
                        money(&us_expected.to_string(), Currency::Usd)
                    );
                    prop_assert_eq!(
                        amount(complete(forward.valuation())),
                        money(&expected.to_string(), Currency::Krw)
                    );
                    let incomplete = portfolio(
                        vec![kr, us, holding("AA", Currency::Usd, "1", "1", None)],
                        "0",
                        "0",
                    )
                    .evaluate(fx);
                    prop_assert_eq!(
                        partial(incomplete.valuation()),
                        complete(forward.valuation())
                    );
                    Ok(())
                },
            )
            .expect("4096 정수 기준");
    }
    #[test]
    fn r11_cost_conversion_inputs_and_exchange_effects() {
        for (usd, fx, expected) in [
            ("7507.50", "1300", "9759750"),
            ("9009.00", "1350", "12162150"),
            ("13200", "1400", "18480000"),
            ("13.47", "1400", "18858"),
        ] {
            let book = portfolio(
                vec![holding("A", Currency::Usd, "1", usd, Some(usd))],
                "0",
                "0",
            );
            assert_eq!(
                amount(complete(
                    book.evaluate(Some(rate(fx, FxRateKind::Mid))).valuation()
                )),
                money(expected, Currency::Krw)
            );
        }
    }
    #[test]
    fn r11_historical_price_and_exchange_effects() {
        let original = portfolio(
            vec![holding("A", Currency::Usd, "100", "70", Some("70"))],
            "0",
            "0",
        );
        let current = portfolio(
            vec![holding("A", Currency::Usd, "100", "70", Some("65"))],
            "0",
            "0",
        );
        let before = amount(complete(
            original
                .evaluate(Some(rate("1200", FxRateKind::Mid)))
                .valuation(),
        ));
        let now = amount(complete(
            current
                .evaluate(Some(rate("1100", FxRateKind::Mid)))
                .valuation(),
        ));
        assert_eq!(
            now.checked_sub(&before),
            Ok(money("-1250000", Currency::Krw))
        );
        let price_effect = rate("1200", FxRateKind::Mid)
            .convert_for_historical_valuation(money("-500", Currency::Usd))
            .expect("가격 효과")
            .into_parts()
            .0;
        assert_eq!(price_effect, money("-600000", Currency::Krw));
        let same_price_old_fx = amount(complete(
            current
                .evaluate(Some(rate("1200", FxRateKind::Mid)))
                .valuation(),
        ));
        let exchange_effect = now.checked_sub(&same_price_old_fx).expect("환 효과");
        assert_eq!(exchange_effect, money("-650000", Currency::Krw));
        assert_eq!(
            price_effect.checked_add(&exchange_effect),
            Ok(money("-1250000", Currency::Krw))
        );
    }
    #[test]
    fn conversion_precision_loss_and_addition_precision_loss_are_partial() {
        let tiny = holding(
            "A",
            Currency::Usd,
            "1",
            "0",
            Some("0.0000000000000000000000000001"),
        );
        let evaluated =
            portfolio(vec![tiny], "0", "0").evaluate(Some(rate("1.1", FxRateKind::Mid)));
        assert_eq!(
            evaluated.holdings().first().expect("행").conversion(),
            Err(EvaluationError::Conversion(FxConversionError::Arithmetic(
                MoneyError::PrecisionLoss
            )))
        );
        assert_eq!(
            amount(partial(evaluated.valuation())),
            money("0", Currency::Krw)
        );
        let big = holding(
            "A",
            Currency::Krw,
            "1",
            "0",
            Some("79228162514264337593543950334"),
        );
        let small = holding("B", Currency::Krw, "1", "0", Some("0.1"));
        let summed = portfolio(vec![big, small], "0", "0").evaluate(None);
        assert_eq!(
            summed.holdings().last().expect("행").currency_sum(),
            Err(EvaluationError::Arithmetic(MoneyError::PrecisionLoss))
        );
        assert_eq!(
            amount(partial(summed.valuation())),
            money("79228162514264337593543950334", Currency::Krw)
        );
    }
    #[test]
    fn acquisition_and_profit_precision_errors_are_exposed() {
        for (quantity, average, current, error) in [
            (
                "2",
                "79228162514264337593543950335",
                "1",
                MoneyError::Overflow,
            ),
            (
                "0.0000000000000000000000000001",
                "1.1",
                "1",
                MoneyError::PrecisionLoss,
            ),
            (
                "1",
                "0.1",
                "79228162514264337593543950335",
                MoneyError::PrecisionLoss,
            ),
        ] {
            let item = holding("A", Currency::Krw, quantity, average, Some(current));
            assert_eq!(
                item.evaluate().computed(),
                Err(EvaluationError::Arithmetic(error))
            );
        }
    }
    #[test]
    fn complete_and_incomplete_payload_accessors_preserve_their_values() {
        let mut total = Total::new(money("5", Currency::Krw));
        let mut observed = Vec::new();
        for _ in 0_u32..2 {
            match &total {
                Total::Complete(value) => observed.push((total.completeness(), *value.value())),
                Total::Incomplete(value) => {
                    observed.push((total.completeness(), *value.subtotal()));
                }
            }
            total.exclude();
        }
        assert_eq!(
            observed,
            vec![
                (
                    crate::TotalCompleteness::Complete,
                    money("5", Currency::Krw)
                ),
                (
                    crate::TotalCompleteness::Incomplete,
                    money("5", Currency::Krw)
                )
            ]
        );
    }
    #[test]
    fn reported_zero_valuation_is_valid_for_zero_quantity() {
        for currency in [Currency::Krw, Currency::Usd] {
            let report = ReportedHolding::new(
                money("0", currency),
                money("0", currency),
                Ratio::parse(&input("0")).expect("비율"),
            )
            .expect("토스 평가 0");
            let original = holding("A", currency, "0", "0", None);
            let item = Holding::new(
                original.instrument().clone(),
                original.quantity(),
                original.acquisition_price(),
                HoldingObservation::new(None, Some(report), time()),
            )
            .expect("0주");
            assert_eq!(item.evaluate().selected(), Ok(money("0", currency)));
            assert_eq!(
                item.evaluate().computed().expect("평가").return_rate(),
                Ok(HoldingReturn::ZeroCost)
            );
        }
    }
}
