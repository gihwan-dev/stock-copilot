use rust_decimal::Decimal;

use crate::{NumericError, NumericInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(Decimal);

impl Quantity {
    pub fn try_new(value: Decimal) -> Result<Self, NumericError> {
        if value < Decimal::ZERO {
            return Err(NumericError::NegativeQuantity);
        }
        Ok(Self(value))
    }

    pub fn parse(input: &NumericInput) -> Result<Self, NumericError> {
        Self::try_new(input.parse_amount()?.as_decimal())
    }

    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::{NumericError, NumericInput, Quantity};
    use rust_decimal::Decimal;

    #[test]
    fn quantity_accepts_zero_and_fractional_values_but_rejects_negatives() {
        for (text, expected) in [
            ("0", Decimal::ZERO),
            ("-0.00", Decimal::ZERO),
            ("00.125", Decimal::new(125, 3)),
            ("0.0000001", Decimal::new(1, 7)),
            ("0.0000000000000000000000000001", Decimal::new(1, 28)),
            ("79228162514264337593543950335", Decimal::MAX),
        ] {
            assert_eq!(
                Quantity::parse(&NumericInput::try_new(text).expect("길이"))
                    .expect("수량")
                    .as_decimal(),
                expected
            );
            assert_eq!(
                Quantity::try_new(expected).expect("내부 수량").as_decimal(),
                expected
            );
        }
        for value in [Decimal::MIN, Decimal::NEGATIVE_ONE, Decimal::new(-1, 28)] {
            assert_eq!(
                Quantity::try_new(value),
                Err(NumericError::NegativeQuantity)
            );
        }
    }
}
