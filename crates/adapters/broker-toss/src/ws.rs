use crate::http::WsUpgradeOutcome;
use crate::order_read::row::{self, ParsedOrder};
use crate::ws_frame::{
    WsFrameError, WsFrameReader, WsWireFrame, generate_ws_key, write_client_close,
    write_client_pong, write_client_text, ws_accept_key,
};
use crate::ws_protocol::{
    WsErrorCode, WsOrderEventKind, WsOrderNotice, WsProtocolError, WsRejectedTopic, WsServerEvent,
    WsSubscriptionSet, WsTopic, decode_server_text,
};
use crate::{
    QueryKey, ReadRequest, ReadRoute, RequestWait, TokenError, TokenRenewal, TossHttpClient,
    TossHttpError, TossWsConnector,
};
use application::{Clock, EventBus, SecretValue};
use domain::{
    AccountId, BrokerOrderId, BrokerOrderPrice, BrokerOrderSize, BrokerOrderStatus, DomainEvent,
    InstrumentId, KnownMarket, MasterInstrumentFacts, OrderOpenState, Quantity, Symbol,
};
use serde_json::{Value, value::RawValue};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::sync::{Mutex, mpsc, watch};
use tokio::task::JoinHandle;

const DECLARATION_RATE_LIMIT: usize = 5;
const DECLARATION_WINDOW_NANOS: i128 = 1_000_000_000;
const MAX_RECONCILE_CLOSED_PAGES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsConfig {
    pub keepalive_interval: Duration,
    pub idle_timeout: Duration,
    pub max_connections_per_account: usize,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub reader_queue_capacity: usize,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            keepalive_interval: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(180),
            max_connections_per_account: 2,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            reader_queue_capacity: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsDisconnectCause {
    AuthRevoked,
    ServerShutdown,
    ServerClosed(Option<u16>),
    AccountConnectionLimitEvicted,
    TransportError,
    ClientClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsClientError {
    NotConnected,
    Unauthorized,
    Forbidden,
    ServiceUnavailable,
    UnexpectedUpgradeStatus(u16),
    Transport(TossHttpError),
    Frame,
    Protocol(WsProtocolError),
    ReconcileFailed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WsReconcileReport {
    pub accounts_checked: Vec<AccountId>,
    pub published_events: Vec<DomainEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsDispatchedEvent {
    SubscriptionsAck {
        id: Option<String>,
        subscribed: Vec<String>,
        rejected: Vec<WsRejectedTopic>,
        pruned: Vec<WsTopic>,
    },
    Pong,
    QuoteUpdated {
        instrument_id: Option<InstrumentId>,
        event: WsServerEvent,
    },
    OrderReconciled {
        notice: WsOrderNotice,
        report: WsReconcileReport,
    },
    RateLimited {
        id: Option<String>,
        message: Option<String>,
    },
    ProtocolErrorFrame {
        id: Option<String>,
        code: WsErrorCode,
        message: Option<String>,
    },
    IgnoredTopic {
        topic: String,
    },
    Disconnected(WsDisconnectCause),
}

struct SlotEntry {
    connection_id: u64,
    evict_tx: watch::Sender<bool>,
}

struct SlotState {
    by_account: BTreeMap<AccountId, VecDeque<SlotEntry>>,
    evictions: usize,
}

pub struct WsAccountSlotPool {
    max_per_account: usize,
    next_id: AtomicU64,
    state: Mutex<SlotState>,
}

impl std::fmt::Debug for WsAccountSlotPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WsAccountSlotPool")
    }
}

impl WsAccountSlotPool {
    #[must_use]
    pub fn new(max_per_account: usize) -> Self {
        Self {
            max_per_account: max_per_account.max(1),
            next_id: AtomicU64::new(1),
            state: Mutex::new(SlotState {
                by_account: BTreeMap::new(),
                evictions: 0,
            }),
        }
    }

    fn allocate_connection_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn active_count(&self, account: &AccountId) -> usize {
        let guard = self.state.lock().await;
        guard.by_account.get(account).map_or(0, VecDeque::len)
    }

    pub async fn eviction_count(&self) -> usize {
        self.state.lock().await.evictions
    }

    fn evict_excess(
        queue: &mut VecDeque<SlotEntry>,
        max_per_account: usize,
        evicted_ids: &mut Vec<u64>,
    ) {
        while queue.len() >= max_per_account {
            let Some(oldest) = queue.pop_front() else {
                break;
            };
            let _send_result = oldest.evict_tx.send(true);
            evicted_ids.push(oldest.connection_id);
        }
    }

    async fn register(
        &self,
        connection_id: u64,
        accounts: &[AccountId],
        evict_tx: &watch::Sender<bool>,
    ) -> Vec<u64> {
        let mut guard = self.state.lock().await;
        let mut evicted_ids = Vec::new();
        for account in accounts {
            let queue = guard.by_account.entry(account.clone()).or_default();
            if queue
                .iter()
                .any(|entry| entry.connection_id == connection_id)
            {
                continue;
            }
            Self::evict_excess(queue, self.max_per_account, &mut evicted_ids);
            queue.push_back(SlotEntry {
                connection_id,
                evict_tx: evict_tx.clone(),
            });
        }
        for evicted_id in &evicted_ids {
            guard.evictions = guard.evictions.saturating_add(1);
            for queue in guard.by_account.values_mut() {
                queue.retain(|entry| entry.connection_id != *evicted_id);
            }
        }
        evicted_ids
    }

    async fn release(&self, connection_id: u64) {
        let mut guard = self.state.lock().await;
        for queue in guard.by_account.values_mut() {
            queue.retain(|entry| entry.connection_id != connection_id);
        }
    }
}

enum InboundSignal {
    Text(String),
    Closed(Option<u16>),
    Evicted,
    FrameError,
}

struct ActiveSession {
    connection_id: u64,
    writer: Arc<Mutex<WriteHalf<reqwest::Upgraded>>>,
    shutdown_tx: watch::Sender<bool>,
    evict_tx: watch::Sender<bool>,
    reader_task: JoinHandle<()>,
    keepalive_task: JoinHandle<()>,
}

#[derive(Default)]
struct SubscriptionState {
    desired: WsSubscriptionSet,
    recent_declarations: VecDeque<i128>,
    declaration_seq: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct OrderSnapshot {
    status: BrokerOrderStatus,
    filled_quantity: Quantity,
    size: BrokerOrderSize,
    price: BrokerOrderPrice,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum CacheStatus {
    #[default]
    Empty,
    Initialized,
}

#[derive(Default)]
struct AccountOrderCache {
    status: CacheStatus,
    by_id: BTreeMap<BrokerOrderId, OrderSnapshot>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OrderChangeKind {
    Filled,
    Cancelled,
    Modified,
}

pub struct TossWsDeps {
    pub connector: TossWsConnector,
    pub http: TossHttpClient,
    pub clock: Arc<dyn Clock>,
    pub wait: Arc<dyn RequestWait>,
    pub master: Arc<MasterInstrumentFacts>,
    pub bus: Arc<EventBus>,
    pub slot_pool: Arc<WsAccountSlotPool>,
}

impl std::fmt::Debug for TossWsDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossWsDeps(<redacted>)")
    }
}

pub struct TossWsClient {
    connector: TossWsConnector,
    http: TossHttpClient,
    clock: Arc<dyn Clock>,
    wait: Arc<dyn RequestWait>,
    master: Arc<MasterInstrumentFacts>,
    bus: Arc<EventBus>,
    slot_pool: Arc<WsAccountSlotPool>,
    config: WsConfig,
    session: Mutex<Option<ActiveSession>>,
    inbound: Mutex<Option<mpsc::Receiver<InboundSignal>>>,
    subscriptions: Mutex<SubscriptionState>,
    orders: Mutex<BTreeMap<AccountId, AccountOrderCache>>,
    sequence: AtomicU64,
}

impl std::fmt::Debug for TossWsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossWsClient(<redacted>)")
    }
}

#[derive(serde::Deserialize)]
struct OrderEnvelope {
    result: OrderPage,
    error: Option<Value>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderPage {
    orders: Vec<Box<RawValue>>,
    has_next: bool,
    next_cursor: Value,
}

fn next_mask(sequence: &AtomicU64) -> [u8; 4] {
    let ticket = sequence.fetch_add(1, Ordering::SeqCst);
    let mixed = ticket.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
    let [byte0, byte1, byte2, byte3, ..] = mixed.to_be_bytes();
    [byte0, byte1, byte2, byte3]
}

fn mask_from_ticket(ticket: u64) -> [u8; 4] {
    let [_, _, _, _, byte4, byte5, byte6, byte7] = ticket.to_be_bytes();
    [byte4, byte5, byte6, byte7]
}

fn now_nanos(clock: &dyn Clock) -> i128 {
    let instant = clock.now();
    i128::from(*instant.utc_seconds().as_i64())
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(*instant.nanosecond().as_u32()))
}

#[must_use]
pub fn compute_backoff(initial: Duration, maximum: Duration, attempt: u32) -> Duration {
    let shift = attempt.min(16);
    let factor = 1_u32.checked_shl(shift).unwrap_or(u32::MAX);
    initial.saturating_mul(factor).min(maximum)
}

fn lookup_instrument_id(
    master: &MasterInstrumentFacts,
    market: KnownMarket,
    symbol: &Symbol,
) -> Option<InstrumentId> {
    master.instruments().iter().find_map(|row| {
        row.quote_instrument()
            .ok()
            .filter(|instrument| *instrument.market() == market && instrument.symbol() == symbol)
            .map(|instrument| instrument.instrument_id().clone())
    })
}

fn is_filled_status(status: &BrokerOrderStatus) -> bool {
    match status {
        BrokerOrderStatus::Filled | BrokerOrderStatus::PartialFilled => true,
        BrokerOrderStatus::Pending
        | BrokerOrderStatus::PendingCancel
        | BrokerOrderStatus::PendingReplace
        | BrokerOrderStatus::Canceled
        | BrokerOrderStatus::Rejected
        | BrokerOrderStatus::CancelRejected
        | BrokerOrderStatus::ReplaceRejected
        | BrokerOrderStatus::Replaced
        | BrokerOrderStatus::Unknown(_) => false,
    }
}

fn is_canceled_status(status: &BrokerOrderStatus) -> bool {
    match status {
        BrokerOrderStatus::Canceled => true,
        BrokerOrderStatus::Pending
        | BrokerOrderStatus::PendingCancel
        | BrokerOrderStatus::PendingReplace
        | BrokerOrderStatus::PartialFilled
        | BrokerOrderStatus::Filled
        | BrokerOrderStatus::Rejected
        | BrokerOrderStatus::CancelRejected
        | BrokerOrderStatus::ReplaceRejected
        | BrokerOrderStatus::Replaced
        | BrokerOrderStatus::Unknown(_) => false,
    }
}

fn is_replaced_status(status: &BrokerOrderStatus) -> bool {
    match status {
        BrokerOrderStatus::Replaced => true,
        BrokerOrderStatus::Pending
        | BrokerOrderStatus::PendingCancel
        | BrokerOrderStatus::PendingReplace
        | BrokerOrderStatus::PartialFilled
        | BrokerOrderStatus::Filled
        | BrokerOrderStatus::Canceled
        | BrokerOrderStatus::Rejected
        | BrokerOrderStatus::CancelRejected
        | BrokerOrderStatus::ReplaceRejected
        | BrokerOrderStatus::Unknown(_) => false,
    }
}

fn record_existing_change(
    previous: &OrderSnapshot,
    current: &OrderSnapshot,
    changes: &mut BTreeSet<OrderChangeKind>,
) {
    if current.filled_quantity.as_decimal() > previous.filled_quantity.as_decimal()
        || (!is_filled_status(&previous.status) && is_filled_status(&current.status))
    {
        changes.insert(OrderChangeKind::Filled);
    }
    if !is_canceled_status(&previous.status) && is_canceled_status(&current.status) {
        changes.insert(OrderChangeKind::Cancelled);
    }
    if (!is_replaced_status(&previous.status) && is_replaced_status(&current.status))
        || current.price != previous.price
        || current.size != previous.size
    {
        changes.insert(OrderChangeKind::Modified);
    }
}

fn record_new_order_change(
    current: &OrderSnapshot,
    hint: Option<&WsOrderEventKind>,
    changes: &mut BTreeSet<OrderChangeKind>,
) {
    if !current.filled_quantity.as_decimal().is_zero() || is_filled_status(&current.status) {
        changes.insert(OrderChangeKind::Filled);
    }
    if is_canceled_status(&current.status) {
        changes.insert(OrderChangeKind::Cancelled);
    }
    if is_replaced_status(&current.status)
        || (hint == Some(&WsOrderEventKind::Replaced)
            && current.status.open_state() == OrderOpenState::Possible)
    {
        changes.insert(OrderChangeKind::Modified);
    }
}

fn diff_orders(
    cache: &AccountOrderCache,
    fetched: &BTreeMap<BrokerOrderId, OrderSnapshot>,
    hint: Option<&WsOrderEventKind>,
) -> BTreeSet<OrderChangeKind> {
    let mut changes = BTreeSet::new();
    let observe_new = cache.status == CacheStatus::Initialized || hint.is_some();
    for (order_id, current) in fetched {
        if let Some(previous) = cache.by_id.get(order_id) {
            record_existing_change(previous, current, &mut changes);
        } else if observe_new {
            record_new_order_change(current, hint, &mut changes);
        }
    }
    if cache.status == CacheStatus::Initialized && hint == Some(&WsOrderEventKind::Replaced) {
        let disappeared_open = cache.by_id.iter().any(|(order_id, previous)| {
            previous.status.open_state() == OrderOpenState::Possible
                && !fetched.contains_key(order_id)
        });
        if disappeared_open {
            changes.insert(OrderChangeKind::Modified);
        }
    }
    changes
}

#[derive(PartialEq, Eq)]
enum ReaderAction {
    Continue,
    Stop,
}

async fn handle_reader_frame(
    next: Result<WsWireFrame, WsFrameError>,
    writer: &Mutex<WriteHalf<reqwest::Upgraded>>,
    inbound_tx: &mpsc::Sender<InboundSignal>,
    mask_ticket: &mut u64,
) -> ReaderAction {
    match next {
        Ok(WsWireFrame::Text(text)) => {
            if inbound_tx.send(InboundSignal::Text(text)).await.is_err() {
                ReaderAction::Stop
            } else {
                ReaderAction::Continue
            }
        }
        Ok(WsWireFrame::Ping(payload)) => {
            *mask_ticket = mask_ticket.wrapping_add(1);
            let mut locked = writer.lock().await;
            let _pong =
                write_client_pong(&mut *locked, &payload, mask_from_ticket(*mask_ticket)).await;
            ReaderAction::Continue
        }
        Ok(WsWireFrame::Pong(_)) => ReaderAction::Continue,
        Ok(WsWireFrame::Close(code)) => {
            let _sent = inbound_tx.send(InboundSignal::Closed(code)).await;
            ReaderAction::Stop
        }
        Err(WsFrameError::ClosedWithoutCloseFrame) => {
            let _sent = inbound_tx.send(InboundSignal::Closed(None)).await;
            ReaderAction::Stop
        }
        Err(_) => {
            let _sent = inbound_tx.send(InboundSignal::FrameError).await;
            ReaderAction::Stop
        }
    }
}

enum RaceOutcome<First, Second> {
    First(First),
    Second(Second),
}

async fn race_two<F1: Future, F2: Future>(
    first: F1,
    second: F2,
) -> RaceOutcome<F1::Output, F2::Output> {
    let mut first_pin = std::pin::pin!(first);
    let mut second_pin = std::pin::pin!(second);
    std::future::poll_fn(|context| {
        if let std::task::Poll::Ready(value) = first_pin.as_mut().poll(context) {
            return std::task::Poll::Ready(RaceOutcome::First(value));
        }
        if let std::task::Poll::Ready(value) = second_pin.as_mut().poll(context) {
            return std::task::Poll::Ready(RaceOutcome::Second(value));
        }
        std::task::Poll::Pending
    })
    .await
}

enum StopSignal {
    Shutdown,
    Evicted,
}

async fn wait_for_stop(
    shutdown_rx: &mut watch::Receiver<bool>,
    evict_rx: &mut watch::Receiver<bool>,
) -> StopSignal {
    loop {
        if *shutdown_rx.borrow() {
            return StopSignal::Shutdown;
        }
        if *evict_rx.borrow() {
            return StopSignal::Evicted;
        }
        match race_two(shutdown_rx.changed(), evict_rx.changed()).await {
            RaceOutcome::First(Ok(())) | RaceOutcome::Second(Ok(())) => {}
            RaceOutcome::First(Err(_)) | RaceOutcome::Second(Err(_)) => {
                return StopSignal::Shutdown;
            }
        }
    }
}

async fn run_reader_loop(
    read_half: ReadHalf<reqwest::Upgraded>,
    writer: Arc<Mutex<WriteHalf<reqwest::Upgraded>>>,
    inbound_tx: mpsc::Sender<InboundSignal>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut evict_rx: watch::Receiver<bool>,
) {
    let mut frame_reader = WsFrameReader::new(read_half);
    let mut mask_ticket = 1_u64;
    loop {
        let outcome = race_two(
            wait_for_stop(&mut shutdown_rx, &mut evict_rx),
            frame_reader.next_frame(),
        )
        .await;
        match outcome {
            RaceOutcome::First(StopSignal::Shutdown) => break,
            RaceOutcome::First(StopSignal::Evicted) => {
                let mut locked = writer.lock().await;
                let _close = write_client_close(&mut *locked, 1000, [0x45, 0x56, 0x49, 0x43]).await;
                let _sent = inbound_tx.send(InboundSignal::Evicted).await;
                break;
            }
            RaceOutcome::Second(next) => {
                let action =
                    handle_reader_frame(next, &writer, &inbound_tx, &mut mask_ticket).await;
                if action == ReaderAction::Stop {
                    break;
                }
            }
        }
    }
}

fn spawn_reader_task(
    read_half: ReadHalf<reqwest::Upgraded>,
    writer: Arc<Mutex<WriteHalf<reqwest::Upgraded>>>,
    inbound_tx: mpsc::Sender<InboundSignal>,
    shutdown_rx: watch::Receiver<bool>,
    evict_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(run_reader_loop(
        read_half,
        writer,
        inbound_tx,
        shutdown_rx,
        evict_rx,
    ))
}

async fn send_keepalive_ping(
    writer: &Mutex<WriteHalf<reqwest::Upgraded>>,
    ticket: &mut u64,
) -> Result<(), WsFrameError> {
    *ticket = ticket.wrapping_add(1);
    let mut locked = writer.lock().await;
    write_client_text(&mut *locked, "PING", mask_from_ticket(*ticket)).await
}

async fn run_keepalive_loop(
    writer: Arc<Mutex<WriteHalf<reqwest::Upgraded>>>,
    wait: Arc<dyn RequestWait>,
    interval: Duration,
    mut shutdown_rx: watch::Receiver<bool>,
    mut evict_rx: watch::Receiver<bool>,
) {
    let mut ticket = 100_u64;
    loop {
        let outcome = race_two(
            wait_for_stop(&mut shutdown_rx, &mut evict_rx),
            wait.wait(interval),
        )
        .await;
        let RaceOutcome::Second(()) = outcome else {
            break;
        };
        if *shutdown_rx.borrow() || *evict_rx.borrow() {
            break;
        }
        if send_keepalive_ping(&writer, &mut ticket).await.is_err() {
            break;
        }
    }
}

fn spawn_keepalive_task(
    writer: Arc<Mutex<WriteHalf<reqwest::Upgraded>>>,
    wait: Arc<dyn RequestWait>,
    interval: Duration,
    shutdown_rx: watch::Receiver<bool>,
    evict_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(run_keepalive_loop(
        writer,
        wait,
        interval,
        shutdown_rx,
        evict_rx,
    ))
}

fn account_secret(account: &AccountId) -> SecretValue {
    SecretValue::new(account.as_i64().to_string().into_bytes())
}

fn parse_order_page_rows(
    body: &[u8],
    master: &MasterInstrumentFacts,
) -> Result<(Vec<ParsedOrder>, Option<String>), WsClientError> {
    let envelope: OrderEnvelope =
        serde_json::from_slice(body).map_err(|_error| WsClientError::ReconcileFailed)?;
    if envelope.error.is_some_and(|err_val| !err_val.is_null()) {
        return Err(WsClientError::ReconcileFailed);
    }
    let mut parsed_rows = Vec::new();
    for raw in envelope.result.orders {
        if let Ok(parsed) = row::parse_raw(raw.get(), master) {
            parsed_rows.push(parsed);
        }
    }
    let next_cursor = match (envelope.result.has_next, envelope.result.next_cursor) {
        (true, Value::String(cursor)) if !cursor.is_empty() => Some(cursor),
        _ => None,
    };
    Ok((parsed_rows, next_cursor))
}

fn snapshot_from_parsed(parsed: &ParsedOrder) -> OrderSnapshot {
    OrderSnapshot {
        status: parsed.status.clone(),
        filled_quantity: parsed.filled_quantity,
        size: parsed.size,
        price: parsed.price,
    }
}

fn map_upgrade_outcome(outcome: WsUpgradeOutcome) -> Result<reqwest::Upgraded, WsClientError> {
    match outcome {
        WsUpgradeOutcome::Connected(stream) => Ok(stream),
        WsUpgradeOutcome::Unauthorized => Err(WsClientError::Unauthorized),
        WsUpgradeOutcome::Forbidden => Err(WsClientError::Forbidden),
        WsUpgradeOutcome::ServiceUnavailable => Err(WsClientError::ServiceUnavailable),
        WsUpgradeOutcome::UnexpectedStatus(code) => {
            Err(WsClientError::UnexpectedUpgradeStatus(code))
        }
    }
}

impl TossWsClient {
    #[must_use]
    pub fn new(deps: TossWsDeps, config: WsConfig) -> Self {
        Self {
            connector: deps.connector,
            http: deps.http,
            clock: deps.clock,
            wait: deps.wait,
            master: deps.master,
            bus: deps.bus,
            slot_pool: deps.slot_pool,
            config,
            session: Mutex::new(None),
            inbound: Mutex::new(None),
            subscriptions: Mutex::new(SubscriptionState::default()),
            orders: Mutex::new(BTreeMap::new()),
            sequence: AtomicU64::new(1),
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.session.lock().await.is_some()
    }

    pub async fn desired_subscriptions(&self) -> WsSubscriptionSet {
        self.subscriptions.lock().await.desired.clone()
    }

    pub async fn disconnect(&self) {
        let previous = self.session.lock().await.take();
        let _dropped_rx = self.inbound.lock().await.take();
        if let Some(active) = previous {
            let _signal = active.shutdown_tx.send(true);
            let mask = next_mask(&self.sequence);
            {
                let mut locked_writer = active.writer.lock().await;
                let _close = write_client_close(&mut *locked_writer, 1000, mask).await;
            }
            active.keepalive_task.abort();
            active.reader_task.abort();
            self.slot_pool.release(active.connection_id).await;
        }
    }

    pub async fn connect(&self, token: &SecretValue) -> Result<(), WsClientError> {
        self.disconnect().await;
        let conn_ticket = self.slot_pool.allocate_connection_id();
        let ws_key = generate_ws_key(conn_ticket);
        let expected_accept = ws_accept_key(&ws_key);
        let outcome = self
            .connector
            .open_upgrade(token, &ws_key, &expected_accept)
            .await
            .map_err(WsClientError::Transport)?;
        let upgraded = map_upgrade_outcome(outcome)?;
        let (read_half, write_half) = tokio::io::split(upgraded);
        let writer = Arc::new(Mutex::new(write_half));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (evict_tx, evict_rx) = watch::channel(false);
        let (inbound_tx, inbound_rx) = mpsc::channel(self.config.reader_queue_capacity.max(1));
        let accounts = self
            .subscriptions
            .lock()
            .await
            .desired
            .subscribed_accounts();
        if !accounts.is_empty() {
            let _evicted = self
                .slot_pool
                .register(conn_ticket, &accounts, &evict_tx)
                .await;
        }
        let reader_task = spawn_reader_task(
            read_half,
            Arc::clone(&writer),
            inbound_tx,
            shutdown_rx.clone(),
            evict_rx.clone(),
        );
        let keepalive_task = spawn_keepalive_task(
            Arc::clone(&writer),
            Arc::clone(&self.wait),
            self.config.keepalive_interval,
            shutdown_rx,
            evict_rx,
        );
        *self.inbound.lock().await = Some(inbound_rx);
        *self.session.lock().await = Some(ActiveSession {
            connection_id: conn_ticket,
            writer,
            shutdown_tx,
            evict_tx,
            reader_task,
            keepalive_task,
        });
        Ok(())
    }

    fn try_admit_slot(state: &mut SubscriptionState, now: i128) -> Result<String, Duration> {
        let cutoff = now.saturating_sub(DECLARATION_WINDOW_NANOS);
        while state
            .recent_declarations
            .front()
            .is_some_and(|stamp| *stamp <= cutoff)
        {
            let _removed = state.recent_declarations.pop_front();
        }
        if state.recent_declarations.len() < DECLARATION_RATE_LIMIT {
            state.recent_declarations.push_back(now);
            state.declaration_seq = state.declaration_seq.saturating_add(1);
            return Ok(format!("decl-{}", state.declaration_seq));
        }
        let oldest = state.recent_declarations.front().copied().unwrap_or(now);
        let wake_at = oldest.saturating_add(DECLARATION_WINDOW_NANOS);
        let delta = wake_at.saturating_sub(now).max(1);
        Err(u64::try_from(delta).map_or(Duration::from_secs(1), Duration::from_nanos))
    }

    async fn admit_declaration(&self) -> String {
        loop {
            let outcome = {
                let mut state = self.subscriptions.lock().await;
                Self::try_admit_slot(&mut state, now_nanos(self.clock.as_ref()))
            };
            match outcome {
                Ok(id) => return id,
                Err(wait_needed) => self.wait.wait(wait_needed).await,
            }
        }
    }

    async fn send_declaration_text(
        &self,
        set: &WsSubscriptionSet,
        declaration_id: &str,
    ) -> Result<(), WsClientError> {
        let payload = set.encode_declaration(Some(declaration_id));
        let (writer, connection_id, evict_tx) = {
            let guard = self.session.lock().await;
            let Some(active) = guard.as_ref() else {
                return Err(WsClientError::NotConnected);
            };
            (
                Arc::clone(&active.writer),
                active.connection_id,
                active.evict_tx.clone(),
            )
        };
        let accounts = set.subscribed_accounts();
        if !accounts.is_empty() {
            let _evicted = self
                .slot_pool
                .register(connection_id, &accounts, &evict_tx)
                .await;
        }
        let mask = next_mask(&self.sequence);
        let mut locked = writer.lock().await;
        write_client_text(&mut *locked, &payload, mask)
            .await
            .map_err(|_error| WsClientError::Frame)
    }

    pub async fn declare_subscriptions(&self, set: WsSubscriptionSet) -> Result<(), WsClientError> {
        if set.len() > crate::ws_protocol::MAX_TOPICS_PER_CONNECTION {
            return Err(WsClientError::Protocol(WsProtocolError::TooManyTopics));
        }
        let declaration_id = self.admit_declaration().await;
        self.send_declaration_text(&set, &declaration_id).await?;
        self.subscriptions.lock().await.desired = set;
        Ok(())
    }

    pub async fn redeclare_subscriptions(&self) -> Result<(), WsClientError> {
        let snapshot = self.subscriptions.lock().await.desired.clone();
        if snapshot.is_empty() {
            return Ok(());
        }
        let declaration_id = self.admit_declaration().await;
        self.send_declaration_text(&snapshot, &declaration_id).await
    }

    async fn fetch_closed_pages(
        &self,
        account: &AccountId,
        token: &SecretValue,
        fetched: &mut BTreeMap<BrokerOrderId, OrderSnapshot>,
    ) -> Result<(), WsClientError> {
        let mut cursor: Option<String> = None;
        let mut seen_cursors = BTreeSet::new();
        for _page in 0_usize..MAX_RECONCILE_CLOSED_PAGES {
            let mut closed_query = ReadRequest::new(ReadRoute::Orders)
                .with_account(account_secret(account))
                .with_query(QueryKey::Status, "CLOSED".to_owned())
                .with_query(QueryKey::Limit, "100".to_owned());
            if let Some(next_cursor) = &cursor {
                closed_query = closed_query.with_query(QueryKey::Cursor, next_cursor.clone());
            }
            let closed_resp = self
                .http
                .get(&closed_query, token)
                .await
                .map_err(WsClientError::Transport)?;
            if !(200..300).contains(&closed_resp.status()) {
                return Err(WsClientError::ReconcileFailed);
            }
            let (closed_rows, next_cursor) = parse_order_page_rows(
                closed_resp.body().as_exposed_secret(),
                self.master.as_ref(),
            )?;
            for parsed in closed_rows {
                fetched.insert(parsed.order_id.clone(), snapshot_from_parsed(&parsed));
            }
            match next_cursor {
                Some(next) if seen_cursors.insert(next.clone()) => cursor = Some(next),
                _ => break,
            }
        }
        Ok(())
    }

    async fn fetch_account_orders(
        &self,
        account: &AccountId,
        token: &SecretValue,
    ) -> Result<BTreeMap<BrokerOrderId, OrderSnapshot>, WsClientError> {
        let mut fetched = BTreeMap::new();
        let open_query = ReadRequest::new(ReadRoute::Orders)
            .with_account(account_secret(account))
            .with_query(QueryKey::Status, "OPEN".to_owned());
        let open_resp = self
            .http
            .get(&open_query, token)
            .await
            .map_err(WsClientError::Transport)?;
        if !(200..300).contains(&open_resp.status()) {
            return Err(WsClientError::ReconcileFailed);
        }
        let (open_rows, _ignored_cursor) =
            parse_order_page_rows(open_resp.body().as_exposed_secret(), self.master.as_ref())?;
        for parsed in open_rows {
            fetched.insert(parsed.order_id.clone(), snapshot_from_parsed(&parsed));
        }
        self.fetch_closed_pages(account, token, &mut fetched)
            .await?;
        Ok(fetched)
    }

    async fn reconcile_account_with_token(
        &self,
        account: &AccountId,
        token: &SecretValue,
        hint: Option<&WsOrderEventKind>,
    ) -> Result<Vec<DomainEvent>, WsClientError> {
        let fetched = self.fetch_account_orders(account, token).await?;
        let changes = {
            let mut cache_map = self.orders.lock().await;
            let entry = cache_map.entry(account.clone()).or_default();
            let computed = diff_orders(entry, &fetched, hint);
            entry.by_id = fetched;
            entry.status = CacheStatus::Initialized;
            computed
        };
        let mut published = Vec::new();
        for kind in changes {
            let event = match kind {
                OrderChangeKind::Filled => DomainEvent::OrderFilled(account.clone()),
                OrderChangeKind::Cancelled => DomainEvent::OrderCancelled(account.clone()),
                OrderChangeKind::Modified => DomainEvent::OrderModified(account.clone()),
            };
            let _delivery = self.bus.publish(event.clone());
            published.push(event);
        }
        Ok(published)
    }

    pub async fn reconcile_orders_with_token(
        &self,
        token: &SecretValue,
    ) -> Result<WsReconcileReport, WsClientError> {
        let accounts = self
            .subscriptions
            .lock()
            .await
            .desired
            .subscribed_accounts();
        let mut report = WsReconcileReport::default();
        for account in accounts {
            let events = self
                .reconcile_account_with_token(&account, token, None)
                .await?;
            report.accounts_checked.push(account);
            report.published_events.extend(events);
        }
        Ok(report)
    }

    pub async fn reconnect_with_backoff(
        &self,
        token: &SecretValue,
        attempt: u32,
    ) -> Result<WsReconcileReport, WsClientError> {
        self.disconnect().await;
        let delay = compute_backoff(
            self.config.initial_backoff,
            self.config.max_backoff,
            attempt,
        );
        self.wait.wait(delay).await;
        self.connect(token).await?;
        self.redeclare_subscriptions().await?;
        self.reconcile_orders_with_token(token).await
    }

    fn publish_quote_update(
        &self,
        market: KnownMarket,
        symbol: &Symbol,
        event: WsServerEvent,
    ) -> WsDispatchedEvent {
        let instrument_id = lookup_instrument_id(&self.master, market, symbol);
        if let Some(id) = &instrument_id {
            let _delivery = self.bus.publish(DomainEvent::PriceUpdated(id.clone()));
        }
        WsDispatchedEvent::QuoteUpdated {
            instrument_id,
            event,
        }
    }

    async fn dispatch_error_frame(
        &self,
        id: Option<String>,
        code: WsErrorCode,
        message: Option<String>,
    ) -> WsDispatchedEvent {
        match code {
            WsErrorCode::AuthRevoked => {
                self.disconnect().await;
                WsDispatchedEvent::Disconnected(WsDisconnectCause::AuthRevoked)
            }
            WsErrorCode::ServerShutdown => {
                self.disconnect().await;
                WsDispatchedEvent::Disconnected(WsDisconnectCause::ServerShutdown)
            }
            WsErrorCode::RateLimitExceeded => {
                self.wait.wait(Duration::from_secs(1)).await;
                WsDispatchedEvent::RateLimited { id, message }
            }
            other @ (WsErrorCode::WrongFormat
            | WsErrorCode::NoType
            | WsErrorCode::InvalidType
            | WsErrorCode::NoCodes
            | WsErrorCode::TooManyTopics
            | WsErrorCode::TooMany
            | WsErrorCode::InternalError
            | WsErrorCode::Unknown(_)) => WsDispatchedEvent::ProtocolErrorFrame {
                id,
                code: other,
                message,
            },
        }
    }

    async fn dispatch_server_event(
        &self,
        event: WsServerEvent,
        token: &SecretValue,
    ) -> Result<WsDispatchedEvent, WsClientError> {
        match event {
            WsServerEvent::SubscriptionsAck {
                id,
                subscribed,
                rejected,
            } => {
                let pruned = self
                    .subscriptions
                    .lock()
                    .await
                    .desired
                    .apply_rejected(&rejected);
                Ok(WsDispatchedEvent::SubscriptionsAck {
                    id,
                    subscribed,
                    rejected,
                    pruned,
                })
            }
            WsServerEvent::Pong => Ok(WsDispatchedEvent::Pong),
            WsServerEvent::Trade(tick) => {
                let (market, symbol) = (tick.market, tick.symbol.clone());
                Ok(self.publish_quote_update(market, &symbol, WsServerEvent::Trade(tick)))
            }
            WsServerEvent::Orderbook(tick) => {
                let (market, symbol) = (tick.market, tick.symbol.clone());
                Ok(self.publish_quote_update(market, &symbol, WsServerEvent::Orderbook(tick)))
            }
            WsServerEvent::PersonalOrder(notice) => {
                let events = self
                    .reconcile_account_with_token(
                        &notice.account_id,
                        token,
                        Some(&notice.event_kind),
                    )
                    .await?;
                let report = WsReconcileReport {
                    accounts_checked: vec![notice.account_id.clone()],
                    published_events: events,
                };
                Ok(WsDispatchedEvent::OrderReconciled { notice, report })
            }
            WsServerEvent::Error { id, code, message } => {
                Ok(self.dispatch_error_frame(id, code, message).await)
            }
            WsServerEvent::UnknownTopic { topic } => Ok(WsDispatchedEvent::IgnoredTopic { topic }),
        }
    }

    pub async fn next_event(
        &self,
        token: &SecretValue,
    ) -> Result<WsDispatchedEvent, WsClientError> {
        let signal = {
            let mut guard = self.inbound.lock().await;
            let Some(receiver) = guard.as_mut() else {
                return Err(WsClientError::NotConnected);
            };
            receiver.recv().await
        };
        match signal {
            Some(InboundSignal::Text(text)) => {
                let parsed = decode_server_text(&text).map_err(WsClientError::Protocol)?;
                self.dispatch_server_event(parsed, token).await
            }
            Some(InboundSignal::Closed(code)) => {
                self.disconnect().await;
                Ok(WsDispatchedEvent::Disconnected(
                    WsDisconnectCause::ServerClosed(code),
                ))
            }
            Some(InboundSignal::Evicted) => {
                self.disconnect().await;
                Ok(WsDispatchedEvent::Disconnected(
                    WsDisconnectCause::AccountConnectionLimitEvicted,
                ))
            }
            Some(InboundSignal::FrameError) => {
                self.disconnect().await;
                Ok(WsDispatchedEvent::Disconnected(
                    WsDisconnectCause::TransportError,
                ))
            }
            None => {
                self.disconnect().await;
                Ok(WsDispatchedEvent::Disconnected(
                    WsDisconnectCause::ClientClosed,
                ))
            }
        }
    }
}

impl TokenRenewal for TossWsClient {
    async fn close(&self) -> Result<(), TokenError> {
        self.disconnect().await;
        Ok(())
    }

    async fn reconnect(&self, token: &SecretValue) -> Result<(), TokenError> {
        self.connect(token)
            .await
            .map_err(|_error| TokenError::Renewal)
    }

    async fn resubscribe(&self) -> Result<(), TokenError> {
        self.redeclare_subscriptions()
            .await
            .map_err(|_error| TokenError::Renewal)
    }

    async fn reconcile_orders(&self, token: &SecretValue) -> Result<(), TokenError> {
        self.reconcile_orders_with_token(token)
            .await
            .map(|_report| ())
            .map_err(|_error| TokenError::Renewal)
    }
}

impl TokenRenewal for Arc<TossWsClient> {
    async fn close(&self) -> Result<(), TokenError> {
        self.disconnect().await;
        Ok(())
    }

    async fn reconnect(&self, token: &SecretValue) -> Result<(), TokenError> {
        self.connect(token)
            .await
            .map_err(|_error| TokenError::Renewal)
    }

    async fn resubscribe(&self) -> Result<(), TokenError> {
        self.redeclare_subscriptions()
            .await
            .map_err(|_error| TokenError::Renewal)
    }

    async fn reconcile_orders(&self, token: &SecretValue) -> Result<(), TokenError> {
        self.reconcile_orders_with_token(token)
            .await
            .map(|_report| ())
            .map_err(|_error| TokenError::Renewal)
    }
}
