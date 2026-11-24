use std::future::Future;

use domain::{
    CalibrationKey, InvestorProfile, Scorecard, ScorecardError, ScorecardObservation, Scorecards,
};

use crate::{Clock, SettingsRepository, SystemPortError};

pub trait ScorecardObservationRepository: Send + Sync {
    fn scorecard_observations(
        &self,
    ) -> impl Future<Output = Result<Vec<ScorecardObservation>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileView {
    Saved(InvestorProfile),
    Default(InvestorProfile),
}

#[derive(Debug)]
pub struct GetProfile<'a, Repository> {
    repository: &'a Repository,
}

impl<'a, Repository: SettingsRepository> GetProfile<'a, Repository> {
    #[must_use]
    pub const fn new(repository: &'a Repository) -> Self {
        Self { repository }
    }

    pub async fn execute(&self) -> Result<ProfileView, SystemPortError> {
        Ok(match self.repository.profile().await? {
            Some(profile) => ProfileView::Saved(profile),
            None => ProfileView::Default(InvestorProfile::default()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardSelection {
    All,
    Combination(CalibrationKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardView {
    NoObservations,
    Cards(Scorecards),
    Single(Scorecard),
    UnknownCombination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetScorecardError {
    Storage(SystemPortError),
    Calculation(ScorecardError),
}

#[derive(Debug)]
pub struct GetScorecard<'a, Repository, TimeSource> {
    repository: &'a Repository,
    clock: &'a TimeSource,
}

impl<'a, Repository: ScorecardObservationRepository, TimeSource: Clock>
    GetScorecard<'a, Repository, TimeSource>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, clock: &'a TimeSource) -> Self {
        Self { repository, clock }
    }

    pub async fn execute(
        &self,
        selection: ScorecardSelection,
    ) -> Result<ScorecardView, GetScorecardError> {
        let observations = self
            .repository
            .scorecard_observations()
            .await
            .map_err(GetScorecardError::Storage)?;
        if observations.is_empty() {
            return Ok(ScorecardView::NoObservations);
        }
        let cards = Scorecards::calculate(&observations, self.clock.now())
            .map_err(GetScorecardError::Calculation)?;
        Ok(match selection {
            ScorecardSelection::All => ScorecardView::Cards(cards),
            ScorecardSelection::Combination(key) => cards
                .select(&key)
                .cloned()
                .map_or(ScorecardView::UnknownCombination, ScorecardView::Single),
        })
    }
}
