use crate::{
    ExcludedSecurityType, Exclusions, ExclusionsError, Horizon, HorizonSelection, HoursPermission,
    HoursPermissionState, ProfileWeight, ProfileWeightError, Ratio, SectorId, WeightCaps,
    WeightCapsError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RiskAppetite {
    Conservative,
    #[default]
    Neutral,
    Aggressive,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvestorProfile {
    horizon: Horizon,
    hours_permission: HoursPermission,
    risk_appetite: RiskAppetite,
    exclusions: Exclusions,
    weight_caps: WeightCaps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedInvestorProfile {
    pub horizon: HorizonSelection,
    pub hours_permission: HoursPermissionState,
    pub risk_appetite: RiskAppetite,
    pub sectors: Vec<SectorId>,
    pub security_types: Vec<ExcludedSecurityType>,
    pub instrument_cap: Ratio,
    pub instrument_warning: Ratio,
    pub sector_cap: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvestorProfileError {
    HoursNotAllowed,
    WeightValue(ProfileWeightError),
    WeightCaps(WeightCapsError),
    Exclusions(ExclusionsError),
}

impl InvestorProfile {
    pub fn try_new(
        horizon: Horizon,
        hours_permission: HoursPermission,
        risk_appetite: RiskAppetite,
        exclusions: Exclusions,
        weight_caps: WeightCaps,
    ) -> Result<Self, InvestorProfileError> {
        if horizon.selection() == HorizonSelection::Hours
            && hours_permission.state() == HoursPermissionState::Disabled
        {
            return Err(InvestorProfileError::HoursNotAllowed);
        }
        Ok(Self {
            horizon,
            hours_permission,
            risk_appetite,
            exclusions,
            weight_caps,
        })
    }

    pub fn restore_from_saved_state(
        saved: SavedInvestorProfile,
    ) -> Result<Self, InvestorProfileError> {
        let cap = ProfileWeight::try_new(saved.instrument_cap)
            .map_err(InvestorProfileError::WeightValue)?;
        let warning = ProfileWeight::try_new(saved.instrument_warning)
            .map_err(InvestorProfileError::WeightValue)?;
        let sector =
            ProfileWeight::try_new(saved.sector_cap).map_err(InvestorProfileError::WeightValue)?;
        let weight_caps =
            WeightCaps::try_new(cap, warning, sector).map_err(InvestorProfileError::WeightCaps)?;
        let exclusions = Exclusions::try_new(saved.sectors, saved.security_types)
            .map_err(InvestorProfileError::Exclusions)?;
        Self::try_new(
            Horizon::new(saved.horizon),
            HoursPermission::restore_from_saved_state(saved.hours_permission),
            saved.risk_appetite,
            exclusions,
            weight_caps,
        )
    }

    #[must_use]
    pub fn saved_state(&self) -> SavedInvestorProfile {
        SavedInvestorProfile {
            horizon: self.horizon.selection(),
            hours_permission: self.hours_permission.state(),
            risk_appetite: self.risk_appetite,
            sectors: self.exclusions.sectors().to_vec(),
            security_types: self.exclusions.security_types().to_vec(),
            instrument_cap: self.weight_caps.instrument_cap().as_ratio(),
            instrument_warning: self.weight_caps.instrument_warning().as_ratio(),
            sector_cap: self.weight_caps.sector_cap().as_ratio(),
        }
    }

    #[must_use]
    pub fn horizon(&self) -> Horizon {
        self.horizon
    }
    #[must_use]
    pub fn hours_permission(&self) -> HoursPermission {
        self.hours_permission
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

    pub fn with_horizon(&self, horizon: Horizon) -> Result<Self, InvestorProfileError> {
        Self::try_new(
            horizon,
            self.hours_permission,
            self.risk_appetite,
            self.exclusions.clone(),
            self.weight_caps,
        )
    }
    pub fn with_hours_permission(
        &self,
        hours_permission: HoursPermission,
    ) -> Result<Self, InvestorProfileError> {
        Self::try_new(
            self.horizon,
            hours_permission,
            self.risk_appetite,
            self.exclusions.clone(),
            self.weight_caps,
        )
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
    use crate::{
        Exclusions, Horizon, HorizonSelection, HoursPermission, InvestorProfile, RiskAppetite,
        WeightCaps,
    };

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
        let days = Horizon::new(HorizonSelection::FewDays);
        let changed = profile
            .with_horizon(days)
            .expect("기간 변경")
            .with_risk_appetite(RiskAppetite::Conservative);
        assert_eq!(changed.horizon(), days);
        assert_eq!(changed.risk_appetite(), RiskAppetite::Conservative);
        assert_eq!(profile, InvestorProfile::default());
        let aggressive = InvestorProfile::try_new(
            days,
            HoursPermission::default(),
            RiskAppetite::Aggressive,
            Exclusions::default(),
            WeightCaps::default(),
        )
        .expect("프로필");
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
            InvestorProfile::try_new(
                days,
                HoursPermission::default(),
                RiskAppetite::Aggressive,
                exclusions,
                caps
            )
            .expect("프로필")
        );
        assert_eq!(customized.exclusions().sectors().len(), 1);
        assert_eq!(customized.weight_caps(), caps);
        assert_eq!(aggressive.exclusions(), &Exclusions::default());
        assert_eq!(aggressive.weight_caps(), WeightCaps::default());
        assert_eq!(profile, InvestorProfile::default());
    }
}

#[cfg(test)]
mod review_tests {
    use crate::{
        Exclusions, ExclusionsError, Horizon, HorizonSelection, HoursPermission,
        HoursPermissionState, InvestorProfile, InvestorProfileError, NumericInput,
        ProfileWeightError, Ratio, RiskAppetite, SavedInvestorProfile, SectorId, WeightCapsError,
    };
    use proptest::test_runner::{Config, TestRunner};

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("길이")).expect("비율")
    }

    fn saved_hours() -> SavedInvestorProfile {
        SavedInvestorProfile {
            horizon: HorizonSelection::Hours,
            hours_permission: HoursPermissionState::Enabled,
            risk_appetite: RiskAppetite::Aggressive,
            sectors: vec![SectorId::try_new("Energy").expect("섹터")],
            security_types: vec![crate::ExcludedSecurityType::Etf],
            instrument_cap: ratio("0.3"),
            instrument_warning: ratio("0.15"),
            sector_cap: ratio("0.5"),
        }
    }

    #[test]
    fn saved_hours_round_trip_uses_accessors_without_confirmation() {
        let saved = saved_hours();
        let profile =
            InvestorProfile::restore_from_saved_state(saved.clone()).expect("저장된 프로필");
        let restored = InvestorProfile::restore_from_saved_state(SavedInvestorProfile {
            horizon: profile.horizon().selection(),
            hours_permission: profile.hours_permission().state(),
            risk_appetite: profile.risk_appetite(),
            sectors: profile.exclusions().sectors().to_vec(),
            security_types: profile.exclusions().security_types().to_vec(),
            instrument_cap: profile.weight_caps().instrument_cap().as_ratio(),
            instrument_warning: profile.weight_caps().instrument_warning().as_ratio(),
            sector_cap: profile.weight_caps().sector_cap().as_ratio(),
        })
        .expect("확인 없이 복원");
        assert_eq!(restored, profile);
        assert_eq!(profile.saved_state(), saved);
        assert_eq!(
            profile.with_hours_permission(HoursPermission::default()),
            Err(InvestorProfileError::HoursNotAllowed)
        );
        assert_eq!(profile.horizon().selection(), HorizonSelection::Hours);
        let days = profile
            .with_horizon(Horizon::new(HorizonSelection::FewDays))
            .expect("기간 변경");
        let disabled = days
            .with_hours_permission(HoursPermission::default())
            .expect("허용 끄기");
        assert_eq!(
            disabled.hours_permission().state(),
            HoursPermissionState::Disabled
        );
        assert_eq!(disabled.horizon().selection(), HorizonSelection::FewDays);
        assert_eq!(
            days.hours_permission().state(),
            HoursPermissionState::Enabled
        );
    }

    #[test]
    fn constructors_updates_and_restore_enforce_permission_horizon_consistency() {
        let profile = InvestorProfile::default();
        assert_eq!(
            profile.hours_permission().state(),
            HoursPermissionState::Disabled
        );
        assert_eq!(
            profile.with_horizon(Horizon::new(HorizonSelection::Hours)),
            Err(InvestorProfileError::HoursNotAllowed)
        );
        assert_eq!(
            InvestorProfile::try_new(
                Horizon::new(HorizonSelection::Hours),
                HoursPermission::default(),
                RiskAppetite::Neutral,
                Exclusions::default(),
                crate::WeightCaps::default()
            ),
            Err(InvestorProfileError::HoursNotAllowed)
        );
    }

    #[test]
    fn every_saved_permission_horizon_combination_is_checked() {
        let profile = InvestorProfile::default();
        let cases = [
            HorizonSelection::FewDays,
            HorizonSelection::WeeksToMonths,
            HorizonSelection::HalfYearOrMore,
            HorizonSelection::Hours,
        ]
        .into_iter()
        .flat_map(|horizon| {
            [
                HoursPermissionState::Disabled,
                HoursPermissionState::Enabled,
            ]
            .map(|permission| (horizon, permission))
        });
        for (horizon, hours_permission) in cases {
            let mut saved = saved_hours();
            saved.horizon = horizon;
            saved.hours_permission = hours_permission;
            let actual = InvestorProfile::restore_from_saved_state(saved.clone());
            if horizon == HorizonSelection::Hours
                && hours_permission == HoursPermissionState::Disabled
            {
                assert_eq!(actual, Err(InvestorProfileError::HoursNotAllowed));
            } else {
                let restored = actual.expect("정합한 저장 값");
                assert_eq!(restored.saved_state(), saved);
                assert_eq!(
                    InvestorProfile::try_new(
                        restored.horizon(),
                        restored.hours_permission(),
                        restored.risk_appetite(),
                        restored.exclusions().clone(),
                        restored.weight_caps()
                    )
                    .expect("일반 생성"),
                    restored
                );
            }
        }
        assert_eq!(
            InvestorProfile::restore_from_saved_state(profile.saved_state()).expect("기본값 복원"),
            profile
        );
    }

    #[test]
    fn restore_revalidates_every_weight_and_relationship() {
        for invalid in ["-0.1", "0", "1.0001"] {
            for field in 0_u8..3 {
                let mut saved = saved_hours();
                match field {
                    0 => saved.instrument_cap = ratio(invalid),
                    1 => saved.instrument_warning = ratio(invalid),
                    _ => saved.sector_cap = ratio(invalid),
                }
                assert_eq!(
                    InvestorProfile::restore_from_saved_state(saved),
                    Err(InvestorProfileError::WeightValue(
                        ProfileWeightError::OutOfRange
                    ))
                );
            }
        }
        let mut warning = saved_hours();
        warning.instrument_warning = ratio("0.4");
        assert_eq!(
            InvestorProfile::restore_from_saved_state(warning),
            Err(InvestorProfileError::WeightCaps(
                WeightCapsError::WarningExceedsInstrument
            ))
        );
        let mut sector = saved_hours();
        sector.sector_cap = ratio("0.2");
        assert_eq!(
            InvestorProfile::restore_from_saved_state(sector),
            Err(InvestorProfileError::WeightCaps(
                WeightCapsError::InstrumentExceedsSector
            ))
        );
        let mut duplicate = saved_hours();
        duplicate.sectors.extend(duplicate.sectors.clone());
        assert_eq!(
            InvestorProfile::restore_from_saved_state(duplicate),
            Err(InvestorProfileError::Exclusions(
                ExclusionsError::DuplicateSector
            ))
        );
        let mut duplicate_type = saved_hours();
        duplicate_type
            .security_types
            .extend(duplicate_type.security_types.clone());
        assert_eq!(
            InvestorProfile::restore_from_saved_state(duplicate_type),
            Err(InvestorProfileError::Exclusions(
                ExclusionsError::DuplicateSecurityType
            ))
        );
        let mut oversized = saved_hours();
        oversized.sectors = (0_u16..257)
            .map(|id| SectorId::try_new(&format!("s-{id}")).expect("섹터"))
            .collect();
        assert_eq!(
            InvestorProfile::restore_from_saved_state(oversized),
            Err(InvestorProfileError::Exclusions(
                ExclusionsError::TooManySectors
            ))
        );
    }

    #[test]
    fn restore_ranges_match_independent_integer_reference() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(-2_i64..=102, -2_i64..=102, -2_i64..=102),
                |(cap, warning, sector)| {
                    let mut saved = saved_hours();
                    saved.instrument_cap = Ratio::new(rust_decimal::Decimal::new(cap, 2));
                    saved.instrument_warning = Ratio::new(rust_decimal::Decimal::new(warning, 2));
                    saved.sector_cap = Ratio::new(rust_decimal::Decimal::new(sector, 2));
                    let expected = (1..=100).contains(&cap)
                        && (1..=100).contains(&warning)
                        && (1..=100).contains(&sector)
                        && warning <= cap
                        && cap <= sector;
                    let actual = InvestorProfile::restore_from_saved_state(saved.clone());
                    proptest::prop_assert_eq!(actual.is_ok(), expected);
                    if let Ok(profile) = actual {
                        proptest::prop_assert_eq!(profile.saved_state(), saved);
                    }
                    Ok(())
                },
            )
            .expect("독립 정수 기준");
    }

    #[test]
    fn risk_appetite_order_is_conservative_neutral_aggressive() {
        let appetites = [
            RiskAppetite::Conservative,
            RiskAppetite::Neutral,
            RiskAppetite::Aggressive,
        ];
        for (left_index, left) in appetites.iter().enumerate() {
            for (right_index, right) in appetites.iter().enumerate() {
                assert_eq!(left.cmp(right), left_index.cmp(&right_index));
            }
        }
    }
}
