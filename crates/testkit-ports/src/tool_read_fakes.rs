use application::{
    AllocationToolView, PerformancePeriod, PerformanceToolView, PortfolioToolReads,
    PortfolioToolView, SystemPortError, TransactionsToolView,
};

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
