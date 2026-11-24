CREATE TABLE wake_triggers (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('disclosure', 'price_move', 'news_burst')),
    instrument TEXT NOT NULL,
    disclosure_source TEXT CHECK (disclosure_source IN ('dart', 'kind', 'sec_edgar')),
    receipt TEXT,
    grade TEXT CHECK (grade IN ('urgent', 'high', 'normal', 'low')),
    action TEXT CHECK (action IN ('wake_and_notify', 'wake', 'briefing', 'collect_only')),
    recorded_day INTEGER NOT NULL CHECK (recorded_day BETWEEN 10101 AND 99991231),
    recorded_seconds INTEGER NOT NULL,
    recorded_nanos INTEGER NOT NULL CHECK (recorded_nanos BETWEEN 0 AND 999999999),
    recorded_offset INTEGER NOT NULL CHECK (recorded_offset BETWEEN -93599 AND 93599),
    CHECK ((kind = 'disclosure') = (disclosure_source IS NOT NULL AND receipt IS NOT NULL AND grade IS NOT NULL AND action IS NOT NULL)),
    CHECK (kind = 'disclosure' OR (disclosure_source IS NULL AND receipt IS NULL AND grade IS NULL AND action IS NULL))
) STRICT;
CREATE UNIQUE INDEX wake_triggers_disclosure_once ON wake_triggers(disclosure_source, receipt, instrument, grade, action) WHERE kind = 'disclosure';
CREATE UNIQUE INDEX wake_triggers_daily_once ON wake_triggers(kind, instrument, recorded_day) WHERE kind != 'disclosure';
CREATE INDEX wake_triggers_by_time ON wake_triggers(recorded_seconds, recorded_nanos);
CREATE TRIGGER wake_triggers_no_update BEFORE UPDATE ON wake_triggers BEGIN SELECT RAISE(ABORT, 'wake triggers are append only'); END;
CREATE TRIGGER wake_triggers_no_delete BEFORE DELETE ON wake_triggers BEGIN SELECT RAISE(ABORT, 'wake triggers are append only'); END;
CREATE TABLE web_collected_items (
    id INTEGER PRIMARY KEY,
    subject_kind TEXT NOT NULL CHECK (subject_kind IN ('symbol', 'instrument')),
    subject TEXT NOT NULL,
    payload TEXT NOT NULL
) STRICT;
CREATE INDEX web_collected_items_subject ON web_collected_items(subject_kind, subject, id);
CREATE TRIGGER web_collected_items_no_update BEFORE UPDATE ON web_collected_items BEGIN SELECT RAISE(ABORT, 'web collected items are append only'); END;
CREATE TRIGGER web_collected_items_no_delete BEFORE DELETE ON web_collected_items BEGIN SELECT RAISE(ABORT, 'web collected items are append only'); END;
