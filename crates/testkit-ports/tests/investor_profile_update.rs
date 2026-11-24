#[cfg(test)]
mod tests {
    use application::{
        InvestorProfileRequest, RiskChangeConfirmation, RiskSettingsChangeOutcome,
        SettingsRepository, UpdateInvestorProfile,
    };
    use domain::{
        ExcludedSecurityType, Exclusions, HorizonSelection, HoursPermissionState, InvestorProfile,
        InvestorProfileError, NumericInput, OrderPermission, ProfileWeight, RiskAppetite,
        RiskChange, RiskSettings, SavedMarketOrderSetting, SectorId, WeightCaps,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        ScriptedPresence,
        memory::{MemoryReply, MemorySettingsRepository},
    };

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
                .push(MemoryReply::Memory);
            repository.script_risk_settings().push(MemoryReply::Memory);
            repository.script_profile().push(MemoryReply::Memory);
            repository.script_store_profile().push(MemoryReply::Memory);
        }
        repository
    }

    fn caps(cap: &str, warning: &str, sector: &str) -> WeightCaps {
        let weight =
            |text| ProfileWeight::parse(&NumericInput::try_new(text).expect("입력")).expect("비중");
        WeightCaps::try_new(weight(cap), weight(warning), weight(sector)).expect("상한")
    }

    fn request(
        horizon: HorizonSelection,
        risk_appetite: RiskAppetite,
        weight_caps: WeightCaps,
    ) -> InvestorProfileRequest {
        InvestorProfileRequest {
            horizon,
            risk_appetite,
            exclusions: Exclusions::default(),
            weight_caps,
        }
    }

    #[test]
    fn safer_or_neutral_changes_save_without_any_confirmation() {
        let repository = repository();
        let presence = ScriptedPresence::new([], []);
        let events = application::EventBusBuilder::default().build();
        let update = UpdateInvestorProfile::new(&repository, &presence, &events);
        let exclusions = Exclusions::try_new(
            vec![SectorId::try_new("bio").expect("섹터")],
            vec![ExcludedSecurityType::Etn],
        )
        .expect("제외");
        let outcome = run(update.execute(InvestorProfileRequest {
            horizon: HorizonSelection::HalfYearOrMore,
            risk_appetite: RiskAppetite::Conservative,
            exclusions: exclusions.clone(),
            weight_caps: caps("0.15", "0.08", "0.30"),
        }));
        assert_eq!(
            outcome,
            Ok(RiskSettingsChangeOutcome::Changed(
                application::DeliveryReport::default()
            ))
        );
        assert_eq!(
            presence.risk_change_calls(),
            Vec::<RiskChange>::new(),
            "확인 창 없음"
        );
        let saved = run(repository.profile()).expect("조회").expect("저장됨");
        assert_eq!(
            saved.horizon().selection(),
            HorizonSelection::HalfYearOrMore
        );
        assert_eq!(saved.risk_appetite(), RiskAppetite::Conservative);
        assert_eq!(saved.exclusions(), &exclusions);
        assert_eq!(
            run(update.execute(InvestorProfileRequest {
                horizon: HorizonSelection::HalfYearOrMore,
                risk_appetite: RiskAppetite::Conservative,
                exclusions,
                weight_caps: caps("0.15", "0.08", "0.30"),
            })),
            Ok(RiskSettingsChangeOutcome::Unchanged)
        );
    }

    #[test]
    fn riskier_changes_each_need_native_confirmation_and_denial_keeps_profile() {
        for refusal in [
            RiskChangeConfirmation::Denied,
            RiskChangeConfirmation::Unavailable,
        ] {
            let repository = repository();
            let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed, refusal]);
            let events = application::EventBusBuilder::default().build();
            let update = UpdateInvestorProfile::new(&repository, &presence, &events);
            let outcome = run(update.execute(request(
                HorizonSelection::WeeksToMonths,
                RiskAppetite::Aggressive,
                caps("0.30", "0.10", "0.50"),
            )));
            let expected = match refusal {
                RiskChangeConfirmation::Denied => RiskSettingsChangeOutcome::Denied,
                RiskChangeConfirmation::Unavailable => RiskSettingsChangeOutcome::Unavailable,
                RiskChangeConfirmation::Confirmed => panic!("거부만"),
            };
            assert_eq!(outcome, Ok(expected));
            assert_eq!(
                presence.risk_change_calls(),
                vec![
                    RiskChange::RelaxWeightCaps(caps("0.30", "0.10", "0.50")),
                    RiskChange::IncreaseRiskAppetite(RiskAppetite::Aggressive),
                ]
            );
            assert_eq!(run(repository.profile()).expect("조회"), None, "저장 안 됨");
        }
    }

    #[test]
    fn hours_horizon_needs_advanced_permission_and_confirmation() {
        let repository = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed]);
        let events = application::EventBusBuilder::default().build();
        let update = UpdateInvestorProfile::new(&repository, &presence, &events);
        assert_eq!(
            run(update.execute(request(
                HorizonSelection::Hours,
                RiskAppetite::Neutral,
                WeightCaps::default(),
            ))),
            Ok(RiskSettingsChangeOutcome::ProfileRejected(
                InvestorProfileError::HoursNotAllowed
            )),
            "고급 허용 전 거부"
        );
        assert_eq!(presence.risk_change_calls(), Vec::<RiskChange>::new());
        run(repository.store_risk_settings(&RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Enabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        )))
        .expect("고급 허용");
        assert_eq!(
            run(update.execute(request(
                HorizonSelection::Hours,
                RiskAppetite::Neutral,
                WeightCaps::default(),
            ))),
            Ok(RiskSettingsChangeOutcome::Changed(
                application::DeliveryReport::default()
            ))
        );
        assert_eq!(presence.risk_change_calls(), vec![RiskChange::AllowHours]);
        let saved = run(repository.profile()).expect("조회").expect("저장됨");
        assert_eq!(saved.horizon().selection(), HorizonSelection::Hours);
        assert_ne!(saved, InvestorProfile::default());
    }
}
