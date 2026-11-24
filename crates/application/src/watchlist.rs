use crate::{Clock, DeliveryReport, EventBus, SystemPortError};
use domain::{DomainEvent, InstrumentId, ListingStatus, MasterInstrument, Timestamp};
use std::collections::BTreeSet;
use std::future::Future;

const WATCHLIST_CAPACITY: WatchlistCount = WatchlistCount(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatchlistCount(usize);

impl WatchlistCount {
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchlistMembership {
    Included,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchlistStoreOutcome {
    Stored,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchlistError {
    CapacityExceeded,
    DuplicateInstrument,
    InactiveInstrument,
    ConcurrentUpdate,
    Storage(SystemPortError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchlistEntry {
    instrument: InstrumentId,
    added_at: Timestamp,
}

impl WatchlistEntry {
    #[must_use]
    pub const fn new(instrument: InstrumentId, added_at: Timestamp) -> Self {
        Self {
            instrument,
            added_at,
        }
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub const fn added_at(&self) -> Timestamp {
        self.added_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Watchlist(Vec<WatchlistEntry>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchlistMutation {
    Changed,
    Unchanged,
}

impl Watchlist {
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn try_from_entries(entries: Vec<WatchlistEntry>) -> Result<Self, WatchlistError> {
        if entries.len() > WATCHLIST_CAPACITY.0 {
            return Err(WatchlistError::CapacityExceeded);
        }
        let mut seen = BTreeSet::new();
        for entry in &entries {
            if !seen.insert(entry.instrument.clone()) {
                return Err(WatchlistError::DuplicateInstrument);
            }
        }
        Ok(Self(entries))
    }

    #[must_use]
    pub fn entries(&self) -> &[WatchlistEntry] {
        &self.0
    }

    #[must_use]
    pub fn instruments(&self) -> Vec<InstrumentId> {
        self.0
            .iter()
            .map(|entry| entry.instrument.clone())
            .collect()
    }

    #[must_use]
    pub fn count(&self) -> WatchlistCount {
        WatchlistCount(self.0.len())
    }

    #[must_use]
    pub fn membership(&self, instrument: &InstrumentId) -> WatchlistMembership {
        if self.0.iter().any(|entry| &entry.instrument == instrument) {
            WatchlistMembership::Included
        } else {
            WatchlistMembership::Absent
        }
    }

    fn with_added(
        &self,
        instrument: InstrumentId,
        added_at: Timestamp,
    ) -> Result<(Self, WatchlistMutation), WatchlistError> {
        if self.membership(&instrument) == WatchlistMembership::Included {
            return Ok((self.clone(), WatchlistMutation::Unchanged));
        }
        if self.0.len() >= WATCHLIST_CAPACITY.0 {
            return Err(WatchlistError::CapacityExceeded);
        }
        let mut entries = self.0.clone();
        entries.push(WatchlistEntry::new(instrument, added_at));
        Ok((Self(entries), WatchlistMutation::Changed))
    }

    fn with_removed(&self, instrument: &InstrumentId) -> (Self, WatchlistMutation) {
        if self.membership(instrument) == WatchlistMembership::Absent {
            return (self.clone(), WatchlistMutation::Unchanged);
        }
        let entries = self
            .0
            .iter()
            .filter(|entry| &entry.instrument != instrument)
            .cloned()
            .collect();
        (Self(entries), WatchlistMutation::Changed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingOrigin {
    Holding,
    Watchlist,
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingTarget {
    instrument: InstrumentId,
    origin: TrackingOrigin,
}

impl TrackingTarget {
    #[must_use]
    pub const fn for_holding(instrument: InstrumentId) -> Self {
        Self {
            instrument,
            origin: TrackingOrigin::Holding,
        }
    }

    #[must_use]
    pub const fn for_watchlist(instrument: InstrumentId) -> Self {
        Self {
            instrument,
            origin: TrackingOrigin::Watchlist,
        }
    }

    #[must_use]
    pub const fn for_candidate(instrument: InstrumentId) -> Self {
        Self {
            instrument,
            origin: TrackingOrigin::Candidate,
        }
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub const fn origin(&self) -> TrackingOrigin {
        self.origin
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackedUniverse(Vec<TrackingTarget>);

impl TrackedUniverse {
    #[must_use]
    pub fn compose(
        holdings: &[InstrumentId],
        watchlist: &Watchlist,
        candidates: &[InstrumentId],
    ) -> Self {
        let mut seen = BTreeSet::new();
        let mut targets = Vec::new();
        for instrument in holdings {
            if seen.insert(instrument.clone()) {
                targets.push(TrackingTarget::for_holding(instrument.clone()));
            }
        }
        for entry in watchlist.entries() {
            if seen.insert(entry.instrument.clone()) {
                targets.push(TrackingTarget::for_watchlist(entry.instrument.clone()));
            }
        }
        for instrument in candidates {
            if seen.insert(instrument.clone()) {
                targets.push(TrackingTarget::for_candidate(instrument.clone()));
            }
        }
        Self(targets)
    }

    #[must_use]
    pub fn estimate_targets(&self) -> &[TrackingTarget] {
        &self.0
    }

    #[must_use]
    pub fn monitoring_targets(&self) -> Vec<TrackingTarget> {
        self.0
            .iter()
            .filter(|target| match target.origin {
                TrackingOrigin::Holding | TrackingOrigin::Watchlist => true,
                TrackingOrigin::Candidate => false,
            })
            .cloned()
            .collect()
    }
}

pub trait WatchlistRepository: Send + Sync {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send;
    fn store(
        &self,
        previous: &Watchlist,
        next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchlistOutcome {
    Added {
        watchlist: Watchlist,
        delivery: DeliveryReport,
    },
    AlreadyPresent(Watchlist),
    Removed {
        watchlist: Watchlist,
        delivery: DeliveryReport,
    },
    NotFound(Watchlist),
}

#[derive(Debug)]
pub struct ManageWatchlist<'a, Repository, Time> {
    repository: &'a Repository,
    clock: &'a Time,
    events: &'a EventBus,
}

impl<'a, Repository: WatchlistRepository, Time: Clock> ManageWatchlist<'a, Repository, Time> {
    #[must_use]
    pub const fn new(repository: &'a Repository, clock: &'a Time, events: &'a EventBus) -> Self {
        Self {
            repository,
            clock,
            events,
        }
    }

    pub async fn list(&self) -> Result<Watchlist, WatchlistError> {
        self.repository
            .load()
            .await
            .map_err(WatchlistError::Storage)
    }

    pub async fn add(&self, master: &MasterInstrument) -> Result<WatchlistOutcome, WatchlistError> {
        match master.row().listing_status {
            ListingStatus::Active => {}
            ListingStatus::Scheduled | ListingStatus::Delisted => {
                return Err(WatchlistError::InactiveInstrument);
            }
        }
        let previous = self
            .repository
            .load()
            .await
            .map_err(WatchlistError::Storage)?;
        let (next, mutation) =
            previous.with_added(master.row().instrument_id.clone(), self.clock.now())?;
        match mutation {
            WatchlistMutation::Unchanged => Ok(WatchlistOutcome::AlreadyPresent(previous)),
            WatchlistMutation::Changed => match self
                .repository
                .store(&previous, &next)
                .await
                .map_err(WatchlistError::Storage)?
            {
                WatchlistStoreOutcome::Conflict => Err(WatchlistError::ConcurrentUpdate),
                WatchlistStoreOutcome::Stored => Ok(WatchlistOutcome::Added {
                    watchlist: next,
                    delivery: self.events.publish(DomainEvent::WatchlistChanged),
                }),
            },
        }
    }

    pub async fn remove(
        &self,
        instrument: &InstrumentId,
    ) -> Result<WatchlistOutcome, WatchlistError> {
        let previous = self
            .repository
            .load()
            .await
            .map_err(WatchlistError::Storage)?;
        let (next, mutation) = previous.with_removed(instrument);
        match mutation {
            WatchlistMutation::Unchanged => Ok(WatchlistOutcome::NotFound(previous)),
            WatchlistMutation::Changed => match self
                .repository
                .store(&previous, &next)
                .await
                .map_err(WatchlistError::Storage)?
            {
                WatchlistStoreOutcome::Conflict => Err(WatchlistError::ConcurrentUpdate),
                WatchlistStoreOutcome::Stored => Ok(WatchlistOutcome::Removed {
                    watchlist: next,
                    delivery: self.events.publish(DomainEvent::WatchlistChanged),
                }),
            },
        }
    }

    pub async fn tracking_universe(
        &self,
        holdings: &[InstrumentId],
        candidates: &[InstrumentId],
    ) -> Result<TrackedUniverse, WatchlistError> {
        let watchlist = self.list().await?;
        Ok(TrackedUniverse::compose(holdings, &watchlist, candidates))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ManageWatchlist, TrackedUniverse, TrackingOrigin, TrackingTarget, WATCHLIST_CAPACITY,
        Watchlist, WatchlistEntry, WatchlistError, WatchlistMembership, WatchlistOutcome,
        WatchlistRepository, WatchlistStoreOutcome,
    };
    use crate::{
        Clock, EventBusBuilder, EventSubscriber, SubscriberFailure, SystemFailure, SystemPort,
        SystemPortError,
    };
    use domain::{
        CommonShare, Currency, DisplayInstrumentName, DomainEvent, DomainEventKind, InstrumentId,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds, Symbol,
        Timestamp, TradingSuspension, UnixSeconds,
    };
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("동기 테스트 퓨처"),
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

    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목 ID")
    }

    fn master(instrument: &str, symbol: &str, status: ListingStatus) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: id(instrument),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            name: DisplayInstrumentName::try_new("표본 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: status,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: at(100),
        })
        .expect("마스터")
    }

    struct FixedClock(Timestamp);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    #[derive(Default)]
    struct MemoryWatchlistRepo {
        state: RwLock<Watchlist>,
        conflict: RwLock<bool>,
        fail_load: RwLock<bool>,
    }

    impl WatchlistRepository for MemoryWatchlistRepo {
        fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
            let err = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
            let result = match (self.fail_load.read(), self.state.read()) {
                (Ok(fail), _) if *fail => Err(err),
                (Ok(_), Ok(state)) => Ok(state.clone()),
                _ => Err(err),
            };
            std::future::ready(result)
        }

        fn store(
            &self,
            previous: &Watchlist,
            next: &Watchlist,
        ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
            let err =
                SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
            let result = match (self.conflict.read(), self.state.write()) {
                (Ok(flag), _) if *flag => Ok(WatchlistStoreOutcome::Conflict),
                (Ok(_), Ok(mut guard)) if &*guard == previous => {
                    *guard = next.clone();
                    Ok(WatchlistStoreOutcome::Stored)
                }
                (Ok(_), Ok(_)) => Ok(WatchlistStoreOutcome::Conflict),
                (Err(_), _) | (Ok(_), Err(_)) => Err(err),
            };
            std::future::ready(result)
        }
    }

    struct EventCollector(Arc<RwLock<Vec<DomainEvent>>>);
    impl EventSubscriber for EventCollector {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .write()
                .map_err(|_poison| SubscriberFailure::Unavailable)?
                .push(event.clone());
            Ok(())
        }
    }

    fn bus_with_collector() -> (crate::EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let collected = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::WatchlistChanged]),
            EventCollector(Arc::clone(&collected)),
        );
        (builder.build(), collected)
    }

    #[test]
    fn add_remove_and_idempotency_emit_events_only_on_change() {
        let repo = MemoryWatchlistRepo::default();
        let clock = FixedClock(at(1_000));
        let (bus, events) = bus_with_collector();
        let usecase = ManageWatchlist::new(&repo, &clock, &bus);
        let alpha = master("kr-alpha", "005930", ListingStatus::Active);

        let added = run(usecase.add(&alpha)).expect("추가");
        match added {
            WatchlistOutcome::Added { .. } => {}
            WatchlistOutcome::AlreadyPresent(_)
            | WatchlistOutcome::Removed { .. }
            | WatchlistOutcome::NotFound(_) => panic!("추가 결과 기대"),
        }
        assert_eq!(events.read().expect("읽기").len(), 1);
        assert_eq!(
            events.read().expect("읽기")[0],
            DomainEvent::WatchlistChanged
        );

        let duplicate = run(usecase.add(&alpha)).expect("중복");
        match duplicate {
            WatchlistOutcome::AlreadyPresent(_) => {}
            WatchlistOutcome::Added { .. }
            | WatchlistOutcome::Removed { .. }
            | WatchlistOutcome::NotFound(_) => panic!("중복 결과 기대"),
        }
        assert_eq!(events.read().expect("읽기").len(), 1);

        let listed = run(usecase.list()).expect("조회");
        assert_eq!(*listed.count().as_usize(), 1);
        assert_eq!(
            listed.membership(&id("kr-alpha")),
            WatchlistMembership::Included
        );
        assert_eq!(
            listed.membership(&id("kr-beta")),
            WatchlistMembership::Absent
        );

        let missing = run(usecase.remove(&id("kr-beta"))).expect("없는 제거");
        match missing {
            WatchlistOutcome::NotFound(_) => {}
            WatchlistOutcome::Added { .. }
            | WatchlistOutcome::AlreadyPresent(_)
            | WatchlistOutcome::Removed { .. } => panic!("미존재 결과 기대"),
        }
        assert_eq!(events.read().expect("읽기").len(), 1);

        let removed = run(usecase.remove(&id("kr-alpha"))).expect("제거");
        match removed {
            WatchlistOutcome::Removed { .. } => {}
            WatchlistOutcome::Added { .. }
            | WatchlistOutcome::AlreadyPresent(_)
            | WatchlistOutcome::NotFound(_) => panic!("제거 결과 기대"),
        }
        assert_eq!(events.read().expect("읽기").len(), 2);
        assert_eq!(*run(usecase.list()).expect("빈 목록").count().as_usize(), 0);
    }

    #[test]
    fn rejects_inactive_instruments_duplicates_capacity_and_conflicts() {
        let repo = MemoryWatchlistRepo::default();
        let clock = FixedClock(at(1_000));
        let (bus, events) = bus_with_collector();
        let usecase = ManageWatchlist::new(&repo, &clock, &bus);

        for status in [ListingStatus::Delisted, ListingStatus::Scheduled] {
            let inactive = master("kr-inactive", "005930", status);
            assert_eq!(
                run(usecase.add(&inactive)),
                Err(WatchlistError::InactiveInstrument)
            );
        }
        assert!(events.read().expect("읽기").is_empty());

        let dup_entries = vec![
            WatchlistEntry::new(id("dup"), at(1)),
            WatchlistEntry::new(id("dup"), at(2)),
        ];
        assert_eq!(
            Watchlist::try_from_entries(dup_entries),
            Err(WatchlistError::DuplicateInstrument)
        );

        let full_entries: Vec<_> = (0..*WATCHLIST_CAPACITY.as_usize())
            .map(|index| WatchlistEntry::new(id(&format!("inst-{index}")), at(1)))
            .collect();
        *repo.state.write().expect("채움") =
            Watchlist::try_from_entries(full_entries).expect("상한");
        let extra = master("kr-overflow", "005930", ListingStatus::Active);
        assert_eq!(
            run(usecase.add(&extra)),
            Err(WatchlistError::CapacityExceeded)
        );

        *repo.state.write().expect("초기화") = Watchlist::empty();
        *repo.conflict.write().expect("충돌") = true;
        assert_eq!(
            run(usecase.add(&extra)),
            Err(WatchlistError::ConcurrentUpdate)
        );
        assert!(events.read().expect("읽기").is_empty());
    }

    #[test]
    fn tracking_universe_reflects_watchlist_with_priority_deduplication() {
        let repo = MemoryWatchlistRepo::default();
        let clock = FixedClock(at(2_000));
        let (bus, _) = bus_with_collector();
        let usecase = ManageWatchlist::new(&repo, &clock, &bus);

        let held_and_watched = master("inst-1", "005930", ListingStatus::Active);
        let watched_only = master("inst-2", "000660", ListingStatus::Active);
        run(usecase.add(&held_and_watched)).expect("추가1");
        run(usecase.add(&watched_only)).expect("추가2");

        let universe =
            run(usecase
                .tracking_universe(&[id("inst-1"), id("inst-1")], &[id("inst-2"), id("inst-3")]))
            .expect("대상 조합");

        assert_eq!(
            universe.estimate_targets(),
            &[
                TrackingTarget::for_holding(id("inst-1")),
                TrackingTarget::for_watchlist(id("inst-2")),
                TrackingTarget::for_candidate(id("inst-3")),
            ]
        );
        assert_eq!(
            universe.monitoring_targets(),
            vec![
                TrackingTarget::for_holding(id("inst-1")),
                TrackingTarget::for_watchlist(id("inst-2")),
            ]
        );
        assert_eq!(
            TrackedUniverse::compose(&[], &Watchlist::empty(), &[id("c-1")]).estimate_targets()[0]
                .origin(),
            TrackingOrigin::Candidate
        );
    }
}
