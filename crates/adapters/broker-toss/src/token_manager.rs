use crate::{RawResponse, ReadRequest, TossHttpClient, TossHttpError, issue_token};
use application::{
    Clock, DeliveryReport, EventBus, PortErrorKind, ReadEndpoint, RetryDelay, SecretKey,
    SecretStore, SecretValue, SystemPortError,
};
use domain::{DomainEvent, Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    MissingCredentials,
    Store(SystemPortError),
    Http(TossHttpError),
    Rejected(PortErrorKind),
    InvalidResponse,
    Renewal,
    RevokedRepeated,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerConnection {
    Disconnected,
    Connected,
    DuplicateKey,
}

pub trait TokenRenewal: Send + Sync {
    fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send;
    fn reconnect(&self, token: &SecretValue)
    -> impl Future<Output = Result<(), TokenError>> + Send;
    fn resubscribe(&self) -> impl Future<Output = Result<(), TokenError>> + Send;
    fn reconcile_orders(
        &self,
        token: &SecretValue,
    ) -> impl Future<Output = Result<(), TokenError>> + Send;
}
#[derive(Debug)]
struct Token {
    secret: SecretValue,
    refresh_at: Timestamp,
    expires_at: Timestamp,
}
#[derive(Debug, Clone)]
pub struct TokenLease(Arc<Token>);
impl TokenLease {
    #[must_use]
    pub fn secret(&self) -> &SecretValue {
        &self.0.secret
    }
}
#[derive(Debug)]
struct State {
    token: Option<TokenLease>,
    connection: BrokerConnection,
    last_error: Option<TokenError>,
    delivery: Option<DeliveryReport>,
}
struct Generation(AtomicUsize);
pub struct TokenManager<Store, Time, Renewal> {
    client: TossHttpClient,
    store: Store,
    clock: Time,
    renewal: Renewal,
    bus: Arc<EventBus>,
    generation: Generation,
    state: Mutex<State>,
}
#[derive(serde::Deserialize)]
enum TokenType {
    Bearer,
}
#[derive(serde::Deserialize)]
struct TokenResponse {
    #[serde(deserialize_with = "deserialize_secret")]
    access_token: SecretValue,
    token_type: TokenType,
    expires_in: u64,
}
struct SecretVisitor;
impl serde::de::Visitor<'_> for SecretVisitor {
    type Value = SecretValue;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("token string")
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(SecretValue::new(v.as_bytes().to_vec()))
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(SecretValue::new(v.into_bytes()))
    }
}
fn deserialize_secret<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<SecretValue, D::Error> {
    deserializer.deserialize_string(SecretVisitor)
}
#[derive(PartialEq, Eq)]
enum ConnectionAttempt {
    Normal,
    Recovery,
}
enum Acquisition {
    Shared(TokenLease),
    Issued(TokenLease),
}
impl Acquisition {
    fn token(self) -> TokenLease {
        match self {
            Self::Shared(token) | Self::Issued(token) => token,
        }
    }
}
#[derive(PartialEq, Eq)]
enum ReissueBudget {
    Available,
    Spent,
}
#[derive(PartialEq, Eq)]
enum AttemptCompletion {
    Pending,
    Completed,
}
struct Attempt<'a> {
    state: &'a mut State,
    bus: &'a EventBus,
    generation: &'a Generation,
    completion: AttemptCompletion,
}
fn change_connection(state: &mut State, bus: &EventBus, connection: BrokerConnection) {
    if state.connection != connection {
        state.connection = connection;
        state.delivery = Some(bus.publish(DomainEvent::BrokerConnectionChanged));
    }
}
impl Attempt<'_> {
    fn complete(&mut self, result: &Result<TokenLease, TokenError>) {
        self.completion = AttemptCompletion::Completed;
        self.state.last_error = result.as_ref().err().copied();
        self.generation.0.fetch_add(1, Ordering::SeqCst);
        match result {
            Ok(token) => {
                self.state.token = Some(token.clone());
                change_connection(self.state, self.bus, BrokerConnection::Connected);
            }
            Err(_error) => change_connection(self.state, self.bus, BrokerConnection::Disconnected),
        }
    }
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if self.completion == AttemptCompletion::Pending {
            self.state.last_error = Some(TokenError::Renewal);
            self.generation.0.fetch_add(1, Ordering::SeqCst);
            change_connection(self.state, self.bus, BrokerConnection::Disconnected);
        }
    }
}
fn parse_token(response: &RawResponse, started: Timestamp) -> Result<TokenLease, TokenError> {
    if response.status() != 200 {
        return Err(TokenError::Rejected(
            response
                .token_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
                .port_error()
                .kind(),
        ));
    }
    let parsed: TokenResponse = serde_json::from_slice(response.body().as_exposed_secret())
        .map_err(|_error| TokenError::InvalidResponse)?;
    let TokenType::Bearer = parsed.token_type;
    if parsed.access_token.as_exposed_secret().is_empty()
        || parsed.expires_in < 2
        || !parsed
            .access_token
            .as_exposed_secret()
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(byte))
    {
        return Err(TokenError::InvalidResponse);
    }
    let lifetime = TimeSpan::try_new(SpanSeconds::new(parsed.expires_in), Nanosecond::new(0))
        .map_err(|_error| TokenError::InvalidResponse)?;
    let margin = TimeSpan::try_new(
        SpanSeconds::new(std::cmp::min(60, parsed.expires_in.saturating_sub(1))),
        Nanosecond::new(0),
    )
    .map_err(|_error| TokenError::InvalidResponse)?;
    let expires_at = started
        .checked_add(lifetime)
        .map_err(|_error| TokenError::InvalidResponse)?;
    let refresh_at = expires_at
        .checked_sub(margin)
        .map_err(|_error| TokenError::InvalidResponse)?;
    Ok(TokenLease(Arc::new(Token {
        secret: parsed.access_token,
        refresh_at,
        expires_at,
    })))
}
impl<Store: SecretStore, Time: Clock, Renewal: TokenRenewal> TokenManager<Store, Time, Renewal> {
    pub fn new(
        client: TossHttpClient,
        store: Store,
        clock: Time,
        renewal: Renewal,
        bus: Arc<EventBus>,
    ) -> Self {
        Self {
            client,
            store,
            clock,
            renewal,
            bus,
            generation: Generation(AtomicUsize::new(0)),
            state: Mutex::new(State {
                token: None,
                connection: BrokerConnection::Disconnected,
                last_error: None,
                delivery: None,
            }),
        }
    }
    fn transition(&self, state: &mut State, connection: BrokerConnection) {
        change_connection(state, &self.bus, connection);
    }
    pub async fn connection(&self) -> BrokerConnection {
        self.state.lock().await.connection
    }
    pub async fn delivery_report(&self) -> Option<DeliveryReport> {
        self.state.lock().await.delivery.clone()
    }
    async fn issue(&self) -> Result<TokenLease, TokenError> {
        let id = self
            .store
            .load(SecretKey::TossClientId)
            .await
            .map_err(TokenError::Store)?
            .ok_or(TokenError::MissingCredentials)?;
        let secret = self
            .store
            .load(SecretKey::TossClientSecret)
            .await
            .map_err(TokenError::Store)?
            .ok_or(TokenError::MissingCredentials)?;
        let started = self.clock.now();
        let response = issue_token(&self.client, &id, &secret)
            .await
            .map_err(TokenError::Http)?;
        let token = parse_token(&response, started)?;
        if self.clock.now() >= token.0.expires_at {
            return Err(TokenError::InvalidResponse);
        }
        self.renewal.close().await?;
        self.renewal.reconnect(token.secret()).await?;
        self.renewal.resubscribe().await?;
        self.renewal.reconcile_orders(token.secret()).await?;
        if self.clock.now() >= token.0.expires_at {
            return Err(TokenError::InvalidResponse);
        }
        Ok(token)
    }
    async fn acquire(
        &self,
        rejected: Option<&TokenLease>,
        mode: ConnectionAttempt,
    ) -> Result<Acquisition, TokenError> {
        let observed = self.generation.0.load(Ordering::SeqCst);
        let mut state = self.state.lock().await;
        if state.connection == BrokerConnection::DuplicateKey && mode == ConnectionAttempt::Normal {
            return Err(TokenError::RevokedRepeated);
        }
        if observed != self.generation.0.load(Ordering::SeqCst) {
            if let Some(error) = state.last_error {
                return Err(error);
            }
            if let Some(token) = &state.token
                && self.clock.now() < token.0.expires_at
            {
                return Ok(Acquisition::Shared(token.clone()));
            }
        }
        if let Some(token) = &state.token
            && self.clock.now() < token.0.refresh_at
            && rejected.is_none_or(|old| !Arc::ptr_eq(&old.0, &token.0))
        {
            return Ok(Acquisition::Shared(token.clone()));
        }
        state.token = None;
        let mut attempt = Attempt {
            state: &mut state,
            bus: &self.bus,
            generation: &self.generation,
            completion: AttemptCompletion::Pending,
        };
        let result = self.issue().await;
        attempt.complete(&result);
        result.map(Acquisition::Issued)
    }
    fn authentication_error(
        &self,
        state: &mut State,
        failure: AuthenticationFailure,
    ) -> TokenError {
        match failure {
            AuthenticationFailure::Revoked => {
                self.transition(state, BrokerConnection::DuplicateKey);
                TokenError::RevokedRepeated
            }
            AuthenticationFailure::InvalidToken => {
                self.transition(state, BrokerConnection::Disconnected);
                TokenError::Rejected(PortErrorKind::InvalidRequest(
                    application::RequestRejection::InvalidToken,
                ))
            }
        }
    }
    pub async fn token(&self) -> Result<TokenLease, TokenError> {
        self.acquire(None, ConnectionAttempt::Normal)
            .await
            .map(Acquisition::token)
    }
    pub async fn retry_connection(&self) -> Result<TokenLease, TokenError> {
        self.acquire(None, ConnectionAttempt::Recovery)
            .await
            .map(Acquisition::token)
    }
    async fn close_rejected(
        &self,
        rejected: &TokenLease,
        failure: AuthenticationFailure,
    ) -> TokenError {
        let mut state = self.state.lock().await;
        match &state.token {
            Some(current) if !Arc::ptr_eq(&current.0, &rejected.0) => TokenError::Superseded,
            Some(_) => {
                state.token = None;
                let error = self.authentication_error(&mut state, failure);
                state.last_error = Some(error);
                error
            }
            None => state.last_error.unwrap_or(TokenError::Renewal),
        }
    }
    pub async fn get(&self, request: &ReadRequest) -> Result<RawResponse, TokenError> {
        let mut token = self.token().await?;
        let mut budget = ReissueBudget::Available;
        loop {
            let response = self
                .client
                .get(request, token.secret())
                .await
                .map_err(TokenError::Http)?;
            let Some(failure) = authentication_failure(&response) else {
                return Ok(response);
            };
            if budget == ReissueBudget::Spent {
                return Err(self.close_rejected(&token, failure).await);
            }
            match self
                .acquire(Some(&token), ConnectionAttempt::Normal)
                .await?
            {
                Acquisition::Shared(shared) => token = shared,
                Acquisition::Issued(issued) => {
                    token = issued;
                    budget = ReissueBudget::Spent;
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
enum AuthenticationFailure {
    Revoked,
    InvalidToken,
}
fn authentication_failure(response: &RawResponse) -> Option<AuthenticationFailure> {
    if response.status() != 401 {
        return None;
    }
    let kind = response
        .read_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
        .port_error()
        .kind();
    if kind == PortErrorKind::AccessDenied(application::AccessDenial::TokenRevokedRepeated) {
        Some(AuthenticationFailure::Revoked)
    } else if kind == PortErrorKind::InvalidRequest(application::RequestRejection::InvalidToken) {
        Some(AuthenticationFailure::InvalidToken)
    } else {
        None
    }
}

impl<Store, Time, Renewal> std::fmt::Debug for TokenManager<Store, Time, Renewal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenManager(<redacted>)")
    }
}
