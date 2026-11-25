CREATE TABLE information_times (
    item_id INTEGER PRIMARY KEY REFERENCES information_items(id),
    published_payload TEXT,
    collected_payload TEXT NOT NULL
) STRICT;
