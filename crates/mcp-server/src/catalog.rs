use std::sync::Arc;

use application::{Clock, MarketToolReads, PortfolioToolReads};

use crate::pagination::CursorStore;
use crate::tools::{ToolRegistry, ToolRegistryError};

pub mod knowledge;
pub mod market;
pub mod news;
pub mod portfolio;
mod render;

#[derive(Debug)]
pub struct AnalysisCatalogPorts<P, M> {
    pub portfolio: Arc<P>,
    pub market: Arc<M>,
}

pub fn build_analysis_catalog<P, M, C>(
    ports: &AnalysisCatalogPorts<P, M>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<ToolRegistry, ToolRegistryError>
where
    P: PortfolioToolReads + 'static,
    M: MarketToolReads + 'static,
    C: Clock + 'static,
{
    let mut registry = ToolRegistry::new();
    portfolio::register_portfolio_tools(&mut registry, &ports.portfolio, cursors)?;
    market::register_market_tools(&mut registry, &ports.market, cursors)?;
    Ok(registry)
}
