use crate::{
    DeliveryReport, EventBus, RiskChangeConfirmation, SettingsRepository, SystemPortError,
    user_presence::UserPresence,
};
use domain::{
    HoursPermissionState, InvestorProfile, OrderPermission, RiskChange, RiskSettings,
    SavedMarketOrderSetting, WeightCapChange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskSettingsChangeOutcome {
    Changed(DeliveryReport),
    Unchanged,
    NotConfirmed(RiskChangeConfirmation),
}

#[derive(Debug)]
pub struct ChangeRiskSettings<'a, Repository, Presence> {
    repository: &'a Repository,
    presence: &'a Presence,
    events: &'a EventBus,
}
impl<'a, Repository: SettingsRepository, Presence: UserPresence>
    ChangeRiskSettings<'a, Repository, Presence>
{
    #[must_use]
    pub const fn new(
        repository: &'a Repository,
        presence: &'a Presence,
        events: &'a EventBus,
    ) -> Self {
        Self {
            repository,
            presence,
            events,
        }
    }

    pub async fn execute(
        &self,
        requested: RiskSettings,
    ) -> Result<RiskSettingsChangeOutcome, SystemPortError> {
        let previous = if let Some(settings) = self.repository.risk_settings().await? {
            settings
        } else {
            self.repository.profile().await?.as_ref().map_or_else(
                || initial_settings(&InvestorProfile::default()),
                initial_settings,
            )
        };
        let Some(changes) = plan(previous, requested) else {
            return Ok(RiskSettingsChangeOutcome::Unchanged);
        };
        for change in changes {
            match self.presence.confirm_risk_change(change).await {
                RiskChangeConfirmation::Confirmed => (),
                outcome
                @ (RiskChangeConfirmation::Denied | RiskChangeConfirmation::Unavailable) => {
                    return Ok(RiskSettingsChangeOutcome::NotConfirmed(outcome));
                }
            }
        }
        self.repository.store_risk_settings(&requested).await?;
        Ok(RiskSettingsChangeOutcome::Changed(
            self.events.publish(domain::DomainEvent::SettingsChanged),
        ))
    }
}
fn initial_settings(profile: &InvestorProfile) -> RiskSettings {
    RiskSettings::new(
        OrderPermission::Disabled,
        SavedMarketOrderSetting::Disabled,
        profile.hours_permission().state(),
        profile.weight_caps(),
        profile.risk_appetite(),
    )
}
fn plan(previous: RiskSettings, requested: RiskSettings) -> Option<Vec<RiskChange>> {
    if requested == previous {
        None
    } else {
        Some(confirmations(previous, requested))
    }
}
fn confirmations(previous: RiskSettings, requested: RiskSettings) -> Vec<RiskChange> {
    let mut changes = Vec::new();
    if previous.orders() == OrderPermission::Disabled
        && requested.orders() == OrderPermission::Enabled
    {
        changes.push(RiskChange::EnableOrders);
    }
    if previous.market_orders() == SavedMarketOrderSetting::Disabled
        && requested.market_orders() == SavedMarketOrderSetting::Enabled
    {
        changes.push(RiskChange::AllowMarketOrders);
    }
    if previous.hours() == HoursPermissionState::Disabled
        && requested.hours() == HoursPermissionState::Enabled
    {
        changes.push(RiskChange::AllowHours);
    }
    match requested.weight_caps().change_from(previous.weight_caps()) {
        WeightCapChange::Relaxed => {
            changes.push(RiskChange::RelaxWeightCaps(requested.weight_caps()));
        }
        WeightCapChange::NotRelaxed => (),
    }
    if requested.risk_appetite() > previous.risk_appetite() {
        changes.push(RiskChange::IncreaseRiskAppetite(requested.risk_appetite()));
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::{initial_settings, plan};
    use domain::{
        HoursPermissionState, InvestorProfile, NumericInput, OrderPermission, ProfileWeight,
        RiskAppetite, RiskChange, RiskSettings, SavedMarketOrderSetting, WeightCaps,
    };
    fn caps(cap: &str, warning: &str, sector: &str) -> WeightCaps {
        let weight =
            |text| ProfileWeight::parse(&NumericInput::try_new(text).expect("입력")).expect("비중");
        WeightCaps::try_new(weight(cap), weight(warning), weight(sector)).expect("상한")
    }
    fn safe() -> RiskSettings {
        initial_settings(&InvestorProfile::default())
    }
    #[test]
    fn no_change_and_initial_values_match_documented_defaults() {
        assert_eq!(
            safe(),
            RiskSettings::new(
                OrderPermission::Disabled,
                SavedMarketOrderSetting::Disabled,
                HoursPermissionState::Disabled,
                WeightCaps::default(),
                RiskAppetite::Neutral
            )
        );
        assert_eq!(plan(safe(), safe()), None);
        let profile = InvestorProfile::default()
            .with_weight_caps(caps("0.18", "0.07", "0.36"))
            .with_risk_appetite(RiskAppetite::Conservative);
        assert_eq!(
            initial_settings(&profile),
            RiskSettings::new(
                OrderPermission::Disabled,
                SavedMarketOrderSetting::Disabled,
                HoursPermissionState::Disabled,
                profile.weight_caps(),
                RiskAppetite::Conservative
            )
        );
    }
    #[test]
    fn every_permission_transition_is_independently_planned() {
        let base = safe();
        let on = RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Enabled,
            HoursPermissionState::Enabled,
            base.weight_caps(),
            base.risk_appetite(),
        );
        assert_eq!(
            plan(base, on),
            Some(vec![
                RiskChange::EnableOrders,
                RiskChange::AllowMarketOrders,
                RiskChange::AllowHours
            ])
        );
        assert_eq!(plan(on, base), Some(vec![]));
        assert_eq!(plan(on, on), None);
        for target in [
            RiskSettings::new(
                OrderPermission::Enabled,
                base.market_orders(),
                base.hours(),
                base.weight_caps(),
                base.risk_appetite(),
            ),
            RiskSettings::new(
                base.orders(),
                SavedMarketOrderSetting::Enabled,
                base.hours(),
                base.weight_caps(),
                base.risk_appetite(),
            ),
            RiskSettings::new(
                base.orders(),
                base.market_orders(),
                HoursPermissionState::Enabled,
                base.weight_caps(),
                base.risk_appetite(),
            ),
        ]
        .into_iter()
        .zip([
            RiskChange::EnableOrders,
            RiskChange::AllowMarketOrders,
            RiskChange::AllowHours,
        ]) {
            assert_eq!(plan(base, target.0), Some(vec![target.1]));
            assert_eq!(plan(target.0, base), Some(vec![]));
        }
    }
    #[test]
    fn each_cap_can_independently_relax_and_warning_is_not_a_cap() {
        let base = safe();
        for weights in [caps("0.25", "0.13", "0.37"), caps("0.17", "0.09", "0.43")] {
            let requested = RiskSettings::new(
                base.orders(),
                base.market_orders(),
                base.hours(),
                weights,
                base.risk_appetite(),
            );
            assert_eq!(
                plan(base, requested),
                Some(vec![RiskChange::RelaxWeightCaps(weights)])
            );
        }
        for weights in [caps("0.2", "0.12", "0.4"), caps("0.18", "0.08", "0.36")] {
            assert_eq!(
                plan(
                    base,
                    RiskSettings::new(
                        base.orders(),
                        base.market_orders(),
                        base.hours(),
                        weights,
                        base.risk_appetite()
                    )
                ),
                Some(vec![])
            );
        }
    }
    fn appetite_expectation(
        order: std::cmp::Ordering,
        appetite: RiskAppetite,
    ) -> Option<Vec<RiskChange>> {
        match order {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => Some(vec![]),
            std::cmp::Ordering::Greater => Some(vec![RiskChange::IncreaseRiskAppetite(appetite)]),
        }
    }
    #[test]
    fn all_nine_appetite_pairs_follow_the_documented_order() {
        let appetites = [
            RiskAppetite::Conservative,
            RiskAppetite::Neutral,
            RiskAppetite::Aggressive,
        ];
        let settings = |appetite| {
            RiskSettings::new(
                safe().orders(),
                safe().market_orders(),
                safe().hours(),
                safe().weight_caps(),
                appetite,
            )
        };
        for (old_index, old) in appetites.into_iter().enumerate() {
            for (new_index, new) in appetites.into_iter().enumerate() {
                let expected = appetite_expectation(new_index.cmp(&old_index), new);
                assert_eq!(plan(settings(old), settings(new)), expected);
            }
        }
    }
}
