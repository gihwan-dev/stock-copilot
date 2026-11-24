use std::sync::Arc;

use application::{
    ArticleToolPort, Clock, MarketToolReads, NewsToolReads, PortfolioToolReads, ThesisRepository,
    ThesisToolPort, WebResearchToolPort,
};

use crate::pagination::CursorStore;
use crate::tools::{ToolRegistry, ToolRegistryError};

pub mod article;
pub mod estimates;
pub mod knowledge;
pub mod market;
pub mod news;
pub mod portfolio;
mod render;
pub mod research;
pub mod thesis;

pub trait CatalogSection {
    fn register_into(&self, registry: &mut ToolRegistry) -> Result<(), ToolRegistryError>;
}

impl<A, S, H, X, G, O, C> CatalogSection for Arc<estimates::EstimateToolPorts<A, S, H, X, G, O, C>>
where
    A: application::EstimateAssignmentRepository + 'static,
    S: application::EstimateSubmissionRepository + 'static,
    H: application::ToolCallHistory + 'static,
    X: application::EstimateRunContextSource + 'static,
    G: application::ProfileSource + 'static,
    O: application::ScorecardObservationRepository + 'static,
    C: Clock + 'static,
{
    fn register_into(&self, registry: &mut ToolRegistry) -> Result<(), ToolRegistryError> {
        estimates::register_estimate_tools(registry, self)
    }
}

#[derive(Debug)]
pub struct AnalysisCatalogPorts<P, M, N, W, A, T, R, H, E> {
    pub portfolio: Arc<P>,
    pub market: Arc<M>,
    pub news: Arc<N>,
    pub research: Arc<W>,
    pub article: Arc<A>,
    pub thesis: Arc<T>,
    pub thesis_repository: Arc<R>,
    pub tool_history: Arc<H>,
    pub estimates: E,
}

pub fn build_analysis_catalog<P, M, N, W, A, T, R, H, E, C>(
    ports: &AnalysisCatalogPorts<P, M, N, W, A, T, R, H, E>,
    clock: &Arc<C>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    P: PortfolioToolReads + 'static,
    M: MarketToolReads + 'static,
    N: NewsToolReads + 'static,
    W: WebResearchToolPort + 'static,
    A: ArticleToolPort + 'static,
    T: ThesisToolPort + 'static,
    R: ThesisRepository + 'static,
    H: application::ToolCallHistory + 'static,
    E: CatalogSection,
    C: Clock + 'static,
{
    let mut registry = ToolRegistry::new();
    portfolio::register_portfolio_tools(&mut registry, &ports.portfolio, cursors)?;
    market::register_market_tools(&mut registry, &ports.market, cursors)?;
    news::register_news_tools(&mut registry, &ports.news, cursors)?;
    research::register_web_research_tool(&mut registry, &ports.research, clock)?;
    article::register_read_article_tool(&mut registry, &ports.article)?;
    thesis::register_thesis_tools(
        &mut registry,
        &ports.thesis,
        &ports.thesis_repository,
        &ports.tool_history,
    )?;
    ports.estimates.register_into(&mut registry)?;
    Ok(registry)
}
