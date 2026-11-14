use crate::{
    BuyingPower, CashLedger, ConfirmedFlowPeriod, Currency, Money, SnapshotError, Timestamp,
    Transaction, TransactionKind,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCashBasis {
    OrderBuyingPower,
    SettledLedger,
    Unavailable,
}
#[derive(Debug, Clone, Copy)]
pub struct SnapshotLedgers<'action> {
    kr: (&'action CashLedger, &'action ConfirmedFlowPeriod),
    us: (&'action CashLedger, &'action ConfirmedFlowPeriod),
    kr_classifications: &'action [crate::TransferClassification],
    us_classifications: &'action [crate::TransferClassification],
}
impl<'action> SnapshotLedgers<'action> {
    #[must_use]
    pub const fn new(
        kr: (&'action CashLedger, &'action ConfirmedFlowPeriod),
        us: (&'action CashLedger, &'action ConfirmedFlowPeriod),
    ) -> Self {
        Self {
            kr,
            us,
            kr_classifications: &[],
            us_classifications: &[],
        }
    }
    #[must_use]
    pub const fn with_transfer_classifications(
        mut self,
        kr: &'action [crate::TransferClassification],
        us: &'action [crate::TransferClassification],
    ) -> Self {
        self.kr_classifications = kr;
        self.us_classifications = us;
        self
    }
    pub(crate) fn load(self, at: Timestamp, rows: &crate::SettledRowSet) -> SnapshotCashData {
        if self.kr.1.window() != self.us.1.window() {
            let mut result = SnapshotCashData::unavailable();
            result.values = SnapshotAmounts::new(
                Err(SnapshotError::OutsideConfirmedPeriod),
                Err(SnapshotError::OutsideConfirmedPeriod),
            );
            return result;
        }
        let kr = currency_cash(self.kr, Currency::Krw, at, rows, self.kr_classifications);
        let us = currency_cash(self.us, Currency::Usd, at, rows, self.us_classifications);
        let basis = if kr.is_ok() && us.is_ok() {
            SnapshotCashBasis::SettledLedger
        } else {
            SnapshotCashBasis::Unavailable
        };
        SnapshotCashData {
            basis,
            values: SnapshotAmounts::new(
                kr.as_ref().map(|entry| entry.0).map_err(|cause| *cause),
                us.as_ref().map(|entry| entry.0).map_err(|cause| *cause),
            ),
            buying_power: None,
            periods: Some((self.kr.1.clone(), self.us.1.clone())),
            exchanges: kr.into_iter().chain(us).flat_map(|entry| entry.1).collect(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotAmounts {
    kr: Result<Money, SnapshotError>,
    us: Result<Money, SnapshotError>,
}
impl SnapshotAmounts {
    pub(crate) const fn new(
        kr: Result<Money, SnapshotError>,
        us: Result<Money, SnapshotError>,
    ) -> Self {
        Self { kr, us }
    }
    pub(crate) const fn get(self, currency: Currency) -> Result<Money, SnapshotError> {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
    }
    pub(crate) fn add(&mut self, currency: Currency, amount: Result<Money, SnapshotError>) {
        let value = self.get(currency).and_then(|entry| {
            entry
                .checked_add(&amount?)
                .map_err(SnapshotError::Arithmetic)
        });
        match currency {
            Currency::Krw => self.kr = value,
            Currency::Usd => self.us = value,
        }
    }
    pub(crate) fn invalidate(&mut self, currency: Currency, error: SnapshotError) {
        match currency {
            Currency::Krw => self.kr = Err(error),
            Currency::Usd => self.us = Err(error),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnapshotCashData {
    pub(crate) basis: SnapshotCashBasis,
    pub(crate) values: SnapshotAmounts,
    pub(crate) buying_power: Option<(BuyingPower, BuyingPower)>,
    pub(crate) periods: Option<(ConfirmedFlowPeriod, ConfirmedFlowPeriod)>,
    pub(crate) exchanges: Vec<Transaction>,
}
impl SnapshotCashData {
    pub(crate) fn unavailable() -> Self {
        Self {
            basis: SnapshotCashBasis::Unavailable,
            values: SnapshotAmounts::new(
                Err(SnapshotError::MissingCash),
                Err(SnapshotError::MissingCash),
            ),
            buying_power: None,
            periods: None,
            exchanges: Vec::new(),
        }
    }
    pub(crate) fn observed(kr: BuyingPower, us: BuyingPower) -> Self {
        Self {
            basis: SnapshotCashBasis::OrderBuyingPower,
            values: SnapshotAmounts::new(
                Err(SnapshotError::WrongCashBasis),
                Err(SnapshotError::WrongCashBasis),
            ),
            buying_power: Some((kr, us)),
            periods: None,
            exchanges: Vec::new(),
        }
    }
}
fn currency_cash(
    input: (&CashLedger, &ConfirmedFlowPeriod),
    currency: Currency,
    at: Timestamp,
    source: &crate::SettledRowSet,
    classifications: &[crate::TransferClassification],
) -> Result<(Money, Vec<Transaction>), SnapshotError> {
    if input.0.currency() != currency {
        return Err(SnapshotError::CalendarMismatch);
    }
    let (ending, rows) = input
        .0
        .snapshot_basis(input.1, classifications)
        .map_err(SnapshotError::Cash)?;
    let day = crate::valuation_snapshot_calendar::kst_date(at)?;
    let window = input.1.window();
    if day < window.from() || day > window.to() {
        return Err(SnapshotError::OutsideConfirmedPeriod);
    }
    let mut received = source
        .active_settled_for_calculation()
        .filter(|row| {
            row.data().currency == currency
                && row.data().settlement_date >= window.from()
                && row.data().settlement_date <= window.to()
        })
        .cloned()
        .collect::<Vec<_>>();
    received.sort_unstable();
    if received.as_slice() != rows {
        return Err(SnapshotError::Cash(
            crate::CashUnavailable::IncompleteResponse,
        ));
    }
    let exchanges = rows
        .iter()
        .filter(|row| {
            row.data().kind == TransactionKind::ExchangeIn
                || row.data().kind == TransactionKind::ExchangeOut
        })
        .cloned()
        .collect();
    balance_at(ending, rows, at).map(|value| (value, exchanges))
}
pub(crate) fn balance_at(
    ending: Money,
    rows: &[Transaction],
    at: Timestamp,
) -> Result<Money, SnapshotError> {
    let day = crate::valuation_snapshot_calendar::kst_date(at)?;
    let mut value = ending;
    for row in rows.iter().rev() {
        let time = row
            .data()
            .transaction_time
            .ok_or(SnapshotError::MissingFlowTime)?;
        if row.data().settlement_date > day || time > at {
            value = value
                .checked_sub(
                    &crate::cash_ledger::cash_delta(row.data()).map_err(SnapshotError::Cash)?,
                )
                .map_err(SnapshotError::Arithmetic)?;
        }
    }
    Ok(value)
}
