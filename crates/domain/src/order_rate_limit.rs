use crate::{Nanosecond, SpanSeconds, TimeSpan, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderCount(u32);

impl OrderCount {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HourlyOrderLimitError {
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HourlyOrderLimit(OrderCount);

impl Default for HourlyOrderLimit {
    fn default() -> Self {
        Self(OrderCount::new(10))
    }
}

impl HourlyOrderLimit {
    pub fn try_new(count: OrderCount) -> Result<Self, HourlyOrderLimitError> {
        let raw = *count.as_u32();
        if !(1_u32..=30_u32).contains(&raw) {
            return Err(HourlyOrderLimitError::OutOfRange);
        }
        Ok(Self(count))
    }

    #[must_use]
    pub const fn limit(self) -> OrderCount {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRateDecision {
    Allowed {
        remaining: OrderCount,
    },
    Exceeded {
        limit: HourlyOrderLimit,
        recent_count: OrderCount,
        retry_at: Timestamp,
    },
    ClockRollback,
}

#[must_use]
pub fn evaluate_hourly_order_limit(
    limit: HourlyOrderLimit,
    now: Timestamp,
    submitted_at: &[Timestamp],
) -> OrderRateDecision {
    let Ok(window) = TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0)) else {
        return OrderRateDecision::ClockRollback;
    };
    let mut recent_count = 0_u32;
    let mut earliest_in_window: Option<Timestamp> = None;
    for &stamp in submitted_at {
        if stamp > now {
            return OrderRateDecision::ClockRollback;
        }
        let Ok(elapsed) = stamp.elapsed_until(now) else {
            return OrderRateDecision::ClockRollback;
        };
        if elapsed < window {
            recent_count = recent_count.saturating_add(1);
            earliest_in_window = match earliest_in_window {
                Some(existing) if existing <= stamp => Some(existing),
                Some(_) | None => Some(stamp),
            };
        }
    }
    let cap = *limit.limit().as_u32();
    if recent_count >= cap {
        let Some(earliest) = earliest_in_window else {
            return OrderRateDecision::ClockRollback;
        };
        let Ok(retry_at) = earliest.checked_add(window) else {
            return OrderRateDecision::ClockRollback;
        };
        return OrderRateDecision::Exceeded {
            limit,
            recent_count: OrderCount::new(recent_count),
            retry_at,
        };
    }
    OrderRateDecision::Allowed {
        remaining: OrderCount::new(cap.saturating_sub(recent_count)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HourlyOrderLimit, HourlyOrderLimitError, OrderCount, OrderRateDecision,
        evaluate_hourly_order_limit,
    };
    use crate::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    #[test]
    fn hourly_limit_defaults_to_ten_and_validates_bounds() {
        assert_eq!(
            HourlyOrderLimit::default().limit(),
            OrderCount::new(10),
            "기본 상한은 시간당 10건"
        );
        assert_eq!(
            HourlyOrderLimit::try_new(OrderCount::new(0)),
            Err(HourlyOrderLimitError::OutOfRange),
            "0건은 불가"
        );
        assert_eq!(
            HourlyOrderLimit::try_new(OrderCount::new(31)),
            Err(HourlyOrderLimitError::OutOfRange),
            "31건은 상한 초과"
        );
        assert_eq!(
            HourlyOrderLimit::try_new(OrderCount::new(1))
                .expect("하한")
                .limit(),
            OrderCount::new(1),
            "하한 1건 허용"
        );
        assert_eq!(
            HourlyOrderLimit::try_new(OrderCount::new(30))
                .expect("상한")
                .limit(),
            OrderCount::new(30),
            "상한 30건 허용"
        );
    }

    #[test]
    fn rolling_one_hour_window_enforces_exact_boundary_and_clock_rollback() {
        let limit = HourlyOrderLimit::try_new(OrderCount::new(2)).expect("2건 상한");
        let now = at(10_000);
        assert_eq!(
            evaluate_hourly_order_limit(limit, now, &[at(6_400), at(6_401)]),
            OrderRateDecision::Allowed {
                remaining: OrderCount::new(1)
            },
            "정확히 3600초 지난 기록은 창에서 제외된다"
        );
        assert_eq!(
            evaluate_hourly_order_limit(limit, now, &[at(6_401), at(9_000)]),
            OrderRateDecision::Exceeded {
                limit,
                recent_count: OrderCount::new(2),
                retry_at: at(10_001),
            },
            "창 안의 2건은 상한 도달로 거부되고 재개 시각을 계산한다"
        );
        assert_eq!(
            evaluate_hourly_order_limit(limit, now, &[at(10_001)]),
            OrderRateDecision::ClockRollback,
            "미래 시각 기록은 보수적으로 거부한다"
        );
    }
}
