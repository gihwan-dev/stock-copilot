use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{
    AccessDenial, PortErrorKind, SecretKey, SecretStore, SecretValue, SystemFailure, SystemPort,
    SystemPortError,
};
use broker_toss::{TokenError, TossEndpoint};
use domain::{Nanosecond, SpanSeconds, TimeSpan};
use testkit::ScriptedHttpServer;
use testkit_ports::SteppedClock;

use super::{
    ConnectionListener, TossConnectionState, TossKeys, TossLink, TossLinkBackend, TossLinkFailure,
    TossProbe, connection_state, live_toss_read,
};
use crate::ipc::state::BoxFuture;

const ID_SENTINEL: &str = "client-id-sentinel-7f3a";
const SECRET_SENTINEL: &str = "client-secret-sentinel-91bc";

#[derive(Debug, Default)]
struct MemorySecretStore {
    values: std::sync::RwLock<Vec<(SecretKey, Vec<u8>)>>,
    writes: AtomicUsize,
    failing: std::sync::RwLock<Option<SystemPortError>>,
}

impl MemorySecretStore {
    fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn fail_writes_with(&self, error: SystemPortError) {
        *self.failing.write().expect("failing") = Some(error);
    }
}

impl SecretStore for MemorySecretStore {
    fn load(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
        std::future::ready(Ok(self
            .values
            .write()
            .expect("values")
            .iter()
            .find(|(stored, _)| *stored == key)
            .map(|(_, value)| SecretValue::new(value.clone()))))
    }

    fn store(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        self.writes.fetch_add(1, Ordering::SeqCst);
        let failing = *self.failing.read().expect("failing");
        if let Some(error) = failing {
            return std::future::ready(Err(error));
        }
        let mut values = self.values.write().expect("values");
        values.retain(|(stored, _)| *stored != key);
        values.push((key, value.as_exposed_secret().to_vec()));
        std::future::ready(Ok(application::WriteCompleted))
    }

    fn remove(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        self.values
            .write()
            .expect("values")
            .retain(|(stored, _)| *stored != key);
        std::future::ready(Ok(application::WriteCompleted))
    }
}

#[derive(Default)]
struct ScriptedProbe {
    replies: std::sync::RwLock<Vec<Result<(), TokenError>>>,
    checks: AtomicUsize,
    reconnects: AtomicUsize,
}

impl ScriptedProbe {
    fn with(replies: Vec<Result<(), TokenError>>) -> Arc<Self> {
        Arc::new(Self {
            replies: std::sync::RwLock::new(replies),
            ..Self::default()
        })
    }

    fn next(&self) -> Result<(), TokenError> {
        self.replies
            .write()
            .map_or(Err(TokenError::Renewal), |mut replies| {
                replies.reverse();
                let reply = replies.pop().unwrap_or(Err(TokenError::Renewal));
                replies.reverse();
                reply
            })
    }
}

impl TossProbe for ScriptedProbe {
    fn check(&self) -> BoxFuture<'_, Result<(), TokenError>> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        let reply = self.next();
        Box::pin(std::future::ready(reply))
    }

    fn reconnect(&self) -> BoxFuture<'_, Result<(), TokenError>> {
        self.reconnects.fetch_add(1, Ordering::SeqCst);
        let reply = self.next();
        Box::pin(std::future::ready(reply))
    }
}

#[derive(Default)]
struct CountingListener(AtomicUsize);

impl ConnectionListener for CountingListener {
    fn connected(&self) -> BoxFuture<'_, ()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(()))
    }
}

fn clock() -> Arc<SteppedClock> {
    Arc::new(SteppedClock::new(
        broker_toss::TossTimestamp::try_from("2026-11-25T00:00:00Z")
            .expect("clock")
            .into_domain(),
    ))
}

fn keys(id: &str, secret: &str) -> TossKeys {
    TossKeys::new(
        SecretValue::new(id.as_bytes().to_vec()),
        SecretValue::new(secret.as_bytes().to_vec()),
    )
}

struct Fixture {
    secrets: Arc<MemorySecretStore>,
    probe: Arc<ScriptedProbe>,
    listener: Arc<CountingListener>,
    clock: Arc<SteppedClock>,
    link: TossLink<MemorySecretStore>,
}

fn fixture(replies: Vec<Result<(), TokenError>>) -> Fixture {
    let secrets = Arc::new(MemorySecretStore::default());
    let probe = ScriptedProbe::with(replies);
    let listener = Arc::new(CountingListener::default());
    let clock = clock();
    let link = TossLink::new(
        Arc::clone(&secrets),
        Arc::<ScriptedProbe>::clone(&probe),
        Arc::<CountingListener>::clone(&listener),
        Arc::<SteppedClock>::clone(&clock),
    );
    Fixture {
        secrets,
        probe,
        listener,
        clock,
        link,
    }
}

async fn stored(store: &MemorySecretStore, key: SecretKey) -> Option<Vec<u8>> {
    store
        .load(key)
        .await
        .expect("load")
        .map(|value| value.as_exposed_secret().to_vec())
}

fn minutes(count: u64) -> TimeSpan {
    TimeSpan::try_new(
        SpanSeconds::new(count.checked_mul(60).expect("minutes")),
        Nanosecond::new(0),
    )
    .expect("span")
}

#[test]
fn check_without_stored_keys_needs_keys_and_never_calls_toss() {
    let fx = fixture(Vec::new());
    let result = tauri::async_runtime::block_on(fx.link.check());
    assert_eq!(result, Ok(TossConnectionState::NeedsKeys));
    assert_eq!(fx.probe.checks.load(Ordering::SeqCst), 0);
    assert_eq!(fx.probe.reconnects.load(Ordering::SeqCst), 0);
}

#[test]
fn submit_stores_trimmed_keys_then_issues_a_token_and_notifies_connection() {
    let fx = fixture(vec![Ok(())]);
    let result = tauri::async_runtime::block_on(fx.link.submit(keys(
        &format!("  {ID_SENTINEL}\n"),
        &format!("\t{SECRET_SENTINEL} "),
    )));
    assert_eq!(result, Ok(TossConnectionState::Connected));
    assert_eq!(fx.probe.reconnects.load(Ordering::SeqCst), 1);
    assert_eq!(fx.listener.0.load(Ordering::SeqCst), 1);
    tauri::async_runtime::block_on(async {
        assert_eq!(
            stored(&fx.secrets, SecretKey::TossClientId).await,
            Some(ID_SENTINEL.as_bytes().to_vec())
        );
        assert_eq!(
            stored(&fx.secrets, SecretKey::TossClientSecret).await,
            Some(SECRET_SENTINEL.as_bytes().to_vec())
        );
    });
}

#[test]
fn submit_rejects_blank_or_control_character_keys_without_storing_or_calling_toss() {
    for (id, secret) in [
        ("   ", SECRET_SENTINEL),
        (ID_SENTINEL, ""),
        ("id\u{7}bell", SECRET_SENTINEL),
        ("두글자", SECRET_SENTINEL),
    ] {
        let fx = fixture(vec![Ok(())]);
        let result = tauri::async_runtime::block_on(fx.link.submit(keys(id, secret)));
        assert_eq!(result, Ok(TossConnectionState::InvalidKey), "{id:?}");
        assert_eq!(fx.probe.reconnects.load(Ordering::SeqCst), 0);
        assert_eq!(fx.secrets.writes(), 0, "{id:?}");
    }
}

#[test]
fn submit_reports_the_screen_state_for_each_token_refusal() {
    for (reply, expected) in [
        (
            Err(TokenError::Rejected(PortErrorKind::AccessDenied(
                AccessDenial::IpNotAllowed,
            ))),
            TossConnectionState::IpNotAllowed,
        ),
        (
            Err(TokenError::Rejected(PortErrorKind::AccessDenied(
                AccessDenial::InvalidCredentials,
            ))),
            TossConnectionState::InvalidKey,
        ),
        (
            Err(TokenError::Rejected(PortErrorKind::AccessDenied(
                AccessDenial::Forbidden,
            ))),
            TossConnectionState::Forbidden,
        ),
        (
            Err(TokenError::Rejected(PortErrorKind::AccessDenied(
                AccessDenial::AccountNotFound,
            ))),
            TossConnectionState::AccountNotFound,
        ),
        (
            Err(TokenError::RevokedRepeated),
            TossConnectionState::KeyInUse,
        ),
    ] {
        let fx = fixture(vec![reply]);
        let result =
            tauri::async_runtime::block_on(fx.link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)));
        assert_eq!(result, Ok(expected));
        assert_eq!(fx.listener.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn transport_failure_is_a_retryable_failure_not_a_connection_state() {
    assert_eq!(
        connection_state(Err(TokenError::Http(broker_toss::TossHttpError::Transport))),
        Err(TossLinkFailure::Unavailable)
    );
}

#[test]
fn store_failure_is_reported_and_toss_is_not_called() {
    let fx = fixture(vec![Ok(())]);
    let failure = SystemPortError::new(
        SystemPort::SecretStore,
        SystemFailure::Unavailable(application::RetryDelay::Unknown),
    );
    fx.secrets.fail_writes_with(failure);
    let result = tauri::async_runtime::block_on(fx.link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)));
    assert_eq!(result, Err(TossLinkFailure::Store(failure)));
    assert_eq!(fx.probe.reconnects.load(Ordering::SeqCst), 0);
}

#[test]
fn failed_checks_retry_toss_at_most_once_a_minute_and_new_keys_retry_at_once() {
    let ip = Err(TokenError::Rejected(PortErrorKind::AccessDenied(
        AccessDenial::IpNotAllowed,
    )));
    let fx = fixture(vec![ip, ip, Ok(()), Ok(())]);
    tauri::async_runtime::block_on(async {
        assert_eq!(
            fx.link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)).await,
            Ok(TossConnectionState::IpNotAllowed)
        );
        assert_eq!(fx.link.check().await, Ok(TossConnectionState::IpNotAllowed));
        assert_eq!(fx.probe.checks.load(Ordering::SeqCst), 0);
        fx.clock.advance_by(minutes(1)).expect("advance");
        assert_eq!(fx.link.check().await, Ok(TossConnectionState::IpNotAllowed));
        assert_eq!(fx.probe.checks.load(Ordering::SeqCst), 1);
        fx.clock.advance_by(minutes(1)).expect("advance");
        assert_eq!(fx.link.check().await, Ok(TossConnectionState::Connected));
        assert_eq!(fx.probe.checks.load(Ordering::SeqCst), 2);
        assert_eq!(
            fx.link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)).await,
            Ok(TossConnectionState::Connected)
        );
        assert_eq!(fx.probe.reconnects.load(Ordering::SeqCst), 2);
    });
}

fn http(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn token_ok() -> Vec<u8> {
    http(
        "200 OK",
        r#"{"access_token":"fake-access-token","token_type":"Bearer","expires_in":3600}"#,
    )
}

fn access_denied() -> Vec<u8> {
    http("403 Forbidden", r#"{"error":"access_denied"}"#)
}

fn live_link(
    server: &ScriptedHttpServer,
    secrets: &Arc<MemorySecretStore>,
) -> TossLink<MemorySecretStore> {
    let system = Arc::new(platform_macos::SystemClock::new().expect("clock"));
    let reader = live_toss_read(
        &TossEndpoint::local(server.port()).expect("endpoint"),
        Arc::clone(secrets),
        Arc::clone(&system),
        Arc::new(application::EventBus::default()),
    )
    .expect("reader");
    TossLink::new(
        Arc::clone(secrets),
        Arc::new(reader),
        Arc::new(super::NoListener),
        clock(),
    )
}

#[test]
fn pasted_keys_reach_the_fake_toss_token_endpoint_and_ip_refusal_then_success_is_reported() {
    let server = ScriptedHttpServer::start(vec![access_denied(), token_ok()]).expect("server");
    let secrets = Arc::new(MemorySecretStore::default());
    let link = live_link(&server, &secrets);
    tauri::async_runtime::block_on(async {
        assert_eq!(
            link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)).await,
            Ok(TossConnectionState::IpNotAllowed)
        );
        assert_eq!(
            link.submit(keys(ID_SENTINEL, SECRET_SENTINEL)).await,
            Ok(TossConnectionState::Connected)
        );
        assert_eq!(link.check().await, Ok(TossConnectionState::Connected));
    });
    let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
    assert_eq!(requests.matches("grant_type=client_credentials").count(), 2);
    assert!(requests.contains(ID_SENTINEL));
    assert!(requests.contains(SECRET_SENTINEL));
}

#[test]
fn key_values_never_appear_in_debug_output_of_link_types() {
    let pasted = keys(ID_SENTINEL, SECRET_SENTINEL);
    let fx = fixture(vec![Ok(())]);
    let shown = format!("{pasted:?} {:?}", fx.link);
    assert!(!shown.contains(ID_SENTINEL));
    assert!(!shown.contains(SECRET_SENTINEL));
}
