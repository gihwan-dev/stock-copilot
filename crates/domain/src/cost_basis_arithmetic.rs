use crate::{Amount, Currency, LotError, Money, MoneyError, Quantity, ShareCount, ShareRatio};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Digit(u8);
#[derive(Debug, Clone, Copy)]
struct Places(u32);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Natural(Vec<Digit>);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Positive(Natural);
impl Positive {
    fn new(value: Natural) -> Option<Self> {
        if value == Natural::zero() {
            None
        } else {
            Some(Self(value))
        }
    }
}
impl Natural {
    fn zero() -> Self {
        Self(vec![Digit(0)])
    }
    fn one() -> Self {
        Self(vec![Digit(1)])
    }
    fn amount(value: Amount) -> Self {
        Self(
            value
                .as_decimal()
                .mantissa()
                .unsigned_abs()
                .to_string()
                .bytes()
                .rev()
                .map(|byte| Digit(byte.wrapping_sub(b'0')))
                .collect(),
        )
    }
    fn count(value: ShareCount) -> Self {
        Self(
            value
                .as_u64()
                .to_string()
                .bytes()
                .rev()
                .map(|byte| Digit(byte.wrapping_sub(b'0')))
                .collect(),
        )
    }
    fn normalize(mut self) -> Self {
        while self.0.len() > 1 && self.0.last() == Some(&Digit(0)) {
            self.0.truncate(self.0.len().saturating_sub(1));
        }
        self
    }
    fn times_ten(&self) -> Self {
        let mut result = vec![Digit(0)];
        result.extend_from_slice(&self.0);
        Self(result).normalize()
    }
    fn scaled(&self, places: Places) -> Self {
        let mut result = self.clone();
        for _ in 0..places.0 {
            result = result.times_ten();
        }
        result
    }
    fn plus(&self, other: &Self) -> Self {
        let length = self.0.len().max(other.0.len());
        let mut left = self.0.clone();
        left.resize(length, Digit(0));
        let mut right = other.0.clone();
        right.resize(length, Digit(0));
        let mut carry = Digit(0);
        let mut result = Vec::new();
        for (first, second) in left.iter().zip(right) {
            let total = first.0.wrapping_add(second.0).wrapping_add(carry.0);
            result.push(Digit(total.wrapping_rem(10)));
            carry = Digit(total.wrapping_div(10));
        }
        if carry != Digit(0) {
            result.push(carry);
        }
        Self(result).normalize()
    }
    fn difference(&self, other: &Self) -> Self {
        let mut right = other.0.clone();
        right.resize(self.0.len(), Digit(0));
        let mut borrow = Digit(0);
        let mut result = Vec::new();
        for (first, second) in self.0.iter().zip(right) {
            let value = first
                .0
                .wrapping_add(10)
                .wrapping_sub(second.0)
                .wrapping_sub(borrow.0);
            result.push(Digit(value.wrapping_rem(10)));
            borrow = if value < 10 { Digit(1) } else { Digit(0) };
        }
        Self(result).normalize()
    }
    fn product(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        let mut shifted = self.clone();
        for digit in &other.0 {
            for _ in 0..digit.0 {
                result = result.plus(&shifted);
            }
            shifted = shifted.times_ten();
        }
        result
    }
    fn divide(&self, divisor: &Positive) -> (Self, Self) {
        let mut remainder = Self::zero();
        let mut quotient = Vec::new();
        for digit in self.0.iter().rev() {
            remainder = remainder.times_ten().plus(&Self(vec![*digit]));
            let mut next = Digit(0);
            while remainder >= divisor.0 {
                remainder = remainder.difference(&divisor.0);
                next = Digit(next.0.wrapping_add(1));
            }
            quotient.push(next);
        }
        quotient.reverse();
        (Self(quotient).normalize(), remainder)
    }
    fn to_amount(&self, mut places: Places) -> Result<Amount, LotError> {
        let maximum = Self::amount(Amount::new(rust_decimal::Decimal::MAX));
        if self > &maximum.scaled(places) {
            return Err(MoneyError::Overflow.into());
        }
        let mut coefficient = self.clone();
        while places.0 > 0 && coefficient.0.first() == Some(&Digit(0)) {
            coefficient = Self(coefficient.0.into_iter().skip(1).collect()).normalize();
            if coefficient.0.is_empty() {
                coefficient = Self::zero();
            }
            places.0 = places.0.wrapping_sub(1);
        }
        if coefficient > maximum {
            return Err(MoneyError::PrecisionLoss.into());
        }
        Ok(coefficient.bounded_amount(places))
    }
    fn bounded_amount(&self, places: Places) -> Amount {
        let value = self.0.iter().rev().fold(0_i128, |total, digit| {
            total.wrapping_mul(10).wrapping_add(i128::from(digit.0))
        });
        Amount::new(rust_decimal::Decimal::from_i128_with_scale(value, places.0))
    }
    fn text(&self) -> IntegerText {
        IntegerText(
            self.0
                .iter()
                .rev()
                .map(|digit| char::from(b'0'.wrapping_add(digit.0)))
                .collect(),
        )
    }
}
impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.len().cmp(&other.0.len()).then_with(|| {
            self.0
                .iter()
                .rev()
                .map(|digit| digit.0)
                .cmp(other.0.iter().rev().map(|digit| digit.0))
        })
    }
}
impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerText(String);
impl IntegerText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityShape {
    Whole,
    Fractional,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareQuantity {
    numerator: Natural,
    denominator: Positive,
}
impl ShareQuantity {
    fn reduced(numerator: &Natural, denominator: &Positive) -> Self {
        let mut first = numerator.clone();
        let mut second = denominator.0.clone();
        while let Some(divisor) = Positive::new(second) {
            let (_, remainder) = first.divide(&divisor);
            first = divisor.0;
            second = remainder;
        }
        let gcd = Positive(first);
        Self {
            numerator: numerator.divide(&gcd).0,
            denominator: Positive(denominator.0.divide(&gcd).0),
        }
    }
    #[must_use]
    pub fn zero() -> Self {
        Self {
            numerator: Natural::zero(),
            denominator: Positive(Natural::one()),
        }
    }
    #[must_use]
    pub fn from_quantity(value: Quantity) -> Self {
        Self::reduced(
            &Natural::amount(Amount::new(value.as_decimal())),
            &Positive(Natural::one().scaled(Places(value.as_decimal().scale()))),
        )
    }
    #[must_use]
    pub fn numerator(&self) -> IntegerText {
        self.numerator.text()
    }
    #[must_use]
    pub fn denominator(&self) -> IntegerText {
        self.denominator.0.text()
    }
    #[must_use]
    pub fn shape(&self) -> QuantityShape {
        if self.denominator.0 == Natural::one() {
            QuantityShape::Whole
        } else {
            QuantityShape::Fractional
        }
    }
    pub fn exact_quantity(&self) -> Result<Quantity, LotError> {
        let (coefficient, remainder) = self.numerator.scaled(Places(28)).divide(&self.denominator);
        if remainder != Natural::zero() {
            return Err(MoneyError::PrecisionLoss.into());
        }
        let amount = coefficient.to_amount(Places(28))?;
        Quantity::try_new(amount.as_decimal()).map_err(LotError::from)
    }
    pub fn whole_quantity(&self) -> Result<Quantity, LotError> {
        let amount = self
            .numerator
            .divide(&self.denominator)
            .0
            .to_amount(Places(0))?;
        Quantity::try_new(amount.as_decimal()).map_err(LotError::from)
    }
    #[must_use]
    pub fn fractional(&self) -> Self {
        Self::reduced(
            &self.numerator.divide(&self.denominator).1,
            &self.denominator,
        )
    }
    #[must_use]
    pub fn plus(&self, other: &Self) -> Self {
        Self::reduced(
            &self
                .numerator
                .product(&other.denominator.0)
                .plus(&other.numerator.product(&self.denominator.0)),
            &Positive(self.denominator.0.product(&other.denominator.0)),
        )
    }
    pub fn checked_sub(&self, other: &Self) -> Result<Self, LotError> {
        if self < other {
            return Err(LotError::InsufficientQuantity);
        }
        Ok(self.exact_difference(other))
    }
    fn exact_difference(&self, other: &Self) -> Self {
        Self::reduced(
            &self
                .numerator
                .product(&other.denominator.0)
                .difference(&other.numerator.product(&self.denominator.0)),
            &Positive(self.denominator.0.product(&other.denominator.0)),
        )
    }
    #[must_use]
    pub(crate) fn split(&self, requested: &Self) -> (Self, Self) {
        let taken = self.clone().min(requested.clone());
        let remainder = self.exact_difference(&taken);
        (taken, remainder)
    }
    #[must_use]
    pub fn one() -> Self {
        Self {
            numerator: Natural::one(),
            denominator: Positive(Natural::one()),
        }
    }
    #[must_use]
    pub fn scaled(&self, ratio: ShareRatio) -> Self {
        Self::reduced(
            &self.numerator.product(&Natural::count(ratio.new_shares())),
            &Positive(
                self.denominator
                    .0
                    .product(&Natural::count(ratio.old_shares())),
            ),
        )
    }
}
impl Ord for ShareQuantity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numerator
            .product(&other.denominator.0)
            .cmp(&other.numerator.product(&self.denominator.0))
    }
}
impl PartialOrd for ShareQuantity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) fn rounded_cost(
    cost: Money,
    take: &ShareQuantity,
    total: &ShareQuantity,
) -> Result<Money, LotError> {
    if total == &ShareQuantity::zero() {
        return Err(MoneyError::DivisionByZero.into());
    }
    if take > total {
        return Err(LotError::InsufficientQuantity);
    }
    if cost.amount().as_decimal().is_sign_negative() && !cost.amount().as_decimal().is_zero() {
        return Err(LotError::NegativeCost);
    }
    if take == total {
        return Ok(cost);
    }
    let places = Places(match cost.currency() {
        Currency::Krw => 0,
        Currency::Usd => 2,
    });
    let numerator = Natural::amount(cost.amount())
        .product(&take.numerator)
        .product(&total.denominator.0)
        .scaled(places);
    let denominator = take
        .denominator
        .0
        .product(&total.numerator)
        .scaled(Places(cost.amount().as_decimal().scale()));
    let coefficient = numerator.divide(&Positive(denominator)).0;
    Ok(Money::new(coefficient.to_amount(places)?, cost.currency()))
}

pub(crate) fn unit_ratio(
    cost: Money,
    shares: &ShareQuantity,
) -> Result<crate::RatioQuotient, LotError> {
    if shares == &ShareQuantity::zero() {
        return Err(MoneyError::DivisionByZero.into());
    }
    if cost.amount().as_decimal().is_sign_negative() && !cost.amount().as_decimal().is_zero() {
        return Err(LotError::NegativeCost);
    }
    let numerator = Natural::amount(cost.amount()).product(&shares.denominator.0);
    let denominator = Positive(
        shares
            .numerator
            .scaled(Places(cost.amount().as_decimal().scale())),
    );
    let maximum = Natural::amount(Amount::new(rust_decimal::Decimal::MAX));
    if numerator > maximum.product(&denominator.0) {
        return Err(MoneyError::Overflow.into());
    }
    let mut places = Places(28);
    loop {
        let (floor, remainder) = numerator.scaled(places).divide(&denominator);
        let twice = remainder.plus(&remainder);
        let candidate = if twice > denominator.0
            || (twice == denominator.0
                && floor
                    .0
                    .first()
                    .is_some_and(|digit| digit.0.wrapping_rem(2) == 1))
        {
            floor.plus(&Natural::one())
        } else {
            floor
        };
        if candidate <= maximum {
            let amount = candidate.bounded_amount(places);
            return Ok(if remainder == Natural::zero() {
                crate::RatioQuotient::Exact(crate::Ratio::new(amount.as_decimal()))
            } else {
                crate::RatioQuotient::Approximate(crate::ApproximateRatio::new(amount.as_decimal()))
            });
        }
        places = Places(places.0.wrapping_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::{rounded_cost, unit_ratio};
    use crate::{
        Amount, AmountInput, Currency, LotError, Money, MoneyError, NumericInput, Quantity,
        QuantityShape, Ratio, RatioQuotient, ShareCount, ShareQuantity, ShareRatio,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn qty(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(value)).expect("금액"),
            currency,
        )
    }
    fn ratio(old: u64, new: u64) -> ShareRatio {
        ShareRatio::new(
            ShareCount::try_new(old).expect("구주"),
            ShareCount::try_new(new).expect("신주"),
        )
    }
    #[test]
    fn fractions_are_exact_and_integer_ratios_roundtrip() {
        let third = ShareQuantity::from_quantity(qty("1")).scaled(ratio(3, 1));
        assert_eq!(third.numerator().as_str(), "1");
        assert_eq!(third.denominator().as_str(), "3");
        assert_eq!(third.shape(), QuantityShape::Fractional);
        assert_eq!(
            third.exact_quantity(),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_u64..1_000_000, 0_u32..=6, 1_u64..=1000, 1_u64..=1000),
                |(coefficient, scale, old, new)| {
                    let value = Quantity::try_new(rust_decimal::Decimal::from_i128_with_scale(
                        i128::from(coefficient),
                        scale,
                    ))
                    .expect("원본");
                    let original = ShareQuantity::from_quantity(value);
                    let transform = ratio(old, new);
                    let forward = original.scaled(transform);
                    assert_eq!(forward.scaled(transform.inverse()), original);
                    assert_eq!(
                        forward.fractional().plus(&ShareQuantity::from_quantity(
                            forward.whole_quantity().expect("정수부")
                        )),
                        forward
                    );
                    assert_eq!(forward.checked_sub(&forward), Ok(ShareQuantity::zero()));
                    Ok(())
                },
            )
            .expect("정수 비율 왕복");
    }
    #[test]
    fn rounding_matches_independent_bounded_integer_division() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    0_u64..1_000_000_000,
                    0_u32..=4,
                    1_u64..10000,
                    0_u64..10000,
                    0_u32..=6,
                    proptest::sample::select(vec![Currency::Krw, Currency::Usd]),
                ),
                |(coefficient, scale, total, seed, quantity_scale, currency)| {
                    let take = seed
                        .checked_rem(total.checked_add(1).expect("상한"))
                        .expect("나머지");
                    let cost = Money::new(
                        Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                            i128::from(coefficient),
                            scale,
                        )),
                        currency,
                    );
                    let shares = |value| {
                        ShareQuantity::from_quantity(
                            Quantity::try_new(rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(value),
                                quantity_scale,
                            ))
                            .expect("수량"),
                        )
                    };
                    let result = rounded_cost(cost, &shares(take), &shares(total)).expect("배분");
                    let expected = expected_portion(cost, coefficient, scale, take, total);
                    assert_eq!(result, expected);
                    assert_eq!(
                        result.checked_add(&cost.checked_sub(&result).expect("잔여")),
                        Ok(cost)
                    );
                    Ok(())
                },
            )
            .expect("독립 정수 원가 배분");
    }
    fn expected_portion(cost: Money, coefficient: u64, scale: u32, take: u64, total: u64) -> Money {
        let digits = match cost.currency() {
            Currency::Krw => 0,
            Currency::Usd => 2,
        };
        let units = u128::from(coefficient)
            .checked_mul(u128::from(take))
            .expect("곱")
            .checked_mul(10_u128.checked_pow(digits).expect("단위"))
            .expect("분자")
            .checked_div(
                u128::from(total)
                    .checked_mul(10_u128.checked_pow(scale).expect("스케일"))
                    .expect("분모"),
            )
            .expect("절사");
        if take == total {
            cost
        } else {
            Money::new(
                Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                    i128::try_from(units).expect("범위"),
                    digits,
                )),
                cost.currency(),
            )
        }
    }
    #[test]
    fn reference_allocation_covers_full_partial_and_zero_sales_deterministically() {
        for (currency, take, expected) in [
            (Currency::Krw, 3_u64, "1.2345"),
            (Currency::Krw, 1, "0"),
            (Currency::Krw, 0, "0"),
            (Currency::Usd, 3, "1.2345"),
            (Currency::Usd, 1, "0.41"),
            (Currency::Usd, 0, "0"),
        ] {
            let cost = money("1.2345", currency);
            let independent = expected_portion(cost, 12345, 4, take, 3);
            assert_eq!(independent, money(expected, currency));
            assert_eq!(
                rounded_cost(
                    cost,
                    &ShareQuantity::from_quantity(qty(&take.to_string())),
                    &ShareQuantity::from_quantity(qty("3"))
                ),
                Ok(independent)
            );
        }
    }
    #[test]
    fn numeric_boundaries_are_closed_and_do_not_use_approximate_money() {
        let zero = ShareQuantity::zero();
        let one = ShareQuantity::from_quantity(qty("1"));
        assert_eq!(
            rounded_cost(money("1", Currency::Krw), &one, &zero),
            Err(LotError::Arithmetic(MoneyError::DivisionByZero))
        );
        assert_eq!(
            rounded_cost(money("1", Currency::Krw), &one, &zero.plus(&zero)),
            Err(LotError::Arithmetic(MoneyError::DivisionByZero))
        );
        assert_eq!(
            rounded_cost(money("-1", Currency::Krw), &zero, &one),
            Err(LotError::NegativeCost)
        );
        assert_eq!(
            rounded_cost(money("1", Currency::Krw), &one, &one.scaled(ratio(2, 1))),
            Err(LotError::InsufficientQuantity)
        );
        assert_eq!(zero.checked_sub(&one), Err(LotError::InsufficientQuantity));
        let max = ShareQuantity::from_quantity(qty("79228162514264337593543950335"));
        assert_eq!(
            max.scaled(ratio(1, 2)).whole_quantity(),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            max.scaled(ratio(1, 2)).exact_quantity(),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            max.checked_sub(&ShareQuantity::from_quantity(qty("0.1")))
                .expect("유리수 차")
                .exact_quantity(),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        assert_eq!(
            max.scaled(ratio(u64::MAX, 1)).scaled(ratio(1, u64::MAX)),
            max
        );
        assert_eq!(
            ShareQuantity::from_quantity(qty("0")).shape(),
            QuantityShape::Whole
        );
    }
    #[test]
    fn unit_prices_preserve_exactness_and_nearest_even_approximation() {
        let third = ShareQuantity::from_quantity(qty("1")).scaled(ratio(3, 1));
        assert_eq!(
            unit_ratio(money("100", Currency::Krw), &third),
            Ok(RatioQuotient::Exact(
                Ratio::parse(&NumericInput::try_new("300").expect("입력")).expect("단가")
            ))
        );
        let three = ShareQuantity::from_quantity(qty("3"));
        assert_eq!(
            unit_ratio(money("1", Currency::Krw), &three),
            Ok(RatioQuotient::Approximate(crate::ApproximateRatio::new(
                money("0.3333333333333333333333333333", Currency::Krw)
                    .amount()
                    .as_decimal()
            )))
        );
        assert_eq!(
            unit_ratio(money("1", Currency::Krw), &ShareQuantity::zero()),
            Err(LotError::Arithmetic(MoneyError::DivisionByZero))
        );
        assert_eq!(
            unit_ratio(money("-1", Currency::Krw), &three),
            Err(LotError::NegativeCost)
        );
        assert_eq!(
            unit_ratio(
                money("79228162514264337593543950335", Currency::Krw),
                &ShareQuantity::from_quantity(qty("0.1"))
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
    }
    #[test]
    fn unit_price_ties_round_to_even_at_available_precision() {
        for (input, expected) in [
            ("0.0000000000000000000000000001", "0"),
            (
                "0.0000000000000000000000000003",
                "0.0000000000000000000000000002",
            ),
            (
                "79228162514264337593543950335",
                "39614081257132168796771975168",
            ),
        ] {
            assert_eq!(
                unit_ratio(
                    money(input, Currency::Krw),
                    &ShareQuantity::from_quantity(qty("2"))
                ),
                Ok(RatioQuotient::Approximate(crate::ApproximateRatio::new(
                    money(expected, Currency::Krw).amount().as_decimal()
                )))
            );
        }
    }
}
