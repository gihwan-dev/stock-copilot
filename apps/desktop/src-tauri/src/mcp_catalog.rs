use application::{
    AiRunId, ArticleFetchFailure, ArticleToolPort, BodyReadingSetting, Clock,
    EstimateAssignmentRepository, EstimateRunContextSource, EstimateSubmissionRepository, EventBus,
    MarketToolReads, NewsToolReads, PortfolioToolReads, ProfileSource, RetryDelay,
    ScorecardObservationRepository, SettingsRepository, SystemFailure, SystemPort, SystemPortError,
    ThesisRepository, ThesisToolPort, ToolCallHistory, WebResearchToolFailure, WebResearchToolPort,
};
use domain::{InstrumentId, KnownMarket, Symbol};
use mcp_server::{
    AnalysisCatalogPorts, EstimateToolPorts, ToolRegistry, ToolRegistryError,
    build_analysis_catalog, make_cursor_store,
};
use std::collections::BTreeSet;
use std::future::{Future, ready};
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub const ANALYSIS_MCP_SERVER: &str = "stock";

#[derive(Debug, Default, Clone, Copy)]
pub struct UnconnectedToolData;

fn unconnected() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

fn fail<T>() -> impl Future<Output = Result<T, SystemPortError>> + Send
where
    T: Send,
{
    ready(Err(unconnected()))
}

impl PortfolioToolReads for UnconnectedToolData {
    fn portfolio(
        &self,
    ) -> impl Future<Output = Result<Option<application::PortfolioToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn allocation(
        &self,
    ) -> impl Future<Output = Result<Option<application::AllocationToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn transactions(
        &self,
    ) -> impl Future<Output = Result<Option<application::TransactionsToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn performance(
        &self,
        _period: application::PerformancePeriod,
    ) -> impl Future<Output = Result<Option<application::PerformanceToolView>, SystemPortError>> + Send
    {
        fail()
    }
}

impl MarketToolReads for UnconnectedToolData {
    fn quote(
        &self,
        _symbol: &Symbol,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::QuoteToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn candles(
        &self,
        _symbol: &Symbol,
        _market: KnownMarket,
        _window: application::CandleWindow,
    ) -> impl Future<Output = Result<Option<application::CandlesToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn market_overview(
        &self,
    ) -> impl Future<Output = Result<Option<application::MarketOverviewToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn rankings(
        &self,
        _kind: application::RankingToolKind,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::RankingsToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn sectors(
        &self,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::SectorsToolView>, SystemPortError>> + Send
    {
        fail()
    }

    fn investor_flow(
        &self,
        _symbol: &Symbol,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::InvestorFlowToolView>, SystemPortError>> + Send
    {
        fail()
    }
}

impl NewsToolReads for UnconnectedToolData {
    fn search_news(
        &self,
        _query: &application::CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<application::NewsToolLine>, SystemPortError>> + Send {
        fail()
    }

    fn disclosures(
        &self,
        _query: &application::CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<application::DisclosureToolLine>, SystemPortError>> + Send
    {
        fail()
    }
}

impl WebResearchToolPort for UnconnectedToolData {
    fn known_targets(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<Vec<Symbol>, SystemPortError>> + Send {
        fail()
    }

    fn request_research(
        &self,
        _run: &AiRunId,
        _request: &application::WebResearchToolRequest,
    ) -> impl Future<Output = Result<application::WebResearchToolOutcome, WebResearchToolFailure>> + Send
    {
        ready(Err(WebResearchToolFailure::Unavailable(unconnected())))
    }
}

impl ArticleToolPort for UnconnectedToolData {
    fn locate(
        &self,
        _handle: &application::CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<application::ArticleLocation>, SystemPortError>> + Send
    {
        fail()
    }

    fn body_reading(
        &self,
        _source: &domain::NewsSource,
    ) -> impl Future<Output = Result<BodyReadingSetting, SystemPortError>> + Send {
        ready(Ok(BodyReadingSetting::Disabled))
    }

    fn fetch_body(
        &self,
        _url: &domain::NormalizedUrl,
    ) -> impl Future<Output = Result<application::ArticleBody, ArticleFetchFailure>> + Send {
        ready(Err(ArticleFetchFailure::Unavailable(unconnected())))
    }
}

impl ThesisToolPort for UnconnectedToolData {
    fn resolve_target(
        &self,
        _run: &AiRunId,
        _symbol: &Symbol,
    ) -> impl Future<Output = Result<Option<InstrumentId>, SystemPortError>> + Send {
        fail()
    }

    fn collected_evidence(
        &self,
        _run: &AiRunId,
        _instrument: &InstrumentId,
        _handle: &application::CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<domain::EvidenceRef>, SystemPortError>> + Send {
        fail()
    }

    fn run_metadata(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<application::ThesisRunMetadata, SystemPortError>> + Send {
        fail()
    }
}

impl EstimateRunContextSource for UnconnectedToolData {
    fn estimate_context(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<application::EstimateRunContext, SystemPortError>> + Send {
        fail()
    }
}

#[derive(Debug)]
pub struct StoredProfile<S>(pub Arc<S>);

impl<S: SettingsRepository> ProfileSource for StoredProfile<S> {
    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<domain::InvestorProfile>, SystemPortError>> + Send {
        self.0.profile()
    }
}

pub struct CatalogStores<R, E> {
    pub theses: Arc<R>,
    pub estimates: Arc<E>,
    pub events: Arc<EventBus>,
}

impl<R, E> std::fmt::Debug for CatalogStores<R, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CatalogStores")
    }
}

pub fn analysis_catalog<C, R, E>(
    clock: &Arc<C>,
    stores: CatalogStores<R, E>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    C: Clock + 'static,
    R: ThesisRepository + 'static,
    E: EstimateAssignmentRepository
        + EstimateSubmissionRepository
        + ScorecardObservationRepository
        + ToolCallHistory
        + SettingsRepository
        + 'static,
{
    let data = Arc::new(UnconnectedToolData);
    let cursors = make_cursor_store(Arc::clone(clock));
    let ports = AnalysisCatalogPorts {
        portfolio: Arc::clone(&data),
        market: Arc::clone(&data),
        news: Arc::clone(&data),
        research: Arc::clone(&data),
        article: Arc::clone(&data),
        thesis: Arc::clone(&data),
        thesis_repository: stores.theses,
        tool_history: Arc::clone(&stores.estimates),
        estimates: Arc::new(EstimateToolPorts {
            assignments: Arc::clone(&stores.estimates),
            submissions: Arc::clone(&stores.estimates),
            history: Arc::clone(&stores.estimates),
            context: Arc::clone(&data),
            settings: Arc::new(StoredProfile(Arc::clone(&stores.estimates))),
            observations: stores.estimates,
            clock: Arc::clone(clock),
            events: stores.events,
        }),
    };
    build_analysis_catalog(&ports, clock, &cursors)
}

#[must_use]
pub fn allowed_tool_names(server: &str, registry: &ToolRegistry) -> BTreeSet<String> {
    registry
        .tool_names()
        .iter()
        .map(|name| format!("mcp__{server}__{}", name.as_str()))
        .collect()
}
