#[cfg(test)]
mod tests {
    use application::SignalRepository;
    use application::{
        AuditEntry, AuditIntegrity, AuditKind, FxRateQuery, PortfolioRepository, RecordOutcome,
        SecretKey, SecretValue, SystemFailure, SystemPort, SystemPortError, WriteCompleted,
    };
    use domain::{
        AccountId, FxPair, FxRate, FxRateKind, FxSource, InstrumentId, Nanosecond, NewsItem,
        NewsSource, NormalizedUrl, NumericInput, OffsetSeconds, RiskChange, SyncOutcome,
        SyncRecord, Thesis, TimeInterval, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts,
        memory::{
            MemoryMarketDataRepository, MemoryNewsRepository, MemoryPortfolioRepository,
            MemoryReply, MemorySignalRepository, MemoryThesisRepository,
            MemoryTransactionRepository,
        },
        system_fakes::{MemoryAuditLog, MemorySecretStore},
    };
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn memory_replies<I, O>(script: &testkit_ports::Script<I, MemoryReply<O>>, count: usize) {
        for _ in 0..count {
            script.push(MemoryReply::Memory);
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn link_news(repository: &MemoryNewsRepository, instrument: InstrumentId, item: &NewsItem) {
        let domain::InformationSource::News(source) = item.collected().source() else {
            panic!("news source");
        };
        let key = application::InformationKey::News(source.clone(), item.collected().url().clone());
        run(application::InformationLinks::set_instruments(
            repository,
            &key,
            &[instrument],
        ))
        .expect("link");
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn master() -> domain::MasterInstrument {
        domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: domain::Symbol::try_new("EXAMPLE").expect("심볼"),
            instrument_id: id("fixture"),
            name: domain::DisplayInstrumentName::try_new("가상 종목").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: domain::Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: at(0),
        })
        .expect("마스터")
    }
    fn rate(value: &str, pair: FxPair, kind: FxRateKind, start: i64) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("숫자"),
            pair,
            kind,
            TimeInterval::try_new(at(start), at(start.checked_add(20).expect("종료")))
                .expect("기간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    #[test]
    fn latest_sync_preserves_accounts_time_and_failures() {
        let repository = MemoryPortfolioRepository::default();
        let first = AccountId::new(17);
        let second = AccountId::new(29);
        let recent = SyncRecord::new(at(23), SyncOutcome::AccessBlocked);
        let older = SyncRecord::new(at(11), SyncOutcome::Failed);
        memory_replies(repository.script_record(), 3);
        memory_replies(repository.script_latest(), 3);
        run(contracts::portfolio_repository_record(
            &repository,
            &first,
            &recent,
            Ok(WriteCompleted),
        ));
        run(contracts::portfolio_repository_record(
            &repository,
            &first,
            &older,
            Ok(WriteCompleted),
        ));
        run(contracts::portfolio_repository_record(
            &repository,
            &second,
            &older,
            Ok(WriteCompleted),
        ));
        run(contracts::portfolio_repository_latest(
            &repository,
            &first,
            Ok(Some(recent)),
        ));
        run(contracts::portfolio_repository_latest(
            &repository,
            &second,
            Ok(Some(older)),
        ));
        run(contracts::portfolio_repository_latest(
            &repository,
            &AccountId::new(41),
            Ok(None),
        ));
        let denied = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        repository
            .script_record()
            .push(MemoryReply::Return(Err(denied)));
        assert_eq!(
            run(repository.record(&first, &SyncRecord::new(at(47), SyncOutcome::Failed))),
            Err(denied),
            "대본 오류 보존"
        );
        repository.script_latest().push(MemoryReply::Memory);
        assert_eq!(
            run(repository.latest(&first))
                .expect("기존 기록")
                .expect("있음")
                .recorded_at(),
            at(23),
            "실패 쓰기는 상태를 바꾸지 않음"
        );
        assert_eq!(repository.calls().len(), 8, "실패도 호출 순서에 포함");
    }
    #[test]
    fn transaction_and_signal_absence_are_memory_results() {
        let transactions = MemoryTransactionRepository::default();
        let account = AccountId::new(53);
        let record = SyncRecord::new(at(59), SyncOutcome::Failed);
        transactions.script_record().push(MemoryReply::Memory);
        transactions.script_latest().push(MemoryReply::Memory);
        run(contracts::transaction_repository_record(
            &transactions,
            &account,
            &record,
            Ok(WriteCompleted),
        ));
        run(contracts::transaction_repository_latest(
            &transactions,
            &account,
            Ok(Some(record)),
        ));
        let signals = MemorySignalRepository::default();
        let signal_instrument = id("instrument-a");
        let snapshot = run(signals.snapshot(&signal_instrument)).expect("상태");
        run(contracts::signal_repository_commit(
            &signals,
            &signal_instrument,
            &snapshot,
            &application::SignalMutation::Discard(Box::new(domain::SignalTransition::Unchanged)),
            Ok(application::SignalCommitOutcome::Applied(
                application::SignalCommit {
                    events: vec![],
                    no_signal_reason: None,
                },
            )),
        ));
        run(contracts::signal_repository_current(
            &signals,
            &id("instrument-a"),
            Ok(vec![]),
        ));
    }
    #[test]
    fn fx_selects_pair_kind_and_latest_not_after_requested_time() {
        let repository = MemoryMarketDataRepository::default();
        let first = rate("1391", FxPair::UsdKrw, FxRateKind::Mid, 11);
        let latest = rate("1433", FxPair::UsdKrw, FxRateKind::Mid, 23);
        let buy = rate("1477", FxPair::UsdKrw, FxRateKind::Buy, 29);
        let inverse = rate("0.0007", FxPair::KrwUsd, FxRateKind::Mid, 31);
        for value in [first, latest, buy, inverse] {
            repository.script_store_fx_rate().push(MemoryReply::Memory);
            run(contracts::market_data_repository_store_fx_rate(
                &repository,
                &value,
                Ok(application::CacheWriteOutcome::Changed),
            ));
        }
        for (pair, kind, query, expected) in [
            (
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::Latest,
                Some(latest),
            ),
            (
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::At(at(23)),
                Some(latest),
            ),
            (
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::At(at(22)),
                Some(first),
            ),
            (
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::At(at(10)),
                None,
            ),
            (
                FxPair::UsdKrw,
                FxRateKind::Buy,
                FxRateQuery::Latest,
                Some(buy),
            ),
            (
                FxPair::KrwUsd,
                FxRateKind::Mid,
                FxRateQuery::Latest,
                Some(inverse),
            ),
        ] {
            repository.script_fx_rate().push(MemoryReply::Memory);
            run(contracts::market_data_repository_fx_rate(
                &repository,
                pair,
                kind,
                query,
                Ok(expected),
            ));
        }
    }
    #[test]
    fn audit_is_append_only_idempotent_and_verify_is_scripted() {
        let log = MemoryAuditLog::default();
        let first = AuditEntry::new(
            at(67),
            AuditKind::RiskSettingsChanged(RiskChange::AllowHours),
        );
        let second = AuditEntry::new(
            at(71),
            AuditKind::RiskSettingsChanged(RiskChange::EnableOrders),
        );
        memory_replies(log.script_append(), 3);
        run(contracts::audit_log_append(
            &log,
            &first,
            Ok(RecordOutcome::Recorded),
        ));
        run(contracts::audit_log_append(
            &log,
            &first,
            Ok(RecordOutcome::AlreadyRecorded),
        ));
        run(contracts::audit_log_append(
            &log,
            &second,
            Ok(RecordOutcome::Recorded),
        ));
        assert_eq!(
            log.entries(),
            [first, second],
            "중복은 기존 감사 기록을 바꾸지 않음"
        );
        log.script_verify().push(Ok(AuditIntegrity::Tampered));
        run(contracts::audit_log_verify(
            &log,
            Ok(AuditIntegrity::Tampered),
        ));
        assert_eq!(log.calls().len(), 4, "추가 및 검증 순서 기록");
    }
    #[test]
    fn secret_memory_round_trip_replacement_removal_and_key_isolation() {
        let store = MemorySecretStore::default();
        memory_replies(store.script_store(), 3);
        memory_replies(store.script_load(), 4);
        memory_replies(store.script_remove(), 2);
        run(contracts::secret_store_store(
            &store,
            SecretKey::DartApiKey,
            SecretValue::new(vec![17, 23]),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_store(
            &store,
            SecretKey::FredApiKey,
            SecretValue::new(vec![31, 47, 59]),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::DartApiKey,
            Ok(Some(SecretValue::new(vec![17, 23]))),
        ));
        run(contracts::secret_store_store(
            &store,
            SecretKey::DartApiKey,
            SecretValue::new(vec![61]),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::DartApiKey,
            Ok(Some(SecretValue::new(vec![61]))),
        ));
        run(contracts::secret_store_remove(
            &store,
            SecretKey::DartApiKey,
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_remove(
            &store,
            SecretKey::DartApiKey,
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::DartApiKey,
            Ok(None),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::FredApiKey,
            Ok(Some(SecretValue::new(vec![31, 47, 59]))),
        ));
        assert!(
            !format!("{:?}", store.calls()).contains("31, 47"),
            "호출 기록은 비밀 값을 숨김"
        );
    }
    #[test]
    fn thesis_history_restores_through_public_domain_api() {
        let repository = MemoryThesisRepository::default();
        repository.script_store().push(MemoryReply::Memory);
        repository.script_load().push(MemoryReply::Memory);
        run(contracts::thesis_repository_store(
            &repository,
            &id("thesis-a"),
            &Thesis::new(),
            Ok(WriteCompleted),
        ));
        run(contracts::thesis_repository_load(
            &repository,
            &id("thesis-a"),
            Ok(Some(Thesis::new())),
        ));
    }
    #[test]
    fn news_links_are_explicit_fixture_facts_and_do_not_infer_symbols_from_text() {
        let repository = MemoryNewsRepository::default();
        let item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://example.com/fixture-article").expect("주소"),
            UntrustedText::try_new("고정 기사 제목").expect("제목"),
            None,
        );
        link_news(&repository, id("news-a"), &item);
        repository.script_store().push(MemoryReply::Memory);
        repository.script_for_instrument().push(MemoryReply::Memory);
        repository.script_for_instrument().push(MemoryReply::Memory);
        run(contracts::news_repository_store(
            &repository,
            &item,
            Ok(WriteCompleted),
        ));
        run(contracts::news_repository_for_instrument(
            &repository,
            &id("news-a"),
            Ok(vec![item]),
        ));
        run(contracts::news_repository_for_instrument(
            &repository,
            &id("news-b"),
            Ok(vec![]),
        ));
    }
    #[test]
    fn settings_and_remaining_repository_queries_share_public_contract_checks() {
        let settings = testkit_ports::memory::MemorySettingsRepository::default();
        let profile = domain::InvestorProfile::default();
        let risk = domain::RiskSettings::new(
            domain::OrderPermission::Enabled,
            domain::SavedMarketOrderSetting::Disabled,
            domain::HoursPermissionState::Disabled,
            profile.weight_caps(),
            domain::RiskAppetite::Neutral,
        );
        settings.script_store_profile().push(MemoryReply::Memory);
        settings.script_profile().push(MemoryReply::Memory);
        settings
            .script_store_risk_settings()
            .push(MemoryReply::Memory);
        settings.script_risk_settings().push(MemoryReply::Memory);
        run(contracts::settings_repository_store_profile(
            &settings,
            &profile,
            Ok(WriteCompleted),
        ));
        run(contracts::settings_repository_profile(
            &settings,
            Ok(Some(profile)),
        ));
        run(contracts::settings_repository_store_risk_settings(
            &settings,
            &risk,
            Ok(WriteCompleted),
        ));
        run(contracts::settings_repository_risk_settings(
            &settings,
            Ok(Some(risk)),
        ));
        let estimates = testkit_ports::memory::MemoryEstimateRepository::default();
        estimates.script_submissions().push(MemoryReply::Memory);
        run(contracts::estimate_repository_submissions(
            &estimates,
            &id("absent-estimate"),
            Ok(vec![]),
        ));
        let proposals = testkit_ports::memory::MemoryProposalRepository::default();
        proposals.script_draft().push(MemoryReply::Memory);
        let proposal = domain::ProposalId::new(
            domain::ClientOrderId::try_new("absent-proposal").expect("제안 키"),
        );
        run(contracts::proposal_repository_draft(
            &proposals,
            &proposal,
            &master(),
            Ok(None),
        ));
        let valuations = testkit_ports::memory::MemoryValuationRepository::default();
        valuations.script_history().push(MemoryReply::Memory);
        run(contracts::valuation_repository_history(
            &valuations,
            &AccountId::new(83),
            TimeInterval::try_new(at(11), at(29)).expect("기간"),
            Ok(vec![]),
        ));
    }
    #[test]
    fn calibration_appends_exact_duplicates_once_and_returns_latest_version() {
        let repository = testkit_ports::memory::MemoryEstimateRepository::default();
        let part = |text| domain::ModelIdentityPart::try_new(text).expect("모델 식별");
        let key = domain::CalibrationKey::new(
            domain::CalibrationModel::new(
                part("fixture-provider"),
                part("fixture-name"),
                part("fixture-version"),
            ),
            domain::Horizon::default(),
        );
        let calibration = domain::Calibration::try_new(key.clone(), &[]).expect("빈 보정 자료");
        repository
            .script_append_calibration()
            .push(MemoryReply::Memory);
        repository
            .script_append_calibration()
            .push(MemoryReply::Memory);
        repository.script_calibration().push(MemoryReply::Memory);
        run(contracts::estimate_repository_append_calibration(
            &repository,
            &key,
            &calibration,
            Ok(RecordOutcome::Recorded),
        ));
        run(contracts::estimate_repository_append_calibration(
            &repository,
            &key,
            &calibration,
            Ok(RecordOutcome::AlreadyRecorded),
        ));
        run(contracts::estimate_repository_calibration(
            &repository,
            &key,
            Ok(Some(calibration)),
        ));
    }
    #[test]
    fn quote_memory_never_returns_another_instruments_latest_fact() {
        let repository = MemoryMarketDataRepository::default();
        let first = domain::Quote::try_new(
            domain::Symbol::try_new("AAPL").expect("첫 코드"),
            domain::Currency::Usd,
            None,
            Some(at(13)),
            at(17),
        )
        .expect("첫 시세");
        let second = domain::Quote::try_new(
            domain::Symbol::try_new("MSFT").expect("둘째 코드"),
            domain::Currency::Usd,
            None,
            Some(at(23)),
            at(29),
        )
        .expect("둘째 시세");
        repository.script_store_quote().push(MemoryReply::Memory);
        repository.script_store_quote().push(MemoryReply::Memory);
        repository.script_quote().push(MemoryReply::Memory);
        repository.script_quote().push(MemoryReply::Memory);
        repository.script_quote().push(MemoryReply::Memory);
        run(contracts::market_data_repository_store_quote(
            &repository,
            &id("quote-a"),
            &first,
            Ok(application::CacheWriteOutcome::Changed),
        ));
        run(contracts::market_data_repository_store_quote(
            &repository,
            &id("quote-b"),
            &second,
            Ok(application::CacheWriteOutcome::Changed),
        ));
        run(contracts::market_data_repository_quote(
            &repository,
            &id("quote-a"),
            Ok(Some(first)),
        ));
        run(contracts::market_data_repository_quote(
            &repository,
            &id("quote-b"),
            Ok(Some(second)),
        ));
        run(contracts::market_data_repository_quote(
            &repository,
            &id("quote-c"),
            Ok(None),
        ));
    }
}
