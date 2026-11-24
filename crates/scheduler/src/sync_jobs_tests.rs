use crate::fixture_tests::{kr_calendar_with_late_close, kst, runtime, span_secs};
use crate::{
    JobError, MarketCalendarSource, MissedPolicy, NoMarketCalendar, PortfolioSyncJob,
    PortfolioSyncPort, ScheduleRule, ScheduledJob, ScheduledJobId, StartupPolicy, SyncDemand,
    TransactionSyncPort, TransactionSyncTally, TransactionsSyncJob,
};
use crate::{SchedulerEventSubscriber, SchedulerWakeSignal};
use application::EventSubscriber;
use domain::{ClientOrderId, DomainEvent, KnownMarket, MarketCalendar, Timestamp};
use std::sync::Arc;
use tokio::sync::RwLock;

struct BothMarkets(MarketCalendar);

impl MarketCalendarSource for BothMarkets {
    fn calendar(&self, _market: KnownMarket) -> Option<MarketCalendar> {
        Some(self.0.clone())
    }
}

fn both_markets() -> Arc<dyn MarketCalendarSource> {
    Arc::new(BothMarkets(kr_calendar_with_late_close()))
}

#[derive(Default)]
struct CountingPortfolio {
    calls: RwLock<Vec<Timestamp>>,
    failure: Option<JobError>,
}

impl PortfolioSyncPort for CountingPortfolio {
    async fn sync_all(&self, now: Timestamp) -> Result<u32, JobError> {
        self.calls.write().await.push(now);
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(1),
        }
    }
}

#[derive(Default)]
struct CountingTransactions {
    calls: RwLock<Vec<Timestamp>>,
}

impl TransactionSyncPort for CountingTransactions {
    async fn sync_all(&self, now: Timestamp) -> Result<TransactionSyncTally, JobError> {
        self.calls.write().await.push(now);
        Ok(TransactionSyncTally {
            recent_synced: 2,
            full_inspections_completed: 1,
        })
    }
}

#[test]
fn portfolio_sync_runs_every_five_minutes_from_startup() {
    let port = Arc::new(CountingPortfolio::default());
    let job = PortfolioSyncJob::new(
        port,
        Arc::new(NoMarketCalendar),
        Arc::new(SyncDemand::new()),
    );
    let config = job.default_config();
    assert_eq!(job.id(), ScheduledJobId::PortfolioSync);
    assert_eq!(job.startup_policy(), StartupPolicy::RunAtStartup);
    assert_eq!(config.missed_policy, MissedPolicy::Skip);
    assert_eq!(config.schedule, ScheduleRule::Interval(span_secs(300)));
}

#[test]
fn portfolio_sync_skips_closed_market_after_startup_and_runs_after_orders() {
    runtime().block_on(async {
        let port = Arc::new(CountingPortfolio::default());
        let demand = Arc::new(SyncDemand::new());
        let job = PortfolioSyncJob::new(Arc::clone(&port), both_markets(), Arc::clone(&demand));
        let subscriber = SchedulerEventSubscriber::new(Arc::new(SchedulerWakeSignal::new()))
            .with_order_sync(Arc::clone(&demand));
        let evening = kst(2026, 11, 25, 20, 0);
        let first = job.run(evening).await.expect("시작 동기화");
        assert_eq!(first.summary_code, "portfolio_synced");
        let closed = job.run(evening).await.expect("장 마감");
        assert_eq!(closed.summary_code, "market_closed");
        assert_eq!(port.calls.read().await.len(), 1);
        subscriber
            .on_event(&DomainEvent::OrderSubmitted(
                ClientOrderId::try_new("order-1").expect("주문 키"),
            ))
            .expect("구독");
        subscriber
            .on_event(&DomainEvent::SettingsChanged)
            .expect("무시");
        let after_order = job.run(evening).await.expect("주문 직후");
        assert_eq!(after_order.summary_code, "portfolio_synced");
        let again = job.run(evening).await.expect("다시 마감");
        assert_eq!(again.summary_code, "market_closed");
        assert_eq!(port.calls.read().await.len(), 2);
    });
}

#[test]
fn portfolio_sync_runs_while_market_is_open_or_unknown() {
    runtime().block_on(async {
        let port = Arc::new(CountingPortfolio::default());
        let open = PortfolioSyncJob::new(
            Arc::clone(&port),
            both_markets(),
            Arc::new(SyncDemand::new()),
        );
        open.run(kst(2026, 11, 25, 10, 0)).await.expect("시작");
        let during = open.run(kst(2026, 11, 25, 10, 5)).await.expect("장중");
        assert_eq!(during.summary_code, "portfolio_synced");
        let unknown = PortfolioSyncJob::new(
            Arc::clone(&port),
            Arc::new(NoMarketCalendar),
            Arc::new(SyncDemand::new()),
        );
        unknown.run(kst(2026, 11, 25, 22, 0)).await.expect("시작");
        let fallback = unknown.run(kst(2026, 11, 25, 22, 5)).await.expect("모름");
        assert_eq!(fallback.summary_code, "portfolio_synced");
        assert_eq!(port.calls.read().await.len(), 4);
    });
}

#[test]
fn portfolio_sync_failure_is_reported_as_job_failure() {
    runtime().block_on(async {
        let port = Arc::new(CountingPortfolio {
            calls: RwLock::new(Vec::new()),
            failure: Some(JobError::StorageFailure(String::from("down"))),
        });
        let job = PortfolioSyncJob::new(port, both_markets(), Arc::new(SyncDemand::new()));
        let failed = job.run(kst(2026, 11, 25, 10, 0)).await;
        assert_eq!(failed, Err(JobError::StorageFailure(String::from("down"))));
    });
}

#[test]
fn transactions_sync_runs_every_thirty_minutes_and_skips_closed_market() {
    runtime().block_on(async {
        let port = Arc::new(CountingTransactions::default());
        let job = TransactionsSyncJob::new(Arc::clone(&port), both_markets());
        assert_eq!(job.id(), ScheduledJobId::TransactionsSync);
        assert_eq!(job.startup_policy(), StartupPolicy::RunAtStartup);
        assert_eq!(
            job.default_config().schedule,
            ScheduleRule::Interval(span_secs(1_800))
        );
        let evening = kst(2026, 11, 25, 20, 0);
        let first = job.run(evening).await.expect("시작");
        assert_eq!(first.summary_code, "transactions_synced");
        assert_eq!(first.processed_items, 2);
        let closed = job.run(evening).await.expect("마감");
        assert_eq!(closed.summary_code, "market_closed");
        let open = job.run(kst(2026, 11, 26, 10, 0)).await.expect("장중");
        assert_eq!(open.summary_code, "transactions_synced");
        assert_eq!(port.calls.read().await.len(), 2);
    });
}
