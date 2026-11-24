use application::{AiEvent, AiUsage, ResetTime, Utilization};
use domain::{Amount, AmountInput, Nanosecond, Probability, SpanSeconds, TimeSpan, Timestamp};

pub const DEFAULT_UNKNOWN_RESET_COOLDOWN_SECONDS: u64 = 300_u64;
pub const DEFAULT_SCHEDULED_CAP_PERCENT: u8 = 70_u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaConfigError {
    InvalidCapPercent,
    InvalidCooldown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaPolicy {
    scheduled_cap: Utilization,
    unknown_reset_cooldown: TimeSpan,
}

impl QuotaPolicy {
    pub fn try_new(
        scheduled_cap: Utilization,
        unknown_reset_cooldown: TimeSpan,
    ) -> Result<Self, QuotaConfigError> {
        let (seconds, nanos) = unknown_reset_cooldown.as_parts();
        if *seconds.as_u64() == 0_u64 && *nanos.as_u32() == 0_u32 {
            return Err(QuotaConfigError::InvalidCooldown);
        }
        Ok(Self {
            scheduled_cap,
            unknown_reset_cooldown,
        })
    }

    pub fn from_percent_and_cooldown_seconds(
        cap_percent: u8,
        cooldown_seconds: u64,
    ) -> Result<Self, QuotaConfigError> {
        let cap = utilization_from_percent(cap_percent)?;
        let cooldown =
            TimeSpan::try_new(SpanSeconds::new(cooldown_seconds), Nanosecond::new(0_u32))
                .map_err(|_error| QuotaConfigError::InvalidCooldown)?;
        Self::try_new(cap, cooldown)
    }

    pub fn default_policy() -> Result<Self, QuotaConfigError> {
        Self::from_percent_and_cooldown_seconds(
            DEFAULT_SCHEDULED_CAP_PERCENT,
            DEFAULT_UNKNOWN_RESET_COOLDOWN_SECONDS,
        )
    }

    #[must_use]
    pub const fn scheduled_cap(&self) -> Utilization {
        self.scheduled_cap
    }

    #[must_use]
    pub const fn unknown_reset_cooldown(&self) -> TimeSpan {
        self.unknown_reset_cooldown
    }
}

pub fn utilization_from_percent(percent: u8) -> Result<Utilization, QuotaConfigError> {
    if percent > 100_u8 {
        return Err(QuotaConfigError::InvalidCapPercent);
    }
    let text = if percent == 100_u8 {
        String::from("1")
    } else if percent == 0_u8 {
        String::from("0")
    } else {
        format!("0.{percent:02}")
    };
    let input = AmountInput::new(&text);
    let amount = Amount::parse(&input).map_err(|_error| QuotaConfigError::InvalidCapPercent)?;
    let probability = Probability::try_new(amount.as_decimal())
        .map_err(|_error| QuotaConfigError::InvalidCapPercent)?;
    Ok(Utilization::new(probability))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkCategory {
    Chat,
    Scheduled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetSource {
    ProviderReset,
    FallbackCooldown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaState {
    Unknown,
    Available {
        utilization: Utilization,
        observed_at: Timestamp,
    },
    Waiting {
        resumes_at: Timestamp,
        source: ResetSource,
        observed_at: Timestamp,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaTransition {
    Unchanged,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetSummaryKind {
    Normal,
    ScheduledCapReached,
    RateLimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Allowed,
    WaitScheduledBudget {
        utilization: Utilization,
        cap: Utilization,
    },
    WaitRateLimit {
        resumes_at: Timestamp,
        source: ResetSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaTracker {
    policy: QuotaPolicy,
    state: QuotaState,
}

impl QuotaTracker {
    #[must_use]
    pub const fn new(policy: QuotaPolicy) -> Self {
        Self {
            policy,
            state: QuotaState::Unknown,
        }
    }

    #[must_use]
    pub const fn policy(&self) -> QuotaPolicy {
        self.policy
    }

    pub fn update_policy(&mut self, policy: QuotaPolicy, now: Timestamp) -> QuotaTransition {
        let before = self.budget_summary_raw();
        let _refreshed = self.refresh(now);
        self.policy = policy;
        let after = self.budget_summary_raw();
        if before == after {
            QuotaTransition::Unchanged
        } else {
            QuotaTransition::Changed
        }
    }

    pub fn refresh(&mut self, now: Timestamp) -> QuotaTransition {
        let before = self.budget_summary_raw();
        if let QuotaState::Waiting { resumes_at, .. } = self.state
            && now >= resumes_at
        {
            self.state = QuotaState::Unknown;
        }
        let after = self.budget_summary_raw();
        if before == after {
            QuotaTransition::Unchanged
        } else {
            QuotaTransition::Changed
        }
    }

    pub fn observe_event(&mut self, event: &AiEvent, now: Timestamp) -> QuotaTransition {
        match event {
            AiEvent::Quota(usage) => self.observe_usage(*usage, now),
            AiEvent::RateLimited { resets_at } => self.observe_rate_limited(*resets_at, now),
            AiEvent::IsolationChecked(_)
            | AiEvent::TextDelta(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => QuotaTransition::Unchanged,
        }
    }

    pub fn observe_usage(&mut self, usage: AiUsage, now: Timestamp) -> QuotaTransition {
        let before = self.budget_summary_raw();
        let _refreshed = self.refresh(now);
        match usage {
            AiUsage::Unknown => {
                if !matches_active_wait(self.state, now) {
                    self.state = QuotaState::Unknown;
                }
            }
            AiUsage::Available(utilization) => {
                self.state = QuotaState::Available {
                    utilization,
                    observed_at: now,
                };
            }
            AiUsage::Waiting(resets_at) => {
                self.apply_waiting(resets_at, now);
            }
        }
        let after = self.budget_summary_raw();
        if before == after {
            QuotaTransition::Unchanged
        } else {
            QuotaTransition::Changed
        }
    }

    pub fn observe_rate_limited(
        &mut self,
        resets_at: ResetTime,
        now: Timestamp,
    ) -> QuotaTransition {
        let before = self.budget_summary_raw();
        let _refreshed = self.refresh(now);
        self.apply_waiting(resets_at, now);
        let after = self.budget_summary_raw();
        if before == after {
            QuotaTransition::Unchanged
        } else {
            QuotaTransition::Changed
        }
    }

    fn apply_waiting(&mut self, resets_at: ResetTime, now: Timestamp) {
        match resets_at {
            ResetTime::Known(target) if target > now => {
                self.state = QuotaState::Waiting {
                    resumes_at: target,
                    source: ResetSource::ProviderReset,
                    observed_at: now,
                };
            }
            ResetTime::Known(_) | ResetTime::NotProvided => {
                if let QuotaState::Waiting {
                    resumes_at: existing,
                    source: ResetSource::ProviderReset,
                    ..
                } = self.state
                    && existing > now
                {
                    return;
                }
                let fallback = match now.checked_add(self.policy.unknown_reset_cooldown) {
                    Ok(target) => target,
                    Err(_) => now,
                };
                self.state = QuotaState::Waiting {
                    resumes_at: fallback,
                    source: ResetSource::FallbackCooldown,
                    observed_at: now,
                };
            }
        }
    }

    #[must_use]
    pub fn state(&mut self, now: Timestamp) -> QuotaState {
        let _transition = self.refresh(now);
        self.state
    }

    #[must_use]
    pub fn current_usage(&mut self, now: Timestamp) -> AiUsage {
        match self.state(now) {
            QuotaState::Unknown => AiUsage::Unknown,
            QuotaState::Available { utilization, .. } => AiUsage::Available(utilization),
            QuotaState::Waiting {
                resumes_at,
                source: ResetSource::ProviderReset,
                ..
            } => AiUsage::Waiting(ResetTime::Known(resumes_at)),
            QuotaState::Waiting {
                source: ResetSource::FallbackCooldown,
                ..
            } => AiUsage::Waiting(ResetTime::NotProvided),
        }
    }

    #[must_use]
    pub fn resumes_at(&mut self, now: Timestamp) -> Option<Timestamp> {
        match self.state(now) {
            QuotaState::Waiting { resumes_at, .. } => Some(resumes_at),
            QuotaState::Unknown | QuotaState::Available { .. } => None,
        }
    }

    #[must_use]
    pub fn budget_summary(&mut self, now: Timestamp) -> BudgetSummaryKind {
        let _transition = self.refresh(now);
        self.budget_summary_raw()
    }

    #[must_use]
    pub fn evaluate_admission(&mut self, category: WorkCategory, now: Timestamp) -> QuotaDecision {
        match self.state(now) {
            QuotaState::Waiting {
                resumes_at, source, ..
            } => QuotaDecision::WaitRateLimit { resumes_at, source },
            QuotaState::Available { utilization, .. }
                if category == WorkCategory::Scheduled
                    && utilization.as_probability()
                        >= self.policy.scheduled_cap.as_probability() =>
            {
                QuotaDecision::WaitScheduledBudget {
                    utilization,
                    cap: self.policy.scheduled_cap,
                }
            }
            QuotaState::Unknown | QuotaState::Available { .. } => QuotaDecision::Allowed,
        }
    }

    fn budget_summary_raw(&self) -> BudgetSummaryKind {
        match self.state {
            QuotaState::Waiting { .. } => BudgetSummaryKind::RateLimited,
            QuotaState::Available { utilization, .. }
                if utilization.as_probability() >= self.policy.scheduled_cap.as_probability() =>
            {
                BudgetSummaryKind::ScheduledCapReached
            }
            QuotaState::Unknown | QuotaState::Available { .. } => BudgetSummaryKind::Normal,
        }
    }
}

fn matches_active_wait(state: QuotaState, now: Timestamp) -> bool {
    match state {
        QuotaState::Waiting { resumes_at, .. } => resumes_at > now,
        QuotaState::Unknown | QuotaState::Available { .. } => false,
    }
}
