use crate::{
    ConvertedMoney, Currency, FxConversionError, MidConvertedMoney, Money, NumericError,
    NumericInput, Price, TimeInterval, Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxPair {
    UsdKrw,
    KrwUsd,
}

impl FxPair {
    #[must_use]
    pub const fn base(self) -> Currency {
        match self {
            Self::UsdKrw => Currency::Usd,
            Self::KrwUsd => Currency::Krw,
        }
    }

    #[must_use]
    pub const fn quote(self) -> Currency {
        match self {
            Self::UsdKrw => Currency::Krw,
            Self::KrwUsd => Currency::Usd,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FxRateKind {
    #[default]
    Mid,
    Buy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxSource {
    TossOpenApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxRateError {
    InvalidInput(NumericError),
    NonPositive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxRate {
    quote_per_base: Price,
    pair: FxPair,
    kind: FxRateKind,
    validity: TimeInterval,
    source: FxSource,
}

impl FxRate {
    pub fn parse(
        input: &NumericInput,
        pair: FxPair,
        kind: FxRateKind,
        validity: TimeInterval,
        source: FxSource,
    ) -> Result<Self, FxRateError> {
        let quote_per_base = Price::parse(input, pair.quote()).map_err(|error| match error {
            NumericError::NonPositivePrice => FxRateError::NonPositive,
            NumericError::TooLong
            | NumericError::InvalidSyntax
            | NumericError::NotRepresentable
            | NumericError::NegativeQuantity => FxRateError::InvalidInput(error),
        })?;
        Ok(Self {
            quote_per_base,
            pair,
            kind,
            validity,
            source,
        })
    }

    #[must_use]
    pub const fn pair(self) -> FxPair {
        self.pair
    }
    #[must_use]
    pub const fn kind(self) -> FxRateKind {
        self.kind
    }
    #[must_use]
    pub const fn as_of(self) -> Timestamp {
        self.validity.start()
    }
    #[must_use]
    pub const fn source(self) -> FxSource {
        self.source
    }
    #[must_use]
    pub const fn quote_per_base(self) -> Price {
        self.quote_per_base
    }

    #[must_use]
    pub const fn valid_until(self) -> Timestamp {
        self.validity.end()
    }

    #[must_use]
    pub fn validity_at(self, at: Timestamp) -> FxValidity {
        if at >= self.as_of() && at < self.valid_until() {
            FxValidity::Valid
        } else {
            FxValidity::OutsideInterval
        }
    }

    pub fn conservative_with(self, other: Self) -> Result<Self, FxConversionError> {
        if self.pair != other.pair {
            return Err(FxConversionError::PairMismatch);
        }
        if self.quote_per_base.as_money().amount().as_decimal()
            < other.quote_per_base.as_money().amount().as_decimal()
        {
            Ok(other)
        } else {
            Ok(self)
        }
    }

    pub fn convert_for_order(
        self,
        money: Money,
        at: Timestamp,
    ) -> Result<ConvertedMoney, FxConversionError> {
        if self.validity_at(at) != FxValidity::Valid {
            return Err(FxConversionError::OutsideValidity);
        }
        self.convert_for_historical_valuation(money)
    }

    pub fn convert_for_historical_valuation(
        self,
        money: Money,
    ) -> Result<ConvertedMoney, FxConversionError> {
        if self.pair != FxPair::UsdKrw {
            return Err(FxConversionError::UnsupportedDirection);
        }
        if money.currency() != self.pair.base() {
            return Err(FxConversionError::CurrencyMismatch);
        }
        crate::exact_arithmetic::multiply(money.amount(), self.quote_per_base.as_money().amount())
            .map(|amount| {
                ConvertedMoney::from_conversion(Money::new(amount, self.pair.quote()), self)
            })
            .map_err(FxConversionError::Arithmetic)
    }

    pub fn convert_for_aggregation(
        self,
        money: Money,
    ) -> Result<MidConvertedMoney, FxConversionError> {
        if self.kind != FxRateKind::Mid {
            return Err(FxConversionError::MidRequired);
        }
        self.convert_for_historical_valuation(money)
            .map(MidConvertedMoney::from_mid_conversion)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxValidity {
    Valid,
    OutsideInterval,
}

#[cfg(test)]
mod tests {
    use crate::{
        Amount, AmountInput, Currency, FxConversionError, FxPair, FxRate, FxRateError, FxRateKind,
        FxSource, Money, MoneyError, Nanosecond, NumericError, NumericInput, OffsetSeconds,
        TimeInterval, Timestamp, UnixSeconds, ValuationAccuracy,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn timestamp(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn rate(text: &str, kind: FxRateKind, seconds: i64) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(text).expect("길이"),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(
                timestamp(seconds),
                timestamp(seconds.checked_add(60).expect("유효 종료")),
            )
            .expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(text)).expect("금액"),
            currency,
        )
    }

    #[test]
    fn fx_parser_preserves_positive_values_and_rejects_invalid_inputs() {
        for text in ["0", "-0.00", "-1", "-0.0000000000000000000000000001"] {
            assert_eq!(
                FxRate::parse(
                    &NumericInput::try_new(text).expect("길이"),
                    FxPair::UsdKrw,
                    FxRateKind::Mid,
                    TimeInterval::try_new(timestamp(0), timestamp(60)).expect("구간"),
                    FxSource::TossOpenApi
                ),
                Err(FxRateError::NonPositive)
            );
        }
        for (text, error) in [
            ("", NumericError::InvalidSyntax),
            ("+1", NumericError::InvalidSyntax),
            ("1e3", NumericError::InvalidSyntax),
            ("1_000", NumericError::InvalidSyntax),
            (" 1", NumericError::InvalidSyntax),
            ("１２", NumericError::InvalidSyntax),
            ("1.", NumericError::InvalidSyntax),
            (".1", NumericError::InvalidSyntax),
            (
                "0.00000000000000000000000000001",
                NumericError::NotRepresentable,
            ),
            (
                "79228162514264337593543950336",
                NumericError::NotRepresentable,
            ),
        ] {
            assert_eq!(
                FxRate::parse(
                    &NumericInput::try_new(text).expect("길이"),
                    FxPair::UsdKrw,
                    FxRateKind::Mid,
                    TimeInterval::try_new(timestamp(0), timestamp(60)).expect("구간"),
                    FxSource::TossOpenApi
                ),
                Err(FxRateError::InvalidInput(error))
            );
        }
        assert_eq!(
            NumericInput::try_new(&"1".repeat(257)),
            Err(NumericError::TooLong)
        );
        let maximum_length = format!("{}1", "0".repeat(255));
        for text in [
            "0.0000000000000000000000000001",
            "79228162514264337593543950335",
            &maximum_length,
        ] {
            assert_eq!(
                rate(text, FxRateKind::Mid, 0).quote_per_base().as_money(),
                money(text, Currency::Krw)
            );
        }
    }

    #[test]
    fn r11_example_7_uses_mid_and_keeps_context_and_approximate_valuation() {
        assert_eq!(FxRateKind::default(), FxRateKind::Mid);
        let fx = rate("1375", FxRateKind::Mid, 100);
        assert_eq!(fx.pair(), FxPair::UsdKrw);
        assert_eq!(fx.pair().base(), Currency::Usd);
        assert_eq!(fx.pair().quote(), Currency::Krw);
        assert_eq!(fx.kind(), FxRateKind::Mid);
        assert_eq!(fx.as_of(), timestamp(100));
        assert_eq!(fx.source(), FxSource::TossOpenApi);
        let converted = fx
            .convert_for_aggregation(money("5000.00", Currency::Usd))
            .expect("환산");
        assert_eq!(converted.into_parts().1, fx);
        assert_eq!(converted.into_parts().2, ValuationAccuracy::Approximate);
        let total =
            crate::KrwValuation::from_component(crate::KrwValuationComponent::Converted(converted))
                .checked_add(&crate::KrwValuation::from_component(
                    crate::KrwValuationComponent::Unconverted(
                        crate::UnconvertedKrw::parse(
                            &NumericInput::try_new("10000000").expect("입력"),
                        )
                        .expect("원화"),
                    ),
                ))
                .expect("합산");
        assert_eq!(
            total.into_parts(),
            (
                money("16875000", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
    }

    #[test]
    fn buy_is_explicit_and_reverse_or_same_currency_conversion_is_rejected() {
        let fx = rate("1380.5", FxRateKind::Buy, 200);
        assert_eq!(fx.kind(), FxRateKind::Buy);
        assert_eq!(
            fx.convert_for_aggregation(money("2", Currency::Usd)),
            Err(FxConversionError::MidRequired)
        );
        let converted = fx
            .convert_for_historical_valuation(money("2", Currency::Usd))
            .expect("명시적 매수 환율");
        assert_eq!(
            converted.into_parts(),
            (
                money("2761", Currency::Krw),
                fx,
                ValuationAccuracy::Approximate
            )
        );
        for kind in [FxRateKind::Mid, FxRateKind::Buy] {
            let checked_rate = rate("1375", kind, 0);
            assert_eq!(
                checked_rate.convert_for_historical_valuation(money("100", Currency::Krw)),
                Err(FxConversionError::CurrencyMismatch)
            );
            assert_eq!(
                money("1", Currency::Usd).checked_add(&money("1", Currency::Krw)),
                Err(MoneyError::CurrencyMismatch)
            );
        }
    }

    #[test]
    fn conversion_never_rounds_precision_loss_or_overflow_into_money() {
        for (value, fx, expected) in [
            (
                "0.0000000000000000000000000001",
                "1.1",
                MoneyError::PrecisionLoss,
            ),
            ("79228162514264337593543950335", "2", MoneyError::Overflow),
        ] {
            assert_eq!(
                rate(fx, FxRateKind::Mid, 0)
                    .convert_for_historical_valuation(money(value, Currency::Usd)),
                Err(FxConversionError::Arithmetic(expected))
            );
        }
    }

    #[test]
    fn realistic_eight_decimal_inputs_and_signed_zero_are_exact() {
        let fx = rate("1375.12345678", FxRateKind::Mid, 123);
        assert_eq!(
            fx.convert_for_historical_valuation(money("123456789.12345678", Currency::Usd))
                .expect("정확")
                .into_parts()
                .0,
            money("169768326622.4073934965279684", Currency::Krw)
        );
        for (input, expected) in [("0", "0"), ("-0.00", "0"), ("-2", "-2750.24691356")] {
            assert_eq!(
                fx.convert_for_historical_valuation(money(input, Currency::Usd))
                    .expect("정확")
                    .into_parts(),
                (
                    money(expected, Currency::Krw),
                    fx,
                    ValuationAccuracy::Approximate
                )
            );
        }
    }

    #[test]
    fn independent_integer_product_and_metadata_hold_for_4096_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    -1_000_000_000_i64..=1_000_000_000,
                    1_i64..=99_999_999,
                    0_u32..=8,
                    0_u32..=8,
                    0_i64..=2_000_000_000,
                    proptest::arbitrary::any::<bool>(),
                ),
                |(amount, coefficient, amount_scale, rate_scale, seconds, buy)| {
                    let kind = if buy {
                        FxRateKind::Buy
                    } else {
                        FxRateKind::Mid
                    };
                    let fx = rate(
                        &Decimal::new(coefficient, rate_scale).to_string(),
                        kind,
                        seconds,
                    );
                    let input = Money::new(
                        Amount::new(Decimal::new(amount, amount_scale)),
                        Currency::Usd,
                    );
                    let expected_coefficient = i128::from(amount)
                        .checked_mul(i128::from(coefficient))
                        .expect("정수 곱");
                    let expected = Money::new(
                        Amount::new(Decimal::from_i128_with_scale(
                            expected_coefficient,
                            amount_scale.checked_add(rate_scale).expect("스케일"),
                        )),
                        Currency::Krw,
                    );
                    let result = fx
                        .convert_for_historical_valuation(input)
                        .expect("정확한 정수 기준 곱");
                    proptest::prop_assert_eq!(result.rate(), fx);
                    proptest::prop_assert_eq!(
                        result.into_parts(),
                        (expected, fx, ValuationAccuracy::Approximate)
                    );
                    Ok(())
                },
            )
            .expect("독립 정수 기준");
    }
}
