use crate::valuation_snapshot_cash::{SnapshotAmounts, SnapshotCashData};
use crate::{
    CalendarDate, Candle, CandlePeriod, CandleStatus, CorporateAction, CostBasisMethod, Currency,
    FxRate, FxRateKind, InstrumentId, KrwValuation, KrwValuationComponent, LotAssumption, LotBook,
    LotError, LotIdentity, LotListing, Money, PortfolioValuation, PriceAdjustment, SettledRowSet,
    SnapshotCalendar, SnapshotCashBasis, SnapshotDates, SnapshotError, SnapshotLedgers,
    SnapshotObservation, SnapshotPriceBasis, SnapshotSource, Timestamp, Total, TotalCompleteness,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInstrumentHistory {
    identity: LotIdentity,
    actions: Vec<CorporateAction>,
    candles: Vec<Candle>,
}
impl SnapshotInstrumentHistory {
    pub fn new(
        identity: LotIdentity,
        actions: Vec<CorporateAction>,
        candles: Vec<Candle>,
    ) -> Result<Self, SnapshotError> {
        if actions.len() > 4096 || candles.len() > 4096 {
            return Err(SnapshotError::TooManyInputs);
        }
        Ok(Self {
            identity,
            actions,
            candles,
        })
    }
    fn reconstruct_book(
        &self,
        rows: &SettledRowSet,
        day: CalendarDate,
    ) -> Result<LotBook, LotError> {
        let identity = self.historical_identity(day)?;
        let actions = self
            .actions
            .iter()
            .filter(|action| action.date() <= day)
            .cloned()
            .collect::<Vec<_>>();
        LotBook::from_settled(rows, identity, CostBasisMethod::Fifo, &actions, day)
    }
    fn historical_identity(&self, day: CalendarDate) -> Result<LotIdentity, LotError> {
        let mut changes = BTreeMap::new();
        for action in self.actions.iter().filter(|action| action.date() > day) {
            if let crate::CorporateActionKind::SymbolChange { from, to } = action.kind()
                && changes.insert(to.clone(), from.clone()).is_some()
            {
                return Err(LotError::AmbiguousOrder);
            }
        }
        let mut symbol = self.identity.symbol().clone();
        let mut visited = BTreeSet::new();
        while let Some(previous) = changes.get(&symbol) {
            if !visited.insert(symbol.clone()) {
                return Err(LotError::InvalidIdentity);
            }
            symbol = previous.clone();
        }
        Ok(self.identity.with_historical_symbol(symbol))
    }
    #[must_use]
    pub const fn identity(&self) -> &LotIdentity {
        &self.identity
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPosition {
    instrument: InstrumentId,
    book: Result<LotBook, LotError>,
    candle: Option<Candle>,
    value: Result<Money, SnapshotError>,
}
impl SnapshotPosition {
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    pub const fn book(&self) -> Result<&LotBook, &LotError> {
        self.book.as_ref()
    }
    #[must_use]
    pub const fn candle(&self) -> Option<&Candle> {
        self.candle.as_ref()
    }
    pub const fn value(&self) -> Result<Money, SnapshotError> {
        self.value
    }
}

fn position_value(
    book_result: &Result<LotBook, LotError>,
    candidates: &[&Candle],
    identity: &LotIdentity,
) -> Result<Money, SnapshotError> {
    let book = book_result
        .as_ref()
        .map_err(|error| *error)
        .map_err(lot_error)?;
    if (book.listing() == LotListing::Delisted
        && book
            .quantity()
            .is_ok_and(|quantity| !quantity.as_decimal().is_zero()))
        || !book.fractional_entitlements().is_empty()
        || book
            .deliveries()
            .iter()
            .any(|delivery| delivery.status() == crate::DeliveryStatus::Pending)
    {
        return Err(SnapshotError::PendingCorporateCash);
    }
    price_quantity(book.quantity(), candidates, identity)
}
fn price_quantity(
    quantity_result: Result<crate::Quantity, LotError>,
    candidates: &[&Candle],
    identity: &LotIdentity,
) -> Result<Money, SnapshotError> {
    let quantity = quantity_result.map_err(lot_error)?;
    if quantity.as_decimal().is_zero() {
        return Ok(crate::holding::zero(identity.currency()));
    }
    let [candle] = candidates else {
        return Err(SnapshotError::MissingPrice);
    };
    if candle.instrument() != identity.instrument()
        || candle.values().currency() != identity.currency()
        || candle.time().period() != CandlePeriod::Daily
        || candle.status() != CandleStatus::Finalized
        || candle.adjustment() != PriceAdjustment::Raw
    {
        return Err(SnapshotError::InvalidCandle);
    }
    candle
        .values()
        .close()
        .checked_mul(&quantity)
        .map_err(SnapshotError::Arithmetic)
}
pub(crate) fn aggregate(
    native: Money,
    foreign: Money,
    fx: Option<FxRate>,
    at: Timestamp,
) -> Result<KrwValuation, SnapshotError> {
    let value = KrwValuation::from_native_krw(native).map_err(SnapshotError::Conversion)?;
    if foreign.amount().as_decimal().is_zero() {
        return Ok(value);
    }
    let rate = fx.ok_or(SnapshotError::MissingFx)?;
    if rate.kind() != FxRateKind::Mid || rate.validity_at(at) != crate::FxValidity::Valid {
        return Err(SnapshotError::InvalidFx);
    }
    let converted = rate
        .convert_for_aggregation(foreign)
        .map_err(SnapshotError::Conversion)?;
    value
        .checked_add(&KrwValuation::from_component(
            KrwValuationComponent::Converted(converted),
        ))
        .map_err(SnapshotError::Conversion)
}

fn lot_error(error: LotError) -> SnapshotError {
    SnapshotError::Lot(error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SnapshotContext {
    dates: SnapshotDates,
    price_dates: SnapshotDates,
    kr_basis: SnapshotPriceBasis,
    us_basis: SnapshotPriceBasis,
    settlement_date: CalendarDate,
    valuation_at: Timestamp,
    recorded_at: Timestamp,
    fx: Option<FxRate>,
}
impl SnapshotContext {
    fn new(
        calendar: &SnapshotCalendar,
        observation: SnapshotObservation,
    ) -> Result<Self, SnapshotError> {
        let at = observation.valuation_at();
        if observation.recorded_at() < at {
            return Err(SnapshotError::WrongPeriod);
        }
        let settlement_date = calendar.settlement_date(at)?;
        let (kr, kr_basis) = calendar.price(Currency::Krw, at)?;
        let (us, us_basis) = calendar.price(Currency::Usd, at)?;
        Ok(Self {
            dates: calendar.dates(),
            price_dates: SnapshotDates::new(kr, us),
            kr_basis,
            us_basis,
            settlement_date,
            valuation_at: at,
            recorded_at: observation.recorded_at(),
            fx: observation.fx(),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotInputs {
    stocks: SnapshotAmounts,
    cash: SnapshotCashData,
    reported: SnapshotAmounts,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationSnapshot {
    context: SnapshotContext,
    source: SnapshotSource,
    inputs: SnapshotInputs,
    stocks_converted: Result<KrwValuation, SnapshotError>,
    account_converted: Result<KrwValuation, SnapshotError>,
    positions: Vec<SnapshotPosition>,
    assumptions: BTreeSet<LotAssumption>,
}
impl ValuationSnapshot {
    pub fn observe(
        calendar: &SnapshotCalendar,
        portfolio: &PortfolioValuation,
        at: Timestamp,
        fx: Option<FxRate>,
    ) -> Result<Self, SnapshotError> {
        let context = SnapshotContext::new(calendar, SnapshotObservation::new(at, at, fx))?;
        let stock = |currency| match portfolio.currency_total(currency) {
            Total::Complete(entry) => Ok(*entry.value()),
            Total::Incomplete(_) => Err(SnapshotError::IncompleteValuation),
        };
        let inputs = SnapshotInputs {
            stocks: SnapshotAmounts::new(stock(Currency::Krw), stock(Currency::Usd)),
            cash: SnapshotCashData::observed(
                portfolio.buying_power(Currency::Krw),
                portfolio.buying_power(Currency::Usd),
            ),
            reported: SnapshotAmounts::new(
                reported_holdings(portfolio, Currency::Krw),
                reported_holdings(portfolio, Currency::Usd),
            ),
        };
        Ok(Self::finish(
            context,
            SnapshotSource::Observed,
            inputs,
            Vec::new(),
        ))
    }
    pub fn recalculate(
        calendar: &SnapshotCalendar,
        rows: &SettledRowSet,
        histories: &[SnapshotInstrumentHistory],
        ledgers: Option<SnapshotLedgers<'_>>,
        observation: SnapshotObservation,
    ) -> Result<Self, SnapshotError> {
        let context = SnapshotContext::new(calendar, observation)?;
        let ordered = ordered_histories(histories)?;
        let mut stocks = SnapshotAmounts::new(
            Ok(crate::holding::zero(Currency::Krw)),
            Ok(crate::holding::zero(Currency::Usd)),
        );
        let mut positions = Vec::new();
        for history in ordered {
            let position = history_position(history, rows, context);
            stocks.add(history.identity.currency(), position.value);
            positions.push(position);
        }
        for currency in [Currency::Krw, Currency::Usd] {
            if let Err(error) = history_coverage(rows, histories, currency, context) {
                stocks.invalidate(currency, error);
            }
        }
        let cash = match ledgers {
            Some(input) => input.load(context.valuation_at, rows),
            None => SnapshotCashData::unavailable(),
        };
        let inputs = SnapshotInputs {
            stocks,
            cash,
            reported: SnapshotAmounts::new(
                Err(SnapshotError::MissingReportedValue),
                Err(SnapshotError::MissingReportedValue),
            ),
        };
        Ok(Self::finish(
            context,
            SnapshotSource::Recalculated,
            inputs,
            positions,
        ))
    }
    fn finish(
        context: SnapshotContext,
        source: SnapshotSource,
        inputs: SnapshotInputs,
        positions: Vec<SnapshotPosition>,
    ) -> Self {
        let convert = |amounts: SnapshotAmounts| {
            let kr = amounts.get(Currency::Krw)?;
            aggregate(
                kr,
                amounts.get(Currency::Usd)?,
                context.fx,
                context.valuation_at,
            )
        };
        let stocks_converted = convert(inputs.stocks);
        let account = |currency| account_amount(&inputs, currency);
        let account_converted = convert(SnapshotAmounts::new(
            account(Currency::Krw),
            account(Currency::Usd),
        ));
        let assumptions = positions
            .iter()
            .filter_map(|item| item.book.as_ref().ok())
            .flat_map(|book| book.assumptions().iter().copied())
            .collect();
        Self {
            context,
            source,
            inputs,
            stocks_converted,
            account_converted,
            positions,
            assumptions,
        }
    }
    #[must_use]
    pub const fn dates(&self) -> SnapshotDates {
        self.context.dates
    }
    #[must_use]
    pub const fn valuation_at(&self) -> Timestamp {
        self.context.valuation_at
    }
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.context.recorded_at
    }
    #[must_use]
    pub const fn settlement_date(&self) -> CalendarDate {
        self.context.settlement_date
    }
    #[must_use]
    pub const fn source(&self) -> SnapshotSource {
        self.source
    }
    #[must_use]
    pub const fn fx(&self) -> Option<FxRate> {
        self.context.fx
    }
    #[must_use]
    pub const fn price_date(&self, currency: Currency) -> CalendarDate {
        self.context.price_dates.date(currency)
    }
    #[must_use]
    pub const fn price_basis(&self, currency: Currency) -> SnapshotPriceBasis {
        match currency {
            Currency::Krw => self.context.kr_basis,
            Currency::Usd => self.context.us_basis,
        }
    }
    #[must_use]
    pub const fn cash_basis(&self) -> SnapshotCashBasis {
        self.inputs.cash.basis
    }
    #[must_use]
    pub fn buying_power(&self, currency: Currency) -> Option<crate::BuyingPower> {
        self.inputs
            .cash
            .buying_power
            .map(|(kr, us)| match currency {
                Currency::Krw => kr,
                Currency::Usd => us,
            })
    }
    pub const fn stock_value(&self, currency: Currency) -> Result<Money, SnapshotError> {
        self.inputs.stocks.get(currency)
    }
    pub fn account_value(&self, currency: Currency) -> Result<Money, SnapshotError> {
        account_amount(&self.inputs, currency)
    }
    pub fn stock_krw_value(&self) -> Result<&KrwValuation, SnapshotError> {
        self.stocks_converted.as_ref().map_err(|error| *error)
    }
    pub fn account_krw_value(&self) -> Result<&KrwValuation, SnapshotError> {
        self.account_converted.as_ref().map_err(|error| *error)
    }
    #[must_use]
    pub fn completeness(&self) -> TotalCompleteness {
        completeness(&self.account_converted)
    }
    #[must_use]
    pub fn stock_completeness(&self) -> TotalCompleteness {
        completeness(&self.stocks_converted)
    }
    #[must_use]
    pub fn positions(&self) -> &[SnapshotPosition] {
        &self.positions
    }
    #[must_use]
    pub const fn assumptions(&self) -> &BTreeSet<LotAssumption> {
        &self.assumptions
    }
    pub(crate) const fn reported_stock_daily(
        &self,
        currency: Currency,
    ) -> Result<Money, SnapshotError> {
        self.inputs.reported.get(currency)
    }
    pub(crate) fn confirmed_period(
        &self,
        currency: Currency,
    ) -> Option<&crate::ConfirmedFlowPeriod> {
        self.inputs
            .cash
            .periods
            .as_ref()
            .map(|(kr, us)| match currency {
                Currency::Krw => kr,
                Currency::Usd => us,
            })
    }
    pub(crate) fn exchanges(&self) -> &[crate::Transaction] {
        &self.inputs.cash.exchanges
    }
}
fn completeness(value: &Result<KrwValuation, SnapshotError>) -> TotalCompleteness {
    match value {
        Ok(_) => TotalCompleteness::Complete,
        Err(_) => TotalCompleteness::Incomplete,
    }
}
fn account_amount(input: &SnapshotInputs, currency: Currency) -> Result<Money, SnapshotError> {
    let stock = input.stocks.get(currency)?;
    let cash = input.cash.values.get(currency)?;
    if input.cash.basis != SnapshotCashBasis::SettledLedger {
        return Err(SnapshotError::WrongCashBasis);
    }
    stock.checked_add(&cash).map_err(SnapshotError::Arithmetic)
}
fn ordered_histories(
    histories: &[SnapshotInstrumentHistory],
) -> Result<Vec<&SnapshotInstrumentHistory>, SnapshotError> {
    if histories.len() > 4096 {
        return Err(SnapshotError::TooManyInputs);
    }
    let mut sorted = histories.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|history| history.identity.instrument());
    let mut symbols = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for history in &sorted {
        if !ids.insert(history.identity.instrument()) {
            return Err(SnapshotError::DuplicateInstrument);
        }
        for symbol in aliases(history) {
            if !symbols.insert((history.identity.market().clone(), symbol)) {
                return Err(SnapshotError::DuplicateInstrument);
            }
        }
    }
    Ok(sorted)
}
fn aliases(history: &SnapshotInstrumentHistory) -> BTreeSet<crate::Symbol> {
    let mut symbols = BTreeSet::from([history.identity.symbol().clone()]);
    for action in &history.actions {
        if let crate::CorporateActionKind::SymbolChange { from, to } = action.kind() {
            symbols.insert(from.clone());
            symbols.insert(to.clone());
        }
    }
    symbols
}
fn history_coverage(
    rows: &SettledRowSet,
    histories: &[SnapshotInstrumentHistory],
    currency: Currency,
    context: SnapshotContext,
) -> Result<(), SnapshotError> {
    for row in rows.active_settled_for_calculation().filter(|row| {
        row.data().currency == currency
            && row.data().settlement_date <= context.settlement_date
            && (row.data().asset_class.is_some()
                || row.data().symbol.is_some()
                || row.data().quantity.is_some()
                || row.data().kind == crate::TransactionKind::Buy
                || row.data().kind == crate::TransactionKind::Sell
                || row.data().kind == crate::TransactionKind::SecuritiesTransferIn
                || row.data().kind == crate::TransactionKind::SecuritiesTransferOut)
    }) {
        let data = row.data();
        let covered = data.symbol.as_ref().is_some_and(|symbol| {
            histories.iter().any(|history| {
                history.identity.market() == &data.market
                    && history.identity.currency() == currency
                    && aliases(history).contains(symbol)
            })
        });
        if !covered {
            return Err(SnapshotError::MissingHistory);
        }
        if data.quantity.is_some()
            && data
                .transaction_time
                .is_none_or(|time| time > context.valuation_at)
        {
            return Err(SnapshotError::UnsupportedStockTime);
        }
    }
    Ok(())
}
fn snapshot_book(
    history: &SnapshotInstrumentHistory,
    rows: &SettledRowSet,
    context: SnapshotContext,
) -> Result<LotBook, LotError> {
    let symbols = aliases(history);
    if rows.active_settled_for_calculation().any(|row| {
        row.data().settlement_date <= context.settlement_date
            && row.data().market == *history.identity.market()
            && row.data().currency == history.identity.currency()
            && row.data().quantity.is_some()
            && row
                .data()
                .symbol
                .as_ref()
                .is_some_and(|symbol| symbols.contains(symbol))
            && row
                .data()
                .transaction_time
                .is_none_or(|time| time > context.valuation_at)
    }) {
        return Err(LotError::MissingData);
    }
    history.reconstruct_book(rows, context.settlement_date)
}
fn history_position(
    history: &SnapshotInstrumentHistory,
    rows: &SettledRowSet,
    context: SnapshotContext,
) -> SnapshotPosition {
    let book = snapshot_book(history, rows, context);
    let candidates = history
        .candles
        .iter()
        .filter(|currency| {
            currency.time().date() == context.price_dates.date(history.identity.currency())
        })
        .collect::<Vec<_>>();
    let candle = match candidates.as_slice() {
        [single] => Some((**single).clone()),
        _ => None,
    };
    let value = if context.price_dates.date(history.identity.currency()) < context.settlement_date
        && history.actions.iter().any(|action| {
            action.date() > context.price_dates.date(history.identity.currency())
                && action.date() <= context.settlement_date
                && changes_price_basis(action) == PriceBasisEffect::Changed
        })
        && book.as_ref().is_ok_and(|book_value| {
            book_value
                .quantity()
                .is_ok_and(|quantity| !quantity.as_decimal().is_zero())
        }) {
        Err(SnapshotError::InvalidCandle)
    } else {
        position_value(&book, &candidates, &history.identity)
    };
    SnapshotPosition {
        instrument: history.identity.instrument().clone(),
        book,
        candle,
        value,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceBasisEffect {
    Preserved,
    Changed,
}
fn changes_price_basis(action: &CorporateAction) -> PriceBasisEffect {
    match action.kind() {
        crate::CorporateActionKind::SymbolChange { .. } => PriceBasisEffect::Preserved,
        crate::CorporateActionKind::Split(_)
        | crate::CorporateActionKind::ReverseSplit(_)
        | crate::CorporateActionKind::BonusIssue { .. }
        | crate::CorporateActionKind::StockDividend { .. }
        | crate::CorporateActionKind::Delisting
        | crate::CorporateActionKind::Merger
        | crate::CorporateActionKind::Demerger
        | crate::CorporateActionKind::Exchange => PriceBasisEffect::Changed,
    }
}
fn reported_holdings(
    portfolio: &PortfolioValuation,
    currency: Currency,
) -> Result<Money, SnapshotError> {
    let mut values = portfolio
        .holdings()
        .iter()
        .filter(|history| history.holding().currency() == currency)
        .peekable();
    if values.peek().is_none() {
        return Err(SnapshotError::MissingReportedValue);
    }
    values.try_fold(crate::holding::zero(currency), |sum, item| {
        sum.checked_add(
            &item
                .holding()
                .reported()
                .and_then(crate::ReportedHolding::daily_profit_loss)
                .ok_or(SnapshotError::MissingReportedValue)?
                .amount(),
        )
        .map_err(SnapshotError::Arithmetic)
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        AccountChange, AssetClass, BuyingPower, CalendarDate, Cancellation, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, CashLedger, CashLedgerInput,
        Comparison, ConfirmedFlowPeriod, Currency, CurrencyAmounts, ExternalFlows, FlowPeriod,
        FxPair, FxRate, FxRateKind, FxSource, HoldingsDailyProfit, InstrumentId, LotIdentity,
        Market, MarketCalendar, MarketSession, Money, Nanosecond, NumericInput, OffsetSeconds,
        Ohlcv, Portfolio, PortfolioDailyProfitLoss, Price, PriceAdjustment, Quantity, Ratio,
        SessionData, SessionInformation, SettledRowSet, Settlement, SettlementWindow,
        SnapshotCalendar, SnapshotCashBasis, SnapshotError, SnapshotLedgers, SnapshotObservation,
        SnapshotPriceBasis, SnapshotSource, SpanSeconds, Symbol, TimeInterval, TimeSpan, Timestamp,
        TotalCompleteness, Transaction, TransactionData, TransactionKind, UnixSeconds,
        ValuationSnapshot,
    };
    pub(crate) fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    pub(crate) fn at(day: i64, hour: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                day.checked_sub(1)
                    .expect("일")
                    .checked_mul(86400)
                    .expect("초")
                    .checked_add(hour.checked_mul(3600).expect("시"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn date(day: i64) -> CalendarDate {
        at(day, 0).offset_calendar_date()
    }
    fn session(day: i64, market: &Market, open: Option<SessionData>) -> MarketSession {
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: open,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: open,
                after_market: None,
            },
        };
        MarketSession::try_new(date(day), info).expect("세션")
    }
    fn trading(day: i64, market: &Market) -> MarketSession {
        session(
            day,
            market,
            Some(
                SessionData::try_new(
                    TimeInterval::try_new(at(day, 0), at(day, 1)).expect("구간"),
                    None,
                )
                .expect("장"),
            ),
        )
    }
    fn calendars(day: i64, kr: Option<SessionData>, us: Option<SessionData>) -> SnapshotCalendar {
        let make = |market: &Market, open| {
            MarketCalendar::try_new(
                trading(day.checked_sub(1).expect("전일"), market),
                session(day, market, open),
                trading(day.checked_add(1).expect("후일"), market),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(&Market::Kr, kr), &make(&Market::Us, us)).expect("평가 달력")
    }
    pub(crate) fn calendar(day: i64) -> SnapshotCalendar {
        let data = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(day, 0), at(day, 1)).expect("구간"),
                None,
            )
            .expect("장"),
        );
        calendars(day, data, data)
    }
    pub(crate) fn fx(time: Timestamp, value: &str) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(
                time,
                time.checked_add(
                    TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0)).expect("기간"),
                )
                .expect("끝"),
            )
            .expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    pub(crate) fn tx(
        kind: TransactionKind,
        currency: Currency,
        day: i64,
        hour: i64,
        amount: &str,
    ) -> Transaction {
        Transaction::try_new(TransactionData {
            kind,
            market: match currency {
                Currency::Krw => Market::Kr,
                Currency::Usd => Market::Us,
            },
            currency,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: Some(money(amount, currency)),
            commission: money("0", currency),
            tax: money("0", currency),
            net_amount: Some(money(amount, currency)),
            exchange_rate: None,
            settlement_date: date(day),
            settlement: Settlement::Settled,
            transaction_time: Some(at(day, hour)),
            cancellation: Cancellation::Regular,
        })
        .expect("행")
    }
    fn trade(symbol: &str, currency: Currency, day: i64, qty: &str, amount: &str) -> Transaction {
        let mut data = tx(TransactionKind::Buy, currency, day, 1, amount)
            .data()
            .clone();
        data.symbol = Some(Symbol::try_new(symbol).expect("심볼"));
        data.asset_class = Some(AssetClass::Stock);
        data.quantity =
            Some(Quantity::parse(&NumericInput::try_new(qty).expect("입력")).expect("수량"));
        Transaction::try_new(data).expect("매매")
    }
    pub(crate) struct Fixture {
        pub(crate) rows: SettledRowSet,
        pub(crate) kr: CashLedger,
        pub(crate) us: CashLedger,
        pub(crate) kp: FlowPeriod,
        pub(crate) up: FlowPeriod,
    }
    pub(crate) fn confirmed(item: &FlowPeriod) -> &ConfirmedFlowPeriod {
        match item {
            FlowPeriod::Confirmed(entry) => Some(entry),
            FlowPeriod::Uncertain(_) => None,
        }
        .expect("확정")
    }
    pub(crate) fn fixture(
        txs: Vec<Transaction>,
        opening: (&str, &str),
        end: (&str, &str),
    ) -> Fixture {
        fixture_window(
            txs,
            opening,
            end,
            SettlementWindow::try_new(date(1), date(10)).expect("기간"),
        )
    }
    pub(crate) fn fixture_window(
        txs: Vec<Transaction>,
        opening: (&str, &str),
        end: (&str, &str),
        window: SettlementWindow,
    ) -> Fixture {
        let rows = crate::cash_ledger::tests::set(txs);
        let build_ledger = |currency, start, last| {
            CashLedger::reconstruct(
                &rows,
                CashLedgerInput {
                    window,
                    currency,
                    opening: Some(money(start, currency)),
                    evidence: Some(crate::cash_ledger::tests::evidence(&rows, window)),
                    reported: Some(BuyingPower::new(money(last, currency), at(20, 0))),
                },
            )
        };
        let kr = build_ledger(Currency::Krw, opening.0, end.0);
        let us = build_ledger(Currency::Usd, opening.1, end.1);
        let period = |ledger: &CashLedger| match ledger.external_flows(&[]) {
            ExternalFlows::Reconciled(currency) => currency.period(window),
            ExternalFlows::Unavailable(_) => {
                FlowPeriod::Uncertain(crate::FlowUncertainty::OutsideCoverage)
            }
        };
        let kp = period(&kr);
        let up = period(&us);
        Fixture {
            rows,
            kr,
            us,
            kp,
            up,
        }
    }
    pub(crate) fn ledgers(fixture_data: &Fixture) -> SnapshotLedgers<'_> {
        SnapshotLedgers::new(
            (&fixture_data.kr, confirmed(&fixture_data.kp)),
            (&fixture_data.us, confirmed(&fixture_data.up)),
        )
    }
    fn history(
        symbol: &str,
        currency: Currency,
        prices: &[(i64, &str)],
    ) -> crate::SnapshotInstrumentHistory {
        let identity = LotIdentity::new(
            InstrumentId::try_new(symbol).expect("ID"),
            Symbol::try_new(symbol).expect("심볼"),
            match currency {
                Currency::Krw => Market::Kr,
                Currency::Usd => Market::Us,
            },
            currency,
        );
        let candles = prices
            .iter()
            .map(|(day, value)| {
                let item = Price::try_new(money(value, currency)).expect("가격");
                Candle::try_new(
                    InstrumentId::try_new(symbol).expect("ID"),
                    CandleTime::try_new(at(*day, 0), CandlePeriod::Daily).expect("봉 시각"),
                    PriceAdjustment::Raw,
                    Ohlcv::try_new(
                        item,
                        item,
                        item,
                        item,
                        Quantity::parse(&NumericInput::try_new("0").expect("입력"))
                            .expect("거래량"),
                    )
                    .expect("OHLC"),
                    CandleObservation::new(at(20, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect();
        crate::SnapshotInstrumentHistory::new(identity, Vec::new(), candles).expect("이력")
    }
    pub(crate) fn snapshot(
        fixture_data: &Fixture,
        day: i64,
        histories: &[crate::SnapshotInstrumentHistory],
        rate: &str,
    ) -> ValuationSnapshot {
        ValuationSnapshot::recalculate(
            &calendar(day),
            &fixture_data.rows,
            histories,
            Some(ledgers(fixture_data)),
            SnapshotObservation::new(at(day, 7), at(20, 0), Some(fx(at(day, 7), rate))),
        )
        .expect("재계산")
    }
    pub(crate) fn observed(day: i64, kr: &str, us: &str) -> ValuationSnapshot {
        let portfolio = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money(kr, Currency::Krw), at(day, 7)),
            BuyingPower::new(money(us, Currency::Usd), at(day, 7)),
        )
        .expect("포트");
        ValuationSnapshot::observe(&calendar(day), &portfolio.evaluate(None), at(day, 7), None)
            .expect("조회")
    }
    pub(crate) fn reported(kr: &str, us: Option<&str>) -> PortfolioDailyProfitLoss {
        PortfolioDailyProfitLoss::new(
            CurrencyAmounts::new(
                money(kr, Currency::Krw),
                us.map(|entry| money(entry, Currency::Usd)),
            )
            .expect("통화"),
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("율"),
        )
    }
    #[test]
    fn exchange_changes_neither_native_account_performance_nor_holdings_profit() {
        let fixture_data = fixture(
            vec![
                tx(TransactionKind::ExchangeOut, Currency::Krw, 3, 2, "1375000"),
                tx(TransactionKind::ExchangeIn, Currency::Usd, 3, 2, "1000"),
            ],
            ("1375000", "0"),
            ("0", "1000"),
        );
        let old = snapshot(&fixture_data, 2, &[], "1375");
        let current = snapshot(&fixture_data, 3, &[], "1375");
        let account = AccountChange::between(
            &current,
            Some(&old),
            (&fixture_data.kp, &fixture_data.up),
            &[],
        );
        for currency in [Currency::Krw, Currency::Usd] {
            assert_eq!(
                account.currency(currency).expect("계좌 변동").amount(),
                money("0", currency)
            );
        }
        assert_eq!(
            account
                .krw()
                .expect("환산")
                .valuation()
                .clone()
                .into_parts()
                .0,
            money("0", Currency::Krw)
        );
        let daily = HoldingsDailyProfit::compare(
            &current,
            Some(&old),
            &fixture_data.rows,
            Some(reported("0", Some("0"))),
        );
        for currency in [Currency::Krw, Currency::Usd] {
            assert_eq!(
                daily.currency(currency).computed(),
                Ok(money("0", currency))
            );
            assert_eq!(daily.currency(currency).comparison(), Comparison::Matches);
        }
    }
    #[test]
    fn observed_buying_power_is_separate_and_cannot_be_account_cash() {
        let fixture_data = fixture(Vec::new(), ("100", "0"), ("100", "0"));
        let old = observed(2, "100", "0");
        let current = observed(3, "90", "0");
        assert_eq!(current.cash_basis(), SnapshotCashBasis::OrderBuyingPower);
        assert_eq!(
            current.stock_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            current
                .buying_power(Currency::Krw)
                .expect("주문 가능 금액")
                .money()
                .expect("금액"),
            money("90", Currency::Krw)
        );
        assert_eq!(
            AccountChange::between(
                &current,
                Some(&old),
                (&fixture_data.kp, &fixture_data.up),
                &[]
            )
            .currency(Currency::Krw),
            Err(SnapshotError::WrongCashBasis)
        );
        assert_eq!(
            AccountChange::between(
                &snapshot(&fixture_data, 3, &[], "1"),
                Some(&old),
                (&fixture_data.kp, &fixture_data.up),
                &[]
            )
            .currency(Currency::Krw),
            Err(SnapshotError::WrongCashBasis)
        );
    }
    #[test]
    fn dividend_and_interest_are_account_income_but_not_holdings_daily_income() {
        let fixture_data = fixture(
            vec![
                tx(TransactionKind::Dividend, Currency::Krw, 3, 1, "7"),
                tx(TransactionKind::Interest, Currency::Krw, 3, 2, "3"),
            ],
            ("100", "0"),
            ("110", "0"),
        );
        let old = snapshot(&fixture_data, 2, &[], "1");
        let current = snapshot(&fixture_data, 3, &[], "1");
        assert_eq!(
            AccountChange::between(
                &current,
                Some(&old),
                (&fixture_data.kp, &fixture_data.up),
                &[]
            )
            .currency(Currency::Krw)
            .expect("계좌")
            .amount(),
            money("10", Currency::Krw)
        );
        let daily = HoldingsDailyProfit::compare(
            &current,
            Some(&old),
            &fixture_data.rows,
            Some(reported("0", None)),
        );
        assert_eq!(
            daily.currency(Currency::Krw).comparison(),
            Comparison::Matches
        );
    }
    #[test]
    fn weekend_and_evening_flows_use_exclusive_start_inclusive_end() {
        let fixture_data = fixture(
            vec![
                tx(TransactionKind::Deposit, Currency::Krw, 2, 7, "1"),
                tx(TransactionKind::Deposit, Currency::Krw, 2, 20, "2"),
                tx(TransactionKind::Deposit, Currency::Krw, 4, 12, "3"),
                tx(TransactionKind::Deposit, Currency::Krw, 5, 7, "4"),
                tx(TransactionKind::Deposit, Currency::Krw, 5, 8, "5"),
            ],
            ("100", "0"),
            ("115", "0"),
        );
        let old = snapshot(&fixture_data, 2, &[], "1");
        let current = snapshot(&fixture_data, 5, &[], "1");
        assert_eq!(
            old.account_value(Currency::Krw),
            Ok(money("101", Currency::Krw))
        );
        assert_eq!(
            current.account_value(Currency::Krw),
            Ok(money("110", Currency::Krw))
        );
        assert_eq!(
            AccountChange::between(
                &current,
                Some(&old),
                (&fixture_data.kp, &fixture_data.up),
                &[]
            )
            .currency(Currency::Krw)
            .expect("구간")
            .amount(),
            money("0", Currency::Krw)
        );
    }
    #[test]
    fn missing_history_and_outside_confirmed_dates_cannot_be_complete() {
        let fixture_data = fixture(
            vec![trade("A", Currency::Usd, 2, "10", "100")],
            ("100", "1000"),
            ("100", "900"),
        );
        let missing = snapshot(&fixture_data, 3, &[], "1");
        assert_eq!(
            missing.stock_value(Currency::Usd),
            Err(SnapshotError::MissingHistory)
        );
        assert_eq!(missing.completeness(), TotalCompleteness::Incomplete);
        let future = ValuationSnapshot::recalculate(
            &calendar(12),
            &fixture_data.rows,
            &[history("A", Currency::Usd, &[(12, "10")])],
            Some(ledgers(&fixture_data)),
            SnapshotObservation::new(at(12, 7), at(20, 0), Some(fx(at(12, 7), "1"))),
        )
        .expect("부분");
        assert_eq!(
            future.stock_value(Currency::Usd),
            Ok(money("100", Currency::Usd))
        );
        assert_eq!(
            future.account_value(Currency::Usd),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
    }
    #[test]
    fn cash_and_quantity_both_wait_for_settlement_and_trade_date_is_not_invented() {
        let mut data = trade("A", Currency::Usd, 4, "10", "100").data().clone();
        data.transaction_time = Some(at(2, 1));
        let fixture_data = fixture(
            vec![Transaction::try_new(data).expect("선체결 후결제")],
            ("0", "1000"),
            ("0", "900"),
        );
        let history = history("A", Currency::Usd, &[(2, "10"), (4, "10")]);
        let old = snapshot(&fixture_data, 2, std::slice::from_ref(&history), "1");
        let current = snapshot(&fixture_data, 4, &[history], "1");
        assert_eq!(
            old.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        assert_eq!(
            current.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        let daily = HoldingsDailyProfit::compare(
            &current,
            Some(&old),
            &fixture_data.rows,
            Some(reported("0", Some("0"))),
        );
        assert_eq!(
            daily.currency(Currency::Usd).computed(),
            Err(SnapshotError::MissingExecutionDate)
        );
        assert_eq!(
            daily.currency(Currency::Usd).comparison(),
            Comparison::Unavailable
        );
    }
    #[test]
    fn korean_holiday_carries_previous_close_and_the_next_snapshot_is_usable() {
        let fixture_data = fixture(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("1000", "0"),
            ("900", "0"),
        );
        let history = history("A", Currency::Krw, &[(2, "12"), (4, "13")]);
        let data = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(3, 0), at(3, 1)).expect("구간"),
                None,
            )
            .expect("장"),
        );
        let cal = calendars(3, None, data);
        let old = ValuationSnapshot::recalculate(
            &cal,
            &fixture_data.rows,
            std::slice::from_ref(&history),
            Some(ledgers(&fixture_data)),
            SnapshotObservation::new(at(3, 7), at(20, 0), Some(fx(at(3, 7), "1"))),
        )
        .expect("휴장 기록");
        assert_eq!(
            old.price_basis(Currency::Krw),
            SnapshotPriceBasis::HolidayCarry
        );
        assert_eq!(old.price_date(Currency::Krw), date(2));
        assert_eq!(
            old.stock_value(Currency::Krw),
            Ok(money("120", Currency::Krw))
        );
        assert_eq!(old.completeness(), TotalCompleteness::Complete);
        let next = snapshot(&fixture_data, 4, &[history], "1");
        assert_eq!(
            AccountChange::between(&next, Some(&old), (&fixture_data.kp, &fixture_data.up), &[])
                .currency(Currency::Krw)
                .expect("다음 날")
                .amount(),
            money("10", Currency::Krw)
        );
    }
    #[test]
    fn reported_krw_total_never_mixes_with_calculated_usd_and_bad_previous_is_an_error() {
        let fixture_data = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let old = snapshot(&fixture_data, 2, &[], "1");
        let current = snapshot(&fixture_data, 3, &[], "1");
        let daily = HoldingsDailyProfit::compare(
            &current,
            Some(&old),
            &fixture_data.rows,
            Some(reported("7", None)),
        );
        assert_eq!(daily.krw_selected(), Ok(money("7", Currency::Krw)));
        assert_eq!(
            daily.currency(Currency::Usd).selected(),
            Err(SnapshotError::MissingReportedValue)
        );
        assert_eq!(daily.krw_comparison(), Comparison::Unavailable);
        assert_eq!(
            daily
                .currency(Currency::Krw)
                .selected()
                .expect("정본")
                .previous_source(),
            Ok(SnapshotSource::Recalculated)
        );
        let wrong = HoldingsDailyProfit::compare(&old, Some(&current), &fixture_data.rows, None);
        assert_eq!(wrong.previous(), Err(SnapshotError::WrongPeriod));
    }
    #[test]
    fn r11_examples_six_and_seven_preserve_the_original_arithmetic() {
        let f = fixture(
            vec![trade("A", Currency::Usd, 1, "100", "7000")],
            ("0", "7000"),
            ("0", "0"),
        );
        let holdings = history("A", Currency::Usd, &[(2, "70"), (3, "65")]);
        let old = snapshot(&f, 2, std::slice::from_ref(&holdings), "1200");
        let current = snapshot(&f, 3, &[holdings], "1100");
        let result = AccountChange::between(&current, Some(&old), (&f.kp, &f.up), &[]);
        assert_eq!(
            result.currency(Currency::Usd).expect("달러").amount(),
            money("-500", Currency::Usd)
        );
        assert_eq!(
            result
                .krw()
                .expect("원화")
                .valuation()
                .clone()
                .into_parts()
                .0,
            money("-1250000", Currency::Krw)
        );
        let mixed = fixture(Vec::new(), ("10000000", "5000.00"), ("10000000", "5000.00"));
        assert_eq!(
            snapshot(&mixed, 2, &[], "1375")
                .account_krw_value()
                .expect("환산")
                .clone()
                .into_parts()
                .0,
            money("16875000", Currency::Krw)
        );
    }
    fn delisting() -> crate::CorporateAction {
        crate::CorporateAction::try_new(
            crate::CorporateActionId::new(InstrumentId::try_new("delist").expect("ID")),
            date(3),
            crate::CorporateActionKind::Delisting,
        )
        .expect("폐지")
    }
    fn with_actions(
        item: &crate::SnapshotInstrumentHistory,
        actions: Vec<crate::CorporateAction>,
    ) -> crate::SnapshotInstrumentHistory {
        crate::SnapshotInstrumentHistory::new(item.identity().clone(), actions, Vec::new())
            .expect("이력")
    }
    #[test]
    fn sold_out_delisted_position_is_zero_and_sale_day_is_not_falsely_compared() {
        let mut data = trade("A", Currency::Krw, 2, "10", "100").data().clone();
        data.kind = TransactionKind::Sell;
        let f = fixture(
            vec![
                trade("A", Currency::Krw, 1, "10", "100"),
                Transaction::try_new(data).expect("매도"),
            ],
            ("100", "0"),
            ("100", "0"),
        );
        let item = with_actions(&history("A", Currency::Krw, &[]), vec![delisting()]);
        let current = snapshot(&f, 4, &[item], "1");
        assert_eq!(
            current.stock_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(current.completeness(), TotalCompleteness::Complete);
        let old = snapshot(&f, 2, &[history("A", Currency::Krw, &[])], "1");
        let result =
            HoldingsDailyProfit::compare(&current, Some(&old), &f.rows, Some(reported("0", None)));
        assert_eq!(
            result.currency(Currency::Krw).computed(),
            Err(SnapshotError::MissingExecutionDate)
        );
        assert_eq!(
            result.currency(Currency::Krw).comparison(),
            Comparison::Unavailable
        );
    }
    #[test]
    fn pending_delivery_without_other_holdings_remains_incomplete() {
        let mut data = trade("A", Currency::Krw, 3, "10", "100").data().clone();
        data.kind = TransactionKind::Sell;
        let f = fixture(
            vec![
                trade("A", Currency::Krw, 1, "10", "100"),
                Transaction::try_new(data).expect("매도"),
            ],
            ("100", "0"),
            ("100", "0"),
        );
        let ratio = crate::ShareRatio::new(
            crate::ShareCount::try_new(10).expect("구주"),
            crate::ShareCount::try_new(1).expect("배정"),
        );
        let action = crate::CorporateAction::try_new(
            crate::CorporateActionId::new(InstrumentId::try_new("dividend").expect("ID")),
            date(2),
            crate::CorporateActionKind::StockDividend {
                ratio,
                receipt: None,
                basis: crate::DividendBasis::Zero,
            },
        )
        .expect("배당");
        let item = with_actions(&history("A", Currency::Krw, &[(4, "10")]), vec![action]);
        let current = snapshot(&f, 4, &[item], "1");
        assert_eq!(
            current.stock_value(Currency::Krw),
            Err(SnapshotError::PendingCorporateCash)
        );
        assert_eq!(current.completeness(), TotalCompleteness::Incomplete);
        assert_eq!(
            current
                .positions()
                .first()
                .expect("종목")
                .book()
                .expect("로트")
                .quantity()
                .expect("대기 수량")
                .as_decimal(),
            rust_decimal::Decimal::ONE
        );
    }
    #[test]
    fn us_local_price_day_and_kst_settlement_day_are_distinct() {
        let f = fixture(
            vec![trade("A", Currency::Usd, 3, "10", "100")],
            ("0", "1000"),
            ("0", "900"),
        );
        let kr = MarketCalendar::try_new(
            trading(2, &Market::Kr),
            trading(3, &Market::Kr),
            trading(4, &Market::Kr),
        )
        .expect("국내");
        let us = MarketCalendar::try_new(
            trading(1, &Market::Us),
            trading(2, &Market::Us),
            trading(3, &Market::Us),
        )
        .expect("현지");
        let cal = SnapshotCalendar::new(&kr, &us).expect("시차 달력");
        let current = ValuationSnapshot::recalculate(
            &cal,
            &f.rows,
            &[history("A", Currency::Usd, &[(2, "10")])],
            Some(ledgers(&f)),
            SnapshotObservation::new(at(3, 7), at(20, 0), Some(fx(at(3, 7), "1375"))),
        )
        .expect("평가");
        assert_eq!(current.settlement_date(), date(3));
        assert_eq!(current.price_date(Currency::Usd), date(2));
        assert_eq!(
            current.stock_value(Currency::Usd),
            Ok(money("100", Currency::Usd))
        );
        assert_eq!(
            current.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        assert_eq!(current.completeness(), TotalCompleteness::Complete);
    }
    #[test]
    fn uncertain_period_and_stock_only_snapshots_never_yield_account_values() {
        let mut data = trade("A", Currency::Usd, 3, "10", "100").data().clone();
        data.kind = TransactionKind::SecuritiesTransferIn;
        data.amount = None;
        data.net_amount = None;
        let f = fixture(
            vec![Transaction::try_new(data).expect("미분류 입고")],
            ("0", "1000"),
            ("0", "1000"),
        );
        let current = ValuationSnapshot::recalculate(
            &calendar(3),
            &f.rows,
            &[history("A", Currency::Usd, &[(3, "10")])],
            None,
            SnapshotObservation::new(at(3, 7), at(20, 0), Some(fx(at(3, 7), "1"))),
        )
        .expect("주식 부분");
        assert_eq!(
            current.stock_value(Currency::Usd),
            Ok(money("100", Currency::Usd))
        );
        assert_eq!(
            current.account_value(Currency::Usd),
            Err(SnapshotError::MissingCash)
        );
        assert_eq!(
            AccountChange::between(&current, Some(&observed(2, "0", "0")), (&f.kp, &f.up), &[])
                .currency(Currency::Usd),
            Err(SnapshotError::MissingFlows)
        );
    }
    #[test]
    fn usd_external_flow_requires_its_own_mid_rate_and_preserves_all_used_rates() {
        let f = fixture(
            vec![tx(TransactionKind::Deposit, Currency::Usd, 3, 2, "1")],
            ("0", "100"),
            ("0", "101"),
        );
        let old = snapshot(&f, 2, &[], "100");
        let current = snapshot(&f, 3, &[], "100");
        let missing = AccountChange::between(&current, Some(&old), (&f.kp, &f.up), &[]);
        assert_eq!(
            missing.currency(Currency::Usd).expect("달러").amount(),
            money("0", Currency::Usd)
        );
        assert_eq!(missing.krw(), Err(SnapshotError::MissingFx));
        let quote = fx(at(3, 2), "100");
        let result = AccountChange::between(&current, Some(&old), (&f.kp, &f.up), &[quote]);
        let (amount, rates, accuracy) =
            result.krw().expect("원화").valuation().clone().into_parts();
        assert_eq!(amount, money("0", Currency::Krw));
        assert_eq!(rates.len(), 3);
        assert_eq!(accuracy, crate::ValuationAccuracy::Approximate);
        assert_eq!(
            AccountChange::between(&current, Some(&old), (&f.kp, &f.up), &[quote, quote]).krw(),
            Err(SnapshotError::MissingFx)
        );
    }
    #[test]
    fn withdrawals_are_external_outflows_and_deferred_posting_is_not_false_profit() {
        let f = fixture(
            vec![tx(TransactionKind::Withdrawal, Currency::Krw, 3, 2, "10")],
            ("100", "0"),
            ("90", "0"),
        );
        let old = snapshot(&f, 2, &[], "1");
        let current = snapshot(&f, 3, &[], "1");
        assert_eq!(
            AccountChange::between(&current, Some(&old), (&f.kp, &f.up), &[])
                .currency(Currency::Krw)
                .expect("유출")
                .amount(),
            money("0", Currency::Krw)
        );
        let mut data = tx(TransactionKind::Deposit, Currency::Krw, 4, 1, "10")
            .data()
            .clone();
        data.transaction_time = Some(at(2, 1));
        let delayed = fixture(
            vec![Transaction::try_new(data).expect("지연 반영")],
            ("100", "0"),
            ("110", "0"),
        );
        assert_eq!(
            AccountChange::between(
                &snapshot(&delayed, 4, &[], "1"),
                Some(&snapshot(&delayed, 2, &[], "1")),
                (&delayed.kp, &delayed.up),
                &[]
            )
            .currency(Currency::Krw),
            Err(SnapshotError::WrongPeriod)
        );
    }
    #[test]
    fn both_holidays_skip_and_unfinished_sessions_use_previous_close() {
        let holiday = |market: &Market| {
            MarketCalendar::try_new(
                trading(2, market),
                session(3, market, None),
                trading(4, market),
            )
            .expect("휴장 달력")
        };
        assert_eq!(
            SnapshotCalendar::new(&holiday(&Market::Kr), &holiday(&Market::Us)),
            Err(SnapshotError::ClosedDay)
        );
        let later = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(3, 8), at(3, 9)).expect("구간"),
                None,
            )
            .expect("아직 안 끝난 장"),
        );
        let ended = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(3, 0), at(3, 1)).expect("구간"),
                None,
            )
            .expect("끝난 장"),
        );
        let f = fixture(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("100", "0"),
            ("0", "0"),
        );
        let current = ValuationSnapshot::recalculate(
            &calendars(3, later, ended),
            &f.rows,
            &[history("A", Currency::Krw, &[(2, "12"), (3, "13")])],
            Some(ledgers(&f)),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("전일 종가");
        assert_eq!(
            current.price_basis(Currency::Krw),
            SnapshotPriceBasis::AwaitingSession
        );
        assert_eq!(
            current.stock_value(Currency::Krw),
            Ok(money("120", Currency::Krw))
        );
    }
    fn scaled(coefficient: i128, scale: u32) -> String {
        rust_decimal::Decimal::from_i128_with_scale(coefficient, scale).to_string()
    }
    struct Generated {
        forward: Fixture,
        reversed: Fixture,
        histories: Vec<crate::SnapshotInstrumentHistory>,
        expected: Money,
    }
    fn generated(quantity: i64, start: i64, end: i64, deposit: i64) -> Generated {
        let gross = i128::from(quantity)
            .checked_mul(i128::from(start))
            .expect("정수 매수액");
        let opening = scaled(gross.checked_add(10_140_000).expect("원금"), 4);
        let closing = scaled(i128::from(deposit).checked_add(100_000).expect("현금"), 2);
        let transactions = vec![
            trade(
                "A",
                Currency::Krw,
                1,
                &scaled(i128::from(quantity), 2),
                &scaled(gross, 4),
            ),
            trade("B", Currency::Krw, 1, "2", "14"),
            tx(
                TransactionKind::Deposit,
                Currency::Krw,
                3,
                2,
                &scaled(i128::from(deposit), 2),
            ),
        ];
        let forward = fixture(transactions.clone(), (&opening, "0"), (&closing, "0"));
        let mut reverse = transactions;
        reverse.reverse();
        let reversed = fixture(reverse, (&opening, "0"), (&closing, "0"));
        let histories = vec![
            history(
                "A",
                Currency::Krw,
                &[
                    (2, &scaled(i128::from(start), 2)),
                    (3, &scaled(i128::from(end), 2)),
                ],
            ),
            history("B", Currency::Krw, &[(2, "7"), (3, "9")]),
        ];
        let profit = i128::from(quantity)
            .checked_mul(
                i128::from(end)
                    .checked_sub(i128::from(start))
                    .expect("가격 차"),
            )
            .expect("정수 손익")
            .checked_add(40_000)
            .expect("두 번째 종목");
        Generated {
            forward,
            reversed,
            histories,
            expected: money(&scaled(profit, 4), Currency::Krw),
        }
    }
    #[test]
    fn independent_rational_account_change_and_permutations_for_4096_cases() {
        use proptest::{
            prop_assert_eq,
            test_runner::{Config, TestRunner},
        };
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_i64..100, 1_i64..1000, 1_i64..1000, 0_i64..1000),
                |(quantity, start, end, deposit)| {
                    let data = generated(quantity, start, end, deposit);
                    let old = snapshot(&data.forward, 2, &data.histories, "1");
                    let current = snapshot(&data.forward, 3, &data.histories, "1");
                    let account = AccountChange::between(
                        &current,
                        Some(&old),
                        (&data.forward.kp, &data.forward.up),
                        &[],
                    );
                    prop_assert_eq!(
                        account.currency(Currency::Krw).expect("통화별").amount(),
                        data.expected
                    );
                    prop_assert_eq!(
                        account
                            .krw()
                            .expect("원화")
                            .valuation()
                            .clone()
                            .into_parts()
                            .0,
                        data.expected
                    );
                    let mut reversed = data.histories.clone();
                    reversed.reverse();
                    prop_assert_eq!(&current, &snapshot(&data.reversed, 3, &reversed, "1"));
                    prop_assert_eq!(&current, &snapshot(&data.forward, 3, &data.histories, "1"));
                    Ok(())
                },
            )
            .expect("독립 정수 계수와 유리수");
    }
    fn action(day: i64, kind: crate::CorporateActionKind) -> crate::CorporateAction {
        crate::CorporateAction::try_new(
            crate::CorporateActionId::new(InstrumentId::try_new("action").expect("ID")),
            date(day),
            kind,
        )
        .expect("행사")
    }
    fn symbol_change(day: i64, from: &str, to: &str) -> crate::CorporateAction {
        action(
            day,
            crate::CorporateActionKind::SymbolChange {
                from: Symbol::try_new(from).expect("심볼"),
                to: Symbol::try_new(to).expect("심볼"),
            },
        )
    }
    fn holding(currency: Currency, daily: Option<&str>, price: Option<&str>) -> crate::Holding {
        let ratio = Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("율");
        let report = daily.map(|amount| {
            crate::ReportedHolding::new(money("100", currency), money("0", currency), ratio)
                .expect("제공값")
                .with_daily_profit_loss(crate::DailyProfitLoss::new(money(amount, currency), ratio))
                .expect("일간")
        });
        crate::Holding::new(
            crate::ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("A").expect("심볼"),
                match currency {
                    Currency::Krw => Market::Kr,
                    Currency::Usd => Market::Us,
                },
            )
            .expect("종목"),
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            crate::AcquisitionPrice::try_new(money("100", currency)).expect("취득가"),
            crate::HoldingObservation::new(
                price.map(|price_text| Price::try_new(money(price_text, currency)).expect("가격")),
                report,
                at(3, 7),
            ),
        )
        .expect("보유")
    }
    #[test]
    fn accessors_preserve_provenance_and_individual_toss_daily_reports() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let old = observed(2, "0", "0");
        let portfolio_data = Portfolio::new(
            vec![
                holding(Currency::Krw, Some("7"), None),
                holding(Currency::Usd, Some("2"), None),
            ],
            BuyingPower::new(money("0", Currency::Krw), at(3, 7)),
            BuyingPower::new(money("0", Currency::Usd), at(3, 7)),
        )
        .expect("포트");
        let today = ValuationSnapshot::observe(
            &calendar(3),
            &portfolio_data.evaluate(None),
            at(3, 7),
            Some(fx(at(3, 7), "1375")),
        )
        .expect("조회");
        assert_eq!(today.dates().date(Currency::Krw), date(3));
        assert_eq!(today.recorded_at(), at(3, 7));
        assert_eq!(today.price_date(Currency::Usd), date(3));
        assert_eq!(today.stock_completeness(), TotalCompleteness::Complete);
        today.stock_krw_value().expect("주식 환산");
        assert!(today.assumptions().is_empty());
        let result = HoldingsDailyProfit::compare(&today, Some(&old), &f.rows, None);
        let prev = result.previous().expect("전일");
        assert_eq!(prev.source(), SnapshotSource::Observed);
        assert_eq!(prev.stock_completeness(), TotalCompleteness::Complete);
        assert_eq!(prev.account_completeness(), TotalCompleteness::Incomplete);
        assert_eq!(result.reported(), None);
        assert_eq!(
            result.krw_selected(),
            Err(SnapshotError::MissingReportedValue)
        );
        for (currency, expected) in [(Currency::Krw, "7"), (Currency::Usd, "2")] {
            let selected = result.currency(currency).selected().expect("정본");
            assert_eq!(selected.amount(), money(expected, currency));
            assert_eq!(selected.origin(), crate::HoldingsDailyOrigin::TossHoldings);
            assert_eq!(selected.source(), crate::ValuationSource::TossOpenApi);
            assert_eq!(selected.previous_source(), Ok(SnapshotSource::Observed));
            assert_eq!(
                result.currency(currency).comparison(),
                Comparison::Different
            );
        }
        assert_summary_and_missing(&today, &old, &f);
    }
    fn assert_summary_and_missing(today: &ValuationSnapshot, old: &ValuationSnapshot, f: &Fixture) {
        let reported = reported("2757", Some("2"));
        let summary = HoldingsDailyProfit::compare(today, Some(old), &f.rows, Some(reported));
        assert_eq!(
            summary.currency(Currency::Krw).comparison(),
            Comparison::Unavailable
        );
        assert_eq!(
            summary
                .currency(Currency::Usd)
                .selected()
                .expect("정본")
                .origin(),
            crate::HoldingsDailyOrigin::TossSummary
        );
        assert_eq!(
            summary
                .currency(Currency::Usd)
                .selected()
                .expect("정본")
                .source(),
            crate::ValuationSource::TossOpenApi
        );
        assert_eq!(summary.reported(), Some(reported));
        let missing = HoldingsDailyProfit::compare(today, None, &f.rows, Some(reported));
        assert_eq!(missing.previous(), Err(SnapshotError::MissingPrevious));
        assert_eq!(
            missing
                .currency(Currency::Usd)
                .selected()
                .expect("정본")
                .previous_source(),
            Err(SnapshotError::MissingPrevious)
        );
        assert_eq!(
            AccountChange::between(today, None, (&f.kp, &f.up), &[]).previous(),
            Err(SnapshotError::MissingPrevious)
        );
    }
    #[test]
    fn missing_reports_prices_and_arithmetic_never_turn_into_complete_totals() {
        let portfolio_data = Portfolio::new(
            vec![holding(Currency::Krw, None, None)],
            BuyingPower::new(money("0", Currency::Krw), at(3, 7)),
            BuyingPower::new(money("0", Currency::Usd), at(3, 7)),
        )
        .expect("포트");
        let today = ValuationSnapshot::observe(
            &calendar(3),
            &portfolio_data.evaluate(None),
            at(3, 7),
            None,
        )
        .expect("조회");
        assert_eq!(
            today.stock_value(Currency::Krw),
            Err(SnapshotError::IncompleteValuation)
        );
        assert_eq!(
            today.reported_stock_daily(Currency::Krw),
            Err(SnapshotError::MissingReportedValue)
        );
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let old = snapshot(&f, 2, &[], "1375");
        let daily =
            HoldingsDailyProfit::compare(&today, Some(&old), &f.rows, Some(reported("0", None)));
        assert_eq!(
            daily.currency(Currency::Krw).computed(),
            Err(SnapshotError::IncompleteValuation)
        );
        let older_bad = ValuationSnapshot::observe(
            &calendar(2),
            &portfolio_data.evaluate(None),
            at(2, 7),
            None,
        )
        .expect("조회");
        let current = snapshot(&f, 3, &[], "1375");
        assert_eq!(
            HoldingsDailyProfit::compare(&current, Some(&older_bad), &f.rows, None)
                .currency(Currency::Krw)
                .computed(),
            Err(SnapshotError::IncompleteValuation)
        );
        let mut wrong_basis = current;
        wrong_basis.inputs.cash.basis = SnapshotCashBasis::Unavailable;
        assert_eq!(
            wrong_basis.account_value(Currency::Krw),
            Err(SnapshotError::WrongCashBasis)
        );
    }
    #[test]
    fn price_and_quantity_failures_preserve_their_causes() {
        let identity = history("A", Currency::Krw, &[]).identity;
        assert_eq!(
            super::price_quantity(Err(crate::LotError::MissingData), &[], &identity),
            Err(SnapshotError::Lot(crate::LotError::MissingData))
        );
        let quantity = Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량");
        assert_eq!(
            super::price_quantity(Ok(quantity), &[], &identity),
            Err(SnapshotError::MissingPrice)
        );
        let wrong = history("B", Currency::Krw, &[(3, "1")]);
        assert_eq!(
            super::price_quantity(Ok(quantity), &[&wrong.candles[0]], &identity),
            Err(SnapshotError::InvalidCandle)
        );
        let mut huge = history("A", Currency::Krw, &[(3, "79228162514264337593543950335")]);
        assert_eq!(
            super::price_quantity(
                Ok(Quantity::parse(&NumericInput::try_new("2").expect("入力")).expect("수량")),
                &[&huge.candles[0]],
                &identity
            ),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
        huge.candles.push(huge.candles[0].clone());
        assert_eq!(
            super::price_quantity(
                Ok(quantity),
                &huge.candles.iter().collect::<Vec<_>>(),
                &identity
            ),
            Err(SnapshotError::MissingPrice)
        );
        let mut amounts = crate::valuation_snapshot_cash::SnapshotAmounts::new(
            Ok(money("0", Currency::Krw)),
            Ok(money("0", Currency::Usd)),
        );
        amounts.invalidate(Currency::Usd, SnapshotError::MissingHistory);
        assert_eq!(
            amounts.get(Currency::Usd),
            Err(SnapshotError::MissingHistory)
        );
    }
    #[test]
    fn fx_facts_and_projection_errors_remain_errors() {
        let valuation_at = at(3, 7);
        let zero = money("0", Currency::Krw);
        let one = money("1", Currency::Usd);
        assert_eq!(
            super::aggregate(one, one, None, valuation_at),
            Err(SnapshotError::Conversion(
                crate::FxConversionError::CurrencyMismatch
            ))
        );
        assert_eq!(
            super::aggregate(zero, one, None, valuation_at),
            Err(SnapshotError::MissingFx)
        );
        assert_eq!(
            super::aggregate(zero, one, Some(fx(at(2, 7), "1375")), valuation_at),
            Err(SnapshotError::InvalidFx)
        );
        let buy = FxRate::parse(
            &NumericInput::try_new("1375").expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Buy,
            TimeInterval::try_new(valuation_at, at(3, 8)).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율");
        assert_eq!(
            super::aggregate(zero, one, Some(buy), valuation_at),
            Err(SnapshotError::InvalidFx)
        );
        assert_eq!(
            super::aggregate(
                zero,
                money("79228162514264337593543950335", Currency::Usd),
                Some(fx(valuation_at, "2")),
                valuation_at
            ),
            Err(SnapshotError::Conversion(
                crate::FxConversionError::Arithmetic(crate::MoneyError::Overflow)
            ))
        );
        assert_eq!(
            super::aggregate(
                money("79228162514264337593543950335", Currency::Krw),
                one,
                Some(fx(valuation_at, "2")),
                valuation_at
            ),
            Err(SnapshotError::Conversion(
                crate::FxConversionError::Arithmetic(crate::MoneyError::Overflow)
            ))
        );
    }
    #[test]
    fn timestamp_projection_and_calendar_errors_remain_errors() {
        let valuation_at = at(3, 7);
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        assert_eq!(
            ValuationSnapshot::recalculate(
                &calendar(3),
                &f.rows,
                &[],
                None,
                SnapshotObservation::new(valuation_at, at(2, 7), None)
            ),
            Err(SnapshotError::WrongPeriod)
        );
        assert_eq!(
            ValuationSnapshot::recalculate(
                &calendar(3),
                &f.rows,
                &[],
                None,
                SnapshotObservation::new(at(4, 7), at(4, 7), None)
            ),
            Err(SnapshotError::WrongPeriod)
        );
        assert_eq!(calendar(3).price_date(Currency::Krw, at(3, 7)), Ok(date(3)));
        assert_eq!(
            calendar(3).price_date(Currency::Usd, at(1, 0)),
            Err(SnapshotError::MissingPrice)
        );
        assert_eq!(
            ValuationSnapshot::recalculate(
                &calendar(3),
                &f.rows,
                &[],
                None,
                SnapshotObservation::new(at(3, -10), at(3, 7), None)
            ),
            Err(SnapshotError::WrongPeriod)
        );
        let kr = MarketCalendar::try_new(
            trading(2, &Market::Kr),
            trading(3, &Market::Kr),
            trading(4, &Market::Kr),
        )
        .expect("달력");
        assert_eq!(
            SnapshotCalendar::new(&kr, &kr),
            Err(SnapshotError::CalendarMismatch)
        );
    }
    #[test]
    fn timestamp_extreme_offset_projection_is_an_error() {
        let zero = money("0", Currency::Krw);
        let max = Timestamp::from_parts(
            UnixSeconds::new(253_402_300_799),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("상한");
        assert_eq!(
            crate::valuation_snapshot_calendar::kst_date(max),
            Err(SnapshotError::Time(crate::TimeError::OutOfRange))
        );
        assert_eq!(
            calendar(3).settlement_date(max),
            Err(SnapshotError::Time(crate::TimeError::OutOfRange))
        );
        assert_eq!(
            crate::valuation_snapshot_cash::balance_at(zero, &[], max),
            Err(SnapshotError::Time(crate::TimeError::OutOfRange))
        );
    }
    #[test]
    fn history_identity_coverage_and_resource_bounds_are_enforced() {
        let f = fixture(
            vec![trade("A", Currency::Usd, 2, "1", "10")],
            ("0", "10"),
            ("0", "0"),
        );
        let mut instrument_history = history("C", Currency::Usd, &[(2, "10")]);
        instrument_history.actions = vec![symbol_change(4, "A", "B"), symbol_change(5, "B", "C")];
        assert_eq!(
            instrument_history
                .historical_identity(date(2))
                .expect("이전 심볼")
                .symbol(),
            &Symbol::try_new("A").expect("심볼")
        );
        let sample = snapshot(&f, 2, &[instrument_history.clone()], "1375");
        let portfolio_data = &sample.positions()[0];
        assert_eq!(
            portfolio_data.instrument(),
            instrument_history.identity().instrument()
        );
        portfolio_data.book().expect("로트");
        assert!(portfolio_data.candle().is_some());
        assert_eq!(portfolio_data.value(), Ok(money("10", Currency::Usd)));
        let mut branch = instrument_history.clone();
        branch.actions.push(symbol_change(6, "D", "C"));
        assert_eq!(
            branch.historical_identity(date(2)),
            Err(crate::LotError::AmbiguousOrder)
        );
        let mut cycle = instrument_history.clone();
        cycle.actions = vec![symbol_change(4, "A", "C"), symbol_change(5, "C", "A")];
        assert_eq!(
            cycle.historical_identity(date(2)),
            Err(crate::LotError::InvalidIdentity)
        );
        assert_eq!(
            super::ordered_histories(&[instrument_history.clone(), instrument_history]),
            Err(SnapshotError::DuplicateInstrument)
        );
        assert_eq!(
            snapshot(&f, 2, &[branch], "1").positions()[0].value(),
            Err(SnapshotError::Lot(crate::LotError::AmbiguousOrder))
        );
    }
    #[test]
    fn history_aliases_and_resource_limits_are_enforced() {
        let mut instrument_history = history("C", Currency::Usd, &[(2, "10")]);
        instrument_history.actions = vec![symbol_change(4, "A", "B"), symbol_change(5, "B", "C")];
        let mut alias = history("B", Currency::Usd, &[]);
        alias.actions = vec![];
        assert_eq!(
            super::ordered_histories(&[instrument_history.clone(), alias]),
            Err(SnapshotError::DuplicateInstrument)
        );
        assert_eq!(
            super::ordered_histories(&vec![instrument_history.clone(); 4097]),
            Err(SnapshotError::TooManyInputs)
        );
        assert_eq!(
            crate::SnapshotInstrumentHistory::new(
                instrument_history.identity.clone(),
                vec![symbol_change(4, "A", "C"); 4097],
                vec![]
            ),
            Err(SnapshotError::TooManyInputs)
        );
        assert_eq!(
            crate::SnapshotInstrumentHistory::new(
                instrument_history.identity.clone(),
                vec![],
                vec![instrument_history.candles[0].clone(); 4097]
            ),
            Err(SnapshotError::TooManyInputs)
        );
    }
    #[test]
    fn missing_or_late_stock_processing_time_is_incomplete() {
        let mut instrument_history = history("C", Currency::Usd, &[(2, "10")]);
        instrument_history.actions = vec![symbol_change(4, "A", "B"), symbol_change(5, "B", "C")];
        let mut no_time = trade("A", Currency::Usd, 2, "1", "10").data().clone();
        no_time.transaction_time = None;
        let rows = crate::cash_ledger::tests::set(vec![
            Transaction::try_new(no_time.clone()).expect("시각 누락"),
        ]);
        let sample = ValuationSnapshot::recalculate(
            &calendar(2),
            &rows,
            &[instrument_history.clone()],
            None,
            SnapshotObservation::new(at(2, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample.stock_value(Currency::Usd),
            Err(SnapshotError::UnsupportedStockTime)
        );
        assert_eq!(
            sample.positions()[0].book(),
            Err(&crate::LotError::MissingData)
        );
        no_time.transaction_time = Some(at(3, 0));
        let rows2 =
            crate::cash_ledger::tests::set(vec![Transaction::try_new(no_time).expect("늦은 시각")]);
        let sample2 = ValuationSnapshot::recalculate(
            &calendar(2),
            &rows2,
            &[instrument_history],
            None,
            SnapshotObservation::new(at(2, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample2.stock_value(Currency::Usd),
            Err(SnapshotError::UnsupportedStockTime)
        );
        assert_eq!(
            sample2.positions()[0].value(),
            Err(SnapshotError::Lot(crate::LotError::MissingData))
        );
    }
    #[test]
    fn holiday_carry_rejects_corporate_basis_crossing() {
        let f = fixture(
            vec![trade("A", Currency::Krw, 1, "1", "100")],
            ("100", "0"),
            ("0", "0"),
        );
        let mut instrument_history = history("A", Currency::Krw, &[(2, "100")]);
        let ratio = crate::ShareRatio::new(
            crate::ShareCount::try_new(1).expect("수"),
            crate::ShareCount::try_new(2).expect("수"),
        );
        instrument_history.actions = vec![action(3, crate::CorporateActionKind::Split(ratio))];
        let us = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(3, 0), at(3, 1)).expect("구간"),
                None,
            )
            .expect("세션"),
        );
        let sample = ValuationSnapshot::recalculate(
            &calendars(3, None, us),
            &f.rows,
            &[instrument_history],
            Some(ledgers(&f)),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample.stock_value(Currency::Krw),
            Err(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            super::changes_price_basis(&symbol_change(3, "A", "B")),
            super::PriceBasisEffect::Preserved
        );
        let mut instrument_history2 = history("B", Currency::Krw, &[(2, "100")]);
        instrument_history2.actions = vec![symbol_change(3, "A", "B")];
        let sample2 = ValuationSnapshot::recalculate(
            &calendars(3, None, us),
            &f.rows,
            &[instrument_history2],
            Some(ledgers(&f)),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample2.stock_value(Currency::Krw),
            Ok(money("100", Currency::Krw))
        );
    }
    #[test]
    fn ledger_identity_evidence_and_backwards_cash_errors_are_preserved() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        assert_eq!(
            f.kr.snapshot_basis(confirmed(&f.up), &[]),
            Err(crate::CashUnavailable::IncompleteResponse)
        );
        let bad = fixture(Vec::new(), ("0", "0"), ("1", "0"));
        assert_eq!(
            bad.kr.snapshot_basis(confirmed(&f.kp), &[]),
            Err(crate::CashUnavailable::ReportedDifference)
        );
        let swapped = SnapshotLedgers::new((&f.us, confirmed(&f.up)), (&f.kr, confirmed(&f.kp)));
        let sample = ValuationSnapshot::recalculate(
            &calendar(3),
            &f.rows,
            &[],
            Some(swapped),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample.account_value(Currency::Krw),
            Err(SnapshotError::CalendarMismatch)
        );
        let bad_input =
            SnapshotLedgers::new((&bad.kr, confirmed(&f.kp)), (&f.us, confirmed(&f.up)));
        let sample2 = ValuationSnapshot::recalculate(
            &calendar(3),
            &f.rows,
            &[],
            Some(bad_input),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample2.account_value(Currency::Krw),
            Err(SnapshotError::Cash(
                crate::CashUnavailable::ReportedDifference
            ))
        );
    }
    #[test]
    fn cash_source_rows_must_equal_the_reconciled_ledger_rows() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let other = fixture(
            vec![tx(TransactionKind::Deposit, Currency::Krw, 3, 2, "1")],
            ("0", "0"),
            ("1", "0"),
        );
        let sample = ValuationSnapshot::recalculate(
            &calendar(3),
            &other.rows,
            &[],
            Some(ledgers(&f)),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("기록");
        assert_eq!(
            sample.account_value(Currency::Krw),
            Err(SnapshotError::Cash(
                crate::CashUnavailable::IncompleteResponse
            ))
        );
    }
    #[test]
    fn backwards_cash_missing_facts_and_overflow_are_errors() {
        let mut row = tx(TransactionKind::Deposit, Currency::Krw, 4, 1, "1")
            .data()
            .clone();
        row.transaction_time = None;
        assert_eq!(
            crate::valuation_snapshot_cash::balance_at(
                money("0", Currency::Krw),
                &[Transaction::try_new(row.clone()).expect("행")],
                at(3, 7)
            ),
            Err(SnapshotError::MissingFlowTime)
        );
        row.transaction_time = Some(at(4, 1));
        row.net_amount = None;
        assert_eq!(
            crate::valuation_snapshot_cash::balance_at(
                money("0", Currency::Krw),
                &[Transaction::try_new(row).expect("행")],
                at(3, 7)
            ),
            Err(SnapshotError::Cash(
                crate::CashUnavailable::MissingNetAmount
            ))
        );
        let huge = tx(
            TransactionKind::Deposit,
            Currency::Krw,
            4,
            1,
            "79228162514264337593543950335",
        );
        assert_eq!(
            crate::valuation_snapshot_cash::balance_at(
                money("-1", Currency::Krw),
                &[huge],
                at(3, 7)
            ),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn backwards_cash_boundary_and_time_projection_are_checked() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let large = fixture(
            vec![tx(
                TransactionKind::Deposit,
                Currency::Krw,
                4,
                1,
                "79228162514264337593543950335",
            )],
            ("0", "0"),
            ("79228162514264337593543950335", "0"),
        );
        assert_eq!(
            snapshot(&large, 3, &[], "1").account_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            ledgers(&f)
                .load(
                    Timestamp::from_parts(
                        UnixSeconds::new(253_402_300_799),
                        Nanosecond::new(0),
                        OffsetSeconds::new(0)
                    )
                    .expect("상한"),
                    &f.rows
                )
                .values
                .get(Currency::Krw),
            Err(SnapshotError::Time(crate::TimeError::OutOfRange))
        );
    }
    pub(crate) fn account_parts(
        day: i64,
        kr: Result<Money, SnapshotError>,
        us: Result<Money, SnapshotError>,
        rate: Option<FxRate>,
    ) -> ValuationSnapshot {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let base = snapshot(&f, day, &[], "1");
        let mut inputs = base.inputs;
        inputs.stocks = crate::valuation_snapshot_cash::SnapshotAmounts::new(kr, us);
        let mut context = base.context;
        context.fx = rate;
        ValuationSnapshot::finish(context, SnapshotSource::Recalculated, inputs, Vec::new())
    }
    #[test]
    fn mismatched_ledger_windows_and_tokens_are_rejected() {
        let f = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let other = fixture_window(
            Vec::new(),
            ("0", "0"),
            ("0", "0"),
            SettlementWindow::try_new(date(2), date(9)).expect("기간"),
        );
        assert_eq!(
            f.kr.snapshot_basis(confirmed(&other.kp), &[]),
            Err(crate::CashUnavailable::IncompleteResponse)
        );
        let input =
            SnapshotLedgers::new((&f.kr, confirmed(&f.kp)), (&other.us, confirmed(&other.up)));
        assert_eq!(
            input.load(at(3, 7), &f.rows).values.get(Currency::Krw),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
        let old = snapshot(&f, 2, &[], "1");
        let today = snapshot(&f, 3, &[], "1");
        assert_eq!(
            AccountChange::between(&today, Some(&old), (&f.kp, &other.up), &[])
                .currency(Currency::Usd),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
        assert_eq!(
            AccountChange::between(&today, Some(&old), (&other.kp, &other.up), &[])
                .currency(Currency::Usd),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
        let input2 =
            SnapshotLedgers::new((&f.kr, confirmed(&other.kp)), (&f.us, confirmed(&other.up)));
        assert_eq!(
            input2.load(at(3, 7), &f.rows).values.get(Currency::Krw),
            Err(SnapshotError::Cash(
                crate::CashUnavailable::IncompleteResponse
            ))
        );
        let bad = fixture(Vec::new(), ("0", "0"), ("1", "0"));
        assert_eq!(
            bad.kp,
            FlowPeriod::Uncertain(crate::FlowUncertainty::OutsideCoverage)
        );
        assert_eq!(
            SnapshotLedgers::new(
                (&other.kr, confirmed(&other.kp)),
                (&other.us, confirmed(&other.up))
            )
            .load(at(1, 7), &other.rows)
            .values
            .get(Currency::Krw),
            Err(SnapshotError::OutsideConfirmedPeriod)
        );
    }
    #[test]
    fn confirmed_token_cannot_erase_another_ledgers_external_flows() {
        let empty = fixture(Vec::new(), ("0", "0"), ("0", "0"));
        let incoming = fixture(
            vec![tx(TransactionKind::Deposit, Currency::Krw, 3, 2, "10")],
            ("0", "0"),
            ("10", "0"),
        );
        assert_eq!(
            incoming.kr.snapshot_basis(confirmed(&empty.kp), &[]),
            Err(crate::CashUnavailable::IncompleteResponse)
        );
        let input = ledgers(&empty).with_transfer_classifications(&[], &[]);
        assert_eq!(
            input.load(at(3, 7), &empty.rows).values.get(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
    }
    #[test]
    fn per_holding_missing_report_and_sum_overflow_are_explicit() {
        let data = Portfolio::new(
            vec![holding(Currency::Krw, None, Some("100"))],
            BuyingPower::new(money("0", Currency::Krw), at(3, 7)),
            BuyingPower::new(money("0", Currency::Usd), at(3, 7)),
        )
        .expect("포트");
        let item = ValuationSnapshot::observe(&calendar(3), &data.evaluate(None), at(3, 7), None)
            .expect("조회");
        assert_eq!(
            item.reported_stock_daily(Currency::Krw),
            Err(SnapshotError::MissingReportedValue)
        );
        let mut second = holding(Currency::Krw, Some("79228162514264337593543950335"), None);
        let first = second.clone();
        second = crate::Holding::new(
            crate::ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("B").expect("심볼"),
                Market::Kr,
            )
            .expect("종목"),
            second.quantity(),
            second.acquisition_price(),
            crate::HoldingObservation::new(None, second.reported(), at(3, 7)),
        )
        .expect("보유");
        let data2 = Portfolio::new(
            vec![first, second],
            BuyingPower::new(money("0", Currency::Krw), at(3, 7)),
            BuyingPower::new(money("0", Currency::Usd), at(3, 7)),
        )
        .expect("포트");
        let item2 = ValuationSnapshot::observe(&calendar(3), &data2.evaluate(None), at(3, 7), None)
            .expect("조회");
        assert_eq!(
            item2.reported_stock_daily(Currency::Krw),
            Err(SnapshotError::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn each_market_requires_a_completed_prior_price_session() {
        let rows = crate::cash_ledger::tests::set(Vec::new());
        let kr = MarketCalendar::try_new(
            trading(2, &Market::Kr),
            trading(3, &Market::Kr),
            trading(4, &Market::Kr),
        )
        .expect("달력");
        let us = MarketCalendar::try_new(
            trading(4, &Market::Us),
            trading(5, &Market::Us),
            trading(6, &Market::Us),
        )
        .expect("달력");
        let cal = SnapshotCalendar::new(&kr, &us).expect("평가달력");
        assert_eq!(
            ValuationSnapshot::recalculate(
                &cal,
                &rows,
                &[],
                None,
                SnapshotObservation::new(at(3, 7), at(20, 0), None)
            ),
            Err(SnapshotError::MissingPrice)
        );
        let prior = session(
            2,
            &Market::Kr,
            Some(
                SessionData::try_new(
                    TimeInterval::try_new(at(2, 14), at(3, 0)).expect("구간"),
                    None,
                )
                .expect("세션"),
            ),
        );
        let kr2 = MarketCalendar::try_new(prior, trading(3, &Market::Kr), trading(4, &Market::Kr))
            .expect("달력");
        let us2 = MarketCalendar::try_new(
            trading(2, &Market::Us),
            trading(3, &Market::Us),
            trading(4, &Market::Us),
        )
        .expect("달력");
        let cal2 = SnapshotCalendar::new(&kr2, &us2).expect("평가달력");
        assert_eq!(
            ValuationSnapshot::recalculate(
                &cal2,
                &rows,
                &[],
                None,
                SnapshotObservation::new(at(2, 16), at(20, 0), None)
            ),
            Err(SnapshotError::MissingPrice)
        );
    }
    #[test]
    fn observed_period_and_recalculation_coverage_guards_are_public() {
        let data = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money("0", Currency::Krw), at(3, 7)),
            BuyingPower::new(money("0", Currency::Usd), at(3, 7)),
        )
        .expect("포트");
        assert_eq!(
            ValuationSnapshot::observe(&calendar(3), &data.evaluate(None), at(4, 7), None),
            Err(SnapshotError::WrongPeriod)
        );
        let f = fixture(
            vec![trade("A", Currency::Krw, 2, "1", "10")],
            ("10", "0"),
            ("0", "0"),
        );
        let current = snapshot(&f, 3, &[], "1");
        assert_eq!(
            current.stock_value(Currency::Krw),
            Err(SnapshotError::MissingHistory)
        );
        let item = history("A", Currency::Krw, &[(3, "10")]);
        let mut other = history("B", Currency::Krw, &[(3, "10")]);
        other.identity = LotIdentity::new(
            item.identity.instrument().clone(),
            Symbol::try_new("B").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        assert_eq!(
            ValuationSnapshot::recalculate(
                &calendar(3),
                &f.rows,
                &[item, other],
                None,
                SnapshotObservation::new(at(3, 7), at(20, 0), None)
            ),
            Err(SnapshotError::DuplicateInstrument)
        );
        let observed_snapshot =
            ValuationSnapshot::observe(&calendar(3), &data.evaluate(None), at(3, 7), None)
                .expect("조회");
        assert_eq!(
            observed_snapshot.price_basis(Currency::Usd),
            SnapshotPriceBasis::CurrentSession
        );
        assert_eq!(
            observed_snapshot
                .buying_power(Currency::Usd)
                .expect("주문 가능 금액")
                .currency(),
            Currency::Usd
        );
        let uncertain = FlowPeriod::Uncertain(crate::FlowUncertainty::OutsideCoverage);
        let old = observed_snapshot;
        let new = observed(4, "0", "0");
        assert_eq!(
            AccountChange::between(&new, Some(&old), (&uncertain, &f.up), &[])
                .currency(Currency::Krw),
            Err(SnapshotError::MissingFlows)
        );
    }
    #[test]
    #[should_panic(expected = "확정")]
    fn uncertain_fixture_cannot_supply_a_confirmed_period() {
        confirmed(&FlowPeriod::Uncertain(
            crate::FlowUncertainty::OutsideCoverage,
        ));
    }
    #[test]
    fn unknown_asset_metadata_and_foreign_subtotals_cannot_match_a_summary() {
        let mut row = trade("A", Currency::Usd, 2, "1", "10").data().clone();
        row.asset_class = None;
        row.symbol = None;
        row.quantity = None;
        let f = fixture(
            vec![Transaction::try_new(row).expect("메타데이터 누락")],
            ("0", "10"),
            ("0", "0"),
        );
        let current = snapshot(&f, 3, &[], "1");
        assert_eq!(
            current.stock_value(Currency::Usd),
            Err(SnapshotError::MissingHistory)
        );
        let old = observed(2, "0", "0");
        let rows = crate::cash_ledger::tests::set(Vec::new());
        let daily =
            HoldingsDailyProfit::compare(&current, Some(&old), &rows, Some(reported("0", None)));
        assert_eq!(
            daily.currency(Currency::Krw).comparison(),
            Comparison::Unavailable
        );
        let today = observed(4, "0", "0");
        let daily2 =
            HoldingsDailyProfit::compare(&today, Some(&current), &rows, Some(reported("0", None)));
        assert_eq!(
            daily2.currency(Currency::Krw).comparison(),
            Comparison::Unavailable
        );
    }
    pub(crate) fn set_exchange(snapshot: &mut ValuationSnapshot, row: Transaction) {
        snapshot.inputs.cash.exchanges = vec![row];
    }
}
