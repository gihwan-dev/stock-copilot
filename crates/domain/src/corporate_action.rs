use crate::{CalendarDate, InstrumentId, LotError, Money, Quantity, Symbol, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShareCount(u64);
impl ShareCount {
    pub fn try_new(value: u64) -> Result<Self, LotError> {
        if value == 0 {
            return Err(LotError::InvalidRatio);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareRatio {
    old: ShareCount,
    new: ShareCount,
}
impl ShareRatio {
    #[must_use]
    pub const fn new(old: ShareCount, new: ShareCount) -> Self {
        Self { old, new }
    }
    #[must_use]
    pub const fn old_shares(self) -> ShareCount {
        self.old
    }
    #[must_use]
    pub const fn new_shares(self) -> ShareCount {
        self.new
    }
    #[must_use]
    pub const fn inverse(self) -> Self {
        Self {
            old: self.new,
            new: self.old,
        }
    }
}
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
    Split(ShareRatio),
    ReverseSplit(ShareRatio),
    BonusIssue {
        ratio: ShareRatio,
        receipt: Option<Transaction>,
    },
    StockDividend {
        ratio: ShareRatio,
        receipt: Option<Transaction>,
        basis: DividendBasis,
    },
    SymbolChange {
        from: Symbol,
        to: Symbol,
    },
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
    attached_receipt: Option<Transaction>,
}
impl CorporateAction {
    pub fn try_new(
        id: CorporateActionId,
        date: CalendarDate,
        kind: CorporateActionKind,
    ) -> Result<Self, LotError> {
        match &kind {
            CorporateActionKind::Split(ratio) if ratio.new_shares() <= ratio.old_shares() => {
                return Err(LotError::InvalidRatio);
            }
            CorporateActionKind::ReverseSplit(ratio)
                if ratio.new_shares() >= ratio.old_shares() =>
            {
                return Err(LotError::InvalidRatio);
            }
            CorporateActionKind::SymbolChange { from, to } if from == to => {
                return Err(LotError::InvalidIdentity);
            }
            CorporateActionKind::StockDividend {
                basis: DividendBasis::Confirmed(cost),
                ..
            } if cost.amount().as_decimal().is_sign_negative()
                && !cost.amount().as_decimal().is_zero() =>
            {
                return Err(LotError::NegativeCost);
            }
            CorporateActionKind::Split(_)
            | CorporateActionKind::ReverseSplit(_)
            | CorporateActionKind::BonusIssue { .. }
            | CorporateActionKind::StockDividend { .. }
            | CorporateActionKind::SymbolChange { .. }
            | CorporateActionKind::Delisting
            | CorporateActionKind::Merger
            | CorporateActionKind::Demerger
            | CorporateActionKind::Exchange => (),
        }
        Ok(Self {
            id,
            date,
            kind,
            attached_receipt: None,
        })
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
    pub fn with_receipt(mut self, receipt: Transaction) -> Result<Self, LotError> {
        if self.receipt().is_some() {
            return Err(LotError::InvalidReceipt);
        }
        self.attached_receipt = Some(receipt);
        Ok(self)
    }
    pub(crate) fn receipt(&self) -> Option<&Transaction> {
        if let Some(receipt) = &self.attached_receipt {
            return Some(receipt);
        }
        match &self.kind {
            CorporateActionKind::StockDividend { receipt, .. }
            | CorporateActionKind::BonusIssue { receipt, .. } => receipt.as_ref(),
            CorporateActionKind::Split(_)
            | CorporateActionKind::ReverseSplit(_)
            | CorporateActionKind::SymbolChange { .. }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FractionalEntitlement {
    action: CorporateActionId,
    quantity: crate::ShareQuantity,
    cost: crate::LotCost,
}
impl FractionalEntitlement {
    pub(crate) fn new(
        action: CorporateActionId,
        quantity: crate::ShareQuantity,
        cost: crate::LotCost,
    ) -> Self {
        Self {
            action,
            quantity,
            cost,
        }
    }
    #[must_use]
    pub const fn action(&self) -> &CorporateActionId {
        &self.action
    }
    #[must_use]
    pub const fn rational_quantity(&self) -> &crate::ShareQuantity {
        &self.quantity
    }
    pub fn quantity(&self) -> Result<Quantity, LotError> {
        self.quantity.exact_quantity()
    }
    #[must_use]
    pub const fn cost(&self) -> crate::LotCost {
        self.cost
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Pending,
    Received,
    NotReported,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorporateDelivery {
    action: CorporateActionId,
    quantity: crate::ShareQuantity,
    receipt: Option<Transaction>,
    status: DeliveryStatus,
}
impl CorporateDelivery {
    pub(crate) fn new(
        action: CorporateActionId,
        quantity: crate::ShareQuantity,
        receipt: Option<Transaction>,
        status: DeliveryStatus,
    ) -> Self {
        Self {
            action,
            quantity,
            receipt,
            status,
        }
    }
    #[must_use]
    pub const fn action(&self) -> &CorporateActionId {
        &self.action
    }
    #[must_use]
    pub const fn rational_quantity(&self) -> &crate::ShareQuantity {
        &self.quantity
    }
    #[must_use]
    pub const fn receipt(&self) -> Option<&Transaction> {
        self.receipt.as_ref()
    }
    #[must_use]
    pub const fn status(&self) -> DeliveryStatus {
        self.status
    }
}
