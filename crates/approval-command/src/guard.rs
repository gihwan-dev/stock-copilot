use domain::{Nanosecond, ProposalId, SpanSeconds, TimeSpan, Timestamp};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooldownError {
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CooldownSeconds(SpanSeconds);

impl Default for CooldownSeconds {
    fn default() -> Self {
        Self(SpanSeconds::new(60))
    }
}

impl CooldownSeconds {
    pub fn try_new(seconds: SpanSeconds) -> Result<Self, CooldownError> {
        let value = *seconds.as_u64();
        if !(1_u64..=3600_u64).contains(&value) {
            return Err(CooldownError::OutOfRange);
        }
        Ok(Self(seconds))
    }

    #[must_use]
    pub const fn span_seconds(self) -> SpanSeconds {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RequestCount(u32);

impl RequestCount {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }

    fn increment(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogStatus {
    Idle,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiryNotification {
    FirstTime,
    AlreadyNotified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardAcquisitionError {
    DialogInProgress,
    RejectionCooldown { retry_at: Timestamp },
    ClockRollback,
}

#[derive(Debug)]
struct GuardState {
    dialog: DialogStatus,
    rejections: BTreeMap<ProposalId, Timestamp>,
    request_counts: BTreeMap<ProposalId, RequestCount>,
    total_requests: RequestCount,
    expired_notified: BTreeSet<ProposalId>,
}

impl Default for GuardState {
    fn default() -> Self {
        Self {
            dialog: DialogStatus::Idle,
            rejections: BTreeMap::new(),
            request_counts: BTreeMap::new(),
            total_requests: RequestCount::new(0),
            expired_notified: BTreeSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct ApprovalGuard {
    cooldown: CooldownSeconds,
    state: RwLock<GuardState>,
}

impl Default for ApprovalGuard {
    fn default() -> Self {
        Self::new(CooldownSeconds::default())
    }
}

impl ApprovalGuard {
    #[must_use]
    pub fn new(cooldown: CooldownSeconds) -> Self {
        Self {
            cooldown,
            state: RwLock::new(GuardState::default()),
        }
    }

    #[must_use]
    pub const fn cooldown(&self) -> CooldownSeconds {
        self.cooldown
    }

    pub fn try_acquire(
        &self,
        proposal_id: &ProposalId,
        now: Timestamp,
    ) -> Result<DialogLease<'_>, GuardAcquisitionError> {
        let mut state = write_lock(&self.state);
        state.total_requests.increment();
        state
            .request_counts
            .entry(proposal_id.clone())
            .or_default()
            .increment();
        if state.dialog == DialogStatus::Active {
            return Err(GuardAcquisitionError::DialogInProgress);
        }
        if let Some(&rejected_at) = state.rejections.get(proposal_id) {
            check_cooldown(rejected_at, now, self.cooldown)?;
        }
        state.dialog = DialogStatus::Active;
        Ok(DialogLease { guard: self })
    }

    pub fn record_rejection(&self, proposal_id: ProposalId, now: Timestamp) {
        let mut state = write_lock(&self.state);
        let updated = match state.rejections.get(&proposal_id).copied() {
            Some(existing) if existing > now => existing,
            Some(_) | None => now,
        };
        state.rejections.insert(proposal_id, updated);
    }

    #[must_use]
    pub fn mark_expired_once(&self, proposal_id: &ProposalId) -> ExpiryNotification {
        let mut state = write_lock(&self.state);
        if state.expired_notified.insert(proposal_id.clone()) {
            ExpiryNotification::FirstTime
        } else {
            ExpiryNotification::AlreadyNotified
        }
    }

    #[must_use]
    pub fn request_count(&self, proposal_id: &ProposalId) -> RequestCount {
        let state = read_lock(&self.state);
        match state.request_counts.get(proposal_id).copied() {
            Some(count) => count,
            None => RequestCount::new(0),
        }
    }

    #[must_use]
    pub fn total_requests(&self) -> RequestCount {
        read_lock(&self.state).total_requests
    }

    #[must_use]
    pub fn dialog_status(&self) -> DialogStatus {
        read_lock(&self.state).dialog
    }

    fn release_dialog(&self) {
        let mut state = write_lock(&self.state);
        state.dialog = DialogStatus::Idle;
    }
}

#[derive(Debug)]
pub struct DialogLease<'a> {
    guard: &'a ApprovalGuard,
}

impl Drop for DialogLease<'_> {
    fn drop(&mut self) {
        self.guard.release_dialog();
    }
}

fn check_cooldown(
    rejected_at: Timestamp,
    now: Timestamp,
    cooldown: CooldownSeconds,
) -> Result<(), GuardAcquisitionError> {
    if now < rejected_at {
        return Err(GuardAcquisitionError::ClockRollback);
    }
    let Ok(span) = TimeSpan::try_new(cooldown.span_seconds(), Nanosecond::new(0)) else {
        return Err(GuardAcquisitionError::ClockRollback);
    };
    let Ok(retry_at) = rejected_at.checked_add(span) else {
        return Err(GuardAcquisitionError::ClockRollback);
    };
    if now < retry_at {
        return Err(GuardAcquisitionError::RejectionCooldown { retry_at });
    }
    Ok(())
}

fn read_lock(lock: &RwLock<GuardState>) -> RwLockReadGuard<'_, GuardState> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_lock(lock: &RwLock<GuardState>) -> RwLockWriteGuard<'_, GuardState> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
