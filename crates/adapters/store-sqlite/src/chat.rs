mod codec;

use crate::estimates::codec::{Decoder, Encoder, invalid, put_timestamp, take_timestamp};
use crate::{Database, sql::chat as sql};
use application::{
    ChatRepository, ChatRunLedger, ChatRunLedgerEntry, ChatSequence, ChatSessionId,
    ChatSessionRecord, ChatSessionSummary, RecordOutcome, StoredChatContent, StoredChatEntry,
    StoredUserMessage, SystemPortError, ToolCallLedger, ToolCallOutcome, ToolCallRecord,
    UserMessageId, WriteCompleted,
};
use domain::Timestamp;
use rusqlite::{Connection, OptionalExtension, Transaction};

#[derive(Debug)]
pub struct ChatStore<'database> {
    database: &'database Database,
}

impl<'database> ChatStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

fn encode_time(at: Timestamp) -> String {
    let mut out = Encoder::new();
    put_timestamp(&mut out, at);
    out.finish()
}

fn decode_time(raw: &str) -> rusqlite::Result<Timestamp> {
    let mut input = Decoder::new(raw);
    let at = take_timestamp(&mut input)?;
    input.finish()?;
    Ok(at)
}

fn count_of(raw: i64) -> rusqlite::Result<u64> {
    u64::try_from(raw).map_err(invalid)
}

fn summary(
    session: &ChatSessionId,
    created: &str,
    updated: &str,
    count: i64,
) -> rusqlite::Result<ChatSessionSummary> {
    Ok(ChatSessionSummary::new(
        session.clone(),
        decode_time(created)?,
        decode_time(updated)?,
        ChatSequence::new(count_of(count)?),
    ))
}

fn load(
    connection: &Connection,
    session: &ChatSessionId,
) -> rusqlite::Result<Option<ChatSessionRecord>> {
    let stored: Option<(String, String, i64)> = connection
        .query_row(sql::SESSION, [session.as_str()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    let Some((created, updated, count)) = stored else {
        return Ok(None);
    };
    let mut statement = connection.prepare(sql::ENTRIES)?;
    let rows = statement
        .query_map([session.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut entries = Vec::new();
    let mut expected: i64 = 1;
    for (position, payload) in rows {
        if position != expected {
            return Err(invalid(()));
        }
        entries.push(codec::decode_entry(&payload)?);
        expected = expected.checked_add(1).ok_or_else(|| invalid(()))?;
    }
    if u64::try_from(entries.len()).map_err(invalid)? != count_of(count)? {
        return Err(invalid(()));
    }
    Ok(Some(ChatSessionRecord::new(
        summary(session, &created, &updated, count)?,
        entries,
    )))
}

fn ensure(
    transaction: &Transaction<'_>,
    session: &ChatSessionId,
    now: Timestamp,
) -> rusqlite::Result<ChatSessionRecord> {
    if let Some(record) = load(transaction, session)? {
        return Ok(record);
    }
    match transaction.execute(sql::INSERT_SESSION, [session.as_str(), &encode_time(now)])? {
        1 => load(transaction, session)?.ok_or_else(|| invalid(())),
        _ => Err(invalid(())),
    }
}

fn append(
    transaction: &Transaction<'_>,
    session: &ChatSessionId,
    entry: &StoredChatEntry,
) -> rusqlite::Result<WriteCompleted> {
    let count: i64 = transaction
        .query_row(sql::SESSION, [session.as_str()], |row| row.get(2))
        .optional()?
        .ok_or_else(|| invalid(()))?;
    let position = count.checked_add(1).ok_or_else(|| invalid(()))?;
    let payload = codec::encode_entry(entry)?;
    let inserted = transaction.execute(
        sql::INSERT_ENTRY,
        rusqlite::params![
            session.as_str(),
            position,
            codec::user_message_id(entry),
            payload
        ],
    )?;
    let touched = transaction.execute(
        sql::TOUCH_SESSION,
        rusqlite::params![session.as_str(), encode_time(entry.at()), position, count],
    )?;
    match (inserted, touched) {
        (1, 1) => Ok(WriteCompleted),
        _ => Err(invalid(())),
    }
}

fn list(connection: &Connection) -> rusqlite::Result<Vec<ChatSessionSummary>> {
    let mut statement = connection.prepare(sql::SESSIONS)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.iter()
        .map(|(id, created, updated, count)| {
            let session = ChatSessionId::try_new(id).map_err(invalid)?;
            summary(&session, created, updated, *count)
        })
        .collect()
}

fn user_message(
    connection: &Connection,
    session: &ChatSessionId,
    id: &UserMessageId,
) -> rusqlite::Result<Option<StoredUserMessage>> {
    let found: Option<String> = connection
        .query_row(sql::USER_MESSAGE, [session.as_str(), id.as_str()], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(payload) = found else {
        return Ok(None);
    };
    match codec::decode_entry(&payload)?.content() {
        StoredChatContent::User(message) if message.id() == id => Ok(Some(message.clone())),
        StoredChatContent::User(_) | StoredChatContent::Assistant(_) => Err(invalid(())),
    }
}

fn record_run(
    transaction: &Transaction<'_>,
    entry: &ChatRunLedgerEntry,
) -> rusqlite::Result<RecordOutcome> {
    let payload = codec::encode_run(entry)?;
    let existing: Option<i64> = transaction
        .query_row(sql::FIND_RUN, [&payload], |row| row.get(0))
        .optional()?;
    if existing.is_some() {
        return Ok(RecordOutcome::AlreadyRecorded);
    }
    match transaction.execute(sql::INSERT_RUN, [entry.run_id().as_str(), &payload])? {
        1 => Ok(RecordOutcome::Recorded),
        _ => Err(invalid(())),
    }
}

const fn outcome_tag(outcome: ToolCallOutcome) -> &'static str {
    match outcome {
        ToolCallOutcome::Succeeded => "succeeded",
        ToolCallOutcome::ToolError => "tool_error",
        ToolCallOutcome::Rejected => "rejected",
    }
}

struct ToolCallRow {
    run_id: String,
    request_id: String,
    tool_name: String,
    arguments: String,
    result: String,
    outcome: &'static str,
    started: String,
    finished: String,
    protocol: String,
}

impl ToolCallRow {
    fn of(record: &ToolCallRecord) -> Self {
        Self {
            run_id: record.run_id().as_str().to_owned(),
            request_id: record.request_id().as_str().to_owned(),
            tool_name: record.tool_name().as_str().to_owned(),
            arguments: record.arguments_json().as_str().to_owned(),
            result: record.result_json().as_str().to_owned(),
            outcome: outcome_tag(record.outcome()),
            started: encode_time(record.started_at()),
            finished: encode_time(record.finished_at()),
            protocol: record.protocol_version().as_str().to_owned(),
        }
    }

    fn insert(&self, transaction: &Transaction<'_>) -> rusqlite::Result<RecordOutcome> {
        match transaction.execute(
            sql::INSERT_TOOL_CALL,
            rusqlite::params![
                self.run_id,
                self.request_id,
                self.tool_name,
                self.arguments,
                self.result,
                self.outcome,
                self.started,
                self.finished,
                self.protocol
            ],
        )? {
            1 => Ok(RecordOutcome::Recorded),
            _ => Err(invalid(())),
        }
    }
}

impl ChatRepository for ChatStore<'_> {
    async fn ensure_session(
        &self,
        session: &ChatSessionId,
        now: Timestamp,
    ) -> Result<ChatSessionRecord, SystemPortError> {
        let owned_session = session.clone();
        self.database
            .write(move |transaction| ensure(transaction, &owned_session, now))
            .await
    }

    async fn load_session(
        &self,
        session: &ChatSessionId,
    ) -> Result<Option<ChatSessionRecord>, SystemPortError> {
        let owned_session = session.clone();
        self.database
            .read(move |connection| load(connection, &owned_session))
            .await
    }

    async fn list_sessions(&self) -> Result<Vec<ChatSessionSummary>, SystemPortError> {
        self.database.read(list).await
    }

    async fn append_entry(
        &self,
        session: &ChatSessionId,
        entry: &StoredChatEntry,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_session = session.clone();
        let owned_entry = entry.clone();
        self.database
            .write(move |transaction| append(transaction, &owned_session, &owned_entry))
            .await
    }

    async fn load_user_message(
        &self,
        session: &ChatSessionId,
        message_id: &UserMessageId,
    ) -> Result<Option<StoredUserMessage>, SystemPortError> {
        let owned_session = session.clone();
        let id = message_id.clone();
        self.database
            .read(move |connection| user_message(connection, &owned_session, &id))
            .await
    }
}

impl ChatRunLedger for ChatStore<'_> {
    async fn record_chat_run(
        &self,
        entry: &ChatRunLedgerEntry,
    ) -> Result<RecordOutcome, SystemPortError> {
        let owned_entry = entry.clone();
        self.database
            .write(move |transaction| record_run(transaction, &owned_entry))
            .await
    }
}

impl ToolCallLedger for ChatStore<'_> {
    async fn record_call(&self, record: &ToolCallRecord) -> Result<RecordOutcome, SystemPortError> {
        let row = ToolCallRow::of(record);
        self.database
            .write(move |transaction| row.insert(transaction))
            .await
    }
}
