use crate::PortError;
use domain::{
    AccountId, CompleteTransactionResponse, Holding, Quote, QuoteInstrument, ReportedPortfolio,
    SettlementWindow, TransactionBatch, TransactionFilter, UntrustedText,
};

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
    pub instrument: QuoteInstrument,
    pub result: Result<Quote, PortError>,
}
