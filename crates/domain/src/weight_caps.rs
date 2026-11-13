use crate::{NumericError, NumericInput, Ratio};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileWeight(Ratio);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileWeightError {
    Numeric(NumericError),
    OutOfRange,
    NotRepresentable,
}

impl ProfileWeight {
    pub fn try_new(value: Ratio) -> Result<Self, ProfileWeightError> {
        if value.as_decimal() <= Decimal::ZERO || value.as_decimal() > Decimal::ONE {
            return Err(ProfileWeightError::OutOfRange);
        }
        Ok(Self(value))
    }

    pub fn parse(input: &NumericInput) -> Result<Self, ProfileWeightError> {
        Self::try_new(Ratio::parse(input).map_err(ProfileWeightError::Numeric)?)
    }

    pub fn parse_percent(input: &NumericInput) -> Result<Self, ProfileWeightError> {
        let percent = Ratio::parse(input)
            .map_err(ProfileWeightError::Numeric)?
            .as_decimal();
        if percent <= Decimal::ZERO || percent > Decimal::ONE_HUNDRED {
            return Err(ProfileWeightError::OutOfRange);
        }
        let normalized = percent.normalize();
        Decimal::try_from_i128_with_scale(
            normalized.mantissa(),
            normalized.scale().saturating_add(2),
        )
        .map(|value| Self(Ratio::new(value)))
        .map_err(|_error| ProfileWeightError::NotRepresentable)
    }

    #[must_use]
    pub fn as_ratio(self) -> Ratio {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightCaps {
    instrument_cap: ProfileWeight,
    instrument_warning: ProfileWeight,
    sector_cap: ProfileWeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightCapsError {
    WarningExceedsInstrument,
    InstrumentExceedsSector,
}

impl WeightCaps {
    pub fn try_new(
        instrument_cap: ProfileWeight,
        instrument_warning: ProfileWeight,
        sector_cap: ProfileWeight,
    ) -> Result<Self, WeightCapsError> {
        if instrument_warning > instrument_cap {
            return Err(WeightCapsError::WarningExceedsInstrument);
        }
        if instrument_cap > sector_cap {
            return Err(WeightCapsError::InstrumentExceedsSector);
        }
        Ok(Self {
            instrument_cap,
            instrument_warning,
            sector_cap,
        })
    }

    #[must_use]
    pub fn instrument_cap(self) -> ProfileWeight {
        self.instrument_cap
    }
    #[must_use]
    pub fn instrument_warning(self) -> ProfileWeight {
        self.instrument_warning
    }
    #[must_use]
    pub fn sector_cap(self) -> ProfileWeight {
        self.sector_cap
    }
}

impl Default for WeightCaps {
    fn default() -> Self {
        Self {
            instrument_cap: ProfileWeight(Ratio::new(Decimal::new(2, 1))),
            instrument_warning: ProfileWeight(Ratio::new(Decimal::new(1, 1))),
            sector_cap: ProfileWeight(Ratio::new(Decimal::new(4, 1))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{NumericInput, ProfileWeight, ProfileWeightError, WeightCaps, WeightCapsError};
    use proptest::test_runner::{Config, TestRunner};

    fn weight(text: &str) -> ProfileWeight {
        ProfileWeight::parse(&NumericInput::try_new(text).expect("입력 길이")).expect("비율")
    }

    #[test]
    fn default_caps_are_exact_design_percentages() {
        let caps = WeightCaps::default();
        assert_eq!(caps.instrument_cap(), weight("0.2"));
        assert_eq!(caps.instrument_warning(), weight("0.1"));
        assert_eq!(caps.sector_cap(), weight("0.4"));
    }

    #[test]
    fn weights_reject_zero_outside_range_and_noncanonical_input() {
        for text in [
            "0",
            "-0",
            "-0.0000000000000000000000000001",
            "1.0000000000000000000000000001",
            "2",
        ] {
            assert_eq!(
                ProfileWeight::parse(&NumericInput::try_new(text).expect("길이")),
                Err(ProfileWeightError::OutOfRange)
            );
        }
        for text in [
            "",
            " 1",
            "+1",
            "1e0",
            "1_0",
            "10%",
            "１",
            "1.",
            ".1",
            "0.00000000000000000000000000001",
        ] {
            ProfileWeight::parse(&NumericInput::try_new(text).expect("길이"))
                .expect_err("정규형 거부");
            ProfileWeight::parse_percent(&NumericInput::try_new(text).expect("길이"))
                .expect_err("정규형 거부");
        }
        assert_eq!(weight("1").as_ratio().as_decimal().to_string(), "1");
        assert_eq!(
            weight("0.0000000000000000000000000001")
                .as_ratio()
                .as_decimal()
                .to_string(),
            "0.0000000000000000000000000001"
        );
        for text in ["0", "-1", "100.00000000000000000000000001", "101"] {
            assert_eq!(
                ProfileWeight::parse_percent(&NumericInput::try_new(text).expect("길이")),
                Err(ProfileWeightError::OutOfRange)
            );
        }
        assert_eq!(
            ProfileWeight::parse_percent(
                &NumericInput::try_new("0.0000000000000000000000000001").expect("길이")
            ),
            Err(ProfileWeightError::NotRepresentable)
        );
        assert_eq!(
            ProfileWeight::parse_percent(&NumericInput::try_new("00020.00").expect("길이"))
                .expect("퍼센트"),
            weight("0.2")
        );
        assert_eq!(
            ProfileWeight::parse_percent(&NumericInput::try_new("100").expect("길이"))
                .expect("퍼센트"),
            weight("1")
        );
    }

    #[test]
    fn invalid_combinations_fail_and_equal_boundaries_pass() {
        assert_eq!(
            WeightCaps::try_new(weight("0.2"), weight("0.3"), weight("0.4")),
            Err(WeightCapsError::WarningExceedsInstrument)
        );
        assert_eq!(
            WeightCaps::try_new(weight("0.4"), weight("0.1"), weight("0.3")),
            Err(WeightCapsError::InstrumentExceedsSector)
        );
        let caps = WeightCaps::try_new(weight("1"), weight("1"), weight("1")).expect("동일 경계");
        assert_eq!(caps.instrument_cap(), weight("1"));
    }

    #[test]
    fn caps_and_percent_parsing_match_independent_integer_reference() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_i64..=10000, 1_i64..=10000, 1_i64..=10000),
                |(cap, warning, sector)| {
                    let values = [cap, warning, sector].map(|value| {
                        ProfileWeight::parse_percent(
                            &NumericInput::try_new(&format!(
                                "{}.{:02}",
                                value.div_euclid(100),
                                value.rem_euclid(100)
                            ))
                            .expect("길이"),
                        )
                        .expect("퍼센트")
                    });
                    let [instrument_cap, instrument_warning, sector_cap] = values;
                    proptest::prop_assert_eq!(
                        instrument_cap.as_ratio().as_decimal(),
                        rust_decimal::Decimal::new(cap, 4)
                    );
                    let actual =
                        WeightCaps::try_new(instrument_cap, instrument_warning, sector_cap);
                    proptest::prop_assert_eq!(actual.is_ok(), warning <= cap && cap <= sector);
                    if let Ok(caps) = actual {
                        proptest::prop_assert_eq!(caps.instrument_cap(), instrument_cap);
                        proptest::prop_assert_eq!(caps.instrument_warning(), instrument_warning);
                        proptest::prop_assert_eq!(caps.sector_cap(), sector_cap);
                    }
                    Ok(())
                },
            )
            .expect("독립 정수 기준");
    }
    #[test]
    fn arbitrary_ratio_range_matches_signed_integer_reference() {
        use proptest::arbitrary::any;
        use proptest::strategy::Strategy;
        let strategy = (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<bool>(),
            0_u32..=28,
        )
            .prop_map(|(lo, mid, hi, negative, scale)| {
                rust_decimal::Decimal::from_parts(lo, mid, hi, negative, scale)
            });
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |value| {
                let parts = value.unpack();
                let coefficient = u128::from(parts.lo)
                    | (u128::from(parts.mid) << 32_u32)
                    | (u128::from(parts.hi) << 64_u32);
                let expected =
                    !parts.negative && coefficient > 0 && coefficient <= 10_u128.pow(parts.scale);
                let actual = ProfileWeight::try_new(crate::Ratio::new(value));
                proptest::prop_assert_eq!(actual.is_ok(), expected);
                if let Ok(weight) = actual {
                    proptest::prop_assert_eq!(weight.as_ratio().as_decimal(), value);
                }
                Ok(())
            })
            .expect("독립 정수 범위");
    }
}
