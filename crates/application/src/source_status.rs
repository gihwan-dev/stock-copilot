use crate::{
    Clock, DeliveryReport, EventBus, InformationError, SystemFailure, SystemPort, SystemPortError,
    WriteCompleted,
};
use domain::{DataSource, DomainEvent, Timestamp};
use std::{
    collections::BTreeMap,
    fmt::{self, Debug, Formatter},
    future::Future,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    NeverRun,
    Healthy,
    NoData,
    NotConfigured,
    Failed(InformationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAttemptOutcome {
    Succeeded,
    NoData,
    NotConfigured,
    Failed(InformationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatusError {
    InconsistentState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStatus {
    source: DataSource,
    state: SourceState,
    last_success_at: Option<Timestamp>,
    last_attempt_at: Option<Timestamp>,
}

impl SourceStatus {
    #[must_use]
    pub const fn initial(source: DataSource) -> Self {
        Self {
            source,
            state: SourceState::NeverRun,
            last_success_at: None,
            last_attempt_at: None,
        }
    }

    pub const fn try_new(
        source: DataSource,
        state: SourceState,
        last_success_at: Option<Timestamp>,
        last_attempt_at: Option<Timestamp>,
    ) -> Result<Self, SourceStatusError> {
        let valid = match state {
            SourceState::NeverRun => last_success_at.is_none() && last_attempt_at.is_none(),
            SourceState::Healthy | SourceState::NoData => {
                last_success_at.is_some() && last_attempt_at.is_some()
            }
            SourceState::NotConfigured | SourceState::Failed(_) => last_attempt_at.is_some(),
        };
        if !valid {
            return Err(SourceStatusError::InconsistentState);
        }
        Ok(Self {
            source,
            state,
            last_success_at,
            last_attempt_at,
        })
    }

    #[must_use]
    pub const fn apply_attempt(self, at: Timestamp, outcome: SourceAttemptOutcome) -> Self {
        match outcome {
            SourceAttemptOutcome::Succeeded => Self {
                source: self.source,
                state: SourceState::Healthy,
                last_success_at: Some(at),
                last_attempt_at: Some(at),
            },
            SourceAttemptOutcome::NoData => Self {
                source: self.source,
                state: SourceState::NoData,
                last_success_at: Some(at),
                last_attempt_at: Some(at),
            },
            SourceAttemptOutcome::NotConfigured => Self {
                source: self.source,
                state: SourceState::NotConfigured,
                last_success_at: self.last_success_at,
                last_attempt_at: Some(at),
            },
            SourceAttemptOutcome::Failed(error) => Self {
                source: self.source,
                state: SourceState::Failed(error),
                last_success_at: self.last_success_at,
                last_attempt_at: Some(at),
            },
        }
    }

    #[must_use]
    pub const fn source(&self) -> DataSource {
        self.source
    }

    #[must_use]
    pub const fn state(&self) -> SourceState {
        self.state
    }

    #[must_use]
    pub const fn last_success_at(&self) -> Option<Timestamp> {
        self.last_success_at
    }

    #[must_use]
    pub const fn last_attempt_at(&self) -> Option<Timestamp> {
        self.last_attempt_at
    }
}

pub trait SourceStatusRepository: Send + Sync {
    fn find(
        &self,
        source: DataSource,
    ) -> impl Future<Output = Result<Option<SourceStatus>, SystemPortError>> + Send;

    fn list_all(&self) -> impl Future<Output = Result<Vec<SourceStatus>, SystemPortError>> + Send;

    fn save(
        &self,
        status: &SourceStatus,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceStatusReport {
    status: SourceStatus,
    delivery: DeliveryReport,
}

impl SourceStatusReport {
    #[must_use]
    pub const fn new(status: SourceStatus, delivery: DeliveryReport) -> Self {
        Self { status, delivery }
    }

    #[must_use]
    pub const fn status(&self) -> &SourceStatus {
        &self.status
    }

    #[must_use]
    pub const fn delivery(&self) -> &DeliveryReport {
        &self.delivery
    }
}

pub struct RecordSourceStatus<'a, R: SourceStatusRepository> {
    repository: &'a R,
    clock: &'a dyn Clock,
    events: &'a EventBus,
}

impl<R: SourceStatusRepository> Debug for RecordSourceStatus<'_, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordSourceStatus").finish_non_exhaustive()
    }
}

impl<'a, R: SourceStatusRepository> RecordSourceStatus<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R, clock: &'a dyn Clock, events: &'a EventBus) -> Self {
        Self {
            repository,
            clock,
            events,
        }
    }

    pub async fn record(
        &self,
        source: DataSource,
        outcome: SourceAttemptOutcome,
    ) -> Result<SourceStatusReport, SystemPortError> {
        let previous = self
            .repository
            .find(source)
            .await?
            .unwrap_or_else(|| SourceStatus::initial(source));
        if previous.source() != source {
            return Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData,
            ));
        }
        let next = previous.apply_attempt(self.clock.now(), outcome);
        let WriteCompleted = self.repository.save(&next).await?;
        let delivery = self
            .events
            .publish(DomainEvent::SourceStatusChanged(source));
        Ok(SourceStatusReport::new(next, delivery))
    }
}

pub struct GetSourceStatuses<'a, R: SourceStatusRepository> {
    repository: &'a R,
}

impl<R: SourceStatusRepository> Debug for GetSourceStatuses<'_, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("GetSourceStatuses").finish_non_exhaustive()
    }
}

impl<'a, R: SourceStatusRepository> GetSourceStatuses<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub async fn get(&self, source: DataSource) -> Result<SourceStatus, SystemPortError> {
        let found = self.repository.find(source).await?;
        match found {
            Some(status) if status.source() == source => Ok(status),
            Some(_) => Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData,
            )),
            None => Ok(SourceStatus::initial(source)),
        }
    }

    pub async fn list(&self) -> Result<Vec<SourceStatus>, SystemPortError> {
        let stored = self.repository.list_all().await?;
        let mut by_source = BTreeMap::new();
        for status in stored {
            if by_source.insert(status.source(), status).is_some() {
                return Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::InvalidData,
                ));
            }
        }
        Ok(DataSource::all()
            .into_iter()
            .map(|source| {
                by_source
                    .get(&source)
                    .copied()
                    .unwrap_or_else(|| SourceStatus::initial(source))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GetSourceStatuses, RecordSourceStatus, SourceAttemptOutcome, SourceState, SourceStatus,
        SourceStatusError, SourceStatusReport, SourceStatusRepository,
    };
    use crate::{
        Clock, CredentialSource, DeliveryReport, EventBusBuilder, EventSubscriber,
        InformationError, ResetTime, RetryDelay, SubscriberFailure, SystemFailure, SystemPort,
        SystemPortError, WriteCompleted,
    };
    use domain::{
        DataSource, DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds,
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
            mpsc::{Receiver, Sender, channel},
        },
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = Box::pin(future);
        let mut cx = Context::from_waker(Waker::noop());
        match Pin::as_mut(&mut pinned).poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("in-memory future must complete immediately"),
        }
    }

    fn ts(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("timestamp")
    }

    struct FixedClock(AtomicI64);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            ts(self.0.load(Ordering::SeqCst))
        }
    }

    struct EventRecorder(Sender<DomainEvent>);
    impl EventSubscriber for EventRecorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.send(event.clone()) {
                Ok(()) => Ok(()),
                Err(_) => Err(SubscriberFailure::Unavailable),
            }
        }
    }

    enum RepoMode {
        Memory(std::sync::RwLock<BTreeMap<DataSource, SourceStatus>>),
        FindReturns(Result<Option<SourceStatus>, SystemPortError>),
        SaveFails(SystemPortError),
        ListReturns(Result<Vec<SourceStatus>, SystemPortError>),
    }

    struct FakeRepo(RepoMode);
    impl FakeRepo {
        fn memory() -> Self {
            Self(RepoMode::Memory(std::sync::RwLock::new(BTreeMap::new())))
        }
    }

    impl SourceStatusRepository for FakeRepo {
        fn find(
            &self,
            source: DataSource,
        ) -> impl Future<Output = Result<Option<SourceStatus>, SystemPortError>> + Send {
            std::future::ready(match &self.0 {
                RepoMode::Memory(store) => Ok(store.read().expect("read").get(&source).copied()),
                RepoMode::FindReturns(result) => *result,
                RepoMode::SaveFails(_) | RepoMode::ListReturns(_) => Ok(None),
            })
        }

        fn list_all(
            &self,
        ) -> impl Future<Output = Result<Vec<SourceStatus>, SystemPortError>> + Send {
            std::future::ready(match &self.0 {
                RepoMode::Memory(store) => {
                    Ok(store.read().expect("read").values().copied().collect())
                }
                RepoMode::ListReturns(result) => result.clone(),
                RepoMode::FindReturns(_) | RepoMode::SaveFails(_) => Ok(Vec::new()),
            })
        }

        fn save(
            &self,
            status: &SourceStatus,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            std::future::ready(match &self.0 {
                RepoMode::Memory(store) => {
                    store
                        .write()
                        .expect("write")
                        .insert(status.source(), *status);
                    Ok(WriteCompleted)
                }
                RepoMode::SaveFails(error) => Err(*error),
                RepoMode::FindReturns(_) | RepoMode::ListReturns(_) => Ok(WriteCompleted),
            })
        }
    }

    fn bus_with_recorder() -> (crate::EventBus, Receiver<DomainEvent>) {
        let (tx, rx) = channel();
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            BTreeSet::from([DomainEventKind::SourceStatusChanged]),
            EventRecorder(tx),
        );
        (builder.build(), rx)
    }

    #[test]
    fn source_status_invariants_reject_impossible_combinations() {
        let t1 = ts(1_700_000_000);
        let failure = InformationError::Unavailable(ResetTime::NotProvided);
        for (state, success, attempt) in [
            (SourceState::NeverRun, Some(t1), None),
            (SourceState::NeverRun, None, Some(t1)),
            (SourceState::Healthy, None, Some(t1)),
            (SourceState::Healthy, Some(t1), None),
            (SourceState::NoData, None, Some(t1)),
            (SourceState::NoData, Some(t1), None),
            (SourceState::NotConfigured, None, None),
            (SourceState::Failed(failure), Some(t1), None),
        ] {
            assert_eq!(
                SourceStatus::try_new(DataSource::Hankyung, state, success, attempt),
                Err(SourceStatusError::InconsistentState),
                "impossible status must fail"
            );
        }
        assert_eq!(
            SourceStatus::try_new(DataSource::Hankyung, SourceState::NeverRun, None, None),
            Ok(SourceStatus::initial(DataSource::Hankyung))
        );
        for (state, success, attempt) in [
            (SourceState::Healthy, Some(t1), Some(t1)),
            (SourceState::NoData, Some(t1), Some(t1)),
            (SourceState::NotConfigured, None, Some(t1)),
            (SourceState::Failed(failure), Some(t1), Some(t1)),
        ] {
            let valid = SourceStatus::try_new(DataSource::Hankyung, state, success, attempt)
                .expect("valid status");
            assert_eq!(valid.source(), DataSource::Hankyung);
            assert_eq!(valid.state(), state);
            assert_eq!(valid.last_success_at(), success);
            assert_eq!(valid.last_attempt_at(), attempt);
        }
    }

    #[test]
    fn record_source_status_preserves_last_success_on_failure_and_publishes_event() {
        let repo = FakeRepo::memory();
        let clock = Arc::new(FixedClock(AtomicI64::new(1_700_000_100)));
        let (bus, events) = bus_with_recorder();
        let recorder = RecordSourceStatus::new(&repo, clock.as_ref(), &bus);
        let reader = GetSourceStatuses::new(&repo);
        assert!(format!("{recorder:?}").contains("RecordSourceStatus"));
        assert!(format!("{reader:?}").contains("GetSourceStatuses"));
        assert_eq!(
            run(reader.get(DataSource::SecEdgar)).expect("initial"),
            SourceStatus::initial(DataSource::SecEdgar)
        );
        let first = run(recorder.record(DataSource::SecEdgar, SourceAttemptOutcome::Succeeded))
            .expect("first");
        assert_eq!(
            first,
            SourceStatusReport::new(
                SourceStatus::try_new(
                    DataSource::SecEdgar,
                    SourceState::Healthy,
                    Some(ts(1_700_000_100)),
                    Some(ts(1_700_000_100))
                )
                .expect("valid"),
                DeliveryReport::default()
            )
        );
        assert_eq!(first.status().state(), SourceState::Healthy);
        assert_eq!(first.delivery(), &DeliveryReport::default());
        clock.0.store(1_700_000_200, Ordering::SeqCst);
        let err = InformationError::InvalidCredential(CredentialSource::Dart);
        let failed = run(recorder.record(DataSource::SecEdgar, SourceAttemptOutcome::Failed(err)))
            .expect("failed");
        assert_eq!(failed.status().state(), SourceState::Failed(err));
        assert_eq!(failed.status().last_success_at(), Some(ts(1_700_000_100)));
        assert_eq!(failed.status().last_attempt_at(), Some(ts(1_700_000_200)));
        clock.0.store(1_700_000_300, Ordering::SeqCst);
        let unconfigured =
            run(recorder.record(DataSource::SecEdgar, SourceAttemptOutcome::NotConfigured))
                .expect("not configured");
        assert_eq!(unconfigured.status().state(), SourceState::NotConfigured);
        assert_eq!(
            unconfigured.status().last_success_at(),
            Some(ts(1_700_000_100))
        );
        assert_eq!(
            unconfigured.status().last_attempt_at(),
            Some(ts(1_700_000_300))
        );
        clock.0.store(1_700_000_400, Ordering::SeqCst);
        let no_data = run(recorder.record(DataSource::SecEdgar, SourceAttemptOutcome::NoData))
            .expect("no data");
        assert_eq!(no_data.status().state(), SourceState::NoData);
        assert_eq!(no_data.status().last_success_at(), Some(ts(1_700_000_400)));
        assert_eq!(no_data.status().last_attempt_at(), Some(ts(1_700_000_400)));
        let received: Vec<_> = events.try_iter().collect();
        assert_eq!(
            received,
            vec![DomainEvent::SourceStatusChanged(DataSource::SecEdgar); 4]
        );
    }

    #[test]
    fn get_source_statuses_lists_every_source_in_canonical_order() {
        let repo = FakeRepo::memory();
        let clock = FixedClock(AtomicI64::new(1_700_000_400));
        let (bus, _events) = bus_with_recorder();
        let recorder = RecordSourceStatus::new(&repo, &clock, &bus);
        let reader = GetSourceStatuses::new(&repo);
        let saved = run(recorder.record(DataSource::SecEdgar, SourceAttemptOutcome::NoData))
            .expect("saved");
        assert_eq!(
            run(reader.get(DataSource::SecEdgar)).expect("stored"),
            *saved.status()
        );
        let all = run(reader.list()).expect("list");
        let sources: Vec<_> = all.iter().map(SourceStatus::source).collect();
        assert_eq!(sources, DataSource::all().to_vec());
        assert_eq!(
            all.iter()
                .find(|entry| entry.source() == DataSource::SecEdgar)
                .copied(),
            Some(*saved.status())
        );
        assert_eq!(
            all.iter()
                .find(|entry| entry.source() == DataSource::Hankyung)
                .copied(),
            Some(SourceStatus::initial(DataSource::Hankyung))
        );
    }

    #[test]
    fn storage_failure_or_corruption_never_publishes_event() {
        let clock = FixedClock(AtomicI64::new(1_700_000_500));
        let (bus, events) = bus_with_recorder();
        let save_err = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        let fail_repo = FakeRepo(RepoMode::SaveFails(save_err));
        assert_eq!(
            run(RecordSourceStatus::new(&fail_repo, &clock, &bus)
                .record(DataSource::Fred, SourceAttemptOutcome::Succeeded)),
            Err(save_err)
        );
        assert!(events.try_iter().next().is_none());
        let mismatched = FakeRepo(RepoMode::FindReturns(Ok(Some(SourceStatus::initial(
            DataSource::Hankyung,
        )))));
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        assert_eq!(
            run(RecordSourceStatus::new(&mismatched, &clock, &bus)
                .record(DataSource::Fred, SourceAttemptOutcome::Succeeded)),
            Err(invalid)
        );
        assert_eq!(
            run(GetSourceStatuses::new(&mismatched).get(DataSource::Fred)),
            Err(invalid)
        );
        assert!(events.try_iter().next().is_none());
        let find_err =
            SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        let find_fail_repo = FakeRepo(RepoMode::FindReturns(Err(find_err)));
        assert_eq!(
            run(RecordSourceStatus::new(&find_fail_repo, &clock, &bus)
                .record(DataSource::Fred, SourceAttemptOutcome::Succeeded)),
            Err(find_err)
        );
        assert_eq!(
            run(GetSourceStatuses::new(&find_fail_repo).get(DataSource::Fred)),
            Err(find_err)
        );
        let duplicate = FakeRepo(RepoMode::ListReturns(Ok(vec![
            SourceStatus::initial(DataSource::Fred),
            SourceStatus::initial(DataSource::Fred),
        ])));
        assert_eq!(run(GetSourceStatuses::new(&duplicate).list()), Err(invalid));
        let list_err =
            SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
        let list_fail_repo = FakeRepo(RepoMode::ListReturns(Err(list_err)));
        assert_eq!(
            run(GetSourceStatuses::new(&list_fail_repo).list()),
            Err(list_err)
        );
    }
}
