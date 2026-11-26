#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapability, HttpStatus, MarketToolError, MasterNotReady, MissingData,
        PortError, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch, RetryDelay,
        SystemFailure, SystemPort, SystemPortError, UnavailableCause,
    };

    fn mapped(kind: PortErrorKind) -> MarketToolError {
        MarketToolError::from(PortError::new(
            ReadEndpoint::Candles,
            HttpStatus::Success,
            kind,
        ))
    }

    fn unavailable(cause: UnavailableCause) -> PortErrorKind {
        PortErrorKind::Unavailable {
            cause,
            retry: RetryDelay::Unknown,
        }
    }

    #[test]
    fn transient_brokerage_failures_stay_retryable_kinds() {
        assert_eq!(
            mapped(unavailable(UnavailableCause::RateLimited)),
            MarketToolError::RateLimited
        );
        for cause in [
            UnavailableCause::Maintenance,
            UnavailableCause::ServerError,
            UnavailableCause::Network,
            UnavailableCause::ExpiredToken,
        ] {
            assert_eq!(mapped(unavailable(cause)), MarketToolError::Unavailable);
        }
        assert_eq!(
            mapped(PortErrorKind::MasterNotReady(
                MasterNotReady::StatusCoverage
            )),
            MarketToolError::Unavailable
        );
    }

    #[test]
    fn missing_capabilities_and_rejections_are_not_transient() {
        assert_eq!(
            mapped(PortErrorKind::Unsupported(BrokerCapability::DailyCandles)),
            MarketToolError::NotOffered
        );
        assert_eq!(
            mapped(PortErrorKind::NotFound(MissingData::Stock)),
            MarketToolError::NotFound
        );
        assert_eq!(
            mapped(PortErrorKind::MasterNotReady(
                MasterNotReady::MissingInstrument
            )),
            MarketToolError::NotFound
        );
        assert_eq!(
            mapped(PortErrorKind::AccessDenied(AccessDenial::Forbidden)),
            MarketToolError::Rejected
        );
        assert_eq!(
            mapped(PortErrorKind::InvalidRequest(
                RequestRejection::UnsupportedQuery
            )),
            MarketToolError::Rejected
        );
        assert_eq!(
            mapped(PortErrorKind::InvalidResponse(ResponseMismatch::Validation)),
            MarketToolError::InvalidResponse
        );
    }

    #[test]
    fn storage_failures_keep_their_port_error() {
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        assert_eq!(
            MarketToolError::from(failure),
            MarketToolError::Storage(failure)
        );
    }
}
