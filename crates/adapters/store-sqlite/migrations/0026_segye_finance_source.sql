CREATE TABLE source_statuses_next (
    source TEXT PRIMARY KEY CHECK (source IN ('kind_disclosure', 'dart_disclosure_api', 'dart_disclosure_rss', 'dart_fundamentals', 'kind_corp_list', 'hankyung', 'maekyung', 'yonhap', 'edaily', 'segye_finance', 'google_news', 'web_search', 'yahoo_finance', 'sec_edgar', 'bank_of_korea', 'federal_reserve', 'us_treasury', 'fred')),
    payload TEXT NOT NULL
) STRICT;
INSERT INTO source_statuses_next(source, payload) SELECT source, payload FROM source_statuses;
DROP TABLE source_statuses;
ALTER TABLE source_statuses_next RENAME TO source_statuses;
CREATE TRIGGER source_statuses_no_delete BEFORE DELETE ON source_statuses BEGIN SELECT RAISE(ABORT, 'source statuses are replaced, not deleted'); END;
