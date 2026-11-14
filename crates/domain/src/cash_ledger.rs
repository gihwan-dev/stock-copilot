use crate::{
    AssetClass, BuyingPower, Currency, Money, MoneyError, SettledRowSet, SettlementWindow,
    TransactionData, TransactionKind,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerEvidence {
    Complete,
    Incomplete,
    UnknownCurrency,
    UnconfirmedCancellation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashUnavailable {
    UnknownOpening,
    IncompleteResponse,
    UnknownCurrency,
    UnconfirmedCancellation,
    UnclassifiedRow,
    MissingNetAmount,
    CurrencyMismatch,
    MissingReportedCash,
    InvalidReportedCash,
    ReportedDifference,
    Arithmetic(MoneyError),
}
impl LedgerEvidence {
    pub(crate) fn validate(self, rows: &SettledRowSet) -> Result<(), CashUnavailable> {
        match self {
            Self::Incomplete => Err(CashUnavailable::IncompleteResponse),
            Self::UnknownCurrency => Err(CashUnavailable::UnknownCurrency),
            Self::UnconfirmedCancellation => Err(CashUnavailable::UnconfirmedCancellation),
            Self::Complete => rows
                .last_synchronized_at()
                .map(|_| ())
                .ok_or(CashUnavailable::IncompleteResponse),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonCash(Money);
impl ComparisonCash {
    pub fn compare_to(self, reference: Money) -> Result<Ordering, MoneyError> {
        if self.0.currency() != reference.currency() {
            return Err(MoneyError::CurrencyMismatch);
        }
        Ok(self
            .0
            .amount()
            .as_decimal()
            .cmp(&reference.amount().as_decimal()))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashDifference(Money);
impl CashDifference {
    pub fn compare_to(self, reference: Money) -> Result<Ordering, MoneyError> {
        ComparisonCash(self.0).compare_to(reference)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashLedgerState {
    Reconciled(ComparisonCash),
    Unavailable(CashUnavailable),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashComparison {
    Matches,
    Different(CashDifference),
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashLedger {
    currency: Currency,
    state: CashLedgerState,
    comparison: CashComparison,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashLedgerInput {
    pub window: SettlementWindow,
    pub currency: Currency,
    pub opening: Option<Money>,
    pub evidence: LedgerEvidence,
    pub reported: Option<BuyingPower>,
}
impl CashLedger {
    #[must_use]
    pub fn reconstruct(rows: &SettledRowSet, input: CashLedgerInput) -> Self {
        let currency = input.currency;
        match reconstruct(rows, input) {
            Ok((value, delta)) => {
                if delta.amount().as_decimal().is_zero() {
                    Self {
                        currency,
                        state: CashLedgerState::Reconciled(ComparisonCash(value)),
                        comparison: CashComparison::Matches,
                    }
                } else {
                    Self {
                        currency,
                        state: CashLedgerState::Unavailable(CashUnavailable::ReportedDifference),
                        comparison: CashComparison::Different(CashDifference(delta)),
                    }
                }
            }
            Err(reason) => Self {
                currency,
                state: CashLedgerState::Unavailable(reason),
                comparison: CashComparison::Unavailable,
            },
        }
    }
    #[must_use]
    pub const fn state(&self) -> &CashLedgerState {
        &self.state
    }
    #[must_use]
    pub const fn comparison(&self) -> &CashComparison {
        &self.comparison
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}
fn reconstruct(
    rows: &SettledRowSet,
    input: CashLedgerInput,
) -> Result<(Money, Money), CashUnavailable> {
    let CashLedgerInput {
        window,
        currency,
        opening,
        evidence,
        reported,
    } = input;
    evidence.validate(rows)?;
    let mut total = opening.ok_or(CashUnavailable::UnknownOpening)?;
    if total.currency() != currency {
        return Err(CashUnavailable::CurrencyMismatch);
    }
    let mut selected = rows
        .active_settled_for_calculation()
        .filter(|row| {
            row.data().currency == currency
                && window.coverage(row.data().settlement_date)
                    == crate::settled_row_set::WindowCoverage::Inside
        })
        .collect::<Vec<_>>();
    selected.sort_unstable();
    for row in selected {
        let data = row.data();
        let direction = cash_direction(data)?;
        let value = data.net_amount.ok_or(CashUnavailable::MissingNetAmount)?;
        total = match direction {
            crate::FlowDirection::Inflow => total.checked_add(&value),
            crate::FlowDirection::Outflow => total.checked_sub(&value),
        }
        .map_err(CashUnavailable::Arithmetic)?;
    }
    let observed = reported.ok_or(CashUnavailable::MissingReportedCash)?;
    if observed.currency() != currency {
        return Err(CashUnavailable::InvalidReportedCash);
    }
    let reference = observed
        .money()
        .map_err(|_error| CashUnavailable::InvalidReportedCash)?;
    let delta = total
        .checked_sub(&reference)
        .map_err(CashUnavailable::Arithmetic)?;
    Ok((total, delta))
}
pub(crate) fn cash_direction(
    data: &TransactionData,
) -> Result<crate::FlowDirection, CashUnavailable> {
    if let Some(AssetClass::Unknown(_)) = data.asset_class {
        return Err(CashUnavailable::UnclassifiedRow);
    }
    match data.kind {
        TransactionKind::Buy
        | TransactionKind::Withdrawal
        | TransactionKind::ExchangeOut
        | TransactionKind::Fee => Ok(crate::FlowDirection::Outflow),
        TransactionKind::Sell
        | TransactionKind::Deposit
        | TransactionKind::ExchangeIn
        | TransactionKind::Dividend
        | TransactionKind::Interest => Ok(crate::FlowDirection::Inflow),
        TransactionKind::SecuritiesTransferIn
        | TransactionKind::SecuritiesTransferOut
        | TransactionKind::Unknown(_) => Err(CashUnavailable::UnclassifiedRow),
    }
}
