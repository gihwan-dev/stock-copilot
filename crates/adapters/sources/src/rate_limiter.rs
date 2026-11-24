use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use application::Clock;
use tokio::sync::{Mutex, MutexGuard};

use crate::{RateLimitBucket, SourceHttpError};

pub trait SourceWait: Send + Sync {
    fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

#[derive(Debug)]
pub struct TokioSourceWait;

impl SourceWait for TokioSourceWait {
    fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

const MAX_CLOCK_NANOS: i128 = 253_402_300_799_999_999_999;

pub(crate) struct BucketBudget {
    min_interval_nanos: i128,
    last: Option<i128>,
    blocked_until: i128,
}

impl BucketBudget {
    fn new(bucket: RateLimitBucket) -> Self {
        let min_interval = bucket.min_interval();
        let interval = i128::from(min_interval.as_secs())
            .saturating_mul(1_000_000_000)
            .saturating_add(i128::from(min_interval.subsec_nanos()));
        let floor = match bucket.max_calls_per_second() {
            Some(limit) => 1_000_000_000_i128
                .checked_div(i128::from(limit.max(1)))
                .unwrap_or(1_000_000_000),
            None => 1,
        };
        Self {
            min_interval_nanos: interval.max(floor),
            last: None,
            blocked_until: i128::MIN,
        }
    }

    fn deadline(&self) -> i128 {
        self.last.map_or(self.blocked_until, |last| {
            last.saturating_add(self.min_interval_nanos)
                .max(self.blocked_until)
        })
    }
}

pub(crate) struct SourceRateLimiter {
    clock: Arc<dyn Clock>,
    wait: Arc<dyn SourceWait>,
    buckets: [Mutex<BucketBudget>; 14],
}

impl SourceRateLimiter {
    pub(crate) fn new(clock: Arc<dyn Clock>, wait: Arc<dyn SourceWait>) -> Self {
        Self {
            clock,
            wait,
            buckets: RateLimitBucket::ALL.map(|bucket| Mutex::new(BucketBudget::new(bucket))),
        }
    }

    fn now(&self) -> i128 {
        let current = self.clock.now();
        i128::from(*current.utc_seconds().as_i64())
            .saturating_mul(1_000_000_000)
            .saturating_add(i128::from(*current.nanosecond().as_u32()))
    }

    pub(crate) async fn lock(&self, bucket: RateLimitBucket) -> MutexGuard<'_, BucketBudget> {
        let [
            kind,
            open_dart,
            dart_rss,
            hankyung,
            maekyung,
            yonhap,
            edaily,
            google_news,
            yahoo_finance,
            sec_edgar,
            bank_of_korea,
            federal_reserve,
            us_treasury,
            fred,
        ] = &self.buckets;
        match bucket {
            RateLimitBucket::Kind => kind,
            RateLimitBucket::OpenDart => open_dart,
            RateLimitBucket::DartRss => dart_rss,
            RateLimitBucket::Hankyung => hankyung,
            RateLimitBucket::Maekyung => maekyung,
            RateLimitBucket::Yonhap => yonhap,
            RateLimitBucket::Edaily => edaily,
            RateLimitBucket::GoogleNews => google_news,
            RateLimitBucket::YahooFinance => yahoo_finance,
            RateLimitBucket::SecEdgar => sec_edgar,
            RateLimitBucket::BankOfKorea => bank_of_korea,
            RateLimitBucket::FederalReserve => federal_reserve,
            RateLimitBucket::UsTreasury => us_treasury,
            RateLimitBucket::Fred => fred,
        }
        .lock()
        .await
    }

    pub(crate) async fn admit(&self, budget: &mut BucketBudget) -> Result<(), SourceHttpError> {
        loop {
            let now = self.now();
            let deadline = budget.deadline();
            if deadline > MAX_CLOCK_NANOS {
                return Err(SourceHttpError::RateLimited);
            }
            if now >= deadline {
                budget.last = Some(now);
                return Ok(());
            }
            let remaining = deadline.saturating_sub(now);
            let duration = u64::try_from(remaining.min(86_400_000_000_000))
                .map_or(Duration::from_hours(24), Duration::from_nanos);
            self.wait.wait(duration).await;
        }
    }

    pub(crate) fn mark_finished(&self, budget: &mut BucketBudget) {
        let completed = self.now();
        budget.last = Some(
            budget
                .last
                .map_or(completed, |previous| previous.max(completed)),
        );
    }

    pub(crate) fn defer_seconds(&self, budget: &mut BucketBudget, retry_after_seconds: u64) {
        let wait_nanos = i128::from(retry_after_seconds).saturating_mul(1_000_000_000);
        let target = self.now().saturating_add(wait_nanos);
        budget.blocked_until = budget.blocked_until.max(target);
    }
}

#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::Duration;

    use application::Clock;
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

    use super::{SourceRateLimiter, SourceWait};
    use crate::{RateLimitBucket, SourceHttpError};

    struct FixedClock(Timestamp);

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    struct NoopWait;

    impl SourceWait for NoopWait {
        fn wait(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(ready(()))
        }
    }

    #[test]
    fn different_buckets_lock_independently_on_same_rate_limiter() {
        let now = Timestamp::from_parts(
            UnixSeconds::new(1_760_000_000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("timestamp");
        let limiter = SourceRateLimiter::new(Arc::new(FixedClock(now)), Arc::new(NoopWait));
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        rt.block_on(async {
            let mut kind = limiter.lock(RateLimitBucket::Kind).await;
            let mut fed = limiter.lock(RateLimitBucket::FederalReserve).await;
            assert_eq!(limiter.admit(&mut kind).await, Ok(()));
            assert_eq!(limiter.admit(&mut fed).await, Ok(()));
            limiter.defer_seconds(&mut kind, u64::MAX);
            assert_eq!(
                limiter.admit(&mut kind).await,
                Err(SourceHttpError::RateLimited)
            );
        });
    }
}
