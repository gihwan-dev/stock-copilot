use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AuditLog, Clock, CollectionQuery, CollectionTopic,
    IsolationCheck, ModelSelection, ReasoningEffort, ResearchTopic, RunInput, RunPurpose, RunSpec,
    SystemPortError, WatchlistRepository, WebResearchToolFailure, WebResearchToolOutcome,
    WebResearchToolPort, WebResearchToolRequest, WebSearchGuard,
};
use domain::{
    DisplayInstrumentName, Symbol, UntrustedText, WebSearchInspection, WebSearchSensitiveContext,
    research_search_plan,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

const fn plan_topic(topic: ResearchTopic) -> domain::ResearchTopic {
    match topic {
        ResearchTopic::Earnings => domain::ResearchTopic::Earnings,
        ResearchTopic::Guidance => domain::ResearchTopic::Guidance,
        ResearchTopic::Regulation | ResearchTopic::Litigation => domain::ResearchTopic::Regulation,
        ResearchTopic::ManagementChange => domain::ResearchTopic::Management,
        ResearchTopic::ProductLaunch => domain::ResearchTopic::Products,
        ResearchTopic::IndustryTrend | ResearchTopic::SupplyChain => {
            domain::ResearchTopic::Competition
        }
    }
}

const fn collection_topic(topic: ResearchTopic) -> CollectionTopic {
    match topic {
        ResearchTopic::Earnings | ResearchTopic::Guidance => CollectionTopic::Earnings,
        ResearchTopic::Regulation | ResearchTopic::Litigation => CollectionTopic::Disclosures,
        ResearchTopic::ManagementChange
        | ResearchTopic::ProductLaunch
        | ResearchTopic::IndustryTrend
        | ResearchTopic::SupplyChain => CollectionTopic::CompanyNews,
    }
}

fn collection_topics(topics: &[ResearchTopic]) -> Vec<CollectionTopic> {
    let mut seen = Vec::new();
    for topic in topics.iter().copied().map(collection_topic) {
        if !seen.contains(&topic) {
            seen.push(topic);
        }
    }
    seen
}

#[derive(Debug)]
pub struct AppWebResearch<S, A, C> {
    store: Arc<S>,
    collector: Arc<A>,
    clock: Arc<C>,
    sequence: AtomicU64,
}

impl<S, A, C> AppWebResearch<S, A, C> {
    #[must_use]
    pub const fn new(store: Arc<S>, collector: Arc<A>, clock: Arc<C>) -> Self {
        Self {
            store,
            collector,
            clock,
            sequence: AtomicU64::new(1_u64),
        }
    }
}

const fn unavailable(error: SystemPortError) -> WebResearchToolFailure {
    WebResearchToolFailure::Unavailable(error)
}

impl<S, A, C> AppWebResearch<S, A, C>
where
    S: WatchlistRepository + AuditLog,
    A: AiProvider,
    C: Clock,
{
    async fn targets(&self) -> Result<Vec<Symbol>, SystemPortError> {
        Ok(WatchlistRepository::load(self.store.as_ref())
            .await?
            .instruments()
            .iter()
            .filter_map(|instrument| Symbol::try_new(instrument.as_str()).ok())
            .collect())
    }

    async fn approve(
        &self,
        symbols: &[Symbol],
        topics: &[ResearchTopic],
    ) -> Result<(), WebResearchToolFailure> {
        let guard = WebSearchGuard::new(self.clock.as_ref(), self.store.as_ref());
        let plan: Vec<domain::ResearchTopic> = topics.iter().copied().map(plan_topic).collect();
        let context = WebSearchSensitiveContext::new();
        let mut candidates = Vec::new();
        for symbol in symbols {
            let name = DisplayInstrumentName::try_new(symbol.as_str())
                .map_err(|_error| WebResearchToolFailure::QueryRejected)?;
            candidates.extend(research_search_plan(&name, &plan));
        }
        for candidate in &candidates {
            let inspection = guard
                .inspect_code_query(candidate, &context)
                .await
                .map_err(|error| match error {
                    application::WebSearchGuardError::AuditFailed(port) => unavailable(port),
                })?;
            if let WebSearchInspection::Rejected(_rejected) = inspection {
                return Err(WebResearchToolFailure::QueryRejected);
            }
        }
        Ok(())
    }

    fn collection_run(&self, run: &AiRunId) -> Result<AiRunId, WebResearchToolFailure> {
        let sequence = self.sequence.fetch_add(1_u64, Ordering::Relaxed);
        let mut text = String::from("research-");
        text.push_str(&sequence.to_string());
        text.push('-');
        text.push_str(run.as_str());
        let bounded: String = text.chars().take(128).collect();
        AiRunId::try_new(&bounded).map_err(|_error| WebResearchToolFailure::CollectionFailed)
    }

    async fn collect(
        &self,
        run: &AiRunId,
        symbols: Vec<Symbol>,
        topics: &[ResearchTopic],
    ) -> Result<UntrustedText, WebResearchToolFailure> {
        let spec = RunSpec::new(
            self.collection_run(run)?,
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: symbols,
                topics: collection_topics(topics),
            }),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        );
        let mut stream = self
            .collector
            .run(spec)
            .await
            .map_err(|_error| WebResearchToolFailure::CollectionFailed)?;
        let mut summary = String::new();
        loop {
            match stream.next().await {
                Ok(None) => break,
                Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Rejected(_)))) | Err(_) => {
                    return Err(WebResearchToolFailure::CollectionFailed);
                }
                Ok(Some(AiEvent::TextDelta(delta))) => summary.push_str(delta.as_untrusted_str()),
                Ok(Some(_)) => {}
            }
        }
        if summary.trim().is_empty() {
            return Err(WebResearchToolFailure::CollectionFailed);
        }
        UntrustedText::try_new(&summary).map_err(|_error| WebResearchToolFailure::CollectionFailed)
    }
}

impl<S, A, C> WebResearchToolPort for AppWebResearch<S, A, C>
where
    S: WatchlistRepository + AuditLog,
    A: AiProvider,
    C: Clock,
{
    async fn known_targets(&self, _run: &AiRunId) -> Result<Vec<Symbol>, SystemPortError> {
        self.targets().await
    }

    async fn request_research(
        &self,
        run: &AiRunId,
        request: &WebResearchToolRequest,
    ) -> Result<WebResearchToolOutcome, WebResearchToolFailure> {
        let WebResearchToolRequest::CodeTopics { symbols, topics } = request else {
            return Err(WebResearchToolFailure::ModeNotAllowed);
        };
        let known: BTreeSet<Symbol> = self
            .targets()
            .await
            .map_err(unavailable)?
            .into_iter()
            .collect();
        if symbols.is_empty()
            || topics.is_empty()
            || !symbols.iter().all(|symbol| known.contains(symbol))
        {
            return Err(WebResearchToolFailure::QueryRejected);
        }
        self.approve(symbols, topics).await?;
        let summary = self.collect(run, symbols.clone(), topics).await?;
        Ok(WebResearchToolOutcome {
            summary,
            collected_at: self.clock.now(),
        })
    }
}
