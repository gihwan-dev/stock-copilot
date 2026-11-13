use crate::{Exclusions, Horizon, WeightCaps};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiskAppetite {
    Conservative,
    #[default]
    Neutral,
    Aggressive,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvestorProfile {
    horizon: Horizon,
    risk_appetite: RiskAppetite,
    exclusions: Exclusions,
    weight_caps: WeightCaps,
}

impl InvestorProfile {
    #[must_use]
    pub fn new(
        horizon: Horizon,
        risk_appetite: RiskAppetite,
        exclusions: Exclusions,
        weight_caps: WeightCaps,
    ) -> Self {
        Self {
            horizon,
            risk_appetite,
            exclusions,
            weight_caps,
        }
    }

    #[must_use]
    pub fn horizon(&self) -> Horizon {
        self.horizon
    }
    #[must_use]
    pub fn risk_appetite(&self) -> RiskAppetite {
        self.risk_appetite
    }
    #[must_use]
    pub fn exclusions(&self) -> &Exclusions {
        &self.exclusions
    }
    #[must_use]
    pub fn weight_caps(&self) -> WeightCaps {
        self.weight_caps
    }

    #[must_use]
    pub fn with_horizon(&self, horizon: Horizon) -> Self {
        Self {
            horizon,
            ..self.clone()
        }
    }
    #[must_use]
    pub fn with_risk_appetite(&self, risk_appetite: RiskAppetite) -> Self {
        Self {
            risk_appetite,
            ..self.clone()
        }
    }
    #[must_use]
    pub fn with_exclusions(&self, exclusions: Exclusions) -> Self {
        Self {
            exclusions,
            ..self.clone()
        }
    }
    #[must_use]
    pub fn with_weight_caps(&self, weight_caps: WeightCaps) -> Self {
        Self {
            weight_caps,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Exclusions, Horizon, HorizonSelection, InvestorProfile, RiskAppetite, WeightCaps};

    #[test]
    fn defaults_match_design_and_replacements_preserve_original() {
        let profile = InvestorProfile::default();
        assert_eq!(
            profile.horizon().selection(),
            HorizonSelection::WeeksToMonths
        );
        assert_eq!(profile.risk_appetite(), RiskAppetite::Neutral);
        assert_eq!(profile.exclusions(), &Exclusions::default());
        assert_eq!(profile.weight_caps(), WeightCaps::default());
        let days = Horizon::try_new(HorizonSelection::FewDays).expect("기간");
        let changed = profile
            .with_horizon(days)
            .with_risk_appetite(RiskAppetite::Conservative);
        assert_eq!(changed.horizon(), days);
        assert_eq!(changed.risk_appetite(), RiskAppetite::Conservative);
        assert_eq!(profile, InvestorProfile::default());
        let aggressive = InvestorProfile::new(
            days,
            RiskAppetite::Aggressive,
            Exclusions::default(),
            WeightCaps::default(),
        );
        assert_eq!(aggressive.risk_appetite(), RiskAppetite::Aggressive);
        let exclusions = Exclusions::try_new(
            vec![crate::SectorId::try_new("sector-1").expect("섹터")],
            vec![crate::ExcludedSecurityType::Etf],
        )
        .expect("제외");
        let weight =
            crate::ProfileWeight::parse(&crate::NumericInput::try_new("0.5").expect("길이"))
                .expect("비율");
        let caps = WeightCaps::try_new(weight, weight, weight).expect("상한");
        let customized = aggressive
            .with_exclusions(exclusions.clone())
            .with_weight_caps(caps);
        assert_eq!(
            customized,
            InvestorProfile::new(days, RiskAppetite::Aggressive, exclusions, caps)
        );
        assert_eq!(customized.exclusions().sectors().len(), 1);
        assert_eq!(customized.weight_caps(), caps);
        assert_eq!(aggressive.exclusions(), &Exclusions::default());
        assert_eq!(aggressive.weight_caps(), WeightCaps::default());
        assert_eq!(profile, InvestorProfile::default());
    }
}
