use std::sync::Arc;

use application::{
    ArticleToolPort, Clock, MarketToolReads, NewsToolReads, PortfolioToolReads, ThesisRepository,
    ThesisToolPort, WebResearchToolPort,
};

use crate::pagination::CursorStore;
use crate::tools::{ToolRegistry, ToolRegistryError};

pub mod article;
pub mod knowledge;
pub mod market;
pub mod news;
pub mod portfolio;
mod render;
pub mod research;
pub mod thesis;

#[derive(Debug)]
pub struct AnalysisCatalogPorts<P, M, N, W, A, T, R> {
    pub portfolio: Arc<P>,
    pub market: Arc<M>,
    pub news: Arc<N>,
    pub research: Arc<W>,
    pub article: Arc<A>,
    pub thesis: Arc<T>,
    pub thesis_repository: Arc<R>,
}

pub fn build_analysis_catalog<P, M, N, W, A, T, R, C>(
    ports: &AnalysisCatalogPorts<P, M, N, W, A, T, R>,
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
    C: Clock + 'static,
{
    let mut registry = ToolRegistry::new();
    portfolio::register_portfolio_tools(&mut registry, &ports.portfolio, cursors)?;
    market::register_market_tools(&mut registry, &ports.market, cursors)?;
    news::register_news_tools(&mut registry, &ports.news, cursors)?;
    research::register_web_research_tool(&mut registry, &ports.research, clock)?;
    article::register_read_article_tool(&mut registry, &ports.article)?;
    thesis::register_thesis_tools(&mut registry, &ports.thesis, &ports.thesis_repository)?;
    Ok(registry)
}
