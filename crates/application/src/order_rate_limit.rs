use crate::{Clock, SystemPortError};
use domain::{
    HourlyOrderLimit, Nanosecond, OrderRateDecision, SpanSeconds, TimeSpan, Timestamp,
    evaluate_hourly_order_limit,
};
use std::future::Future;

pub trait OrderRateRepository: Send + Sync {
    fn hourly_limit(
        &self,
    ) -> impl Future<Output = Result<Option<HourlyOrderLimit>, SystemPortError>> + Send;

    fn recent_submission_times(
        &self,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<Timestamp>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct CheckOrderRateLimit<'a, Repository, Time> {
    repository: &'a Repository,
    clock: &'a Time,
}

impl<'a, Repository: OrderRateRepository, Time: Clock> CheckOrderRateLimit<'a, Repository, Time> {
    #[must_use]
    pub const fn new(repository: &'a Repository, clock: &'a Time) -> Self {
        Self { repository, clock }
    }

    pub async fn evaluate(&self) -> Result<OrderRateDecision, SystemPortError> {
        let now = self.clock.now();
        let limit = self
            .repository
            .hourly_limit()
            .await?
            .unwrap_or_else(HourlyOrderLimit::default);
        let Ok(window) = TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0)) else {
            return Ok(OrderRateDecision::ClockRollback);
        };
        let Ok(since) = now.checked_sub(window) else {
            return Ok(OrderRateDecision::ClockRollback);
        };
        let times = self.repository.recent_submission_times(since).await?;
        Ok(evaluate_hourly_order_limit(limit, now, &times))
    }
}

#[cfg(test)]
mod tests {
    use super::{CheckOrderRateLimit, OrderRateRepository};
    use crate::{Clock, RetryDelay, SystemFailure, SystemPort, SystemPortError};
    use domain::{
        HourlyOrderLimit, Nanosecond, OffsetSeconds, OrderCount, OrderRateDecision, Timestamp,
        UnixSeconds,
    };
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut pinned = Box::pin(future);
        loop {
            match Pin::as_mut(&mut pinned).poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    struct FixedClock(Timestamp);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    struct StubRateRepo {
        limit: Result<Option<HourlyOrderLimit>, SystemPortError>,
        times: Result<Vec<Timestamp>, SystemPortError>,
    }

    impl OrderRateRepository for StubRateRepo {
        fn hourly_limit(
            &self,
        ) -> impl Future<Output = Result<Option<HourlyOrderLimit>, SystemPortError>> + Send
        {
            std::future::ready(self.limit)
        }

        fn recent_submission_times(
            &self,
            _since: Timestamp,
        ) -> impl Future<Output = Result<Vec<Timestamp>, SystemPortError>> + Send {
            std::future::ready(self.times.clone())
        }
    }

    #[test]
    fn check_order_rate_limit_applies_default_cap_and_propagates_storage_failure() {
        let now = at(20_000);
        let clock = FixedClock(now);
        let ten_recent: Vec<Timestamp> = (0_i64..10_i64)
            .map(|idx| at(19_000_i64.saturating_add(idx)))
            .collect();
        let repo_full = StubRateRepo {
            limit: Ok(None),
            times: Ok(ten_recent),
        };
        let decision =
            block_on(CheckOrderRateLimit::new(&repo_full, &clock).evaluate()).expect("상한 평가");
        assert_eq!(
            decision,
            OrderRateDecision::Exceeded {
                limit: HourlyOrderLimit::default(),
                recent_count: OrderCount::new(10),
                retry_at: at(22_600),
            },
            "기본 10건 상한 초과 시 거부"
        );
        let err = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        let repo_err = StubRateRepo {
            limit: Err(err),
            times: Ok(Vec::new()),
        };
        assert_eq!(
            block_on(CheckOrderRateLimit::new(&repo_err, &clock).evaluate()),
            Err(err),
            "저장소 오류는 그대로 전파한다"
        );
    }
}
