use crate::{RecordOutcome, SystemPortError, WriteCompleted};
use domain::{
    AccountId, Calibration, CalibrationKey, Candle, CandlePeriod, Draft, EstimateOutcome, FxPair,
    FxRate, FxRateKind, InstrumentId, InvestorProfile, NewsItem, Portfolio, PriceAdjustment,
    PriceObservation, Proposal, ProposalExecutionError, ProposalId, Quote, RiskSettings,
    SettledRowSet, SignalSubmission, SignalTaskKey, SyncRecord, Thesis, TimeInterval, Timestamp,
    ValuationSnapshot,
};
use std::future::Future;

pub trait PortfolioRepository: Send + Sync {
    fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<Portfolio>,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn latest(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<Option<SyncRecord<Portfolio>>, SystemPortError>> + Send;
}

pub trait TransactionRepository: Send + Sync {
    fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<SettledRowSet>,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn latest(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>> + Send;
}

pub trait MarketDataRepository: Send + Sync {
    fn store_quote(
        &self,
        instrument: &InstrumentId,
        quote: &Quote,
    ) -> impl Future<Output = Result<CacheWriteOutcome, SystemPortError>> + Send;
    fn quote(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Quote>, SystemPortError>> + Send;
    fn store_candles(
        &self,
        instrument: &InstrumentId,
        candles: &[Candle],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn candles(
        &self,
        instrument: &InstrumentId,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, SystemPortError>> + Send;
    fn store_fx_rate(
        &self,
        rate: &FxRate,
    ) -> impl Future<Output = Result<CacheWriteOutcome, SystemPortError>> + Send;
    fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: FxRateQuery,
    ) -> impl Future<Output = Result<Option<FxRate>, SystemPortError>> + Send;
}

pub trait ValuationCloseEvidenceRepository: Send + Sync {
    fn observed_close_evidence(
        &self,
        _instrument: &domain::ResolvedOrderInstrument,
        _date: domain::CalendarDate,
        _as_of: Timestamp,
    ) -> impl Future<Output = Result<Vec<domain::ObservedCloseEvidence>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheWriteOutcome {
    Changed,
    Unchanged,
    OlderIgnored,
}

pub trait NewsRepository: Send + Sync {
    fn store(
        &self,
        item: &NewsItem,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send;
}

pub trait ThesisRepository: Send + Sync {
    fn store(
        &self,
        instrument: &InstrumentId,
        thesis: &Thesis,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn load(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Thesis>, SystemPortError>> + Send;
}

pub trait EstimateRepository: Send + Sync {
    fn append_submission(
        &self,
        submission: &SignalSubmission,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
    fn submissions(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<SignalSubmission>, SystemPortError>> + Send;
    fn append_resolution(
        &self,
        task: &SignalTaskKey,
        outcome: &EstimateOutcome,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
    fn resolution(
        &self,
        task: &SignalTaskKey,
    ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send;
    fn append_calibration(
        &self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
    fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> impl Future<Output = Result<Option<Calibration>, SystemPortError>> + Send;
}

pub trait ProposalRepository: Send + Sync {
    fn store_draft(
        &self,
        proposal: &Proposal<Draft>,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn draft(
        &self,
        proposal: &ProposalId,
    ) -> impl Future<Output = Result<Option<Proposal<Draft>>, SystemPortError>> + Send;
    fn current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &crate::SignalSnapshot,
        check: DraftCheck<'_>,
    ) -> impl Future<Output = Result<CurrentDraftOutcome, SystemPortError>> + Send;
    fn store_current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &crate::SignalSnapshot,
        proposal: &Proposal<Draft>,
        check: DraftCheck<'_>,
    ) -> impl Future<Output = Result<ConditionalDraftOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy)]
pub struct DraftCheck<'a> {
    pub account: &'a AccountId,
    pub at: Timestamp,
    pub observation: &'a PriceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentDraftOutcome {
    Absent,
    Existing(ProposalId),
    Conflict,
    AccountChanged,
    Invalid(ProposalExecutionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionalDraftOutcome {
    Stored,
    Existing(ProposalId),
    Conflict,
    AccountChanged,
    Invalid(ProposalExecutionError),
}

pub trait ValuationRepository: Send + Sync {
    fn append(
        &self,
        account: &AccountId,
        snapshot: &ValuationSnapshot,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
    fn history(
        &self,
        account: &AccountId,
        window: TimeInterval,
    ) -> impl Future<Output = Result<Vec<ValuationSnapshot>, SystemPortError>> + Send;
}

pub trait SettingsRepository: Send + Sync {
    fn store_profile(
        &self,
        profile: &InvestorProfile,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<InvestorProfile>, SystemPortError>> + Send;
    fn store_risk_settings(
        &self,
        settings: &RiskSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn risk_settings(
        &self,
    ) -> impl Future<Output = Result<Option<RiskSettings>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxRateQuery {
    Latest,
    At(Timestamp),
}

pub use crate::signal_ports::SignalRepository;
