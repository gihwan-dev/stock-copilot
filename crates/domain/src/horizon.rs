#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizonSelection {
    FewDays,
    WeeksToMonths,
    HalfYearOrMore,
    Hours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Horizon(HorizonSelection);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoursPermissionState {
    #[default]
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HoursPermission(HoursPermissionState);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvancedHoursConfirmation(AdvancedHoursSeal);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdvancedHoursSeal;

impl HoursPermission {
    #[must_use]
    pub fn from_advanced_confirmation(confirmation: AdvancedHoursConfirmation) -> Self {
        let AdvancedHoursConfirmation(AdvancedHoursSeal) = confirmation;
        Self(HoursPermissionState::Enabled)
    }

    #[must_use]
    pub fn state(self) -> HoursPermissionState {
        self.0
    }

    pub(crate) fn restore_from_saved_state(state: HoursPermissionState) -> Self {
        Self(state)
    }
}

impl Horizon {
    #[must_use]
    pub fn new(selection: HorizonSelection) -> Self {
        Self(selection)
    }

    #[must_use]
    pub fn selection(self) -> HorizonSelection {
        self.0
    }
}

impl Default for Horizon {
    fn default() -> Self {
        Self(HorizonSelection::WeeksToMonths)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdvancedHoursConfirmation, AdvancedHoursSeal, Horizon, HorizonSelection, HoursPermission,
        HoursPermissionState,
    };
    use crate::{Exclusions, InvestorProfile, InvestorProfileError, RiskAppetite, WeightCaps};

    #[test]
    fn ordinary_construction_rejects_hours_and_default_is_weeks_to_months() {
        assert_eq!(
            Horizon::default().selection(),
            HorizonSelection::WeeksToMonths
        );
        assert_eq!(
            HoursPermission::default().state(),
            HoursPermissionState::Disabled
        );
        for selection in [
            HorizonSelection::FewDays,
            HorizonSelection::WeeksToMonths,
            HorizonSelection::HalfYearOrMore,
        ] {
            let horizon = Horizon::new(selection);
            assert_eq!(horizon.selection(), selection);
            assert_eq!(
                InvestorProfile::try_new(
                    horizon,
                    HoursPermission::default(),
                    RiskAppetite::Neutral,
                    Exclusions::default(),
                    WeightCaps::default()
                )
                .expect("일반 기간")
                .horizon(),
                horizon
            );
        }
        assert_eq!(
            InvestorProfile::default().with_horizon(Horizon::new(HorizonSelection::Hours)),
            Err(InvestorProfileError::HoursNotAllowed)
        );
    }

    #[test]
    fn hours_requires_sealed_advanced_native_confirmation() {
        let evidence = AdvancedHoursConfirmation(AdvancedHoursSeal);
        let permission = HoursPermission::from_advanced_confirmation(evidence);
        assert_eq!(permission.state(), HoursPermissionState::Enabled);
        let allowed = InvestorProfile::default()
            .with_hours_permission(permission)
            .expect("고급 허용");
        assert_eq!(
            allowed.horizon().selection(),
            HorizonSelection::WeeksToMonths
        );
        assert_eq!(allowed.hours_permission(), permission);
        let hours = allowed
            .with_horizon(Horizon::new(HorizonSelection::Hours))
            .expect("허용된 기간");
        assert_eq!(hours.horizon().selection(), HorizonSelection::Hours);
        assert_eq!(hours.hours_permission(), permission);
    }
}
