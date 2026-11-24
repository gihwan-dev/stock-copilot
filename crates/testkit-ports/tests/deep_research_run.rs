#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AiRunId, AnalysisFailure, DeepResearchError, DeepResearchRun, DeepResearchStages,
        EstimateCommitOutcome, EstimateSubmissionRepository, EventBusBuilder, FinishResearch,
        FinishResearchError, GatheredEvidence, ReasoningEffort, ResearchCaseError, ResearchCases,
        ResearchCheckpoint, ResearchCheckpointStore, ResearchConclusion, ResearchOutcome,
        ResearchPrincipleCheck, ResearchStage, ResearchWebOutcome, RunDeepResearch, SubmitEstimate,
        SystemFailure, SystemPort, SystemPortError, ThesisRepository, WriteCompleted,
    };
    use domain::{
        ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimSupport, EstimateAssignments,
        EstimateSubmissionRecord, Evidence, EvidenceBundle, EvidenceItem, EvidenceItemId,
        EvidenceKind, EvidenceRef, EvidenceUrl, InstrumentId, Nanosecond, NormalizedUrl,
        OffsetSeconds, PolicyViolation, ProposedChange, ResearchClaim, ResearchClaimText,
        ResearchDraft, ResearchSourceWarning, ResearchStance, SectorId, Thesis, ThesisOperation,
        ThesisPatch, ThesisStatus, Timestamp, UnixSeconds, UntrustedText, VerifiedClaim,
    };
    use testkit_ports::Script;

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
    fn id(value: &str) -> EvidenceItemId {
        EvidenceItemId::try_new(value).expect("ID")
    }
    fn instrument() -> InstrumentId {
        InstrumentId::try_new("A005930").expect("종목")
    }
    fn ledger() -> EvidenceBundle {
        let at = Timestamp::from_parts(
            UnixSeconds::new(10),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let item = |value: &str, kind| {
            EvidenceItem::new(
                id(value),
                kind,
                at,
                UntrustedText::try_new(value).expect("내용"),
            )
        };
        EvidenceBundle::try_new(
            instrument(),
            vec![
                item("quote-1", EvidenceKind::Quote),
                item("web-1", EvidenceKind::Web),
            ],
        )
        .expect("묶음")
    }
    fn gathered() -> GatheredEvidence {
        GatheredEvidence {
            bundle: ledger(),
            web: ResearchWebOutcome::Collected,
        }
    }
    fn claim(cites: &[&str]) -> ResearchClaim {
        ResearchClaim::try_new(
            ResearchClaimText::try_new("주장").expect("문장"),
            cites.iter().map(|value| id(value)).collect(),
        )
        .expect("주장")
    }
    fn cases() -> ResearchCases {
        ResearchCases {
            bull: ResearchDraft::try_new(ResearchStance::Bull, vec![claim(&["quote-1"])])
                .expect("긍정"),
            bear: ResearchDraft::try_new(ResearchStance::Bear, vec![claim(&["web-1"])])
                .expect("부정"),
        }
    }
    fn verified() -> Vec<VerifiedClaim> {
        vec![
            VerifiedClaim::checked(
                ResearchStance::Bull,
                claim(&["quote-1"]),
                ClaimSupport::Supported,
            ),
            VerifiedClaim::checked(
                ResearchStance::Bear,
                claim(&["web-1"]),
                ClaimSupport::Supported,
            ),
        ]
    }
    fn web_change() -> ProposedChange {
        let text = |value| UntrustedText::try_new(value).expect("텍스트");
        ProposedChange::new(
            ThesisOperation::AddClaim(Claim::new(
                ClaimId::try_new("claim-a").expect("논거 ID"),
                ClaimRole::Supporting,
                ClaimContent::try_new(
                    text("매출"),
                    text("성장"),
                    text("상한"),
                    text("공시"),
                    text("반증"),
                )
                .expect("논거"),
            )),
            Evidence::try_new(vec![EvidenceRef::Web(
                EvidenceUrl::try_new(
                    NormalizedUrl::try_new("https://example.com/a").expect("주소"),
                )
                .expect("근거"),
            )])
            .expect("증거"),
            ChangeReason::try_new(text("근거"), text("model"), text("v1")).expect("이유"),
        )
    }
    fn conclusion() -> ResearchConclusion {
        ResearchConclusion {
            thesis: Some(ThesisPatch::try_new(vec![web_change()]).expect("패치")),
            estimates: Vec::new(),
        }
    }
    fn outcome() -> ResearchOutcome {
        ResearchOutcome {
            tally: domain::tally_research_claims(&verified()),
            source_warnings: Vec::new(),
            principle_violations: Vec::new(),
            thesis_status: ThesisStatus::NoBreakSignal,
            pending_changes: domain::ResearchCount::of_pending(&Thesis::new()),
            deliveries: Vec::new(),
        }
    }
    fn run_id() -> AiRunId {
        AiRunId::try_new("run-1").expect("실행")
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Called {
        Gather,
        Cases,
        Verify,
        Conclude,
        Finish,
    }
    #[derive(Default)]
    struct Stages {
        calls: Script<Called, ()>,
        gather: Script<(), Result<GatheredEvidence, DeepResearchError>>,
        cases: Script<(), Result<ResearchCases, DeepResearchError>>,
        verify: Script<(), Result<Vec<VerifiedClaim>, DeepResearchError>>,
        conclude: Script<(), Result<ResearchConclusion, DeepResearchError>>,
        finish: Script<(), Result<ResearchOutcome, DeepResearchError>>,
    }
    impl Stages {
        fn happy() -> Self {
            let stages = Self::default();
            for _ in 0_u8..5 {
                stages.calls.push(());
            }
            stages.gather.push(Ok(gathered()));
            stages.cases.push(Ok(cases()));
            stages.verify.push(Ok(verified()));
            stages.conclude.push(Ok(conclusion()));
            stages.finish.push(Ok(outcome()));
            stages
        }
    }
    impl DeepResearchStages for Stages {
        async fn gather(&self) -> Result<GatheredEvidence, DeepResearchError> {
            self.calls.run(Called::Gather).await;
            self.gather.run(()).await
        }
        async fn cases(
            &self,
            _ledger: &EvidenceBundle,
        ) -> Result<ResearchCases, DeepResearchError> {
            self.calls.run(Called::Cases).await;
            self.cases.run(()).await
        }
        async fn verify(
            &self,
            _ledger: &EvidenceBundle,
            _cases: &ResearchCases,
        ) -> Result<Vec<VerifiedClaim>, DeepResearchError> {
            self.calls.run(Called::Verify).await;
            self.verify.run(()).await
        }
        async fn conclude(
            &self,
            _ledger: &EvidenceBundle,
            _verified: &[VerifiedClaim],
        ) -> Result<ResearchConclusion, DeepResearchError> {
            self.calls.run(Called::Conclude).await;
            self.conclude.run(()).await
        }
        async fn finish(
            &self,
            _run: &AiRunId,
            _ledger: &EvidenceBundle,
            _verified: &[VerifiedClaim],
            _conclusion: &ResearchConclusion,
        ) -> Result<ResearchOutcome, DeepResearchError> {
            self.calls.run(Called::Finish).await;
            self.finish.run(()).await
        }
    }

    #[derive(Default)]
    struct Checkpoints {
        load: Script<AiRunId, Result<Option<ResearchCheckpoint>, SystemPortError>>,
        save: Script<(AiRunId, ResearchCheckpoint), Result<WriteCompleted, SystemPortError>>,
    }
    impl ResearchCheckpointStore for Checkpoints {
        async fn load(&self, run: &AiRunId) -> Result<Option<ResearchCheckpoint>, SystemPortError> {
            self.load.run(run.clone()).await
        }
        async fn save(
            &self,
            run: &AiRunId,
            checkpoint: &ResearchCheckpoint,
        ) -> Result<WriteCompleted, SystemPortError> {
            self.save.run((run.clone(), checkpoint.clone())).await
        }
    }
    impl Checkpoints {
        fn saved(&self) -> Vec<ResearchCheckpoint> {
            self.save
                .calls()
                .into_iter()
                .map(|(_, checkpoint)| checkpoint)
                .collect()
        }
    }

    fn execute(
        stages: &Stages,
        checkpoints: &Checkpoints,
    ) -> Result<DeepResearchRun, DeepResearchError> {
        run(RunDeepResearch {
            stages,
            checkpoints,
        }
        .execute(&run_id()))
    }

    #[test]
    fn every_stage_boundary_is_saved_in_order() {
        let stages = Stages::happy();
        let checkpoints = Checkpoints::default();
        checkpoints.load.push(Ok(None));
        for _ in 0_u8..5 {
            checkpoints.save.push(Ok(WriteCompleted));
        }
        assert_eq!(
            execute(&stages, &checkpoints),
            Ok(DeepResearchRun::Finished(outcome()))
        );
        assert_eq!(
            stages.calls.calls(),
            vec![
                Called::Gather,
                Called::Cases,
                Called::Verify,
                Called::Conclude,
                Called::Finish
            ]
        );
        assert_eq!(
            checkpoints.saved(),
            vec![
                ResearchCheckpoint::Gathered(gathered()),
                ResearchCheckpoint::Cased(gathered(), cases()),
                ResearchCheckpoint::Verified(gathered(), verified()),
                ResearchCheckpoint::Concluded(gathered(), verified(), conclusion()),
                ResearchCheckpoint::Finished(outcome()),
            ]
        );
    }

    #[test]
    fn chat_interruption_pauses_and_the_next_run_resumes_from_the_saved_stage() {
        let stages = Stages::default();
        for _ in 0_u8..6 {
            stages.calls.push(());
        }
        stages.gather.push(Ok(gathered()));
        stages.cases.push(Ok(cases()));
        stages
            .verify
            .push(Err(DeepResearchError::Verify(AnalysisFailure::Interrupted)));
        let checkpoints = Checkpoints::default();
        checkpoints.load.push(Ok(None));
        checkpoints.save.push(Ok(WriteCompleted));
        checkpoints.save.push(Ok(WriteCompleted));
        assert_eq!(
            execute(&stages, &checkpoints),
            Ok(DeepResearchRun::Paused(ResearchStage::Verify))
        );
        checkpoints
            .load
            .push(Ok(Some(ResearchCheckpoint::Cased(gathered(), cases()))));
        stages.verify.push(Ok(verified()));
        stages.conclude.push(Ok(conclusion()));
        stages.finish.push(Ok(outcome()));
        for _ in 0_u8..3 {
            checkpoints.save.push(Ok(WriteCompleted));
        }
        assert_eq!(
            execute(&stages, &checkpoints),
            Ok(DeepResearchRun::Finished(outcome()))
        );
        assert_eq!(
            stages.calls.calls(),
            vec![
                Called::Gather,
                Called::Cases,
                Called::Verify,
                Called::Verify,
                Called::Conclude,
                Called::Finish
            ]
        );
        assert_eq!(checkpoints.saved().len(), 5);
    }

    #[test]
    fn draft_and_conclusion_interruptions_name_their_stage_and_other_errors_fail() {
        for (error, stage) in [
            (
                DeepResearchError::Cases(ResearchCaseError::Analysis(AnalysisFailure::Interrupted)),
                ResearchStage::Draft,
            ),
            (
                DeepResearchError::Conclude(AnalysisFailure::Interrupted),
                ResearchStage::Conclude,
            ),
        ] {
            let stages = Stages::default();
            stages.calls.push(());
            let checkpoints = Checkpoints::default();
            match stage {
                ResearchStage::Draft => {
                    stages.cases.push(Err(error));
                    checkpoints
                        .load
                        .push(Ok(Some(ResearchCheckpoint::Gathered(gathered()))));
                }
                ResearchStage::Verify | ResearchStage::Conclude => {
                    stages.conclude.push(Err(error));
                    checkpoints.load.push(Ok(Some(ResearchCheckpoint::Verified(
                        gathered(),
                        verified(),
                    ))));
                }
            }
            assert_eq!(
                execute(&stages, &checkpoints),
                Ok(DeepResearchRun::Paused(stage))
            );
            assert_eq!(checkpoints.saved().len(), 0);
        }
        let failing = Stages::default();
        failing.calls.push(());
        failing
            .verify
            .push(Err(DeepResearchError::Verify(AnalysisFailure::Rejected)));
        let checkpoints = Checkpoints::default();
        checkpoints
            .load
            .push(Ok(Some(ResearchCheckpoint::Cased(gathered(), cases()))));
        assert_eq!(
            execute(&failing, &checkpoints),
            Err(DeepResearchError::Verify(AnalysisFailure::Rejected))
        );
    }

    #[test]
    fn finished_runs_are_not_repeated_and_checkpoint_errors_stop() {
        let stages = Stages::default();
        let checkpoints = Checkpoints::default();
        checkpoints
            .load
            .push(Ok(Some(ResearchCheckpoint::Finished(outcome()))));
        assert_eq!(
            execute(&stages, &checkpoints),
            Ok(DeepResearchRun::Finished(outcome()))
        );
        assert_eq!(stages.calls.calls().len(), 0);
        let storage = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let broken = Checkpoints::default();
        broken.load.push(Err(storage));
        assert_eq!(
            execute(&stages, &broken),
            Err(DeepResearchError::Checkpoint(storage))
        );
        let unsaved = Stages::happy();
        let save_fails = Checkpoints::default();
        save_fails.load.push(Ok(None));
        save_fails.save.push(Err(storage));
        assert_eq!(
            execute(&unsaved, &save_fails),
            Err(DeepResearchError::Checkpoint(storage))
        );
        assert_eq!(unsaved.calls.calls(), vec![Called::Gather]);
    }

    #[derive(Default)]
    struct Theses {
        load: Script<InstrumentId, Result<Option<Thesis>, SystemPortError>>,
        store: Script<(InstrumentId, ThesisStatus), Result<WriteCompleted, SystemPortError>>,
    }
    impl ThesisRepository for Theses {
        async fn store(
            &self,
            instrument: &InstrumentId,
            thesis: &Thesis,
        ) -> Result<WriteCompleted, SystemPortError> {
            self.store.run((instrument.clone(), thesis.status())).await
        }
        async fn load(&self, instrument: &InstrumentId) -> Result<Option<Thesis>, SystemPortError> {
            self.load.run(instrument.clone()).await
        }
    }
    type Records = Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>;
    #[derive(Default)]
    struct NoEstimates {
        records: Script<InstrumentId, Result<Records, SystemPortError>>,
        assignments: Script<(), Result<EstimateAssignments<AiRunId>, SystemPortError>>,
        commits: Script<(), Result<EstimateCommitOutcome, SystemPortError>>,
    }
    impl EstimateSubmissionRepository for NoEstimates {
        async fn submission_records(
            &self,
            instrument: &InstrumentId,
        ) -> Result<Records, SystemPortError> {
            self.records.run(instrument.clone()).await
        }
        async fn assignments(&self) -> Result<EstimateAssignments<AiRunId>, SystemPortError> {
            self.assignments.run(()).await
        }
        async fn commit_submission(
            &self,
            _previous: &EstimateAssignments<AiRunId>,
            _next: &EstimateAssignments<AiRunId>,
            _records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
        ) -> Result<EstimateCommitOutcome, SystemPortError> {
            self.commits.run(()).await
        }
    }
    #[derive(Default)]
    struct Principles(Script<InstrumentId, Result<Vec<PolicyViolation>, SystemPortError>>);
    impl ResearchPrincipleCheck for Principles {
        async fn violations(
            &self,
            instrument: &InstrumentId,
        ) -> Result<Vec<PolicyViolation>, SystemPortError> {
            self.0.run(instrument.clone()).await
        }
    }

    fn finish(
        theses: &Theses,
        principles: &Principles,
        conclusion: &ResearchConclusion,
    ) -> Result<ResearchOutcome, FinishResearchError> {
        let events = EventBusBuilder::default().build();
        let repository = NoEstimates::default();
        let estimates = SubmitEstimate::new(&repository, &events);
        run(FinishResearch {
            theses,
            estimates: &estimates,
            principles,
        }
        .execute(&run_id(), &ledger(), &verified(), conclusion))
    }

    #[test]
    fn conclusion_updates_the_note_and_aggregates_warnings_and_principles() {
        let theses = Theses::default();
        theses.load.push(Ok(None));
        theses.store.push(Ok(WriteCompleted));
        let principles = Principles::default();
        let sector = PolicyViolation::SectorCap(SectorId::try_new("semis").expect("섹터"));
        principles.0.push(Ok(vec![sector.clone()]));
        let outcome = finish(&theses, &principles, &conclusion()).expect("마무리");
        assert_eq!(outcome.principle_violations, vec![sector]);
        assert_eq!(*outcome.tally.supported.as_u16(), 2);
        assert_eq!(
            outcome.source_warnings,
            vec![ResearchSourceWarning::WebOnlySupport(
                verified().get(1).expect("웹 주장").clone()
            )]
        );
        assert_eq!(*outcome.pending_changes.as_u16(), 1);
        assert_eq!(outcome.thesis_status, ThesisStatus::NoBreakSignal);
        assert_eq!(outcome.deliveries.len(), 0);
        assert_eq!(
            theses.store.calls(),
            vec![(instrument(), ThesisStatus::NoBreakSignal)]
        );
        assert_eq!(principles.0.calls(), vec![instrument()]);
    }

    #[test]
    fn no_note_change_skips_storage_and_limits_and_errors_stop_finishing() {
        let theses = Theses::default();
        theses.load.push(Ok(Some(Thesis::new())));
        let principles = Principles::default();
        principles.0.push(Ok(Vec::new()));
        let empty = ResearchConclusion {
            thesis: None,
            estimates: Vec::new(),
        };
        let outcome = finish(&theses, &principles, &empty).expect("마무리");
        assert_eq!(*outcome.pending_changes.as_u16(), 0);
        assert_eq!(theses.store.calls().len(), 0);

        let storage = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let failing = Principles::default();
        failing.0.push(Err(storage));
        assert_eq!(
            finish(&Theses::default(), &failing, &empty),
            Err(FinishResearchError::Principles(storage))
        );
        let ok = Principles::default();
        ok.0.push(Ok(Vec::new()));
        let unreadable = Theses::default();
        unreadable.load.push(Err(storage));
        assert_eq!(
            finish(&unreadable, &ok, &empty),
            Err(FinishResearchError::ThesisStorage(storage))
        );
    }
}
