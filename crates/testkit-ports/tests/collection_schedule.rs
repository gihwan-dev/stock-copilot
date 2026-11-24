#[cfg(test)]
mod tests {
    use application::{
        CollectionCadence, CollectionInputs, CollectionMinutes, CollectionPhase, CollectionPlan,
        CollectionScheduler, CollectionTarget, DartCallCount, DartDailyUsage, DartKeyState,
        FomcDay, HttpStatus, PlanCollectionSchedule, PortError, PortErrorKind, ReadEndpoint,
        ResponseMismatch, SystemPortError, WriteCompleted,
    };
    use domain::{
        KnownMarket, MarketCalendar, MarketSession, Nanosecond, OffsetSeconds, SessionData,
        SessionInformation, TimeInterval, Timestamp, UnixSeconds,
    };
    use std::future::Future;
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::scripted::{MarketInfoCall, ScriptedMarketInfo};

    #[derive(Debug, Default)]
    struct RecordingScheduler(RwLock<Vec<CollectionPlan>>);

    impl CollectionScheduler for RecordingScheduler {
        fn apply(
            &self,
            plan: &CollectionPlan,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.0.write().expect("잠금").push(plan.clone());
            std::future::ready(Ok(WriteCompleted))
        }
    }

    fn ready<T, F: Future<Output = T>>(future: F) -> T {
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

    fn open_session() -> SessionData {
        SessionData::try_new(
            TimeInterval::try_new(at(90_000), at(100_000)).expect("구간"),
            None,
        )
        .expect("세션")
    }

    fn calendar(market: KnownMarket) -> MarketCalendar {
        let sessions = [at(0), at(86_400), at(172_800)].map(|stamp| {
            let regular = (stamp == at(86_400)).then(open_session);
            let information = match market {
                KnownMarket::Kr => SessionInformation::Kr {
                    pre_market: None,
                    regular_market: regular,
                    after_market: None,
                },
                KnownMarket::Us => SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: regular,
                    after_market: None,
                },
            };
            MarketSession::try_new(stamp.offset_calendar_date(), information).expect("세션")
        });
        let [previous, today, next] = sessions;
        MarketCalendar::try_new(previous, today, next).expect("달력")
    }

    fn inputs() -> CollectionInputs {
        CollectionInputs {
            fomc: FomcDay::Regular,
            dart_key: DartKeyState::Configured,
            dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(20_000)),
        }
    }

    fn every(minutes: u16) -> CollectionCadence {
        CollectionCadence::Every(CollectionMinutes::try_new(minutes).expect("주기"))
    }

    #[test]
    fn domestic_session_from_toss_calendar_drives_two_minute_kind_and_reaches_the_scheduler() {
        let market = ScriptedMarketInfo::default();
        market.script_calendar().push(Ok(calendar(KnownMarket::Kr)));
        market.script_calendar().push(Ok(calendar(KnownMarket::Us)));
        let scheduler = RecordingScheduler::default();
        let clock = SteppedClock::new(at(95_000));
        let plan = ready(
            PlanCollectionSchedule {
                market_info: &market,
                scheduler: &scheduler,
                clock: &clock,
            }
            .run(inputs()),
        )
        .expect("계획");
        assert_eq!(plan.phase(), CollectionPhase::DomesticSession);
        assert_eq!(plan.cadence(CollectionTarget::KindDisclosure), every(2));
        assert_eq!(
            market.calls(),
            vec![
                MarketInfoCall::Calendar(KnownMarket::Kr),
                MarketInfoCall::Calendar(KnownMarket::Us)
            ]
        );
        assert_eq!(scheduler.0.read().expect("잠금").as_slice(), &[plan]);
    }

    #[test]
    fn us_session_only_and_calendar_failure_fall_back_without_fixed_clock_hours() {
        let market = ScriptedMarketInfo::default();
        market.script_calendar().push(Err(PortError::new(
            ReadEndpoint::Calendar,
            HttpStatus::ServerError,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        )));
        market.script_calendar().push(Ok(calendar(KnownMarket::Us)));
        let scheduler = RecordingScheduler::default();
        let clock = SteppedClock::new(at(95_000));
        let use_case = PlanCollectionSchedule {
            market_info: &market,
            scheduler: &scheduler,
            clock: &clock,
        };
        let plan = ready(use_case.run(inputs())).expect("계획");
        assert_eq!(plan.phase(), CollectionPhase::UsSession);
        assert_eq!(plan.cadence(CollectionTarget::SecEdgar), every(15));

        market.script_calendar().push(Ok(calendar(KnownMarket::Kr)));
        market.script_calendar().push(Ok(calendar(KnownMarket::Us)));
        let late = SteppedClock::new(at(120_000));
        let off = ready(
            PlanCollectionSchedule {
                market_info: &market,
                scheduler: &scheduler,
                clock: &late,
            }
            .run(inputs()),
        )
        .expect("계획");
        assert_eq!(off.phase(), CollectionPhase::OffHours);
        assert_eq!(off.cadence(CollectionTarget::KindDisclosure), every(30));
        assert_eq!(scheduler.0.read().expect("잠금").len(), 2_usize);
    }
}
