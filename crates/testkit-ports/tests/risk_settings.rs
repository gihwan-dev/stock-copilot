#[cfg(test)]
mod tests {
    use application::{
        ChangeRiskSettings, RiskChangeConfirmation, RiskSettingsChangeOutcome, SettingsRepository,
    };
    use domain::{
        HoursPermissionState, OrderPermission, RiskAppetite, RiskSettings, SavedMarketOrderSetting,
        WeightCaps,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{ScriptedPresence, memory::MemorySettingsRepository};

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
        for _ in 0_u8..16 {
            repository
                .script_store_risk_settings()
                .push(testkit_ports::memory::MemoryReply::Memory);
            repository
                .script_risk_settings()
                .push(testkit_ports::memory::MemoryReply::Memory);
            repository
                .script_profile()
                .push(testkit_ports::memory::MemoryReply::Memory);
        }
        repository
    }
    fn safe() -> RiskSettings {
        RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Disabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        )
    }
    fn risky() -> RiskSettings {
        RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Enabled,
            HoursPermissionState::Enabled,
            WeightCaps::default(),
            RiskAppetite::Aggressive,
        )
    }
    #[test]
    fn every_non_confirmation_preserves_all_settings_and_never_requests_order_presence() {
        for outcome in [
            RiskChangeConfirmation::Denied,
            RiskChangeConfirmation::Unavailable,
        ] {
            let repository = repository();
            run(repository.store_risk_settings(&safe())).expect("초기 저장");
            let presence = ScriptedPresence::new([], [outcome]);
            let events = application::EventBusBuilder::default().build();
            let change = ChangeRiskSettings::new(&repository, &presence, &events);
            assert_eq!(
                run(change.execute(risky())),
                Ok(RiskSettingsChangeOutcome::NotConfirmed(outcome))
            );
            assert_eq!(run(repository.risk_settings()).expect("조회"), Some(safe()));
            assert_eq!(presence.order_calls(), vec![]);
            assert_eq!(
                presence.risk_change_calls(),
                vec![domain::RiskChange::EnableOrders]
            );
        }
    }
    #[test]
    fn all_confirmations_precede_one_atomic_write_and_reductions_need_none() {
        let repository = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed; 4]);
        let events = application::EventBusBuilder::default().build();
        let change = ChangeRiskSettings::new(&repository, &presence, &events);
        assert_eq!(
            run(change.execute(risky())),
            Ok(RiskSettingsChangeOutcome::Changed(
                application::DeliveryReport::default()
            ))
        );
        assert_eq!(
            run(repository.risk_settings()).expect("조회"),
            Some(risky())
        );
        assert_eq!(
            presence.risk_change_calls(),
            vec![
                domain::RiskChange::EnableOrders,
                domain::RiskChange::AllowMarketOrders,
                domain::RiskChange::AllowHours,
                domain::RiskChange::IncreaseRiskAppetite(RiskAppetite::Aggressive)
            ]
        );
        assert_eq!(presence.order_calls(), vec![]);
        assert_eq!(
            run(change.execute(safe())),
            Ok(RiskSettingsChangeOutcome::Changed(
                application::DeliveryReport::default()
            ))
        );
        assert_eq!(run(repository.risk_settings()).expect("조회"), Some(safe()));
        assert_eq!(presence.risk_change_calls().len(), 4);
        assert_eq!(
            run(change.execute(safe())),
            Ok(RiskSettingsChangeOutcome::Unchanged)
        );
    }
    fn caps(cap: &str, warning: &str, sector: &str) -> WeightCaps {
        let weight = |value| {
            domain::ProfileWeight::parse(&domain::NumericInput::try_new(value).expect("숫자"))
                .expect("비중")
        };
        WeightCaps::try_new(weight(cap), weight(warning), weight(sector)).expect("상한")
    }
    fn target_caps(weights: WeightCaps) -> RiskSettings {
        RiskSettings::new(
            safe().orders(),
            safe().market_orders(),
            safe().hours(),
            weights,
            safe().risk_appetite(),
        )
    }
    fn targets() -> Vec<(RiskSettings, Vec<domain::RiskChange>)> {
        use domain::RiskChange;
        vec![
            (
                RiskSettings::new(
                    OrderPermission::Enabled,
                    safe().market_orders(),
                    safe().hours(),
                    safe().weight_caps(),
                    safe().risk_appetite(),
                ),
                vec![RiskChange::EnableOrders],
            ),
            (
                RiskSettings::new(
                    safe().orders(),
                    SavedMarketOrderSetting::Enabled,
                    safe().hours(),
                    safe().weight_caps(),
                    safe().risk_appetite(),
                ),
                vec![RiskChange::AllowMarketOrders],
            ),
            (
                RiskSettings::new(
                    safe().orders(),
                    safe().market_orders(),
                    HoursPermissionState::Enabled,
                    safe().weight_caps(),
                    safe().risk_appetite(),
                ),
                vec![RiskChange::AllowHours],
            ),
        ]
        .into_iter()
        .chain(weight_targets())
        .collect()
    }
    fn weight_targets() -> Vec<(RiskSettings, Vec<domain::RiskChange>)> {
        use domain::RiskChange;
        vec![
            (
                target_caps(caps("0.25", "0.13", "0.37")),
                vec![RiskChange::RelaxWeightCaps(caps("0.25", "0.13", "0.37"))],
            ),
            (
                target_caps(caps("0.17", "0.09", "0.43")),
                vec![RiskChange::RelaxWeightCaps(caps("0.17", "0.09", "0.43"))],
            ),
            (
                RiskSettings::new(
                    safe().orders(),
                    safe().market_orders(),
                    safe().hours(),
                    safe().weight_caps(),
                    RiskAppetite::Aggressive,
                ),
                vec![RiskChange::IncreaseRiskAppetite(RiskAppetite::Aggressive)],
            ),
            (target_caps(caps("0.2", "0.12", "0.4")), vec![]),
            (
                RiskSettings::new(
                    safe().orders(),
                    safe().market_orders(),
                    safe().hours(),
                    caps("0.18", "0.08", "0.36"),
                    RiskAppetite::Conservative,
                ),
                vec![],
            ),
        ]
    }
    #[test]
    fn each_field_preserves_exact_values_and_checks_only_risk_increases() {
        for (target, expected) in targets() {
            for response in [
                RiskChangeConfirmation::Confirmed,
                RiskChangeConfirmation::Denied,
                RiskChangeConfirmation::Unavailable,
            ] {
                let repository = repository();
                run(repository.store_risk_settings(&safe())).expect("초기값");
                let presence = ScriptedPresence::new([], [response]);
                let events = application::EventBusBuilder::default().build();
                let change = ChangeRiskSettings::new(&repository, &presence, &events);
                let applied = expected.is_empty() || response == RiskChangeConfirmation::Confirmed;
                assert_eq!(
                    run(change.execute(target)),
                    Ok(if applied {
                        RiskSettingsChangeOutcome::Changed(application::DeliveryReport::default())
                    } else {
                        RiskSettingsChangeOutcome::NotConfirmed(response)
                    })
                );
                assert_eq!(
                    run(repository.risk_settings()).expect("조회"),
                    Some(if applied { target } else { safe() })
                );
                assert_eq!(presence.risk_change_calls(), expected);
                assert_eq!(presence.order_calls(), vec![]);
            }
        }
    }
    #[test]
    fn later_denial_does_not_save_earlier_confirmed_fields() {
        for denied in 0..4 {
            let repository = repository();
            let replies = std::iter::repeat_n(RiskChangeConfirmation::Confirmed, denied)
                .chain([RiskChangeConfirmation::Denied]);
            let presence = ScriptedPresence::new([], replies);
            let events = application::EventBusBuilder::default().build();
            let change = ChangeRiskSettings::new(&repository, &presence, &events);
            assert_eq!(
                run(change.execute(risky())),
                Ok(RiskSettingsChangeOutcome::NotConfirmed(
                    RiskChangeConfirmation::Denied
                ))
            );
            assert_eq!(run(repository.risk_settings()).expect("조회"), None);
            assert_eq!(presence.risk_change_calls().len(), denied + 1);
            assert_eq!(presence.order_calls(), vec![]);
        }
    }
    #[test]
    fn repository_failures_preserve_settings_and_propagate_exact_error() {
        use application::{SystemFailure, SystemPort, SystemPortError};
        use testkit_ports::memory::MemoryReply;
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        for phase in 0_u8..3 {
            let repository = MemorySettingsRepository::default();
            repository
                .script_risk_settings()
                .push(MemoryReply::Return(if phase == 0 {
                    Err(error)
                } else {
                    Ok(None)
                }));
            repository
                .script_profile()
                .push(MemoryReply::Return(if phase == 1 {
                    Err(error)
                } else {
                    Ok(None)
                }));
            repository
                .script_store_risk_settings()
                .push(MemoryReply::Return(Err(error)));
            let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed; 4]);
            let events = application::EventBusBuilder::default().build();
            assert_eq!(
                run(ChangeRiskSettings::new(&repository, &presence, &events).execute(risky())),
                Err(error)
            );
            assert_eq!(
                presence.risk_change_calls().len(),
                if phase == 2 { 4 } else { 0 }
            );
            assert_eq!(presence.order_calls(), vec![]);
        }
    }
    #[derive(Clone)]
    struct Observer(std::sync::Arc<std::sync::RwLock<Vec<domain::DomainEvent>>>);
    impl application::EventSubscriber for Observer {
        fn on_event(
            &self,
            event: &domain::DomainEvent,
        ) -> Result<(), application::SubscriberFailure> {
            let mut recorded = match self.0.write() {
                Ok(recorded) => recorded,
                Err(poisoned) => poisoned.into_inner(),
            };
            recorded.push(event.clone());
            Err(application::SubscriberFailure::Unavailable)
        }
    }
    #[test]
    fn settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success() {
        let observer = Observer(std::sync::Arc::default());
        let mut builder = application::EventBusBuilder::default();
        let subscription = builder.subscribe(
            std::iter::once(domain::DomainEventKind::SettingsChanged).collect(),
            observer.clone(),
        );
        let events = builder.build();
        let repository = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed; 4]);
        let change = ChangeRiskSettings::new(&repository, &presence, &events);
        assert_eq!(
            run(change.execute(risky())),
            Ok(RiskSettingsChangeOutcome::Changed(
                application::DeliveryReport {
                    failures: vec![application::DeliveryFailure {
                        subscription,
                        cause: application::SubscriberFailure::Unavailable
                    }]
                }
            ))
        );
        assert_eq!(
            run(repository.risk_settings()).expect("저장"),
            Some(risky())
        );
        assert_eq!(
            *observer.0.read().expect("사건"),
            vec![domain::DomainEvent::SettingsChanged]
        );
        assert_eq!(
            run(change.execute(risky())),
            Ok(RiskSettingsChangeOutcome::Unchanged)
        );
        assert_eq!(
            *observer.0.read().expect("사건"),
            vec![domain::DomainEvent::SettingsChanged]
        );
    }
    #[test]
    fn missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults() {
        use testkit_ports::memory::MemoryReply;
        for profile in [
            domain::InvestorProfile::default(),
            domain::InvestorProfile::default()
                .with_risk_appetite(RiskAppetite::Conservative)
                .with_weight_caps(caps("0.19", "0.07", "0.38")),
        ] {
            let repository = MemorySettingsRepository::default();
            repository
                .script_risk_settings()
                .push(MemoryReply::Return(Ok(None)));
            repository
                .script_profile()
                .push(MemoryReply::Return(Ok(Some(profile.clone()))));
            let presence = ScriptedPresence::new([], []);
            let events = application::EventBusBuilder::default().build();
            let target = RiskSettings::new(
                OrderPermission::Disabled,
                SavedMarketOrderSetting::Disabled,
                profile.hours_permission().state(),
                profile.weight_caps(),
                profile.risk_appetite(),
            );
            assert_eq!(
                run(ChangeRiskSettings::new(&repository, &presence, &events).execute(target)),
                Ok(RiskSettingsChangeOutcome::Unchanged)
            );
            assert_eq!(presence.calls(), vec![]);
            assert_eq!(repository.script_store_risk_settings().calls(), vec![]);
        }
    }
}
