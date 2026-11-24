#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};

    use application::{
        Clock, DisclosureCardFact, DisclosureCardLevel, DisclosureTriggerLog,
        GetImportantDisclosureCards, SystemPortError,
    };
    use domain::{
        DisclosureGrade, DisclosureKey, InstrumentId, KindReceiptNo, Nanosecond, NormalizedUrl,
        OffsetSeconds, ReceiptNo, Timestamp, UnixSeconds, UntrustedText,
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

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    struct Fixed(Timestamp);

    impl Clock for Fixed {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    #[derive(Debug, Default)]
    struct Log {
        facts: Vec<DisclosureCardFact>,
        asked: RwLock<Vec<Timestamp>>,
    }

    impl DisclosureTriggerLog for Log {
        fn recorded_since(
            &self,
            since: Timestamp,
        ) -> impl Future<Output = Result<Vec<DisclosureCardFact>, SystemPortError>> + Send {
            self.asked.write().expect("잠금").push(since);
            std::future::ready(Ok(self.facts.clone()))
        }
    }

    fn key(number: u8) -> DisclosureKey {
        DisclosureKey::from_receipt(ReceiptNo::Kind(
            KindReceiptNo::try_new(&format!("202611130000{number:02}")).expect("접수"),
        ))
    }

    fn id(raw: &str) -> InstrumentId {
        InstrumentId::try_new(raw).expect("종목")
    }

    fn fact(
        number: u8,
        instrument: &str,
        grade: DisclosureGrade,
        seconds: i64,
    ) -> DisclosureCardFact {
        DisclosureCardFact {
            key: key(number),
            instrument: id(instrument),
            title: UntrustedText::try_new("공시 제목").expect("제목"),
            url: NormalizedUrl::try_new(&format!("https://kind.krx.co.kr/d/{number}"))
                .expect("주소"),
            grade,
            recorded_at: at(seconds),
        }
    }

    #[test]
    fn held_urgent_and_high_disclosures_become_cards_urgent_first_then_newest() {
        let now = 200_000_i64;
        let log = Log {
            facts: vec![
                fact(1, "KR7005930003", DisclosureGrade::High, 199_000),
                fact(2, "KR7005930003", DisclosureGrade::Urgent, 150_000),
                fact(3, "KR7000660001", DisclosureGrade::Urgent, 199_500),
                fact(4, "KR7005930003", DisclosureGrade::Normal, 199_900),
                fact(5, "KR7005930003", DisclosureGrade::High, 100_000),
                fact(6, "KR7005930003", DisclosureGrade::High, 199_800),
                fact(1, "KR7005930003", DisclosureGrade::High, 199_000),
            ],
            asked: RwLock::new(Vec::new()),
        };
        let clock = Fixed(at(now));
        let cards = block_on(
            GetImportantDisclosureCards {
                log: &log,
                clock: &clock,
            }
            .execute(&[id("KR7005930003")]),
        )
        .expect("카드");
        let keys: Vec<DisclosureKey> = cards.iter().map(|card| card.key().clone()).collect();
        assert_eq!(keys, vec![key(2), key(6), key(1)]);
        assert_eq!(cards[0].level(), DisclosureCardLevel::Urgent);
        assert_eq!(cards[1].level(), DisclosureCardLevel::High);
        assert_eq!(cards[0].instrument(), &id("KR7005930003"));
        assert_eq!(log.asked.read().expect("잠금").as_slice(), &[at(113_600)]);
    }

    #[test]
    fn no_holdings_means_no_cards_and_at_most_five_are_shown() {
        let facts: Vec<DisclosureCardFact> = (1_u8..=8)
            .map(|number| {
                fact(
                    number,
                    "KR7005930003",
                    DisclosureGrade::Urgent,
                    i64::from(number) + 190_000,
                )
            })
            .collect();
        let log = Log {
            facts,
            asked: RwLock::new(Vec::new()),
        };
        let clock = Fixed(at(200_000));
        let use_case = GetImportantDisclosureCards {
            log: &log,
            clock: &clock,
        };
        assert_eq!(
            block_on(use_case.execute(&[])).expect("카드").len(),
            0_usize
        );
        let cards = block_on(use_case.execute(&[id("KR7005930003")])).expect("카드");
        assert_eq!(cards.len(), 5_usize);
        assert_eq!(cards[0].key(), &key(8));
    }
}
