use crate::{Script, script::State};
use application::{
    CacheWriteOutcome, EstimateRepository, FxRateQuery, InformationKey, InformationLinks,
    InstrumentMappingRepository, MappingStoreError, MarketDataRepository, NewsRepository,
    PortfolioRepository, ProposalRepository, RecordOutcome, SettingsRepository, SystemPortError,
    ThesisRepository, TransactionRepository, ValuationRepository, WriteCompleted,
};
use domain::{
    AccountId, Calibration, CalibrationKey, Candle, CandlePeriod, Draft, EstimateOutcome, FxPair,
    FxRate, FxRateKind, InstrumentId, InstrumentMappingRecord, InstrumentMappings,
    InstrumentSource, InvestorProfile, MappingAssignment, NewsItem, Portfolio, PriceAdjustment,
    Proposal, ProposalId, Quote, RiskSettings, SettledRowSet, SignalSubmission, SignalTaskKey,
    SyncRecord, Thesis, TimeInterval, Timestamp, ValuationSnapshot,
};
use std::{future::poll_fn, task::Poll};

fn append_candles(stored: &mut Vec<Candle>, incoming: &[Candle]) {
    for value in incoming {
        if !stored.contains(value) {
            stored.push(value.clone());
        }
    }
}
#[derive(Debug)]
pub enum MemoryReply<O> {
    Memory,
    Return(O),
}
#[derive(Debug, Default)]
struct PortfolioRepositoryState {
    records: Vec<(AccountId, SyncRecord<Portfolio>)>,
}
#[derive(Debug, Default)]
pub struct MemoryPortfolioRepository {
    record: MemoryPortfolioRepositoryRecordScript,
    latest: MemoryPortfolioRepositoryLatestScript,
    state: State<PortfolioRepositoryState>,
    calls: State<Vec<crate::scripted::PortfolioRepositoryCall>>,
}
impl MemoryPortfolioRepository {
    #[must_use]
    pub fn script_latest(&self) -> &MemoryPortfolioRepositoryLatestScript {
        &self.latest
    }

    #[must_use]
    pub fn script_record(&self) -> &MemoryPortfolioRepositoryRecordScript {
        &self.record
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::PortfolioRepositoryCall> {
        self.calls.write().clone()
    }
}
impl PortfolioRepository for MemoryPortfolioRepository {
    async fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<Portfolio>,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::PortfolioRepositoryCall::Record(
                account.clone(),
                Box::new(record.clone()),
            ));
        match self.record.run((account.clone(), record.clone())).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                state.records.push((account.clone(), record.clone()));
                Ok(WriteCompleted)
            }
        }
    }
    async fn latest(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<Portfolio>>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::PortfolioRepositoryCall::Latest(
                account.clone(),
            ));
        match self.latest.run((account.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .records
                    .iter()
                    .filter(|(key, _)| key == account)
                    .max_by_key(|(_, value)| value.recorded_at())
                    .map(|(_, value)| value.clone()))
            }
        }
    }
}
#[derive(Debug, Default)]
struct TransactionRepositoryState {
    records: Vec<(AccountId, SyncRecord<SettledRowSet>)>,
}
#[derive(Debug, Default)]
pub struct MemoryTransactionRepository {
    record: MemoryTransactionRepositoryRecordScript,
    latest: MemoryTransactionRepositoryLatestScript,
    state: State<TransactionRepositoryState>,
    calls: State<Vec<crate::scripted::TransactionRepositoryCall>>,
}
impl MemoryTransactionRepository {
    #[must_use]
    pub fn script_record(&self) -> &MemoryTransactionRepositoryRecordScript {
        &self.record
    }
    #[must_use]
    pub fn script_latest(&self) -> &MemoryTransactionRepositoryLatestScript {
        &self.latest
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::TransactionRepositoryCall> {
        self.calls.write().clone()
    }
}
impl TransactionRepository for MemoryTransactionRepository {
    async fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<SettledRowSet>,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::TransactionRepositoryCall::Record(
                account.clone(),
                record.clone(),
            ));
        match self.record.run((account.clone(), record.clone())).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                state.records.push((account.clone(), record.clone()));
                Ok(WriteCompleted)
            }
        }
    }
    async fn latest(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<SettledRowSet>>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::TransactionRepositoryCall::Latest(
                account.clone(),
            ));
        match self.latest.run((account.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .records
                    .iter()
                    .filter(|(key, _)| key == account)
                    .max_by_key(|(_, value)| value.recorded_at())
                    .map(|(_, value)| value.clone()))
            }
        }
    }
}
#[derive(Debug, Default)]
struct MarketDataRepositoryState {
    quotes: Vec<MemoryQuote>,
    candles: Vec<Candle>,
    rates: Vec<FxRate>,
    close_evidence: Vec<domain::ObservedCloseEvidence>,
    close_evidence_error: Option<SystemPortError>,
}
#[derive(Debug)]
struct MemoryQuote {
    instrument: InstrumentId,
    quote: Quote,
    last_server: Option<Timestamp>,
}
fn remember_quote(
    state: &mut MarketDataRepositoryState,
    instrument: &InstrumentId,
    quote: &Quote,
) -> CacheWriteOutcome {
    if let Some(current) = state
        .quotes
        .iter_mut()
        .find(|value| &value.instrument == instrument)
    {
        if quote.collected_at() < current.quote.collected_at() {
            return CacheWriteOutcome::OlderIgnored;
        }
        if let (Some(observed), Some(previous)) = (quote.timestamp(), current.last_server)
            && observed < previous
        {
            return CacheWriteOutcome::OlderIgnored;
        }
        if *quote == current.quote {
            return CacheWriteOutcome::Unchanged;
        }
        current.last_server = quote.timestamp().or(current.last_server);
        current.quote = quote.clone();
    } else {
        state.quotes.push(MemoryQuote {
            instrument: instrument.clone(),
            quote: quote.clone(),
            last_server: quote.timestamp(),
        });
    }
    CacheWriteOutcome::Changed
}
fn remember_fx(state: &mut MarketDataRepositoryState, rate: FxRate) -> CacheWriteOutcome {
    let previous = state
        .rates
        .iter_mut()
        .filter(|value| value.pair() == rate.pair() && value.kind() == rate.kind())
        .max_by_key(|value| value.as_of());
    if let Some(current) = previous {
        if rate.as_of() < current.as_of() {
            return CacheWriteOutcome::OlderIgnored;
        }
        if rate.as_of() == current.as_of() {
            let outcome = if rate.quote_per_base() == current.quote_per_base()
                && rate.source() == current.source()
            {
                CacheWriteOutcome::Unchanged
            } else {
                CacheWriteOutcome::Changed
            };
            *current = rate;
            return outcome;
        }
    }
    state.rates.push(rate);
    CacheWriteOutcome::Changed
}
#[derive(Debug, Default)]
pub struct MemoryMarketDataRepository {
    store_quote: MemoryMarketDataRepositoryStoreQuoteScript,
    quote: MemoryMarketDataRepositoryQuoteScript,
    store_candles: MemoryMarketDataRepositoryStoreCandlesScript,
    candles: MemoryMarketDataRepositoryCandlesScript,
    store_fx_rate: MemoryMarketDataRepositoryStoreFxRateScript,
    fx_rate: MemoryMarketDataRepositoryFxRateScript,
    state: State<MarketDataRepositoryState>,
    calls: State<Vec<crate::scripted::MarketDataRepositoryCall>>,
}
impl MemoryMarketDataRepository {
    pub fn supply_close_evidence(&self, evidence: domain::ObservedCloseEvidence) {
        self.state.write().close_evidence.push(evidence);
    }
    pub fn fail_close_evidence(&self, error: SystemPortError) {
        self.state.write().close_evidence_error = Some(error);
    }
    #[must_use]
    pub fn script_store_quote(&self) -> &MemoryMarketDataRepositoryStoreQuoteScript {
        &self.store_quote
    }
    #[must_use]
    pub fn script_store_candles(&self) -> &MemoryMarketDataRepositoryStoreCandlesScript {
        &self.store_candles
    }

    #[must_use]
    pub fn script_quote(&self) -> &MemoryMarketDataRepositoryQuoteScript {
        &self.quote
    }
    #[must_use]
    pub fn script_candles(&self) -> &MemoryMarketDataRepositoryCandlesScript {
        &self.candles
    }
    #[must_use]
    pub fn script_store_fx_rate(&self) -> &MemoryMarketDataRepositoryStoreFxRateScript {
        &self.store_fx_rate
    }
    #[must_use]
    pub fn script_fx_rate(&self) -> &MemoryMarketDataRepositoryFxRateScript {
        &self.fx_rate
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::MarketDataRepositoryCall> {
        self.calls.write().clone()
    }
}
impl MarketDataRepository for MemoryMarketDataRepository {
    async fn store_quote(
        &self,
        instrument: &InstrumentId,
        quote: &Quote,
    ) -> Result<CacheWriteOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::StoreQuote(
                instrument.clone(),
                quote.clone(),
            ));
        match self
            .store_quote
            .run((instrument.clone(), quote.clone()))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                Ok(remember_quote(&mut state, instrument, quote))
            }
        }
    }
    async fn quote(&self, instrument: &InstrumentId) -> Result<Option<Quote>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::Quote(
                instrument.clone(),
            ));
        match self.quote.run((instrument.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .quotes
                    .iter()
                    .find(|value| &value.instrument == instrument)
                    .map(|value| value.quote.clone()))
            }
        }
    }
    async fn store_candles(
        &self,
        instrument: &InstrumentId,
        candles: &[Candle],
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::StoreCandles(
                instrument.clone(),
                candles.to_vec(),
            ));
        match self
            .store_candles
            .run((instrument.clone(), candles.to_vec()))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                if candles.iter().any(|value| value.instrument() != instrument) {
                    return Err(SystemPortError::new(
                        application::SystemPort::Repository,
                        application::SystemFailure::InvalidData,
                    ));
                }
                let mut state = self.state.write();
                append_candles(&mut state.candles, candles);
                Ok(WriteCompleted)
            }
        }
    }
    async fn candles(
        &self,
        instrument: &InstrumentId,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::Candles(
                instrument.clone(),
                window,
                period,
                adjustment,
            ));
        match self
            .candles
            .run((instrument.clone(), window, period, adjustment))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                let mut values: Vec<_> = state
                    .candles
                    .iter()
                    .filter(|value| {
                        value.instrument() == instrument
                            && value.time().timestamp() >= window.start()
                            && value.time().timestamp() < window.end()
                            && value.time().period() == period
                            && value.adjustment() == adjustment
                    })
                    .cloned()
                    .collect();
                values.sort_by_key(|value| value.time().timestamp());
                Ok(values)
            }
        }
    }
    async fn store_fx_rate(&self, rate: &FxRate) -> Result<CacheWriteOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::StoreFxRate(
                *rate,
            ));
        match self.store_fx_rate.run((*rate,)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                Ok(remember_fx(&mut state, *rate))
            }
        }
    }
    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: FxRateQuery,
    ) -> Result<Option<FxRate>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::MarketDataRepositoryCall::FxRate(
                pair, kind, at,
            ));
        match self.fx_rate.run((pair, kind, at)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .rates
                    .iter()
                    .filter(|value| {
                        value.pair() == pair
                            && value.kind() == kind
                            && match at {
                                FxRateQuery::Latest => true,
                                FxRateQuery::At(requested) => value.as_of() <= requested,
                            }
                    })
                    .max_by_key(|value| value.as_of())
                    .copied())
            }
        }
    }
}
#[derive(Debug, Default)]
struct NewsRepositoryState {
    linked: Vec<(InformationKey, Vec<InstrumentId>)>,
    items: Vec<NewsItem>,
}
fn news_matches(key: &InformationKey, item: &NewsItem) -> bool {
    match (key, item.collected().source()) {
        (InformationKey::News(source, url), domain::InformationSource::News(actual)) => {
            source == actual && url == item.collected().url()
        }
        (
            InformationKey::News(_, _),
            domain::InformationSource::Web | domain::InformationSource::Disclosure(_),
        )
        | (InformationKey::Disclosure(_), _) => false,
    }
}
fn remember_news(state: &mut NewsRepositoryState, item: &NewsItem) {
    let existing = state.items.iter_mut().find(|value| {
        value.collected().source() == item.collected().source()
            && value.collected().url() == item.collected().url()
    });
    if let Some(value) = existing {
        *value = item.clone();
    } else {
        state.items.push(item.clone());
    }
}
fn linked_news(state: &NewsRepositoryState, instrument: &InstrumentId) -> Vec<NewsItem> {
    state
        .items
        .iter()
        .filter(|item| {
            state.linked.iter().any(|(key, instruments)| {
                instruments.contains(instrument) && news_matches(key, item)
            })
        })
        .cloned()
        .collect()
}
#[derive(Debug, Default)]
pub struct MemoryNewsRepository {
    store: MemoryNewsRepositoryStoreScript,
    for_instrument: MemoryNewsRepositoryForInstrumentScript,
    state: State<NewsRepositoryState>,
    calls: State<Vec<crate::scripted::NewsRepositoryCall>>,
}
impl MemoryNewsRepository {
    #[must_use]
    pub fn script_for_instrument(&self) -> &MemoryNewsRepositoryForInstrumentScript {
        &self.for_instrument
    }

    #[must_use]
    pub fn script_store(&self) -> &MemoryNewsRepositoryStoreScript {
        &self.store
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::NewsRepositoryCall> {
        self.calls.write().clone()
    }
}
impl InformationLinks for MemoryNewsRepository {
    fn set_instruments(
        &self,
        item: &InformationKey,
        instruments: &[InstrumentId],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut state = self.state.write();
        state.linked.retain(|(existing, _)| existing != item);
        let unique = instruments
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if !unique.is_empty() {
            state
                .linked
                .push((item.clone(), unique.into_iter().collect()));
        }
        std::future::ready(Ok(WriteCompleted))
    }
}
impl NewsRepository for MemoryNewsRepository {
    async fn store(&self, item: &NewsItem) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::NewsRepositoryCall::Store(item.clone()));
        match self.store.run((item.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                remember_news(&mut state, item);
                Ok(WriteCompleted)
            }
        }
    }
    async fn for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<NewsItem>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::NewsRepositoryCall::ForInstrument(
                instrument.clone(),
            ));
        match self.for_instrument.run((instrument.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(linked_news(&state, instrument))
            }
        }
    }
}
#[derive(Debug, Default)]
struct ThesisRepositoryState {
    records: Vec<(InstrumentId, crate::ThesisRecord)>,
}
#[derive(Debug, Default)]
pub struct MemoryThesisRepository {
    store: MemoryThesisRepositoryStoreScript,
    load: MemoryThesisRepositoryLoadScript,
    state: State<ThesisRepositoryState>,
    calls: State<Vec<crate::scripted::ThesisRepositoryCall>>,
}
impl MemoryThesisRepository {
    #[must_use]
    pub fn script_store(&self) -> &MemoryThesisRepositoryStoreScript {
        &self.store
    }
    #[must_use]
    pub fn script_load(&self) -> &MemoryThesisRepositoryLoadScript {
        &self.load
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::ThesisRepositoryCall> {
        self.calls.write().clone()
    }
}
impl ThesisRepository for MemoryThesisRepository {
    async fn store(
        &self,
        instrument: &InstrumentId,
        thesis: &Thesis,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ThesisRepositoryCall::Store(
                instrument.clone(),
                crate::ThesisRecord::from_thesis(thesis),
            ));
        match self
            .store
            .run((instrument.clone(), crate::ThesisRecord::from_thesis(thesis)))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                state
                    .records
                    .push((instrument.clone(), crate::ThesisRecord::from_thesis(thesis)));
                Ok(WriteCompleted)
            }
        }
    }
    async fn load(&self, instrument: &InstrumentId) -> Result<Option<Thesis>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ThesisRepositoryCall::Load(
                instrument.clone(),
            ));
        match self.load.run((instrument.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                match state
                    .records
                    .iter()
                    .rev()
                    .find(|(key, _)| key == instrument)
                {
                    None => Ok(None),
                    Some((_, value)) => match value.restore() {
                        Ok(thesis) => Ok(Some(thesis)),
                        Err(_error) => Err(SystemPortError::new(
                            application::SystemPort::Repository,
                            application::SystemFailure::InvalidData,
                        )),
                    },
                }
            }
        }
    }
}
#[derive(Debug, Default)]
struct EstimateRepositoryState {
    submissions: Vec<SignalSubmission>,
    resolutions: Vec<(SignalTaskKey, EstimateOutcome)>,
    calibrations: Vec<(CalibrationKey, Calibration)>,
}
#[derive(Debug, Default)]
pub struct MemoryEstimateRepository {
    append_submission: MemoryEstimateRepositoryAppendSubmissionScript,
    submissions: MemoryEstimateRepositorySubmissionsScript,
    append_resolution: MemoryEstimateRepositoryAppendResolutionScript,
    resolution: MemoryEstimateRepositoryResolutionScript,
    append_calibration: MemoryEstimateRepositoryAppendCalibrationScript,
    calibration: MemoryEstimateRepositoryCalibrationScript,
    state: State<EstimateRepositoryState>,
    calls: State<Vec<crate::scripted::EstimateRepositoryCall>>,
}
impl MemoryEstimateRepository {
    #[must_use]
    pub fn script_append_submission(&self) -> &MemoryEstimateRepositoryAppendSubmissionScript {
        &self.append_submission
    }
    #[must_use]
    pub fn script_submissions(&self) -> &MemoryEstimateRepositorySubmissionsScript {
        &self.submissions
    }
    #[must_use]
    pub fn script_resolution(&self) -> &MemoryEstimateRepositoryResolutionScript {
        &self.resolution
    }
    #[must_use]
    pub fn script_append_calibration(&self) -> &MemoryEstimateRepositoryAppendCalibrationScript {
        &self.append_calibration
    }
    #[must_use]
    pub fn script_calibration(&self) -> &MemoryEstimateRepositoryCalibrationScript {
        &self.calibration
    }

    #[must_use]
    pub fn script_append_resolution(&self) -> &MemoryEstimateRepositoryAppendResolutionScript {
        &self.append_resolution
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::EstimateRepositoryCall> {
        self.calls.write().clone()
    }
}
impl EstimateRepository for MemoryEstimateRepository {
    async fn append_submission(
        &self,
        submission: &SignalSubmission,
    ) -> Result<RecordOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::AppendSubmission(
                submission.clone(),
            ));
        match self.append_submission.run((submission.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                if state.submissions.contains(submission) {
                    Ok(RecordOutcome::AlreadyRecorded)
                } else {
                    state.submissions.push(submission.clone());
                    Ok(RecordOutcome::Recorded)
                }
            }
        }
    }
    async fn submissions(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<SignalSubmission>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::Submissions(
                instrument.clone(),
            ));
        match self.submissions.run((instrument.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .submissions
                    .iter()
                    .filter(|value| {
                        value.task_key().task().reference().instrument_id() == Some(instrument)
                    })
                    .cloned()
                    .collect())
            }
        }
    }
    async fn append_resolution(
        &self,
        task: &SignalTaskKey,
        outcome: &EstimateOutcome,
    ) -> Result<RecordOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::AppendResolution(
                task.clone(),
                outcome.clone(),
            ));
        match self
            .append_resolution
            .run((task.clone(), outcome.clone()))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                if state.resolutions.iter().any(|(key, _)| key == task) {
                    Ok(RecordOutcome::AlreadyRecorded)
                } else {
                    state.resolutions.push((task.clone(), outcome.clone()));
                    Ok(RecordOutcome::Recorded)
                }
            }
        }
    }
    async fn resolution(
        &self,
        task: &SignalTaskKey,
    ) -> Result<Option<EstimateOutcome>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::Resolution(
                task.clone(),
            ));
        match self.resolution.run((task.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .resolutions
                    .iter()
                    .find(|(key, _)| key == task)
                    .map(|(_, value)| value.clone()))
            }
        }
    }
    async fn append_calibration(
        &self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> Result<RecordOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::AppendCalibration(
                key.clone(),
                calibration.clone(),
            ));
        match self
            .append_calibration
            .run((key.clone(), calibration.clone()))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                if state
                    .calibrations
                    .iter()
                    .any(|(stored_key, stored)| stored_key == key && stored == calibration)
                {
                    Ok(RecordOutcome::AlreadyRecorded)
                } else {
                    state.calibrations.push((key.clone(), calibration.clone()));
                    Ok(RecordOutcome::Recorded)
                }
            }
        }
    }
    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<Calibration>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::EstimateRepositoryCall::Calibration(
                key.clone(),
            ));
        match self.calibration.run((key.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .calibrations
                    .iter()
                    .rev()
                    .find(|(stored_key, _)| stored_key == key)
                    .map(|(_, value)| value.clone()))
            }
        }
    }
}
#[derive(Debug, Default)]
struct ProposalRepositoryState {
    records: Vec<(ProposalId, Proposal<Draft>)>,
}
#[derive(Debug, Default)]
pub struct MemoryProposalRepository {
    store_draft: MemoryProposalRepositoryStoreDraftScript,
    draft: MemoryProposalRepositoryDraftScript,
    state: State<ProposalRepositoryState>,
    calls: State<Vec<crate::scripted::ProposalRepositoryCall>>,
}
impl MemoryProposalRepository {
    #[must_use]
    pub fn script_store_draft(&self) -> &MemoryProposalRepositoryStoreDraftScript {
        &self.store_draft
    }

    #[must_use]
    pub fn script_draft(&self) -> &MemoryProposalRepositoryDraftScript {
        &self.draft
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::ProposalRepositoryCall> {
        self.calls.write().clone()
    }
}
impl ProposalRepository for MemoryProposalRepository {
    async fn store_draft(
        &self,
        proposal: &Proposal<Draft>,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ProposalRepositoryCall::StoreDraft(
                Box::new(crate::DraftRecord::from_draft(proposal)),
            ));
        match self
            .store_draft
            .run((crate::DraftRecord::from_draft(proposal),))
            .await
        {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                state
                    .records
                    .push((proposal.id().clone(), proposal.clone()));
                Ok(WriteCompleted)
            }
        }
    }
    async fn draft(
        &self,
        proposal: &ProposalId,
    ) -> Result<Option<Proposal<Draft>>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ProposalRepositoryCall::Draft(
                proposal.clone(),
            ));
        match self.draft.run((proposal.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state
                    .records
                    .iter()
                    .rev()
                    .find(|(key, _)| key == proposal)
                    .map(|(_, value)| value.clone()))
            }
        }
    }
}
#[derive(Debug, Default)]
struct ValuationRepositoryState {
    records: Vec<(AccountId, ValuationSnapshot)>,
}
#[derive(Debug, Default)]
pub struct MemoryValuationRepository {
    append: MemoryValuationRepositoryAppendScript,
    history: MemoryValuationRepositoryHistoryScript,
    state: State<ValuationRepositoryState>,
    calls: State<Vec<crate::scripted::ValuationRepositoryCall>>,
}
impl MemoryValuationRepository {
    #[must_use]
    pub fn script_append(&self) -> &MemoryValuationRepositoryAppendScript {
        &self.append
    }

    #[must_use]
    pub fn script_history(&self) -> &MemoryValuationRepositoryHistoryScript {
        &self.history
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::ValuationRepositoryCall> {
        self.calls.write().clone()
    }
}
impl ValuationRepository for MemoryValuationRepository {
    async fn append(
        &self,
        account: &AccountId,
        snapshot: &ValuationSnapshot,
    ) -> Result<RecordOutcome, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ValuationRepositoryCall::Append(
                account.clone(),
                Box::new(snapshot.clone()),
            ));
        match self.append.run((account.clone(), snapshot.clone())).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                if state
                    .records
                    .iter()
                    .any(|(key, value)| key == account && value == snapshot)
                {
                    Ok(RecordOutcome::AlreadyRecorded)
                } else {
                    state.records.push((account.clone(), snapshot.clone()));
                    Ok(RecordOutcome::Recorded)
                }
            }
        }
    }
    async fn history(
        &self,
        account: &AccountId,
        window: TimeInterval,
    ) -> Result<Vec<ValuationSnapshot>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::ValuationRepositoryCall::History(
                account.clone(),
                window,
            ));
        match self.history.run((account.clone(), window)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                let mut values: Vec<_> = state
                    .records
                    .iter()
                    .filter(|(key, value)| {
                        key == account
                            && (window.start()..window.end()).contains(&value.valuation_at())
                    })
                    .map(|(_, value)| value.clone())
                    .collect();
                values.sort_by_key(ValuationSnapshot::valuation_at);
                Ok(values)
            }
        }
    }
}
impl application::ValuationCloseEvidenceRepository for MemoryMarketDataRepository {
    fn observed_close_evidence(
        &self,
        instrument: &domain::ResolvedOrderInstrument,
        date: domain::CalendarDate,
        as_of: Timestamp,
    ) -> impl Future<Output = Result<Vec<domain::ObservedCloseEvidence>, SystemPortError>> + Send
    {
        let state = self.state.write();
        let result = match state.close_evidence_error {
            Some(error) => Err(error),
            None => Ok(state
                .close_evidence
                .iter()
                .filter(|entry| {
                    entry.instrument().symbol() == instrument.symbol()
                        && domain::Market::from(*entry.instrument().market()) == instrument.market()
                        && entry.candle().time().date() == date
                        && entry.candle().collected_at() <= as_of
                })
                .cloned()
                .collect()),
        };
        std::future::ready(result)
    }
}
fn risk_with_profile(previous: Option<RiskSettings>, profile: &InvestorProfile) -> RiskSettings {
    RiskSettings::new(
        previous.map_or(domain::OrderPermission::Disabled, RiskSettings::orders),
        previous.map_or(
            domain::SavedMarketOrderSetting::Disabled,
            RiskSettings::market_orders,
        ),
        profile.hours_permission().state(),
        profile.weight_caps(),
        profile.risk_appetite(),
    )
}
fn profile_after_risk_write(
    previous: Option<&InvestorProfile>,
    settings: RiskSettings,
) -> Result<InvestorProfile, SystemPortError> {
    match previous {
        Some(profile) => profile_with_risk(profile, settings),
        None => profile_with_risk(&InvestorProfile::default(), settings),
    }
}
fn profile_with_risk(
    profile: &InvestorProfile,
    settings: RiskSettings,
) -> Result<InvestorProfile, SystemPortError> {
    let mut saved = profile.saved_state();
    saved.hours_permission = settings.hours();
    saved.risk_appetite = settings.risk_appetite();
    saved.instrument_cap = settings.weight_caps().instrument_cap().as_ratio();
    saved.instrument_warning = settings.weight_caps().instrument_warning().as_ratio();
    saved.sector_cap = settings.weight_caps().sector_cap().as_ratio();
    InvestorProfile::restore_from_saved_state(saved).map_err(|_error| {
        SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        )
    })
}
#[derive(Debug, Default)]
struct SettingsRepositoryState {
    profile: Option<InvestorProfile>,
    risk: Option<RiskSettings>,
}
#[derive(Debug, Default)]
pub struct MemorySettingsRepository {
    store_profile: MemorySettingsRepositoryStoreProfileScript,
    profile: MemorySettingsRepositoryProfileScript,
    store_risk_settings: MemorySettingsRepositoryStoreRiskSettingsScript,
    risk_settings: MemorySettingsRepositoryRiskSettingsScript,
    state: State<SettingsRepositoryState>,
    calls: State<Vec<crate::scripted::SettingsRepositoryCall>>,
}
impl MemorySettingsRepository {
    #[must_use]
    pub fn script_store_profile(&self) -> &MemorySettingsRepositoryStoreProfileScript {
        &self.store_profile
    }
    #[must_use]
    pub fn script_store_risk_settings(&self) -> &MemorySettingsRepositoryStoreRiskSettingsScript {
        &self.store_risk_settings
    }

    #[must_use]
    pub fn script_profile(&self) -> &MemorySettingsRepositoryProfileScript {
        &self.profile
    }
    #[must_use]
    pub fn script_risk_settings(&self) -> &MemorySettingsRepositoryRiskSettingsScript {
        &self.risk_settings
    }

    #[must_use]
    pub fn calls(&self) -> Vec<crate::scripted::SettingsRepositoryCall> {
        self.calls.write().clone()
    }
}
impl SettingsRepository for MemorySettingsRepository {
    async fn store_profile(
        &self,
        profile: &InvestorProfile,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::SettingsRepositoryCall::StoreProfile(
                profile.clone(),
            ));
        match self.store_profile.run((profile.clone(),)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                state.risk = Some(risk_with_profile(state.risk, profile));
                state.profile = Some(profile.clone());
                Ok(WriteCompleted)
            }
        }
    }
    async fn profile(&self) -> Result<Option<InvestorProfile>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::SettingsRepositoryCall::Profile());
        match self.profile.run(()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state.profile.clone())
            }
        }
    }
    async fn store_risk_settings(
        &self,
        settings: &RiskSettings,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::SettingsRepositoryCall::StoreRiskSettings(
                *settings,
            ));
        match self.store_risk_settings.run((*settings,)).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.state.write();
                let profile = profile_after_risk_write(state.profile.as_ref(), *settings)?;
                state.profile = Some(profile);
                state.risk = Some(*settings);
                Ok(WriteCompleted)
            }
        }
    }
    async fn risk_settings(&self) -> Result<Option<RiskSettings>, SystemPortError> {
        self.calls
            .write()
            .push(crate::scripted::SettingsRepositoryCall::RiskSettings());
        match self.risk_settings.run(()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let state = self.state.write();
                Ok(state.risk)
            }
        }
    }
}

pub type MemoryPortfolioRepositoryRecordScript = Script<
    (AccountId, SyncRecord<Portfolio>),
    MemoryReply<Result<WriteCompleted, SystemPortError>>,
>;
pub type MemoryTransactionRepositoryRecordScript = Script<
    (AccountId, SyncRecord<SettledRowSet>),
    MemoryReply<Result<WriteCompleted, SystemPortError>>,
>;
pub type MemoryTransactionRepositoryLatestScript =
    Script<(AccountId,), MemoryReply<Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>>>;
pub type MemoryMarketDataRepositoryQuoteScript =
    Script<(InstrumentId,), MemoryReply<Result<Option<Quote>, SystemPortError>>>;
pub type MemoryMarketDataRepositoryCandlesScript = Script<
    (InstrumentId, TimeInterval, CandlePeriod, PriceAdjustment),
    MemoryReply<Result<Vec<Candle>, SystemPortError>>,
>;
pub type MemoryMarketDataRepositoryStoreFxRateScript =
    Script<(FxRate,), MemoryReply<Result<CacheWriteOutcome, SystemPortError>>>;
pub type MemoryMarketDataRepositoryFxRateScript =
    Script<(FxPair, FxRateKind, FxRateQuery), MemoryReply<Result<Option<FxRate>, SystemPortError>>>;
pub type MemoryNewsRepositoryStoreScript =
    Script<(NewsItem,), MemoryReply<Result<WriteCompleted, SystemPortError>>>;
pub type MemoryThesisRepositoryStoreScript = Script<
    (InstrumentId, crate::ThesisRecord),
    MemoryReply<Result<WriteCompleted, SystemPortError>>,
>;
pub type MemoryThesisRepositoryLoadScript =
    Script<(InstrumentId,), MemoryReply<Result<Option<Thesis>, SystemPortError>>>;
pub type MemoryEstimateRepositoryAppendResolutionScript =
    Script<(SignalTaskKey, EstimateOutcome), MemoryReply<Result<RecordOutcome, SystemPortError>>>;
pub type MemoryProposalRepositoryDraftScript =
    Script<(ProposalId,), MemoryReply<Result<Option<Proposal<Draft>>, SystemPortError>>>;
pub type MemoryValuationRepositoryHistoryScript =
    Script<(AccountId, TimeInterval), MemoryReply<Result<Vec<ValuationSnapshot>, SystemPortError>>>;
pub type MemorySettingsRepositoryProfileScript =
    Script<(), MemoryReply<Result<Option<InvestorProfile>, SystemPortError>>>;
pub type MemorySettingsRepositoryRiskSettingsScript =
    Script<(), MemoryReply<Result<Option<RiskSettings>, SystemPortError>>>;

pub type MemoryPortfolioRepositoryLatestScript =
    Script<(AccountId,), MemoryReply<Result<Option<SyncRecord<Portfolio>>, SystemPortError>>>;
pub type MemoryMarketDataRepositoryStoreQuoteScript =
    Script<(InstrumentId, Quote), MemoryReply<Result<CacheWriteOutcome, SystemPortError>>>;
pub type MemoryMarketDataRepositoryStoreCandlesScript =
    Script<(InstrumentId, Vec<Candle>), MemoryReply<Result<WriteCompleted, SystemPortError>>>;
pub type MemoryNewsRepositoryForInstrumentScript =
    Script<(InstrumentId,), MemoryReply<Result<Vec<NewsItem>, SystemPortError>>>;
pub type MemoryEstimateRepositoryAppendSubmissionScript =
    Script<(SignalSubmission,), MemoryReply<Result<RecordOutcome, SystemPortError>>>;
pub type MemoryEstimateRepositorySubmissionsScript =
    Script<(InstrumentId,), MemoryReply<Result<Vec<SignalSubmission>, SystemPortError>>>;
pub type MemoryEstimateRepositoryResolutionScript =
    Script<(SignalTaskKey,), MemoryReply<Result<Option<EstimateOutcome>, SystemPortError>>>;
pub type MemoryEstimateRepositoryAppendCalibrationScript =
    Script<(CalibrationKey, Calibration), MemoryReply<Result<RecordOutcome, SystemPortError>>>;
pub type MemoryEstimateRepositoryCalibrationScript =
    Script<(CalibrationKey,), MemoryReply<Result<Option<Calibration>, SystemPortError>>>;
pub type MemoryProposalRepositoryStoreDraftScript =
    Script<(crate::DraftRecord,), MemoryReply<Result<WriteCompleted, SystemPortError>>>;
pub type MemoryValuationRepositoryAppendScript =
    Script<(AccountId, ValuationSnapshot), MemoryReply<Result<RecordOutcome, SystemPortError>>>;
pub type MemorySettingsRepositoryStoreProfileScript =
    Script<(InvestorProfile,), MemoryReply<Result<WriteCompleted, SystemPortError>>>;
pub type MemorySettingsRepositoryStoreRiskSettingsScript =
    Script<(RiskSettings,), MemoryReply<Result<WriteCompleted, SystemPortError>>>;

#[derive(Debug, Default)]
struct GlobalInstrumentIdState {
    issuer: domain::IndependentInstrumentIdIssuer,
    assigned_or_adopted: std::collections::BTreeSet<InstrumentId>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryGlobalInstrumentIds {
    state: std::sync::Arc<State<GlobalInstrumentIdState>>,
}

impl MemoryGlobalInstrumentIds {
    #[must_use]
    pub fn with_existing(existing: &[InstrumentId]) -> Self {
        let registry = Self::default();
        for id in existing {
            let _adopted = registry.adopt_existing(id.clone());
        }
        registry
    }

    #[must_use]
    pub fn adopt_existing(&self, id: InstrumentId) -> domain::IdAdoption {
        let mut state = self.state.write();
        let inserted = state.assigned_or_adopted.insert(id.clone());
        let issuer_adopted = state.issuer.adopt_existing(id);
        match (inserted, issuer_adopted) {
            (false, domain::IdAdoption::AlreadyKnown) => domain::IdAdoption::AlreadyKnown,
            (true, _) | (false, domain::IdAdoption::Adopted) => domain::IdAdoption::Adopted,
        }
    }

    #[must_use]
    pub fn reservation(&self, id: &InstrumentId) -> domain::ReservationStatus {
        let state = self.state.write();
        if state.assigned_or_adopted.contains(id) {
            domain::ReservationStatus::Reserved
        } else {
            state.issuer.reservation(id)
        }
    }

    #[must_use]
    pub fn issuer(&self) -> MemoryInstrumentIdIssuer {
        MemoryInstrumentIdIssuer {
            global: self.clone(),
        }
    }

    #[must_use]
    pub fn repository(&self) -> MemoryInstrumentMappingRepository {
        MemoryInstrumentMappingRepository {
            mappings: State::default(),
            global: self.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryInstrumentIdIssuer {
    global: MemoryGlobalInstrumentIds,
}

impl MemoryInstrumentIdIssuer {
    #[must_use]
    pub fn with_existing(existing: &[InstrumentId]) -> Self {
        Self {
            global: MemoryGlobalInstrumentIds::with_existing(existing),
        }
    }

    #[must_use]
    pub fn adopt_existing(&self, id: InstrumentId) -> domain::IdAdoption {
        self.global.adopt_existing(id)
    }

    #[must_use]
    pub fn reservation(&self, id: &InstrumentId) -> domain::ReservationStatus {
        self.global.reservation(id)
    }
}

impl application::InstrumentIdIssuer for MemoryInstrumentIdIssuer {
    fn issue(
        &self,
    ) -> impl Future<Output = Result<domain::IssuedInstrumentId, SystemPortError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(self.global.state.write().issuer.issue().map_err(|_error| {
                SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::RepositoryIntegrity,
                )
            }))
        })
    }
}

#[derive(Debug, Default)]
pub struct MemoryInstrumentMappingRepository {
    mappings: State<InstrumentMappings>,
    global: MemoryGlobalInstrumentIds,
}

impl MemoryInstrumentMappingRepository {
    #[must_use]
    pub fn with_existing_ids(existing: &[InstrumentId]) -> Self {
        let repo = Self::default();
        for id in existing {
            let _adopted = repo.adopt_existing_id(id.clone());
        }
        repo
    }

    #[must_use]
    pub fn adopt_existing_id(&self, id: InstrumentId) -> domain::IdAdoption {
        let mut mappings = self.mappings.write();
        let local = mappings.adopt_existing_id(id.clone());
        let global = self.global.adopt_existing(id);
        match (local, global) {
            (domain::IdAdoption::AlreadyKnown, domain::IdAdoption::AlreadyKnown) => {
                domain::IdAdoption::AlreadyKnown
            }
            (domain::IdAdoption::Adopted, _)
            | (domain::IdAdoption::AlreadyKnown, domain::IdAdoption::Adopted) => {
                domain::IdAdoption::Adopted
            }
        }
    }

    pub fn link_confirmed_change(
        &self,
        change: &domain::ConfirmedIdentifierChange,
    ) -> impl Future<Output = Result<InstrumentId, domain::MappingTransitionError>> + Send {
        poll_fn(move |_| Poll::Ready(self.mappings.write().link_confirmed_change(change)))
    }

    pub fn correct_retirement(
        &self,
        correction: &domain::ConfirmedRetirementCorrection,
    ) -> impl Future<Output = Result<InstrumentId, domain::MappingTransitionError>> + Send {
        poll_fn(move |_| Poll::Ready(self.mappings.write().correct_retirement(correction)))
    }

    pub fn correct_link(
        &self,
        correction: &domain::ConfirmedLinkCorrection,
    ) -> impl Future<Output = Result<InstrumentId, domain::MappingTransitionError>> + Send {
        poll_fn(move |_| Poll::Ready(self.mappings.write().correct_link(correction)))
    }

    pub fn history(&self) -> impl Future<Output = Vec<domain::MappingHistoryEvent>> + Send {
        poll_fn(move |_| Poll::Ready(self.mappings.write().history().to_vec()))
    }

    pub fn history_for_id(
        &self,
        id: &InstrumentId,
    ) -> impl Future<Output = Vec<domain::MappingHistoryEvent>> + Send {
        poll_fn(move |_| Poll::Ready(self.mappings.write().history_for_id(id)))
    }

    fn ensure_now(
        &self,
        source: &InstrumentSource,
        candidate: domain::IssuedInstrumentId,
    ) -> Result<MappingAssignment, MappingStoreError> {
        let mut mappings = self.mappings.write();
        match mappings.resolve(source) {
            Ok(id) => return Ok(MappingAssignment::Existing(id)),
            Err(domain::InstrumentMappingError::Missing) => {}
            Err(error) => return Err(MappingStoreError::Mapping(error)),
        }
        let mut global = self.global.state.write();
        if global
            .assigned_or_adopted
            .contains(candidate.as_instrument_id())
        {
            return Err(MappingStoreError::Mapping(
                domain::InstrumentMappingError::IdAlreadyUsed,
            ));
        }
        let assigned = mappings
            .ensure(source, candidate)
            .map_err(MappingStoreError::Mapping)?;
        if let MappingAssignment::Issued(issued_id) = &assigned {
            global.assigned_or_adopted.insert(issued_id.clone());
            let _adopted = global.issuer.adopt_existing(issued_id.clone());
        }
        Ok(assigned)
    }
}

impl InstrumentMappingRepository for MemoryInstrumentMappingRepository {
    fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .resolve(source)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }

    fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .lookup(source)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }

    fn for_id(
        &self,
        id: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .for_id(id)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }

    fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: domain::IssuedInstrumentId,
    ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
        let mut pending = Some(candidate);
        poll_fn(move |_| {
            let Some(issued) = pending.take() else {
                return Poll::Ready(Err(MappingStoreError::Storage(SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::RepositoryIntegrity,
                ))));
            };
            Poll::Ready(self.ensure_now(source, issued))
        })
    }

    fn retire(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .retire(source)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }
}
mod signal_repository;
pub use signal_repository::MemorySignalEvaluationRepository;
pub use signal_repository::MemorySignalEvaluationRepository as MemorySignalRepository;
