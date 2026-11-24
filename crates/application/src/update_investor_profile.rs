use crate::{
    EventBus, RiskChangeConfirmation, RiskSettingsChangeOutcome, SettingsRepository,
    SystemPortError, user_presence::UserPresence,
};
use domain::{
    Exclusions, HorizonSelection, HoursPermissionState, InvestorProfile, RiskAppetite, RiskChange,
    SavedInvestorProfile, WeightCapChange, WeightCaps,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorProfileRequest {
    pub horizon: HorizonSelection,
    pub risk_appetite: RiskAppetite,
    pub exclusions: Exclusions,
    pub weight_caps: WeightCaps,
}

#[derive(Debug)]
pub struct UpdateInvestorProfile<'a, Repository, Presence> {
    repository: &'a Repository,
    presence: &'a Presence,
    events: &'a EventBus,
}

impl<'a, Repository: SettingsRepository, Presence: UserPresence>
    UpdateInvestorProfile<'a, Repository, Presence>
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
        requested: InvestorProfileRequest,
    ) -> Result<RiskSettingsChangeOutcome, SystemPortError> {
        let stored = self.repository.profile().await?;
        let default_profile = InvestorProfile::default();
        let previous = stored.as_ref().unwrap_or(&default_profile);
        let hours = match self.repository.risk_settings().await? {
            Some(settings) => settings.hours(),
            None => previous.hours_permission().state(),
        };
        let next = match build_profile(&requested, hours) {
            Ok(profile) => profile,
            Err(error) => return Ok(RiskSettingsChangeOutcome::ProfileRejected(error)),
        };
        if next == *previous {
            return Ok(RiskSettingsChangeOutcome::Unchanged);
        }
        for change in profile_confirmations(previous, &next) {
            match self.presence.confirm_risk_change(change).await {
                RiskChangeConfirmation::Confirmed => (),
                RiskChangeConfirmation::Denied => return Ok(RiskSettingsChangeOutcome::Denied),
                RiskChangeConfirmation::Unavailable => {
                    return Ok(RiskSettingsChangeOutcome::Unavailable);
                }
            }
        }
        self.repository.store_profile(&next).await?;
        Ok(RiskSettingsChangeOutcome::Changed(
            self.events.publish(domain::DomainEvent::SettingsChanged),
        ))
    }
}

fn build_profile(
    requested: &InvestorProfileRequest,
    hours: HoursPermissionState,
) -> Result<InvestorProfile, domain::InvestorProfileError> {
    InvestorProfile::restore_from_saved_state(SavedInvestorProfile {
        horizon: requested.horizon,
        hours_permission: hours,
        risk_appetite: requested.risk_appetite,
        sectors: requested.exclusions.sectors().to_vec(),
        security_types: requested.exclusions.security_types().to_vec(),
        instrument_cap: requested.weight_caps.instrument_cap().as_ratio(),
        instrument_warning: requested.weight_caps.instrument_warning().as_ratio(),
        sector_cap: requested.weight_caps.sector_cap().as_ratio(),
    })
}

#[must_use]
pub fn profile_confirmations(
    previous: &InvestorProfile,
    next: &InvestorProfile,
) -> Vec<RiskChange> {
    let mut changes = Vec::new();
    if next.horizon().selection() == HorizonSelection::Hours
        && previous.horizon().selection() != HorizonSelection::Hours
    {
        changes.push(RiskChange::AllowHours);
    }
    match next.weight_caps().change_from(previous.weight_caps()) {
        WeightCapChange::Relaxed => changes.push(RiskChange::RelaxWeightCaps(next.weight_caps())),
        WeightCapChange::NotRelaxed => (),
    }
    if next.risk_appetite() > previous.risk_appetite() {
        changes.push(RiskChange::IncreaseRiskAppetite(next.risk_appetite()));
    }
    changes
}
