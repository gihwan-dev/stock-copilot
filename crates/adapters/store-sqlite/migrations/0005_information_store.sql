CREATE TABLE information_items (
    id INTEGER PRIMARY KEY,
    category TEXT NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    UNIQUE (category, source, external_id)
) STRICT;
CREATE TABLE information_links (
    category TEXT NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    PRIMARY KEY (category, source, external_id, instrument)
) STRICT;
CREATE VIRTUAL TABLE information_fts USING fts5(title_grams, body_grams, tokenize='unicode61', content='', contentless_delete=1);
