#[cfg(test)]
mod tests {
    use crate::candle::tests::series;
    use crate::{Currency, NumericInput, Ratio, Volatility};
    use proptest::test_runner::{Config, RngSeed, TestRunner};
    use rust_decimal::Decimal;
    use std::{cell::Cell, cmp::Ordering};

    #[derive(Clone, Debug)]
    struct Integer(Vec<u32>);
    impl Integer {
        fn new(value: u128) -> Self {
            Self(
                value
                    .to_string()
                    .bytes()
                    .rev()
                    .map(|digit| u32::from(digit.checked_sub(b'0').expect("숫자")))
                    .collect(),
            )
            .trim()
        }
        fn trim(mut self) -> Self {
            while self.0.len() > 1 && self.0.last() == Some(&0) {
                let _digit = self.0.pop();
            }
            self
        }
        fn cmp(&self, rhs: &Self) -> Ordering {
            self.0
                .len()
                .cmp(&rhs.0.len())
                .then_with(|| self.0.iter().rev().cmp(rhs.0.iter().rev()))
        }
        fn scaled(&self, places: u32) -> Self {
            let mut digits = vec![0; usize::try_from(places).expect("scale")];
            digits.extend(&self.0);
            Self(digits).trim()
        }
        fn add(&self, rhs: &Self) -> Self {
            let mut carry = 0_u32;
            let mut digits = Vec::new();
            for index in 0..self.0.len().max(rhs.0.len()) {
                let total = self
                    .0
                    .get(index)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(rhs.0.get(index).copied().unwrap_or(0))
                    .expect("합")
                    .checked_add(carry)
                    .expect("올림");
                digits.push(total.checked_rem(10).expect("기수"));
                carry = total.checked_div(10).expect("기수");
            }
            if carry > 0 {
                digits.push(carry);
            }
            Self(digits).trim()
        }
        fn subtract(&self, rhs: &Self) -> Self {
            assert!(!self.cmp(rhs).is_lt(), "음수 없음");
            let mut borrow = 0_u32;
            let digits = self
                .0
                .iter()
                .enumerate()
                .map(|(index, digit)| {
                    let sub = rhs
                        .0
                        .get(index)
                        .copied()
                        .unwrap_or(0)
                        .checked_add(borrow)
                        .expect("빌림");
                    let total = digit
                        .checked_add(10)
                        .expect("기수")
                        .checked_sub(sub)
                        .expect("차");
                    borrow = u32::from(*digit < sub);
                    total.checked_rem(10).expect("기수")
                })
                .collect();
            assert_eq!(borrow, 0);
            Self(digits).trim()
        }
        fn multiply(&self, rhs: &Self) -> Self {
            let mut digits = vec![
                0_u32;
                self.0
                    .len()
                    .checked_add(rhs.0.len())
                    .expect("길이")
                    .checked_add(1)
                    .expect("올림")
            ];
            for (index, left) in self.0.iter().enumerate() {
                Self::accumulate(&mut digits, index, *left, rhs);
            }
            let mut carry = 0_u32;
            for digit in &mut digits {
                let total = digit.checked_add(carry).expect("올림");
                *digit = total.checked_rem(10).expect("기수");
                carry = total.checked_div(10).expect("기수");
            }
            assert_eq!(carry, 0);
            Self(digits).trim()
        }
        fn accumulate(digits: &mut [u32], index: usize, left: u32, rhs: &Self) {
            for (offset, right) in rhs.0.iter().enumerate() {
                let target = digits
                    .get_mut(index.checked_add(offset).expect("자리"))
                    .expect("용량");
                *target = target
                    .checked_add(left.checked_mul(*right).expect("자리 곱"))
                    .expect("합");
            }
        }
        fn divided(&self, denominator: u128) -> Self {
            let mut remainder = 0_u128;
            let digits = self
                .0
                .iter()
                .rev()
                .map(|digit| {
                    let numerator = remainder
                        .checked_mul(10)
                        .expect("나머지")
                        .checked_add(u128::from(*digit))
                        .expect("자리");
                    remainder = numerator.checked_rem(denominator).expect("양수 분모");
                    u32::try_from(numerator.checked_div(denominator).expect("양수 분모"))
                        .expect("한 자리")
                })
                .collect::<Vec<_>>();
            Self(digits.into_iter().rev().collect()).trim()
        }
        fn difference(&self, rhs: &Self) -> Self {
            if self.cmp(rhs).is_lt() {
                rhs.subtract(self)
            } else {
                self.subtract(rhs)
            }
        }
        fn division_digit(&self, denominator: &Self) -> (Self, u8) {
            let mut remainder = self.clone();
            let mut digit = 0_u8;
            while !remainder.cmp(denominator).is_lt() {
                remainder = remainder.subtract(denominator);
                digit = digit.checked_add(1).expect("한 자리");
            }
            (remainder, digit)
        }
        fn ratio_digits(&self, denominator: &Self) -> Decimal {
            let mut remainder = self.clone();
            let mut digits = String::from("0.");
            assert!(self.cmp(denominator).is_lt(), "오차는 1 미만");
            for _place in 0_u8..28 {
                let (next, digit) = remainder.scaled(1).division_digit(denominator);
                remainder = next;
                digits.push(char::from(b'0'.checked_add(digit).expect("ASCII")));
            }
            Ratio::parse(&NumericInput::try_new(&digits).expect("길이"))
                .expect("비율")
                .as_decimal()
        }
    }

    fn normalized_coefficient(value: Decimal) -> (u128, u32) {
        let mut coefficient = value.mantissa().unsigned_abs();
        let mut scale = value.scale();
        while scale < 28 {
            let Some(next) = coefficient
                .checked_mul(10)
                .filter(|next| *next <= Decimal::MAX.mantissa().unsigned_abs())
            else {
                break;
            };
            coefficient = next;
            scale = scale.checked_add(1).expect("scale");
        }
        (coefficient, scale)
    }
    fn assert_root_bound(value: Decimal) {
        let input = Ratio::parse(&NumericInput::try_new(&value.to_string()).expect("입력 길이"))
            .expect("입력");
        let actual = crate::volatility_arithmetic::square_root(input)
            .expect("제곱근")
            .as_decimal();
        let (coefficient, scale) = normalized_coefficient(actual);
        let lower = Integer::new(coefficient.saturating_sub(2));
        let upper = Integer::new(coefficient.checked_add(2).expect("상한"));
        let exact = Integer::new(value.mantissa().unsigned_abs())
            .scaled(scale.checked_mul(2).expect("제곱 scale"));
        assert!(
            !lower
                .multiply(&lower)
                .scaled(value.scale())
                .cmp(&exact)
                .is_gt(),
            "하한 제곱"
        );
        assert!(
            !upper
                .multiply(&upper)
                .scaled(value.scale())
                .cmp(&exact)
                .is_lt(),
            "상한 제곱"
        );
    }
    #[test]
    fn square_roots_stay_within_two_native_ulps_by_wide_integer_squares() {
        for value in [
            Decimal::ZERO,
            Decimal::ONE,
            Decimal::TWO,
            Decimal::new(1, 28),
            Decimal::MAX,
        ] {
            assert_root_bound(value);
        }
        let config = Config {
            rng_seed: RngSeed::Fixed(0x00d1_1006),
            ..Config::with_cases(4096)
        };
        TestRunner::new(config)
            .run(
                &(0_i128..=Decimal::MAX.mantissa(), 0_u32..=28),
                |(coefficient, scale)| {
                    assert_root_bound(Decimal::from_i128_with_scale(coefficient, scale));
                    Ok(())
                },
            )
            .expect("독립 넓은 정수 2ulp 경계");
    }

    fn reference_variance(prices: &[u128]) -> (Integer, Integer) {
        let mut positive = Integer::new(0);
        let mut negative = Integer::new(0);
        let mut squares = Integer::new(0);
        for (previous, current) in prices.iter().zip(prices.iter().skip(1)) {
            let delta = current.abs_diff(*previous);
            let scaled = Integer::new(delta).scaled(40).divided(*previous);
            squares = squares.add(&scaled.multiply(&scaled));
            if current >= previous {
                positive = positive.add(&scaled);
            } else {
                negative = negative.add(&scaled);
            }
        }
        let sum = positive.difference(&negative);
        let count = prices.iter().skip(1).count();
        let observations = u128::try_from(count).expect("n");
        let numerator = squares
            .multiply(&Integer::new(observations))
            .subtract(&sum.multiply(&sum));
        let denominator = Integer::new(
            observations
                .checked_mul(observations.checked_sub(1).expect("n-1"))
                .expect("분모"),
        )
        .scaled(80);
        (numerator, denominator)
    }
    fn sigma_relative_error_bound(actual: Decimal, prices: &[u128]) -> Decimal {
        let (numerator, denominator) = reference_variance(prices);
        let coefficient = Integer::new(actual.mantissa().unsigned_abs());
        let actual_squared = coefficient.multiply(&coefficient).multiply(&denominator);
        let reference = numerator.scaled(actual.scale().checked_mul(2).expect("scale"));
        let difference = actual_squared.difference(&reference);
        assert!(
            !difference.scaled(15).cmp(&reference).is_gt(),
            "상대 분산 오차 <= 1e-15"
        );
        difference.ratio_digits(&reference)
    }
    fn prices_to_candles(prices: &[u128], currency: Currency) -> Vec<crate::Candle> {
        let texts = prices
            .iter()
            .map(|value| match currency {
                Currency::Krw => value.to_string(),
                Currency::Usd => format!(
                    "{}.{:02}",
                    value.checked_div(100).expect("센트"),
                    value.checked_rem(100).expect("센트")
                )
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned(),
            })
            .collect::<Vec<_>>();
        let references = texts.iter().map(String::as_str).collect::<Vec<_>>();
        let base = series(&references);
        base.iter()
            .zip(texts.iter())
            .map(|(candle, text)| {
                let price = crate::candle::tests::price(text, currency);
                crate::Candle::try_new(
                    candle.instrument().clone(),
                    candle.time(),
                    candle.adjustment(),
                    crate::Ohlcv::try_new(price, price, price, price, candle.values().volume())
                        .expect("OHLCV"),
                    crate::CandleObservation::new(
                        candle.collected_at(),
                        crate::CandleStatus::Finalized,
                    ),
                )
                .expect("봉")
            })
            .collect()
    }
    fn geometric_walk(initial: u128, sigma: i128, trend: i128, shocks: &[bool]) -> Vec<u128> {
        let mut price = initial;
        let mut prices = vec![price];
        for direction in shocks
            .iter()
            .flat_map(|orientation| [*orientation, !orientation])
        {
            let signed = if direction {
                sigma
            } else {
                sigma.checked_neg().expect("부호")
            };
            let factor = 10_000_i128
                .checked_add(trend)
                .expect("추세")
                .checked_add(signed)
                .expect("충격");
            price = price
                .checked_mul(u128::try_from(factor).expect("양수"))
                .expect("가격")
                .checked_add(5_000)
                .expect("최근접")
                .checked_div(10_000)
                .expect("기수");
            prices.push(price);
        }
        prices
    }
    #[test]
    fn realistic_geometric_walks_match_integer_rational_returns_with_relative_error_bound() {
        let maximum = Cell::new(Decimal::ZERO);
        let config = Config {
            rng_seed: RngSeed::Fixed(0x00d1_1007),
            ..Config::with_cases(1024)
        };
        TestRunner::new(config)
            .run(
                &(
                    1_000_u128..=1_000_000,
                    60_i128..=480,
                    -10_i128..=10,
                    proptest::collection::vec(proptest::bool::ANY, 30..61),
                    proptest::bool::ANY,
                ),
                |(initial, sigma, trend, shocks, usd)| {
                    let currency = if usd { Currency::Usd } else { Currency::Krw };
                    let prices = geometric_walk(initial, sigma, trend, &shocks);
                    let value = Volatility::calculate(&prices_to_candles(&prices, currency))
                        .expect("현실 가격 흐름");
                    proptest::prop_assert!(value.daily().as_decimal() >= Decimal::new(5, 3));
                    proptest::prop_assert!(value.daily().as_decimal() <= Decimal::new(5, 2));
                    let error = sigma_relative_error_bound(value.daily().as_decimal(), &prices);
                    maximum.set(maximum.get().max(error));
                    Ok(())
                },
            )
            .expect("40자리 유리수 수익률과 넓은 정수 분산");
        assert!(
            maximum.get() <= Decimal::new(1, 15),
            "최대 상대 오차 상한 <= 1e-15"
        );
    }
    #[test]
    fn high_trend_low_noise_requires_centered_two_pass_variance() {
        let prices = [
            100_u128,
            10_000_000_000_000_101,
            1_000_000_000_000_020_100_000_000_000_100,
        ];
        let value = Volatility::calculate(&prices_to_candles(&prices, Currency::Usd))
            .expect("소거 스트레스");
        let (numerator, denominator) = reference_variance(&prices);
        let coefficient = Integer::new(value.daily().as_decimal().mantissa().unsigned_abs());
        let actual = coefficient.multiply(&coefficient).multiply(&denominator);
        let reference = numerator.scaled(
            value
                .daily()
                .as_decimal()
                .scale()
                .checked_mul(2)
                .expect("scale"),
        );
        assert!(
            !actual
                .difference(&reference)
                .scaled(11)
                .cmp(&reference)
                .is_gt(),
            "상대 오차 <= 1e-11"
        );
    }
}
