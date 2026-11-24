pub mod instrument_mapping;
pub mod instrument_mapping_correction;
pub mod scenarios;
use application::{
    AuditEntry, AuditIntegrity, AuditLog, BrokerRead, CacheWriteOutcome, CollectedObservation,
    DataAvailability, EstimateRepository, FeedCursor, FeedSource, FinancialStatement, Fundamentals,
    FxRateQuery, HoldingsObservation, InformationError, MacroObservation, MacroSeries,
    MacroSeriesId, MarketDataRepository, MarketInfo, NewsFeed, NewsMetadata, NewsRepository,
    Notification, Notifier, PortError, PortfolioRepository, QuoteObservation, RankingQuery,
    RecordOutcome, SectorRankingQuery, SettingsRepository, SignalCommitOutcome, SignalMutation,
    SignalRepository, SignalSnapshot, SystemPortError, ThesisRepository, TransactionRead,
    TransactionRepository, ValuationRepository, WriteCompleted,
};
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Calibration, CalibrationKey, Candle, CandlePeriod, CommissionRateFact,
    CreditTrade, Currency, EstimateOutcome, FlowInterval, FxPair, FxRate, FxRateKind,
    IndicatorCode, InstrumentId, InstrumentMaster, InvestorFlow, InvestorProfile,
    InvestorTradingRankings, KnownMarket, MarketCalendar, MarketIndicatorCandle,
    MarketIndicatorQuote, NewsItem, OrderFxRates, Orderbook, Portfolio, PriceAdjustment,
    PriceLimits, ProgramTrade, ProvenClosedOrderSet, Quote, QuoteInstrument, RaisedSignal,
    RankingLimit, ResolvedOrderInstrument, RiskSettings, SectorCatalog, SectorComposition,
    SectorDetail, SectorEtfs, SectorId, SectorRanking, SecuritiesLending, SellableQuantityFact,
    SettledRowSet, SettlementWindow, ShortSelling, SignalSubmission, SignalTaskKey, StockRanking,
    StockWarnings, Symbol, SyncRecord, Thesis, TimeInterval, Timestamp, TransactionFilter,
    ValuationSnapshot,
};
pub async fn broker_read_accounts<P: BrokerRead>(
    port: &P,
    expected: Result<BrokerAccountRows, PortError>,
) {
    assert_eq!(
        port.accounts().await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_holdings<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    expected: Result<HoldingsObservation, PortError>,
) {
    assert_eq!(
        port.holdings(account).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_transactions<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    window: SettlementWindow,
    filter: &TransactionFilter,
    expected: Result<TransactionRead, PortError>,
) {
    assert_eq!(
        port.transactions(account, window, filter).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_quotes<P: BrokerRead>(
    port: &P,
    instruments: &[QuoteInstrument],
    expected: Result<Vec<QuoteObservation>, PortError>,
) {
    assert_eq!(
        port.quotes(instruments).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_candles<P: BrokerRead>(
    port: &P,
    instrument: &QuoteInstrument,
    query: (TimeInterval, CandlePeriod, PriceAdjustment),
    expected: Result<Vec<Candle>, PortError>,
) {
    let (window, period, adjustment) = query;
    assert_eq!(
        port.candles(instrument, window, period, adjustment).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_buying_power<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    currency: Currency,
    expected: Result<BuyingPower, PortError>,
) {
    assert_eq!(
        port.buying_power(account, currency).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_sellable_quantity<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    instrument: &ResolvedOrderInstrument,
    expected: Result<SellableQuantityFact, PortError>,
) {
    assert_eq!(
        port.sellable_quantity(account, instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_open_orders<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    expected: Result<BrokerOrderSet, PortError>,
) {
    assert_eq!(
        port.open_orders(account).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_closed_orders<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    evaluation_window: TimeInterval,
    expected: Result<ProvenClosedOrderSet, PortError>,
) {
    assert_eq!(
        port.closed_orders(account, evaluation_window).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_order<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    order_id: &BrokerOrderId,
    expected: Result<BrokerOrder, PortError>,
) {
    assert_eq!(
        port.order(account, order_id).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_stock_warnings<P: BrokerRead>(
    port: &P,
    instrument: &ResolvedOrderInstrument,
    expected: Result<StockWarnings, PortError>,
) {
    assert_eq!(
        port.stock_warnings(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_instruments<P: BrokerRead>(
    port: &P,
    market: KnownMarket,
    expected: Result<InstrumentMaster, PortError>,
) {
    assert_eq!(
        port.instruments(market).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_orderbook<P: BrokerRead>(
    port: &P,
    instrument: &QuoteInstrument,
    expected: Result<Orderbook, PortError>,
) {
    assert_eq!(
        port.orderbook(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_price_limits<P: BrokerRead>(
    port: &P,
    instrument: &QuoteInstrument,
    expected: Result<PriceLimits, PortError>,
) {
    assert_eq!(
        port.price_limits(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn broker_read_commission_rate<P: BrokerRead>(
    port: &P,
    account: &AccountId,
    market: KnownMarket,
    expected: Result<CommissionRateFact, PortError>,
) {
    assert_eq!(
        port.commission_rate(account, market).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_fx_rate<P: MarketInfo>(
    port: &P,
    pair: FxPair,
    kind: FxRateKind,
    at: Option<Timestamp>,
    expected: Result<FxRate, PortError>,
) {
    assert_eq!(
        port.fx_rate(pair, kind, at).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_order_fx_rates<P: MarketInfo>(
    port: &P,
    expected: Result<OrderFxRates, PortError>,
) {
    assert_eq!(
        port.order_fx_rates().await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_indicator_quotes<P: MarketInfo>(
    port: &P,
    codes: &[IndicatorCode],
    expected: Result<Vec<MarketIndicatorQuote>, PortError>,
) {
    assert_eq!(
        port.indicator_quotes(codes).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_indicator_candles<P: MarketInfo>(
    port: &P,
    code: IndicatorCode,
    window: TimeInterval,
    period: CandlePeriod,
    expected: Result<Vec<MarketIndicatorCandle>, PortError>,
) {
    assert_eq!(
        port.indicator_candles(code, window, period).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_rankings<P: MarketInfo>(
    port: &P,
    query: &RankingQuery,
    expected: Result<StockRanking, PortError>,
) {
    assert_eq!(
        port.rankings(query).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_stock_investor_flow<P: MarketInfo>(
    port: &P,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<Vec<InvestorFlow>, PortError>,
) {
    assert_eq!(
        port.stock_investor_flow(instrument, from, through).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_indicator_investor_flow<P: MarketInfo>(
    port: &P,
    code: IndicatorCode,
    dates: (CalendarDate, CalendarDate),
    interval: FlowInterval,
    expected: Result<Vec<InvestorFlow>, PortError>,
) {
    let (from, through) = dates;
    assert_eq!(
        port.indicator_investor_flow(code, from, through, interval)
            .await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_sector_composition<P: MarketInfo>(
    port: &P,
    sector: &SectorId,
    market: KnownMarket,
    expected: Result<SectorComposition, PortError>,
) {
    assert_eq!(
        port.sector_composition(sector, market).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_info_calendar<P: MarketInfo>(
    port: &P,
    market: KnownMarket,
    expected: Result<MarketCalendar, PortError>,
) {
    assert_eq!(
        port.calendar(market).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub fn news_feed_source<P: NewsFeed>(port: &P, expected: &FeedSource) {
    assert_eq!(
        &port.source(),
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub fn news_feed_metadata<P: NewsFeed>(port: &P, expected: &NewsMetadata) {
    assert_eq!(
        &port.metadata(),
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn news_feed_new_items<P: NewsFeed>(
    port: &P,
    since: FeedCursor,
    expected: Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError>,
) {
    assert_eq!(
        port.new_items(since).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn fundamentals_statements<P: Fundamentals>(
    port: &P,
    symbol: &Symbol,
    period_end: CalendarDate,
    expected: Result<DataAvailability<Vec<FinancialStatement>>, InformationError>,
) {
    assert_eq!(
        port.statements(symbol, period_end).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn macro_series_daily_values<P: MacroSeries>(
    port: &P,
    series: &MacroSeriesId,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<DataAvailability<Vec<MacroObservation>>, InformationError>,
) {
    assert_eq!(
        port.daily_values(series, from, through).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn portfolio_repository_record<P: PortfolioRepository>(
    port: &P,
    account: &AccountId,
    record: &SyncRecord<Portfolio>,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.record(account, record).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn portfolio_repository_latest<P: PortfolioRepository>(
    port: &P,
    account: &AccountId,
    expected: Result<Option<SyncRecord<Portfolio>>, SystemPortError>,
) {
    assert_eq!(
        port.latest(account).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn transaction_repository_record<P: TransactionRepository>(
    port: &P,
    account: &AccountId,
    record: &SyncRecord<SettledRowSet>,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.record(account, record).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn transaction_repository_latest<P: TransactionRepository>(
    port: &P,
    account: &AccountId,
    expected: Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>,
) {
    assert_eq!(
        port.latest(account).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_store_quote<P: MarketDataRepository>(
    port: &P,
    instrument: &InstrumentId,
    quote: &Quote,
    expected: Result<CacheWriteOutcome, SystemPortError>,
) {
    assert_eq!(
        port.store_quote(instrument, quote).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_quote<P: MarketDataRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected: Result<Option<Quote>, SystemPortError>,
) {
    assert_eq!(
        port.quote(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_store_candles<P: MarketDataRepository>(
    port: &P,
    instrument: &InstrumentId,
    candles: &[Candle],
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store_candles(instrument, candles).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_candles<P: MarketDataRepository>(
    port: &P,
    instrument: &InstrumentId,
    query: (TimeInterval, CandlePeriod, PriceAdjustment),
    expected: Result<Vec<Candle>, SystemPortError>,
) {
    let (window, period, adjustment) = query;
    assert_eq!(
        port.candles(instrument, window, period, adjustment).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_store_fx_rate<P: MarketDataRepository>(
    port: &P,
    rate: &FxRate,
    expected: Result<CacheWriteOutcome, SystemPortError>,
) {
    assert_eq!(
        port.store_fx_rate(rate).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn market_data_repository_fx_rate<P: MarketDataRepository>(
    port: &P,
    pair: FxPair,
    kind: FxRateKind,
    at: FxRateQuery,
    expected: Result<Option<FxRate>, SystemPortError>,
) {
    assert_eq!(
        port.fx_rate(pair, kind, at).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn news_repository_store<P: NewsRepository>(
    port: &P,
    item: &NewsItem,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store(item).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn news_repository_for_instrument<P: NewsRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected: Result<Vec<NewsItem>, SystemPortError>,
) {
    assert_eq!(
        port.for_instrument(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn thesis_repository_store<P: ThesisRepository>(
    port: &P,
    instrument: &InstrumentId,
    thesis: &Thesis,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store(instrument, thesis).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn thesis_repository_load<P: ThesisRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected: Result<Option<Thesis>, SystemPortError>,
) {
    assert_eq!(
        port.load(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_append_submission<P: EstimateRepository>(
    port: &P,
    submission: &SignalSubmission,
    expected: Result<RecordOutcome, SystemPortError>,
) {
    assert_eq!(
        port.append_submission(submission).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_submissions<P: EstimateRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected: Result<Vec<SignalSubmission>, SystemPortError>,
) {
    assert_eq!(
        port.submissions(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_append_resolution<P: EstimateRepository>(
    port: &P,
    task: &SignalTaskKey,
    outcome: &EstimateOutcome,
    expected: Result<RecordOutcome, SystemPortError>,
) {
    assert_eq!(
        port.append_resolution(task, outcome).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_resolution<P: EstimateRepository>(
    port: &P,
    task: &SignalTaskKey,
    expected: Result<Option<EstimateOutcome>, SystemPortError>,
) {
    assert_eq!(
        port.resolution(task).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_append_calibration<P: EstimateRepository>(
    port: &P,
    key: &CalibrationKey,
    calibration: &Calibration,
    expected: Result<RecordOutcome, SystemPortError>,
) {
    assert_eq!(
        port.append_calibration(key, calibration).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn estimate_repository_calibration<P: EstimateRepository>(
    port: &P,
    key: &CalibrationKey,
    expected: Result<Option<Calibration>, SystemPortError>,
) {
    assert_eq!(
        port.calibration(key).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn signal_repository_commit<P: SignalRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected_state: &SignalSnapshot,
    mutation: &SignalMutation,
    expected: Result<SignalCommitOutcome, SystemPortError>,
) {
    assert_eq!(
        port.commit(instrument, expected_state, mutation).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn signal_repository_current<P: SignalRepository>(
    port: &P,
    instrument: &InstrumentId,
    expected_current: Result<Vec<RaisedSignal>, SystemPortError>,
) {
    let expected = expected_current.map(|signals| {
        signals
            .iter()
            .map(domain::HistoricalSignal::from_signal)
            .collect()
    });
    assert_eq!(
        port.current(instrument).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn valuation_repository_append<P: ValuationRepository>(
    port: &P,
    account: &AccountId,
    snapshot: &ValuationSnapshot,
    expected: Result<RecordOutcome, SystemPortError>,
) {
    assert_eq!(
        port.append(account, snapshot).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn valuation_repository_history<P: ValuationRepository>(
    port: &P,
    account: &AccountId,
    window: TimeInterval,
    expected: Result<Vec<ValuationSnapshot>, SystemPortError>,
) {
    assert_eq!(
        port.history(account, window).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn settings_repository_store_profile<P: SettingsRepository>(
    port: &P,
    profile: &InvestorProfile,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store_profile(profile).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn settings_repository_profile<P: SettingsRepository>(
    port: &P,
    expected: Result<Option<InvestorProfile>, SystemPortError>,
) {
    assert_eq!(
        port.profile().await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn settings_repository_store_risk_settings<P: SettingsRepository>(
    port: &P,
    settings: &RiskSettings,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store_risk_settings(settings).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn settings_repository_risk_settings<P: SettingsRepository>(
    port: &P,
    expected: Result<Option<RiskSettings>, SystemPortError>,
) {
    assert_eq!(
        port.risk_settings().await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn notifier_notify<P: Notifier>(
    port: &P,
    notification: Notification,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.notify(notification).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn audit_log_append<P: AuditLog>(
    port: &P,
    entry: &AuditEntry,
    expected: Result<RecordOutcome, SystemPortError>,
) {
    assert_eq!(
        port.append(entry).await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub async fn audit_log_verify<P: AuditLog>(
    port: &P,
    expected: Result<AuditIntegrity, SystemPortError>,
) {
    assert_eq!(
        port.verify().await,
        expected,
        "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존"
    );
}
pub fn clock_now<P: application::Clock>(port: &P, expected: Timestamp) {
    assert_eq!(port.now(), expected, "시계는 지정된 시각을 동기로 반환");
}
pub async fn user_presence_order<P: application::user_presence::UserPresence>(
    port: &P,
    summary: &domain::OrderSummary,
    expected: application::OrderConfirmation,
) {
    assert_eq!(
        port.confirm_order(summary).await,
        expected,
        "주문 확인의 닫힌 결과 보존"
    );
}
pub async fn user_presence_risk_change<P: application::user_presence::UserPresence>(
    port: &P,
    change: domain::RiskChange,
    expected: application::RiskChangeConfirmation,
) {
    assert_eq!(
        port.confirm_risk_change(change).await,
        expected,
        "위험 설정 확인의 별도 결과 보존"
    );
}
pub async fn proposal_repository_store_draft<P: application::ProposalRepository>(
    port: &P,
    proposal: &domain::Proposal<domain::Draft>,
    evidence: &domain::DraftRestoreEvidence,
    master: &domain::MasterInstrument,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store_draft(proposal, evidence, master).await,
        expected,
        "초안 저장 결과 보존"
    );
}
pub async fn proposal_repository_draft<P: application::ProposalRepository>(
    port: &P,
    id: &domain::ProposalId,
    master: &domain::MasterInstrument,
    expected: Result<Option<crate::DraftRecord>, SystemPortError>,
) {
    assert_eq!(
        port.draft(id, master)
            .await
            .map(|value| value.as_ref().map(crate::DraftRecord::from_draft)),
        expected,
        "조회는 승인 권한 없는 초안 사실을 보존"
    );
}
pub async fn secret_store_load<P: application::SecretStore>(
    port: &P,
    key: application::SecretKey,
    expected: Result<Option<application::SecretValue>, SystemPortError>,
) {
    let actual = port
        .load(key)
        .await
        .map(|value| value.map(|secret| secret.as_exposed_secret().to_vec()));
    let expected_bytes =
        expected.map(|value| value.map(|secret| secret.as_exposed_secret().to_vec()));
    assert_eq!(
        actual, expected_bytes,
        "비밀 조회는 부재와 실패를 구분하고 값을 보존"
    );
}
pub async fn secret_store_store<P: application::SecretStore>(
    port: &P,
    key: application::SecretKey,
    value: application::SecretValue,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(
        port.store(key, value).await,
        expected,
        "비밀 저장 결과 보존"
    );
}
pub async fn secret_store_remove<P: application::SecretStore>(
    port: &P,
    key: application::SecretKey,
    expected: Result<WriteCompleted, SystemPortError>,
) {
    assert_eq!(port.remove(key).await, expected, "비밀 삭제는 부재도 성공");
}
pub fn broker_capabilities(
    port: &application::BrokerCapabilities,
    capability: application::BrokerCapability,
    expected: application::CapabilitySupport,
) {
    assert_eq!(
        port.support_for(capability),
        expected,
        "지원 표는 등록한 기능만 지원"
    );
}
pub async fn market_info_investor_trading_rankings<P: MarketInfo>(
    port: &P,
    limit: RankingLimit,
    expected: Result<InvestorTradingRankings, PortError>,
) {
    assert_eq!(
        port.investor_trading_rankings(limit).await,
        expected,
        "ADR 0087 입력에 대응하는 사실과 오류 보존"
    );
}
pub async fn market_info_sectors<P: MarketInfo>(
    port: &P,
    expected: Result<SectorCatalog, PortError>,
) {
    assert_eq!(
        port.sectors().await,
        expected,
        "ADR 0087 입력에 대응하는 사실과 오류 보존"
    );
}
pub async fn market_info_sector<P: MarketInfo>(
    port: &P,
    sector: &SectorId,
    market: KnownMarket,
    expected: Result<SectorDetail, PortError>,
) {
    assert_eq!(
        port.sector(sector, market).await,
        expected,
        "ADR 0087 입력에 대응하는 사실과 오류 보존"
    );
}
pub async fn market_info_sector_rankings<P: MarketInfo>(
    port: &P,
    query: &SectorRankingQuery,
    expected: Result<SectorRanking, PortError>,
) {
    assert_eq!(
        port.sector_rankings(query).await,
        expected,
        "ADR 0087 입력에 대응하는 사실과 오류 보존"
    );
}
pub async fn market_info_sector_etfs<P: MarketInfo>(
    port: &P,
    sector: &SectorId,
    market: KnownMarket,
    expected: Result<SectorEtfs, PortError>,
) {
    assert_eq!(
        port.sector_etfs(sector, market).await,
        expected,
        "ADR 0087 입력에 대응하는 사실과 오류 보존"
    );
}

pub async fn market_info_program_trades<P: MarketInfo>(
    port: &P,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<Vec<ProgramTrade>, PortError>,
) {
    assert_eq!(
        port.program_trades(instrument, from, through).await,
        expected,
        "ADR 0090 사실·부재·오류와 요청 의미 보존"
    );
}

pub async fn market_info_short_selling<P: MarketInfo>(
    port: &P,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<Vec<ShortSelling>, PortError>,
) {
    assert_eq!(
        port.short_selling(instrument, from, through).await,
        expected,
        "ADR 0090 사실·부재·오류와 요청 의미 보존"
    );
}

pub async fn market_info_credit_trades<P: MarketInfo>(
    port: &P,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<Vec<CreditTrade>, PortError>,
) {
    assert_eq!(
        port.credit_trades(instrument, from, through).await,
        expected,
        "ADR 0090 사실·부재·오류와 요청 의미 보존"
    );
}

pub async fn market_info_securities_lending<P: MarketInfo>(
    port: &P,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
    expected: Result<Vec<SecuritiesLending>, PortError>,
) {
    assert_eq!(
        port.securities_lending(instrument, from, through).await,
        expected,
        "ADR 0090 사실·부재·오류와 요청 의미 보존"
    );
}
