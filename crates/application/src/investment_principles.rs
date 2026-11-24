use std::future::Future;

use domain::{Concentration, InvestmentPrinciples, Policy};

use crate::{SystemPortError, WriteCompleted};

pub trait PrinciplesRepository: Send + Sync {
    fn store_principles(
        &self,
        principles: &InvestmentPrinciples,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn principles(
        &self,
    ) -> impl Future<Output = Result<Option<InvestmentPrinciples>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct UpdateInvestmentPrinciples<'a, Repo> {
    repository: &'a Repo,
}

impl<'a, Repo: PrinciplesRepository> UpdateInvestmentPrinciples<'a, Repo> {
    #[must_use]
    pub const fn new(repository: &'a Repo) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        principles: &InvestmentPrinciples,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.repository.store_principles(principles).await
    }
}

#[derive(Debug)]
pub struct ApplyInvestmentPrinciples<'a, Repo> {
    repository: &'a Repo,
}

impl<'a, Repo: PrinciplesRepository> ApplyInvestmentPrinciples<'a, Repo> {
    #[must_use]
    pub const fn new(repository: &'a Repo) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        policy: Policy,
        holdings: &Concentration,
    ) -> Result<Policy, SystemPortError> {
        Ok(match self.repository.principles().await? {
            Some(stored) => stored.apply(policy, holdings),
            None => InvestmentPrinciples::default().apply(policy, holdings),
        })
    }
}
