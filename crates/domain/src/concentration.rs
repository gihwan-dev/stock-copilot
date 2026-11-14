pub use crate::concentration_arithmetic::{
    ConcentrationArithmeticError, ConcentrationDisplay, ConcentrationHhi, ConcentrationMetrics,
    ConcentrationWeight, EffectiveInstrumentCount,
};
use crate::{Allocation, AllocationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationWarning {
    Warning,
    Clear,
    NotApplicable,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationThreshold(u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationThresholdError {
    OutOfRange,
}
impl ConcentrationThreshold {
    pub fn try_new(value: u16) -> Result<Self, ConcentrationThresholdError> {
        if !(1..=4096).contains(&value) {
            return Err(ConcentrationThresholdError::OutOfRange);
        }
        Ok(Self(value))
    }
    pub(crate) const fn as_u16(&self) -> u16 {
        self.0
    }
}
impl Default for ConcentrationThreshold {
    fn default() -> Self {
        Self(5)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationErrorKind {
    Allocation,
    Arithmetic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorCause {
    Allocation(AllocationError),
    Arithmetic(ConcentrationArithmeticError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcentrationError(ErrorCause);
impl ConcentrationError {
    #[must_use]
    pub const fn kind(self) -> ConcentrationErrorKind {
        match self.0 {
            ErrorCause::Allocation(_) => ConcentrationErrorKind::Allocation,
            ErrorCause::Arithmetic(_) => ConcentrationErrorKind::Arithmetic,
        }
    }
    #[must_use]
    pub const fn allocation_error(self) -> Option<AllocationError> {
        match self.0 {
            ErrorCause::Allocation(error) => Some(error),
            ErrorCause::Arithmetic(_) => None,
        }
    }
    #[must_use]
    pub const fn arithmetic_error(self) -> Option<ConcentrationArithmeticError> {
        match self.0 {
            ErrorCause::Allocation(_) => None,
            ErrorCause::Arithmetic(error) => Some(error),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationKind {
    Available,
    NotApplicable,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Available(Box<ConcentrationMetrics>),
    NotApplicable,
    Unavailable(ConcentrationError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationStatus(State);
impl ConcentrationStatus {
    #[must_use]
    pub const fn kind(&self) -> ConcentrationKind {
        match self.0 {
            State::Available(_) => ConcentrationKind::Available,
            State::NotApplicable => ConcentrationKind::NotApplicable,
            State::Unavailable(_) => ConcentrationKind::Unavailable,
        }
    }
    #[must_use]
    pub fn metrics(&self) -> Option<&ConcentrationMetrics> {
        match &self.0 {
            State::Available(value) => Some(value.as_ref()),
            State::NotApplicable | State::Unavailable(_) => None,
        }
    }
    #[must_use]
    pub const fn error(&self) -> Option<&ConcentrationError> {
        match &self.0 {
            State::Unavailable(error) => Some(error),
            State::Available(_) | State::NotApplicable => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concentration(ConcentrationStatus);
impl Concentration {
    #[must_use]
    pub fn new(allocation: &Allocation) -> Self {
        let values = match allocation.holdings().instruments() {
            Err(AllocationError::ZeroTotal) => {
                return Self(ConcentrationStatus(State::NotApplicable));
            }
            Err(error) => {
                return Self(ConcentrationStatus(State::Unavailable(ConcentrationError(
                    ErrorCause::Allocation(*error),
                ))));
            }
            Ok(values) => values,
        };
        let weights: Vec<_> = values
            .entries()
            .iter()
            .map(|entry| entry.weight().concentration_statistics_input())
            .collect();
        Self::calculated(crate::concentration_arithmetic::calculate(&weights))
    }
    fn calculated(result: Result<ConcentrationMetrics, ConcentrationArithmeticError>) -> Self {
        let state = match result {
            Ok(value) => State::Available(Box::new(value)),
            Err(error) => State::Unavailable(ConcentrationError(ErrorCause::Arithmetic(error))),
        };
        Self(ConcentrationStatus(state))
    }
    #[must_use]
    pub const fn status(&self) -> &ConcentrationStatus {
        &self.0
    }
    #[must_use]
    pub fn warning(&self) -> ConcentrationWarning {
        self.warning_with_threshold(&ConcentrationThreshold::default())
    }
    #[must_use]
    pub fn warning_with_threshold(
        &self,
        threshold: &ConcentrationThreshold,
    ) -> ConcentrationWarning {
        match &self.0.0 {
            State::Available(value) => {
                if value.effective_count().below(threshold).is_lt() {
                    ConcentrationWarning::Warning
                } else {
                    ConcentrationWarning::Clear
                }
            }
            State::NotApplicable => ConcentrationWarning::NotApplicable,
            State::Unavailable(_) => ConcentrationWarning::Unavailable,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{
        Concentration, ConcentrationKind, ConcentrationMetrics, ConcentrationThreshold,
        ConcentrationWarning,
    };
    use crate::{
        AcquisitionPrice, BuyingPower, Currency, Holding, HoldingObservation, Market, Money,
        Nanosecond, NumericInput, OffsetSeconds, Portfolio, Quantity, Ratio, ReportedHolding,
        ResolvedOrderInstrument, SectorAssignments, Symbol, Timestamp, UnixSeconds,
    };
    use crate::{Allocation, AllocationError, ValuationAccuracy};
    use proptest::test_runner::{Config, TestRunner};

    fn allocation(values: &[Option<&str>], cash: &str) -> Allocation {
        let input = |text: &str| NumericInput::try_new(text).expect("입력");
        let money = |text: &str| Money::parse(&input(text), Currency::Krw).expect("금액");
        let time = Timestamp::from_parts(
            UnixSeconds::new(1000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let holdings = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let instrument = ResolvedOrderInstrument::from_master_lookup_result(
                    Symbol::try_new(&format!("S{index}")).expect("심볼"),
                    Market::Kr,
                )
                .expect("종목");
                let reported = value.map(|text| {
                    ReportedHolding::new(
                        money(text),
                        money("0"),
                        Ratio::parse(&input("0")).expect("율"),
                    )
                    .expect("평가")
                });
                Holding::new(
                    instrument,
                    Quantity::parse(&input("1")).expect("수량"),
                    AcquisitionPrice::try_new(money("0")).expect("원가"),
                    HoldingObservation::new(None, reported, time),
                )
                .expect("보유")
            })
            .collect();
        let valuation = Portfolio::new(
            holdings,
            BuyingPower::new(money(cash), time),
            BuyingPower::new(
                Money::parse(&input("0"), Currency::Usd).expect("달러 현금"),
                time,
            ),
        )
        .expect("포트폴리오")
        .evaluate(None);
        Allocation::new(&valuation, &SectorAssignments::default())
    }
    fn metrics(value: &Concentration) -> &ConcentrationMetrics {
        value.status().metrics().expect("계산 가능")
    }
    #[test]
    fn reviewer_cash_table_and_r11_are_holdings_only() {
        for (values, cash, expected, warning) in [
            (
                vec![Some("1")],
                "9",
                "1.000000",
                ConcentrationWarning::Warning,
            ),
            (
                vec![Some("1"); 2],
                "18",
                "2.000000",
                ConcentrationWarning::Warning,
            ),
            (
                vec![Some("4")],
                "6",
                "1.000000",
                ConcentrationWarning::Warning,
            ),
            (
                vec![Some("1"); 5],
                "5",
                "5.000000",
                ConcentrationWarning::Clear,
            ),
            (
                vec![
                    Some("35"),
                    Some("25"),
                    Some("15"),
                    Some("10"),
                    Some("10"),
                    Some("5"),
                ],
                "900",
                "4.347826",
                ConcentrationWarning::Warning,
            ),
        ] {
            let value = Concentration::new(&allocation(&values, cash));
            assert_eq!(
                metrics(&value).effective_count().display().as_str(),
                expected
            );
            assert_eq!(value.warning(), warning);
            if values.len() == 6 {
                assert_eq!(metrics(&value).hhi().display().as_str(), "0.2300");
                assert_eq!(metrics(&value).top_one().display().as_str(), "0.35");
                assert_eq!(metrics(&value).top_three().display().as_str(), "0.75");
                assert_eq!(metrics(&value).top_five().display().as_str(), "0.95");
            }
        }
    }
    #[test]
    fn empty_incomplete_threshold_and_exact_boundary() {
        for cash in ["0", "100"] {
            let value = Concentration::new(&allocation(&[], cash));
            assert_eq!(value.status().kind(), ConcentrationKind::NotApplicable);
            assert_eq!(value.warning(), ConcentrationWarning::NotApplicable);
            assert_eq!(value.status().error(), None);
        }
        let missing = Concentration::new(&allocation(&[None], "10"));
        assert_eq!(missing.status().kind(), ConcentrationKind::Unavailable);
        assert_eq!(
            missing.status().error().expect("오류").allocation_error(),
            Some(AllocationError::Incomplete)
        );
        assert_eq!(missing.warning(), ConcentrationWarning::Unavailable);
        let five = Concentration::new(&allocation(&[Some("1"); 5], "50"));
        assert_eq!(five.warning(), ConcentrationWarning::Clear);
        assert_eq!(
            five.warning_with_threshold(&ConcentrationThreshold::try_new(6).expect("기준")),
            ConcentrationWarning::Warning
        );
        let below = Concentration::new(&allocation(
            &[
                Some("1.0000000000000000000000000001"),
                Some("1"),
                Some("1"),
                Some("1"),
                Some("1"),
            ],
            "50",
        ));
        assert_eq!(
            metrics(&below).effective_count().display().as_str(),
            "5.000000"
        );
        assert_eq!(below.warning(), ConcentrationWarning::Warning);
        assert_eq!(
            ConcentrationThreshold::try_new(0),
            Err(super::ConcentrationThresholdError::OutOfRange)
        );
        assert_eq!(
            ConcentrationThreshold::try_new(4097),
            Err(super::ConcentrationThresholdError::OutOfRange)
        );
        let _threshold = ConcentrationThreshold::try_new(4096).expect("최대 기준");
    }
    #[test]
    fn exact_display_midpoints_round_away_from_zero() {
        let value = Concentration::new(&allocation(&[Some("1"), Some("7")], "100"));
        assert_eq!(metrics(&value).hhi().display().as_str(), "0.7813");
        assert_eq!(metrics(&value).top_one().display().as_str(), "0.88");
        assert_eq!(
            metrics(&value).effective_count().display().as_str(),
            "1.280000"
        );
    }
    #[test]
    fn extreme_values_display_safely_and_debug_is_redacted() {
        for text in [
            "0.0000000000000000000000000001",
            "79228162514264337593543950335",
        ] {
            let value = Concentration::new(&allocation(&[Some(text)], "0"));
            assert_eq!(
                metrics(&value).effective_count().display().as_str(),
                "1.000000"
            );
            assert!(!format!("{value:?}").contains(text));
        }
        for count in [1, 3, 5, 10, 37, 4096] {
            let value = Concentration::new(&allocation(&vec![Some("1"); count], "0"));
            assert_eq!(
                metrics(&value).effective_count().display().as_str(),
                format!("{count}.000000")
            );
        }
    }
    fn rounded(numerator: u128, denominator: u128, scale: u32) -> String {
        let factor = 10_u128.checked_pow(scale).expect("배율");
        let mut quotient = numerator.checked_div(denominator).expect("정수 몫");
        let mut remainder = numerator.checked_rem(denominator).expect("나머지");
        for _ in 0..scale {
            let shifted = remainder.checked_mul(10).expect("자리 이동");
            quotient = quotient
                .checked_mul(10)
                .and_then(|value| {
                    value.checked_add(shifted.checked_div(denominator).expect("자리 몫"))
                })
                .expect("누적 몫");
            remainder = shifted.checked_rem(denominator).expect("자리 나머지");
        }
        let units = quotient
            .checked_add(u128::from(
                remainder.checked_mul(2).expect("동률") >= denominator,
            ))
            .expect("반올림");
        format!(
            "{}.{:0width$}",
            units.checked_div(factor).expect("정수"),
            units.checked_rem(factor).expect("소수"),
            width = usize::try_from(scale).expect("폭")
        )
    }
    fn assert_case(mut values: Vec<u64>, cash: u32) {
        let total = values.iter().map(|value| u128::from(*value)).sum::<u128>();
        let squares = values
            .iter()
            .map(|value| {
                u128::from(*value)
                    .checked_mul(u128::from(*value))
                    .expect("제곱")
            })
            .sum::<u128>();
        let texts: Vec<_> = values
            .iter()
            .map(|value| format!("{}.{:04}", value.div_euclid(10000), value.rem_euclid(10000)))
            .collect();
        let actual = Concentration::new(&allocation(
            &texts
                .iter()
                .map(|value| Some(value.as_str()))
                .collect::<Vec<_>>(),
            &cash.to_string(),
        ));
        if total == 0 {
            assert_eq!(actual.warning(), ConcentrationWarning::NotApplicable);
            return;
        }
        let result = metrics(&actual);
        let total_squared = total.checked_mul(total).expect("분모 제곱");
        assert_eq!(
            result.hhi().display().as_str(),
            rounded(squares, total_squared, 4)
        );
        assert_eq!(
            result.effective_count().display().as_str(),
            rounded(total_squared, squares, 6)
        );
        values.sort_by(|left, right| right.cmp(left));
        for (size, weight) in [
            (1, result.top_one()),
            (3, result.top_three()),
            (5, result.top_five()),
        ] {
            assert_eq!(
                weight.display().as_str(),
                rounded(
                    values
                        .iter()
                        .take(size)
                        .map(|value| u128::from(*value))
                        .sum(),
                    total,
                    2
                )
            );
        }
        assert_warning(&actual, squares, total_squared);
        assert_eq!(result.accuracy(), ValuationAccuracy::Approximate);
    }
    #[test]
    fn independent_rationals_cover_sixteen_orders_and_exact_midpoints() {
        for values in [
            vec![0],
            vec![1, 7],
            vec![1, 10_000_000_000_000_000],
            vec![2865, 2762, 404, 5443, 5760],
            vec![1; 10],
        ] {
            assert_case(values, 999);
        }
        TestRunner::new(Config::with_cases(4096))
            .run(
                &proptest::collection::vec(
                    proptest::prop_oneof![
                        proptest::strategy::Just(1_u64),
                        proptest::strategy::Just(10_000_000_000_000_000_u64),
                        0_u64..10_000_000_000_000_000
                    ],
                    1..16,
                ),
                |values| {
                    assert_case(values, 100);
                    Ok(())
                },
            )
            .expect("4096 독립 유리수 대조");
    }
    fn assert_warning(actual: &Concentration, squares: u128, total_squared: u128) {
        assert_eq!(
            actual.warning(),
            if squares.checked_mul(5).expect("경계") > total_squared {
                ConcentrationWarning::Warning
            } else {
                ConcentrationWarning::Clear
            }
        );
    }
    #[test]
    fn allocation_weight_equality_ignores_concentration_provenance() {
        let first = allocation(&[Some("1"), Some("1")], "0");
        let second = allocation(&[Some("10"), Some("10")], "0");
        assert_eq!(
            first
                .holdings()
                .instruments()
                .as_ref()
                .expect("구성비")
                .entries()
                .first()
                .expect("행")
                .weight(),
            second
                .holdings()
                .instruments()
                .as_ref()
                .expect("구성비")
                .entries()
                .first()
                .expect("행")
                .weight()
        );
        assert_eq!(
            first
                .assets()
                .instruments()
                .as_ref()
                .expect("자산 비중")
                .entries()
                .first()
                .expect("행")
                .weight(),
            second
                .assets()
                .instruments()
                .as_ref()
                .expect("자산 비중")
                .entries()
                .first()
                .expect("행")
                .weight()
        );
    }
    #[test]
    fn status_error_views_and_comparisons_are_closed() {
        let one = Concentration::new(&allocation(&[Some("1")], "0"));
        let two = Concentration::new(&allocation(&[Some("1"); 2], "100"));
        assert_eq!(one.status().kind(), ConcentrationKind::Available);
        assert_eq!(one.status().error(), None);
        assert_eq!(
            metrics(&one).hhi().compare(metrics(&two).hhi()),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            metrics(&one)
                .effective_count()
                .compare(metrics(&two).effective_count()),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            metrics(&one).top_one().compare(metrics(&two).top_one()),
            std::cmp::Ordering::Greater
        );
        let absent = Concentration::new(&allocation(&[], "0"));
        assert_eq!(absent.status().metrics(), None);
        let missing = Concentration::new(&allocation(&[None], "0"));
        let error = missing.status().error().expect("오류");
        assert_eq!(error.kind(), super::ConcentrationErrorKind::Allocation);
        assert_eq!(error.arithmetic_error(), None);
        let arithmetic = Concentration::calculated(Err(super::ConcentrationArithmeticError::Range));
        let arithmetic_error = arithmetic.status().error().expect("산술 오류");
        assert_eq!(
            arithmetic_error.kind(),
            super::ConcentrationErrorKind::Arithmetic
        );
        assert_eq!(arithmetic_error.allocation_error(), None);
        assert_eq!(
            arithmetic_error.arithmetic_error(),
            Some(super::ConcentrationArithmeticError::Range)
        );
        assert_eq!(arithmetic.warning(), ConcentrationWarning::Unavailable);
    }
}
