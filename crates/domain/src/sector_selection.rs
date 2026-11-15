use crate::{
    AllocationInstrument, Amount, Currency, InstrumentClassification, MasterInstrument,
    SectorAmount, SectorAssignment, SectorComposition, SectorId,
};
use std::cmp::Ordering;
use std::collections::BTreeSet;

pub(crate) trait AssignmentIdentity {
    fn allocation_identity(&self) -> AllocationInstrument;
}
impl AssignmentIdentity for crate::ResolvedOrderInstrument {
    fn allocation_identity(&self) -> AllocationInstrument {
        AllocationInstrument::new(self)
    }
}
impl AssignmentIdentity for crate::QuoteInstrument {
    fn allocation_identity(&self) -> AllocationInstrument {
        AllocationInstrument {
            market: (*self.market()).into(),
            symbol: self.symbol().clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorMembership {
    Unspecified,
    Single,
    Multiple,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorSelectionRule {
    LargestCompositionWeightThenSectorId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorSelectionError {
    MissingMembership,
    MissingWeight,
    DuplicateSector,
    InvalidMaster,
    Arithmetic,
}
#[derive(Clone, PartialEq, Eq)]
struct Candidate {
    id: SectorId,
    value: Amount,
    total: Amount,
}

#[must_use]
pub const fn sector_membership(assignment: &SectorAssignment) -> SectorMembership {
    assignment.membership
}
#[must_use]
pub const fn sector_selection_rule() -> SectorSelectionRule {
    SectorSelectionRule::LargestCompositionWeightThenSectorId
}

pub fn sector_assignment_from_compositions(
    master: &MasterInstrument,
    compositions: &[SectorComposition],
) -> Result<SectorAssignment, SectorSelectionError> {
    let identity = master
        .quote_instrument()
        .or(Err(SectorSelectionError::InvalidMaster))?;
    let mut selected: Option<Candidate> = None;
    let mut sectors = BTreeSet::new();
    let mut membership = SectorMembership::Single;
    for composition in compositions {
        if composition.market() != *identity.market()
            || !composition.symbols().contains(identity.symbol())
        {
            continue;
        }
        if !sectors.insert(composition.sector().clone()) {
            return Err(SectorSelectionError::DuplicateSector);
        }
        let candidate = composition_candidate(composition, identity.symbol())?;
        if let Some(previous) = &selected {
            membership = SectorMembership::Multiple;
            let order = crate::exact_arithmetic::compare_sector_fractions(
                candidate.value,
                candidate.total,
                previous.value,
                previous.total,
            );
            if order == Ordering::Less || (order == Ordering::Equal && candidate.id > previous.id) {
                continue;
            }
        }
        selected = Some(candidate);
    }
    let winner = selected.ok_or(SectorSelectionError::MissingMembership)?;
    let mut assignment =
        SectorAssignment::new(&identity, InstrumentClassification::Sector(winner.id));
    assignment.membership = membership;
    Ok(assignment)
}

fn composition_candidate(
    composition: &SectorComposition,
    symbol: &crate::Symbol,
) -> Result<Candidate, SectorSelectionError> {
    let mut total = crate::Money::new(Amount::new(rust_decimal::Decimal::ZERO), Currency::Krw);
    let mut selected = None;
    for member in composition.stocks() {
        let value = match member.market_capitalization() {
            SectorAmount::Unavailable => return Err(SectorSelectionError::MissingWeight),
            SectorAmount::Available(value) => value,
        };
        total = total
            .checked_add(&value)
            .or(Err(SectorSelectionError::Arithmetic))?;
        if member.symbol() == symbol {
            selected = Some(value.amount());
        }
    }
    if total.amount().as_decimal() <= rust_decimal::Decimal::ZERO {
        return Err(SectorSelectionError::MissingWeight);
    }
    Ok(Candidate {
        id: composition.sector().clone(),
        value: selected.ok_or(SectorSelectionError::MissingMembership)?,
        total: total.amount(),
    })
}
#[must_use]
pub fn sector_assignments_membership(
    assignments: &crate::SectorAssignments,
    instrument: &AllocationInstrument,
) -> Option<SectorMembership> {
    assignments.1.get(instrument).copied()
}

impl std::fmt::Debug for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Candidate([REDACTED])")
    }
}
#[cfg(test)]
mod tests {
    use super::{
        SectorMembership, SectorSelectionError, SectorSelectionRule,
        sector_assignment_from_compositions, sector_assignments_membership, sector_membership,
        sector_selection_rule,
    };
    use crate::valuation_snapshot::tests::at;
    use crate::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentClassification, InstrumentId,
        KnownMarket, ListingStatus, Market, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, NumericInput, SectorComposition, SectorId, SectorStockRow, Symbol,
    };
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("005930").expect("종목"),
            instrument_id: InstrumentId::try_new("stable").expect("식별"),
            name: DisplayInstrumentName::try_new("테스트").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(2, 0),
        })
        .expect("마스터")
    }
    fn composition(id: &str, first: Option<&str>, second: Option<&str>) -> SectorComposition {
        SectorComposition::from_stocks_response(
            SectorId::try_new(id).expect("섹터"),
            KnownMarket::Kr,
            [("005930", first), ("000660", second)]
                .map(|(symbol, value)| SectorStockRow {
                    symbol: Symbol::try_new(symbol).expect("심볼"),
                    currency: Currency::Krw,
                    market_capitalization: value
                        .map(|text| NumericInput::try_new(text).expect("수치")),
                    trading_amount: None,
                })
                .into(),
            at(2, 0),
        )
        .expect("구성")
    }
    #[test]
    fn largest_weight_wins_ties_use_id_and_multiple_membership_survives() {
        let rows = [
            composition("B", Some("10"), Some("30")),
            composition("A", Some("20"), Some("60")),
            composition("C", Some("30"), Some("20")),
        ];
        let assignment = sector_assignment_from_compositions(&master(), &rows).expect("선택");
        assert_eq!(
            assignment.classification(),
            &InstrumentClassification::Sector(SectorId::try_new("C").expect("섹터"))
        );
        assert_eq!(sector_membership(&assignment), SectorMembership::Multiple);
        assert_eq!(assignment.instrument().symbol().as_str(), "005930");
        assert_eq!(assignment.instrument().market(), &Market::Kr);
        assert_eq!(
            sector_selection_rule(),
            SectorSelectionRule::LargestCompositionWeightThenSectorId
        );
        let collection = crate::SectorAssignments::try_new(vec![assignment.clone()]).expect("목록");
        assert_eq!(
            sector_assignments_membership(&collection, assignment.instrument()),
            Some(SectorMembership::Multiple)
        );
        let tied = sector_assignment_from_compositions(&master(), &rows[..2]).expect("동률");
        assert_eq!(
            tied.classification(),
            &InstrumentClassification::Sector(SectorId::try_new("A").expect("섹터"))
        );
        let single = sector_assignment_from_compositions(&master(), &rows[..1]).expect("하나");
        assert_eq!(sector_membership(&single), SectorMembership::Single);
        let mut reverse = rows.to_vec();
        reverse.reverse();
        assert_eq!(
            sector_assignment_from_compositions(&master(), &reverse),
            Ok(assignment)
        );
    }
    #[test]
    fn missing_weights_membership_duplicates_and_unknown_master_are_closed() {
        assert_eq!(
            sector_assignment_from_compositions(&master(), &[]),
            Err(SectorSelectionError::MissingMembership)
        );
        for row in [
            composition("A", None, Some("1")),
            composition("A", Some("1"), None),
            composition("A", Some("0"), Some("0")),
        ] {
            assert_eq!(
                sector_assignment_from_compositions(&master(), &[row]),
                Err(SectorSelectionError::MissingWeight)
            );
        }
        let row = composition("A", Some("2"), Some("3"));
        assert_eq!(
            sector_assignment_from_compositions(&master(), &[row.clone(), row]),
            Err(SectorSelectionError::DuplicateSector)
        );
        let huge = composition("A", Some("79228162514264337593543950335"), Some("1"));
        assert_eq!(
            sector_assignment_from_compositions(&master(), &[huge]),
            Err(SectorSelectionError::Arithmetic)
        );
        let mut input = master().row().clone();
        input.listing_market = MasterCode::try_new("OTHER").expect("시장");
        assert_eq!(
            sector_assignment_from_compositions(
                &MasterInstrument::try_new(input).expect("마스터"),
                &[]
            ),
            Err(SectorSelectionError::InvalidMaster)
        );
    }
    #[test]
    fn selection_candidates_redact_amounts_and_ignore_unrelated_members() {
        let composition = composition("A", Some("123456789"), Some("333"));
        let candidate =
            super::composition_candidate(&composition, &Symbol::try_new("005930").expect("심볼"))
                .expect("후보");
        assert_eq!(format!("{candidate:?}"), "Candidate([REDACTED])");
        let unrelated = SectorComposition::from_stocks_response(
            SectorId::try_new("B").expect("섹터"),
            KnownMarket::Us,
            vec![],
            at(2, 0),
        )
        .expect("구성");
        assert_eq!(
            sector_assignment_from_compositions(&master(), &[unrelated]),
            Err(SectorSelectionError::MissingMembership)
        );
    }
    #[test]
    fn fractional_weights_and_wide_products_preserve_exact_order() {
        let rows = [
            composition("A", Some("0.1"), Some("0.2")),
            composition("B", Some("0.2"), Some("0.3")),
        ];
        let result = sector_assignment_from_compositions(&master(), &rows).expect("분수 선택");
        assert_eq!(
            result.classification(),
            &InstrumentClassification::Sector(SectorId::try_new("B").expect("섹터"))
        );
        let amount = |text| {
            crate::Money::parse(&NumericInput::try_new(text).expect("수치"), Currency::Krw)
                .expect("금액")
                .amount()
        };
        let unbalanced = [
            composition("A", Some("1"), Some("0.4")),
            composition("B", Some("0.7"), Some("0.3")),
        ];
        assert_eq!(
            sector_assignment_from_compositions(&master(), &unbalanced)
                .expect("다른 스케일")
                .classification(),
            &InstrumentClassification::Sector(SectorId::try_new("A").expect("섹터"))
        );
        assert_eq!(
            crate::exact_arithmetic::compare_sector_fractions(
                amount("7922816251426433759354395033"),
                amount("0.1"),
                amount("79228162514264337593543950330"),
                amount("1")
            ),
            std::cmp::Ordering::Equal
        );
        let maximum = amount("79228162514264337593543950335");
        let tiny = amount("0.0000000000000000000000000001");
        assert_eq!(
            crate::exact_arithmetic::compare_sector_fractions(maximum, tiny, tiny, maximum),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            crate::exact_arithmetic::compare_sector_fractions(tiny, maximum, maximum, tiny),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            super::composition_candidate(&rows[0], &Symbol::try_new("000001").expect("심볼")),
            Err(SectorSelectionError::MissingMembership)
        );
    }
    #[test]
    fn internal_order_identity_constructor_preserves_legacy_keys() {
        let mut row = master().row().clone();
        row.korean_detail = Some(crate::KoreanMasterDetail {
            liquidation: crate::LiquidationTrading::Regular,
            krx: crate::TradingSuspension::Clear,
            nxt: crate::NxtTrading::Unsupported,
        });
        let resolved = MasterInstrument::try_new(row)
            .expect("마스터")
            .order_instrument()
            .expect("식별");
        let assignment =
            crate::SectorAssignment::new(&resolved, InstrumentClassification::Unclassified);
        assert_eq!(
            assignment.instrument(),
            &crate::AllocationInstrument::new(&resolved)
        );
        assert_eq!(
            sector_membership(&assignment),
            SectorMembership::Unspecified
        );
    }
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(testkit::property_cases()))]
        #[test]
        fn selection_matches_independent_integer_cross_products(first in 1_u32..10000,first_other in 1_u32..10000,second in 1_u32..10000,second_other in 1_u32..10000) {
            let rows=[composition("A",Some(&first.to_string()),Some(&first_other.to_string())),composition("B",Some(&second.to_string()),Some(&second_other.to_string()))];
            let left=u64::from(first).checked_mul(u64::from(second).checked_add(u64::from(second_other)).expect("합")).expect("곱");
            let right=u64::from(second).checked_mul(u64::from(first).checked_add(u64::from(first_other)).expect("합")).expect("곱");
            let expected=if left>=right {"A"} else {"B"};
            let selected=sector_assignment_from_compositions(&master(),&rows).expect("선택");
            proptest::prop_assert_eq!(selected.classification(),&InstrumentClassification::Sector(SectorId::try_new(expected).expect("섹터")));
        }
    }
}
