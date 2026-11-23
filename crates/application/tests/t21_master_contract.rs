#[cfg(test)]
mod tests {
    use application::{
        ClassifiedError, ErrorClass, HttpStatus, MasterNotReady, PortError, PortErrorKind,
        ReadEndpoint, RetryDelay,
    };
    use domain::{
        InstrumentMappingError, InstrumentMaster, InstrumentMasterEntry, InstrumentSource,
        IsinCode, KnownMarket, Symbol,
    };

    fn source() -> InstrumentSource {
        InstrumentSource::new(
            KnownMarket::Kr,
            Symbol::try_new("005930").expect("symbol"),
            IsinCode::try_new("KR7005930003").expect("isin"),
        )
    }

    #[test]
    fn an_unusable_instrument_is_explicit_and_cannot_be_converted_into_complete_facts() {
        for reason in [
            InstrumentMappingError::IdentityUncertain,
            InstrumentMappingError::Retired,
            InstrumentMappingError::IdAlreadyUsed,
        ] {
            let entry = InstrumentMasterEntry::Unavailable {
                source: source(),
                reason: reason.clone(),
            };
            let master = InstrumentMaster::try_new(vec![entry.clone()]).expect("master");
            assert_eq!(master.entries(), &[entry]);
            assert_eq!(
                master.complete_facts(),
                Err(domain::InstrumentMasterError::UnavailableInstrument)
            );
        }
    }

    #[test]
    fn duplicate_unusable_sources_are_not_a_complete_universe() {
        let entry = InstrumentMasterEntry::Unavailable {
            source: source(),
            reason: InstrumentMappingError::IdentityUncertain,
        };
        assert_eq!(
            InstrumentMaster::try_new(vec![entry.clone(), entry]),
            Err(domain::InstrumentMasterError::DuplicateIdentity)
        );
    }

    #[test]
    fn symbol_case_cannot_hide_a_duplicate_unusable_source() {
        let first = InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new("AAPL").expect("symbol"),
            IsinCode::try_new("US0378331005").expect("isin"),
        );
        let second = InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new("aapl").expect("symbol"),
            IsinCode::try_new("US0378331005").expect("isin"),
        );
        assert_eq!(
            InstrumentMaster::try_new(vec![
                InstrumentMasterEntry::Unavailable {
                    source: first,
                    reason: InstrumentMappingError::IdentityUncertain
                },
                InstrumentMasterEntry::Unavailable {
                    source: second,
                    reason: InstrumentMappingError::IdentityUncertain
                },
            ]),
            Err(domain::InstrumentMasterError::DuplicateIdentity)
        );
    }

    #[test]
    fn an_unready_master_has_its_own_non_bug_classification() {
        for reason in [
            MasterNotReady::StatusCoverage,
            MasterNotReady::IdentityContinuity,
            MasterNotReady::MissingInstrument,
        ] {
            let error = PortError::new(
                ReadEndpoint::Instruments,
                HttpStatus::NotReceived,
                PortErrorKind::MasterNotReady(reason),
            );
            assert_eq!(error.kind(), PortErrorKind::MasterNotReady(reason));
            assert_eq!(error.class(), ErrorClass::Wait(RetryDelay::Unknown));
        }
    }

    #[test]
    fn system_failures_keep_their_original_classification() {
        use application::{SystemFailure, SystemPort, SystemPortError};
        for failure in [
            SystemFailure::RepositoryIntegrity,
            SystemFailure::InvalidData,
            SystemFailure::PermissionDenied,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ] {
            let original = SystemPortError::new(SystemPort::Repository, failure);
            for reason in [
                MasterNotReady::Storage(original),
                MasterNotReady::Issuer(original),
            ] {
                let error = PortError::new(
                    ReadEndpoint::Instruments,
                    HttpStatus::NotReceived,
                    PortErrorKind::MasterNotReady(reason),
                );
                assert_eq!(error.kind(), PortErrorKind::MasterNotReady(reason));
                assert_eq!(error.class(), original.class());
            }
        }
    }
}
