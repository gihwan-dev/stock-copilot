use crate::ai_runtime::ChatTurnBook;
use crate::mcp_portfolio::CatalogPortfolio;
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AuditLog, Clock, CollectionQuery, CollectionTopic,
    IsolationCheck, ModelSelection, ReasoningEffort, ResearchTopic, RunInput, RunPurpose, RunSpec,
    SystemPortError, WatchlistRepository, WebResearchToolFailure, WebResearchToolOutcome,
    WebResearchToolPort, WebResearchToolRequest, WebSearchGuard,
};
use application::{
    ChatRepository, ChatRunLedger, ChatSignalState, ChatStopSignal, ChatWebResearch,
    ChatWebResearchFailure, ChatWebResearchRequest,
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
    turns: ChatTurnBook,
    sequence: AtomicU64,
    holdings: CatalogPortfolio,
}

impl<S, A, C> AppWebResearch<S, A, C> {
    #[must_use]
    pub fn new(store: Arc<S>, collector: Arc<A>, clock: Arc<C>, turns: ChatTurnBook) -> Self {
        Self {
            store,
            collector,
            clock,
            turns,
            sequence: AtomicU64::new(1_u64),
            holdings: CatalogPortfolio::default(),
        }
    }

    #[must_use]
    pub fn with_holdings(self, holdings: CatalogPortfolio) -> Self {
        Self { holdings, ..self }
    }
}

#[derive(Debug, Clone, Copy)]
struct KeepRunning;

impl ChatStopSignal for KeepRunning {
    fn state(&self) -> ChatSignalState {
        ChatSignalState::Active
    }
}

const fn user_failure(failure: ChatWebResearchFailure) -> WebResearchToolFailure {
    match failure {
        ChatWebResearchFailure::UserMessageMissing => WebResearchToolFailure::UserMessageMissing,
        ChatWebResearchFailure::Storage(_)
        | ChatWebResearchFailure::AuditFailed(_)
        | ChatWebResearchFailure::IsolationRejected(_)
        | ChatWebResearchFailure::IsolationNotVerified
        | ChatWebResearchFailure::ForbiddenToolAttempt
        | ChatWebResearchFailure::ProviderUnavailable
        | ChatWebResearchFailure::InvalidResponse(_)
        | ChatWebResearchFailure::InvalidCredential(_)
        | ChatWebResearchFailure::RateLimited(_)
        | ChatWebResearchFailure::Cancelled
        | ChatWebResearchFailure::Interrupted => WebResearchToolFailure::CollectionFailed,
    }
}

const fn unavailable(error: SystemPortError) -> WebResearchToolFailure {
    WebResearchToolFailure::Unavailable(error)
}

async fn completed_search(
    stream: &mut impl AiEventStream,
    candidate: &domain::WebSearchCandidateQuery,
) -> Result<UntrustedText, WebResearchToolFailure> {
    let failed = WebResearchToolFailure::CollectionFailed;
    if stream.next().await.map_err(|_error| failed)?
        != Some(AiEvent::IsolationChecked(IsolationCheck::Passed))
    {
        return Err(failed);
    }
    let mut completed_query = None;
    while let Some(event) = stream.next().await.map_err(|_error| failed)? {
        match event {
            AiEvent::WebSearch { query, action }
                if action.as_untrusted_str() == "completed"
                    && query.as_untrusted_str() == candidate.as_str() =>
            {
                completed_query = Some(query);
            }
            AiEvent::WebSearch { action, .. } if action.as_untrusted_str() == "failed" => {
                return Err(failed);
            }
            AiEvent::IsolationChecked(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::RateLimited { .. }
            | AiEvent::McpUnavailable
            | AiEvent::Quota(application::AiUsage::Waiting(_)) => return Err(failed),
            AiEvent::WebSearch { .. }
            | AiEvent::TextDelta(_)
            | AiEvent::Unknown(_)
            | AiEvent::TokenUsage(_)
            | AiEvent::ActualModel(_)
            | AiEvent::ModelRerouted(_)
            | AiEvent::Quota(application::AiUsage::Available(_) | application::AiUsage::Unknown) => {
            }
        }
    }
    completed_query.ok_or(failed)
}

fn outcome(
    observations: &[application::SearchObservation],
    query: &UntrustedText,
    max_age_days: i64,
    collected_at: domain::Timestamp,
) -> Result<WebResearchToolOutcome, WebResearchToolFailure> {
    let failed = WebResearchToolFailure::CollectionFailed;
    let mut lines = Vec::new();
    for item in observations {
        let age = item
            .date
            .days_until(collected_at.offset_calendar_date())
            .map_err(|_error| failed)?;
        if &item.query != query || !(0..=max_age_days).contains(age.as_i64()) {
            continue;
        }
        let Some((_, rest)) = item.url.as_str().split_once("://") else {
            continue;
        };
        let host = rest.split(['/', '?', '#']).next().ok_or(failed)?;
        let date_label = match item.date_kind {
            application::SourceDateKind::Published => "보도일",
            application::SourceDateKind::Observed => "관측일",
            application::SourceDateKind::Updated => "페이지 갱신일",
        };
        lines.push(format!(
            "{host} | {date_label} {:04}-{:02}-{:02} | {}",
            item.date.year().as_i32(),
            item.date.month().as_u8(),
            item.date.day().as_u8(),
            item.title.as_untrusted_str()
        ));
    }
    if lines.is_empty() {
        return Err(failed);
    }
    let summary = UntrustedText::try_new(&lines.join("\n")).map_err(|_error| failed)?;
    Ok(WebResearchToolOutcome {
        summary,
        collected_at,
    })
}

impl<S, A, C> AppWebResearch<S, A, C>
where
    S: WatchlistRepository + AuditLog + ChatRepository + ChatRunLedger,
    A: AiProvider,
    C: Clock,
{
    async fn user_message(
        &self,
        run: &AiRunId,
    ) -> Result<WebResearchToolOutcome, WebResearchToolFailure> {
        let turn = self
            .turns
            .turn_of(run)
            .ok_or(WebResearchToolFailure::UserMessageMissing)?;
        let request = ChatWebResearchRequest::new(
            turn.session,
            turn.turn,
            self.collection_run(run)?,
            ModelSelection::Fast,
            ReasoningEffort::Low,
        );
        let findings = ChatWebResearch::new(
            self.collector.as_ref(),
            self.store.as_ref(),
            self.store.as_ref(),
            self.store.as_ref(),
            self.clock.as_ref(),
        )
        .execute(&request, &KeepRunning)
        .await
        .map_err(user_failure)?;
        if findings.summary().as_untrusted_str().trim().is_empty() {
            return Err(WebResearchToolFailure::CollectionFailed);
        }
        Ok(WebResearchToolOutcome {
            summary: findings.summary().clone(),
            collected_at: self.clock.now(),
        })
    }

    async fn market_outlook(
        &self,
        run: &AiRunId,
        market: application::OutlookMarket,
        horizon: application::OutlookHorizon,
    ) -> Result<WebResearchToolOutcome, WebResearchToolFailure> {
        let market_name = match market {
            application::OutlookMarket::Kr => "Korean stock market KOSPI",
            application::OutlookMarket::Us => "US stock market SPY",
        };
        let (period, max_age_days) = match horizon {
            application::OutlookHorizon::Day => ("day", 1),
            application::OutlookHorizon::Week => ("week", 7),
            application::OutlookHorizon::TwoWeeks => ("fortnight", 14),
            application::OutlookHorizon::Month => ("month", 31),
            application::OutlookHorizon::Quarter => ("quarter", 92),
        };
        let candidate = domain::WebSearchCandidateQuery::new(&format!(
            "{market_name} index performance past {period} and outlook next {period} dated economic releases"
        ));
        let guard = WebSearchGuard::new(self.clock.as_ref(), self.store.as_ref());
        let inspection = guard
            .inspect_code_query(&candidate, &WebSearchSensitiveContext::new())
            .await
            .map_err(|application::WebSearchGuardError::AuditFailed(error)| unavailable(error))?;
        let WebSearchInspection::Approved(query) = inspection else {
            return Err(WebResearchToolFailure::QueryRejected);
        };
        let spec = RunSpec::new(
            self.collection_run(run)?,
            RunInput::Collection(CollectionQuery::MarketOutlook(query)),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        );
        let failed = WebResearchToolFailure::CollectionFailed;
        let mut stream = self.collector.run(spec).await.map_err(|_error| failed)?;
        let completed = completed_search(&mut stream, &candidate).await?;
        outcome(
            stream.search_observations(),
            &completed,
            max_age_days,
            self.clock.now(),
        )
    }

    async fn targets(&self) -> Result<Vec<Symbol>, SystemPortError> {
        let watched = WatchlistRepository::load(self.store.as_ref()).await?;
        let mut symbols: BTreeSet<Symbol> = self.holdings.held().await.into_iter().collect();
        symbols.extend(
            watched
                .instruments()
                .iter()
                .filter_map(|instrument| Symbol::try_new(instrument.as_str()).ok()),
        );
        Ok(symbols.into_iter().collect())
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
            let name = match self.holdings.name(symbol) {
                Some(name) => name,
                None => DisplayInstrumentName::try_new(symbol.as_str())
                    .map_err(|_error| WebResearchToolFailure::QueryRejected)?,
            };
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
    S: WatchlistRepository + AuditLog + ChatRepository + ChatRunLedger,
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
        let (symbols, topics) = match request {
            WebResearchToolRequest::CodeTopics { symbols, topics } => (symbols, topics),
            WebResearchToolRequest::MarketOutlook { market, horizon } => {
                return self.market_outlook(run, *market, *horizon).await;
            }
            WebResearchToolRequest::StoredUserMessage => return self.user_message(run).await,
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
