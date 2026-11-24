CREATE TABLE order_rate_limit (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    hourly_limit INTEGER NOT NULL CHECK (hourly_limit BETWEEN 1 AND 30)
) STRICT;
CREATE TRIGGER order_rate_limit_no_delete BEFORE DELETE ON order_rate_limit BEGIN SELECT RAISE(ABORT, 'order rate limit is replaced, not deleted'); END;
CREATE TABLE order_submissions (
    id INTEGER PRIMARY KEY,
    utc_seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos BETWEEN 0 AND 999999999),
    offset_seconds INTEGER NOT NULL CHECK (offset_seconds BETWEEN -93599 AND 93599)
) STRICT;
CREATE INDEX order_submissions_by_time ON order_submissions(utc_seconds, nanos);
CREATE TRIGGER order_submissions_no_update BEFORE UPDATE ON order_submissions BEGIN SELECT RAISE(ABORT, 'order submissions are append only'); END;
CREATE TRIGGER order_submissions_no_delete BEFORE DELETE ON order_submissions BEGIN SELECT RAISE(ABORT, 'order submissions are append only'); END;
