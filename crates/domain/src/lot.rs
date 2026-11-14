use crate::cost_basis::add_cost;
use crate::cost_basis_arithmetic::rounded_cost;
use crate::{
    Amount, AssetClass, CalendarDate, CorporateAction, CorporateActionId, CorporateActionKind,
    CorporateDelivery, CostBasisMethod, Currency, DeliveryStatus, DividendBasis,
    FractionalEntitlement, InstrumentId, LotCost, LotListing, LotTaxStatus, LotUnitPrice, Market,
    Money, MoneyError, Quantity, QuantityShape, SettledRowSet, ShareQuantity, ShareRatio, Symbol,
    Transaction, TransactionKind,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotError {
    Arithmetic(MoneyError),
    InvalidRatio,
    InvalidQuantity,
    NegativeCost,
    InvalidIdentity,
    MissingData,
    InsufficientQuantity,
    AmbiguousOrder,
    DuplicateAction,
    FutureAction,
    InvalidReceipt,
    CurrencyMismatch,
    ManualActionRequired,
    Delisted,
    FractionalCashRequired,
}
impl From<MoneyError> for LotError {
    fn from(value: MoneyError) -> Self {
        Self::Arithmetic(value)
    }
}
impl From<crate::NumericError> for LotError {
    fn from(_value: crate::NumericError) -> Self {
        Self::InvalidQuantity
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotCostCurrency {
    Trading,
    SettlementKrw,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotIdentity {
    instrument: InstrumentId,
    symbol: Symbol,
    market: Market,
    currency: Currency,
    cost_currency: LotCostCurrency,
}
impl LotIdentity {
    pub(crate) fn with_historical_symbol(&self, symbol: Symbol) -> Self {
        let mut identity = self.clone();
        identity.symbol = symbol;
        identity
    }
    #[must_use]
    pub const fn new(
        instrument: InstrumentId,
        symbol: Symbol,
        market: Market,
        currency: Currency,
    ) -> Self {
        Self {
            instrument,
            symbol,
            market,
            currency,
            cost_currency: LotCostCurrency::Trading,
        }
    }
    #[must_use]
    pub fn with_cost_currency(mut self, value: LotCostCurrency) -> Self {
        self.cost_currency = value;
        self
    }
    #[must_use]
    pub const fn cost_currency(&self) -> Currency {
        match self.cost_currency {
            LotCostCurrency::Trading => self.currency,
            LotCostCurrency::SettlementKrw => Currency::Krw,
        }
    }
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn market(&self) -> &Market {
        &self.market
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lot {
    acquired: CalendarDate,
    quantity: ShareQuantity,
    cost: LotCost,
    tax_status: LotTaxStatus,
}
impl Lot {
    #[must_use]
    pub const fn acquired(&self) -> CalendarDate {
        self.acquired
    }
    pub fn quantity(&self) -> Result<Quantity, LotError> {
        self.quantity.exact_quantity()
    }
    #[must_use]
    pub const fn rational_quantity(&self) -> &ShareQuantity {
        &self.quantity
    }
    #[must_use]
    pub const fn cost(&self) -> LotCost {
        self.cost
    }
    #[must_use]
    pub const fn tax_status(&self) -> LotTaxStatus {
        self.tax_status
    }
    pub fn unit_price(&self) -> Result<Option<LotUnitPrice>, LotError> {
        match self.cost {
            LotCost::Known(cost) => LotUnitPrice::calculate(cost, &self.quantity).map(Some),
            LotCost::Unknown => Ok(None),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LotAssumption {
    TransactionOrder,
    DelistedSettlement,
    ReceiptMatch,
    FractionalReceiptQuantity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotRealization {
    transaction: Transaction,
    consumed: LotCost,
    gain: LotCost,
    assumptions: BTreeSet<LotAssumption>,
}
impl LotRealization {
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }
    #[must_use]
    pub const fn consumed_cost(&self) -> LotCost {
        self.consumed
    }
    #[must_use]
    pub const fn gain(&self) -> LotCost {
        self.gain
    }
    #[must_use]
    pub const fn assumptions(&self) -> &BTreeSet<LotAssumption> {
        &self.assumptions
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostPool {
    quantity: ShareQuantity,
    cost: LotCost,
}
impl CostPool {
    pub fn quantity(&self) -> Result<Quantity, LotError> {
        self.quantity.exact_quantity()
    }
    #[must_use]
    pub const fn rational_quantity(&self) -> &ShareQuantity {
        &self.quantity
    }
    #[must_use]
    pub const fn cost(&self) -> LotCost {
        self.cost
    }
    pub fn unit_price(&self) -> Result<Option<LotUnitPrice>, LotError> {
        match self.cost {
            LotCost::Known(_) if self.quantity == ShareQuantity::zero() => Ok(None),
            LotCost::Known(cost) => LotUnitPrice::calculate(cost, &self.quantity).map(Some),
            LotCost::Unknown => Ok(None),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotBook {
    identity: LotIdentity,
    method: CostBasisMethod,
    lots: Vec<Lot>,
    pool: CostPool,
    realized: LotCost,
    realizations: Vec<LotRealization>,
    listing: LotListing,
    applied: BTreeSet<CorporateActionId>,
    fractional: Vec<FractionalEntitlement>,
    deliveries: Vec<CorporateDelivery>,
    assumptions: BTreeSet<LotAssumption>,
    reserved: BTreeSet<Transaction>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TradeOrder {
    Acquisition,
    Disposal,
    Other,
}
#[derive(Debug, Clone, Copy)]
struct Allocation {
    ratio: ShareRatio,
    basis: Option<DividendBasis>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Consumption {
    Oldest,
    Newest,
}
impl LotBook {
    pub fn from_settled(
        state: &SettledRowSet,
        mut identity: LotIdentity,
        method: CostBasisMethod,
        actions: &[CorporateAction],
        as_of: CalendarDate,
    ) -> Result<Self, LotError> {
        validate_identity(&identity)?;
        let mut ordered = actions.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|action| action.date());
        validate_actions(&ordered, as_of)?;
        let (initial, history) = symbol_history(&identity.symbol, &ordered)?;
        identity.symbol = initial;
        let all = state
            .active_settled_for_calculation()
            .filter(|row| {
                row.data().market == identity.market
                    && row.data().asset_class == Some(AssetClass::Stock)
                    && row
                        .data()
                        .symbol
                        .as_ref()
                        .is_some_and(|symbol| history.contains(symbol))
            })
            .collect::<Vec<_>>();
        let mut eligible = all
            .iter()
            .copied()
            .filter(|row| row.data().settlement_date <= as_of)
            .collect::<Vec<_>>();
        eligible.sort_by_key(|row| row.data().settlement_date);
        let mut result = Self::empty(identity, method);
        let mut trades = eligible.into_iter().peekable();
        for action in ordered {
            let mut preceding = Vec::new();
            while let Some(row) = trades.next_if(|row| row.data().settlement_date < action.date()) {
                preceding.push(row);
            }
            result.apply_batch(&preceding)?;
            result.apply_action(action, &all, as_of)?;
            if trades.clone().any(|row| {
                row.data().settlement_date == action.date()
                    && !result.reserved.contains(row)
                    && trade_order(&row.data().kind) != TradeOrder::Other
            }) {
                return Err(LotError::AmbiguousOrder);
            }
        }
        result.apply_batch(&trades.collect::<Vec<_>>())?;
        Ok(result)
    }
    fn empty(identity: LotIdentity, method: CostBasisMethod) -> Self {
        let cost = LotCost::Known(zero(identity.cost_currency()));
        Self {
            identity,
            method,
            lots: Vec::new(),
            pool: CostPool {
                quantity: ShareQuantity::zero(),
                cost,
            },
            realized: cost,
            realizations: Vec::new(),
            listing: LotListing::Listed,
            applied: BTreeSet::new(),
            fractional: Vec::new(),
            deliveries: Vec::new(),
            assumptions: BTreeSet::new(),
            reserved: BTreeSet::new(),
        }
    }
    #[must_use]
    pub const fn identity(&self) -> &LotIdentity {
        &self.identity
    }
    #[must_use]
    pub fn lots(&self) -> &[Lot] {
        &self.lots
    }
    #[must_use]
    pub const fn pool(&self) -> &CostPool {
        &self.pool
    }
    #[must_use]
    pub const fn method(&self) -> CostBasisMethod {
        self.method
    }
    #[must_use]
    pub const fn realized_gain(&self) -> LotCost {
        self.realized
    }
    #[must_use]
    pub fn realizations(&self) -> &[LotRealization] {
        &self.realizations
    }
    #[must_use]
    pub const fn listing(&self) -> LotListing {
        self.listing
    }
    #[must_use]
    pub const fn applied_actions(&self) -> &BTreeSet<CorporateActionId> {
        &self.applied
    }
    #[must_use]
    pub fn fractional_entitlements(&self) -> &[FractionalEntitlement] {
        &self.fractional
    }
    #[must_use]
    pub fn deliveries(&self) -> &[CorporateDelivery] {
        &self.deliveries
    }
    #[must_use]
    pub const fn assumptions(&self) -> &BTreeSet<LotAssumption> {
        &self.assumptions
    }
    pub fn quantity(&self) -> Result<Quantity, LotError> {
        self.pool.quantity()
    }
    pub fn total_cost(&self) -> Result<LotCost, LotError> {
        self.fractional
            .iter()
            .try_fold(self.pool.cost, |total, claim| {
                add_cost(total, claim.cost()).map_err(LotError::from)
            })
    }
    fn apply_batch(&mut self, rows: &[&Transaction]) -> Result<(), LotError> {
        let mut days = BTreeMap::<CalendarDate, Vec<&Transaction>>::new();
        for row in rows.iter().filter(|row| !self.reserved.contains(*row)) {
            days.entry(row.data().settlement_date)
                .or_default()
                .push(row);
        }
        for (date, mut day) in days {
            let unknown_days = day
                .iter()
                .filter(|row| {
                    row.data().transaction_time.is_none()
                        && trade_order(&row.data().kind) != TradeOrder::Other
                })
                .map(|_| date)
                .collect::<BTreeSet<_>>();
            day.sort_by(|left, right| canonical_order(left, right, &unknown_days));
            if day.array_windows::<2>().any(|[left, right]| {
                left != right
                    && (unknown_days.contains(&date)
                        || left.data().transaction_time == right.data().transaction_time)
            }) {
                self.assumptions.insert(LotAssumption::TransactionOrder);
            }
            for row in day {
                self.apply_unreserved(row)?;
            }
        }
        Ok(())
    }
    fn apply_unreserved(&mut self, row: &Transaction) -> Result<LotListing, LotError> {
        if row.data().kind == TransactionKind::SecuritiesTransferIn
            && self.deliveries.iter().any(|delivery| {
                delivery.receipt().is_none()
                    && (delivery.status() == DeliveryStatus::Pending
                        || row.data().quantity.is_some_and(|value| {
                            ShareQuantity::from_quantity(value) == *delivery.rational_quantity()
                        }))
            })
        {
            return Err(LotError::InvalidReceipt);
        }
        self.apply_row(row)
    }
    fn apply_row(&mut self, row: &Transaction) -> Result<LotListing, LotError> {
        if row.data().currency != self.identity.currency {
            return Err(LotError::CurrencyMismatch);
        }
        match row.data().kind {
            TransactionKind::Buy | TransactionKind::SecuritiesTransferIn => {
                if self.listing == LotListing::Delisted {
                    return Err(LotError::Delisted);
                }
                let cost = if row.data().kind == TransactionKind::Buy {
                    LotCost::Known(self.native_cost(row, purchase_cost(row)?)?)
                } else {
                    LotCost::Unknown
                };
                self.acquire(
                    row.data().settlement_date,
                    ShareQuantity::from_quantity(positive_quantity(row)?),
                    cost,
                    if cost == LotCost::Unknown {
                        LotTaxStatus::NeedsConfirmation
                    } else {
                        LotTaxStatus::Ordinary
                    },
                )?;
            }
            TransactionKind::Sell | TransactionKind::SecuritiesTransferOut => {
                let consumed =
                    self.consume(&ShareQuantity::from_quantity(positive_quantity(row)?))?;
                if self.listing == LotListing::Delisted {
                    self.assumptions.insert(LotAssumption::DelistedSettlement);
                }
                if row.data().kind == TransactionKind::Sell {
                    self.record_sale(row, consumed)?;
                }
            }
            TransactionKind::Unknown(_) => return Err(LotError::MissingData),
            TransactionKind::Deposit
            | TransactionKind::Withdrawal
            | TransactionKind::ExchangeIn
            | TransactionKind::ExchangeOut
            | TransactionKind::Dividend
            | TransactionKind::Interest
            | TransactionKind::Fee => (),
        }
        Ok(self.listing)
    }
    fn acquire(
        &mut self,
        date: CalendarDate,
        quantity: ShareQuantity,
        cost: LotCost,
        tax_status: LotTaxStatus,
    ) -> Result<LotCost, LotError> {
        self.pool.quantity = self.pool.quantity.plus(&quantity);
        self.pool.cost = add_cost(self.pool.cost, cost)?;
        self.lots.push(Lot {
            acquired: date,
            quantity,
            cost,
            tax_status,
        });
        Ok(cost)
    }
    fn consume(&mut self, quantity: &ShareQuantity) -> Result<LotCost, LotError> {
        self.consume_with(quantity, Consumption::Oldest)
    }
    fn consume_with(
        &mut self,
        quantity: &ShareQuantity,
        order: Consumption,
    ) -> Result<LotCost, LotError> {
        if quantity > &self.pool.quantity {
            return Err(LotError::InsufficientQuantity);
        }
        let average = if self.method == CostBasisMethod::MovingAverage {
            Some(cost_portion(self.pool.cost, quantity, &self.pool.quantity)?)
        } else {
            None
        };
        let fifo = consume_lots(
            &mut self.lots,
            quantity,
            self.identity.cost_currency(),
            order,
        )?;
        self.pool.quantity = self.pool.quantity.split(quantity).1;
        let used = average.unwrap_or(fifo);
        self.pool.cost = if self.pool.quantity == ShareQuantity::zero() {
            LotCost::Known(zero(self.identity.cost_currency()))
        } else if self.method == CostBasisMethod::Fifo {
            self.lots.iter().try_fold(
                LotCost::Known(zero(self.identity.cost_currency())),
                |total, lot| add_cost(total, lot.cost),
            )?
        } else {
            cost_subtract(self.pool.cost, used)?
        };
        Ok(used)
    }
    fn record_sale(&mut self, row: &Transaction, consumed: LotCost) -> Result<LotCost, LotError> {
        let proceeds = row
            .data()
            .amount
            .ok_or(LotError::MissingData)?
            .checked_sub(&row.data().commission)?
            .checked_sub(&row.data().tax)?;
        let gain = cost_subtract(LotCost::Known(self.native_cost(row, proceeds)?), consumed)?;
        self.realized = add_cost(self.realized, gain)?;
        self.realizations.push(LotRealization {
            transaction: row.clone(),
            consumed,
            gain,
            assumptions: self.assumptions.clone(),
        });
        Ok(gain)
    }
    fn native_cost(&self, row: &Transaction, value: Money) -> Result<Money, LotError> {
        if self.identity.currency == self.identity.cost_currency() {
            return Ok(value);
        }
        let rate = row
            .data()
            .exchange_rate
            .filter(|rate| rate.as_decimal() > rust_decimal::Decimal::ZERO)
            .ok_or(LotError::MissingData)?;
        Ok(Money::new(
            value.checked_mul(&rate)?.amount(),
            Currency::Krw,
        ))
    }
    fn apply_action(
        &mut self,
        action: &CorporateAction,
        all: &[&Transaction],
        as_of: CalendarDate,
    ) -> Result<LotListing, LotError> {
        if self.listing == LotListing::Delisted {
            return Err(LotError::Delisted);
        }
        match action.kind() {
            CorporateActionKind::Split(ratio) | CorporateActionKind::ReverseSplit(ratio) => {
                self.transform(action, *ratio, all, as_of)?;
            }
            CorporateActionKind::BonusIssue { ratio, .. } => {
                self.allocate(
                    action,
                    Allocation {
                        ratio: *ratio,
                        basis: None,
                    },
                    all,
                    as_of,
                )?;
            }
            CorporateActionKind::StockDividend { ratio, basis, .. } => {
                self.allocate(
                    action,
                    Allocation {
                        ratio: *ratio,
                        basis: Some(*basis),
                    },
                    all,
                    as_of,
                )?;
            }
            CorporateActionKind::SymbolChange { to, .. } => {
                self.identity.symbol = to.clone();
            }
            CorporateActionKind::Delisting => {
                self.listing = LotListing::Delisted;
            }
            CorporateActionKind::Merger
            | CorporateActionKind::Demerger
            | CorporateActionKind::Exchange => return Err(LotError::ManualActionRequired),
        }
        self.applied.insert(action.id().clone());
        Ok(self.listing)
    }
    fn transform(
        &mut self,
        action: &CorporateAction,
        ratio: ShareRatio,
        all: &[&Transaction],
        as_of: CalendarDate,
    ) -> Result<LotCost, LotError> {
        let before = self.pool.quantity.clone();
        let gross = before.scaled(ratio);
        let receipt = self.link_receipt(action, &gross, all)?;
        let actual = self.received_quantity(&before, &gross, receipt.as_ref())?;
        for lot in &mut self.lots {
            lot.quantity = lot.quantity.scaled(ratio);
        }
        self.pool.quantity = gross.clone();
        self.settle_quantity(action, &gross, &actual, Consumption::Oldest)?;
        self.record_delivery(action, actual, receipt, as_of);
        Ok(self.pool.cost)
    }
    fn allocate(
        &mut self,
        action: &CorporateAction,
        allocation: Allocation,
        all: &[&Transaction],
        as_of: CalendarDate,
    ) -> Result<LotCost, LotError> {
        let Allocation { ratio, basis } = allocation;
        let before = self.pool.quantity.clone();
        let gross = before.scaled(ratio);
        let receipt = self.link_receipt(action, &gross, all)?;
        let actual = self.received_quantity(&before, &gross, receipt.as_ref())?;
        match basis {
            None => {
                let total = before.plus(&gross);
                for lot in &mut self.lots {
                    lot.quantity = lot.quantity.plus(&lot.quantity.scaled(ratio));
                    lot.tax_status = LotTaxStatus::NeedsConfirmation;
                }
                self.pool.quantity = total;
            }
            Some(value) => {
                let cost = self.dividend_cost(value)?;
                if gross != ShareQuantity::zero() {
                    self.acquire(
                        action.date(),
                        gross.clone(),
                        cost,
                        LotTaxStatus::NeedsConfirmation,
                    )?;
                }
            }
        }
        self.settle_quantity(
            action,
            &gross,
            &actual,
            if basis.is_some() {
                Consumption::Newest
            } else {
                Consumption::Oldest
            },
        )?;
        self.record_delivery(action, actual, receipt, as_of);
        Ok(self.pool.cost)
    }
    fn dividend_cost(&self, basis: DividendBasis) -> Result<LotCost, LotError> {
        match basis {
            DividendBasis::Zero => Ok(LotCost::Known(zero(self.identity.cost_currency()))),
            DividendBasis::Unknown => Ok(LotCost::Unknown),
            DividendBasis::Confirmed(cost) => {
                if cost.currency() != self.identity.cost_currency() {
                    return Err(LotError::CurrencyMismatch);
                }
                Ok(LotCost::Known(cost))
            }
        }
    }
    fn received_quantity(
        &mut self,
        before: &ShareQuantity,
        gross: &ShareQuantity,
        receipt: Option<&Transaction>,
    ) -> Result<ShareQuantity, LotError> {
        if self.identity.market == Market::Us
            && before.shape() == QuantityShape::Fractional
            && gross.shape() == QuantityShape::Fractional
        {
            let observed = ShareQuantity::from_quantity(positive_quantity(
                receipt.ok_or(LotError::MissingData)?,
            )?);
            let floor = ShareQuantity::from_quantity(gross.whole_quantity()?);
            if observed < floor || observed > floor.plus(&ShareQuantity::one()) {
                return Err(LotError::InvalidReceipt);
            }
            self.assumptions
                .insert(LotAssumption::FractionalReceiptQuantity);
            return Ok(observed);
        }
        let actual = ShareQuantity::from_quantity(gross.whole_quantity()?);
        if let Some(observed) = receipt
            && ShareQuantity::from_quantity(positive_quantity(observed)?) != actual
        {
            return Err(LotError::InvalidReceipt);
        }
        Ok(actual)
    }
    fn link_receipt(
        &mut self,
        action: &CorporateAction,
        expected: &ShareQuantity,
        all: &[&Transaction],
    ) -> Result<Option<Transaction>, LotError> {
        let mut candidates = all
            .iter()
            .copied()
            .filter(|row| {
                row.data().kind == TransactionKind::SecuritiesTransferIn
                    && receipt_window(action.date(), row.data().settlement_date)
                        == ReceiptWindow::Inside
                    && row.data().currency == self.identity.currency
                    && !self.reserved.contains(*row)
            })
            .collect::<Vec<_>>();
        if let Some(explicit) = action.receipt() {
            candidates.retain(|row| *row == explicit);
        } else {
            let whole = expected.whole_quantity()?;
            let floor = ShareQuantity::from_quantity(whole);
            let fractional_range = (self.identity.market == Market::Us
                && self.pool.quantity.shape() == QuantityShape::Fractional)
                .then(|| ReceiptBounds {
                    minimum: floor.clone(),
                    maximum: floor.plus(&ShareQuantity::one()),
                });
            candidates.retain(|row| {
                candidate_quantity(row, whole, fractional_range.as_ref())
                    == ReceiptQuantity::Matches
            });
        }
        if candidates.len() > 1 {
            return Err(LotError::InvalidReceipt);
        }
        let selected = candidates.first().copied().cloned();
        if action.receipt().is_some() && selected.is_none() {
            return Err(LotError::InvalidReceipt);
        }
        if let Some(receipt) = &selected {
            self.reserved.insert(receipt.clone());
            self.assumptions.insert(LotAssumption::ReceiptMatch);
        }
        Ok(selected)
    }
    fn settle_quantity(
        &mut self,
        action: &CorporateAction,
        gross: &ShareQuantity,
        actual: &ShareQuantity,
        order: Consumption,
    ) -> Result<LotCost, LotError> {
        if actual > gross {
            let extra = actual.split(gross).1;
            for lot in self.lots.iter_mut().rev().take(1) {
                lot.quantity = lot.quantity.plus(&extra);
            }
            self.pool.quantity = self.pool.quantity.plus(&extra);
            return Ok(LotCost::Known(zero(self.identity.cost_currency())));
        }
        self.carve_cash(action, &gross.split(actual).1, order)
    }
    fn carve_cash(
        &mut self,
        action: &CorporateAction,
        fraction: &ShareQuantity,
        order: Consumption,
    ) -> Result<LotCost, LotError> {
        if fraction == &ShareQuantity::zero() {
            return Ok(LotCost::Known(zero(self.identity.cost_currency())));
        }
        let cost = self.consume_with(fraction, order)?;
        self.fractional.push(FractionalEntitlement::new(
            action.id().clone(),
            fraction.clone(),
            cost,
        ));
        Ok(cost)
    }
    fn record_delivery(
        &mut self,
        action: &CorporateAction,
        quantity: ShareQuantity,
        receipt: Option<Transaction>,
        as_of: CalendarDate,
    ) {
        let status = match &receipt {
            Some(row) if row.data().settlement_date <= as_of => DeliveryStatus::Received,
            None if quantity == ShareQuantity::zero() => DeliveryStatus::NotReported,
            None => match action.kind() {
                CorporateActionKind::BonusIssue { .. }
                | CorporateActionKind::StockDividend { .. }
                    if quantity != ShareQuantity::zero() =>
                {
                    DeliveryStatus::Pending
                }
                CorporateActionKind::Split(_)
                | CorporateActionKind::ReverseSplit(_)
                | CorporateActionKind::BonusIssue { .. }
                | CorporateActionKind::StockDividend { .. }
                | CorporateActionKind::SymbolChange { .. }
                | CorporateActionKind::Delisting
                | CorporateActionKind::Merger
                | CorporateActionKind::Demerger
                | CorporateActionKind::Exchange => DeliveryStatus::NotReported,
            },
            Some(_) => DeliveryStatus::Pending,
        };
        self.deliveries.push(CorporateDelivery::new(
            action.id().clone(),
            quantity,
            receipt,
            status,
        ));
    }
}
fn consume_lots(
    lots: &mut Vec<Lot>,
    quantity: &ShareQuantity,
    currency: Currency,
    order: Consumption,
) -> Result<LotCost, LotError> {
    if order == Consumption::Newest {
        lots.reverse();
    }
    let mut remaining = quantity.clone();
    let mut total = LotCost::Known(zero(currency));
    for lot in lots.iter_mut() {
        if remaining == ShareQuantity::zero() {
            break;
        }
        let (take, remainder) = lot.quantity.split(&remaining);
        let used = cost_portion(lot.cost, &take, &lot.quantity)?;
        lot.cost = cost_subtract(lot.cost, used)?;
        lot.quantity = remainder;
        remaining = remaining.split(&take).1;
        total = add_cost(total, used)?;
    }
    lots.retain(|lot| lot.quantity != ShareQuantity::zero());
    if order == Consumption::Newest {
        lots.reverse();
    }
    Ok(total)
}
fn zero(currency: Currency) -> Money {
    Money::new(Amount::new(rust_decimal::Decimal::ZERO), currency)
}
fn positive_quantity(row: &Transaction) -> Result<Quantity, LotError> {
    row.data()
        .quantity
        .filter(|quantity| !quantity.as_decimal().is_zero())
        .ok_or(LotError::MissingData)
}
fn purchase_cost(row: &Transaction) -> Result<Money, LotError> {
    Ok(row
        .data()
        .amount
        .ok_or(LotError::MissingData)?
        .checked_add(&row.data().commission)?
        .checked_add(&row.data().tax)?)
}
fn cost_subtract(total: LotCost, used: LotCost) -> Result<LotCost, LotError> {
    match (total, used) {
        (LotCost::Known(first), LotCost::Known(second)) => first
            .checked_sub(&second)
            .map(LotCost::Known)
            .map_err(LotError::from),
        _ => Ok(LotCost::Unknown),
    }
}
fn cost_portion(
    cost: LotCost,
    take: &ShareQuantity,
    total: &ShareQuantity,
) -> Result<LotCost, LotError> {
    match cost {
        LotCost::Known(value) => rounded_cost(value, take, total).map(LotCost::Known),
        LotCost::Unknown => Ok(LotCost::Unknown),
    }
}
fn validate_identity(identity: &LotIdentity) -> Result<Currency, LotError> {
    match (&identity.market, identity.currency) {
        (Market::Kr, Currency::Krw) | (Market::Us, Currency::Usd) => Ok(identity.currency),
        _ => Err(LotError::InvalidIdentity),
    }
}
fn validate_actions(
    actions: &[&CorporateAction],
    as_of: CalendarDate,
) -> Result<CalendarDate, LotError> {
    let mut ids = BTreeSet::new();
    for action in actions {
        if action.date() > as_of {
            return Err(LotError::FutureAction);
        }
        if !ids.insert(action.id()) {
            return Err(LotError::DuplicateAction);
        }
    }
    if actions
        .array_windows::<2>()
        .any(|[left, right]| left.date() == right.date())
    {
        return Err(LotError::AmbiguousOrder);
    }
    Ok(as_of)
}
fn symbol_history(
    anchor: &Symbol,
    actions: &[&CorporateAction],
) -> Result<(Symbol, Vec<Symbol>), LotError> {
    let mut initial = anchor.clone();
    let mut history = Vec::new();
    let mut seen = BTreeSet::new();
    for action in actions {
        if let CorporateActionKind::SymbolChange { from, to } = action.kind() {
            if history.last().is_some_and(|last| last != from) || seen.contains(to) {
                return Err(LotError::InvalidIdentity);
            }
            if history.is_empty() {
                initial = from.clone();
                history.push(from.clone());
                seen.insert(from.clone());
            }
            history.push(to.clone());
            seen.insert(to.clone());
        }
    }
    if history.is_empty() {
        return Ok((initial, vec![anchor.clone()]));
    }
    if !seen.contains(anchor) {
        return Err(LotError::InvalidIdentity);
    }
    Ok((initial, history))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiptWindow {
    Inside,
    Outside,
}
fn receipt_window(start: CalendarDate, end: CalendarDate) -> ReceiptWindow {
    if start
        .days_until(end)
        .is_ok_and(|days| (0..=90).contains(days.as_i64()))
    {
        ReceiptWindow::Inside
    } else {
        ReceiptWindow::Outside
    }
}
fn trade_order(kind: &TransactionKind) -> TradeOrder {
    match kind {
        TransactionKind::Buy | TransactionKind::SecuritiesTransferIn => TradeOrder::Acquisition,
        TransactionKind::Sell | TransactionKind::SecuritiesTransferOut => TradeOrder::Disposal,
        TransactionKind::Deposit
        | TransactionKind::Withdrawal
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Dividend
        | TransactionKind::Interest
        | TransactionKind::Fee
        | TransactionKind::Unknown(_) => TradeOrder::Other,
    }
}
#[derive(Debug, Clone)]
struct ReceiptBounds {
    minimum: ShareQuantity,
    maximum: ShareQuantity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiptQuantity {
    Matches,
    Differs,
}
fn candidate_quantity(
    row: &Transaction,
    whole: Quantity,
    range: Option<&ReceiptBounds>,
) -> ReceiptQuantity {
    if row.data().quantity == Some(whole)
        || range
            .zip(row.data().quantity)
            .is_some_and(|(bounds, value)| {
                ShareQuantity::from_quantity(value) >= bounds.minimum
                    && ShareQuantity::from_quantity(value) <= bounds.maximum
            })
    {
        ReceiptQuantity::Matches
    } else {
        ReceiptQuantity::Differs
    }
}
fn canonical_order(
    left: &Transaction,
    right: &Transaction,
    unknown_days: &BTreeSet<CalendarDate>,
) -> std::cmp::Ordering {
    left.data()
        .settlement_date
        .cmp(&right.data().settlement_date)
        .then_with(|| {
            if unknown_days.contains(&left.data().settlement_date) {
                std::cmp::Ordering::Equal
            } else {
                left.data()
                    .transaction_time
                    .cmp(&right.data().transaction_time)
            }
        })
        .then_with(|| trade_order(&left.data().kind).cmp(&trade_order(&right.data().kind)))
        .then_with(|| left.cmp(right))
}
#[cfg(test)]
mod tests {
    use crate::{
        AccountId, Amount, AmountInput, AssetClass, CalendarDate, CalendarYear, Cancellation,
        CorporateAction, CorporateActionId, CorporateActionKind, CostBasisMethod, Currency,
        DayNumber, DividendBasis, InstrumentId, LotBook, LotCost, LotCostCurrency, LotError,
        LotListing, Market, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds, Quantity,
        Ratio, SettledRowSet, Settlement, ShareCount, ShareRatio, StoredTransaction, Symbol,
        Timestamp, Transaction, TransactionBatch, TransactionData, TransactionKind, UnixSeconds,
    };
    fn money(value: &str) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(value)).expect("금액"),
            Currency::Krw,
        )
    }
    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(1),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn at(second: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(second),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn symbol() -> Symbol {
        Symbol::try_new("005930").expect("종목")
    }
    fn identity() -> crate::LotIdentity {
        crate::LotIdentity::new(
            InstrumentId::try_new("instrument").expect("식별"),
            symbol(),
            Market::Kr,
            Currency::Krw,
        )
    }
    fn ratio(old: u64, new: u64) -> ShareRatio {
        ShareRatio::new(
            ShareCount::try_new(old).expect("구주"),
            ShareCount::try_new(new).expect("신주"),
        )
    }
    fn action(day: u8, id: &str, kind: CorporateActionKind) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(InstrumentId::try_new(id).expect("식별")),
            date(day),
            kind,
        )
        .expect("행사")
    }
    fn row(day: u8, kind: TransactionKind, qty: &str, amount: Option<&str>) -> Transaction {
        Transaction::try_new(TransactionData {
            kind,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: Some(symbol()),
            asset_class: Some(AssetClass::Stock),
            isin: None,
            quantity: Some(quantity(qty)),
            filled_price: None,
            amount: amount.map(money),
            commission: money("0"),
            tax: money("0"),
            net_amount: None,
            exchange_rate: None,
            settlement_date: date(day),
            settlement: Settlement::Settled,
            transaction_time: None,
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    fn rows(transactions: Vec<Transaction>) -> SettledRowSet {
        SettledRowSet::try_restore(
            AccountId::new(1),
            transactions
                .into_iter()
                .map(|transaction| {
                    StoredTransaction::try_restore(transaction, at(1), None).expect("이력")
                })
                .collect(),
            TransactionBatch::try_new(vec![]).expect("임시"),
            Some(at(1)),
        )
        .expect("멀티셋")
    }
    fn book(
        transactions: Vec<Transaction>,
        method: CostBasisMethod,
        actions: &[CorporateAction],
    ) -> Result<LotBook, LotError> {
        LotBook::from_settled(&rows(transactions), identity(), method, actions, date(31))
    }
    fn charged(day: u8, kind: TransactionKind, qty: &str, values: [&str; 3]) -> Transaction {
        let mut data = row(day, kind, qty, Some(values[0])).data().clone();
        data.commission = money(values[1]);
        data.tax = money(values[2]);
        Transaction::try_new(data).expect("실제 비용")
    }
    fn usd(day: u8, kind: TransactionKind, qty: &str, values: [&str; 5]) -> Transaction {
        let mut data = charged(day, kind, qty, [values[0], values[1], values[2]])
            .data()
            .clone();
        data.currency = Currency::Usd;
        data.market = Market::Us;
        data.symbol = Some(Symbol::try_new("AAPL").expect("종목"));
        data.amount = data
            .amount
            .map(|value| Money::new(value.amount(), Currency::Usd));
        data.commission = Money::new(data.commission.amount(), Currency::Usd);
        data.tax = Money::new(data.tax.amount(), Currency::Usd);
        data.exchange_rate =
            Some(Ratio::parse(&NumericInput::try_new(values[3]).expect("입력")).expect("환율"));
        data.transaction_time = Some(at(values[4].parse().expect("시각")));
        Transaction::try_new(data).expect("달러 거래")
    }
    #[test]
    fn r11_example_two_keeps_fifo_and_pool_results() {
        for (method, cost, gain) in [
            (CostBasisMethod::Fifo, "1150", "300"),
            (CostBasisMethod::MovingAverage, "1100", "250"),
        ] {
            let result = book(
                vec![
                    row(1, TransactionKind::Buy, "10", Some("1000")),
                    row(2, TransactionKind::Buy, "10", Some("1200")),
                    row(3, TransactionKind::Sell, "5", Some("650")),
                    row(4, TransactionKind::Buy, "5", Some("550")),
                    row(5, TransactionKind::Sell, "10", Some("1250")),
                ],
                method,
                &[],
            )
            .expect("원장");
            assert_eq!(result.quantity().expect("수량"), quantity("10"));
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money(cost))
            );
            assert_eq!(result.realized_gain(), LotCost::Known(money(gain)));
        }
    }
    #[test]
    fn r11_example_five_uses_per_settlement_fx() {
        for (method, cost, gain) in [
            (CostBasisMethod::Fifo, "9729720", "6268962"),
            (CostBasisMethod::MovingAverage, "8768760", "5308002"),
        ] {
            let input = rows(vec![
                usd(
                    1,
                    TransactionKind::Buy,
                    "50",
                    ["7500", "7.50", "0", "1300", "1"],
                ),
                usd(
                    2,
                    TransactionKind::Buy,
                    "50",
                    ["9000", "9", "0", "1350", "2"],
                ),
                usd(
                    3,
                    TransactionKind::Sell,
                    "60",
                    ["13200", "13.20", "0.27", "1400", "3"],
                ),
            ]);
            let target = crate::LotIdentity::new(
                InstrumentId::try_new("instrument").expect("식별"),
                Symbol::try_new("AAPL").expect("종목"),
                Market::Us,
                Currency::Usd,
            )
            .with_cost_currency(LotCostCurrency::SettlementKrw);
            let result =
                LotBook::from_settled(&input, target, method, &[], date(31)).expect("환산");
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money(cost))
            );
            assert_eq!(result.realized_gain(), LotCost::Known(money(gain)));
        }
    }
    #[test]
    fn realistic_partial_sales_and_average_buys_preserve_every_won() {
        for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
            let result = book(
                vec![
                    charged(1, TransactionKind::Buy, "3", ["213702", "32", "0"]),
                    row(2, TransactionKind::Sell, "1", Some("80000")),
                ],
                method,
                &[],
            )
            .expect("부분 매도");
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money("142490"))
            );
            let second_result = book(
                vec![
                    row(1, TransactionKind::Buy, "7", Some("100000")),
                    row(2, TransactionKind::Sell, "2", Some("40000")),
                ],
                method,
                &[],
            )
            .expect("7주 중 2주");
            assert_eq!(
                second_result.total_cost().expect("원가"),
                LotCost::Known(money("71429"))
            );
        }
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "1", Some("100")),
                row(2, TransactionKind::Buy, "2", Some("250")),
            ],
            CostBasisMethod::MovingAverage,
            &[],
        )
        .expect("매수 풀");
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("350"))
        );
    }
    #[test]
    fn integer_share_ratios_do_not_drift() {
        for (change, expected) in [
            (CorporateActionKind::ReverseSplit(ratio(3, 1)), "10"),
            (CorporateActionKind::Split(ratio(3, 7)), "70"),
        ] {
            let result = book(
                vec![row(1, TransactionKind::Buy, "30", Some("3000"))],
                CostBasisMethod::Fifo,
                &[action(2, "ratio", change)],
            )
            .expect("정수비율");
            assert_eq!(result.quantity().expect("수량"), quantity(expected));
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money("3000"))
            );
        }
    }
    #[test]
    fn symbol_chain_can_be_queried_from_either_end() {
        let old = symbol();
        let new = Symbol::try_new("000660").expect("새 코드");
        let change = action(
            2,
            "rename",
            CorporateActionKind::SymbolChange {
                from: old,
                to: new.clone(),
            },
        );
        let input = rows(vec![row(1, TransactionKind::Buy, "10", Some("1000"))]);
        let first = LotBook::from_settled(
            &input,
            identity(),
            CostBasisMethod::Fifo,
            std::slice::from_ref(&change),
            date(31),
        )
        .expect("옛 코드");
        let target = crate::LotIdentity::new(
            InstrumentId::try_new("instrument").expect("식별"),
            new,
            Market::Kr,
            Currency::Krw,
        );
        let second =
            LotBook::from_settled(&input, target, CostBasisMethod::Fifo, &[change], date(31))
                .expect("새 코드");
        assert_eq!(first, second);
        assert_eq!(second.quantity().expect("수량"), quantity("10"));
    }
    #[test]
    fn all_corporate_fractional_shares_become_cash_claims() {
        for kind in [
            CorporateActionKind::BonusIssue {
                ratio: ratio(10, 3),
                receipt: None,
            },
            CorporateActionKind::StockDividend {
                ratio: ratio(10, 3),
                receipt: None,
                basis: DividendBasis::Zero,
            },
        ] {
            let receipt = row(3, TransactionKind::SecuritiesTransferIn, "4", None);
            let result = book(
                vec![row(1, TransactionKind::Buy, "15", Some("1500")), receipt],
                CostBasisMethod::Fifo,
                &[action(2, "allocation", kind)],
            )
            .expect("단주");
            assert_eq!(result.quantity().expect("수량"), quantity("19"));
            assert_eq!(result.fractional_entitlements().len(), 1);
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money("1500"))
            );
        }
        let result = book(
            vec![row(1, TransactionKind::Buy, "3", Some("300"))],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "merge",
                CorporateActionKind::ReverseSplit(ratio(2, 1)),
            )],
        )
        .expect("병합");
        assert_eq!(result.quantity().expect("수량"), quantity("1"));
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("300"))
        );
    }
    #[test]
    fn delayed_receipt_is_counted_once_and_pending_is_visible() {
        let input = rows(vec![
            row(1, TransactionKind::Buy, "10", Some("1000")),
            row(22, TransactionKind::SecuritiesTransferIn, "2", None),
        ]);
        let event = action(
            2,
            "bonus",
            CorporateActionKind::BonusIssue {
                ratio: ratio(5, 1),
                receipt: None,
            },
        );
        for (as_of, expected) in [
            (date(10), crate::DeliveryStatus::Pending),
            (date(31), crate::DeliveryStatus::Received),
        ] {
            let result = LotBook::from_settled(
                &input,
                identity(),
                CostBasisMethod::Fifo,
                std::slice::from_ref(&event),
                as_of,
            )
            .expect("지연 입고");
            assert_eq!(result.quantity().expect("수량"), quantity("12"));
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money("1000"))
            );
            assert_eq!(
                result.deliveries().first().expect("연결").status(),
                expected
            );
        }
    }
    #[test]
    fn simultaneous_fills_have_canonical_order_and_a_notice() {
        let buys = [
            row(1, TransactionKind::Buy, "1", Some("200")),
            row(1, TransactionKind::Buy, "1", Some("100")),
        ];
        let sale = row(1, TransactionKind::Sell, "1", Some("300"));
        let first = book(
            vec![buys[0].clone(), sale.clone(), buys[1].clone()],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("동시각");
        let second = book(
            vec![buys[1].clone(), buys[0].clone(), sale],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("순열");
        assert_eq!(first, second);
        assert_eq!(first.realized_gain(), LotCost::Known(money("200")));
        assert!(
            first
                .assumptions()
                .contains(&crate::LotAssumption::TransactionOrder)
        );
    }
    #[test]
    fn only_the_sale_consuming_unknown_cost_is_unknown() {
        let result = book(
            vec![
                row(1, TransactionKind::SecuritiesTransferIn, "1", None),
                row(2, TransactionKind::Sell, "1", Some("100")),
                row(3, TransactionKind::Buy, "1", Some("100")),
                row(4, TransactionKind::Sell, "1", Some("200")),
            ],
            CostBasisMethod::MovingAverage,
            &[],
        )
        .expect("재매수");
        assert_eq!(
            result.realizations().first().expect("첫 매도").gain(),
            LotCost::Unknown
        );
        assert_eq!(
            result.realizations().last().expect("다음 매도").gain(),
            LotCost::Known(money("100"))
        );
    }
    #[test]
    fn delisted_settlement_sales_are_accepted_and_marked() {
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "1", Some("100")),
                row(3, TransactionKind::Sell, "1", Some("200")),
            ],
            CostBasisMethod::Fifo,
            &[action(2, "delist", CorporateActionKind::Delisting)],
        )
        .expect("정리매매 결제");
        assert_eq!(result.listing(), LotListing::Delisted);
        assert_eq!(result.realized_gain(), LotCost::Known(money("100")));
        assert!(
            result
                .assumptions()
                .contains(&crate::LotAssumption::DelistedSettlement)
        );
    }
    fn decimal(value: u64, scale: u32) -> String {
        rust_decimal::Decimal::from_i128_with_scale(i128::from(value), scale).to_string()
    }
    fn realistic_quantities(seed: u64, currency: Currency) -> (u32, [u64; 5], u64) {
        let scale = match currency {
            Currency::Krw => 0,
            Currency::Usd => {
                if seed.checked_rem(2).expect("짝수") == 0 {
                    3
                } else {
                    6
                }
            }
        };
        let first = seed
            .checked_mul(7919)
            .expect("수량 난수")
            .checked_rem(100_000)
            .expect("범위")
            .checked_add(3)
            .expect("양수");
        let second = seed
            .checked_mul(104_729)
            .expect("수량 난수")
            .checked_rem(100_000)
            .expect("범위")
            .checked_add(5)
            .expect("양수");
        let total = first.checked_add(second).expect("합계");
        let sold = total.checked_div(3).expect("첫 매도");
        let next = total
            .checked_sub(sold)
            .expect("잔여")
            .checked_div(2)
            .expect("다음 매도");
        let last = total
            .checked_sub(sold)
            .expect("잔여")
            .checked_sub(next)
            .expect("전량");
        let price = seed
            .checked_mul(1049)
            .expect("가격 난수")
            .checked_rem(1_000_000)
            .expect("범위")
            .checked_add(100)
            .expect("가격");
        (scale, [first, second, sold, next, last], price)
    }
    fn realistic_fee(amount: u64, scale: u32, currency: Currency) -> u64 {
        match currency {
            Currency::Krw => amount
                .checked_mul(15)
                .expect("분자")
                .checked_div(100_000)
                .expect("절사"),
            Currency::Usd => {
                let unit = 10_u64.checked_pow(scale).expect("단위");
                if amount <= 10_u64.checked_mul(unit).expect("면제 경계") {
                    0
                } else {
                    amount
                        .checked_div(unit.checked_mul(10).expect("비율 분모"))
                        .expect("센트 절사")
                }
            }
        }
    }
    fn realistic_values(qty: u64, price: u64, scale: u32, currency: Currency) -> [String; 3] {
        let digits = match currency {
            Currency::Krw => 0,
            Currency::Usd => 2,
        };
        let amount_scale = scale.checked_add(digits).expect("금액 단위");
        let amount = qty.checked_mul(price).expect("대금");
        let fee = realistic_fee(amount, amount_scale, currency);
        [
            decimal(qty, scale),
            decimal(amount, amount_scale),
            decimal(fee, digits),
        ]
    }
    fn realistic_rows(seed: u64, currency: Currency) -> (Vec<Transaction>, Money) {
        let (scale, quantities, price) = realistic_quantities(seed, currency);
        let transactions = quantities
            .into_iter()
            .enumerate()
            .map(|(index, qty)| {
                let day = u8::try_from(index.checked_add(1).expect("날짜")).expect("범위");
                let kind = if index < 2 {
                    TransactionKind::Buy
                } else {
                    TransactionKind::Sell
                };
                let observed_price = if index == 1 {
                    price.checked_add(137).expect("다른 가격")
                } else {
                    price
                };
                realistic_row(
                    day,
                    kind,
                    realistic_values(qty, observed_price, scale, currency),
                    currency,
                )
            })
            .collect::<Vec<_>>();
        let expected = transactions
            .iter()
            .filter(|row| row.data().kind == TransactionKind::Buy)
            .fold(Money::new(money("0").amount(), currency), |total, row| {
                total
                    .checked_add(&row.data().amount.expect("입력 대금"))
                    .expect("대금 합")
                    .checked_add(&row.data().commission)
                    .expect("실제 비용 합")
            });
        (transactions, expected)
    }
    fn realistic_row(
        day: u8,
        kind: TransactionKind,
        values: [String; 3],
        currency: Currency,
    ) -> Transaction {
        let [qty, amount, fee] = values;
        match currency {
            Currency::Krw => charged(day, kind, &qty, [&amount, &fee, "0"]),
            Currency::Usd => usd(day, kind, &qty, [&amount, &fee, "0", "1", "1"]),
        }
    }
    fn currency_identity(currency: Currency) -> crate::LotIdentity {
        match currency {
            Currency::Krw => identity(),
            Currency::Usd => crate::LotIdentity::new(
                InstrumentId::try_new("instrument").expect("식별"),
                Symbol::try_new("AAPL").expect("종목"),
                Market::Us,
                Currency::Usd,
            ),
        }
    }
    fn assert_conservation(result: &LotBook, expected: Money) {
        let consumed = result.realizations().iter().fold(
            LotCost::Known(Money::new(money("0").amount(), expected.currency())),
            |sum, sale| crate::cost_basis::add_cost(sum, sale.consumed_cost()).expect("소진 합"),
        );
        assert_eq!(
            crate::cost_basis::add_cost(consumed, result.total_cost().expect("잔여")),
            Ok(LotCost::Known(expected))
        );
    }
    fn realistic_table(method: CostBasisMethod, currency: Currency) {
        let mut outcomes = Vec::new();
        for seed in 1_u64..=1500 {
            let (transactions, expected) = realistic_rows(seed, currency);
            for prefix in [3_usize, 4, 5] {
                let result = LotBook::from_settled(
                    &rows(transactions.iter().take(prefix).cloned().collect()),
                    currency_identity(currency),
                    method,
                    &[],
                    date(31),
                );
                outcomes.push(result);
                assert_conservation(
                    outcomes
                        .last()
                        .expect("실행 결과")
                        .as_ref()
                        .expect("정상 입력"),
                    expected,
                );
            }
        }
        assert_eq!(
            outcomes.iter().filter(|value| value.is_err()).count(),
            0,
            "현실 입력 1500열의 모든 매도 접두부"
        );
    }
    #[test]
    fn realistic_fifo_krw_1500_sequences_have_zero_errors() {
        realistic_table(CostBasisMethod::Fifo, Currency::Krw);
    }
    #[test]
    fn realistic_fifo_usd_1500_sequences_have_zero_errors() {
        realistic_table(CostBasisMethod::Fifo, Currency::Usd);
    }
    #[test]
    fn realistic_average_krw_1500_sequences_have_zero_errors() {
        realistic_table(CostBasisMethod::MovingAverage, Currency::Krw);
    }
    #[test]
    fn realistic_average_usd_1500_sequences_have_zero_errors() {
        realistic_table(CostBasisMethod::MovingAverage, Currency::Usd);
    }
    fn micro(value: rust_decimal::Decimal) -> u128 {
        value
            .mantissa()
            .unsigned_abs()
            .checked_mul(10_u128.pow(8_u32.checked_sub(value.scale()).expect("8자리 입력")))
            .expect("기준 정수")
    }
    fn reference_consumed(transactions: &[Transaction], method: CostBasisMethod) -> Vec<LotCost> {
        let mut sources = std::collections::VecDeque::<(u128, u128)>::new();
        let mut pool_qty = 0_u128;
        let mut pool_cost = 0_u128;
        let mut result = Vec::new();
        for tx in transactions {
            let data = tx.data();
            let qty = micro(data.quantity.expect("수량").as_decimal());
            let unit = match data.currency {
                Currency::Krw => 100_000_000_u128,
                Currency::Usd => 1_000_000,
            };
            if data.kind == TransactionKind::Buy {
                let cost = micro(data.amount.expect("금액").amount().as_decimal())
                    .checked_add(micro(data.commission.amount().as_decimal()))
                    .expect("수수료")
                    .checked_add(micro(data.tax.amount().as_decimal()))
                    .expect("세금");
                sources.push_back((qty, cost));
                pool_qty = pool_qty.checked_add(qty).expect("기준 합차");
                pool_cost = pool_cost.checked_add(cost).expect("기준 합차");
            } else {
                let used = reference_sale(&mut sources, (qty, unit), method, (pool_qty, pool_cost));
                pool_qty = pool_qty.checked_sub(qty).expect("기준 합차");
                pool_cost = pool_cost.checked_sub(used).expect("기준 합차");
                result.push(LotCost::Known(Money::new(
                    Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                        i128::try_from(used).expect("기준 원가"),
                        8,
                    )),
                    data.currency,
                )));
            }
        }
        result
    }
    fn reference_portion(cost: u128, take: u128, held: u128, unit: u128) -> u128 {
        cost.checked_mul(take)
            .expect("분자")
            .checked_div(held)
            .expect("수량 비율")
            .checked_div(unit)
            .expect("통화 단위 절사")
            .checked_mul(unit)
            .expect("금액 복원")
    }
    fn reference_sale(
        sources: &mut std::collections::VecDeque<(u128, u128)>,
        sale: (u128, u128),
        method: CostBasisMethod,
        pool: (u128, u128),
    ) -> u128 {
        let (qty, unit) = sale;
        if method == CostBasisMethod::MovingAverage {
            return if qty == pool.0 {
                pool.1
            } else {
                reference_portion(pool.1, qty, pool.0, unit)
            };
        }
        let mut remaining = qty;
        let mut consumed = 0_u128;
        while remaining > 0 {
            let (held, cost) = sources.pop_front().expect("보유 초과 없음");
            let take = remaining.min(held);
            let used = if take == held {
                cost
            } else {
                reference_portion(cost, take, held, unit)
            };
            remaining = remaining.checked_sub(take).expect("기준 합차");
            consumed = consumed.checked_add(used).expect("기준 합차");
            if take < held {
                sources.push_front((
                    held.checked_sub(take).expect("수량"),
                    cost.checked_sub(used).expect("원가"),
                ));
            }
        }
        consumed
    }
    fn assert_reference(result: &LotBook, transactions: &[Transaction], method: CostBasisMethod) {
        assert_eq!(
            result
                .realizations()
                .iter()
                .map(crate::LotRealization::consumed_cost)
                .collect::<Vec<_>>(),
            reference_consumed(transactions, method)
        );
    }
    #[test]
    fn corporate_split_inverse_preserves_whole_quantities_and_total_cost() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(1024))
            .run(
                &(1_u64..1000, 2_u64..1000, 1_u64..1000),
                |(lots, new, cost)| {
                    let original = lots.checked_mul(3).expect("정수 배수");
                    let actions = [
                        action(
                            2,
                            "split",
                            CorporateActionKind::Split(ratio(3, new.checked_add(3).expect("비율"))),
                        ),
                        action(
                            3,
                            "reverse",
                            CorporateActionKind::ReverseSplit(ratio(
                                new.checked_add(3).expect("비율"),
                                3,
                            )),
                        ),
                    ];
                    let result = book(
                        vec![row(
                            1,
                            TransactionKind::Buy,
                            &original.to_string(),
                            Some(&cost.to_string()),
                        )],
                        CostBasisMethod::Fifo,
                        &actions,
                    )
                    .expect("정수 권리 왕복");
                    assert_eq!(result.quantity(), Ok(quantity(&original.to_string())));
                    assert_eq!(
                        result.total_cost(),
                        Ok(LotCost::Known(money(&cost.to_string())))
                    );
                    assert_eq!(result.fractional_entitlements(), []);
                    Ok(())
                },
            )
            .expect("분할 병합 왕복");
    }
    #[test]
    fn moving_average_pool_conserves_cost_for_generated_fractional_sales() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(3000))
            .run(
                &(
                    1_u64..100_000,
                    proptest::sample::select(vec![Currency::Krw, Currency::Usd]),
                ),
                |(seed, currency)| {
                    let (transactions, cost) = realistic_rows(seed, currency);
                    let result = LotBook::from_settled(
                        &rows(transactions.clone()),
                        currency_identity(currency),
                        CostBasisMethod::MovingAverage,
                        &[],
                        date(31),
                    )
                    .expect("생성한 이동평균");
                    assert_conservation(&result, cost);
                    assert_reference(&result, &transactions, result.method());
                    assert_eq!(result.quantity().expect("청산"), quantity("0"));
                    Ok(())
                },
            )
            .expect("이동평균 풀 원가 보존");
    }
    #[test]
    fn fifo_generated_nondivisible_fees_and_fractional_quantities_conserve_cost() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(3000))
            .run(
                &(
                    1_u64..100_000,
                    proptest::sample::select(vec![Currency::Krw, Currency::Usd]),
                ),
                |(seed, currency)| {
                    let (transactions, cost) = realistic_rows(seed, currency);
                    let result = LotBook::from_settled(
                        &rows(transactions.clone()),
                        currency_identity(currency),
                        CostBasisMethod::Fifo,
                        &[],
                        date(31),
                    )
                    .expect("생성한 FIFO");
                    assert_conservation(&result, cost);
                    assert_reference(&result, &transactions, result.method());
                    assert_eq!(result.quantity().expect("청산"), quantity("0"));
                    Ok(())
                },
            )
            .expect("FIFO 원가 보존");
    }
    #[test]
    fn us_fractional_corporate_receipts_are_authoritative() {
        for observed in ["1.49", "1.5", "1.51"] {
            let receipt = usd(
                3,
                TransactionKind::SecuritiesTransferIn,
                observed,
                ["0", "0", "0", "1", "3"],
            );
            let event = action(2, "us-split", CorporateActionKind::Split(ratio(1, 3)))
                .with_receipt(receipt.clone())
                .expect("행사 연결");
            let input = rows(vec![
                usd(1, TransactionKind::Buy, "0.5", ["100", "0", "0", "1", "1"]),
                receipt,
            ]);
            let result = LotBook::from_settled(
                &input,
                currency_identity(Currency::Usd),
                CostBasisMethod::Fifo,
                &[event],
                date(31),
            )
            .expect("실제 소수 수량");
            assert_eq!(result.quantity().expect("수량"), quantity(observed));
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(Money::new(money("100").amount(), Currency::Usd))
            );
            assert!(
                result
                    .assumptions()
                    .contains(&crate::LotAssumption::FractionalReceiptQuantity)
            );
        }
    }
    #[test]
    fn stock_dividend_cash_fraction_comes_from_the_new_lot() {
        let event = action(
            2,
            "div",
            CorporateActionKind::StockDividend {
                ratio: ratio(10, 3),
                receipt: None,
                basis: DividendBasis::Zero,
            },
        );
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "15", Some("1500")),
                row(3, TransactionKind::SecuritiesTransferIn, "4", None),
            ],
            CostBasisMethod::Fifo,
            &[event],
        )
        .expect("배당 단주");
        assert_eq!(
            result
                .lots()
                .first()
                .expect("원래 로트")
                .quantity()
                .expect("수량"),
            quantity("15")
        );
        assert_eq!(
            result
                .lots()
                .last()
                .expect("신주 로트")
                .quantity()
                .expect("수량"),
            quantity("4")
        );
        assert_eq!(
            result
                .fractional_entitlements()
                .first()
                .expect("현금 청구")
                .cost(),
            LotCost::Known(money("0"))
        );
    }
    #[test]
    fn delayed_receipt_without_time_does_not_reorder_observed_trades() {
        let mut sale = row(22, TransactionKind::Sell, "1", Some("150"))
            .data()
            .clone();
        sale.transaction_time = Some(at(10));
        let mut purchase = row(22, TransactionKind::Buy, "1", Some("200"))
            .data()
            .clone();
        purchase.transaction_time = Some(at(20));
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "10", Some("1000")),
                row(22, TransactionKind::SecuritiesTransferIn, "2", None),
                Transaction::try_new(purchase).expect("매수"),
                Transaction::try_new(sale).expect("매도"),
            ],
            CostBasisMethod::MovingAverage,
            &[action(
                2,
                "bonus",
                CorporateActionKind::BonusIssue {
                    ratio: ratio(5, 1),
                    receipt: None,
                },
            )],
        )
        .expect("입고와 실제 거래");
        assert_eq!(
            result.total_cost().expect("잔여"),
            LotCost::Known(money("1117"))
        );
        assert_eq!(result.realized_gain(), LotCost::Known(money("67")));
        assert!(
            !result
                .assumptions()
                .contains(&crate::LotAssumption::TransactionOrder)
        );
    }
    #[test]
    fn public_views_preserve_sources_prices_and_states() {
        let purchase = row(1, TransactionKind::Buy, "3", Some("100"));
        let sale = row(2, TransactionKind::Sell, "1", Some("50"));
        for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
            let result = book(vec![purchase.clone(), sale.clone()], method, &[]).expect("원장");
            assert_eq!(result.method(), method);
            assert_eq!(result.identity(), &identity());
            assert_eq!(result.identity().instrument(), identity().instrument());
            assert_eq!(result.identity().symbol(), &symbol());
            assert_eq!(result.identity().market(), &Market::Kr);
            assert_eq!(result.identity().currency(), Currency::Krw);
            assert_eq!(result.pool().cost(), LotCost::Known(money("67")));
            assert_eq!(
                result.pool().rational_quantity().exact_quantity(),
                Ok(quantity("2"))
            );
            let lot = result.lots().first().expect("로트");
            assert_eq!(lot.acquired(), date(1));
            assert_eq!(lot.cost(), LotCost::Known(money("67")));
            assert_eq!(lot.rational_quantity().exact_quantity(), Ok(quantity("2")));
            assert_eq!(lot.tax_status(), crate::LotTaxStatus::Ordinary);
            let price = lot.unit_price().expect("단가").expect("알려짐");
            assert_eq!(price.currency(), Currency::Krw);
            assert_eq!(
                price.quotient(),
                crate::RatioQuotient::Exact(
                    Ratio::parse(&NumericInput::try_new("33.5").expect("입력")).expect("평단")
                )
            );
            assert_eq!(result.pool().unit_price(), Ok(Some(price)));
            assert_eq!(
                result.realizations().first().expect("매도").transaction(),
                &sale
            );
            assert!(
                result
                    .realizations()
                    .first()
                    .expect("매도")
                    .assumptions()
                    .is_empty()
            );
            assert_eq!(result.applied_actions().len(), 0);
        }
    }
    #[test]
    fn public_unknown_and_empty_prices_are_absent() {
        let unknown = book(
            vec![row(1, TransactionKind::SecuritiesTransferIn, "1", None)],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("미상");
        assert_eq!(unknown.lots().first().expect("로트").unit_price(), Ok(None));
        assert_eq!(unknown.pool().unit_price(), Ok(None));
        assert_eq!(
            book(vec![], CostBasisMethod::MovingAverage, &[])
                .expect("빈 원장")
                .pool()
                .unit_price(),
            Ok(None)
        );
        assert_eq!(
            LotError::from(crate::NumericError::NegativeQuantity),
            LotError::InvalidQuantity
        );
    }
    #[test]
    fn constructors_reject_invalid_ratios_identities_and_costs() {
        assert_eq!(ShareCount::try_new(0), Err(LotError::InvalidRatio));
        let id = CorporateActionId::new(InstrumentId::try_new("action").expect("ID"));
        assert_eq!(
            id.instrument_id(),
            &InstrumentId::try_new("action").expect("ID")
        );
        for kind in [
            CorporateActionKind::Split(ratio(1, 1)),
            CorporateActionKind::Split(ratio(2, 1)),
            CorporateActionKind::ReverseSplit(ratio(1, 1)),
            CorporateActionKind::ReverseSplit(ratio(1, 2)),
        ] {
            assert_eq!(
                CorporateAction::try_new(id.clone(), date(2), kind),
                Err(LotError::InvalidRatio)
            );
        }
        assert_eq!(
            CorporateAction::try_new(
                id.clone(),
                date(2),
                CorporateActionKind::SymbolChange {
                    from: symbol(),
                    to: symbol()
                }
            ),
            Err(LotError::InvalidIdentity)
        );
        for value in ["-1", "-0.01"] {
            assert_eq!(
                CorporateAction::try_new(
                    id.clone(),
                    date(2),
                    CorporateActionKind::StockDividend {
                        ratio: ratio(1, 1),
                        receipt: None,
                        basis: DividendBasis::Confirmed(money(value))
                    }
                ),
                Err(LotError::NegativeCost)
            );
        }
        for value in ["0", "-0", "1"] {
            let confirmed = CorporateAction::try_new(
                id.clone(),
                date(2),
                CorporateActionKind::StockDividend {
                    ratio: ratio(1, 1),
                    receipt: None,
                    basis: DividendBasis::Confirmed(money(value)),
                },
            )
            .expect("확인 원가");
            assert_eq!(confirmed.date(), date(2));
        }
    }
    #[test]
    fn duplicate_receipts_and_invalid_market_are_rejected() {
        let receipt = row(3, TransactionKind::SecuritiesTransferIn, "2", None);
        let with = action(
            2,
            "bonus",
            CorporateActionKind::BonusIssue {
                ratio: ratio(5, 1),
                receipt: Some(receipt.clone()),
            },
        );
        assert_eq!(
            with.with_receipt(receipt.clone()),
            Err(LotError::InvalidReceipt)
        );
        let attached = action(2, "split", CorporateActionKind::Split(ratio(1, 2)))
            .with_receipt(receipt.clone())
            .expect("연결");
        assert_eq!(
            attached.with_receipt(receipt),
            Err(LotError::InvalidReceipt)
        );
        let invalid = crate::LotIdentity::new(
            InstrumentId::try_new("instrument").expect("ID"),
            symbol(),
            Market::Kr,
            Currency::Usd,
        );
        assert_eq!(
            LotBook::from_settled(&rows(vec![]), invalid, CostBasisMethod::Fifo, &[], date(31)),
            Err(LotError::InvalidIdentity)
        );
    }
    #[test]
    fn actions_reject_future_duplicate_same_date_and_same_day_trade() {
        let split = action(2, "split", CorporateActionKind::Split(ratio(1, 2)));
        assert_eq!(
            LotBook::from_settled(
                &rows(vec![]),
                identity(),
                CostBasisMethod::Fifo,
                std::slice::from_ref(&split),
                date(1)
            ),
            Err(LotError::FutureAction)
        );
        assert_eq!(
            book(
                vec![],
                CostBasisMethod::Fifo,
                &[split.clone(), split.clone()]
            ),
            Err(LotError::DuplicateAction)
        );
        assert_eq!(
            book(
                vec![],
                CostBasisMethod::Fifo,
                &[
                    split.clone(),
                    action(2, "other", CorporateActionKind::Delisting)
                ]
            ),
            Err(LotError::AmbiguousOrder)
        );
        for kind in [
            TransactionKind::Buy,
            TransactionKind::Sell,
            TransactionKind::SecuritiesTransferIn,
            TransactionKind::SecuritiesTransferOut,
        ] {
            assert_eq!(
                book(
                    vec![row(2, kind, "1", Some("100"))],
                    CostBasisMethod::Fifo,
                    std::slice::from_ref(&split)
                ),
                Err(LotError::AmbiguousOrder)
            );
        }
    }
    #[test]
    fn delisted_and_manual_actions_have_closed_errors() {
        let first = action(2, "end", CorporateActionKind::Delisting);
        assert_eq!(
            book(
                vec![],
                CostBasisMethod::Fifo,
                &[
                    first.clone(),
                    action(3, "split", CorporateActionKind::Split(ratio(1, 2)))
                ]
            ),
            Err(LotError::Delisted)
        );
        assert_eq!(
            book(
                vec![row(3, TransactionKind::Buy, "1", Some("100"))],
                CostBasisMethod::Fifo,
                &[first]
            ),
            Err(LotError::Delisted)
        );
        for kind in [
            CorporateActionKind::Merger,
            CorporateActionKind::Demerger,
            CorporateActionKind::Exchange,
        ] {
            assert_eq!(
                book(vec![], CostBasisMethod::Fifo, &[action(2, "manual", kind)]),
                Err(LotError::ManualActionRequired)
            );
        }
    }
    #[test]
    fn symbol_history_requires_connected_acyclic_chain_and_accepts_every_alias() {
        let middle = Symbol::try_new("NEW").expect("심볼");
        let last = Symbol::try_new("LAST").expect("심볼");
        let changes = [
            action(
                2,
                "first",
                CorporateActionKind::SymbolChange {
                    from: symbol(),
                    to: middle.clone(),
                },
            ),
            action(
                4,
                "second",
                CorporateActionKind::SymbolChange {
                    from: middle.clone(),
                    to: last.clone(),
                },
            ),
        ];
        let mut midrow = row(3, TransactionKind::Buy, "2", Some("250"))
            .data()
            .clone();
        midrow.symbol = Some(middle.clone());
        let input = rows(vec![
            row(1, TransactionKind::Buy, "1", Some("100")),
            Transaction::try_new(midrow).expect("거래"),
        ]);
        for alias in [symbol(), middle, last.clone()] {
            let target = crate::LotIdentity::new(
                InstrumentId::try_new("instrument").expect("ID"),
                alias,
                Market::Kr,
                Currency::Krw,
            );
            let result =
                LotBook::from_settled(&input, target, CostBasisMethod::Fifo, &changes, date(31))
                    .expect("이어짐");
            assert_eq!(result.identity().symbol(), &last);
            assert_eq!(result.quantity(), Ok(quantity("3")));
            assert_eq!(result.total_cost(), Ok(LotCost::Known(money("350"))));
        }
    }
    #[test]
    fn symbol_history_rejects_disconnected_cycle_and_unrelated_anchor() {
        let middle = Symbol::try_new("NEW").expect("심볼");
        let last = Symbol::try_new("LAST").expect("심볼");
        let changes = [
            action(
                2,
                "first",
                CorporateActionKind::SymbolChange {
                    from: symbol(),
                    to: middle.clone(),
                },
            ),
            action(
                4,
                "second",
                CorporateActionKind::SymbolChange {
                    from: middle.clone(),
                    to: last.clone(),
                },
            ),
        ];
        let input = rows(vec![]);
        for next in [
            CorporateActionKind::SymbolChange {
                from: symbol(),
                to: last,
            },
            CorporateActionKind::SymbolChange {
                from: middle,
                to: symbol(),
            },
        ] {
            assert_eq!(
                book(
                    vec![],
                    CostBasisMethod::Fifo,
                    &[changes[0].clone(), action(4, "bad", next)]
                ),
                Err(LotError::InvalidIdentity)
            );
        }
        let isolated = crate::LotIdentity::new(
            InstrumentId::try_new("instrument").expect("ID"),
            Symbol::try_new("OTHER").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        assert_eq!(
            LotBook::from_settled(&input, isolated, CostBasisMethod::Fifo, &changes, date(31)),
            Err(LotError::InvalidIdentity)
        );
    }
    #[test]
    fn cash_claims_delivery_accessors_and_pending_receipts_are_explicit() {
        let receipt = row(22, TransactionKind::SecuritiesTransferIn, "4", None);
        let bonus = action(
            2,
            "bonus",
            CorporateActionKind::BonusIssue {
                ratio: ratio(10, 3),
                receipt: None,
            },
        );
        let input = rows(vec![
            row(1, TransactionKind::Buy, "15", Some("1500")),
            receipt.clone(),
        ]);
        for (asof, status) in [
            (date(2), crate::DeliveryStatus::Pending),
            (date(31), crate::DeliveryStatus::Received),
        ] {
            let result = LotBook::from_settled(
                &input,
                identity(),
                CostBasisMethod::Fifo,
                std::slice::from_ref(&bonus),
                asof,
            )
            .expect("지연 입고");
            assert_eq!(result.quantity(), Ok(quantity("19")));
            assert_eq!(result.total_cost(), Ok(LotCost::Known(money("1500"))));
            let claim = result.fractional_entitlements().first().expect("단주");
            assert_eq!(claim.action(), bonus.id());
            assert_eq!(claim.quantity(), Ok(quantity("0.5")));
            assert_eq!(
                claim.rational_quantity().exact_quantity(),
                Ok(quantity("0.5"))
            );
            let delivery = result.deliveries().first().expect("입고");
            assert_eq!(delivery.action(), bonus.id());
            assert_eq!(
                delivery.rational_quantity().exact_quantity(),
                Ok(quantity("4"))
            );
            assert_eq!(delivery.receipt(), Some(&receipt));
            assert_eq!(delivery.status(), status);
            assert_eq!(result.applied_actions().len(), 1);
        }
    }
    #[test]
    fn pending_and_zero_entitlements_have_explicit_delivery_states() {
        let pending = book(
            vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "pending",
                CorporateActionKind::BonusIssue {
                    ratio: ratio(5, 1),
                    receipt: None,
                },
            )],
        )
        .expect("입고 전");
        assert_eq!(pending.quantity(), Ok(quantity("12")));
        assert_eq!(
            pending.deliveries().first().expect("입고").status(),
            crate::DeliveryStatus::Pending
        );
        let empty = book(
            vec![],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "empty",
                CorporateActionKind::BonusIssue {
                    ratio: ratio(5, 1),
                    receipt: None,
                },
            )],
        )
        .expect("권리 없음");
        assert_eq!(
            empty.deliveries().first().expect("입고").status(),
            crate::DeliveryStatus::NotReported
        );
    }
    #[test]
    fn receipt_matching_rejects_ambiguous_wrong_reused_and_late_rows() {
        let purchase = row(1, TransactionKind::Buy, "10", Some("1000"));
        let receipt = row(22, TransactionKind::SecuritiesTransferIn, "2", None);
        let bonus = action(
            2,
            "bonus",
            CorporateActionKind::BonusIssue {
                ratio: ratio(5, 1),
                receipt: None,
            },
        );
        assert_eq!(
            book(
                vec![
                    purchase.clone(),
                    receipt.clone(),
                    row(23, TransactionKind::SecuritiesTransferIn, "2", None)
                ],
                CostBasisMethod::Fifo,
                std::slice::from_ref(&bonus)
            ),
            Err(LotError::InvalidReceipt)
        );
        let wrong = row(22, TransactionKind::SecuritiesTransferIn, "3", None);
        assert_eq!(
            book(
                vec![purchase.clone(), wrong.clone()],
                CostBasisMethod::Fifo,
                &[bonus.clone().with_receipt(wrong).expect("명시")]
            ),
            Err(LotError::InvalidReceipt)
        );
        assert_eq!(
            book(
                vec![purchase.clone()],
                CostBasisMethod::Fifo,
                &[bonus.clone().with_receipt(receipt).expect("명시")]
            ),
            Err(LotError::InvalidReceipt)
        );
        assert_eq!(
            book(
                vec![
                    purchase,
                    row(22, TransactionKind::SecuritiesTransferIn, "3", None)
                ],
                CostBasisMethod::Fifo,
                std::slice::from_ref(&bonus)
            ),
            Err(LotError::InvalidReceipt)
        );
    }
    #[test]
    fn receipts_cannot_be_reused_or_linked_beyond_window() {
        let purchase = row(1, TransactionKind::Buy, "10", Some("1000"));
        let receipt = row(22, TransactionKind::SecuritiesTransferIn, "2", None);
        let bonus = action(
            2,
            "bonus",
            CorporateActionKind::BonusIssue {
                ratio: ratio(5, 1),
                receipt: None,
            },
        );
        let next = action(
            3,
            "dividend",
            CorporateActionKind::StockDividend {
                ratio: ratio(6, 1),
                receipt: Some(receipt.clone()),
                basis: DividendBasis::Zero,
            },
        );
        assert_eq!(
            book(
                vec![purchase.clone(), receipt.clone()],
                CostBasisMethod::Fifo,
                &[bonus, next]
            ),
            Err(LotError::InvalidReceipt)
        );
        let mut late_data = receipt.data().clone();
        late_data.settlement_date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(5),
            DayNumber::new(3),
        )
        .expect("날짜");
        let late = Transaction::try_new(late_data).expect("거래");
        let split = action(2, "late", CorporateActionKind::Split(ratio(1, 2)))
            .with_receipt(late.clone())
            .expect("명시");
        assert_eq!(
            book(vec![purchase, late], CostBasisMethod::Fifo, &[split]),
            Err(LotError::InvalidReceipt)
        );
    }
    #[test]
    fn dividend_basis_has_zero_confirmed_unknown_and_currency_contracts() {
        for (basis, expected) in [
            (DividendBasis::Zero, LotCost::Known(money("1000"))),
            (
                DividendBasis::Confirmed(money("100")),
                LotCost::Known(money("1100")),
            ),
            (DividendBasis::Unknown, LotCost::Unknown),
        ] {
            let result = book(
                vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "dividend",
                    CorporateActionKind::StockDividend {
                        ratio: ratio(5, 1),
                        receipt: None,
                        basis,
                    },
                )],
            )
            .expect("배당");
            assert_eq!(result.quantity(), Ok(quantity("12")));
            assert_eq!(result.total_cost(), Ok(expected));
            assert_eq!(
                result.lots().last().expect("배당 로트").tax_status(),
                crate::LotTaxStatus::NeedsConfirmation
            );
        }
    }
    #[test]
    fn dividend_cost_currency_and_empty_entitlement_are_checked() {
        let dollar = Money::new(money("100").amount(), Currency::Usd);
        assert_eq!(
            book(
                vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "dividend",
                    CorporateActionKind::StockDividend {
                        ratio: ratio(5, 1),
                        receipt: None,
                        basis: DividendBasis::Confirmed(dollar)
                    }
                )]
            ),
            Err(LotError::CurrencyMismatch)
        );
        let empty = book(
            vec![],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "dividend",
                CorporateActionKind::StockDividend {
                    ratio: ratio(5, 1),
                    receipt: None,
                    basis: DividendBasis::Zero,
                },
            )],
        )
        .expect("권리 없음");
        assert_eq!(empty.quantity(), Ok(quantity("0")));
    }
    #[test]
    fn missing_data_oversells_transfers_and_cash_transactions_have_closed_outcomes() {
        for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
            assert_eq!(
                book(
                    vec![
                        row(1, TransactionKind::Buy, "1", Some("100")),
                        row(2, TransactionKind::Sell, "2", Some("200"))
                    ],
                    method,
                    &[]
                ),
                Err(LotError::InsufficientQuantity)
            );
            for missing in [
                row(1, TransactionKind::Buy, "1", None),
                row(1, TransactionKind::Buy, "0", Some("100")),
            ] {
                assert_eq!(book(vec![missing], method, &[]), Err(LotError::MissingData));
            }
            assert_eq!(
                book(
                    vec![
                        row(1, TransactionKind::Buy, "2", Some("100")),
                        row(2, TransactionKind::Sell, "1", None)
                    ],
                    method,
                    &[]
                ),
                Err(LotError::MissingData)
            );
        }
    }
    #[test]
    fn external_transfers_cash_and_wrong_currency_are_classified() {
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "2", Some("100")),
                row(2, TransactionKind::SecuritiesTransferOut, "1", None),
            ],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("출고");
        assert_eq!(result.quantity(), Ok(quantity("1")));
        assert_eq!(result.realizations(), []);
        let mut cash = Vec::new();
        for kind in [
            TransactionKind::Deposit,
            TransactionKind::Withdrawal,
            TransactionKind::ExchangeIn,
            TransactionKind::ExchangeOut,
            TransactionKind::Dividend,
            TransactionKind::Interest,
            TransactionKind::Fee,
        ] {
            cash.push(row(1, kind, "1", Some("100")));
        }
        assert_eq!(
            book(cash, CostBasisMethod::Fifo, &[])
                .expect("현금")
                .quantity(),
            Ok(quantity("0"))
        );
        let unknown = TransactionKind::new(crate::TransactionCode::try_new("NEW").expect("코드"));
        assert_eq!(
            book(vec![row(1, unknown, "1", None)], CostBasisMethod::Fifo, &[]),
            Err(LotError::MissingData)
        );
        let mut foreign = row(1, TransactionKind::Buy, "1", Some("100"))
            .data()
            .clone();
        foreign.currency = Currency::Usd;
        foreign.amount = Some(Money::new(money("100").amount(), Currency::Usd));
        foreign.commission = Money::new(money("0").amount(), Currency::Usd);
        foreign.tax = foreign.commission;
        assert_eq!(
            book(
                vec![Transaction::try_new(foreign).expect("통화")],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::CurrencyMismatch)
        );
    }
    fn assert_arithmetic_rows(
        transactions: Vec<Transaction>,
        method: CostBasisMethod,
        error: crate::MoneyError,
    ) {
        assert_eq!(
            book(transactions, method, &[]),
            Err(LotError::Arithmetic(error))
        );
    }
    #[test]
    fn exact_amount_boundaries_propagate_through_purchase_and_sale() {
        let max = "79228162514264337593543950335";
        for values in [
            ["79228162514264337593543950334", "0.1", "0"],
            ["79228162514264337593543950334", "0", "0.1"],
        ] {
            assert_arithmetic_rows(
                vec![charged(1, TransactionKind::Buy, "1", values)],
                CostBasisMethod::Fifo,
                crate::MoneyError::PrecisionLoss,
            );
        }
        assert_arithmetic_rows(
            vec![
                row(1, TransactionKind::Buy, "1", Some(max)),
                row(2, TransactionKind::Buy, "1", Some("1")),
            ],
            CostBasisMethod::Fifo,
            crate::MoneyError::Overflow,
        );
        for values in [[max, "0.1", "0"], [max, "0", "0.1"], ["0", max, max]] {
            assert_arithmetic_rows(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("1")),
                    charged(2, TransactionKind::Sell, "1", values),
                ],
                CostBasisMethod::Fifo,
                if values[2] == max {
                    crate::MoneyError::Overflow
                } else {
                    crate::MoneyError::PrecisionLoss
                },
            );
        }
        assert_arithmetic_rows(
            vec![
                row(1, TransactionKind::Buy, "1", Some(max)),
                charged(2, TransactionKind::Sell, "1", ["0", "1", "0"]),
            ],
            CostBasisMethod::Fifo,
            crate::MoneyError::Overflow,
        );
        assert_arithmetic_rows(
            vec![
                row(1, TransactionKind::Buy, "1", Some("0")),
                row(2, TransactionKind::Sell, "1", Some(max)),
                row(3, TransactionKind::Buy, "1", Some("0")),
                row(4, TransactionKind::Sell, "1", Some(max)),
            ],
            CostBasisMethod::Fifo,
            crate::MoneyError::Overflow,
        );
    }
    fn dollar_book(
        transactions: Vec<Transaction>,
        method: CostBasisMethod,
        actions: &[CorporateAction],
    ) -> Result<LotBook, LotError> {
        LotBook::from_settled(
            &rows(transactions),
            currency_identity(Currency::Usd),
            method,
            actions,
            date(31),
        )
    }
    #[test]
    fn fractional_currency_precision_failures_remain_explicit() {
        let max = "79228162514264337593543950335";
        for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
            assert_eq!(
                dollar_book(
                    vec![
                        usd(1, TransactionKind::Buy, "11", [max, "0", "0", "1", "1"]),
                        usd(2, TransactionKind::Sell, "2", ["100", "0", "0", "1", "2"])
                    ],
                    method,
                    &[]
                ),
                Err(LotError::Arithmetic(crate::MoneyError::PrecisionLoss))
            );
            assert_eq!(
                dollar_book(
                    vec![
                        usd(1, TransactionKind::Buy, max, [max, "0", "0", "1", "1"]),
                        usd(2, TransactionKind::Sell, "0.5", ["100", "0", "0", "1", "2"])
                    ],
                    method,
                    &[]
                ),
                Err(LotError::Arithmetic(crate::MoneyError::PrecisionLoss))
            );
        }
    }
    #[test]
    fn fx_absence_nonpositive_and_unrepresentable_results_are_errors() {
        let target =
            currency_identity(Currency::Usd).with_cost_currency(LotCostCurrency::SettlementKrw);
        for rate in [None, Some("0"), Some("-1")] {
            let mut data = usd(1, TransactionKind::Buy, "1", ["100", "0", "0", "1", "1"])
                .data()
                .clone();
            data.exchange_rate = rate.map(|value| {
                Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("환율")
            });
            assert_eq!(
                LotBook::from_settled(
                    &rows(vec![Transaction::try_new(data).expect("거래")]),
                    target.clone(),
                    CostBasisMethod::Fifo,
                    &[],
                    date(31)
                ),
                Err(LotError::MissingData)
            );
        }
        for (amount, rate, error) in [
            (
                "79228162514264337593543950335",
                "2",
                crate::MoneyError::Overflow,
            ),
            (
                "0.0000000000000000000000000001",
                "0.1",
                crate::MoneyError::PrecisionLoss,
            ),
        ] {
            assert_eq!(
                LotBook::from_settled(
                    &rows(vec![usd(
                        1,
                        TransactionKind::Buy,
                        "1",
                        [amount, "0", "0", rate, "1"]
                    )]),
                    target.clone(),
                    CostBasisMethod::Fifo,
                    &[],
                    date(31)
                ),
                Err(LotError::Arithmetic(error))
            );
        }
    }
    #[test]
    fn sale_fx_is_required_independently_of_purchase_fx() {
        let target =
            currency_identity(Currency::Usd).with_cost_currency(LotCostCurrency::SettlementKrw);
        let mut sale = usd(2, TransactionKind::Sell, "1", ["150", "0", "0", "1", "2"])
            .data()
            .clone();
        sale.exchange_rate = None;
        assert_eq!(
            LotBook::from_settled(
                &rows(vec![
                    usd(1, TransactionKind::Buy, "1", ["100", "0", "0", "1", "1"]),
                    Transaction::try_new(sale).expect("매도")
                ]),
                target,
                CostBasisMethod::Fifo,
                &[],
                date(31)
            ),
            Err(LotError::MissingData)
        );
    }
    #[test]
    fn fractional_us_receipt_requires_observation_and_bounded_quantity() {
        let purchase = usd(1, TransactionKind::Buy, "0.75", ["100", "0", "0", "1", "1"]);
        let split = action(2, "split", CorporateActionKind::Split(ratio(1, 2)));
        assert_eq!(
            dollar_book(
                vec![purchase.clone()],
                CostBasisMethod::Fifo,
                std::slice::from_ref(&split)
            ),
            Err(LotError::MissingData)
        );
        for observed in ["0.99", "2.01", "0"] {
            let receipt = usd(
                3,
                TransactionKind::SecuritiesTransferIn,
                observed,
                ["0", "0", "0", "1", "1"],
            );
            assert_eq!(
                dollar_book(
                    vec![purchase.clone(), receipt.clone()],
                    CostBasisMethod::Fifo,
                    &[split.clone().with_receipt(receipt).expect("명시")]
                ),
                Err(if observed == "0" {
                    LotError::MissingData
                } else {
                    LotError::InvalidReceipt
                })
            );
        }
        let receipt = usd(
            3,
            TransactionKind::SecuritiesTransferIn,
            "1.49",
            ["0", "0", "0", "1", "1"],
        );
        let result = dollar_book(
            vec![purchase, receipt.clone()],
            CostBasisMethod::Fifo,
            &[split],
        )
        .expect("소수 입고 자동 연결");
        assert_eq!(result.quantity(), Ok(quantity("1.49")));
        assert_eq!(
            result.deliveries().first().expect("입고").receipt(),
            Some(&receipt)
        );
    }
    #[test]
    fn split_receipt_mismatch_and_quantity_overflow_are_errors() {
        let max = "79228162514264337593543950335";
        for explicit in [
            None,
            Some(row(3, TransactionKind::SecuritiesTransferIn, "1", None)),
        ] {
            let mut split = action(2, "split", CorporateActionKind::Split(ratio(1, 2)));
            let mut input = vec![row(1, TransactionKind::Buy, max, Some("100"))];
            if let Some(receipt) = explicit {
                input.push(receipt.clone());
                split = split.with_receipt(receipt).expect("입고");
            }
            assert_eq!(
                book(input, CostBasisMethod::Fifo, &[split]),
                Err(LotError::Arithmetic(crate::MoneyError::Overflow))
            );
        }
        let receipt = row(3, TransactionKind::SecuritiesTransferIn, "0", None);
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "10", Some("100")),
                    receipt.clone()
                ],
                CostBasisMethod::Fifo,
                &[action(2, "split", CorporateActionKind::Split(ratio(1, 2)))
                    .with_receipt(receipt)
                    .expect("명시")]
            ),
            Err(LotError::MissingData)
        );
    }
    #[test]
    fn fifo_interleaved_fractional_buys_and_sales_match_independent_queue() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(1024))
            .run(
                &proptest::collection::vec((1_u64..1000, 1_u64..1_000_000, 1_u64..1000), 1..8),
                |steps| {
                    let mut input = Vec::new();
                    let mut held = 0_u64;
                    for (index, (bought, cost, requested)) in steps.into_iter().enumerate() {
                        let day = u8::try_from(index.checked_mul(2).expect("날짜"))
                            .expect("범위")
                            .checked_add(1)
                            .expect("매수일");
                        held = held.checked_add(bought).expect("취득");
                        let sold = held.min(requested);
                        held = held.checked_sub(sold).expect("처분");
                        input.push(usd(
                            day,
                            TransactionKind::Buy,
                            &decimal(bought, 3),
                            [&decimal(cost, 4), "0.03", "0", "1", "1"],
                        ));
                        input.push(usd(
                            day.checked_add(1).expect("매도일"),
                            TransactionKind::Sell,
                            &decimal(sold, 3),
                            ["100", "0", "0", "1", "1"],
                        ));
                    }
                    for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
                        let result = dollar_book(input.clone(), method, &[]).expect("교차 매매");
                        assert_reference(&result, &input, method);
                        assert_eq!(result.quantity(), Ok(quantity(&decimal(held, 3))));
                    }
                    Ok(())
                },
            )
            .expect("독립 큐와 교차 순서 비교");
    }
    #[test]
    fn late_unlinked_replacement_receipt_is_not_an_external_deposit() {
        let late_date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(5),
            DayNumber::new(3),
        )
        .expect("상한 뒤");
        for qty in [Some(quantity("20")), None] {
            let mut data = row(3, TransactionKind::SecuritiesTransferIn, "20", None)
                .data()
                .clone();
            data.settlement_date = late_date;
            data.quantity = qty;
            assert_eq!(
                LotBook::from_settled(
                    &rows(vec![
                        row(1, TransactionKind::Buy, "10", Some("100")),
                        Transaction::try_new(data).expect("입고")
                    ]),
                    identity(),
                    CostBasisMethod::Fifo,
                    &[action(2, "split", CorporateActionKind::Split(ratio(1, 2)))],
                    late_date
                ),
                Err(if qty.is_some() {
                    LotError::InvalidReceipt
                } else {
                    LotError::MissingData
                })
            );
        }
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("100")),
                    row(2, TransactionKind::Sell, "0", Some("100"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::MissingData)
        );
    }
    #[test]
    fn price_views_do_not_hide_unrepresentable_unit_prices() {
        let result = book(
            vec![row(
                1,
                TransactionKind::Buy,
                "0.1",
                Some("79228162514264337593543950335"),
            )],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("총원가 정본");
        assert_eq!(
            result.lots().first().expect("로트").unit_price(),
            Err(LotError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            result.pool().unit_price(),
            Err(LotError::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn errors_before_an_action_and_unknown_pool_extremes_propagate() {
        assert_eq!(
            book(
                vec![row(1, TransactionKind::Buy, "1", None)],
                CostBasisMethod::Fifo,
                &[action(2, "split", CorporateActionKind::Split(ratio(1, 2)))]
            ),
            Err(LotError::MissingData)
        );
        let max = "79228162514264337593543950335";
        let input = vec![
            row(1, TransactionKind::SecuritiesTransferIn, "1", None),
            row(2, TransactionKind::Buy, "1", Some(max)),
            row(3, TransactionKind::Buy, "1", Some(max)),
            row(4, TransactionKind::Sell, "1", Some("100")),
        ];
        assert_arithmetic_rows(
            input.clone(),
            CostBasisMethod::Fifo,
            crate::MoneyError::Overflow,
        );
        let mut with_sale = input;
        with_sale.push(row(5, TransactionKind::Sell, "2", Some("100")));
        assert_arithmetic_rows(
            with_sale,
            CostBasisMethod::MovingAverage,
            crate::MoneyError::Overflow,
        );
        assert_eq!(
            dollar_book(
                vec![
                    usd(1, TransactionKind::Buy, "1", ["0", "0", "0", "1", "1"]),
                    usd(2, TransactionKind::Buy, max, [max, "0", "0", "1", "2"]),
                    usd(3, TransactionKind::Sell, "0.5", ["100", "0", "0", "1", "3"])
                ],
                CostBasisMethod::MovingAverage,
                &[]
            ),
            Err(LotError::Arithmetic(crate::MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn corporate_cash_cost_and_dividend_acquisition_do_not_hide_precision_errors() {
        let max = "79228162514264337593543950335";
        for kind in [
            CorporateActionKind::ReverseSplit(ratio(3, 2)),
            CorporateActionKind::BonusIssue {
                ratio: ratio(3, 1),
                receipt: None,
            },
        ] {
            assert_eq!(
                dollar_book(
                    vec![usd(1, TransactionKind::Buy, "2", [max, "0", "0", "1", "1"])],
                    CostBasisMethod::Fifo,
                    &[action(2, "cash", kind)]
                ),
                Err(LotError::Arithmetic(crate::MoneyError::PrecisionLoss))
            );
        }
        let confirmed = Money::new(money("1").amount(), Currency::Usd);
        assert_eq!(
            dollar_book(
                vec![usd(1, TransactionKind::Buy, "2", [max, "0", "0", "1", "1"])],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "dividend",
                    CorporateActionKind::StockDividend {
                        ratio: ratio(1, 1),
                        receipt: None,
                        basis: DividendBasis::Confirmed(confirmed)
                    }
                )]
            ),
            Err(LotError::Arithmetic(crate::MoneyError::Overflow))
        );
        let basis = Money::new(money(max).amount(), Currency::Usd);
        assert_eq!(
            dollar_book(
                vec![usd(1, TransactionKind::Buy, "1", ["0", "0", "0", "1", "1"])],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "dividend",
                    CorporateActionKind::StockDividend {
                        ratio: ratio(3, 4),
                        receipt: None,
                        basis: DividendBasis::Confirmed(basis)
                    }
                )]
            ),
            Err(LotError::Arithmetic(crate::MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn fractional_us_gross_quantity_must_fit_the_observed_boundary() {
        let receipt = usd(
            3,
            TransactionKind::SecuritiesTransferIn,
            "1",
            ["0", "0", "0", "1", "3"],
        );
        let split = action(2, "split", CorporateActionKind::Split(ratio(11, 120)))
            .with_receipt(receipt.clone())
            .expect("입고");
        assert_eq!(
            dollar_book(
                vec![
                    usd(
                        1,
                        TransactionKind::Buy,
                        "7922816251426433759354395033.5",
                        ["100", "0", "0", "1", "1"]
                    ),
                    receipt
                ],
                CostBasisMethod::Fifo,
                &[split]
            ),
            Err(LotError::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn receipt_window_includes_effective_day_and_day_ninety() {
        let end = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(4),
            DayNumber::new(2),
        )
        .expect("90일 뒤");
        for received in [date(2), end] {
            let mut data = row(2, TransactionKind::SecuritiesTransferIn, "2", None)
                .data()
                .clone();
            data.settlement_date = received;
            let result = LotBook::from_settled(
                &rows(vec![
                    row(1, TransactionKind::Buy, "10", Some("1000")),
                    Transaction::try_new(data).expect("입고"),
                ]),
                identity(),
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "bonus",
                    CorporateActionKind::BonusIssue {
                        ratio: ratio(5, 1),
                        receipt: None,
                    },
                )],
                end,
            )
            .expect("포함 경계");
            assert_eq!(result.quantity(), Ok(quantity("12")));
            assert_eq!(
                result.deliveries().first().expect("입고").status(),
                crate::DeliveryStatus::Received
            );
        }
    }
    fn stamped(transaction: &Transaction, second: i64) -> Transaction {
        let mut data = transaction.data().clone();
        data.transaction_time = Some(at(second));
        Transaction::try_new(data).expect("시각")
    }
    #[test]
    fn cancelled_original_tombstone_and_pending_never_create_lots() {
        let at = Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let mut cancelled = row(1, TransactionKind::Buy, "10", Some("1000"))
            .data()
            .clone();
        cancelled.cancellation = Cancellation::Cancelled;
        let mut pending = row(2, TransactionKind::Buy, "10", Some("1000"))
            .data()
            .clone();
        pending.settlement = Settlement::Pending;
        let state = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(
                    row(1, TransactionKind::Buy, "10", Some("1000")),
                    at,
                    Some(at),
                )
                .expect("사라진 원본"),
                StoredTransaction::try_restore(
                    Transaction::try_new(cancelled).expect("취소"),
                    at,
                    None,
                )
                .expect("취소 이력"),
            ],
            TransactionBatch::try_new(vec![Transaction::try_new(pending).expect("미결제")])
                .expect("임시"),
            Some(at),
        )
        .expect("결제 멀티셋");
        let result =
            LotBook::from_settled(&state, identity(), CostBasisMethod::Fifo, &[], date(31))
                .expect("계산");
        assert_eq!(result.lots(), []);
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("0"))
        );
    }
    #[test]
    fn calculation_filters_exclude_foreign_assets_symbols_and_future_rows() {
        let mut foreign = row(1, TransactionKind::Buy, "1", None).data().clone();
        foreign.market = Market::Us;
        foreign.currency = Currency::Usd;
        foreign.commission = Money::new(money("0").amount(), Currency::Usd);
        foreign.tax = foreign.commission;
        let mut bond = row(1, TransactionKind::Buy, "1", Some("1")).data().clone();
        bond.asset_class = Some(AssetClass::Bond);
        let mut other = row(1, TransactionKind::Buy, "1", Some("1")).data().clone();
        other.symbol = Some(Symbol::try_new("000660").expect("다른 종목"));
        for data in [foreign, bond, other] {
            assert_eq!(
                book(
                    vec![Transaction::try_new(data).expect("행")],
                    CostBasisMethod::Fifo,
                    &[]
                )
                .expect("대상 아님")
                .lots(),
                []
            );
        }
        let input = rows(vec![row(31, TransactionKind::Buy, "1", Some("1"))]);
        let result =
            LotBook::from_settled(&input, identity(), CostBasisMethod::Fifo, &[], date(30))
                .expect("미래 결제 제외");
        assert_eq!(result.lots(), []);
    }
    #[test]
    fn indistinguishable_multiset_rows_preserve_multiplicity() {
        for method in [CostBasisMethod::Fifo, CostBasisMethod::MovingAverage] {
            let buy = row(1, TransactionKind::Buy, "1", Some("100"));
            let result = book(
                vec![
                    buy.clone(),
                    buy,
                    row(2, TransactionKind::Sell, "1", Some("300")),
                ],
                method,
                &[],
            )
            .expect("동일한 두 행");
            assert_eq!(result.quantity().expect("수량"), quantity("1"));
            assert_eq!(
                result.total_cost().expect("잔여 원가"),
                LotCost::Known(money("100"))
            );
            assert_eq!(result.realized_gain(), LotCost::Known(money("200")));
        }
    }
    #[test]
    fn same_day_distinct_times_determine_fifo_independent_of_input_order() {
        let old = stamped(&row(1, TransactionKind::Buy, "1", Some("200")), 1);
        let recent = stamped(&row(1, TransactionKind::Buy, "1", Some("100")), 2);
        let result = book(
            vec![recent, old, row(2, TransactionKind::Sell, "1", Some("300"))],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("관측한 시각 순서");
        assert_eq!(
            result.total_cost().expect("잔여 원가"),
            LotCost::Known(money("100"))
        );
        assert_eq!(result.realized_gain(), LotCost::Known(money("100")));
    }
    #[test]
    fn as_of_date_is_inclusive_for_actions_and_transactions() {
        let event = action(31, "today", CorporateActionKind::Delisting);
        let result = book(
            vec![row(1, TransactionKind::Buy, "1", Some("1"))],
            CostBasisMethod::Fifo,
            &[event],
        )
        .expect("오늘 행사");
        assert_eq!(result.listing(), LotListing::Delisted);
        assert_eq!(
            book(
                vec![row(31, TransactionKind::Buy, "1", Some("1"))],
                CostBasisMethod::Fifo,
                &[]
            )
            .expect("오늘 결제")
            .quantity()
            .expect("수량"),
            quantity("1")
        );
    }
    #[test]
    fn r11_example_two_every_moving_average_snapshot_matches_d05() {
        let trades = [
            row(1, TransactionKind::Buy, "10", Some("1000")),
            row(2, TransactionKind::Buy, "10", Some("1200")),
            row(3, TransactionKind::Sell, "5", Some("650")),
            row(4, TransactionKind::Buy, "5", Some("550")),
            row(5, TransactionKind::Sell, "10", Some("1250")),
        ];
        for (count, expected) in [
            (1_usize, "1000"),
            (2, "2200"),
            (3, "1650"),
            (4, "2200"),
            (5, "1100"),
        ] {
            assert_eq!(
                book(
                    trades.iter().take(count).cloned().collect(),
                    CostBasisMethod::MovingAverage,
                    &[]
                )
                .expect("단계")
                .total_cost()
                .expect("원가"),
                LotCost::Known(money(expected))
            );
        }
    }
    #[test]
    fn fifo_same_instant_fills_are_deterministic_and_mark_assumptions() {
        let low = stamped(&row(1, TransactionKind::Buy, "1", Some("100")), 1);
        let high = stamped(&row(1, TransactionKind::Buy, "1", Some("200")), 1);
        let sell = stamped(&row(1, TransactionKind::Sell, "1", Some("150")), 1);
        for input in [
            vec![sell.clone(), high.clone(), low.clone()],
            vec![low, high, sell],
        ] {
            let result = book(input, CostBasisMethod::Fifo, &[]).expect("같은 시각");
            assert_eq!(result.total_cost(), Ok(LotCost::Known(money("200"))));
            assert_eq!(result.realized_gain(), LotCost::Known(money("50")));
            assert!(
                result
                    .assumptions()
                    .contains(&crate::LotAssumption::TransactionOrder)
            );
            assert!(
                result
                    .realizations()
                    .first()
                    .expect("손익")
                    .assumptions()
                    .contains(&crate::LotAssumption::TransactionOrder)
            );
        }
        let result = dollar_book(
            vec![usd(
                1,
                TransactionKind::Buy,
                "1",
                ["100", "0", "0", "1", "1"],
            )],
            CostBasisMethod::MovingAverage,
            &[],
        )
        .expect("달러 풀");
        assert_eq!(
            result
                .pool()
                .unit_price()
                .expect("단가")
                .expect("알려짐")
                .currency(),
            Currency::Usd
        );
    }
    #[test]
    fn bonus_keeps_acquisition_date_and_marks_tax_basis_for_confirmation() {
        let result = book(
            vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "bonus",
                CorporateActionKind::BonusIssue {
                    ratio: ratio(5, 1),
                    receipt: None,
                },
            )],
        )
        .expect("무상증자");
        let lot = result.lots().first().expect("취득 로트");
        assert_eq!(lot.acquired(), date(1));
        assert_eq!(lot.quantity(), Ok(quantity("12")));
        assert_eq!(lot.cost(), LotCost::Known(money("1000")));
        assert_eq!(lot.tax_status(), crate::LotTaxStatus::NeedsConfirmation);
    }
}
