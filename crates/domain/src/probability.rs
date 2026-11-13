use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Probability(Decimal);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbabilityError {
    OutOfRange,
}

impl Probability {
    pub fn try_new(value: Decimal) -> Result<Self, ProbabilityError> {
        if !(Decimal::ZERO..=Decimal::ONE).contains(&value) {
            return Err(ProbabilityError::OutOfRange);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiProbability(Probability);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProbabilityInput(String);

impl AiProbabilityInput {
    pub fn try_new(value: &str) -> Result<Self, AiProbabilityError> {
        if !(1..=2).contains(&value.len()) {
            return Err(AiProbabilityError::InvalidLength);
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProbabilityError {
    InvalidLength,
    InvalidPercent,
}

impl AiProbability {
    pub fn parse(input: &AiProbabilityInput) -> Result<Self, AiProbabilityError> {
        if input.0.starts_with('0') || !input.0.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AiProbabilityError::InvalidPercent);
        }
        input
            .0
            .parse::<i64>()
            .map(|percent| Self(Probability(Decimal::new(percent, 2))))
            .map_err(|_error| AiProbabilityError::InvalidPercent)
    }

    #[must_use]
    pub fn into_probability(self) -> Probability {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AiProbability, AiProbabilityError, AiProbabilityInput, Probability, ProbabilityError,
    };
    use proptest::arbitrary::any;
    use proptest::strategy::Strategy;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn parse_ai(text: &str) -> Result<AiProbability, AiProbabilityError> {
        let input = AiProbabilityInput::try_new(text)?;
        AiProbability::parse(&input)
    }

    #[test]
    fn all_ai_integer_percentages_preserve_exact_values() {
        for percent in 1_i64..=99 {
            let text = percent.to_string();
            let estimate = parse_ai(&text).expect("1~99 정수 퍼센트");
            let probability = estimate.into_probability();
            assert_eq!(
                probability.as_decimal().to_string(),
                format!("0.{percent:02}")
            );
        }
    }

    #[test]
    fn ai_rejects_noncanonical_inputs_and_checks_length_first() {
        for text in ["", "100", "50.5", "50%", "050", " 50", "５０", "5e1", "999"] {
            assert_eq!(parse_ai(text), Err(AiProbabilityError::InvalidLength));
        }
        for text in [
            "0", "00", "01", "+1", "+50", "-1", " 1", "1 ", "1\n", "a", "1a", "é", "١",
        ] {
            let expected = if text.len() > 2 {
                AiProbabilityError::InvalidLength
            } else {
                AiProbabilityError::InvalidPercent
            };
            assert_eq!(parse_ai(text), Err(expected));
        }
        let text = "9".repeat(1_000_000);
        assert_eq!(parse_ai(&text), Err(AiProbabilityError::InvalidLength));
    }

    #[test]
    fn calculated_probabilities_include_endpoints_and_preserve_precision() {
        for value in [
            Decimal::ZERO,
            Decimal::ONE,
            Decimal::new(1, 28),
            Decimal::from_i128_with_scale(9_999_999_999_999_999_999_999_999_999, 28),
            Decimal::new(2475, 4),
        ] {
            assert_eq!(
                Probability::try_new(value)
                    .expect("닫힌 구간 확률")
                    .as_decimal(),
                value
            );
        }
        for value in [
            Decimal::MIN,
            Decimal::new(-1, 28),
            Decimal::from_i128_with_scale(10_000_000_000_000_000_000_000_000_001, 28),
            Decimal::MAX,
        ] {
            assert_eq!(
                Probability::try_new(value),
                Err(ProbabilityError::OutOfRange)
            );
        }
    }

    #[test]
    fn decimal_range_matches_an_independent_integer_reference() {
        let arbitrary = (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<bool>(),
            0_u32..=28,
        );
        let small = (-20_000_i64..=20_000).prop_map(|coefficient| {
            let value = Decimal::new(coefficient, 4);
            value.unpack()
        });
        let decimals = proptest::prop_oneof![
            arbitrary.prop_map(|(low, mid, high, negative, scale)| Decimal::from_parts(
                low, mid, high, negative, scale
            )),
            small.prop_map(|parts| Decimal::from_parts(
                parts.lo,
                parts.mid,
                parts.hi,
                parts.negative,
                parts.scale
            )),
        ];
        TestRunner::new(Config::with_cases(4096))
            .run(&decimals, |value| {
                let parts = value.unpack();
                let coefficient = u128::from(parts.lo)
                    | (u128::from(parts.mid) << 32_u32)
                    | (u128::from(parts.hi) << 64_u32);
                let valid = (coefficient == 0 || !parts.negative)
                    && coefficient <= 10_u128.pow(parts.scale);
                let actual = Probability::try_new(value);
                proptest::prop_assert_eq!(actual.is_ok(), valid);
                if let Ok(probability) = actual {
                    proptest::prop_assert_eq!(probability.as_decimal(), value);
                }
                Ok(())
            })
            .expect("독립 정수 기준과 일치");
    }

    #[test]
    fn untrusted_strings_match_the_exhaustive_canonical_set() {
        let canonical: Vec<_> = (1_i64..=99).map(|value| value.to_string()).collect();
        let inputs = proptest::collection::vec(any::<u8>(), 0..128);
        TestRunner::new(Config::with_cases(4096))
            .run(&inputs, |bytes| {
                let text = String::from_utf8_lossy(&bytes);
                proptest::prop_assert_eq!(
                    parse_ai(&text).is_ok(),
                    canonical.iter().any(|candidate| candidate == &text)
                );
                Ok(())
            })
            .expect("정규 문자열 집합과 일치");
    }

    #[test]
    fn random_out_of_range_integer_percentages_fail() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(100_u64..=u64::MAX, 1_u64..=u64::MAX),
                |(positive, negative)| {
                    for text in [positive.to_string(), format!("-{negative}")] {
                        proptest::prop_assert!(parse_ai(&text).is_err());
                    }
                    Ok(())
                },
            )
            .expect("범위 밖 정수 거부");
    }

    #[test]
    fn probability_comparison_matches_integer_ordering() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_i64..=10_000, 0_i64..=10_000), |(left, right)| {
                let first = Probability::try_new(Decimal::new(left, 4)).expect("유효 확률");
                let second = Probability::try_new(Decimal::new(right, 4)).expect("유효 확률");
                proptest::prop_assert_eq!(first.cmp(&second), left.cmp(&right));
                Ok(())
            })
            .expect("정수 비교와 일치");
    }
}
