use crate::PortError;
use domain::{
    AccountDisplayFact, AccountId, CompleteTransactionResponse, Holding, Quote, ReportedPortfolio,
    ResolvedOrderInstrument, SettlementWindow, TransactionBatch, TransactionFilter, UntrustedText,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountObservation {
    account: AccountId,
    display: AccountDisplayFact,
}
impl AccountObservation {
    #[must_use]
    pub const fn from_broker_result(account: AccountId, display: AccountDisplayFact) -> Self {
        Self { account, display }
    }
    #[must_use]
    pub const fn account(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub const fn display(&self) -> &AccountDisplayFact {
        &self.display
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingsObservation {
    holdings: Vec<Holding>,
    reported: ReportedPortfolio,
}
impl HoldingsObservation {
    #[must_use]
    pub const fn from_broker_result(holdings: Vec<Holding>, reported: ReportedPortfolio) -> Self {
        Self { holdings, reported }
    }
    #[must_use]
    pub fn holdings(&self) -> &[Holding] {
        &self.holdings
    }
    #[must_use]
    pub const fn reported(&self) -> &ReportedPortfolio {
        &self.reported
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantinedTransaction {
    UnknownCurrency(Vec<UntrustedText>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompleteReason {
    QuarantinedRows,
    RowLimitExceeded,
    PageInterrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteTransactions {
    pub account: AccountId,
    pub window: SettlementWindow,
    pub filter: TransactionFilter,
    pub received: Vec<TransactionBatch>,
    pub quarantined: Vec<QuarantinedTransaction>,
    pub reason: IncompleteReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionRead {
    Complete(CompleteTransactionResponse),
    Incomplete(IncompleteTransactions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteObservation {
    pub instrument: ResolvedOrderInstrument,
    pub result: Result<Quote, PortError>,
}
