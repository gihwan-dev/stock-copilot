use crate::{
    CalendarDate, Candle, CandlePeriod, CandleStatus, CorporateAction, CostBasisMethod, Currency,
    CurrencyAmounts, FxRate, FxRateKind, InstrumentId, KrwValuation, KrwValuationComponent,
    LotAssumption, LotBook, LotError, LotIdentity, LotListing, Market, MarketCalendar, Money,
    MoneyError, PortfolioValuation, PriceAdjustment, SettledRowSet, Timestamp, Total,
    TotalCompleteness,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    CalendarMismatch,
    ClosedDay,
    MissingPrice,
    InvalidCandle,
    MissingFx,
    InvalidFx,
    MissingCash,
    IncompleteValuation,
    MissingPrevious,
    WrongPeriod,
    MissingFlows,
    DuplicateInstrument,
    TooManyInputs,
    PendingCorporateCash,
    Lot(LotError),
    Arithmetic(MoneyError),
    Conversion(crate::FxConversionError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotObservation {
    at: Timestamp,
    fx: Option<FxRate>,
}
impl SnapshotObservation {
    #[must_use]
    pub const fn new(at: Timestamp, fx: Option<FxRate>) -> Self {
        Self { at, fx }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSource {
    Observed,
    Recalculated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotDates {
    krw: CalendarDate,
    usd: CalendarDate,
}
impl SnapshotDates {
    #[must_use]
    pub const fn date(self, currency: Currency) -> CalendarDate {
        match currency {
            Currency::Krw => self.krw,
            Currency::Usd => self.usd,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCalendar {
    today: SnapshotDates,
    previous: SnapshotDates,
}
impl SnapshotCalendar {
    pub fn new(kr: &MarketCalendar, us: &MarketCalendar) -> Result<Self, SnapshotError> {
        if kr.today().market() != Market::Kr || us.today().market() != Market::Us {
            return Err(SnapshotError::CalendarMismatch);
        }
        if kr.today().sessions().is_empty() || us.today().sessions().is_empty() {
            return Err(SnapshotError::ClosedDay);
        }
        Ok(Self {
            today: SnapshotDates {
                krw: kr.today().date(),
                usd: us.today().date(),
            },
            previous: SnapshotDates {
                krw: kr.previous_business_day().date(),
                usd: us.previous_business_day().date(),
            },
        })
    }
    #[must_use]
    pub const fn dates(self) -> SnapshotDates {
        self.today
    }
    #[must_use]
    pub const fn previous_dates(self) -> SnapshotDates {
        self.previous
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCashInput {
    dates: SnapshotDates,
    amounts: Option<CurrencyAmounts>,
}
impl SnapshotCashInput {
    #[must_use]
    pub const fn reconciled(calendar: &SnapshotCalendar, amounts: CurrencyAmounts) -> Self {
        Self {
            dates: calendar.today,
            amounts: Some(amounts),
        }
    }
    #[must_use]
    pub const fn unavailable(calendar: &SnapshotCalendar) -> Self {
        Self {
            dates: calendar.today,
            amounts: None,
        }
    }
    pub fn amount(self, currency: Currency) -> Result<Money, SnapshotError> {
        self.amounts
            .map(|amounts| amounts.resolved(currency))
            .ok_or(SnapshotError::MissingCash)
    }
}
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationSnapshot {
    dates: SnapshotDates,
    recorded_at: Timestamp,
    source: SnapshotSource,
    krw: Result<Money, SnapshotError>,
    usd: Result<Money, SnapshotError>,
    converted: Result<KrwValuation, SnapshotError>,
    fx: Option<FxRate>,
    positions: Vec<SnapshotPosition>,
    assumptions: BTreeSet<LotAssumption>,
}
impl ValuationSnapshot {
    #[must_use]
    pub fn observe(
        calendar: &SnapshotCalendar,
        portfolio: &PortfolioValuation,
        at: Timestamp,
        fx: Option<FxRate>,
    ) -> Self {
        let value = |currency| {
            let stock = match portfolio.currency_total(currency) {
                Total::Complete(value) => Ok(*value.value()),
                Total::Incomplete(_) => Err(SnapshotError::IncompleteValuation),
            }?;
            stock
                .checked_add(
                    &portfolio
                        .buying_power(currency)
                        .money()
                        .map_err(|_error| SnapshotError::MissingCash)?,
                )
                .map_err(SnapshotError::Arithmetic)
        };
        Self::finish(
            (calendar.today, at, SnapshotSource::Observed),
            (value(Currency::Krw), value(Currency::Usd)),
            fx,
            Vec::new(),
        )
    }
    pub fn recalculate(
        calendar: &SnapshotCalendar,
        rows: &SettledRowSet,
        histories: &[SnapshotInstrumentHistory],
        cash: SnapshotCashInput,
        observation: SnapshotObservation,
    ) -> Result<Self, SnapshotError> {
        if histories.len() > 4096 {
            return Err(SnapshotError::TooManyInputs);
        }
        if cash.dates != calendar.today {
            return Err(SnapshotError::WrongPeriod);
        }
        let mut ordered = histories.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|history| history.identity.instrument());
        if ordered
            .array_windows::<2>()
            .any(|[left, right]| left.identity.instrument() == right.identity.instrument())
        {
            return Err(SnapshotError::DuplicateInstrument);
        }
        let mut krw = cash.amount(Currency::Krw);
        let mut usd = cash.amount(Currency::Usd);
        let mut positions = Vec::new();
        for history in ordered {
            let currency = history.identity.currency();
            let day = calendar.today.date(currency);
            let actions = history
                .actions
                .iter()
                .filter(|action| action.date() <= day)
                .cloned()
                .collect::<Vec<_>>();
            let book = LotBook::from_settled(
                rows,
                history.identity.clone(),
                CostBasisMethod::Fifo,
                &actions,
                day,
            );
            let candidates = history
                .candles
                .iter()
                .filter(|candle| candle.time().date() == day)
                .collect::<Vec<_>>();
            let candle = candidates.first().copied().cloned();
            let value = position_value(&book, &candidates, &history.identity);
            let total = match currency {
                Currency::Krw => &mut krw,
                Currency::Usd => &mut usd,
            };
            *total =
                total.and_then(|sum| sum.checked_add(&value?).map_err(SnapshotError::Arithmetic));
            positions.push(SnapshotPosition {
                instrument: history.identity.instrument().clone(),
                book,
                candle,
                value,
            });
        }
        Ok(Self::finish(
            (calendar.today, observation.at, SnapshotSource::Recalculated),
            (krw, usd),
            observation.fx,
            positions,
        ))
    }
    fn finish(
        context: (SnapshotDates, Timestamp, SnapshotSource),
        values: (Result<Money, SnapshotError>, Result<Money, SnapshotError>),
        fx: Option<FxRate>,
        positions: Vec<SnapshotPosition>,
    ) -> Self {
        let (dates, recorded_at, source) = context;
        let (krw, usd) = values;
        let converted = krw.and_then(|native| aggregate(native, usd?, fx, dates.usd));
        let assumptions = positions
            .iter()
            .filter_map(|position| position.book.as_ref().ok())
            .flat_map(|book| book.assumptions().iter().copied())
            .collect();
        Self {
            dates,
            recorded_at,
            source,
            krw,
            usd,
            converted,
            fx,
            positions,
            assumptions,
        }
    }
    #[must_use]
    pub const fn dates(&self) -> SnapshotDates {
        self.dates
    }
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
    #[must_use]
    pub const fn source(&self) -> SnapshotSource {
        self.source
    }
    #[must_use]
    pub fn completeness(&self) -> TotalCompleteness {
        if self.converted.is_ok() {
            TotalCompleteness::Complete
        } else {
            TotalCompleteness::Incomplete
        }
    }
    pub const fn currency_value(&self, currency: Currency) -> Result<Money, SnapshotError> {
        match currency {
            Currency::Krw => self.krw,
            Currency::Usd => self.usd,
        }
    }
    pub fn krw_value(&self) -> Result<&KrwValuation, SnapshotError> {
        self.converted.as_ref().map_err(|error| *error)
    }
    #[must_use]
    pub const fn fx(&self) -> Option<FxRate> {
        self.fx
    }
    #[must_use]
    pub fn positions(&self) -> &[SnapshotPosition] {
        &self.positions
    }
    #[must_use]
    pub const fn assumptions(&self) -> &BTreeSet<LotAssumption> {
        &self.assumptions
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
    if book.listing() == LotListing::Delisted
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
    day: CalendarDate,
) -> Result<KrwValuation, SnapshotError> {
    let result = KrwValuation::from_native_krw(native).map_err(SnapshotError::Conversion)?;
    if foreign.amount().as_decimal().is_zero() {
        return Ok(result);
    }
    let rate = fx.ok_or(SnapshotError::MissingFx)?;
    if rate.kind() != FxRateKind::Mid || rate.as_of().offset_calendar_date() != day {
        return Err(SnapshotError::InvalidFx);
    }
    let converted = rate
        .convert_for_aggregation(foreign)
        .map_err(SnapshotError::Conversion)?;
    result
        .checked_add(&KrwValuation::from_component(
            KrwValuationComponent::Converted(converted),
        ))
        .map_err(SnapshotError::Conversion)
}

fn lot_error(error: LotError) -> SnapshotError {
    SnapshotError::Lot(error)
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, AdjustmentBasis, AssetClass, BuyingPower, Cancellation, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, Comparison, CorporateAction,
        CorporateActionId, CorporateActionKind, Currency, CurrencyAmounts, DailyPortfolioProfit,
        FxConversionError, FxPair, FxRate, FxRateKind, FxSource, InstrumentId, LotError,
        LotIdentity, Market, MarketCalendar, MarketSession, Money, MoneyError, Nanosecond,
        NumericInput, OffsetSeconds, Ohlcv, Portfolio, PortfolioDailyProfitLoss, Price,
        PriceAdjustment, Quantity, Ratio, SessionData, SessionInformation, SettledRowSet,
        Settlement, ShareCount, ShareRatio, SnapshotCalendar, SnapshotCashInput, SnapshotError,
        SnapshotExternalFlows, SnapshotInstrumentHistory, SnapshotObservation, SnapshotPrevious,
        SnapshotSource, SpanSeconds, StoredTransaction, Symbol, TimeInterval, TimeSpan, Timestamp,
        TotalCompleteness, Transaction, TransactionBatch, TransactionData, TransactionKind,
        UnixSeconds, ValuationAccuracy, ValuationSnapshot, ValuationSource,
    };

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn at(day: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_767_225_600_i64
                    .checked_add(day.checked_mul(86_400).expect("일"))
                    .expect("초"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn session(day: i64, market: &Market) -> MarketSession {
        let data = Some(
            SessionData::try_new(
                TimeInterval::try_new(
                    at(day),
                    at(day)
                        .checked_add(
                            TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0))
                                .expect("기간"),
                        )
                        .expect("끝"),
                )
                .expect("구간"),
                None,
            )
            .expect("세션"),
        );
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        MarketSession::try_new(at(day).offset_calendar_date(), info).expect("날짜")
    }
    fn calendar(day: i64) -> SnapshotCalendar {
        let make = |market: Market| {
            MarketCalendar::try_new(
                session(day.checked_sub(1).expect("전일"), &market),
                session(day, &market),
                session(day.checked_add(1).expect("다음"), &market),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(Market::Kr), &make(Market::Us)).expect("평가 달력")
    }
    fn rate(day: i64, value: &str) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(day), at(day.checked_add(1).expect("다음"))).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn observed(day: i64, krw: &str, usd: &str, fx: Option<FxRate>) -> ValuationSnapshot {
        let book = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money(krw, Currency::Krw), at(day)),
            BuyingPower::new(money(usd, Currency::Usd), at(day)),
        )
        .expect("포트");
        ValuationSnapshot::observe(&calendar(day), &book.evaluate(fx), at(day), fx)
    }

    #[test]
    fn r11_example_seven_and_actual_provenance_preserve_mid_rate() {
        let fx = rate(2, "1375");
        let snapshot = observed(2, "10000000", "5000.00", Some(fx));
        assert_eq!(snapshot.source(), SnapshotSource::Observed);
        assert_eq!(snapshot.completeness(), TotalCompleteness::Complete);
        assert_eq!(
            snapshot.currency_value(Currency::Usd),
            Ok(money("5000.00", Currency::Usd))
        );
        assert_eq!(
            snapshot.krw_value().expect("환산").clone().into_parts(),
            (
                money("16875000", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
    }

    #[test]
    fn r11_example_six_daily_includes_exchange_effect_and_toss_is_selected() {
        let previous = observed(1, "0", "7000", Some(rate(1, "1200")));
        let today = observed(2, "0", "6500", Some(rate(2, "1100")));
        let flows = SnapshotExternalFlows::classified(
            &calendar(2),
            CurrencyAmounts::new(money("0", Currency::Krw), Some(money("0", Currency::Usd)))
                .expect("흐름"),
        );
        let daily =
            DailyPortfolioProfit::calculate(&calendar(2), &today, Some(&previous), &flows, None);
        assert_eq!(
            daily.currency(Currency::Usd).computed(),
            Ok(money("-500", Currency::Usd))
        );
        assert_eq!(
            daily.krw_computed().expect("원화").clone().into_parts().0,
            money("-1250000", Currency::Krw)
        );
        assert_eq!(daily.previous(), SnapshotPrevious::Observed);
        let reported = PortfolioDailyProfitLoss::new(
            CurrencyAmounts::new(
                money("-100.00", Currency::Krw),
                Some(money("-490.00", Currency::Usd)),
            )
            .expect("보고"),
            Ratio::parse(&NumericInput::try_new("-0.1").expect("입력")).expect("율"),
        );
        let selected = DailyPortfolioProfit::calculate(
            &calendar(2),
            &today,
            Some(&previous),
            &flows,
            Some(reported),
        );
        assert_eq!(
            selected
                .currency(Currency::Usd)
                .selected()
                .expect("정본")
                .value(),
            money("-490.00", Currency::Usd)
        );
        assert_eq!(
            selected.currency(Currency::Usd).source(),
            Some(ValuationSource::TossOpenApi)
        );
        assert_eq!(
            selected.currency(Currency::Usd).comparison(),
            Comparison::Different
        );
        assert_eq!(selected.reported(), Some(reported));
    }

    #[test]
    fn missing_previous_or_fx_never_becomes_zero_profit() {
        let today = observed(2, "10", "3", None);
        assert_eq!(today.completeness(), TotalCompleteness::Incomplete);
        assert_eq!(today.krw_value(), Err(SnapshotError::MissingFx));
        let flows = SnapshotExternalFlows::unavailable(&calendar(2));
        let daily = DailyPortfolioProfit::calculate(&calendar(2), &today, None, &flows, None);
        assert_eq!(daily.previous(), SnapshotPrevious::Missing);
        assert_eq!(
            daily.currency(Currency::Krw).computed(),
            Err(SnapshotError::MissingPrevious)
        );
        assert_eq!(
            daily.currency(Currency::Krw).comparison(),
            Comparison::NotProvided
        );
    }

    fn identity(symbol: &str) -> LotIdentity {
        LotIdentity::new(
            InstrumentId::try_new(symbol).expect("ID"),
            Symbol::try_new(symbol).expect("심볼"),
            Market::Us,
            Currency::Usd,
        )
    }
    fn buy(symbol: &str, day: i64, quantity: &str) -> Transaction {
        Transaction::try_new(TransactionData {
            kind: TransactionKind::Buy,
            market: Market::Us,
            currency: Currency::Usd,
            symbol: Some(Symbol::try_new(symbol).expect("심볼")),
            asset_class: Some(AssetClass::Stock),
            isin: None,
            quantity: Some(
                Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"),
            ),
            filled_price: None,
            amount: Some(money("100", Currency::Usd)),
            commission: money("0", Currency::Usd),
            tax: money("0", Currency::Usd),
            net_amount: Some(money("100", Currency::Usd)),
            exchange_rate: None,
            settlement_date: at(day).offset_calendar_date(),
            settlement: Settlement::Settled,
            transaction_time: Some(at(day)),
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    fn rows(transactions: Vec<Transaction>) -> SettledRowSet {
        SettledRowSet::try_restore(
            AccountId::new(1),
            transactions
                .into_iter()
                .map(|row| StoredTransaction::try_restore(row, at(4), None).expect("저장"))
                .collect(),
            TransactionBatch::try_new(Vec::new()).expect("미결제"),
            Some(at(4)),
        )
        .expect("원장")
    }
    fn candle(
        day: i64,
        symbol: &str,
        close: &str,
        adjustment: PriceAdjustment,
        status: CandleStatus,
    ) -> Candle {
        let price = Price::try_new(money(close, Currency::Usd)).expect("종가");
        Candle::try_new(
            InstrumentId::try_new(symbol).expect("ID"),
            CandleTime::try_new(at(day), CandlePeriod::Daily).expect("봉 시각"),
            adjustment,
            Ohlcv::try_new(
                price,
                price,
                price,
                price,
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("거래량"),
            )
            .expect("OHLC"),
            CandleObservation::new(at(4), status),
        )
        .expect("봉")
    }
    fn cash(day: i64) -> SnapshotCashInput {
        SnapshotCashInput::reconciled(
            &calendar(day),
            CurrencyAmounts::new(money("0", Currency::Krw), Some(money("0", Currency::Usd)))
                .expect("현금"),
        )
    }
    fn reconstruct(
        day: i64,
        history: &[SnapshotInstrumentHistory],
        state: &SettledRowSet,
    ) -> ValuationSnapshot {
        ValuationSnapshot::recalculate(
            &calendar(day),
            state,
            history,
            cash(day),
            SnapshotObservation::new(at(4), Some(rate(day, "1375"))),
        )
        .expect("재계산")
    }

    #[test]
    fn historical_split_uses_settlement_quantity_and_unadjusted_close() {
        let state = rows(vec![buy("A", 1, "10"), buy("A", 3, "5")]);
        let split = CorporateAction::try_new(
            CorporateActionId::new(InstrumentId::try_new("split").expect("ID")),
            at(2).offset_calendar_date(),
            CorporateActionKind::Split(ShareRatio::new(
                ShareCount::try_new(1).expect("구주"),
                ShareCount::try_new(2).expect("신주"),
            )),
        )
        .expect("분할");
        let history = SnapshotInstrumentHistory::new(
            identity("A"),
            vec![split],
            vec![
                candle(1, "A", "100", PriceAdjustment::Raw, CandleStatus::Finalized),
                candle(2, "A", "50", PriceAdjustment::Raw, CandleStatus::Finalized),
            ],
        )
        .expect("이력");
        for day in [1, 2] {
            let result = reconstruct(day, std::slice::from_ref(&history), &state);
            assert_eq!(result.source(), SnapshotSource::Recalculated);
            assert_eq!(
                result.currency_value(Currency::Usd),
                Ok(money("1000", Currency::Usd))
            );
            let position = result.positions().first().expect("종목");
            assert_eq!(position.instrument(), identity("A").instrument());
            assert_eq!(position.value(), Ok(money("1000", Currency::Usd)));
            assert_eq!(
                position.candle().expect("봉").time().date(),
                at(day).offset_calendar_date()
            );
            let _book = position.book().expect("로트");
            assert!(result.assumptions().is_empty());
            assert_eq!(result.recorded_at(), at(4));
        }
        let old = reconstruct(1, &[history], &state);
        let current = observed(2, "0", "1000", Some(rate(2, "1375")));
        let flows = SnapshotExternalFlows::classified(
            &calendar(2),
            CurrencyAmounts::new(money("0", Currency::Krw), None).expect("흐름"),
        );
        assert_eq!(
            DailyPortfolioProfit::calculate(&calendar(2), &current, Some(&old), &flows, None)
                .previous(),
            SnapshotPrevious::Recalculated
        );
    }

    #[test]
    fn missing_wrong_live_adjusted_or_duplicate_candles_remain_incomplete() {
        let state = rows(vec![buy("A", 1, "10")]);
        let raw = candle(2, "A", "100", PriceAdjustment::Raw, CandleStatus::Finalized);
        for candles in [
            Vec::new(),
            vec![raw.clone(), raw],
            vec![candle(
                2,
                "B",
                "100",
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
            )],
            vec![candle(
                2,
                "A",
                "100",
                PriceAdjustment::Adjusted(AdjustmentBasis::new(at(3))),
                CandleStatus::Finalized,
            )],
            vec![candle(
                2,
                "A",
                "100",
                PriceAdjustment::Raw,
                CandleStatus::InProgress,
            )],
        ] {
            let history =
                SnapshotInstrumentHistory::new(identity("A"), Vec::new(), candles).expect("이력");
            let result = reconstruct(2, &[history], &state);
            assert_eq!(result.completeness(), TotalCompleteness::Incomplete);
            let _error = result.currency_value(Currency::Usd).expect_err("불완전");
        }
        let empty =
            SnapshotInstrumentHistory::new(identity("B"), Vec::new(), Vec::new()).expect("빈 보유");
        assert_eq!(
            reconstruct(2, &[empty], &state).currency_value(Currency::Usd),
            Ok(money("0", Currency::Usd))
        );
    }

    #[test]
    fn independent_integer_reference_order_and_repeatability_for_4096_cases() {
        use proptest::{
            prop_assert_eq,
            test_runner::{Config, TestRunner},
        };
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_i64..1000, 1_i64..1000, 1_i64..1000),
                |(quantity, close, exchange)| {
                    let state = rows(vec![buy("A", 1, &scaled_text(quantity)), buy("B", 1, "2")]);
                    let input = vec![history("A", &scaled_text(close)), history("B", "3")];
                    let observation =
                        SnapshotObservation::new(at(4), Some(rate(2, &scaled_text(exchange))));
                    let result = ValuationSnapshot::recalculate(
                        &calendar(2),
                        &state,
                        &input,
                        cash(2),
                        observation,
                    )
                    .expect("평가");
                    let expected = i128::from(quantity)
                        .checked_mul(i128::from(close))
                        .expect("정수 곱")
                        .checked_add(60_000)
                        .expect("합");
                    prop_assert_eq!(
                        result.currency_value(Currency::Usd),
                        Ok(reference_money(expected, 4, Currency::Usd))
                    );
                    prop_assert_eq!(
                        result.krw_value().expect("환산").clone().into_parts().0,
                        reference_money(
                            expected
                                .checked_mul(i128::from(exchange))
                                .expect("환산 정수"),
                            6,
                            Currency::Krw
                        )
                    );
                    let mut reversed = input.clone();
                    reversed.reverse();
                    let evaluate = |inputs: &[SnapshotInstrumentHistory]| {
                        ValuationSnapshot::recalculate(
                            &calendar(2),
                            &state,
                            inputs,
                            cash(2),
                            observation,
                        )
                        .expect("평가")
                    };
                    prop_assert_eq!(&result, &evaluate(&reversed));
                    prop_assert_eq!(result, evaluate(&input));
                    Ok(())
                },
            )
            .expect("독립 정수 기준");
    }

    fn history(symbol: &str, close: &str) -> SnapshotInstrumentHistory {
        SnapshotInstrumentHistory::new(
            identity(symbol),
            Vec::new(),
            vec![candle(
                2,
                symbol,
                close,
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
            )],
        )
        .expect("이력")
    }

    #[test]
    fn input_bounds_missing_cash_duplicates_and_wrong_period_are_rejected() {
        let item = history("A", "100");
        assert_eq!(item.identity(), &identity("A"));
        let observation = SnapshotObservation::new(at(4), Some(rate(2, "1375")));
        let state = rows(vec![buy("A", 1, "10")]);
        for (inputs, cash_input, expected) in [
            (
                vec![item.clone(); 4097],
                cash(2),
                SnapshotError::TooManyInputs,
            ),
            (
                vec![item.clone(); 2],
                cash(2),
                SnapshotError::DuplicateInstrument,
            ),
            (vec![item.clone()], cash(1), SnapshotError::WrongPeriod),
        ] {
            assert_eq!(
                ValuationSnapshot::recalculate(
                    &calendar(2),
                    &state,
                    &inputs,
                    cash_input,
                    observation
                ),
                Err(expected)
            );
        }
        let missing = SnapshotCashInput::unavailable(&calendar(2));
        assert_eq!(
            missing.amount(Currency::Usd),
            Err(SnapshotError::MissingCash)
        );
        let partial =
            ValuationSnapshot::recalculate(&calendar(2), &state, &[item], missing, observation)
                .expect("부분 평가");
        assert_eq!(partial.completeness(), TotalCompleteness::Incomplete);
        assert_eq!(
            partial.currency_value(Currency::Krw),
            Err(SnapshotError::MissingCash)
        );
    }
    #[test]
    fn candle_action_bounds_and_delisting_remain_explicit() {
        let state = rows(vec![buy("A", 1, "10")]);
        let example = candle(2, "A", "1", PriceAdjustment::Raw, CandleStatus::Finalized);
        assert_eq!(
            SnapshotInstrumentHistory::new(identity("A"), Vec::new(), vec![example; 4097]),
            Err(SnapshotError::TooManyInputs)
        );
        let action = CorporateAction::try_new(
            CorporateActionId::new(InstrumentId::try_new("D").expect("ID")),
            at(2).offset_calendar_date(),
            CorporateActionKind::Delisting,
        )
        .expect("폐지");
        assert_eq!(
            SnapshotInstrumentHistory::new(identity("A"), vec![action.clone(); 4097], Vec::new()),
            Err(SnapshotError::TooManyInputs)
        );
        let delisted =
            SnapshotInstrumentHistory::new(identity("A"), vec![action], Vec::new()).expect("이력");
        assert_eq!(
            reconstruct(2, &[delisted], &state).currency_value(Currency::Usd),
            Err(SnapshotError::PendingCorporateCash)
        );
    }

    #[test]
    fn invalid_lot_and_fx_arithmetic_failures_remain_explicit() {
        let bad_identity = LotIdentity::new(
            InstrumentId::try_new("A").expect("ID"),
            Symbol::try_new("A").expect("심볼"),
            Market::Kr,
            Currency::Usd,
        );
        let bad =
            SnapshotInstrumentHistory::new(bad_identity, Vec::new(), Vec::new()).expect("이력");
        let partial = reconstruct(2, &[bad], &rows(Vec::new()));
        assert_eq!(
            partial.currency_value(Currency::Usd),
            Err(SnapshotError::Lot(LotError::InvalidIdentity))
        );
        let native = observed(2, "1", "0", None);
        assert_eq!(
            native.krw_value().expect("원화").accuracy(),
            ValuationAccuracy::Exact
        );
        for fx in [
            rate(1, "1375"),
            FxRate::parse(
                &NumericInput::try_new("1375").expect("입력"),
                FxPair::UsdKrw,
                FxRateKind::Buy,
                TimeInterval::try_new(at(2), at(3)).expect("구간"),
                FxSource::TossOpenApi,
            )
            .expect("매수 환율"),
        ] {
            assert_eq!(
                observed(2, "1", "1", Some(fx)).krw_value(),
                Err(SnapshotError::InvalidFx)
            );
        }
        assert_eq!(
            observed(2, "-1", "0", None).currency_value(Currency::Krw),
            Err(SnapshotError::MissingCash)
        );
        assert_eq!(
            observed(2, "0", "79228162514264337593543950335", Some(rate(2, "2"))).krw_value(),
            Err(SnapshotError::Conversion(FxConversionError::Arithmetic(
                MoneyError::Overflow
            )))
        );
    }

    #[test]
    fn calendars_validate_markets_holidays_and_keep_local_dates() {
        let make = |market: &Market| {
            MarketCalendar::try_new(session(1, market), session(2, market), session(3, market))
                .expect("달력")
        };
        assert_eq!(
            SnapshotCalendar::new(&make(&Market::Us), &make(&Market::Kr)),
            Err(SnapshotError::CalendarMismatch)
        );
        let holiday = MarketSession::try_new(
            at(2).offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("휴장");
        let closed =
            MarketCalendar::try_new(session(1, &Market::Kr), holiday, session(3, &Market::Kr))
                .expect("휴장 달력");
        assert_eq!(
            SnapshotCalendar::new(&closed, &make(&Market::Us)),
            Err(SnapshotError::ClosedDay)
        );
        assert_eq!(
            calendar(2).dates().date(Currency::Krw),
            at(2).offset_calendar_date()
        );
        assert_eq!(
            calendar(2).previous_dates().date(Currency::Usd),
            at(1).offset_calendar_date()
        );
    }

    fn flows(day: i64, native: &str, foreign: &str) -> SnapshotExternalFlows {
        SnapshotExternalFlows::classified(
            &calendar(day),
            CurrencyAmounts::new(
                money(native, Currency::Krw),
                Some(money(foreign, Currency::Usd)),
            )
            .expect("흐름"),
        )
    }
    #[test]
    fn daily_external_deposit_withdrawal_selection_and_missing_inputs() {
        let old = observed(1, "1000", "10", Some(rate(1, "1200")));
        let current = observed(2, "1500", "9", Some(rate(2, "1300")));
        let external = flows(2, "500", "-1");
        assert_eq!(
            external.amount(Currency::Usd),
            Ok(money("-1", Currency::Usd))
        );
        let reported = PortfolioDailyProfitLoss::new(
            CurrencyAmounts::new(
                money("0.00", Currency::Krw),
                Some(money("0.00", Currency::Usd)),
            )
            .expect("통화"),
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("율"),
        );
        let result = DailyPortfolioProfit::calculate(
            &calendar(2),
            &current,
            Some(&old),
            &external,
            Some(reported),
        );
        for currency in [Currency::Krw, Currency::Usd] {
            assert_eq!(
                result.currency(currency).computed(),
                Ok(money("0", currency))
            );
            assert_eq!(result.currency(currency).comparison(), Comparison::Matches);
        }
        assert_eq!(
            result
                .krw_computed()
                .expect("환 효과")
                .clone()
                .into_parts()
                .0,
            money("1000", Currency::Krw)
        );
        assert_eq!(
            result
                .krw_selected()
                .expect("정본 합")
                .clone()
                .into_parts()
                .0,
            money("0", Currency::Krw)
        );
        assert_eq!(result.krw_comparison(), Comparison::Unavailable);
        assert_eq!(result.current_source(), SnapshotSource::Observed);
    }
    #[test]
    fn daily_missing_inputs_keep_toss_and_comparison_unavailable() {
        let old = observed(1, "1000", "10", Some(rate(1, "1200")));
        let current = observed(2, "1500", "9", Some(rate(2, "1300")));
        let external = flows(2, "500", "-1");
        let reported = PortfolioDailyProfitLoss::new(
            CurrencyAmounts::new(
                money("0.00", Currency::Krw),
                Some(money("0.00", Currency::Usd)),
            )
            .expect("통화"),
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("율"),
        );
        let missing = DailyPortfolioProfit::calculate(
            &calendar(2),
            &current,
            None,
            &external,
            Some(reported),
        );
        assert_eq!(
            missing.currency(Currency::Usd).comparison(),
            Comparison::Unavailable
        );
        assert_eq!(
            missing
                .currency(Currency::Usd)
                .selected()
                .expect("정본")
                .source(),
            ValuationSource::TossOpenApi
        );
        let no_flows = DailyPortfolioProfit::calculate(
            &calendar(2),
            &current,
            Some(&old),
            &SnapshotExternalFlows::unavailable(&calendar(2)),
            None,
        );
        assert_eq!(
            no_flows.currency(Currency::Krw).computed(),
            Err(SnapshotError::MissingFlows)
        );
        assert_eq!(no_flows.currency(Currency::Krw).source(), None);
        assert_eq!(no_flows.krw_comparison(), Comparison::NotProvided);
        let incomplete = observed(1, "0", "1", None);
        assert_eq!(
            DailyPortfolioProfit::calculate(
                &calendar(2),
                &current,
                Some(&incomplete),
                &external,
                None
            )
            .previous(),
            SnapshotPrevious::Incomplete
        );
    }

    #[test]
    fn daily_wrong_dates_and_signed_flow_overflow_are_not_silenced() {
        let current = observed(2, "1", "0", None);
        let previous = observed(1, "0", "0", None);
        for (today, old, external) in [
            (&previous, &previous, flows(2, "0", "0")),
            (&current, &current, flows(2, "0", "0")),
            (&current, &previous, flows(1, "0", "0")),
        ] {
            let result =
                DailyPortfolioProfit::calculate(&calendar(2), today, Some(old), &external, None);
            assert_eq!(
                result.currency(Currency::Krw).computed(),
                Err(SnapshotError::WrongPeriod)
            );
        }
        let result = DailyPortfolioProfit::calculate(
            &calendar(2),
            &current,
            Some(&previous),
            &flows(2, "-79228162514264337593543950335", "0"),
            None,
        );
        assert_eq!(
            result.currency(Currency::Krw).computed(),
            Err(SnapshotError::Arithmetic(MoneyError::Overflow))
        );
    }

    fn reference_money(coefficient: i128, scale: u32, currency: Currency) -> Money {
        Money::new(
            crate::Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                coefficient,
                scale,
            )),
            currency,
        )
    }

    fn scaled_text(value: i64) -> String {
        rust_decimal::Decimal::new(value, 2).to_string()
    }
    fn missing_stock(day: i64, currency: Currency) -> ValuationSnapshot {
        let market = match currency {
            Currency::Krw => Market::Kr,
            Currency::Usd => Market::Us,
        };
        let item = crate::Holding::new(
            crate::ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("A").expect("심볼"),
                market,
            )
            .expect("종목"),
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            crate::AcquisitionPrice::try_new(money("1", currency)).expect("취득가"),
            crate::HoldingObservation::new(None, None, at(day)),
        )
        .expect("보유");
        let portfolio = Portfolio::new(
            vec![item],
            BuyingPower::new(money("0", Currency::Krw), at(day)),
            BuyingPower::new(money("0", Currency::Usd), at(day)),
        )
        .expect("포트");
        ValuationSnapshot::observe(&calendar(day), &portfolio.evaluate(None), at(day), None)
    }
    #[test]
    fn incomplete_currency_and_selection_paths_are_visible_independently() {
        let current = observed(2, "1", "1", Some(rate(2, "1")));
        for currency in [Currency::Krw, Currency::Usd] {
            let old = missing_stock(1, currency);
            assert_eq!(
                old.currency_value(currency),
                Err(SnapshotError::IncompleteValuation)
            );
            let result = DailyPortfolioProfit::calculate(
                &calendar(2),
                &current,
                Some(&old),
                &flows(2, "0", "0"),
                None,
            );
            assert_eq!(
                result.currency(currency).computed(),
                Err(SnapshotError::IncompleteValuation)
            );
            assert_eq!(
                result.krw_computed(),
                Err(SnapshotError::IncompleteValuation)
            );
        }
        let old = observed(1, "0", "0", None);
        let broken_today = observed(2, "0", "1", None);
        let result = DailyPortfolioProfit::calculate(
            &calendar(2),
            &broken_today,
            Some(&old),
            &flows(2, "0", "0"),
            None,
        );
        assert_eq!(result.krw_computed(), Err(SnapshotError::MissingFx));
        let foreign_flow = DailyPortfolioProfit::calculate(
            &calendar(2),
            &observed(2, "0", "0", None),
            Some(&old),
            &flows(2, "0", "1"),
            None,
        );
        assert_eq!(foreign_flow.krw_computed(), Err(SnapshotError::MissingFx));
        let reported = PortfolioDailyProfitLoss::new(
            CurrencyAmounts::new(money("1", Currency::Krw), None).expect("금액"),
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("율"),
        );
        let selected = DailyPortfolioProfit::calculate(
            &calendar(2),
            &current,
            None,
            &flows(2, "0", "0"),
            Some(reported),
        );
        assert_eq!(selected.krw_selected(), Err(SnapshotError::MissingPrevious));
    }
    #[test]
    fn domestic_history_and_aggregation_currency_contract_are_covered() {
        let native_identity = LotIdentity::new(
            InstrumentId::try_new("KR").expect("ID"),
            Symbol::try_new("KR").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        let empty = SnapshotInstrumentHistory::new(native_identity, Vec::new(), Vec::new())
            .expect("국내 이력");
        assert_eq!(
            reconstruct(2, &[empty], &rows(Vec::new())).currency_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            crate::valuation_snapshot::aggregate(
                money("1", Currency::Usd),
                money("0", Currency::Usd),
                None,
                at(2).offset_calendar_date()
            ),
            Err(SnapshotError::Conversion(
                FxConversionError::CurrencyMismatch
            ))
        );
        let result = DailyPortfolioProfit::calculate(
            &calendar(2),
            &observed(2, "0", "0", None),
            Some(&observed(1, "-1", "0", None)),
            &flows(2, "0", "0"),
            None,
        );
        assert_eq!(result.krw_computed(), Err(SnapshotError::MissingCash));
    }
    #[test]
    fn quantity_failure_and_current_missing_price_are_not_zero() {
        assert_eq!(
            super::price_quantity(Err(LotError::InvalidQuantity), &[], &identity("A")),
            Err(SnapshotError::Lot(LotError::InvalidQuantity))
        );
        let previous = observed(1, "0", "0", None);
        for currency in [Currency::Krw, Currency::Usd] {
            let current = missing_stock(2, currency);
            let result = DailyPortfolioProfit::calculate(
                &calendar(2),
                &current,
                Some(&previous),
                &flows(2, "0", "0"),
                None,
            );
            assert_eq!(
                result.currency(currency).computed(),
                Err(SnapshotError::IncompleteValuation)
            );
        }
    }
    #[test]
    fn fractional_cash_and_pending_share_delivery_are_incomplete() {
        let ratio = ShareRatio::new(
            ShareCount::try_new(3).expect("구주"),
            ShareCount::try_new(1).expect("신주"),
        );
        for kind in [
            CorporateActionKind::ReverseSplit(ratio),
            CorporateActionKind::BonusIssue {
                ratio,
                receipt: None,
            },
        ] {
            let action = CorporateAction::try_new(
                CorporateActionId::new(InstrumentId::try_new("action").expect("ID")),
                at(2).offset_calendar_date(),
                kind,
            )
            .expect("행사");
            let item = SnapshotInstrumentHistory::new(
                identity("A"),
                vec![action],
                vec![candle(
                    2,
                    "A",
                    "100",
                    PriceAdjustment::Raw,
                    CandleStatus::Finalized,
                )],
            )
            .expect("이력");
            assert_eq!(
                reconstruct(2, &[item], &rows(vec![buy("A", 1, "10")]))
                    .currency_value(Currency::Usd),
                Err(SnapshotError::PendingCorporateCash)
            );
        }
    }
}
