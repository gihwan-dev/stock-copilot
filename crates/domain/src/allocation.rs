use crate::{
    AllocationInstrument, AllocationSector, Currency, FxRate, Market, Money, MoneyError,
    PortfolioValuation, RatioQuotient, SectorAssignment, Total, TotalCompleteness,
    ValuationAccuracy,
};
use std::collections::BTreeMap;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    Incomplete,
    ZeroTotal,
    NegativeValue,
    DuplicateAssignment,
    MissingFx,
    Arithmetic(MoneyError),
    Conversion(crate::FxConversionError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationBasis {
    Holdings,
    HoldingsAndBuyingPower,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationWeight {
    quotient: RatioQuotient,
    accuracy: ValuationAccuracy,
}
impl AllocationWeight {
    #[must_use]
    pub const fn quotient(self) -> RatioQuotient {
        self.quotient
    }
    #[must_use]
    pub const fn accuracy(self) -> ValuationAccuracy {
        self.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AllocationDisplay(u32);
impl AllocationDisplay {
    #[must_use]
    pub const fn as_units(&self) -> u32 {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationEntry<K> {
    key: K,
    weight: AllocationWeight,
    display: AllocationDisplay,
}
impl<K> AllocationEntry<K> {
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }
    #[must_use]
    pub const fn weight(&self) -> AllocationWeight {
        self.weight
    }
    #[must_use]
    pub const fn display(&self) -> &AllocationDisplay {
        &self.display
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationWeights<K> {
    entries: Vec<AllocationEntry<K>>,
    basis: AllocationBasis,
    accuracy: ValuationAccuracy,
}
impl<K> AllocationWeights<K> {
    #[must_use]
    pub fn entries(&self) -> &[AllocationEntry<K>] {
        &self.entries
    }
    #[must_use]
    pub const fn basis(&self) -> AllocationBasis {
        self.basis
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    valuation: PortfolioValuation,
    instruments: Result<AllocationWeights<AllocationInstrument>, AllocationError>,
    sectors: Result<AllocationWeights<AllocationSector>, AllocationError>,
    countries: Result<AllocationWeights<Market>, AllocationError>,
    currencies: Result<AllocationWeights<AllocationCurrency>, AllocationError>,
}
impl Allocation {
    #[must_use]
    pub fn new(valuation: &PortfolioValuation, assignments: &[SectorAssignment]) -> Self {
        let rates = match valuation.assets() {
            Total::Complete(value) => value.value().clone(),
            Total::Incomplete(value) => value.subtotal().clone(),
        }
        .into_parts()
        .1;
        let holdings = valued_holdings(valuation, &rates);
        let instruments = holdings
            .clone()
            .and_then(|value| weights(value, AllocationBasis::Holdings));
        let sectors = holdings
            .clone()
            .and_then(|value| sector_weights(value, assignments));
        let countries = holdings.clone().and_then(|items| {
            weights(
                items
                    .into_iter()
                    .map(|(key, value)| (key.market().clone(), value))
                    .collect(),
                AllocationBasis::Holdings,
            )
        });
        let currencies = holdings.and_then(|value| currency_weights(value, valuation, &rates));
        Self {
            valuation: valuation.clone(),
            instruments,
            sectors,
            countries,
            currencies,
        }
    }
    #[must_use]
    pub const fn valuation(&self) -> &PortfolioValuation {
        &self.valuation
    }
    #[must_use]
    pub const fn instruments(
        &self,
    ) -> &Result<AllocationWeights<AllocationInstrument>, AllocationError> {
        &self.instruments
    }
    #[must_use]
    pub const fn sectors(&self) -> &Result<AllocationWeights<AllocationSector>, AllocationError> {
        &self.sectors
    }
    #[must_use]
    pub const fn countries(&self) -> &Result<AllocationWeights<Market>, AllocationError> {
        &self.countries
    }
    #[must_use]
    pub const fn currencies(
        &self,
    ) -> &Result<AllocationWeights<AllocationCurrency>, AllocationError> {
        &self.currencies
    }
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
) -> Result<Vec<(AllocationInstrument, Money)>, AllocationError> {
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
            let converted = converted(money, rates)?;
            row.conversion()
                .map_err(|_error| AllocationError::Incomplete)?;
            Ok((AllocationInstrument::new(row.instrument()), converted))
        })
        .collect()
}
fn sector_weights(
    values: Vec<(AllocationInstrument, Money)>,
    assignments: &[SectorAssignment],
) -> Result<AllocationWeights<AllocationSector>, AllocationError> {
    let mut mapping = BTreeMap::new();
    for assignment in assignments {
        if mapping
            .insert(assignment.instrument(), assignment.sector())
            .is_some()
        {
            return Err(AllocationError::DuplicateAssignment);
        }
    }
    weights(
        values
            .into_iter()
            .map(|(key, value)| {
                let sector = mapping
                    .get(&key)
                    .map_or(AllocationSector::Unclassified, |id| {
                        AllocationSector::Classified((*id).clone())
                    });
                (sector, value)
            })
            .collect(),
        AllocationBasis::Holdings,
    )
}
fn currency_weights(
    values: Vec<(AllocationInstrument, Money)>,
    valuation: &PortfolioValuation,
    rates: &[FxRate],
) -> Result<AllocationWeights<AllocationCurrency>, AllocationError> {
    let mut currencies: Vec<_> = values
        .into_iter()
        .map(|(key, value)| {
            (
                match key.market() {
                    Market::Kr => AllocationCurrency::Krw,
                    Market::Us | Market::Unknown(_) => AllocationCurrency::Usd,
                },
                value,
            )
        })
        .collect();
    for currency in [Currency::Krw, Currency::Usd] {
        let cash = valuation
            .buying_power(currency)
            .money()
            .map_err(|_error| AllocationError::Incomplete)?;
        currencies.push((AllocationCurrency::new(currency), converted(cash, rates)?));
    }
    if valuation.assets().completeness() == TotalCompleteness::Incomplete {
        return Err(AllocationError::Incomplete);
    }
    weights(currencies, AllocationBasis::HoldingsAndBuyingPower)
}
pub(crate) fn weights<K: Ord + Clone>(
    values: Vec<(K, Money)>,
    basis: AllocationBasis,
) -> Result<AllocationWeights<K>, AllocationError> {
    let (total, grouped) = grouped_values(values)?;
    if grouped.is_empty() {
        return Err(AllocationError::ZeroTotal);
    }
    let mut entries = Vec::new();
    let mut remainders = Vec::new();
    let mut remaining = 10000_u32;
    for (key, value) in grouped {
        let weight = calculation_weight(value, total)?;
        let (units, remainder) =
            crate::exact_arithmetic::allocation_parts(value.amount(), total.amount());
        remaining = remaining.saturating_sub(units.as_units());
        remainders.push((key.clone(), remainder));
        entries.push(AllocationEntry {
            key,
            weight,
            display: AllocationDisplay(units.as_units()),
        });
    }
    remainders.sort_by(|(left_key, left_remainder), (right_key, right_remainder)| {
        right_remainder
            .cmp(left_remainder)
            .then_with(|| left_key.cmp(right_key))
    });
    let winners: std::collections::BTreeSet<_> = remainders
        .into_iter()
        .zip(0_u32..)
        .take_while(|(_, index)| *index < remaining)
        .map(|(value, _)| value)
        .map(|(key, _)| key)
        .collect();
    for entry in &mut entries {
        if winners.contains(&entry.key) {
            entry.display.0 = entry.display.0.saturating_add(1);
        }
    }
    Ok(AllocationWeights {
        entries,
        basis,
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
        if value.amount().as_decimal().is_sign_negative() && !value.amount().as_decimal().is_zero()
        {
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

pub(crate) fn calculation_weight(
    value: Money,
    total: Money,
) -> Result<AllocationWeight, AllocationError> {
    value
        .checked_ratio(&total)
        .map(|quotient| AllocationWeight {
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

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, Allocation, AllocationBasis, AllocationError, AllocationSector,
        BuyingPower, Currency, CurrencyAmounts, DisplayDirection, ExclusionDecision, Exclusions,
        FxPair, FxRate, FxRateKind, FxSource, Holding, HoldingObservation, Market, Money,
        MoneyError, Nanosecond, NumericInput, OffsetSeconds, Portfolio, PortfolioValuation, Price,
        Quantity, RatioQuotient, ReportedPortfolio, ResolvedOrderInstrument, SectorAssignment,
        SectorId, Symbol, TimeInterval, Timestamp, TotalCompleteness, UnixSeconds,
        ValuationAccuracy, allocation, exact_arithmetic,
    };
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
    fn holding(symbol: &str, value: Option<&str>, currency: Currency) -> Holding {
        Holding::new(
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(symbol).expect("심볼"),
                match currency {
                    Currency::Krw => Market::Kr,
                    Currency::Usd => Market::Us,
                },
            )
            .expect("종목"),
            Quantity::parse(&input("1")).expect("수량"),
            AcquisitionPrice::try_new(money("1", currency)).expect("원가"),
            HoldingObservation::new(
                value.map(|text| Price::parse(&input(text), currency).expect("시세")),
                None,
                time(),
            ),
        )
        .expect("보유")
    }
    fn portfolio(items: Vec<Holding>, cash: &str) -> PortfolioValuation {
        Portfolio::new(
            items,
            BuyingPower::new(money(cash, Currency::Krw), time()),
            BuyingPower::new(money("0", Currency::Usd), time()),
        )
        .expect("포트폴리오")
        .evaluate(None)
    }
    #[test]
    fn allocation_three_equal_ties_and_cash_basis_are_explicit() {
        let value = portfolio(
            vec![
                holding("C", Some("1"), Currency::Krw),
                holding("A", Some("1"), Currency::Krw),
                holding("B", Some("1"), Currency::Krw),
            ],
            "3",
        );
        let allocation = Allocation::new(&value, &[]);
        let rows = allocation.instruments().as_ref().expect("종목");
        assert_eq!(
            rows.entries()
                .iter()
                .map(|remainder| remainder.display().as_units())
                .collect::<Vec<_>>(),
            vec![3334, 3333, 3333]
        );
        assert_eq!(rows.basis(), AllocationBasis::Holdings);
        assert_eq!(rows.accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(
            allocation.currencies().as_ref().expect("통화").basis(),
            AllocationBasis::HoldingsAndBuyingPower
        );
        assert_eq!(
            allocation
                .sectors()
                .as_ref()
                .expect("섹터")
                .entries()
                .first()
                .expect("행")
                .key(),
            &AllocationSector::Unclassified
        );
        assert_eq!(allocation.valuation(), &value);
    }
    #[test]
    fn allocation_incomplete_empty_and_cash_errors_never_make_complete_weights() {
        let missing = portfolio(vec![holding("A", None, Currency::Krw)], "0");
        assert_eq!(
            Allocation::new(&missing, &[]).instruments(),
            &Err(AllocationError::Incomplete)
        );
        let empty = portfolio(vec![], "1");
        assert_eq!(
            Allocation::new(&empty, &[]).countries(),
            &Err(AllocationError::ZeroTotal)
        );
        assert!(Allocation::new(&empty, &[]).currencies().is_ok());
        let bad = portfolio(vec![holding("A", Some("1"), Currency::Krw)], "-1");
        assert!(Allocation::new(&bad, &[]).instruments().is_ok());
        assert_eq!(
            Allocation::new(&bad, &[]).currencies(),
            &Err(AllocationError::Incomplete)
        );
    }
    #[test]
    fn allocation_independent_integer_reference_and_permutations() {
        TestRunner::new(Config::with_cases(4096))
            .run(&proptest::collection::vec(1_u32..10000, 1..40), |values| {
                let mut amounts: Vec<_> = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| (index, money(&value.to_string(), Currency::Krw)))
                    .collect();
                let expected_total: u128 = values.iter().map(|value| u128::from(*value)).sum();
                let actual =
                    allocation::weights(amounts.clone(), AllocationBasis::Holdings).expect("배분");
                let sum: u32 = actual
                    .entries()
                    .iter()
                    .map(|row| row.display().as_units())
                    .sum();
                assert_eq!(sum, 10000);
                let reference = integer_reference(&values, expected_total);
                for (row, (index, quotient, _)) in actual.entries().iter().zip(reference) {
                    assert_eq!(row.key(), &index);
                    assert_eq!(u128::from(row.display().as_units()), quotient);
                    let numerator = u128::from(values.get(index).copied().expect("값"))
                        .checked_mul(10000)
                        .expect("분자");
                    assert!(
                        quotient
                            .checked_mul(expected_total)
                            .expect("표시")
                            .abs_diff(numerator)
                            < expected_total
                    );
                    assert_eq!(row.weight().accuracy(), ValuationAccuracy::Approximate);
                    assert!(match row.weight().quotient() {
                        RatioQuotient::Exact(ratio) =>
                            ratio.as_decimal() >= rust_decimal::Decimal::ZERO,
                        RatioQuotient::Approximate(ratio) =>
                            ratio.original_direction() == DisplayDirection::Up,
                    });
                }
                amounts.reverse();
                assert_eq!(
                    allocation::weights(amounts, AllocationBasis::Holdings).expect("역순"),
                    actual
                );
                Ok(())
            })
            .expect("4096 독립 기준");
    }

    fn integer_reference(values: &[u32], expected_total: u128) -> Vec<(usize, u128, u128)> {
        let mut reference: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let scaled = u128::from(*value).checked_mul(10000).expect("곱");
                (
                    index,
                    scaled.checked_div(expected_total).expect("몫"),
                    scaled.checked_rem(expected_total).expect("나머지"),
                )
            })
            .collect();
        let floor_sum: u128 = reference.iter().map(|(_, quotient, _)| *quotient).sum();
        reference.sort_by_key(|(index, _, remainder)| (std::cmp::Reverse(*remainder), *index));
        for (_, quotient, _) in reference
            .iter_mut()
            .take(usize::try_from(10000_u128.checked_sub(floor_sum).expect("잔여")).expect("개수"))
        {
            *quotient = quotient.checked_add(1).expect("배분");
        }
        reference.sort_by_key(|(index, _, _)| *index);
        reference
    }
    #[test]
    fn allocation_sector_identity_duplicates_and_exclusions_agree() {
        let item = holding("A", Some("2"), Currency::Krw);
        let sector = SectorId::try_new("Tech").expect("섹터");
        let assignment = SectorAssignment::new(item.instrument(), sector.clone());
        assert_eq!(assignment.sector(), &sector);
        assert_eq!(assignment.instrument().symbol(), item.instrument().symbol());
        assert_eq!(assignment.instrument().market(), &Market::Kr);
        let value = portfolio(vec![item], "0");
        let allocated = Allocation::new(&value, std::slice::from_ref(&assignment));
        assert_eq!(
            allocated
                .sectors()
                .as_ref()
                .expect("섹터")
                .entries()
                .first()
                .expect("행")
                .key(),
            &AllocationSector::Classified(sector.clone())
        );
        assert_eq!(
            Allocation::new(&value, &[assignment.clone(), assignment]).sectors(),
            &Err(AllocationError::DuplicateAssignment)
        );
        let exclusions = Exclusions::try_new(vec![sector.clone()], vec![]).expect("제외");
        assert_eq!(
            exclusions.sector_decision(&sector),
            ExclusionDecision::Excluded
        );
        assert_eq!(
            exclusions.sector_decision(&SectorId::try_new("tech").expect("소문자")),
            ExclusionDecision::Included
        );
    }
    #[test]
    fn allocation_zero_negative_extremes_and_many_small_residuals() {
        let zero = money("0", Currency::Krw);
        assert_eq!(
            allocation::weights(vec![(0_u32, zero)], AllocationBasis::Holdings),
            Err(AllocationError::ZeroTotal)
        );
        assert_eq!(
            allocation::weights(
                vec![(0_u32, money("-1", Currency::Krw))],
                AllocationBasis::Holdings
            ),
            Err(AllocationError::NegativeValue)
        );
        assert_eq!(
            allocation::weights(
                vec![(0_u32, money("1", Currency::Usd))],
                AllocationBasis::Holdings
            ),
            Err(AllocationError::Arithmetic(MoneyError::CurrencyMismatch))
        );
        let maximum = money("79228162514264337593543950335", Currency::Krw);
        assert_eq!(
            allocation::weights(
                vec![(0_u32, maximum), (1_u32, maximum)],
                AllocationBasis::Holdings
            ),
            Err(AllocationError::Arithmetic(MoneyError::Overflow))
        );
        for amount in [
            maximum,
            money("0.0000000000000000000000000001", Currency::Krw),
        ] {
            let single = allocation::weights(vec![(0_u32, amount)], AllocationBasis::Holdings)
                .expect("한 항목");
            assert_eq!(
                single.entries().first().expect("행").display().as_units(),
                10000
            );
        }
    }
    #[test]
    fn allocation_many_small_residuals_and_grouping() {
        let zero = money("0", Currency::Krw);
        let tiny = allocation::weights(
            (0_u32..4096)
                .map(|index| {
                    (
                        index,
                        money("0.0000000000000000000000000001", Currency::Krw),
                    )
                })
                .collect(),
            AllocationBasis::Holdings,
        )
        .expect("작은 비중 다수");
        assert_eq!(
            tiny.entries()
                .iter()
                .map(|row| row.display().as_units())
                .sum::<u32>(),
            10000
        );
        assert_eq!(
            tiny.entries().first().expect("첫 행").display().as_units(),
            3
        );
        assert_eq!(
            tiny.entries().last().expect("끝 행").display().as_units(),
            2
        );
        let thirds = allocation::weights(
            vec![
                (0_u32, money("1", Currency::Krw)),
                (1_u32, money("2", Currency::Krw)),
            ],
            AllocationBasis::Holdings,
        )
        .expect("셋");
        assert_eq!(
            thirds
                .entries()
                .iter()
                .map(|row| row.display().as_units())
                .collect::<Vec<_>>(),
            vec![3333, 6667]
        );
        let grouped = allocation::weights(
            vec![
                (0_u32, money("1", Currency::Krw)),
                (0_u32, money("1", Currency::Krw)),
                (1_u32, zero),
            ],
            AllocationBasis::Holdings,
        )
        .expect("그룹");
        assert_eq!(
            grouped.entries().last().expect("0 행").display().as_units(),
            0
        );
    }
    fn fx() -> FxRate {
        FxRate::parse(
            &input("1375"),
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
    #[test]
    fn allocation_r11_currency_country_and_cash_use_krw_values() {
        let original = Portfolio::new(
            vec![
                holding("KR", Some("10000000"), Currency::Krw),
                holding("US", Some("5000"), Currency::Usd),
            ],
            BuyingPower::new(money("0", Currency::Krw), time()),
            BuyingPower::new(money("0", Currency::Usd), time()),
        )
        .expect("혼합");
        let evaluated = original.evaluate(Some(fx()));
        let allocation = Allocation::new(&evaluated, &[]);
        let currencies = allocation.currencies().as_ref().expect("통화");
        assert_eq!(
            currencies
                .entries()
                .iter()
                .map(|row| (row.key().as_currency(), row.display().as_units()))
                .collect::<Vec<_>>(),
            vec![(Currency::Krw, 5926), (Currency::Usd, 4074)]
        );
        assert_eq!(
            allocation
                .countries()
                .as_ref()
                .expect("국가")
                .entries()
                .iter()
                .map(|row| row.key().clone())
                .collect::<Vec<_>>(),
            vec![Market::Kr, Market::Us]
        );
        assert_eq!(
            Allocation::new(&original.evaluate(None), &[]).instruments(),
            &Err(AllocationError::Incomplete)
        );
        let cash = Portfolio::new(
            vec![],
            BuyingPower::new(money("0", Currency::Krw), time()),
            BuyingPower::new(money("1", Currency::Usd), time()),
        )
        .expect("달러 현금")
        .evaluate(Some(fx()));
        let cash_allocation = Allocation::new(&cash, &[]);
        assert_eq!(
            cash_allocation
                .currencies()
                .as_ref()
                .expect("통화")
                .entries()
                .last()
                .expect("달러")
                .display()
                .as_units(),
            10000
        );
    }
    #[test]
    fn allocation_reported_complete_total_cannot_hide_missing_constituent() {
        let amounts = CurrencyAmounts::new(money("20", Currency::Krw), None).expect("요약");
        let original = Portfolio::new(
            vec![holding("A", None, Currency::Krw)],
            BuyingPower::new(money("0", Currency::Krw), time()),
            BuyingPower::new(money("0", Currency::Usd), time()),
        )
        .expect("포트폴리오")
        .with_reported(ReportedPortfolio::new(amounts, amounts, amounts, time()));
        let value = original.evaluate(None);
        assert_eq!(
            value.valuation().completeness(),
            TotalCompleteness::Complete
        );
        assert_eq!(
            Allocation::new(&value, &[]).instruments(),
            &Err(AllocationError::Incomplete)
        );
    }

    #[test]
    fn allocation_wide_remainders_order_and_mixed_scales_match_integer_fractions() {
        let total = money("3", Currency::Krw).amount();
        let first = exact_arithmetic::allocation_parts(money("1", Currency::Krw).amount(), total).1;
        let second =
            exact_arithmetic::allocation_parts(money("2", Currency::Krw).amount(), total).1;
        assert_eq!(first.partial_cmp(&second), Some(std::cmp::Ordering::Less));
        let rows = allocation::weights(
            vec![
                (0_u32, money("0.01", Currency::Krw)),
                (1_u32, money("0.0200", Currency::Krw)),
                (2_u32, money("0.03", Currency::Krw)),
            ],
            AllocationBasis::Holdings,
        )
        .expect("다른 스케일");
        assert_eq!(
            rows.entries()
                .iter()
                .map(|row| row.display().as_units())
                .collect::<Vec<_>>(),
            vec![1667, 3333, 5000]
        );
        let wide = allocation::weights(
            vec![
                (0_u32, money("10000000000000000000000000000", Currency::Krw)),
                (1_u32, money("1", Currency::Krw)),
            ],
            AllocationBasis::Holdings,
        )
        .expect("큰 계수");
        assert_eq!(
            wide.entries()
                .iter()
                .map(|row| row.display().as_units())
                .collect::<Vec<_>>(),
            vec![10000, 0]
        );
    }

    #[test]
    fn allocation_arithmetic_errors_preserve_independent_boundaries() {
        assert_eq!(
            allocation::calculation_weight(money("1", Currency::Krw), money("0", Currency::Krw)),
            Err(AllocationError::ZeroTotal)
        );
        assert_eq!(
            allocation::weights(
                vec![
                    (0_u32, money("0.4", Currency::Krw)),
                    (1_u32, money("0.6", Currency::Krw)),
                    (1_u32, money("10000000000000000000000000000", Currency::Krw))
                ],
                AllocationBasis::Holdings
            ),
            Err(AllocationError::Arithmetic(MoneyError::PrecisionLoss))
        );
        let cash = Portfolio::new(
            vec![],
            BuyingPower::new(money("0", Currency::Krw), time()),
            BuyingPower::new(money("1", Currency::Usd), time()),
        )
        .expect("현금")
        .evaluate(None);
        assert_eq!(
            Allocation::new(&cash, &[]).currencies(),
            &Err(AllocationError::MissingFx)
        );
    }

    fn reported_valuation(items: Vec<Holding>, cash: &str, summary: &str) -> PortfolioValuation {
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
    #[test]
    fn allocation_reported_totals_do_not_hide_fx_accumulation_or_asset_failures() {
        let foreign = reported_valuation(vec![holding("US", Some("1"), Currency::Usd)], "0", "1");
        assert_eq!(
            Allocation::new(&foreign, &[]).instruments(),
            &Err(AllocationError::MissingFx)
        );
        let overflow = reported_valuation(
            vec![
                holding("A", Some("79228162514264337593543950335"), Currency::Krw),
                holding("B", Some("1"), Currency::Krw),
            ],
            "0",
            "1",
        );
        assert_eq!(
            Allocation::new(&overflow, &[]).instruments(),
            &Err(AllocationError::Incomplete)
        );
        let assets = reported_valuation(vec![], "1", "79228162514264337593543950335");
        assert_eq!(
            Allocation::new(&assets, &[]).currencies(),
            &Err(AllocationError::Incomplete)
        );
        assert_eq!(
            allocation::calculation_weight(money("1", Currency::Usd), money("1", Currency::Krw)),
            Err(AllocationError::Arithmetic(MoneyError::CurrencyMismatch))
        );
    }

    #[test]
    fn allocation_integer_floor_is_independent_of_residual_distribution() {
        for (value, total, expected) in [
            ("0", "3", 0),
            ("1", "3", 3333),
            ("2", "3", 6666),
            ("3", "3", 10000),
            ("1", "6", 1666),
        ] {
            let (units, _) = exact_arithmetic::allocation_parts(
                money(value, Currency::Krw).amount(),
                money(total, Currency::Krw).amount(),
            );
            assert_eq!(units.as_units(), expected);
        }
    }
}
