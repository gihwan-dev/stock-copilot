use crate::transaction_order::CancellationIdentity;
use crate::transaction_response::KindCoverage;
use crate::{
    AccountId, CalculationEffect, CalendarDate, Cancellation, CompleteTransactionResponse,
    Settlement, Timestamp, Transaction,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationError {
    TooManyRows,
    TooMuchHistory,
    InvalidFilter,
    OutsideFilter,
    AccountMismatch,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowCoverage {
    Inside,
    Outside,
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
    pub(crate) fn coverage(self, date: CalendarDate) -> WindowCoverage {
        if date < self.from || date > self.to {
            WindowCoverage::Outside
        } else {
            WindowCoverage::Inside
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowCount(usize);
impl RowCount {
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
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
    account: AccountId,
    history: Vec<StoredTransaction>,
    pending: TransactionBatch,
    last_synchronized_at: Option<Timestamp>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisappearanceWarning {
    None,
    MassDisappearance {
        missing: RowCount,
        scoped_active: RowCount,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationNotice {
    UnconfirmedCancellation {
        cancellation: Transaction,
        possible_originals: RowCount,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    state: SettledRowSet,
    changes: Vec<RowChange>,
    warning: DisappearanceWarning,
    notices: Vec<ReconciliationNotice>,
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
    #[must_use]
    pub const fn disappearance_warning(&self) -> DisappearanceWarning {
        self.warning
    }
    #[must_use]
    pub fn notices(&self) -> &[ReconciliationNotice] {
        &self.notices
    }
}
impl SettledRowSet {
    #[must_use]
    pub const fn new(account: AccountId) -> Self {
        Self {
            account,
            history: Vec::new(),
            pending: TransactionBatch(Vec::new()),
            last_synchronized_at: None,
        }
    }
    pub fn try_restore(
        account: AccountId,
        history: Vec<StoredTransaction>,
        mut pending: TransactionBatch,
        last_synchronized_at: Option<Timestamp>,
    ) -> Result<Self, ReconciliationError> {
        if history.len() > 1_000_000 {
            return Err(ReconciliationError::TooMuchHistory);
        }
        if pending
            .0
            .iter()
            .any(|row| row.data().settlement != Settlement::Pending)
        {
            return Err(ReconciliationError::InvalidHistory);
        }
        if last_synchronized_at.is_none() && (!history.is_empty() || !pending.0.is_empty()) {
            return Err(ReconciliationError::InvalidHistory);
        }
        if let Some(at) = last_synchronized_at
            && history.iter().any(|row| {
                row.first_seen > at || row.disappeared_at.is_some_and(|instant| instant > at)
            })
        {
            return Err(ReconciliationError::InvalidHistory);
        }
        pending.0.sort_unstable();
        Ok(Self {
            account,
            history,
            pending,
            last_synchronized_at,
        })
    }
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub fn history_rows(&self) -> &[StoredTransaction] {
        &self.history
    }
    pub fn active_settled_rows(&self) -> impl Iterator<Item = &StoredTransaction> {
        self.history
            .iter()
            .filter(|row| row.disappeared_at.is_none())
    }
    pub fn active_settled_for_calculation(&self) -> impl Iterator<Item = &Transaction> {
        self.active_settled_rows()
            .map(StoredTransaction::transaction)
            .filter(|row| row.calculation_effect() == CalculationEffect::SettledTransaction)
    }
    #[must_use]
    pub const fn pending_rows(&self) -> &TransactionBatch {
        &self.pending
    }
    #[must_use]
    pub const fn last_synchronized_at(&self) -> Option<Timestamp> {
        self.last_synchronized_at
    }
    pub fn reconcile(
        &self,
        incoming: &CompleteTransactionResponse,
        at: Timestamp,
    ) -> Result<Reconciliation, ReconciliationError> {
        if &self.account != incoming.account_id() {
            return Err(ReconciliationError::AccountMismatch);
        }
        if self.last_synchronized_at.is_some_and(|last| at < last) {
            return Err(ReconciliationError::InvalidHistory);
        }
        let mut remaining = settled_groups(incoming);
        let mut history = self.history.clone();
        let mut changes = Vec::new();
        let (mut scoped_active, mut missing) = (0_usize, 0_usize);
        for stored in &mut history {
            if stored.disappeared_at.is_some()
                || incoming
                    .window()
                    .coverage(stored.transaction.data().settlement_date)
                    == WindowCoverage::Outside
                || incoming.filter().coverage(&stored.transaction.data().kind)
                    == KindCoverage::Excluded
            {
                continue;
            }
            scoped_active = scoped_active.saturating_add(1);
            if remaining
                .get_mut(&stored.transaction)
                .and_then(Vec::pop)
                .is_some()
            {
                changes.push(RowChange::Unchanged(stored.clone()));
            } else {
                stored.disappeared_at = Some(at);
                missing = missing.saturating_add(1);
                changes.push(RowChange::Disappeared(stored.clone()));
            }
        }
        for transaction in remaining.into_values().flatten() {
            let stored = StoredTransaction {
                transaction,
                first_seen: at,
                disappeared_at: None,
            };
            changes.push(RowChange::Added(stored.clone()));
            history.push(stored);
        }
        let pending = self.replace_pending(incoming)?;
        if pending != self.pending {
            changes.push(RowChange::PendingReplaced {
                before: self.pending.clone(),
                after: pending.clone(),
            });
        }
        let state = Self::try_restore(self.account.clone(), history, pending, Some(at))?;
        let notices = cancellation_notices(&state, incoming);
        Ok(Reconciliation {
            state,
            changes,
            warning: disappearance_warning(RowCount(missing), RowCount(scoped_active)),
            notices,
        })
    }
    fn replace_pending(
        &self,
        incoming: &CompleteTransactionResponse,
    ) -> Result<TransactionBatch, ReconciliationError> {
        let mut pending = self
            .pending
            .0
            .iter()
            .filter(|row| incoming.filter().coverage(&row.data().kind) == KindCoverage::Excluded)
            .cloned()
            .collect::<Vec<_>>();
        pending.extend(
            incoming
                .rows()
                .as_slice()
                .iter()
                .filter(|row| row.data().settlement == Settlement::Pending)
                .cloned(),
        );
        pending.sort_unstable();
        TransactionBatch::try_new(pending)
    }
}
fn settled_groups(
    incoming: &CompleteTransactionResponse,
) -> BTreeMap<Transaction, Vec<Transaction>> {
    let mut grouped = BTreeMap::<Transaction, Vec<Transaction>>::new();
    for row in incoming
        .rows()
        .as_slice()
        .iter()
        .filter(|row| row.data().settlement == Settlement::Settled)
    {
        grouped.entry(row.clone()).or_default().push(row.clone());
    }
    grouped
}
fn disappearance_warning(missing: RowCount, scoped_active: RowCount) -> DisappearanceWarning {
    if missing.0 >= 20 || (missing.0 >= 5 && missing.0.saturating_mul(2) >= scoped_active.0) {
        DisappearanceWarning::MassDisappearance {
            missing,
            scoped_active,
        }
    } else {
        DisappearanceWarning::None
    }
}
fn cancellation_notices(
    state: &SettledRowSet,
    incoming: &CompleteTransactionResponse,
) -> Vec<ReconciliationNotice> {
    let mut candidates = BTreeMap::<CancellationIdentity, RowCount>::new();
    let cancellations = incoming
        .rows()
        .as_slice()
        .iter()
        .filter(|row| row.data().cancellation == Cancellation::Cancelled)
        .collect::<Vec<_>>();
    if cancellations.is_empty() {
        return Vec::new();
    }
    for cancellation in &cancellations {
        let _previous = candidates.insert(CancellationIdentity::of(cancellation), RowCount(0));
    }
    for original in state.active_settled_for_calculation().filter(|row| {
        incoming.window().coverage(row.data().settlement_date) == WindowCoverage::Outside
    }) {
        if let Some(count) = candidates.get_mut(&CancellationIdentity::of(original)) {
            count.0 = count.0.saturating_add(1);
        }
    }
    cancellations
        .into_iter()
        .filter_map(|cancellation| {
            candidates
                .get(&CancellationIdentity::of(cancellation))
                .filter(|count| count.0 > 0)
                .map(|count| ReconciliationNotice::UnconfirmedCancellation {
                    cancellation: cancellation.clone(),
                    possible_originals: *count,
                })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use crate::{
        AccountId, Amount, AmountInput, CalendarDate, CalendarYear, Cancellation,
        CompleteTransactionResponse, CompletionAttestation, Currency, DayNumber,
        DisappearanceWarning, Market, Money, MonthNumber, Nanosecond, OffsetSeconds,
        ReconciliationError, ReconciliationNotice, SettledRowSet, Settlement, SettlementWindow,
        Timestamp, Transaction, TransactionBatch, TransactionCode, TransactionData,
        TransactionFilter, TransactionKind, UnixSeconds,
    };
    use proptest::{collection, prelude::ProptestConfig, prop_assert, prop_assert_eq, proptest};

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn time(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn row(day: u8, code: &str, settlement: Settlement) -> Transaction {
        let zero = Money::new(
            Amount::parse(&AmountInput::new("0")).expect("금액"),
            Currency::Krw,
        );
        Transaction::try_new(TransactionData {
            kind: TransactionKind::new(TransactionCode::try_new(code).expect("코드")),
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: Some(zero),
            commission: zero,
            tax: zero,
            net_amount: Some(zero),
            exchange_rate: None,
            settlement_date: date(day),
            settlement,
            transaction_time: Some(time(1)),
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    fn response(
        rows: Vec<Transaction>,
        from: u8,
        to: u8,
        filter: TransactionFilter,
    ) -> CompleteTransactionResponse {
        CompleteTransactionResponse::after_last_page_without_quarantine(
            AccountId::new(1),
            SettlementWindow::try_new(date(from), date(to)).expect("창"),
            filter,
            TransactionBatch::try_new(rows).expect("목록"),
            CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .expect("완전 응답")
    }
    fn apply(state: &SettledRowSet, rows: Vec<Transaction>, at: i64) -> super::Reconciliation {
        state
            .reconcile(&response(rows, 1, 28, TransactionFilter::all()), time(at))
            .expect("대조")
    }
    #[test]
    fn complete_response_requires_attestation_account_scope_and_selected_kinds() {
        let constructor: fn(
            AccountId,
            SettlementWindow,
            TransactionFilter,
            TransactionBatch,
            CompletionAttestation,
        ) -> Result<CompleteTransactionResponse, ReconciliationError> =
            CompleteTransactionResponse::after_last_page_without_quarantine;
        let window = SettlementWindow::try_new(date(1), date(14)).expect("창");
        let complete = constructor(
            AccountId::new(2),
            window,
            TransactionFilter::all(),
            TransactionBatch::try_new(vec![]).expect("목록"),
            CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .expect("완전 응답");
        assert_eq!(
            SettledRowSet::new(AccountId::new(1)).reconcile(&complete, time(10)),
            Err(ReconciliationError::AccountMismatch)
        );
        assert_eq!(complete.account_id(), &AccountId::new(2));
        assert_eq!(complete.window(), window);
        assert_eq!(complete.rows().as_slice(), []);
        assert_eq!(complete.filter().selected_kinds(), None);
        assert_eq!(window.from(), date(1));
        assert_eq!(window.to(), date(14));
    }
    #[test]
    fn partial_buy_sell_response_preserves_deposits_and_dividends() {
        let deposit = row(1, "DEPOSIT", Settlement::Settled);
        let dividend = row(2, "DIVIDEND", Settlement::Settled);
        let buy = row(3, "BUY", Settlement::Settled);
        let first = apply(
            &SettledRowSet::new(AccountId::new(1)),
            vec![deposit, dividend, buy],
            10,
        );
        let filter = TransactionFilter::only(vec![TransactionKind::Buy, TransactionKind::Sell])
            .expect("필터");
        let result = first
            .state()
            .reconcile(&response(vec![], 1, 14, filter), time(20))
            .expect("부분 조회");
        assert_eq!(result.state().active_settled_for_calculation().count(), 2);
        assert_eq!(result.state().history_rows().len(), 3);
    }
    #[test]
    fn fifty_missing_rows_raise_mass_disappearance_warning_and_remain_in_history() {
        let value = row(1, "BUY", Settlement::Settled);
        let first = apply(&SettledRowSet::new(AccountId::new(1)), vec![value; 50], 10);
        let result = first
            .state()
            .reconcile(&response(vec![], 1, 14, TransactionFilter::all()), time(20))
            .expect("대조");
        assert_eq!(result.state().active_settled_for_calculation().count(), 0);
        assert_eq!(result.state().history_rows().len(), 50);
        assert_eq!(
            result.disappearance_warning(),
            DisappearanceWarning::MassDisappearance {
                missing: super::RowCount(50),
                scoped_active: super::RowCount(50)
            }
        );
    }
    #[test]
    fn cancellation_outside_window_has_no_effect_and_reports_unconfirmed_original() {
        let original = row(1, "BUY", Settlement::Settled);
        let first = apply(
            &SettledRowSet::new(AccountId::new(1)),
            vec![original.clone()],
            10,
        );
        let mut cancellation = original.data().clone();
        cancellation.settlement_date = date(20);
        cancellation.transaction_time = Some(time(20));
        cancellation.cancellation = Cancellation::Cancelled;
        let cancelled = Transaction::try_new(cancellation).expect("취소");
        assert_eq!(
            cancelled.calculation_effect(),
            crate::CalculationEffect::NoEffect
        );
        let result = first
            .state()
            .reconcile(
                &response(vec![cancelled.clone()], 7, 20, TransactionFilter::all()),
                time(30),
            )
            .expect("창 밖 취소");
        assert_eq!(result.state().active_settled_for_calculation().count(), 1);
        assert_eq!(result.state().active_settled_rows().count(), 2);
        assert_eq!(result.notices().len(), 1);
        match result.notices().first().expect("경고") {
            ReconciliationNotice::UnconfirmedCancellation {
                cancellation: observed_cancellation,
                possible_originals,
            } => {
                assert_eq!(observed_cancellation, &cancelled);
                assert_eq!(*possible_originals.as_usize(), 1);
            }
        }
        let full = apply(result.state(), vec![cancelled], 40);
        assert_eq!(full.state().active_settled_for_calculation().count(), 0);
        assert_eq!(full.state().history_rows().len(), 2);
    }
    #[test]
    fn pending_is_replaced_globally_even_when_expected_date_leaves_window() {
        let pending = row(2, "BUY", Settlement::Pending);
        let first = apply(&SettledRowSet::new(AccountId::new(1)), vec![pending], 10);
        let settled = row(16, "BUY", Settlement::Settled);
        let result = first
            .state()
            .reconcile(
                &response(vec![settled], 3, 16, TransactionFilter::all()),
                time(20),
            )
            .expect("결제");
        assert_eq!(result.state().pending_rows().as_slice(), []);
        assert_eq!(result.state().active_settled_for_calculation().count(), 1);
        let future = response(
            vec![row(28, "BUY", Settlement::Pending)],
            3,
            16,
            TransactionFilter::all(),
        );
        assert_eq!(future.rows().as_slice().len(), 1);
    }
    #[test]
    fn synchronization_timestamp_rejects_1000_3000_2000_even_with_no_rows() {
        let first = apply(&SettledRowSet::new(AccountId::new(1)), vec![], 1000);
        let next = apply(first.state(), vec![], 3000);
        assert_eq!(next.state().last_synchronized_at(), Some(time(3000)));
        assert_eq!(
            next.state().reconcile(
                &response(vec![], 1, 28, TransactionFilter::all()),
                time(2000)
            ),
            Err(ReconciliationError::InvalidHistory)
        );
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn mixed_windows_filters_pending_and_missing_history_match_independent_counts(
            old in collection::vec((1_u8..29, 0_u8..12, proptest::bool::ANY), 0..24),
            new in collection::vec((1_u8..29, 0_u8..12, proptest::bool::ANY), 0..24),
            bounds in (1_u8..29, 1_u8..29), selected in proptest::bool::ANY, selection in collection::btree_set(0_u8..12,1..=12)
        ) {
            let codes = ["BUY", "SELL", "SECURITIES_TRANSFER_IN", "SECURITIES_TRANSFER_OUT", "DEPOSIT", "WITHDRAWAL", "EXCHANGE_IN", "EXCHANGE_OUT", "DIVIDEND", "INTEREST", "FEE", "FUTURE"];
            let make = |values: &[(u8,u8,bool)]| values.iter().map(|(day, kind, pending)| row(*day, codes.get(usize::from(*kind)).expect("종류"), if *pending {Settlement::Pending} else {Settlement::Settled})).collect::<Vec<_>>();
            let initial = apply(&SettledRowSet::new(AccountId::new(1)), make(&old), 10);
            let tombstones = apply(initial.state(), vec![], 20);
            let restored = apply(tombstones.state(), make(&old), 30);
            let from = bounds.0.min(bounds.1); let to = bounds.0.max(bounds.1);
            let included = |kind: u8| !selected || selection.contains(&kind);
            let received = new.iter().copied().filter(|(day,kind,pending)| included(*kind) && (*pending || (*day >= from && *day <= to))).collect::<Vec<_>>();
            let filter = if selected {TransactionFilter::only(selection.iter().map(|kind| TransactionKind::new(TransactionCode::try_new(codes.get(usize::from(*kind)).expect("종류")).expect("코드"))).collect()).expect("필터")} else {TransactionFilter::all()};
            let result = restored.state().reconcile(&response(make(&received), from, to, filter.clone()), time(40)).expect("대조");
            let mut expected = old.iter().copied().filter(|(day,kind,pending)| !*pending && (!included(*kind) || *day < from || *day > to)).collect::<Vec<_>>();
            expected.extend(received.iter().copied().filter(|(_,_,pending)| !*pending)); expected.sort_unstable();
            let mut actual = result.state().active_settled_for_calculation().map(|transaction| (*transaction.data().settlement_date.day().as_u8(), u8::try_from(codes.iter().position(|code| *code == transaction.data().kind.as_code().as_str()).expect("종류")).expect("종류 범위"), false)).collect::<Vec<_>>();
            actual.sort_unstable(); prop_assert_eq!(actual, expected);
            let mut expected_pending = old.iter().copied().filter(|(_,kind,pending)| *pending && !included(*kind)).collect::<Vec<_>>();
            expected_pending.extend(received.iter().copied().filter(|(_,_,pending)| *pending)); expected_pending.sort_unstable();
            let mut actual_pending = result.state().pending_rows().as_slice().iter().map(|transaction| (*transaction.data().settlement_date.day().as_u8(), u8::try_from(codes.iter().position(|code| *code == transaction.data().kind.as_code().as_str()).expect("종류")).expect("종류 범위"), true)).collect::<Vec<_>>();
            actual_pending.sort_unstable(); prop_assert_eq!(actual_pending,expected_pending);
            prop_assert!(result.state().history_rows().len() >= restored.state().history_rows().len());
            let repeat = result.state().reconcile(&response(make(&received), from, to, filter.clone()), time(40)).expect("반복");
            prop_assert_eq!(repeat.state(), result.state());
            let mut reversed = received; reversed.reverse();
            let other = restored.state().reconcile(&response(make(&reversed), from, to, filter), time(40)).expect("역순");
            prop_assert_eq!(other.state(), result.state());
        }
    }
    #[test]
    fn restoration_requires_valid_observation_history_and_round_trips_all_state() {
        let value = row(1, "BUY", Settlement::Settled);
        assert_eq!(
            crate::StoredTransaction::try_restore(
                row(1, "BUY", Settlement::Pending),
                time(10),
                None
            ),
            Err(ReconciliationError::PendingInHistory)
        );
        assert_eq!(
            crate::StoredTransaction::try_restore(value.clone(), time(10), Some(time(9))),
            Err(ReconciliationError::InvalidHistory)
        );
        let stored = crate::StoredTransaction::try_restore(value.clone(), time(10), Some(time(20)))
            .expect("보존 행");
        assert_eq!(stored.first_seen(), time(10));
        assert_eq!(stored.disappeared_at(), Some(time(20)));
        assert_eq!(stored.transaction(), &value);
        let restore = |history, pending, at| {
            SettledRowSet::try_restore(
                AccountId::new(1),
                history,
                TransactionBatch::try_new(pending).expect("임시 목록"),
                at,
            )
        };
        assert_eq!(
            restore(vec![], vec![value], Some(time(30))),
            Err(ReconciliationError::InvalidHistory)
        );
        assert_eq!(
            restore(vec![stored.clone()], vec![], None),
            Err(ReconciliationError::InvalidHistory)
        );
        assert_eq!(
            restore(vec![], vec![row(1, "BUY", Settlement::Pending)], None),
            Err(ReconciliationError::InvalidHistory)
        );
        assert_eq!(
            restore(vec![stored.clone()], vec![], Some(time(9))),
            Err(ReconciliationError::InvalidHistory)
        );
        assert_eq!(
            restore(vec![stored.clone()], vec![], Some(time(19))),
            Err(ReconciliationError::InvalidHistory)
        );
        let restored = restore(vec![stored], vec![], Some(time(20))).expect("복원");
        assert_eq!(restored.account_id(), &AccountId::new(1));
        assert_eq!(
            restore(vec![], vec![], None).expect("비어 있는 초기 상태"),
            SettledRowSet::new(AccountId::new(1))
        );
        let repeat = apply(&restored, vec![], 20);
        assert_eq!(repeat.state(), &restored);
        assert_eq!(repeat.changes(), []);
    }
    #[test]
    fn scoped_response_rejects_wrong_kinds_and_only_bounds_settled_dates() {
        assert_eq!(
            TransactionFilter::only(vec![]),
            Err(ReconciliationError::InvalidFilter)
        );
        assert_eq!(
            TransactionFilter::only(vec![TransactionKind::Buy; 65]),
            Err(ReconciliationError::InvalidFilter)
        );
        let filter = TransactionFilter::only(vec![TransactionKind::Buy; 64]).expect("필터 상한");
        assert_eq!(
            filter.selected_kinds(),
            Some([TransactionKind::Buy].as_slice())
        );
        assert_eq!(
            SettlementWindow::try_new(date(14), date(1)),
            Err(ReconciliationError::InvalidWindow)
        );
        let window = SettlementWindow::try_new(date(7), date(14)).expect("창");
        for value in [
            row(7, "SELL", Settlement::Settled),
            row(20, "SELL", Settlement::Pending),
        ] {
            assert_eq!(
                CompleteTransactionResponse::after_last_page_without_quarantine(
                    AccountId::new(1),
                    window,
                    filter.clone(),
                    TransactionBatch::try_new(vec![value]).expect("목록"),
                    CompletionAttestation::AllPagesAndAllMatchingPendingReceived
                ),
                Err(ReconciliationError::OutsideFilter)
            );
        }
        for day in [6, 15] {
            assert_eq!(
                CompleteTransactionResponse::after_last_page_without_quarantine(
                    AccountId::new(1),
                    window,
                    TransactionFilter::all(),
                    TransactionBatch::try_new(vec![row(day, "BUY", Settlement::Settled)])
                        .expect("목록"),
                    CompletionAttestation::AllPagesAndAllMatchingPendingReceived
                ),
                Err(ReconciliationError::OutsideWindow)
            );
        }
        let _boundaries = response(
            vec![
                row(7, "BUY", Settlement::Settled),
                row(14, "BUY", Settlement::Settled),
            ],
            7,
            14,
            filter,
        );
    }
    #[test]
    fn response_and_global_pending_capacity_fail_atomically() {
        let value = row(1, "BUY", Settlement::Pending);
        assert_eq!(
            TransactionBatch::try_new(vec![value.clone(); 4097]),
            Err(ReconciliationError::TooManyRows)
        );
        let saved = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![],
            TransactionBatch::try_new(vec![value; 4096]).expect("상한"),
            Some(time(10)),
        )
        .expect("복원");
        let selected = TransactionFilter::only(vec![TransactionKind::Sell]).expect("필터");
        let incoming = response(vec![row(1, "SELL", Settlement::Pending)], 1, 14, selected);
        assert_eq!(
            saved.reconcile(&incoming, time(20)),
            Err(ReconciliationError::TooManyRows)
        );
        assert_eq!(saved.pending_rows().as_slice().len(), 4096);
        assert_eq!(saved.last_synchronized_at(), Some(time(10)));
    }
    #[test]
    fn million_row_history_bound_is_separate_from_response_bound() {
        let value = crate::StoredTransaction::try_restore(
            row(1, "BUY", Settlement::Settled),
            time(1),
            Some(time(2)),
        )
        .expect("이력");
        assert_eq!(
            SettledRowSet::try_restore(
                AccountId::new(1),
                vec![value.clone(); 1_000_001],
                TransactionBatch::try_new(vec![]).expect("임시"),
                Some(time(10))
            )
            .as_ref()
            .map(SettledRowSet::history_rows)
            .map(<[crate::StoredTransaction]>::len),
            Err(&ReconciliationError::TooMuchHistory)
        );
        let saved = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![value; 1_000_000],
            TransactionBatch::try_new(vec![]).expect("임시"),
            Some(time(10)),
        )
        .expect("이력 상한");
        assert_eq!(
            saved
                .reconcile(
                    &response(
                        vec![row(1, "BUY", Settlement::Settled)],
                        1,
                        14,
                        TransactionFilter::all()
                    ),
                    time(20)
                )
                .as_ref()
                .map(super::Reconciliation::state)
                .map(SettledRowSet::history_rows)
                .map(<[crate::StoredTransaction]>::len),
            Err(&ReconciliationError::TooMuchHistory)
        );
        assert_eq!(saved.history_rows().len(), 1_000_000);
    }
    #[test]
    fn warning_thresholds_include_count_and_ratio_boundaries() {
        use super::{RowCount, disappearance_warning};
        for (missing, total) in [(0, 0), (4, 4), (5, 11), (19, 1000)] {
            assert_eq!(
                disappearance_warning(RowCount(missing), RowCount(total)),
                DisappearanceWarning::None
            );
        }
        for (missing, total) in [(5, 10), (20, 1000)] {
            assert_eq!(
                disappearance_warning(RowCount(missing), RowCount(total)),
                DisappearanceWarning::MassDisappearance {
                    missing: RowCount(missing),
                    scoped_active: RowCount(total)
                }
            );
        }
    }
    #[test]
    fn duplicates_reappear_as_new_observations_and_calculation_never_includes_tombstones() {
        let value = row(1, "BUY", Settlement::Settled);
        let first = apply(
            &SettledRowSet::new(AccountId::new(1)),
            vec![value.clone(); 3],
            10,
        );
        let missing = apply(first.state(), vec![value.clone(); 2], 20);
        let returned = apply(missing.state(), vec![value; 3], 30);
        assert_eq!(returned.state().history_rows().len(), 4);
        assert_eq!(returned.state().active_settled_for_calculation().count(), 3);
        assert_eq!(returned.changes().len(), 3);
        for old_count in 0..=16 {
            for new_count in 0..=16 {
                let duplicate = row(1, "BUY", Settlement::Settled);
                let original = apply(
                    &SettledRowSet::new(AccountId::new(1)),
                    vec![duplicate.clone(); old_count],
                    10,
                );
                let result = apply(original.state(), vec![duplicate; new_count], 20);
                assert_eq!(
                    result.state().history_rows().len(),
                    old_count.max(new_count)
                );
                assert_eq!(
                    result.state().active_settled_for_calculation().count(),
                    new_count
                );
            }
        }
    }
    #[test]
    fn cancellation_with_different_amount_never_claims_an_original_candidate() {
        let original = row(1, "BUY", Settlement::Settled);
        let mut different = original.data().clone();
        different.amount = Some(Money::new(
            Amount::parse(&AmountInput::new("1")).expect("금액"),
            Currency::Krw,
        ));
        let first = apply(
            &SettledRowSet::new(AccountId::new(1)),
            vec![Transaction::try_new(different).expect("다른 원거래")],
            10,
        );
        let mut cancelled = original.data().clone();
        cancelled.cancellation = Cancellation::Cancelled;
        cancelled.settlement_date = date(20);
        let result = first
            .state()
            .reconcile(
                &response(
                    vec![Transaction::try_new(cancelled).expect("취소")],
                    7,
                    20,
                    TransactionFilter::all(),
                ),
                time(30),
            )
            .expect("대조");
        assert_eq!(result.notices(), []);
        assert_eq!(result.state().active_settled_for_calculation().count(), 1);
    }
    #[test]
    fn simultaneous_first_observation_and_disappearance_are_valid_on_restore() {
        let stored = crate::StoredTransaction::try_restore(
            row(1, "BUY", Settlement::Settled),
            time(10),
            Some(time(10)),
        )
        .expect("동시각 관측과 사라짐");
        assert_eq!(stored.first_seen(), time(10));
        assert_eq!(stored.disappeared_at(), Some(time(10)));
        let restored = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![stored],
            TransactionBatch::try_new(vec![]).expect("임시"),
            Some(time(10)),
        )
        .expect("동시각 복원");
        assert_eq!(restored.active_settled_for_calculation().count(), 0);
        assert_eq!(restored.history_rows().len(), 1);
    }
}
