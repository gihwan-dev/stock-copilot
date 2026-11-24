use crate::script::State;
use application::{PrinciplesRepository, SystemPortError, WriteCompleted};
use domain::InvestmentPrinciples;
use std::future::{Future, ready};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct MemoryPrinciplesRepository {
    state: Arc<State<PrinciplesState>>,
}

#[derive(Debug, Default)]
struct PrinciplesState {
    stored: Option<InvestmentPrinciples>,
    failure: Option<SystemPortError>,
}

impl MemoryPrinciplesRepository {
    #[must_use]
    pub fn stored(&self) -> Option<InvestmentPrinciples> {
        self.state.write().stored.clone()
    }

    pub fn fail_with(&self, error: SystemPortError) {
        self.state.write().failure = Some(error);
    }
}

impl PrinciplesRepository for MemoryPrinciplesRepository {
    fn store_principles(
        &self,
        principles: &InvestmentPrinciples,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut state = self.state.write();
        if let Some(error) = state.failure {
            return ready(Err(error));
        }
        state.stored = Some(principles.clone());
        ready(Ok(WriteCompleted))
    }

    fn principles(
        &self,
    ) -> impl Future<Output = Result<Option<InvestmentPrinciples>, SystemPortError>> + Send {
        let state = self.state.write();
        ready(match state.failure {
            Some(error) => Err(error),
            None => Ok(state.stored.clone()),
        })
    }
}
