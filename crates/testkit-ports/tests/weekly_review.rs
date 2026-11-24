#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AnalysisFailure, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RetryDelay,
        RunWeeklyReview, ScorecardRefresh, ScorecardUpdate, SystemFailure, SystemPort,
        SystemPortError, UnavailableCause, WeeklyFindings, WeeklyNarrative, WeeklyReviewCard,
        WeeklyReviewCards, WeeklyReviewError, WeeklyReviewInputs, WeeklyReviewNarrator,
        WeeklyReviewRun, WeeklyReviewSource, WriteCompleted,
    };
    use domain::{
        BuyingPower, CalendarDate, CalendarYear, Currency, DayNumber, InvestorProfile, Money,
        MonthNumber, Nanosecond, NumericInput, OffsetSeconds, Policy, PolicyPrinciple,
        PolicySecurityFacts, PolicyText, PolicyViolation, Portfolio, SectorAssignments, Timestamp,
        UnixSeconds, UntrustedText,
    };
    use testkit_ports::{Script, SteppedClock};

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
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn kst(day: u8, hour: i64) -> Timestamp {
        let midnight = domain::valuation_time(date(day))
            .utc_seconds()
            .as_i64()
            .checked_sub(57_600)
            .expect("자정");
        Timestamp::from_parts(
            UnixSeconds::new(
                midnight
                    .checked_add(hour.checked_mul(3_600).expect("시"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn inputs(policy: Policy) -> WeeklyReviewInputs {
        let at = kst(28, 9);
        let portfolio = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money("1000", Currency::Krw), at),
            BuyingPower::new(money("0", Currency::Usd), at),
        )
        .expect("포트폴리오");
        WeeklyReviewInputs {
            valuation: portfolio.evaluate(None),
            assignments: SectorAssignments::default(),
            facts: PolicySecurityFacts::default(),
            policy,
        }
    }
    fn default_policy() -> Policy {
        Policy::new(InvestorProfile::default())
    }
    fn storage() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }

    #[derive(Default)]
    struct Source(Script<(), Result<WeeklyReviewInputs, PortError>>);
    impl WeeklyReviewSource for Source {
        async fn inputs(&self) -> Result<WeeklyReviewInputs, PortError> {
            self.0.run(()).await
        }
    }
    #[derive(Default)]
    struct Scorecards(Script<(), Result<WriteCompleted, SystemPortError>>);
    impl ScorecardRefresh for Scorecards {
        async fn refresh(&self) -> Result<WriteCompleted, SystemPortError> {
            self.0.run(()).await
        }
    }
    #[derive(Default)]
    struct Narrator(Script<WeeklyFindings, Result<UntrustedText, AnalysisFailure>>);
    impl WeeklyReviewNarrator for Narrator {
        async fn narrate(
            &self,
            findings: &WeeklyFindings,
        ) -> Result<UntrustedText, AnalysisFailure> {
            self.0.run(findings.clone()).await
        }
    }
    #[derive(Default)]
    struct Cards {
        last: Script<(), Result<Option<CalendarDate>, SystemPortError>>,
        save: Script<WeeklyReviewCard, Result<WriteCompleted, SystemPortError>>,
    }
    impl WeeklyReviewCards for Cards {
        async fn last_week(&self) -> Result<Option<CalendarDate>, SystemPortError> {
            self.last.run(()).await
        }
        async fn save(&self, card: &WeeklyReviewCard) -> Result<WriteCompleted, SystemPortError> {
            self.save.run(card.clone()).await
        }
    }
    #[derive(Default)]
    struct Fixture {
        source: Source,
        scorecards: Scorecards,
        narrator: Narrator,
        cards: Cards,
    }
    impl Fixture {
        fn execute(&self, now: Timestamp) -> Result<WeeklyReviewRun, WeeklyReviewError> {
            let clock = SteppedClock::new(now);
            run(RunWeeklyReview {
                source: &self.source,
                scorecards: &self.scorecards,
                narrator: &self.narrator,
                cards: &self.cards,
                clock: &clock,
            }
            .execute())
        }
    }

    #[test]
    fn saturday_morning_checks_principles_refreshes_scorecard_and_saves_one_card() {
        let fixture = Fixture::default();
        fixture.cards.last.push(Ok(Some(date(21))));
        fixture.source.0.push(Ok(inputs(default_policy())));
        fixture.scorecards.0.push(Ok(WriteCompleted));
        let note = UntrustedText::try_new("이번 주 요약").expect("요약");
        fixture.narrator.0.push(Ok(note.clone()));
        fixture.cards.save.push(Ok(WriteCompleted));
        let direct = inputs(default_policy());
        let expected =
            match direct
                .policy
                .check_current(&direct.valuation, &direct.assignments, &direct.facts)
            {
                Ok(list) => WeeklyFindings::Checked(list),
                Err(blocked) => WeeklyFindings::Unavailable(blocked),
            };
        let card = WeeklyReviewCard {
            week: date(28),
            findings: expected.clone(),
            scorecard: ScorecardUpdate::Refreshed,
            narrative: WeeklyNarrative::Written(note),
        };
        assert_eq!(
            fixture.execute(kst(28, 9)),
            Ok(WeeklyReviewRun::Saved(card.clone()))
        );
        assert_eq!(fixture.cards.save.calls(), vec![card]);
        assert_eq!(fixture.narrator.0.calls(), vec![expected]);
    }

    #[test]
    fn unresolved_principles_and_failed_side_steps_still_produce_the_card() {
        let fixture = Fixture::default();
        fixture.cards.last.push(Ok(None));
        let blocked = default_policy().with_unresolved_principle(
            PolicyPrinciple::from_missing_contract(PolicyText::try_new("현금 비중").expect("원칙")),
        );
        fixture.source.0.push(Ok(inputs(blocked)));
        fixture.scorecards.0.push(Err(storage()));
        fixture.narrator.0.push(Err(AnalysisFailure::Unavailable));
        fixture.cards.save.push(Ok(WriteCompleted));
        let Ok(WeeklyReviewRun::Saved(card)) = fixture.execute(kst(28, 6)) else {
            panic!("저장");
        };
        assert_eq!(
            card.findings,
            WeeklyFindings::Unavailable(PolicyViolation::UnresolvedPrinciples)
        );
        assert_eq!(card.scorecard, ScorecardUpdate::Failed(storage()));
        assert_eq!(
            card.narrative,
            WeeklyNarrative::Missing(AnalysisFailure::Unavailable)
        );
    }

    #[test]
    fn other_times_and_finished_weeks_do_nothing() {
        let fixture = Fixture::default();
        for now in [kst(28, 5), kst(28, 12), kst(27, 9), kst(29, 9)] {
            assert_eq!(fixture.execute(now), Ok(WeeklyReviewRun::NotDue));
        }
        assert_eq!(fixture.cards.last.calls().len(), 0);
        fixture.cards.last.push(Ok(Some(date(28))));
        assert_eq!(
            fixture.execute(kst(28, 10)),
            Ok(WeeklyReviewRun::AlreadyDone(date(28)))
        );
        assert_eq!(fixture.source.0.calls().len(), 0);
    }

    #[test]
    fn storage_and_input_failures_stop_without_saving() {
        let lookup = Fixture::default();
        lookup.cards.last.push(Err(storage()));
        assert_eq!(
            lookup.execute(kst(28, 9)),
            Err(WeeklyReviewError::Cards(storage()))
        );
        let missing = Fixture::default();
        missing.cards.last.push(Ok(None));
        let error = PortError::new(
            ReadEndpoint::Holdings,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        );
        missing.source.0.push(Err(error));
        assert_eq!(
            missing.execute(kst(28, 9)),
            Err(WeeklyReviewError::Inputs(error))
        );
        assert_eq!(missing.cards.save.calls().len(), 0);
        let unsaved = Fixture::default();
        unsaved.cards.last.push(Ok(None));
        unsaved.source.0.push(Ok(inputs(default_policy())));
        unsaved.scorecards.0.push(Ok(WriteCompleted));
        unsaved.narrator.0.push(Err(AnalysisFailure::Interrupted));
        unsaved.cards.save.push(Err(storage()));
        assert_eq!(
            unsaved.execute(kst(28, 9)),
            Err(WeeklyReviewError::Cards(storage()))
        );
    }
}
