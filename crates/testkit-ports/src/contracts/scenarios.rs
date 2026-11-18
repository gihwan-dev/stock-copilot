use application::{
    AuditEntry, AuditLog, CacheWriteOutcome, EstimateRepository, FxRateQuery, MarketDataRepository,
    NewsRepository, PortfolioRepository, ProposalRepository, RecordOutcome, SecretKey, SecretStore,
    SecretValue, SettingsRepository, SignalRepository, ThesisRepository, TransactionRepository,
    ValuationRepository, WriteCompleted,
};
use domain::{
    AccountId, Calibration, CalibrationKey, Candle, Draft, EstimateOutcome, FxRate, InstrumentId,
    InvestorProfile, NewsItem, Portfolio, Proposal, Quote, RaisedSignal, RiskSettings,
    SettledRowSet, SignalSubmission, SyncRecord, Thesis, TimeInterval, Timestamp,
    ValuationSnapshot,
};

pub async fn portfolio_latest<P: PortfolioRepository>(
    port: &P,
    accounts: (&AccountId, &AccountId),
    records: (&SyncRecord<Portfolio>, &SyncRecord<Portfolio>),
) {
    let (account, other) = accounts;
    let (older, newer) = records;
    assert_ne!(
        account, other,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert!(
        older.recorded_at() < newer.recorded_at(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    super::portfolio_repository_latest(port, account, Ok(None)).await;
    for record in [newer, older] {
        super::portfolio_repository_record(port, account, record, Ok(WriteCompleted)).await;
    }
    super::portfolio_repository_record(port, other, older, Ok(WriteCompleted)).await;
    super::portfolio_repository_latest(port, account, Ok(Some(newer.clone()))).await;
    super::portfolio_repository_latest(port, other, Ok(Some(older.clone()))).await;
}

pub async fn transaction_latest<P: TransactionRepository>(
    port: &P,
    accounts: (&AccountId, &AccountId),
    records: (&SyncRecord<SettledRowSet>, &SyncRecord<SettledRowSet>),
) {
    let (account, other) = accounts;
    let (older, newer) = records;
    assert_ne!(
        account, other,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert!(
        older.recorded_at() < newer.recorded_at(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    super::transaction_repository_latest(port, account, Ok(None)).await;
    for record in [newer, older] {
        super::transaction_repository_record(port, account, record, Ok(WriteCompleted)).await;
    }
    super::transaction_repository_record(port, other, older, Ok(WriteCompleted)).await;
    super::transaction_repository_latest(port, account, Ok(Some(newer.clone()))).await;
    super::transaction_repository_latest(port, other, Ok(Some(older.clone()))).await;
}

pub async fn signal_replacement<P: SignalRepository>(
    port: &P,
    instruments: (&InstrumentId, &InstrumentId),
    versions: (&[RaisedSignal], &[RaisedSignal]),
) {
    let (instrument, other) = instruments;
    let (first, second) = versions;
    assert_ne!(
        instrument, other,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_ne!(first, [], "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    assert_ne!(second, [], "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    assert_ne!(first, second, "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    super::signal_repository_current(port, instrument, Ok(vec![])).await;
    super::signal_repository_replace(port, instrument, first, Ok(WriteCompleted)).await;
    super::signal_repository_current(port, instrument, Ok(first.to_vec())).await;
    super::signal_repository_replace(port, other, first, Ok(WriteCompleted)).await;
    super::signal_repository_replace(port, instrument, second, Ok(WriteCompleted)).await;
    super::signal_repository_current(port, instrument, Ok(second.to_vec())).await;
    super::signal_repository_replace(port, instrument, &[], Ok(WriteCompleted)).await;
    super::signal_repository_current(port, instrument, Ok(vec![])).await;
    super::signal_repository_current(port, other, Ok(first.to_vec())).await;
}

pub async fn thesis_roundtrip<P: ThesisRepository>(
    port: &P,
    instruments: (&InstrumentId, &InstrumentId),
    versions: (&Thesis, &Thesis),
) {
    let (instrument, other) = instruments;
    let (first, second) = versions;
    assert_ne!(
        instrument, other,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_ne!(
        first.history(),
        [],
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_ne!(first, second, "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    super::thesis_repository_load(port, instrument, Ok(None)).await;
    super::thesis_repository_store(port, instrument, first, Ok(WriteCompleted)).await;
    assert_eq!(
        port.load(instrument)
            .await
            .as_ref()
            .map(|value| value.as_ref()),
        Ok(Some(first)),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    super::thesis_repository_store(port, other, first, Ok(WriteCompleted)).await;
    super::thesis_repository_store(port, instrument, second, Ok(WriteCompleted)).await;
    assert_eq!(
        port.load(instrument)
            .await
            .as_ref()
            .map(|value| value.as_ref()),
        Ok(Some(second)),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_eq!(
        port.load(other).await.as_ref().map(|value| value.as_ref()),
        Ok(Some(first)),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
}

pub async fn proposal_roundtrip<P: ProposalRepository>(
    port: &P,
    versions: (&Proposal<Draft>, &Proposal<Draft>),
    other: &Proposal<Draft>,
) {
    let (first, second) = versions;
    assert_eq!(first.id(), second.id(), "같은 제안의 다른 저장 사실");
    assert_ne!(first.id(), other.id(), "서로 다른 제안 키 증인");
    assert_ne!(
        crate::DraftRecord::from_draft(first),
        crate::DraftRecord::from_draft(second),
        "가격 범위 등 다른 사실 증인"
    );
    super::proposal_repository_draft(port, first.id(), Ok(None)).await;
    super::proposal_repository_store_draft(port, first, Ok(WriteCompleted)).await;
    super::proposal_repository_draft(
        port,
        first.id(),
        Ok(Some(crate::DraftRecord::from_draft(first))),
    )
    .await;
    super::proposal_repository_store_draft(port, second, Ok(WriteCompleted)).await;
    super::proposal_repository_store_draft(port, other, Ok(WriteCompleted)).await;
    super::proposal_repository_draft(
        port,
        first.id(),
        Ok(Some(crate::DraftRecord::from_draft(second))),
    )
    .await;
    super::proposal_repository_draft(
        port,
        other.id(),
        Ok(Some(crate::DraftRecord::from_draft(other))),
    )
    .await;
}

pub async fn estimate_append_only<P: EstimateRepository>(
    port: &P,
    submission: &SignalSubmission,
    outcomes: (&EstimateOutcome, &EstimateOutcome),
    other: &SignalSubmission,
) {
    let (first, second) = outcomes;
    let instrument = submission.task_key().task().volatility().instrument();
    let other_instrument = other.task_key().task().volatility().instrument();
    let key = submission.task_key();
    let other_key = other.task_key();
    assert_ne!(first, second, "서로 다른 판정 증인");
    assert_ne!(key, other_key, "서로 다른 과제 키 증인");
    assert_ne!(instrument, other_instrument, "서로 다른 종목 증인");
    super::estimate_repository_submissions(port, instrument, Ok(vec![])).await;
    super::estimate_repository_resolution(port, key, Ok(None)).await;
    for (value, expected) in [
        (submission, RecordOutcome::Recorded),
        (submission, RecordOutcome::AlreadyRecorded),
        (other, RecordOutcome::Recorded),
    ] {
        super::estimate_repository_append_submission(port, value, Ok(expected)).await;
    }
    super::estimate_repository_submissions(port, instrument, Ok(vec![submission.clone()])).await;
    super::estimate_repository_submissions(port, other_instrument, Ok(vec![other.clone()])).await;
    for (task, outcome, expected) in [
        (key, first, RecordOutcome::Recorded),
        (key, second, RecordOutcome::AlreadyRecorded),
        (other_key, second, RecordOutcome::Recorded),
    ] {
        super::estimate_repository_append_resolution(port, task, outcome, Ok(expected)).await;
    }
    super::estimate_repository_resolution(port, key, Ok(Some(first.clone()))).await;
    super::estimate_repository_resolution(port, other_key, Ok(Some(second.clone()))).await;
}

pub async fn calibration_versions<P: EstimateRepository>(
    port: &P,
    key: &CalibrationKey,
    versions: (&Calibration, &Calibration),
    other: (&CalibrationKey, &Calibration),
) {
    let (first, second) = versions;
    assert_ne!(first, second, "서로 다른 보정 버전 증인");
    assert_ne!(key, other.0, "서로 다른 보정 키 증인");
    super::estimate_repository_calibration(port, key, Ok(None)).await;
    for (value, expected) in [
        (first, RecordOutcome::Recorded),
        (first, RecordOutcome::AlreadyRecorded),
        (second, RecordOutcome::Recorded),
    ] {
        super::estimate_repository_append_calibration(port, key, value, Ok(expected)).await;
    }
    super::estimate_repository_append_calibration(
        port,
        other.0,
        other.1,
        Ok(RecordOutcome::Recorded),
    )
    .await;
    super::estimate_repository_calibration(port, key, Ok(Some(second.clone()))).await;
    super::estimate_repository_calibration(port, other.0, Ok(Some(other.1.clone()))).await;
}

#[derive(Debug, Clone, Copy)]
pub struct CandleHistoryFacts<'facts> {
    pub instrument: &'facts InstrumentId,
    pub window: TimeInterval,
    pub boundaries: (&'facts Candle, &'facts Candle),
    pub outside: (&'facts Candle, &'facts Candle),
    pub distractors: &'facts [Candle],
}
pub async fn candle_history<P: MarketDataRepository>(port: &P, facts: CandleHistoryFacts<'_>) {
    let CandleHistoryFacts {
        instrument,
        window,
        boundaries,
        outside,
        distractors,
    } = facts;
    let (first, last) = boundaries;
    let ordered = vec![first.clone(), last.clone()];
    assert!(
        first.time().timestamp() < last.time().timestamp(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_eq!(
        first.time().timestamp(),
        window.start(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_eq!(
        outside.1.time().timestamp(),
        window.end(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    validate_candle_witnesses(instrument, window, first, distractors);
    let period = first.time().period();
    let adjustment = first.adjustment();
    for value in distractors
        .iter()
        .filter(|value| value.instrument() != instrument)
    {
        reject_candle_mismatch(port, instrument, (first, value), window).await;
    }
    let mut input = ordered.clone();
    input.reverse();
    input.extend([outside.0.clone(), outside.1.clone(), first.clone()]);
    super::market_data_repository_store_candles(port, instrument, &input, Ok(WriteCompleted)).await;
    super::market_data_repository_store_candles(port, instrument, &ordered, Ok(WriteCompleted))
        .await;
    for value in distractors {
        super::market_data_repository_store_candles(
            port,
            value.instrument(),
            std::slice::from_ref(value),
            Ok(WriteCompleted),
        )
        .await;
    }
    super::market_data_repository_candles(
        port,
        instrument,
        (window, period, adjustment),
        Ok(ordered.clone()),
    )
    .await;
}

pub async fn valuation_history<P: ValuationRepository>(
    port: &P,
    accounts: (&AccountId, &AccountId),
    window: TimeInterval,
    ordered: &[ValuationSnapshot],
    outside: &ValuationSnapshot,
) {
    let (account, other) = accounts;
    assert_ne!(
        account, other,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_ne!(ordered, [], "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    for snapshot in ordered.iter().rev() {
        super::valuation_repository_append(port, account, snapshot, Ok(RecordOutcome::Recorded))
            .await;
        super::valuation_repository_append(
            port,
            account,
            snapshot,
            Ok(RecordOutcome::AlreadyRecorded),
        )
        .await;
    }
    super::valuation_repository_append(port, account, outside, Ok(RecordOutcome::Recorded)).await;
    for snapshot in ordered.iter().take(1) {
        super::valuation_repository_append(port, other, snapshot, Ok(RecordOutcome::Recorded))
            .await;
    }
    super::valuation_repository_history(port, account, window, Ok(ordered.to_vec())).await;
    super::valuation_repository_history(
        port,
        other,
        window,
        Ok(ordered.iter().take(1).cloned().collect()),
    )
    .await;
}

pub async fn news_roundtrip<P: NewsRepository>(
    port: &P,
    instruments: (&InstrumentId, &InstrumentId),
    items: (&NewsItem, &NewsItem),
) {
    let (instrument, other) = instruments;
    let (item, other_item) = items;
    assert_ne!(instrument, other, "서로 다른 뉴스 종목 증인");
    assert_ne!(item, other_item, "서로 다른 기사 증인");
    super::news_repository_for_instrument(port, instrument, Ok(vec![])).await;
    for value in [item, item, other_item] {
        super::news_repository_store(port, value, Ok(WriteCompleted)).await;
    }
    super::news_repository_for_instrument(port, instrument, Ok(vec![item.clone()])).await;
    super::news_repository_for_instrument(port, other, Ok(vec![other_item.clone()])).await;
}

pub async fn settings_roundtrip<P: SettingsRepository>(
    port: &P,
    profiles: (&InvestorProfile, &InvestorProfile),
    risks: (&RiskSettings, &RiskSettings, &RiskSettings),
) {
    let (initial, changed) = profiles;
    let (original, revised, restored) = risks;
    assert_ne!(
        initial, changed,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    assert_ne!(
        original, revised,
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    super::settings_repository_profile(port, Ok(None)).await;
    super::settings_repository_risk_settings(port, Ok(None)).await;
    super::settings_repository_store_profile(port, initial, Ok(WriteCompleted)).await;
    super::settings_repository_profile(port, Ok(Some(initial.clone()))).await;
    super::settings_repository_risk_settings(port, Ok(Some(*original))).await;
    super::settings_repository_store_risk_settings(port, original, Ok(WriteCompleted)).await;
    super::settings_repository_risk_settings(port, Ok(Some(*original))).await;
    super::settings_repository_store_risk_settings(port, revised, Ok(WriteCompleted)).await;
    super::settings_repository_risk_settings(port, Ok(Some(*revised))).await;
    super::settings_repository_profile(port, Ok(Some(changed.clone()))).await;
    super::settings_repository_store_profile(port, initial, Ok(WriteCompleted)).await;
    super::settings_repository_risk_settings(port, Ok(Some(*restored))).await;
}

pub async fn audit_append_only<P: AuditLog>(port: &P, entries: (&AuditEntry, &AuditEntry)) {
    let (first, second) = entries;
    assert_ne!(first, second, "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    super::audit_log_append(port, first, Ok(RecordOutcome::Recorded)).await;
    super::audit_log_append(port, first, Ok(RecordOutcome::AlreadyRecorded)).await;
    super::audit_log_append(port, second, Ok(RecordOutcome::Recorded)).await;
}

pub async fn secret_lifecycle<P: SecretStore>(
    port: &P,
    keys: (SecretKey, SecretKey),
    values: (&SecretValue, &SecretValue),
) {
    let (key, other) = keys;
    let (first, second) = values;
    assert_ne!(key, other, "ADR 계약: 독립된 증인과 저장·조회 의미 보존");
    assert_ne!(
        first.as_exposed_secret(),
        second.as_exposed_secret(),
        "ADR 계약: 독립된 증인과 저장·조회 의미 보존"
    );
    super::secret_store_load(port, key, Ok(None)).await;
    super::secret_store_store(
        port,
        key,
        SecretValue::new(first.as_exposed_secret().to_vec()),
        Ok(WriteCompleted),
    )
    .await;
    super::secret_store_store(
        port,
        other,
        SecretValue::new(first.as_exposed_secret().to_vec()),
        Ok(WriteCompleted),
    )
    .await;
    super::secret_store_store(
        port,
        key,
        SecretValue::new(second.as_exposed_secret().to_vec()),
        Ok(WriteCompleted),
    )
    .await;
    super::secret_store_load(
        port,
        key,
        Ok(Some(SecretValue::new(second.as_exposed_secret().to_vec()))),
    )
    .await;
    super::secret_store_remove(port, key, Ok(WriteCompleted)).await;
    super::secret_store_remove(port, key, Ok(WriteCompleted)).await;
    super::secret_store_load(port, key, Ok(None)).await;
    super::secret_store_load(
        port,
        other,
        Ok(Some(SecretValue::new(first.as_exposed_secret().to_vec()))),
    )
    .await;
}

pub async fn quote_latest<P: MarketDataRepository>(
    port: &P,
    instruments: (&InstrumentId, &InstrumentId),
    quotes: (&Quote, &Quote),
) {
    let (instrument, other) = instruments;
    let (older, newer) = quotes;
    assert_ne!(instrument, other, "서로 다른 종목 증인");
    assert!(
        older.collected_at() < newer.collected_at(),
        "서로 다른 수집 시각 증인"
    );
    super::market_data_repository_quote(port, instrument, Ok(None)).await;
    for (quote, outcome) in [
        (newer, CacheWriteOutcome::Changed),
        (older, CacheWriteOutcome::OlderIgnored),
    ] {
        super::market_data_repository_store_quote(port, instrument, quote, Ok(outcome)).await;
    }
    super::market_data_repository_store_quote(port, other, older, Ok(CacheWriteOutcome::Changed))
        .await;
    super::market_data_repository_quote(port, instrument, Ok(Some(newer.clone()))).await;
    super::market_data_repository_quote(port, other, Ok(Some(older.clone()))).await;
}

pub async fn fx_latest_not_after<P: MarketDataRepository>(
    port: &P,
    rates: (&FxRate, &FxRate),
    queries: (Timestamp, Timestamp),
    others: (&FxRate, &FxRate),
) {
    let (older, newer) = rates;
    let (before_all, between) = queries;
    assert_eq!(older.pair(), newer.pair(), "같은 통화쌍 증인");
    assert_eq!(older.kind(), newer.kind(), "같은 환율 종류 증인");
    assert!(
        before_all < older.as_of() && older.as_of() < newer.as_of(),
        "이전·최신 시각 증인"
    );
    assert!(
        older.as_of() <= between && between < newer.as_of(),
        "두 관측 사이 시각 증인"
    );
    assert_ne!(older.pair(), others.0.pair(), "서로 다른 통화쌍 증인");
    assert_ne!(older.kind(), others.1.kind(), "서로 다른 환율 종류 증인");
    assert!(
        others.0.as_of() > newer.as_of() && others.1.as_of() > newer.as_of(),
        "다른 키의 더 최신 자료 증인"
    );
    let pair = older.pair();
    let kind = older.kind();
    for rate in [older, newer, others.0, others.1] {
        super::market_data_repository_store_fx_rate(port, rate, Ok(CacheWriteOutcome::Changed))
            .await;
    }
    for (query, expected) in [
        (FxRateQuery::Latest, Some(*newer)),
        (FxRateQuery::At(newer.as_of()), Some(*newer)),
        (FxRateQuery::At(between), Some(*older)),
        (FxRateQuery::At(before_all), None),
    ] {
        super::market_data_repository_fx_rate(port, pair, kind, query, Ok(expected)).await;
    }
    for rate in <[_; 2]>::from(others) {
        super::market_data_repository_fx_rate(
            port,
            rate.pair(),
            rate.kind(),
            FxRateQuery::Latest,
            Ok(Some(*rate)),
        )
        .await;
    }
}

pub async fn settings_risk_first<P: SettingsRepository>(
    port: &P,
    risk: &RiskSettings,
    expected_profile: &InvestorProfile,
) {
    super::settings_repository_profile(port, Ok(None)).await;
    super::settings_repository_risk_settings(port, Ok(None)).await;
    super::settings_repository_store_risk_settings(port, risk, Ok(WriteCompleted)).await;
    super::settings_repository_profile(port, Ok(Some(expected_profile.clone()))).await;
    super::settings_repository_risk_settings(port, Ok(Some(*risk))).await;
}

async fn reject_candle_mismatch<P: MarketDataRepository>(
    port: &P,
    instrument: &InstrumentId,
    values: (&Candle, &Candle),
    window: TimeInterval,
) {
    let (matching, other) = values;
    assert!(
        port.store_candles(instrument, &[matching.clone(), other.clone()])
            .await
            .is_err(),
        "저장 인자와 봉 종목 불일치는 거부"
    );
    for value in [matching, other] {
        assert_eq!(
            port.candles(
                value.instrument(),
                window,
                value.time().period(),
                value.adjustment()
            )
            .await,
            Ok(vec![]),
            "거부된 봉 묶음은 어느 종목에도 저장되지 않음"
        );
    }
}
fn validate_candle_witnesses(
    instrument: &InstrumentId,
    window: TimeInterval,
    first: &Candle,
    distractors: &[Candle],
) {
    assert!(
        distractors
            .iter()
            .all(|value| value.time().timestamp() >= window.start()
                && value.time().timestamp() < window.end()),
        "방해 자료도 조회 기간 안에 위치"
    );
    assert!(
        distractors
            .iter()
            .any(|value| value.instrument() != instrument),
        "서로 다른 종목 봉 증인"
    );
    assert!(
        distractors
            .iter()
            .any(|value| value.instrument() == instrument
                && value.time().period() != first.time().period()),
        "서로 다른 봉 주기 증인"
    );
    assert!(
        distractors
            .iter()
            .any(|value| value.instrument() == instrument
                && value.adjustment() != first.adjustment()),
        "서로 다른 수정 기준 증인"
    );
}
