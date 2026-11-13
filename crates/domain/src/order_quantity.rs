use crate::Quantity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderValueError {
    NonPositive,
    FractionalQuantity,
    QuantityScale,
    CurrencyMismatch,
    PricePrecision,
    AmountPrecision,
    NumericValue(crate::NumericError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WholeOrderQuantity(Quantity);

impl WholeOrderQuantity {
    pub fn try_new(value: Quantity) -> Result<Self, OrderValueError> {
        if value.as_decimal().is_zero() {
            return Err(OrderValueError::NonPositive);
        }
        if value.as_decimal().normalize().scale() != 0 {
            return Err(OrderValueError::FractionalQuantity);
        }
        Quantity::try_new(value.as_decimal().normalize())
            .map(Self)
            .map_err(OrderValueError::NumericValue)
    }

    #[must_use]
    pub fn quantity(self) -> Quantity {
        self.0
    }

    #[must_use]
    pub fn wire_value(self) -> OrderWireValue {
        OrderWireValue::from_amount(crate::Amount::new(self.0.as_decimal()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellOrderQuantity(Quantity);

impl SellOrderQuantity {
    pub fn try_new(value: Quantity) -> Result<Self, OrderValueError> {
        if value.as_decimal().is_zero() {
            return Err(OrderValueError::NonPositive);
        }
        if value.as_decimal().normalize().scale() > 6 {
            return Err(OrderValueError::QuantityScale);
        }
        Quantity::try_new(value.as_decimal().normalize())
            .map(Self)
            .map_err(OrderValueError::NumericValue)
    }

    #[must_use]
    pub fn quantity(self) -> Quantity {
        self.0
    }

    #[must_use]
    pub fn wire_value(self) -> OrderWireValue {
        OrderWireValue::from_amount(crate::Amount::new(self.0.as_decimal()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderWireValue(String);

impl OrderWireValue {
    pub(crate) fn from_amount(value: crate::Amount) -> Self {
        Self(value.as_decimal().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{OrderValueError, Quantity, SellOrderQuantity, WholeOrderQuantity};
    use crate::{NumericError, NumericInput};
    use proptest::test_runner::{Config, TestRunner};

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("길이")).expect("수량")
    }

    #[test]
    fn quantity_boundaries_and_wire_length() {
        for text in ["0", "0.0"] {
            assert_eq!(
                WholeOrderQuantity::try_new(quantity(text)),
                Err(OrderValueError::NonPositive)
            );
            assert_eq!(
                SellOrderQuantity::try_new(quantity(text)),
                Err(OrderValueError::NonPositive)
            );
        }
        assert_eq!(
            Quantity::parse(&NumericInput::try_new("-1").expect("길이")),
            Err(NumericError::NegativeQuantity)
        );
        for text in ["1", "1.0000000", "79228162514264337593543950335"] {
            assert_eq!(
                WholeOrderQuantity::try_new(quantity(text))
                    .expect("정수")
                    .quantity(),
                quantity(text)
            );
            assert_eq!(
                SellOrderQuantity::try_new(quantity(text))
                    .expect("매도")
                    .quantity(),
                quantity(text)
            );
        }
        for text in ["0.00001", "0.000001", "1.123456"] {
            assert_eq!(
                WholeOrderQuantity::try_new(quantity(text)),
                Err(OrderValueError::FractionalQuantity)
            );
            assert_eq!(
                SellOrderQuantity::try_new(quantity(text))
                    .expect("소수 매도")
                    .quantity(),
                quantity(text)
            );
        }
        assert_eq!(
            SellOrderQuantity::try_new(quantity("0.0000001")),
            Err(OrderValueError::QuantityScale)
        );
        assert_eq!(
            SellOrderQuantity::try_new(quantity("12345678901234567890123.123456"))
                .expect("30자")
                .quantity(),
            quantity("12345678901234567890123.123456")
        );
    }

    #[test]
    fn thirty_character_quantity_is_the_actual_wire_value() {
        let value =
            SellOrderQuantity::try_new(quantity("12345678901234567890123.123456")).expect("수량");
        assert_eq!(
            value.wire_value().as_str(),
            "12345678901234567890123.123456"
        );
        assert_eq!(value.wire_value().as_str().len(), 30);
    }

    #[test]
    fn quantity_matches_independent_integer_table() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u32..1_000_000, 0_u32..=8), |(coefficient, scale)| {
                let value =
                    rust_decimal::Decimal::from_i128_with_scale(i128::from(coefficient), scale);
                let quantity = Quantity::try_new(value).expect("일반 수량");
                let denominator = 10_u64.pow(scale);
                let integer = u64::from(coefficient) % denominator == 0;
                let six_places = scale <= 6 || u64::from(coefficient) % 10_u64.pow(scale - 6) == 0;
                proptest::prop_assert_eq!(
                    WholeOrderQuantity::try_new(quantity).is_ok(),
                    coefficient > 0 && integer
                );
                proptest::prop_assert_eq!(
                    SellOrderQuantity::try_new(quantity).is_ok(),
                    coefficient > 0 && six_places
                );
                Ok(())
            })
            .expect("독립 기준");
    }

    mod review_regressions {
        use crate::{NumericInput, Quantity, SellOrderQuantity, WholeOrderQuantity};

        #[test]
        fn quantities_store_normalized_values_and_expose_canonical_wire_strings() {
            let whole = WholeOrderQuantity::try_new(
                Quantity::parse(&NumericInput::try_new("1.000000").expect("입력")).expect("수량"),
            )
            .expect("주문");
            assert_eq!(whole.quantity().as_decimal().scale(), 0);
            assert_eq!(whole.wire_value().as_str(), "1");
            let sell = SellOrderQuantity::try_new(
                Quantity::parse(&NumericInput::try_new("0.0000010").expect("입력")).expect("수량"),
            )
            .expect("주문");
            assert_eq!(sell.quantity().as_decimal().scale(), 6);
            assert_eq!(sell.wire_value().as_str(), "0.000001");
        }
    }
}
