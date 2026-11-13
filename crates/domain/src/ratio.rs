use rust_decimal::Decimal;

use crate::{NumericError, NumericInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ratio(Decimal);

impl Ratio {
    #[must_use]
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    pub fn parse(input: &NumericInput) -> Result<Self, NumericError> {
        input
            .parse_amount()
            .map(|amount| Self::new(amount.as_decimal()))
    }

    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::{NumericInput, Ratio};
    use rust_decimal::Decimal;

    #[test]
    fn ratio_is_a_signed_decimal_fraction_not_a_probability() {
        for (text, expected) in [
            ("-79228162514264337593543950335", Decimal::MIN),
            ("-1.5", Decimal::new(-15, 1)),
            ("-0", Decimal::ZERO),
            ("1", Decimal::ONE),
            ("1.5", Decimal::new(15, 1)),
            ("0.1516", Decimal::new(1516, 4)),
            ("79228162514264337593543950335", Decimal::MAX),
        ] {
            assert_eq!(
                Ratio::parse(&NumericInput::try_new(text).expect("길이"))
                    .expect("비율")
                    .as_decimal(),
                expected
            );
        }
    }
}
