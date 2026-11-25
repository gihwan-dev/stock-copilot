use crate::ai_runtime::RunMetadataBook;
use crate::mcp_research::AppWebResearch;
use crate::mcp_sources::{ArticleBodies, StoredArticles, StoredNews, StoredThesis};
use application::{
    Clock, EstimateAssignmentRepository, EstimateSubmissionRepository, EventBus, MarketToolReads,
    PortfolioToolReads, ProfileSource, RetryDelay, ScorecardObservationRepository,
    SettingsRepository, SystemFailure, SystemPort, SystemPortError, ThesisRepository,
    ToolCallHistory,
};
use application::{CollectedItemReads, NewsDataSettingsRepository, WatchlistRepository};
use domain::{KnownMarket, Symbol};
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

#[derive(Debug)]
pub struct StoredProfile<S>(pub Arc<S>);

impl<S: SettingsRepository> ProfileSource for StoredProfile<S> {
    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<domain::InvestorProfile>, SystemPortError>> + Send {
        self.0.profile()
    }
}

pub struct CatalogStores<R, E, B, W> {
    pub theses: Arc<R>,
    pub estimates: Arc<E>,
    pub bodies: Arc<B>,
    pub collector: Arc<W>,
    pub turns: crate::ai_runtime::ChatTurnBook,
    pub runs: RunMetadataBook,
    pub events: Arc<EventBus>,
    pub market: crate::broker_link::BrokerMarketTools,
    pub portfolio: crate::mcp_portfolio::CatalogPortfolio,
    pub estimate_runs: crate::estimate_flow::EstimateRunBook,
}

impl<R, E, B, W> std::fmt::Debug for CatalogStores<R, E, B, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CatalogStores")
    }
}

pub fn analysis_catalog<C, R, E, B, W>(
    clock: &Arc<C>,
    stores: CatalogStores<R, E, B, W>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    C: Clock + 'static,
    R: ThesisRepository + 'static,
    E: EstimateAssignmentRepository
        + EstimateSubmissionRepository
        + ScorecardObservationRepository
        + ToolCallHistory
        + SettingsRepository
        + CollectedItemReads
        + NewsDataSettingsRepository
        + WatchlistRepository
        + application::AuditLog
        + application::ChatRepository
        + application::ChatRunLedger
        + 'static,
    B: ArticleBodies + 'static,
    W: application::AiProvider + 'static,
{
    let cursors = make_cursor_store(Arc::clone(clock));
    let ports = AnalysisCatalogPorts {
        portfolio: Arc::new(stores.portfolio.clone()),
        market: Arc::new(stores.market),
        news: Arc::new(StoredNews::new(
            Arc::clone(&stores.estimates),
            Arc::clone(clock),
        )),
        research: Arc::new(
            AppWebResearch::new(
                Arc::clone(&stores.estimates),
                stores.collector,
                Arc::clone(clock),
                stores.turns,
            )
            .with_holdings(stores.portfolio),
        ),
        article: Arc::new(StoredArticles::new(
            Arc::clone(&stores.estimates),
            stores.bodies,
            Arc::clone(clock),
        )),
        thesis: Arc::new(StoredThesis::new(
            Arc::clone(&stores.estimates),
            stores.runs,
        )),
        thesis_repository: stores.theses,
        tool_history: Arc::clone(&stores.estimates),
        estimates: Arc::new(EstimateToolPorts {
            assignments: Arc::clone(&stores.estimates),
            submissions: Arc::clone(&stores.estimates),
            history: Arc::clone(&stores.estimates),
            context: Arc::new(stores.estimate_runs),
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
