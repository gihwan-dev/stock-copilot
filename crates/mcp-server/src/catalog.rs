use std::sync::Arc;

use application::{
    ArticleToolPort, Clock, MarketToolReads, NewsToolReads, PortfolioToolReads, WebResearchToolPort,
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

#[derive(Debug)]
pub struct AnalysisCatalogPorts<P, M, N, W, A> {
    pub portfolio: Arc<P>,
    pub market: Arc<M>,
    pub news: Arc<N>,
    pub research: Arc<W>,
    pub article: Arc<A>,
}

pub fn build_analysis_catalog<P, M, N, W, A, C>(
    ports: &AnalysisCatalogPorts<P, M, N, W, A>,
    clock: &Arc<C>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    P: PortfolioToolReads + 'static,
    M: MarketToolReads + 'static,
    N: NewsToolReads + 'static,
    W: WebResearchToolPort + 'static,
    A: ArticleToolPort + 'static,
    C: Clock + 'static,
{
    let mut registry = ToolRegistry::new();
    portfolio::register_portfolio_tools(&mut registry, &ports.portfolio, cursors)?;
    market::register_market_tools(&mut registry, &ports.market, cursors)?;
    news::register_news_tools(&mut registry, &ports.news, cursors)?;
    research::register_web_research_tool(&mut registry, &ports.research, clock)?;
    article::register_read_article_tool(&mut registry, &ports.article)?;
    Ok(registry)
}
