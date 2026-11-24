#[cfg(test)]
mod tests {
    use application::{
        CollectionCadence, CollectionConditions, CollectionMinutes, CollectionMinutesError,
        CollectionPhase, CollectionPlan, CollectionTarget, DartCallCount, DartDailyUsage,
        DartKeyState, DartQuotaLevel, FomcDay, StartupCatchUp, collection_cadence,
        collection_phase, startup_catch_up,
    };
    use domain::MarketState;

    fn every(minutes: u16) -> CollectionCadence {
        CollectionCadence::Every(CollectionMinutes::try_new(minutes).expect("주기"))
    }

    fn usage(used: u32) -> DartDailyUsage {
        DartDailyUsage::new(DartCallCount::new(used), DartCallCount::new(20_000))
    }

    fn conditions(phase: CollectionPhase) -> CollectionConditions {
        CollectionConditions {
            phase,
            fomc: FomcDay::Regular,
            dart_key: DartKeyState::Configured,
            dart_usage: usage(0),
        }
    }

    #[test]
    fn cadence_matches_the_design_table_for_every_phase() {
        use CollectionPhase::{DomesticSession, OffHours, UsSession};
        use CollectionTarget::*;
        let table: [(CollectionTarget, [u16; 3]); 8] = [
            (KindDisclosure, [2, 30, 30]),
            (DartList, [10, 30, 30]),
            (PressRss, [10, 30, 30]),
            (HankyungRss, [30, 30, 30]),
            (GoogleNews, [60, 60, 60]),
            (YahooFinance, [60, 30, 60]),
            (SecEdgar, [60, 15, 60]),
            (CentralBankRss, [60, 60, 60]),
        ];
        for (target, [domestic, us, other]) in table {
            assert_eq!(
                collection_cadence(target, &conditions(DomesticSession)),
                every(domestic),
                "{target:?}"
            );
            assert_eq!(
                collection_cadence(target, &conditions(UsSession)),
                every(us),
                "{target:?}"
            );
            assert_eq!(
                collection_cadence(target, &conditions(OffHours)),
                every(other),
                "{target:?}"
            );
        }
        for phase in [DomesticSession, UsSession, OffHours] {
            assert_eq!(
                collection_cadence(TreasuryAndFred, &conditions(phase)),
                CollectionCadence::AfterUsClose
            );
        }
    }

    #[test]
    fn fomc_day_shortens_central_bank_rss_only_during_the_us_session() {
        let mut fomc = conditions(CollectionPhase::UsSession);
        fomc.fomc = FomcDay::Scheduled;
        assert_eq!(
            collection_cadence(CollectionTarget::CentralBankRss, &fomc),
            every(10)
        );
        fomc.phase = CollectionPhase::DomesticSession;
        assert_eq!(
            collection_cadence(CollectionTarget::CentralBankRss, &fomc),
            every(60)
        );
        fomc.phase = CollectionPhase::OffHours;
        assert_eq!(
            collection_cadence(CollectionTarget::CentralBankRss, &fomc),
            every(60)
        );
    }

    #[test]
    fn dart_interval_doubles_from_half_of_the_daily_limit_and_stops_when_exhausted() {
        assert_eq!(usage(9_999).level(), DartQuotaLevel::BelowHalf);
        assert_eq!(usage(10_000).level(), DartQuotaLevel::HalfOrMore);
        assert_eq!(usage(20_000).level(), DartQuotaLevel::Exhausted);
        assert_eq!(
            DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(0)).level(),
            DartQuotaLevel::Exhausted
        );
        let mut half = conditions(CollectionPhase::DomesticSession);
        half.dart_usage = usage(9_999);
        assert_eq!(
            collection_cadence(CollectionTarget::DartList, &half),
            every(10)
        );
        half.dart_usage = usage(10_000);
        assert_eq!(
            collection_cadence(CollectionTarget::DartList, &half),
            every(20)
        );
        half.phase = CollectionPhase::OffHours;
        assert_eq!(
            collection_cadence(CollectionTarget::DartList, &half),
            every(60)
        );
        half.dart_usage = usage(20_000);
        assert_eq!(
            collection_cadence(CollectionTarget::DartList, &half),
            CollectionCadence::Off
        );
        assert_eq!(
            collection_cadence(CollectionTarget::KindDisclosure, &half),
            every(30)
        );
        let mut missing = conditions(CollectionPhase::DomesticSession);
        missing.dart_key = DartKeyState::Missing;
        assert_eq!(
            collection_cadence(CollectionTarget::DartList, &missing),
            CollectionCadence::Off
        );
    }

    #[test]
    fn phase_follows_toss_market_states_with_domestic_priority() {
        use MarketState::*;
        assert_eq!(
            collection_phase(RegularMarket, RegularMarket),
            CollectionPhase::DomesticSession
        );
        assert_eq!(
            collection_phase(AfterMarket, Closed),
            CollectionPhase::DomesticSession
        );
        assert_eq!(
            collection_phase(Closed, PreMarket),
            CollectionPhase::UsSession
        );
        assert_eq!(
            collection_phase(Unknown, DayMarket),
            CollectionPhase::UsSession
        );
        assert_eq!(collection_phase(Closed, Closed), CollectionPhase::OffHours);
        assert_eq!(
            collection_phase(Unknown, Unknown),
            CollectionPhase::OffHours
        );
    }

    #[test]
    fn plan_covers_every_target_and_kind_catches_up_one_day() {
        let plan = CollectionPlan::build(&conditions(CollectionPhase::DomesticSession));
        assert_eq!(plan.entries().len(), CollectionTarget::ALL.len());
        assert_eq!(plan.phase(), CollectionPhase::DomesticSession);
        assert_eq!(plan.cadence(CollectionTarget::KindDisclosure), every(2));
        let StartupCatchUp::OneDay(items) = startup_catch_up(CollectionTarget::KindDisclosure)
        else {
            panic!("KIND 하루치");
        };
        assert_eq!(items.as_u16(), &500);
        assert_eq!(
            startup_catch_up(CollectionTarget::GoogleNews),
            StartupCatchUp::MissedSinceLastRun
        );
        assert_eq!(CollectionMinutes::try_new(0), Err(CollectionMinutesError));
        assert_eq!(
            CollectionMinutes::try_new(1_441),
            Err(CollectionMinutesError)
        );
    }
}
