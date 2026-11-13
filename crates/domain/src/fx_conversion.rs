use crate::{Currency, FxRate, Money, MoneyError, NumericError, NumericInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationAccuracy {
    Exact,
    Approximate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxConversionError {
    CurrencyMismatch,
    MidRequired,
    Arithmetic(MoneyError),
    TooManyRates,
    OutsideValidity,
    PairMismatch,
    UnsupportedDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertedMoney {
    money: Money,
    rate: FxRate,
}

impl ConvertedMoney {
    pub(crate) const fn from_conversion(money: Money, rate: FxRate) -> Self {
        Self { money, rate }
    }
    #[must_use]
    pub const fn rate(self) -> FxRate {
        self.rate
    }
    #[must_use]
    pub const fn into_parts(self) -> (Money, FxRate, ValuationAccuracy) {
        (self.money, self.rate, ValuationAccuracy::Approximate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidConvertedMoney(ConvertedMoney);

impl MidConvertedMoney {
    pub(crate) const fn from_mid_conversion(converted: ConvertedMoney) -> Self {
        Self(converted)
    }
    #[must_use]
    pub const fn into_parts(self) -> (Money, FxRate, ValuationAccuracy) {
        self.0.into_parts()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnconvertedKrw(Money);

impl UnconvertedKrw {
    pub fn parse(input: &NumericInput) -> Result<Self, NumericError> {
        Money::parse(input, Currency::Krw).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrwValuationComponent {
    Unconverted(UnconvertedKrw),
    Converted(MidConvertedMoney),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KrwValuation {
    money: Money,
    rates: Vec<FxRate>,
}

impl KrwValuation {
    #[must_use]
    pub fn from_component(component: KrwValuationComponent) -> Self {
        match component {
            KrwValuationComponent::Unconverted(value) => Self {
                money: value.0,
                rates: Vec::new(),
            },
            KrwValuationComponent::Converted(value) => {
                let (money, rate, _accuracy) = value.into_parts();
                Self {
                    money,
                    rates: vec![rate],
                }
            }
        }
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, FxConversionError> {
        let mut rates = self.rates.clone();
        for rate in &other.rates {
            if rates.contains(rate) {
                continue;
            }
            if rates.len() >= 64 {
                return Err(FxConversionError::TooManyRates);
            }
            rates.push(*rate);
        }
        let money = self
            .money
            .checked_add(&other.money)
            .map_err(FxConversionError::Arithmetic)?;
        Ok(Self { money, rates })
    }

    #[must_use]
    pub fn accuracy(&self) -> ValuationAccuracy {
        if self.rates.is_empty() {
            ValuationAccuracy::Exact
        } else {
            ValuationAccuracy::Approximate
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (Money, Vec<FxRate>, ValuationAccuracy) {
        let accuracy = self.accuracy();
        (self.money, self.rates, accuracy)
    }
}

#[cfg(test)]
mod review_tests {
    use crate::{
        AmountError, Currency, FxConversionError, FxPair, FxRate, FxRateKind, FxSource, FxValidity,
        KrwValuation, KrwValuationComponent, Money, Nanosecond, NumericError, NumericInput,
        OffsetSeconds, TimeInterval, Timestamp, UnconvertedKrw, UnixSeconds, ValuationAccuracy,
    };

    fn time(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn input(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("입력 길이")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&input(value), currency).expect("금액")
    }
    fn fx(value: &str, kind: FxRateKind, start: i64, end: i64) -> FxRate {
        FxRate::parse(
            &input(value),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(time(start), time(end)).expect("유효 구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn native(value: &str) -> KrwValuation {
        KrwValuation::from_component(KrwValuationComponent::Unconverted(
            UnconvertedKrw::parse(&input(value)).expect("원화"),
        ))
    }
    fn mid(rate: FxRate, value: &str) -> KrwValuation {
        KrwValuation::from_component(KrwValuationComponent::Converted(
            rate.convert_for_aggregation(money(value, Currency::Usd))
                .expect("Mid 환산"),
        ))
    }

    #[test]
    fn money_public_parser_preserves_currency_and_exact_canonical_input() {
        for currency in [Currency::Krw, Currency::Usd] {
            for value in ["0", "-0.00", "-1.23456789", "79228162514264337593543950335"] {
                let parsed = Money::parse(&input(value), currency).expect("정확 금액");
                assert_eq!(parsed.currency(), currency);
                assert_eq!(
                    parsed.amount(),
                    crate::Amount::parse(&crate::AmountInput::new(value)).expect("D-01 기준")
                );
            }
        }
        assert_eq!(
            Money::parse(&input("1e3"), Currency::Krw),
            Err(NumericError::InvalidSyntax)
        );
        assert_eq!(
            Money::parse(&input("1.00000000000000000000000000001"), Currency::Usd),
            Err(NumericError::NotRepresentable)
        );
        assert_eq!(
            UnconvertedKrw::parse(&input("bad")),
            Err(NumericError::InvalidSyntax)
        );
        assert_eq!(
            crate::Amount::parse(&crate::AmountInput::new("bad")),
            Err(AmountError::InvalidSyntax)
        );
    }

    #[test]
    fn different_mid_rates_are_all_preserved_and_duplicates_are_deduplicated() {
        let first = fx("1375", FxRateKind::Mid, 100, 160);
        let second = fx("1500", FxRateKind::Mid, 999_999, 1_000_059);
        let first_value = mid(first, "1");
        let second_value = mid(second, "2");
        let sum = native("10000000")
            .checked_add(&first_value)
            .expect("합산")
            .checked_add(&second_value)
            .expect("합산")
            .checked_add(&first_value)
            .expect("중복 환율");
        assert_eq!(sum.accuracy(), ValuationAccuracy::Approximate);
        assert_eq!(
            sum.into_parts(),
            (
                money("10005750", Currency::Krw),
                vec![first, second],
                ValuationAccuracy::Approximate
            )
        );
        let exact = native("100").checked_add(&native("-25")).expect("원화 합");
        assert_eq!(exact.accuracy(), ValuationAccuracy::Exact);
        assert_eq!(
            exact.into_parts(),
            (money("75", Currency::Krw), vec![], ValuationAccuracy::Exact)
        );
        assert_eq!(
            first_value
                .checked_add(&native("0"))
                .expect("표지 보존")
                .accuracy(),
            ValuationAccuracy::Approximate
        );
        let changed_value = fx("1376", FxRateKind::Mid, 100, 160);
        let changed_end = fx("1375", FxRateKind::Mid, 100, 161);
        assert_eq!(
            first_value
                .checked_add(&mid(changed_value, "0"))
                .expect("다른 값")
                .checked_add(&mid(changed_end, "0"))
                .expect("다른 유효 기간")
                .into_parts()
                .1,
            vec![first, changed_value, changed_end]
        );
        assert_eq!(
            native("10000000")
                .checked_add(&mid(first, "5000"))
                .expect("R-11 예제 7")
                .into_parts()
                .0,
            money("16875000", Currency::Krw)
        );
    }

    #[test]
    fn sixty_four_distinct_rates_are_bounded_without_losing_context() {
        let mut total = native("0");
        for start in 0_i64..64 {
            total = total
                .checked_add(&mid(
                    fx(
                        "1",
                        FxRateKind::Mid,
                        start,
                        start.checked_add(1).expect("끝"),
                    ),
                    "1",
                ))
                .expect("64개까지");
        }
        let duplicate = total
            .checked_add(&mid(fx("1", FxRateKind::Mid, 0, 1), "1"))
            .expect("중복은 허용");
        let (amount, rates, accuracy) = duplicate.into_parts();
        assert_eq!(amount, money("65", Currency::Krw));
        assert_eq!(rates.len(), 64);
        assert_eq!(accuracy, ValuationAccuracy::Approximate);
        assert_eq!(
            total.checked_add(&mid(fx("1", FxRateKind::Mid, 64, 65), "1")),
            Err(FxConversionError::TooManyRates)
        );
        assert_eq!(total.into_parts().1.len(), 64);
        assert_eq!(
            native("79228162514264337593543950335").checked_add(&native("1")),
            Err(FxConversionError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            native("10").checked_add(&native("0.0000000000000000000000000001")),
            Err(FxConversionError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
    }

    #[test]
    fn validity_is_start_inclusive_end_exclusive_and_history_remains_available() {
        let rate = fx("1375", FxRateKind::Mid, 100, 160);
        assert_eq!(rate.valid_until(), time(160));
        for (now, expected) in [
            (99, FxValidity::OutsideInterval),
            (100, FxValidity::Valid),
            (159, FxValidity::Valid),
            (160, FxValidity::OutsideInterval),
            (161, FxValidity::OutsideInterval),
        ] {
            assert_eq!(rate.validity_at(time(now)), expected);
            match expected {
                FxValidity::Valid => assert_eq!(
                    rate.convert_for_order(money("2", Currency::Usd), time(now)),
                    rate.convert_for_historical_valuation(money("2", Currency::Usd))
                ),
                FxValidity::OutsideInterval => assert_eq!(
                    rate.convert_for_order(money("2", Currency::Usd), time(now)),
                    Err(FxConversionError::OutsideValidity)
                ),
            }
        }
        assert_eq!(
            rate.convert_for_historical_valuation(money("2", Currency::Usd))
                .expect("과거 평가")
                .into_parts()
                .0,
            money("2750", Currency::Krw)
        );
        assert_eq!(
            rate.convert_for_aggregation(money("2", Currency::Usd))
                .expect("과거 합산")
                .into_parts()
                .0,
            money("2750", Currency::Krw)
        );
        for end in [99, 100] {
            assert_eq!(
                TimeInterval::try_new(time(100), time(end)),
                Err(crate::TimeIntervalError::NonIncreasing)
            );
        }
        let buy = fx("1380.5", FxRateKind::Buy, 100, 160);
        assert_eq!(
            buy.convert_for_aggregation(money("2", Currency::Usd)),
            Err(FxConversionError::MidRequired)
        );
        assert_eq!(
            buy.convert_for_order(money("2", Currency::Usd), time(100))
                .expect("유효 매수 환율")
                .rate(),
            buy
        );
        assert_eq!(
            rate.convert_for_order(money("2", Currency::Krw), time(100)),
            Err(FxConversionError::CurrencyMismatch)
        );
    }

    #[test]
    fn conservative_selection_keeps_the_selected_rate_and_rejects_other_pairs() {
        let mid_rate = fx("1375", FxRateKind::Mid, 100, 160);
        let buy = fx("1380.5", FxRateKind::Buy, 120, 180);
        assert_eq!(mid_rate.conservative_with(buy), Ok(buy));
        assert_eq!(buy.conservative_with(mid_rate), Ok(buy));
        let lower_buy = fx("1370", FxRateKind::Buy, 120, 180);
        assert_eq!(lower_buy.conservative_with(mid_rate), Ok(mid_rate));
        let equal_buy = fx("1375", FxRateKind::Buy, 120, 180);
        assert_eq!(mid_rate.conservative_with(equal_buy), Ok(mid_rate));
        let reverse = FxRate::parse(
            &input("0.001"),
            FxPair::KrwUsd,
            FxRateKind::Mid,
            TimeInterval::try_new(time(100), time(160)).expect("유효 구간"),
            FxSource::TossOpenApi,
        )
        .expect("방향 기록");
        assert_eq!(reverse.pair().base(), Currency::Krw);
        assert_eq!(reverse.pair().quote(), Currency::Usd);
        assert_eq!(
            mid_rate.conservative_with(reverse),
            Err(FxConversionError::PairMismatch)
        );
        assert_eq!(
            reverse.convert_for_historical_valuation(money("100", Currency::Krw)),
            Err(FxConversionError::UnsupportedDirection)
        );
    }
}
