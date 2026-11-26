use crate::{ReadRequest, ReadRoute, TokenManager, TokenRenewal};
use application::{
    Clock, HoldingsObservation, HttpStatus, PortError, PortErrorKind, ReadEndpoint,
    ResponseMismatch, RetryDelay, SecretStore, SecretValue,
};
use domain::{
    AccountId, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
    BrokerAccountTypeCode, MasterInstrumentFacts, Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnmatchedHoldings {
    Reject,
    ShowReported,
}

#[derive(Clone, Copy)]
pub(crate) enum MasterErrorMode {
    Legacy,
    Dedicated,
}

impl MasterErrorMode {
    pub(crate) fn missing(self, endpoint: ReadEndpoint, legacy: PortError) -> PortError {
        match self {
            Self::Legacy => legacy,
            Self::Dedicated => PortError::new(
                endpoint,
                HttpStatus::Success,
                PortErrorKind::MasterNotReady(application::MasterNotReady::MissingInstrument),
            ),
        }
    }

    pub(crate) fn conflict(self, endpoint: ReadEndpoint, legacy: PortError) -> PortError {
        match self {
            Self::Legacy => legacy,
            Self::Dedicated => invalid(endpoint, ResponseMismatch::Validation),
        }
    }
}

pub struct TossAccountRead<Store, Time, Renewal> {
    tokens: TokenManager<Store, Time, Renewal>,
    clock: Time,
    master: std::sync::RwLock<std::sync::Arc<MasterInstrumentFacts>>,
    candle_sessions: Vec<domain::MarketSession>,
    live_sessions: std::sync::RwLock<Vec<domain::MarketSession>>,
    session_supply: SessionSupply,
    unmatched: UnmatchedHoldings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionSupply {
    Fixed,
    Live,
}
impl<Store, Time, Renewal> std::fmt::Debug for TossAccountRead<Store, Time, Renewal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossAccountRead(<redacted>)")
    }
}
impl<Store: SecretStore, Time: Clock, Renewal: TokenRenewal> TossAccountRead<Store, Time, Renewal> {
    #[must_use]
    pub fn with_instrument_master<
        Authority: application::InstrumentMasterAuthority,
        Issuer: application::InstrumentIdIssuer,
    >(
        self,
        authority: Authority,
        issuer: Issuer,
    ) -> crate::TossInstrumentRead<Store, Time, Renewal, Authority, Issuer> {
        crate::TossInstrumentRead {
            reader: self,
            authority,
            issuer,
        }
    }

    pub fn new(
        tokens: TokenManager<Store, Time, Renewal>,
        clock: Time,
        master: MasterInstrumentFacts,
    ) -> Self {
        Self {
            tokens,
            clock,
            master: std::sync::RwLock::new(std::sync::Arc::new(master)),
            candle_sessions: Vec::new(),
            live_sessions: std::sync::RwLock::new(Vec::new()),
            session_supply: SessionSupply::Fixed,
            unmatched: UnmatchedHoldings::Reject,
        }
    }

    #[must_use]
    pub const fn showing_unmatched_holdings(mut self) -> Self {
        self.unmatched = UnmatchedHoldings::ShowReported;
        self
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
        let body: serde_json::Value = serde_json::from_slice(response.body().as_exposed_secret())
            .map_err(|error| {
            crate::diagnostics::decode_failed(endpoint, &error);
            invalid(endpoint, ResponseMismatch::Schema)
        })?;
        if body.get("error").is_some_and(|error| !error.is_null()) {
            crate::diagnostics::schema_failed(endpoint, "envelope error field is set");
            return Err(invalid(endpoint, ResponseMismatch::Schema));
        }
        body.get("result")
            .cloned()
            .filter(|value| !value.is_null())
            .ok_or_else(|| {
                crate::diagnostics::schema_failed(endpoint, "envelope result is missing");
                invalid(endpoint, ResponseMismatch::Schema)
            })
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
        master_errors: MasterErrorMode,
    ) -> Result<HoldingsObservation, PortError> {
        let request = ReadRequest::new(ReadRoute::Holdings)
            .with_account(SecretValue::new(account.as_i64().to_string().into_bytes()));
        let value = self.read(request, ReadEndpoint::Holdings).await?;
        crate::holding_read::holdings(
            &value,
            &self.market_master(),
            self.clock.now(),
            (master_errors, self.unmatched),
        )
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
    #[must_use]
    pub const fn with_live_market_sessions(mut self) -> Self {
        self.session_supply = SessionSupply::Live;
        self
    }
    pub(crate) const fn session_supply(&self) -> SessionSupply {
        self.session_supply
    }
    pub(crate) fn known_sessions(&self) -> Vec<domain::MarketSession> {
        let mut known = self.candle_sessions.clone();
        match self.live_sessions.read() {
            Ok(guard) => known.extend(guard.iter().cloned()),
            Err(poisoned) => known.extend(poisoned.into_inner().iter().cloned()),
        }
        known
    }
    pub(crate) fn remember_sessions(&self, calendar: &domain::MarketCalendar) {
        let mut rows = vec![
            calendar.previous_business_day().clone(),
            calendar.today().clone(),
            calendar.next_business_day().clone(),
        ];
        rows.extend(
            calendar
                .implied_holidays()
                .iter()
                .map(|holiday| holiday.today().clone()),
        );
        let known = self.known_sessions();
        let fresh: Vec<domain::MarketSession> = rows
            .into_iter()
            .filter(|row| {
                !known
                    .iter()
                    .any(|entry| entry.market() == row.market() && entry.date() == row.date())
            })
            .collect();
        match self.live_sessions.write() {
            Ok(mut guard) => guard.extend(fresh),
            Err(poisoned) => poisoned.into_inner().extend(fresh),
        }
    }
    #[must_use]
    pub fn with_market_master(self, master: MasterInstrumentFacts) -> Self {
        self.replace_market_master(master);
        self
    }
    pub fn replace_market_master(&self, master: MasterInstrumentFacts) {
        let replacement = std::sync::Arc::new(master);
        match self.master.write() {
            Ok(mut guard) => *guard = replacement,
            Err(poisoned) => *poisoned.into_inner() = replacement,
        }
    }
    pub(crate) fn market_master(&self) -> std::sync::Arc<MasterInstrumentFacts> {
        match self.master.read() {
            Ok(guard) => std::sync::Arc::clone(&guard),
            Err(poisoned) => std::sync::Arc::clone(&poisoned.into_inner()),
        }
    }
    pub async fn instrument_master<
        Authority: application::InstrumentMasterAuthority,
        Issuer: application::InstrumentIdIssuer,
    >(
        &self,
        authority: &Authority,
        issuer: &Issuer,
        market: domain::KnownMarket,
    ) -> Result<domain::InstrumentMaster, PortError> {
        crate::instrument_master::fetch_parts(
            self,
            (authority, issuer),
            market,
            crate::instrument_master::RowPolicy::SkipInvalid,
        )
        .await
    }
    pub async fn reconnect(&self) -> Result<(), crate::TokenError> {
        self.tokens
            .reconnect_with_stored_keys()
            .await
            .map(|_lease| ())
    }
    pub async fn connection_status(&self) -> Result<(), crate::TokenError> {
        self.tokens.token().await.map(|_lease| ())
    }
    pub async fn calendar_on(
        &self,
        market: domain::KnownMarket,
        date: domain::CalendarDate,
    ) -> Result<domain::MarketCalendar, PortError> {
        crate::market_calendar::fetch_on(self, market, date).await
    }
    pub async fn history_daily_candles(
        &self,
        instrument: &domain::QuoteInstrument,
        window: domain::TimeInterval,
    ) -> Result<Vec<domain::Candle>, PortError> {
        crate::market_read::candles::history(self, instrument, window).await
    }
    pub fn tokens(&self) -> &TokenManager<Store, Time, Renewal> {
        &self.tokens
    }
    pub(crate) async fn fetch_order(
        &self,
        account: &AccountId,
        order_id: &domain::BrokerOrderId,
        master_errors: MasterErrorMode,
    ) -> Result<domain::BrokerOrder, PortError> {
        crate::order_read::order(self, account, order_id, master_errors).await
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
                application::BrokerCapability::SellableQuantity,
                application::BrokerCapability::OpenOrders,
                application::BrokerCapability::ClosedOrders,
                application::BrokerCapability::Order,
                application::BrokerCapability::StockWarnings,
                application::BrokerCapability::CommissionRate,
                application::BrokerCapability::IndicatorQuotes,
                application::BrokerCapability::IndicatorCandles,
                application::BrokerCapability::Calendar,
                application::BrokerCapability::Sectors,
                application::BrokerCapability::Sector,
                application::BrokerCapability::SectorComposition,
                application::BrokerCapability::SectorEtfs,
                application::BrokerCapability::Rankings,
                application::BrokerCapability::InvestorTradingRankings,
                application::BrokerCapability::SectorRankings,
                application::BrokerCapability::StockInvestorFlow,
                application::BrokerCapability::IndicatorInvestorFlow,
                application::BrokerCapability::ProgramTrades,
                application::BrokerCapability::ShortSelling,
                application::BrokerCapability::CreditTrades,
                application::BrokerCapability::SecuritiesLending,
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
    let failure = || {
        crate::diagnostics::validation_failed(ReadEndpoint::Accounts, "account row rejected");
        invalid(ReadEndpoint::Accounts, ResponseMismatch::Validation)
    };
    let decoded: Vec<broker_toss_schema::types::Account> =
        serde_json::from_value(value).map_err(|error| {
            crate::diagnostics::decode_failed(ReadEndpoint::Accounts, &error);
            invalid(ReadEndpoint::Accounts, ResponseMismatch::Schema)
        })?;
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
