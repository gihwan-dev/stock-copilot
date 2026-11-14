use crate::{CalendarDate, InstrumentId, LotError, Money, Quantity, Ratio, Symbol, Transaction};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CorporateActionId(InstrumentId);
impl CorporateActionId {
    #[must_use]
    pub const fn new(value: InstrumentId) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividendBasis {
    Zero,
    Confirmed(Money),
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorporateActionKind {
    Split(Ratio),
    ReverseSplit(Ratio),
    BonusIssue {
        ratio: Ratio,
        receipt: Option<Transaction>,
    },
    StockDividend {
        receipt: Transaction,
        basis: DividendBasis,
    },
    SymbolChange(Symbol),
    Delisting,
    Merger,
    Demerger,
    Exchange,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorporateAction {
    id: CorporateActionId,
    date: CalendarDate,
    kind: CorporateActionKind,
}
impl CorporateAction {
    pub fn try_new(
        id: CorporateActionId,
        date: CalendarDate,
        kind: CorporateActionKind,
    ) -> Result<Self, LotError> {
        match &kind {
            CorporateActionKind::Split(ratio)
                if ratio.as_decimal() <= rust_decimal::Decimal::ONE =>
            {
                return Err(LotError::InvalidRatio);
            }
            CorporateActionKind::ReverseSplit(ratio)
                if ratio.as_decimal() <= rust_decimal::Decimal::ZERO
                    || ratio.as_decimal() >= rust_decimal::Decimal::ONE =>
            {
                return Err(LotError::InvalidRatio);
            }
            CorporateActionKind::BonusIssue { ratio, .. }
                if ratio.as_decimal() <= rust_decimal::Decimal::ZERO =>
            {
                return Err(LotError::InvalidRatio);
            }
            CorporateActionKind::StockDividend {
                basis: DividendBasis::Confirmed(cost),
                ..
            } if cost.amount().as_decimal().is_sign_negative() => {
                return Err(LotError::NegativeCost);
            }
            CorporateActionKind::Split(_)
            | CorporateActionKind::ReverseSplit(_)
            | CorporateActionKind::BonusIssue { .. }
            | CorporateActionKind::StockDividend { .. }
            | CorporateActionKind::SymbolChange(_)
            | CorporateActionKind::Delisting
            | CorporateActionKind::Merger
            | CorporateActionKind::Demerger
            | CorporateActionKind::Exchange => (),
        }
        Ok(Self { id, date, kind })
    }
    #[must_use]
    pub const fn id(&self) -> &CorporateActionId {
        &self.id
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub const fn kind(&self) -> &CorporateActionKind {
        &self.kind
    }
    pub(crate) fn receipt(&self) -> Option<&Transaction> {
        match &self.kind {
            CorporateActionKind::StockDividend { receipt, .. } => Some(receipt),
            CorporateActionKind::BonusIssue { receipt, .. } => receipt.as_ref(),
            CorporateActionKind::Split(_)
            | CorporateActionKind::ReverseSplit(_)
            | CorporateActionKind::SymbolChange(_)
            | CorporateActionKind::Delisting
            | CorporateActionKind::Merger
            | CorporateActionKind::Demerger
            | CorporateActionKind::Exchange => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotListing {
    Listed,
    Delisted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotTaxStatus {
    Ordinary,
    NeedsConfirmation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FractionalEntitlement(Quantity);
impl FractionalEntitlement {
    pub(crate) const fn new(quantity: Quantity) -> Self {
        Self(quantity)
    }
    #[must_use]
    pub const fn quantity(self) -> Quantity {
        self.0
    }
}
