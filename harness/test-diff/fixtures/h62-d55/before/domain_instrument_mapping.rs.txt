#[cfg(test)]
mod tests {
    use domain::{
        InstrumentId, InstrumentMappingError, InstrumentMappings, InstrumentSource, IsinCode,
        KnownMarket, MappingAssignment, Symbol,
    };
    use proptest::{
        prop_assert_eq, prop_assert_ne,
        test_runner::{Config, TestRunner},
    };

    fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("심볼"),
            IsinCode::try_new(isin).expect("ISIN"),
        )
    }

    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("내부 ID")
    }

    #[test]
    fn mapping_preserves_each_source_field_and_uses_existing_id() {
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let mut mappings = InstrumentMappings::default();
        assert_eq!(first.market(), KnownMarket::Kr);
        assert_eq!(first.symbol().as_str(), "005930");
        assert_eq!(first.isin().as_str(), "KR7005930003");
        assert_eq!(
            mappings.resolve(&first),
            Err(InstrumentMappingError::Missing)
        );
        assert_eq!(
            mappings.ensure(&first, &id("opaque-1")),
            Ok(MappingAssignment::Issued(id("opaque-1")))
        );
        assert_eq!(
            mappings.ensure(&first, &id("opaque-2")),
            Ok(MappingAssignment::Existing(id("opaque-1")))
        );
        assert_eq!(mappings.resolve(&first), Ok(id("opaque-1")));
        let other = source(KnownMarket::Us, "AAPL", "US0378331005");
        assert_eq!(
            mappings.resolve(&other),
            Err(InstrumentMappingError::Missing)
        );
    }

    #[test]
    fn uncertain_changes_fail_without_mutating_or_reusing_candidates() {
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let renamed = source(KnownMarket::Kr, "005931", "KR7005930003");
        let reused = source(KnownMarket::Kr, "005930", "KR7000000001");
        let transferred = source(KnownMarket::Us, "AAPL", "KR7005930003");
        let independent = source(KnownMarket::Us, "MSFT", "US5949181045");
        let mut mappings = InstrumentMappings::default();
        mappings.ensure(&original, &id("opaque-1")).expect("발급");
        for candidate in [&renamed, &reused, &transferred] {
            assert_eq!(
                mappings.resolve(candidate),
                Err(InstrumentMappingError::IdentityUncertain)
            );
            assert_eq!(
                mappings.retire(candidate),
                Err(InstrumentMappingError::IdentityUncertain)
            );
            assert_eq!(
                mappings.ensure(candidate, &id("opaque-2")),
                Err(InstrumentMappingError::IdentityUncertain)
            );
        }
        assert_eq!(
            mappings.ensure(&independent, &id("opaque-1")),
            Err(InstrumentMappingError::IdAlreadyUsed)
        );
        assert_eq!(mappings.resolve(&original), Ok(id("opaque-1")));
        assert_eq!(
            mappings.ensure(&independent, &id("opaque-2")),
            Ok(MappingAssignment::Issued(id("opaque-2")))
        );
    }

    #[test]
    fn delisting_is_permanent_and_relisting_requires_external_resolution() {
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let reused = source(KnownMarket::Kr, "005930", "KR7000000001");
        let missing = source(KnownMarket::Us, "AAPL", "US0378331005");
        let mut mappings = InstrumentMappings::default();
        assert_eq!(
            mappings.retire(&missing),
            Err(InstrumentMappingError::Missing)
        );
        mappings.ensure(&original, &id("opaque-1")).expect("발급");
        assert_eq!(mappings.retire(&original), Ok(id("opaque-1")));
        assert_eq!(mappings.retire(&original), Ok(id("opaque-1")));
        assert_eq!(
            mappings.resolve(&original),
            Err(InstrumentMappingError::Retired)
        );
        assert_eq!(
            mappings.ensure(&original, &id("opaque-2")),
            Err(InstrumentMappingError::Retired)
        );
        assert_eq!(
            mappings.ensure(&reused, &id("opaque-2")),
            Err(InstrumentMappingError::IdentityUncertain)
        );
        assert_eq!(
            mappings.resolve(&reused),
            Err(InstrumentMappingError::IdentityUncertain)
        );
        assert_eq!(
            mappings.ensure(&missing, &id("opaque-1")),
            Err(InstrumentMappingError::IdAlreadyUsed)
        );
    }

    #[test]
    fn same_symbol_in_different_markets_needs_a_distinct_isin_and_candidate() {
        let domestic = source(KnownMarket::Kr, "ABC", "KR0000000001");
        let foreign = source(KnownMarket::Us, "ABC", "US0000000002");
        let mut mappings = InstrumentMappings::default();
        assert_eq!(
            mappings.ensure(&domestic, &id("domestic")),
            Ok(MappingAssignment::Issued(id("domestic")))
        );
        assert_eq!(
            mappings.ensure(&foreign, &id("foreign")),
            Ok(MappingAssignment::Issued(id("foreign")))
        );
        assert_eq!(mappings.resolve(&domestic), Ok(id("domestic")));
        assert_eq!(mappings.resolve(&foreign), Ok(id("foreign")));
    }

    #[test]
    fn mapping_properties_hold_with_fixed_independent_candidates() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(0_u32..100_000, 100_000_u32..200_000),
                |(left_number, right_number)| {
                    let left = source(
                        KnownMarket::Kr,
                        &format!("S{left_number}"),
                        &format!("K{left_number}"),
                    );
                    let right = source(
                        KnownMarket::Us,
                        &format!("S{right_number}"),
                        &format!("U{right_number}"),
                    );
                    let left_id = id("opaque-left-independent");
                    let right_id = id("opaque-right-independent");
                    let mut forward = InstrumentMappings::default();
                    let mut reverse = InstrumentMappings::default();
                    prop_assert_eq!(
                        forward.ensure(&left, &left_id),
                        Ok(MappingAssignment::Issued(left_id.clone()))
                    );
                    prop_assert_eq!(
                        forward.ensure(&right, &right_id),
                        Ok(MappingAssignment::Issued(right_id.clone()))
                    );
                    prop_assert_eq!(
                        reverse.ensure(&right, &right_id),
                        Ok(MappingAssignment::Issued(right_id.clone()))
                    );
                    prop_assert_eq!(
                        reverse.ensure(&left, &left_id),
                        Ok(MappingAssignment::Issued(left_id.clone()))
                    );
                    prop_assert_eq!(
                        forward.ensure(&left, &right_id.clone()),
                        Ok(MappingAssignment::Existing(left_id.clone()))
                    );
                    prop_assert_eq!(forward.resolve(&left), reverse.resolve(&left));
                    prop_assert_eq!(forward.resolve(&right), reverse.resolve(&right));
                    prop_assert_ne!(
                        forward.resolve(&left).expect("왼쪽"),
                        forward.resolve(&right).expect("오른쪽")
                    );
                    Ok(())
                },
            )
            .expect("대응 속성");
    }

    #[test]
    fn case_variants_never_become_distinct_new_sources() {
        let original = source(KnownMarket::Us, "AAPL", "US0378331005");
        let symbol_variant = source(KnownMarket::Us, "aapl", "US0000000002");
        let isin_variant = source(KnownMarket::Us, "DIFF", "us0378331005");
        let mut mappings = InstrumentMappings::default();
        mappings.ensure(&original, &id("opaque-1")).expect("발급");
        for changed in [&symbol_variant, &isin_variant] {
            assert_eq!(
                mappings.ensure(changed, &id("opaque-2")),
                Err(InstrumentMappingError::IdentityUncertain)
            );
            assert_eq!(
                mappings.resolve(changed),
                Err(InstrumentMappingError::IdentityUncertain)
            );
        }
    }

    #[test]
    fn retiring_an_unrelated_source_leaves_the_existing_mapping_unchanged() {
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let unrelated = source(KnownMarket::Us, "AAPL", "US0378331005");
        let mut mappings = InstrumentMappings::default();
        mappings.ensure(&original, &id("opaque-1")).expect("발급");
        assert_eq!(
            mappings.retire(&unrelated),
            Err(InstrumentMappingError::Missing)
        );
        assert_eq!(mappings.resolve(&original), Ok(id("opaque-1")));
        assert_eq!(
            mappings.resolve(&unrelated),
            Err(InstrumentMappingError::Missing)
        );
    }
}
