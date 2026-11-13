use crate::{CalendarDate, Settlement, Timestamp, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationError {
    TooManyRows,
    InvalidWindow,
    OutsideWindow,
    InvalidHistory,
    PendingInHistory,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementWindow {
    from: CalendarDate,
    to: CalendarDate,
}
impl SettlementWindow {
    pub fn try_new(from: CalendarDate, to: CalendarDate) -> Result<Self, ReconciliationError> {
        if from > to {
            return Err(ReconciliationError::InvalidWindow);
        }
        Ok(Self { from, to })
    }
    #[must_use]
    pub const fn from(self) -> CalendarDate {
        self.from
    }
    #[must_use]
    pub const fn to(self) -> CalendarDate {
        self.to
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBatch(Vec<Transaction>);
impl TransactionBatch {
    pub fn try_new(rows: Vec<Transaction>) -> Result<Self, ReconciliationError> {
        if rows.len() > 4096 {
            return Err(ReconciliationError::TooManyRows);
        }
        Ok(Self(rows))
    }
    #[must_use]
    pub fn as_slice(&self) -> &[Transaction] {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTransaction {
    transaction: Transaction,
    first_seen: Timestamp,
    disappeared_at: Option<Timestamp>,
}
impl StoredTransaction {
    pub fn try_restore(
        transaction: Transaction,
        first_seen: Timestamp,
        disappeared_at: Option<Timestamp>,
    ) -> Result<Self, ReconciliationError> {
        if transaction.data().settlement == Settlement::Pending {
            return Err(ReconciliationError::PendingInHistory);
        }
        if disappeared_at.is_some_and(|at| at < first_seen) {
            return Err(ReconciliationError::InvalidHistory);
        }
        Ok(Self {
            transaction,
            first_seen,
            disappeared_at,
        })
    }
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }
    #[must_use]
    pub const fn first_seen(&self) -> Timestamp {
        self.first_seen
    }
    #[must_use]
    pub const fn disappeared_at(&self) -> Option<Timestamp> {
        self.disappeared_at
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledRowSet {
    rows: Vec<StoredTransaction>,
    pending: TransactionBatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowChange {
    Added(StoredTransaction),
    Unchanged(StoredTransaction),
    Disappeared(StoredTransaction),
    PendingReplaced {
        before: TransactionBatch,
        after: TransactionBatch,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    state: SettledRowSet,
    changes: Vec<RowChange>,
}
impl Reconciliation {
    #[must_use]
    pub const fn state(&self) -> &SettledRowSet {
        &self.state
    }
    #[must_use]
    pub fn changes(&self) -> &[RowChange] {
        &self.changes
    }
}
impl Default for SettledRowSet {
    fn default() -> Self {
        Self::new()
    }
}
impl SettledRowSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            pending: TransactionBatch(Vec::new()),
        }
    }
    pub fn try_restore(
        rows: Vec<StoredTransaction>,
        pending: TransactionBatch,
    ) -> Result<Self, ReconciliationError> {
        if rows.len() > 4096 {
            return Err(ReconciliationError::TooManyRows);
        }
        if pending
            .0
            .iter()
            .any(|entry| entry.data().settlement != Settlement::Pending)
        {
            return Err(ReconciliationError::InvalidHistory);
        }
        Ok(Self { rows, pending })
    }
    #[must_use]
    pub fn rows(&self) -> &[StoredTransaction] {
        &self.rows
    }
    #[must_use]
    pub const fn pending(&self) -> &TransactionBatch {
        &self.pending
    }
    pub fn reconcile(
        &self,
        incoming: &TransactionBatch,
        window: SettlementWindow,
        at: Timestamp,
    ) -> Result<Reconciliation, ReconciliationError> {
        if incoming.0.iter().any(|entry| {
            entry.data().settlement_date < window.from || entry.data().settlement_date > window.to
        }) {
            return Err(ReconciliationError::OutsideWindow);
        }
        if self.rows.iter().any(|entry| {
            entry.first_seen > at || entry.disappeared_at.is_some_and(|instant| instant > at)
        }) {
            return Err(ReconciliationError::InvalidHistory);
        }
        let (pending_rows, settled_rows): (Vec<_>, Vec<_>) = incoming
            .0
            .iter()
            .cloned()
            .partition(|entry| entry.data().settlement == Settlement::Pending);
        let mut remaining = settled_rows.into_iter().map(Some).collect::<Vec<_>>();
        let (mut rows, mut changes) = (Vec::new(), Vec::new());
        for previous in &self.rows {
            let (stored, change) = reconcile_saved(previous, &mut remaining, window, at);
            rows.push(stored);
            changes.push(change);
        }
        for transaction in remaining.into_iter().flatten() {
            let stored = StoredTransaction {
                transaction,
                first_seen: at,
                disappeared_at: None,
            };
            changes.push(RowChange::Added(stored.clone()));
            rows.push(stored);
        }
        let mut next_pending = self
            .pending
            .0
            .iter()
            .filter(|entry| {
                entry.data().settlement_date < window.from
                    || entry.data().settlement_date > window.to
            })
            .cloned()
            .collect::<Vec<_>>();
        next_pending.extend(pending_rows);
        let pending = TransactionBatch::try_new(next_pending)?;
        if same_multiset(&self.pending, &pending) == MultisetComparison::Different {
            changes.push(RowChange::PendingReplaced {
                before: self.pending.clone(),
                after: pending.clone(),
            });
        } else {
            return Ok(Reconciliation {
                state: Self::try_restore(rows, self.pending.clone())?,
                changes,
            });
        }
        Ok(Reconciliation {
            state: Self::try_restore(rows, pending)?,
            changes,
        })
    }
}
#[derive(PartialEq, Eq)]
enum MultisetComparison {
    Same,
    Different,
}
fn same_multiset(left: &TransactionBatch, right: &TransactionBatch) -> MultisetComparison {
    let mut remaining = right.0.iter().cloned().map(Some).collect::<Vec<_>>();
    for value in &left.0 {
        if consume(&mut remaining, value).is_none() {
            return MultisetComparison::Different;
        }
    }
    if remaining.into_iter().flatten().next().is_none() {
        MultisetComparison::Same
    } else {
        MultisetComparison::Different
    }
}
fn consume(remaining: &mut [Option<Transaction>], value: &Transaction) -> Option<Transaction> {
    remaining
        .iter_mut()
        .find(|entry| entry.as_ref() == Some(value))
        .and_then(Option::take)
}
fn reconcile_saved(
    previous: &StoredTransaction,
    remaining: &mut [Option<Transaction>],
    window: SettlementWindow,
    at: Timestamp,
) -> (StoredTransaction, RowChange) {
    let mut stored = previous.clone();
    let change = if stored.disappeared_at.is_some()
        || stored.transaction.data().settlement_date < window.from
        || stored.transaction.data().settlement_date > window.to
        || consume(remaining, &stored.transaction).is_some()
    {
        RowChange::Unchanged(stored.clone())
    } else {
        stored.disappeared_at = Some(at);
        RowChange::Disappeared(stored.clone())
    };
    (stored, change)
}

#[cfg(test)]
mod tests {
    use super::{
        ReconciliationError, SettledRowSet, SettlementWindow, StoredTransaction, TransactionBatch,
    };
    use crate::{
        Amount, AmountInput, CalendarDate, CalendarYear, Cancellation, Currency, DayNumber, Market,
        Money, MonthNumber, Nanosecond, OffsetSeconds, Settlement, Timestamp, Transaction,
        TransactionCode, TransactionData, TransactionKind, UnixSeconds,
    };
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn row(day: u8, settlement: Settlement) -> Transaction {
        let zero = Money::new(
            Amount::parse(&AmountInput::new("0")).expect("금액"),
            Currency::Krw,
        );
        Transaction::try_new(TransactionData {
            kind: TransactionKind::new(TransactionCode::try_new("BUY").expect("코드")),
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: None,
            commission: zero,
            tax: zero,
            net_amount: None,
            exchange_rate: None,
            settlement_date: date(day),
            settlement,
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    #[test]
    fn combined_capacity_failures_preserve_history() {
        let settled = row(14, Settlement::Settled(time()));
        let stored =
            StoredTransaction::try_restore(settled.clone(), time(), Some(time())).expect("보존");
        let full = SettledRowSet::try_restore(
            vec![stored; 4096],
            TransactionBatch::try_new(vec![]).expect("임시"),
        )
        .expect("상한");
        let window = SettlementWindow::try_new(date(14), date(14)).expect("창");
        assert_eq!(
            full.reconcile(
                &TransactionBatch::try_new(vec![settled.clone()]).expect("입력"),
                window,
                time()
            ),
            Err(ReconciliationError::TooManyRows)
        );
        assert_eq!(
            full.reconcile(
                &TransactionBatch::try_new(vec![settled, row(14, Settlement::Pending)])
                    .expect("입력"),
                window,
                time()
            ),
            Err(ReconciliationError::TooManyRows)
        );
        let provisional = SettledRowSet::try_restore(
            vec![],
            TransactionBatch::try_new(vec![row(13, Settlement::Pending); 4096]).expect("임시 상한"),
        )
        .expect("복원");
        assert_eq!(
            provisional.reconcile(
                &TransactionBatch::try_new(vec![row(14, Settlement::Pending)]).expect("입력"),
                window,
                time()
            ),
            Err(ReconciliationError::TooManyRows)
        );
        assert_eq!(full.rows().len(), 4096);
    }
}
