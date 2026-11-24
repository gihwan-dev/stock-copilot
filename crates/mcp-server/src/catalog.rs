use std::sync::Arc;

use application::{Clock, MarketToolReads, NewsToolReads, PortfolioToolReads};

use crate::pagination::CursorStore;
use crate::tools::{ToolRegistry, ToolRegistryError};

pub mod knowledge;
pub mod market;
pub mod news;
pub mod portfolio;
mod render;

#[derive(Debug)]
pub struct AnalysisCatalogPorts<P, M, N> {
    pub portfolio: Arc<P>,
    pub market: Arc<M>,
    pub news: Arc<N>,
}

pub fn build_analysis_catalog<P, M, N, C>(
    ports: &AnalysisCatalogPorts<P, M, N>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    P: PortfolioToolReads + 'static,
    M: MarketToolReads + 'static,
    N: NewsToolReads + 'static,
    C: Clock + 'static,
{
    let mut registry = ToolRegistry::new();
    portfolio::register_portfolio_tools(&mut registry, &ports.portfolio, cursors)?;
    market::register_market_tools(&mut registry, &ports.market, cursors)?;
    news::register_news_tools(&mut registry, &ports.news, cursors)?;
    Ok(registry)
}
