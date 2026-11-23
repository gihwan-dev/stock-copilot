#[cfg(test)]
mod tests {
    use domain::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, InstrumentMappingError,
        InstrumentMaster, InstrumentMasterEntry, InstrumentMasterError, InstrumentSource, IsinCode,
        KnownMarket, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };

    fn master(symbol: &str, id: &str, exchange: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(exchange).expect("exchange"),
            symbol: Symbol::try_new(symbol).expect("symbol"),
            instrument_id: InstrumentId::try_new(id).expect("id"),
            name: DisplayInstrumentName::try_new("합성 이름").expect("name"),
            security_type: MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("type")),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(15),
                Nanosecond::new(20),
                OffsetSeconds::new(0),
            )
            .expect("time"),
        })
        .expect("master")
    }
    fn source(symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new(symbol).expect("symbol"),
            IsinCode::try_new(isin).expect("isin"),
        )
    }

    #[test]
    fn available_entries_form_complete_facts_without_reordering() {
        let first = master("AAPL", "opaque-11", "NASDAQ");
        let second = master("MSFT", "opaque-22", "NASDAQ");
        let entries = vec![
            InstrumentMasterEntry::Available(first.clone()),
            InstrumentMasterEntry::Available(second.clone()),
        ];
        let result = InstrumentMaster::try_new(entries.clone()).expect("master");
        assert_eq!(result.entries(), entries);
        assert_eq!(
            result.complete_facts().expect("complete").instruments(),
            &[first, second]
        );
        assert_eq!(
            InstrumentMaster::try_new(vec![])
                .expect("empty")
                .complete_facts()
                .expect("complete")
                .instruments(),
            []
        );
    }

    #[test]
    fn uncertain_retired_and_collision_rows_are_explicit_and_incomplete() {
        for reason in [
            InstrumentMappingError::IdentityUncertain,
            InstrumentMappingError::Retired,
            InstrumentMappingError::IdAlreadyUsed,
        ] {
            let available = InstrumentMasterEntry::Available(master("AAPL", "opaque-11", "NASDAQ"));
            let unavailable = InstrumentMasterEntry::Unavailable {
                source: source("MSFT", "US5949181045"),
                reason,
            };
            let result = InstrumentMaster::try_new(vec![available.clone(), unavailable.clone()])
                .expect("master");
            assert_eq!(result.entries(), &[available, unavailable]);
            assert_eq!(
                result.complete_facts(),
                Err(InstrumentMasterError::UnavailableInstrument)
            );
        }
    }

    #[test]
    fn malformed_or_duplicate_entries_are_rejected() {
        let available = InstrumentMasterEntry::Available(master("AAPL", "opaque-11", "NASDAQ"));
        let same_id = InstrumentMasterEntry::Available(master("MSFT", "opaque-11", "NASDAQ"));
        let same_symbol = InstrumentMasterEntry::Available(master("AAPL", "opaque-22", "NASDAQ"));
        let same_symbol_different_case = InstrumentMasterEntry::Unavailable {
            source: source("aapl", "US0378331005"),
            reason: InstrumentMappingError::Retired,
        };
        let missing = InstrumentMasterEntry::Unavailable {
            source: source("MSFT", "US5949181045"),
            reason: InstrumentMappingError::Missing,
        };
        let unknown =
            InstrumentMasterEntry::Available(master("IBM", "opaque-33", "UNKNOWN_EXCHANGE"));
        for duplicate in [same_id, same_symbol, same_symbol_different_case] {
            assert_eq!(
                InstrumentMaster::try_new(vec![available.clone(), duplicate]),
                Err(InstrumentMasterError::DuplicateIdentity)
            );
        }
        assert_eq!(
            InstrumentMaster::try_new(vec![missing]),
            Err(InstrumentMasterError::InvalidEntry)
        );
        assert_eq!(
            InstrumentMaster::try_new(vec![unknown]),
            Err(InstrumentMasterError::InvalidEntry)
        );
    }
}
