use crate::fixture_tests::{RecordingJob, code_config, kst, runtime, span_secs};
use crate::{
    BridgeBinding, BridgedSchedulerEvents, DisabledAiJobQueue, JobRegistry, MissedPolicy,
    ScheduleRule, ScheduledJobId, Scheduler, SchedulerEventBridge, StartupPolicy, SyncDemand,
};
use application::EventBusBuilder;
use domain::{ClientOrderId, DomainEvent};
use std::sync::Arc;
use testkit_ports::SteppedClock;

#[test]
fn bridge_forwards_order_events_to_scheduler_after_binding() {
    runtime().block_on(async {
        let bridge = Arc::new(SchedulerEventBridge::new());
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            SchedulerEventBridge::subscribed_kinds(),
            BridgedSchedulerEvents(Arc::clone(&bridge)),
        );
        let bus = builder.build();
        let order = DomainEvent::OrderSubmitted(ClientOrderId::try_new("order-1").expect("키"));
        assert_eq!(bus.publish(order.clone()).failures.len(), 0);
        assert_eq!(bridge.binding(), BridgeBinding::Unbound);
        let mut registry = JobRegistry::new();
        let (job, runs) = RecordingJob::new(
            ScheduledJobId::PortfolioSync,
            code_config(ScheduleRule::Interval(span_secs(3_600)), MissedPolicy::Skip),
            StartupPolicy::WaitForSchedule,
        );
        registry.register(job).expect("등록");
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0)));
        let scheduler =
            Scheduler::new(clock, Arc::new(DisabledAiJobQueue), registry).expect("스케줄러");
        assert_eq!(scheduler.tick_once().await.records.len(), 0);
        bridge.bind(
            scheduler
                .event_subscriber()
                .with_order_sync(Arc::new(SyncDemand::new())),
        );
        assert_eq!(bridge.binding(), BridgeBinding::Bound);
        assert_eq!(bus.publish(order).failures.len(), 0);
        assert_eq!(scheduler.tick_once().await.records.len(), 1);
        assert_eq!(runs.read().await.len(), 1);
    });
}
