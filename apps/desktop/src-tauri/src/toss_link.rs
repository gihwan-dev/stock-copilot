use std::sync::Arc;

use application::{
    AccessDenial, Clock, SecretKey, SecretStore, SecretValue, SystemPortError, WriteCompleted,
};
use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use tokio::sync::Mutex;

use crate::ipc::state::BoxFuture;

const MAX_KEY_BYTES: usize = 512;
const RETRY_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossConnectionState {
    NeedsKeys,
    Connected,
    IpNotAllowed,
    InvalidKey,
    Forbidden,
    AccountNotFound,
    KeyInUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossLinkFailure {
    Unavailable,
    Store(SystemPortError),
}

pub type TossLinkResult = Result<TossConnectionState, TossLinkFailure>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFailure {
    MissingKeys,
    Denied(AccessDenial),
    RejectedKey,
    KeyInUse,
    Store(SystemPortError),
    Unavailable,
}

pub type ProbeResult = Result<(), ProbeFailure>;

pub trait TossProbe: Send + Sync {
    fn check(&self) -> BoxFuture<'_, ProbeResult>;
    fn reconnect(&self) -> BoxFuture<'_, ProbeResult>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableProbe;

impl TossProbe for UnavailableProbe {
    fn check(&self) -> BoxFuture<'_, ProbeResult> {
        Box::pin(std::future::ready(Err(ProbeFailure::Unavailable)))
    }

    fn reconnect(&self) -> BoxFuture<'_, ProbeResult> {
        self.check()
    }
}

pub trait ConnectionListener: Send + Sync {
    fn connected(&self) -> BoxFuture<'_, ()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoListener;

impl ConnectionListener for NoListener {
    fn connected(&self) -> BoxFuture<'_, ()> {
        Box::pin(std::future::ready(()))
    }
}

pub trait TossLinkBackend: Send + Sync {
    fn check(&self) -> BoxFuture<'_, TossLinkResult>;
    fn submit(&self, keys: TossKeys) -> BoxFuture<'_, TossLinkResult>;
}

pub struct TossKeys {
    client_id: SecretValue,
    secret: SecretValue,
}

impl std::fmt::Debug for TossKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossKeys(<redacted>)")
    }
}

impl TossKeys {
    #[must_use]
    pub const fn new(client_id: SecretValue, secret: SecretValue) -> Self {
        Self { client_id, secret }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyShape {
    Usable,
    Unusable,
}

fn trimmed(value: &SecretValue) -> (SecretValue, KeyShape) {
    let bytes = value.as_exposed_secret();
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index.saturating_add(1));
    let kept: &[u8] = bytes.get(start..end).map_or(&[], |slice| slice);
    let shape = if kept.is_empty()
        || kept.len() > MAX_KEY_BYTES
        || !kept.iter().all(u8::is_ascii_graphic)
    {
        KeyShape::Unusable
    } else {
        KeyShape::Usable
    };
    (SecretValue::new(kept.to_vec()), shape)
}

const fn denial_state(denial: AccessDenial) -> TossConnectionState {
    match denial {
        AccessDenial::IpNotAllowed => TossConnectionState::IpNotAllowed,
        AccessDenial::InvalidCredentials | AccessDenial::LoginUserNotFound => {
            TossConnectionState::InvalidKey
        }
        AccessDenial::Forbidden => TossConnectionState::Forbidden,
        AccessDenial::AccountNotFound => TossConnectionState::AccountNotFound,
        AccessDenial::TokenRevokedRepeated => TossConnectionState::KeyInUse,
    }
}

pub const fn connection_state(result: ProbeResult) -> TossLinkResult {
    match result {
        Ok(()) => Ok(TossConnectionState::Connected),
        Err(ProbeFailure::MissingKeys) => Ok(TossConnectionState::NeedsKeys),
        Err(ProbeFailure::Denied(denial)) => Ok(denial_state(denial)),
        Err(ProbeFailure::RejectedKey) => Ok(TossConnectionState::InvalidKey),
        Err(ProbeFailure::KeyInUse) => Ok(TossConnectionState::KeyInUse),
        Err(ProbeFailure::Store(error)) => Err(TossLinkFailure::Store(error)),
        Err(ProbeFailure::Unavailable) => Err(TossLinkFailure::Unavailable),
    }
}

struct Recent {
    at: Timestamp,
    result: TossLinkResult,
}

pub struct TossLink<S> {
    secrets: Arc<S>,
    probe: Arc<dyn TossProbe>,
    listener: Arc<dyn ConnectionListener>,
    clock: Arc<dyn Clock>,
    recent: Mutex<Option<Recent>>,
}

impl<S> std::fmt::Debug for TossLink<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossLink(<redacted>)")
    }
}

fn retry_window() -> Option<TimeSpan> {
    TimeSpan::try_new(SpanSeconds::new(RETRY_SECONDS), Nanosecond::new(0)).ok()
}

impl<S: SecretStore + 'static> TossLink<S> {
    #[must_use]
    pub fn new(
        secrets: Arc<S>,
        probe: Arc<dyn TossProbe>,
        listener: Arc<dyn ConnectionListener>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            secrets,
            probe,
            listener,
            clock,
            recent: Mutex::new(None),
        }
    }

    async fn has_keys(&self) -> Result<KeyPresence, TossLinkFailure> {
        for key in [SecretKey::TossClientId, SecretKey::TossClientSecret] {
            let loaded = self
                .secrets
                .load(key)
                .await
                .map_err(TossLinkFailure::Store)?;
            if loaded.is_none() {
                return Ok(KeyPresence::Missing);
            }
        }
        Ok(KeyPresence::Present)
    }

    fn still_recent(&self, recent: &Recent) -> bool {
        let now = self.clock.now();
        retry_window()
            .and_then(|window| recent.at.checked_add(window).ok())
            .is_some_and(|until| now < until)
    }

    async fn settle(&self, result: TossLinkResult) -> TossLinkResult {
        *self.recent.lock().await = Some(Recent {
            at: self.clock.now(),
            result,
        });
        if result == Ok(TossConnectionState::Connected) {
            self.listener.connected().await;
        }
        result
    }

    async fn run_check(&self) -> TossLinkResult {
        if self.has_keys().await? == KeyPresence::Missing {
            return Ok(TossConnectionState::NeedsKeys);
        }
        {
            let recent = self.recent.lock().await;
            if let Some(previous) = recent.as_ref()
                && previous.result != Ok(TossConnectionState::Connected)
                && self.still_recent(previous)
            {
                return previous.result;
            }
        }
        let result = connection_state(self.probe.check().await);
        self.settle(result).await
    }

    async fn store(&self, key: SecretKey, value: SecretValue) -> Result<(), TossLinkFailure> {
        self.secrets
            .store(key, value)
            .await
            .map(|_written: WriteCompleted| ())
            .map_err(TossLinkFailure::Store)
    }

    async fn run_submit(&self, keys: TossKeys) -> TossLinkResult {
        let ((client_id, id_shape), (secret, secret_shape)) =
            (trimmed(&keys.client_id), trimmed(&keys.secret));
        if id_shape == KeyShape::Unusable || secret_shape == KeyShape::Unusable {
            return Ok(TossConnectionState::InvalidKey);
        }
        self.store(SecretKey::TossClientId, client_id).await?;
        self.store(SecretKey::TossClientSecret, secret).await?;
        let result = connection_state(self.probe.reconnect().await);
        self.settle(result).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyPresence {
    Present,
    Missing,
}

impl<S: SecretStore + 'static> TossLinkBackend for TossLink<S> {
    fn check(&self) -> BoxFuture<'_, TossLinkResult> {
        Box::pin(self.run_check())
    }

    fn submit(&self, keys: TossKeys) -> BoxFuture<'_, TossLinkResult> {
        Box::pin(self.run_submit(keys))
    }
}

#[derive(Clone)]
pub struct TossLinkState(Arc<dyn TossLinkBackend>);

impl std::fmt::Debug for TossLinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossLinkState(<redacted>)")
    }
}

impl TossLinkState {
    #[must_use]
    pub fn new(backend: Arc<dyn TossLinkBackend>) -> Self {
        Self(backend)
    }

    #[must_use]
    pub fn backend(&self) -> &dyn TossLinkBackend {
        self.0.as_ref()
    }
}

pub struct SharedSecrets<S>(pub Arc<S>);

impl<S> std::fmt::Debug for SharedSecrets<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedSecrets(<redacted>)")
    }
}

impl<S: SecretStore> SecretStore for SharedSecrets<S> {
    fn load(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
        self.0.load(key)
    }

    fn store(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        self.0.store(key, value)
    }

    fn remove(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        self.0.remove(key)
    }
}

#[derive(Debug)]
pub struct SharedClock<C>(pub Arc<C>);

impl<C> Clone for SharedClock<C> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<C: Clock> Clock for SharedClock<C> {
    fn now(&self) -> Timestamp {
        self.0.now()
    }
}

#[cfg(test)]
mod tests;
