#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        GatherEvidenceError, GatherResearchEvidence, GatheredEvidence, ResearchEvidenceSource,
        ResearchWebCollector, ResearchWebOutcome, ResearchWebStore, SystemFailure, SystemPort,
        SystemPortError, WebCollectionFailure, WebFinding, WebSearchGuard, WriteCompleted,
    };
    use domain::{
        ApprovedWebSearchQuery, CollectedItem, DisplayInstrumentName, EvidenceBundleError,
        EvidenceItem, EvidenceItemId, EvidenceKind, InformationSource, InstrumentId, Nanosecond,
        NormalizedUrl, OffsetSeconds, ResearchTopic, SensitivePersonalName, Timestamp, UnixSeconds,
        UntrustedText, WebSearchSensitiveContext,
    };
    use testkit_ports::{Script, SteppedClock, memory::MemoryReply, system_fakes::MemoryAuditLog};

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
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn instrument() -> InstrumentId {
        InstrumentId::try_new("A005930").expect("종목")
    }
    fn name() -> DisplayInstrumentName {
        DisplayInstrumentName::try_new("삼성전자").expect("이름")
    }
    fn local(id: &str, kind: EvidenceKind) -> EvidenceItem {
        EvidenceItem::new(
            EvidenceItemId::try_new(id).expect("ID"),
            kind,
            at(50),
            UntrustedText::try_new("로컬").expect("내용"),
        )
    }
    fn finding(url: &str, title: &str, summary: Option<&str>) -> WebFinding {
        WebFinding::new(
            NormalizedUrl::try_new(url).expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            summary.map(|text| UntrustedText::try_new(text).expect("요약")),
        )
    }

    type SourceReply = Result<Vec<EvidenceItem>, SystemPortError>;
    #[derive(Default)]
    struct Source(Script<InstrumentId, SourceReply>);
    impl ResearchEvidenceSource for Source {
        async fn gather(&self, instrument: &InstrumentId) -> SourceReply {
            self.0.run(instrument.clone()).await
        }
    }
    type CollectReply = Result<Vec<WebFinding>, WebCollectionFailure>;
    #[derive(Default)]
    struct Collector(Script<Vec<String>, CollectReply>);
    impl ResearchWebCollector for Collector {
        async fn collect(&self, queries: &[ApprovedWebSearchQuery]) -> CollectReply {
            self.0
                .run(
                    queries
                        .iter()
                        .map(|query| query.text().as_str().to_owned())
                        .collect(),
                )
                .await
        }
    }
    type StoreReply = Result<WriteCompleted, SystemPortError>;
    #[derive(Default)]
    struct Store(Script<(InstrumentId, Vec<CollectedItem>), StoreReply>);
    impl ResearchWebStore for Store {
        async fn save(&self, instrument: &InstrumentId, items: &[CollectedItem]) -> StoreReply {
            self.0.run((instrument.clone(), items.to_vec())).await
        }
    }

    struct Fixture {
        source: Source,
        audit: MemoryAuditLog,
        collector: Collector,
        store: Store,
        clock: SteppedClock,
    }
    impl Fixture {
        fn new() -> Self {
            let source = Source::default();
            source.0.push(Ok(vec![
                local("quote-1", EvidenceKind::Quote),
                local("thesis-1", EvidenceKind::Thesis),
            ]));
            let audit = MemoryAuditLog::default();
            for _ in 0_u8..4 {
                audit.script_append().push(MemoryReply::Memory);
            }
            Self {
                source,
                audit,
                collector: Collector::default(),
                store: Store::default(),
                clock: SteppedClock::new(at(1_000)),
            }
        }
        fn execute(
            &self,
            topics: &[ResearchTopic],
            context: &WebSearchSensitiveContext,
        ) -> Result<GatheredEvidence, GatherEvidenceError> {
            let guard = WebSearchGuard::new(&self.clock, &self.audit);
            run(GatherResearchEvidence {
                source: &self.source,
                guard: &guard,
                collector: &self.collector,
                store: &self.store,
                clock: &self.clock,
            }
            .execute(&instrument(), &name(), topics, context))
        }
    }

    #[test]
    fn web_findings_join_the_bundle_with_ids_collection_time_and_web_source() {
        let fixture = Fixture::new();
        fixture.collector.0.push(Ok(vec![
            finding("https://a.example/1", "제목1", Some("요약1")),
            finding("https://a.example/1", "중복", None),
            finding("https://b.example/2", "제목2", None),
        ]));
        fixture.store.0.push(Ok(WriteCompleted));
        let gathered = fixture
            .execute(
                &[ResearchTopic::Earnings, ResearchTopic::Regulation],
                &WebSearchSensitiveContext::new(),
            )
            .expect("수집");
        assert_eq!(gathered.web, ResearchWebOutcome::Collected);
        assert_eq!(
            fixture.collector.0.calls(),
            vec![vec!["삼성전자 실적".to_owned(), "삼성전자 규제".to_owned()]]
        );
        let items = gathered.bundle.items();
        assert_eq!(
            items
                .iter()
                .map(|item| (item.id().as_str(), item.kind(), item.collected_at()))
                .collect::<Vec<_>>(),
            vec![
                ("quote-1", EvidenceKind::Quote, at(50)),
                ("thesis-1", EvidenceKind::Thesis, at(50)),
                ("web-1", EvidenceKind::Web, at(1_000)),
                ("web-2", EvidenceKind::Web, at(1_000)),
            ]
        );
        assert_eq!(
            items.get(2).expect("웹 1").content().as_untrusted_str(),
            "제목1\n요약1"
        );
        let saved = fixture.store.0.calls();
        let [(stored_for, stored)] = saved.as_slice() else {
            panic!("저장 한 번");
        };
        assert_eq!(stored_for, &instrument());
        assert_eq!(stored.len(), 2);
        for item in stored {
            assert_eq!(item.source(), &InformationSource::Web);
        }
        assert_eq!(fixture.audit.entries().len(), 2);
    }

    #[test]
    fn rejected_queries_never_reach_collection_and_failures_keep_local_evidence() {
        let fixture = Fixture::new();
        let context = WebSearchSensitiveContext::new()
            .with_personal_name(SensitivePersonalName::try_new("삼성전자").expect("이름"));
        let gathered = fixture
            .execute(&[ResearchTopic::Earnings], &context)
            .expect("수집");
        assert_eq!(gathered.web, ResearchWebOutcome::NoApprovedQuery);
        assert_eq!(gathered.bundle.items().len(), 2);
        assert_eq!(fixture.collector.0.calls().len(), 0);
        assert_eq!(fixture.audit.entries().len(), 1);

        let failed = Fixture::new();
        failed
            .collector
            .0
            .push(Err(WebCollectionFailure::Unavailable));
        let after_failure = failed
            .execute(
                &[ResearchTopic::Earnings],
                &WebSearchSensitiveContext::new(),
            )
            .expect("수집");
        assert_eq!(
            after_failure.web,
            ResearchWebOutcome::Failed(WebCollectionFailure::Unavailable)
        );
        assert_eq!(after_failure.bundle.items().len(), 2);
        assert_eq!(failed.store.0.calls().len(), 0);
    }

    #[test]
    fn source_store_and_bundle_errors_stop_the_stage() {
        let storage = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let source = Fixture {
            source: Source::default(),
            ..Fixture::new()
        };
        source.source.0.push(Err(storage));
        assert_eq!(
            source.execute(
                &[ResearchTopic::Earnings],
                &WebSearchSensitiveContext::new()
            ),
            Err(GatherEvidenceError::Source(storage))
        );
        let empty = Fixture {
            source: Source::default(),
            ..Fixture::new()
        };
        empty.source.0.push(Ok(Vec::new()));
        assert_eq!(
            empty.execute(
                &[ResearchTopic::Earnings],
                &WebSearchSensitiveContext::new()
            ),
            Err(GatherEvidenceError::Bundle(EvidenceBundleError::Empty))
        );
        let store = Fixture::new();
        store
            .collector
            .0
            .push(Ok(vec![finding("https://a.example/1", "제목", None)]));
        store.store.0.push(Err(storage));
        assert_eq!(
            store.execute(
                &[ResearchTopic::Earnings],
                &WebSearchSensitiveContext::new()
            ),
            Err(GatherEvidenceError::Store(storage))
        );
        let clash = Fixture {
            source: Source::default(),
            ..Fixture::new()
        };
        clash
            .source
            .0
            .push(Ok(vec![local("web-1", EvidenceKind::News)]));
        clash
            .collector
            .0
            .push(Ok(vec![finding("https://a.example/1", "제목", None)]));
        clash.store.0.push(Ok(WriteCompleted));
        assert_eq!(
            clash.execute(
                &[ResearchTopic::Earnings],
                &WebSearchSensitiveContext::new()
            ),
            Err(GatherEvidenceError::Bundle(
                EvidenceBundleError::DuplicateId
            ))
        );
    }
}
