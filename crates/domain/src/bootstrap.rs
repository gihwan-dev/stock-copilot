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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapInterval {
    InsufficientSamples,
    Ready(EdgeInterval),
}
impl BootstrapInterval {
    #[must_use]
    pub fn into_ready(self) -> Option<EdgeInterval> {
        match self {
            Self::Ready(value) => Some(value),
            Self::InsufficientSamples => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInterval {
    lower: PerformanceValue,
    upper: PerformanceValue,
    direction: EdgeDirection,
}
impl EdgeInterval {
    #[must_use]
    pub const fn lower(&self) -> PerformanceValue {
        self.lower
    }
    #[must_use]
    pub const fn upper(&self) -> PerformanceValue {
        self.upper
    }
    #[must_use]
    pub const fn direction(&self) -> EdgeDirection {
        self.direction
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Count(usize);
#[derive(Clone, Copy)]
struct Cluster {
    sum: Number,
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
    fn new(clusters: &[Cluster]) -> Result<Self, PerformanceError> {
        let scale = Scale(
            clusters
                .iter()
                .map(|value| value.sum.as_decimal().scale())
                .fold(0_u32, u32::max),
        );
        let values = clusters
            .iter()
            .map(|value| {
                let number = value.sum.as_decimal();
                let distance = scale.0.wrapping_sub(number.scale());
                let factor = 10_i128.wrapping_pow(distance);
                let sum = number
                    .mantissa()
                    .checked_mul(factor)
                    .ok_or(PerformanceError::Arithmetic)?;
                Ok(ScaledCluster {
                    sum: Coefficient(sum),
                    count: value.count,
                })
            })
            .collect::<Result<Vec<_>, PerformanceError>>()?;
        Ok(Self { values, scale })
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
            sum: Number::new(Decimal::ZERO),
            count: Count(0),
        });
        cluster.sum = cluster.sum.add(value.number())?;
        cluster.count.0 = cluster.count.0.wrapping_add(1);
    }
    if days.len() < 30 {
        return Ok(BootstrapInterval::InsufficientSamples);
    }
    let groups = days.into_values().collect::<Vec<_>>();
    let clusters = PreparedClusters::new(&groups)?;
    let mut generator = Generator::new(seed);
    let mut samples = vec![Number::new(Decimal::ZERO); 2000];
    for value in &mut samples {
        *value = sample(&clusters, &mut generator)?;
    }
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
    Ok(BootstrapInterval::Ready(EdgeInterval {
        lower: PerformanceValue::from_number(lower),
        upper: PerformanceValue::from_number(upper),
        direction,
    }))
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
    let value = Decimal::try_from_i128_with_scale(sum.0, clusters.scale.0)
        .map_err(|_error| PerformanceError::Arithmetic)?;
    Number::new(value).div(Number::new(Decimal::from(count.0)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        BootstrapInterval, BootstrapSeed, Cluster, Count, EdgeDirection, Generator,
        PreparedClusters, interval, sample,
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
                "PerformanceValue([REDACTED])"
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
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn sampled_mean_matches_independent_integer_ratio_and_convex_hull(seed in any::<u64>(), values in proptest::collection::vec((-1000_i32..1000_i32,1_usize..5),1..9)) {
            let clusters=values.iter().map(|(value,count)| Cluster { sum:Number::new(Decimal::from(*value).checked_mul(Decimal::from(*count)).expect("작은 정수")),count:Count(*count) }).collect::<Vec<_>>();
            let prepared=PreparedClusters::new(&clusters).expect("공통 정수 자릿수");
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
            let expected=Decimal::from(sum).checked_div(Decimal::from(count)).expect("유리수");
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
        TestRunner::new(Config::with_cases(4096))
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
        assert_eq!(result.lower().to_string(), "11.866667");
        assert_eq!(result.upper().to_string(), "18.000000");
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
                sum: Number::new(Decimal::MAX),
                count: Count(1),
            },
            Cluster {
                sum: Number::new(Decimal::new(1, 28)),
                count: Count(1),
            },
        ];
        assert_eq!(
            PreparedClusters::new(&groups).map(|_value| ()),
            Err(crate::PerformanceError::Arithmetic)
        );
        let empty = PreparedClusters::new(&[]).expect("빈 준비");
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
