#[cfg(test)]
mod tests {
    mod a27_fixtures;
    use a27_fixtures as fixtures;
    use application::{
        EvaluateSignalError, EvaluateSignalInputs, EvaluateSignalOutcome, EvaluateSignals,
        EventBusBuilder, ProposalRepository, SignalCommitEvidence, SignalCommitOutcome,
        SignalEvaluation, SignalEvaluationData, SignalInvalidationOutcome,
        SignalInvalidationRepository, SignalMutation, SignalRepository, SignalRestorationBasis,
        SignalSourceFacts, SubmitEstimate, SystemFailure, SystemPort, SystemPortError,
        WriteCompleted,
    };
    use domain::{
        AcquisitionPrice, BuySignalOccurrenceId, BuySignalRecord, BuyingPower, Calibration,
        CalibrationSnapshot, ClientOrderId, Currency, EstimateSubmissionRecord,
        HistoricalPortfolio, HistoricalPortfolioError, HistoricalSignal, Holding,
        HoldingObservation, InvestorProfile, KoreanMasterDetail, LiquidationTrading, ListingStatus,
        Market, MasterCode, MasterInstrument, MasterInstrumentError, MasterInstrumentFacts,
        MasterSecurityType, NoSignalReason, NxtTrading, OrderIntent, OrderSide, OrderTerms, Policy,
        Portfolio, Proposal, ProposalId, ProposalLifetime, ProposalPriceRange, RaisedSignal,
        ReportedHolding, SectorAssignments, SectorCatalog, SectorComposition, SectorDepth,
        SectorId, SectorMetadataRow, SectorStockRow, SignalDecision, Symbol, Timestamp,
        TradingSuspension, WholeOrderQuantity, sector_assignment_from_compositions,
        signal_transition_from_history,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{
        MemoryProposalRepository, MemoryReply, MemorySignalEvaluationRepository,
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

    struct StaticInputs(EvaluateSignalInputs);
    impl SignalEvaluationData for StaticInputs {
        fn inputs(
            &self,
            _record: &EstimateSubmissionRecord<application::AiRunId, application::ReasoningEffort>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send {
            std::future::ready(Ok(self.0.clone()))
        }
    }

    fn kr_calendars() -> Vec<domain::MarketCalendar> {
        (60_i64..=66)
            .map(|day| fixtures::calendar(day, &Market::Kr))
            .collect()
    }

    fn repository_with_submissions() -> MemorySignalEvaluationRepository {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("보정 위치");
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(&repository, &events).execute(
            &fixtures::run_id(),
            vec![
                fixtures::input("task-old", "99", &fixtures::master()),
                fixtures::input("task-other", "10", &fixtures::other_master()),
            ],
        ))
        .expect("제출");
        for master in [fixtures::master(), fixtures::other_master()] {
            let snap = run(repository.snapshot(&master.row().instrument_id)).expect("스냅숏");
            repository.save_sources(
                snap.submission.expect("제출 ID"),
                SignalSourceFacts {
                    items: vec![fixtures::item()],
                    disclosures: vec![],
                    tools: vec![],
                },
            );
        }
        repository
    }

    fn non_tradable_variants(base: &MasterInstrument) -> Vec<MasterInstrument> {
        let mut suspended = base.row().clone();
        suspended.korean_detail = Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Suspended,
            nxt: NxtTrading::Unsupported,
        });
        let mut delisted = base.row().clone();
        delisted.listing_status = ListingStatus::Delisted;
        let mut scheduled = base.row().clone();
        scheduled.listing_status = ListingStatus::Scheduled;
        let mut liquidation = base.row().clone();
        liquidation.korean_detail = Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Liquidation,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        });
        let mut unknown_type = base.row().clone();
        unknown_type.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("UNKNOWN_SEC").expect("유형"));
        let mut missing_status = base.row().clone();
        missing_status.korean_detail = None;
        [
            suspended,
            delisted,
            scheduled,
            liquidation,
            unknown_type,
            missing_status,
        ]
        .into_iter()
        .map(|row| MasterInstrument::try_new(row).expect("비거래 마스터"))
        .collect()
    }

    fn commit_buy_signal(
        repository: &MemorySignalEvaluationRepository,
        source: &MasterInstrument,
        occ_name: &str,
    ) -> (RaisedSignal, BuySignalRecord, SignalCommitEvidence) {
        let decision = fixtures::decision(source, "99", None);
        let signal = decision.clone().into_signal().expect("매수 신호");
        let buy = match &signal {
            RaisedSignal::Buy(value) => value,
            RaisedSignal::Sell(_) => panic!("매수 필요"),
        };
        let record = BuySignalRecord::from_signal_checked(
            BuySignalOccurrenceId::try_new(occ_name).expect("ID"),
            buy,
            fixtures::day(60),
            &kr_calendars(),
        )
        .expect("발생");
        let proof = fixtures::evidence_for_decision(&decision, Some(&record)).expect("근거");
        let snap = run(repository.snapshot(&source.row().instrument_id)).expect("스냅숏");
        let transition = signal_transition_from_history(None, &decision).expect("전이");
        let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision,
            transition,
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(record.clone()),
            evidence: Some(proof.clone()),
        }));
        match run(repository.commit(&source.row().instrument_id, &snap, &mutation)).expect("커밋")
        {
            SignalCommitOutcome::Applied(_) => (signal, record, proof),
            SignalCommitOutcome::Conflict | SignalCommitOutcome::SubmissionDiscarded => {
                panic!("적용 필요")
            }
        }
    }

    fn assignment_for(primary: &MasterInstrument, now: Timestamp) -> SectorAssignments {
        let sector = SectorId::try_new("tech").expect("섹터");
        let catalog = SectorCatalog::from_response(
            vec![SectorMetadataRow {
                id: sector.clone(),
                name: domain::DisplayInstrumentName::try_new("기술").expect("이름"),
                depth: SectorDepth::Major,
                parent: None,
            }],
            now,
        )
        .expect("목록");
        let composition = SectorComposition::from_stocks_response(
            sector,
            domain::KnownMarket::Kr,
            vec![SectorStockRow {
                symbol: primary.row().symbol.clone(),
                currency: Currency::Krw,
                market_capitalization: Some(domain::NumericInput::try_new("100000").expect("시총")),
                trading_amount: None,
            }],
            now,
        )
        .expect("구성");
        let assignment =
            sector_assignment_from_compositions(primary, &catalog, &[composition]).expect("분류");
        SectorAssignments::try_new(vec![assignment]).expect("분류")
    }

    fn inputs_for(
        primary: &MasterInstrument,
        holding_masters: Vec<MasterInstrument>,
        portfolio: Portfolio,
    ) -> EvaluateSignalInputs {
        let now = fixtures::day(60);
        let head = fixtures::head(0);
        let table = Calibration::try_new(fixtures::calibration_key(), &[]).expect("표");
        let valuation = portfolio.evaluate(None);
        let decision = fixtures::decision(primary, "99", None);
        let proof = fixtures::evidence_for_decision(&decision, None).expect("근거");
        let target = fixtures::eligible_for_master("102", primary)
            .task()
            .reference()
            .clone();
        let restored = proof.signal.evidence;
        EvaluateSignalInputs {
            calibration: Some(CalibrationSnapshot::try_new(table, head.clone()).expect("스냅숏")),
            calibration_head: Some(head),
            valuation: Some(valuation),
            policy: Policy::new(InvestorProfile::default()),
            assignments: assignment_for(primary, now),
            security: MasterInstrumentFacts::try_new(vec![primary.clone()])
                .expect("마스터")
                .policy_security_facts(),
            target_price: Some(target),
            calendars: kr_calendars(),
            quantity_costs: None,
            restoration: Some(SignalRestorationBasis {
                master: primary.clone(),
                holding_masters,
                estimate: restored.estimate,
                commission: restored.commission,
                tax: restored.tax,
                maturity: restored.maturity,
                portfolio: Some(portfolio),
                rate: restored.assessment.rate,
            }),
        }
    }

    fn portfolio_holding_other(other: &MasterInstrument) -> Portfolio {
        let now = fixtures::day(60);
        Portfolio::new(
            vec![
                Holding::new(
                    other.order_instrument().expect("다른 종목"),
                    fixtures::quantity("5"),
                    AcquisitionPrice::try_new(fixtures::money("87", Currency::Krw)).expect("원가"),
                    HoldingObservation::new(
                        None,
                        Some(
                            ReportedHolding::new(
                                fixtures::money("113", Currency::Krw),
                                fixtures::money("7", Currency::Krw),
                                fixtures::ratio("0.07"),
                            )
                            .expect("평가"),
                        ),
                        now,
                    ),
                )
                .expect("보유"),
            ],
            BuyingPower::new(fixtures::money("10000", Currency::Krw), now),
            BuyingPower::new(fixtures::money("0", Currency::Usd), now),
        )
        .expect("포트")
    }

    fn verify_non_tradable_discard_and_read(master: &MasterInstrument, variant: &MasterInstrument) {
        let repo = repository_with_submissions();
        let (signal, record, _) = commit_buy_signal(&repo, master, "occ-nt");
        repo.save_master(variant.clone());
        let snap = run(repo.snapshot(&master.row().instrument_id)).expect("조회 허용");
        assert_eq!(
            snap.current.as_ref().expect("현재").signal,
            HistoricalSignal::from_signal(&signal)
        );
        assert_eq!(
            run(repo.current(&master.row().instrument_id)).expect("현재 허용"),
            vec![HistoricalSignal::from_signal(&signal)]
        );
        let occurrences = run(repo.occurrences(
            &master.row().instrument_id,
            &master.order_instrument().expect("종목"),
        ))
        .expect("이력 허용");
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].id(), record.id());
        let discard_transition = signal_transition_from_history(
            Some(&snap.current.as_ref().expect("현재").signal),
            &SignalDecision::None(NoSignalReason::Expired),
        )
        .expect("해제 전이");
        match run(repo.commit(
            &master.row().instrument_id,
            &snap,
            &SignalMutation::Discard(Box::new(discard_transition)),
        ))
        .expect("폐기 커밋 허용")
        {
            SignalCommitOutcome::Applied(_) => (),
            SignalCommitOutcome::Conflict | SignalCommitOutcome::SubmissionDiscarded => {
                panic!("폐기 적용 필요");
            }
        }
        assert_eq!(
            run(repo.current(&master.row().instrument_id)).expect("비어 있음"),
            Vec::new()
        );
    }

    fn verify_non_tradable_invalidate_and_reeval(
        master: &MasterInstrument,
        variant: &MasterInstrument,
    ) {
        let repo_inv = repository_with_submissions();
        commit_buy_signal(&repo_inv, master, "occ-inv");
        repo_inv.save_master(variant.clone());
        let inv_snap = run(repo_inv.snapshot(&master.row().instrument_id)).expect("스냅숏");
        match run(repo_inv.invalidate(&master.row().instrument_id, &inv_snap)).expect("무효화 허용")
        {
            SignalInvalidationOutcome::Applied(_) => (),
            SignalInvalidationOutcome::Conflict
            | SignalInvalidationOutcome::SubmissionDiscarded => panic!("무효화 적용 필요"),
        }
        let repo_eval = repository_with_submissions();
        commit_buy_signal(&repo_eval, master, "occ-eval");
        repo_eval.save_master(variant.clone());
        let now = fixtures::day(60);
        let empty_portfolio = Portfolio::new(
            vec![],
            BuyingPower::new(fixtures::money("10000", Currency::Krw), now),
            BuyingPower::new(fixtures::money("0", Currency::Usd), now),
        )
        .expect("포트");
        let mut inputs = inputs_for(master, vec![], empty_portfolio);
        inputs.target_price = None;
        let events = EventBusBuilder::default().build();
        match run(
            EvaluateSignals::new(&repo_eval, &StaticInputs(inputs), &events).execute(
                &master.row().instrument_id,
                now,
                BuySignalOccurrenceId::try_new("occ-eval-2").expect("ID"),
            ),
        )
        .expect("재평가 허용")
        {
            EvaluateSignalOutcome::Applied {
                reason: Some(NoSignalReason::Unavailable(_)),
                ..
            } => (),
            EvaluateSignalOutcome::Applied { .. }
            | EvaluateSignalOutcome::NoSubmission
            | EvaluateSignalOutcome::SubmissionDiscarded
            | EvaluateSignalOutcome::Unavailable { .. } => panic!("재평가 적용 필요"),
        }
    }

    #[test]
    fn non_tradable_master_still_allows_reading_discarding_invalidating_and_reevaluating_signals() {
        let master = fixtures::master();
        for variant in non_tradable_variants(&master) {
            verify_non_tradable_discard_and_read(&master, &variant);
            verify_non_tradable_invalidate_and_reeval(&master, &variant);
        }
    }

    #[test]
    fn evaluating_stock_a_while_holding_stock_b_commits_and_tracks_stock_b_master_integrity() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let repo = repository_with_submissions();
        let events = EventBusBuilder::default().build();
        let inputs = inputs_for(
            &master_a,
            vec![master_a.clone(), master_b.clone()],
            portfolio_holding_other(&master_b),
        );
        match run(
            EvaluateSignals::new(&repo, &StaticInputs(inputs), &events).execute(
                &master_a.row().instrument_id,
                fixtures::day(60),
                BuySignalOccurrenceId::try_new("occ-multi").expect("ID"),
            ),
        )
        .expect("다종목 포트폴리오 평가 성공")
        {
            EvaluateSignalOutcome::Applied { .. } => (),
            EvaluateSignalOutcome::NoSubmission
            | EvaluateSignalOutcome::SubmissionDiscarded
            | EvaluateSignalOutcome::Unavailable { .. } => panic!("적용 필요"),
        }
        let id_a = &master_a.row().instrument_id;
        let identity_a = master_a.order_instrument().expect("종목 A");
        assert!(run(repo.snapshot(id_a)).expect("스냅숏").current.is_some());
        let mut mutated_b = master_b.row().clone();
        mutated_b.symbol = Symbol::try_new("999999").expect("변경 심볼");
        repo.save_master(MasterInstrument::try_new(mutated_b).expect("변경 마스터"));
        let integrity =
            SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
        assert_eq!(run(repo.snapshot(id_a)), Err(integrity));
        assert_eq!(run(repo.current(id_a)), Err(integrity));
        assert_eq!(run(repo.occurrences(id_a, &identity_a)), Err(integrity));
        repo.save_master(master_b.clone());
        assert!(run(repo.snapshot(id_a)).expect("복구").current.is_some());
        for non_tradable_b in non_tradable_variants(&master_b) {
            repo.save_master(non_tradable_b);
            assert!(
                run(repo.snapshot(id_a))
                    .expect("비거래 스냅숏")
                    .current
                    .is_some()
            );
            assert_eq!(run(repo.current(id_a)).expect("비거래 현재").len(), 1);
            assert_eq!(
                run(repo.occurrences(id_a, &identity_a))
                    .expect("비거래 이력")
                    .len(),
                1
            );
        }
    }

    fn execute_expecting_unavailable(
        inputs: EvaluateSignalInputs,
        occ: &str,
    ) -> EvaluateSignalError {
        let master_a = fixtures::master();
        let repo = repository_with_submissions();
        let events = EventBusBuilder::default().build();
        let outcome = run(
            EvaluateSignals::new(&repo, &StaticInputs(inputs), &events).execute(
                &master_a.row().instrument_id,
                fixtures::day(60),
                BuySignalOccurrenceId::try_new(occ).expect("ID"),
            ),
        )
        .expect("무효화 반환");
        match outcome {
            EvaluateSignalOutcome::Unavailable { cause, .. } => *cause,
            EvaluateSignalOutcome::Applied { .. }
            | EvaluateSignalOutcome::NoSubmission
            | EvaluateSignalOutcome::SubmissionDiscarded => panic!("Unavailable 필요"),
        }
    }

    #[test]
    fn evaluate_signals_rejects_missing_conflicting_and_corrupted_restoration_inputs() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let portfolio = portfolio_holding_other(&master_b);
        assert_eq!(
            execute_expecting_unavailable(
                inputs_for(&master_a, vec![], portfolio.clone()),
                "occ-miss"
            ),
            EvaluateSignalError::RestorationPortfolio(
                HistoricalPortfolioError::MasterMissingOrAmbiguous
            )
        );
        let mut conflicting_a = master_a.row().clone();
        conflicting_a.name = domain::DisplayInstrumentName::try_new("충돌 이름").expect("이름");
        assert_eq!(
            execute_expecting_unavailable(
                inputs_for(
                    &master_a,
                    vec![
                        MasterInstrument::try_new(conflicting_a).expect("충돌"),
                        master_b.clone(),
                    ],
                    portfolio.clone(),
                ),
                "occ-conf"
            ),
            EvaluateSignalError::RestorationMasters(MasterInstrumentError::DuplicateIdentity)
        );
        let mut mismatch = inputs_for(&master_a, vec![master_b.clone()], portfolio.clone());
        let other_portfolio = Portfolio::new(
            vec![],
            BuyingPower::new(fixtures::money("999999", Currency::Krw), fixtures::day(60)),
            BuyingPower::new(fixtures::money("0", Currency::Usd), fixtures::day(60)),
        )
        .expect("다른 포트");
        mismatch.valuation = Some(other_portfolio.evaluate(None));
        assert_eq!(
            execute_expecting_unavailable(mismatch, "occ-val"),
            EvaluateSignalError::RestorationValuationMismatch
        );
        let mut no_basis = inputs_for(&master_a, vec![master_b.clone()], portfolio.clone());
        no_basis.restoration = None;
        assert_eq!(
            execute_expecting_unavailable(no_basis, "occ-none"),
            EvaluateSignalError::RestorationUnavailable
        );
        let mut bad_basis = inputs_for(&master_a, vec![master_b], portfolio);
        let est = &mut bad_basis.restoration.as_mut().expect("근거").estimate;
        est.evidence.candles.clear();
        assert_eq!(
            execute_expecting_unavailable(bad_basis, "occ-corrupt"),
            EvaluateSignalError::Restoration(domain::SignalRestoreError::Estimate(
                domain::EstimateRestoreError::Volatility(domain::VolatilityError::SampleCount)
            ))
        );
    }

    fn buy_record(decision: &SignalDecision, occ: &str) -> BuySignalRecord {
        match decision {
            SignalDecision::Buy(value) => BuySignalRecord::from_signal_checked(
                BuySignalOccurrenceId::try_new(occ).expect("ID"),
                value,
                fixtures::day(60),
                &kr_calendars(),
            )
            .expect("발생"),
            SignalDecision::Sell(_) | SignalDecision::None(_) => panic!("매수 필요"),
        }
    }

    #[test]
    fn memory_signal_repository_rejects_none_with_evidence_and_mismatched_signal_evidence() {
        let master = fixtures::master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let repo = repository_with_submissions();
        let snap = run(repo.snapshot(&master.row().instrument_id)).expect("스냅숏");
        let decision_99 = fixtures::decision(&master, "99", None);
        let proof_buy = fixtures::evidence_for_decision(&decision_99, None).expect("근거");
        let none_decision = SignalDecision::None(NoSignalReason::Expired);
        let bad_none = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: none_decision.clone(),
            transition: signal_transition_from_history(None, &none_decision).expect("전이"),
            calibration_head: Some(fixtures::head(0)),
            occurrence: None,
            evidence: Some(proof_buy),
        }));
        assert_eq!(
            run(repo.commit(&master.row().instrument_id, &snap, &bad_none)),
            Err(invalid)
        );
        let decision_98 = fixtures::decision(&master, "98", None);
        let rec_99 = buy_record(&decision_99, "occ-99");
        let rec_98 = buy_record(&decision_98, "occ-98");
        let proof_98 = fixtures::evidence_for_decision(&decision_98, Some(&rec_98)).expect("근거");
        let trans_99 = signal_transition_from_history(None, &decision_99).expect("전이");
        let bad_sig = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: decision_99,
            transition: trans_99,
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(rec_99),
            evidence: Some(proof_98),
        }));
        assert_eq!(
            run(repo.commit(&master.row().instrument_id, &snap, &bad_sig)),
            Err(invalid)
        );
    }

    #[test]
    fn memory_signal_repository_rejects_mismatched_and_missing_occurrence_evidence() {
        let master = fixtures::master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let repo = repository_with_submissions();
        let snap = run(repo.snapshot(&master.row().instrument_id)).expect("스냅숏");
        let decision_99 = fixtures::decision(&master, "99", None);
        let rec_99 = buy_record(&decision_99, "occ-99");
        let rec_alt = buy_record(&decision_99, "occ-99-alt");
        let trans_99 = signal_transition_from_history(None, &decision_99).expect("전이");
        let proof_alt =
            fixtures::evidence_for_decision(&decision_99, Some(&rec_alt)).expect("근거 alt");
        let bad_mismatch = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: decision_99.clone(),
            transition: trans_99.clone(),
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(rec_99.clone()),
            evidence: Some(proof_alt),
        }));
        assert_eq!(
            run(repo.commit(&master.row().instrument_id, &snap, &bad_mismatch)),
            Err(invalid)
        );
        let mut proof_missing =
            fixtures::evidence_for_decision(&decision_99, Some(&rec_99)).expect("근거");
        proof_missing.occurrence = None;
        let bad_missing = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: decision_99,
            transition: trans_99,
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(rec_99),
            evidence: Some(proof_missing),
        }));
        assert_eq!(
            run(repo.commit(&master.row().instrument_id, &snap, &bad_missing)),
            Err(invalid)
        );
    }

    #[test]
    fn memory_signal_repository_rejects_extra_occurrence_evidence_on_sell() {
        let master = fixtures::master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let decision_99 = fixtures::decision(&master, "99", None);
        let rec_99 = buy_record(&decision_99, "occ-99");
        let repo_sell = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repo_sell
            .advance_calibration_head(fixtures::head(0))
            .expect("보정");
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(&repo_sell, &events).execute(
            &fixtures::run_id(),
            vec![fixtures::input("task-old", "10", &master)],
        ))
        .expect("제출");
        let snap_sell = run(repo_sell.snapshot(&master.row().instrument_id)).expect("스냅숏");
        let decision_sell = fixtures::decision(&master, "10", Some("5"));
        let mut proof_sell = fixtures::evidence_for_decision(&decision_sell, None).expect("매도");
        proof_sell.occurrence = fixtures::evidence_for_decision(&decision_99, Some(&rec_99))
            .expect("근거")
            .occurrence;
        let bad_sell = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap_sell.submission.clone().expect("제출"),
            decision: decision_sell.clone(),
            transition: signal_transition_from_history(None, &decision_sell).expect("전이"),
            calibration_head: Some(fixtures::head(0)),
            occurrence: None,
            evidence: Some(proof_sell),
        }));
        assert_eq!(
            run(repo_sell.commit(&master.row().instrument_id, &snap_sell, &bad_sell)),
            Err(invalid)
        );
        HistoricalPortfolio::capture(
            &Portfolio::new(
                vec![],
                BuyingPower::new(fixtures::money("0", Currency::Krw), fixtures::day(60)),
                BuyingPower::new(fixtures::money("0", Currency::Usd), fixtures::day(60)),
            )
            .expect("포트"),
            &[],
        )
        .expect("빈 포트");
    }

    fn kr_draft_with_evidence(
        master: &MasterInstrument,
        name: &str,
        qty: &str,
    ) -> (Proposal<domain::Draft>, domain::DraftRestoreEvidence) {
        let task = fixtures::eligible_for_master("102", master);
        let stored_est = fixtures::candidate_stored_task(
            task.task(),
            master,
            domain::Horizon::new(domain::HorizonSelection::FewDays),
        )
        .expect("과제 근거");
        let range = ProposalPriceRange::from_estimate_task(
            task.task().range_facts(),
            task.task().reference().clone(),
        )
        .expect("범위");
        let id = ProposalId::new(ClientOrderId::try_new(name).expect("ID"));
        let intent = OrderIntent::new(
            domain::AccountId::new(11),
            master.order_instrument().expect("종목"),
            id.as_client_order_id().clone(),
            OrderTerms::KrMarket {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(fixtures::quantity(qty)).expect("정수"),
            },
        )
        .expect("의도");
        let draft = Proposal::new(
            id,
            intent,
            fixtures::day(60),
            range,
            ProposalLifetime::default(),
        )
        .expect("초안");
        let evidence = domain::DraftRestoreEvidence::capture(
            &draft,
            stored_est,
            master,
            ProposalLifetime::default(),
            task.task().reference(),
        )
        .expect("초안 근거");
        (draft, evidence)
    }

    #[test]
    fn memory_proposal_repository_rejects_swapped_valid_evidence_and_non_tradable_master() {
        let master = fixtures::master();
        let (draft_a, evidence_a) = kr_draft_with_evidence(&master, "draft-a", "3");
        let (_draft_b, evidence_b) = kr_draft_with_evidence(&master, "draft-b", "4");
        let repository = MemoryProposalRepository::default();
        repository.script_store_draft().push(MemoryReply::Memory);
        repository.script_store_draft().push(MemoryReply::Memory);
        repository.script_draft().push(MemoryReply::Memory);
        let integrity =
            SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
        assert_eq!(
            run(repository.store_draft(&draft_a, &evidence_b, &master)),
            Err(integrity)
        );
        assert_eq!(
            run(repository.store_draft(&draft_a, &evidence_a, &master)),
            Ok(WriteCompleted)
        );
        let mut suspended = master.row().clone();
        suspended.korean_detail = Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Suspended,
            nxt: NxtTrading::Unsupported,
        });
        let suspended_master = MasterInstrument::try_new(suspended).expect("거래정지");
        match run(repository.draft(draft_a.id(), &suspended_master)) {
            Err(error) => assert_eq!(error, integrity),
            Ok(_) => panic!("거래정지 초안 거부 필요"),
        }
    }

    fn shifted_offset_master(base: &MasterInstrument) -> MasterInstrument {
        let mut row = base.row().clone();
        let next_offset = if row.observed_at.offset_seconds() == domain::OffsetSeconds::new(0) {
            domain::OffsetSeconds::new(32_400)
        } else {
            domain::OffsetSeconds::new(0)
        };
        row.observed_at = Timestamp::from_parts(
            row.observed_at.utc_seconds(),
            row.observed_at.nanosecond(),
            next_offset,
        )
        .expect("오프셋");
        assert_ne!(
            row.observed_at.offset_seconds(),
            base.row().observed_at.offset_seconds()
        );
        MasterInstrument::try_new(row).expect("오프셋 마스터")
    }

    fn run_applied_signal(
        repo: &MemorySignalEvaluationRepository,
        events: &application::EventBus,
        id: &domain::InstrumentId,
        inputs: EvaluateSignalInputs,
        occ: &str,
    ) {
        let outcome = run(
            EvaluateSignals::new(repo, &StaticInputs(inputs), events).execute(
                id,
                fixtures::day(60),
                BuySignalOccurrenceId::try_new(occ).expect("ID"),
            ),
        )
        .expect("평가 성공");
        assert_eq!(
            match outcome {
                EvaluateSignalOutcome::Applied { reason, .. } => reason,
                EvaluateSignalOutcome::NoSubmission
                | EvaluateSignalOutcome::SubmissionDiscarded
                | EvaluateSignalOutcome::Unavailable { .. } => panic!("적용 필요"),
            },
            None
        );
    }

    #[test]
    fn delisted_holding_symbol_reuse_still_allows_signal_lifecycle_and_new_active_commits() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let repo = repository_with_submissions();
        let events = EventBusBuilder::default().build();
        let id_a = &master_a.row().instrument_id;
        run_applied_signal(
            &repo,
            &events,
            id_a,
            inputs_for(
                &master_a,
                vec![master_b.clone()],
                portfolio_holding_other(&master_b),
            ),
            "occ-reuse",
        );
        let mut delisted_b_row = master_b.row().clone();
        delisted_b_row.listing_status = ListingStatus::Delisted;
        repo.save_master(MasterInstrument::try_new(delisted_b_row).expect("폐지 B"));
        let mut reused_c_row = master_b.row().clone();
        reused_c_row.instrument_id = domain::InstrumentId::try_new("sample-c-reused").expect("C");
        let reused_c = MasterInstrument::try_new(reused_c_row).expect("재사용 C");
        repo.save_master(reused_c.clone());
        let identity_a = master_a.order_instrument().expect("종목 A");
        let snap = run(repo.snapshot(id_a)).expect("심볼 재사용 공존 조회");
        assert!(snap.current.is_some());
        assert_eq!(run(repo.current(id_a)).expect("현재").len(), 1);
        assert_eq!(
            run(repo.occurrences(id_a, &identity_a))
                .expect("이력")
                .len(),
            1
        );
        match run(repo.invalidate(id_a, &snap)).expect("무효화 허용") {
            SignalInvalidationOutcome::Applied(emitted) => assert_ne!(emitted, Vec::new()),
            SignalInvalidationOutcome::Conflict
            | SignalInvalidationOutcome::SubmissionDiscarded => panic!("무효화 적용 필요"),
        }
        assert_eq!(run(repo.snapshot(id_a)).expect("무효화 후").current, None);
        run_applied_signal(
            &repo,
            &events,
            id_a,
            inputs_for(
                &master_a,
                vec![reused_c.clone()],
                portfolio_holding_other(&reused_c),
            ),
            "occ-reuse-2",
        );
        assert_eq!(
            run(repo.occurrences(id_a, &identity_a))
                .expect("이력 2")
                .len(),
            2
        );
    }

    #[test]
    fn stale_tradable_commit_is_rejected_when_stored_master_is_non_tradable() {
        let master = fixtures::master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let decision = fixtures::decision(&master, "99", None);
        let record = buy_record(&decision, "occ-stale-nt");
        let proof = fixtures::evidence_for_decision(&decision, Some(&record)).expect("근거");
        let transition = signal_transition_from_history(None, &decision).expect("전이");
        for non_tradable in non_tradable_variants(&master) {
            let repo = repository_with_submissions();
            repo.save_master(non_tradable);
            let snap = run(repo.snapshot(&master.row().instrument_id)).expect("스냅숏");
            let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
                submission: snap.submission.clone().expect("제출"),
                decision: decision.clone(),
                transition: transition.clone(),
                calibration_head: Some(fixtures::head(0)),
                occurrence: Some(record.clone()),
                evidence: Some(proof.clone()),
            }));
            assert_eq!(
                run(repo.commit(&master.row().instrument_id, &snap, &mutation)),
                Err(invalid)
            );
        }
    }

    #[test]
    fn master_dedup_checks_offset_and_rejects_active_symbol_collision_in_repo() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let portfolio = portfolio_holding_other(&master_b);
        let diff_offset_a = shifted_offset_master(&master_a);
        assert_eq!(
            execute_expecting_unavailable(
                inputs_for(
                    &master_a,
                    vec![diff_offset_a.clone(), master_b.clone()],
                    portfolio
                ),
                "occ-off"
            ),
            EvaluateSignalError::RestorationMasters(MasterInstrumentError::DuplicateIdentity)
        );
        let repo = repository_with_submissions();
        repo.save_master(master_b.clone());
        let snap = run(repo.snapshot(&master_a.row().instrument_id)).expect("스냅숏");
        let decision = fixtures::decision(&master_a, "99", None);
        let record = buy_record(&decision, "occ-off-repo");
        let transition = signal_transition_from_history(None, &decision).expect("전이");
        let mut proof_offset =
            fixtures::evidence_for_decision(&decision, Some(&record)).expect("근거");
        proof_offset.holding_masters = vec![diff_offset_a];
        let bad_offset = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: decision.clone(),
            transition: transition.clone(),
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(record.clone()),
            evidence: Some(proof_offset),
        }));
        assert_eq!(
            run(repo.commit(&master_a.row().instrument_id, &snap, &bad_offset)),
            Err(invalid)
        );
        let mut active_collision_row = master_b.row().clone();
        active_collision_row.instrument_id =
            domain::InstrumentId::try_new("active-dup-symbol").expect("충돌 ID");
        let mut proof_collide =
            fixtures::evidence_for_decision(&decision, Some(&record)).expect("근거");
        proof_collide.holding_masters =
            vec![MasterInstrument::try_new(active_collision_row).expect("활성 충돌")];
        let bad_collide = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision,
            transition,
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(record),
            evidence: Some(proof_collide),
        }));
        assert_eq!(
            run(repo.commit(&master_a.row().instrument_id, &snap, &bad_collide)),
            Err(invalid)
        );
    }

    fn shift_ts_offset(value: Timestamp) -> Timestamp {
        let next = if value.offset_seconds() == domain::OffsetSeconds::new(0) {
            domain::OffsetSeconds::new(32_400)
        } else {
            domain::OffsetSeconds::new(0)
        };
        Timestamp::from_parts(value.utc_seconds(), value.nanosecond(), next).expect("시각 오프셋")
    }

    fn assert_commit_invalid(
        repo: &MemorySignalEvaluationRepository,
        master: &MasterInstrument,
        decision: &SignalDecision,
        record: &BuySignalRecord,
        evidence: SignalCommitEvidence,
    ) {
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let snap = run(repo.snapshot(&master.row().instrument_id)).expect("스냅숏");
        let transition = signal_transition_from_history(None, decision).expect("전이");
        let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snap.submission.clone().expect("제출"),
            decision: decision.clone(),
            transition,
            calibration_head: Some(fixtures::head(0)),
            occurrence: Some(record.clone()),
            evidence: Some(evidence),
        }));
        assert_eq!(
            run(repo.commit(&master.row().instrument_id, &snap, &mutation)),
            Err(invalid)
        );
    }

    #[test]
    fn commit_rejects_occurrence_signal_swap_offset_drift_and_wrong_master() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let decision_99 = fixtures::decision(&master_a, "99", None);
        let decision_98 = fixtures::decision(&master_a, "98", None);
        let rec_99 = buy_record(&decision_99, "occ-bind");
        let rec_98 = buy_record(&decision_98, "occ-bind");
        let base_proof =
            fixtures::evidence_for_decision(&decision_99, Some(&rec_99)).expect("근거");
        let repo = repository_with_submissions();
        repo.save_master(master_a.clone());
        let mut swapped_occ = base_proof.clone();
        swapped_occ.occurrence = fixtures::evidence_for_decision(&decision_98, Some(&rec_98))
            .expect("98 근거")
            .occurrence;
        assert_commit_invalid(&repo, &master_a, &decision_99, &rec_99, swapped_occ);
        let mut maturity_offset = base_proof.clone();
        let occ_sig = &mut maturity_offset.occurrence.as_mut().expect("이력").signal;
        occ_sig.evidence.maturity = shift_ts_offset(occ_sig.evidence.maturity);
        occ_sig.original.maturity = occ_sig.evidence.maturity;
        assert_commit_invalid(&repo, &master_a, &decision_99, &rec_99, maturity_offset);
        let buy_99 = match &decision_99 {
            SignalDecision::Buy(value) => value,
            SignalDecision::Sell(_) | SignalDecision::None(_) => panic!("매수"),
        };
        let shifted_rec = BuySignalRecord::from_signal_checked(
            BuySignalOccurrenceId::try_new("occ-bind").expect("ID"),
            buy_99,
            shift_ts_offset(fixtures::day(60)),
            &kr_calendars(),
        )
        .expect("시프트 발생");
        let mut raised_offset = base_proof.clone();
        raised_offset.occurrence = Some(
            domain::StoredSignalOccurrence::capture(
                &shifted_rec,
                base_proof.signal.clone(),
                &master_a,
            )
            .expect("시프트 근거"),
        );
        assert_commit_invalid(&repo, &master_a, &decision_99, &rec_99, raised_offset);
        let mut wrong_master = base_proof;
        wrong_master.master = master_b;
        assert_commit_invalid(&repo, &master_a, &decision_99, &rec_99, wrong_master);
    }

    #[test]
    fn commit_rejects_scheduled_symbol_collision_and_tampered_original_or_expected() {
        let master_a = fixtures::master();
        let master_b = fixtures::other_master();
        let decision = fixtures::decision(&master_a, "99", None);
        let record = buy_record(&decision, "occ-m5-m10");
        let base_proof = fixtures::evidence_for_decision(&decision, Some(&record)).expect("근거");
        let repo = repository_with_submissions();
        let mut scheduled_b_row = master_b.row().clone();
        scheduled_b_row.listing_status = ListingStatus::Scheduled;
        repo.save_master(MasterInstrument::try_new(scheduled_b_row).expect("상장예정 B"));
        let mut dup_active_row = master_b.row().clone();
        dup_active_row.instrument_id =
            domain::InstrumentId::try_new("active-vs-sched").expect("ID");
        let mut sched_collide = base_proof.clone();
        sched_collide.holding_masters =
            vec![MasterInstrument::try_new(dup_active_row).expect("활성 충돌")];
        assert_commit_invalid(&repo, &master_a, &decision, &record, sched_collide);
        let mut tampered_original = base_proof.clone();
        tampered_original.signal.original.maturity =
            shift_ts_offset(tampered_original.signal.original.maturity);
        assert_commit_invalid(&repo, &master_a, &decision, &record, tampered_original);
        let mut tampered_expected = base_proof;
        tampered_expected.signal.expected.probability =
            domain::AiProbability::parse(&domain::AiProbabilityInput::try_new("91").expect("입력"))
                .expect("확률")
                .into_probability();
        assert_commit_invalid(&repo, &master_a, &decision, &record, tampered_expected);
    }

    #[test]
    fn sell_commit_without_occurrence_rejects_tampered_original_or_expected() {
        let master = fixtures::master();
        let invalid = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let repo = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repo.advance_calibration_head(fixtures::head(0))
            .expect("보정");
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(&repo, &events).execute(
            &fixtures::run_id(),
            vec![fixtures::input("task-old", "10", &master)],
        ))
        .expect("제출");
        let snap = run(repo.snapshot(&master.row().instrument_id)).expect("스냅숏");
        let sell = fixtures::decision(&master, "10", Some("5"));
        let transition = signal_transition_from_history(None, &sell).expect("전이");
        let base_proof = fixtures::evidence_for_decision(&sell, None).expect("매도 근거");
        let mut tampered_original = base_proof.clone();
        tampered_original.signal.original.maturity =
            shift_ts_offset(tampered_original.signal.original.maturity);
        let mut tampered_expected = base_proof;
        tampered_expected.signal.expected.probability =
            domain::AiProbability::parse(&domain::AiProbabilityInput::try_new("15").expect("입력"))
                .expect("확률")
                .into_probability();
        let assert_rejected = |proof: SignalCommitEvidence| {
            let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
                submission: snap.submission.clone().expect("제출"),
                decision: sell.clone(),
                transition: transition.clone(),
                calibration_head: Some(fixtures::head(0)),
                occurrence: None,
                evidence: Some(proof),
            }));
            assert_eq!(
                run(repo.commit(&master.row().instrument_id, &snap, &mutation)),
                Err(invalid)
            );
        };
        assert_rejected(tampered_original);
        assert_rejected(tampered_expected);
    }
}
