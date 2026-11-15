use crate::{
    AllocationInstrument, Currency, InstrumentClassification, MasterInstrument, SectorAssignment,
    SectorCatalog, SectorComposition, SectorDepth, SectorId,
};
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
    Single,
    Multiple,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorSelectionRule {
    LargestMajorCapitalizationThenSectorId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorAssignmentReason {
    SingleMajorSector,
    LargestMajorCapitalization,
    CapitalizationUnavailable,
    EtfFundFromMaster,
    NoMajorMembership,
    UnknownSecurityType,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorSelectionError {
    DuplicateSector,
    InvalidMaster,
    InvalidCatalog,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AssignmentDetails {
    pub(crate) membership: Option<SectorMembership>,
    pub(crate) reason: SectorAssignmentReason,
}
#[derive(Clone, PartialEq, Eq)]
struct Candidate {
    id: SectorId,
    capitalization: Option<crate::Money>,
}
impl std::fmt::Debug for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Candidate")
            .field("id", &self.id)
            .field("capitalization", &"[REDACTED]")
            .finish()
    }
}
#[must_use]
pub fn sector_membership(assignment: &SectorAssignment) -> Option<SectorMembership> {
    assignment.details?.membership
}
#[must_use]
pub fn sector_assignment_reason(assignment: &SectorAssignment) -> Option<SectorAssignmentReason> {
    assignment.details.map(|details| details.reason)
}
#[must_use]
pub const fn sector_selection_rule() -> SectorSelectionRule {
    SectorSelectionRule::LargestMajorCapitalizationThenSectorId
}
#[must_use]
pub fn sector_assignments_membership(
    assignments: &crate::SectorAssignments,
    instrument: &AllocationInstrument,
) -> Option<SectorMembership> {
    assignments.1.get(instrument)?.membership
}
#[must_use]
pub fn sector_assignments_reason(
    assignments: &crate::SectorAssignments,
    instrument: &AllocationInstrument,
) -> Option<SectorAssignmentReason> {
    assignments.1.get(instrument).map(|details| details.reason)
}
pub fn sector_assignment_from_compositions(
    master: &MasterInstrument,
    catalog: &SectorCatalog,
    compositions: &[SectorComposition],
) -> Result<SectorAssignment, SectorSelectionError> {
    let identity = master
        .quote_instrument()
        .or(Err(SectorSelectionError::InvalidMaster))?;
    if let Some((classification, reason)) = master_classification(master) {
        return Ok(assignment(&identity, classification, None, reason));
    }
    let candidates = major_candidates(&identity, catalog, compositions)?;
    let Some((winner, membership, reason)) = select_major(&candidates) else {
        return Ok(assignment(
            &identity,
            InstrumentClassification::Unclassified,
            None,
            SectorAssignmentReason::NoMajorMembership,
        ));
    };
    Ok(assignment(
        &identity,
        InstrumentClassification::Sector(winner.id.clone()),
        Some(membership),
        reason,
    ))
}
fn assignment(
    identity: &crate::QuoteInstrument,
    classification: InstrumentClassification,
    membership: Option<SectorMembership>,
    reason: SectorAssignmentReason,
) -> SectorAssignment {
    let mut result = SectorAssignment::new(identity, classification);
    result.details = Some(AssignmentDetails { membership, reason });
    result
}
fn master_classification(
    master: &MasterInstrument,
) -> Option<(InstrumentClassification, SectorAssignmentReason)> {
    match master.row().security_type.known() {
        Some(
            crate::ExcludedSecurityType::Etf
            | crate::ExcludedSecurityType::ForeignEtf
            | crate::ExcludedSecurityType::InfrastructureFund,
        ) => {
            return Some((
                InstrumentClassification::EtfFund,
                SectorAssignmentReason::EtfFundFromMaster,
            ));
        }
        None => {
            return Some((
                InstrumentClassification::Unclassified,
                SectorAssignmentReason::UnknownSecurityType,
            ));
        }
        Some(
            crate::ExcludedSecurityType::Stock
            | crate::ExcludedSecurityType::ForeignStock
            | crate::ExcludedSecurityType::DepositaryReceipt
            | crate::ExcludedSecurityType::Reit
            | crate::ExcludedSecurityType::Etn
            | crate::ExcludedSecurityType::StockWarrants,
        ) => (),
    }
    None
}
fn major_candidates(
    identity: &crate::QuoteInstrument,
    catalog: &SectorCatalog,
    compositions: &[SectorComposition],
) -> Result<Vec<Candidate>, SectorSelectionError> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for composition in compositions {
        if composition.market() != *identity.market()
            || !composition.symbols().contains(identity.symbol())
        {
            continue;
        }
        let sector = catalog
            .sectors()
            .iter()
            .find(|sector| sector.id() == composition.sector())
            .ok_or(SectorSelectionError::InvalidCatalog)?;
        if sector.depth() != SectorDepth::Major {
            continue;
        }
        if !seen.insert(sector.id().clone()) {
            return Err(SectorSelectionError::DuplicateSector);
        }
        candidates.push(Candidate {
            id: sector.id().clone(),
            capitalization: total_capitalization(composition),
        });
    }
    Ok(candidates)
}
fn select_major(
    candidates: &[Candidate],
) -> Option<(&Candidate, SectorMembership, SectorAssignmentReason)> {
    let (first, rest) = candidates.split_first()?;
    let membership = match rest.len() {
        0 => SectorMembership::Single,
        _ => SectorMembership::Multiple,
    };
    let reason = if candidates
        .iter()
        .any(|candidate| candidate.capitalization.is_none())
    {
        SectorAssignmentReason::CapitalizationUnavailable
    } else if membership == SectorMembership::Single {
        SectorAssignmentReason::SingleMajorSector
    } else {
        SectorAssignmentReason::LargestMajorCapitalization
    };
    let capitalization = |candidate: &Candidate| match reason {
        SectorAssignmentReason::LargestMajorCapitalization => candidate
            .capitalization
            .map(|value| value.amount().as_decimal()),
        SectorAssignmentReason::SingleMajorSector
        | SectorAssignmentReason::CapitalizationUnavailable
        | SectorAssignmentReason::EtfFundFromMaster
        | SectorAssignmentReason::NoMajorMembership
        | SectorAssignmentReason::UnknownSecurityType => None,
    };
    let winner = rest.iter().fold(first, |winner, candidate| {
        if capitalization(candidate) > capitalization(winner)
            || (capitalization(candidate) == capitalization(winner) && candidate.id < winner.id)
        {
            candidate
        } else {
            winner
        }
    });
    Some((winner, membership, reason))
}
fn total_capitalization(composition: &SectorComposition) -> Option<crate::Money> {
    let values = composition
        .stocks()
        .iter()
        .map(|member| match member.market_capitalization() {
            crate::SectorAmount::Unavailable => None,
            crate::SectorAmount::Available(value) => Some(value),
        })
        .collect::<Option<Vec<_>>>()?;
    values.into_iter().try_fold(
        crate::Money::new(
            crate::Amount::new(rust_decimal::Decimal::ZERO),
            Currency::Krw,
        ),
        |total, value| total.checked_add(&value).ok(),
    )
}
#[cfg(test)]
mod tests {
    use super::{
        SectorAssignmentReason, SectorMembership, SectorSelectionError, SectorSelectionRule,
        sector_assignment_from_compositions, sector_assignment_reason,
        sector_assignments_membership, sector_assignments_reason, sector_membership,
        sector_selection_rule,
    };
    use crate::valuation_snapshot::tests::at;
    use crate::{
        Currency, InstrumentClassification, KnownMarket, MasterCode, MasterInstrument,
        NumericInput, SectorCatalog, SectorComposition, SectorDepth, SectorId, SectorMetadataRow,
        SectorStockRow, Symbol,
    };
    fn id(value: &str) -> SectorId {
        SectorId::try_new(value).expect("섹터")
    }
    fn master(symbol: &str, market: KnownMarket, kind: &str) -> MasterInstrument {
        MasterInstrument::try_new(crate::MasterInstrumentRow {
            listing_market: MasterCode::try_new(match market {
                KnownMarket::Kr => "KOSPI",
                KnownMarket::Us => "NASDAQ",
            })
            .expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: crate::InstrumentId::try_new("stable").expect("ID"),
            name: crate::DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: crate::MasterSecurityType::from_code(
                MasterCode::try_new(kind).expect("유형"),
            ),
            currency: match market {
                KnownMarket::Kr => Currency::Krw,
                KnownMarket::Us => Currency::Usd,
            },
            listing_status: crate::ListingStatus::Active,
            common_share: crate::CommonShare::Common,
            korean_detail: None,
            observed_at: at(2, 0),
        })
        .expect("마스터")
    }
    fn catalog() -> SectorCatalog {
        SectorCatalog::from_response(
            [
                ("A", SectorDepth::Major, None),
                ("B", SectorDepth::Major, None),
                ("a", SectorDepth::Middle, Some("A")),
                ("x", SectorDepth::Minor, Some("a")),
                ("y", SectorDepth::Minor, Some("a")),
            ]
            .into_iter()
            .map(|(value, depth, parent)| SectorMetadataRow {
                id: id(value),
                name: crate::DisplayInstrumentName::try_new(value).expect("이름"),
                depth,
                parent: parent.map(id),
            })
            .collect(),
            at(2, 0),
        )
        .expect("전체 목록")
    }
    fn composition(
        sector: &str,
        market: KnownMarket,
        rows: &[(&str, Option<&str>)],
    ) -> SectorComposition {
        SectorComposition::from_stocks_response(
            id(sector),
            market,
            rows.iter()
                .map(|(symbol, value)| SectorStockRow {
                    symbol: Symbol::try_new(symbol).expect("종목"),
                    currency: Currency::Krw,
                    market_capitalization: value
                        .map(|text| NumericInput::try_new(text).expect("시총")),
                    trading_amount: None,
                })
                .collect(),
            at(2, 0),
        )
        .expect("구성")
    }
    fn kr(sector: &str, first: Option<&str>, second: Option<&str>) -> SectorComposition {
        composition(
            sector,
            KnownMarket::Kr,
            &[("005930", first), ("000660", second)],
        )
    }
    fn classify(symbol: &str, rows: &[SectorComposition]) -> crate::SectorAssignment {
        sector_assignment_from_compositions(
            &master(symbol, KnownMarket::Kr, "STOCK"),
            &catalog(),
            rows,
        )
        .expect("분류")
    }
    fn semiconductor_compositions() -> [SectorComposition; 3] {
        [
            kr("A", Some("600"), Some("400")),
            composition("x", KnownMarket::Kr, &[("005930", Some("600"))]),
            composition("y", KnownMarket::Kr, &[("000660", Some("400"))]),
        ]
    }
    #[test]
    fn samsung_and_hynix_stay_in_the_same_major_sector() {
        let rows = semiconductor_compositions();
        for symbol in ["005930", "000660"] {
            let result = classify(symbol, &rows);
            assert_eq!(
                result.classification(),
                &InstrumentClassification::Sector(id("A"))
            );
            assert_eq!(sector_membership(&result), Some(SectorMembership::Single));
            assert_eq!(
                sector_assignment_reason(&result),
                Some(SectorAssignmentReason::SingleMajorSector)
            );
        }
    }
    #[test]
    fn larger_major_total_wins_even_when_member_weight_is_smaller() {
        let rows = [
            kr("A", Some("100"), Some("900")),
            kr("B", Some("100"), Some("100")),
        ];
        let result = classify("005930", &rows);
        assert_eq!(
            result.classification(),
            &InstrumentClassification::Sector(id("A"))
        );
        assert_eq!(sector_membership(&result), Some(SectorMembership::Multiple));
        assert_eq!(
            sector_assignment_reason(&result),
            Some(SectorAssignmentReason::LargestMajorCapitalization)
        );
        let values = crate::SectorAssignments::try_new(vec![result.clone()]).expect("목록");
        assert_eq!(
            sector_assignments_membership(&values, result.instrument()),
            Some(SectorMembership::Multiple)
        );
        assert_eq!(
            sector_assignments_reason(&values, result.instrument()),
            Some(SectorAssignmentReason::LargestMajorCapitalization)
        );
        assert_eq!(
            sector_selection_rule(),
            SectorSelectionRule::LargestMajorCapitalizationThenSectorId
        );
        let mut reverse = rows.to_vec();
        reverse.reverse();
        assert_eq!(classify("005930", &reverse), result);
    }
    #[test]
    fn equal_totals_and_missing_totals_use_sector_id_order() {
        for rows in [
            [kr("B", Some("2"), Some("8")), kr("A", Some("9"), Some("1"))],
            [kr("B", Some("2"), None), kr("A", Some("9"), Some("1000"))],
            [kr("B", Some("99999"), Some("8")), kr("A", Some("9"), None)],
        ] {
            let result = classify("005930", &rows);
            assert_eq!(
                result.classification(),
                &InstrumentClassification::Sector(id("A"))
            );
            assert_eq!(sector_membership(&result), Some(SectorMembership::Multiple));
            let mut reverse = rows.to_vec();
            reverse.reverse();
            assert_eq!(classify("005930", &reverse), result);
        }
        assert_eq!(
            sector_assignment_reason(&classify(
                "005930",
                &[kr("B", None, Some("2")), kr("A", Some("1"), Some("100"))]
            )),
            Some(SectorAssignmentReason::CapitalizationUnavailable)
        );
    }
    #[test]
    fn single_membership_does_not_need_peer_capitalization() {
        let result = classify("005930", &[kr("A", Some("2"), None)]);
        assert_eq!(
            result.classification(),
            &InstrumentClassification::Sector(id("A"))
        );
        assert_eq!(sector_membership(&result), Some(SectorMembership::Single));
        assert_eq!(
            sector_assignment_reason(&result),
            Some(SectorAssignmentReason::CapitalizationUnavailable)
        );
    }
    #[test]
    fn etf_and_fund_classification_comes_from_master() {
        for kind in ["ETF", "FOREIGN_ETF", "INFRASTRUCTURE_FUND"] {
            let result = sector_assignment_from_compositions(
                &master("005930", KnownMarket::Kr, kind),
                &catalog(),
                &[],
            )
            .expect("ETF 펀드");
            assert_eq!(result.classification(), &InstrumentClassification::EtfFund);
            assert_eq!(sector_membership(&result), None);
            assert_eq!(
                sector_assignment_reason(&result),
                Some(SectorAssignmentReason::EtfFundFromMaster)
            );
        }
    }
    #[test]
    fn missing_major_and_unknown_type_remain_explicitly_unclassified() {
        for rows in [
            vec![],
            vec![kr("a", Some("1"), Some("2"))],
            vec![composition("A", KnownMarket::Kr, &[("000660", Some("1"))])],
        ] {
            let result = classify("005930", &rows);
            assert_eq!(
                result.classification(),
                &InstrumentClassification::Unclassified
            );
            assert_eq!(
                sector_assignment_reason(&result),
                Some(SectorAssignmentReason::NoMajorMembership)
            );
        }
        let result = sector_assignment_from_compositions(
            &master("005930", KnownMarket::Kr, "UNKNOWN"),
            &catalog(),
            &[kr("A", Some("1"), Some("2"))],
        )
        .expect("미분류");
        assert_eq!(
            result.classification(),
            &InstrumentClassification::Unclassified
        );
        assert_eq!(
            sector_assignment_reason(&result),
            Some(SectorAssignmentReason::UnknownSecurityType)
        );
    }
    #[test]
    fn identical_symbols_from_other_markets_are_not_memberships() {
        let rows = [composition(
            "A",
            KnownMarket::Us,
            &[("ABCDEF", Some("100"))],
        )];
        let result = sector_assignment_from_compositions(
            &master("ABCDEF", KnownMarket::Kr, "STOCK"),
            &catalog(),
            &rows,
        )
        .expect("시장 분리");
        assert_eq!(
            result.classification(),
            &InstrumentClassification::Unclassified
        );
        let matching = sector_assignment_from_compositions(
            &master("ABCDEF", KnownMarket::Us, "FOREIGN_STOCK"),
            &catalog(),
            &rows,
        )
        .expect("같은 시장");
        assert_eq!(
            matching.classification(),
            &InstrumentClassification::Sector(id("A"))
        );
    }
    #[test]
    fn invalid_catalog_duplicates_overflow_and_master_are_closed() {
        let instrument_master = master("005930", KnownMarket::Kr, "STOCK");
        let row = kr("A", Some("1"), Some("2"));
        assert_eq!(
            sector_assignment_from_compositions(
                &instrument_master,
                &catalog(),
                &[row.clone(), row]
            ),
            Err(SectorSelectionError::DuplicateSector)
        );
        assert_eq!(
            sector_assignment_from_compositions(
                &instrument_master,
                &catalog(),
                &[kr("missing", Some("1"), Some("2"))]
            ),
            Err(SectorSelectionError::InvalidCatalog)
        );
        let overflow = classify(
            "005930",
            &[
                kr("A", Some("79228162514264337593543950335"), Some("1")),
                kr("B", Some("1"), Some("1")),
            ],
        );
        assert_eq!(
            overflow.classification(),
            &InstrumentClassification::Sector(id("A"))
        );
        assert_eq!(
            sector_assignment_reason(&overflow),
            Some(SectorAssignmentReason::CapitalizationUnavailable)
        );
        let mut invalid_master_row = instrument_master.row().clone();
        invalid_master_row.listing_market = MasterCode::try_new("UNKNOWN").expect("시장");
        assert_eq!(
            sector_assignment_from_compositions(
                &MasterInstrument::try_new(invalid_master_row).expect("마스터"),
                &catalog(),
                &[]
            ),
            Err(SectorSelectionError::InvalidMaster)
        );
    }
    #[test]
    fn legacy_internal_metadata_is_absent_and_candidate_debug_only_redacts_money() {
        let instrument = crate::ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("ABC").expect("심볼"),
            crate::Market::Us,
        )
        .expect("식별");
        let legacy =
            crate::SectorAssignment::new(&instrument, InstrumentClassification::Unclassified);
        assert_eq!(sector_membership(&legacy), None);
        assert_eq!(sector_assignment_reason(&legacy), None);
        let values = crate::SectorAssignments::try_new(vec![legacy.clone()]).expect("목록");
        assert_eq!(
            sector_assignments_membership(&values, legacy.instrument()),
            None
        );
        assert_eq!(
            sector_assignments_reason(&values, legacy.instrument()),
            None
        );
        let candidate = super::Candidate {
            id: id("visible-sector"),
            capitalization: Some(
                crate::Money::parse(
                    &NumericInput::try_new("123456789").expect("수치"),
                    Currency::Krw,
                )
                .expect("금액"),
            ),
        };
        let debug = format!("{candidate:?}");
        assert!(debug.contains("visible-sector"));
        assert!(!debug.contains("123456789"));
    }
    fn held(symbol: &str, amount: &str) -> crate::Holding {
        let money = |value| {
            crate::Money::parse(&NumericInput::try_new(value).expect("수치"), Currency::Krw)
                .expect("금액")
        };
        let instrument = crate::ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("종목"),
            crate::Market::Kr,
        )
        .expect("식별");
        let report = crate::ReportedHolding::new(
            money(amount),
            money("0"),
            crate::Ratio::parse(&NumericInput::try_new("0").expect("수익률")).expect("수익률"),
        )
        .expect("보유 평가");
        crate::Holding::new(
            instrument,
            crate::Quantity::parse(&NumericInput::try_new("1").expect("수량")).expect("수량"),
            crate::AcquisitionPrice::try_new(money("0")).expect("원가"),
            crate::HoldingObservation::new(None, Some(report), at(2, 0)),
        )
        .expect("보유")
    }
    fn semiconductor_allocation(assignments: &crate::SectorAssignments) -> crate::Allocation {
        let cash = |value, currency| {
            crate::Money::parse(&NumericInput::try_new(value).expect("수치"), currency)
                .expect("현금")
        };
        let portfolio = crate::Portfolio::new(
            vec![held("005930", "200"), held("000660", "200")],
            crate::BuyingPower::new(cash("600", Currency::Krw), at(2, 0)),
            crate::BuyingPower::new(cash("0", Currency::Usd), at(2, 0)),
        )
        .expect("포트폴리오");
        crate::Allocation::new(&portfolio.evaluate(None), assignments)
    }
    #[test]
    fn asset_sector_cap_combines_samsung_and_hynix_instead_of_splitting_subsectors() {
        let rows = semiconductor_compositions();
        let assignments = crate::SectorAssignments::try_new(
            ["005930", "000660"]
                .map(|symbol| classify(symbol, &rows))
                .into(),
        )
        .expect("섹터 대응");
        let allocation = semiconductor_allocation(&assignments);
        let sectors = allocation.assets().sectors().as_ref().expect("자산 섹터");
        let semiconductor = sectors
            .entries()
            .iter()
            .find(|entry| entry.key() == &crate::AllocationSector::Classified(id("A")))
            .expect("같은 대분류");
        assert_eq!(semiconductor.display().units().as_u32(), 4000);
        assert_eq!(
            semiconductor.weight().cmp_profile_weight(
                &crate::ProfileWeight::parse(&NumericInput::try_new("0.3").expect("상한"))
                    .expect("상한")
            ),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            allocation
                .holdings()
                .sectors()
                .as_ref()
                .expect("보유 구성")
                .entries()
                .len(),
            1
        );
    }
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(testkit::property_cases()))]
        #[test]
        fn selection_matches_independent_major_totals(first_total in 1_u32..10000,second_total in 1_u32..10000) {
            let rows=[kr("B",Some("1"),Some(&second_total.to_string())),kr("A",Some("1"),Some(&first_total.to_string()))];let expected=if first_total>=second_total {"A"} else {"B"};
            let actual=classify("005930",&rows);let expected_classification=InstrumentClassification::Sector(id(expected));
            proptest::prop_assert_eq!(actual.classification(),&expected_classification);
        }
    }
}
