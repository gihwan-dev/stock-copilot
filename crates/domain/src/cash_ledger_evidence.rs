use crate::settled_row_set::WindowCoverage;
use crate::transaction_order::CancellationIdentity;
use crate::{
    CalendarDate, Cancellation, CashUnavailable, CompleteTransactionResponse, Currency,
    SettledRowSet, Settlement, SettlementWindow, Transaction,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvidence(CompleteTransactionResponse);
impl LedgerEvidence {
    pub fn from_complete_response(
        response: CompleteTransactionResponse,
    ) -> Result<Self, CashUnavailable> {
        if response.filter().selected_kinds().is_some() {
            return Err(CashUnavailable::IncompleteResponse);
        }
        Ok(Self(response))
    }
    pub(crate) fn validate(
        &self,
        rows: &SettledRowSet,
        window: SettlementWindow,
    ) -> Result<(), CashUnavailable> {
        if rows.account_id() != self.0.account_id()
            || rows.last_synchronized_at().is_none()
            || self.0.window().from() > window.from()
            || self.0.window().to() < window.to()
        {
            return Err(CashUnavailable::IncompleteResponse);
        }
        let mut stored = rows
            .active_settled_rows()
            .map(crate::StoredTransaction::transaction)
            .filter(|row| window.coverage(row.data().settlement_date) == WindowCoverage::Inside)
            .collect::<Vec<_>>();
        let mut received = self
            .0
            .rows()
            .as_slice()
            .iter()
            .filter(|row| {
                row.data().settlement == Settlement::Settled
                    && window.coverage(row.data().settlement_date) == WindowCoverage::Inside
            })
            .collect::<Vec<_>>();
        stored.sort_unstable();
        received.sort_unstable();
        if stored != received {
            return Err(CashUnavailable::IncompleteResponse);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CancellationRange {
    pub(crate) from: CalendarDate,
    pub(crate) to: CalendarDate,
}
#[derive(Default)]
struct CancellationGroup {
    gone: Vec<CalendarDate>,
    active: Vec<CalendarDate>,
    cancelled: Vec<CalendarDate>,
}
pub(crate) fn cancellation_ranges(
    rows: &SettledRowSet,
    currency: Currency,
) -> Vec<CancellationRange> {
    let mut groups = BTreeMap::<CancellationIdentity, CancellationGroup>::new();
    for stored in rows.history_rows() {
        let row = stored.transaction();
        let data = row.data();
        if data.currency != currency {
            continue;
        }
        let group = groups.entry(CancellationIdentity::of(row)).or_default();
        match (data.cancellation, stored.disappeared_at()) {
            (Cancellation::Regular, Some(_)) => group.gone.push(data.settlement_date),
            (Cancellation::Regular, None) => group.active.push(data.settlement_date),
            (Cancellation::Cancelled, None) => group.cancelled.push(data.settlement_date),
            (Cancellation::Cancelled, Some(_)) => (),
        }
    }
    let mut ranges = Vec::new();
    for mut group in groups.into_values() {
        group.cancelled.sort_unstable();
        for date in group.cancelled {
            if group.gone.pop().is_some() {
                continue;
            }
            ranges.push(CancellationRange {
                from: group
                    .active
                    .iter()
                    .copied()
                    .min()
                    .map_or(date, |first| first.min(date)),
                to: group
                    .active
                    .iter()
                    .copied()
                    .max()
                    .map_or(date, |last| last.max(date)),
            });
        }
    }
    ranges.sort_by_key(|range| (range.from, range.to));
    ranges
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LedgerFlowFacts {
    pub(crate) window: SettlementWindow,
    pub(crate) currency: Currency,
    pub(crate) rows: Vec<Transaction>,
    pub(crate) cancellations: Vec<CancellationRange>,
}

#[cfg(test)]
mod tests {
    use super::{CancellationRange, LedgerEvidence, cancellation_ranges};
    use crate::cash_ledger::tests::{at, date, input, row, set, window};
    use crate::{
        AccountId, CompletionAttestation, StoredTransaction, Transaction, TransactionBatch,
        TransactionFilter, TransactionKind,
    };
    use crate::{
        Cancellation, CashUnavailable, CompleteTransactionResponse, Currency, SettledRowSet,
        SettlementWindow,
    };
    #[test]
    fn partial_wrong_account_and_narrow_responses_cannot_assert_completeness() {
        let empty = TransactionBatch::try_new(vec![]).expect("목록");
        let partial = CompleteTransactionResponse::after_last_page_without_quarantine(
            AccountId::new(1),
            window(),
            TransactionFilter::only(vec![TransactionKind::Deposit]).expect("필터"),
            empty.clone(),
            CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .expect("응답");
        assert_eq!(
            LedgerEvidence::from_complete_response(partial),
            Err(CashUnavailable::IncompleteResponse)
        );
        let rows = set(vec![]);
        for (account, period) in [
            (AccountId::new(2), window()),
            (
                AccountId::new(1),
                SettlementWindow::try_new(date(1), date(2)).expect("기간"),
            ),
        ] {
            let response = CompleteTransactionResponse::after_last_page_without_quarantine(
                account,
                period,
                TransactionFilter::all(),
                empty.clone(),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("응답");
            let mut request = input(&rows, Currency::Krw, "100");
            request.evidence =
                Some(LedgerEvidence::from_complete_response(response).expect("증거"));
            assert_eq!(
                crate::CashLedger::reconstruct(&rows, request).state(),
                &crate::CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
            );
        }
    }
    #[test]
    fn cancellation_multiset_resolves_only_disappeared_originals_and_ignores_other_currency() {
        let original = row("DEPOSIT", "10", Currency::Krw);
        let mut cancelled = original.data().clone();
        cancelled.cancellation = Cancellation::Cancelled;
        cancelled.settlement_date = date(5);
        let cancellation = Transaction::try_new(cancelled).expect("취소");
        let lone = set(vec![cancellation.clone()]);
        assert_eq!(
            cancellation_ranges(&lone, Currency::Krw),
            vec![CancellationRange {
                from: date(5),
                to: date(5)
            }]
        );
        assert_eq!(cancellation_ranges(&lone, Currency::Usd), vec![]);
        let disappeared_cancellation =
            StoredTransaction::try_restore(cancellation.clone(), at(900_000), Some(at(900_000)))
                .expect("사라진 취소");
        let restored = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(original.clone(), at(900_000), Some(at(900_000)))
                    .expect("원거래"),
                StoredTransaction::try_restore(cancellation.clone(), at(900_000), None)
                    .expect("취소"),
                disappeared_cancellation,
            ],
            TransactionBatch::try_new(vec![]).expect("목록"),
            Some(at(900_000)),
        )
        .expect("복원");
        assert_eq!(cancellation_ranges(&restored, Currency::Krw), vec![]);
        let double = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(original, at(900_000), Some(at(900_000)))
                    .expect("원거래"),
                StoredTransaction::try_restore(cancellation.clone(), at(900_000), None)
                    .expect("취소"),
                StoredTransaction::try_restore(cancellation, at(900_000), None).expect("취소"),
            ],
            TransactionBatch::try_new(vec![]).expect("목록"),
            Some(at(900_000)),
        )
        .expect("복원");
        assert_eq!(
            cancellation_ranges(&double, Currency::Krw),
            vec![CancellationRange {
                from: date(5),
                to: date(5)
            }]
        );
    }
}
