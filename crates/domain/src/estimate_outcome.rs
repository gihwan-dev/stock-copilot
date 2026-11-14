use crate::estimate_outcome_arithmetic::OutcomeBoundaries;
use crate::{
    AdjustmentBasis, AiProbability, CalibrationEstimate, CalibrationEstimateId, CalibrationFact,
    CalibrationKey, CalibrationModel, CalibrationOutcome, Candle, CandlePeriod, CandleStatus,
    EstimateTask, Horizon, HorizonSelection, InstrumentId, MarketCalendar, MarketSession,
    MarketState, Nanosecond, PriceAdjustment, ShareRatio, TimeInterval, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeAdjustment {
    instrument: InstrumentId,
    reference_at: Timestamp,
    basis: AdjustmentBasis,
    ratio: ShareRatio,
}
impl OutcomeAdjustment {
    #[must_use]
    pub const fn new(
        instrument: InstrumentId,
        reference_at: Timestamp,
        basis: AdjustmentBasis,
        ratio: ShareRatio,
    ) -> Self {
        Self {
            instrument,
            reference_at,
            basis,
            ratio,
        }
    }
    #[must_use]
    pub const fn basis(&self) -> AdjustmentBasis {
        self.basis
    }
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn reference_at(&self) -> Timestamp {
        self.reference_at
    }
    #[must_use]
    pub const fn ratio(&self) -> ShareRatio {
        self.ratio
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeFailure {
    Stop,
    Expired,
    Ambiguous,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeUnavailable {
    Calendar,
    Adjustment,
    InvalidCandle,
    ConflictingCandles,
    MissingCandles,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeState {
    Success,
    Failure(OutcomeFailure),
    Ongoing,
    Unavailable(OutcomeUnavailable),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeEvidence {
    instrument: InstrumentId,
    interval: TimeInterval,
    horizon: Horizon,
    at: Timestamp,
    period: Option<CandlePeriod>,
    adjustment: OutcomeAdjustment,
}
impl OutcomeEvidence {
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn interval(&self) -> TimeInterval {
        self.interval
    }
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
    #[must_use]
    pub const fn period(&self) -> Option<CandlePeriod> {
        self.period
    }
    #[must_use]
    pub const fn adjustment(&self) -> &OutcomeAdjustment {
        &self.adjustment
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSuccess(OutcomeEvidence);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeFailed {
    evidence: OutcomeEvidence,
    reason: OutcomeFailure,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomePending(OutcomeEvidence);
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimateOutcome {
    Success(OutcomeSuccess),
    Failure(OutcomeFailed),
    Ongoing(OutcomePending),
    Unavailable(OutcomeUnavailable),
}
impl EstimateOutcome {
    #[must_use]
    pub fn state(&self) -> OutcomeState {
        match self {
            Self::Success(_) => OutcomeState::Success,
            Self::Failure(value) => OutcomeState::Failure(value.reason),
            Self::Ongoing(_) => OutcomeState::Ongoing,
            Self::Unavailable(reason) => OutcomeState::Unavailable(*reason),
        }
    }
    #[must_use]
    pub fn evidence(&self) -> Option<&OutcomeEvidence> {
        match self {
            Self::Success(value) => Some(&value.0),
            Self::Failure(value) => Some(&value.evidence),
            Self::Ongoing(value) => Some(&value.0),
            Self::Unavailable(_) => None,
        }
    }
    #[must_use]
    pub fn calibration_fact(
        &self,
        id: CalibrationEstimateId,
        model: CalibrationModel,
        probability: AiProbability,
    ) -> Option<CalibrationFact> {
        let (evidence, outcome) = match self {
            Self::Success(value) => (&value.0, CalibrationOutcome::Hit),
            Self::Failure(value) => (&value.evidence, CalibrationOutcome::Miss),
            Self::Ongoing(_) | Self::Unavailable(_) => return None,
        };
        Some(CalibrationFact::new(
            CalibrationEstimate::new(
                id,
                evidence.instrument.clone(),
                CalibrationKey::new(model, evidence.horizon),
                evidence.interval,
                probability,
            ),
            outcome,
        ))
    }
    #[must_use]
    pub fn evaluate(
        task: &EstimateTask,
        calendars: &[MarketCalendar],
        adjustment: &OutcomeAdjustment,
        candles: &[Candle],
        now: Timestamp,
    ) -> Self {
        match evaluate(task, calendars, adjustment, candles, now) {
            Ok(result) => result,
            Err(reason) => Self::Unavailable(reason),
        }
    }
}
struct Schedule {
    rows: Vec<MarketSession>,
    interval: TimeInterval,
    horizon: Horizon,
}
fn regular(row: &MarketSession) -> Option<TimeInterval> {
    row.sessions()
        .iter()
        .find(|segment| segment.state() == MarketState::RegularMarket)
        .map(|segment| segment.session().interval())
}
fn schedule(
    task: &EstimateTask,
    calendars: &[MarketCalendar],
) -> Result<Schedule, OutcomeUnavailable> {
    let start = task.reference().observed_at();
    let mut rows = calendar_rows(task, calendars)?;
    let first = rows
        .first()
        .and_then(regular)
        .ok_or(OutcomeUnavailable::Calendar)?;
    if start < first.start() || start >= first.end() {
        return Err(OutcomeUnavailable::Calendar);
    }
    let days = task.horizon().as_ratio().as_decimal();
    let (selection, count) = if days == rust_decimal::Decimal::new(25, 2) {
        (HorizonSelection::Hours, 0_usize)
    } else if days == rust_decimal::Decimal::from(5_u32) {
        (HorizonSelection::FewDays, 5)
    } else if days == rust_decimal::Decimal::from(40_u32) {
        (HorizonSelection::WeeksToMonths, 40)
    } else {
        (HorizonSelection::HalfYearOrMore, 126)
    };
    let endpoint = if selection == HorizonSelection::Hours {
        hours_end(&rows, first, start)
    } else {
        rows.iter()
            .skip(1)
            .filter_map(regular)
            .nth(count.saturating_sub(1))
            .map(TimeInterval::end)
            .ok_or(OutcomeUnavailable::Calendar)
    };
    let end = endpoint?;
    rows.retain(|row| regular(row).is_some_and(|segment| segment.start() < end));
    TimeInterval::try_new(start, end)
        .map(|interval| Schedule {
            rows,
            interval,
            horizon: Horizon::new(selection),
        })
        .map_err(OutcomeUnavailable::from)
}
#[derive(Clone, Copy)]
enum Touch {
    Neither,
    Target,
    Stop,
    Both,
}
fn touch(candle: &Candle, boundaries: &OutcomeBoundaries) -> Touch {
    let values = candle.values();
    match (
        boundaries.target_order(values.high()).is_ge(),
        boundaries.stop_order(values.low()).is_le(),
    ) {
        (false, false) => Touch::Neither,
        (true, false) => Touch::Target,
        (false, true) => Touch::Stop,
        (true, true) => Touch::Both,
    }
}
#[derive(Clone, Copy)]
enum Resolution {
    Success,
    Failure(OutcomeFailure),
}
fn resolved(
    mut evidence: OutcomeEvidence,
    at: Timestamp,
    period: Option<CandlePeriod>,
    state: Resolution,
) -> EstimateOutcome {
    evidence.at = at;
    evidence.period = period;
    match state {
        Resolution::Success => EstimateOutcome::Success(OutcomeSuccess(evidence)),
        Resolution::Failure(reason) => EstimateOutcome::Failure(OutcomeFailed { evidence, reason }),
    }
}

fn minutes(
    evidence: &OutcomeEvidence,
    session: TimeInterval,
    boundaries: &OutcomeBoundaries,
    candles: &[&Candle],
    now: Timestamp,
) -> Option<EstimateOutcome> {
    let mut cursor = session.start().max(evidence.interval.start());
    let terminal = session.end().min(evidence.interval.end());
    let limit = terminal.min(now);
    let initial = cursor;
    for candle in candles.iter().filter(|bar| {
        bar.time().period() == CandlePeriod::Minute
            && bar.time().timestamp() > initial
            && bar.time().timestamp() <= limit
    }) {
        let end = candle.time().timestamp();
        if *cursor.nanosecond().as_u32() != 0
            || end
                .utc_seconds()
                .as_i64()
                .wrapping_sub(*cursor.utc_seconds().as_i64())
                != 60
        {
            return Some(resolved(
                evidence.clone(),
                cursor,
                Some(CandlePeriod::Minute),
                Resolution::Failure(OutcomeFailure::Ambiguous),
            ));
        }
        let state = candle_resolution(candle, boundaries);
        if let Some(decision) = state {
            return Some(resolved(
                evidence.clone(),
                end,
                Some(CandlePeriod::Minute),
                decision,
            ));
        }
        cursor = end;
    }
    if cursor < limit
        && (now >= terminal
            || limit
                .utc_seconds()
                .as_i64()
                .wrapping_sub(*cursor.utc_seconds().as_i64())
                >= 60)
    {
        return Some(resolved(
            evidence.clone(),
            cursor,
            Some(CandlePeriod::Minute),
            Resolution::Failure(OutcomeFailure::Ambiguous),
        ));
    }
    None
}

fn evaluate(
    task: &EstimateTask,
    calendars: &[MarketCalendar],
    adjustment: &OutcomeAdjustment,
    candles: &[Candle],
    now: Timestamp,
) -> Result<EstimateOutcome, OutcomeUnavailable> {
    let schedule = schedule(task, calendars)?;
    if adjustment.instrument() != task.volatility().instrument()
        || adjustment.reference_at() != task.reference().observed_at()
        || *adjustment.basis().as_timestamp() < adjustment.reference_at()
    {
        return Err(OutcomeUnavailable::Adjustment);
    }
    let boundaries = OutcomeBoundaries::new(
        task.reference().price(),
        task.target(),
        task.stop(),
        adjustment.ratio(),
    );
    let bars = validated_candles(task, adjustment, candles, schedule.interval)?;
    let evidence = OutcomeEvidence {
        instrument: adjustment.instrument.clone(),
        interval: schedule.interval,
        horizon: schedule.horizon,
        at: now,
        period: None,
        adjustment: adjustment.clone(),
    };
    if let Some(result) = scan_rows(&schedule, &evidence, &boundaries, &bars, now)? {
        return Ok(result);
    }
    if now >= schedule.interval.end() {
        Ok(resolved(
            evidence,
            schedule.interval.end(),
            None,
            Resolution::Failure(OutcomeFailure::Expired),
        ))
    } else {
        Ok(EstimateOutcome::Ongoing(OutcomePending(evidence)))
    }
}

fn calendar_rows(
    task: &EstimateTask,
    calendars: &[MarketCalendar],
) -> Result<Vec<MarketSession>, OutcomeUnavailable> {
    let start = task.reference().observed_at();
    let mut sorted = calendars.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|calendar| calendar.today().date());
    sorted.dedup();
    let mut rows = Vec::new();
    let mut previous: Option<&MarketCalendar> = None;
    for calendar in sorted {
        if calendar.today().date() < start.offset_calendar_date() {
            continue;
        }
        if calendar.today().market() != task.reference().instrument().market() {
            return Err(OutcomeUnavailable::Calendar);
        }
        if let Some(prior) = previous {
            if prior.next_business_day() != calendar.today()
                || calendar.previous_business_day() != prior.today()
            {
                return Err(OutcomeUnavailable::Calendar);
            }
        } else if calendar.today().date() != start.offset_calendar_date() {
            return Err(OutcomeUnavailable::Calendar);
        }
        rows.push(calendar.today().clone());
        previous = Some(calendar);
    }
    let sessions = rows.iter().filter_map(regular).collect::<Vec<_>>();
    if sessions
        .iter()
        .zip(sessions.iter().skip(1))
        .any(|(left, right)| left.end() > right.start())
    {
        return Err(OutcomeUnavailable::Calendar);
    }
    Ok(rows)
}

#[derive(Clone, Copy)]
struct OutcomeNanos(u64);
fn session_nanos(start: Timestamp, end: Timestamp) -> OutcomeNanos {
    OutcomeNanos(
        end.utc_seconds()
            .as_i64()
            .wrapping_sub(*start.utc_seconds().as_i64())
            .wrapping_mul(1_000_000_000)
            .wrapping_add(i64::from(*end.nanosecond().as_u32()))
            .wrapping_sub(i64::from(*start.nanosecond().as_u32()))
            .unsigned_abs(),
    )
}
fn advance_session(start: Timestamp, span: OutcomeNanos) -> Result<Timestamp, OutcomeUnavailable> {
    let nanos = u64::from(*start.nanosecond().as_u32()).wrapping_add(span.0);
    let seconds = i64::from_le_bytes(nanos.wrapping_div(1_000_000_000).to_le_bytes());
    let [first, second, third, fourth, _, _, _, _] =
        nanos.wrapping_rem(1_000_000_000).to_le_bytes();
    Timestamp::from_parts(
        crate::UnixSeconds::new(start.utc_seconds().as_i64().wrapping_add(seconds)),
        Nanosecond::new(u32::from_le_bytes([first, second, third, fourth])),
        start.offset_seconds(),
    )
    .map_err(OutcomeUnavailable::from)
}
impl From<crate::TimeError> for OutcomeUnavailable {
    fn from(_: crate::TimeError) -> Self {
        Self::Calendar
    }
}
impl From<crate::TimeIntervalError> for OutcomeUnavailable {
    fn from(_: crate::TimeIntervalError) -> Self {
        Self::Calendar
    }
}
fn hours_end(
    rows: &[MarketSession],
    first: TimeInterval,
    start: Timestamp,
) -> Result<Timestamp, OutcomeUnavailable> {
    let mut remaining = OutcomeNanos(session_nanos(first.start(), first.end()).0.wrapping_div(4));
    for session in rows.iter().filter_map(regular) {
        let cursor = session.start().max(start);
        let available = session_nanos(cursor, session.end());
        if remaining.0 <= available.0 {
            return advance_session(cursor, remaining);
        }
        remaining = OutcomeNanos(remaining.0.wrapping_sub(available.0));
    }
    Err(OutcomeUnavailable::Calendar)
}

fn validated_candles<'data>(
    task: &EstimateTask,
    adjustment: &OutcomeAdjustment,
    candles: &'data [Candle],
    interval: TimeInterval,
) -> Result<Vec<&'data Candle>, OutcomeUnavailable> {
    let mut bars = candles
        .iter()
        .filter(|bar| {
            bar.time().date() >= interval.start().offset_calendar_date()
                && bar.time().date() <= interval.end().offset_calendar_date()
        })
        .collect::<Vec<_>>();
    bars.sort_by_key(|bar| {
        (
            bar.time().timestamp(),
            u8::from(bar.time().period() != CandlePeriod::Daily),
        )
    });
    for (left, right) in bars.iter().zip(bars.iter().skip(1)) {
        if left.time() == right.time()
            && (left.values() != right.values()
                || left.adjustment() != right.adjustment()
                || left.status() != right.status()
                || left.instrument() != right.instrument()
                || left.time().date() != right.time().date())
        {
            return Err(OutcomeUnavailable::ConflictingCandles);
        }
    }
    for bar in &bars {
        if bar.instrument() != adjustment.instrument()
            || bar.values().currency() != task.reference().price().as_money().currency()
            || bar.status() != CandleStatus::Finalized
        {
            return Err(OutcomeUnavailable::InvalidCandle);
        }
        if bar.adjustment() != PriceAdjustment::Adjusted(adjustment.basis()) {
            return Err(OutcomeUnavailable::Adjustment);
        }
    }
    bars.dedup_by(|left, right| left.time() == right.time());
    Ok(bars)
}

fn scan_rows(
    schedule: &Schedule,
    evidence: &OutcomeEvidence,
    boundaries: &OutcomeBoundaries,
    bars: &[&Candle],
    now: Timestamp,
) -> Result<Option<EstimateOutcome>, OutcomeUnavailable> {
    for (row, session) in schedule
        .rows
        .iter()
        .filter_map(|row| regular(row).map(|session| (row, session)))
    {
        if session.start() >= now {
            break;
        }
        let reference_day = row.date() == schedule.interval.start().offset_calendar_date();
        let hours = schedule.horizon.selection() == HorizonSelection::Hours;
        if reference_day || hours || now < session.end() {
            if let Some(result) = minutes(evidence, session, boundaries, bars, now) {
                return Ok(Some(result));
            }
            continue;
        }
        let daily = bars
            .iter()
            .find(|bar| {
                bar.time().period() == CandlePeriod::Daily && bar.time().date() == row.date()
            })
            .ok_or(OutcomeUnavailable::MissingCandles)?;
        let state = match touch(daily, boundaries) {
            Touch::Neither => None,
            Touch::Target => Some(Resolution::Success),
            Touch::Stop => Some(Resolution::Failure(OutcomeFailure::Stop)),
            Touch::Both => {
                if let Some(result) = minutes(evidence, session, boundaries, bars, now) {
                    return Ok(Some(result));
                }
                return Ok(Some(resolved(
                    evidence.clone(),
                    session.end(),
                    Some(CandlePeriod::Minute),
                    Resolution::Failure(OutcomeFailure::Ambiguous),
                )));
            }
        };
        if let Some(decision) = state {
            return Ok(Some(resolved(
                evidence.clone(),
                daily.time().timestamp(),
                Some(CandlePeriod::Daily),
                decision,
            )));
        }
    }
    Ok(None)
}

fn candle_resolution(candle: &Candle, boundaries: &OutcomeBoundaries) -> Option<Resolution> {
    match touch(candle, boundaries) {
        Touch::Neither => None,
        Touch::Target => Some(Resolution::Success),
        Touch::Stop => Some(Resolution::Failure(OutcomeFailure::Stop)),
        Touch::Both => Some(Resolution::Failure(OutcomeFailure::Ambiguous)),
    }
}

#[cfg(test)]
mod tests {
    use crate::candle::tests::{at, price, series};
    use crate::{
        AdjustmentBasis, Candle, CandlePeriod, CandleStatus, EstimateOutcome, EstimateTask,
        MarketCalendar, OutcomeAdjustment, OutcomeFailure, OutcomeState, OutcomeUnavailable,
        PriceAdjustment, TimeInterval, Timestamp,
    };
    use crate::{
        CandleObservation, CandleTime, Currency, EstimateContext, Horizon, HoursPermission,
        InstrumentId, Market, NumericInput, OffsetSeconds, Ohlcv, PriceObservation, Quantity,
        ResolvedOrderInstrument, RiskAppetite, SessionData, SessionInformation, Symbol, Volatility,
    };
    use proptest::prelude::any;
    use proptest::test_runner::Config;
    use proptest::{prop_assert_eq, proptest};

    fn instant(day: i64, minute: i64) -> Timestamp {
        let seconds = at(day)
            .utc_seconds()
            .as_i64()
            .checked_add(minute.checked_mul(60).expect("분"))
            .expect("초");
        Timestamp::from_parts(
            crate::UnixSeconds::new(seconds),
            crate::Nanosecond::new(0),
            OffsetSeconds::new(-18000),
        )
        .expect("시각")
    }
    fn task() -> EstimateTask {
        let data = (0_u32..61)
            .map(|index| {
                if index.wrapping_rem(2) == 0 {
                    "100"
                } else {
                    "102"
                }
            })
            .collect::<Vec<_>>();
        let volatility = Volatility::calculate(&series(&data)).expect("변동성");
        let reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("ABC").expect("심볼"),
                Market::Us,
            )
            .expect("시장"),
            price("100", Currency::Usd),
            instant(61, 900),
        )
        .expect("관측");
        EstimateTask::create(
            EstimateContext::new(reference, instant(61, 900)),
            volatility,
            Horizon::new(crate::HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }
    fn row(day: i64) -> crate::MarketSession {
        crate::MarketSession::try_new(
            instant(day, 900).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(instant(day, 900), instant(day, 904)).expect("구간"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("행")
    }
    fn calendars() -> Vec<MarketCalendar> {
        (61_i64..=66)
            .map(|day| {
                MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("전일")),
                    row(day),
                    row(day.checked_add(1).expect("익일")),
                )
                .expect("달력")
            })
            .collect()
    }
    fn adjustment(old: u64, new: u64) -> OutcomeAdjustment {
        OutcomeAdjustment::new(
            InstrumentId::try_new("sample").expect("식별자"),
            instant(61, 900),
            AdjustmentBasis::new(instant(66, 904)),
            crate::ShareRatio::new(
                crate::ShareCount::try_new(old).expect("구주"),
                crate::ShareCount::try_new(new).expect("신주"),
            ),
        )
    }
    fn candle(
        day: i64,
        minute: i64,
        period: CandlePeriod,
        prices: (&str, &str),
        factor: &OutcomeAdjustment,
    ) -> Candle {
        let (high, low) = prices;
        let time = if period == CandlePeriod::Daily {
            instant(day, 300)
        } else {
            instant(day, minute)
        };
        Candle::try_new(
            InstrumentId::try_new("sample").expect("종목"),
            CandleTime::try_new(time, period).expect("봉 시각"),
            PriceAdjustment::Adjusted(factor.basis()),
            Ohlcv::try_new(
                price(
                    if high == "200" && low == "1" {
                        "100"
                    } else {
                        low
                    },
                    Currency::Usd,
                ),
                price(high, Currency::Usd),
                price(low, Currency::Usd),
                price(low, Currency::Usd),
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(instant(70, 904), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    fn flat(factor: &OutcomeAdjustment, value: &str) -> Vec<Candle> {
        let mut result = (901..=904)
            .map(|minute| candle(61, minute, CandlePeriod::Minute, (value, value), factor))
            .collect::<Vec<_>>();
        result.extend(
            (62..=66).map(|day| candle(day, 0, CandlePeriod::Daily, (value, value), factor)),
        );
        result
    }
    fn judge(candles: &[Candle], factor: &OutcomeAdjustment) -> EstimateOutcome {
        EstimateOutcome::evaluate(&task(), &calendars(), factor, candles, instant(70, 904))
    }
    #[test]
    fn target_stop_expiry_and_reference_day_ignore_prior_moves() {
        let factor = adjustment(1, 1);
        let mut bars = flat(&factor, "100");
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
        bars.push(candle(61, 0, CandlePeriod::Daily, ("200", "1"), &factor));
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
        bars.retain(|right_digit| {
            right_digit.time().date() != instant(62, 900).offset_calendar_date()
        });
        bars.push(candle(62, 0, CandlePeriod::Daily, ("200", "100"), &factor));
        assert_eq!(judge(&bars, &factor).state(), OutcomeState::Success);
        bars.retain(|right_digit| {
            right_digit.time().date() != instant(62, 900).offset_calendar_date()
        });
        bars.push(candle(62, 0, CandlePeriod::Daily, ("100", "1"), &factor));
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Stop)
        );
    }
    #[test]
    fn split_and_reverse_split_do_not_create_immediate_hits() {
        for (old, new, value) in [
            (1, 2, "50"),
            (2, 1, "200"),
            (3, 7, "42.85714285714285714285714286"),
        ] {
            let factor = adjustment(old, new);
            assert_eq!(
                judge(&flat(&factor, value), &factor).state(),
                OutcomeState::Failure(OutcomeFailure::Expired)
            );
        }
    }
    #[test]
    fn ambiguity_missing_minutes_and_missing_days_are_distinct() {
        let factor = adjustment(1, 1);
        let mut bars = flat(&factor, "100");
        bars.retain(|bar| bar.time().timestamp() != instant(61, 901));
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
        let mut missing_days = flat(&factor, "100");
        missing_days.retain(|right_digit| {
            right_digit.time().date() != instant(62, 900).offset_calendar_date()
        });
        assert_eq!(
            judge(&missing_days, &factor).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::MissingCandles)
        );
        missing_days.push(candle(62, 0, CandlePeriod::Daily, ("200", "1"), &factor));
        missing_days.push(candle(62, 901, CandlePeriod::Minute, ("200", "1"), &factor));
        assert_eq!(
            judge(&missing_days, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
    }
    #[test]
    fn ongoing_calendar_and_adjustment_validation() {
        let factor = adjustment(1, 1);
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &calendars(), &factor, &[], instant(61, 900))
                .state(),
            OutcomeState::Ongoing
        );
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &[], &factor, &[], instant(70, 900)).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        let mut bars = flat(&factor, "100");
        bars.push(candle(
            62,
            0,
            CandlePeriod::Daily,
            ("200", "100"),
            &adjustment(2, 1),
        ));
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::ConflictingCandles)
        );
    }
    fn reference_outcome(states: &[u8]) -> OutcomeState {
        for state in states {
            if *state == 1 {
                return OutcomeState::Success;
            }
            if *state == 2 {
                return OutcomeState::Failure(OutcomeFailure::Stop);
            }
        }
        OutcomeState::Failure(OutcomeFailure::Expired)
    }
    fn generated_bars(states: &[u8], factor: &OutcomeAdjustment, scale: u64) -> Vec<Candle> {
        let neutral = 100_u64.checked_mul(scale).expect("기준").to_string();
        let upper = 200_u64.checked_mul(scale).expect("목표").to_string();
        let lower = scale.to_string();
        let mut bars = (901..=904)
            .map(|minute| {
                candle(
                    61,
                    minute,
                    CandlePeriod::Minute,
                    (&neutral, &neutral),
                    factor,
                )
            })
            .collect::<Vec<_>>();
        bars.extend(states.iter().enumerate().map(|(index, state)| {
            let day = 62_i64
                .checked_add(i64::try_from(index).expect("날짜"))
                .expect("날짜");
            let prices = if *state == 1 {
                (&upper, &neutral)
            } else if *state == 2 {
                (&neutral, &lower)
            } else {
                (&neutral, &neutral)
            };
            candle(
                day,
                0,
                CandlePeriod::Daily,
                (prices.0.as_str(), prices.1.as_str()),
                factor,
            )
        }));
        bars
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn shuffled_inputs_match_linear_reference_and_append_is_monotone(states in proptest::collection::vec(0_u8..3,5), salt in any::<i64>()) {
            let factor=adjustment(1,1);
            let mut bars=generated_bars(&states,&factor,1);
            let terminal_day=states.iter().position(|state|*state!=0).map_or(66_i64,|position|62_i64.checked_add(i64::try_from(position).expect("위치")).expect("종결일"));
            let prefix=bars.iter().filter(|bar|bar.time().date()<=row(terminal_day).date()).cloned().collect::<Vec<_>>();
            let original=judge(&prefix,&factor);
            prop_assert_eq!(original.state(),linear_reference(&bars));
            prop_assert_eq!(original.state(),reference_outcome(&states));
            prop_assert_eq!(&judge(&bars,&factor),&original);
            bars.sort_by_key(|bar|bar.time().timestamp().utc_seconds().as_i64().wrapping_mul(salt));
            prop_assert_eq!(&judge(&bars,&factor),&original);
            bars.push(candle(68,0,CandlePeriod::Daily,("300","1"),&factor));
            prop_assert_eq!(&judge(&bars,&factor),&original);
        }
        #[test]
        fn scaling_both_price_bases_preserves_outcome(states in proptest::collection::vec(0_u8..3,5), scale in 1_u64..11) {
            let factor=adjustment(scale,1);
            prop_assert_eq!(judge(&generated_bars(&states,&factor,scale),&factor).state(),reference_outcome(&states));
        }
    }

    fn fact_inputs() -> (
        crate::CalibrationEstimateId,
        crate::CalibrationModel,
        crate::AiProbability,
    ) {
        let part = crate::ModelIdentityPart::try_new("fixed").expect("모델");
        (
            crate::CalibrationEstimateId::try_new("estimate-1").expect("ID"),
            crate::CalibrationModel::new(part.clone(), part.clone(), part),
            crate::AiProbability::parse(&crate::AiProbabilityInput::try_new("60").expect("입력"))
                .expect("확률"),
        )
    }
    #[test]
    fn calibration_conversion_keeps_original_interval_and_only_resolves_terminal_states() {
        let factor = adjustment(1, 1);
        let bars = flat(&factor, "100");
        let expired = judge(&bars, &factor);
        let (miss_id, miss_model, miss_probability) = fact_inputs();
        let fact = expired
            .calibration_fact(miss_id, miss_model, miss_probability)
            .expect("실패 사실");
        assert_eq!(fact.outcome(), crate::CalibrationOutcome::Miss);
        assert_eq!(
            fact.estimate().interval(),
            crate::CalibrationInterval::from_interval(
                TimeInterval::try_new(instant(61, 900), instant(66, 904)).expect("구간")
            )
        );
        assert_eq!(expired.evidence().expect("근거").at(), instant(66, 904));
        assert_eq!(expired.evidence().expect("근거").period(), None);
        assert_eq!(expired.evidence().expect("근거").adjustment(), &factor);
        assert_eq!(
            expired.evidence().expect("근거").instrument(),
            factor.instrument()
        );
        let hit = judge(
            &[candle(
                61,
                901,
                CandlePeriod::Minute,
                ("200", "100"),
                &factor,
            )],
            &factor,
        );
        let (hit_id, hit_model, hit_probability) = fact_inputs();
        assert_eq!(
            hit.calibration_fact(hit_id, hit_model, hit_probability)
                .expect("성공 사실")
                .outcome(),
            crate::CalibrationOutcome::Hit
        );
        for result in [
            EstimateOutcome::evaluate(&task(), &calendars(), &factor, &[], instant(61, 900)),
            EstimateOutcome::Unavailable(OutcomeUnavailable::Calendar),
        ] {
            let (id, model, probability) = fact_inputs();
            assert_eq!(result.calibration_fact(id, model, probability), None);
        }
        assert_eq!(
            EstimateOutcome::Unavailable(OutcomeUnavailable::Calendar).evidence(),
            None
        );
    }
    #[test]
    fn boundary_equality_and_minute_order_gap_are_explicit() {
        let factor = adjustment(1, 1);
        let task_value = task();
        for (distance, direction) in [
            (task_value.target(), OutcomeState::Success),
            (
                task_value.stop(),
                OutcomeState::Failure(OutcomeFailure::Stop),
            ),
        ] {
            let multiplier = if direction == OutcomeState::Success {
                rust_decimal::Decimal::ONE.checked_add(distance.as_decimal())
            } else {
                rust_decimal::Decimal::ONE.checked_sub(distance.as_decimal())
            }
            .expect("배율");
            let boundary = task_value
                .reference()
                .price()
                .as_money()
                .checked_mul(&crate::Ratio::new(multiplier))
                .expect("정확 경계");
            assert_eq!(
                judge(&[boundary_touch(boundary, direction, &factor)], &factor).state(),
                direction
            );
        }
    }
    #[test]
    fn minute_order_and_gap_are_explicit() {
        let factor = adjustment(1, 1);
        let mut bars = flat(&factor, "100");
        bars.retain(|bar| bar.time().date() != instant(62, 900).offset_calendar_date());
        bars.push(candle(62, 0, CandlePeriod::Daily, ("200", "1"), &factor));
        bars.push(candle(
            62,
            901,
            CandlePeriod::Minute,
            ("100", "100"),
            &factor,
        ));
        bars.push(candle(
            62,
            902,
            CandlePeriod::Minute,
            ("200", "100"),
            &factor,
        ));
        bars.push(candle(62, 903, CandlePeriod::Minute, ("100", "1"), &factor));
        assert_eq!(judge(&bars, &factor).state(), OutcomeState::Success);
        assert_eq!(
            judge(
                &[candle(
                    61,
                    901,
                    CandlePeriod::Minute,
                    ("200", "200"),
                    &factor
                )],
                &factor
            )
            .state(),
            OutcomeState::Success
        );
        assert_eq!(
            judge(
                &[candle(61, 901, CandlePeriod::Minute, ("1", "1"), &factor)],
                &factor
            )
            .state(),
            OutcomeState::Failure(OutcomeFailure::Stop)
        );
    }
    #[test]
    fn invalid_adjustment_raw_live_identity_and_calendar_gaps_are_unavailable() {
        let factor = adjustment(1, 1);
        let sample = candle(61, 901, CandlePeriod::Minute, ("100", "100"), &factor);
        for (instrument, basis, status, expected) in [
            (
                factor.instrument().clone(),
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
                OutcomeUnavailable::Adjustment,
            ),
            (
                factor.instrument().clone(),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(instant(65, 904))),
                CandleStatus::Finalized,
                OutcomeUnavailable::Adjustment,
            ),
            (
                factor.instrument().clone(),
                sample.adjustment(),
                CandleStatus::InProgress,
                OutcomeUnavailable::InvalidCandle,
            ),
            (
                InstrumentId::try_new("other").expect("ID"),
                sample.adjustment(),
                CandleStatus::Finalized,
                OutcomeUnavailable::InvalidCandle,
            ),
        ] {
            let bar = Candle::try_new(
                instrument,
                sample.time(),
                basis,
                sample.values(),
                CandleObservation::new(sample.collected_at(), status),
            )
            .expect("봉");
            assert_eq!(
                judge(&[bar], &factor).state(),
                OutcomeState::Unavailable(expected)
            );
        }
    }
    #[test]
    fn adjustment_reference_and_calendar_gaps_are_unavailable() {
        let factor = adjustment(1, 1);
        let mismatch = OutcomeAdjustment::new(
            factor.instrument().clone(),
            instant(61, 901),
            factor.basis(),
            factor.ratio(),
        );
        assert_eq!(
            judge(&[], &mismatch).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Adjustment)
        );
        let mut missing = full_calendars();
        missing.retain(|calendar| calendar.today().date() != row(63).date());
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &missing, &factor, &[], instant(70, 900)).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        let mut duplicate = calendars();
        duplicate.extend(calendars());
        duplicate.reverse();
        assert_eq!(
            EstimateOutcome::evaluate(
                &task(),
                &duplicate,
                &factor,
                &flat(&factor, "100"),
                instant(70, 904)
            ),
            judge(&flat(&factor, "100"), &factor)
        );
    }
    #[test]
    fn hours_use_only_minutes_and_cross_closed_time_without_counting_it() {
        let original = task();
        let hours = EstimateTask::create(
            EstimateContext::new(original.reference().clone(), original.created_at()),
            original.volatility().clone(),
            Horizon::new(crate::HorizonSelection::Hours),
            HoursPermission::restore_from_saved_state(crate::HoursPermissionState::Enabled),
            RiskAppetite::Neutral,
        )
        .expect("시간 과제");
        let factor = adjustment(1, 1);
        let bars = [
            candle(61, 0, CandlePeriod::Daily, ("200", "1"), &factor),
            candle(61, 901, CandlePeriod::Minute, ("100", "100"), &factor),
        ];
        let result =
            EstimateOutcome::evaluate(&hours, &calendars(), &factor, &bars, instant(70, 904));
        assert_eq!(
            result.state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
        assert_eq!(
            result.evidence().expect("근거").interval().end(),
            instant(61, 901)
        );
        assert_eq!(
            EstimateOutcome::evaluate(&hours, &calendars(), &factor, &[], instant(70, 904)).state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
        let partial = instant(61, 900).checked_add(half_minute()).expect("시각");
        assert_eq!(
            EstimateOutcome::evaluate(&hours, &calendars(), &factor, &[], partial).state(),
            OutcomeState::Ongoing
        );
    }
    #[test]
    fn hours_cross_closed_time_without_counting_it() {
        let original = task();
        let factor = adjustment(1, 1);
        let late_reference = PriceObservation::from_identified_quote_parts(
            original.volatility().instrument().clone(),
            original.reference().instrument().clone(),
            original.reference().price(),
            instant(61, 903).checked_add(half_minute()).expect("시각"),
        )
        .expect("관측");
        let late = EstimateTask::create(
            EstimateContext::new(late_reference.clone(), late_reference.observed_at()),
            original.volatility().clone(),
            Horizon::new(crate::HorizonSelection::Hours),
            HoursPermission::restore_from_saved_state(crate::HoursPermissionState::Enabled),
            RiskAppetite::Neutral,
        )
        .expect("시간 과제");
        let late_adjustment = OutcomeAdjustment::new(
            factor.instrument().clone(),
            late.reference().observed_at(),
            factor.basis(),
            factor.ratio(),
        );
        assert_eq!(
            EstimateOutcome::evaluate(
                &late,
                &[calendars().into_iter().next().expect("기준일 달력")],
                &late_adjustment,
                &[],
                instant(70, 904)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        let late_result =
            EstimateOutcome::evaluate(&late, &calendars(), &late_adjustment, &[], instant(70, 904));
        assert_eq!(
            late_result.evidence().expect("근거").interval().end(),
            instant(62, 900).checked_add(half_minute()).expect("시각")
        );
        assert_eq!(
            late_result.state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
    }

    #[test]
    fn unresolved_daily_both_touch_cannot_be_followed_by_a_success() {
        let factor = adjustment(1, 1);
        let mut bars = flat(&factor, "100");
        bars.retain(|bar| bar.time().date() != row(62).date());
        bars.push(candle(62, 0, CandlePeriod::Daily, ("200", "1"), &factor));
        bars.extend(
            (901..=904)
                .map(|minute| candle(62, minute, CandlePeriod::Minute, ("100", "100"), &factor)),
        );
        bars.retain(|bar| bar.time().date() != row(63).date());
        bars.push(candle(63, 0, CandlePeriod::Daily, ("200", "100"), &factor));
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
    }
    #[test]
    fn longer_horizons_and_calendar_start_boundaries() {
        let original = task();
        let factor = adjustment(1, 1);
        let full = full_calendars();
        for (selection, end_day) in [
            (crate::HorizonSelection::WeeksToMonths, 101),
            (crate::HorizonSelection::HalfYearOrMore, 187),
        ] {
            let long = EstimateTask::create(
                EstimateContext::new(original.reference().clone(), original.created_at()),
                original.volatility().clone(),
                Horizon::new(selection),
                HoursPermission::default(),
                RiskAppetite::Neutral,
            )
            .expect("긴 과제");
            let result =
                EstimateOutcome::evaluate(&long, &full, &factor, &[], original.created_at());
            assert_eq!(result.state(), OutcomeState::Ongoing);
            assert_eq!(
                result.evidence().expect("근거").interval().end(),
                instant(end_day, 904)
            );
        }
        assert_eq!(
            EstimateOutcome::evaluate(
                &original,
                &full.iter().skip(2).cloned().collect::<Vec<_>>(),
                &factor,
                &[],
                instant(70, 904)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        let early_reference = PriceObservation::from_identified_quote_parts(
            original.volatility().instrument().clone(),
            original.reference().instrument().clone(),
            original.reference().price(),
            instant(61, 899),
        )
        .expect("이른 관측");
        let early = EstimateTask::create(
            EstimateContext::new(early_reference, instant(61, 899)),
            original.volatility().clone(),
            Horizon::new(crate::HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("이른 과제");
        assert_eq!(
            EstimateOutcome::evaluate(&early, &full, &factor, &[], instant(70, 904)).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn wrong_market_calendar_is_rejected_and_minute_low_without_gap_stops() {
        let kr_row = |day| {
            let start = Timestamp::from_parts(
                at(day).utc_seconds(),
                crate::Nanosecond::new(0),
                OffsetSeconds::new(32400),
            )
            .expect("한국 시각");
            crate::MarketSession::try_new(
                start.offset_calendar_date(),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(
                            TimeInterval::try_new(
                                start,
                                start
                                    .checked_add(
                                        crate::TimeSpan::try_new(
                                            crate::SpanSeconds::new(240),
                                            crate::Nanosecond::new(0),
                                        )
                                        .expect("장 길이"),
                                    )
                                    .expect("종료"),
                            )
                            .expect("장 구간"),
                            None,
                        )
                        .expect("장"),
                    ),
                    after_market: None,
                },
            )
            .expect("한국 행")
        };
        let calendar =
            MarketCalendar::try_new(kr_row(60), kr_row(61), kr_row(62)).expect("한국 달력");
        let factor = adjustment(1, 1);
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &[calendar], &factor, &[], instant(70, 904)).state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn minute_low_without_gap_stops() {
        let factor = adjustment(1, 1);
        let original = candle(61, 901, CandlePeriod::Minute, ("100", "1"), &factor);
        let bar = Candle::try_new(
            original.instrument().clone(),
            original.time(),
            original.adjustment(),
            Ohlcv::try_new(
                price("100", Currency::Usd),
                price("100", Currency::Usd),
                price("1", Currency::Usd),
                price("1", Currency::Usd),
                original.values().volume(),
            )
            .expect("장중 하락"),
            CandleObservation::new(original.collected_at(), CandleStatus::Finalized),
        )
        .expect("분봉");
        assert_eq!(
            judge(&[bar], &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Stop)
        );
    }

    fn full_calendars() -> Vec<MarketCalendar> {
        (60_i64..=187)
            .map(|day| {
                MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("이전")),
                    row(day),
                    row(day.checked_add(1).expect("다음")),
                )
                .expect("달력")
            })
            .collect::<Vec<_>>()
    }

    #[test]
    fn duplicate_conflicts_inspect_all_sealed_candle_facts() {
        let factor = adjustment(1, 1);
        let sample = candle(61, 901, CandlePeriod::Minute, ("100", "100"), &factor);
        for (instrument, adjusted, status) in [
            (
                factor.instrument().clone(),
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
            ),
            (
                factor.instrument().clone(),
                sample.adjustment(),
                CandleStatus::InProgress,
            ),
            (
                InstrumentId::try_new("other").expect("ID"),
                sample.adjustment(),
                CandleStatus::Finalized,
            ),
        ] {
            let other = Candle::try_new(
                instrument,
                sample.time(),
                adjusted,
                sample.values(),
                CandleObservation::new(sample.collected_at(), status),
            )
            .expect("충돌 봉");
            assert_eq!(
                judge(&[sample.clone(), other], &factor).state(),
                OutcomeState::Unavailable(OutcomeUnavailable::ConflictingCandles)
            );
        }
        let hit = judge(
            &[candle(
                61,
                901,
                CandlePeriod::Minute,
                ("200", "100"),
                &factor,
            )],
            &factor,
        );
        assert_eq!(
            hit.evidence().expect("성공 근거").period(),
            Some(CandlePeriod::Minute)
        );
    }
    #[test]
    fn hours_skip_explicit_holiday_rows() {
        let original = task();
        let reference = PriceObservation::from_identified_quote_parts(
            original.volatility().instrument().clone(),
            original.reference().instrument().clone(),
            original.reference().price(),
            instant(61, 903).checked_add(half_minute()).expect("시각"),
        )
        .expect("관측");
        let hours = EstimateTask::create(
            EstimateContext::new(reference.clone(), reference.observed_at()),
            original.volatility().clone(),
            Horizon::new(crate::HorizonSelection::Hours),
            HoursPermission::restore_from_saved_state(crate::HoursPermissionState::Enabled),
            RiskAppetite::Neutral,
        )
        .expect("시간 과제");
        let maybe_holiday = |day| {
            if day == 62 {
                crate::MarketSession::try_new(
                    row(day).date(),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: None,
                        regular_market: None,
                        after_market: None,
                    },
                )
                .expect("휴장")
            } else {
                row(day)
            }
        };
        let calendars = (61_i64..=63)
            .map(|day| {
                MarketCalendar::try_new(
                    maybe_holiday(day.checked_sub(1).expect("이전")),
                    maybe_holiday(day),
                    maybe_holiday(day.checked_add(1).expect("다음")),
                )
                .expect("휴장 달력")
            })
            .collect::<Vec<_>>();
        let adjustment = OutcomeAdjustment::new(
            original.volatility().instrument().clone(),
            hours.reference().observed_at(),
            AdjustmentBasis::new(instant(66, 904)),
            adjustment(1, 1).ratio(),
        );
        let result =
            EstimateOutcome::evaluate(&hours, &calendars, &adjustment, &[], instant(70, 904));
        assert_eq!(
            result.evidence().expect("만기 근거").interval().end(),
            instant(63, 900).checked_add(half_minute()).expect("시각")
        );
    }

    fn half_minute() -> crate::TimeSpan {
        crate::TimeSpan::try_new(crate::SpanSeconds::new(30), crate::Nanosecond::new(0))
            .expect("30초")
    }

    fn boundary_touch(
        boundary: crate::Money,
        direction: OutcomeState,
        factor: &OutcomeAdjustment,
    ) -> Candle {
        let base = price("100", Currency::Usd);
        let edge = crate::Price::try_new(boundary).expect("경계 가격");
        let (high, low) = if direction == OutcomeState::Success {
            (edge, base)
        } else {
            (base, edge)
        };
        let sample = candle(61, 901, CandlePeriod::Minute, ("100", "100"), factor);
        Candle::try_new(
            sample.instrument().clone(),
            sample.time(),
            sample.adjustment(),
            Ohlcv::try_new(base, high, low, base, sample.values().volume()).expect("봉 경계"),
            CandleObservation::new(sample.collected_at(), CandleStatus::Finalized),
        )
        .expect("경계 분봉")
    }

    #[test]
    fn short_calendar_and_time_error_mapping_stay_unavailable() {
        let factor = adjustment(1, 1);
        assert_eq!(
            EstimateOutcome::evaluate(
                &task(),
                &[calendars().into_iter().next().expect("기준 달력")],
                &factor,
                &[],
                instant(70, 904)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        assert_eq!(
            OutcomeUnavailable::from(crate::TimeError::OutOfRange),
            OutcomeUnavailable::Calendar
        );
        assert_eq!(
            OutcomeUnavailable::from(crate::TimeIntervalError::NonIncreasing),
            OutcomeUnavailable::Calendar
        );
        let bar = candle(61, 901, CandlePeriod::Minute, ("100", "100"), &factor);
        let mut bars = flat(&factor, "100");
        bars.push(bar);
        assert_eq!(
            judge(&bars, &factor).state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
    }
    #[test]
    fn connected_dates_cannot_reverse_actual_regular_sessions() {
        let contextual_row = |day| {
            if day == 60 {
                crate::MarketSession::try_new(
                    row(day).date(),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: None,
                        regular_market: None,
                        after_market: None,
                    },
                )
                .expect("휴장")
            } else if day == 62 {
                crate::MarketSession::try_new(
                    row(day).date(),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(instant(60, 900), instant(60, 904))
                                    .expect("역순 장 구간"),
                                None,
                            )
                            .expect("역순 장"),
                        ),
                        after_market: None,
                    },
                )
                .expect("현지일 행")
            } else {
                row(day)
            }
        };
        let schedule = (61_i64..=66)
            .map(|day| {
                MarketCalendar::try_new(
                    contextual_row(day.checked_sub(1).expect("이전")),
                    contextual_row(day),
                    contextual_row(day.checked_add(1).expect("다음")),
                )
                .expect("연결된 응답")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &schedule, &adjustment(1, 1), &[], instant(70, 904))
                .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn both_boundaries_in_one_minute_fail_even_when_open_gaps_past_a_boundary() {
        let factor = adjustment(1, 1);
        let sample = candle(61, 901, CandlePeriod::Minute, ("200", "1"), &factor);
        for opening in ["200", "1"] {
            let bar = Candle::try_new(
                sample.instrument().clone(),
                sample.time(),
                sample.adjustment(),
                Ohlcv::try_new(
                    price(opening, Currency::Usd),
                    price("200", Currency::Usd),
                    price("1", Currency::Usd),
                    price("100", Currency::Usd),
                    sample.values().volume(),
                )
                .expect("두 경계"),
                CandleObservation::new(sample.collected_at(), CandleStatus::Finalized),
            )
            .expect("갭 분봉");
            assert_eq!(
                judge(&[bar], &factor).state(),
                OutcomeState::Failure(OutcomeFailure::Ambiguous)
            );
        }
    }
    #[test]
    fn duplicate_daily_instants_with_different_local_dates_are_order_independent() {
        let factor = adjustment(1, 1);
        let sample = candle(61, 901, CandlePeriod::Minute, ("100", "100"), &factor);
        let mut bars = [-43_200_i32, 43_200_i32]
            .into_iter()
            .map(|offset| {
                let timestamp = Timestamp::from_parts(
                    instant(62, 720).utc_seconds(),
                    crate::Nanosecond::new(0),
                    OffsetSeconds::new(offset),
                )
                .expect("같은 절대 시각");
                Candle::try_new(
                    sample.instrument().clone(),
                    CandleTime::try_new(timestamp, CandlePeriod::Daily).expect("현지 자정"),
                    sample.adjustment(),
                    sample.values(),
                    CandleObservation::new(sample.collected_at(), CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect::<Vec<_>>();
        let forward = judge(&bars, &factor);
        assert_eq!(
            forward.state(),
            OutcomeState::Unavailable(OutcomeUnavailable::ConflictingCandles)
        );
        bars.reverse();
        assert_eq!(judge(&bars, &factor), forward);
    }
    fn linear_reference(candles: &[Candle]) -> OutcomeState {
        let task_value = task();
        let target_multiplier = rust_decimal::Decimal::ONE
            .checked_add(task_value.target().as_decimal())
            .expect("목표 배율");
        let stop_multiplier = rust_decimal::Decimal::ONE
            .checked_sub(task_value.stop().as_decimal())
            .expect("손절 배율");
        let target = task_value
            .reference()
            .price()
            .as_money()
            .checked_mul(&crate::Ratio::new(target_multiplier))
            .expect("독립 목표 경계");
        let stop = task_value
            .reference()
            .price()
            .as_money()
            .checked_mul(&crate::Ratio::new(stop_multiplier))
            .expect("독립 손절 경계");
        let mut sorted = candles
            .iter()
            .filter(|bar| bar.time().period() == CandlePeriod::Daily)
            .collect::<Vec<_>>();
        sorted.sort_by_key(|bar| bar.time().timestamp());
        for bar in sorted {
            if bar.values().high().as_money().amount().as_decimal() >= target.amount().as_decimal()
            {
                return OutcomeState::Success;
            }
            if bar.values().low().as_money().amount().as_decimal() <= stop.amount().as_decimal() {
                return OutcomeState::Failure(OutcomeFailure::Stop);
            }
        }
        OutcomeState::Failure(OutcomeFailure::Expired)
    }
}
