use rust_decimal::Decimal;

use crate::{
    DisclosureKey, DomainEvent, InstrumentId, Nanosecond, Price, SpanSeconds, TimeSpan, Timestamp,
    UntrustedText,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DisclosureGrade {
    Low,
    Normal,
    High,
    Urgent,
}

impl DisclosureGrade {
    #[must_use]
    pub const fn downgraded(self) -> Self {
        match self {
            Self::Urgent => Self::High,
            Self::High => Self::Normal,
            Self::Normal | Self::Low => Self::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureRevision {
    Original,
    Correction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldingStatus {
    Held,
    NotHeld,
}

#[must_use]
pub fn grade_disclosure(title: &UntrustedText, revision: DisclosureRevision) -> DisclosureGrade {
    let text = title.as_untrusted_str();
    let urgent = "매매거래정지|거래정지|관리종목|상장폐지|불성실공시|투자주의|투자경고|투자위험|조회공시|감사의견|횡령|배임|회생|부도";
    let high = "유상증자|전환사채|신주인수권부사채|교환사채|감자|합병|분할|최대주주|자기주식|배당|잠정실적|(잠정)실적|영업실적|단일판매|공급계약|주요사항보고서";
    let low = "파생상품|주식선물|ETN|ELW|워런트|집합투자|투자신탁|자산유동화|일괄신고추가서류|일괄신고 추가서류";
    let contains = |words: &str| words.split('|').any(|word| text.contains(word));
    let grade = if contains(urgent) {
        DisclosureGrade::Urgent
    } else if contains(high) {
        DisclosureGrade::High
    } else if contains(low) {
        DisclosureGrade::Low
    } else {
        DisclosureGrade::Normal
    };
    match revision {
        DisclosureRevision::Original => grade,
        DisclosureRevision::Correction => grade.downgraded(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureAction {
    WakeAndNotify,
    Wake,
    Briefing,
    CollectOnly,
}

#[must_use]
pub const fn disclosure_action(grade: DisclosureGrade, holding: HoldingStatus) -> DisclosureAction {
    match (holding, grade) {
        (HoldingStatus::Held, DisclosureGrade::Urgent) => DisclosureAction::WakeAndNotify,
        (HoldingStatus::Held, DisclosureGrade::High) => DisclosureAction::Wake,
        (HoldingStatus::Held, DisclosureGrade::Normal) => DisclosureAction::Briefing,
        (HoldingStatus::Held, DisclosureGrade::Low)
        | (
            HoldingStatus::NotHeld,
            DisclosureGrade::Urgent
            | DisclosureGrade::High
            | DisclosureGrade::Normal
            | DisclosureGrade::Low,
        ) => DisclosureAction::CollectOnly,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MovePercent(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NewsBurstCount(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeThresholdError {
    OutOfRange,
}

impl MovePercent {
    pub const fn try_new(value: u8) -> Result<Self, WakeThresholdError> {
        if value == 0 || value > 30 {
            return Err(WakeThresholdError::OutOfRange);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

impl NewsBurstCount {
    pub const fn try_new(value: u8) -> Result<Self, WakeThresholdError> {
        if value < 2 || value > 50 {
            return Err(WakeThresholdError::OutOfRange);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeThresholds {
    pub daily_move: MovePercent,
    pub news_per_hour: NewsBurstCount,
}

impl Default for WakeThresholds {
    fn default() -> Self {
        Self {
            daily_move: MovePercent(5),
            news_per_hour: NewsBurstCount(5),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerJudgement {
    Triggered,
    Quiet,
    Undetermined,
}

#[must_use]
pub fn daily_move_trigger(previous_close: &Price, current: &Price) -> TriggerJudgement {
    daily_move_trigger_with(previous_close, current, WakeThresholds::default())
}

#[must_use]
pub fn daily_move_trigger_with(
    previous_close: &Price,
    current: &Price,
    thresholds: WakeThresholds,
) -> TriggerJudgement {
    let previous = previous_close.as_money();
    let latest = current.as_money();
    if previous.currency() != latest.currency() {
        return TriggerJudgement::Undetermined;
    }
    let base = previous.amount().as_decimal();
    let now = latest.amount().as_decimal();
    if base <= Decimal::ZERO {
        return TriggerJudgement::Undetermined;
    }
    let scaled_change = now
        .checked_sub(base)
        .map(|change| change.abs())
        .and_then(|change| change.checked_mul(Decimal::ONE_HUNDRED));
    let threshold = base.checked_mul(Decimal::from(thresholds.daily_move.0));
    match (scaled_change, threshold) {
        (Some(change), Some(limit)) if change >= limit => TriggerJudgement::Triggered,
        (Some(_), Some(_)) => TriggerJudgement::Quiet,
        (None, Some(_) | None) | (Some(_), None) => TriggerJudgement::Undetermined,
    }
}

#[must_use]
pub fn news_burst_trigger(published: &[Timestamp], now: Timestamp) -> TriggerJudgement {
    news_burst_trigger_with(published, now, WakeThresholds::default())
}

#[must_use]
pub fn news_burst_trigger_with(
    published: &[Timestamp],
    now: Timestamp,
    thresholds: WakeThresholds,
) -> TriggerJudgement {
    let needed = usize::from(thresholds.news_per_hour.0);
    let Ok(window) = TimeSpan::try_new(SpanSeconds::new(3_600), Nanosecond::new(0)) else {
        return TriggerJudgement::Undetermined;
    };
    let Ok(start) = now.checked_sub(window) else {
        return TriggerJudgement::Undetermined;
    };
    let recent = published
        .iter()
        .filter(|stamp| start < **stamp && **stamp <= now)
        .take(needed)
        .count();
    if recent >= needed {
        TriggerJudgement::Triggered
    } else {
        TriggerJudgement::Quiet
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeTrigger {
    Disclosure {
        key: DisclosureKey,
        instrument: InstrumentId,
        grade: DisclosureGrade,
        action: DisclosureAction,
    },
    PriceMove(InstrumentId),
    NewsBurst(InstrumentId),
}

impl WakeTrigger {
    #[must_use]
    pub fn event(&self) -> DomainEvent {
        match self {
            Self::Disclosure { key, .. } => DomainEvent::DisclosureTriggerRecorded(key.clone()),
            Self::PriceMove(instrument) => DomainEvent::PriceMoved(instrument.clone()),
            Self::NewsBurst(instrument) => DomainEvent::NewsActivityIncreased(instrument.clone()),
        }
    }
}
