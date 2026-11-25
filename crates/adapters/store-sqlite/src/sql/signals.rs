pub(crate) const CREATE: &str = "CREATE TABLE signal_masters (
    instrument_id TEXT PRIMARY KEY,
    master_payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_sources (
    task_id TEXT PRIMARY KEY,
    sources_payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_selected_account (
    slot INTEGER PRIMARY KEY CHECK (slot = 1),
    account INTEGER NOT NULL
) STRICT;
CREATE TABLE signal_states (
    instrument TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    current_payload TEXT
) STRICT;
CREATE TABLE signal_occurrences (
    id INTEGER PRIMARY KEY,
    occurrence_id TEXT NOT NULL UNIQUE,
    instrument_id TEXT NOT NULL,
    market TEXT NOT NULL,
    symbol TEXT NOT NULL,
    occurrence_payload TEXT NOT NULL
) STRICT;
CREATE INDEX signal_occurrences_instrument ON signal_occurrences(instrument_id, id);
CREATE INDEX signal_occurrences_symbol ON signal_occurrences(market, symbol, id);
CREATE TRIGGER signal_occurrences_no_update BEFORE UPDATE ON signal_occurrences BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TRIGGER signal_occurrences_no_delete BEFORE DELETE ON signal_occurrences BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TRIGGER signal_occurrences_no_replace BEFORE INSERT ON signal_occurrences WHEN EXISTS (SELECT 1 FROM signal_occurrences WHERE occurrence_id = NEW.occurrence_id) BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TABLE signal_discarded_submissions (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    task_id TEXT NOT NULL,
    UNIQUE (instrument, task_id)
) STRICT;
CREATE TRIGGER signal_discarded_submissions_no_update BEFORE UPDATE ON signal_discarded_submissions BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TRIGGER signal_discarded_submissions_no_delete BEFORE DELETE ON signal_discarded_submissions BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TRIGGER signal_discarded_submissions_no_replace BEFORE INSERT ON signal_discarded_submissions WHEN EXISTS (SELECT 1 FROM signal_discarded_submissions WHERE instrument = NEW.instrument AND task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TABLE proposal_drafts (
    id INTEGER PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('standalone', 'guarded')),
    instrument TEXT NOT NULL,
    account INTEGER NOT NULL,
    expires_seconds INTEGER NOT NULL,
    expires_nanos INTEGER NOT NULL CHECK (expires_nanos >= 0 AND expires_nanos < 1000000000),
    expires_offset INTEGER NOT NULL CHECK (expires_offset BETWEEN -86340 AND 86340 AND expires_offset % 60 = 0),
    state TEXT NOT NULL CHECK (state IN ('current', 'superseded')),
    snapshot_payload TEXT,
    evidence_payload TEXT NOT NULL,
    CHECK ((mode = 'standalone' AND snapshot_payload IS NULL) OR (mode = 'guarded' AND snapshot_payload IS NOT NULL))
) STRICT;
CREATE INDEX proposal_drafts_proposal ON proposal_drafts(proposal_id, id DESC);
CREATE INDEX proposal_drafts_instrument_current ON proposal_drafts(instrument, state, id DESC);
CREATE TRIGGER proposal_drafts_no_delete BEFORE DELETE ON proposal_drafts BEGIN SELECT RAISE(ABORT, 'proposal drafts are append only'); END;
CREATE TRIGGER proposal_drafts_supersede_only BEFORE UPDATE ON proposal_drafts WHEN OLD.state != 'current' OR NEW.state != 'superseded' OR NEW.id != OLD.id OR NEW.proposal_id != OLD.proposal_id OR NEW.mode != OLD.mode OR NEW.instrument != OLD.instrument OR NEW.account != OLD.account OR NEW.expires_seconds != OLD.expires_seconds OR NEW.expires_nanos != OLD.expires_nanos OR NEW.expires_offset != OLD.expires_offset OR coalesce(NEW.snapshot_payload, '') != coalesce(OLD.snapshot_payload, '') OR (NEW.snapshot_payload IS NULL) != (OLD.snapshot_payload IS NULL) OR NEW.evidence_payload != OLD.evidence_payload BEGIN SELECT RAISE(ABORT, 'proposal draft can only transition from current to superseded'); END;
";
pub(crate) const UPSERT_MASTER: &str = "INSERT INTO signal_masters(instrument_id, master_payload) VALUES (?1, ?2) ON CONFLICT(instrument_id) DO UPDATE SET master_payload = excluded.master_payload";
pub(crate) const ALL_MASTERS: &str =
    "SELECT master_payload FROM signal_masters ORDER BY instrument_id";
pub(crate) const UPSERT_SOURCES: &str = "INSERT INTO signal_sources(task_id, sources_payload) VALUES (?1, ?2) ON CONFLICT(task_id) DO UPDATE SET sources_payload = excluded.sources_payload";
pub(crate) const SOURCES_BY_TASK: &str =
    "SELECT sources_payload FROM signal_sources WHERE task_id = ?1";
pub(crate) const UPSERT_SELECTED_ACCOUNT: &str = "INSERT INTO signal_selected_account(slot, account) VALUES (1, ?1) ON CONFLICT(slot) DO UPDATE SET account = excluded.account";
pub(crate) const SELECTED_ACCOUNT: &str =
    "SELECT account FROM signal_selected_account WHERE slot = 1";
pub(crate) const STATE_BY_INSTRUMENT: &str =
    "SELECT revision, current_payload FROM signal_states WHERE instrument = ?1";
pub(crate) const UPSERT_STATE: &str = "INSERT INTO signal_states(instrument, revision, current_payload) VALUES (?1, ?2, ?3) ON CONFLICT(instrument) DO UPDATE SET revision = excluded.revision, current_payload = excluded.current_payload";
pub(crate) const LATEST_SUBMISSION_RECORD_BY_INSTRUMENT: &str = "SELECT r.task_id, r.run_id, r.record_payload, t.status, t.task_payload FROM estimate_submission_records r JOIN estimate_tasks t ON t.task_id = r.task_id WHERE r.instrument = ?1 ORDER BY r.id DESC LIMIT 1";
pub(crate) const SUBMISSION_RECORD_BY_TASK_ID: &str = "SELECT r.task_id, r.run_id, r.record_payload, t.status, t.task_payload FROM estimate_submission_records r JOIN estimate_tasks t ON t.task_id = r.task_id WHERE r.task_id = ?1";
pub(crate) const INSERT_OCCURRENCE: &str = "INSERT INTO signal_occurrences(occurrence_id, instrument_id, market, symbol, occurrence_payload) VALUES (?1, ?2, ?3, ?4, ?5)";
pub(crate) const OCCURRENCE_BY_ID: &str =
    "SELECT occurrence_payload FROM signal_occurrences WHERE occurrence_id = ?1";
pub(crate) const OCCURRENCES_FOR_INSTRUMENT: &str = "SELECT occurrence_payload FROM signal_occurrences WHERE instrument_id = ?1 OR (market = ?2 AND symbol = ?3) ORDER BY id";
pub(crate) const ALL_OCCURRENCE_IDS: &str =
    "SELECT occurrence_id FROM signal_occurrences ORDER BY id";
pub(crate) const IS_DISCARDED: &str =
    "SELECT 1 FROM signal_discarded_submissions WHERE instrument = ?1 AND task_id = ?2";
pub(crate) const INSERT_DISCARDED: &str =
    "INSERT INTO signal_discarded_submissions(instrument, task_id) VALUES (?1, ?2)";
pub(crate) const DRAFT_EXISTS_BY_ID: &str =
    "SELECT 1 FROM proposal_drafts WHERE proposal_id = ?1 LIMIT 1";
pub(crate) const LATEST_CURRENT_DRAFT_BY_ID: &str = "SELECT mode, instrument, account, expires_seconds, expires_nanos, expires_offset, snapshot_payload, evidence_payload FROM proposal_drafts WHERE proposal_id = ?1 AND state = 'current' ORDER BY id DESC LIMIT 1";
pub(crate) const CURRENT_GUARDED_DRAFT_BY_INSTRUMENT: &str = "SELECT id, proposal_id, account, expires_seconds, expires_nanos, expires_offset, snapshot_payload, evidence_payload FROM proposal_drafts WHERE instrument = ?1 AND mode = 'guarded' AND state = 'current' ORDER BY id DESC LIMIT 1";
pub(crate) const SUPERSEDE_DRAFT_BY_ROW_ID: &str =
    "UPDATE proposal_drafts SET state = 'superseded' WHERE id = ?1 AND state = 'current'";
pub(crate) const SUPERSEDE_STANDALONE_BY_PROPOSAL_ID: &str = "UPDATE proposal_drafts SET state = 'superseded' WHERE proposal_id = ?1 AND mode = 'standalone' AND state = 'current'";
pub(crate) const SUPERSEDE_GUARDED_BY_INSTRUMENT: &str = "UPDATE proposal_drafts SET state = 'superseded' WHERE instrument = ?1 AND mode = 'guarded' AND state = 'current'";
pub(crate) const INSERT_DRAFT: &str = "INSERT INTO proposal_drafts(proposal_id, mode, instrument, account, expires_seconds, expires_nanos, expires_offset, state, snapshot_payload, evidence_payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'current', ?8, ?9)";
pub(crate) const DRAFT_INSTRUMENT_BY_ID: &str =
    "SELECT instrument FROM proposal_drafts WHERE proposal_id = ?1 ORDER BY id DESC LIMIT 1";
