#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectorId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorIdError {
    InvalidFormat,
}

impl SectorId {
    pub fn try_new(value: &str) -> Result<Self, SectorIdError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(SectorIdError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExcludedSecurityType {
    Stock,
    ForeignStock,
    DepositaryReceipt,
    InfrastructureFund,
    Reit,
    Etf,
    ForeignEtf,
    Etn,
    StockWarrants,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Exclusions {
    sectors: Vec<SectorId>,
    security_types: Vec<ExcludedSecurityType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionsError {
    TooManySectors,
    DuplicateSector,
    DuplicateSecurityType,
}

impl Exclusions {
    pub fn try_new(
        sectors: Vec<SectorId>,
        security_types: Vec<ExcludedSecurityType>,
    ) -> Result<Self, ExclusionsError> {
        if sectors.len() > 256 {
            return Err(ExclusionsError::TooManySectors);
        }
        let mut sorted_sectors = sectors.clone();
        sorted_sectors.sort();
        sorted_sectors.dedup();
        if sorted_sectors.len() != sectors.len() {
            return Err(ExclusionsError::DuplicateSector);
        }
        let mut sorted_types = security_types.clone();
        sorted_types.sort();
        sorted_types.dedup();
        if sorted_types.len() != security_types.len() {
            return Err(ExclusionsError::DuplicateSecurityType);
        }
        Ok(Self {
            sectors,
            security_types,
        })
    }

    #[must_use]
    pub fn sectors(&self) -> &[SectorId] {
        &self.sectors
    }
    #[must_use]
    pub fn security_types(&self) -> &[ExcludedSecurityType] {
        &self.security_types
    }
}

#[cfg(test)]
mod tests {
    use crate::{ExcludedSecurityType, Exclusions, ExclusionsError, SectorId, SectorIdError};
    use proptest::test_runner::{Config, TestRunner};

    #[test]
    fn sector_identifier_follows_opaque_identifier_boundary() {
        for text in ["", "with space", "한글", "\n"] {
            assert_eq!(SectorId::try_new(text), Err(SectorIdError::InvalidFormat));
        }
        assert_eq!(
            SectorId::try_new(&"a".repeat(65)),
            Err(SectorIdError::InvalidFormat)
        );
        let text = "a".repeat(64);
        assert_eq!(SectorId::try_new(&text).expect("식별자").as_str(), text);
    }

    #[test]
    fn exclusions_are_empty_by_default_and_reject_duplicates_and_overflow() {
        let empty = Exclusions::default();
        assert_eq!(empty.sectors(), []);
        assert_eq!(empty.security_types(), []);
        let sector = SectorId::try_new("sector-1").expect("섹터");
        assert_eq!(
            Exclusions::try_new(vec![sector.clone(), sector], vec![]),
            Err(ExclusionsError::DuplicateSector)
        );
        assert_eq!(
            Exclusions::try_new(
                vec![],
                vec![ExcludedSecurityType::Stock, ExcludedSecurityType::Stock]
            ),
            Err(ExclusionsError::DuplicateSecurityType)
        );
        let sectors: Vec<_> = (0_u16..256)
            .map(|index| SectorId::try_new(&format!("s-{index}")).expect("섹터"))
            .collect();
        let full =
            Exclusions::try_new(sectors.clone(), vec![ExcludedSecurityType::Etf]).expect("상한");
        assert_eq!(full.sectors(), sectors);
        assert_eq!(full.security_types(), &[ExcludedSecurityType::Etf]);
        let mut excess = sectors;
        excess.push(SectorId::try_new("extra").expect("섹터"));
        assert_eq!(
            Exclusions::try_new(excess, vec![]),
            Err(ExclusionsError::TooManySectors)
        );
        assert_eq!(
            Exclusions::try_new(
                vec![],
                vec![
                    ExcludedSecurityType::Stock,
                    ExcludedSecurityType::Etf,
                    ExcludedSecurityType::Stock
                ]
            ),
            Err(ExclusionsError::DuplicateSecurityType)
        );
        assert_eq!(SectorId::try_new("a").expect("최소 식별자").as_str(), "a");
        let all = vec![
            ExcludedSecurityType::Stock,
            ExcludedSecurityType::ForeignStock,
            ExcludedSecurityType::DepositaryReceipt,
            ExcludedSecurityType::InfrastructureFund,
            ExcludedSecurityType::Reit,
            ExcludedSecurityType::Etf,
            ExcludedSecurityType::ForeignEtf,
            ExcludedSecurityType::Etn,
            ExcludedSecurityType::StockWarrants,
        ];
        assert_eq!(
            Exclusions::try_new(vec![], all.clone())
                .expect("스펙 목록")
                .security_types(),
            all
        );
    }

    #[test]
    fn exclusion_uniqueness_matches_independent_set_reference() {
        TestRunner::new(Config::with_cases(512))
            .run(&proptest::collection::vec(0_u16..300, 0..300), |ids| {
                let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
                let sectors = ids
                    .iter()
                    .map(|id| SectorId::try_new(&format!("s-{id}")).expect("섹터"))
                    .collect();
                let expected = if ids.len() > 256 {
                    Err(ExclusionsError::TooManySectors)
                } else if unique.len() != ids.len() {
                    Err(ExclusionsError::DuplicateSector)
                } else {
                    Ok(())
                };
                proptest::prop_assert_eq!(
                    Exclusions::try_new(sectors, vec![]).map(|_| ()),
                    expected
                );
                Ok(())
            })
            .expect("독립 집합 기준");
    }
}
