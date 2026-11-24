use application::{
    AiRunId, AllocationToolView, CandleWindow, CandlesToolView, CollectedItemQuery,
    DisclosureToolLine, InvestorFlowToolView, MarketOverviewToolView, MarketToolReads,
    NewsToolLine, NewsToolReads, PerformancePeriod, PerformanceToolView, PortfolioToolReads,
    PortfolioToolView, QuoteToolView, RankingToolKind, RankingsToolView, SectorsToolView,
    SystemPortError, TransactionsToolView, WebResearchToolFailure, WebResearchToolOutcome,
    WebResearchToolPort, WebResearchToolRequest,
};

use domain::{KnownMarket, Symbol};
use std::future::{Future, ready};

use crate::script::State;

pub type ReadReply<View> = Result<Option<View>, SystemPortError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortfolioReadCall {
    Portfolio,
    Allocation,
    Transactions,
    Performance(PerformancePeriod),
}

#[derive(Debug)]
pub struct FixedPortfolioToolReads {
    portfolio: ReadReply<PortfolioToolView>,
    allocation: ReadReply<AllocationToolView>,
    transactions: ReadReply<TransactionsToolView>,
    performance: ReadReply<PerformanceToolView>,
    calls: State<Vec<PortfolioReadCall>>,
}

impl Default for FixedPortfolioToolReads {
    fn default() -> Self {
        Self {
            portfolio: Ok(None),
            allocation: Ok(None),
            transactions: Ok(None),
            performance: Ok(None),
            calls: State::default(),
        }
    }
}

impl FixedPortfolioToolReads {
    #[must_use]
    pub fn with_portfolio(mut self, reply: ReadReply<PortfolioToolView>) -> Self {
        self.portfolio = reply;
        self
    }

    #[must_use]
    pub fn with_allocation(mut self, reply: ReadReply<AllocationToolView>) -> Self {
        self.allocation = reply;
        self
    }

    #[must_use]
    pub fn with_transactions(mut self, reply: ReadReply<TransactionsToolView>) -> Self {
        self.transactions = reply;
        self
    }

    #[must_use]
    pub fn with_performance(mut self, reply: ReadReply<PerformanceToolView>) -> Self {
        self.performance = reply;
        self
    }

    #[must_use]
    pub fn calls(&self) -> Vec<PortfolioReadCall> {
        self.calls.write().clone()
    }

    fn record(&self, call: PortfolioReadCall) {
        self.calls.write().push(call);
    }
}

impl PortfolioToolReads for FixedPortfolioToolReads {
    fn portfolio(&self) -> impl Future<Output = ReadReply<PortfolioToolView>> + Send {
        self.record(PortfolioReadCall::Portfolio);
        ready(self.portfolio.clone())
    }

    fn allocation(&self) -> impl Future<Output = ReadReply<AllocationToolView>> + Send {
        self.record(PortfolioReadCall::Allocation);
        ready(self.allocation.clone())
    }

    fn transactions(&self) -> impl Future<Output = ReadReply<TransactionsToolView>> + Send {
        self.record(PortfolioReadCall::Transactions);
        ready(self.transactions.clone())
    }

    fn performance(
        &self,
        period: PerformancePeriod,
    ) -> impl Future<Output = ReadReply<PerformanceToolView>> + Send {
        self.record(PortfolioReadCall::Performance(period));
        ready(self.performance.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketReadCall {
    Quote(Symbol, KnownMarket),
    Candles(Symbol, KnownMarket, CandleWindow),
    Overview,
    Rankings(RankingToolKind, KnownMarket),
    Sectors(KnownMarket),
    InvestorFlow(Symbol, KnownMarket),
}

#[derive(Debug)]
pub struct FixedMarketToolReads {
    quote: ReadReply<QuoteToolView>,
    candles: ReadReply<CandlesToolView>,
    overview: ReadReply<MarketOverviewToolView>,
    rankings: ReadReply<RankingsToolView>,
    sectors: ReadReply<SectorsToolView>,
    investor_flow: ReadReply<InvestorFlowToolView>,
    calls: State<Vec<MarketReadCall>>,
}

impl Default for FixedMarketToolReads {
    fn default() -> Self {
        Self {
            quote: Ok(None),
            candles: Ok(None),
            overview: Ok(None),
            rankings: Ok(None),
            sectors: Ok(None),
            investor_flow: Ok(None),
            calls: State::default(),
        }
    }
}

impl FixedMarketToolReads {
    #[must_use]
    pub fn with_quote(mut self, reply: ReadReply<QuoteToolView>) -> Self {
        self.quote = reply;
        self
    }

    #[must_use]
    pub fn with_candles(mut self, reply: ReadReply<CandlesToolView>) -> Self {
        self.candles = reply;
        self
    }

    #[must_use]
    pub fn with_overview(mut self, reply: ReadReply<MarketOverviewToolView>) -> Self {
        self.overview = reply;
        self
    }

    #[must_use]
    pub fn with_rankings(mut self, reply: ReadReply<RankingsToolView>) -> Self {
        self.rankings = reply;
        self
    }

    #[must_use]
    pub fn with_sectors(mut self, reply: ReadReply<SectorsToolView>) -> Self {
        self.sectors = reply;
        self
    }

    #[must_use]
    pub fn with_investor_flow(mut self, reply: ReadReply<InvestorFlowToolView>) -> Self {
        self.investor_flow = reply;
        self
    }

    #[must_use]
    pub fn calls(&self) -> Vec<MarketReadCall> {
        self.calls.write().clone()
    }

    fn record(&self, call: MarketReadCall) {
        self.calls.write().push(call);
    }
}

impl MarketToolReads for FixedMarketToolReads {
    fn quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = ReadReply<QuoteToolView>> + Send {
        self.record(MarketReadCall::Quote(symbol.clone(), market));
        ready(self.quote.clone())
    }

    fn candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: CandleWindow,
    ) -> impl Future<Output = ReadReply<CandlesToolView>> + Send {
        self.record(MarketReadCall::Candles(symbol.clone(), market, window));
        ready(self.candles.clone())
    }

    fn market_overview(&self) -> impl Future<Output = ReadReply<MarketOverviewToolView>> + Send {
        self.record(MarketReadCall::Overview);
        ready(self.overview.clone())
    }

    fn rankings(
        &self,
        kind: RankingToolKind,
        market: KnownMarket,
    ) -> impl Future<Output = ReadReply<RankingsToolView>> + Send {
        self.record(MarketReadCall::Rankings(kind, market));
        ready(self.rankings.clone())
    }

    fn sectors(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = ReadReply<SectorsToolView>> + Send {
        self.record(MarketReadCall::Sectors(market));
        ready(self.sectors.clone())
    }

    fn investor_flow(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = ReadReply<InvestorFlowToolView>> + Send {
        self.record(MarketReadCall::InvestorFlow(symbol.clone(), market));
        ready(self.investor_flow.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsReadCall {
    SearchNews(CollectedItemQuery),
    Disclosures(CollectedItemQuery),
}

#[derive(Debug)]
pub struct FixedNewsToolReads {
    news: Result<Vec<NewsToolLine>, SystemPortError>,
    disclosures: Result<Vec<DisclosureToolLine>, SystemPortError>,
    calls: State<Vec<NewsReadCall>>,
}

impl Default for FixedNewsToolReads {
    fn default() -> Self {
        Self {
            news: Ok(Vec::new()),
            disclosures: Ok(Vec::new()),
            calls: State::default(),
        }
    }
}

impl FixedNewsToolReads {
    #[must_use]
    pub fn with_news(mut self, reply: Result<Vec<NewsToolLine>, SystemPortError>) -> Self {
        self.news = reply;
        self
    }

    #[must_use]
    pub fn with_disclosures(
        mut self,
        reply: Result<Vec<DisclosureToolLine>, SystemPortError>,
    ) -> Self {
        self.disclosures = reply;
        self
    }

    #[must_use]
    pub fn calls(&self) -> Vec<NewsReadCall> {
        self.calls.write().clone()
    }
}

impl NewsToolReads for FixedNewsToolReads {
    fn search_news(
        &self,
        query: &CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<NewsToolLine>, SystemPortError>> + Send {
        self.calls
            .write()
            .push(NewsReadCall::SearchNews(query.clone()));
        ready(self.news.clone())
    }

    fn disclosures(
        &self,
        query: &CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<DisclosureToolLine>, SystemPortError>> + Send {
        self.calls
            .write()
            .push(NewsReadCall::Disclosures(query.clone()));
        ready(self.disclosures.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebResearchCall {
    KnownTargets(AiRunId),
    Request(AiRunId, WebResearchToolRequest),
}

#[derive(Debug)]
pub struct FixedWebResearchToolPort {
    known: Result<Vec<Symbol>, SystemPortError>,
    outcome: Result<WebResearchToolOutcome, WebResearchToolFailure>,
    calls: State<Vec<WebResearchCall>>,
}

impl FixedWebResearchToolPort {
    #[must_use]
    pub fn new(
        known: Result<Vec<Symbol>, SystemPortError>,
        outcome: Result<WebResearchToolOutcome, WebResearchToolFailure>,
    ) -> Self {
        Self {
            known,
            outcome,
            calls: State::default(),
        }
    }

    #[must_use]
    pub fn calls(&self) -> Vec<WebResearchCall> {
        self.calls.write().clone()
    }
}

impl WebResearchToolPort for FixedWebResearchToolPort {
    fn known_targets(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<Vec<Symbol>, SystemPortError>> + Send {
        self.calls
            .write()
            .push(WebResearchCall::KnownTargets(run.clone()));
        ready(self.known.clone())
    }

    fn request_research(
        &self,
        run: &AiRunId,
        request: &WebResearchToolRequest,
    ) -> impl Future<Output = Result<WebResearchToolOutcome, WebResearchToolFailure>> + Send {
        self.calls
            .write()
            .push(WebResearchCall::Request(run.clone(), request.clone()));
        ready(self.outcome.clone())
    }
}
