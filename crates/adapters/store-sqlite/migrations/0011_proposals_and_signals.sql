CREATE TABLE signal_masters (
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
