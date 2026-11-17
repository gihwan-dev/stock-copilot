use crate::{ReadRequest, ReadRoute, TokenManager, TokenRenewal};
use application::{
    Clock, HoldingsObservation, HttpStatus, PortError, PortErrorKind, ReadEndpoint,
    ResponseMismatch, RetryDelay, SecretStore, SecretValue,
};
use domain::{
    AccountId, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
    BrokerAccountTypeCode, MasterInstrumentFacts, Timestamp,
};

pub struct TossAccountRead<Store, Time, Renewal> {
    tokens: TokenManager<Store, Time, Renewal>,
    clock: Time,
    master: MasterInstrumentFacts,
    candle_sessions: Vec<domain::MarketSession>,
}
impl<Store, Time, Renewal> std::fmt::Debug for TossAccountRead<Store, Time, Renewal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossAccountRead(<redacted>)")
    }
}
impl<Store: SecretStore, Time: Clock, Renewal: TokenRenewal> TossAccountRead<Store, Time, Renewal> {
    pub fn new(
        tokens: TokenManager<Store, Time, Renewal>,
        clock: Time,
        master: MasterInstrumentFacts,
    ) -> Self {
        Self {
            tokens,
            clock,
            master,
            candle_sessions: Vec::new(),
        }
    }
    async fn read(
        &self,
        request: ReadRequest,
        endpoint: ReadEndpoint,
    ) -> Result<serde_json::Value, PortError> {
        let response = self
            .tokens
            .get(&request)
            .await
            .map_err(|error| token_error(error, endpoint))?;
        if !(200..300).contains(&response.status()) {
            return Err(response
                .read_error(endpoint, retry_delay(response.headers().retry_after()))
                .port_error());
        }
        let body: serde_json::Value =
            serde_json::from_slice(response.body().as_exposed_secret())
                .map_err(|_error| invalid(endpoint, ResponseMismatch::Schema))?;
        if body.get("error").is_some_and(|error| !error.is_null()) {
            return Err(invalid(endpoint, ResponseMismatch::Schema));
        }
        body.get("result")
            .cloned()
            .filter(|value| !value.is_null())
            .ok_or_else(|| invalid(endpoint, ResponseMismatch::Schema))
    }
    pub(crate) async fn fetch_accounts(&self) -> Result<BrokerAccountRows, PortError> {
        let value = self
            .read(
                ReadRequest::new(ReadRoute::Accounts),
                ReadEndpoint::Accounts,
            )
            .await?;
        accounts(value, self.clock.now())
    }
    pub(crate) async fn fetch_holdings(
        &self,
        account: &AccountId,
    ) -> Result<HoldingsObservation, PortError> {
        let request = ReadRequest::new(ReadRoute::Holdings)
            .with_account(SecretValue::new(account.as_i64().to_string().into_bytes()));
        let value = self.read(request, ReadEndpoint::Holdings).await?;
        crate::holding_read::holdings(&value, &self.master, self.clock.now())
    }
    pub(crate) async fn fetch_transactions(
        &self,
        account: &AccountId,
        window: domain::SettlementWindow,
        filter: &domain::TransactionFilter,
    ) -> Result<application::TransactionRead, PortError> {
        crate::transaction_read::fetch(&self.tokens, account, window, filter).await
    }
    #[must_use]
    pub fn with_candle_sessions(mut self, sessions: Vec<domain::MarketSession>) -> Self {
        self.candle_sessions = sessions;
        self
    }
    pub(crate) fn market_master(&self) -> &MasterInstrumentFacts {
        &self.master
    }
    pub(crate) async fn market_read(
        &self,
        request: ReadRequest,
        endpoint: ReadEndpoint,
    ) -> Result<serde_json::Value, PortError> {
        self.read(request, endpoint).await
    }
    pub(crate) fn market_now(&self) -> Timestamp {
        self.clock.now()
    }
    pub(crate) fn market_sessions(&self) -> &[domain::MarketSession] {
        &self.candle_sessions
    }
    #[must_use]
    pub fn capabilities() -> application::BrokerCapabilities {
        application::BrokerCapabilities::new(
            [
                application::BrokerCapability::DailyCandles,
                application::BrokerCapability::MinuteCandles,
                application::BrokerCapability::AdjustedHistory,
                application::BrokerCapability::RawHistory,
            ]
            .into_iter()
            .collect(),
        )
    }
}
pub(crate) fn retry_delay(header: Option<&str>) -> RetryDelay {
    header
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|seconds| {
            domain::TimeSpan::try_new(
                domain::SpanSeconds::new(seconds),
                domain::Nanosecond::new(0),
            )
            .ok()
        })
        .map_or(RetryDelay::Unknown, RetryDelay::Known)
}
pub(crate) fn invalid(endpoint: ReadEndpoint, mismatch: ResponseMismatch) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::Success,
        PortErrorKind::InvalidResponse(mismatch),
    )
}
pub(crate) fn token_error(error: crate::TokenError, endpoint: ReadEndpoint) -> PortError {
    use crate::{TokenError, TossHttpError};
    use application::{AccessDenial, UnavailableCause};
    let kind = match error {
        TokenError::Rejected(kind) => kind,
        TokenError::RevokedRepeated => {
            PortErrorKind::AccessDenied(AccessDenial::TokenRevokedRepeated)
        }
        TokenError::MissingCredentials | TokenError::Http(TossHttpError::InvalidCredential) => {
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials)
        }
        TokenError::InvalidResponse | TokenError::Http(TossHttpError::ResponseTooLarge) => {
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema)
        }
        TokenError::Http(TossHttpError::RateLimited) => PortErrorKind::Unavailable {
            cause: UnavailableCause::RateLimited,
            retry: RetryDelay::Unknown,
        },
        TokenError::Store(store_failure) => store_error(store_failure),
        TokenError::Http(TossHttpError::InvalidEndpoint | TossHttpError::Configuration) => {
            PortErrorKind::InvalidRequest(application::RequestRejection::MalformedRequest)
        }
        TokenError::Http(TossHttpError::Transport)
        | TokenError::Renewal
        | TokenError::Superseded => PortErrorKind::Unavailable {
            cause: UnavailableCause::Network,
            retry: RetryDelay::Unknown,
        },
    };
    PortError::new(endpoint, HttpStatus::NotReceived, kind)
}
fn store_error(error: application::SystemPortError) -> PortErrorKind {
    match error.failure() {
        application::SystemFailure::PermissionDenied => {
            PortErrorKind::AccessDenied(application::AccessDenial::Forbidden)
        }
        application::SystemFailure::Unavailable(retry) => PortErrorKind::Unavailable {
            cause: application::UnavailableCause::Network,
            retry,
        },
        application::SystemFailure::InvalidData
        | application::SystemFailure::RepositoryIntegrity
        | application::SystemFailure::SecretIntegrity => {
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        }
    }
}
fn accounts(value: serde_json::Value, observed: Timestamp) -> Result<BrokerAccountRows, PortError> {
    let failure = || invalid(ReadEndpoint::Accounts, ResponseMismatch::Validation);
    let decoded: Vec<broker_toss_schema::types::Account> = serde_json::from_value(value)
        .map_err(|_error| invalid(ReadEndpoint::Accounts, ResponseMismatch::Schema))?;
    let rows = decoded
        .into_iter()
        .map(|row| {
            let code = row.kind.to_string();
            BrokerAccountRow::from_broker_account_result(
                AccountId::new(row.seq),
                BrokerAccountTypeCode::new(&code),
                BrokerAccountNumberInput::new(row.no),
                observed,
            )
            .map_err(|_error| failure())
        })
        .collect::<Result<Vec<_>, _>>()?;
    BrokerAccountRows::try_new(rows).map_err(|_error| failure())
}

#[cfg(test)]
mod tests {
    use super::token_error;
    use crate::{TokenError, TossHttpError};
    use application::{
        AccessDenial, HttpStatus, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch,
        SystemFailure, SystemPort, SystemPortError,
    };

    #[test]
    fn server_retry_delay_preserves_only_representable_decimal_seconds() {
        for (header, seconds) in [
            ("0", 0_u64),
            ("2", 2),
            ("0007", 7),
            ("9223372036854775807", 9_223_372_036_854_775_807),
        ] {
            let expected = application::RetryDelay::Known(
                domain::TimeSpan::try_new(
                    domain::SpanSeconds::new(seconds),
                    domain::Nanosecond::new(0),
                )
                .expect("span"),
            );
            assert_eq!(super::retry_delay(Some(header)), expected);
        }
        for header in [
            None,
            Some(""),
            Some(" 2"),
            Some("+2"),
            Some("-2"),
            Some("2.0"),
            Some("9223372036854775808"),
            Some("18446744073709551616"),
            Some("Thu, 01 Jan 2037 00:00:00 GMT"),
        ] {
            assert_eq!(super::retry_delay(header), application::RetryDelay::Unknown);
        }
    }
    #[test]
    fn unavailable_store_preserves_its_retry_delay() {
        let delay = application::RetryDelay::Known(
            domain::TimeSpan::try_new(domain::SpanSeconds::new(17), domain::Nanosecond::new(0))
                .expect("delay"),
        );
        let error = TokenError::Store(SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(delay),
        ));
        assert_eq!(
            token_error(error, ReadEndpoint::Accounts).kind(),
            PortErrorKind::Unavailable {
                cause: application::UnavailableCause::Network,
                retry: delay
            }
        );
    }
    #[test]
    fn local_token_failures_do_not_become_network_waits() {
        for (failure, expected) in [
            (
                TokenError::Store(SystemPortError::new(
                    SystemPort::SecretStore,
                    SystemFailure::PermissionDenied,
                )),
                PortErrorKind::AccessDenied(AccessDenial::Forbidden),
            ),
            (
                TokenError::Store(SystemPortError::new(
                    SystemPort::SecretStore,
                    SystemFailure::SecretIntegrity,
                )),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            ),
            (
                TokenError::Store(SystemPortError::new(
                    SystemPort::SecretStore,
                    SystemFailure::RepositoryIntegrity,
                )),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            ),
            (
                TokenError::Store(SystemPortError::new(
                    SystemPort::SecretStore,
                    SystemFailure::InvalidData,
                )),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            ),
            (
                TokenError::Http(TossHttpError::InvalidCredential),
                PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
            ),
            (
                TokenError::Http(TossHttpError::Configuration),
                PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
            ),
            (
                TokenError::Http(TossHttpError::InvalidEndpoint),
                PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
            ),
        ] {
            let result = token_error(failure, ReadEndpoint::Holdings);
            assert_eq!(result.kind(), expected);
            assert_eq!(result.endpoint(), ReadEndpoint::Holdings);
            assert_eq!(result.status(), HttpStatus::NotReceived);
        }
    }
}
