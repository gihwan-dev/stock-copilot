CREATE TABLE briefings (
    id_key TEXT PRIMARY KEY,
    seq INTEGER NOT NULL UNIQUE CHECK (seq >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER briefings_no_delete BEFORE DELETE ON briefings BEGIN SELECT RAISE(ABORT, 'briefings are replaced, not deleted'); END;
