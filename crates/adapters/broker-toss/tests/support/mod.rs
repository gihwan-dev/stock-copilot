pub(crate) use tests::{FakeTime, time};
#[cfg(test)]
mod tests {
    use application::Clock;
    use broker_toss::RequestWait;
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };
    use std::time::Duration;

    #[derive(Default)]
    pub(crate) struct FakeTime(AtomicI64);
    impl Clock for FakeTime {
        fn now(&self) -> Timestamp {
            let nanos = self.0.load(Ordering::SeqCst);
            Timestamp::from_parts(
                UnixSeconds::new(
                    1_800_000_000_i64
                        .saturating_add(nanos.checked_div(1_000_000_000).expect("divisor")),
                ),
                Nanosecond::new(u32::try_from(nanos.rem_euclid(1_000_000_000)).expect("nanos")),
                OffsetSeconds::new(0),
            )
            .expect("time")
        }
    }
    impl RequestWait for FakeTime {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(async move {
                self.0.fetch_add(
                    i64::try_from(duration.as_nanos()).expect("duration"),
                    Ordering::SeqCst,
                );
            })
        }
    }
    pub(crate) fn time() -> Arc<FakeTime> {
        Arc::new(FakeTime::default())
    }
}
