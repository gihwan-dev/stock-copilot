CREATE TABLE thesis_history_times (
    instrument TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (instrument, sequence),
    FOREIGN KEY (instrument, sequence) REFERENCES thesis_history(instrument, sequence)
) STRICT;
