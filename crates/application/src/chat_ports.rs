use crate::{
    AiRunId, ChatChunk, ChatQueueAcquisition, ChatRunLedgerEntry, ChatSessionId, ChatSessionRecord,
    ChatSessionSummary, ChatSignalState, ChatSinkClosed, RecordOutcome, StoredChatEntry,
    StoredUserMessage, SystemPortError, UserMessageId, WriteCompleted,
};
use domain::Timestamp;
use std::future::Future;

pub trait ChatRepository: Send + Sync {
    fn ensure_session(
        &self,
        session: &ChatSessionId,
        now: Timestamp,
    ) -> impl Future<Output = Result<ChatSessionRecord, SystemPortError>> + Send;

    fn load_session(
        &self,
        session: &ChatSessionId,
    ) -> impl Future<Output = Result<Option<ChatSessionRecord>, SystemPortError>> + Send;

    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatSessionSummary>, SystemPortError>> + Send;

    fn append_entry(
        &self,
        session: &ChatSessionId,
        entry: &StoredChatEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;

    fn load_user_message(
        &self,
        session: &ChatSessionId,
        message_id: &UserMessageId,
    ) -> impl Future<Output = Result<Option<StoredUserMessage>, SystemPortError>> + Send;
}

pub trait ChatRunLedger: Send + Sync {
    fn record_chat_run(
        &self,
        entry: &ChatRunLedgerEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
}

pub trait ChatQueuePort: Send + Sync {
    fn acquire_chat(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ChatQueueAcquisition, SystemPortError>> + Send;

    fn release_chat(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

pub trait ChatChunkSink: Send {
    fn send(&mut self, chunk: ChatChunk)
    -> impl Future<Output = Result<(), ChatSinkClosed>> + Send;
}

pub trait ChatStopSignal: Send + Sync {
    fn state(&self) -> ChatSignalState;
}
