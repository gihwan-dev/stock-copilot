use crate::{AnalysisFailure, Clock, PortError, SystemPortError, WriteCompleted};
use domain::{
    CalendarDate, Policy, PolicySecurityFacts, PolicyViolation, PortfolioValuation,
    SectorAssignments, TimeError, UntrustedText, WeeklyReviewSlot, weekly_review_slot,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyReviewInputs {
    pub valuation: PortfolioValuation,
    pub assignments: SectorAssignments,
    pub facts: PolicySecurityFacts,
    pub policy: Policy,
}

pub trait WeeklyReviewSource: Send + Sync {
    fn inputs(&self) -> impl Future<Output = Result<WeeklyReviewInputs, PortError>> + Send;
}

pub trait ScorecardRefresh: Send + Sync {
    fn refresh(&self) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

pub trait WeeklyReviewNarrator: Send + Sync {
    fn narrate(
        &self,
        findings: &WeeklyFindings,
    ) -> impl Future<Output = Result<UntrustedText, AnalysisFailure>> + Send;
}

pub trait WeeklyReviewCards: Send + Sync {
    fn last_week(
        &self,
    ) -> impl Future<Output = Result<Option<CalendarDate>, SystemPortError>> + Send;
    fn save(
        &self,
        card: &WeeklyReviewCard,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeeklyFindings {
    Checked(Vec<PolicyViolation>),
    Unavailable(PolicyViolation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorecardUpdate {
    Refreshed,
    Failed(SystemPortError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeeklyNarrative {
    Written(UntrustedText),
    Missing(AnalysisFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyReviewCard {
    pub week: CalendarDate,
    pub findings: WeeklyFindings,
    pub scorecard: ScorecardUpdate,
    pub narrative: WeeklyNarrative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeeklyReviewRun {
    NotDue,
    AlreadyDone(CalendarDate),
    Saved(WeeklyReviewCard),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeeklyReviewError {
    Time(TimeError),
    Cards(SystemPortError),
    Inputs(PortError),
}

#[derive(Debug)]
pub struct RunWeeklyReview<'a, Source, Scorecards, Narrator, Cards, Time> {
    pub source: &'a Source,
    pub scorecards: &'a Scorecards,
    pub narrator: &'a Narrator,
    pub cards: &'a Cards,
    pub clock: &'a Time,
}

impl<Source, Scorecards, Narrator, Cards, Time>
    RunWeeklyReview<'_, Source, Scorecards, Narrator, Cards, Time>
where
    Source: WeeklyReviewSource,
    Scorecards: ScorecardRefresh,
    Narrator: WeeklyReviewNarrator,
    Cards: WeeklyReviewCards,
    Time: Clock,
{
    pub async fn execute(&self) -> Result<WeeklyReviewRun, WeeklyReviewError> {
        let WeeklyReviewSlot::Due(week) =
            weekly_review_slot(self.clock.now()).map_err(WeeklyReviewError::Time)?
        else {
            return Ok(WeeklyReviewRun::NotDue);
        };
        if self
            .cards
            .last_week()
            .await
            .map_err(WeeklyReviewError::Cards)?
            == Some(week)
        {
            return Ok(WeeklyReviewRun::AlreadyDone(week));
        }
        let inputs = self
            .source
            .inputs()
            .await
            .map_err(WeeklyReviewError::Inputs)?;
        let findings =
            match inputs
                .policy
                .check_current(&inputs.valuation, &inputs.assignments, &inputs.facts)
            {
                Ok(violations) => WeeklyFindings::Checked(violations),
                Err(blocked) => WeeklyFindings::Unavailable(blocked),
            };
        let scorecard = match self.scorecards.refresh().await {
            Ok(_written) => ScorecardUpdate::Refreshed,
            Err(error) => ScorecardUpdate::Failed(error),
        };
        let narrative = match self.narrator.narrate(&findings).await {
            Ok(text) => WeeklyNarrative::Written(text),
            Err(failure) => WeeklyNarrative::Missing(failure),
        };
        let card = WeeklyReviewCard {
            week,
            findings,
            scorecard,
            narrative,
        };
        self.cards
            .save(&card)
            .await
            .map_err(WeeklyReviewError::Cards)?;
        Ok(WeeklyReviewRun::Saved(card))
    }
}
