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
    fn reconcile_orders(&self) -> impl Future<Output = Result<(), TokenError>> + Send;
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
    bus: EventBus,
    generation: Generation,
    state: Mutex<State>,
}
#[derive(serde::Deserialize)]
struct TokenResponse<'a> {
    access_token: &'a str,
    token_type: &'a str,
    expires_in: u64,
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
    let parsed: TokenResponse<'_> = serde_json::from_slice(response.body().as_exposed_secret())
        .map_err(|_error| TokenError::InvalidResponse)?;
    if parsed.token_type != "Bearer"
        || parsed.access_token.is_empty()
        || parsed.expires_in < 2
        || !parsed
            .access_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(&byte))
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
        secret: SecretValue::new(parsed.access_token.as_bytes().to_vec()),
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
        bus: EventBus,
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
        if state.connection != connection {
            state.connection = connection;
            let report = self.bus.publish(DomainEvent::BrokerConnectionChanged);
            state.delivery = Some(report);
        }
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
        self.renewal.reconcile_orders().await?;
        if self.clock.now() >= token.0.expires_at {
            return Err(TokenError::InvalidResponse);
        }
        Ok(token)
    }
    async fn acquire(&self, rejected: Option<&TokenLease>) -> Result<TokenLease, TokenError> {
        let observed = self.generation.0.load(Ordering::SeqCst);
        let mut state = self.state.lock().await;
        if state.connection == BrokerConnection::DuplicateKey {
            return Err(TokenError::RevokedRepeated);
        }
        if observed != self.generation.0.load(Ordering::SeqCst)
            && let Some(error) = state.last_error
        {
            return Err(error);
        }
        if let Some(token) = &state.token
            && self.clock.now() < token.0.refresh_at
            && rejected.is_none_or(|old| !Arc::ptr_eq(&old.0, &token.0))
        {
            return Ok(token.clone());
        }
        state.token = None;
        let result = self.issue().await;
        state.last_error = result.as_ref().err().copied();
        self.generation.0.fetch_add(1, Ordering::SeqCst);
        match &result {
            Ok(token) => {
                state.token = Some(token.clone());
                self.transition(&mut state, BrokerConnection::Connected);
            }
            Err(_error) => self.transition(&mut state, BrokerConnection::Disconnected),
        }
        result
    }
    pub async fn token(&self) -> Result<TokenLease, TokenError> {
        self.acquire(None).await
    }
    pub async fn get(&self, request: &ReadRequest) -> Result<RawResponse, TokenError> {
        let token = self.token().await?;
        let response = self
            .client
            .get(request, token.secret())
            .await
            .map_err(TokenError::Http)?;
        if !revoked(&response) {
            return Ok(response);
        }
        let refreshed = self.acquire(Some(&token)).await?;
        let retried = self
            .client
            .get(request, refreshed.secret())
            .await
            .map_err(TokenError::Http)?;
        if revoked(&retried) {
            let mut state = self.state.lock().await;
            state.token = None;
            self.transition(&mut state, BrokerConnection::DuplicateKey);
            return Err(TokenError::RevokedRepeated);
        }
        Ok(retried)
    }
}
fn revoked(response: &RawResponse) -> bool {
    response.status() == 401
        && response
            .read_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
            .port_error()
            .kind()
            == PortErrorKind::AccessDenied(application::AccessDenial::TokenRevokedRepeated)
}

impl<Store, Time, Renewal> std::fmt::Debug for TokenManager<Store, Time, Renewal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenManager(<redacted>)")
    }
}
