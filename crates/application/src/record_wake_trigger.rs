use crate::{DeliveryReport, EventBus, SystemPortError};
use domain::{
    Disclosure, DisclosureAction, DisclosureRevision, HoldingStatus, InstrumentId, Price,
    Timestamp, TriggerJudgement, WakeTrigger, daily_move_trigger, disclosure_action,
    grade_disclosure, news_burst_trigger,
};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerWrite {
    Recorded,
    AlreadyRecorded,
}

pub trait WakeTriggerRepository: Send + Sync {
    fn record(
        &self,
        trigger: &WakeTrigger,
    ) -> impl Future<Output = Result<TriggerWrite, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeTriggerOutcome {
    Recorded(WakeTrigger, DeliveryReport),
    AlreadyRecorded,
    DisclosureNotWaking(DisclosureAction),
    NotTriggered(TriggerJudgement),
}

#[derive(Debug)]
pub struct RecordWakeTriggers<'a, Repository> {
    pub repository: &'a Repository,
    pub events: &'a EventBus,
}

impl<Repository: WakeTriggerRepository> RecordWakeTriggers<'_, Repository> {
    async fn record(&self, trigger: WakeTrigger) -> Result<WakeTriggerOutcome, SystemPortError> {
        match self.repository.record(&trigger).await? {
            TriggerWrite::Recorded => {
                let report = self.events.publish(trigger.event());
                Ok(WakeTriggerOutcome::Recorded(trigger, report))
            }
            TriggerWrite::AlreadyRecorded => Ok(WakeTriggerOutcome::AlreadyRecorded),
        }
    }

    pub async fn disclosure(
        &self,
        disclosure: &Disclosure,
        instrument: &InstrumentId,
        revision: DisclosureRevision,
        holding: HoldingStatus,
    ) -> Result<WakeTriggerOutcome, SystemPortError> {
        let grade = grade_disclosure(disclosure.collected().title(), revision);
        let action = disclosure_action(grade, holding);
        match action {
            DisclosureAction::WakeAndNotify | DisclosureAction::Wake => {
                self.record(WakeTrigger::Disclosure {
                    key: disclosure.key().clone(),
                    instrument: instrument.clone(),
                    grade,
                    action,
                })
                .await
            }
            DisclosureAction::Briefing | DisclosureAction::CollectOnly => {
                Ok(WakeTriggerOutcome::DisclosureNotWaking(action))
            }
        }
    }

    pub async fn price_move(
        &self,
        instrument: &InstrumentId,
        holding: HoldingStatus,
        previous_close: &Price,
        current: &Price,
    ) -> Result<WakeTriggerOutcome, SystemPortError> {
        if holding == HoldingStatus::NotHeld {
            return Ok(WakeTriggerOutcome::NotTriggered(TriggerJudgement::Quiet));
        }
        match daily_move_trigger(previous_close, current) {
            TriggerJudgement::Triggered => {
                self.record(WakeTrigger::PriceMove(instrument.clone()))
                    .await
            }
            judgement @ (TriggerJudgement::Quiet | TriggerJudgement::Undetermined) => {
                Ok(WakeTriggerOutcome::NotTriggered(judgement))
            }
        }
    }

    pub async fn news_burst(
        &self,
        instrument: &InstrumentId,
        published: &[Timestamp],
        now: Timestamp,
    ) -> Result<WakeTriggerOutcome, SystemPortError> {
        match news_burst_trigger(published, now) {
            TriggerJudgement::Triggered => {
                self.record(WakeTrigger::NewsBurst(instrument.clone()))
                    .await
            }
            judgement @ (TriggerJudgement::Quiet | TriggerJudgement::Undetermined) => {
                Ok(WakeTriggerOutcome::NotTriggered(judgement))
            }
        }
    }
}
