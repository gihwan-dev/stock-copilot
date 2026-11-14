use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Number(Decimal);
#[derive(Clone, Copy)]
struct Limit(u32);
impl Number {
    fn add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }
    fn sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
    fn mul(self, rhs: Self) -> Option<Self> {
        self.0.checked_mul(rhs.0).map(Self)
    }
    fn div(self, rhs: Self) -> Option<Self> {
        self.0.checked_div(rhs.0).map(Self)
    }
    fn neg(self) -> Self {
        {
            let mut value = self.0;
            value.set_sign_negative(value.is_sign_positive());
            Self(value)
        }
    }
}
#[derive(Clone, Copy)]
struct Computation(Option<Number>);
impl Computation {
    fn add(self, rhs: Self) -> Self {
        Self(self.0.zip(rhs.0).and_then(|(left, right)| left.add(right)))
    }
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.zip(rhs.0).and_then(|(left, right)| left.sub(right)))
    }
    fn mul(self, rhs: Self) -> Self {
        Self(self.0.zip(rhs.0).and_then(|(left, right)| left.mul(right)))
    }
    fn div(self, rhs: Self) -> Self {
        Self(self.0.zip(rhs.0).and_then(|(left, right)| left.div(right)))
    }
    fn neg(self) -> Self {
        Self(self.0.map(Number::neg))
    }
}
fn constant(value: Number) -> Computation {
    Computation(Some(value))
}
fn ln(value: Computation, limit: Limit) -> Computation {
    let one = constant(Number(Decimal::ONE));
    let ratio = value.sub(one).div(value.add(one));
    let square = ratio.mul(ratio);
    let mut power = ratio;
    let mut sum = constant(Number(Decimal::ZERO));
    for index in 0..limit.0 {
        let denominator = constant(Number(Decimal::from(index)))
            .mul(constant(Number(Decimal::TWO)))
            .add(one);
        let term = power.div(denominator);
        sum = sum.add(term);
        if term.0.is_some_and(|candidate| candidate.0.is_zero()) {
            return sum.mul(constant(Number(Decimal::TWO)));
        }
        power = power.mul(square);
    }
    Computation(None)
}
fn exp(value: Computation, limit: Limit) -> Computation {
    if value
        .0
        .is_some_and(|candidate| candidate.0 < Decimal::from(-64_i32))
    {
        return constant(Number(Decimal::ZERO));
    }
    let reduced = value.div(constant(Number(Decimal::from(1024_u32))));
    let mut term = constant(Number(Decimal::ONE));
    let mut sum = term;
    for index in 1..=limit.0 {
        term = term
            .mul(reduced)
            .div(constant(Number(Decimal::from(index))));
        sum = sum.add(term);
        if term.0.is_some_and(|candidate| candidate.0.is_zero()) {
            for _step in 0_u32..10 {
                sum = sum.mul(sum);
            }
            return sum;
        }
    }
    Computation(None)
}
fn pi() -> Number {
    Number(Decimal::from_i128_with_scale(
        31_415_926_535_897_932_384_626_433_833,
        28,
    ))
}
fn sine(value: Computation, limit: Limit) -> Computation {
    let period = constant(pi()).mul(constant(Number(Decimal::TWO)));
    let reduced = Computation(
        value
            .0
            .zip(period.0)
            .and_then(|(candidate, divisor)| candidate.0.checked_rem(divisor.0).map(Number)),
    );
    let square = reduced.mul(reduced).neg();
    let mut term = reduced;
    let mut sum = term;
    for index in 1..=limit.0 {
        let twice = constant(Number(Decimal::from(index))).mul(constant(Number(Decimal::TWO)));
        let denominator = twice.mul(twice.add(constant(Number(Decimal::ONE))));
        term = term.mul(square).div(denominator);
        sum = sum.add(term);
        if term.0.is_some_and(|candidate| candidate.0.is_zero()) {
            return sum;
        }
    }
    Computation(None)
}
struct SeriesParameters {
    start: Number,
    length: Number,
    prefactor: Number,
    variance: Number,
    initial: Number,
}
fn prepare_series(
    target: Number,
    stop: Number,
    sigma: Number,
    limit: Limit,
) -> Option<SeriesParameters> {
    let one = constant(Number(Decimal::ONE));
    let two = constant(Number(Decimal::TWO));
    let target_value = constant(target);
    let stop_value = constant(stop);
    let start = ln(one.sub(stop_value), limit).neg();
    let length = ln(one.add(target_value).div(one.sub(stop_value)), limit);
    let prefactor = exp(start.sub(length).div(two), limit);
    let variance = constant(sigma).mul(constant(sigma));
    let initial = stop_value.div(target_value.add(stop_value));
    start
        .0
        .zip(length.0)
        .zip(prefactor.0)
        .zip(variance.0)
        .zip(initial.0)
        .map(
            |((((start_value, length_value), prefactor_value), variance_value), initial_value)| {
                SeriesParameters {
                    start: start_value,
                    length: length_value,
                    prefactor: prefactor_value,
                    variance: variance_value,
                    initial: initial_value,
                }
            },
        )
}
fn calculate(
    target: Number,
    stop: Number,
    sigma: Number,
    days: Number,
    limit: Limit,
) -> Option<Number> {
    let parameters = prepare_series(target, stop, sigma, limit)?;
    let two = constant(Number(Decimal::TWO));
    let mut sum = constant(parameters.initial);
    for index in 1..=limit.0 {
        let frequency = constant(Number(Decimal::from(index)))
            .mul(constant(pi()))
            .div(constant(parameters.length));
        let square = frequency.mul(frequency);
        let variance = constant(parameters.variance);
        let decay = variance
            .mul(square)
            .div(two)
            .add(variance.div(constant(Number(Decimal::from(8_u32)))))
            .mul(constant(days));
        if decay
            .0
            .is_some_and(|candidate| candidate.0 >= Decimal::from(64_u32))
        {
            return sum.0;
        }
        let coefficient = two.mul(frequency).div(
            constant(parameters.length).mul(constant(Number(Decimal::new(25, 2))).add(square)),
        );
        let term = constant(parameters.prefactor)
            .mul(coefficient)
            .mul(sine(frequency.mul(constant(parameters.start)), limit))
            .mul(exp(decay.neg(), limit))
            .0?;
        sum = if index.is_multiple_of(2) {
            sum.add(constant(term))
        } else {
            sum.sub(constant(term))
        };
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EstimateDistance(Decimal);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaseProbability(Decimal);
impl EstimateDistance {
    pub(crate) fn as_decimal(&self) -> Decimal {
        self.0
    }
}
impl BaseProbability {
    pub(crate) fn as_decimal(&self) -> Decimal {
        self.0
    }
}
impl std::fmt::Display for EstimateDistance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_decimal().fmt(f)
    }
}
impl std::fmt::Display for BaseProbability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_decimal().fmt(f)
    }
}
pub(crate) fn distances(
    profile: crate::RiskAppetite,
    sigma: crate::DailyVolatility,
    days: crate::Ratio,
) -> Option<(EstimateDistance, EstimateDistance, BaseProbability)> {
    let (target_multiplier, stop_multiplier) = match profile {
        crate::RiskAppetite::Conservative => {
            (Number(Decimal::new(8, 1)), Number(Decimal::new(4, 1)))
        }
        crate::RiskAppetite::Neutral => (Number(Decimal::ONE), Number(Decimal::new(5, 1))),
        crate::RiskAppetite::Aggressive => {
            (Number(Decimal::new(12, 1)), Number(Decimal::new(6, 1)))
        }
    };
    let root = crate::volatility_arithmetic::square_root(days).ok()?;
    let scale = constant(Number(sigma.as_decimal())).mul(constant(Number(root.as_decimal())));
    let pair = constant(target_multiplier)
        .mul(scale)
        .0
        .zip(constant(stop_multiplier).mul(scale).0);
    let (scale_value, (target, stop)) = scale.0.zip(pair)?;
    let boundaries = approve_distances(scale_value, target, stop)?;
    calculate(
        target,
        stop,
        Number(sigma.as_decimal()),
        Number(days.as_decimal()),
        Limit(2048),
    )
    .and_then(approve_probability)
    .map(|approved| (boundaries.0, boundaries.1, approved))
}
fn approve_distances(
    scale: Number,
    target: Number,
    stop: Number,
) -> Option<(EstimateDistance, EstimateDistance)> {
    if scale.0 < Decimal::new(1, 4) || stop.0 > Decimal::new(9, 1) {
        return None;
    }
    Some((EstimateDistance(target.0), EstimateDistance(stop.0)))
}
fn approve_probability(value: Number) -> Option<BaseProbability> {
    if !(Decimal::new(2, 1)..=Decimal::new(4, 1)).contains(&value.0) {
        return None;
    }
    Some(BaseProbability(value.0))
}
#[cfg(test)]
mod tests {
    use super::{
        BaseProbability, EstimateDistance, Limit, Number, approve_distances, approve_probability,
        calculate, constant, distances, exp, ln, sine,
    };
    use crate::{NumericInput, Ratio};
    use rust_decimal::Decimal;
    fn number(text: &str) -> Number {
        Number(
            Ratio::parse(&NumericInput::try_new(text).expect("길이"))
                .expect("숫자")
                .as_decimal(),
        )
    }
    #[test]
    fn design_confirmation_values_preserve_four_percent_decimal_places() {
        for (target, stop, sigma, days, expected) in [
            (
                "0.1",
                "0.05",
                "0.02",
                "20",
                "0.2474753464918288210760968794",
            ),
            (
                "0.1",
                "0.05",
                "0.01",
                "20",
                "0.0315183054289253790417033055",
            ),
            (
                "0.2",
                "0.1",
                "0.02",
                "120",
                "0.3032576591178544776222480224",
            ),
        ] {
            let actual = calculate(
                number(target),
                number(stop),
                number(sigma),
                number(days),
                Limit(2048),
            )
            .expect("급수");
            assert!(
                actual
                    .0
                    .checked_sub(number(expected).0)
                    .expect("차이")
                    .abs()
                    < Decimal::new(1, 18)
            );
        }
    }
    const CASES: [(&str, &str, &str, &str, &str); 81] = [
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.01",
            "5",
            "0.0274892375627886864985196135",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.01",
            "20",
            "0.1728802532157645247292627348",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.01",
            "120",
            "0.1999999447548658822297477684",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.02",
            "5",
            "0.1728802532157645247292627348",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.02",
            "20",
            "0.1999895589495408957038224921",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.02",
            "120",
            "0.1999999999999999999999999998",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.05",
            "5",
            "0.1999999713095234506007611130",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.05",
            "20",
            "0.1999999999999999999999999999",
        ),
        (
            "0.05",
            "0.0125000000000000000000000000",
            "0.05",
            "120",
            "0.2000000000000000000000000000",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.01",
            "5",
            "0.0284008099605269796900555655",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.01",
            "20",
            "0.2430559448953712493513285178",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.01",
            "120",
            "0.3333220184594272635120026376",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.02",
            "5",
            "0.2430559448953712493513285178",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.02",
            "20",
            "0.3329214259117028208166314457",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.02",
            "120",
            "0.3333333333333333332325704396",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.05",
            "5",
            "0.3333261138522767400006942370",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.05",
            "20",
            "0.3333333333333333333166329791",
        ),
        (
            "0.05",
            "0.0250000000000000000000000000",
            "0.05",
            "120",
            "0.3333333333333333333333333333",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.01",
            "5",
            "0.0284093080858074727112622921",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.01",
            "20",
            "0.2636286636445708327003297322",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.01",
            "120",
            "0.4283272578224345679759480214",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.02",
            "5",
            "0.2636286636445708327003297322",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.02",
            "20",
            "0.4252585007536703210130053419",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.02",
            "120",
            "0.4285714285714129028906548792",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.05",
            "5",
            "0.4283951786867583748587539418",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.05",
            "20",
            "0.4285714285714243177046730747",
        ),
        (
            "0.05",
            "0.0375000000000000000000000000",
            "0.05",
            "120",
            "0.4285714285714285714285714285",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.01",
            "5",
            "0.0000192818806434224819910594",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.01",
            "20",
            "0.0304782863676702512882632091",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.01",
            "120",
            "0.1936592225244998770397390851",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.02",
            "5",
            "0.0304782863676702512882632091",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.02",
            "20",
            "0.1753736354713926269484427526",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.02",
            "120",
            "0.1999999685270445611261779491",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.05",
            "5",
            "0.1946485230721058977936393042",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.05",
            "20",
            "0.1999999840316744903474288452",
        ),
        (
            "0.1",
            "0.0250000000000000000000000000",
            "0.05",
            "120",
            "0.1999999999999999999999999999",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.01",
            "5",
            "0.0000192819446620739841442093",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.01",
            "20",
            "0.0315183054289253790417033054",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.01",
            "120",
            "0.2990212437629202803887321780",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.02",
            "5",
            "0.0315183054289253790417033054",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.02",
            "20",
            "0.2474753464918288210760968793",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.02",
            "120",
            "0.3333245479255282303769861722",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.05",
            "5",
            "0.3027435263538836809734330099",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.05",
            "20",
            "0.3333277842526913606588705970",
        ),
        (
            "0.1",
            "0.0500000000000000000000000000",
            "0.05",
            "120",
            "0.3333333333333333333333333333",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.01",
            "5",
            "0.0000192819446620748169694689",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.01",
            "20",
            "0.0315274757653478401269552978",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.01",
            "120",
            "0.3453908896605016260534671855",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.02",
            "5",
            "0.0315274757653478401269552978",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.02",
            "20",
            "0.2683819827016611914687837289",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.02",
            "120",
            "0.4283483593058146774876704823",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.05",
            "5",
            "0.3519513070538801513245348778",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.05",
            "20",
            "0.4284108957322999395214425772",
        ),
        (
            "0.1",
            "0.0750000000000000000000000000",
            "0.05",
            "120",
            "0.4285714285714285714283495667",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.01",
            "5",
            "0.0000000000000003222690906280",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.01",
            "20",
            "0.0000416657579124766898564596",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.01",
            "120",
            "0.0791028434938497608841296521",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.02",
            "5",
            "0.0000416657579124766898564596",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.02",
            "20",
            "0.0365446027620858770786795035",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.02",
            "120",
            "0.1952151965280809698902392774",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.05",
            "5",
            "0.0840070885388798989603873840",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.05",
            "20",
            "0.1960077544923427717961393580",
        ),
        (
            "0.2",
            "0.0500000000000000000000000000",
            "0.05",
            "120",
            "0.1999999999994107211570649980",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.01",
            "5",
            "0.0000000000000003222690906280",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.01",
            "20",
            "0.0000416659295993974832512746",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.01",
            "120",
            "0.0872778554675410966270125311",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.02",
            "5",
            "0.0000416659295993974832512746",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.02",
            "20",
            "0.0378530619892456584990470360",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.02",
            "120",
            "0.3032576591178544776222480223",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.05",
            "5",
            "0.0934762789745568839013852052",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.05",
            "20",
            "0.3066447658302311972427531132",
        ),
        (
            "0.2",
            "0.1000000000000000000000000000",
            "0.05",
            "120",
            "0.3333333246469364276324924654",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.01",
            "5",
            "0.0000000000000003222690906280",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.01",
            "20",
            "0.0000416659295993988640994136",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.01",
            "120",
            "0.0875783673737089978812411659",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.02",
            "5",
            "0.0000416659295993988640994136",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.02",
            "20",
            "0.0378631822994235945441077544",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.02",
            "120",
            "0.3503323668619202616951802572",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.05",
            "5",
            "0.0938712642016128150454121430",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.05",
            "20",
            "0.3565781735091548656150197988",
        ),
        (
            "0.2",
            "0.1500000000000000000000000000",
            "0.05",
            "120",
            "0.4285692501268747852747751197",
        ),
    ];
    #[test]
    fn all_independent_hundred_digit_reference_anchors_match() {
        for (target, stop, sigma, days, expected) in CASES {
            let actual = calculate(
                number(target),
                number(stop),
                number(sigma),
                number(days),
                Limit(2048),
            )
            .expect("전 격자 수렴");
            assert!(
                actual
                    .0
                    .checked_sub(number(expected).0)
                    .expect("차이")
                    .abs()
                    < Decimal::new(1, 18)
            );
        }
    }
    fn rescaled_days_for_equal_variance(
        sigma: Number,
        days: Number,
        sample_sigma: Number,
    ) -> Number {
        let time_variance = sigma
            .0
            .checked_mul(sigma.0)
            .and_then(|variance| variance.checked_mul(days.0))
            .expect("독립 시간분산");
        let sample_variance = sample_sigma
            .0
            .checked_mul(sample_sigma.0)
            .expect("새 일간 분산");
        Number(
            time_variance
                .checked_div(sample_variance)
                .expect("같은 시간분산의 H"),
        )
    }
    #[test]
    fn property_matches_independent_reference_after_variance_preserving_time_rescaling() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    0..CASES.len(),
                    500_000_000_000_000_000_i64..=5_000_000_000_000_000_000_i64,
                ),
                |(index, mantissa)| {
                    let (target, stop, sigma, days, expected) =
                        *CASES.get(index).expect("기준 격자");
                    let sample_sigma = Number(Decimal::new(mantissa, 20));
                    let sample_days =
                        rescaled_days_for_equal_variance(number(sigma), number(days), sample_sigma);
                    let actual = calculate(
                        number(target),
                        number(stop),
                        sample_sigma,
                        sample_days,
                        Limit(2048),
                    )
                    .expect("급수");
                    proptest::prop_assert!(
                        actual
                            .0
                            .checked_sub(number(expected).0)
                            .expect("차이")
                            .abs()
                            < Decimal::new(1, 18),
                        "actual={} expected={}",
                        actual.0,
                        expected
                    );
                    Ok(())
                },
            )
            .expect("독립 100자리 정수 기준");
    }
    #[test]
    fn primitive_independent_values() {
        for (kind, input, expected) in [
            (
                "ln",
                "0.8500000000000000000000000000",
                "-0.1625189294977749131856889583",
            ),
            (
                "ln",
                "1.4117647058823529411764705882",
                "0.3448404862917295393974069834",
            ),
            (
                "exp",
                "-0.1000000000000000000000000000",
                "0.9048374180359595731642490594",
            ),
            (
                "sine",
                "6.0000000000000000000000000000",
                "-0.2794154981989258728115554466",
            ),
            (
                "sine",
                "1.0000000000000000000000000000",
                "0.8414709848078965066525023216",
            ),
            ("sine", "7", "0.6569865987187890903969990916"),
            ("sine", "10", "-0.5440211108893698134047476619"),
        ] {
            let argument = constant(number(input));
            let actual = match kind {
                "ln" => ln(argument, Limit(2048)),
                "exp" => exp(argument, Limit(2048)),
                _ => sine(argument, Limit(2048)),
            }
            .0
            .expect("수렴");
            assert!(
                actual
                    .0
                    .checked_sub(number(expected).0)
                    .expect("차이")
                    .abs()
                    < Decimal::new(1, 22)
            );
        }
    }
    #[test]
    fn supported_distance_boundaries_are_inclusive_and_never_clamped() {
        let low = number("0.0001");
        let low_stop = number("0.00005");
        assert_eq!(
            approve_distances(low, low, low_stop),
            Some((EstimateDistance(low.0), EstimateDistance(low_stop.0)))
        );
        assert_eq!(
            approve_distances(number("0.0000999999999999999999999999"), low, low_stop),
            None
        );
        assert_eq!(
            approve_distances(number("1.8"), number("1.8"), number("0.9")),
            Some((
                EstimateDistance(number("1.8").0),
                EstimateDistance(number("0.9").0)
            ))
        );
        assert_eq!(
            approve_distances(
                number("1.8"),
                number("1.8"),
                number("0.9000000000000000000000000001")
            ),
            None
        );
    }
    #[test]
    fn draft_probability_bounds_and_distance_failures_are_explicit() {
        for value in [
            "0.1999999999999999999999999999",
            "0.4000000000000000000000000001",
        ] {
            assert_eq!(approve_probability(number(value)), None);
        }
        for value in ["0.2", "0.4"] {
            assert_eq!(
                approve_probability(number(value)),
                Some(BaseProbability(number(value).0))
            );
        }
        let volatility =
            crate::Volatility::calculate(&crate::candle::tests::series(&["100", "102", "100"]))
                .expect("일간");
        assert_eq!(
            distances(
                crate::RiskAppetite::Neutral,
                volatility.daily(),
                Ratio::new(Decimal::NEGATIVE_ONE)
            ),
            None
        );
        assert_eq!(
            distances(
                crate::RiskAppetite::Neutral,
                volatility.daily(),
                Ratio::new(Decimal::from(100_000_000_u32))
            ),
            None
        );
        let huge = crate::Volatility::calculate(&crate::candle::tests::series(&[
            "1",
            "390000000000000",
            "1",
        ]))
        .expect("큰 일간");
        assert_eq!(
            distances(
                crate::RiskAppetite::Aggressive,
                huge.daily(),
                Ratio::new(Decimal::MAX)
            ),
            None
        );
        let tiny = crate::Volatility::calculate(&crate::candle::tests::series(&[
            "1000000000",
            "1000000001",
            "1000000000",
        ]))
        .expect("작은 일간");
        assert_eq!(
            distances(
                crate::RiskAppetite::Neutral,
                tiny.daily(),
                Ratio::new(Decimal::new(1, 28))
            ),
            None
        );
    }
    #[test]
    fn invalid_probability_arithmetic_returns_none_without_continuing_the_series() {
        assert_eq!(
            calculate(
                number("0"),
                number("0"),
                number("0"),
                number("0"),
                Limit(2048)
            ),
            None
        );
        assert_eq!(
            calculate(
                number("0.1"),
                number("0.05"),
                Number(Decimal::MAX),
                number("20"),
                Limit(2048)
            ),
            None
        );
        let tiny = number("0.0000000000000000000000000001");
        assert_eq!(
            calculate(tiny, tiny, number("0.02"), number("20"), Limit(2048)),
            None
        );
    }
    #[test]
    fn bounded_functions_reject_exhaustion_and_checked_overflow() {
        assert_eq!(ln(constant(number("2")), Limit(0)).0, None);
        assert_eq!(exp(constant(number("1")), Limit(0)).0, None);
        assert_eq!(sine(constant(number("1")), Limit(0)).0, None);
        assert_eq!(
            exp(constant(number("-65")), Limit(128)).0,
            Some(number("0"))
        );
        assert_eq!(
            exp(constant(number("-64")), Limit(128)).0,
            Some(number("0.0000000000000000000000000002"))
        );
        assert_eq!(exp(constant(number("1000")), Limit(128)).0, None);
        assert_eq!(
            calculate(
                number("0.01"),
                number("0.01"),
                number("0.00000001"),
                number("1"),
                Limit(100)
            ),
            None
        );
        assert_eq!(number("1").div(number("0")), None);
        assert_eq!(Number(Decimal::MAX).add(number("1")), None);
        assert_eq!(Number(Decimal::MIN).sub(number("1")), None);
        assert_eq!(Number(Decimal::MAX).mul(number("2")), None);
    }
}
