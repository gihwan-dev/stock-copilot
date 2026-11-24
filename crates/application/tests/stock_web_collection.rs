#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};

    use application::{
        CollectionQuery, CollectionTopic, EventBusBuilder, EventSubscriber, RetryDelay,
        RunStockWebCollection, STOCK_WEB_TOPICS, StockWebCollectionOutcome, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError, WebCollectionFailure, WebCollectionRunner,
        WebCollectionStore, WebFinding, WriteCompleted, plan_stock_web_collection,
    };
    use domain::{
        CollectedItem, DomainEvent, DomainEventKind, InformationSource, NormalizedUrl, Symbol,
        UntrustedText,
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut pinned = Box::pin(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    type Reply = Result<Vec<WebFinding>, WebCollectionFailure>;

    #[derive(Debug, Default)]
    struct Runner {
        replies: RwLock<VecDeque<Reply>>,
        queries: RwLock<Vec<CollectionQuery>>,
    }

    impl WebCollectionRunner for Runner {
        fn collect(&self, query: &CollectionQuery) -> impl Future<Output = Reply> + Send {
            self.queries.write().expect("잠금").push(query.clone());
            let reply = self
                .replies
                .write()
                .expect("잠금")
                .pop_front()
                .unwrap_or(Err(WebCollectionFailure::Unavailable));
            std::future::ready(reply)
        }
    }

    #[derive(Debug, Default)]
    struct Store {
        saved: RwLock<Vec<(Symbol, Vec<CollectedItem>)>>,
        fail: RwLock<Option<SystemPortError>>,
    }

    impl WebCollectionStore for Store {
        fn save(
            &self,
            instrument: &Symbol,
            items: &[CollectedItem],
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let failure = *self.fail.read().expect("잠금");
            if failure.is_none() {
                self.saved
                    .write()
                    .expect("잠금")
                    .push((instrument.clone(), items.to_vec()));
            }
            std::future::ready(failure.map_or(Ok(WriteCompleted), Err))
        }
    }

    struct Recorder(Arc<RwLock<Vec<DomainEvent>>>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .write()
                .map_err(|_poison| SubscriberFailure::Unavailable)?
                .push(event.clone());
            Ok(())
        }
    }

    fn symbol(raw: &str) -> Symbol {
        Symbol::try_new(raw).expect("종목")
    }

    fn finding(url: &str, title: &str) -> WebFinding {
        WebFinding::new(
            NormalizedUrl::try_new(url).expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            Some(UntrustedText::try_new("요약").expect("요약")),
        )
    }

    #[test]
    fn plan_uses_code_topics_for_known_stocks_only_without_free_text() {
        let plan = plan_stock_web_collection(&[symbol("005930"), symbol("AAPL"), symbol("005930")]);
        assert_eq!(plan.len(), 2_usize);
        assert_eq!(
            plan[0].query(),
            CollectionQuery::CodeTopics {
                instruments: vec![symbol("005930")],
                topics: vec![
                    CollectionTopic::CompanyNews,
                    CollectionTopic::Earnings,
                    CollectionTopic::Disclosures
                ],
            }
        );
        assert_eq!(STOCK_WEB_TOPICS.len(), 3_usize);
        let many: Vec<Symbol> = (0_u8..30)
            .map(|index| symbol(&format!("A{index:05}")))
            .collect();
        assert_eq!(plan_stock_web_collection(&many).len(), 20_usize);
    }

    #[test]
    fn results_are_saved_with_web_source_and_one_collection_event() {
        let runner = Runner::default();
        runner.replies.write().expect("잠금").extend([
            Ok(vec![
                finding("https://a.example.com/1?utm_source=x", "삼성전자 신제품"),
                finding("https://a.example.com/1", "중복"),
                finding("https://b.example.com/2", "삼성전자 실적 전망"),
            ]),
            Err(WebCollectionFailure::Rejected),
        ]);
        let store = Store::default();
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            [DomainEventKind::CollectionFinished].into(),
            Recorder(Arc::clone(&events)),
        );
        let bus = builder.build();
        let plan = plan_stock_web_collection(&[symbol("005930"), symbol("AAPL")]);
        let report = block_on(
            RunStockWebCollection {
                runner: &runner,
                store: &store,
                events: &bus,
            }
            .run(&plan),
        );
        let StockWebCollectionOutcome::Saved(items) = report.results[0].outcome() else {
            panic!("저장");
        };
        assert_eq!(items.len(), 2_usize);
        assert!(
            items
                .iter()
                .all(|item| item.source() == &InformationSource::Web)
        );
        assert_eq!(
            report.results[1].outcome(),
            &StockWebCollectionOutcome::RunFailed(WebCollectionFailure::Rejected)
        );
        assert_eq!(store.saved.read().expect("잠금").len(), 1_usize);
        assert_eq!(
            events.read().expect("잠금").as_slice(),
            &[DomainEvent::CollectionFinished(InformationSource::Web)]
        );
        assert_eq!(runner.queries.read().expect("잠금").len(), 2_usize);
    }

    #[test]
    fn store_failure_or_no_success_publishes_nothing() {
        let runner = Runner::default();
        runner
            .replies
            .write()
            .expect("잠금")
            .push_back(Ok(vec![finding("https://a.example.com/1", "제목")]));
        let store = Store::default();
        let error = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        *store.fail.write().expect("잠금") = Some(error);
        let bus = EventBusBuilder::default().build();
        let report = block_on(
            RunStockWebCollection {
                runner: &runner,
                store: &store,
                events: &bus,
            }
            .run(&plan_stock_web_collection(&[symbol("005930")])),
        );
        assert_eq!(
            report.results[0].outcome(),
            &StockWebCollectionOutcome::StoreFailed(error)
        );
        assert_eq!(report.delivery, None);
    }
}
