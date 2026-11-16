#[cfg(test)]
mod tests {
    use application::user_presence::UserPresence;
    use application::{Clock, RiskChangeConfirmation};
    use domain::{Nanosecond, OffsetSeconds, RiskChange, Timestamp, UnixSeconds};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{ClockStepError, PresenceCall, Script, ScriptedPresence, SteppedClock};
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("가짜는 즉시 완료"),
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
    #[test]
    fn scripts_preserve_order_and_arguments() {
        let script = Script::new([
            RiskChangeConfirmation::Denied,
            RiskChangeConfirmation::Confirmed,
            RiskChangeConfirmation::Unavailable,
        ]);
        assert_eq!(
            run(script.run(RiskChange::EnableOrders)),
            RiskChangeConfirmation::Denied
        );
        assert_eq!(
            run(script.run(RiskChange::AllowHours)),
            RiskChangeConfirmation::Confirmed
        );
        assert_eq!(
            run(script.run(RiskChange::AllowMarketOrders)),
            RiskChangeConfirmation::Unavailable
        );
        assert_eq!(
            script.calls(),
            [
                RiskChange::EnableOrders,
                RiskChange::AllowHours,
                RiskChange::AllowMarketOrders
            ]
        );
    }
    #[test]
    fn clock_never_moves_on_its_own_and_rejects_backwards() {
        let clock = SteppedClock::new(at(11));
        assert_eq!(clock.now(), at(11));
        assert_eq!(clock.now(), at(11));
        assert_eq!(clock.advance_to(at(23)), Ok(()));
        assert_eq!(clock.advance_to(at(23)), Ok(()));
        assert_eq!(clock.advance_to(at(17)), Err(ClockStepError::Backwards));
        assert_eq!(clock.now(), at(23));
        assert_eq!(clock.calls(), [at(11), at(11), at(23)], "각 동기 조회 기록");
    }
    #[test]
    fn risk_confirmation_is_scripted_and_recorded_separately() {
        let presence = ScriptedPresence::new(
            [],
            [
                RiskChangeConfirmation::Confirmed,
                RiskChangeConfirmation::Denied,
                RiskChangeConfirmation::Unavailable,
            ],
        );
        assert_eq!(
            run(presence.confirm_risk_change(RiskChange::EnableOrders)),
            RiskChangeConfirmation::Confirmed
        );
        assert_eq!(
            run(presence.confirm_risk_change(RiskChange::AllowHours)),
            RiskChangeConfirmation::Denied
        );
        assert_eq!(
            run(presence.confirm_risk_change(RiskChange::AllowMarketOrders)),
            RiskChangeConfirmation::Unavailable
        );
        assert_eq!(
            presence.calls(),
            [
                PresenceCall::RiskChange(RiskChange::EnableOrders),
                PresenceCall::RiskChange(RiskChange::AllowHours),
                PresenceCall::RiskChange(RiskChange::AllowMarketOrders)
            ]
        );
        assert!(presence.order_calls().is_empty(), "주문 확인 호출 없음");
        assert_eq!(
            presence.risk_change_calls(),
            [
                RiskChange::EnableOrders,
                RiskChange::AllowHours,
                RiskChange::AllowMarketOrders
            ]
        );
    }
    #[test]
    fn empty_scripts_fail_immediately() {
        let script = Script::<RiskChange, RiskChangeConfirmation>::new([]);
        std::panic::catch_unwind(|| run(script.run(RiskChange::AllowHours)))
            .expect_err("빈 대본은 즉시 실패");
        assert_eq!(script.calls(), [RiskChange::AllowHours]);
    }
    #[test]
    fn order_results_and_later_script_pushes_preserve_fifo() {
        let script = Script::<(), application::OrderConfirmation>::new([]);
        script.push(application::OrderConfirmation::Confirmed);
        script.push(application::OrderConfirmation::Denied);
        script.push(application::OrderConfirmation::Unavailable);
        assert_eq!(
            run(script.run(())),
            application::OrderConfirmation::Confirmed,
            "첫 결과"
        );
        assert_eq!(
            run(script.run(())),
            application::OrderConfirmation::Denied,
            "두 번째 결과"
        );
        assert_eq!(
            run(script.run(())),
            application::OrderConfirmation::Unavailable,
            "세 번째 결과"
        );
    }
}
