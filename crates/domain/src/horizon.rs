#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizonSelection {
    FewDays,
    WeeksToMonths,
    HalfYearOrMore,
    Hours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Horizon(HorizonSelection);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvancedHoursConfirmation(AdvancedHoursSeal);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdvancedHoursSeal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizonError {
    ConfirmationRequired,
}

impl Horizon {
    pub fn try_new(selection: HorizonSelection) -> Result<Self, HorizonError> {
        match selection {
            HorizonSelection::Hours => Err(HorizonError::ConfirmationRequired),
            HorizonSelection::FewDays
            | HorizonSelection::WeeksToMonths
            | HorizonSelection::HalfYearOrMore => Ok(Self(selection)),
        }
    }

    #[must_use]
    pub fn from_advanced_confirmation(confirmation: AdvancedHoursConfirmation) -> Self {
        let AdvancedHoursConfirmation(AdvancedHoursSeal) = confirmation;
        Self(HorizonSelection::Hours)
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
        AdvancedHoursConfirmation, AdvancedHoursSeal, Horizon, HorizonError, HorizonSelection,
    };

    #[test]
    fn ordinary_construction_rejects_hours_and_default_is_weeks_to_months() {
        assert_eq!(
            Horizon::default().selection(),
            HorizonSelection::WeeksToMonths
        );
        for selection in [
            HorizonSelection::FewDays,
            HorizonSelection::WeeksToMonths,
            HorizonSelection::HalfYearOrMore,
        ] {
            assert_eq!(
                Horizon::try_new(selection).expect("일반 기간").selection(),
                selection
            );
        }
        assert_eq!(
            Horizon::try_new(HorizonSelection::Hours),
            Err(HorizonError::ConfirmationRequired)
        );
    }

    #[test]
    fn hours_requires_sealed_advanced_native_confirmation() {
        let evidence = AdvancedHoursConfirmation(AdvancedHoursSeal);
        assert_eq!(
            Horizon::from_advanced_confirmation(evidence).selection(),
            HorizonSelection::Hours
        );
    }
}
