use crate::{Amount, EstimateDistance, Price, ShareRatio};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Digit(u8);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Natural(Vec<Digit>);
#[derive(Clone, Copy)]
struct Places(u32);
impl Natural {
    fn coefficient(value: Amount) -> Self {
        Self(
            value
                .as_decimal()
                .mantissa()
                .unsigned_abs()
                .to_string()
                .bytes()
                .rev()
                .map(|right_digit| Digit(right_digit.wrapping_sub(b'0')))
                .collect(),
        )
        .normalized()
    }
    fn normalized(mut self) -> Self {
        while self.0.len() > 1 && self.0.last() == Some(&Digit(0)) {
            self.0.truncate(self.0.len().saturating_sub(1));
        }
        self
    }
    fn scaled(&self, places: Places) -> Self {
        let mut result = self.clone();
        for _ in 0..places.0 {
            let mut shifted = vec![Digit(0)];
            shifted.extend_from_slice(&result.0);
            result = Self(shifted);
        }
        result.normalized()
    }
    fn plus(&self, other: &Self) -> Self {
        let mut right = other.0.clone();
        let mut left = self.0.clone();
        let size = left.len().max(right.len());
        left.resize(size, Digit(0));
        right.resize(size, Digit(0));
        let mut carry = Digit(0);
        let mut result = Vec::new();
        for (left_digit, right_digit) in left.iter().zip(right) {
            let total = left_digit
                .0
                .wrapping_add(right_digit.0)
                .wrapping_add(carry.0);
            result.push(Digit(total.wrapping_rem(10)));
            carry = Digit(total.wrapping_div(10));
        }
        if carry.0 != 0 {
            result.push(carry);
        }
        Self(result).normalized()
    }
    fn minus(&self, other: &Self) -> Self {
        let mut right = other.0.clone();
        right.resize(self.0.len(), Digit(0));
        let mut borrow = Digit(0);
        let mut result = Vec::new();
        for (left_digit, right_digit) in self.0.iter().zip(right) {
            let value = left_digit
                .0
                .wrapping_add(10)
                .wrapping_sub(right_digit.0)
                .wrapping_sub(borrow.0);
            result.push(Digit(value.wrapping_rem(10)));
            borrow = if value < 10 { Digit(1) } else { Digit(0) };
        }
        Self(result).normalized()
    }
    fn times(&self, other: &Self) -> Self {
        let mut result = Self(vec![Digit(0)]);
        let mut shifted = self.clone();
        for digit in &other.0 {
            for _ in 0..digit.0 {
                result = result.plus(&shifted);
            }
            shifted = shifted.scaled(Places(1));
        }
        result
    }
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.len().cmp(&other.0.len()).then_with(|| {
            self.0
                .iter()
                .rev()
                .map(|day| day.0)
                .cmp(other.0.iter().rev().map(|day| day.0))
        })
    }
}
#[derive(Clone)]
struct Fraction {
    numerator: Natural,
    denominator: Natural,
}
impl Fraction {
    fn amount(value: Amount) -> Self {
        Self {
            numerator: Natural::coefficient(value),
            denominator: Natural(vec![Digit(1)]).scaled(Places(value.as_decimal().scale())),
        }
    }
    fn times(&self, other: &Self) -> Self {
        Self {
            numerator: self.numerator.times(&other.numerator),
            denominator: self.denominator.times(&other.denominator),
        }
    }
    fn cmp(&self, other: &Self) -> Ordering {
        self.numerator
            .times(&other.denominator)
            .cmp(&other.numerator.times(&self.denominator))
    }
}
#[derive(Clone)]
pub(crate) struct OutcomeBoundaries {
    target: Fraction,
    stop: Fraction,
}
impl OutcomeBoundaries {
    pub(crate) fn new(
        reference: Price,
        target: EstimateDistance,
        stop: EstimateDistance,
        ratios: &[ShareRatio],
    ) -> Self {
        let mut base = Fraction::amount(reference.as_money().amount());
        for ratio in ratios {
            base = base.times(&Fraction {
                numerator: Natural::coefficient(Amount::new(rust_decimal::Decimal::from(
                    *ratio.old_shares().as_u64(),
                ))),
                denominator: Natural::coefficient(Amount::new(rust_decimal::Decimal::from(
                    *ratio.new_shares().as_u64(),
                ))),
            });
        }
        let up = Fraction::amount(Amount::new(target.as_decimal()));
        let down = Fraction::amount(Amount::new(stop.as_decimal()));
        Self {
            target: base.times(&Fraction {
                numerator: up.denominator.plus(&up.numerator),
                denominator: up.denominator,
            }),
            stop: base.times(&Fraction {
                numerator: down.denominator.minus(&down.numerator),
                denominator: down.denominator,
            }),
        }
    }
    pub(crate) fn target_order(&self, price: Price) -> Ordering {
        Fraction::amount(price.as_money().amount()).cmp(&self.target)
    }
    pub(crate) fn stop_order(&self, price: Price) -> Ordering {
        Fraction::amount(price.as_money().amount()).cmp(&self.stop)
    }
}
