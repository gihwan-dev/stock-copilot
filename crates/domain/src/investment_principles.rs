use crate::{
    Concentration, ConcentrationThreshold, NumericInput, Policy, PolicyText, ProfileWeight,
    concentration_for_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipleNote(PolicyText);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipleNoteError {
    Empty,
    TooLong,
}

impl PrincipleNote {
    pub fn try_new(text: &PolicyText) -> Result<Self, PrincipleNoteError> {
        let trimmed = text.as_str().trim();
        if trimmed.is_empty() {
            return Err(PrincipleNoteError::Empty);
        }
        if trimmed.chars().count() > 4000 {
            return Err(PrincipleNoteError::TooLong);
        }
        PolicyText::try_new(trimmed)
            .map(Self)
            .map_err(|_too_long| PrincipleNoteError::TooLong)
    }

    #[must_use]
    pub const fn text(&self) -> &PolicyText {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CashFloor {
    #[default]
    None,
    FivePercent,
    TenPercent,
    TwentyPercent,
    ThirtyPercent,
}

impl CashFloor {
    #[must_use]
    pub fn minimum_weight(self) -> Option<ProfileWeight> {
        let percent = match self {
            Self::None => return None,
            Self::FivePercent => "5",
            Self::TenPercent => "10",
            Self::TwentyPercent => "20",
            Self::ThirtyPercent => "30",
        };
        let input = NumericInput::try_new(percent).ok()?;
        ProfileWeight::parse_percent(&input).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConcentrationFloor {
    Three,
    Four,
    #[default]
    Five,
    Six,
    Eight,
    Ten,
}

impl ConcentrationFloor {
    #[must_use]
    pub const fn threshold(self) -> ConcentrationThreshold {
        ConcentrationThreshold::new(match self {
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
            Self::Six => 6,
            Self::Eight => 8,
            Self::Ten => 10,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvestmentPrinciples {
    note: Option<PrincipleNote>,
    cash: CashFloor,
    concentration: ConcentrationFloor,
}

impl InvestmentPrinciples {
    #[must_use]
    pub const fn new(
        note: Option<PrincipleNote>,
        cash: CashFloor,
        concentration: ConcentrationFloor,
    ) -> Self {
        Self {
            note,
            cash,
            concentration,
        }
    }

    #[must_use]
    pub const fn note(&self) -> Option<&PrincipleNote> {
        self.note.as_ref()
    }

    #[must_use]
    pub const fn cash(&self) -> CashFloor {
        self.cash
    }

    #[must_use]
    pub const fn concentration(&self) -> ConcentrationFloor {
        self.concentration
    }

    #[must_use]
    pub fn apply(&self, policy: Policy, holdings: &Concentration) -> Policy {
        let with_note = match &self.note {
            Some(note) => policy.with_context(note.text().clone()),
            None => policy,
        };
        let with_cash = match self.cash.minimum_weight() {
            Some(minimum) => with_note.with_minimum_cash(minimum),
            None => with_note,
        };
        with_cash.with_concentration(concentration_for_policy(
            holdings.warning_with_threshold(&self.concentration.threshold()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::PolicyText;

    fn policy_text(raw: &str) -> PolicyText {
        PolicyText::try_new(raw).expect("글")
    }

    fn note(raw: &str) -> Result<super::PrincipleNote, PrincipleNoteError> {
        super::PrincipleNote::try_new(&policy_text(raw))
    }

    use super::{CashFloor, ConcentrationFloor, InvestmentPrinciples, PrincipleNoteError};

    #[test]
    fn note_is_trimmed_and_bounded() {
        assert_eq!(
            note("  장기 분산  ").expect("원칙").text().as_str(),
            "장기 분산"
        );
        assert_eq!(note(" \n "), Err(PrincipleNoteError::Empty));
        assert_eq!(note(&"가".repeat(4001)), Err(PrincipleNoteError::TooLong));
        note(&"가".repeat(4000)).expect("최대 길이");
    }

    #[test]
    fn cash_floor_maps_to_profile_weight_or_none() {
        assert_eq!(CashFloor::None.minimum_weight(), None);
        for floor in [
            CashFloor::FivePercent,
            CashFloor::TenPercent,
            CashFloor::TwentyPercent,
            CashFloor::ThirtyPercent,
        ] {
            assert!(floor.minimum_weight().is_some(), "{floor:?}");
        }
        assert!(
            CashFloor::FivePercent.minimum_weight() < CashFloor::ThirtyPercent.minimum_weight()
        );
    }

    #[test]
    fn defaults_follow_design_chapter_eight() {
        let defaults = InvestmentPrinciples::default();
        assert_eq!(defaults.note(), None);
        assert_eq!(defaults.cash(), CashFloor::None);
        assert_eq!(defaults.concentration(), ConcentrationFloor::Five);
        assert_eq!(
            ConcentrationFloor::Five.threshold(),
            crate::ConcentrationThreshold::default()
        );
    }

    mod gate {
        use super::super::{CashFloor, ConcentrationFloor, InvestmentPrinciples, PrincipleNote};
        use crate::{
            AcquisitionPrice, AllocationInstrument, BuyingPower, Concentration, Currency,
            ExcludedSecurityType, Holding, HoldingObservation, InvestorProfile, Market, Money,
            Nanosecond, NumericInput, OffsetSeconds, Policy, PolicyConcentration,
            PolicySecurityFacts, PolicyText, PolicyViolation, Portfolio, PortfolioValuation,
            Quantity, Ratio, ReportedHolding, ResolvedOrderInstrument, SectorAssignments, Symbol,
            Timestamp, UnixSeconds,
        };

        fn input(text: &str) -> NumericInput {
            NumericInput::try_new(text).expect("입력")
        }

        fn won(text: &str) -> Money {
            Money::parse(&input(text), Currency::Krw).expect("금액")
        }

        fn instrument(index: usize) -> ResolvedOrderInstrument {
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(&format!("P{index}")).expect("심볼"),
                Market::Kr,
            )
            .expect("종목")
        }

        fn valuation(holdings: usize, each: &str, cash: &str) -> PortfolioValuation {
            let time = Timestamp::from_parts(
                UnixSeconds::new(1000),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각");
            let rows = (0..holdings)
                .map(|index| {
                    Holding::new(
                        instrument(index),
                        Quantity::parse(&input("1")).expect("수량"),
                        AcquisitionPrice::try_new(won("0")).expect("원가"),
                        HoldingObservation::new(
                            None,
                            Some(
                                ReportedHolding::new(
                                    won(each),
                                    won("0"),
                                    Ratio::parse(&input("0")).expect("율"),
                                )
                                .expect("평가"),
                            ),
                            time,
                        ),
                    )
                    .expect("보유")
                })
                .collect();
            Portfolio::new(
                rows,
                BuyingPower::new(won(cash), time),
                BuyingPower::new(
                    Money::parse(&input("0"), Currency::Usd).expect("달러"),
                    time,
                ),
            )
            .expect("포트폴리오")
            .evaluate(None)
        }

        fn facts(holdings: usize) -> PolicySecurityFacts {
            PolicySecurityFacts::from_master_lookup_results(
                (0..holdings)
                    .map(|index| {
                        (
                            AllocationInstrument::new(&instrument(index)),
                            ExcludedSecurityType::Stock,
                        )
                    })
                    .collect(),
            )
        }

        fn concentration(value: &PortfolioValuation) -> Concentration {
            Concentration::new(&crate::Allocation::new(
                value,
                &SectorAssignments::default(),
            ))
        }

        fn findings(
            principles: &InvestmentPrinciples,
            value: &PortfolioValuation,
        ) -> Vec<PolicyViolation> {
            principles
                .apply(
                    Policy::new(InvestorProfile::default()),
                    &concentration(value),
                )
                .check_current(value, &SectorAssignments::default(), &facts(4))
                .expect("검사")
        }

        #[test]
        fn cash_floor_reaches_the_gate_as_minimum_cash() {
            let value = valuation(4, "10", "10");
            let strict = InvestmentPrinciples::new(
                None,
                CashFloor::ThirtyPercent,
                ConcentrationFloor::Three,
            );
            let loose =
                InvestmentPrinciples::new(None, CashFloor::TenPercent, ConcentrationFloor::Three);
            assert!(findings(&strict, &value).contains(&PolicyViolation::MinimumCash));
            assert!(!findings(&loose, &value).contains(&PolicyViolation::MinimumCash));
        }

        #[test]
        fn concentration_floor_changes_the_warning() {
            let value = valuation(4, "10", "10");
            let holdings = concentration(&value);
            let base = || Policy::new(InvestorProfile::default());
            assert_eq!(
                InvestmentPrinciples::new(None, CashFloor::None, ConcentrationFloor::Five)
                    .apply(base(), &holdings)
                    .concentration(),
                PolicyConcentration::Warning
            );
            assert_eq!(
                InvestmentPrinciples::new(None, CashFloor::None, ConcentrationFloor::Four)
                    .apply(base(), &holdings)
                    .concentration(),
                PolicyConcentration::Clear
            );
            assert!(
                findings(
                    &InvestmentPrinciples::new(None, CashFloor::None, ConcentrationFloor::Six),
                    &value
                )
                .contains(&PolicyViolation::ConcentrationWarning)
            );
        }

        #[test]
        fn the_free_note_becomes_ai_context() {
            let note = PrincipleNote::try_new(&PolicyText::try_new("배당주 위주").expect("글"))
                .expect("원칙");
            let applied =
                InvestmentPrinciples::new(Some(note), CashFloor::None, ConcentrationFloor::Five)
                    .apply(
                        Policy::new(InvestorProfile::default()),
                        &concentration(&valuation(4, "10", "10")),
                    );
            assert_eq!(
                applied
                    .context()
                    .iter()
                    .map(PolicyText::as_str)
                    .collect::<Vec<_>>(),
                vec!["배당주 위주"]
            );
            assert_eq!(
                InvestmentPrinciples::default()
                    .apply(
                        Policy::new(InvestorProfile::default()),
                        &concentration(&valuation(4, "10", "10")),
                    )
                    .context()
                    .len(),
                0
            );
        }
    }
}
