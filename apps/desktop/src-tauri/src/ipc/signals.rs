use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use application::{
    AiRunId, Clock, EstimateSubmissionRepository, GetScorecard, PortError, ReasoningEffort,
    ScorecardObservationRepository, ScorecardSelection, ScorecardView, SignalRepository,
    SignalSkip, SignalSkipHistory, SignalSkipRepository, SignalSnapshot, SignalSourceFacts,
    SkipReason, SkipReasonError, SkipSignal, SystemPortError,
};
use domain::{
    CalendarDate, CalibrationBin, CalibrationKey, EstimateSubmissionRecord,
    HistoricalSignalDirection, InformationSource, InstrumentId, IssuedEstimateTaskId, KnownMarket,
    Market, MarketCalendar, MarketTradingStatus, OffsetSeconds, Price, Probability, Scorecard,
    ScorecardBin, Symbol, Timestamp,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::instruments::{InstrumentsIpcState, last_prices, tracked_instruments};
use super::portfolio::{UiMoney, precision, ui_money};
use super::state::{BoxFuture, ScreenState};
use crate::holdings_link::InstrumentDirectory;

#[cfg(test)]
mod tests;

const KST: i32 = 32_400;

type SubmissionRecord = EstimateSubmissionRecord<AiRunId, ReasoningEffort>;

pub trait SignalBoard: Send + Sync {
    fn snapshot(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<SignalSnapshot, SystemPortError>>;
    fn records(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<SubmissionRecord>, SystemPortError>>;
    fn source_facts(
        &self,
        task: IssuedEstimateTaskId,
    ) -> BoxFuture<'_, Result<SignalSourceFacts, SystemPortError>>;
    fn latest_skip(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Timestamp>, SystemPortError>>;
    fn skip(
        &self,
        instrument: InstrumentId,
        reason: Option<SkipReason>,
    ) -> BoxFuture<'_, Result<SignalSkip, SystemPortError>>;
    fn scorecards(&self) -> BoxFuture<'_, Option<ScorecardView>>;
}

pub trait MarketCalendars: Send + Sync {
    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>>;
}

struct StoredBoard<R, C> {
    repository: Arc<R>,
    clock: Arc<C>,
}

impl<R, C> SignalBoard for StoredBoard<R, C>
where
    R: SignalRepository
        + EstimateSubmissionRepository
        + SignalSkipRepository
        + SignalSkipHistory
        + ScorecardObservationRepository
        + 'static,
    C: Clock + 'static,
{
    fn snapshot(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<SignalSnapshot, SystemPortError>> {
        Box::pin(
            async move { SignalRepository::snapshot(self.repository.as_ref(), &instrument).await },
        )
    }

    fn records(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<SubmissionRecord>, SystemPortError>> {
        Box::pin(async move { self.repository.submission_records(&instrument).await })
    }

    fn source_facts(
        &self,
        task: IssuedEstimateTaskId,
    ) -> BoxFuture<'_, Result<SignalSourceFacts, SystemPortError>> {
        Box::pin(async move { self.repository.sources(&task).await })
    }

    fn latest_skip(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Timestamp>, SystemPortError>> {
        Box::pin(async move { self.repository.latest_skip(&instrument).await })
    }

    fn skip(
        &self,
        instrument: InstrumentId,
        reason: Option<SkipReason>,
    ) -> BoxFuture<'_, Result<SignalSkip, SystemPortError>> {
        Box::pin(async move {
            SkipSignal::new(self.repository.as_ref(), self.clock.as_ref())
                .execute(instrument, reason)
                .await
        })
    }

    fn scorecards(&self) -> BoxFuture<'_, Option<ScorecardView>> {
        Box::pin(async move {
            GetScorecard::new(self.repository.as_ref(), self.clock.as_ref())
                .execute(ScorecardSelection::All)
                .await
                .ok()
        })
    }
}

pub trait CurrentProposals: Send + Sync {
    fn current(
        &self,
        instrument: &InstrumentId,
    ) -> BoxFuture<'_, Result<Option<domain::ProposalId>, SystemPortError>>;
}

#[derive(Clone)]
pub struct SignalsIpcState {
    proposals: Option<Arc<dyn CurrentProposals>>,
    board: Arc<dyn SignalBoard>,
    calendars: Arc<dyn MarketCalendars>,
    clock: Arc<dyn Clock>,
    cached: Arc<RwLock<BTreeMap<(KnownMarket, CalendarDate), MarketCalendar>>>,
}

impl fmt::Debug for SignalsIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignalsIpcState")
    }
}

impl SignalsIpcState {
    #[must_use]
    pub fn with_proposals(self, proposals: Arc<dyn CurrentProposals>) -> Self {
        Self {
            proposals: Some(proposals),
            ..self
        }
    }

    #[must_use]
    pub fn new<R, C>(repository: Arc<R>, calendars: Arc<dyn MarketCalendars>, clock: Arc<C>) -> Self
    where
        R: SignalRepository
            + EstimateSubmissionRepository
            + SignalSkipRepository
            + SignalSkipHistory
            + ScorecardObservationRepository
            + 'static,
        C: Clock + 'static,
    {
        Self {
            proposals: None,
            board: Arc::new(StoredBoard {
                repository,
                clock: Arc::clone(&clock),
            }),
            calendars,
            clock,
            cached: Arc::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum SignalsScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetSignalsRequest {
    pub scope: SignalsScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SkipSignalRequest {
    pub symbol: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiSignalDirection {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiMarketState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "closed")]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiOrderSwitch {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiSignalSourceKind {
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "web")]
    Web,
    #[serde(rename = "news")]
    News,
    #[serde(rename = "disclosure")]
    Disclosure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSignalSource {
    pub kind: UiSignalSourceKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSignalCard {
    pub symbol: String,
    pub name: String,
    pub direction: UiSignalDirection,
    pub market: UiMarketState,
    pub order_switch: UiOrderSwitch,
    pub price: Option<UiMoney>,
    pub ai_probability: String,
    pub tracked_probability: String,
    pub threshold: String,
    pub bin_hit_rate: Option<String>,
    pub bin_count: String,
    pub rationale: String,
    pub counterargument: String,
    pub sources: Vec<UiSignalSource>,
    pub proposal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSignalsView {
    pub cards: Vec<UiSignalCard>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiSkipOutcome {
    #[serde(rename = "recorded")]
    Recorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiSkipResult {
    pub outcome: UiSkipOutcome,
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn storage(failure: &SystemPortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

fn probability_text(probability: Probability) -> Option<String> {
    Some(
        domain::display_probability_percent(probability, precision(1)?)
            .as_str()
            .to_owned(),
    )
}

fn ratio_text(ratio: &domain::Ratio) -> Option<String> {
    Some(
        domain::display_percent(ratio, precision(1)?)
            .as_str()
            .to_owned(),
    )
}

fn cited(facts: &SignalSourceFacts) -> Vec<UiSignalSource> {
    let items = facts.items.iter().map(|item| UiSignalSource {
        kind: match item.source() {
            InformationSource::Web => UiSignalSourceKind::Web,
            InformationSource::News(_) => UiSignalSourceKind::News,
            InformationSource::Disclosure(_) => UiSignalSourceKind::Disclosure,
        },
        label: item.title().as_untrusted_str().to_owned(),
    });
    let disclosures = facts.disclosures.iter().map(|disclosure| UiSignalSource {
        kind: UiSignalSourceKind::Disclosure,
        label: disclosure.collected().title().as_untrusted_str().to_owned(),
    });
    let tools = facts.tools.iter().map(|tool| UiSignalSource {
        kind: UiSignalSourceKind::Tool,
        label: tool.as_str().to_owned(),
    });
    items.chain(disclosures).chain(tools).collect()
}

fn bin_facts(
    view: Option<&ScorecardView>,
    key: &CalibrationKey,
    tracked: Probability,
) -> (Option<String>, String) {
    let bin = CalibrationBin::for_probability(tracked);
    let cards: Vec<&Scorecard> = match view {
        Some(ScorecardView::Cards(cards)) => cards.cards().iter().collect(),
        Some(ScorecardView::Single(card)) => vec![card],
        Some(ScorecardView::NoObservations | ScorecardView::UnknownCombination) | None => {
            Vec::new()
        }
    };
    let observed = cards.into_iter().find_map(|card| match card {
        Scorecard::Ready(metrics) if metrics.key() == key => {
            metrics.reliability().iter().find_map(|row| match row {
                ScorecardBin::Observed(point) if point.bin() == bin => Some(point.clone()),
                ScorecardBin::Observed(_) | ScorecardBin::NoData(_) => None,
            })
        }
        Scorecard::Ready(_) | Scorecard::Pending(_) => None,
    });
    match observed {
        Some(point) => (
            precision(1).and_then(|digits| {
                domain::display_scorecard_percent(point.hit_rate(), digits)
                    .map(|shown| shown.as_str().to_owned())
            }),
            point.count().as_usize().to_string(),
        ),
        None => (None, String::from("0")),
    }
}

const fn known_market(market: &Market) -> Option<KnownMarket> {
    match market {
        Market::Kr => Some(KnownMarket::Kr),
        Market::Us => Some(KnownMarket::Us),
        Market::Unknown(_) => None,
    }
}

async fn market_state(state: &SignalsIpcState, market: Option<KnownMarket>) -> UiMarketState {
    let Some(known) = market else {
        return UiMarketState::Closed;
    };
    let now = state.clock.now();
    let Ok(date) = now.date_at_offset(OffsetSeconds::new(KST)) else {
        return UiMarketState::Closed;
    };
    let cached = {
        let calendars = match state.cached.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        calendars.get(&(known, date)).cloned()
    };
    let calendar = match cached {
        Some(found) => found,
        None => match state.calendars.calendar_on(known, date).await {
            Ok(fetched) => {
                let mut calendars = match state.cached.write() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let _previous = calendars.insert((known, date), fetched.clone());
                fetched
            }
            Err(_unknown) => return UiMarketState::Closed,
        },
    };
    match calendar.state_at(now).trading_status() {
        MarketTradingStatus::Open => UiMarketState::Open,
        MarketTradingStatus::Closed | MarketTradingStatus::Unknown => UiMarketState::Closed,
    }
}

async fn order_switch(screen: Option<&ScreenState>) -> UiOrderSwitch {
    let Some(backend) = screen.map(ScreenState::backend) else {
        return UiOrderSwitch::Off;
    };
    match backend.load_settings().await {
        Ok(loaded) => match loaded.risk_settings.map(domain::RiskSettings::orders) {
            Some(domain::OrderPermission::Enabled) => UiOrderSwitch::On,
            Some(domain::OrderPermission::Disabled) | None => UiOrderSwitch::Off,
        },
        Err(_unreadable) => UiOrderSwitch::Off,
    }
}

struct CardInputs<'a> {
    symbol: &'a Symbol,
    name: &'a str,
    price: Option<Price>,
    market: UiMarketState,
    switch: UiOrderSwitch,
    scorecards: Option<&'a ScorecardView>,
    proposal: Option<domain::ProposalId>,
}

fn card(
    inputs: &CardInputs<'_>,
    stored: &application::StoredSignal,
    record: &SubmissionRecord,
    facts: &SignalSourceFacts,
) -> Option<UiSignalCard> {
    let shown = stored.signal.display_facts();
    let (direction, threshold) = match shown.direction {
        HistoricalSignalDirection::Buy { .. } => (UiSignalDirection::Buy, shown.buy_threshold),
        HistoricalSignalDirection::Sell { .. } => (UiSignalDirection::Sell, shown.sell_threshold),
    };
    let (bin_hit_rate, bin_count) = bin_facts(
        inputs.scorecards,
        stored.signal.calibration_key(),
        shown.probability,
    );
    let narrative = record.narrative();
    Some(UiSignalCard {
        symbol: inputs.symbol.as_str().to_owned(),
        name: inputs.name.to_owned(),
        direction,
        market: inputs.market,
        order_switch: inputs.switch,
        price: match inputs.price {
            Some(value) => Some(ui_money(&value.as_money())?),
            None => None,
        },
        ai_probability: probability_text(record.submission().probability().into_probability())?,
        tracked_probability: probability_text(shown.probability)?,
        threshold: ratio_text(&threshold)?,
        bin_hit_rate,
        bin_count,
        rationale: narrative.rationale.as_untrusted_str().to_owned(),
        counterargument: narrative.counterargument.as_untrusted_str().to_owned(),
        sources: cited(facts),
        proposal: inputs
            .proposal
            .as_ref()
            .map(|id| id.as_client_order_id().as_str().to_owned()),
    })
}

async fn raised_signal(
    state: &SignalsIpcState,
    instrument: InstrumentId,
) -> Result<Option<(application::StoredSignal, SubmissionRecord)>, IpcError> {
    let snapshot = state
        .board
        .snapshot(instrument.clone())
        .await
        .map_err(|failure| storage(&failure))?;
    let Some(stored) = snapshot.current else {
        return Ok(None);
    };
    let records = state
        .board
        .records(instrument.clone())
        .await
        .map_err(|failure| storage(&failure))?;
    let Some(record) = records
        .into_iter()
        .find(|record| *record.task_id() == stored.submission)
    else {
        return Ok(None);
    };
    let skipped = state
        .board
        .latest_skip(instrument)
        .await
        .map_err(|failure| storage(&failure))?;
    if skipped.is_some_and(|at| at >= record.submission().submitted_at()) {
        return Ok(None);
    }
    Ok(Some((stored, record)))
}

pub async fn load_signals(
    state: &SignalsIpcState,
    instruments: &InstrumentsIpcState,
    screen: Option<&ScreenState>,
    directory: &InstrumentDirectory,
) -> Result<UiSignalsView, IpcError> {
    let tracked = tracked_instruments(instruments, screen, directory).await?;
    let mut raised = Vec::new();
    for row in tracked {
        let instrument = row.entry.master.row().instrument_id.clone();
        if let Some((stored, record)) = raised_signal(state, instrument).await? {
            raised.push((row, stored, record));
        }
    }
    if raised.is_empty() {
        return Ok(UiSignalsView { cards: Vec::new() });
    }
    let quoted = raised
        .iter()
        .filter_map(|(row, _, _)| row.entry.master.quote_instrument().ok())
        .collect();
    let prices = last_prices(instruments, quoted).await;
    let switch = order_switch(screen).await;
    let scorecards = state.board.scorecards().await;
    let mut cards = Vec::new();
    for (row, stored, record) in &raised {
        let facts = state
            .board
            .source_facts(stored.submission.clone())
            .await
            .map_err(|failure| storage(&failure))?;
        let market = market_state(state, known_market(&stored.signal.market())).await;
        let proposal = match &state.proposals {
            Some(lookup) => lookup
                .current(&row.entry.master.row().instrument_id)
                .await
                .map_err(|failure| storage(&failure))?,
            None => None,
        };
        let inputs = CardInputs {
            symbol: &row.symbol,
            name: row.entry.name.as_str(),
            price: prices
                .as_ref()
                .and_then(|found| found.get(&row.symbol).copied()),
            market,
            switch,
            scorecards: scorecards.as_ref(),
            proposal,
        };
        cards.push(card(&inputs, stored, record, &facts).ok_or(error(IpcErrorKind::Bug))?);
    }
    Ok(UiSignalsView { cards })
}

pub async fn skip_signal_card(
    state: &SignalsIpcState,
    directory: &InstrumentDirectory,
    symbol: &Symbol,
    typed: &str,
) -> Result<UiSkipResult, IpcError> {
    let entry = directory.entry(symbol).ok_or(error(IpcErrorKind::NoData))?;
    let reason = match SkipReason::try_new(typed) {
        Ok(valid) => Some(valid),
        Err(SkipReasonError::Empty) => None,
        Err(SkipReasonError::MultipleLines | SkipReasonError::TooLong) => {
            return Err(error(IpcErrorKind::UserFixable));
        }
    };
    let _recorded = state
        .board
        .skip(entry.master.row().instrument_id.clone(), reason)
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(UiSkipResult {
        outcome: UiSkipOutcome::Recorded,
    })
}

fn managed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(SignalsIpcState, InstrumentDirectory), IpcError> {
    let state = app
        .try_state::<SignalsIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
    Ok((state, directory))
}

#[tauri::command]
#[specta::specta]
pub async fn get_signals<R: Runtime>(
    app: AppHandle<R>,
    request: GetSignalsRequest,
) -> Result<UiSignalsView, IpcError> {
    let GetSignalsRequest {
        scope: SignalsScope::Current,
    } = request;
    let (state, directory) = managed(&app)?;
    let instruments = app
        .try_state::<InstrumentsIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))?;
    let screen = app.try_state::<ScreenState>();
    load_signals(
        &state,
        &instruments,
        screen.as_ref().map(tauri::State::inner),
        &directory,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn skip_signal<R: Runtime>(
    app: AppHandle<R>,
    request: SkipSignalRequest,
) -> Result<UiSkipResult, IpcError> {
    let symbol =
        Symbol::try_new(&request.symbol).map_err(|_invalid| error(IpcErrorKind::UserFixable))?;
    let (state, directory) = managed(&app)?;
    skip_signal_card(&state, &directory, &symbol, &request.reason).await
}
