#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};

    use application::{
        EventBusBuilder, EventSubscriber, RecordWakeTriggers, RetryDelay, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError, TriggerWrite, WakeTriggerOutcome,
        WakeTriggerRepository,
    };
    use domain::{
        Currency, Disclosure, DisclosureAction, DisclosureGrade, DisclosureKey, DisclosureRevision,
        DomainEvent, DomainEventKind, HoldingStatus, InstrumentId, KindReceiptNo, Nanosecond,
        NormalizedUrl, NumericInput, OffsetSeconds, Price, ReceiptNo, Timestamp, TriggerJudgement,
        UnixSeconds, UntrustedText, WakeTrigger,
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

    #[derive(Debug, Default)]
    struct Journal {
        writes: RwLock<Vec<WakeTrigger>>,
        events: Arc<RwLock<Vec<DomainEvent>>>,
        fail: RwLock<Option<SystemPortError>>,
    }

    impl Journal {
        fn append(&self, trigger: &WakeTrigger) -> TriggerWrite {
            let mut writes = self.writes.write().expect("잠금");
            if writes.contains(trigger) {
                return TriggerWrite::AlreadyRecorded;
            }
            writes.push(trigger.clone());
            TriggerWrite::Recorded
        }
    }

    impl WakeTriggerRepository for Journal {
        fn record(
            &self,
            trigger: &WakeTrigger,
        ) -> impl Future<Output = Result<TriggerWrite, SystemPortError>> + Send {
            let failure = *self.fail.read().expect("잠금");
            std::future::ready(failure.map_or_else(|| Ok(self.append(trigger)), Err))
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

    fn bus(events: &Arc<RwLock<Vec<DomainEvent>>>) -> application::EventBus {
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            [
                DomainEventKind::DisclosureTriggerRecorded,
                DomainEventKind::PriceMoved,
                DomainEventKind::NewsActivityIncreased,
            ]
            .into(),
            Recorder(Arc::clone(events)),
        );
        builder.build()
    }

    fn disclosure(title: &str) -> Disclosure {
        Disclosure::new(
            ReceiptNo::Kind(KindReceiptNo::try_new("20261113000001").expect("접수")),
            NormalizedUrl::try_new("https://kind.krx.co.kr/disclosure/1").expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            None,
        )
    }

    fn samsung() -> InstrumentId {
        InstrumentId::try_new("KR7005930003").expect("종목")
    }

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    #[test]
    fn urgent_disclosure_is_recorded_before_one_trigger_event_and_duplicates_stay_silent() {
        let journal = Journal::default();
        let events = Arc::clone(&journal.events);
        let bus = bus(&events);
        let use_case = RecordWakeTriggers {
            repository: &journal,
            events: &bus,
        };
        let item = disclosure("[유]매매거래정지");
        let outcome = block_on(use_case.disclosure(
            &item,
            &samsung(),
            DisclosureRevision::Original,
            HoldingStatus::Held,
        ))
        .expect("기록");
        let expected = WakeTrigger::Disclosure {
            key: DisclosureKey::from_receipt(ReceiptNo::Kind(
                KindReceiptNo::try_new("20261113000001").expect("접수"),
            )),
            instrument: samsung(),
            grade: DisclosureGrade::Urgent,
            action: DisclosureAction::WakeAndNotify,
        };
        let WakeTriggerOutcome::Recorded(recorded, report) = outcome else {
            panic!("기록 결과");
        };
        assert_eq!(recorded, expected);
        assert_eq!(report.failures, vec![]);
        assert_eq!(journal.writes.read().expect("잠금").as_slice(), &[expected]);
        assert_eq!(
            events.read().expect("잠금").as_slice(),
            &[DomainEvent::DisclosureTriggerRecorded(item.key().clone())]
        );
        assert_eq!(
            block_on(use_case.disclosure(
                &item,
                &samsung(),
                DisclosureRevision::Original,
                HoldingStatus::Held,
            )),
            Ok(WakeTriggerOutcome::AlreadyRecorded)
        );
        assert_eq!(events.read().expect("잠금").len(), 1_usize);
    }

    #[test]
    fn corrected_high_disclosure_and_unheld_stocks_do_not_wake() {
        let journal = Journal::default();
        let events = Arc::clone(&journal.events);
        let bus = bus(&events);
        let use_case = RecordWakeTriggers {
            repository: &journal,
            events: &bus,
        };
        assert_eq!(
            block_on(use_case.disclosure(
                &disclosure("[기재정정]유상증자결정"),
                &samsung(),
                DisclosureRevision::Correction,
                HoldingStatus::Held,
            )),
            Ok(WakeTriggerOutcome::DisclosureNotWaking(
                DisclosureAction::Briefing
            ))
        );
        assert_eq!(
            block_on(use_case.disclosure(
                &disclosure("[유]매매거래정지"),
                &samsung(),
                DisclosureRevision::Original,
                HoldingStatus::NotHeld,
            )),
            Ok(WakeTriggerOutcome::DisclosureNotWaking(
                DisclosureAction::CollectOnly
            ))
        );
        let corrected_urgent = block_on(use_case.disclosure(
            &disclosure("[기재정정]매매거래정지"),
            &samsung(),
            DisclosureRevision::Correction,
            HoldingStatus::Held,
        ))
        .expect("기록");
        let WakeTriggerOutcome::Recorded(WakeTrigger::Disclosure { grade, action, .. }, _report) =
            corrected_urgent
        else {
            panic!("한 단계 낮춘 높음");
        };
        assert_eq!(
            (grade, action),
            (DisclosureGrade::High, DisclosureAction::Wake)
        );
        assert_eq!(journal.writes.read().expect("잠금").len(), 1_usize);
    }

    #[test]
    fn failed_write_publishes_nothing() {
        let journal = Journal::default();
        *journal.fail.write().expect("잠금") = Some(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ));
        let events = Arc::clone(&journal.events);
        let bus = bus(&events);
        let use_case = RecordWakeTriggers {
            repository: &journal,
            events: &bus,
        };
        let error = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        assert_eq!(
            block_on(use_case.price_move(
                &samsung(),
                HoldingStatus::Held,
                &price("10000"),
                &price("9000"),
            )),
            Err(error)
        );
        assert_eq!(events.read().expect("잠금").len(), 0_usize);
    }

    #[test]
    fn price_and_news_triggers_publish_their_own_events() {
        let journal = Journal::default();
        let events = Arc::clone(&journal.events);
        let bus = bus(&events);
        let use_case = RecordWakeTriggers {
            repository: &journal,
            events: &bus,
        };
        assert_eq!(
            block_on(use_case.price_move(
                &samsung(),
                HoldingStatus::Held,
                &price("10000"),
                &price("10100"),
            )),
            Ok(WakeTriggerOutcome::NotTriggered(TriggerJudgement::Quiet))
        );
        assert_eq!(
            block_on(use_case.price_move(
                &samsung(),
                HoldingStatus::NotHeld,
                &price("10000"),
                &price("20000"),
            )),
            Ok(WakeTriggerOutcome::NotTriggered(TriggerJudgement::Quiet))
        );
        let moved = block_on(use_case.price_move(
            &samsung(),
            HoldingStatus::Held,
            &price("10000"),
            &price("10500"),
        ))
        .expect("가격");
        let WakeTriggerOutcome::Recorded(WakeTrigger::PriceMove(_), _report) = moved else {
            panic!("가격 기록");
        };
        let stamps = [at(100), at(200), at(300), at(400), at(500)];
        let burst = block_on(use_case.news_burst(&samsung(), &stamps, at(600))).expect("뉴스");
        let WakeTriggerOutcome::Recorded(WakeTrigger::NewsBurst(_), _burst_report) = burst else {
            panic!("뉴스 기록");
        };
        assert_eq!(
            events.read().expect("잠금").as_slice(),
            &[
                DomainEvent::PriceMoved(samsung()),
                DomainEvent::NewsActivityIncreased(samsung())
            ]
        );
    }
}
