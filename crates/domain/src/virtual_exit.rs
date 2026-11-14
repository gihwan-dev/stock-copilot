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
}
impl std::fmt::Debug for VirtualExitSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VirtualExitSignal")
    }
}
impl VirtualExitSignal {
    pub fn try_new(
        task: EstimateTask,
        target: Price,
        stop: Price,
        maturity: Timestamp,
    ) -> Result<Self, VirtualExitSignalError> {
        let entry = task.reference().clone();
        Self::from_entry(task, entry, target, stop, maturity)
    }
    pub fn from_entry(
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
    pub const fn target(&self) -> Price {
        self.target
    }
    #[must_use]
    pub const fn stop(&self) -> Price {
        self.stop
    }
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
}
impl StopExit {
    #[must_use]
    pub const fn as_fill(&self) -> &VirtualExitFill {
        &self.0
    }
}
impl MaturityExit {
    #[must_use]
    pub const fn as_fill(&self) -> &VirtualExitFill {
        &self.0
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
            Self::Target(value) => Some(value.0.into_intraday()),
            Self::Stop(value) => Some(value.0.into_intraday()),
            Self::Maturity(value) => Some(value.0),
            Self::Ongoing | Self::Unavailable(_) => None,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct VirtualExitFill {
    task: EstimateTask,
    interval: crate::TimeInterval,
    entered_at: Timestamp,
    entry_close: crate::CheckResult,
    exit_close: crate::CheckResult,
    entry: Price,
    price: Price,
    at: Timestamp,
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
    pub const fn at(&self) -> Timestamp {
        self.at
    }
    pub(crate) const fn entered_at(&self) -> Timestamp {
        self.entered_at
    }
    pub(crate) const fn entry(&self) -> Price {
        self.entry
    }
    fn into_intraday(mut self) -> Self {
        self.exit_close = crate::CheckResult::Failed;
        self
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
    entry_close: crate::CheckResult,
    exit_close: crate::CheckResult,
}
impl ExitContext<'_> {
    fn fill(self, price: Price, at: Timestamp) -> VirtualExitFill {
        VirtualExitFill {
            task: self.task.clone(),
            interval: self.interval,
            entered_at: self.interval.start(),
            entry_close: self.entry_close,
            exit_close: self.exit_close,
            entry: self.prices.entry,
            price,
            at,
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
    let context = ExitContext {
        task,
        interval,
        prices: prices(signal, adjustment)?,
        entry_close: entry_close(signal.entry().observed_at(), calendars),
        exit_close: crate::CheckResult::Failed,
    };
    let mut result = VirtualExit::Ongoing;
    for row in rows {
        let session = row.interval;
        if session.start() >= now {
            break;
        }
        let end = session.end().min(interval.end());
        let start = session.start().max(interval.start());
        let row_context = ExitContext {
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
        if row.date == interval.start().offset_calendar_date()
            || now < session.end()
            || end < session.end()
        {
            if let Some(value) = minutes(&bars, start, end, now, row_context)? {
                result = value;
                break;
            }
        } else if let Some(value) = daily_exit(daily, &bars, session, now, row_context)? {
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
fn entry_close(at: Timestamp, calendars: &[MarketCalendar]) -> crate::CheckResult {
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
            context.fill(context.prices.stop, session.end()),
        ))));
    }
    if target {
        return Ok(Some(VirtualExit::Target(TargetExit(
            context.fill(context.prices.target, session.end()),
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
fn adjusted_price(
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
    let target = adjusted_price(signal.target(), adjustment)?;
    let stop = adjusted_price(signal.stop(), adjustment)?;
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
    pub(crate) fn virtual_exit_fixture_fill(
        task: &EstimateTask,
        price: crate::Price,
        at: crate::Timestamp,
    ) -> crate::VirtualExit {
        super::VirtualExit::Maturity(super::MaturityExit(super::VirtualExitFill {
            task: task.clone(),
            interval: TimeInterval::try_new(task.reference().observed_at(), at).expect("구간"),
            entered_at: task.reference().observed_at(),
            price,
            at,
            entry_close: crate::CheckResult::Failed,
            exit_close: crate::CheckResult::Failed,
            entry: task.reference().price(),
        }))
    }

    fn context(source: &EstimateTask, end: crate::Timestamp) -> ExitContext<'_> {
        ExitContext {
            task: source,
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
                assert_eq!(fill.at(), instant(62, 0));
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
        assert_eq!(result.at(), instant(62, 0));
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
        VirtualExit, VirtualExitSignal,
    };
    fn source() -> EstimateTask {
        let original = crate::signal_return_scenarios::tests::task(0);
        EstimateTask::create(
            EstimateContext::new(original.reference().clone(), original.created_at()),
            original.volatility().clone(),
            Horizon::new(HorizonSelection::Hours),
            HoursPermission::restore_from_saved_state(HoursPermissionState::Enabled),
            RiskAppetite::Neutral,
        )
        .expect("장중 과제")
    }
    #[test]
    fn d29_hours_maturity_uses_its_minute_close_and_cannot_claim_daily_benchmark() {
        let signal = VirtualExitSignal::try_new(
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
        assert_eq!(fill.at(), instant(62, 1));
        assert_eq!(fill.benchmark_ready(), crate::CheckResult::Failed);
    }
    #[test]
    fn d29_ongoing_missing_daily_and_partial_minute_maturity_remain_closed() {
        let signal = VirtualExitSignal::try_new(
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
        let short = VirtualExitSignal::try_new(
            source(),
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
}
