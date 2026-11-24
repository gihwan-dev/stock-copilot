#[cfg(test)]
mod tests {
    use application::{
        AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRun, AiJobTiming, AiRunId,
        EventBusBuilder, EventSubscriber, RecordAiJobFinished, RunKind, RunPurpose,
        SubscriberFailure, SystemFailure, SystemPort, SystemPortError, TokenUsage,
    };
    use domain::{
        AiJobId, DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        sync::{Arc, mpsc},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::MemoryAiJobRecords;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn job(value: &str) -> AiJobId {
        AiJobId::try_new(value).expect("실행 ID")
    }
    fn record(id: &str, outcome: AiJobOutcome) -> AiJobFinishRecord {
        AiJobFinishRecord::new(
            AiJobRun::new(job(id), RunKind::Analysis, RunPurpose::Chat),
            outcome,
            AiJobTiming::try_new(at(90), at(100)).expect("시간"),
            TokenUsage::zero(),
        )
    }
    struct Observer {
        records: Arc<MemoryAiJobRecords>,
        events: mpsc::Sender<(DomainEvent, Option<AiJobFinishRecord>)>,
    }
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            let DomainEvent::AiJobFinished(id) = event else {
                return Err(SubscriberFailure::Unavailable);
            };
            let stored =
                run(self.records.finished(id)).map_err(|_e| SubscriberFailure::Unavailable)?;
            self.events
                .send((event.clone(), stored))
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }
    fn bus(
        records: &Arc<MemoryAiJobRecords>,
    ) -> (
        application::EventBus,
        mpsc::Receiver<(DomainEvent, Option<AiJobFinishRecord>)>,
    ) {
        let (sender, received) = mpsc::channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::AiJobFinished).collect(),
            Observer {
                records: Arc::clone(records),
                events: sender,
            },
        );
        (builder.build(), received)
    }

    #[test]
    fn event_carries_job_id_and_subscriber_reads_stored_record() {
        let records = Arc::new(MemoryAiJobRecords::default());
        let (events, received) = bus(&records);
        let finished = record("chat-1", AiJobOutcome::Completed);
        let delivery =
            run(RecordAiJobFinished::new(records.as_ref(), &events).execute(finished.clone()))
                .expect("저장");
        assert_eq!(delivery.failures.len(), 0);
        assert_eq!(
            received.try_recv().expect("이벤트"),
            (DomainEvent::AiJobFinished(job("chat-1")), Some(finished)),
            "구독자는 이벤트 시점에 이미 저장된 기록을 읽는다"
        );
        assert!(received.try_recv().is_err(), "이벤트는 한 번");
    }

    #[test]
    fn concurrent_jobs_are_distinguished_by_id() {
        let records = Arc::new(MemoryAiJobRecords::default());
        let (events, received) = bus(&records);
        let chat = record("chat-1", AiJobOutcome::Completed);
        let scheduled = AiJobFinishRecord::new(
            AiJobRun::new(
                job("sched-1"),
                RunKind::Collection,
                RunPurpose::WebCollection,
            ),
            AiJobOutcome::Interrupted,
            AiJobTiming::try_new(at(110), at(120)).expect("시간"),
            TokenUsage::zero(),
        );
        let usecase = RecordAiJobFinished::new(records.as_ref(), &events);
        run(usecase.execute(scheduled.clone())).expect("예약 저장");
        run(usecase.execute(chat.clone())).expect("채팅 저장");
        assert_eq!(
            received.try_recv().expect("첫 이벤트").1,
            Some(scheduled.clone())
        );
        assert_eq!(received.try_recv().expect("둘째 이벤트").1, Some(chat));
        assert_eq!(run(records.finished(&job("sched-1"))), Ok(Some(scheduled)));
        assert_eq!(run(records.finished(&job("missing"))), Ok(None));
    }

    #[test]
    fn storage_failure_and_duplicate_publish_nothing() {
        let records = Arc::new(MemoryAiJobRecords::default());
        let (events, received) = bus(&records);
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        records.fail_next_store(failure);
        let usecase = RecordAiJobFinished::new(records.as_ref(), &events);
        assert_eq!(
            run(usecase.execute(record("chat-1", AiJobOutcome::Failed))),
            Err(failure)
        );
        assert!(received.try_recv().is_err(), "저장 실패면 이벤트 없음");
        assert_eq!(records.records(), vec![]);
        run(usecase.execute(record("chat-1", AiJobOutcome::Completed))).expect("저장");
        let _first = received.try_recv().expect("첫 저장 이벤트");
        let repeated = run(usecase.execute(record("chat-1", AiJobOutcome::Failed))).expect("중복");
        assert_eq!(repeated.failures.len(), 0);
        assert!(
            received.try_recv().is_err(),
            "이미 있는 기록이면 이벤트 없음"
        );
        assert_eq!(
            records.records(),
            vec![record("chat-1", AiJobOutcome::Completed)]
        );
    }

    #[test]
    fn run_id_maps_to_the_same_job_id() {
        let run_id = AiRunId::try_new("sched-7.a_b").expect("실행 ID");
        assert_eq!(run_id.job_id(), Ok(job("sched-7.a_b")));
        assert_eq!(run_id.as_str(), "sched-7.a_b");
        let spec = application::RunSpec::new(
            run_id,
            application::RunInput::Analysis(
                application::AnalysisPrompt::try_new("brief").expect("prompt"),
            ),
            application::ModelSelection::Default,
            application::ReasoningEffort::Default,
            RunPurpose::MorningBriefing,
        );
        assert_eq!(
            AiJobRun::of_spec(&spec),
            Ok(AiJobRun::new(
                job("sched-7.a_b"),
                RunKind::Analysis,
                RunPurpose::MorningBriefing
            ))
        );
        assert_eq!(
            AiRunId::try_new("a/b"),
            Err(application::IdentifierError::InvalidFormat)
        );
        assert_eq!(
            AiRunId::try_new(&"a".repeat(129)),
            Err(application::IdentifierError::InvalidFormat)
        );
    }
}
