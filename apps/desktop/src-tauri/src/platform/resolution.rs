use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use application::resolve_estimates::{ResolutionDataIssue, ResolutionInputs, ResolutionMarketData};
use application::{AiRunId, PortError, ReasoningEffort};
use domain::{
    AdjustmentBasis, CalendarDate, Candle, CandlePeriod, Currency, EstimateOutcome,
    EstimateSubmissionRecord, EstimateTask, KnownMarket, MarketCalendar, MarketState, Nanosecond,
    OutcomeAdjustment, OutcomeState, OutcomeUnavailable, PriceAdjustment, QuoteInstrument,
    SavedQuoteInstrument, SpanSeconds, TimeInterval, TimeSpan, Timestamp,
};

use crate::ipc::state::BoxFuture;

const FIRST_CALENDAR_DAYS: usize = 8;
const MORE_CALENDAR_DAYS: usize = 20;
const MAX_CALENDAR_DAYS: usize = 140;
const MAX_MINUTE_DAYS: usize = 10;
const DAY_SECONDS: u64 = 86_400;

pub trait ResolutionSource: Send + Sync {
    fn candles<'a>(
        &'a self,
        instrument: &'a QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>>;
    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>>;
}

pub trait ResolutionReads: Send + Sync {
    fn inputs<'a>(
        &'a self,
        record: &'a EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> BoxFuture<'a, Result<ResolutionInputs, ResolutionDataIssue>>;
}

#[derive(Clone)]
pub struct ResolutionDataSlot(pub Arc<dyn ResolutionReads>);

impl std::fmt::Debug for ResolutionDataSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResolutionDataSlot")
    }
}

pub struct SlotData<'a>(pub &'a dyn ResolutionReads);

impl std::fmt::Debug for SlotData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SlotData")
    }
}

impl ResolutionMarketData for SlotData<'_> {
    async fn inputs(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> Result<ResolutionInputs, ResolutionDataIssue> {
        self.0.inputs(record, now).await
    }
}

type CalendarCache = BTreeMap<(KnownMarket, CalendarDate), MarketCalendar>;

pub struct ResolutionData<S> {
    source: S,
    calendars: Mutex<CalendarCache>,
}

impl<S> std::fmt::Debug for ResolutionData<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResolutionData")
    }
}

const fn incomplete(reason: OutcomeUnavailable) -> ResolutionDataIssue {
    ResolutionDataIssue::Incomplete(reason)
}

fn quote_of(task: &EstimateTask) -> Result<QuoteInstrument, ResolutionDataIssue> {
    let market = KnownMarket::try_from_market(&task.reference().instrument().market())
        .map_err(|_error| incomplete(OutcomeUnavailable::MissingCandles))?;
    let currency = match market {
        KnownMarket::Kr => Currency::Krw,
        KnownMarket::Us => Currency::Usd,
    };
    QuoteInstrument::restore_from_saved_state(SavedQuoteInstrument {
        market,
        currency,
        symbol: task.reference().instrument().symbol().clone(),
        instrument_id: task.volatility().instrument().clone(),
    })
    .map_err(|_error| incomplete(OutcomeUnavailable::MissingCandles))
}

fn same_prices(adjusted: &[Candle], raw: &[Candle]) -> bool {
    adjusted.len() == raw.len()
        && adjusted.iter().all(|candle| {
            raw.iter().any(|other| {
                other.time() == candle.time()
                    && other.values().open() == candle.values().open()
                    && other.values().high() == candle.values().high()
                    && other.values().low() == candle.values().low()
                    && other.values().close() == candle.values().close()
            })
        })
}

fn regular_session(calendar: &MarketCalendar) -> Option<TimeInterval> {
    calendar
        .today()
        .sessions()
        .iter()
        .find(|segment| segment.state() == MarketState::RegularMarket)
        .map(|segment| segment.session().interval())
}

fn state_of(
    task: &EstimateTask,
    calendars: &[MarketCalendar],
    adjustment: &OutcomeAdjustment,
    candles: &[Candle],
    now: Timestamp,
) -> OutcomeState {
    EstimateOutcome::evaluate_with_complete_data(task, calendars, adjustment, candles, now).state()
}

impl<S: ResolutionSource> ResolutionData<S> {
    #[must_use]
    pub fn new(source: S) -> Self {
        Self {
            source,
            calendars: Mutex::new(BTreeMap::new()),
        }
    }

    async fn calendar(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> Result<MarketCalendar, PortError> {
        if let Some(found) = self.calendars.lock().await.get(&(market, date)) {
            return Ok(found.clone());
        }
        let fetched = self.source.calendar_on(market, date).await?;
        let _previous = self
            .calendars
            .lock()
            .await
            .insert((market, date), fetched.clone());
        Ok(fetched)
    }

    async fn extend(
        &self,
        market: KnownMarket,
        chain: &mut Vec<MarketCalendar>,
        first: CalendarDate,
        count: usize,
    ) -> Result<(), PortError> {
        for _ in 0..count {
            let date = chain
                .last()
                .map_or(first, |last| last.next_business_day().date());
            let calendar = self.calendar(market, date).await?;
            chain.push(calendar);
        }
        Ok(())
    }

    async fn minutes(
        &self,
        quote: &QuoteInstrument,
        windows: &[TimeInterval],
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, ResolutionDataIssue> {
        let mut collected = Vec::new();
        for window in windows {
            let mut fetched = self
                .source
                .candles(quote, *window, CandlePeriod::Minute, adjustment)
                .await
                .map_err(ResolutionDataIssue::Read)?;
            collected.append(&mut fetched);
        }
        Ok(collected)
    }

    async fn daily(
        &self,
        quote: &QuoteInstrument,
        start: Timestamp,
        now: Timestamp,
        priced: PriceAdjustment,
    ) -> Result<Vec<Candle>, ResolutionDataIssue> {
        let day = TimeSpan::try_new(SpanSeconds::new(DAY_SECONDS), Nanosecond::new(0))
            .map_err(|_error| incomplete(OutcomeUnavailable::MissingCandles))?;
        let window = start
            .checked_sub(day)
            .ok()
            .and_then(|begin| TimeInterval::try_new(begin, now).ok())
            .ok_or(incomplete(OutcomeUnavailable::MissingCandles))?;
        let adjusted = self
            .source
            .candles(quote, window, CandlePeriod::Daily, priced)
            .await
            .map_err(ResolutionDataIssue::Read)?;
        let raw = self
            .source
            .candles(quote, window, CandlePeriod::Daily, PriceAdjustment::Raw)
            .await
            .map_err(ResolutionDataIssue::Read)?;
        if same_prices(&adjusted, &raw) {
            Ok(adjusted)
        } else {
            Err(incomplete(OutcomeUnavailable::Adjustment))
        }
    }

    pub async fn gather(
        &self,
        task: &EstimateTask,
        now: Timestamp,
    ) -> Result<ResolutionInputs, ResolutionDataIssue> {
        let quote = quote_of(task)?;
        let market = quote.saved_state().market;
        let start = task.reference().observed_at();
        let basis_at = Timestamp::from_parts(
            now.utc_seconds(),
            Nanosecond::new(0),
            start.offset_seconds(),
        )
        .and_then(|at| at.in_exchange_zone(market))
        .map_err(|_error| incomplete(OutcomeUnavailable::Adjustment))?;
        let basis = AdjustmentBasis::new(basis_at);
        let adjustment = OutcomeAdjustment::from_actions(task, basis, &[]).map_err(incomplete)?;
        let priced = PriceAdjustment::Adjusted(basis);
        let mut candles = self.daily(&quote, start, now, priced).await?;
        let first = start.offset_calendar_date();
        let mut calendars = Vec::new();
        self.extend(market, &mut calendars, first, FIRST_CALENDAR_DAYS)
            .await
            .map_err(ResolutionDataIssue::Read)?;
        let opening = sessions(&calendars, start, now, 1);
        candles.append(&mut self.minutes(&quote, &opening, priced).await?);
        while calendars.len() < MAX_CALENDAR_DAYS
            && state_of(task, &calendars, &adjustment, &candles, now)
                == OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        {
            self.extend(market, &mut calendars, first, MORE_CALENDAR_DAYS)
                .await
                .map_err(ResolutionDataIssue::Read)?;
        }
        if state_of(task, &calendars, &adjustment, &candles, now)
            == OutcomeState::Unavailable(OutcomeUnavailable::MissingCandles)
        {
            let later: Vec<TimeInterval> = sessions(&calendars, start, now, MAX_MINUTE_DAYS + 1)
                .into_iter()
                .skip(1)
                .collect();
            candles.append(&mut self.minutes(&quote, &later, priced).await?);
        }
        Ok(ResolutionInputs {
            calendars,
            adjustment,
            candles,
        })
    }
}

fn sessions(
    calendars: &[MarketCalendar],
    start: Timestamp,
    now: Timestamp,
    count: usize,
) -> Vec<TimeInterval> {
    calendars
        .iter()
        .filter_map(regular_session)
        .filter(|session| session.end() > start && session.start() < now)
        .take(count)
        .filter_map(|session| {
            TimeInterval::try_new(start.max(session.start()), session.end().min(now)).ok()
        })
        .collect()
}

impl<S: ResolutionSource> ResolutionReads for ResolutionData<S> {
    fn inputs<'a>(
        &'a self,
        record: &'a EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> BoxFuture<'a, Result<ResolutionInputs, ResolutionDataIssue>> {
        Box::pin(self.gather(record.submission().task_key().task(), now))
    }
}

impl<S: ResolutionSource> crate::estimate_flow::CalendarDays for ResolutionData<S> {
    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        Box::pin(self.calendar(market, date))
    }
}

#[cfg(test)]
mod tests;
