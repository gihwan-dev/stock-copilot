use crate::{
    AllocationInstrument, AllocationSector, Amount, Currency, FxRate, Market, Money, MoneyError,
    PortfolioValuation, ProfileWeight, RatioQuotient, SectorAssignments, Total, TotalCompleteness,
    ValuationAccuracy,
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationCurrency {
    Krw,
    Usd,
}
impl AllocationCurrency {
    const fn new(currency: Currency) -> Self {
        match currency {
            Currency::Krw => Self::Krw,
            Currency::Usd => Self::Usd,
        }
    }
    #[must_use]
    pub const fn as_currency(self) -> Currency {
        match self {
            Self::Krw => Currency::Krw,
            Self::Usd => Currency::Usd,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetInstrument {
    Holding(AllocationInstrument),
    Cash(AllocationCurrency),
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetCountry {
    Market(Market),
    Cash,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    Incomplete,
    ZeroTotal,
    NegativeValue,
    NegativeCash(Currency),
    TotalMismatch,
    MissingFx,
    Arithmetic(MoneyError),
    Conversion(crate::FxConversionError),
}
#[derive(Debug, Clone, Copy, Eq)]
struct AllocationValue {
    quotient: RatioQuotient,
    accuracy: ValuationAccuracy,
    concentration: crate::concentration_arithmetic::CompositionInput,
}
impl PartialEq for AllocationValue {
    fn eq(&self, other: &Self) -> bool {
        self.quotient == other.quotient && self.accuracy == other.accuracy
    }
}
impl AllocationValue {
    fn amount(self) -> Amount {
        match self.quotient {
            RatioQuotient::Exact(value) => Amount::new(value.as_decimal()),
            RatioQuotient::Approximate(value) => Amount::new(value.as_decimal()),
        }
    }
    fn compare(self, other: Self) -> Ordering {
        self.amount().as_decimal().cmp(&other.amount().as_decimal())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetWeight(AllocationValue);
impl AssetWeight {
    #[must_use]
    pub fn cmp_profile_weight(&self, limit: &ProfileWeight) -> Ordering {
        self.0
            .amount()
            .as_decimal()
            .cmp(&limit.as_ratio().as_decimal())
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.compare(other.0)
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.0.accuracy
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositionWeight(AllocationValue);
impl CompositionWeight {
    pub(crate) const fn concentration_statistics_input(
        &self,
    ) -> crate::concentration_arithmetic::CompositionInput {
        self.0.concentration
    }

    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.compare(other.0)
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.0.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AllocationDisplayUnits(u32);
impl AllocationDisplayUnits {
    #[must_use]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationDisplayRange {
    Zero,
    BelowUnit,
    AtLeastUnit,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationDisplay {
    units: AllocationDisplayUnits,
    range: AllocationDisplayRange,
}
impl AllocationDisplay {
    #[must_use]
    pub const fn units(&self) -> &AllocationDisplayUnits {
        &self.units
    }
    #[must_use]
    pub const fn range(&self) -> AllocationDisplayRange {
        self.range
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationEntry<K, W> {
    key: K,
    weight: W,
    display: AllocationDisplay,
}
impl<K, W> AllocationEntry<K, W> {
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }
    #[must_use]
    pub const fn weight(&self) -> &W {
        &self.weight
    }
    #[must_use]
    pub const fn display(&self) -> &AllocationDisplay {
        &self.display
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Weights<K, W> {
    entries: Vec<AllocationEntry<K, W>>,
    accuracy: ValuationAccuracy,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetAllocation<K>(Weights<K, AssetWeight>);
impl<K> AssetAllocation<K> {
    #[must_use]
    pub fn entries(&self) -> &[AllocationEntry<K, AssetWeight>] {
        &self.0.entries
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.0.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingsComposition<K>(Weights<K, CompositionWeight>);
impl<K> HoldingsComposition<K> {
    #[must_use]
    pub fn entries(&self) -> &[AllocationEntry<K, CompositionWeight>] {
        &self.0.entries
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.0.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetAllocations {
    instruments: Result<AssetAllocation<AssetInstrument>, AllocationError>,
    sectors: Result<AssetAllocation<AllocationSector>, AllocationError>,
    countries: Result<AssetAllocation<AssetCountry>, AllocationError>,
    currencies: Result<AssetAllocation<AllocationCurrency>, AllocationError>,
}
impl AssetAllocations {
    #[must_use]
    pub const fn instruments(&self) -> &Result<AssetAllocation<AssetInstrument>, AllocationError> {
        &self.instruments
    }
    #[must_use]
    pub const fn sectors(&self) -> &Result<AssetAllocation<AllocationSector>, AllocationError> {
        &self.sectors
    }
    #[must_use]
    pub const fn countries(&self) -> &Result<AssetAllocation<AssetCountry>, AllocationError> {
        &self.countries
    }
    #[must_use]
    pub const fn currencies(
        &self,
    ) -> &Result<AssetAllocation<AllocationCurrency>, AllocationError> {
        &self.currencies
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingsAllocations {
    instruments: Result<HoldingsComposition<AllocationInstrument>, AllocationError>,
    sectors: Result<HoldingsComposition<AllocationSector>, AllocationError>,
    countries: Result<HoldingsComposition<Market>, AllocationError>,
    currencies: Result<HoldingsComposition<AllocationCurrency>, AllocationError>,
}
impl HoldingsAllocations {
    #[must_use]
    pub const fn instruments(
        &self,
    ) -> &Result<HoldingsComposition<AllocationInstrument>, AllocationError> {
        &self.instruments
    }
    #[must_use]
    pub const fn sectors(&self) -> &Result<HoldingsComposition<AllocationSector>, AllocationError> {
        &self.sectors
    }
    #[must_use]
    pub const fn countries(&self) -> &Result<HoldingsComposition<Market>, AllocationError> {
        &self.countries
    }
    #[must_use]
    pub const fn currencies(
        &self,
    ) -> &Result<HoldingsComposition<AllocationCurrency>, AllocationError> {
        &self.currencies
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    valuation: PortfolioValuation,
    assets: AssetAllocations,
    holdings: HoldingsAllocations,
}
impl Allocation {
    #[must_use]
    pub fn new(valuation: &PortfolioValuation, assignments: &SectorAssignments) -> Self {
        let rates = match valuation.assets() {
            Total::Complete(value) => value.value().clone(),
            Total::Incomplete(value) => value.subtotal().clone(),
        }
        .into_parts()
        .1;
        let holdings = valued_holdings(valuation, &rates);
        Self {
            valuation: valuation.clone(),
            assets: build_assets(
                holdings
                    .clone()
                    .and_then(|values| asset_values(values, valuation, &rates)),
                assignments,
            ),
            holdings: build_holdings(holdings, assignments),
        }
    }
    #[must_use]
    pub const fn valuation(&self) -> &PortfolioValuation {
        &self.valuation
    }
    #[must_use]
    pub const fn assets(&self) -> &AssetAllocations {
        &self.assets
    }
    #[must_use]
    pub const fn holdings(&self) -> &HoldingsAllocations {
        &self.holdings
    }
}
#[derive(Debug, Clone)]
struct HoldingValue {
    instrument: AllocationInstrument,
    value: Money,
    currency: AllocationCurrency,
}
#[derive(Debug, Clone)]
struct AssetValues {
    holdings: Vec<HoldingValue>,
    cash: Vec<(AllocationCurrency, Money)>,
}
fn converted(value: Money, rates: &[FxRate]) -> Result<Money, AllocationError> {
    if value.currency() == Currency::Krw {
        return Ok(value);
    }
    if value.amount().as_decimal().is_zero() {
        return Ok(crate::holding::zero(Currency::Krw));
    }
    rates
        .first()
        .ok_or(AllocationError::MissingFx)?
        .convert_for_aggregation(value)
        .map(|converted| converted.into_parts().0)
        .map_err(AllocationError::Conversion)
}
fn valued_holdings(
    valuation: &PortfolioValuation,
    rates: &[FxRate],
) -> Result<Vec<HoldingValue>, AllocationError> {
    if valuation.valuation().completeness() == TotalCompleteness::Incomplete {
        return Err(AllocationError::Incomplete);
    }
    valuation
        .holdings()
        .iter()
        .map(|row| {
            let money = row
                .valuation()
                .selected()
                .map_err(|_error| AllocationError::Incomplete)?;
            let value = converted(money, rates)?;
            row.conversion()
                .map_err(|_error| AllocationError::Incomplete)?;
            Ok(HoldingValue {
                instrument: AllocationInstrument::new(row.instrument()),
                value,
                currency: AllocationCurrency::new(row.holding().currency()),
            })
        })
        .collect()
}
fn asset_values(
    holdings: Vec<HoldingValue>,
    valuation: &PortfolioValuation,
    rates: &[FxRate],
) -> Result<AssetValues, AllocationError> {
    let mut cash = Vec::new();
    for currency in [Currency::Krw, Currency::Usd] {
        let value = valuation
            .buying_power(currency)
            .money()
            .map_err(|_error| AllocationError::NegativeCash(currency))?;
        cash.push((AllocationCurrency::new(currency), converted(value, rates)?));
    }
    let expected = match valuation.assets() {
        Total::Complete(value) => value.value().clone().into_parts().0,
        Total::Incomplete(_) => return Err(AllocationError::Incomplete),
    };
    let mut values: Vec<_> = holdings
        .iter()
        .map(|row| row.value)
        .chain(cash.iter().map(|(_, value)| *value))
        .collect();
    values.sort_by_key(|value| value.amount().as_decimal());
    let total = values
        .iter()
        .try_fold(crate::holding::zero(Currency::Krw), |sum, value| {
            sum.checked_add(value)
        })
        .map_err(AllocationError::Arithmetic)?;
    if total != expected {
        return Err(AllocationError::TotalMismatch);
    }
    Ok(AssetValues { holdings, cash })
}
fn build_holdings(
    values: Result<Vec<HoldingValue>, AllocationError>,
    assignments: &SectorAssignments,
) -> HoldingsAllocations {
    HoldingsAllocations {
        instruments: values.clone().and_then(|rows| {
            composition_weights(
                rows.into_iter()
                    .map(|row| (row.instrument, row.value))
                    .collect(),
            )
        }),
        sectors: values.clone().and_then(|rows| {
            composition_weights(
                rows.into_iter()
                    .map(|row| (assignments.sector(&row.instrument), row.value))
                    .collect(),
            )
        }),
        countries: values.clone().and_then(|rows| {
            composition_weights(
                rows.into_iter()
                    .map(|row| (row.instrument.market().clone(), row.value))
                    .collect(),
            )
        }),
        currencies: values.and_then(|rows| {
            composition_weights(
                rows.into_iter()
                    .map(|row| (row.currency, row.value))
                    .collect(),
            )
        }),
    }
}
fn build_assets(
    values: Result<AssetValues, AllocationError>,
    assignments: &SectorAssignments,
) -> AssetAllocations {
    AssetAllocations {
        instruments: values.clone().and_then(|rows| {
            asset_weights(
                rows.holdings
                    .into_iter()
                    .map(|row| (AssetInstrument::Holding(row.instrument), row.value))
                    .chain(
                        rows.cash
                            .into_iter()
                            .map(|(currency, value)| (AssetInstrument::Cash(currency), value)),
                    )
                    .collect(),
            )
        }),
        sectors: values.clone().and_then(|rows| {
            asset_weights(
                rows.holdings
                    .into_iter()
                    .map(|row| (assignments.sector(&row.instrument), row.value))
                    .chain(
                        rows.cash
                            .into_iter()
                            .map(|(_, value)| (AllocationSector::Cash, value)),
                    )
                    .collect(),
            )
        }),
        countries: values.clone().and_then(|rows| {
            asset_weights(
                rows.holdings
                    .into_iter()
                    .map(|row| {
                        (
                            AssetCountry::Market(row.instrument.market().clone()),
                            row.value,
                        )
                    })
                    .chain(
                        rows.cash
                            .into_iter()
                            .map(|(_, value)| (AssetCountry::Cash, value)),
                    )
                    .collect(),
            )
        }),
        currencies: values.and_then(|rows| {
            asset_weights(
                rows.holdings
                    .into_iter()
                    .map(|row| (row.currency, row.value))
                    .chain(rows.cash)
                    .collect(),
            )
        }),
    }
}
fn asset_weights<K: Ord + Clone>(
    values: Vec<(K, Money)>,
) -> Result<AssetAllocation<K>, AllocationError> {
    weights(values, AssetWeight).map(AssetAllocation)
}
fn composition_weights<K: Ord + Clone>(
    values: Vec<(K, Money)>,
) -> Result<HoldingsComposition<K>, AllocationError> {
    weights(values, CompositionWeight).map(HoldingsComposition)
}
fn weights<K: Ord + Clone, W>(
    values: Vec<(K, Money)>,
    make_weight: fn(AllocationValue) -> W,
) -> Result<Weights<K, W>, AllocationError> {
    let (total, grouped) = grouped_values(values)?;
    if grouped.is_empty() {
        return Err(AllocationError::ZeroTotal);
    }
    let mut entries = Vec::new();
    let mut remainders = Vec::new();
    let mut remaining = 10000_u32;
    for (key, value) in grouped {
        let weight = make_weight(calculated_weight(value, total)?);
        let (units, remainder) =
            crate::exact_arithmetic::allocation_parts(value.amount(), total.amount());
        let range = if value.amount().as_decimal().is_zero() {
            AllocationDisplayRange::Zero
        } else if units.as_units() == 0 {
            AllocationDisplayRange::BelowUnit
        } else {
            AllocationDisplayRange::AtLeastUnit
        };
        remaining = remaining.saturating_sub(units.as_units());
        remainders.push((key.clone(), remainder));
        entries.push(AllocationEntry {
            key,
            weight,
            display: AllocationDisplay {
                units: AllocationDisplayUnits(units.as_units()),
                range,
            },
        });
    }
    remainders.sort_by(|(left_key, left_remainder), (right_key, right_remainder)| {
        right_remainder
            .cmp(left_remainder)
            .then_with(|| left_key.cmp(right_key))
    });
    let winners: BTreeSet<_> = remainders
        .into_iter()
        .zip(0_u32..)
        .take_while(|(_, index)| *index < remaining)
        .map(|((key, _), _)| key)
        .collect();
    for entry in &mut entries {
        if winners.contains(&entry.key) {
            entry.display.units.0 = entry.display.units.0.saturating_add(1);
        }
    }
    Ok(Weights {
        entries,
        accuracy: ValuationAccuracy::Approximate,
    })
}
fn grouped_values<K: Ord>(
    mut values: Vec<(K, Money)>,
) -> Result<(Money, BTreeMap<K, Money>), AllocationError> {
    values.sort_by(|(left_key, left_value), (right_key, right_value)| {
        left_key.cmp(right_key).then_with(|| {
            left_value
                .amount()
                .as_decimal()
                .cmp(&right_value.amount().as_decimal())
        })
    });
    let mut grouped = BTreeMap::new();
    let mut total = crate::holding::zero(Currency::Krw);
    for (key, value) in values {
        if value.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            return Err(AllocationError::NegativeValue);
        }
        total = total
            .checked_add(&value)
            .map_err(AllocationError::Arithmetic)?;
        let current = grouped
            .entry(key)
            .or_insert_with(|| crate::holding::zero(Currency::Krw));
        *current = current
            .checked_add(&value)
            .map_err(AllocationError::Arithmetic)?;
    }
    Ok((total, grouped))
}
fn calculated_weight(value: Money, total: Money) -> Result<AllocationValue, AllocationError> {
    value
        .checked_ratio(&total)
        .map(|quotient| AllocationValue {
            concentration: crate::concentration_arithmetic::CompositionInput::from_evaluation(
                value, total, quotient,
            ),
            quotient,
            accuracy: ValuationAccuracy::Approximate,
        })
        .map_err(|error| match error {
            MoneyError::DivisionByZero => AllocationError::ZeroTotal,
            other @ (MoneyError::CurrencyMismatch
            | MoneyError::Overflow
            | MoneyError::PrecisionLoss) => AllocationError::Arithmetic(other),
        })
}

pub(crate) fn policy_krw_holding_values(
    valuation: &PortfolioValuation,
    rates: &[FxRate],
) -> Result<Vec<(AllocationInstrument, Money)>, AllocationError> {
    valued_holdings(valuation, rates).map(|rows| {
        rows.into_iter()
            .map(|row| (row.instrument, row.value))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{asset_weights, calculated_weight, composition_weights, converted};
    use crate::{
        AcquisitionPrice, Allocation, AllocationCurrency, AllocationDisplayRange, AllocationEntry,
        AllocationError, AllocationInstrument, AllocationSector, AssetCountry, AssetInstrument,
        BuyingPower, Currency, CurrencyAmounts, ExclusionDecision, Exclusions, FxPair, FxRate,
        FxRateKind, FxSource, Holding, HoldingObservation, InstrumentClassification, Market, Money,
        MoneyError, Nanosecond, NumericInput, OffsetSeconds, Portfolio, PortfolioValuation,
        ProfileWeight, Quantity, Ratio, ReportedHolding, ReportedPortfolio,
        ResolvedOrderInstrument, SectorAssignment, SectorAssignments, SectorAssignmentsError,
        SectorId, Symbol, TimeInterval, Timestamp, UnixSeconds, ValuationAccuracy,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;
    use std::{cmp::Ordering, collections::BTreeMap, fmt::Debug};

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
    fn holding(symbol: &str, value: Option<&str>, currency: Currency) -> Holding {
        let instrument = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("심볼"),
            match currency {
                Currency::Krw => Market::Kr,
                Currency::Usd => Market::Us,
            },
        )
        .expect("종목");
        let report = value.map(|text| {
            ReportedHolding::new(
                money(text, currency),
                money("0", currency),
                Ratio::parse(&input("0")).expect("율"),
            )
            .expect("보고 평가")
        });
        Holding::new(
            instrument,
            Quantity::parse(&input("1")).expect("수량"),
            AcquisitionPrice::try_new(money("0", currency)).expect("원가"),
            HoldingObservation::new(None, report, time()),
        )
        .expect("보유")
    }
    fn rate(value: &str) -> FxRate {
        FxRate::parse(
            &input(value),
            FxPair::UsdKrw,
            FxRateKind::Mid,
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
    fn portfolio(
        items: Vec<Holding>,
        krw: &str,
        usd: &str,
        fx: Option<FxRate>,
    ) -> PortfolioValuation {
        Portfolio::new(
            items,
            BuyingPower::new(money(krw, Currency::Krw), time()),
            BuyingPower::new(money(usd, Currency::Usd), time()),
        )
        .expect("포트폴리오")
        .evaluate(fx)
    }
    fn reported(items: Vec<Holding>, cash: &str, summary: &str) -> PortfolioValuation {
        let amounts = CurrencyAmounts::new(money(summary, Currency::Krw), None).expect("요약");
        Portfolio::new(
            items,
            BuyingPower::new(money(cash, Currency::Krw), time()),
            BuyingPower::new(money("0", Currency::Usd), time()),
        )
        .expect("원본")
        .with_reported(ReportedPortfolio::new(amounts, amounts, amounts, time()))
        .evaluate(None)
    }
    fn units<K, W>(rows: &[AllocationEntry<K, W>]) -> Vec<u32> {
        rows.iter()
            .map(|row| row.display().units().as_u32())
            .collect()
    }
    #[test]
    fn asset_policy_denominator_and_holdings_composition_are_separate() {
        let valuation = portfolio(
            vec![
                holding("KR", Some("1"), Currency::Krw),
                holding("US", Some("1"), Currency::Usd),
            ],
            "1000000",
            "0",
            Some(rate("1375")),
        );
        let allocation = Allocation::new(&valuation, &SectorAssignments::default());
        assert_eq!(allocation.valuation(), &valuation);
        let assets = allocation.assets().instruments().as_ref().expect("총자산");
        assert_eq!(units(assets.entries()), vec![0, 14, 9986, 0]);
        let composition = allocation
            .holdings()
            .instruments()
            .as_ref()
            .expect("구성비");
        assert_eq!(units(composition.entries()), vec![7, 9993]);
        let cap = ProfileWeight::parse(&input("0.2")).expect("상한");
        let krw = assets.entries().first().expect("원화");
        let usd = assets.entries().get(1).expect("달러");
        assert_eq!(usd.weight().cmp_profile_weight(&cap), Ordering::Less);
        assert_eq!(krw.weight().compare(usd.weight()), Ordering::Less);
        assert_eq!(
            composition
                .entries()
                .first()
                .expect("첫 구성")
                .weight()
                .compare(composition.entries().get(1).expect("둘째 구성").weight()),
            Ordering::Less
        );
        assert_eq!(assets.accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(composition.accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(usd.weight().accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(
            composition
                .entries()
                .first()
                .expect("첫 구성")
                .weight()
                .accuracy(),
            ValuationAccuracy::Approximate
        );
        assert_eq!(krw.display().range(), AllocationDisplayRange::BelowUnit);
        assert_eq!(
            assets
                .entries()
                .last()
                .expect("빈 달러 현금")
                .display()
                .range(),
            AllocationDisplayRange::Zero
        );
        assert_currency_keys(&allocation);
    }
    fn assert_currency_keys(allocation: &Allocation) {
        assert_eq!(
            allocation
                .assets()
                .currencies()
                .as_ref()
                .expect("통화")
                .entries()
                .iter()
                .map(|row| row.key().as_currency())
                .collect::<Vec<_>>(),
            vec![Currency::Krw, Currency::Usd]
        );
    }
    #[test]
    fn finite_quotients_from_fx_remain_approximate_and_compare_with_caps() {
        let valuation = portfolio(
            vec![
                holding("KR", Some("1375"), Currency::Krw),
                holding("US", Some("1"), Currency::Usd),
            ],
            "0",
            "0",
            Some(rate("1375")),
        );
        let allocation = Allocation::new(&valuation, &SectorAssignments::default());
        let rows = allocation
            .assets()
            .instruments()
            .as_ref()
            .expect("자산")
            .entries();
        assert_eq!(
            rows.first()
                .expect("원화")
                .weight()
                .cmp_profile_weight(&ProfileWeight::parse(&input("0.5")).expect("상한")),
            Ordering::Equal
        );
        assert_eq!(
            rows.first()
                .expect("원화")
                .weight()
                .compare(rows.get(1).expect("달러").weight()),
            Ordering::Equal
        );
        assert_eq!(
            rows.first()
                .expect("원화")
                .weight()
                .cmp_profile_weight(&ProfileWeight::parse(&input("0.2")).expect("상한")),
            Ordering::Greater
        );
        assert_eq!(
            rows.first().expect("원화").weight().accuracy(),
            ValuationAccuracy::Approximate
        );
    }
    #[test]
    fn sectors_include_etf_cash_and_unclassified_with_validated_assignments() {
        let items = vec![
            holding("A", Some("1"), Currency::Krw),
            holding("B", Some("1"), Currency::Krw),
            holding("C", Some("1"), Currency::Krw),
            holding("D", Some("1"), Currency::Krw),
        ];
        let sector = SectorId::try_new("Tech").expect("섹터");
        let first = SectorAssignment::new(
            items.first().expect("종목").instrument(),
            InstrumentClassification::Sector(sector.clone()),
        );
        assert_eq!(
            first.classification(),
            &InstrumentClassification::Sector(sector.clone())
        );
        assert_eq!(
            first.instrument().symbol(),
            items.first().expect("종목").instrument().symbol()
        );
        assert_eq!(first.instrument().market(), &Market::Kr);
        assert_eq!(
            SectorAssignments::try_new(vec![first.clone(), first.clone()]),
            Err(SectorAssignmentsError::DuplicateInstrument)
        );
        let assignments = SectorAssignments::try_new(vec![
            first,
            SectorAssignment::new(
                items.get(1).expect("ETF").instrument(),
                InstrumentClassification::EtfFund,
            ),
            SectorAssignment::new(
                items.get(2).expect("미분류").instrument(),
                InstrumentClassification::Unclassified,
            ),
        ])
        .expect("대응");
        let valuation = portfolio(items, "4", "0", None);
        let allocation = Allocation::new(&valuation, &assignments);
        let rows = allocation
            .assets()
            .sectors()
            .as_ref()
            .expect("자산 섹터")
            .entries();
        assert_eq!(
            rows.iter().map(|row| row.key().clone()).collect::<Vec<_>>(),
            vec![
                AllocationSector::Classified(sector),
                AllocationSector::EtfFund,
                AllocationSector::Cash,
                AllocationSector::Unclassified
            ]
        );
        assert_eq!(units(rows), vec![1250, 1250, 5000, 2500]);
    }
    #[test]
    fn sector_identity_and_unheld_duplicates_are_validated_at_the_boundary() {
        let sector = SectorId::try_new("Tech").expect("섹터");
        let exclusions = Exclusions::try_new(vec![sector.clone()], vec![]).expect("제외");
        assert_eq!(
            exclusions.sector_decision(&sector),
            ExclusionDecision::Excluded
        );
        assert_eq!(
            exclusions.sector_decision(&SectorId::try_new("tech").expect("소문자")),
            ExclusionDecision::Included
        );
        let absent = holding("ABSENT", Some("1"), Currency::Krw);
        let duplicate =
            SectorAssignment::new(absent.instrument(), InstrumentClassification::EtfFund);
        assert_eq!(
            SectorAssignments::try_new(vec![duplicate.clone(), duplicate]),
            Err(SectorAssignmentsError::DuplicateInstrument)
        );
    }
    #[test]
    fn zero_cash_only_and_negative_cash_have_explicit_denominator_results() {
        let assignments = SectorAssignments::default();
        for currency in [Currency::Krw, Currency::Usd] {
            let (krw, usd) = match currency {
                Currency::Krw => ("-1", "0"),
                Currency::Usd => ("0", "-1"),
            };
            let valuation = portfolio(
                vec![holding("A", Some("1"), Currency::Krw)],
                krw,
                usd,
                Some(rate("1375")),
            );
            let allocation = Allocation::new(&valuation, &assignments);
            assert_eq!(
                allocation.assets().currencies(),
                &Err(AllocationError::NegativeCash(currency))
            );
            assert!(allocation.holdings().currencies().is_ok());
        }
    }
    #[test]
    fn cash_only_empty_and_zero_holdings_have_explicit_results() {
        let assignments = SectorAssignments::default();
        let cash = Allocation::new(
            &portfolio(vec![], "1", "1", Some(rate("1375"))),
            &assignments,
        );
        assert_eq!(
            cash.holdings().instruments(),
            &Err(AllocationError::ZeroTotal)
        );
        assert_eq!(
            units(cash.assets().sectors().as_ref().expect("현금").entries()),
            vec![10000]
        );
        assert_eq!(
            units(
                cash.assets()
                    .countries()
                    .as_ref()
                    .expect("현금 국가")
                    .entries()
            ),
            vec![10000]
        );
        assert_eq!(
            cash.assets()
                .countries()
                .as_ref()
                .expect("국가")
                .entries()
                .first()
                .expect("현금")
                .key(),
            &AssetCountry::Cash
        );
        let zero = Allocation::new(
            &portfolio(vec![holding("Z", Some("0"), Currency::Usd)], "0", "0", None),
            &assignments,
        );
        assert_eq!(
            zero.assets().instruments(),
            &Err(AllocationError::ZeroTotal)
        );
        assert_eq!(
            zero.holdings().instruments(),
            &Err(AllocationError::ZeroTotal)
        );
        assert_eq!(
            Allocation::new(&portfolio(vec![], "0", "0", None), &assignments)
                .holdings()
                .instruments(),
            &Err(AllocationError::ZeroTotal)
        );
    }
    #[test]
    fn incomplete_valuation_and_summary_mismatch_never_leak_asset_weights() {
        let assignments = SectorAssignments::default();
        for valuation in [
            portfolio(vec![holding("A", None, Currency::Krw)], "0", "0", None),
            reported(vec![holding("A", None, Currency::Krw)], "0", "1"),
        ] {
            let allocation = Allocation::new(&valuation, &assignments);
            assert_eq!(
                allocation.assets().instruments(),
                &Err(AllocationError::Incomplete)
            );
            assert_eq!(
                allocation.holdings().instruments(),
                &Err(AllocationError::Incomplete)
            );
        }
        let mismatch = Allocation::new(
            &reported(vec![holding("A", Some("1"), Currency::Krw)], "0", "20"),
            &assignments,
        );
        assert_eq!(
            mismatch.assets().instruments(),
            &Err(AllocationError::TotalMismatch)
        );
        assert!(mismatch.holdings().instruments().is_ok());
        let maximum = "79228162514264337593543950335";
        let assets = Allocation::new(&reported(vec![], "1", maximum), &assignments);
        assert_eq!(
            assets.assets().instruments(),
            &Err(AllocationError::Incomplete)
        );
        let overflow = Allocation::new(
            &reported(vec![holding("A", Some(maximum), Currency::Krw)], "1", "1"),
            &assignments,
        );
        assert_eq!(
            overflow.assets().instruments(),
            &Err(AllocationError::Arithmetic(MoneyError::Overflow))
        );
        let rows = vec![
            holding("A", Some(maximum), Currency::Krw),
            holding("B", Some("1"), Currency::Krw),
        ];
        assert_eq!(
            Allocation::new(&reported(rows, "0", "1"), &assignments)
                .holdings()
                .instruments(),
            &Err(AllocationError::Incomplete)
        );
    }
    #[test]
    fn missing_fx_in_incomplete_total_is_not_a_partial_composition() {
        let assignments = SectorAssignments::default();
        let missing_fx = Allocation::new(
            &portfolio(
                vec![holding("US", Some("1"), Currency::Usd)],
                "0",
                "0",
                None,
            ),
            &assignments,
        );
        assert_eq!(
            missing_fx.holdings().instruments(),
            &Err(AllocationError::Incomplete)
        );
    }
    #[test]
    fn fx_absence_conversion_precision_and_reported_failures_remain_visible() {
        let assignments = SectorAssignments::default();
        let missing = Allocation::new(&portfolio(vec![], "0", "1", None), &assignments);
        assert_eq!(
            missing.assets().currencies(),
            &Err(AllocationError::MissingFx)
        );
        let foreign = Allocation::new(
            &reported(vec![holding("US", Some("1"), Currency::Usd)], "0", "1"),
            &assignments,
        );
        assert_eq!(
            foreign.holdings().instruments(),
            &Err(AllocationError::MissingFx)
        );
        let precision = Allocation::new(
            &portfolio(
                vec![holding("US", Some("1"), Currency::Usd)],
                "0",
                "0.0000000000000000000000000001",
                Some(rate("0.0000000000000000000000000001")),
            ),
            &assignments,
        );
        assert_eq!(
            precision.assets().currencies(),
            &Err(AllocationError::Conversion(
                crate::FxConversionError::Arithmetic(MoneyError::PrecisionLoss)
            ))
        );
        assert_eq!(
            converted(money("1", Currency::Usd), &[]),
            Err(AllocationError::MissingFx)
        );
    }
    #[test]
    fn display_ranges_preserve_positive_tiny_values_and_exact_unit_sum() {
        let valuation = portfolio(
            vec![
                holding("A", Some("0.0000000000000000000000000001"), Currency::Krw),
                holding("B", Some("1"), Currency::Krw),
            ],
            "0",
            "0",
            None,
        );
        let allocation = Allocation::new(&valuation, &SectorAssignments::default());
        let rows = allocation
            .holdings()
            .instruments()
            .as_ref()
            .expect("구성")
            .entries();
        assert_eq!(units(rows), vec![0, 10000]);
        assert_eq!(
            rows.first().expect("미소 비중").display().range(),
            AllocationDisplayRange::BelowUnit
        );
        assert_eq!(
            rows.get(1).expect("큰 비중").display().range(),
            AllocationDisplayRange::AtLeastUnit
        );
        let equal = Allocation::new(
            &portfolio(
                vec![
                    holding("C", Some("1"), Currency::Krw),
                    holding("A", Some("1"), Currency::Krw),
                    holding("B", Some("1"), Currency::Krw),
                ],
                "0",
                "0",
                None,
            ),
            &SectorAssignments::default(),
        );
        assert_eq!(
            units(
                equal
                    .holdings()
                    .instruments()
                    .as_ref()
                    .expect("동률")
                    .entries()
            ),
            vec![3334, 3333, 3333]
        );
        let empty = Allocation::new(
            &portfolio(vec![], "0", "0", None),
            &SectorAssignments::default(),
        );
        assert_public_axes(&empty, &[], 0, 0);
        let total = money("3", Currency::Krw).amount();
        let first =
            crate::exact_arithmetic::allocation_parts(money("1", Currency::Krw).amount(), total).1;
        let second =
            crate::exact_arithmetic::allocation_parts(money("2", Currency::Krw).amount(), total).1;
        assert_eq!(first.partial_cmp(&second), Some(Ordering::Less));
    }
    #[test]
    fn extreme_and_many_small_holdings_keep_allocation_contracts() {
        for value in [
            "79228162514264337593543950335",
            "0.0000000000000000000000000001",
        ] {
            let allocation = Allocation::new(
                &portfolio(
                    vec![holding("A", Some(value), Currency::Krw)],
                    "0",
                    "0",
                    None,
                ),
                &SectorAssignments::default(),
            );
            assert_eq!(
                units(
                    allocation
                        .holdings()
                        .instruments()
                        .as_ref()
                        .expect("극값")
                        .entries()
                ),
                vec![10000]
            );
            assert_eq!(
                units(
                    allocation
                        .assets()
                        .instruments()
                        .as_ref()
                        .expect("자산 극값")
                        .entries()
                ),
                vec![10000, 0, 0]
            );
        }
        let items = (0_u32..4096)
            .map(|index| {
                holding(
                    &format!("S{index:04}"),
                    Some("0.0000000000000000000000000001"),
                    Currency::Krw,
                )
            })
            .collect();
        let allocation = Allocation::new(
            &portfolio(items, "0", "0", None),
            &SectorAssignments::default(),
        );
        let rows = allocation
            .holdings()
            .instruments()
            .as_ref()
            .expect("미소 비중 다수")
            .entries();
        assert_eq!(units(rows).iter().sum::<u32>(), 10000);
        assert_eq!(rows.first().expect("첫 행").display().units().as_u32(), 3);
        assert_eq!(rows.last().expect("끝 행").display().units().as_u32(), 2);
    }
    #[test]
    fn arithmetic_validation_rejects_negative_currency_overflow_and_precision_loss() {
        assert_eq!(
            composition_weights(vec![(0_u32, money("-1", Currency::Krw))]),
            Err(AllocationError::NegativeValue)
        );
        assert_eq!(
            asset_weights(vec![(0_u32, money("1", Currency::Usd))]),
            Err(AllocationError::Arithmetic(MoneyError::CurrencyMismatch))
        );
        assert_eq!(
            calculated_weight(money("1", Currency::Usd), money("1", Currency::Krw)),
            Err(AllocationError::Arithmetic(MoneyError::CurrencyMismatch))
        );
        assert_eq!(
            composition_weights(vec![
                (0_u32, money("0.4", Currency::Krw)),
                (1_u32, money("0.6", Currency::Krw)),
                (1_u32, money("10000000000000000000000000000", Currency::Krw))
            ]),
            Err(AllocationError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
    #[derive(Clone)]
    struct ReferenceHolding {
        instrument: AllocationInstrument,
        market: Market,
        currency: AllocationCurrency,
        sector: AllocationSector,
        coefficient: u128,
    }
    struct ReferenceAxes<I, C> {
        instruments: Vec<(I, u128)>,
        sectors: Vec<(AllocationSector, u128)>,
        countries: Vec<(C, u128)>,
        currencies: Vec<(AllocationCurrency, u128)>,
    }
    fn composition_reference(
        rows: &[ReferenceHolding],
    ) -> ReferenceAxes<AllocationInstrument, Market> {
        ReferenceAxes {
            instruments: rows
                .iter()
                .map(|row| (row.instrument.clone(), row.coefficient))
                .collect(),
            sectors: rows
                .iter()
                .map(|row| (row.sector.clone(), row.coefficient))
                .collect(),
            countries: rows
                .iter()
                .map(|row| (row.market.clone(), row.coefficient))
                .collect(),
            currencies: rows
                .iter()
                .map(|row| (row.currency, row.coefficient))
                .collect(),
        }
    }
    fn asset_reference(
        rows: &[ReferenceHolding],
        krw: u128,
        usd: u128,
    ) -> ReferenceAxes<AssetInstrument, AssetCountry> {
        let mut result = ReferenceAxes {
            instruments: rows
                .iter()
                .map(|row| {
                    (
                        AssetInstrument::Holding(row.instrument.clone()),
                        row.coefficient,
                    )
                })
                .collect(),
            sectors: rows
                .iter()
                .map(|row| (row.sector.clone(), row.coefficient))
                .collect(),
            countries: rows
                .iter()
                .map(|row| (AssetCountry::Market(row.market.clone()), row.coefficient))
                .collect(),
            currencies: rows
                .iter()
                .map(|row| (row.currency, row.coefficient))
                .collect(),
        };
        for (currency, value) in [
            (AllocationCurrency::Krw, krw),
            (AllocationCurrency::Usd, usd),
        ] {
            result
                .instruments
                .push((AssetInstrument::Cash(currency), value));
            result.sectors.push((AllocationSector::Cash, value));
            result.countries.push((AssetCountry::Cash, value));
            result.currencies.push((currency, value));
        }
        result
    }
    fn reference<K: Ord + Clone>(values: Vec<(K, u128)>) -> Vec<(K, u128, u128)> {
        let mut groups = BTreeMap::new();
        for (key, value) in values {
            let current = groups.entry(key).or_insert(0_u128);
            *current = current.checked_add(value).expect("독립 합");
        }
        let total: u128 = groups.values().sum();
        let mut rows: Vec<_> = groups
            .into_iter()
            .map(|(key, value)| {
                let numerator = value.checked_mul(10000).expect("독립 분자");
                (
                    key,
                    numerator.checked_div(total).expect("독립 몫"),
                    numerator.checked_rem(total).expect("독립 잔여"),
                )
            })
            .collect();
        let floor: u128 = rows.iter().map(|(_, value, _)| *value).sum();
        rows.sort_by(
            |(left_key, _, left_remainder), (right_key, _, right_remainder)| {
                right_remainder
                    .cmp(left_remainder)
                    .then_with(|| left_key.cmp(right_key))
            },
        );
        for (_, value, _) in rows.iter_mut().take(
            usize::try_from(10000_u128.checked_sub(floor).expect("독립 잔여 수")).expect("수"),
        ) {
            *value = value.checked_add(1).expect("독립 배분");
        }
        rows.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));
        rows
    }
    fn assert_rows<K: Ord + Clone + Debug, W>(
        actual: &[AllocationEntry<K, W>],
        expected: Vec<(K, u128)>,
    ) {
        let total: u128 = expected.iter().map(|(_, value)| *value).sum();
        let reference = reference(expected.clone());
        let mut coefficients = BTreeMap::new();
        for (key, value) in expected {
            let current = coefficients.entry(key).or_insert(0_u128);
            *current = current.checked_add(value).expect("독립 그룹 합");
        }
        assert_eq!(actual.len(), reference.len());
        assert_eq!(units(actual).iter().sum::<u32>(), 10000);
        for (row, (key, value, _)) in actual.iter().zip(reference) {
            assert_eq!(row.key(), &key);
            assert_eq!(u128::from(row.display().units().as_u32()), value);
            let numerator = coefficients
                .get(&key)
                .expect("원시 그룹")
                .checked_mul(10000)
                .expect("참값 분자");
            assert!(
                value
                    .checked_mul(total)
                    .expect("표시 분자")
                    .abs_diff(numerator)
                    < total
            );
            assert_eq!(
                row.display().range(),
                if numerator == 0 {
                    AllocationDisplayRange::Zero
                } else if numerator < total {
                    AllocationDisplayRange::BelowUnit
                } else {
                    AllocationDisplayRange::AtLeastUnit
                }
            );
        }
    }
    fn assert_public_axes(
        allocation: &Allocation,
        rows: &[ReferenceHolding],
        krw: u128,
        usd: u128,
    ) {
        assert_composition_axes(allocation, rows);
        assert_asset_axes(allocation, rows, krw, usd);
    }
    fn assert_composition_axes(allocation: &Allocation, rows: &[ReferenceHolding]) {
        let composition = composition_reference(rows);
        if rows.iter().map(|row| row.coefficient).sum::<u128>() == 0 {
            assert_eq!(
                allocation.holdings().instruments(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.holdings().sectors(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.holdings().countries(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.holdings().currencies(),
                &Err(AllocationError::ZeroTotal)
            );
        } else {
            assert_rows(
                allocation
                    .holdings()
                    .instruments()
                    .as_ref()
                    .expect("구성 종목")
                    .entries(),
                composition.instruments,
            );
            assert_rows(
                allocation
                    .holdings()
                    .sectors()
                    .as_ref()
                    .expect("구성 섹터")
                    .entries(),
                composition.sectors,
            );
            assert_rows(
                allocation
                    .holdings()
                    .countries()
                    .as_ref()
                    .expect("구성 국가")
                    .entries(),
                composition.countries,
            );
            assert_rows(
                allocation
                    .holdings()
                    .currencies()
                    .as_ref()
                    .expect("구성 통화")
                    .entries(),
                composition.currencies,
            );
        }
    }
    fn assert_asset_axes(allocation: &Allocation, rows: &[ReferenceHolding], krw: u128, usd: u128) {
        let assets = asset_reference(rows, krw, usd);
        let total: u128 = assets.instruments.iter().map(|(_, value)| *value).sum();
        if total == 0 {
            assert_eq!(
                allocation.assets().instruments(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.assets().sectors(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.assets().countries(),
                &Err(AllocationError::ZeroTotal)
            );
            assert_eq!(
                allocation.assets().currencies(),
                &Err(AllocationError::ZeroTotal)
            );
        } else {
            assert_rows(
                allocation
                    .assets()
                    .instruments()
                    .as_ref()
                    .expect("자산 종목")
                    .entries(),
                assets.instruments,
            );
            assert_rows(
                allocation
                    .assets()
                    .sectors()
                    .as_ref()
                    .expect("자산 섹터")
                    .entries(),
                assets.sectors,
            );
            assert_rows(
                allocation
                    .assets()
                    .countries()
                    .as_ref()
                    .expect("자산 국가")
                    .entries(),
                assets.countries,
            );
            assert_rows(
                allocation
                    .assets()
                    .currencies()
                    .as_ref()
                    .expect("자산 통화")
                    .entries(),
                assets.currencies,
            );
        }
    }
    fn generated_rows(
        samples: &[(u32, u32, u64, u8)],
    ) -> (Vec<Holding>, SectorAssignments, Vec<ReferenceHolding>) {
        let mut holdings = Vec::new();
        let mut assignments = Vec::new();
        let mut references = Vec::new();
        for (index, (coefficient, scale, _, market)) in samples.iter().enumerate() {
            let currency = if *market == 0 {
                Currency::Krw
            } else {
                Currency::Usd
            };
            let decimal_text =
                Decimal::from_i128_with_scale(i128::from(*coefficient), *scale).to_string();
            let item = holding(&format!("S{index:04}"), Some(&decimal_text), currency);
            let classification = match index.checked_rem(4).expect("네 분류") {
                0 => Some(InstrumentClassification::Sector(
                    SectorId::try_new("Tech").expect("섹터"),
                )),
                1 => Some(InstrumentClassification::EtfFund),
                2 => Some(InstrumentClassification::Unclassified),
                _ => None,
            };
            let sector = classification
                .as_ref()
                .map_or(AllocationSector::Unclassified, |value| match value {
                    InstrumentClassification::Sector(id) => {
                        AllocationSector::Classified(id.clone())
                    }
                    InstrumentClassification::EtfFund => AllocationSector::EtfFund,
                    InstrumentClassification::Unclassified => AllocationSector::Unclassified,
                });
            if let Some(value) = classification {
                assignments.push(SectorAssignment::new(item.instrument(), value));
            }
            let normalized = u128::from(*coefficient)
                .checked_mul(
                    10_u128
                        .checked_pow(6_u32.checked_sub(*scale).expect("스케일"))
                        .expect("스케일 계수"),
                )
                .expect("정수 정렬");
            let converted = normalized
                .checked_mul(if *market == 0 { 1 } else { 1375 })
                .expect("독립 환산");
            references.push(ReferenceHolding {
                instrument: AllocationInstrument::new(item.instrument()),
                market: item.instrument().market(),
                currency: if *market == 0 {
                    AllocationCurrency::Krw
                } else {
                    AllocationCurrency::Usd
                },
                sector,
                coefficient: converted,
            });
            holdings.push(item);
        }
        (
            holdings,
            SectorAssignments::try_new(assignments).expect("대응"),
            references,
        )
    }
    fn assert_generated_case(samples: &[(u32, u32, u64, u8)], krw: u32, usd: u32) {
        let (holdings, assignments, references) = generated_rows(samples);
        let original = Allocation::new(
            &portfolio(
                holdings.clone(),
                &krw.to_string(),
                &usd.to_string(),
                Some(rate("1375")),
            ),
            &assignments,
        );
        let mut shuffled: Vec<_> = holdings
            .into_iter()
            .zip(samples.iter().map(|(_, _, order, _)| *order))
            .collect();
        shuffled.sort_by_key(|(_, order)| *order);
        let permutation = Allocation::new(
            &portfolio(
                shuffled.into_iter().map(|(item, _)| item).collect(),
                &krw.to_string(),
                &usd.to_string(),
                Some(rate("1375")),
            ),
            &assignments,
        );
        assert_eq!(permutation, original);
        assert_public_axes(
            &original,
            &references,
            u128::from(krw).checked_mul(1_000_000).expect("원화 정렬"),
            u128::from(usd)
                .checked_mul(1_375_000_000)
                .expect("달러 환산"),
        );
    }
    fn assert_extreme_case(extreme: u8) {
        let value = if extreme == 0 {
            "79228162514264337593543950335"
        } else {
            "0.0000000000000000000000000001"
        };
        let allocation = Allocation::new(
            &portfolio(
                vec![holding("EXTREME", Some(value), Currency::Krw)],
                "0",
                "0",
                None,
            ),
            &SectorAssignments::default(),
        );
        assert_eq!(
            units(
                allocation
                    .assets()
                    .instruments()
                    .as_ref()
                    .expect("극값 자산")
                    .entries()
            ),
            vec![10000, 0, 0]
        );
        assert_eq!(
            units(
                allocation
                    .holdings()
                    .instruments()
                    .as_ref()
                    .expect("극값 구성")
                    .entries()
            ),
            vec![10000]
        );
    }
    #[test]
    fn generated_public_allocations_match_independent_integers_and_arbitrary_permutations() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    proptest::collection::vec(
                        (
                            0_u32..1000,
                            0_u32..=6,
                            proptest::arbitrary::any::<u64>(),
                            0_u8..2,
                        ),
                        0..30,
                    ),
                    0_u32..1000,
                    0_u32..1000,
                    0_u8..10,
                ),
                |(samples, krw, usd, extreme)| {
                    match extreme.cmp(&2) {
                        Ordering::Less => assert_extreme_case(extreme),
                        Ordering::Equal => assert_generated_case(&[], 0, 0),
                        Ordering::Greater => assert_generated_case(&samples, krw, usd),
                    }
                    Ok(())
                },
            )
            .expect("4096 공개 경로 독립 정수 대조");
    }
}
