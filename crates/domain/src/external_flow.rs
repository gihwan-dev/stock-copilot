use crate::cash_ledger::cash_direction;
use crate::{
    CalendarDate, CashUnavailable, Currency, LedgerEvidence, Money, SettledRowSet,
    SettlementWindow, TransactionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    Inflow,
    Outflow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalFlow {
    date: CalendarDate,
    direction: FlowDirection,
    amount: Money,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalFlows {
    Available(Vec<ExternalFlow>),
    Unavailable(CashUnavailable),
}
impl ExternalFlow {
    #[must_use]
    pub fn from_settled_rows(
        rows: &SettledRowSet,
        window: SettlementWindow,
        evidence: LedgerEvidence,
    ) -> ExternalFlows {
        match collect(rows, window, evidence) {
            Ok(flows) => ExternalFlows::Available(flows),
            Err(reason) => ExternalFlows::Unavailable(reason),
        }
    }
    #[must_use]
    pub const fn date(self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub fn currency(self) -> Currency {
        self.amount.currency()
    }
    #[must_use]
    pub const fn direction(self) -> FlowDirection {
        self.direction
    }
    #[must_use]
    pub const fn amount(self) -> Money {
        self.amount
    }
}
fn collect(
    rows: &SettledRowSet,
    window: SettlementWindow,
    evidence: LedgerEvidence,
) -> Result<Vec<ExternalFlow>, CashUnavailable> {
    evidence.validate(rows)?;
    let mut selected = rows
        .active_settled_for_calculation()
        .filter(|row| {
            window.coverage(row.data().settlement_date)
                == crate::settled_row_set::WindowCoverage::Inside
        })
        .collect::<Vec<_>>();
    selected.sort_unstable();
    let mut flows = Vec::new();
    for row in selected {
        let data = row.data();
        let direction = cash_direction(data)?;
        match data.kind {
            TransactionKind::Deposit | TransactionKind::Withdrawal => {
                if data.asset_class.is_none() {
                    flows.push(ExternalFlow {
                        date: data.settlement_date,
                        direction,
                        amount: data.net_amount.ok_or(CashUnavailable::MissingNetAmount)?,
                    });
                }
            }
            TransactionKind::Buy
            | TransactionKind::Sell
            | TransactionKind::ExchangeIn
            | TransactionKind::ExchangeOut
            | TransactionKind::Dividend
            | TransactionKind::Interest
            | TransactionKind::Fee
            | TransactionKind::SecuritiesTransferIn
            | TransactionKind::SecuritiesTransferOut
            | TransactionKind::Unknown(_) => (),
        }
    }
    Ok(flows)
}
