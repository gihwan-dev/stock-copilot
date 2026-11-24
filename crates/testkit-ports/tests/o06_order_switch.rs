#[cfg(test)]
mod tests {
    use application::{
        ChangeRiskSettings, ReadOrderButton, RiskChangeConfirmation, RiskSettingsChangeOutcome,
    };
    use domain::{
        HoursPermissionState, MarketTradingStatus, OrderButtonHiddenReason, OrderButtonState,
        OrderPermission, ProposalAvailability, RiskAppetite, RiskChange, RiskSettings,
        SavedMarketOrderSetting, WeightCaps,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::ScriptedPresence;
    use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};

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

    fn repository() -> MemorySettingsRepository {
        let repository = MemorySettingsRepository::default();
        for _ in 0_u8..8 {
            repository
                .script_store_risk_settings()
                .push(MemoryReply::Memory);
            repository.script_risk_settings().push(MemoryReply::Memory);
            repository.script_profile().push(MemoryReply::Memory);
        }
        repository
    }

    fn with_orders(orders: OrderPermission) -> RiskSettings {
        RiskSettings::new(
            orders,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Disabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        )
    }

    fn button(repository: &MemorySettingsRepository) -> OrderButtonState {
        run(ReadOrderButton::new(repository)
            .state(MarketTradingStatus::Open, ProposalAvailability::Available))
        .expect("조회")
    }

    #[test]
    fn fresh_install_hides_the_order_button() {
        let repository = repository();
        assert_eq!(
            button(&repository),
            OrderButtonState::Hidden(OrderButtonHiddenReason::OrdersDisabled)
        );
    }

    #[test]
    fn denied_confirmation_keeps_switch_off_and_button_hidden() {
        for refusal in [
            RiskChangeConfirmation::Denied,
            RiskChangeConfirmation::Unavailable,
        ] {
            let repository = repository();
            let presence = ScriptedPresence::new([], [refusal]);
            let events = application::EventBusBuilder::default().build();
            let change = ChangeRiskSettings::new(&repository, &presence, &events);
            let outcome = run(change.execute(with_orders(OrderPermission::Enabled))).expect("변경");
            assert_ne!(
                outcome,
                RiskSettingsChangeOutcome::Changed(application::DeliveryReport::default())
            );
            assert_eq!(presence.risk_change_calls(), vec![RiskChange::EnableOrders]);
            assert_eq!(presence.order_calls(), vec![]);
            assert_eq!(
                button(&repository),
                OrderButtonState::Hidden(OrderButtonHiddenReason::OrdersDisabled)
            );
        }
    }

    #[test]
    fn confirmed_switch_shows_button_only_when_market_open_and_proposal_present() {
        let repository = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed]);
        let events = application::EventBusBuilder::default().build();
        let change = ChangeRiskSettings::new(&repository, &presence, &events);
        let _changed = run(change.execute(with_orders(OrderPermission::Enabled))).expect("변경");
        assert_eq!(button(&repository), OrderButtonState::Shown);
        let reader = ReadOrderButton::new(&repository);
        assert_eq!(
            run(reader.state(MarketTradingStatus::Closed, ProposalAvailability::Available))
                .expect("조회"),
            OrderButtonState::Hidden(OrderButtonHiddenReason::MarketNotOpen)
        );
        assert_eq!(
            run(reader.state(MarketTradingStatus::Open, ProposalAvailability::Missing))
                .expect("조회"),
            OrderButtonState::Hidden(OrderButtonHiddenReason::NoProposal)
        );
        let _off = run(change.execute(with_orders(OrderPermission::Disabled))).expect("끄기");
        assert_eq!(
            button(&repository),
            OrderButtonState::Hidden(OrderButtonHiddenReason::OrdersDisabled)
        );
        assert_eq!(presence.risk_change_calls(), vec![RiskChange::EnableOrders]);
    }
}
