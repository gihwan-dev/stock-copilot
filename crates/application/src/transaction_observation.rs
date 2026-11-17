use domain::{
    AccountId, CompleteTransactionResponse, CompletionAttestation, ReconciliationError,
    SettlementWindow, Transaction, TransactionBatch, TransactionFilter,
};

#[derive(Clone, PartialEq, Eq)]
pub struct RawTransactionRow(String);
impl RawTransactionRow {
    #[must_use]
    pub fn new(source: String) -> Self {
        Self(source)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for RawTransactionRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawTransactionRow([redacted])")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransaction {
    transaction: Transaction,
    source: RawTransactionRow,
}
impl ObservedTransaction {
    #[must_use]
    pub const fn new(transaction: Transaction, source: RawTransactionRow) -> Self {
        Self {
            transaction,
            source,
        }
    }
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }
    #[must_use]
    pub const fn source(&self) -> &RawTransactionRow {
        &self.source
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransactionBatch {
    rows: Vec<ObservedTransaction>,
    normalized: TransactionBatch,
}
impl ObservedTransactionBatch {
    pub fn try_new(rows: Vec<ObservedTransaction>) -> Result<Self, ReconciliationError> {
        let normalized =
            TransactionBatch::try_new(rows.iter().map(|row| row.transaction().clone()).collect())?;
        Ok(Self { rows, normalized })
    }
    #[must_use]
    pub fn observed_rows(&self) -> &[ObservedTransaction] {
        &self.rows
    }
    #[must_use]
    pub fn as_slice(&self) -> &[Transaction] {
        self.normalized.as_slice()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTransactionObservation {
    rows: ObservedTransactionBatch,
    normalized: CompleteTransactionResponse,
}
impl CompleteTransactionObservation {
    pub fn after_last_page_without_quarantine(
        account: AccountId,
        window: SettlementWindow,
        filter: TransactionFilter,
        rows: ObservedTransactionBatch,
        completion: CompletionAttestation,
    ) -> Result<Self, ReconciliationError> {
        let normalized = CompleteTransactionResponse::after_last_page_without_quarantine(
            account,
            window,
            filter,
            rows.normalized.clone(),
            completion,
        )?;
        Ok(Self { rows, normalized })
    }
    #[must_use]
    pub const fn normalized(&self) -> &CompleteTransactionResponse {
        &self.normalized
    }
    #[must_use]
    pub const fn rows(&self) -> &ObservedTransactionBatch {
        &self.rows
    }
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        self.normalized.account_id()
    }
    #[must_use]
    pub const fn window(&self) -> SettlementWindow {
        self.normalized.window()
    }
    #[must_use]
    pub const fn filter(&self) -> &TransactionFilter {
        self.normalized.filter()
    }
}
