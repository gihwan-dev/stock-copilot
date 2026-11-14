use crate::{ConcentrationWarning, PolicyConcentration};

#[must_use]
pub const fn concentration_for_policy(warning: ConcentrationWarning) -> PolicyConcentration {
    match warning {
        ConcentrationWarning::Warning => PolicyConcentration::Warning,
        ConcentrationWarning::Clear => PolicyConcentration::Clear,
        ConcentrationWarning::NotApplicable => PolicyConcentration::NotApplicable,
        ConcentrationWarning::Unavailable => PolicyConcentration::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::concentration_for_policy;
    use crate::{ConcentrationWarning, PolicyConcentration};
    #[test]
    fn all_four_concentration_states_are_preserved() {
        for (warning, policy) in [
            (ConcentrationWarning::Warning, PolicyConcentration::Warning),
            (ConcentrationWarning::Clear, PolicyConcentration::Clear),
            (
                ConcentrationWarning::NotApplicable,
                PolicyConcentration::NotApplicable,
            ),
            (
                ConcentrationWarning::Unavailable,
                PolicyConcentration::Unavailable,
            ),
        ] {
            assert_eq!(concentration_for_policy(warning), policy);
        }
    }
}
