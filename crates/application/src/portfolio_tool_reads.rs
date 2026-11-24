use std::future::Future;

use domain::{
    Currency, DisplayInstrumentName, FxRate, MaskedAccountNumber, Money, Quantity, Ratio, Symbol,
    Timestamp, TotalCompleteness,
};

use crate::SystemPortError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingToolLine {
    pub symbol: Symbol,
    pub name: DisplayInstrumentName,
    pub quantity: Quantity,
    pub valuation: Option<Money>,
    pub purchase: Option<Money>,
    pub profit: Option<Money>,
    pub weight: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyingPowerLine {
    pub currency: Currency,
    pub amount: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioToolView {
    pub account: MaskedAccountNumber,
    pub recorded_at: Timestamp,
    pub completeness: TotalCompleteness,
    pub holdings: Vec<HoldingToolLine>,
    pub buying_power: Vec<BuyingPowerLine>,
    pub approximate_total_assets: Option<Money>,
    pub fx_used: Option<FxRate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationAxis {
    Sector,
    Country,
    Currency,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AllocationLabel(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationLabelError {
    Empty,
    TooLong,
}

impl AllocationLabel {
    pub fn try_new(raw: &str) -> Result<Self, AllocationLabelError> {
        if raw.trim().is_empty() {
            return Err(AllocationLabelError::Empty);
        }
        if raw.chars().count() > 128_usize {
            return Err(AllocationLabelError::TooLong);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationLine {
    pub axis: AllocationAxis,
    pub label: AllocationLabel,
    pub weight: Ratio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationLine {
    pub largest_holding_weight: Option<Ratio>,
    pub top_five_weight: Option<Ratio>,
    pub herfindahl_index: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationToolView {
    pub account: MaskedAccountNumber,
    pub recorded_at: Timestamp,
    pub completeness: TotalCompleteness,
    pub lines: Vec<AllocationLine>,
    pub concentration: ConcentrationLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionToolKind {
    Buy,
    Sell,
    Dividend,
    Deposit,
    Withdrawal,
    Fee,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionToolFilter {
    All,
    Trades,
    Dividends,
    CashFlows,
}

impl TransactionToolFilter {
    #[must_use]
    pub const fn admits(self, kind: TransactionToolKind) -> TransactionAdmission {
        let admitted = match self {
            Self::All => true,
            Self::Trades => matches_trade(kind),
            Self::Dividends => matches_dividend(kind),
            Self::CashFlows => matches_cash_flow(kind),
        };
        if admitted {
            TransactionAdmission::Admitted
        } else {
            TransactionAdmission::Excluded
        }
    }
}

const fn matches_trade(kind: TransactionToolKind) -> bool {
    match kind {
        TransactionToolKind::Buy | TransactionToolKind::Sell => true,
        TransactionToolKind::Dividend
        | TransactionToolKind::Deposit
        | TransactionToolKind::Withdrawal
        | TransactionToolKind::Fee
        | TransactionToolKind::Other => false,
    }
}

const fn matches_dividend(kind: TransactionToolKind) -> bool {
    match kind {
        TransactionToolKind::Dividend => true,
        TransactionToolKind::Buy
        | TransactionToolKind::Sell
        | TransactionToolKind::Deposit
        | TransactionToolKind::Withdrawal
        | TransactionToolKind::Fee
        | TransactionToolKind::Other => false,
    }
}

const fn matches_cash_flow(kind: TransactionToolKind) -> bool {
    match kind {
        TransactionToolKind::Deposit | TransactionToolKind::Withdrawal => true,
        TransactionToolKind::Buy
        | TransactionToolKind::Sell
        | TransactionToolKind::Dividend
        | TransactionToolKind::Fee
        | TransactionToolKind::Other => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionAdmission {
    Admitted,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionToolLine {
    pub occurred_at: Timestamp,
    pub kind: TransactionToolKind,
    pub symbol: Option<Symbol>,
    pub quantity: Option<Quantity>,
    pub amount: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionsToolView {
    pub account: MaskedAccountNumber,
    pub recorded_at: Timestamp,
    pub lines: Vec<TransactionToolLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformancePeriod {
    OneMonth,
    ThreeMonths,
    SixMonths,
    OneYear,
    SinceStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleSufficiency {
    Sufficient,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PerformanceBenchmark {
    Kospi,
    Kosdaq,
    Sp500,
    Nasdaq100,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkReturnLine {
    pub benchmark: PerformanceBenchmark,
    pub period_return: Ratio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskMetrics {
    pub annualized_volatility: Option<Ratio>,
    pub max_drawdown: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceToolView {
    pub account: MaskedAccountNumber,
    pub period: PerformancePeriod,
    pub computed_at: Timestamp,
    pub sample: SampleSufficiency,
    pub time_weighted_return: Option<Ratio>,
    pub money_weighted_return: Option<Ratio>,
    pub benchmarks: Vec<BenchmarkReturnLine>,
    pub risk: RiskMetrics,
}

pub trait PortfolioToolReads: Send + Sync {
    fn portfolio(
        &self,
    ) -> impl Future<Output = Result<Option<PortfolioToolView>, SystemPortError>> + Send;

    fn allocation(
        &self,
    ) -> impl Future<Output = Result<Option<AllocationToolView>, SystemPortError>> + Send;

    fn transactions(
        &self,
    ) -> impl Future<Output = Result<Option<TransactionsToolView>, SystemPortError>> + Send;

    fn performance(
        &self,
        period: PerformancePeriod,
    ) -> impl Future<Output = Result<Option<PerformanceToolView>, SystemPortError>> + Send;
}
