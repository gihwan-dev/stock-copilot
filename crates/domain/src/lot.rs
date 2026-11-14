use crate::cost_basis::{add_cost, portion};
use crate::{
    Amount, AssetClass, CalendarDate, CorporateAction, CorporateActionId, CorporateActionKind,
    CostBasisMethod, Currency, DividendBasis, FractionalEntitlement, InstrumentId, LotCost,
    LotListing, LotTaxStatus, LotUnitPrice, Market, Money, MoneyError, Quantity, Ratio,
    SettledRowSet, Symbol, Transaction, TransactionKind,
};
use std::collections::BTreeSet;

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
    quantity: Quantity,
    cost: LotCost,
    tax_status: LotTaxStatus,
}
impl Lot {
    #[must_use]
    pub const fn acquired(&self) -> CalendarDate {
        self.acquired
    }
    #[must_use]
    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }
    #[must_use]
    pub const fn cost(&self) -> LotCost {
        self.cost
    }
    #[must_use]
    pub const fn tax_status(&self) -> LotTaxStatus {
        self.tax_status
    }
    pub fn unit_price(&self) -> Result<Option<LotUnitPrice>, MoneyError> {
        match self.cost {
            LotCost::Known(cost) => LotUnitPrice::calculate(cost, self.quantity).map(Some),
            LotCost::Unknown => Ok(None),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FractionalSettlement {
    RetainShares,
    CashInLieu,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotBook {
    identity: LotIdentity,
    method: CostBasisMethod,
    lots: Vec<Lot>,
    realized: LotCost,
    listing: LotListing,
    applied: BTreeSet<CorporateActionId>,
    fractional: Vec<FractionalEntitlement>,
}
impl LotBook {
    pub fn from_settled(
        state: &SettledRowSet,
        identity: LotIdentity,
        method: CostBasisMethod,
        actions: &[CorporateAction],
        as_of: CalendarDate,
    ) -> Result<Self, LotError> {
        let rows = calculation_rows(state, &identity, actions, as_of)?;
        let mut ordered = actions.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|action| action.date());
        let mut result = Self {
            realized: LotCost::Known(zero(identity.cost_currency())),
            identity,
            method,
            lots: Vec::new(),
            listing: LotListing::Listed,
            applied: BTreeSet::new(),
            fractional: Vec::new(),
        };
        result.validate_actions(&ordered, &rows, as_of)?;
        let mut pending = ordered.into_iter().peekable();
        for row in rows {
            while let Some(next_action) =
                pending.next_if(|candidate| candidate.date() <= row.data().settlement_date)
            {
                result.apply(next_action)?;
            }
            if actions.iter().any(|action| action.receipt() == Some(row)) {
                continue;
            }
            result.apply_row(row)?;
        }
        for action in pending {
            result.apply(action)?;
        }
        Ok(result)
    }
    fn validate_actions(
        &mut self,
        ordered: &[&CorporateAction],
        rows: &[&Transaction],
        as_of: CalendarDate,
    ) -> Result<LotListing, LotError> {
        for action in ordered {
            if action.date() > as_of {
                return Err(LotError::FutureAction);
            }
            if !self.applied.insert(action.id().clone()) {
                return Err(LotError::DuplicateAction);
            }
            if let Some(receipt) = action.receipt()
                && (rows.iter().filter(|row| **row == receipt).count() != 1
                    || receipt.data().kind != TransactionKind::SecuritiesTransferIn
                    || receipt.data().settlement_date != action.date()
                    || receipt.data().currency != self.identity.currency)
            {
                return Err(LotError::InvalidReceipt);
            }
            if rows.iter().any(|row| {
                row.data().settlement_date == action.date() && Some(*row) != action.receipt()
            }) {
                return Err(LotError::AmbiguousOrder);
            }
        }
        if ordered
            .array_windows::<2>()
            .any(|[first, second]| first.date() == second.date())
        {
            return Err(LotError::AmbiguousOrder);
        }
        Ok(self.listing)
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
    pub const fn method(&self) -> CostBasisMethod {
        self.method
    }
    #[must_use]
    pub const fn realized_gain(&self) -> LotCost {
        self.realized
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
    pub fn quantity(&self) -> Result<Quantity, LotError> {
        let value =
            self.lots
                .iter()
                .try_fold(Amount::new(rust_decimal::Decimal::ZERO), |total, lot| {
                    crate::exact_arithmetic::add(total, Amount::new(lot.quantity.as_decimal()))
                        .map_err(LotError::from)
                })?;
        Quantity::try_new(value.as_decimal()).map_err(LotError::from)
    }
    pub fn total_cost(&self) -> Result<LotCost, LotError> {
        self.lots.iter().try_fold(
            LotCost::Known(zero(self.identity.cost_currency())),
            |total, lot| add_cost(total, lot.cost).map_err(LotError::from),
        )
    }
    fn apply_row(&mut self, row: &Transaction) -> Result<LotListing, LotError> {
        if row.data().symbol.as_ref() != Some(&self.identity.symbol) {
            return Err(LotError::InvalidIdentity);
        }
        if row.data().currency != self.identity.currency {
            return Err(LotError::CurrencyMismatch);
        }
        match row.data().kind {
            TransactionKind::Buy | TransactionKind::SecuritiesTransferIn => {
                self.require_active()?;
                let quantity = positive_quantity(row)?;
                let cost = if row.data().kind == TransactionKind::Buy {
                    LotCost::Known(self.native_cost(row, purchase_cost(row)?)?)
                } else {
                    LotCost::Unknown
                };
                self.lots.push(Lot {
                    acquired: row.data().settlement_date,
                    quantity,
                    cost,
                    tax_status: if cost == LotCost::Unknown {
                        LotTaxStatus::NeedsConfirmation
                    } else {
                        LotTaxStatus::Ordinary
                    },
                });
                self.rebalance()?;
            }
            TransactionKind::Sell | TransactionKind::SecuritiesTransferOut => {
                self.require_active()?;
                let quantity = positive_quantity(row)?;
                let used = self.consume(quantity)?;
                if row.data().kind == TransactionKind::Sell {
                    let proceeds = row
                        .data()
                        .amount
                        .ok_or(LotError::MissingData)?
                        .checked_sub(&row.data().commission)?
                        .checked_sub(&row.data().tax)?;
                    let proceeds_in_basis = self.native_cost(row, proceeds)?;
                    let gain = realized_difference(proceeds_in_basis, used)?;
                    self.realized = add_cost(self.realized, gain)?;
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
    fn require_active(&self) -> Result<LotListing, LotError> {
        if self.listing == LotListing::Delisted {
            return Err(LotError::Delisted);
        }
        if !self.fractional.is_empty() {
            return Err(LotError::FractionalCashRequired);
        }
        Ok(self.listing)
    }
    fn rebalance(&mut self) -> Result<CostBasisMethod, LotError> {
        if self.method == CostBasisMethod::MovingAverage {
            let total = self.total_cost()?;
            let quantity = self.quantity()?;
            for lot in &mut self.lots {
                lot.cost = match total {
                    LotCost::Known(cost) => LotCost::Known(portion(cost, lot.quantity, quantity)?),
                    LotCost::Unknown => LotCost::Unknown,
                };
            }
        }
        Ok(self.method)
    }
    fn consume(&mut self, quantity: Quantity) -> Result<LotCost, LotError> {
        if quantity > self.quantity()? {
            return Err(LotError::InsufficientQuantity);
        }
        let mut remaining = quantity;
        let mut used = LotCost::Known(zero(self.identity.cost_currency()));
        for lot in &mut self.lots {
            if remaining.as_decimal().is_zero() {
                break;
            }
            let take = remaining.min(lot.quantity);
            let cost = match lot.cost {
                LotCost::Known(cost) => LotCost::Known(portion(cost, take, lot.quantity)?),
                LotCost::Unknown => LotCost::Unknown,
            };
            lot.cost = match (lot.cost, cost) {
                (LotCost::Known(total), LotCost::Known(consumed)) => {
                    LotCost::Known(total.checked_sub(&consumed)?)
                }
                _ => LotCost::Unknown,
            };
            lot.quantity = quantity_sub(lot.quantity, take)?;
            remaining = quantity_sub(remaining, take)?;
            used = add_cost(used, cost)?;
        }
        self.lots.retain(|lot| !lot.quantity.as_decimal().is_zero());
        Ok(used)
    }
    fn apply(&mut self, action: &CorporateAction) -> Result<LotListing, LotError> {
        self.require_active()?;
        if action
            .receipt()
            .is_some_and(|receipt| receipt.data().symbol.as_ref() != Some(&self.identity.symbol))
        {
            return Err(LotError::InvalidReceipt);
        }
        match action.kind() {
            CorporateActionKind::Split(ratio) => {
                self.scale(*ratio, FractionalSettlement::RetainShares)?;
            }
            CorporateActionKind::ReverseSplit(ratio) => {
                self.scale(*ratio, FractionalSettlement::CashInLieu)?;
            }
            CorporateActionKind::BonusIssue { ratio, receipt } => {
                let one = Ratio::new(rust_decimal::Decimal::ONE);
                let before = self.quantity()?;
                let after =
                    self.scale(one.checked_add(ratio)?, FractionalSettlement::RetainShares)?;
                if let Some(observed) = receipt
                    && quantity_sub(after, before)? != positive_quantity(observed)?
                {
                    return Err(LotError::InvalidReceipt);
                }
                for lot in &mut self.lots {
                    lot.tax_status = LotTaxStatus::NeedsConfirmation;
                }
            }
            CorporateActionKind::StockDividend { receipt, basis } => {
                let cost = match basis {
                    DividendBasis::Zero => LotCost::Known(zero(self.identity.cost_currency())),
                    DividendBasis::Confirmed(cost) => LotCost::Known(self.confirmed_cost(*cost)?),
                    DividendBasis::Unknown => LotCost::Unknown,
                };
                self.lots.push(Lot {
                    acquired: action.date(),
                    quantity: positive_quantity(receipt)?,
                    cost,
                    tax_status: LotTaxStatus::NeedsConfirmation,
                });
                self.rebalance()?;
            }
            CorporateActionKind::SymbolChange(symbol) => {
                self.identity.symbol = symbol.clone();
            }
            CorporateActionKind::Delisting => {
                self.listing = LotListing::Delisted;
            }
            CorporateActionKind::Merger
            | CorporateActionKind::Demerger
            | CorporateActionKind::Exchange => return Err(LotError::ManualActionRequired),
        }
        Ok(self.listing)
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
    fn confirmed_cost(&self, cost: Money) -> Result<Money, LotError> {
        if cost.currency() != self.identity.cost_currency() {
            return Err(LotError::CurrencyMismatch);
        }
        Ok(cost)
    }
    fn scale(
        &mut self,
        ratio: Ratio,
        treatment: FractionalSettlement,
    ) -> Result<Quantity, LotError> {
        for lot in &mut self.lots {
            lot.quantity = scaled_quantity(lot.quantity, ratio)?;
        }
        let total = self.quantity()?;
        Quantity::try_new(total.as_decimal().fract())
            .map_err(LotError::from)
            .map(|fraction| {
                if !fraction.as_decimal().is_zero() && treatment == FractionalSettlement::CashInLieu
                {
                    self.fractional.push(FractionalEntitlement::new(fraction));
                }
                total
            })
    }
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
fn quantity_sub(left: Quantity, right: Quantity) -> Result<Quantity, LotError> {
    let amount = crate::exact_arithmetic::subtract(
        Amount::new(left.as_decimal()),
        Amount::new(right.as_decimal()),
    )?;
    Quantity::try_new(amount.as_decimal()).map_err(LotError::from)
}

impl From<crate::NumericError> for LotError {
    fn from(_value: crate::NumericError) -> Self {
        Self::InvalidQuantity
    }
}

fn calculation_rows<'a>(
    state: &'a SettledRowSet,
    identity: &LotIdentity,
    actions: &[CorporateAction],
    as_of: CalendarDate,
) -> Result<Vec<&'a Transaction>, LotError> {
    match (&identity.market, identity.currency) {
        (Market::Kr, Currency::Krw) | (Market::Us, Currency::Usd) => (),
        _ => return Err(LotError::InvalidIdentity),
    }
    let mut symbols = vec![identity.symbol.clone()];
    for action in actions {
        if let CorporateActionKind::SymbolChange(symbol) = action.kind() {
            symbols.push(symbol.clone());
        }
    }
    let mut rows = state
        .active_settled_for_calculation()
        .filter(|row| {
            row.data().settlement_date <= as_of
                && row.data().market == identity.market
                && row.data().asset_class == Some(AssetClass::Stock)
                && row
                    .data()
                    .symbol
                    .as_ref()
                    .is_some_and(|symbol| symbols.contains(symbol))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.data()
            .settlement_date
            .cmp(&right.data().settlement_date)
            .then_with(|| {
                left.data()
                    .transaction_time
                    .cmp(&right.data().transaction_time)
            })
    });
    for [left, right] in rows.array_windows::<2>() {
        if left.data().settlement_date == right.data().settlement_date
            && left != right
            && (left.data().transaction_time.is_none()
                || left.data().transaction_time == right.data().transaction_time)
        {
            return Err(LotError::AmbiguousOrder);
        }
    }
    Ok(rows)
}

fn realized_difference(proceeds: Money, used: LotCost) -> Result<LotCost, MoneyError> {
    match used {
        LotCost::Known(cost) => proceeds.checked_sub(&cost).map(LotCost::Known),
        LotCost::Unknown => Ok(LotCost::Unknown),
    }
}
fn scaled_quantity(quantity: Quantity, ratio: Ratio) -> Result<Quantity, LotError> {
    let amount = crate::exact_arithmetic::multiply(
        Amount::new(quantity.as_decimal()),
        Amount::new(ratio.as_decimal()),
    )
    .map_err(LotError::from)?;
    Quantity::try_new(amount.as_decimal()).map_err(LotError::from)
}
fn purchase_cost(row: &Transaction) -> Result<Money, LotError> {
    Ok(row
        .data()
        .amount
        .ok_or(LotError::MissingData)?
        .checked_add(&row.data().commission)?
        .checked_add(&row.data().tax)?)
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, Amount, AmountInput, AssetClass, CalendarDate, CalendarYear, Cancellation,
        CorporateAction, CorporateActionId, CorporateActionKind, CostBasisMethod, Currency,
        DayNumber, DividendBasis, InstrumentId, Lot, LotBook, LotCost, LotError, LotIdentity,
        LotListing, LotTaxStatus, Market, Money, MoneyError, MonthNumber, Nanosecond, NumericInput,
        OffsetSeconds, Quantity, Ratio, RatioQuotient, SettledRowSet, Settlement,
        StoredTransaction, Symbol, Timestamp, Transaction, TransactionBatch, TransactionData,
        TransactionKind, UnixSeconds,
    };

    fn money(text: &str) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(text)).expect("금액"),
            Currency::Krw,
        )
    }
    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(1),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn symbol() -> Symbol {
        Symbol::try_new("005930").expect("종목")
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
                    StoredTransaction::try_restore(
                        transaction,
                        Timestamp::from_parts(
                            UnixSeconds::new(1),
                            Nanosecond::new(0),
                            OffsetSeconds::new(0),
                        )
                        .expect("시각"),
                        None,
                    )
                    .expect("저장")
                })
                .collect(),
            TransactionBatch::try_new(vec![]).expect("임시"),
            Some(
                Timestamp::from_parts(
                    UnixSeconds::new(1),
                    Nanosecond::new(0),
                    OffsetSeconds::new(0),
                )
                .expect("시각"),
            ),
        )
        .expect("복원")
    }
    fn book(
        transactions: Vec<Transaction>,
        method: CostBasisMethod,
        actions: &[CorporateAction],
    ) -> Result<LotBook, LotError> {
        LotBook::from_settled(
            &rows(transactions),
            LotIdentity::new(instrument(), symbol(), Market::Kr, Currency::Krw),
            method,
            actions,
            date(31),
        )
    }
    #[test]
    fn r11_example_two_fifo_and_moving_average() {
        for (method, expected, gains) in [
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
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money(expected))
            );
            assert_eq!(result.realized_gain(), LotCost::Known(money(gains)));
            assert_eq!(result.quantity().expect("수량"), quantity("10"));
            if method == CostBasisMethod::Fifo {
                assert_eq!(
                    result.lots().iter().map(Lot::quantity).collect::<Vec<_>>(),
                    vec![quantity("5"), quantity("5")]
                );
                assert_eq!(
                    result.lots().iter().map(Lot::cost).collect::<Vec<_>>(),
                    vec![LotCost::Known(money("600")), LotCost::Known(money("550"))]
                );
            }
        }
    }
    #[test]
    fn unknown_transfer_and_overselling_are_never_zero_clipped() {
        let result = book(
            vec![row(1, TransactionKind::SecuritiesTransferIn, "2", None)],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("입고");
        assert_eq!(result.total_cost().expect("원가"), LotCost::Unknown);
        assert_eq!(
            book(
                vec![row(1, TransactionKind::Sell, "1", Some("100"))],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::InsufficientQuantity)
        );
    }

    fn instrument() -> InstrumentId {
        InstrumentId::try_new("instrument").expect("식별")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }
    fn action(day: u8, id: &str, kind: CorporateActionKind) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(InstrumentId::try_new(id).expect("식별")),
            date(day),
            kind,
        )
        .expect("행사")
    }
    #[test]
    fn r11_q5_split_reverse_bonus_keep_cost_and_dates() {
        for (kind, qty) in [
            (CorporateActionKind::Split(ratio("3")), "30"),
            (CorporateActionKind::ReverseSplit(ratio("0.5")), "5"),
            (
                CorporateActionKind::BonusIssue {
                    ratio: ratio("0.2"),
                    receipt: None,
                },
                "12",
            ),
        ] {
            let event = action(2, "event", kind);
            let result = book(
                vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
                CostBasisMethod::Fifo,
                std::slice::from_ref(&event),
            )
            .expect("행사");
            assert_eq!(result.quantity().expect("수량"), quantity(qty));
            assert_eq!(
                result.total_cost().expect("원가"),
                LotCost::Known(money("1000"))
            );
            let lot = result.lots().first().expect("로트");
            assert_eq!(lot.acquired(), date(1));
            let price = lot.unit_price().expect("단가").expect("알려진 원가");
            assert_eq!(price.currency(), Currency::Krw);
            assert_eq!(
                price.quotient(),
                match qty {
                    "30" => money("1000")
                        .checked_ratio(&money("30"))
                        .expect("근사 단가"),
                    "5" => RatioQuotient::Exact(ratio("200")),
                    _ => money("1000")
                        .checked_ratio(&money("12"))
                        .expect("근사 단가"),
                }
            );
            assert_eq!(
                result.applied_actions().iter().collect::<Vec<_>>(),
                vec![event.id()]
            );
            assert_eq!(event.id().instrument_id().as_str(), "event");
        }
        let result = book(
            vec![row(1, TransactionKind::Buy, "10", Some("1000"))],
            CostBasisMethod::Fifo,
            &[
                action(2, "split", CorporateActionKind::Split(ratio("2"))),
                action(3, "merge", CorporateActionKind::ReverseSplit(ratio("0.5"))),
            ],
        )
        .expect("왕복");
        assert_eq!(result.quantity().expect("수량"), quantity("10"));
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("1000"))
        );
    }
    #[test]
    fn r11_q5_dividend_receipt_is_consumed_once_and_tax_unconfirmed() {
        for (basis, expected) in [
            (DividendBasis::Zero, LotCost::Known(money("1000"))),
            (
                DividendBasis::Confirmed(money("20")),
                LotCost::Known(money("1020")),
            ),
            (DividendBasis::Unknown, LotCost::Unknown),
        ] {
            let receipt = row(2, TransactionKind::SecuritiesTransferIn, "2", None);
            let result = book(
                vec![
                    row(1, TransactionKind::Buy, "10", Some("1000")),
                    receipt.clone(),
                ],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "dividend",
                    CorporateActionKind::StockDividend { receipt, basis },
                )],
            )
            .expect("주식배당");
            assert_eq!(result.quantity().expect("수량"), quantity("12"));
            assert_eq!(result.total_cost().expect("원가"), expected);
            assert_eq!(
                result.lots().last().expect("배당 로트").tax_status(),
                LotTaxStatus::NeedsConfirmation
            );
        }
        let receipt = row(2, TransactionKind::SecuritiesTransferIn, "2", None);
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "10", Some("1000")),
                receipt.clone(),
            ],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "bonus",
                CorporateActionKind::BonusIssue {
                    ratio: ratio("0.2"),
                    receipt: Some(receipt),
                },
            )],
        )
        .expect("무상증자");
        assert_eq!(result.quantity().expect("수량"), quantity("12"));
    }
    #[test]
    fn r11_q5_symbol_change_delisting_and_fractional_cash_are_explicit() {
        let renamed = Symbol::try_new("000660").expect("새 코드");
        let result = book(
            vec![row(1, TransactionKind::Buy, "3", Some("100"))],
            CostBasisMethod::Fifo,
            &[
                action(
                    2,
                    "rename",
                    CorporateActionKind::SymbolChange(renamed.clone()),
                ),
                action(3, "delist", CorporateActionKind::Delisting),
            ],
        )
        .expect("코드변경 폐지");
        assert_eq!(result.identity().instrument(), &instrument());
        assert_eq!(result.identity().symbol(), &renamed);
        assert_eq!(result.identity().market(), &Market::Kr);
        assert_eq!(result.identity().currency(), Currency::Krw);
        assert_eq!(result.listing(), LotListing::Delisted);
        assert_eq!(result.method(), CostBasisMethod::Fifo);
        assert_eq!(result.lots().first().expect("로트").acquired(), date(1));
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("100"))
        );
        let fractional = book(
            vec![row(1, TransactionKind::Buy, "3", Some("100"))],
            CostBasisMethod::Fifo,
            &[action(
                2,
                "merge",
                CorporateActionKind::ReverseSplit(ratio("0.5")),
            )],
        )
        .expect("단주");
        assert_eq!(fractional.quantity().expect("수량"), quantity("1.5"));
        assert_eq!(
            fractional
                .fractional_entitlements()
                .first()
                .expect("단주")
                .quantity(),
            quantity("0.5")
        );
        assert_eq!(
            fractional.total_cost().expect("원가"),
            LotCost::Known(money("100"))
        );
    }
    #[test]
    fn corporate_action_rejects_invalid_inputs_duplicate_future_and_ambiguous_order() {
        for kind in [
            CorporateActionKind::Split(ratio("1")),
            CorporateActionKind::ReverseSplit(ratio("0")),
            CorporateActionKind::ReverseSplit(ratio("1")),
            CorporateActionKind::BonusIssue {
                ratio: ratio("0"),
                receipt: None,
            },
        ] {
            assert_eq!(
                CorporateAction::try_new(CorporateActionId::new(instrument()), date(2), kind),
                Err(LotError::InvalidRatio)
            );
        }
        assert_eq!(
            CorporateAction::try_new(
                CorporateActionId::new(instrument()),
                date(2),
                CorporateActionKind::StockDividend {
                    receipt: row(2, TransactionKind::SecuritiesTransferIn, "1", None),
                    basis: DividendBasis::Confirmed(money("-1"))
                }
            ),
            Err(LotError::NegativeCost)
        );
        let event = action(2, "duplicate", CorporateActionKind::Split(ratio("2")));
        assert_eq!(
            book(vec![], CostBasisMethod::Fifo, &[event.clone(), event]),
            Err(LotError::DuplicateAction)
        );
        assert_eq!(
            book(
                vec![row(2, TransactionKind::Buy, "1", Some("1"))],
                CostBasisMethod::Fifo,
                &[action(2, "ambiguous", CorporateActionKind::Delisting)]
            ),
            Err(LotError::AmbiguousOrder)
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("1")),
                    row(1, TransactionKind::Sell, "1", Some("2"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::AmbiguousOrder)
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
    fn fifo_matches_independent_unit_queue_and_split_roundtrips() {
        use proptest::prelude::any;
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &proptest::collection::vec((1_u8..8, 1_u16..1000, any::<u8>()), 1..10),
                |steps| {
                    assert_fifo_reference(&steps);
                    Ok(())
                },
            )
            .expect("독립 FIFO");
    }
    #[test]
    fn split_roundtrip_preserves_cost_property() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    1_u16..10000,
                    1_u32..100_000,
                    proptest::sample::select(vec![
                        ("2", "0.5"),
                        ("4", "0.25"),
                        ("5", "0.2"),
                        ("10", "0.1"),
                    ]),
                ),
                |(count, cost, (forward, reverse))| {
                    let result = book(
                        vec![row(
                            1,
                            TransactionKind::Buy,
                            &count.to_string(),
                            Some(&cost.to_string()),
                        )],
                        CostBasisMethod::Fifo,
                        &[
                            action(2, "split", CorporateActionKind::Split(ratio(forward))),
                            action(
                                3,
                                "merge",
                                CorporateActionKind::ReverseSplit(ratio(reverse)),
                            ),
                        ],
                    )
                    .expect("행사 왕복");
                    assert_eq!(
                        result.total_cost().expect("원가"),
                        LotCost::Known(money(&cost.to_string()))
                    );
                    assert_eq!(
                        result.quantity().expect("수량"),
                        quantity(&count.to_string())
                    );
                    Ok(())
                },
            )
            .expect("원가 보존과 왕복");
    }

    fn assert_fifo_reference(steps: &[(u8, u16, u8)]) {
        use std::collections::VecDeque;
        let mut transactions = Vec::new();
        let mut reference = VecDeque::new();
        let mut gains = 0_i64;
        for (index, (count, price, sell_seed)) in steps.iter().enumerate() {
            let buy_day = u8::try_from(
                index
                    .checked_mul(2)
                    .expect("날짜")
                    .checked_add(1)
                    .expect("날짜"),
            )
            .expect("범위");
            let cost = u32::from(*count)
                .checked_mul(u32::from(*price))
                .expect("금액");
            transactions.push(row(
                buy_day,
                TransactionKind::Buy,
                &count.to_string(),
                Some(&cost.to_string()),
            ));
            for _ in 0..*count {
                reference.push_back(i64::from(*price));
            }
            let sell_count = usize::from(*sell_seed)
                .checked_rem(reference.len().checked_add(1).expect("수량"))
                .expect("나머지");
            if sell_count == 0 {
                continue;
            }
            let proceeds = i64::try_from(sell_count)
                .expect("수량")
                .checked_mul(1000)
                .expect("대금");
            transactions.push(row(
                buy_day.checked_add(1).expect("날짜"),
                TransactionKind::Sell,
                &sell_count.to_string(),
                Some(&proceeds.to_string()),
            ));
            let mut consumed = 0_i64;
            for _ in 0..sell_count {
                consumed = consumed
                    .checked_add(reference.pop_front().expect("보유"))
                    .expect("원가");
            }
            gains = gains
                .checked_add(proceeds.checked_sub(consumed).expect("차이"))
                .expect("손익");
        }
        let result = book(transactions, CostBasisMethod::Fifo, &[]).expect("FIFO");
        assert_fifo_totals(&result, &reference, gains);
    }
    fn assert_fifo_totals(
        result: &LotBook,
        reference: &std::collections::VecDeque<i64>,
        gains: i64,
    ) {
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money(&reference.iter().sum::<i64>().to_string()))
        );
        assert_eq!(
            result.realized_gain(),
            LotCost::Known(money(&gains.to_string()))
        );
        assert_eq!(
            result.quantity().expect("수량"),
            quantity(&reference.len().to_string())
        );
    }
    #[test]
    fn failures_unknown_costs_and_receipt_boundaries_remain_explicit() {
        let buy = row(1, TransactionKind::Buy, "3", Some("1"));
        assert_eq!(
            book(
                vec![buy, row(2, TransactionKind::Sell, "1", Some("2"))],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        let unknown = book(
            vec![
                row(1, TransactionKind::SecuritiesTransferIn, "3", None),
                row(2, TransactionKind::Buy, "1", Some("1")),
                row(3, TransactionKind::Sell, "1", Some("2")),
            ],
            CostBasisMethod::MovingAverage,
            &[],
        )
        .expect("미상");
        assert_eq!(unknown.realized_gain(), LotCost::Unknown);
        assert_eq!(unknown.lots().first().expect("로트").unit_price(), Ok(None));
        let out = book(
            vec![
                row(1, TransactionKind::Buy, "3", Some("3")),
                row(2, TransactionKind::SecuritiesTransferOut, "3", None),
            ],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("출고");
        assert_eq!(out.realized_gain(), LotCost::Known(money("0")));
        assert_eq!(out.quantity().expect("수량"), quantity("0"));
    }
    #[test]
    fn action_error_boundaries() {
        let buy = row(1, TransactionKind::Buy, "3", Some("1"));
        let event = action(31, "future", CorporateActionKind::Delisting);
        assert_eq!(
            LotBook::from_settled(
                &rows(vec![]),
                LotIdentity::new(instrument(), symbol(), Market::Kr, Currency::Krw),
                CostBasisMethod::Fifo,
                &[event],
                date(30)
            ),
            Err(LotError::FutureAction)
        );
        assert_eq!(
            book(
                vec![buy.clone(), row(4, TransactionKind::Sell, "1", Some("2"))],
                CostBasisMethod::Fifo,
                &[action(2, "delist", CorporateActionKind::Delisting)]
            ),
            Err(LotError::Delisted)
        );
        assert_eq!(
            book(
                vec![buy, row(4, TransactionKind::Sell, "1", Some("2"))],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "merge",
                    CorporateActionKind::ReverseSplit(ratio("0.5"))
                )]
            ),
            Err(LotError::FractionalCashRequired)
        );
    }
    #[test]
    fn invalid_receipt_boundaries() {
        for receipt in [
            row(2, TransactionKind::SecuritiesTransferIn, "1", None),
            row(2, TransactionKind::Buy, "1", Some("1")),
        ] {
            assert_eq!(
                book(
                    vec![],
                    CostBasisMethod::Fifo,
                    &[action(
                        2,
                        "bad",
                        CorporateActionKind::StockDividend {
                            receipt,
                            basis: DividendBasis::Zero
                        }
                    )]
                ),
                Err(LotError::InvalidReceipt)
            );
        }
        let receipt = row(2, TransactionKind::SecuritiesTransferIn, "1", None);
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "10", Some("10")),
                    receipt.clone()
                ],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "badbonus",
                    CorporateActionKind::BonusIssue {
                        ratio: ratio("0.2"),
                        receipt: Some(receipt)
                    }
                )]
            ),
            Err(LotError::InvalidReceipt)
        );
    }
    #[test]
    fn required_trade_fields_currency_and_irrelevant_rows() {
        for kind in [TransactionKind::Buy, TransactionKind::Sell] {
            let mut data = row(1, kind.clone(), "1", None).data().clone();
            assert_eq!(
                book(
                    vec![Transaction::try_new(data.clone()).expect("거래")],
                    CostBasisMethod::Fifo,
                    &[]
                ),
                Err(if kind == TransactionKind::Sell {
                    LotError::InsufficientQuantity
                } else {
                    LotError::MissingData
                })
            );
            data.quantity = None;
            assert_eq!(
                book(
                    vec![Transaction::try_new(data).expect("거래")],
                    CostBasisMethod::Fifo,
                    &[]
                ),
                Err(LotError::MissingData)
            );
        }
        let mut cancelled = row(1, TransactionKind::Buy, "1", Some("1")).data().clone();
        cancelled.cancellation = Cancellation::Cancelled;
        let empty = book(
            vec![Transaction::try_new(cancelled).expect("취소")],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("취소 제외");
        assert_eq!(empty.lots(), []);
        let unknown =
            TransactionKind::new(crate::TransactionCode::try_new("FUTURE").expect("종류"));
        assert_eq!(
            book(vec![row(1, unknown, "1", None)], CostBasisMethod::Fifo, &[]),
            Err(LotError::MissingData)
        );
        for kind in [
            TransactionKind::Dividend,
            TransactionKind::Interest,
            TransactionKind::Fee,
            TransactionKind::Deposit,
            TransactionKind::Withdrawal,
            TransactionKind::ExchangeIn,
            TransactionKind::ExchangeOut,
        ] {
            assert_eq!(
                book(vec![row(1, kind, "1", None)], CostBasisMethod::Fifo, &[])
                    .expect("현금은 로트 아님")
                    .lots(),
                []
            );
        }
    }
    #[test]
    fn identity_currency_boundaries() {
        let mut wrong = row(1, TransactionKind::SecuritiesTransferIn, "1", None)
            .data()
            .clone();
        wrong.currency = Currency::Usd;
        wrong.commission = Money::new(money("0").amount(), Currency::Usd);
        wrong.tax = wrong.commission;
        assert_eq!(
            book(
                vec![Transaction::try_new(wrong).expect("거래")],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::CurrencyMismatch)
        );
        assert_eq!(
            LotBook::from_settled(
                &rows(vec![]),
                LotIdentity::new(instrument(), symbol(), Market::Kr, Currency::Usd),
                CostBasisMethod::Fifo,
                &[],
                date(31)
            ),
            Err(LotError::InvalidIdentity)
        );
    }
    #[test]
    fn arithmetic_overflow_is_reported_without_rounding() {
        let max = "79228162514264337593543950335";
        let buys = vec![
            row(1, TransactionKind::Buy, "1", Some(max)),
            row(2, TransactionKind::Buy, "1", Some(max)),
        ];
        assert_eq!(
            book(buys.clone(), CostBasisMethod::Fifo, &[])
                .expect("독립 로트")
                .total_cost(),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            book(buys, CostBasisMethod::MovingAverage, &[]),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        let quantities = book(
            vec![
                row(1, TransactionKind::Buy, max, Some("0")),
                row(2, TransactionKind::Buy, "1", Some("0")),
            ],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("로트");
        assert_eq!(
            quantities.quantity(),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        let tiny = book(
            vec![row(
                1,
                TransactionKind::Buy,
                "0.0000000000000000000000000001",
                Some(max),
            )],
            CostBasisMethod::Fifo,
            &[],
        )
        .expect("소수 로트");
        assert_eq!(
            tiny.lots().first().expect("로트").unit_price(),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            book(
                vec![row(1, TransactionKind::Buy, max, Some("0"))],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "overflow",
                    CorporateActionKind::Split(ratio("2"))
                )]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            super::quantity_sub(quantity("0"), quantity("1")),
            Err(LotError::InvalidQuantity)
        );
    }
    #[test]
    fn dividend_currency_receipt_and_date_boundaries() {
        let receipt = row(2, TransactionKind::SecuritiesTransferIn, "1", None);
        assert_eq!(
            book(
                vec![receipt.clone()],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "div",
                    CorporateActionKind::StockDividend {
                        receipt: receipt.clone(),
                        basis: DividendBasis::Confirmed(Money::new(
                            money("1").amount(),
                            Currency::Usd
                        ))
                    }
                )]
            ),
            Err(LotError::CurrencyMismatch)
        );
        assert_eq!(
            book(
                vec![receipt.clone()],
                CostBasisMethod::Fifo,
                &[action(
                    3,
                    "div",
                    CorporateActionKind::StockDividend {
                        receipt,
                        basis: DividendBasis::Zero
                    }
                )]
            ),
            Err(LotError::InvalidReceipt)
        );
        assert_eq!(
            book(
                vec![],
                CostBasisMethod::Fifo,
                &[
                    action(2, "one", CorporateActionKind::Delisting),
                    action(2, "two", CorporateActionKind::Delisting)
                ]
            ),
            Err(LotError::AmbiguousOrder)
        );
        let renamed = Symbol::try_new("000660").expect("종목");
        assert_eq!(
            book(
                vec![row(3, TransactionKind::Buy, "1", Some("1"))],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "rename",
                    CorporateActionKind::SymbolChange(renamed)
                )]
            ),
            Err(LotError::InvalidIdentity)
        );
    }
    fn usd_row(day: u8, kind: TransactionKind, values: [&str; 5]) -> Transaction {
        let [qty, amount, fee, tax, fx] = values;
        let mut data = row(day, kind, qty, None).data().clone();
        data.market = Market::Us;
        data.symbol = Some(Symbol::try_new("AAPL").expect("종목"));
        data.currency = Currency::Usd;
        data.amount = Some(Money::new(money(amount).amount(), Currency::Usd));
        data.commission = Money::new(money(fee).amount(), Currency::Usd);
        data.tax = Money::new(money(tax).amount(), Currency::Usd);
        data.exchange_rate = Some(ratio(fx));
        Transaction::try_new(data).expect("달러 거래")
    }
    #[test]
    fn r11_example_five_uses_each_settlement_rate_and_actual_costs() {
        for (method, gain, remaining) in [
            (CostBasisMethod::Fifo, "6268962", "9729720"),
            (CostBasisMethod::MovingAverage, "5308002", "8768760"),
        ] {
            let input = rows(vec![
                usd_row(1, TransactionKind::Buy, ["50", "7500", "7.50", "0", "1300"]),
                usd_row(2, TransactionKind::Buy, ["50", "9000", "9", "0", "1350"]),
                usd_row(
                    3,
                    TransactionKind::Sell,
                    ["60", "13200", "13.20", "0.27", "1400"],
                ),
            ]);
            let identity = LotIdentity::new(
                instrument(),
                Symbol::try_new("AAPL").expect("종목"),
                Market::Us,
                Currency::Usd,
            )
            .with_cost_currency(crate::LotCostCurrency::SettlementKrw);
            let result =
                LotBook::from_settled(&input, identity, method, &[], date(31)).expect("원화 원가");
            assert_eq!(result.realized_gain(), LotCost::Known(money(gain)));
            assert_eq!(
                result.total_cost().expect("잔여"),
                LotCost::Known(money(remaining))
            );
        }
    }
    fn charged(kind: TransactionKind, amount: &str, fee: &str, tax: &str) -> Transaction {
        let mut data = row(2, kind, "1", Some(amount)).data().clone();
        data.commission = money(fee);
        data.tax = money(tax);
        Transaction::try_new(data).expect("실제 비용")
    }
    #[test]
    fn actual_cost_arithmetic_errors_are_not_dropped() {
        let max = "79228162514264337593543950335";
        for trade in [
            charged(TransactionKind::Buy, max, "1", "0"),
            charged(TransactionKind::Buy, max, "0", "1"),
        ] {
            assert_eq!(
                book(vec![trade], CostBasisMethod::Fifo, &[]),
                Err(LotError::Arithmetic(MoneyError::Overflow))
            );
        }
        for trade in [
            charged(TransactionKind::Sell, "0.1", max, "0"),
            charged(TransactionKind::Sell, "0.1", "0", max),
        ] {
            assert_eq!(
                book(
                    vec![row(1, TransactionKind::Buy, "1", Some("0")), trade],
                    CostBasisMethod::Fifo,
                    &[]
                ),
                Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
            );
        }
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some(max)),
                    charged(TransactionKind::Sell, "0", "1", "0")
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        let mut missing = row(2, TransactionKind::Sell, "1", None).data().clone();
        missing.commission = money("0");
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("0")),
                    Transaction::try_new(missing).expect("금액 부재")
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::MissingData)
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("0")),
                    row(2, TransactionKind::Sell, "1", Some(max)),
                    row(3, TransactionKind::Buy, "1", Some("0")),
                    row(4, TransactionKind::Sell, "1", Some("1"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
    }
    #[test]
    fn partial_cost_and_quantity_precision_failures_remain_errors() {
        let max = "79228162514264337593543950335";
        assert_eq!(
            crate::cost_basis::portion(money(max), quantity("2"), quantity("3")),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            crate::cost_basis::portion(money("1"), quantity("1"), quantity("0")),
            Err(MoneyError::DivisionByZero)
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "10", Some(max)),
                    row(2, TransactionKind::Sell, "1", Some("1"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, max, Some("0")),
                    row(2, TransactionKind::Sell, "0.1", Some("1"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn remaining_quantity_precision_and_cost_accumulation_failures() {
        let max = "79228162514264337593543950335";
        let max_less_one = "79228162514264337593543950334";
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "0.1", Some("0")),
                    row(2, TransactionKind::Buy, "0.9", Some("0")),
                    row(3, TransactionKind::Buy, max_less_one, Some("0")),
                    row(4, TransactionKind::Sell, max, Some("1"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some(max)),
                    row(2, TransactionKind::Buy, "1", Some("1")),
                    row(3, TransactionKind::Sell, "2", Some("1"))
                ],
                CostBasisMethod::Fifo,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("1")),
                    row(2, TransactionKind::Buy, "2", Some("0"))
                ],
                CostBasisMethod::MovingAverage,
                &[]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn aggregate_quantity_failures_are_preserved_in_each_operation() {
        let max = "79228162514264337593543950335";
        let buys = vec![
            row(1, TransactionKind::Buy, max, Some("0")),
            row(2, TransactionKind::Buy, "1", Some("0")),
        ];
        assert_eq!(
            book(buys.clone(), CostBasisMethod::MovingAverage, &[]),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        let mut sold = buys.clone();
        sold.push(row(3, TransactionKind::Sell, "1", Some("1")));
        assert_eq!(
            book(sold, CostBasisMethod::Fifo, &[]),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            book(
                buys,
                CostBasisMethod::Fifo,
                &[action(
                    3,
                    "bonus",
                    CorporateActionKind::BonusIssue {
                        ratio: ratio("1"),
                        receipt: None
                    }
                )]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
        assert_eq!(
            book(
                vec![
                    row(
                        1,
                        TransactionKind::Buy,
                        "39614081257132168796771975167",
                        Some("0")
                    ),
                    row(2, TransactionKind::Buy, "1", Some("0"))
                ],
                CostBasisMethod::Fifo,
                &[action(3, "split", CorporateActionKind::Split(ratio("2")))]
            ),
            Err(LotError::Arithmetic(MoneyError::Overflow))
        );
    }
    #[test]
    fn actions_reject_overflow_missing_quantity_and_followup_after_delisting() {
        let max = "79228162514264337593543950335";
        for event in [
            action(
                3,
                "bonus",
                CorporateActionKind::BonusIssue {
                    ratio: ratio(max),
                    receipt: None,
                },
            ),
            action(
                3,
                "large",
                CorporateActionKind::BonusIssue {
                    ratio: ratio("1"),
                    receipt: None,
                },
            ),
        ] {
            assert_eq!(
                book(
                    vec![row(1, TransactionKind::Buy, max, Some("0"))],
                    CostBasisMethod::Fifo,
                    &[event]
                ),
                Err(LotError::Arithmetic(MoneyError::Overflow))
            );
        }
        assert_eq!(
            book(
                vec![],
                CostBasisMethod::Fifo,
                &[
                    action(2, "delist", CorporateActionKind::Delisting),
                    action(3, "after", CorporateActionKind::Split(ratio("2")))
                ]
            ),
            Err(LotError::Delisted)
        );
    }
    #[test]
    fn dividend_quantity_and_rebalance_errors() {
        for kind in [
            CorporateActionKind::StockDividend {
                receipt: row(2, TransactionKind::SecuritiesTransferIn, "0", None),
                basis: DividendBasis::Zero,
            },
            CorporateActionKind::BonusIssue {
                ratio: ratio("1"),
                receipt: Some(row(2, TransactionKind::SecuritiesTransferIn, "0", None)),
            },
        ] {
            assert_eq!(
                book(
                    vec![row(2, TransactionKind::SecuritiesTransferIn, "0", None)],
                    CostBasisMethod::Fifo,
                    &[action(2, "zero", kind)]
                ),
                Err(LotError::MissingData)
            );
        }
        let receipt = row(2, TransactionKind::SecuritiesTransferIn, "2", None);
        assert_eq!(
            book(
                vec![
                    row(1, TransactionKind::Buy, "1", Some("1")),
                    receipt.clone()
                ],
                CostBasisMethod::MovingAverage,
                &[action(
                    2,
                    "div",
                    CorporateActionKind::StockDividend {
                        receipt,
                        basis: DividendBasis::Zero
                    }
                )]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn settlement_fx_missing_zero_and_overflow_are_errors() {
        let identity = LotIdentity::new(
            instrument(),
            Symbol::try_new("AAPL").expect("종목"),
            Market::Us,
            Currency::Usd,
        )
        .with_cost_currency(crate::LotCostCurrency::SettlementKrw);
        for rate in ["0", "-1", "79228162514264337593543950335"] {
            let input = rows(vec![usd_row(
                1,
                TransactionKind::Buy,
                ["1", "2", "0", "0", rate],
            )]);
            assert_eq!(
                LotBook::from_settled(
                    &input,
                    identity.clone(),
                    CostBasisMethod::Fifo,
                    &[],
                    date(31)
                ),
                Err(if rate == "79228162514264337593543950335" {
                    LotError::Arithmetic(MoneyError::Overflow)
                } else {
                    LotError::MissingData
                })
            );
        }
        let input = rows(vec![
            usd_row(1, TransactionKind::Buy, ["1", "2", "0", "0", "1"]),
            usd_row(2, TransactionKind::Sell, ["1", "3", "0", "0", "0"]),
        ]);
        assert_eq!(
            LotBook::from_settled(&input, identity, CostBasisMethod::Fifo, &[], date(31)),
            Err(LotError::MissingData)
        );
        assert_eq!(
            book(
                vec![row(3, TransactionKind::Buy, "1", Some("1"))],
                CostBasisMethod::Fifo,
                &[action(2, "delist", CorporateActionKind::Delisting)]
            ),
            Err(LotError::Delisted)
        );
    }
    #[test]
    fn bonus_additional_quantity_precision_and_intraday_order() {
        let receipt = row(2, TransactionKind::SecuritiesTransferIn, "1", None);
        assert_eq!(
            book(
                vec![
                    row(
                        1,
                        TransactionKind::Buy,
                        "7922816251426433759354395033.5",
                        Some("0")
                    ),
                    receipt.clone()
                ],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "bonus",
                    CorporateActionKind::BonusIssue {
                        ratio: ratio("9"),
                        receipt: Some(receipt)
                    }
                )]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
        for sell_time in [1_i64, 2_i64] {
            let mut buy = row(1, TransactionKind::Buy, "1", Some("1")).data().clone();
            buy.transaction_time = Some(
                Timestamp::from_parts(
                    UnixSeconds::new(1),
                    Nanosecond::new(0),
                    OffsetSeconds::new(0),
                )
                .expect("시각"),
            );
            let mut sell = row(1, TransactionKind::Sell, "1", Some("2")).data().clone();
            sell.transaction_time = Some(
                Timestamp::from_parts(
                    UnixSeconds::new(sell_time),
                    Nanosecond::new(0),
                    OffsetSeconds::new(0),
                )
                .expect("시각"),
            );
            let outcome = book(
                vec![
                    Transaction::try_new(buy).expect("매수"),
                    Transaction::try_new(sell).expect("매도"),
                ],
                CostBasisMethod::Fifo,
                &[],
            );
            if sell_time == 1 {
                assert_eq!(outcome, Err(LotError::AmbiguousOrder));
            } else {
                assert_eq!(
                    outcome.expect("시간 순서").realized_gain(),
                    LotCost::Known(money("1"))
                );
            }
        }
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
        let result = LotBook::from_settled(
            &state,
            LotIdentity::new(instrument(), symbol(), Market::Kr, Currency::Krw),
            CostBasisMethod::Fifo,
            &[],
            date(31),
        )
        .expect("계산");
        assert_eq!(result.lots(), []);
        assert_eq!(
            result.total_cost().expect("원가"),
            LotCost::Known(money("0"))
        );
    }
    #[test]
    fn fractional_entitlement_uses_whole_instrument_and_bonus_preserves_cost_property() {
        let result = book(
            vec![
                row(1, TransactionKind::Buy, "1", Some("100")),
                row(2, TransactionKind::Buy, "1", Some("200")),
            ],
            CostBasisMethod::Fifo,
            &[action(
                3,
                "merge",
                CorporateActionKind::ReverseSplit(ratio("0.5")),
            )],
        )
        .expect("합산 정수 주식");
        assert_eq!(result.quantity().expect("수량"), quantity("1"));
        assert_eq!(result.fractional_entitlements(), []);
    }
    #[test]
    fn bonus_preserves_cost_property() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_u16..10000, 1_u32..100_000, 1_u16..100),
                |(count, cost, additional)| {
                    let result = book(
                        vec![row(
                            1,
                            TransactionKind::Buy,
                            &count.to_string(),
                            Some(&cost.to_string()),
                        )],
                        CostBasisMethod::Fifo,
                        &[action(
                            2,
                            "bonus",
                            CorporateActionKind::BonusIssue {
                                ratio: ratio(&additional.to_string()),
                                receipt: None,
                            },
                        )],
                    )
                    .expect("무상증자");
                    assert_eq!(
                        result.total_cost().expect("원가"),
                        LotCost::Known(money(&cost.to_string()))
                    );
                    assert_eq!(
                        result.quantity().expect("수량"),
                        quantity(
                            &u32::from(count)
                                .checked_mul(u32::from(additional).checked_add(1).expect("증가율"))
                                .expect("수량")
                                .to_string()
                        )
                    );
                    Ok(())
                },
            )
            .expect("무상증자 원가 보존");
    }
    #[test]
    fn receipt_must_match_symbol_effective_on_its_date() {
        let receipt = row(3, TransactionKind::SecuritiesTransferIn, "1", None);
        assert_eq!(
            book(
                vec![receipt.clone()],
                CostBasisMethod::Fifo,
                &[
                    action(
                        2,
                        "rename",
                        CorporateActionKind::SymbolChange(Symbol::try_new("000660").expect("종목"))
                    ),
                    action(
                        3,
                        "div",
                        CorporateActionKind::StockDividend {
                            receipt,
                            basis: DividendBasis::Zero
                        }
                    )
                ]
            ),
            Err(LotError::InvalidReceipt)
        );
    }
    fn stamped(transaction: &Transaction, second: i64) -> Transaction {
        let mut data = transaction.data().clone();
        data.transaction_time = Some(
            Timestamp::from_parts(
                UnixSeconds::new(second),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        );
        Transaction::try_new(data).expect("시각 있는 거래")
    }
    #[test]
    fn same_day_different_buys_require_an_observed_order() {
        let first = row(1, TransactionKind::Buy, "1", Some("100"));
        let second = row(1, TransactionKind::Buy, "1", Some("200"));
        for transactions in [
            vec![first.clone(), second.clone()],
            vec![stamped(&first, 1), stamped(&second, 1)],
        ] {
            assert_eq!(
                book(transactions, CostBasisMethod::Fifo, &[]),
                Err(LotError::AmbiguousOrder)
            );
        }
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
    fn ordinary_and_unknown_acquisitions_keep_distinct_tax_status() {
        for (kind, expected) in [
            (TransactionKind::Buy, LotTaxStatus::Ordinary),
            (
                TransactionKind::SecuritiesTransferIn,
                LotTaxStatus::NeedsConfirmation,
            ),
        ] {
            let result = book(
                vec![row(1, kind, "1", Some("100"))],
                CostBasisMethod::Fifo,
                &[],
            )
            .expect("취득");
            assert_eq!(result.lots().first().expect("로트").tax_status(), expected);
        }
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
        let result = LotBook::from_settled(
            &input,
            LotIdentity::new(instrument(), symbol(), Market::Kr, Currency::Krw),
            CostBasisMethod::Fifo,
            &[],
            date(30),
        )
        .expect("미래 결제 제외");
        assert_eq!(result.lots(), []);
    }
    #[test]
    fn splits_and_bonus_issues_retain_fractional_shares() {
        for kind in [
            CorporateActionKind::Split(ratio("3")),
            CorporateActionKind::BonusIssue {
                ratio: ratio("2"),
                receipt: None,
            },
        ] {
            let result = book(
                vec![row(1, TransactionKind::Buy, "0.5", Some("100"))],
                CostBasisMethod::Fifo,
                &[action(2, "increase", kind)],
            )
            .expect("소수 주식 증가");
            assert_eq!(result.quantity().expect("수량"), quantity("1.5"));
            assert_eq!(result.fractional_entitlements(), []);
        }
    }
    #[test]
    fn reverse_split_rejects_quantity_underflow_without_erasing_the_lot() {
        assert_eq!(
            book(
                vec![row(
                    1,
                    TransactionKind::Buy,
                    "0.0000000000000000000000000001",
                    Some("0")
                )],
                CostBasisMethod::Fifo,
                &[action(
                    2,
                    "underflow",
                    CorporateActionKind::ReverseSplit(ratio("0.5"))
                )]
            ),
            Err(LotError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
}
