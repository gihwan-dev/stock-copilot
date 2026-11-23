use crate::{
    Candle, CandlePeriod, EstimateTask, MarketCalendar, MarketState, OutcomeAdjustment,
    OutcomeUnavailable, Price, Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualExitSignalError {
    CurrencyMismatch,
    InvalidBounds,
    InvalidEntry,
}
#[derive(Clone, PartialEq, Eq)]
pub struct VirtualExitSignal {
    task: EstimateTask,
    entry: crate::PriceObservation,
    maturity: Timestamp,
    target: Price,
    stop: Price,
    origin: Option<crate::ScorecardSignalRecord>,
    anchor: ExitAnchor,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExitAnchor {
    Recorded,
    Estimate,
}
impl std::fmt::Debug for VirtualExitSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VirtualExitSignal")
    }
}
impl VirtualExitSignal {
    pub(crate) fn for_estimate(
        task: &EstimateTask,
        maturity: Timestamp,
    ) -> Result<Self, VirtualExitSignalError> {
        let entry = task.reference().clone();
        if maturity <= entry.observed_at() {
            return Err(VirtualExitSignalError::InvalidEntry);
        }
        let (target, stop) = task_prices(task, entry.price())?;
        Ok(Self {
            task: task.clone(),
            entry,
            maturity,
            target,
            stop,
            origin: None,
            anchor: ExitAnchor::Estimate,
        })
    }
    pub fn from_record(
        record: &crate::ScorecardSignalRecord,
    ) -> Result<Self, VirtualExitSignalError> {
        let task = record.task().clone();
        let entry = record.entry().clone();
        let maturity = record.interval().end();
        match record.direction() {
            crate::ScorecardSignalDirection::Buy => record_prices(record)
                .and_then(|(target, stop)| Self::from_entry(task, entry, target, stop, maturity)),
            crate::ScorecardSignalDirection::SellAvoidance => Ok(Self {
                target: entry.price(),
                stop: entry.price(),
                task,
                entry,
                maturity,
                origin: None,
                anchor: ExitAnchor::Recorded,
            }),
        }
        .map(|mut result| {
            result.origin = Some(record.clone());
            result
        })
    }
    pub(crate) fn direction(&self) -> crate::ScorecardSignalDirection {
        self.origin.as_ref().map_or(
            crate::ScorecardSignalDirection::Buy,
            crate::ScorecardSignalRecord::direction,
        )
    }
    pub(crate) fn from_entry(
        task: EstimateTask,
        entry: crate::PriceObservation,
        target: Price,
        stop: Price,
        maturity: Timestamp,
    ) -> Result<Self, VirtualExitSignalError> {
        if entry.instrument() != task.reference().instrument()
            || entry.instrument_id() != Some(task.volatility().instrument())
            || entry.observed_at() < task.created_at()
            || maturity <= entry.observed_at()
        {
            return Err(VirtualExitSignalError::InvalidEntry);
        }
        let money = entry.price().as_money();
        if money.currency() != task.reference().price().as_money().currency()
            || target.as_money().currency() != money.currency()
            || stop.as_money().currency() != money.currency()
        {
            return Err(VirtualExitSignalError::CurrencyMismatch);
        }
        if stop.as_money().amount().as_decimal() >= money.amount().as_decimal()
            || target.as_money().amount().as_decimal() <= money.amount().as_decimal()
        {
            return Err(VirtualExitSignalError::InvalidBounds);
        }
        Ok(Self {
            task,
            entry,
            maturity,
            target,
            stop,
            origin: None,
            anchor: ExitAnchor::Recorded,
        })
    }
    #[must_use]
    pub const fn maturity(&self) -> Timestamp {
        self.maturity
    }
    #[must_use]
    pub const fn entry(&self) -> &crate::PriceObservation {
        &self.entry
    }
    #[must_use]
    pub const fn task(&self) -> &EstimateTask {
        &self.task
    }
    #[must_use]
    pub fn target(&self) -> Option<Price> {
        match self.direction() {
            crate::ScorecardSignalDirection::Buy => Some(self.target),
            crate::ScorecardSignalDirection::SellAvoidance => None,
        }
    }
    #[must_use]
    pub fn stop(&self) -> Option<Price> {
        match self.direction() {
            crate::ScorecardSignalDirection::Buy => Some(self.stop),
            crate::ScorecardSignalDirection::SellAvoidance => None,
        }
    }
    pub(crate) const fn estimate_bounds(&self) -> (Price, Price) {
        (self.target, self.stop)
    }
}
fn record_prices(
    record: &crate::ScorecardSignalRecord,
) -> Result<(Price, Price), VirtualExitSignalError> {
    task_prices(record.task(), record.entry().price())
}
fn task_prices(
    task: &EstimateTask,
    entry: Price,
) -> Result<(Price, Price), VirtualExitSignalError> {
    let one = crate::Ratio::new(rust_decimal::Decimal::ONE);
    let up = one.checked_add(&crate::Ratio::new(task.target().as_decimal()));
    let down = one.checked_sub(&crate::Ratio::new(task.stop().as_decimal()));
    up.ok()
        .zip(down.ok())
        .and_then(|(target, stop)| {
            record_bound(entry, target)
                .ok()
                .zip(record_bound(entry, stop).ok())
        })
        .ok_or(VirtualExitSignalError::InvalidBounds)
}
fn record_bound(entry: Price, factor: crate::Ratio) -> Result<Price, VirtualExitSignalError> {
    entry
        .as_money()
        .checked_mul(&factor)
        .ok()
        .and_then(|money| Price::try_new(money).ok())
        .ok_or(VirtualExitSignalError::InvalidBounds)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetExit(VirtualExitFill);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopExit(VirtualExitFill);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaturityExit(VirtualExitFill);
impl TargetExit {
    #[must_use]
    pub const fn as_fill(&self) -> &VirtualExitFill {
        &self.0
    }
    pub(crate) fn into_fill(self) -> VirtualExitFill {
        self.0.into_intraday()
    }
}
impl StopExit {
    #[must_use]
    pub const fn as_fill(&self) -> &VirtualExitFill {
        &self.0
    }
    pub(crate) fn into_fill(self) -> VirtualExitFill {
        self.0.into_intraday()
    }
}
impl MaturityExit {
    #[must_use]
    pub const fn as_fill(&self) -> &VirtualExitFill {
        &self.0
    }
    pub(crate) fn into_fill(self) -> VirtualExitFill {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualExit {
    Ongoing,
    Unavailable(OutcomeUnavailable),
    Target(TargetExit),
    Stop(StopExit),
    Maturity(MaturityExit),
}
impl VirtualExit {
    #[must_use]
    pub fn into_fill(self) -> Option<VirtualExitFill> {
        match self {
            Self::Target(value) => Some(value.into_fill()),
            Self::Stop(value) => Some(value.into_fill()),
            Self::Maturity(value) => Some(value.into_fill()),
            Self::Ongoing | Self::Unavailable(_) => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualExitMoment {
    At(Timestamp),
    TradingDay(crate::CalendarDate),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExitDates {
    entry: crate::CalendarDate,
    exit: crate::CalendarDate,
}
#[derive(Clone, PartialEq, Eq)]
pub struct VirtualExitFill {
    task: EstimateTask,
    interval: crate::TimeInterval,
    entered_at: Timestamp,
    dates: ExitDates,
    moment: VirtualExitMoment,
    origin: Option<crate::ScorecardSignalRecord>,
    entry_close: crate::CheckResult,
    exit_close: crate::CheckResult,
    entry: Price,
    price: Price,
}
impl std::fmt::Debug for VirtualExitFill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VirtualExitFill")
    }
}
impl VirtualExitFill {
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub const fn at(&self) -> VirtualExitMoment {
        self.moment
    }
    #[must_use]
    pub const fn moment(&self) -> VirtualExitMoment {
        self.moment
    }
    pub(crate) const fn entry_day(&self) -> crate::CalendarDate {
        self.dates.entry
    }
    pub(crate) const fn exit_day(&self) -> crate::CalendarDate {
        self.dates.exit
    }
    pub(crate) const fn exit_close(&self) -> crate::CheckResult {
        self.exit_close
    }
    fn into_day(mut self, day: crate::CalendarDate) -> Self {
        self.moment = VirtualExitMoment::TradingDay(day);
        self.exit_close = crate::CheckResult::Failed;
        self
    }
    #[must_use]
    pub const fn entered_at(&self) -> Timestamp {
        self.entered_at
    }
    pub(crate) const fn entry(&self) -> Price {
        self.entry
    }
    fn into_intraday(mut self) -> Self {
        self.exit_close = crate::CheckResult::Failed;
        self
    }
    pub(crate) fn identity(&self) -> Option<&crate::ScorecardSignalRecord> {
        self.origin.as_ref()
    }
    pub(crate) fn benchmark_ready(&self) -> crate::CheckResult {
        if self.entry_close == crate::CheckResult::Passed
            && self.exit_close == crate::CheckResult::Passed
        {
            crate::CheckResult::Passed
        } else {
            crate::CheckResult::Failed
        }
    }
    pub(crate) const fn task(&self) -> &EstimateTask {
        &self.task
    }
    pub(crate) const fn interval(&self) -> crate::TimeInterval {
        self.interval
    }
}
pub(crate) struct VirtualExitSession {
    pub(crate) date: crate::CalendarDate,
    pub(crate) interval: crate::TimeInterval,
}
#[derive(Clone, Copy)]
struct ExitContext<'a> {
    task: &'a EstimateTask,
    interval: crate::TimeInterval,
    prices: ExitPrices,
    dates: ExitDates,
    origin: Option<&'a crate::ScorecardSignalRecord>,
    entry_close: crate::CheckResult,
    exit_close: crate::CheckResult,
}
impl ExitContext<'_> {
    fn fill(self, price: Price, at: Timestamp) -> VirtualExitFill {
        VirtualExitFill {
            task: self.task.clone(),
            interval: self.interval,
            entered_at: self.interval.start(),
            dates: self.dates,
            moment: VirtualExitMoment::At(at),
            origin: self.origin.cloned(),
            entry_close: self.entry_close,
            exit_close: self.exit_close,
            entry: self.prices.entry,
            price,
        }
    }
}

#[must_use]
pub fn virtual_exit(
    signal: &VirtualExitSignal,
    calendars: &[MarketCalendar],
    adjustment: &OutcomeAdjustment,
    candles: &[Candle],
    now: Timestamp,
) -> VirtualExit {
    match evaluate(signal, calendars, adjustment, candles, now) {
        Ok(value) => value,
        Err(error) => VirtualExit::Unavailable(error),
    }
}
fn evaluate(
    signal: &VirtualExitSignal,
    calendars: &[MarketCalendar],
    adjustment: &OutcomeAdjustment,
    candles: &[Candle],
    now: Timestamp,
) -> Result<VirtualExit, OutcomeUnavailable> {
    let task = signal.task();
    if adjustment.instrument() != task.volatility().instrument()
        || adjustment.reference_at() != signal.entry().observed_at()
    {
        return Err(OutcomeUnavailable::Adjustment);
    }
    let (interval, rows, bars) =
        crate::estimate_outcome::virtual_exit_inputs(signal, calendars, adjustment, candles)?;
    let prices = prices(signal, adjustment)?;
    let date_result = match signal.anchor {
        ExitAnchor::Recorded => signal_dates(signal.entry().observed_at(), calendars),
        ExitAnchor::Estimate => estimate_dates(&rows),
    };
    date_result.map_or_else(Err, |dates| {
        let context = ExitContext {
            task,
            interval,
            prices,
            dates,
            origin: signal.origin.as_ref(),
            entry_close: entry_close(signal.entry().observed_at(), calendars),
            exit_close: crate::CheckResult::Failed,
        };
        evaluate_rows(context, rows, &bars, now)
    })
}

fn estimate_dates(rows: &[VirtualExitSession]) -> Result<ExitDates, OutcomeUnavailable> {
    let day = rows.first().ok_or(OutcomeUnavailable::Calendar)?.date;
    Ok(ExitDates {
        entry: day,
        exit: day,
    })
}
fn evaluate_rows(
    context: ExitContext<'_>,
    rows: Vec<VirtualExitSession>,
    bars: &[&Candle],
    now: Timestamp,
) -> Result<VirtualExit, OutcomeUnavailable> {
    let interval = context.interval;
    let mut result = VirtualExit::Ongoing;
    for row in rows {
        let session = row.interval;
        let end = session.end().min(interval.end());
        let start = session.start().max(interval.start());
        let row_context = ExitContext {
            dates: ExitDates {
                entry: context.dates.entry,
                exit: row.date,
            },
            exit_close: if end == session.end() {
                crate::CheckResult::Passed
            } else {
                crate::CheckResult::Failed
            },
            ..context
        };
        let daily = bars.iter().copied().find(|bar| {
            bar.time().period() == CandlePeriod::Daily && bar.time().date() == row.date
        });
        if row.date == context.dates.entry || now < session.end() || end < session.end() {
            if let Some(value) = minutes(bars, start, end, now, row_context)? {
                result = value;
                break;
            }
        } else if let Some(value) = daily_exit(daily, bars, session, now, row_context)? {
            result = value;
            break;
        }
        if now < end {
            break;
        }
        if end == interval.end() {
            return Err(OutcomeUnavailable::MissingCandles);
        }
    }
    Ok(result)
}
fn signal_dates(
    at: Timestamp,
    calendars: &[MarketCalendar],
) -> Result<ExitDates, OutcomeUnavailable> {
    let days = calendars
        .iter()
        .flat_map(|calendar| {
            [
                calendar.previous_business_day(),
                calendar.today(),
                calendar.next_business_day(),
            ]
        })
        .filter(|row| {
            row.sessions().iter().any(|part| {
                part.session().interval().start() <= at && at <= part.session().interval().end()
            })
        })
        .map(crate::MarketSession::date)
        .collect::<std::collections::BTreeSet<_>>();
    if days.len() != 1 {
        return Err(OutcomeUnavailable::Calendar);
    }
    days.into_iter()
        .next()
        .map(|day| ExitDates {
            entry: day,
            exit: day,
        })
        .ok_or(OutcomeUnavailable::Calendar)
}
pub(crate) fn entry_close(at: Timestamp, calendars: &[MarketCalendar]) -> crate::CheckResult {
    let closed = calendars
        .iter()
        .flat_map(|calendar| {
            [
                calendar.previous_business_day(),
                calendar.today(),
                calendar.next_business_day(),
            ]
        })
        .flat_map(crate::MarketSession::sessions)
        .any(|part| {
            part.state() == MarketState::RegularMarket && part.session().interval().end() == at
        });
    if closed {
        crate::CheckResult::Passed
    } else {
        crate::CheckResult::Failed
    }
}
fn daily_exit(
    daily: Option<&Candle>,
    bars: &[&Candle],
    session: crate::TimeInterval,
    now: Timestamp,
    context: ExitContext<'_>,
) -> Result<Option<VirtualExit>, OutcomeUnavailable> {
    let maturity = context.interval.end();
    let bar = daily.ok_or(OutcomeUnavailable::MissingCandles)?;
    let values = bar.values();
    if context
        .origin
        .is_some_and(|record| record.direction() == crate::ScorecardSignalDirection::SellAvoidance)
    {
        return Ok((session.end() == maturity)
            .then(|| VirtualExit::Maturity(MaturityExit(context.fill(values.close(), maturity)))));
    }
    if let Some(value) = opening(values.open(), session.start(), context) {
        return Ok(Some(value));
    }
    let target = values.high().as_money().amount().as_decimal()
        >= context.prices.target.as_money().amount().as_decimal();
    let stop = values.low().as_money().amount().as_decimal()
        <= context.prices.stop.as_money().amount().as_decimal();
    if target && stop {
        return minutes(bars, session.start(), session.end(), now, context);
    }
    if stop {
        return Ok(Some(VirtualExit::Stop(StopExit(
            context
                .fill(context.prices.stop, session.end())
                .into_day(context.dates.exit),
        ))));
    }
    if target {
        return Ok(Some(VirtualExit::Target(TargetExit(
            context
                .fill(context.prices.target, session.end())
                .into_day(context.dates.exit),
        ))));
    }
    if session.end() == maturity {
        return Ok(Some(VirtualExit::Maturity(MaturityExit(
            context.fill(values.close(), maturity),
        ))));
    }
    Ok(None)
}
fn opening(price: Price, at: Timestamp, context: ExitContext<'_>) -> Option<VirtualExit> {
    let value = price.as_money().amount().as_decimal();
    if value <= context.prices.stop.as_money().amount().as_decimal() {
        Some(VirtualExit::Stop(StopExit(context.fill(price, at))))
    } else if value >= context.prices.target.as_money().amount().as_decimal() {
        Some(VirtualExit::Target(TargetExit(context.fill(price, at))))
    } else {
        None
    }
}
fn minutes(
    bars: &[&Candle],
    start: Timestamp,
    end: Timestamp,
    now: Timestamp,
    context: ExitContext<'_>,
) -> Result<Option<VirtualExit>, OutcomeUnavailable> {
    let cursor = crate::domain_time::virtual_exit_minute_start(start);
    scan_minutes(bars, MinuteWindow { start, end, now }, cursor, context)
}
#[derive(Clone, Copy)]
struct MinuteWindow {
    start: Timestamp,
    end: Timestamp,
    now: Timestamp,
}
enum MinuteStep<'a> {
    Pending,
    Observed(Timestamp, &'a Candle),
}
fn minute_step<'a>(
    bars: &[&'a Candle],
    cursor: Timestamp,
    window: MinuteWindow,
) -> Result<MinuteStep<'a>, OutcomeUnavailable> {
    let next = crate::TimeSpan::try_new(crate::SpanSeconds::new(60), crate::Nanosecond::new(0))
        .and_then(|span| cursor.checked_add(span))
        .map_err(OutcomeUnavailable::from)?;
    if next > window.now.min(window.end) {
        return Ok(MinuteStep::Pending);
    }
    match bars
        .iter()
        .copied()
        .find(|bar| bar.time().period() == CandlePeriod::Minute && bar.time().timestamp() == next)
    {
        Some(bar) => Ok(MinuteStep::Observed(next, bar)),
        None if window.now < window.end => Ok(MinuteStep::Pending),
        None => Err(OutcomeUnavailable::MissingCandles),
    }
}

fn scan_minutes(
    bars: &[&Candle],
    window: MinuteWindow,
    mut cursor: Timestamp,
    context: ExitContext<'_>,
) -> Result<Option<VirtualExit>, OutcomeUnavailable> {
    let MinuteWindow { start, end, .. } = window;
    loop {
        let MinuteStep::Observed(next, bar) = minute_step(bars, cursor, window)? else {
            return Ok(None);
        };
        let values = bar.values();
        if context.origin.is_some_and(|record| {
            record.direction() == crate::ScorecardSignalDirection::SellAvoidance
        }) {
            if next == end && end == context.interval.end() {
                return Ok(Some(VirtualExit::Maturity(MaturityExit(
                    context.fill(values.close(), end),
                ))));
            }
            cursor = next;
            continue;
        }
        if cursor >= start
            && let Some(value) = opening(values.open(), cursor, context)
        {
            return Ok(Some(value));
        }
        let stop = values.low().as_money().amount().as_decimal()
            <= context.prices.stop.as_money().amount().as_decimal();
        let target = values.high().as_money().amount().as_decimal()
            >= context.prices.target.as_money().amount().as_decimal();
        if cursor < start && (stop || target) {
            return Err(OutcomeUnavailable::MissingCandles);
        }
        if stop {
            return Ok(Some(VirtualExit::Stop(StopExit(
                context.fill(context.prices.stop, next),
            ))));
        }
        if target {
            return Ok(Some(VirtualExit::Target(TargetExit(
                context.fill(context.prices.target, next),
            ))));
        }
        if next == end && end == context.interval.end() {
            return Ok(Some(VirtualExit::Maturity(MaturityExit(
                context.fill(values.close(), end),
            ))));
        }
        cursor = next;
    }
}
#[derive(Clone, Copy)]
struct ExitPrices {
    entry: Price,
    target: Price,
    stop: Price,
}
fn adjustment_error<Error>(_error: Error) -> OutcomeUnavailable {
    OutcomeUnavailable::Adjustment
}
pub(crate) fn adjusted_price(
    original: Price,
    adjustment: &OutcomeAdjustment,
) -> Result<Price, OutcomeUnavailable> {
    let mut amount = original.as_money().amount();
    for ratio in adjustment.ratios() {
        let denominator =
            crate::Amount::new(rust_decimal::Decimal::from(*ratio.new_shares().as_u64()));
        let quotient = crate::exact_arithmetic::multiply(
            amount,
            crate::Amount::new(rust_decimal::Decimal::from(*ratio.old_shares().as_u64())),
        )
        .and_then(|numerator| crate::exact_arithmetic::divide(numerator, denominator))
        .map_err(adjustment_error)?;
        amount = match quotient {
            crate::RatioQuotient::Exact(value) => crate::Amount::new(value.as_decimal()),
            crate::RatioQuotient::Approximate(_value) => {
                return Err(OutcomeUnavailable::Adjustment);
            }
        };
    }
    Price::try_new(crate::Money::new(amount, original.as_money().currency()))
        .map_err(adjustment_error)
}

fn prices(
    signal: &VirtualExitSignal,
    adjustment: &OutcomeAdjustment,
) -> Result<ExitPrices, OutcomeUnavailable> {
    let entry = adjusted_price(signal.entry().price(), adjustment)?;
    let target = adjusted_price(signal.target, adjustment)?;
    let stop = adjusted_price(signal.stop, adjustment)?;
    Ok(ExitPrices {
        entry,
        target,
        stop,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{ExitContext, ExitPrices, daily_exit, minutes, opening};
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{exit_summary, instant};
    use crate::{
        Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, Currency, EstimateTask,
        NumericInput, Ohlcv, PriceAdjustment, Quantity, TimeInterval,
    };
    #[test]
    fn d54_estimate_exit_requires_a_regular_session() {
        assert_eq!(
            super::estimate_dates(&[]),
            Err(crate::OutcomeUnavailable::Calendar)
        );
    }
    pub(crate) fn signal_fixture(
        task: EstimateTask,
        target: crate::Price,
        stop: crate::Price,
        maturity: crate::Timestamp,
    ) -> Result<crate::VirtualExitSignal, crate::VirtualExitSignalError> {
        let entry = task.reference().clone();
        crate::VirtualExitSignal::from_entry(task, entry, target, stop, maturity)
    }
    pub(crate) fn virtual_exit_fixture_fill(
        task: &EstimateTask,
        price: crate::Price,
        at: crate::Timestamp,
    ) -> crate::VirtualExit {
        super::VirtualExit::Maturity(super::MaturityExit(super::VirtualExitFill {
            task: task.clone(),
            interval: TimeInterval::try_new(task.reference().observed_at(), at).expect("구간"),
            entered_at: task.reference().observed_at(),
            dates: super::ExitDates {
                entry: task.reference().observed_at().offset_calendar_date(),
                exit: at.offset_calendar_date(),
            },
            moment: super::VirtualExitMoment::At(at),
            origin: None,
            price,
            entry_close: crate::CheckResult::Failed,
            exit_close: crate::CheckResult::Failed,
            entry: task.reference().price(),
        }))
    }

    fn context(source: &EstimateTask, end: crate::Timestamp) -> ExitContext<'_> {
        ExitContext {
            task: source,
            origin: None,
            dates: super::ExitDates {
                entry: source.reference().observed_at().offset_calendar_date(),
                exit: end.offset_calendar_date(),
            },
            interval: TimeInterval::try_new(source.reference().observed_at(), end).expect("구간"),
            prices: ExitPrices {
                entry: price("100", Currency::Krw),
                target: price("110", Currency::Krw),
                stop: price("90", Currency::Krw),
            },
            entry_close: crate::CheckResult::Failed,
            exit_close: crate::CheckResult::Failed,
        }
    }
    fn bar(period: CandlePeriod, values: [&str; 4]) -> Candle {
        let [open, high, low, close] = values;
        let stamp = if period == CandlePeriod::Daily {
            instant(62, -540)
        } else {
            instant(62, 1)
        };
        Candle::try_new(
            crate::InstrumentId::try_new("sample").expect("종목"),
            CandleTime::try_new(stamp, period).expect("시각"),
            PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(instant(66, 4))),
            Ohlcv::try_new(
                price(open, Currency::Krw),
                price(high, Currency::Krw),
                price(low, Currency::Krw),
                price(close, Currency::Krw),
                Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            )
            .expect("OHLC"),
            CandleObservation::new(instant(66, 4), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    #[test]
    fn review_sealed_buy_getters_and_sell_minute_path_keep_recorded_facts() {
        let buy = crate::scorecard_signal_review::tests::record(
            0,
            crate::ScorecardSignalDirection::Buy,
            "0",
        );
        let signal = super::VirtualExitSignal::from_record(&buy).expect("봉인 매수");
        assert_eq!(
            signal.target().expect("목표").as_money().currency(),
            Currency::Krw
        );
        assert_eq!(
            signal.stop().expect("손절").as_money().currency(),
            Currency::Krw
        );
        assert_eq!(signal.entry(), buy.entry());
        let sell = crate::scorecard_signal_review::tests::record(
            0,
            crate::ScorecardSignalDirection::SellAvoidance,
            "0",
        );
        let source = sell.task();
        let context = ExitContext {
            origin: Some(&sell),
            ..context(source, instant(62, 4))
        };
        let candles = (1_i64..=4)
            .map(|minute| {
                crate::signal_return_contract::tests::candle(
                    62,
                    minute,
                    CandlePeriod::Minute,
                    ["100", "200", "1", "120"],
                )
            })
            .collect::<Vec<_>>();
        let bars = candles.iter().collect::<Vec<_>>();
        let exit = minutes(
            &bars,
            instant(62, 0),
            instant(62, 4),
            instant(67, 4),
            context,
        )
        .expect("분봉")
        .expect("매도 만기");
        assert_eq!(exit_summary(&exit).0, "maturity");
        let fill = exit.into_fill().expect("청산");
        assert_eq!(fill.price(), price("120", Currency::Krw));
        assert_eq!(fill.entered_at(), source.reference().observed_at());
        assert_eq!(
            super::signal_dates(instant(70, 0), &[]).map(|dates| dates.entry),
            Err(crate::OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn review_daily_both_uses_minute_sequence_and_target_is_intraday() {
        let source = crate::signal_return_scenarios::tests::task(0);
        let mut context = context(&source, instant(67, 4));
        context.entry_close = crate::CheckResult::Passed;
        context.exit_close = crate::CheckResult::Passed;
        let daily = bar(CandlePeriod::Daily, ["100", "110", "90", "100"]);
        let first = bar(CandlePeriod::Minute, ["100", "110", "100", "100"]);
        let session = TimeInterval::try_new(instant(62, 0), instant(62, 4)).expect("장");
        let exit = daily_exit(Some(&daily), &[&first], session, instant(67, 4), context)
            .expect("분봉 확인")
            .expect("목표 선행");
        assert_eq!(exit_summary(&exit).0, "target");
        let fill = exit.into_fill().expect("목표 청산");
        assert_eq!(fill.benchmark_ready(), crate::CheckResult::Failed);
        assert_eq!(fill.moment(), super::VirtualExitMoment::At(instant(62, 1)));
    }
    fn review_market_row(day: i64, us: Option<()>) -> crate::MarketSession {
        let offset = crate::OffsetSeconds::new(if us.is_some() { -18000 } else { 32400 });
        let stamp = |minute| {
            let at = instant(day, minute);
            crate::Timestamp::from_parts(at.utc_seconds(), at.nanosecond(), offset)
                .expect("시장 오프셋")
        };
        let interval = TimeInterval::try_new(
            stamp(if us.is_some() { 900 } else { 0 }),
            stamp(if us.is_some() { 904 } else { 4 }),
        )
        .expect("장");
        let regular = Some(crate::SessionData::try_new(interval, None).expect("장"));
        let info = if us.is_some() {
            crate::SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: regular,
                after_market: None,
            }
        } else {
            crate::SessionInformation::Kr {
                pre_market: regular,
                regular_market: None,
                after_market: None,
            }
        };
        crate::MarketSession::try_new(crate::signal_return_contract::tests::row(day).date(), info)
            .expect("거래일")
    }
    #[test]
    fn review_native_session_date_and_regular_close_are_distinct_from_korean_date_and_premarket() {
        let us = crate::MarketCalendar::try_new(
            review_market_row(60, Some(())),
            review_market_row(61, Some(())),
            review_market_row(62, Some(())),
        )
        .expect("미국 달력");
        let kr = crate::MarketCalendar::try_new(
            review_market_row(60, None),
            review_market_row(61, None),
            review_market_row(62, None),
        )
        .expect("한국 달력");
        let at = instant(61, 904);
        let native = super::signal_dates(at, std::slice::from_ref(&us)).expect("시장 거래일");
        assert_eq!(
            native.entry,
            crate::signal_return_contract::tests::row(61).date()
        );
        assert_ne!(native.entry, at.offset_calendar_date());
        assert_eq!(
            super::signal_dates(instant(61, 4), std::slice::from_ref(&kr))
                .expect("같은 날짜")
                .entry,
            native.entry
        );
        assert_eq!(super::entry_close(at, &[us]), crate::CheckResult::Passed);
        assert_eq!(
            super::entry_close(instant(61, 4), &[kr]),
            crate::CheckResult::Failed
        );
    }
    #[test]
    fn d29_opening_boundary_prices_are_inclusive_and_gaps_use_open() {
        let source = crate::thresholds::tests::task();
        let context = context(&source, instant(66, 4));
        for (value, expected) in [
            ("90", Some("stop")),
            ("80", Some("stop")),
            ("110", Some("target")),
            ("120", Some("target")),
            ("100", None),
        ] {
            let result = opening(price(value, Currency::Krw), instant(62, 0), context);
            assert_eq!(
                result.as_ref().map(|outcome| exit_summary(outcome).0),
                expected
            );
            if let Some(exit) = result {
                let fill = exit.into_fill().expect("닫힌 청산");
                assert_eq!(fill.price(), price(value, Currency::Krw));
                assert_eq!(fill.at(), super::VirtualExitMoment::At(instant(62, 0)));
            }
        }
    }
    #[test]
    fn d29_daily_touches_and_maturity_close_use_independent_110_90_examples() {
        let source = crate::thresholds::tests::task();
        let context = context(&source, instant(62, 4));
        let session = TimeInterval::try_new(instant(62, 0), instant(62, 4)).expect("장");
        for (values, expected, exit_price) in [
            (["100", "110", "100", "100"], "target", "110"),
            (["100", "100", "90", "100"], "stop", "90"),
            (["100", "101", "100", "101"], "maturity", "101"),
        ] {
            let candle = bar(CandlePeriod::Daily, values);
            let result = daily_exit(Some(&candle), &[], session, instant(66, 4), context)
                .expect("청산")
                .expect("완료");
            assert_eq!(exit_summary(&result).0, expected);
            assert_eq!(
                result.into_fill().expect("닫힌 청산").price(),
                price(exit_price, Currency::Krw)
            );
        }
    }
    #[test]
    fn d29_minute_touches_are_inclusive_and_same_bar_both_stops_first() {
        let source = crate::thresholds::tests::task();
        let context = context(&source, instant(66, 4));
        for (values, expected, exit_price) in [
            (["100", "110", "100", "100"], "target", "110"),
            (["100", "100", "90", "100"], "stop", "90"),
            (["100", "110", "90", "100"], "stop", "90"),
        ] {
            let candle = bar(CandlePeriod::Minute, values);
            let result = minutes(
                &[&candle],
                instant(62, 0),
                instant(62, 4),
                instant(62, 1),
                context,
            )
            .expect("판정")
            .expect("도달");
            assert_eq!(exit_summary(&result).0, expected);
            assert_eq!(
                result.into_fill().expect("닫힌 청산").price(),
                price(exit_price, Currency::Krw)
            );
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::{ExitContext, ExitPrices, MinuteWindow, daily_exit, minute_step, minutes};
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{candle, exit_summary, instant, task};
    use crate::{CandlePeriod, Currency, OutcomeUnavailable, TimeInterval};
    fn context(end: crate::Timestamp, source: &crate::EstimateTask) -> ExitContext<'_> {
        ExitContext {
            task: source,
            origin: None,
            dates: super::ExitDates {
                entry: source.reference().observed_at().offset_calendar_date(),
                exit: end.offset_calendar_date(),
            },
            interval: TimeInterval::try_new(instant(62, 0), end).expect("구간"),
            prices: ExitPrices {
                entry: price("100", Currency::Krw),
                target: price("110", Currency::Krw),
                stop: price("90", Currency::Krw),
            },
            entry_close: crate::CheckResult::Failed,
            exit_close: crate::CheckResult::Failed,
        }
    }
    #[test]
    fn d29_minutes_handle_open_gaps_partial_entry_maturity_and_pending_missing_data() {
        let source = task();
        let end = instant(62, 1);
        let context = context(end, &source);
        let gap = candle(62, 1, CandlePeriod::Minute, ["120"; 4]);
        let result = minutes(&[&gap], instant(62, 0), end, end, context)
            .expect("갭")
            .expect("청산")
            .into_fill()
            .expect("체결");
        assert_eq!(result.price(), price("120", Currency::Krw));
        assert_eq!(result.at(), super::VirtualExitMoment::At(instant(62, 0)));
        let flat = candle(62, 1, CandlePeriod::Minute, ["100", "101", "100", "101"]);
        let maturity = minutes(&[&flat], instant(62, 0), end, end, context)
            .expect("만기")
            .expect("청산");
        assert_eq!(exit_summary(&maturity).0, "maturity");
        assert_eq!(
            maturity.into_fill().expect("체결").price(),
            price("101", Currency::Krw)
        );
        assert_eq!(
            minutes(&[], instant(62, 0), instant(62, 4), end, context),
            Ok(None)
        );
        let partial = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(
                instant(62, 0)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(30)
                    .expect("초"),
            ),
            crate::Nanosecond::new(0),
            instant(62, 0).offset_seconds(),
        )
        .expect("부분 분");
        let target = candle(62, 1, CandlePeriod::Minute, ["100", "110", "100", "100"]);
        assert_eq!(
            minutes(&[&target], partial, end, end, context),
            Err(OutcomeUnavailable::MissingCandles)
        );
        let daily = candle(62, 0, CandlePeriod::Daily, ["100", "110", "90", "100"]);
        let both = candle(62, 1, CandlePeriod::Minute, ["100", "110", "90", "100"]);
        let exit = daily_exit(
            Some(&daily),
            &[&both],
            TimeInterval::try_new(instant(62, 0), end).expect("장"),
            end,
            context,
        )
        .expect("양쪽")
        .expect("청산");
        assert_eq!(exit_summary(&exit).0, "stop");
    }
    #[test]
    fn d29_time_limits_are_closed_errors_without_panicking() {
        let source = task();
        let lower = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(-62_135_596_741),
            crate::Nanosecond::new(123),
            crate::OffsetSeconds::new(0),
        )
        .expect("연도 시작");
        let rounded = crate::domain_time::virtual_exit_minute_start(lower);
        assert_eq!(*rounded.utc_seconds().as_i64(), -62_135_596_800_i64);
        assert_eq!(*rounded.nanosecond().as_u32(), 0_u32);
        assert_eq!(
            minutes(
                &[],
                lower,
                instant(62, 1),
                instant(62, 1),
                context(instant(62, 1), &source)
            ),
            Err(OutcomeUnavailable::MissingCandles)
        );
        let upper = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(253_402_300_780),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("연도 끝");
        assert_eq!(
            minute_step(
                &[],
                upper,
                MinuteWindow {
                    start: upper,
                    end: upper,
                    now: upper
                }
            )
            .map(|_step| ()),
            Err(OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn d29_partial_entry_stop_and_both_touches_do_not_invent_post_entry_fills() {
        let source = task();
        let end = instant(62, 1);
        let context = context(end, &source);
        let partial = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(
                instant(62, 0)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(30)
                    .expect("분 중간"),
            ),
            crate::Nanosecond::new(0),
            instant(62, 0).offset_seconds(),
        )
        .expect("진입");
        for values in [["100", "100", "90", "100"], ["100", "110", "90", "100"]] {
            let bar = candle(62, 1, CandlePeriod::Minute, values);
            assert_eq!(
                minutes(&[&bar], partial, end, end, context),
                Err(OutcomeUnavailable::MissingCandles)
            );
        }
    }
}

#[cfg(test)]
mod maturity_tests {
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{
        adjustment, calendars, candle, entry_bars, instant, task,
    };
    use crate::signal_return_scenarios::tests::{bar, calendar};
    use crate::{
        AdjustmentBasis, CandlePeriod, Currency, EstimateContext, EstimateTask, Horizon,
        HorizonSelection, HoursPermission, HoursPermissionState, MarketCalendar, MarketSession,
        OutcomeUnavailable, RiskAppetite, SessionData, SessionInformation, TimeInterval,
        VirtualExit,
    };
    fn source() -> EstimateTask {
        source_at(4)
    }
    fn source_at(minute: i64) -> EstimateTask {
        let original = crate::signal_return_scenarios::tests::task(0);
        EstimateTask::create(
            EstimateContext::new(
                crate::PriceObservation::from_identified_quote_parts(
                    original.volatility().instrument().clone(),
                    original.reference().instrument().clone(),
                    original.reference().price(),
                    instant(61, minute),
                )
                .expect("실제 기준"),
                instant(61, minute),
            ),
            original.volatility().clone(),
            Horizon::new(HorizonSelection::Hours),
            HoursPermission::restore_from_saved_state(HoursPermissionState::Enabled),
            RiskAppetite::Neutral,
        )
        .expect("장중 과제")
    }
    #[test]
    fn d29_hours_maturity_uses_its_minute_close_and_cannot_claim_daily_benchmark() {
        let signal = crate::virtual_exit::tests::signal_fixture(
            source(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(62, 1),
        )
        .expect("장중 신호");
        let basis = AdjustmentBasis::new(instant(62, 1));
        let adjusted = crate::virtual_exit_adjustment(&signal, basis, &[]).expect("조정");
        let calendars = [calendar(61), calendar(62)];
        let bars = [bar(62, 1, CandlePeriod::Minute, "101", basis)];
        let result = crate::virtual_exit(&signal, &calendars, &adjusted, &bars, instant(62, 1));
        let fill = result.into_fill().expect("장중 만기");
        assert_eq!(fill.price(), price("101", Currency::Krw));
        assert_eq!(fill.at(), super::VirtualExitMoment::At(instant(62, 1)));
        assert_eq!(fill.benchmark_ready(), crate::CheckResult::Failed);
    }
    #[test]
    fn d29_ongoing_missing_daily_and_partial_minute_maturity_remain_closed() {
        let signal = crate::virtual_exit::tests::signal_fixture(
            task(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(66, 4),
        )
        .expect("신호");
        assert_eq!(
            crate::virtual_exit(
                &signal,
                &calendars(),
                &adjustment(),
                &entry_bars(),
                instant(61, 2)
            ),
            VirtualExit::Ongoing
        );
        assert_eq!(
            crate::virtual_exit(
                &signal,
                &calendars(),
                &adjustment(),
                &entry_bars(),
                instant(66, 4)
            ),
            VirtualExit::Unavailable(OutcomeUnavailable::MissingCandles)
        );
        let mut bars = entry_bars();
        bars[0] = candle(61, 1, CandlePeriod::Minute, ["100", "110", "100", "100"]);
        assert_eq!(
            crate::signal_return_contract::tests::exit_summary(&crate::virtual_exit(
                &signal,
                &calendars(),
                &adjustment(),
                &bars,
                instant(61, 2)
            ))
            .0,
            "target"
        );
    }
    #[test]
    fn d29_partial_minute_maturity_has_no_invented_close() {
        let session = |day: i64| {
            MarketSession::try_new(
                instant(day, 0).offset_calendar_date(),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(
                            TimeInterval::try_new(instant(day, 0), instant(day, 3))
                                .expect("3분 장"),
                            None,
                        )
                        .expect("장"),
                    ),
                    after_market: None,
                },
            )
            .expect("장")
        };
        let first =
            MarketCalendar::try_new(session(60), session(61), session(62)).expect("첫 달력");
        let next =
            MarketCalendar::try_new(session(61), session(62), session(63)).expect("다음 달력");
        let end = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(
                instant(62, 0)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(45)
                    .expect("45초"),
            ),
            crate::Nanosecond::new(0),
            instant(62, 0).offset_seconds(),
        )
        .expect("부분 분 만기");
        let short = crate::virtual_exit::tests::signal_fixture(
            source_at(3),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            end,
        )
        .expect("짧은 만기");
        let adjusted =
            crate::virtual_exit_adjustment(&short, AdjustmentBasis::new(end), &[]).expect("조정");
        assert_eq!(
            crate::virtual_exit(&short, &[first, next], &adjusted, &[], end),
            VirtualExit::Unavailable(OutcomeUnavailable::MissingCandles)
        );
    }

    #[test]
    fn d42_elapsed_intraday_maturity_still_uses_the_minute_close() {
        let signal = crate::virtual_exit::tests::signal_fixture(
            source(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(62, 1),
        )
        .expect("장중 신호");
        let basis = AdjustmentBasis::new(instant(62, 1));
        let adjusted = crate::virtual_exit_adjustment(&signal, basis, &[]).expect("조정");
        let bars = [bar(62, 1, CandlePeriod::Minute, "103", basis)];
        let fill = crate::virtual_exit(
            &signal,
            &[calendar(61), calendar(62)],
            &adjusted,
            &bars,
            instant(63, 4),
        )
        .into_fill()
        .expect("지난 장중 만기");
        assert_eq!(fill.price(), price("103", Currency::Krw));
        assert_eq!(fill.moment(), super::VirtualExitMoment::At(instant(62, 1)));
    }
}

#[cfg(test)]
mod d42_tests {
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{
        adjustment, calendars, candle, entry_bars, instant, task,
    };
    use crate::signal_return_scenarios::tests::{bar, calendar};
    use crate::{CandlePeriod, Currency, VirtualExit, VirtualExitSignal};
    #[test]
    fn d42_daily_selection_uses_the_current_trading_day() {
        let signal = super::tests::signal_fixture(
            task(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(66, 4),
        )
        .expect("신호");
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            CandlePeriod::Daily,
            ["101", "104", "99", "103"],
        ));
        bars.push(candle(
            63,
            0,
            CandlePeriod::Daily,
            ["102", "111", "98", "109"],
        ));
        let exit = crate::virtual_exit(&signal, &calendars(), &adjustment(), &bars, instant(66, 4));
        assert_eq!(
            crate::signal_return_contract::tests::exit_summary(&exit).0,
            "target"
        );
        let target = exit.into_fill().expect("두 번째 거래일 목표 도달");
        assert_eq!(target.price(), price("110", Currency::Krw));
        assert_eq!(
            target.moment(),
            super::VirtualExitMoment::TradingDay(instant(63, 0).offset_calendar_date())
        );
    }
    #[test]
    fn d42_regular_session_intraday_entry_is_not_a_close() {
        let days = [calendar(61)];
        assert_eq!(
            super::entry_close(instant(61, 3), &days),
            crate::CheckResult::Failed
        );
        assert_eq!(
            super::entry_close(instant(61, 4), &days),
            crate::CheckResult::Passed
        );
    }
    #[test]
    fn d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity() {
        let record = crate::scorecard_signal_review::tests::record(
            0,
            crate::ScorecardSignalDirection::SellAvoidance,
            "0",
        );
        let signal = VirtualExitSignal::from_record(&record).expect("매도 기록");
        let basis = crate::AdjustmentBasis::new(instant(67, 4));
        let adjusted = crate::virtual_exit_adjustment(&signal, basis, &[]).expect("조정");
        let days = (61_i64..=67).map(calendar).collect::<Vec<_>>();
        let mut bars = (1_i64..=4)
            .map(|minute| bar(61, minute, CandlePeriod::Minute, "101", basis))
            .collect::<Vec<_>>();
        bars.extend((62_i64..=66).map(|day| bar(day, 0, CandlePeriod::Daily, "103", basis)));
        bars.push(bar(67, 0, CandlePeriod::Daily, "107", basis));
        bars.extend(
            ["102", "104", "106", "107"]
                .into_iter()
                .zip(1_i64..=4)
                .map(|(close, minute)| bar(67, minute, CandlePeriod::Minute, close, basis)),
        );
        for now in [
            instant(61, 2),
            instant(62, 4),
            instant(66, 4),
            instant(67, 2),
        ] {
            assert_eq!(
                crate::virtual_exit(&signal, &days, &adjusted, &bars, now),
                VirtualExit::Ongoing
            );
        }
        let exit = crate::virtual_exit(&signal, &days, &adjusted, &bars, instant(67, 4));
        assert_eq!(
            crate::signal_return_contract::tests::exit_summary(&exit).0,
            "maturity"
        );
        let value = exit.into_fill().expect("기록된 만기에만 청산");
        assert_eq!(value.price(), price("107", Currency::Krw));
        assert_eq!(value.moment(), super::VirtualExitMoment::At(instant(67, 4)));
    }
}
