use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use application::resolve_estimates::EstimateResolutionRepository;
use application::{
    AiRunId, BrokerRead, DraftProposalData, DraftProposalInputs, EstimateRepository,
    EvaluateSignalInputs, FxRateQuery, MarketDataRepository, MarketInfo, PortfolioRepository,
    ReasoningEffort, RetryDelay, SettingsRepository, SignalEvaluationData, SignalQuantityCosts,
    SignalRestorationBasis, StoredSignal, SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    AccountId, AdjustmentBasis, Calibration, CalibrationKey, CalibrationLedgerHead,
    CalibrationSnapshot, Candle, CandlePeriod, CandleStatus, EstimateRestoreEvidence,
    EstimateSubmissionRecord, EstimateTask, FxPair, FxRate, FxRateKind, HorizonSelection,
    HoursPermissionState, InstrumentId, InvestorProfile, KnownMarket, MasterInstrument,
    MasterInstrumentFacts, Nanosecond, OrderFxRates, OrderSide, Policy, Portfolio, PriceAdjustment,
    PriceObservation, QuoteInstrument, SectorAssignment, SectorAssignments, SectorCatalog,
    SectorComposition, SectorDepth, SpanSeconds, SyncOutcome, TimeInterval, TimeSpan, Timestamp,
    estimate_maturity, sector_assignment_from_compositions,
};

use crate::broker_link::AccountSlot;
use crate::estimate_flow::{CalendarDays, task_calendars, trade_tax};
use crate::holdings_link::InstrumentDirectory;

const CANDLE_LOOKBACK_DAYS: u64 = 150;
const DAY_SECONDS: u64 = 86_400;
const SECTOR_REFRESH_SECONDS: i64 = 86_400;

fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

fn port_unavailable<E>(_error: E) -> SystemPortError {
    unavailable()
}

#[must_use]
pub fn task_candle_window(created_at: Timestamp) -> Option<TimeInterval> {
    let span = TimeSpan::try_new(
        SpanSeconds::new(CANDLE_LOOKBACK_DAYS.saturating_mul(DAY_SECONDS)),
        Nanosecond::new(0),
    )
    .ok()?;
    TimeInterval::try_new(created_at.checked_sub(span).ok()?, created_at).ok()
}

#[must_use]
pub fn settled_daily(candles: Vec<Candle>, created_at: Timestamp) -> Vec<Candle> {
    let today = created_at.offset_calendar_date();
    candles
        .into_iter()
        .filter(|candle| candle.status() == CandleStatus::Finalized && candle.time().date() < today)
        .collect()
}

pub async fn task_candles<B: BrokerRead>(
    broker: &B,
    instrument: &QuoteInstrument,
    created_at: Timestamp,
) -> Result<Vec<Candle>, SystemPortError> {
    let window = task_candle_window(created_at).ok_or_else(unavailable)?;
    let candles = broker
        .candles(
            instrument,
            window,
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(created_at)),
        )
        .await
        .map_err(port_unavailable)?;
    Ok(settled_daily(candles, created_at))
}

struct SectorSnapshot {
    at: Timestamp,
    catalog: SectorCatalog,
    compositions: Vec<SectorComposition>,
}

pub struct SignalDataParts<B, S, F> {
    pub broker: Arc<B>,
    pub store: Arc<S>,
    pub portfolios: Arc<F>,
    pub directory: InstrumentDirectory,
    pub accounts: AccountSlot,
    pub days: Arc<dyn CalendarDays>,
}

impl<B, S, F> std::fmt::Debug for SignalDataParts<B, S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignalDataParts")
    }
}

pub struct ProductSignalData<B, S, F> {
    parts: SignalDataParts<B, S, F>,
    sectors: tokio::sync::Mutex<BTreeMap<KnownMarket, Arc<SectorSnapshot>>>,
}

impl<B, S, F> std::fmt::Debug for ProductSignalData<B, S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductSignalData")
    }
}

struct Holdings {
    portfolio: Option<Portfolio>,
    rate: Option<FxRate>,
    masters: Vec<MasterInstrument>,
}

fn market_of(task: &EstimateTask) -> Result<KnownMarket, SystemPortError> {
    KnownMarket::try_from_market(&task.reference().instrument().market()).map_err(port_unavailable)
}

impl<B, S, F> ProductSignalData<B, S, F>
where
    B: BrokerRead + MarketInfo,
    S: EstimateResolutionRepository
        + EstimateRepository
        + MarketDataRepository
        + SettingsRepository
        + SubmissionLookup,
    F: PortfolioRepository,
{
    #[must_use]
    pub fn new(parts: SignalDataParts<B, S, F>) -> Self {
        Self {
            parts,
            sectors: tokio::sync::Mutex::new(BTreeMap::new()),
        }
    }

    fn account(&self) -> Result<AccountId, SystemPortError> {
        self.parts
            .accounts
            .current()
            .into_iter()
            .next()
            .ok_or_else(unavailable)
    }

    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<(Option<CalibrationSnapshot>, Option<CalibrationLedgerHead>), SystemPortError> {
        if let Some(found) =
            EstimateResolutionRepository::calibration(self.parts.store.as_ref(), key).await?
        {
            return Ok((Some(found.snapshot), Some(found.head)));
        }
        let empty = Calibration::try_new(key.clone(), &[]).map_err(port_unavailable)?;
        let _recorded = self.parts.store.append_calibration(key, &empty).await?;
        Ok(
            match EstimateResolutionRepository::calibration(self.parts.store.as_ref(), key).await? {
                Some(found) => (Some(found.snapshot), Some(found.head)),
                None => (None, None),
            },
        )
    }

    async fn holdings(&self, account: &AccountId) -> Result<Holdings, SystemPortError> {
        let Some(record) = self.parts.portfolios.latest(account).await? else {
            return Ok(Holdings {
                portfolio: None,
                rate: None,
                masters: Vec::new(),
            });
        };
        let portfolio = match record.outcome() {
            SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => Some(value.clone()),
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => None,
        };
        let rate = self
            .parts
            .store
            .fx_rate(
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::At(record.recorded_at()),
            )
            .await?;
        let masters = portfolio
            .iter()
            .flat_map(Portfolio::holdings)
            .filter_map(|holding| {
                let market = KnownMarket::try_from_market(&holding.instrument().market()).ok()?;
                let id = self
                    .parts
                    .directory
                    .instrument_of(market, holding.instrument().symbol())?;
                self.parts.directory.master(&id)
            })
            .collect();
        Ok(Holdings {
            portfolio,
            rate,
            masters,
        })
    }

    async fn sector_snapshot(
        &self,
        market: KnownMarket,
        now: Timestamp,
    ) -> Result<Arc<SectorSnapshot>, SystemPortError> {
        let mut cache = self.sectors.lock().await;
        if let Some(found) = cache.get(&market).filter(|found| {
            now.utc_seconds()
                .as_i64()
                .saturating_sub(*found.at.utc_seconds().as_i64())
                < SECTOR_REFRESH_SECONDS
        }) {
            return Ok(Arc::clone(found));
        }
        let catalog = self
            .parts
            .broker
            .sectors()
            .await
            .map_err(port_unavailable)?;
        let mut compositions = Vec::new();
        for sector in catalog
            .sectors()
            .iter()
            .filter(|sector| sector.depth() == SectorDepth::Major)
        {
            if let Ok(found) = self
                .parts
                .broker
                .sector_composition(sector.id(), market)
                .await
            {
                compositions.push(found);
            }
        }
        let snapshot = Arc::new(SectorSnapshot {
            at: now,
            catalog,
            compositions,
        });
        let _previous = cache.insert(market, Arc::clone(&snapshot));
        Ok(snapshot)
    }

    async fn classifications(
        &self,
        masters: &[MasterInstrument],
        now: Timestamp,
    ) -> Result<SectorAssignments, SystemPortError> {
        let mut assignments: Vec<SectorAssignment> = Vec::new();
        for master in masters {
            let Ok(quote) = master.quote_instrument() else {
                continue;
            };
            let snapshot = self.sector_snapshot(*quote.market(), now).await?;
            if let Ok(found) = sector_assignment_from_compositions(
                master,
                &snapshot.catalog,
                &snapshot.compositions,
            ) {
                assignments.push(found);
            }
        }
        SectorAssignments::try_new(assignments).map_err(port_unavailable)
    }

    async fn target_price(
        &self,
        master: &MasterInstrument,
        quote: &QuoteInstrument,
    ) -> Result<Option<PriceObservation>, SystemPortError> {
        let quotes = self
            .parts
            .broker
            .quotes(std::slice::from_ref(quote))
            .await
            .map_err(port_unavailable)?;
        let Some(Ok(found)) = quotes.into_iter().next().map(|observed| observed.result) else {
            return Ok(None);
        };
        let Some(price) = found.last_price() else {
            return Ok(None);
        };
        let at = found.timestamp().unwrap_or_else(|| found.collected_at());
        Ok(master.price_observation(price, at).ok())
    }

    async fn order_fx(&self, market: KnownMarket) -> OrderFxRates {
        match market {
            KnownMarket::Kr => OrderFxRates::missing(),
            KnownMarket::Us => match self.parts.broker.order_fx_rates().await {
                Ok(rates) => rates,
                Err(_error) => OrderFxRates::missing(),
            },
        }
    }

    async fn restoration(
        &self,
        task: &EstimateTask,
        master: &MasterInstrument,
        quote: &QuoteInstrument,
        context: (
            &Holdings,
            &[domain::MarketCalendar],
            Option<&domain::CommissionRateFact>,
        ),
    ) -> Result<Option<SignalRestorationBasis>, SystemPortError> {
        let (holdings, calendars, commission) = context;
        let saved = task.saved_state();
        let candles = task_candles(self.parts.broker.as_ref(), quote, saved.created_at).await?;
        let permission = if saved.horizon.selection() == HorizonSelection::Hours {
            HoursPermissionState::Enabled
        } else {
            HoursPermissionState::Disabled
        };
        let evidence = EstimateRestoreEvidence {
            master: master.row().clone(),
            side: OrderSide::Buy,
            candles,
            reference_price: saved.reference_price,
            reference_at: saved.reference_at,
            task_at: saved.created_at,
            horizon: saved.horizon,
            permission,
            appetite: saved.appetite,
        };
        let (Ok(estimate), Some(cost), Some(tax), Ok(maturity)) = (
            evidence.capture(task, master),
            commission,
            trade_tax(saved.market),
            estimate_maturity(task, calendars),
        ) else {
            return Ok(None);
        };
        Ok(Some(SignalRestorationBasis {
            master: master.clone(),
            holding_masters: holdings.masters.clone(),
            estimate,
            commission: cost.clone(),
            tax,
            maturity,
            portfolio: holdings.portfolio.clone(),
            rate: holdings.rate,
        }))
    }

    async fn classified(
        &self,
        master: &MasterInstrument,
        holdings: &Holdings,
        now: Timestamp,
    ) -> Result<(domain::PolicySecurityFacts, SectorAssignments), SystemPortError> {
        let mut all = vec![master.clone()];
        for extra in &holdings.masters {
            if !all
                .iter()
                .any(|known| known.row().instrument_id == extra.row().instrument_id)
            {
                all.push(extra.clone());
            }
        }
        let security = MasterInstrumentFacts::try_new(all.clone())
            .map_err(port_unavailable)?
            .policy_security_facts();
        Ok((security, self.classifications(&all, now).await?))
    }

    pub async fn evaluation(
        &self,
        task: &EstimateTask,
        key: &CalibrationKey,
        now: Timestamp,
    ) -> Result<(EvaluateSignalInputs, MasterInstrument), SystemPortError> {
        let instrument = task.volatility().instrument();
        let master = self
            .parts
            .directory
            .master(instrument)
            .ok_or_else(unavailable)?;
        let quote = master.quote_instrument().map_err(port_unavailable)?;
        let market = market_of(task)?;
        let account = self.account()?;
        let (calibration, calibration_head) = self.calibration(key).await?;
        let holdings = self.holdings(&account).await?;
        let (security, assignments) = self.classified(&master, &holdings, now).await?;
        let profile = self
            .parts
            .store
            .profile()
            .await?
            .unwrap_or_else(InvestorProfile::default);
        let calendars = task_calendars(self.parts.days.as_ref(), task).await?;
        let commission = self
            .parts
            .broker
            .commission_rate(&account, market)
            .await
            .ok();
        let restoration = self
            .restoration(
                task,
                &master,
                &quote,
                (&holdings, &calendars, commission.as_ref()),
            )
            .await?;
        let quantity_costs = match commission {
            Some(value) => Some(SignalQuantityCosts {
                fx: self.order_fx(market).await,
                commission: value,
            }),
            None => None,
        };
        let inputs = EvaluateSignalInputs {
            calibration,
            calibration_head,
            valuation: holdings
                .portfolio
                .as_ref()
                .map(|portfolio| portfolio.evaluate(holdings.rate)),
            policy: Policy::new(profile),
            assignments,
            security,
            target_price: self.target_price(&master, &quote).await?,
            calendars,
            quantity_costs,
            restoration,
        };
        Ok((inputs, master))
    }

    async fn draft_inputs(
        &self,
        instrument: &InstrumentId,
        account: &AccountId,
        current: &StoredSignal,
        now: Timestamp,
    ) -> Result<DraftProposalInputs, SystemPortError> {
        let records = self.parts.store.submissions_for(instrument).await?;
        let record = records
            .iter()
            .rev()
            .find(|record| record.task_id() == &current.submission)
            .ok_or_else(unavailable)?;
        let task = record.submission().task_key().task();
        let (evaluation, master) = self
            .evaluation(task, record.submission().calibration_key(), now)
            .await?;
        let quote = master.quote_instrument().map_err(port_unavailable)?;
        let book = self
            .parts
            .broker
            .orderbook(&quote)
            .await
            .map_err(port_unavailable)?;
        let limits = self
            .parts
            .broker
            .price_limits(&quote)
            .await
            .map_err(port_unavailable)?;
        let sellable_quantity = match current.signal.side() {
            OrderSide::Buy => None,
            OrderSide::Sell => {
                let resolved = master
                    .order_gate_facts()
                    .and_then(|facts| facts.instrument_for(OrderSide::Sell))
                    .map_err(port_unavailable)?;
                Some(
                    self.parts
                        .broker
                        .sellable_quantity(account, &resolved)
                        .await
                        .map_err(port_unavailable)?,
                )
            }
        };
        Ok(DraftProposalInputs {
            evaluation,
            master,
            book,
            limits,
            sellable_quantity,
        })
    }
}

pub trait SubmissionLookup: Send + Sync {
    fn submissions_for(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send;
}

impl<T: application::EstimateSubmissionRepository> SubmissionLookup for T {
    fn submissions_for(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send {
        self.submission_records(instrument)
    }
}

impl<B, S, F> SignalEvaluationData for ProductSignalData<B, S, F>
where
    B: BrokerRead + MarketInfo,
    S: EstimateResolutionRepository
        + EstimateRepository
        + MarketDataRepository
        + SettingsRepository
        + SubmissionLookup,
    F: PortfolioRepository,
{
    async fn inputs(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> Result<EvaluateSignalInputs, SystemPortError> {
        self.evaluation(
            record.submission().task_key().task(),
            record.submission().calibration_key(),
            now,
        )
        .await
        .map(|(inputs, _master)| inputs)
    }
}

impl<B, S, F> DraftProposalData for ProductSignalData<B, S, F>
where
    B: BrokerRead + MarketInfo,
    S: EstimateResolutionRepository
        + EstimateRepository
        + MarketDataRepository
        + SettingsRepository
        + SubmissionLookup,
    F: PortfolioRepository,
{
    fn inputs(
        &self,
        instrument: &InstrumentId,
        account: &AccountId,
        current: &StoredSignal,
        now: Timestamp,
    ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
        self.draft_inputs(instrument, account, current, now)
    }
}

const OPEN_POLL_SECONDS: u64 = 60;
const MAX_OPEN_WAITS: u8 = 3;
const SHORT_ID_CHARS: usize = 8;

pub trait SignalPipelineRun: Send + Sync {
    fn on_submitted(&self, instrument: InstrumentId) -> crate::ipc::state::BoxFuture<'_, ()>;
}

#[derive(Default)]
pub struct SignalPipelineSlot(std::sync::OnceLock<Arc<dyn SignalPipelineRun>>);

impl std::fmt::Debug for SignalPipelineSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignalPipelineSlot")
    }
}

impl SignalPipelineSlot {
    #[must_use]
    pub fn subscribed_kinds() -> std::collections::BTreeSet<domain::DomainEventKind> {
        std::collections::BTreeSet::from([domain::DomainEventKind::EstimateSubmitted])
    }

    pub fn bind(&self, run: Arc<dyn SignalPipelineRun>) {
        let _first = self.0.set(run);
    }

    #[must_use]
    pub fn bound(&self) -> Option<Arc<dyn SignalPipelineRun>> {
        self.0.get().map(Arc::clone)
    }
}

#[derive(Debug)]
pub struct BridgedSignalPipeline(pub Arc<SignalPipelineSlot>);

impl application::EventSubscriber for BridgedSignalPipeline {
    fn on_event(&self, event: &domain::DomainEvent) -> Result<(), application::SubscriberFailure> {
        if let (domain::DomainEvent::EstimateSubmitted(submitted), Some(run)) =
            (event, self.0.bound())
        {
            let instrument = submitted.clone();
            let _spawned = tauri::async_runtime::spawn(async move {
                run.on_submitted(instrument).await;
            });
        }
        Ok(())
    }
}

fn short_id(instrument: &InstrumentId) -> String {
    instrument
        .as_str()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(SHORT_ID_CHARS)
        .collect()
}

#[must_use]
pub fn occurrence_id(
    instrument: &InstrumentId,
    now: Timestamp,
) -> Option<domain::BuySignalOccurrenceId> {
    domain::BuySignalOccurrenceId::try_new(&format!(
        "occ-{}-{}-{}",
        short_id(instrument),
        now.utc_seconds().as_i64(),
        now.nanosecond().as_u32()
    ))
    .ok()
}

#[must_use]
pub fn proposal_id(instrument: &InstrumentId, now: Timestamp) -> Option<domain::ProposalId> {
    domain::ClientOrderId::try_new(&format!(
        "p-{}-{}{:09}",
        short_id(instrument),
        now.utc_seconds().as_i64(),
        now.nanosecond().as_u32()
    ))
    .ok()
    .map(domain::ProposalId::new)
}

pub struct FlowReestimate(pub Arc<dyn crate::estimate_flow::EstimateFlow>);

impl std::fmt::Debug for FlowReestimate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlowReestimate")
    }
}

impl application::ReestimateScheduler for FlowReestimate {
    fn schedule(
        &self,
        instrument: &InstrumentId,
        _task: &domain::IssuedEstimateTaskId,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        let flow = Arc::clone(&self.0);
        let subject = application::IntradayReviewSubject::PriceMove(instrument.clone());
        let _spawned = tauri::async_runtime::spawn(async move {
            let _outcome = flow.intraday(&subject).await;
        });
        std::future::ready(Ok(application::WriteCompleted))
    }
}

pub struct ProductSignalPipeline<B, S, F, C> {
    pub data: ProductSignalData<B, S, F>,
    pub store: Arc<S>,
    pub clock: Arc<C>,
    pub events: Arc<application::EventBus>,
    pub reestimate: FlowReestimate,
}

impl<B, S, F, C> std::fmt::Debug for ProductSignalPipeline<B, S, F, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductSignalPipeline")
    }
}

fn next_regular_open(calendar: &domain::MarketCalendar, now: Timestamp) -> Option<Timestamp> {
    [calendar.today(), calendar.next_business_day()]
        .into_iter()
        .filter_map(|row| {
            row.sessions()
                .iter()
                .find(|segment| segment.state() == domain::MarketState::RegularMarket)
                .map(|segment| segment.session().interval().start())
        })
        .find(|start| *start > now)
}

impl<B, S, F, C> ProductSignalPipeline<B, S, F, C>
where
    B: BrokerRead + MarketInfo + 'static,
    S: EstimateResolutionRepository
        + EstimateRepository
        + MarketDataRepository
        + SettingsRepository
        + application::SignalRepository
        + application::SignalInvalidationRepository
        + application::EstimateSubmissionRepository
        + application::ProposalRepository
        + 'static,
    F: PortfolioRepository + 'static,
    C: application::Clock + 'static,
{
    async fn evaluate(&self, instrument: &InstrumentId) -> Option<()> {
        let now = self.clock.now();
        let occurrence = occurrence_id(instrument, now)?;
        application::EvaluateSignals::new(self.store.as_ref(), &self.data, &self.events)
            .execute(instrument, now, occurrence)
            .await
            .ok()
            .map(|_outcome| ())
    }

    async fn draft(&self, instrument: &InstrumentId) -> Option<application::DraftProposalOutcome> {
        let account = self.data.parts.accounts.current().into_iter().next()?;
        let id = proposal_id(instrument, self.clock.now())?;
        application::DraftProposal::new(
            self.store.as_ref(),
            &self.data,
            &self.reestimate,
            self.clock.as_ref(),
            &self.events,
        )
        .execute(instrument, &account, id)
        .await
        .ok()
    }

    async fn wait_for_open(&self, instrument: &InstrumentId) -> Option<()> {
        let master = self.data.parts.directory.master(instrument)?;
        let quote = master.quote_instrument().ok()?;
        let now = self.clock.now();
        let calendar = self
            .data
            .parts
            .days
            .calendar_on(*quote.market(), now.offset_calendar_date())
            .await
            .ok()?;
        let open = next_regular_open(&calendar, now)?;
        while self.clock.now() < open {
            tokio::time::sleep(std::time::Duration::from_secs(OPEN_POLL_SECONDS)).await;
        }
        Some(())
    }

    pub async fn run(&self, instrument: InstrumentId) {
        if self.evaluate(&instrument).await.is_none() {
            return;
        }
        for _ in 0..MAX_OPEN_WAITS {
            let closed = match self.draft(&instrument).await {
                Some(application::DraftProposalOutcome::MarketClosed) => true,
                Some(
                    application::DraftProposalOutcome::Created { .. }
                    | application::DraftProposalOutcome::Existing { .. }
                    | application::DraftProposalOutcome::NoSignal
                    | application::DraftProposalOutcome::ReestimateScheduled { .. },
                )
                | None => false,
            };
            if !closed || self.wait_for_open(&instrument).await.is_none() {
                return;
            }
        }
    }
}

impl<B, S, F, C> SignalPipelineRun for ProductSignalPipeline<B, S, F, C>
where
    B: BrokerRead + MarketInfo + 'static,
    S: EstimateResolutionRepository
        + EstimateRepository
        + MarketDataRepository
        + SettingsRepository
        + application::SignalRepository
        + application::SignalInvalidationRepository
        + application::EstimateSubmissionRepository
        + application::ProposalRepository
        + 'static,
    F: PortfolioRepository + 'static,
    C: application::Clock + 'static,
{
    fn on_submitted(&self, instrument: InstrumentId) -> crate::ipc::state::BoxFuture<'_, ()> {
        Box::pin(self.run(instrument))
    }
}
