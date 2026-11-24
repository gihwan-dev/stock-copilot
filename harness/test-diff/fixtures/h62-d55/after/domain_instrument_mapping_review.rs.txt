#[cfg(test)]
mod tests {
    use domain::{
        InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
        InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, MappingAssignment, Symbol,
    };
    use proptest::{
        collection::vec,
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    use std::collections::BTreeMap;

    fn source(symbol: &str, isin: &str) -> InstrumentSource {
        source_in(KnownMarket::Us, symbol, isin)
    }

    fn source_in(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("심볼"),
            IsinCode::try_new(isin).expect("ISIN"),
        )
    }

    fn issued_slot(slot: u8) -> domain::IssuedInstrumentId {
        let mut issuer = domain::IndependentInstrumentIdIssuer::new();
        for _ in 0..slot {
            let _skip = issuer.issue().expect("건너뛰기");
        }
        issuer.issue().expect("후보 발급")
    }

    fn issued(value: &str) -> domain::IssuedInstrumentId {
        let slot = match value {
            "opaque-1" => 0_u8,
            "opaque-2" => 1,
            "unknown" => 2,
            "candidate-0" => 3,
            "candidate-1" => 4,
            "candidate-2" => 5,
            other => panic!("알 수 없는 후보 라벨: {other}"),
        };
        issued_slot(slot)
    }

    fn id(value: &str) -> InstrumentId {
        issued(value).as_instrument_id().clone()
    }

    #[test]
    fn read_only_lookup_keeps_identity_and_status_after_delisting() {
        let first = source("AAPL", "US0378331005");
        let other = source("MSFT", "US5949181045");
        let ambiguous = source("AAPL", "US0000000002");
        let mut mappings = InstrumentMappings::default();
        assert_eq!(
            mappings.lookup(&first),
            Err(InstrumentMappingError::Missing)
        );
        assert_eq!(
            mappings.for_id(&id("opaque-1")),
            Err(InstrumentMappingError::Missing)
        );
        mappings.ensure(&first, issued("opaque-1")).expect("발급");
        mappings.ensure(&other, issued("opaque-2")).expect("발급");
        let active = mappings.lookup(&first).expect("활성 조회");
        assert_eq!(active.source(), &first);
        assert_eq!(active.id(), &id("opaque-1"));
        assert_eq!(active.status(), InstrumentMappingStatus::Active);
        assert_eq!(mappings.resolve(&first), Ok(id("opaque-1")));
        assert_eq!(mappings.for_id(&id("opaque-1")), Ok(vec![active]));
        assert_eq!(
            mappings.lookup(&ambiguous),
            Err(InstrumentMappingError::IdentityUncertain)
        );
        mappings.retire(&first).expect("폐지");
        let retired = mappings.lookup(&first).expect("폐지 이력 조회");
        assert_eq!(retired.source(), &first);
        assert_eq!(retired.id(), &id("opaque-1"));
        assert_eq!(retired.status(), InstrumentMappingStatus::Retired);
        assert_eq!(mappings.for_id(&id("opaque-1")), Ok(vec![retired]));
        assert_eq!(
            mappings.resolve(&first),
            Err(InstrumentMappingError::Retired)
        );
        assert_eq!(mappings.resolve(&other), Ok(id("opaque-2")));
        assert_eq!(
            mappings.for_id(&id("unknown")),
            Err(InstrumentMappingError::Missing)
        );
    }

    #[test]
    fn typed_storage_record_preserves_source_id_and_status() {
        let original = source("AAPL", "US0378331005");
        let record = InstrumentMappingRecord::new(
            original.clone(),
            id("opaque-1"),
            InstrumentMappingStatus::Retired,
        );
        assert_eq!(record.source(), &original);
        assert_eq!(record.id(), &id("opaque-1"));
        assert_eq!(record.status(), InstrumentMappingStatus::Retired);
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ModelStatus {
        Active,
        Retired,
    }

    fn related(left: u8, right: u8) -> bool {
        [
            (0, 2),
            (2, 0),
            (0, 3),
            (3, 0),
            (0, 5),
            (5, 0),
            (2, 5),
            (5, 2),
        ]
        .contains(&(left, right))
    }

    struct ReferenceCase {
        sources: [InstrumentSource; 6],
        candidates: [InstrumentId; 3],
        model: BTreeMap<u8, (u8, ModelStatus)>,
        ledger: InstrumentMappings,
    }

    impl ReferenceCase {
        fn new(salt: u16) -> Self {
            Self {
                sources: [
                    source(&format!("S{salt}A"), &format!("I{salt}A")),
                    source(&format!("S{salt}B"), &format!("I{salt}B")),
                    source(&format!("S{salt}A"), &format!("I{salt}C")),
                    source(&format!("S{salt}C"), &format!("I{salt}A")),
                    source_in(KnownMarket::Kr, &format!("S{salt}A"), &format!("I{salt}D")),
                    source(&format!("s{salt}a"), &format!("I{salt}E")),
                ],
                candidates: [id("candidate-0"), id("candidate-1"), id("candidate-2")],
                model: BTreeMap::new(),
                ledger: InstrumentMappings::default(),
            }
        }

        fn candidate(&self, number: u8) -> InstrumentId {
            self.candidates
                .get(usize::from(number))
                .expect("후보 범위")
                .clone()
        }

        fn is_uncertain(&self, number: u8) -> bool {
            self.model.keys().any(|other| related(*other, number))
        }

        fn expected_ensure(
            &mut self,
            source_number: u8,
            candidate_number: u8,
        ) -> Result<MappingAssignment, InstrumentMappingError> {
            match self.model.get(&source_number).copied() {
                Some((assigned, ModelStatus::Active)) => {
                    Ok(MappingAssignment::Existing(self.candidate(assigned)))
                }
                Some((_, ModelStatus::Retired)) => Err(InstrumentMappingError::Retired),
                None if self.is_uncertain(source_number) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None if self
                    .model
                    .values()
                    .any(|(assigned, _)| *assigned == candidate_number) =>
                {
                    Err(InstrumentMappingError::IdAlreadyUsed)
                }
                None => {
                    self.model
                        .insert(source_number, (candidate_number, ModelStatus::Active));
                    Ok(MappingAssignment::Issued(self.candidate(candidate_number)))
                }
            }
        }

        fn expected_retire(
            &mut self,
            source_number: u8,
        ) -> Result<InstrumentId, InstrumentMappingError> {
            match self.model.get(&source_number).copied() {
                Some((assigned, _)) => {
                    self.model
                        .insert(source_number, (assigned, ModelStatus::Retired));
                    Ok(self.candidate(assigned))
                }
                None if self.is_uncertain(source_number) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None => Err(InstrumentMappingError::Missing),
            }
        }

        fn expected_resolve(
            &self,
            source_number: u8,
        ) -> Result<InstrumentId, InstrumentMappingError> {
            match self.model.get(&source_number).copied() {
                Some((assigned, ModelStatus::Active)) => Ok(self.candidate(assigned)),
                Some((_, ModelStatus::Retired)) => Err(InstrumentMappingError::Retired),
                None if self.is_uncertain(source_number) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None => Err(InstrumentMappingError::Missing),
            }
        }

        fn model_record(
            &self,
            source_number: u8,
            assigned: u8,
            status: ModelStatus,
        ) -> InstrumentMappingRecord {
            let source = self
                .sources
                .get(usize::from(source_number))
                .expect("원본 범위")
                .clone();
            let state = match status {
                ModelStatus::Active => InstrumentMappingStatus::Active,
                ModelStatus::Retired => InstrumentMappingStatus::Retired,
            };
            InstrumentMappingRecord::new(source, self.candidate(assigned), state)
        }

        fn check_lookup(&self, source_number: u8) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let source = self
                .sources
                .get(usize::from(source_number))
                .expect("원본 범위");
            let expected = match self.model.get(&source_number).copied() {
                Some((assigned, status)) => Ok(self.model_record(source_number, assigned, status)),
                None if self.is_uncertain(source_number) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None => Err(InstrumentMappingError::Missing),
            };
            prop_assert_eq!(self.ledger.lookup(source), expected);
            prop_assert_eq!(&self.ledger, &before);
            Ok(())
        }

        fn check_for_id(&self, candidate_number: u8) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let expected = match self
                .model
                .iter()
                .find(|(_, (assigned, _))| *assigned == candidate_number)
            {
                Some((source_number, (assigned, status))) => {
                    Ok(vec![self.model_record(*source_number, *assigned, *status)])
                }
                None => Err(InstrumentMappingError::Missing),
            };
            prop_assert_eq!(
                self.ledger.for_id(&self.candidate(candidate_number)),
                expected
            );
            prop_assert_eq!(&self.ledger, &before);
            Ok(())
        }

        fn check_ensure(
            &mut self,
            source_number: u8,
            candidate_number: u8,
        ) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let expected = self.expected_ensure(source_number, candidate_number);
            let source = self
                .sources
                .get(usize::from(source_number))
                .expect("원본 범위");
            let candidate = issued(&format!("candidate-{candidate_number}"));
            let actual = self.ledger.ensure(source, candidate);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn check_retire(&mut self, source_number: u8) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let expected = self.expected_retire(source_number);
            let source = self
                .sources
                .get(usize::from(source_number))
                .expect("원본 범위");
            let actual = self.ledger.retire(source);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn check_resolve(&self, source_number: u8) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let source = self
                .sources
                .get(usize::from(source_number))
                .expect("원본 범위");
            prop_assert_eq!(
                self.ledger.resolve(source),
                self.expected_resolve(source_number)
            );
            prop_assert_eq!(&self.ledger, &before);
            Ok(())
        }

        fn run(&mut self, operations: Vec<(u8, u8, u8)>) -> proptest::test_runner::TestCaseResult {
            for (operation, source_number, candidate_number) in operations {
                match operation {
                    0 => self.check_ensure(source_number, candidate_number)?,
                    1 => self.check_retire(source_number)?,
                    _ => self.check_resolve(source_number)?,
                }
                self.check_lookup(source_number)?;
                self.check_for_id(candidate_number)?;
            }
            Ok(())
        }
    }

    #[test]
    fn generated_operation_sequences_match_the_independent_reference_model() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(0_u16..10_000, vec((0_u8..3, 0_u8..6, 0_u8..3), 0..40)),
                |(salt, operations)| ReferenceCase::new(salt).run(operations),
            )
            .expect("원장 상태 기계 속성");
    }
}
