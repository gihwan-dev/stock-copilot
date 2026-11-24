use crate::engine::JobTriggers;
use crate::registry::{JobFuture, ScheduledJob};
use crate::types::{
    JobConfig, JobEnablement, JobError, JobKind, JobRunDetail, MissedPolicy, ScheduleRule,
    ScheduledJobId,
};
use application::{EventSubscriber, SubscriberFailure};
use domain::{CalendarDate, DisclosureKey, DomainEvent, InstrumentId, OffsetSeconds, Timestamp};
use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WatchCause {
    UrgentDisclosure(DisclosureKey),
    PriceMove(InstrumentId),
    NewsSurge(InstrumentId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum WatchEventKey {
    Disclosure(DisclosureKey),
    Daily(WatchCause, CalendarDate),
}

pub trait IntradayReviewPort: Send + Sync {
    fn review(
        &self,
        cause: WatchCause,
        now: Timestamp,
    ) -> impl Future<Output = Result<(), JobError>> + Send;
}

#[derive(Debug, Default)]
struct InboxState {
    pending: Vec<WatchCause>,
    handled: BTreeSet<WatchEventKey>,
}

#[derive(Debug, Default)]
pub struct IntradayWatchInbox {
    state: RwLock<InboxState>,
}

impl IntradayWatchInbox {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, cause: WatchCause) {
        let mut guard = match self.state.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !guard.pending.contains(&cause) {
            guard.pending.push(cause);
        }
    }

    fn take_new(&self, today: CalendarDate) -> Vec<(WatchCause, WatchEventKey)> {
        let mut guard = match self.state.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.handled.retain(|key| match key {
            WatchEventKey::Disclosure(_) => true,
            WatchEventKey::Daily(_, day) => *day >= today,
        });
        let pending = guard.pending.clone();
        guard.pending.clear();
        pending
            .into_iter()
            .map(|cause| {
                let key = match &cause {
                    WatchCause::UrgentDisclosure(disclosure) => {
                        WatchEventKey::Disclosure(disclosure.clone())
                    }
                    WatchCause::PriceMove(_) | WatchCause::NewsSurge(_) => {
                        WatchEventKey::Daily(cause.clone(), today)
                    }
                };
                (cause, key)
            })
            .filter(|(_, key)| !guard.handled.contains(key))
            .collect()
    }

    fn mark_handled(&self, key: WatchEventKey) {
        let mut guard = match self.state.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.handled.insert(key);
    }
}

#[derive(Debug)]
pub struct IntradayWatchSubscriber {
    inbox: Arc<IntradayWatchInbox>,
    triggers: Arc<JobTriggers>,
}

impl IntradayWatchSubscriber {
    #[must_use]
    pub const fn new(inbox: Arc<IntradayWatchInbox>, triggers: Arc<JobTriggers>) -> Self {
        Self { inbox, triggers }
    }

    fn enqueue(&self, cause: WatchCause) {
        self.inbox.push(cause);
        self.triggers.trigger(ScheduledJobId::IntradayWatch);
    }
}

impl EventSubscriber for IntradayWatchSubscriber {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match event {
            DomainEvent::DisclosureTriggerRecorded(key) => {
                self.enqueue(WatchCause::UrgentDisclosure(key.clone()));
                Ok(())
            }
            DomainEvent::PriceMoved(instrument) => {
                self.enqueue(WatchCause::PriceMove(instrument.clone()));
                Ok(())
            }
            DomainEvent::NewsActivityIncreased(instrument) => {
                self.enqueue(WatchCause::NewsSurge(instrument.clone()));
                Ok(())
            }
            DomainEvent::PortfolioSyncRecorded(_)
            | DomainEvent::TransactionsSyncRecorded(_)
            | DomainEvent::PriceUpdated(_)
            | DomainEvent::DisclosureArrived(_)
            | DomainEvent::ProposalCreated(_)
            | DomainEvent::ProposalExpired(_)
            | DomainEvent::ProposalApproved(_)
            | DomainEvent::ProposalRejected(_)
            | DomainEvent::ProposalSkipped(_)
            | DomainEvent::OrderSubmitted(_)
            | DomainEvent::OrderOutcomeUnknown(_)
            | DomainEvent::BrokerConnectionChanged
            | DomainEvent::SettingsChanged
            | DomainEvent::PolicyChanged
            | DomainEvent::OrderSwitchChanged
            | DomainEvent::WatchlistChanged
            | DomainEvent::ThesisUpdated(_)
            | DomainEvent::SelfCheckChanged
            | DomainEvent::MarketSessionChanged(_)
            | DomainEvent::SignalRaised(_)
            | DomainEvent::SignalCleared(_)
            | DomainEvent::EstimateSubmitted(_)
            | DomainEvent::EstimateResolved(_)
            | DomainEvent::ValuationSnapshotTaken(_)
            | DomainEvent::OrderRejected(_)
            | DomainEvent::OrderOutcomeResolved(_)
            | DomainEvent::OrderFilled(_)
            | DomainEvent::OrderCancelled(_)
            | DomainEvent::OrderModified(_)
            | DomainEvent::CollectionFinished(_)
            | DomainEvent::NewsClusterGrew(_)
            | DomainEvent::AiBudgetChanged
            | DomainEvent::AccountSelectionChanged
            | DomainEvent::StoredDataChanged
            | DomainEvent::FxRateUpdated(_)
            | DomainEvent::SourceStatusChanged(_)
            | DomainEvent::BriefingFinished(_)
            | DomainEvent::AiJobFinished(_) => Ok(()),
        }
    }
}

#[derive(Debug)]
pub struct IntradayWatchJob<P> {
    port: Arc<P>,
    inbox: Arc<IntradayWatchInbox>,
}

impl<P: IntradayReviewPort> IntradayWatchJob<P> {
    #[must_use]
    pub const fn new(port: Arc<P>, inbox: Arc<IntradayWatchInbox>) -> Self {
        Self { port, inbox }
    }

    async fn review_pending(&self, now: Timestamp) -> Result<JobRunDetail, JobError> {
        let today = now
            .date_at_offset(OffsetSeconds::new(32_400))
            .map_err(|error| JobError::ExecutionFailed(format!("date: {error:?}")))?;
        let mut reviewed = 0_u32;
        let mut first_failure = None;
        for (cause, key) in self.inbox.take_new(today) {
            match self.port.review(cause, now).await {
                Ok(()) => {
                    self.inbox.mark_handled(key);
                    reviewed = reviewed.saturating_add(1);
                }
                Err(error) => first_failure = first_failure.or(Some(error)),
            }
        }
        match first_failure {
            Some(error) => Err(error),
            None => Ok(JobRunDetail::new("intraday_reviewed", reviewed)),
        }
    }
}

impl<P: IntradayReviewPort> ScheduledJob for IntradayWatchJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::IntradayWatch
    }

    fn default_config(&self) -> JobConfig {
        JobConfig {
            kind: JobKind::AiBackground,
            schedule: ScheduleRule::EventTriggered,
            missed_policy: MissedPolicy::Skip,
            enabled: JobEnablement::Enabled,
        }
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(self.review_pending(now))
    }
}
