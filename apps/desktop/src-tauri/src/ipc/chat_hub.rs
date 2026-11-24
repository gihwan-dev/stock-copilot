use application::{
    ChatChunk, ChatChunkSink, ChatRepository, ChatSessionId, ChatSessionRecord, ChatSignalState,
    ChatSinkClosed, ChatStopSignal, ClassifiedError, SendChatMessage, SendChatOutcome,
    SendChatRequest, SendWarmChatMessage, SystemPortError,
};
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::async_runtime::Mutex;

use crate::ports::{PortTypes, Ports};

use super::chat_dto::{
    ActiveChatTurnDto, ActiveTurnStateDto, CancelChatTurnRequest, CancelChatTurnResponse,
    CancelOutcomeDto, ChatChunkEvent, GetChatHistoryRequest, GetChatHistoryResponse,
    ResubscribeChatTurnRequest, ResubscribeChatTurnResponse, SendChatMessageRequest,
    SendChatMessageResponse, chunk_to_dto, into_domain_request, outcome_to_response,
    session_entries_to_dto,
};
use super::error::{IpcError, IpcErrorKind};

pub type ChunkDelivery =
    Arc<dyn Fn(ChatChunkEvent) -> Result<(), ChatSinkClosed> + Send + Sync + 'static>;
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Default)]
pub struct TurnStopSignal {
    state: AtomicU8,
}

impl TurnStopSignal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
        }
    }

    pub fn cancel(&self) {
        self.state.store(1, Ordering::SeqCst);
    }
}

impl ChatStopSignal for TurnStopSignal {
    fn state(&self) -> ChatSignalState {
        match self.state.load(Ordering::SeqCst) {
            0 => ChatSignalState::Active,
            1 => ChatSignalState::Cancelled,
            _ => ChatSignalState::Interrupted,
        }
    }
}

struct TurnStreamState {
    session_id: String,
    turn_id: String,
    state: ActiveTurnStateDto,
    next_sequence: u32,
    chunks: Vec<ChatChunkEvent>,
    subscribers: Vec<ChunkDelivery>,
    stop: Arc<TurnStopSignal>,
}

#[derive(Default)]
pub struct ChatStreamHub {
    turns: Mutex<BTreeMap<String, TurnStreamState>>,
    latest_by_session: Mutex<BTreeMap<String, String>>,
}

impl fmt::Debug for ChatStreamHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatStreamHub(<redacted>)")
    }
}

impl ChatStreamHub {
    pub async fn register_turn(
        &self,
        session_id: String,
        turn_id: String,
        initial_subscriber: ChunkDelivery,
    ) -> Arc<TurnStopSignal> {
        let stop = Arc::new(TurnStopSignal::new());
        let entry = TurnStreamState {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            state: ActiveTurnStateDto::Running,
            next_sequence: 1,
            chunks: Vec::new(),
            subscribers: vec![initial_subscriber],
            stop: Arc::clone(&stop),
        };
        self.turns.lock().await.insert(turn_id.clone(), entry);
        self.latest_by_session
            .lock()
            .await
            .insert(session_id, turn_id);
        stop
    }

    pub async fn publish(&self, turn_id: &str, chunk: &ChatChunk) {
        let is_terminal = is_terminal_chunk(chunk);
        let dto = chunk_to_dto(chunk);
        let mut guard = self.turns.lock().await;
        let Some(turn) = guard.get_mut(turn_id) else {
            return;
        };
        let sequence = turn.next_sequence;
        turn.next_sequence = sequence.saturating_add(1);
        let event = ChatChunkEvent {
            sequence,
            turn_id: turn.turn_id.clone(),
            chunk: dto,
        };
        turn.chunks.push(event.clone());
        if is_terminal {
            turn.state = ActiveTurnStateDto::Finished;
        }
        turn.subscribers
            .retain(|subscriber| subscriber(event.clone()).is_ok());
    }

    pub async fn mark_finished(&self, turn_id: &str) {
        let mut guard = self.turns.lock().await;
        if let Some(turn) = guard.get_mut(turn_id) {
            turn.state = ActiveTurnStateDto::Finished;
        }
    }

    pub async fn active_turn_for_session(&self, session_id: &str) -> Option<ActiveChatTurnDto> {
        let latest = self
            .latest_by_session
            .lock()
            .await
            .get(session_id)
            .cloned()?;
        let guard = self.turns.lock().await;
        let turn = guard.get(&latest)?;
        if turn.session_id != session_id {
            return None;
        }
        let last_sequence = turn.next_sequence.saturating_sub(1);
        Some(ActiveChatTurnDto {
            turn_id: turn.turn_id.clone(),
            last_sequence,
            state: turn.state,
        })
    }

    pub async fn resubscribe(
        &self,
        turn_id: &str,
        after_sequence: u32,
        subscriber: ChunkDelivery,
    ) -> Result<ResubscribeChatTurnResponse, IpcError> {
        let mut guard = self.turns.lock().await;
        let Some(turn) = guard.get_mut(turn_id) else {
            return Err(IpcError {
                kind: IpcErrorKind::NoData,
            });
        };
        let mut replayed: u32 = 0;
        let mut open = true;
        for event in turn
            .chunks
            .iter()
            .filter(|event| event.sequence > after_sequence)
        {
            if subscriber(event.clone()).is_err() {
                open = false;
                break;
            }
            replayed = replayed.saturating_add(1);
        }
        if open && turn.state == ActiveTurnStateDto::Running {
            turn.subscribers.push(subscriber);
        }
        Ok(ResubscribeChatTurnResponse {
            turn_id: turn.turn_id.clone(),
            replayed_count: replayed,
            state: turn.state,
        })
    }

    pub async fn cancel(&self, turn_id: &str) -> CancelChatTurnResponse {
        let guard = self.turns.lock().await;
        let outcome = match guard.get(turn_id) {
            Some(turn) if turn.state == ActiveTurnStateDto::Running => {
                turn.stop.cancel();
                CancelOutcomeDto::Signaled
            }
            Some(_) | None => CancelOutcomeDto::NotRunning,
        };
        CancelChatTurnResponse {
            turn_id: turn_id.to_owned(),
            outcome,
        }
    }
}

const fn is_terminal_chunk(chunk: &ChatChunk) -> bool {
    match chunk {
        ChatChunk::TextDelta(_)
        | ChatChunk::ToolCallStarted { .. }
        | ChatChunk::ToolCallFinished { .. }
        | ChatChunk::Card(_)
        | ChatChunk::Sources(_) => false,
        ChatChunk::Done(_)
        | ChatChunk::Cancelled
        | ChatChunk::Interrupted
        | ChatChunk::RateLimited { .. }
        | ChatChunk::Failed(_) => true,
    }
}

pub struct HubChunkSink {
    turn_id: String,
    hub: Arc<ChatStreamHub>,
}

impl fmt::Debug for HubChunkSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HubChunkSink")
            .field("turn_id", &self.turn_id)
            .finish_non_exhaustive()
    }
}

impl HubChunkSink {
    #[must_use]
    pub const fn new(turn_id: String, hub: Arc<ChatStreamHub>) -> Self {
        Self { turn_id, hub }
    }
}

impl ChatChunkSink for HubChunkSink {
    async fn send(&mut self, chunk: ChatChunk) -> Result<(), ChatSinkClosed> {
        self.hub.publish(&self.turn_id, &chunk).await;
        Ok(())
    }
}

pub trait ChatBackend: Send + Sync {
    fn send_turn(
        &self,
        request: SendChatRequest,
        sink: HubChunkSink,
        stop: Arc<TurnStopSignal>,
    ) -> BoxFuture<'_, SendChatOutcome>;

    fn load_session(
        &self,
        session_id: ChatSessionId,
    ) -> BoxFuture<'_, Result<Option<ChatSessionRecord>, SystemPortError>>;
}

pub struct PortsChatBackend<P: PortTypes> {
    ai_provider: Arc<P::AiProvider>,
    chat_repository: Arc<P::ChatRepository>,
    chat_ledger: Arc<P::ChatRunLedger>,
    chat_queue: Arc<P::ChatQueuePort>,
    warm_chat: Arc<P::WarmChatRuntime>,
    clock: Arc<P::Clock>,
}

impl<P: PortTypes> fmt::Debug for PortsChatBackend<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PortsChatBackend(<redacted>)")
    }
}

impl<P: PortTypes> PortsChatBackend<P> {
    #[must_use]
    pub fn from_ports(ports: &Ports<P>) -> Self {
        Self {
            ai_provider: Arc::clone(&ports.ai_provider),
            chat_repository: Arc::clone(&ports.chat_repository),
            chat_ledger: Arc::clone(&ports.chat_ledger),
            chat_queue: Arc::clone(&ports.chat_queue),
            warm_chat: Arc::clone(&ports.warm_chat),
            clock: Arc::clone(&ports.clock),
        }
    }
}

impl<P: PortTypes> ChatBackend for PortsChatBackend<P> {
    fn send_turn(
        &self,
        request: SendChatRequest,
        mut sink: HubChunkSink,
        stop: Arc<TurnStopSignal>,
    ) -> BoxFuture<'_, SendChatOutcome> {
        Box::pin(async move {
            let sender = SendChatMessage::new(
                self.ai_provider.as_ref(),
                self.chat_repository.as_ref(),
                self.chat_ledger.as_ref(),
                self.chat_queue.as_ref(),
                self.clock.as_ref(),
            );
            let use_case =
                SendWarmChatMessage::with_default_max_age(sender, self.warm_chat.as_ref());
            use_case.execute(request, &mut sink, stop.as_ref()).await
        })
    }

    fn load_session(
        &self,
        session_id: ChatSessionId,
    ) -> BoxFuture<'_, Result<Option<ChatSessionRecord>, SystemPortError>> {
        Box::pin(async move { self.chat_repository.load_session(&session_id).await })
    }
}

pub struct ChatIpcState {
    backend: Arc<dyn ChatBackend>,
    hub: Arc<ChatStreamHub>,
}

impl fmt::Debug for ChatIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatIpcState(<redacted>)")
    }
}

impl ChatIpcState {
    #[must_use]
    pub fn from_ports<P: PortTypes>(ports: &Ports<P>) -> Self {
        Self {
            backend: Arc::new(PortsChatBackend::from_ports(ports)),
            hub: Arc::new(ChatStreamHub::default()),
        }
    }

    #[must_use]
    pub fn hub(&self) -> Arc<ChatStreamHub> {
        Arc::clone(&self.hub)
    }

    pub async fn send_message(
        &self,
        request: SendChatMessageRequest,
        delivery: ChunkDelivery,
    ) -> Result<SendChatMessageResponse, IpcError> {
        let domain_request = into_domain_request(request)?;
        let session_id = domain_request.session_id().as_str().to_owned();
        let turn_id = domain_request.turn_id().as_str().to_owned();
        let stop = self
            .hub
            .register_turn(session_id, turn_id.clone(), delivery)
            .await;
        let sink = HubChunkSink::new(turn_id.clone(), Arc::clone(&self.hub));
        let outcome = self.backend.send_turn(domain_request, sink, stop).await;
        self.hub.mark_finished(&turn_id).await;
        Ok(outcome_to_response(&outcome))
    }

    pub async fn get_history(
        &self,
        request: GetChatHistoryRequest,
    ) -> Result<GetChatHistoryResponse, IpcError> {
        let session_id =
            ChatSessionId::try_new(&request.session_id).map_err(|_error| IpcError {
                kind: IpcErrorKind::Bug,
            })?;
        let record = self
            .backend
            .load_session(session_id)
            .await
            .map_err(|error| IpcError::from(error.class()))?;
        let active_turn = self.hub.active_turn_for_session(&request.session_id).await;
        Ok(GetChatHistoryResponse {
            session_id: request.session_id,
            entries: session_entries_to_dto(record.as_ref()),
            active_turn,
        })
    }

    pub async fn resubscribe(
        &self,
        request: ResubscribeChatTurnRequest,
        delivery: ChunkDelivery,
    ) -> Result<ResubscribeChatTurnResponse, IpcError> {
        self.hub
            .resubscribe(&request.turn_id, request.after_sequence, delivery)
            .await
    }

    pub async fn cancel(
        &self,
        request: CancelChatTurnRequest,
    ) -> Result<CancelChatTurnResponse, IpcError> {
        Ok(self.hub.cancel(&request.turn_id).await)
    }
}
