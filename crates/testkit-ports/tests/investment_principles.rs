#[cfg(test)]
mod tests {
    use application::{
        ApplyInvestmentPrinciples, RetryDelay, SystemFailure, SystemPort, SystemPortError,
        UpdateInvestmentPrinciples,
    };
    use domain::{
        Allocation, BuyingPower, CashFloor, Concentration, ConcentrationFloor, Currency,
        InvestmentPrinciples, InvestorProfile, Money, Nanosecond, NumericInput, OffsetSeconds,
        Policy, PolicyConcentration, PolicyText, Portfolio, PrincipleNote, SectorAssignments,
        Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::principles_fakes::MemoryPrinciplesRepository;

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

    fn empty_holdings() -> Concentration {
        let time = Timestamp::from_parts(
            UnixSeconds::new(10),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let zero = |currency| {
            Money::parse(&NumericInput::try_new("0").expect("입력"), currency).expect("금액")
        };
        let valuation = Portfolio::new(
            Vec::new(),
            BuyingPower::new(zero(Currency::Krw), time),
            BuyingPower::new(zero(Currency::Usd), time),
        )
        .expect("포트폴리오")
        .evaluate(None);
        Concentration::new(&Allocation::new(&valuation, &SectorAssignments::default()))
    }

    fn principles() -> InvestmentPrinciples {
        InvestmentPrinciples::new(
            Some(
                PrincipleNote::try_new(
                    &PolicyText::try_new("레버리지 상품은 사지 않는다").expect("글"),
                )
                .expect("원칙"),
            ),
            CashFloor::TwentyPercent,
            ConcentrationFloor::Six,
        )
    }

    fn unavailable() -> SystemPortError {
        SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )
    }

    #[test]
    fn saved_principles_reach_the_policy_for_gate_and_ai_context() {
        let repo = MemoryPrinciplesRepository::default();
        run(UpdateInvestmentPrinciples::new(&repo).execute(&principles())).expect("저장");
        assert_eq!(repo.stored(), Some(principles()));
        let policy = run(ApplyInvestmentPrinciples::new(&repo)
            .execute(Policy::new(InvestorProfile::default()), &empty_holdings()))
        .expect("적용");
        assert_eq!(
            policy
                .context()
                .iter()
                .map(PolicyText::as_str)
                .collect::<Vec<_>>(),
            vec!["레버리지 상품은 사지 않는다"]
        );
        assert_eq!(
            policy.minimum_cash(),
            CashFloor::TwentyPercent.minimum_weight()
        );
        assert_eq!(policy.concentration(), PolicyConcentration::NotApplicable);
    }

    #[test]
    fn missing_principles_fall_back_to_the_defaults() {
        let repo = MemoryPrinciplesRepository::default();
        let policy = run(ApplyInvestmentPrinciples::new(&repo)
            .execute(Policy::new(InvestorProfile::default()), &empty_holdings()))
        .expect("적용");
        assert_eq!(policy.context().len(), 0_usize);
        assert_eq!(policy.minimum_cash(), None);
    }

    #[test]
    fn storage_failures_are_reported_and_nothing_is_saved() {
        let repo = MemoryPrinciplesRepository::default();
        repo.fail_with(unavailable());
        assert_eq!(
            run(UpdateInvestmentPrinciples::new(&repo).execute(&principles())),
            Err(unavailable())
        );
        assert_eq!(repo.stored(), None);
        assert_eq!(
            run(ApplyInvestmentPrinciples::new(&repo)
                .execute(Policy::new(InvestorProfile::default()), &empty_holdings(),)),
            Err(unavailable())
        );
    }
}
