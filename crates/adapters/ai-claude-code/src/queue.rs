use crate::quota::{
    BudgetSummaryKind, QuotaConfigError, QuotaDecision, QuotaPolicy, QuotaTracker, QuotaTransition,
    ResetSource, WorkCategory,
};
use application::{
    AiEvent, AiProvider, AiRunId, AiUsage, BackgroundPreemption, ChatQueueAcquisition,
    ChatQueuePort, Clock, EventBus, ResetTime, RetryDelay, SecretValue, SystemFailure, SystemPort,
    SystemPortError, Utilization, WriteCompleted,
};
use domain::{DomainEvent, Timestamp};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

const TOKEN_RANDOM_BYTES: usize = 32_usize;
const ID_RANDOM_BYTES: usize = 8_usize;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(1_u64);

fn read_random_bytes(len: usize) -> Result<Vec<u8>, SystemPortError> {
    let mut file = File::open("/dev/urandom").map_err(|_error| {
        SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )
    })?;
    let mut buffer = vec![0_u8; len];
    file.read_exact(&mut buffer).map_err(|_error| {
        SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )
    })?;
    Ok(buffer)
}

fn encode_hex_bytes(raw: &[u8]) -> Result<Vec<u8>, SystemPortError> {
    let capacity = raw.len().checked_mul(2_usize).ok_or_else(|| {
        SystemPortError::new(SystemPort::SecretStore, SystemFailure::SecretIntegrity)
    })?;
    let mut out = Vec::with_capacity(capacity);
    for &byte in raw {
        let hi = usize::from(byte >> 4_u8);
        let lo = usize::from(byte & 0x0f_u8);
        let hi_char = HEX_DIGITS.get(hi).copied().ok_or_else(|| {
            SystemPortError::new(SystemPort::SecretStore, SystemFailure::SecretIntegrity)
        })?;
        let lo_char = HEX_DIGITS.get(lo).copied().ok_or_else(|| {
            SystemPortError::new(SystemPort::SecretStore, SystemFailure::SecretIntegrity)
        })?;
        out.push(hi_char);
        out.push(lo_char);
    }
    Ok(out)
}

pub fn generate_run_token() -> Result<SecretValue, SystemPortError> {
    let raw = read_random_bytes(TOKEN_RANDOM_BYTES)?;
    let hex_bytes = encode_hex_bytes(&raw)?;
    Ok(SecretValue::new(hex_bytes))
}

pub fn issue_unique_run_id(prefix: &str) -> Result<AiRunId, SystemPortError> {
    let seq = RUN_SEQUENCE.fetch_add(1_u64, Ordering::Relaxed);
    let raw = read_random_bytes(ID_RANDOM_BYTES)?;
    let hex_bytes = encode_hex_bytes(&raw)?;
    let suffix = String::from_utf8(hex_bytes).map_err(|_error| {
        SystemPortError::new(SystemPort::SecretStore, SystemFailure::SecretIntegrity)
    })?;
    let candidate = format!("{prefix}-{seq}-{suffix}");
    AiRunId::try_new(&candidate)
        .map_err(|_error| SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData))
}

pub struct RunCredentials {
    run_id: AiRunId,
    session_id: AiRunId,
    token: SecretValue,
}

impl fmt::Debug for RunCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunCredentials")
            .field("run_id", &self.run_id)
            .field("session_id", &self.session_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl RunCredentials {
    pub fn issue(prefix: &str) -> Result<Self, SystemPortError> {
        let run_id = issue_unique_run_id(prefix)?;
        let session_id = issue_unique_run_id("session")?;
        let token = generate_run_token()?;
        Ok(Self {
            run_id,
            session_id,
            token,
        })
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub const fn session_id(&self) -> &AiRunId {
        &self.session_id
    }

    #[must_use]
    pub const fn token(&self) -> &SecretValue {
        &self.token
    }

    #[must_use]
    pub fn into_parts(self) -> (AiRunId, AiRunId, SecretValue) {
        (self.run_id, self.session_id, self.token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledResumeKind {
    Fresh,
    ResumedAfterChat,
}

#[derive(Debug)]
pub struct ScheduledSlotGrant {
    pub credentials: RunCredentials,
    pub resume_kind: ScheduledResumeKind,
}

#[derive(Debug)]
pub enum ScheduledSlotAdmission {
    Admitted(ScheduledSlotGrant),
    PausedForChat,
    Busy,
    BudgetReached {
        utilization: Utilization,
        cap: Utilization,
    },
    RateLimited {
        resumes_at: Timestamp,
        source: ResetSource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledReleaseOutcome {
    Completed,
    PausedAtStepBoundary,
}

#[derive(Debug)]
pub struct ChatSlotGrant {
    pub credentials: RunCredentials,
    pub preempted_background: BackgroundPreemption,
}

#[derive(Debug)]
pub enum ChatSlotAdmission {
    Acquired(ChatSlotGrant),
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub active_chat: Option<AiRunId>,
    pub active_scheduled: Option<AiRunId>,
    pub pending_resume_job: Option<String>,
    pub budget_summary: BudgetSummaryKind,
    pub active_slot_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduledPauseState {
    Running,
    PauseSignaled,
}

#[derive(Debug, Clone)]
struct ActiveScheduledSlot {
    job_key: String,
    run_id: AiRunId,
    pause_state: ScheduledPauseState,
}

#[derive(Debug)]
struct QueueState {
    active_chat: Option<AiRunId>,
    active_scheduled: Option<ActiveScheduledSlot>,
    pending_resume_job: Option<String>,
    quota: QuotaTracker,
}

#[derive(Debug)]
enum BeginChatDecision {
    Busy,
    AcquiredNoPreempt,
    AcquiredPreempt(AiRunId),
}

impl QueueState {
    const fn new(policy: QuotaPolicy) -> Self {
        Self {
            active_chat: None,
            active_scheduled: None,
            pending_resume_job: None,
            quota: QuotaTracker::new(policy),
        }
    }

    const fn active_slot_count(&self) -> usize {
        let chat_count = if self.active_chat.is_some() {
            1_usize
        } else {
            0_usize
        };
        let scheduled_count = if self.active_scheduled.is_some() {
            1_usize
        } else {
            0_usize
        };
        chat_count.saturating_add(scheduled_count)
    }

    fn begin_chat(&mut self, chat_run_id: AiRunId) -> BeginChatDecision {
        if self.active_chat.is_some() {
            return BeginChatDecision::Busy;
        }
        self.active_chat = Some(chat_run_id);
        match &mut self.active_scheduled {
            Some(slot) if slot.pause_state == ScheduledPauseState::Running => {
                slot.pause_state = ScheduledPauseState::PauseSignaled;
                self.pending_resume_job = Some(slot.job_key.clone());
                BeginChatDecision::AcquiredPreempt(slot.run_id.clone())
            }
            Some(_) | None => BeginChatDecision::AcquiredNoPreempt,
        }
    }

    fn finish_scheduled(&mut self, run: &AiRunId, outcome: ScheduledReleaseOutcome) {
        let Some(slot) = &self.active_scheduled else {
            return;
        };
        if &slot.run_id != run {
            return;
        }
        let job_key = slot.job_key.clone();
        self.active_scheduled = None;
        match outcome {
            ScheduledReleaseOutcome::PausedAtStepBoundary => {
                self.pending_resume_job = Some(job_key);
            }
            ScheduledReleaseOutcome::Completed => {
                if self.pending_resume_job.as_deref() == Some(job_key.as_str()) {
                    self.pending_resume_job = None;
                }
            }
        }
    }
}

pub struct AiRunnerQueue<P: AiProvider> {
    provider: Arc<P>,
    clock: Arc<dyn Clock>,
    event_bus: Arc<EventBus>,
    state: Mutex<QueueState>,
}

impl<P: AiProvider> fmt::Debug for AiRunnerQueue<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiRunnerQueue").finish_non_exhaustive()
    }
}

impl<P: AiProvider> AiRunnerQueue<P> {
    #[must_use]
    pub fn new(
        provider: Arc<P>,
        clock: Arc<dyn Clock>,
        event_bus: Arc<EventBus>,
        policy: QuotaPolicy,
    ) -> Self {
        Self {
            provider,
            clock,
            event_bus,
            state: Mutex::new(QueueState::new(policy)),
        }
    }

    pub fn with_default_policy(
        provider: Arc<P>,
        clock: Arc<dyn Clock>,
        event_bus: Arc<EventBus>,
    ) -> Result<Self, QuotaConfigError> {
        let policy = QuotaPolicy::default_policy()?;
        Ok(Self::new(provider, clock, event_bus, policy))
    }

    pub async fn acquire_chat_slot(&self) -> Result<ChatSlotAdmission, SystemPortError> {
        let credentials = RunCredentials::issue("chat")?;
        let acquisition = self
            .acquire_chat_internal(credentials.run_id().clone())
            .await;
        match acquisition {
            ChatQueueAcquisition::Acquired {
                preempted_background,
            } => Ok(ChatSlotAdmission::Acquired(ChatSlotGrant {
                credentials,
                preempted_background,
            })),
            ChatQueueAcquisition::Busy => Ok(ChatSlotAdmission::Busy),
        }
    }

    pub async fn release_chat_slot(
        &self,
        run: &AiRunId,
    ) -> Result<WriteCompleted, SystemPortError> {
        let mut guard = self.state.lock().await;
        if guard.active_chat.as_ref() == Some(run) {
            guard.active_chat = None;
        }
        Ok(WriteCompleted)
    }

    pub async fn request_scheduled_slot(
        &self,
        job_key: &str,
    ) -> Result<ScheduledSlotAdmission, SystemPortError> {
        let now = self.clock.now();
        let (transition, admission) = {
            let mut guard = self.state.lock().await;
            let transition = guard.quota.refresh(now);
            let decision = guard.quota.evaluate_admission(WorkCategory::Scheduled, now);
            let result = match decision {
                QuotaDecision::WaitRateLimit { resumes_at, source } => {
                    Ok(ScheduledSlotAdmission::RateLimited { resumes_at, source })
                }
                QuotaDecision::WaitScheduledBudget { utilization, cap } => {
                    Ok(ScheduledSlotAdmission::BudgetReached { utilization, cap })
                }
                QuotaDecision::Allowed => admit_scheduled_inner(&mut guard, job_key),
            };
            (transition, result)
        };
        self.publish_if_changed(transition);
        admission
    }

    pub async fn release_scheduled_slot(
        &self,
        run: &AiRunId,
        outcome: ScheduledReleaseOutcome,
    ) -> Result<WriteCompleted, SystemPortError> {
        let mut guard = self.state.lock().await;
        guard.finish_scheduled(run, outcome);
        Ok(WriteCompleted)
    }

    pub async fn observe_ai_event(&self, event: &AiEvent) -> QuotaTransition {
        let now = self.clock.now();
        let transition = {
            let mut guard = self.state.lock().await;
            guard.quota.observe_event(event, now)
        };
        self.publish_if_changed(transition);
        transition
    }

    pub async fn observe_usage(&self, usage: AiUsage) -> QuotaTransition {
        let now = self.clock.now();
        let transition = {
            let mut guard = self.state.lock().await;
            guard.quota.observe_usage(usage, now)
        };
        self.publish_if_changed(transition);
        transition
    }

    pub async fn observe_rate_limited(&self, resets_at: ResetTime) -> QuotaTransition {
        let now = self.clock.now();
        let transition = {
            let mut guard = self.state.lock().await;
            guard.quota.observe_rate_limited(resets_at, now)
        };
        self.publish_if_changed(transition);
        transition
    }

    pub async fn refresh_budget(&self) -> QuotaTransition {
        let now = self.clock.now();
        let transition = {
            let mut guard = self.state.lock().await;
            guard.quota.refresh(now)
        };
        self.publish_if_changed(transition);
        transition
    }

    pub async fn set_scheduled_cap_percent(
        &self,
        cap_percent: u8,
    ) -> Result<QuotaTransition, QuotaConfigError> {
        let now = self.clock.now();
        let transition = {
            let mut guard = self.state.lock().await;
            let current_cooldown = guard.quota.policy().unknown_reset_cooldown();
            let cap = crate::quota::utilization_from_percent(cap_percent)?;
            let updated_policy = QuotaPolicy::try_new(cap, current_cooldown)?;
            guard.quota.update_policy(updated_policy, now)
        };
        self.publish_if_changed(transition);
        Ok(transition)
    }

    pub async fn evaluate_chat_quota(&self) -> QuotaDecision {
        let now = self.clock.now();
        let (transition, decision) = {
            let mut guard = self.state.lock().await;
            let transition = guard.quota.refresh(now);
            let decision = guard.quota.evaluate_admission(WorkCategory::Chat, now);
            (transition, decision)
        };
        self.publish_if_changed(transition);
        decision
    }

    pub async fn snapshot(&self) -> QueueSnapshot {
        let now = self.clock.now();
        let (transition, snap) = {
            let mut guard = self.state.lock().await;
            let transition = guard.quota.refresh(now);
            let summary = guard.quota.budget_summary(now);
            let snap = QueueSnapshot {
                active_chat: guard.active_chat.clone(),
                active_scheduled: guard
                    .active_scheduled
                    .as_ref()
                    .map(|slot| slot.run_id.clone()),
                pending_resume_job: guard.pending_resume_job.clone(),
                budget_summary: summary,
                active_slot_count: guard.active_slot_count(),
            };
            (transition, snap)
        };
        self.publish_if_changed(transition);
        snap
    }

    async fn acquire_chat_internal(&self, chat_run_id: AiRunId) -> ChatQueueAcquisition {
        let decision = {
            let mut guard = self.state.lock().await;
            guard.begin_chat(chat_run_id)
        };
        match decision {
            BeginChatDecision::Busy => ChatQueueAcquisition::Busy,
            BeginChatDecision::AcquiredNoPreempt => ChatQueueAcquisition::Acquired {
                preempted_background: BackgroundPreemption::NoneRunning,
            },
            BeginChatDecision::AcquiredPreempt(scheduled_run_id) => {
                match self.provider.interrupt(&scheduled_run_id).await {
                    Ok(()) | Err(_) => {}
                }
                ChatQueueAcquisition::Acquired {
                    preempted_background: BackgroundPreemption::SignaledPause,
                }
            }
        }
    }

    fn publish_if_changed(&self, transition: QuotaTransition) {
        if transition == QuotaTransition::Changed {
            let _report = self.event_bus.publish(DomainEvent::AiBudgetChanged);
        }
    }
}

fn admit_scheduled_inner(
    guard: &mut QueueState,
    job_key: &str,
) -> Result<ScheduledSlotAdmission, SystemPortError> {
    if guard.active_chat.is_some() {
        return Ok(ScheduledSlotAdmission::PausedForChat);
    }
    if guard.active_scheduled.is_some() {
        return Ok(ScheduledSlotAdmission::Busy);
    }
    let credentials = RunCredentials::issue("scheduled")?;
    let resume_kind = if guard.pending_resume_job.as_deref() == Some(job_key) {
        guard.pending_resume_job = None;
        ScheduledResumeKind::ResumedAfterChat
    } else {
        ScheduledResumeKind::Fresh
    };
    guard.active_scheduled = Some(ActiveScheduledSlot {
        job_key: String::from(job_key),
        run_id: credentials.run_id().clone(),
        pause_state: ScheduledPauseState::Running,
    });
    Ok(ScheduledSlotAdmission::Admitted(ScheduledSlotGrant {
        credentials,
        resume_kind,
    }))
}

impl<P: AiProvider> ChatQueuePort for AiRunnerQueue<P> {
    async fn acquire_chat(&self, run: &AiRunId) -> Result<ChatQueueAcquisition, SystemPortError> {
        Ok(self.acquire_chat_internal(run.clone()).await)
    }

    async fn release_chat(&self, run: &AiRunId) -> Result<WriteCompleted, SystemPortError> {
        self.release_chat_slot(run).await
    }
}
