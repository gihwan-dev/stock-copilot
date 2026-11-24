use crate::{Clock, MarketInfo, SystemPortError, WriteCompleted};
use domain::{KnownMarket, Market, MarketState, MarketTradingStatus, market_state_at};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CollectionTarget {
    KindDisclosure,
    DartList,
    PressRss,
    HankyungRss,
    GoogleNews,
    YahooFinance,
    SecEdgar,
    CentralBankRss,
    TreasuryAndFred,
}

impl CollectionTarget {
    pub const ALL: [Self; 9] = [
        Self::KindDisclosure,
        Self::DartList,
        Self::PressRss,
        Self::HankyungRss,
        Self::GoogleNews,
        Self::YahooFinance,
        Self::SecEdgar,
        Self::CentralBankRss,
        Self::TreasuryAndFred,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CollectionMinutes(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionMinutesError;

impl CollectionMinutes {
    pub const fn try_new(minutes: u16) -> Result<Self, CollectionMinutesError> {
        if minutes == 0 || minutes > 1_440 {
            return Err(CollectionMinutesError);
        }
        Ok(Self(minutes))
    }

    #[must_use]
    pub const fn as_u16(&self) -> &u16 {
        &self.0
    }

    fn doubled(self) -> Self {
        Self(self.0.saturating_mul(2).min(1_440))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionCadence {
    Every(CollectionMinutes),
    AfterUsClose,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionPhase {
    DomesticSession,
    UsSession,
    OffHours,
}

#[must_use]
pub const fn collection_phase(domestic: MarketState, us: MarketState) -> CollectionPhase {
    match (domestic.trading_status(), us.trading_status()) {
        (MarketTradingStatus::Open, _) => CollectionPhase::DomesticSession,
        (MarketTradingStatus::Closed | MarketTradingStatus::Unknown, MarketTradingStatus::Open) => {
            CollectionPhase::UsSession
        }
        (
            MarketTradingStatus::Closed | MarketTradingStatus::Unknown,
            MarketTradingStatus::Closed | MarketTradingStatus::Unknown,
        ) => CollectionPhase::OffHours,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FomcDay {
    Scheduled,
    Regular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartKeyState {
    Configured,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DartCallCount(u32);

impl DartCallCount {
    #[must_use]
    pub const fn new(count: u32) -> Self {
        Self(count)
    }

    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartQuotaLevel {
    BelowHalf,
    HalfOrMore,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DartDailyUsage {
    used: DartCallCount,
    limit: DartCallCount,
}

impl DartDailyUsage {
    #[must_use]
    pub const fn new(used: DartCallCount, limit: DartCallCount) -> Self {
        Self { used, limit }
    }

    #[must_use]
    pub fn level(self) -> DartQuotaLevel {
        if self.limit.0 == 0 || self.used.0 >= self.limit.0 {
            return DartQuotaLevel::Exhausted;
        }
        if u64::from(self.used.0).saturating_mul(2) >= u64::from(self.limit.0) {
            DartQuotaLevel::HalfOrMore
        } else {
            DartQuotaLevel::BelowHalf
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionConditions {
    pub phase: CollectionPhase,
    pub fomc: FomcDay,
    pub dart_key: DartKeyState,
    pub dart_usage: DartDailyUsage,
}

const MIN_2: CollectionMinutes = CollectionMinutes(2);
const MIN_10: CollectionMinutes = CollectionMinutes(10);
const MIN_15: CollectionMinutes = CollectionMinutes(15);
const MIN_30: CollectionMinutes = CollectionMinutes(30);
const MIN_60: CollectionMinutes = CollectionMinutes(60);

const fn by_phase(
    phase: CollectionPhase,
    domestic: CollectionMinutes,
    us: CollectionMinutes,
    other: CollectionMinutes,
) -> CollectionCadence {
    CollectionCadence::Every(match phase {
        CollectionPhase::DomesticSession => domestic,
        CollectionPhase::UsSession => us,
        CollectionPhase::OffHours => other,
    })
}

fn dart_cadence(conditions: &CollectionConditions) -> CollectionCadence {
    if conditions.dart_key == DartKeyState::Missing {
        return CollectionCadence::Off;
    }
    let base = by_phase(conditions.phase, MIN_10, MIN_30, MIN_30);
    match (conditions.dart_usage.level(), base) {
        (DartQuotaLevel::Exhausted, _) => CollectionCadence::Off,
        (DartQuotaLevel::HalfOrMore, CollectionCadence::Every(minutes)) => {
            CollectionCadence::Every(minutes.doubled())
        }
        (
            DartQuotaLevel::BelowHalf | DartQuotaLevel::HalfOrMore,
            CollectionCadence::Every(_) | CollectionCadence::AfterUsClose | CollectionCadence::Off,
        ) => base,
    }
}

#[must_use]
pub fn collection_cadence(
    target: CollectionTarget,
    conditions: &CollectionConditions,
) -> CollectionCadence {
    let phase = conditions.phase;
    match target {
        CollectionTarget::KindDisclosure => by_phase(phase, MIN_2, MIN_30, MIN_30),
        CollectionTarget::DartList => dart_cadence(conditions),
        CollectionTarget::PressRss => by_phase(phase, MIN_10, MIN_30, MIN_30),
        CollectionTarget::HankyungRss => by_phase(phase, MIN_30, MIN_30, MIN_30),
        CollectionTarget::GoogleNews => by_phase(phase, MIN_60, MIN_60, MIN_60),
        CollectionTarget::YahooFinance => by_phase(phase, MIN_60, MIN_30, MIN_60),
        CollectionTarget::SecEdgar => by_phase(phase, MIN_60, MIN_15, MIN_60),
        CollectionTarget::CentralBankRss => match conditions.fomc {
            FomcDay::Scheduled => by_phase(phase, MIN_60, MIN_10, MIN_60),
            FomcDay::Regular => by_phase(phase, MIN_60, MIN_60, MIN_60),
        },
        CollectionTarget::TreasuryAndFred => CollectionCadence::AfterUsClose,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatchUpItems(u16);

impl CatchUpItems {
    #[must_use]
    pub const fn as_u16(&self) -> &u16 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupCatchUp {
    MissedSinceLastRun,
    OneDay(CatchUpItems),
}

#[must_use]
pub const fn startup_catch_up(target: CollectionTarget) -> StartupCatchUp {
    match target {
        CollectionTarget::KindDisclosure => StartupCatchUp::OneDay(CatchUpItems(500)),
        CollectionTarget::DartList
        | CollectionTarget::PressRss
        | CollectionTarget::HankyungRss
        | CollectionTarget::GoogleNews
        | CollectionTarget::YahooFinance
        | CollectionTarget::SecEdgar
        | CollectionTarget::CentralBankRss
        | CollectionTarget::TreasuryAndFred => StartupCatchUp::MissedSinceLastRun,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedCollection {
    pub target: CollectionTarget,
    pub cadence: CollectionCadence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionPlan {
    phase: CollectionPhase,
    entries: Vec<PlannedCollection>,
}

impl CollectionPlan {
    #[must_use]
    pub fn build(conditions: &CollectionConditions) -> Self {
        Self {
            phase: conditions.phase,
            entries: CollectionTarget::ALL
                .into_iter()
                .map(|target| PlannedCollection {
                    target,
                    cadence: collection_cadence(target, conditions),
                })
                .collect(),
        }
    }

    #[must_use]
    pub const fn phase(&self) -> CollectionPhase {
        self.phase
    }

    #[must_use]
    pub fn entries(&self) -> &[PlannedCollection] {
        &self.entries
    }

    #[must_use]
    pub fn cadence(&self, target: CollectionTarget) -> CollectionCadence {
        self.entries
            .iter()
            .find(|entry| entry.target == target)
            .map_or(CollectionCadence::Off, |entry| entry.cadence)
    }
}

pub trait CollectionScheduler: Send + Sync {
    fn apply(
        &self,
        plan: &CollectionPlan,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionInputs {
    pub fomc: FomcDay,
    pub dart_key: DartKeyState,
    pub dart_usage: DartDailyUsage,
}

#[derive(Debug)]
pub struct PlanCollectionSchedule<'a, Info, Scheduler, Time> {
    pub market_info: &'a Info,
    pub scheduler: &'a Scheduler,
    pub clock: &'a Time,
}

impl<Info: MarketInfo, Scheduler: CollectionScheduler, Time: Clock>
    PlanCollectionSchedule<'_, Info, Scheduler, Time>
{
    async fn state(&self, market: KnownMarket, wanted: &Market) -> MarketState {
        match self.market_info.calendar(market).await {
            Ok(calendar) => market_state_at(&[calendar], wanted, self.clock.now()),
            Err(_error) => MarketState::Unknown,
        }
    }

    pub async fn run(&self, inputs: CollectionInputs) -> Result<CollectionPlan, SystemPortError> {
        let domestic = self.state(KnownMarket::Kr, &Market::Kr).await;
        let us = self.state(KnownMarket::Us, &Market::Us).await;
        let plan = CollectionPlan::build(&CollectionConditions {
            phase: collection_phase(domestic, us),
            fomc: inputs.fomc,
            dart_key: inputs.dart_key,
            dart_usage: inputs.dart_usage,
        });
        let _written: WriteCompleted = self.scheduler.apply(&plan).await?;
        Ok(plan)
    }
}
