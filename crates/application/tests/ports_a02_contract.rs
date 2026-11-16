#[cfg(test)]
mod tests {
    fn port_source(path: &str) -> String {
        std::fs::read_to_string(path).expect("포트 소스")
    }
    fn compact(text: &str) -> String {
        text.chars()
            .filter(|value| !value.is_whitespace())
            .collect::<String>()
            .replace(",)", ")")
    }
    fn check_trait(source: &str, name: &str, methods: &[&str]) {
        let declaration = format!("pub trait {name}: Send + Sync {{");
        let body = source
            .split_once(&declaration)
            .expect("공개 포트")
            .1
            .split_once('}')
            .expect("닫힌 trait")
            .0;
        let actual: Vec<String> = body
            .split(';')
            .map(compact)
            .filter(|value| !value.is_empty())
            .collect();
        let expected: Vec<String> = methods.iter().map(|method| compact(method)).collect();
        assert_eq!(actual, expected, "{name} 계약");
    }
    #[test]
    fn system_traits_match_the_contract_and_have_no_extra_methods() {
        check_trait(
            &port_source("src/clock.rs"),
            "Clock",
            &["fn now(&self) -> Timestamp"],
        );
        check_trait(
            &port_source("src/notifier.rs"),
            "Notifier",
            &[
                "fn notify(&self, notification: Notification) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &port_source("src/user_presence.rs"),
            "UserPresence",
            &[
                "fn confirm_order(&self, summary: &OrderSummary) -> impl Future<Output = OrderConfirmation> + Send",
                "fn confirm_risk_change(&self, change: RiskChange) -> impl Future<Output = RiskChangeConfirmation> + Send",
            ],
        );
        check_trait(
            &port_source("src/audit_log.rs"),
            "AuditLog",
            &[
                "fn append(&self, entry: &AuditEntry) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send",
                "fn verify(&self) -> impl Future<Output = Result<AuditIntegrity, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &port_source("src/secret_port.rs"),
            "SecretStore",
            &[
                "fn load(&self, key: SecretKey) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send",
                "fn store(&self, key: SecretKey, value: SecretValue) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn remove(&self, key: SecretKey) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
            ],
        );
    }
    #[test]
    fn storage_traits_use_only_sealed_domain_values() {
        let source = port_source("src/storage_ports.rs");
        check_trait(
            &source,
            "PortfolioRepository",
            &[
                "fn record(&self, account: &AccountId, record: &SyncRecord<Portfolio>) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn latest(&self, account: &AccountId) -> impl Future<Output = Result<Option<SyncRecord<Portfolio>>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "TransactionRepository",
            &[
                "fn record(&self, account: &AccountId, record: &SyncRecord<SettledRowSet>) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn latest(&self, account: &AccountId) -> impl Future<Output = Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "MarketDataRepository",
            &[
                "fn store_quote(&self, instrument: &InstrumentId, quote: &Quote) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn quote(&self, instrument: &InstrumentId) -> impl Future<Output = Result<Option<Quote>, SystemPortError>> + Send",
                "fn store_candles(&self, instrument: &InstrumentId, candles: &[Candle]) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn candles(&self, instrument: &InstrumentId, window: TimeInterval, period: CandlePeriod, adjustment: PriceAdjustment) -> impl Future<Output = Result<Vec<Candle>, SystemPortError>> + Send",
                "fn store_fx_rate(&self, rate: &FxRate) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn fx_rate(&self, pair: FxPair, kind: FxRateKind, at: FxRateQuery) -> impl Future<Output = Result<Option<FxRate>, SystemPortError>> + Send",
            ],
        );
    }

    #[test]
    fn content_and_signal_storage_preserve_domain_types() {
        let source = port_source("src/storage_ports.rs");
        check_trait(
            &source,
            "NewsRepository",
            &[
                "fn store(&self, item: &NewsItem) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn for_instrument(&self, instrument: &InstrumentId) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "ThesisRepository",
            &[
                "fn store(&self, instrument: &InstrumentId, thesis: &Thesis) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn load(&self, instrument: &InstrumentId) -> impl Future<Output = Result<Option<Thesis>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "SignalRepository",
            &[
                "fn replace(&self, instrument: &InstrumentId, signals: &[RaisedSignal]) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn current(&self, instrument: &InstrumentId) -> impl Future<Output = Result<Vec<RaisedSignal>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "ProposalRepository",
            &[
                "fn store_draft(&self, proposal: &Proposal<Draft>) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn draft(&self, proposal: &ProposalId) -> impl Future<Output = Result<Option<Proposal<Draft>>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "ValuationRepository",
            &[
                "fn append(&self, account: &AccountId, snapshot: &ValuationSnapshot) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send",
                "fn history(&self, account: &AccountId, window: TimeInterval) -> impl Future<Output = Result<Vec<ValuationSnapshot>, SystemPortError>> + Send",
            ],
        );
        check_trait(
            &source,
            "SettingsRepository",
            &[
                "fn store_profile(&self, profile: &InvestorProfile) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn profile(&self) -> impl Future<Output = Result<Option<InvestorProfile>, SystemPortError>> + Send",
                "fn store_risk_settings(&self, settings: &RiskSettings) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send",
                "fn risk_settings(&self) -> impl Future<Output = Result<Option<RiskSettings>, SystemPortError>> + Send",
            ],
        );
    }
    #[test]
    fn estimate_storage_keeps_append_only_submissions_and_resolutions() {
        check_trait(
            &port_source("src/storage_ports.rs"),
            "EstimateRepository",
            &[
                "fn append_submission(&self, submission: &SignalSubmission) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send",
                "fn submissions(&self, instrument: &InstrumentId) -> impl Future<Output = Result<Vec<SignalSubmission>, SystemPortError>> + Send",
                "fn append_resolution(&self, task: &SignalTaskKey, outcome: &EstimateOutcome) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send",
                "fn resolution(&self, task: &SignalTaskKey) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send",
                "fn append_calibration(&self, key: &CalibrationKey, calibration: &Calibration) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send",
                "fn calibration(&self, key: &CalibrationKey) -> impl Future<Output = Result<Option<Calibration>, SystemPortError>> + Send",
            ],
        );
    }
}
