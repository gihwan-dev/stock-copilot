use crate::performance_arithmetic::Number;
use crate::{CalendarDate, PerformanceError, PerformanceValue};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BootstrapSeed(u64);
impl BootstrapSeed {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}
impl std::fmt::Debug for BootstrapSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BootstrapSeed")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Superior,
    Inferior,
    EdgeNotConfirmed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithheldReason {
    IncomparableSamples,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithheldCount(usize);
impl WithheldCount {
    pub(crate) const fn new(value: usize) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapInterval {
    InsufficientSamples,
    Withheld(WithheldReason, WithheldCount),
    Ready(EdgeInterval),
}
impl BootstrapInterval {
    #[must_use]
    pub fn into_ready(self) -> Option<EdgeInterval> {
        match self {
            Self::Ready(value) => Some(value),
            Self::InsufficientSamples | Self::Withheld(_, _) => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInterval {
    lower: BootstrapValue,
    upper: BootstrapValue,
    direction: EdgeDirection,
}
impl EdgeInterval {
    #[must_use]
    pub const fn lower(&self) -> BootstrapValue {
        self.lower
    }
    #[must_use]
    pub const fn upper(&self) -> BootstrapValue {
        self.upper
    }
    #[must_use]
    pub const fn direction(&self) -> EdgeDirection {
        self.direction
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapValue(Number);
impl BootstrapValue {
    pub(crate) const fn number(self) -> Number {
        self.0
    }
}
impl std::fmt::Display for BootstrapValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.12}", self.number().as_decimal())
    }
}
fn rounded_integer(
    numerator: Coefficient,
    denominator: Coefficient,
) -> Result<Coefficient, PerformanceError> {
    let (quotient, remainder) = numerator
        .0
        .checked_div(denominator.0)
        .zip(numerator.0.checked_rem(denominator.0))
        .ok_or(PerformanceError::Arithmetic)?;
    let doubled = remainder.unsigned_abs().wrapping_mul(2);
    let divisor = denominator.0.unsigned_abs();
    if doubled > divisor || (doubled == divisor && quotient.wrapping_rem(2) != 0) {
        quotient
            .checked_add(numerator.0.signum().wrapping_mul(denominator.0.signum()))
            .map(Coefficient)
            .ok_or(PerformanceError::Arithmetic)
    } else {
        Ok(Coefficient(quotient))
    }
}
fn coefficient(value: Number) -> Result<Coefficient, PerformanceError> {
    let decimal = value.as_decimal();
    if decimal.scale() <= 12 {
        decimal
            .mantissa()
            .checked_mul(10_i128.wrapping_pow(12_u32.wrapping_sub(decimal.scale())))
            .map(Coefficient)
            .ok_or(PerformanceError::Arithmetic)
    } else {
        rounded_integer(
            Coefficient(decimal.mantissa()),
            Coefficient(10_i128.wrapping_pow(decimal.scale().wrapping_sub(12))),
        )
    }
}
fn quantized(value: Number) -> Result<Coefficient, PerformanceError> {
    let result = coefficient(value)?;
    if result.0.unsigned_abs() > 1_000_000_000_000_000_000_000_000_u128 {
        return Err(PerformanceError::Arithmetic);
    }
    Ok(result)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Count(usize);
#[derive(Clone, Copy)]
struct Cluster {
    sum: Coefficient,
    count: Count,
}
#[derive(Clone, Copy)]
struct Coefficient(i128);
#[derive(Clone, Copy)]
struct Scale(u32);
struct ScaledCluster {
    sum: Coefficient,
    count: Count,
}
struct PreparedClusters {
    values: Vec<ScaledCluster>,
    scale: Scale,
}
impl PreparedClusters {
    fn new(clusters: &[Cluster]) -> Self {
        Self {
            values: clusters
                .iter()
                .map(|value| ScaledCluster {
                    sum: value.sum,
                    count: value.count,
                })
                .collect(),
            scale: Scale(12),
        }
    }
}
struct RandomWord(u64);
struct Generator(u64);
impl Generator {
    fn new(seed: BootstrapSeed) -> Self {
        Self(seed.0)
    }
    fn word(&mut self) -> RandomWord {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut word = self.0;
        word = (word ^ (word >> 30_u32)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        word = (word ^ (word >> 27_u32)).wrapping_mul(0x94d0_49bb_1331_11eb);
        RandomWord(word ^ (word >> 31_u32))
    }
    fn index(&mut self, count: Count) -> Result<Count, PerformanceError> {
        if count.0 > 65536 {
            return Err(PerformanceError::TooManyInputs);
        }
        let remainder = 65536_usize
            .checked_rem(count.0)
            .ok_or(PerformanceError::Arithmetic)?;
        let ceiling = 65536_usize.wrapping_sub(remainder);
        loop {
            let [low, high, ..] = self.word().0.to_le_bytes();
            let candidate = usize::from(u16::from_le_bytes([low, high]));
            if candidate < ceiling {
                return candidate
                    .checked_rem(count.0)
                    .map(Count)
                    .ok_or(PerformanceError::Arithmetic);
            }
        }
    }
}
pub(crate) fn interval(
    input: &[(CalendarDate, PerformanceValue)],
    seed: BootstrapSeed,
) -> Result<BootstrapInterval, PerformanceError> {
    if input.len() > 65536 {
        return Err(PerformanceError::TooManyInputs);
    }
    let mut ordered = input.to_vec();
    ordered.sort_by_key(|(day, value)| (*day, value.number()));
    let mut days: BTreeMap<CalendarDate, Cluster> = BTreeMap::new();
    for (day, value) in ordered {
        let cluster = days.entry(day).or_insert(Cluster {
            sum: Coefficient(0),
            count: Count(0),
        });
        cluster.sum = quantized(value.number()).and_then(|quantum| {
            cluster
                .sum
                .0
                .checked_add(quantum.0)
                .map(Coefficient)
                .ok_or(PerformanceError::Arithmetic)
        })?;
        cluster.count.0 = cluster.count.0.wrapping_add(1);
    }
    if days.len() < 30 {
        return Ok(BootstrapInterval::InsufficientSamples);
    }
    let groups = days.into_values().collect::<Vec<_>>();
    draw_interval(&PreparedClusters::new(&groups), seed)
}
fn draw_interval(
    clusters: &PreparedClusters,
    seed: BootstrapSeed,
) -> Result<BootstrapInterval, PerformanceError> {
    let mut generator = Generator::new(seed);
    let mut samples = vec![Number::new(Decimal::ZERO); 2000];
    samples
        .iter_mut()
        .try_for_each(|value| sample(clusters, &mut generator).map(|draw| *value = draw))
        .map(|()| percentiles(samples))
}
fn percentiles(mut samples: Vec<Number>) -> BootstrapInterval {
    let (prefix, upper_value, _tail) = samples.select_nth_unstable(1949);
    let upper = *upper_value;
    let (_head, lower_value, _middle) = prefix.select_nth_unstable(49);
    let lower = *lower_value;
    let zero = Number::new(Decimal::ZERO);
    let direction = if lower > zero {
        EdgeDirection::Superior
    } else if upper < zero {
        EdgeDirection::Inferior
    } else {
        EdgeDirection::EdgeNotConfirmed
    };
    BootstrapInterval::Ready(EdgeInterval {
        lower: BootstrapValue(lower),
        upper: BootstrapValue(upper),
        direction,
    })
}
fn sample(
    clusters: &PreparedClusters,
    generator: &mut Generator,
) -> Result<Number, PerformanceError> {
    let mut sum = Coefficient(0);
    let mut count = Count(0);
    for _ in &clusters.values {
        let index = generator.index(Count(clusters.values.len()))?;
        let update = clusters
            .values
            .get(index.0)
            .ok_or(PerformanceError::Arithmetic)
            .and_then(|cluster| {
                sum.0
                    .checked_add(cluster.sum.0)
                    .zip(count.0.checked_add(cluster.count.0))
                    .map(|(next_sum, next_count)| (Coefficient(next_sum), Count(next_count)))
                    .ok_or(PerformanceError::Arithmetic)
            })?;
        (sum, count) = update;
    }
    let divisor = Decimal::from(count.0).mantissa();
    rounded_integer(sum, Coefficient(divisor))
        .and_then(|mean| {
            Decimal::try_from_i128_with_scale(mean.0, clusters.scale.0)
                .map_err(|_error| PerformanceError::Arithmetic)
        })
        .map(Number::new)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        BootstrapInterval, BootstrapSeed, Cluster, Coefficient, Count, EdgeDirection, Generator,
        PreparedClusters, coefficient, interval, sample,
    };
    use crate::PerformanceValue;
    use crate::performance_arithmetic::Number;
    use proptest::prelude::any;
    use proptest::test_runner::Config;
    use proptest::{prop_assert, prop_assert_eq, proptest};
    use rust_decimal::Decimal;
    pub(crate) fn reference_ratio(numerator: i64, denominator: i64) -> Decimal {
        Decimal::from(numerator)
            .checked_div(Decimal::from(denominator))
            .expect("독립 유리수")
    }
    pub(crate) fn reference_mean(sum: i64, count: i64) -> Decimal {
        let scaled = sum.checked_mul(1_000_000_000_000).expect("독립 배율");
        let quotient = scaled.checked_div(count).expect("독립 몫");
        let remainder = scaled.checked_rem(count).expect("독립 나머지");
        let distance = remainder.abs().checked_mul(2).expect("독립 반올림");
        let rounded = if distance > count {
            quotient.checked_add(sum.signum()).expect("독립 반올림 몫")
        } else {
            quotient
        };
        Decimal::from_i128_with_scale(i128::from(rounded), 12)
    }
    fn observations(value: Decimal) -> Vec<(crate::CalendarDate, PerformanceValue)> {
        (0_i64..30)
            .map(|day| {
                (
                    crate::candle::tests::at(day).offset_calendar_date(),
                    PerformanceValue::from_number(Number::new(value)),
                )
            })
            .collect()
    }
    #[test]
    fn closed_directions_include_both_zero_endpoints() {
        for (value, expected) in [
            (Decimal::ONE, EdgeDirection::Superior),
            (-Decimal::ONE, EdgeDirection::Inferior),
            (Decimal::ZERO, EdgeDirection::EdgeNotConfirmed),
        ] {
            let result = interval(&observations(value), BootstrapSeed::new(0))
                .expect("계산")
                .into_ready()
                .expect("준비된 구간");
            assert_eq!(result.lower().number().as_decimal(), value);
            assert_eq!(result.upper().number().as_decimal(), value);
            assert_eq!(result.direction(), expected);
            assert_eq!(
                format!("{:?}", result.lower()),
                "BootstrapValue(Number([REDACTED]))"
            );
        }
    }
    #[test]
    fn thirty_observations_and_thirty_dates_are_both_required() {
        assert_eq!(BootstrapInterval::InsufficientSamples.into_ready(), None);
        let data = observations(Decimal::ZERO);
        assert_eq!(
            interval(&data[..29], BootstrapSeed::new(1)),
            Ok(BootstrapInterval::InsufficientSamples)
        );
        assert_eq!(
            interval(&vec![data[0]; 30], BootstrapSeed::new(1)),
            Ok(BootstrapInterval::InsufficientSamples)
        );
        assert_eq!(
            interval(&vec![data[0]; 65537], BootstrapSeed::new(1)),
            Err(crate::PerformanceError::TooManyInputs)
        );
        assert_eq!(
            interval(&[], BootstrapSeed::new(1)),
            Ok(BootstrapInterval::InsufficientSamples)
        );
    }
    proptest! {
        #![proptest_config(Config::with_cases(testkit::property_cases()))]
        #[test]
        fn sampled_mean_matches_independent_integer_ratio_and_convex_hull(seed in any::<u64>(), values in proptest::collection::vec((-1000_i32..1000_i32,1_usize..5),1..9)) {
            let clusters=values.iter().map(|(value,count)| Cluster { sum:coefficient(Number::new(Decimal::from(*value).checked_mul(Decimal::from(*count)).expect("작은 정수"))).expect("고정 배율 계수"),count:Count(*count) }).collect::<Vec<_>>();
            let prepared=PreparedClusters::new(&clusters);
            let actual=sample(&prepared,&mut Generator::new(BootstrapSeed::new(seed))).expect("평균");
            let mut independent=Generator::new(BootstrapSeed::new(seed));
            let mut sum=0_i64;
            let mut count=0_i64;
            for _ in &values {
                let index=independent.index(Count(values.len())).expect("색인");
                let (value,size)=values.get(index.0).expect("표본");
                let weight=i64::try_from(*size).expect("작은 묶음");
                sum=sum.checked_add(i64::from(*value).checked_mul(weight).expect("곱")).expect("합");
                count=count.checked_add(weight).expect("건수");
            }
            let expected=reference_mean(sum,count);
            prop_assert_eq!(actual.as_decimal(),expected);
            let minimum=values.iter().map(|(value,_)| *value).min().expect("표본");
            let maximum=values.iter().map(|(value,_)| *value).max().expect("표본");
            prop_assert!(actual.as_decimal()>=Decimal::from(minimum));
            prop_assert!(actual.as_decimal()<=Decimal::from(maximum));
        }
    }
    fn scaled_observations(step: i64) -> Vec<(crate::CalendarDate, PerformanceValue)> {
        (0_i64..30)
            .flat_map(|day| {
                let date = crate::candle::tests::at(day).offset_calendar_date();
                let value = day.checked_mul(step).expect("작은 정수");
                [value, value.checked_add(1).expect("두 번째")].map(|amount| {
                    (
                        date,
                        PerformanceValue::from_number(Number::new(Decimal::from(amount))),
                    )
                })
            })
            .collect()
    }
    #[test]
    fn sorted_date_clusters_are_seed_deterministic_for_4096_input_permutations() {
        use proptest::test_runner::TestRunner;
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(any::<u64>(), 1_i64..10000_i64), |(seed, step)| {
                let mut input = scaled_observations(step);
                let first = interval(&input, BootstrapSeed::new(seed)).expect("첫 구간");
                input.reverse();
                let second = interval(&input, BootstrapSeed::new(seed)).expect("재정렬 구간");
                prop_assert_eq!(first, second);
                Ok(())
            })
            .expect("4096개 입력 순서");
    }
    #[test]
    fn splitmix_words_rejection_and_seed_accessor_have_fixed_integer_witnesses() {
        for (seed, expected) in [
            (0_u64, 0xe220_a839_7b1d_cdaf_u64),
            (1_u64, 0x910a_2dec_8902_5cc1_u64),
            (42_u64, 0xbdd7_3226_2feb_6e95_u64),
        ] {
            let input = BootstrapSeed::new(seed);
            assert_eq!(*input.as_u64(), seed);
            assert_eq!(Generator::new(input).word().0, expected);
        }
        assert_eq!(
            Generator::new(BootstrapSeed::new(0)).index(Count(0)),
            Err(crate::PerformanceError::Arithmetic)
        );
        assert_eq!(
            Generator::new(BootstrapSeed::new(0)).index(Count(65537)),
            Err(crate::PerformanceError::TooManyInputs)
        );
        assert_eq!(
            Generator::new(BootstrapSeed::new(0)).index(Count(32769)),
            Ok(Count(26100))
        );
        assert_eq!(
            Generator::new(BootstrapSeed::new(0)).index(Count(65536)),
            Ok(Count(52655))
        );
    }
    #[test]
    fn percentile_example_matches_independent_integer_fraction_reference() {
        let input = scaled_observations(1);
        let result = interval(&input, BootstrapSeed::new(42))
            .expect("합성 예제")
            .into_ready()
            .expect("준비된 구간");
        assert_eq!(result.lower().to_string(), "11.866666666667");
        assert_eq!(result.upper().to_string(), "18.000000000000");
        let other = interval(&input, BootstrapSeed::new(1))
            .expect("다른 시드")
            .into_ready()
            .expect("준비된 구간");
        assert_ne!(result, other);
    }
    #[test]
    fn zero_inside_a_nonzero_interval_never_claims_an_edge() {
        let input = (0_i64..30)
            .map(|day| {
                (
                    crate::candle::tests::at(day).offset_calendar_date(),
                    PerformanceValue::from_number(Number::new(if day < 15 {
                        -Decimal::ONE
                    } else {
                        Decimal::ONE
                    })),
                )
            })
            .collect::<Vec<_>>();
        let result = interval(&input, BootstrapSeed::new(42))
            .expect("구간")
            .into_ready()
            .expect("준비된 구간");
        assert!(result.lower().number().as_decimal() < Decimal::ZERO);
        assert!(result.upper().number().as_decimal() > Decimal::ZERO);
        assert_eq!(result.direction(), EdgeDirection::EdgeNotConfirmed);
    }
    #[test]
    fn extreme_scales_and_empty_mean_are_closed_arithmetic_errors() {
        let groups = [
            Cluster {
                sum: Coefficient(i128::MAX),
                count: Count(1),
            },
            Cluster {
                sum: Coefficient(1),
                count: Count(1),
            },
        ];
        assert_eq!(
            sample(
                &PreparedClusters::new(&groups),
                &mut Generator::new(BootstrapSeed::new(0))
            )
            .map(|_value| ()),
            Err(crate::PerformanceError::Arithmetic)
        );
        let empty = PreparedClusters::new(&[]);
        assert_eq!(
            sample(&empty, &mut Generator::new(BootstrapSeed::new(0))).map(|_value| ()),
            Err(crate::PerformanceError::Arithmetic)
        );
        assert_eq!(
            interval(&observations(Decimal::MAX), BootstrapSeed::new(0)),
            Err(crate::PerformanceError::Arithmetic)
        );
        let data = observations(Decimal::ZERO);
        assert_eq!(
            interval(&vec![data[0]; 65536], BootstrapSeed::new(0)),
            Ok(BootstrapInterval::InsufficientSamples)
        );
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::{
        BootstrapSeed, Coefficient, Count, Generator, PreparedClusters, Scale, ScaledCluster,
        interval, sample,
    };
    use crate::performance_arithmetic::Number;
    use crate::{PerformanceError, PerformanceValue};
    use rust_decimal::Decimal;
    #[test]
    fn d29_cluster_accumulation_and_sample_integer_overflow_are_errors() {
        let value = (
            crate::candle::tests::at(0).offset_calendar_date(),
            PerformanceValue::from_number(Number::new(Decimal::MAX)),
        );
        assert_eq!(
            interval(&[value, value], BootstrapSeed::new(0)),
            Err(PerformanceError::Arithmetic)
        );
        for (sum, count) in [(i128::MAX, 1_usize), (1_i128, usize::MAX)] {
            let groups = PreparedClusters {
                values: (0_usize..2)
                    .map(|_| ScaledCluster {
                        sum: Coefficient(sum),
                        count: Count(count),
                    })
                    .collect(),
                scale: Scale(0),
            };
            assert_eq!(
                sample(&groups, &mut Generator::new(BootstrapSeed::new(0))),
                Err(PerformanceError::Arithmetic)
            );
        }
        let too_many = PreparedClusters {
            values: (0_usize..65537)
                .map(|_| ScaledCluster {
                    sum: Coefficient(0),
                    count: Count(1),
                })
                .collect(),
            scale: Scale(0),
        };
        assert_eq!(
            sample(&too_many, &mut Generator::new(BootstrapSeed::new(0))),
            Err(PerformanceError::TooManyInputs)
        );
    }
}

#[cfg(test)]
mod scale_tests {
    #[test]
    fn d29_interval_reports_common_coefficient_scale_overflow() {
        let mut input = (0_i64..30)
            .map(|day| {
                (
                    crate::candle::tests::at(day).offset_calendar_date(),
                    crate::PerformanceValue::from_number(
                        crate::performance_arithmetic::Number::new(rust_decimal::Decimal::ZERO),
                    ),
                )
            })
            .collect::<Vec<_>>();
        input[0].1 = crate::PerformanceValue::from_number(
            crate::performance_arithmetic::Number::new(rust_decimal::Decimal::MAX),
        );
        input[1].1 = crate::PerformanceValue::from_number(
            crate::performance_arithmetic::Number::new(rust_decimal::Decimal::new(1, 28)),
        );
        assert_eq!(
            super::interval(&input, super::BootstrapSeed::new(0)),
            Err(crate::PerformanceError::Arithmetic)
        );
    }
}

#[cfg(test)]
mod review_tests {
    use super::{BootstrapSeed, EdgeDirection, interval};
    use crate::{PerformanceValue, performance_arithmetic::Number};
    use rust_decimal::Decimal;
    #[test]
    fn review_28_place_returns_do_not_overflow_before_mean() {
        let third = Decimal::ONE
            .checked_div(Decimal::from(3_u32))
            .expect("삼분의 일");
        let fee = Decimal::new(15, 5);
        let cost = Decimal::ONE
            .checked_sub(fee)
            .and_then(|received| {
                received.checked_div(Decimal::ONE.checked_add(fee).expect("매수 비용"))
            })
            .and_then(|value| value.checked_sub(Decimal::ONE))
            .and_then(|value| value.checked_add(Decimal::new(3, 1)))
            .expect("실제 양방향 비용과 벤치마크");
        let near = Decimal::new(5, 2)
            .checked_add(Decimal::new(1, 28))
            .expect("28자리 꼬리");
        for (value, count, expected) in [
            (third, 30_i64, "0.333333333333"),
            (cost, 200, "0.299700044993"),
            (cost, 2000, "0.299700044993"),
            (near, 200, "0.050000000000"),
        ] {
            let input = (0_i64..count)
                .map(|day| {
                    (
                        crate::candle::tests::at(day).offset_calendar_date(),
                        PerformanceValue::from_number(Number::new(value)),
                    )
                })
                .collect::<Vec<_>>();
            let result = interval(&input, BootstrapSeed::new(42))
                .expect("실제 정밀도")
                .into_ready()
                .expect("30일 이상");
            assert_eq!(result.direction(), EdgeDirection::Superior);
            assert_eq!(result.lower().to_string(), expected);
            assert_eq!(result.upper().to_string(), expected);
        }
    }
}

#[cfg(test)]
mod fixed_point_tests {
    use super::{Coefficient, Number, coefficient, rounded_integer};
    use rust_decimal::Decimal;
    #[test]
    fn review_static_integer_bounds_cover_cluster_repetition() {
        let coefficient_limit = 1_000_000_000_000_000_000_000_000_i128;
        let input_limit = 65_536_i128;
        let cluster = coefficient_limit
            .checked_mul(input_limit)
            .expect("묶음 합 상한");
        assert!(cluster <= Decimal::MAX.mantissa());
        let resampled = cluster.checked_mul(input_limit).expect("반복 추출 합 상한");
        assert!(resampled < i128::MAX);
    }
    #[test]
    fn review_half_even_quantization_and_signed_mean_have_independent_ties() {
        for (mantissa, expected) in [(25_i128, 2_i128), (35, 4), (-25, -2), (-35, -4)] {
            let input = Number::new(Decimal::from_i128_with_scale(mantissa, 13));
            assert_eq!(coefficient(input).expect("양자화").0, expected);
            assert_eq!(
                rounded_integer(Coefficient(mantissa), Coefficient(10))
                    .expect("평균")
                    .0,
                expected
            );
        }
    }
}

#[cfg(test)]
mod percentile_review_tests {
    use crate::PerformanceValue;
    use crate::performance_arithmetic::Number;
    use rust_decimal::Decimal;
    fn reference_index(state: &mut u64) -> i128 {
        loop {
            *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let first = (*state ^ (*state >> 30_u32)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            let second = (first ^ (first >> 27_u32)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let candidate = (second ^ (second >> 31_u32)) & 65535_u64;
            if candidate < 65520_u64 {
                return i128::from(candidate.wrapping_rem(30));
            }
        }
    }
    #[test]
    fn review_adjacent_percentile_ranks_have_distinct_independent_integer_means() {
        let mut state = 42_u64;
        let mut expected = Vec::new();
        for _ in 0_usize..2000 {
            let mut sum = 0_i128;
            for _ in 0_usize..30 {
                sum = sum
                    .checked_add(
                        1_i128
                            .checked_shl(u32::try_from(reference_index(&mut state)).expect("색인"))
                            .expect("거듭제곱"),
                    )
                    .expect("합");
            }
            let scaled = sum.checked_mul(1_000_000_000_000).expect("독립 고정 배율");
            let quotient = scaled.checked_div(30).expect("독립 몫");
            let remainder = scaled.checked_rem(30).expect("독립 나머지");
            let rounded = if remainder.checked_mul(2).expect("거리") > 30 {
                quotient.checked_add(1).expect("반올림")
            } else {
                quotient
            };
            expected.push(rounded);
        }
        expected.sort_unstable();
        assert_ne!(expected[49], expected[50]);
        assert_ne!(expected[1949], expected[1950]);
        let input = (0_u32..30)
            .map(|day| {
                (
                    crate::candle::tests::at(i64::from(day)).offset_calendar_date(),
                    PerformanceValue::from_number(Number::new(Decimal::from(
                        1_u64.checked_shl(day).expect("독립 표본"),
                    ))),
                )
            })
            .collect::<Vec<_>>();
        let actual = super::interval(&input, super::BootstrapSeed::new(42))
            .expect("구간")
            .into_ready()
            .expect("준비");
        assert_eq!(
            actual.lower().number().as_decimal(),
            Decimal::from_i128_with_scale(expected[49], 12)
        );
        assert_eq!(
            actual.upper().number().as_decimal(),
            Decimal::from_i128_with_scale(expected[1949], 12)
        );
    }
}

#[cfg(test)]
mod quantization_review_tests {
    use super::{BootstrapSeed, Cluster, Count, Generator, PreparedClusters, quantized, sample};
    use crate::performance_arithmetic::Number;
    use proptest::prelude::{any, prop_assert_eq, proptest};
    use proptest::test_runner::Config;
    use rust_decimal::Decimal;

    fn independent_round(numerator: i128, divisor: i128) -> i128 {
        let quotient = numerator.checked_div(divisor).expect("몫");
        let remainder = numerator.checked_rem(divisor).expect("나머지");
        let twice = remainder.abs().checked_mul(2).expect("두 배");
        if twice > divisor || (twice == divisor && quotient.checked_rem(2).expect("짝수") != 0) {
            quotient.checked_add(numerator.signum()).expect("반올림")
        } else {
            quotient
        }
    }
    #[test]
    fn review_quantized_magnitude_limit_has_a_closed_error() {
        assert_eq!(
            quantized(Number::new(Decimal::from(1_000_000_000_001_i64))).map(|value| value.0),
            Err(crate::PerformanceError::Arithmetic)
        );
        assert_eq!(
            quantized(Number::new(Decimal::from(1_000_000_000_000_i64)))
                .expect("상한")
                .0,
            1_000_000_000_000_000_000_000_000_i128
        );
        assert_eq!(super::tests::reference_mean(3, 2), Decimal::new(15, 1));
    }
    proptest! {
        #![proptest_config(Config::with_cases(testkit::property_cases()))]
        #[test]
        fn review_quantization_and_resampled_i128_mean_match_independent_integer_reference(seed in any::<u64>(), coefficients in proptest::collection::vec(-9_999_999_999_999_999_i64..9_999_999_999_999_999_i64,1..12)) {
            let expected = coefficients.iter().map(|coefficient|independent_round(i128::from(*coefficient),10_000_000_000_000_000)).collect::<Vec<_>>();
            let clusters = coefficients.iter().zip(&expected).map(|(coefficient,reference)| {
                let actual = quantized(Number::new(Decimal::from_i128_with_scale(i128::from(*coefficient),28))).expect("양자화");
                assert_eq!(actual.0,*reference);
                Cluster { sum:actual,count:Count(1) }
            }).collect::<Vec<_>>();
            let prepared = PreparedClusters::new(&clusters);
            let actual = sample(&prepared,&mut Generator::new(BootstrapSeed::new(seed))).expect("평균");
            let mut independent = Generator::new(BootstrapSeed::new(seed));
            let sum = expected.iter().try_fold(0_i128,|sum,_| {
                let index=independent.index(Count(expected.len())).expect("색인");
                sum.checked_add(expected[index.0])
            }).expect("정수 합");
            let expected_mean=independent_round(sum,i128::try_from(expected.len()).expect("건수"));
            prop_assert_eq!(actual.as_decimal(),Decimal::from_i128_with_scale(expected_mean,12));
        }
    }
}
