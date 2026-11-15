#[cfg(test)]
mod tests {
    use application::{
        BrokerCapabilities, BrokerCapability, BugDiagnostic, CapabilitySupport, ErrorClass,
        HttpStatus, PortError, PortErrorKind, ReadEndpoint, classify_port_error,
    };
    #[test]
    fn d44_capabilities_and_errors_preserve_each_new_endpoint() {
        let features = [
            (BrokerCapability::ProgramTrades, ReadEndpoint::ProgramTrades),
            (BrokerCapability::ShortSelling, ReadEndpoint::ShortSelling),
            (BrokerCapability::CreditTrades, ReadEndpoint::CreditTrades),
            (
                BrokerCapability::SecuritiesLending,
                ReadEndpoint::SecuritiesLending,
            ),
        ];
        let none = BrokerCapabilities::new(std::collections::BTreeSet::new());
        for (cap, endpoint) in features {
            assert_eq!(none.support_for(cap), CapabilitySupport::Unsupported);
            let one = BrokerCapabilities::new(std::iter::once(cap).collect());
            for (other, _) in features {
                assert_eq!(
                    one.support_for(other),
                    if cap == other {
                        CapabilitySupport::Supported
                    } else {
                        CapabilitySupport::Unsupported
                    }
                );
            }
            let err = PortError::new(
                endpoint,
                HttpStatus::NotReceived,
                PortErrorKind::Unsupported(cap),
            );
            assert_eq!(err.endpoint(), endpoint);
            assert_eq!(
                classify_port_error(err),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(cap))
            );
        }
    }
}
