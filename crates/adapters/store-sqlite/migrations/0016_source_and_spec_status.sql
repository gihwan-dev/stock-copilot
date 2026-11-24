CREATE TABLE source_statuses (
    source TEXT PRIMARY KEY CHECK (source IN ('kind_disclosure', 'dart_disclosure_api', 'dart_disclosure_rss', 'dart_fundamentals', 'kind_corp_list', 'hankyung', 'maekyung', 'yonhap', 'edaily', 'google_news', 'web_search', 'yahoo_finance', 'sec_edgar', 'bank_of_korea', 'federal_reserve', 'us_treasury', 'fred')),
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER source_statuses_no_delete BEFORE DELETE ON source_statuses BEGIN SELECT RAISE(ABORT, 'source statuses are replaced, not deleted'); END;
CREATE TABLE spec_status_records (
    id INTEGER PRIMARY KEY,
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER spec_status_records_no_update BEFORE UPDATE ON spec_status_records BEGIN SELECT RAISE(ABORT, 'spec status records are append only'); END;
CREATE TRIGGER spec_status_records_no_delete BEFORE DELETE ON spec_status_records BEGIN SELECT RAISE(ABORT, 'spec status records are append only'); END;
CREATE TABLE news_data_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER news_data_settings_no_delete BEFORE DELETE ON news_data_settings BEGIN SELECT RAISE(ABORT, 'news data settings are replaced, not deleted'); END;
