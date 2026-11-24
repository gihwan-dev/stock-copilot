#[cfg(test)]
mod tests {
    use application::{
        AiEvent, BriefingContext, BriefingContextSource, BriefingFailure, BriefingOutcome,
        BriefingRecord, BriefingRepository, BriefingRunError, BriefingSource, CollectionQuery,
        EventBus, EventBusBuilder, EventSubscriber, InformationError, IsolationCheck,
        IsolationFailure, ResetTime, RunBriefing, RunInput, RunKind, RunPurpose, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError, WriteCompleted,
    };
    use domain::{
        BriefingId, BriefingKind, CalendarDate, DomainEvent, DomainEventKind, InformationSource,
        Nanosecond, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiEventStream, ScriptedAiProvider};

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

    fn kst_0820() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_795_995_600),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn briefing_date() -> CalendarDate {
        kst_0820()
            .date_at_offset(OffsetSeconds::new(32_400))
            .expect("날짜")
    }

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("텍스트")
    }

    fn passed_then(events: Vec<AiEvent>) -> ScriptedAiEventStream {
        let mut all = vec![AiEvent::IsolationChecked(IsolationCheck::Passed)];
        all.extend(events);
        ScriptedAiEventStream::from_events(all)
    }

    #[derive(Default)]
    struct MemoryBriefings {
        saved: RwLock<Vec<BriefingRecord>>,
        failure: Option<SystemPortError>,
    }

    impl BriefingRepository for MemoryBriefings {
        fn save(
            &self,
            record: &BriefingRecord,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let result = if let Some(error) = self.failure {
                Err(error)
            } else {
                let mut saved = self.saved.write().expect("기록");
                saved.retain(|existing| existing.id != record.id);
                saved.push(record.clone());
                Ok(WriteCompleted)
            };
            std::future::ready(result)
        }

        fn find(
            &self,
            id: &BriefingId,
        ) -> impl Future<Output = Result<Option<BriefingRecord>, SystemPortError>> + Send {
            std::future::ready(Ok(self
                .saved
                .read()
                .expect("기록")
                .iter()
                .find(|record| record.id == *id)
                .cloned()))
        }

        fn recent(
            &self,
        ) -> impl Future<Output = Result<Vec<BriefingRecord>, SystemPortError>> + Send {
            std::future::ready(Ok(self.saved.read().expect("기록").clone()))
        }
    }

    struct FixedContext(Result<BriefingContext, SystemPortError>);

    impl BriefingContextSource for FixedContext {
        fn context(
            &self,
            _kind: BriefingKind,
            _now: Timestamp,
        ) -> impl Future<Output = Result<BriefingContext, SystemPortError>> + Send {
            std::future::ready(self.0.clone())
        }
    }

    fn news_source() -> BriefingSource {
        BriefingSource {
            title: text("삼성전자 3분기 잠정실적 발표"),
            url: None,
            origin: InformationSource::Web,
            observed_at: kst_0820(),
        }
    }

    fn samsung() -> Symbol {
        Symbol::try_new("005930").expect("종목")
    }

    fn holding_context() -> FixedContext {
        let instruments = vec![samsung()];
        FixedContext(Ok(BriefingContext {
            instruments,
            evidence: vec![news_source()],
            code_facts: vec![text("원달러 1,380원")],
        }))
    }

    struct Recorder(Arc<RwLock<Vec<DomainEvent>>>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut guard) => guard.push(event.clone()),
                Err(poisoned) => poisoned.into_inner().push(event.clone()),
            }
            Ok(())
        }
    }

    fn recording_bus() -> (EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::BriefingFinished]),
            Recorder(Arc::clone(&events)),
        );
        (builder.build(), events)
    }

    fn assert_collection_call(call: &AiProviderCall) {
        let AiProviderCall::Run(spec) = call else {
            panic!("수집 실행이 아님: {call:?}");
        };
        assert_eq!(spec.kind(), RunKind::Collection);
        assert_eq!(spec.purpose(), RunPurpose::WebCollection);
        let RunInput::Collection(CollectionQuery::CodeTopics { instruments, .. }) = spec.input()
        else {
            panic!("코드 주제가 아님");
        };
        assert_eq!(instruments, &vec![samsung()]);
    }

    fn assert_analysis_call(call: &AiProviderCall) {
        let AiProviderCall::Run(spec) = call else {
            panic!("분석 실행이 아님: {call:?}");
        };
        assert_eq!(spec.kind(), RunKind::Analysis);
        assert_eq!(spec.purpose(), RunPurpose::MorningBriefing);
        let RunInput::Analysis(analysis) = spec.input() else {
            panic!("분석 입력이 아님");
        };
        let prompt = analysis.as_str();
        assert!(prompt.contains("<untrusted_text>"));
        assert!(prompt.contains("[1] (web) 삼성전자 3분기 잠정실적 발표"));
        assert!(prompt.contains("[2] (web) 웹: 삼성전자 신제품 공개"));
        assert!(prompt.contains("원달러 1,380원"));
    }

    #[test]
    fn morning_briefing_collects_then_analyzes_and_announces_saved_record() {
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::TextDelta(text(
                "웹: 삼성전자 신제품 공개",
            ))])));
        provider.script_run().push(Ok(passed_then(vec![
            AiEvent::TextDelta(text("삼성전자가 잠정실적을 냈습니다 [1]. ")),
            AiEvent::TextDelta(text("신제품을 공개했습니다 [2].")),
        ])));
        let repository = MemoryBriefings::default();
        let context = holding_context();
        let clock = SteppedClock::new(kst_0820());
        let (events, received) = recording_bus();
        let report = run(RunBriefing {
            ai: &provider,
            repository: &repository,
            context: &context,
            clock: &clock,
            events: &events,
        }
        .execute(BriefingKind::Morning))
        .expect("브리핑");
        let id = BriefingId::new(BriefingKind::Morning, briefing_date());
        assert_eq!(report.record.id, id);
        match &report.record.outcome {
            BriefingOutcome::Completed { body, sources } => {
                assert_eq!(
                    body.as_untrusted_str(),
                    "삼성전자가 잠정실적을 냈습니다 [1]. 신제품을 공개했습니다 [2]."
                );
                assert_eq!(sources.len(), 2);
                assert_eq!(sources[1].origin, InformationSource::Web);
                assert_eq!(
                    sources[1].title.as_untrusted_str(),
                    "웹: 삼성전자 신제품 공개"
                );
            }
            BriefingOutcome::Failed(failure) => panic!("실패: {failure:?}"),
        }
        let calls = provider.calls();
        assert_eq!(calls.len(), 2);
        assert_collection_call(&calls[0]);
        assert_analysis_call(&calls[1]);
        assert_eq!(
            run(repository.find(&id)).expect("조회"),
            Some(report.record)
        );
        assert_eq!(
            *received.read().expect("이벤트"),
            vec![DomainEvent::BriefingFinished(id)]
        );
    }

    #[test]
    fn collection_failure_still_analyzes_with_code_evidence() {
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Err(InformationError::Unavailable(ResetTime::NotProvided)));
        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::TextDelta(text("요약 [1]"))])));
        let repository = MemoryBriefings::default();
        let context = holding_context();
        let clock = SteppedClock::new(kst_0820());
        let report = run(RunBriefing {
            ai: &provider,
            repository: &repository,
            context: &context,
            clock: &clock,
            events: &EventBus::default(),
        }
        .execute(BriefingKind::PreUsMarket))
        .expect("점검");
        match report.record.outcome {
            BriefingOutcome::Completed { sources, .. } => assert_eq!(sources, vec![news_source()]),
            BriefingOutcome::Failed(failure) => panic!("실패: {failure:?}"),
        }
    }

    #[test]
    fn closing_review_skips_collection_and_empty_analysis_is_saved_as_failure_without_event() {
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::TextDelta(text("  "))])));
        let repository = MemoryBriefings::default();
        let context = holding_context();
        let clock = SteppedClock::new(kst_0820());
        let (events, received) = recording_bus();
        let report = run(RunBriefing {
            ai: &provider,
            repository: &repository,
            context: &context,
            clock: &clock,
            events: &events,
        }
        .execute(BriefingKind::ClosingReview))
        .expect("마감 리뷰");
        assert_eq!(
            report.record.outcome,
            BriefingOutcome::Failed(BriefingFailure::EmptyResult)
        );
        assert_eq!(report.delivery, None);
        assert_eq!(provider.calls().len(), 1);
        assert_eq!(received.read().expect("이벤트").len(), 0);
        assert_eq!(repository.saved.read().expect("기록").len(), 1);
    }

    #[test]
    fn isolation_rejection_and_rate_limit_are_recorded_failures() {
        for (events, expected) in [
            (
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    IsolationFailure::Tools,
                ))],
                BriefingFailure::IsolationRejected,
            ),
            (
                vec![AiEvent::RateLimited {
                    resets_at: ResetTime::NotProvided,
                }],
                BriefingFailure::RateLimited,
            ),
        ] {
            let provider = ScriptedAiProvider::default();
            provider
                .script_run()
                .push(Ok(ScriptedAiEventStream::from_events(events)));
            let repository = MemoryBriefings::default();
            let context = holding_context();
            let clock = SteppedClock::new(kst_0820());
            let report = run(RunBriefing {
                ai: &provider,
                repository: &repository,
                context: &context,
                clock: &clock,
                events: &EventBus::default(),
            }
            .execute(BriefingKind::ClosingReview))
            .expect("기록");
            assert_eq!(report.record.outcome, BriefingOutcome::Failed(expected));
        }
    }

    #[test]
    fn context_and_storage_failures_are_errors_and_nothing_is_announced() {
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let provider = ScriptedAiProvider::default();
        let repository = MemoryBriefings::default();
        let clock = SteppedClock::new(kst_0820());
        let (events, received) = recording_bus();
        let broken = FixedContext(Err(error));
        let context_failure = run(RunBriefing {
            ai: &provider,
            repository: &repository,
            context: &broken,
            clock: &clock,
            events: &events,
        }
        .execute(BriefingKind::Morning));
        assert_eq!(context_failure, Err(BriefingRunError::Context(error)));
        assert_eq!(provider.calls().len(), 0);

        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::TextDelta(text("요약"))])));
        let failing = MemoryBriefings {
            saved: RwLock::new(Vec::new()),
            failure: Some(error),
        };
        let empty = FixedContext(Ok(BriefingContext::default()));
        let storage_failure = run(RunBriefing {
            ai: &provider,
            repository: &failing,
            context: &empty,
            clock: &clock,
            events: &events,
        }
        .execute(BriefingKind::Morning));
        assert_eq!(storage_failure, Err(BriefingRunError::Storage(error)));
        assert_eq!(provider.calls().len(), 1);
        assert_eq!(received.read().expect("이벤트").len(), 0);
    }
}
