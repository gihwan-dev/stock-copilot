use crate::{
    AccountId, ReconciliationError, Settlement, SettlementWindow, TransactionBatch, TransactionKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum KindSelection {
    All,
    Only(Vec<TransactionKind>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionFilter(KindSelection);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KindCoverage {
    Included,
    Excluded,
}
impl TransactionFilter {
    #[must_use]
    pub const fn all() -> Self {
        Self(KindSelection::All)
    }
    pub fn only(mut kinds: Vec<TransactionKind>) -> Result<Self, ReconciliationError> {
        if kinds.is_empty() || kinds.len() > 64 {
            return Err(ReconciliationError::InvalidFilter);
        }
        kinds.sort_unstable();
        kinds.dedup();
        Ok(Self(KindSelection::Only(kinds)))
    }
    #[must_use]
    pub fn selected_kinds(&self) -> Option<&[TransactionKind]> {
        match &self.0 {
            KindSelection::All => None,
            KindSelection::Only(kinds) => Some(kinds),
        }
    }
    pub(crate) fn coverage(&self, kind: &TransactionKind) -> KindCoverage {
        match &self.0 {
            KindSelection::All => KindCoverage::Included,
            KindSelection::Only(kinds) => {
                if kinds.binary_search(kind).is_ok() {
                    KindCoverage::Included
                } else {
                    KindCoverage::Excluded
                }
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionAttestation {
    AllPagesAndAllMatchingPendingReceived,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTransactionResponse {
    account: AccountId,
    window: SettlementWindow,
    filter: TransactionFilter,
    rows: TransactionBatch,
}
impl CompleteTransactionResponse {
    pub fn after_last_page_without_quarantine(
        account: AccountId,
        window: SettlementWindow,
        filter: TransactionFilter,
        rows: TransactionBatch,
        completion: CompletionAttestation,
    ) -> Result<Self, ReconciliationError> {
        let CompletionAttestation::AllPagesAndAllMatchingPendingReceived = completion;
        for row in rows.as_slice() {
            if filter.coverage(&row.data().kind) == KindCoverage::Excluded {
                return Err(ReconciliationError::OutsideFilter);
            }
            if row.data().settlement == Settlement::Settled
                && window.coverage(row.data().settlement_date)
                    == super::settled_row_set::WindowCoverage::Outside
            {
                return Err(ReconciliationError::OutsideWindow);
            }
        }
        Ok(Self {
            account,
            window,
            filter,
            rows,
        })
    }
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub const fn window(&self) -> SettlementWindow {
        self.window
    }
    #[must_use]
    pub const fn filter(&self) -> &TransactionFilter {
        &self.filter
    }
    #[must_use]
    pub const fn rows(&self) -> &TransactionBatch {
        &self.rows
    }
}
