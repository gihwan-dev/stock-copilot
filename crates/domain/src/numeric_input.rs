use crate::{Amount, AmountError, AmountInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericInput(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    TooLong,
    InvalidSyntax,
    NotRepresentable,
    NegativeQuantity,
    NonPositivePrice,
}

impl NumericInput {
    pub fn try_new(value: &str) -> Result<Self, NumericError> {
        if value.len() > 256 {
            return Err(NumericError::TooLong);
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn parse_amount(&self) -> Result<Amount, NumericError> {
        Amount::parse(&AmountInput::new(&self.0)).map_err(|error| match error {
            AmountError::InvalidSyntax => NumericError::InvalidSyntax,
            AmountError::NotRepresentable => NumericError::NotRepresentable,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Amount, Currency, Money, NumericError, NumericInput, Price, Quantity, Ratio};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    #[test]
    fn input_length_is_bounded_before_copying() {
        let boundary = "0".repeat(256);
        let input = NumericInput::try_new(&boundary).expect("256바이트");
        assert_eq!(
            Ratio::parse(&input).expect("영").as_decimal(),
            Decimal::ZERO
        );
        for size in [257, 1_000_000] {
            assert_eq!(
                NumericInput::try_new(&"0".repeat(size)),
                Err(NumericError::TooLong)
            );
        }
    }

    #[test]
    fn all_parsers_reject_invalid_syntax_and_inexact_values() {
        for (texts, expected) in [
            (
                vec![
                    "", "-", ".1", "1.", "1.2.3", "1e5", "1_000", "+1", " 1", "1 ", "１２", "--1",
                ],
                NumericError::InvalidSyntax,
            ),
            (
                vec![
                    "0.00000000000000000000000000001",
                    "79228162514264337593543950336",
                    "1.00000000000000000000000000000",
                ],
                NumericError::NotRepresentable,
            ),
        ] {
            for text in texts {
                let input = NumericInput::try_new(text).expect("길이");
                assert_eq!(Quantity::parse(&input), Err(expected));
                assert_eq!(Price::parse(&input, Currency::Usd), Err(expected));
                assert_eq!(Ratio::parse(&input), Err(expected));
            }
        }
    }

    #[test]
    fn independent_integer_coefficients_preserve_exact_values_and_ranges() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    -79_228_162_514_264_337_593_543_950_335_i128
                        ..=79_228_162_514_264_337_593_543_950_335_i128,
                    0_u32..=28,
                    proptest::arbitrary::any::<bool>(),
                ),
                |(coefficient, scale, usd)| {
                    let digits = format!(
                        "{:0>width$}",
                        coefficient.abs(),
                        width = usize::try_from(scale)
                            .expect("scale")
                            .checked_add(1)
                            .expect("width")
                    );
                    let split = digits
                        .len()
                        .checked_sub(usize::try_from(scale).expect("scale"))
                        .expect("split");
                    let unsigned = if scale == 0 {
                        digits
                    } else {
                        let (whole, fraction) = digits.split_at_checked(split).expect("ASCII 경계");
                        format!("{whole}.{fraction}")
                    };
                    let text = if coefficient < 0 {
                        format!("-{unsigned}")
                    } else {
                        unsigned
                    };
                    let input = NumericInput::try_new(&text).expect("길이 제한 입력");
                    let expected = Decimal::from_i128_with_scale(coefficient, scale);
                    let currency = if usd { Currency::Usd } else { Currency::Krw };
                    proptest::prop_assert_eq!(
                        Ratio::parse(&input).expect("비율").as_decimal(),
                        expected
                    );
                    proptest::prop_assert_eq!(Ratio::new(expected).as_decimal(), expected);
                    let quantity = Quantity::parse(&input);
                    if coefficient < 0 {
                        proptest::prop_assert_eq!(quantity, Err(NumericError::NegativeQuantity));
                    } else {
                        proptest::prop_assert_eq!(quantity.expect("수량").as_decimal(), expected);
                    }
                    proptest::prop_assert_eq!(Quantity::try_new(expected), quantity);
                    let price = Price::parse(&input, currency);
                    let money = Money::new(Amount::new(expected), currency);
                    if coefficient <= 0 {
                        proptest::prop_assert_eq!(price, Err(NumericError::NonPositivePrice));
                    } else {
                        proptest::prop_assert_eq!(price.expect("가격").as_money(), money);
                    }
                    proptest::prop_assert_eq!(Price::try_new(money), price);
                    Ok(())
                },
            )
            .expect("독립 정수 계수 기준");
    }
    #[test]
    fn leading_zero_decimal_input_matches_amount_parser() {
        let text = format!("{}1.5", "0".repeat(57));
        let input = NumericInput::try_new(&text).expect("정상 길이 입력");
        assert_eq!(
            Ratio::parse(&input).expect("비율").as_decimal(),
            Decimal::new(15, 1)
        );
    }
}
