use crate::{Amount, Currency, Money, OrderValueError, OrderWireValue, Price};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KrOrderPrice(Price);

impl KrOrderPrice {
    pub fn try_new(value: Price) -> Result<Self, OrderValueError> {
        if value.as_money().currency() != Currency::Krw {
            return Err(OrderValueError::CurrencyMismatch);
        }
        if value.as_money().amount().as_decimal().normalize().scale() != 0 {
            return Err(OrderValueError::PricePrecision);
        }
        Price::try_new(Money::new(
            Amount::new(value.as_money().amount().as_decimal().normalize()),
            value.as_money().currency(),
        ))
        .map(Self)
        .map_err(OrderValueError::NumericValue)
    }

    #[must_use]
    pub fn price(self) -> Price {
        self.0
    }

    #[must_use]
    pub fn wire_value(self) -> OrderWireValue {
        OrderWireValue::from_amount(self.0.as_money().amount())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsOrderPrice(Price);

impl UsOrderPrice {
    pub fn try_new(value: Price) -> Result<Self, OrderValueError> {
        if value.as_money().currency() != Currency::Usd {
            return Err(OrderValueError::CurrencyMismatch);
        }
        let maximum_scale = match value
            .as_money()
            .amount()
            .as_decimal()
            .cmp(&rust_decimal::Decimal::ONE)
        {
            Ordering::Less => 4,
            Ordering::Equal | Ordering::Greater => 2,
        };
        if value.as_money().amount().as_decimal().normalize().scale() > maximum_scale {
            return Err(OrderValueError::PricePrecision);
        }
        Price::try_new(Money::new(
            Amount::new(value.as_money().amount().as_decimal().normalize()),
            value.as_money().currency(),
        ))
        .map(Self)
        .map_err(OrderValueError::NumericValue)
    }

    #[must_use]
    pub fn price(self) -> Price {
        self.0
    }

    #[must_use]
    pub fn wire_value(self) -> OrderWireValue {
        OrderWireValue::from_amount(self.0.as_money().amount())
    }
}

#[cfg(test)]
mod tests {
    use super::{Currency, KrOrderPrice, OrderValueError, Price, UsOrderPrice};
    use crate::NumericInput;

    fn price(text: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("길이"), currency).expect("가격")
    }

    #[test]
    fn prices_require_market_currency_and_exact_api_precision() {
        for text in ["1", "70000", "79228162514264337593543950335"] {
            let value = price(text, Currency::Krw);
            assert_eq!(KrOrderPrice::try_new(value).expect("원화").price(), value);
            assert_eq!(
                UsOrderPrice::try_new(value),
                Err(OrderValueError::CurrencyMismatch)
            );
        }
        for text in ["0.0001", "0.9999", "1", "1.01", "100.12000"] {
            let value = price(text, Currency::Usd);
            assert_eq!(UsOrderPrice::try_new(value).expect("달러").price(), value);
            assert_eq!(
                KrOrderPrice::try_new(value),
                Err(OrderValueError::CurrencyMismatch)
            );
        }
        assert_eq!(
            KrOrderPrice::try_new(price("1.1", Currency::Krw)),
            Err(OrderValueError::PricePrecision)
        );
        for text in ["0.00001", "1.001"] {
            assert_eq!(
                UsOrderPrice::try_new(price(text, Currency::Usd)),
                Err(OrderValueError::PricePrecision)
            );
        }
    }
    #[test]
    fn prices_match_independent_integer_tick_table() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u32..=200_000, 0_u32..=6), |(coefficient, scale)| {
                let denominator = 10_u64.pow(scale);
                let integer = u64::from(coefficient) % denominator == 0;
                let tick_places = if u64::from(coefficient) < denominator {
                    4
                } else {
                    2
                };
                let valid_us = scale <= tick_places
                    || u64::from(coefficient) % 10_u64.pow(scale - tick_places) == 0;
                let amount = crate::Amount::parse(&crate::AmountInput::new(
                    &rust_decimal::Decimal::from_i128_with_scale(i128::from(coefficient), scale)
                        .to_string(),
                ))
                .expect("금액");
                let kr = Price::try_new(crate::Money::new(amount, Currency::Krw)).expect("가격");
                let us = Price::try_new(crate::Money::new(amount, Currency::Usd)).expect("가격");
                proptest::prop_assert_eq!(KrOrderPrice::try_new(kr).is_ok(), integer);
                proptest::prop_assert_eq!(UsOrderPrice::try_new(us).is_ok(), valid_us);
                Ok(())
            })
            .expect("독립 호가 정밀도 표");
    }

    mod review_regressions {
        use crate::{Currency, KrOrderPrice, NumericInput, Price, UsOrderPrice};

        #[test]
        fn prices_store_normalized_values_and_expose_canonical_wire_strings() {
            let kr = KrOrderPrice::try_new(
                Price::parse(
                    &NumericInput::try_new("70000.000").expect("입력"),
                    Currency::Krw,
                )
                .expect("가격"),
            )
            .expect("주문");
            assert_eq!(kr.price().as_money().amount().as_decimal().scale(), 0);
            assert_eq!(kr.wire_value().as_str(), "70000");
            let us = UsOrderPrice::try_new(
                Price::parse(
                    &NumericInput::try_new("0.123400").expect("입력"),
                    Currency::Usd,
                )
                .expect("가격"),
            )
            .expect("주문");
            assert_eq!(us.price().as_money().amount().as_decimal().scale(), 4);
            assert_eq!(us.wire_value().as_str(), "0.1234");
        }
    }
}
