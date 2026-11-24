CREATE TABLE watchlist_entries (
    position INTEGER PRIMARY KEY CHECK (position BETWEEN 0 AND 199),
    instrument TEXT NOT NULL UNIQUE,
    added_seconds INTEGER NOT NULL,
    added_nanos INTEGER NOT NULL CHECK (added_nanos BETWEEN 0 AND 999999999),
    added_offset INTEGER NOT NULL CHECK (added_offset BETWEEN -93599 AND 93599)
) STRICT;
CREATE TABLE signal_skips (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    reason TEXT CHECK (reason IS NULL OR (length(reason) BETWEEN 1 AND 200 AND instr(reason, char(10)) = 0 AND instr(reason, char(13)) = 0)),
    skipped_seconds INTEGER NOT NULL,
    skipped_nanos INTEGER NOT NULL CHECK (skipped_nanos BETWEEN 0 AND 999999999),
    skipped_offset INTEGER NOT NULL CHECK (skipped_offset BETWEEN -93599 AND 93599)
) STRICT;
CREATE TRIGGER signal_skips_no_update BEFORE UPDATE ON signal_skips BEGIN SELECT RAISE(ABORT, 'signal skips are append only'); END;
CREATE TRIGGER signal_skips_no_delete BEFORE DELETE ON signal_skips BEGIN SELECT RAISE(ABORT, 'signal skips are append only'); END;
