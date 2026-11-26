use crate::platform::collector::{
    CollectionPlanner, PLAN_REUSE_SECONDS, PlanFuture, ReusedPlan, off_hours_plan,
};
use application::Clock;
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

struct MovingClock(AtomicI64);

impl Clock for MovingClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(self.0.load(Ordering::SeqCst)),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
}

#[derive(Default)]
struct CountingPlanner(AtomicUsize);

impl CollectionPlanner for CountingPlanner {
    fn plan(&self) -> PlanFuture<'_> {
        let _seen = self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(off_hours_plan()))
    }
}

#[test]
fn the_collection_plan_is_reused_for_a_few_minutes_instead_of_every_tick() {
    let inner = Arc::new(CountingPlanner::default());
    let clock = Arc::new(MovingClock(AtomicI64::new(1_790_000_000)));
    let shared_inner: Arc<dyn CollectionPlanner> = Arc::<CountingPlanner>::clone(&inner);
    let shared_clock: Arc<dyn Clock> = Arc::<MovingClock>::clone(&clock);
    let reused = ReusedPlan::new(shared_inner, shared_clock);
    let block = tauri::async_runtime::block_on;
    assert_eq!(block(reused.plan()), off_hours_plan());
    for _tick in 0_u8..9_u8 {
        let _later = clock.0.fetch_add(30, Ordering::SeqCst);
        let _plan = block(reused.plan());
    }
    assert_eq!(
        inner.0.load(Ordering::SeqCst),
        1,
        "5분 안에는 캘린더를 다시 묻지 않는다"
    );
    let _past = clock.0.fetch_add(PLAN_REUSE_SECONDS, Ordering::SeqCst);
    let _expired = block(reused.plan());
    assert_eq!(inner.0.load(Ordering::SeqCst), 2);
    let _backwards = clock.0.fetch_sub(3_600, Ordering::SeqCst);
    let _rewound = block(reused.plan());
    assert_eq!(
        inner.0.load(Ordering::SeqCst),
        3,
        "시계가 뒤로 가면 다시 세운다"
    );
}
