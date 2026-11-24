CREATE TABLE chat_sessions (
    session_id TEXT PRIMARY KEY,
    created_payload TEXT NOT NULL,
    updated_payload TEXT NOT NULL,
    entry_count INTEGER NOT NULL CHECK (entry_count >= 0)
) STRICT;
CREATE TRIGGER chat_sessions_no_delete BEFORE DELETE ON chat_sessions BEGIN SELECT RAISE(ABORT, 'chat sessions are kept'); END;
CREATE TRIGGER chat_sessions_fixed_identity BEFORE UPDATE OF session_id, created_payload ON chat_sessions BEGIN SELECT RAISE(ABORT, 'chat session identity is fixed'); END;
CREATE TABLE chat_entries (
    session_id TEXT NOT NULL REFERENCES chat_sessions(session_id),
    position INTEGER NOT NULL CHECK (position >= 1),
    user_message_id TEXT,
    entry_payload TEXT NOT NULL,
    PRIMARY KEY (session_id, position)
) STRICT;
CREATE INDEX chat_entries_user_message ON chat_entries(session_id, user_message_id, position);
CREATE TRIGGER chat_entries_no_update BEFORE UPDATE ON chat_entries BEGIN SELECT RAISE(ABORT, 'chat entries are append only'); END;
CREATE TRIGGER chat_entries_no_delete BEFORE DELETE ON chat_entries BEGIN SELECT RAISE(ABORT, 'chat entries are append only'); END;
CREATE TRIGGER chat_entries_contiguous BEFORE INSERT ON chat_entries WHEN NEW.position IS NOT (SELECT coalesce(max(position), 0) + 1 FROM chat_entries WHERE session_id = NEW.session_id) BEGIN SELECT RAISE(ABORT, 'chat entry position must be contiguous'); END;
CREATE TABLE chat_run_ledger (
    id INTEGER PRIMARY KEY,
    run_id TEXT NOT NULL,
    entry_payload TEXT NOT NULL UNIQUE
) STRICT;
CREATE INDEX chat_run_ledger_run ON chat_run_ledger(run_id);
CREATE TRIGGER chat_run_ledger_no_update BEFORE UPDATE ON chat_run_ledger BEGIN SELECT RAISE(ABORT, 'chat run ledger is append only'); END;
CREATE TRIGGER chat_run_ledger_no_delete BEFORE DELETE ON chat_run_ledger BEGIN SELECT RAISE(ABORT, 'chat run ledger is append only'); END;
CREATE TABLE tool_call_ledger (
    id INTEGER PRIMARY KEY,
    run_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'tool_error', 'rejected')),
    started_payload TEXT NOT NULL,
    finished_payload TEXT NOT NULL,
    protocol_version TEXT NOT NULL
) STRICT;
CREATE INDEX tool_call_ledger_run ON tool_call_ledger(run_id, id);
CREATE TRIGGER tool_call_ledger_no_update BEFORE UPDATE ON tool_call_ledger BEGIN SELECT RAISE(ABORT, 'tool call ledger is append only'); END;
CREATE TRIGGER tool_call_ledger_no_delete BEFORE DELETE ON tool_call_ledger BEGIN SELECT RAISE(ABORT, 'tool call ledger is append only'); END;
