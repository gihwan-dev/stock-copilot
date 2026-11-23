CREATE TABLE thesis_notes (
    instrument TEXT PRIMARY KEY
) STRICT;
CREATE TABLE thesis_history (
    instrument TEXT NOT NULL REFERENCES thesis_notes(instrument),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    payload TEXT NOT NULL,
    PRIMARY KEY (instrument, sequence)
) STRICT;
CREATE TRIGGER thesis_history_no_update BEFORE UPDATE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
CREATE TRIGGER thesis_history_no_delete BEFORE DELETE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
CREATE TRIGGER thesis_history_no_replace BEFORE INSERT ON thesis_history WHEN EXISTS (SELECT 1 FROM thesis_history WHERE instrument = NEW.instrument AND sequence = NEW.sequence) BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
