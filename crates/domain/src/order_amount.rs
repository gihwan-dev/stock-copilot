use crate::{Amount, Currency, Money, OrderValueError, OrderWireValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsdOrderAmount(Money);

impl UsdOrderAmount {
    pub fn try_new(value: Money) -> Result<Self, OrderValueError> {
        if value.currency() != Currency::Usd {
            return Err(OrderValueError::CurrencyMismatch);
        }
        if value.amount().as_decimal() <= rust_decimal::Decimal::ZERO {
            return Err(OrderValueError::NonPositive);
        }
        if value.amount().as_decimal().normalize().scale() > 2 {
            return Err(OrderValueError::AmountPrecision);
        }
        Ok(Self(Money::new(
            Amount::new(value.amount().as_decimal().normalize()),
            value.currency(),
        )))
    }

    #[must_use]
    pub fn money(self) -> Money {
        self.0
    }

    #[must_use]
    pub fn wire_value(self) -> OrderWireValue {
        OrderWireValue::from_amount(self.0.amount())
    }
}

#[cfg(test)]
mod tests {
    use super::{Currency, Money, OrderValueError, UsdOrderAmount};
    use crate::{Amount, AmountInput};

    #[test]
    fn amount_requires_positive_usd_and_canonical_wire_value() {
        for text in ["0.01", "100.5", "79228162514264337593543950335"] {
            let money = Money::new(
                Amount::parse(&AmountInput::new(text)).expect("금액"),
                Currency::Usd,
            );
            assert_eq!(
                UsdOrderAmount::try_new(money).expect("양수 달러").money(),
                money
            );
            assert_eq!(
                UsdOrderAmount::try_new(Money::new(money.amount(), Currency::Krw)),
                Err(OrderValueError::CurrencyMismatch)
            );
        }
        for text in ["0", "-1", "-0.00001"] {
            assert_eq!(
                UsdOrderAmount::try_new(Money::new(
                    Amount::parse(&AmountInput::new(text)).expect("금액"),
                    Currency::Usd
                )),
                Err(OrderValueError::NonPositive)
            );
        }
        let maximum = UsdOrderAmount::try_new(Money::new(
            Amount::parse(&AmountInput::new("79228162514264337593543950335")).expect("금액"),
            Currency::Usd,
        ))
        .expect("최대 금액");
        assert_eq!(
            maximum.wire_value().as_str(),
            "79228162514264337593543950335"
        );
    }

    mod review_regressions {
        use crate::{Amount, AmountInput, Currency, Money, OrderValueError, UsdOrderAmount};

        #[test]
        fn amount_stores_cents_and_exposes_canonical_wire_strings() {
            for (text, wire, scale) in [
                ("100.5000", "100.5", 1),
                ("1.00", "1", 0),
                ("0.0100", "0.01", 2),
                (
                    "123456789012345678901234567.89",
                    "123456789012345678901234567.89",
                    2,
                ),
            ] {
                let value = UsdOrderAmount::try_new(Money::new(
                    Amount::parse(&AmountInput::new(text)).expect("금액"),
                    Currency::Usd,
                ))
                .expect("주문");
                assert_eq!(value.money().amount().as_decimal().scale(), scale);
                assert_eq!(value.wire_value().as_str(), wire);
            }
            for text in ["0.001", "0.0000000000000000000000000001"] {
                assert_eq!(
                    UsdOrderAmount::try_new(Money::new(
                        Amount::parse(&AmountInput::new(text)).expect("금액"),
                        Currency::Usd
                    )),
                    Err(OrderValueError::AmountPrecision)
                );
            }
        }
    }
    mod property_tests {
        use crate::{Amount, Currency, Money, UsdOrderAmount};
        use proptest::test_runner::{Config, TestRunner};

        #[test]
        fn amount_precision_matches_independent_cent_table() {
            TestRunner::new(Config::with_cases(4096))
                .run(
                    &(-1_000_000_i64..=1_000_000, 0_u32..=8),
                    |(coefficient, scale)| {
                        let cents = scale <= 2 || coefficient % 10_i64.pow(scale - 2) == 0;
                        let money = Money::new(
                            Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(coefficient),
                                scale,
                            )),
                            Currency::Usd,
                        );
                        proptest::prop_assert_eq!(
                            UsdOrderAmount::try_new(money).is_ok(),
                            coefficient > 0 && cents
                        );
                        Ok(())
                    },
                )
                .expect("독립 센트 기준");
        }
    }
}
