use std::collections::BTreeMap;
use std::future::{Future, ready};
use std::sync::{Arc, RwLock};

use application::{
    AiProvider, AiRunId, BackgroundBudget, BrokerRead, Clock, CycleAdmission, CycleProgress,
    EstimateAssignmentRepository, EstimateRunContext, EstimateRunContextSource,
    EstimateRunIdentity, EstimateRunRegistry, EstimateSkip, EstimateTaskMaterialSource,
    EstimateTaskMaterials, EstimateTrigger, IntradayReviewSubject, PortError, PortfolioRepository,
    RetryDelay, RunEstimateCycle, RunSingleEstimate, SettingsRepository, SingleEstimateOutcome,
    SystemFailure, SystemPort, SystemPortError, TrackedUniverse, TrackingTarget,
    WatchlistRepository,
};
use domain::{
    CalendarDate, CostRate, EstimateContext, EstimateTask, InstrumentId, InvestorProfile,
    KnownMarket, Market, MarketCalendar, NumericInput, OutcomeUnavailable, Ratio, SyncOutcome,
    Timestamp, TradeTaxFact, Volatility, VolatilityError, estimate_maturity,
};
use scheduler::{EstimateCycleTally, JobError};

use crate::broker_link::AccountSlot;
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::state::BoxFuture;

#[cfg(test)]
mod tests;

const FIRST_CALENDAR_DAYS: usize = 8;
const MORE_CALENDAR_DAYS: usize = 20;
const MAX_CALENDAR_DAYS: usize = 140;
const KR_SELL_TAX: &str = "0.0020";
const US_SELL_TAX: &str = "0.0002";

fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

pub(super) fn record_estimate_note(job: &str, summary: &str, count: u32) {
    crate::diagnostics_log::record(&application::DiagnosticEvent::Job(
        application::JobDiagnosis {
            job: application::DiagnosticText::new(job),
            note: application::JobRunNote::Executed {
                summary: application::DiagnosticText::new(summary),
                processed: application::JobItemCount::new(count),
            },
        },
    ));
}

fn broker_unavailable(_error: PortError) -> SystemPortError {
    unavailable()
}

#[derive(Debug, Clone, Default)]
pub struct EstimateRunBook(Arc<RwLock<BTreeMap<String, EstimateRunContext>>>);

impl EstimateRunBook {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lookup(&self, run: &AiRunId) -> Option<EstimateRunContext> {
        match self.0.read() {
            Ok(guard) => guard.get(run.as_str()).cloned(),
            Err(poisoned) => poisoned.into_inner().get(run.as_str()).cloned(),
        }
    }
}

impl EstimateRunRegistry for EstimateRunBook {
    fn register(&self, context: EstimateRunContext) {
        let key = context.run.as_str().to_owned();
        match self.0.write() {
            Ok(mut guard) => {
                let _previous = guard.insert(key, context);
            }
            Err(poisoned) => {
                let _previous = poisoned.into_inner().insert(key, context);
            }
        }
    }

    fn skipped(&self, trigger: &EstimateTrigger, reason: EstimateSkip) {
        let job = match trigger {
            EstimateTrigger::Cycle => "estimate_cycle_target",
            EstimateTrigger::Intraday(_) => "intraday_estimate_target",
        };
        let detail = match reason {
            EstimateSkip::NoMaterials => "no_materials",
            EstimateSkip::NotTracked => "not_tracked",
            EstimateSkip::NoQuote => "no_quote",
            EstimateSkip::StaleReference => "stale_reference",
            EstimateSkip::Candles => "invalid_candles",
            EstimateSkip::VolatilitySamples => "insufficient_daily_bars",
            EstimateSkip::TaskRejected => "task_rejected",
            EstimateSkip::ExcessiveCost => "excessive_cost",
            EstimateSkip::AiFailed => "analysis_failed",
            EstimateSkip::NotSubmitted => "not_submitted",
        };
        record_estimate_note(job, detail, 0);
    }

    fn release(&self, run: &AiRunId) {
        match self.0.write() {
            Ok(mut guard) => {
                let _removed = guard.remove(run.as_str());
            }
            Err(poisoned) => {
                let _removed = poisoned.into_inner().remove(run.as_str());
            }
        }
    }
}

impl EstimateRunContextSource for EstimateRunBook {
    fn estimate_context(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<EstimateRunContext, SystemPortError>> + Send {
        ready(self.lookup(run).ok_or_else(unavailable))
    }
}

#[must_use]
pub fn trade_tax(market: KnownMarket) -> Option<TradeTaxFact> {
    let sell = match market {
        KnownMarket::Kr => KR_SELL_TAX,
        KnownMarket::Us => US_SELL_TAX,
    };
    let rate = |text: &str| {
        let ratio = Ratio::parse(&NumericInput::try_new(text).ok()?).ok()?;
        CostRate::try_new(ratio).ok()
    };
    Some(TradeTaxFact::from_schedule_result(
        Market::from(market),
        rate("0")?,
        rate(sell)?,
    ))
}

pub trait CalendarDays: Send + Sync {
    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>>;
}

async fn extend(
    days: &dyn CalendarDays,
    market: KnownMarket,
    chain: &mut Vec<MarketCalendar>,
    first: CalendarDate,
    count: usize,
) -> Result<(), SystemPortError> {
    for _ in 0..count {
        let date = chain
            .last()
            .map_or(first, |last| last.next_business_day().date());
        chain.push(
            days.calendar_on(market, date)
                .await
                .map_err(broker_unavailable)?,
        );
    }
    Ok(())
}

pub async fn task_calendars(
    days: &dyn CalendarDays,
    task: &EstimateTask,
) -> Result<Vec<MarketCalendar>, SystemPortError> {
    let market = KnownMarket::try_from_market(&task.reference().instrument().market())
        .map_err(|_error| unavailable())?;
    let first = task.reference().observed_at().offset_calendar_date();
    let mut chain = Vec::new();
    extend(days, market, &mut chain, first, FIRST_CALENDAR_DAYS).await?;
    while chain.len() < MAX_CALENDAR_DAYS
        && estimate_maturity(task, &chain) == Err(OutcomeUnavailable::Calendar)
    {
        extend(days, market, &mut chain, first, MORE_CALENDAR_DAYS).await?;
    }
    Ok(chain)
}

pub struct ProductMaterials<B, S> {
    pub broker: Arc<B>,
    pub settings: Arc<S>,
    pub directory: InstrumentDirectory,
    pub accounts: AccountSlot,
    pub days: Arc<dyn CalendarDays>,
}

impl<B, S> std::fmt::Debug for ProductMaterials<B, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductMaterials")
    }
}

impl<B: BrokerRead, S: SettingsRepository> ProductMaterials<B, S> {
    async fn reference_materials(
        &self,
        master: &domain::MasterInstrument,
        quote_instrument: &domain::QuoteInstrument,
        instant: Timestamp,
    ) -> Result<Result<(EstimateContext, Vec<domain::Candle>), EstimateSkip>, SystemPortError> {
        let market = *quote_instrument.market();
        let now = instant
            .in_exchange_zone(market)
            .map_err(|_error| unavailable())?;
        let calendar = self
            .days
            .calendar_on(market, now.offset_calendar_date())
            .await
            .map_err(broker_unavailable)?;
        let (context, finalized) = match calendar.state_at(now) {
            domain::MarketState::Unknown => return Ok(Err(EstimateSkip::StaleReference)),
            domain::MarketState::Closed => {
                let series =
                    crate::signal_flow::daily_series(self.broker.as_ref(), quote_instrument, now)
                        .await?;
                let Some(reference) =
                    domain::closed_estimate_reference(master, now, &calendar, &series)
                else {
                    return Ok(Err(EstimateSkip::StaleReference));
                };
                (reference, crate::signal_flow::settled_daily(series, now))
            }
            domain::MarketState::RegularMarket
            | domain::MarketState::DayMarket
            | domain::MarketState::PreMarket
            | domain::MarketState::AfterMarket => {
                return self
                    .live_materials(master, quote_instrument, &calendar, now)
                    .await;
            }
        };
        Ok(Ok((context, finalized)))
    }

    async fn live_materials(
        &self,
        master: &domain::MasterInstrument,
        quote_instrument: &domain::QuoteInstrument,
        calendar: &MarketCalendar,
        now: Timestamp,
    ) -> Result<Result<(EstimateContext, Vec<domain::Candle>), EstimateSkip>, SystemPortError> {
        let quotes = self
            .broker
            .quotes(std::slice::from_ref(quote_instrument))
            .await
            .map_err(broker_unavailable)?;
        let Some(quote) = quotes
            .into_iter()
            .find(|entry| &entry.instrument == quote_instrument)
            .and_then(|entry| entry.result.ok())
        else {
            return Ok(Err(EstimateSkip::NoQuote));
        };
        let at = now
            .max(quote.collected_at())
            .in_exchange_zone(*quote_instrument.market())
            .map_err(|_error| unavailable())?;
        let Some((price, trade_at)) = quote.last_price().zip(quote.timestamp()) else {
            return Ok(Err(EstimateSkip::NoQuote));
        };
        if !calendar.sessions().iter().any(|segment| {
            let interval = segment.session().interval();
            interval.start() <= trade_at && trade_at <= at && at < interval.end()
        }) {
            return Ok(Err(EstimateSkip::StaleReference));
        }
        let local = trade_at
            .in_exchange_zone(*quote_instrument.market())
            .map_err(|_error| unavailable())?;
        let Ok(reference) = master.price_observation(price, local) else {
            return Ok(Err(EstimateSkip::StaleReference));
        };
        let candles =
            crate::signal_flow::task_candles(self.broker.as_ref(), quote_instrument, at).await?;
        Ok(Ok((EstimateContext::new(reference, at), candles)))
    }

    async fn gather(
        &self,
        target: &TrackingTarget,
        now: Timestamp,
    ) -> Result<Result<EstimateTaskMaterials, EstimateSkip>, SystemPortError> {
        let Some(master) = self.directory.master(target.instrument()) else {
            return Ok(Err(EstimateSkip::NotTracked));
        };
        let (Ok(quote_instrument), Some(account)) = (
            master.quote_instrument(),
            self.accounts.current().into_iter().next(),
        ) else {
            return Ok(Err(EstimateSkip::NotTracked));
        };
        let market = *quote_instrument.market();
        let (context, finalized) = match self
            .reference_materials(&master, &quote_instrument, now)
            .await?
        {
            Ok(materials) => materials,
            Err(reason) => return Ok(Err(reason)),
        };
        let volatility = match Volatility::calculate(&finalized) {
            Ok(value) => value,
            Err(VolatilityError::SampleCount) => {
                return Ok(Err(EstimateSkip::VolatilitySamples));
            }
            Err(
                VolatilityError::Unadjusted
                | VolatilityError::MixedSeries
                | VolatilityError::Unordered
                | VolatilityError::NotDaily
                | VolatilityError::Arithmetic
                | VolatilityError::NonConvergence
                | VolatilityError::Unfinalized,
            ) => return Ok(Err(EstimateSkip::Candles)),
        };
        let profile = self
            .settings
            .profile()
            .await?
            .unwrap_or_else(InvestorProfile::default);
        let commission = self.broker.commission_rate(&account, market).await.ok();
        Ok(Ok(EstimateTaskMaterials {
            context,
            volatility,
            profile,
            commission,
            tax: trade_tax(market),
        }))
    }
}

impl<B: BrokerRead, S: SettingsRepository> EstimateTaskMaterialSource for ProductMaterials<B, S> {
    async fn materials(
        &self,
        target: &TrackingTarget,
        now: Timestamp,
    ) -> Result<Option<EstimateTaskMaterials>, SystemPortError> {
        Ok(self.gather(target, now).await?.ok())
    }

    fn explained_materials(
        &self,
        target: &TrackingTarget,
        now: Timestamp,
    ) -> impl Future<Output = Result<Result<EstimateTaskMaterials, EstimateSkip>, SystemPortError>> + Send
    {
        self.gather(target, now)
    }

    fn calendars(
        &self,
        task: &EstimateTask,
    ) -> impl Future<Output = Result<Vec<MarketCalendar>, SystemPortError>> + Send {
        task_calendars(self.days.as_ref(), task)
    }
}

pub trait BackgroundQuota: Send + Sync {
    fn admission(&self) -> BoxFuture<'_, CycleAdmission>;
}

pub struct QueueBudget<'a>(pub &'a dyn BackgroundQuota);

impl std::fmt::Debug for QueueBudget<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QueueBudget")
    }
}

impl BackgroundBudget for QueueBudget<'_> {
    async fn admit(&self, _now: Timestamp) -> Result<CycleAdmission, SystemPortError> {
        Ok(self.0.admission().await)
    }

    fn release(&self, _now: Timestamp) -> impl Future<Output = Result<(), SystemPortError>> + Send {
        ready(Ok(()))
    }
}

pub trait EstimateFlow: Send + Sync {
    fn cycle(&self) -> BoxFuture<'_, Result<EstimateCycleTally, JobError>>;
    fn intraday<'a>(
        &'a self,
        subject: &'a IntradayReviewSubject,
    ) -> BoxFuture<'a, Result<SingleEstimateOutcome, JobError>>;
}

#[derive(Clone)]
pub struct EstimateFlowSlot(pub Arc<dyn EstimateFlow>);

impl std::fmt::Debug for EstimateFlowSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateFlowSlot")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableEstimateFlow;

impl EstimateFlow for UnavailableEstimateFlow {
    fn cycle(&self) -> BoxFuture<'_, Result<EstimateCycleTally, JobError>> {
        Box::pin(ready(Err(JobError::PortUnavailable(String::from(
            "estimate-flow",
        )))))
    }

    fn intraday<'a>(
        &'a self,
        _subject: &'a IntradayReviewSubject,
    ) -> BoxFuture<'a, Result<SingleEstimateOutcome, JobError>> {
        Box::pin(ready(Ok(SingleEstimateOutcome::NoTask)))
    }
}

pub struct EstimateFlowParts<B, S, F, A, C> {
    pub materials: ProductMaterials<B, S>,
    pub store: Arc<S>,
    pub portfolios: Arc<F>,
    pub ai: Arc<A>,
    pub quota: Arc<dyn BackgroundQuota>,
    pub clock: Arc<C>,
    pub runs: EstimateRunBook,
    pub identity: EstimateRunIdentity,
}

impl<B, S, F, A, C> std::fmt::Debug for EstimateFlowParts<B, S, F, A, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateFlowParts")
    }
}

pub struct ProductEstimateFlow<B, S, F, A, C> {
    parts: EstimateFlowParts<B, S, F, A, C>,
    resume: tokio::sync::Mutex<Option<InstrumentId>>,
}

impl<B, S, F, A, C> std::fmt::Debug for ProductEstimateFlow<B, S, F, A, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductEstimateFlow")
    }
}

fn storage(error: impl std::fmt::Debug) -> JobError {
    JobError::StorageFailure(format!("{error:?}"))
}

impl<B, S, F, A, C> ProductEstimateFlow<B, S, F, A, C>
where
    B: BrokerRead + 'static,
    S: SettingsRepository + EstimateAssignmentRepository + WatchlistRepository + 'static,
    F: PortfolioRepository + 'static,
    A: AiProvider + 'static,
    C: Clock + 'static,
{
    #[must_use]
    pub fn new(parts: EstimateFlowParts<B, S, F, A, C>) -> Self {
        Self {
            parts,
            resume: tokio::sync::Mutex::new(None),
        }
    }

    async fn holdings(&self) -> Result<Vec<InstrumentId>, JobError> {
        let mut found = Vec::new();
        for account in &self.parts.materials.accounts.current() {
            let Some(record) = self
                .parts
                .portfolios
                .latest(account)
                .await
                .map_err(storage)?
            else {
                continue;
            };
            let portfolio = match record.outcome() {
                SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => value,
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => continue,
            };
            found.extend(
                portfolio
                    .holdings()
                    .iter()
                    .filter_map(|holding| self.holding_id(holding)),
            );
        }
        Ok(found)
    }

    fn holding_id(&self, holding: &domain::Holding) -> Option<InstrumentId> {
        let instrument = holding.instrument();
        let market = KnownMarket::try_from_market(&instrument.market()).ok()?;
        self.parts
            .materials
            .directory
            .instrument_of(market, instrument.symbol())
    }

    async fn universe(&self) -> Result<TrackedUniverse, JobError> {
        let holdings = self.holdings().await?;
        let watchlist = self.parts.store.load().await.map_err(storage)?;
        Ok(TrackedUniverse::compose(&holdings, &watchlist, &[]))
    }

    fn runner(&self) -> RunSingleEstimate<'_, ProductMaterials<B, S>, S, A, EstimateRunBook, C> {
        RunSingleEstimate {
            materials: &self.parts.materials,
            tasks: self.parts.store.as_ref(),
            ai: self.parts.ai.as_ref(),
            runs: &self.parts.runs,
            clock: self.parts.clock.as_ref(),
            identity: &self.parts.identity,
        }
    }

    async fn run_cycle(&self) -> Result<EstimateCycleTally, JobError> {
        let universe = self.universe().await?;
        let mut resume = self.resume.lock().await;
        let runner = self.runner();
        let budget = QueueBudget(self.parts.quota.as_ref());
        let report = RunEstimateCycle::new(&runner, &budget, self.parts.clock.as_ref())
            .execute(&universe, resume.as_ref())
            .await
            .map_err(storage)?;
        *resume = match &report.progress {
            CycleProgress::Finished => None,
            CycleProgress::Paused { resume_after, .. } => resume_after.clone(),
        };
        let submitted = report
            .attempted
            .iter()
            .filter(|(_, outcome)| *outcome == SingleEstimateOutcome::Submitted)
            .count();
        record_estimate_note(
            "estimate_cycle_attempts",
            match &report.progress {
                CycleProgress::Finished => "finished",
                CycleProgress::Paused { .. } => "paused",
            },
            u32::try_from(report.attempted.len())
                .ok()
                .unwrap_or(u32::MAX),
        );
        Ok(EstimateCycleTally {
            attempted: u32::try_from(report.attempted.len())
                .ok()
                .unwrap_or(u32::MAX),
            submitted: u32::try_from(submitted).ok().unwrap_or(u32::MAX),
        })
    }

    async fn run_intraday(
        &self,
        subject: &IntradayReviewSubject,
    ) -> Result<SingleEstimateOutcome, JobError> {
        let instrument = match subject {
            IntradayReviewSubject::PriceMove(instrument)
            | IntradayReviewSubject::NewsSurge(instrument) => instrument,
            IntradayReviewSubject::UrgentDisclosure(_) => return Ok(SingleEstimateOutcome::NoTask),
        };
        let universe = self.universe().await?;
        let Some(target) = universe
            .monitoring_targets()
            .into_iter()
            .find(|target| target.instrument() == instrument)
        else {
            return Ok(SingleEstimateOutcome::NoTask);
        };
        self.runner()
            .run(&target, &EstimateTrigger::Intraday(subject.clone()))
            .await
            .map_err(storage)
    }
}

impl<B, S, F, A, C> EstimateFlow for ProductEstimateFlow<B, S, F, A, C>
where
    B: BrokerRead + 'static,
    S: SettingsRepository + EstimateAssignmentRepository + WatchlistRepository + 'static,
    F: PortfolioRepository + 'static,
    A: AiProvider + 'static,
    C: Clock + 'static,
{
    fn cycle(&self) -> BoxFuture<'_, Result<EstimateCycleTally, JobError>> {
        Box::pin(self.run_cycle())
    }

    fn intraday<'a>(
        &'a self,
        subject: &'a IntradayReviewSubject,
    ) -> BoxFuture<'a, Result<SingleEstimateOutcome, JobError>> {
        Box::pin(self.run_intraday(subject))
    }
}
