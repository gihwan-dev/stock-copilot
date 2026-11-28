CREATE INDEX ai_job_records_finished ON ai_job_records(finished_seconds, finished_nanos);
CREATE UNIQUE INDEX audit_log_entry ON audit_log(at_payload, kind, payload);
CREATE INDEX chat_entries_user_message ON chat_entries(session_id, user_message_id, position);
CREATE INDEX chat_run_ledger_run ON chat_run_ledger(run_id);
CREATE INDEX daily_valuation_snapshots_lookup ON daily_valuation_snapshots(account, as_of_seconds, as_of_nanos, id);
CREATE INDEX estimate_calibrations_lookup ON estimate_calibrations(key_hash, position DESC);
CREATE INDEX estimate_resolution_unavailable_task ON estimate_resolution_unavailable(task_id, id);
CREATE INDEX estimate_submission_records_instrument ON estimate_submission_records(instrument, id);
CREATE INDEX estimate_submissions_instrument ON estimate_submissions(instrument, id);
CREATE INDEX instrument_mapping_events_instrument_seq ON instrument_mapping_events(instrument_id, seq);
CREATE UNIQUE INDEX instrument_mappings_active_retired_id_key ON instrument_mappings(instrument_id) WHERE status IN ('active', 'retired');
CREATE UNIQUE INDEX instrument_mappings_active_retired_isin_key ON instrument_mappings(upper(isin)) WHERE status IN ('active', 'retired');
CREATE UNIQUE INDEX instrument_mappings_active_retired_symbol_key ON instrument_mappings(market, upper(symbol)) WHERE status IN ('active', 'retired');
CREATE INDEX instrument_mappings_isin_key ON instrument_mappings(upper(isin));
CREATE UNIQUE INDEX instrument_mappings_superseded_event_key ON instrument_mappings(superseded_event_seq) WHERE superseded_event_seq IS NOT NULL;
CREATE INDEX instrument_mappings_symbol_key ON instrument_mappings(market, upper(symbol));
CREATE INDEX job_execution_records_by_job ON job_execution_records(job, seq);
CREATE INDEX order_submissions_by_time ON order_submissions(utc_seconds, nanos);
CREATE INDEX proposal_drafts_instrument_current ON proposal_drafts(instrument, state, id DESC);
CREATE INDEX proposal_drafts_proposal ON proposal_drafts(proposal_id, id DESC);
CREATE INDEX proposal_validation_reports_lookup ON proposal_validation_reports(proposal_id, id);
CREATE INDEX signal_occurrences_instrument ON signal_occurrences(instrument_id, id);
CREATE INDEX signal_occurrences_symbol ON signal_occurrences(market, symbol, id);
CREATE INDEX tool_call_ledger_run ON tool_call_ledger(run_id, id);
CREATE INDEX transaction_entries_account_id ON transaction_entries(account, id);
CREATE INDEX wake_triggers_by_time ON wake_triggers(recorded_seconds, recorded_nanos);
CREATE UNIQUE INDEX wake_triggers_daily_once ON wake_triggers(kind, instrument, recorded_day) WHERE kind != 'disclosure';
CREATE UNIQUE INDEX wake_triggers_disclosure_once ON wake_triggers(disclosure_source, receipt, instrument, grade, action) WHERE kind = 'disclosure';
CREATE INDEX web_collected_items_subject ON web_collected_items(subject_kind, subject, id);
CREATE TABLE ai_job_records (
    job_id TEXT PRIMARY KEY,
    finished_seconds INTEGER NOT NULL,
    finished_nanos INTEGER NOT NULL CHECK (finished_nanos BETWEEN 0 AND 999999999),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE ai_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE audit_log (
    seq INTEGER PRIMARY KEY CHECK (seq >= 1),
    at_payload TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    prev_hash TEXT NOT NULL CHECK (length(prev_hash) = 64),
    hash TEXT NOT NULL UNIQUE CHECK (length(hash) = 64)
) STRICT;
CREATE TABLE briefings (
    id_key TEXT PRIMARY KEY,
    seq INTEGER NOT NULL UNIQUE CHECK (seq >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE chat_entries (
    session_id TEXT NOT NULL REFERENCES chat_sessions(session_id),
    position INTEGER NOT NULL CHECK (position >= 1),
    user_message_id TEXT,
    entry_payload TEXT NOT NULL,
    PRIMARY KEY (session_id, position)
) STRICT;
CREATE TABLE chat_run_ledger (
    id INTEGER PRIMARY KEY,
    run_id TEXT NOT NULL,
    entry_payload TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE chat_sessions (
    session_id TEXT PRIMARY KEY,
    created_payload TEXT NOT NULL,
    updated_payload TEXT NOT NULL,
    entry_count INTEGER NOT NULL CHECK (entry_count >= 0)
) STRICT;
CREATE TABLE daily_valuation_days (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    day_year INTEGER NOT NULL CHECK (day_year BETWEEN 1 AND 9999),
    day_month INTEGER NOT NULL CHECK (day_month BETWEEN 1 AND 12),
    day_number INTEGER NOT NULL CHECK (day_number BETWEEN 1 AND 31),
    result_hash TEXT NOT NULL,
    result_payload TEXT NOT NULL,
    UNIQUE (account, day_year, day_month, day_number)
) STRICT;
CREATE TABLE daily_valuation_snapshots (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    as_of_seconds INTEGER NOT NULL,
    as_of_nanos INTEGER NOT NULL CHECK (as_of_nanos >= 0 AND as_of_nanos < 1000000000),
    as_of_offset INTEGER NOT NULL CHECK (as_of_offset BETWEEN -86340 AND 86340 AND as_of_offset % 60 = 0),
    snapshot_hash TEXT NOT NULL,
    snapshot_payload TEXT NOT NULL,
    UNIQUE (account, snapshot_hash)
) STRICT;
CREATE TABLE dashboard_research (
    section TEXT PRIMARY KEY,
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_calibrations (
    id INTEGER PRIMARY KEY,
    key_hash TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    key_payload TEXT NOT NULL,
    table_hash TEXT NOT NULL,
    table_payload TEXT NOT NULL,
    UNIQUE (key_hash, position)
) STRICT;
CREATE TABLE estimate_resolution_unavailable (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES estimate_submission_records(task_id),
    reason_payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_resolutions (
    id INTEGER PRIMARY KEY,
    task_key_hash TEXT NOT NULL UNIQUE,
    task_payload TEXT NOT NULL,
    outcome_payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_submission_records (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE REFERENCES estimate_tasks(task_id),
    run_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    record_payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_submissions (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    submission_hash TEXT NOT NULL UNIQUE,
    submission_payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_task_resolutions (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE REFERENCES estimate_submission_records(task_id),
    outcome_payload TEXT NOT NULL
) STRICT;
CREATE TABLE estimate_tasks (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('available', 'consumed')),
    task_payload TEXT NOT NULL
) STRICT;
CREATE TABLE holdings_snapshots (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    UNIQUE (account, seconds, nanos)
) STRICT;
CREATE VIRTUAL TABLE information_fts USING fts5(title_grams, body_grams, tokenize='unicode61', content='', contentless_delete=1);
CREATE TABLE 'information_fts_config'(k PRIMARY KEY, v) WITHOUT ROWID;
CREATE TABLE 'information_fts_data'(id INTEGER PRIMARY KEY, block BLOB);
CREATE TABLE 'information_fts_docsize'(id INTEGER PRIMARY KEY, sz BLOB, origin INTEGER);
CREATE TABLE 'information_fts_idx'(segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID;
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
CREATE TABLE information_times (
    item_id INTEGER PRIMARY KEY REFERENCES information_items(id),
    published_payload TEXT,
    collected_payload TEXT NOT NULL
) STRICT;
CREATE TABLE instrument_id_issuer (
    slot INTEGER PRIMARY KEY CHECK (slot = 1),
    last_issued TEXT NOT NULL CHECK (instr(last_issued, char(0)) = 0 AND length(last_issued) = 16 AND last_issued NOT GLOB '*[^0-9a-f]*' AND last_issued != '0000000000000000')
) STRICT;
CREATE TABLE instrument_mapping_events (
    seq INTEGER PRIMARY KEY CHECK (seq > 0),
    event_kind TEXT NOT NULL CHECK (event_kind IN ('identifier_changed', 'retirement_corrected', 'link_corrected')),
    instrument_id TEXT NOT NULL CHECK (instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'),
    market TEXT NOT NULL CHECK (market IN ('KR', 'US')),
    prev_symbol TEXT NOT NULL CHECK (instr(prev_symbol, char(0)) = 0 AND length(prev_symbol) BETWEEN 1 AND 32 AND prev_symbol GLOB '[A-Za-z0-9]*' AND prev_symbol NOT GLOB '*[^A-Za-z0-9.-]*'),
    prev_isin TEXT NOT NULL CHECK (instr(prev_isin, char(0)) = 0 AND length(prev_isin) BETWEEN 1 AND 64 AND prev_isin NOT GLOB '*[^!-~]*'),
    next_symbol TEXT CHECK (next_symbol IS NULL OR (instr(next_symbol, char(0)) = 0 AND length(next_symbol) BETWEEN 1 AND 32 AND next_symbol GLOB '[A-Za-z0-9]*' AND next_symbol NOT GLOB '*[^A-Za-z0-9.-]*')),
    next_isin TEXT CHECK (next_isin IS NULL OR (instr(next_isin, char(0)) = 0 AND length(next_isin) BETWEEN 1 AND 64 AND next_isin NOT GLOB '*[^!-~]*')),
    disclosure_source TEXT NOT NULL CHECK (disclosure_source IN ('DART', 'KIND', 'SEC_EDGAR')),
    disclosure_receipt_no TEXT NOT NULL CHECK (instr(disclosure_receipt_no, char(0)) = 0),
    action_kind TEXT CHECK (action_kind IS NULL OR action_kind IN ('symbol_change', 'split', 'reverse_split')),
    action_year INTEGER CHECK (action_year IS NULL OR action_year BETWEEN 1 AND 9999),
    action_month INTEGER CHECK (action_month IS NULL OR action_month BETWEEN 1 AND 12),
    action_day INTEGER CHECK (action_day IS NULL OR action_day BETWEEN 1 AND 31),
    action_from_symbol TEXT CHECK (action_from_symbol IS NULL OR (instr(action_from_symbol, char(0)) = 0 AND length(action_from_symbol) BETWEEN 1 AND 32 AND action_from_symbol GLOB '[A-Za-z0-9]*' AND action_from_symbol NOT GLOB '*[^A-Za-z0-9.-]*')),
    action_to_symbol TEXT CHECK (action_to_symbol IS NULL OR (instr(action_to_symbol, char(0)) = 0 AND length(action_to_symbol) BETWEEN 1 AND 32 AND action_to_symbol GLOB '[A-Za-z0-9]*' AND action_to_symbol NOT GLOB '*[^A-Za-z0-9.-]*')),
    action_from_count TEXT CHECK (action_from_count IS NULL OR (instr(action_from_count, char(0)) = 0 AND length(action_from_count) BETWEEN 1 AND 20 AND action_from_count NOT GLOB '*[^0-9]*' AND action_from_count NOT GLOB '0*')),
    action_to_count TEXT CHECK (action_to_count IS NULL OR (instr(action_to_count, char(0)) = 0 AND length(action_to_count) BETWEEN 1 AND 20 AND action_to_count NOT GLOB '*[^0-9]*' AND action_to_count NOT GLOB '0*')),
    note TEXT NOT NULL CHECK (length(CAST(note AS BLOB)) BETWEEN 1 AND 512 AND length(CAST(trim(note, ' ' || char(9) || char(10) || char(13)) AS BLOB)) > 0),
    recorded_seconds INTEGER NOT NULL CHECK (recorded_seconds BETWEEN -62135596800 AND 253402300799),
    recorded_nanos INTEGER NOT NULL CHECK (recorded_nanos >= 0 AND recorded_nanos < 1000000000),
    recorded_offset INTEGER NOT NULL CHECK (recorded_offset BETWEEN -86340 AND 86340 AND recorded_offset % 60 = 0),
    CHECK ((disclosure_source IN ('DART', 'KIND') AND length(disclosure_receipt_no) = 14 AND disclosure_receipt_no NOT GLOB '*[^0-9]*') OR (disclosure_source = 'SEC_EDGAR' AND length(disclosure_receipt_no) = 20 AND disclosure_receipt_no GLOB '[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9][0-9][0-9][0-9][0-9]')),
    CHECK ((event_kind = 'identifier_changed' AND next_symbol IS NOT NULL AND next_isin IS NOT NULL AND NOT (upper(prev_symbol) = upper(next_symbol) AND upper(prev_isin) = upper(next_isin)) AND action_kind IS NOT NULL AND action_year IS NOT NULL AND action_month IS NOT NULL AND action_day IS NOT NULL AND ((action_kind = 'symbol_change' AND action_from_symbol IS NOT NULL AND action_to_symbol IS NOT NULL AND upper(action_from_symbol) != upper(action_to_symbol) AND upper(action_from_symbol) = upper(prev_symbol) AND upper(action_to_symbol) = upper(next_symbol) AND action_from_count IS NULL AND action_to_count IS NULL) OR (action_kind = 'split' AND action_from_symbol IS NULL AND action_to_symbol IS NULL AND action_from_count IS NOT NULL AND action_to_count IS NOT NULL AND (length(action_to_count) > length(action_from_count) OR (length(action_to_count) = length(action_from_count) AND action_to_count > action_from_count))) OR (action_kind = 'reverse_split' AND action_from_symbol IS NULL AND action_to_symbol IS NULL AND action_from_count IS NOT NULL AND action_to_count IS NOT NULL AND (length(action_from_count) > length(action_to_count) OR (length(action_from_count) = length(action_to_count) AND action_from_count > action_to_count))))) OR (event_kind = 'retirement_corrected' AND next_symbol IS NULL AND next_isin IS NULL AND action_kind IS NULL AND action_year IS NULL AND action_month IS NULL AND action_day IS NULL AND action_from_symbol IS NULL AND action_to_symbol IS NULL AND action_from_count IS NULL AND action_to_count IS NULL) OR (event_kind = 'link_corrected' AND next_symbol IS NOT NULL AND next_isin IS NOT NULL AND NOT (upper(prev_symbol) = upper(next_symbol) AND upper(prev_isin) = upper(next_isin)) AND action_kind IS NULL AND action_year IS NULL AND action_month IS NULL AND action_day IS NULL AND action_from_symbol IS NULL AND action_to_symbol IS NULL AND action_from_count IS NULL AND action_to_count IS NULL))
) STRICT;
CREATE TABLE instrument_mappings (
    instrument_id TEXT NOT NULL CHECK (instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'),
    market TEXT NOT NULL CHECK (market IN ('KR', 'US')),
    symbol TEXT NOT NULL CHECK (instr(symbol, char(0)) = 0 AND length(symbol) BETWEEN 1 AND 32 AND symbol GLOB '[A-Za-z0-9]*' AND symbol NOT GLOB '*[^A-Za-z0-9.-]*'),
    isin TEXT NOT NULL CHECK (instr(isin, char(0)) = 0 AND length(isin) BETWEEN 1 AND 64 AND isin NOT GLOB '*[^!-~]*'),
    status TEXT NOT NULL CHECK (status IN ('active', 'superseded', 'retired')),
    superseded_event_seq INTEGER REFERENCES instrument_mapping_events(seq),
    PRIMARY KEY (instrument_id, market, symbol, isin),
    UNIQUE (market, symbol, isin),
    CHECK ((status = 'superseded' AND superseded_event_seq IS NOT NULL) OR (status IN ('active', 'retired') AND superseded_event_seq IS NULL))
) STRICT;
CREATE TABLE investment_principles (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE job_execution_records (
    seq INTEGER PRIMARY KEY,
    job TEXT NOT NULL,
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE job_schedule_settings (
    job TEXT PRIMARY KEY,
    enabled TEXT NOT NULL CHECK (enabled IN ('enabled', 'disabled')),
    hour INTEGER CHECK (hour IS NULL OR (hour >= 0 AND hour <= 23)),
    minute INTEGER CHECK (minute IS NULL OR (minute >= 0 AND minute <= 59)),
    CHECK ((hour IS NULL) = (minute IS NULL))
) STRICT;
CREATE TABLE market_candles (
    instrument TEXT NOT NULL,
    period TEXT NOT NULL,
    adjustment TEXT NOT NULL,
    basis TEXT NOT NULL,
    basis_offset INTEGER,
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL,
    offset INTEGER NOT NULL,
    currency TEXT NOT NULL,
    open_price TEXT NOT NULL CHECK (instr(open_price, char(0)) = 0 AND open_price NOT GLOB '*[^0-9.-]*' AND open_price GLOB '*[0-9]*' AND open_price NOT GLOB '?*-*' AND open_price NOT GLOB '*.*.*' AND open_price NOT GLOB '.*' AND open_price NOT GLOB '-.*' AND open_price NOT GLOB '*.'),
    high_price TEXT NOT NULL CHECK (instr(high_price, char(0)) = 0 AND high_price NOT GLOB '*[^0-9.-]*' AND high_price GLOB '*[0-9]*' AND high_price NOT GLOB '?*-*' AND high_price NOT GLOB '*.*.*' AND high_price NOT GLOB '.*' AND high_price NOT GLOB '-.*' AND high_price NOT GLOB '*.'),
    low_price TEXT NOT NULL CHECK (instr(low_price, char(0)) = 0 AND low_price NOT GLOB '*[^0-9.-]*' AND low_price GLOB '*[0-9]*' AND low_price NOT GLOB '?*-*' AND low_price NOT GLOB '*.*.*' AND low_price NOT GLOB '.*' AND low_price NOT GLOB '-.*' AND low_price NOT GLOB '*.'),
    close_price TEXT NOT NULL CHECK (instr(close_price, char(0)) = 0 AND close_price NOT GLOB '*[^0-9.-]*' AND close_price GLOB '*[0-9]*' AND close_price NOT GLOB '?*-*' AND close_price NOT GLOB '*.*.*' AND close_price NOT GLOB '.*' AND close_price NOT GLOB '-.*' AND close_price NOT GLOB '*.'),
    quantity TEXT NOT NULL CHECK (instr(quantity, char(0)) = 0 AND quantity NOT GLOB '*[^0-9.-]*' AND quantity GLOB '*[0-9]*' AND quantity NOT GLOB '?*-*' AND quantity NOT GLOB '*.*.*' AND quantity NOT GLOB '.*' AND quantity NOT GLOB '-.*' AND quantity NOT GLOB '*.'),
    collected_seconds INTEGER NOT NULL,
    collected_nanos INTEGER NOT NULL,
    collected_offset INTEGER NOT NULL,
    status TEXT NOT NULL,
    PRIMARY KEY (instrument, period, adjustment, basis, seconds, nanos)
) STRICT;
CREATE TABLE market_fx_rates (
    pair TEXT NOT NULL,
    kind TEXT NOT NULL,
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL,
    offset INTEGER NOT NULL,
    until_seconds INTEGER NOT NULL,
    until_nanos INTEGER NOT NULL,
    until_offset INTEGER NOT NULL,
    currency TEXT NOT NULL,
    exchange_rate TEXT NOT NULL CHECK (instr(exchange_rate, char(0)) = 0 AND exchange_rate NOT GLOB '*[^0-9.-]*' AND exchange_rate GLOB '*[0-9]*' AND exchange_rate NOT GLOB '?*-*' AND exchange_rate NOT GLOB '*.*.*' AND exchange_rate NOT GLOB '.*' AND exchange_rate NOT GLOB '-.*' AND exchange_rate NOT GLOB '*.'),
    source TEXT NOT NULL,
    PRIMARY KEY (pair, kind, seconds, nanos)
) STRICT;
CREATE TABLE market_quotes (
    instrument TEXT PRIMARY KEY,
    symbol TEXT NOT NULL,
    currency TEXT NOT NULL CHECK (currency IN ('KRW', 'USD')),
    last_price TEXT CHECK (instr(last_price, char(0)) = 0 AND last_price NOT GLOB '*[^0-9.-]*' AND last_price GLOB '*[0-9]*' AND last_price NOT GLOB '?*-*' AND last_price NOT GLOB '*.*.*' AND last_price NOT GLOB '.*' AND last_price NOT GLOB '-.*' AND last_price NOT GLOB '*.'),
    observed_seconds INTEGER,
    observed_nanos INTEGER,
    observed_offset INTEGER,
    collected_seconds INTEGER NOT NULL,
    collected_nanos INTEGER NOT NULL,
    collected_offset INTEGER NOT NULL,
    last_server_seconds INTEGER,
    last_server_nanos INTEGER,
    last_server_offset INTEGER,
    CHECK ((observed_seconds IS NULL AND observed_nanos IS NULL AND observed_offset IS NULL) OR (observed_seconds IS NOT NULL AND observed_nanos IS NOT NULL AND observed_offset IS NOT NULL)),
    CHECK ((last_server_seconds IS NULL AND last_server_nanos IS NULL AND last_server_offset IS NULL) OR (last_server_seconds IS NOT NULL AND last_server_nanos IS NOT NULL AND last_server_offset IS NOT NULL)),
    CHECK (observed_seconds IS NULL OR last_server_seconds IS NOT NULL)
) STRICT;
CREATE TABLE master_instruments (
    market TEXT NOT NULL CHECK (market IN ('KR', 'US')),
    position INTEGER NOT NULL CHECK (position >= 0),
    listing_market TEXT NOT NULL,
    symbol TEXT NOT NULL,
    instrument_id TEXT NOT NULL,
    name TEXT NOT NULL,
    security_type TEXT NOT NULL,
    currency TEXT NOT NULL CHECK (currency IN ('KRW', 'USD')),
    listing_status TEXT NOT NULL CHECK (listing_status IN ('scheduled', 'active', 'delisted')),
    common_share TEXT NOT NULL CHECK (common_share IN ('common', 'other')),
    liquidation TEXT CHECK (liquidation IS NULL OR liquidation IN ('regular', 'liquidation')),
    krx TEXT CHECK (krx IS NULL OR krx IN ('clear', 'suspended')),
    nxt TEXT CHECK (nxt IS NULL OR nxt IN ('unsupported', 'clear', 'suspended')),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (market, position),
    CHECK ((liquidation IS NULL) = (krx IS NULL) AND (krx IS NULL) = (nxt IS NULL))
) STRICT;
CREATE TABLE master_refreshes (
    market TEXT PRIMARY KEY CHECK (market IN ('KR', 'US')),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL
) STRICT;
CREATE TABLE migration_ledger (
    number INTEGER PRIMARY KEY CHECK (number > 0),
    name TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    applied_at INTEGER NOT NULL
) STRICT;
CREATE TABLE news_data_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE order_rate_limit (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    hourly_limit INTEGER NOT NULL CHECK (hourly_limit BETWEEN 1 AND 30)
) STRICT;
CREATE TABLE order_submissions (
    id INTEGER PRIMARY KEY,
    utc_seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos BETWEEN 0 AND 999999999),
    offset_seconds INTEGER NOT NULL CHECK (offset_seconds BETWEEN -93599 AND 93599)
) STRICT;
CREATE TABLE proposal_consumptions (
    proposal_id TEXT PRIMARY KEY
) STRICT;
CREATE TABLE proposal_drafts (
    id INTEGER PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('standalone', 'guarded')),
    instrument TEXT NOT NULL,
    account INTEGER NOT NULL,
    expires_seconds INTEGER NOT NULL,
    expires_nanos INTEGER NOT NULL CHECK (expires_nanos >= 0 AND expires_nanos < 1000000000),
    expires_offset INTEGER NOT NULL CHECK (expires_offset BETWEEN -86340 AND 86340 AND expires_offset % 60 = 0),
    state TEXT NOT NULL CHECK (state IN ('current', 'superseded')),
    snapshot_payload TEXT,
    evidence_payload TEXT NOT NULL,
    CHECK ((mode = 'standalone' AND snapshot_payload IS NULL) OR (mode = 'guarded' AND snapshot_payload IS NOT NULL))
) STRICT;
CREATE TABLE proposal_validation_reports (
    id INTEGER PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    c01 TEXT NOT NULL CHECK (c01 IN ('passed', 'failed', 'unknown')),
    c02 TEXT NOT NULL CHECK (c02 IN ('passed', 'failed', 'unknown')),
    c03 TEXT NOT NULL CHECK (c03 IN ('passed', 'failed', 'unknown')),
    c04 TEXT NOT NULL CHECK (c04 IN ('passed', 'failed', 'unknown')),
    c05 TEXT NOT NULL CHECK (c05 IN ('passed', 'failed', 'unknown')),
    c06 TEXT NOT NULL CHECK (c06 IN ('passed', 'failed', 'unknown')),
    c07 TEXT NOT NULL CHECK (c07 IN ('passed', 'failed', 'unknown')),
    c08 TEXT NOT NULL CHECK (c08 IN ('passed', 'failed', 'unknown')),
    c09 TEXT NOT NULL CHECK (c09 IN ('passed', 'failed', 'unknown')),
    c10 TEXT NOT NULL CHECK (c10 IN ('passed', 'failed', 'unknown')),
    c11 TEXT NOT NULL CHECK (c11 IN ('passed', 'failed', 'unknown')),
    c12 TEXT NOT NULL CHECK (c12 IN ('passed', 'failed', 'unknown'))
) STRICT;
CREATE TABLE settings (
    name TEXT PRIMARY KEY CHECK (name IN ('investor_profile', 'risk_settings', 'toss_spec_hash')),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_discarded_submissions (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    task_id TEXT NOT NULL,
    UNIQUE (instrument, task_id)
) STRICT;
CREATE TABLE signal_masters (
    instrument_id TEXT PRIMARY KEY,
    master_payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_occurrences (
    id INTEGER PRIMARY KEY,
    occurrence_id TEXT NOT NULL UNIQUE,
    instrument_id TEXT NOT NULL,
    market TEXT NOT NULL,
    symbol TEXT NOT NULL,
    occurrence_payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_selected_account (
    slot INTEGER PRIMARY KEY CHECK (slot = 1),
    account INTEGER NOT NULL
) STRICT;
CREATE TABLE signal_skips (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    reason TEXT CHECK (reason IS NULL OR (length(reason) BETWEEN 1 AND 200 AND instr(reason, char(10)) = 0 AND instr(reason, char(13)) = 0)),
    skipped_seconds INTEGER NOT NULL,
    skipped_nanos INTEGER NOT NULL CHECK (skipped_nanos BETWEEN 0 AND 999999999),
    skipped_offset INTEGER NOT NULL CHECK (skipped_offset BETWEEN -93599 AND 93599)
) STRICT;
CREATE TABLE signal_sources (
    task_id TEXT PRIMARY KEY,
    sources_payload TEXT NOT NULL
) STRICT;
CREATE TABLE signal_states (
    instrument TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    current_payload TEXT
) STRICT;
CREATE TABLE snapshot_cash (
    snapshot INTEGER NOT NULL REFERENCES holdings_snapshots(id) ON DELETE CASCADE,
    currency TEXT NOT NULL CHECK (currency IN ('KRW', 'USD')),
    amount TEXT NOT NULL CHECK (instr(amount, char(0)) = 0 AND amount NOT GLOB '*[^0-9.-]*' AND amount GLOB '*[0-9]*' AND amount NOT GLOB '?*-*' AND amount NOT GLOB '*.*.*' AND amount NOT GLOB '.*' AND amount NOT GLOB '-.*' AND amount NOT GLOB '*.'),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (snapshot, currency)
) STRICT;
CREATE TABLE snapshot_holdings (
    snapshot INTEGER NOT NULL REFERENCES holdings_snapshots(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    symbol TEXT NOT NULL,
    currency TEXT NOT NULL CHECK (currency IN ('KRW', 'USD')),
    quantity TEXT NOT NULL CHECK (instr(quantity, char(0)) = 0 AND quantity NOT GLOB '*[^0-9.-]*' AND quantity GLOB '*[0-9]*' AND quantity NOT GLOB '?*-*' AND quantity NOT GLOB '*.*.*' AND quantity NOT GLOB '.*' AND quantity NOT GLOB '-.*' AND quantity NOT GLOB '*.'),
    acquisition_price TEXT NOT NULL CHECK (instr(acquisition_price, char(0)) = 0 AND acquisition_price NOT GLOB '*[^0-9.-]*' AND acquisition_price GLOB '*[0-9]*' AND acquisition_price NOT GLOB '?*-*' AND acquisition_price NOT GLOB '*.*.*' AND acquisition_price NOT GLOB '.*' AND acquisition_price NOT GLOB '-.*' AND acquisition_price NOT GLOB '*.'),
    current_price TEXT CHECK (instr(current_price, char(0)) = 0 AND current_price NOT GLOB '*[^0-9.-]*' AND current_price GLOB '*[0-9]*' AND current_price NOT GLOB '?*-*' AND current_price NOT GLOB '*.*.*' AND current_price NOT GLOB '.*' AND current_price NOT GLOB '-.*' AND current_price NOT GLOB '*.'),
    valuation TEXT CHECK (instr(valuation, char(0)) = 0 AND valuation NOT GLOB '*[^0-9.-]*' AND valuation GLOB '*[0-9]*' AND valuation NOT GLOB '?*-*' AND valuation NOT GLOB '*.*.*' AND valuation NOT GLOB '.*' AND valuation NOT GLOB '-.*' AND valuation NOT GLOB '*.'),
    profit TEXT CHECK (instr(profit, char(0)) = 0 AND profit NOT GLOB '*[^0-9.-]*' AND profit GLOB '*[0-9]*' AND profit NOT GLOB '?*-*' AND profit NOT GLOB '*.*.*' AND profit NOT GLOB '.*' AND profit NOT GLOB '-.*' AND profit NOT GLOB '*.'),
    return_rate TEXT CHECK (instr(return_rate, char(0)) = 0 AND return_rate NOT GLOB '*[^0-9.-]*' AND return_rate GLOB '*[0-9]*' AND return_rate NOT GLOB '?*-*' AND return_rate NOT GLOB '*.*.*' AND return_rate NOT GLOB '.*' AND return_rate NOT GLOB '-.*' AND return_rate NOT GLOB '*.'),
    daily_profit TEXT CHECK (instr(daily_profit, char(0)) = 0 AND daily_profit NOT GLOB '*[^0-9.-]*' AND daily_profit GLOB '*[0-9]*' AND daily_profit NOT GLOB '?*-*' AND daily_profit NOT GLOB '*.*.*' AND daily_profit NOT GLOB '.*' AND daily_profit NOT GLOB '-.*' AND daily_profit NOT GLOB '*.'),
    daily_rate TEXT CHECK (instr(daily_rate, char(0)) = 0 AND daily_rate NOT GLOB '*[^0-9.-]*' AND daily_rate GLOB '*[0-9]*' AND daily_rate NOT GLOB '?*-*' AND daily_rate NOT GLOB '*.*.*' AND daily_rate NOT GLOB '.*' AND daily_rate NOT GLOB '-.*' AND daily_rate NOT GLOB '*.'),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (snapshot, position),
    UNIQUE (snapshot, symbol, currency),
    CHECK ((valuation IS NULL AND profit IS NULL AND return_rate IS NULL AND daily_profit IS NULL AND daily_rate IS NULL) OR (valuation IS NOT NULL AND profit IS NOT NULL AND return_rate IS NOT NULL AND ((daily_profit IS NULL AND daily_rate IS NULL) OR (daily_profit IS NOT NULL AND daily_rate IS NOT NULL))))
) STRICT;
CREATE TABLE "source_statuses" (
    source TEXT PRIMARY KEY CHECK (source IN ('kind_disclosure', 'dart_disclosure_api', 'dart_disclosure_rss', 'dart_fundamentals', 'kind_corp_list', 'hankyung', 'maekyung', 'yonhap', 'edaily', 'segye_finance', 'google_news', 'web_search', 'yahoo_finance', 'sec_edgar', 'bank_of_korea', 'federal_reserve', 'us_treasury', 'fred')),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE spec_status_records (
    id INTEGER PRIMARY KEY,
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE thesis_history (
    instrument TEXT NOT NULL REFERENCES thesis_notes(instrument),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    payload TEXT NOT NULL,
    PRIMARY KEY (instrument, sequence)
) STRICT;
CREATE TABLE thesis_history_times (
    instrument TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (instrument, sequence),
    FOREIGN KEY (instrument, sequence) REFERENCES thesis_history(instrument, sequence)
) STRICT;
CREATE TABLE thesis_notes (
    instrument TEXT PRIMARY KEY
) STRICT;
CREATE TABLE tool_call_ledger (
    id INTEGER PRIMARY KEY,
    run_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'tool_error', 'rejected')),
    started_payload TEXT NOT NULL,
    finished_payload TEXT NOT NULL,
    protocol_version TEXT NOT NULL
) STRICT;
CREATE TABLE transaction_entries (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('sync', 'summary', 'history')),
    payload TEXT NOT NULL
) STRICT;
CREATE TABLE transaction_full_inspections (
    account INTEGER PRIMARY KEY,
    completed_at TEXT NOT NULL
) STRICT;
CREATE TABLE transaction_rows (
    id INTEGER PRIMARY KEY,
    entry INTEGER NOT NULL REFERENCES transaction_entries(id),
    scope TEXT NOT NULL,
    page INTEGER NOT NULL CHECK (page >= 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    first_seen TEXT,
    disappeared_at TEXT,
    kind TEXT NOT NULL,
    market TEXT NOT NULL,
    currency TEXT NOT NULL,
    symbol TEXT,
    asset_class TEXT,
    isin TEXT,
    quantity TEXT CHECK (instr(quantity, char(0)) = 0 AND quantity NOT GLOB '*[^0-9.-]*' AND quantity GLOB '*[0-9]*' AND quantity NOT GLOB '?*-*' AND quantity NOT GLOB '*.*.*' AND quantity NOT GLOB '.*' AND quantity NOT GLOB '-.*' AND quantity NOT GLOB '*.'),
    filled_price TEXT CHECK (instr(filled_price, char(0)) = 0 AND filled_price NOT GLOB '*[^0-9.-]*' AND filled_price GLOB '*[0-9]*' AND filled_price NOT GLOB '?*-*' AND filled_price NOT GLOB '*.*.*' AND filled_price NOT GLOB '.*' AND filled_price NOT GLOB '-.*' AND filled_price NOT GLOB '*.'),
    amount TEXT CHECK (instr(amount, char(0)) = 0 AND amount NOT GLOB '*[^0-9.-]*' AND amount GLOB '*[0-9]*' AND amount NOT GLOB '?*-*' AND amount NOT GLOB '*.*.*' AND amount NOT GLOB '.*' AND amount NOT GLOB '-.*' AND amount NOT GLOB '*.'),
    commission TEXT NOT NULL CHECK (instr(commission, char(0)) = 0 AND commission NOT GLOB '*[^0-9.-]*' AND commission GLOB '*[0-9]*' AND commission NOT GLOB '?*-*' AND commission NOT GLOB '*.*.*' AND commission NOT GLOB '.*' AND commission NOT GLOB '-.*' AND commission NOT GLOB '*.'),
    tax TEXT NOT NULL CHECK (instr(tax, char(0)) = 0 AND tax NOT GLOB '*[^0-9.-]*' AND tax GLOB '*[0-9]*' AND tax NOT GLOB '?*-*' AND tax NOT GLOB '*.*.*' AND tax NOT GLOB '.*' AND tax NOT GLOB '-.*' AND tax NOT GLOB '*.'),
    net_amount TEXT CHECK (instr(net_amount, char(0)) = 0 AND net_amount NOT GLOB '*[^0-9.-]*' AND net_amount GLOB '*[0-9]*' AND net_amount NOT GLOB '?*-*' AND net_amount NOT GLOB '*.*.*' AND net_amount NOT GLOB '.*' AND net_amount NOT GLOB '-.*' AND net_amount NOT GLOB '*.'),
    exchange_rate TEXT CHECK (instr(exchange_rate, char(0)) = 0 AND exchange_rate NOT GLOB '*[^0-9.-]*' AND exchange_rate GLOB '*[0-9]*' AND exchange_rate NOT GLOB '?*-*' AND exchange_rate NOT GLOB '*.*.*' AND exchange_rate NOT GLOB '.*' AND exchange_rate NOT GLOB '-.*' AND exchange_rate NOT GLOB '*.'),
    settlement_date TEXT NOT NULL,
    settlement TEXT NOT NULL,
    transaction_time TEXT,
    cancellation TEXT NOT NULL,
    UNIQUE (entry, scope, page, position)
) STRICT;
CREATE TABLE transaction_sources (
    row_id INTEGER PRIMARY KEY REFERENCES transaction_rows(id),
    source TEXT NOT NULL
) STRICT;
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
CREATE TABLE watchlist_entries (
    position INTEGER PRIMARY KEY CHECK (position BETWEEN 0 AND 199),
    instrument TEXT NOT NULL UNIQUE,
    added_seconds INTEGER NOT NULL,
    added_nanos INTEGER NOT NULL CHECK (added_nanos BETWEEN 0 AND 999999999),
    added_offset INTEGER NOT NULL CHECK (added_offset BETWEEN -93599 AND 93599)
) STRICT;
CREATE TABLE web_collected_items (
    id INTEGER PRIMARY KEY,
    subject_kind TEXT NOT NULL CHECK (subject_kind IN ('symbol', 'instrument')),
    subject TEXT NOT NULL,
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER ai_job_records_no_delete BEFORE DELETE ON ai_job_records BEGIN SELECT RAISE(ABORT, 'ai job records are append only'); END;
CREATE TRIGGER ai_job_records_no_update BEFORE UPDATE ON ai_job_records BEGIN SELECT RAISE(ABORT, 'ai job records are append only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_seq BEFORE INSERT ON audit_log WHEN NEW.seq IS NOT (SELECT coalesce(max(seq), 0) + 1 FROM audit_log) BEGIN SELECT RAISE(ABORT, 'audit_log seq must be contiguous'); END;
CREATE TRIGGER briefings_no_delete BEFORE DELETE ON briefings BEGIN SELECT RAISE(ABORT, 'briefings are replaced, not deleted'); END;
CREATE TRIGGER chat_entries_contiguous BEFORE INSERT ON chat_entries WHEN NEW.position IS NOT (SELECT coalesce(max(position), 0) + 1 FROM chat_entries WHERE session_id = NEW.session_id) BEGIN SELECT RAISE(ABORT, 'chat entry position must be contiguous'); END;
CREATE TRIGGER chat_entries_no_delete BEFORE DELETE ON chat_entries BEGIN SELECT RAISE(ABORT, 'chat entries are append only'); END;
CREATE TRIGGER chat_entries_no_update BEFORE UPDATE ON chat_entries BEGIN SELECT RAISE(ABORT, 'chat entries are append only'); END;
CREATE TRIGGER chat_run_ledger_no_delete BEFORE DELETE ON chat_run_ledger BEGIN SELECT RAISE(ABORT, 'chat run ledger is append only'); END;
CREATE TRIGGER chat_run_ledger_no_update BEFORE UPDATE ON chat_run_ledger BEGIN SELECT RAISE(ABORT, 'chat run ledger is append only'); END;
CREATE TRIGGER chat_sessions_fixed_identity BEFORE UPDATE OF session_id, created_payload ON chat_sessions BEGIN SELECT RAISE(ABORT, 'chat session identity is fixed'); END;
CREATE TRIGGER chat_sessions_no_delete BEFORE DELETE ON chat_sessions BEGIN SELECT RAISE(ABORT, 'chat sessions are kept'); END;
CREATE TRIGGER daily_valuation_days_no_delete BEFORE DELETE ON daily_valuation_days BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
CREATE TRIGGER daily_valuation_days_no_replace BEFORE INSERT ON daily_valuation_days WHEN EXISTS (SELECT 1 FROM daily_valuation_days WHERE account = NEW.account AND day_year = NEW.day_year AND day_month = NEW.day_month AND day_number = NEW.day_number) BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
CREATE TRIGGER daily_valuation_days_no_update BEFORE UPDATE ON daily_valuation_days BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
CREATE TRIGGER daily_valuation_snapshots_no_delete BEFORE DELETE ON daily_valuation_snapshots BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TRIGGER daily_valuation_snapshots_no_replace BEFORE INSERT ON daily_valuation_snapshots WHEN EXISTS (SELECT 1 FROM daily_valuation_snapshots WHERE account = NEW.account AND snapshot_hash = NEW.snapshot_hash) BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TRIGGER daily_valuation_snapshots_no_update BEFORE UPDATE ON daily_valuation_snapshots BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TRIGGER estimate_calibrations_no_delete BEFORE DELETE ON estimate_calibrations BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TRIGGER estimate_calibrations_no_replace BEFORE INSERT ON estimate_calibrations WHEN EXISTS (SELECT 1 FROM estimate_calibrations WHERE key_hash = NEW.key_hash AND position = NEW.position) BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TRIGGER estimate_calibrations_no_update BEFORE UPDATE ON estimate_calibrations BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TRIGGER estimate_resolution_unavailable_after_resolved BEFORE INSERT ON estimate_resolution_unavailable WHEN EXISTS (SELECT 1 FROM estimate_task_resolutions WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'resolved estimate cannot record unavailable'); END;
CREATE TRIGGER estimate_resolution_unavailable_no_delete BEFORE DELETE ON estimate_resolution_unavailable BEGIN SELECT RAISE(ABORT, 'estimate unavailable records are append only'); END;
CREATE TRIGGER estimate_resolution_unavailable_no_update BEFORE UPDATE ON estimate_resolution_unavailable BEGIN SELECT RAISE(ABORT, 'estimate unavailable records are append only'); END;
CREATE TRIGGER estimate_resolutions_no_delete BEFORE DELETE ON estimate_resolutions BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TRIGGER estimate_resolutions_no_replace BEFORE INSERT ON estimate_resolutions WHEN EXISTS (SELECT 1 FROM estimate_resolutions WHERE task_key_hash = NEW.task_key_hash) BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TRIGGER estimate_resolutions_no_update BEFORE UPDATE ON estimate_resolutions BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TRIGGER estimate_submission_records_no_delete BEFORE DELETE ON estimate_submission_records BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TRIGGER estimate_submission_records_no_replace BEFORE INSERT ON estimate_submission_records WHEN EXISTS (SELECT 1 FROM estimate_submission_records WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TRIGGER estimate_submission_records_no_update BEFORE UPDATE ON estimate_submission_records BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TRIGGER estimate_submissions_no_delete BEFORE DELETE ON estimate_submissions BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TRIGGER estimate_submissions_no_replace BEFORE INSERT ON estimate_submissions WHEN EXISTS (SELECT 1 FROM estimate_submissions WHERE submission_hash = NEW.submission_hash) BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TRIGGER estimate_submissions_no_update BEFORE UPDATE ON estimate_submissions BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TRIGGER estimate_task_resolutions_no_delete BEFORE DELETE ON estimate_task_resolutions BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TRIGGER estimate_task_resolutions_no_replace BEFORE INSERT ON estimate_task_resolutions WHEN EXISTS (SELECT 1 FROM estimate_task_resolutions WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TRIGGER estimate_task_resolutions_no_update BEFORE UPDATE ON estimate_task_resolutions BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TRIGGER estimate_tasks_consume_only BEFORE UPDATE ON estimate_tasks WHEN OLD.status != 'available' OR NEW.status != 'consumed' OR NEW.id != OLD.id OR NEW.task_id != OLD.task_id OR NEW.run_id != OLD.run_id OR NEW.instrument != OLD.instrument OR NEW.task_payload != OLD.task_payload BEGIN SELECT RAISE(ABORT, 'estimate task can only transition from available to consumed'); END;
CREATE TRIGGER estimate_tasks_insert_available_only BEFORE INSERT ON estimate_tasks WHEN NEW.status != 'available' BEGIN SELECT RAISE(ABORT, 'estimate task must start available'); END;
CREATE TRIGGER estimate_tasks_no_delete BEFORE DELETE ON estimate_tasks BEGIN SELECT RAISE(ABORT, 'estimate tasks are append only'); END;
CREATE TRIGGER estimate_tasks_no_replace BEFORE INSERT ON estimate_tasks WHEN EXISTS (SELECT 1 FROM estimate_tasks WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate task cannot be replaced'); END;
CREATE TRIGGER instrument_id_issuer_monotonic BEFORE UPDATE ON instrument_id_issuer WHEN NEW.slot != OLD.slot OR NEW.last_issued <= OLD.last_issued BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor must advance monotonically'); END;
CREATE TRIGGER instrument_id_issuer_no_delete BEFORE DELETE ON instrument_id_issuer BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor cannot be deleted'); END;
CREATE TRIGGER instrument_id_issuer_single_insert BEFORE INSERT ON instrument_id_issuer WHEN EXISTS (SELECT 1 FROM instrument_id_issuer) BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor already exists'); END;
CREATE TRIGGER instrument_mapping_events_monotonic_seq BEFORE INSERT ON instrument_mapping_events WHEN NEW.seq != coalesce((SELECT max(seq) FROM instrument_mapping_events), 0) + 1 BEGIN SELECT RAISE(ABORT, 'instrument mapping event sequence must advance monotonically without gaps'); END;
CREATE TRIGGER instrument_mapping_events_no_delete BEFORE DELETE ON instrument_mapping_events BEGIN SELECT RAISE(ABORT, 'instrument mapping events are append only'); END;
CREATE TRIGGER instrument_mapping_events_no_update BEFORE UPDATE ON instrument_mapping_events BEGIN SELECT RAISE(ABORT, 'instrument mapping events are append only'); END;
CREATE TRIGGER instrument_mappings_insert_active_only BEFORE INSERT ON instrument_mappings WHEN NEW.status != 'active' OR NEW.superseded_event_seq IS NOT NULL BEGIN SELECT RAISE(ABORT, 'instrument mapping must start active'); END;
CREATE TRIGGER instrument_mappings_no_delete BEFORE DELETE ON instrument_mappings WHEN OLD.status != 'active' OR NOT EXISTS (SELECT 1 FROM instrument_mapping_events WHERE seq = (SELECT max(seq) FROM instrument_mapping_events WHERE instrument_id = OLD.instrument_id) AND event_kind = 'link_corrected' AND instrument_id = OLD.instrument_id AND market = OLD.market AND next_symbol = OLD.symbol AND next_isin = OLD.isin) BEGIN SELECT RAISE(ABORT, 'instrument mappings cannot be deleted'); END;
CREATE TRIGGER instrument_mappings_no_replace BEFORE INSERT ON instrument_mappings WHEN EXISTS (SELECT 1 FROM instrument_mappings WHERE market = NEW.market AND upper(symbol) = upper(NEW.symbol) AND instrument_id != NEW.instrument_id) OR EXISTS (SELECT 1 FROM instrument_mappings WHERE upper(isin) = upper(NEW.isin) AND instrument_id != NEW.instrument_id) OR EXISTS (SELECT 1 FROM instrument_mappings WHERE instrument_id = NEW.instrument_id AND (status IN ('active', 'retired') OR market != NEW.market OR (upper(symbol) = upper(NEW.symbol) AND upper(isin) = upper(NEW.isin)))) OR (EXISTS (SELECT 1 FROM instrument_mappings WHERE instrument_id = NEW.instrument_id) AND NOT EXISTS (SELECT 1 FROM instrument_mapping_events WHERE seq = (SELECT max(seq) FROM instrument_mapping_events WHERE instrument_id = NEW.instrument_id) AND event_kind = 'identifier_changed' AND instrument_id = NEW.instrument_id AND market = NEW.market AND next_symbol = NEW.symbol AND next_isin = NEW.isin)) BEGIN SELECT RAISE(ABORT, 'instrument mapping cannot replace existing history'); END;
CREATE TRIGGER instrument_mappings_retire_only BEFORE UPDATE ON instrument_mappings WHEN NEW.instrument_id != OLD.instrument_id OR NEW.market != OLD.market OR NEW.symbol != OLD.symbol OR NEW.isin != OLD.isin OR NOT ((OLD.status = 'active' AND NEW.status = 'retired' AND OLD.superseded_event_seq IS NULL AND NEW.superseded_event_seq IS NULL) OR (OLD.status = 'active' AND NEW.status = 'superseded' AND OLD.superseded_event_seq IS NULL AND NEW.superseded_event_seq IS NOT NULL AND EXISTS (SELECT 1 FROM instrument_mapping_events WHERE seq = NEW.superseded_event_seq AND seq = (SELECT max(seq) FROM instrument_mapping_events WHERE instrument_id = OLD.instrument_id) AND event_kind = 'identifier_changed' AND instrument_id = OLD.instrument_id AND market = OLD.market AND prev_symbol = OLD.symbol AND prev_isin = OLD.isin)) OR (OLD.status = 'retired' AND NEW.status = 'active' AND OLD.superseded_event_seq IS NULL AND NEW.superseded_event_seq IS NULL AND EXISTS (SELECT 1 FROM instrument_mapping_events WHERE seq = (SELECT max(seq) FROM instrument_mapping_events WHERE instrument_id = OLD.instrument_id) AND event_kind = 'retirement_corrected' AND instrument_id = OLD.instrument_id AND market = OLD.market AND prev_symbol = OLD.symbol AND prev_isin = OLD.isin)) OR (OLD.status = 'superseded' AND NEW.status = 'active' AND OLD.superseded_event_seq IS NOT NULL AND NEW.superseded_event_seq IS NULL AND EXISTS (SELECT 1 FROM instrument_mapping_events WHERE seq = (SELECT max(seq) FROM instrument_mapping_events WHERE instrument_id = OLD.instrument_id) AND event_kind = 'link_corrected' AND instrument_id = OLD.instrument_id AND market = OLD.market AND prev_symbol = OLD.symbol AND prev_isin = OLD.isin))) BEGIN SELECT RAISE(ABORT, 'instrument mapping transition is not permitted'); END;
CREATE TRIGGER investment_principles_no_delete BEFORE DELETE ON investment_principles BEGIN SELECT RAISE(ABORT, 'investment principles are replaced, not deleted'); END;
CREATE TRIGGER news_data_settings_no_delete BEFORE DELETE ON news_data_settings BEGIN SELECT RAISE(ABORT, 'news data settings are replaced, not deleted'); END;
CREATE TRIGGER order_rate_limit_no_delete BEFORE DELETE ON order_rate_limit BEGIN SELECT RAISE(ABORT, 'order rate limit is replaced, not deleted'); END;
CREATE TRIGGER order_submissions_no_delete BEFORE DELETE ON order_submissions BEGIN SELECT RAISE(ABORT, 'order submissions are append only'); END;
CREATE TRIGGER order_submissions_no_update BEFORE UPDATE ON order_submissions BEGIN SELECT RAISE(ABORT, 'order submissions are append only'); END;
CREATE TRIGGER proposal_consumptions_no_delete BEFORE DELETE ON proposal_consumptions BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
CREATE TRIGGER proposal_consumptions_no_replace BEFORE INSERT ON proposal_consumptions WHEN EXISTS (SELECT 1 FROM proposal_consumptions WHERE proposal_id = NEW.proposal_id) BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
CREATE TRIGGER proposal_consumptions_no_update BEFORE UPDATE ON proposal_consumptions BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
CREATE TRIGGER proposal_drafts_no_delete BEFORE DELETE ON proposal_drafts BEGIN SELECT RAISE(ABORT, 'proposal drafts are append only'); END;
CREATE TRIGGER proposal_drafts_supersede_only BEFORE UPDATE ON proposal_drafts WHEN OLD.state != 'current' OR NEW.state != 'superseded' OR NEW.id != OLD.id OR NEW.proposal_id != OLD.proposal_id OR NEW.mode != OLD.mode OR NEW.instrument != OLD.instrument OR NEW.account != OLD.account OR NEW.expires_seconds != OLD.expires_seconds OR NEW.expires_nanos != OLD.expires_nanos OR NEW.expires_offset != OLD.expires_offset OR coalesce(NEW.snapshot_payload, '') != coalesce(OLD.snapshot_payload, '') OR (NEW.snapshot_payload IS NULL) != (OLD.snapshot_payload IS NULL) OR NEW.evidence_payload != OLD.evidence_payload BEGIN SELECT RAISE(ABORT, 'proposal draft can only transition from current to superseded'); END;
CREATE TRIGGER settings_no_delete BEFORE DELETE ON settings BEGIN SELECT RAISE(ABORT, 'settings are replaced, not deleted'); END;
CREATE TRIGGER signal_discarded_submissions_no_delete BEFORE DELETE ON signal_discarded_submissions BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TRIGGER signal_discarded_submissions_no_replace BEFORE INSERT ON signal_discarded_submissions WHEN EXISTS (SELECT 1 FROM signal_discarded_submissions WHERE instrument = NEW.instrument AND task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TRIGGER signal_discarded_submissions_no_update BEFORE UPDATE ON signal_discarded_submissions BEGIN SELECT RAISE(ABORT, 'signal discarded submissions are append only'); END;
CREATE TRIGGER signal_occurrences_no_delete BEFORE DELETE ON signal_occurrences BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TRIGGER signal_occurrences_no_replace BEFORE INSERT ON signal_occurrences WHEN EXISTS (SELECT 1 FROM signal_occurrences WHERE occurrence_id = NEW.occurrence_id) BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TRIGGER signal_occurrences_no_update BEFORE UPDATE ON signal_occurrences BEGIN SELECT RAISE(ABORT, 'signal occurrences are append only'); END;
CREATE TRIGGER signal_skips_no_delete BEFORE DELETE ON signal_skips BEGIN SELECT RAISE(ABORT, 'signal skips are append only'); END;
CREATE TRIGGER signal_skips_no_update BEFORE UPDATE ON signal_skips BEGIN SELECT RAISE(ABORT, 'signal skips are append only'); END;
CREATE TRIGGER source_statuses_no_delete BEFORE DELETE ON source_statuses BEGIN SELECT RAISE(ABORT, 'source statuses are replaced, not deleted'); END;
CREATE TRIGGER spec_status_records_no_delete BEFORE DELETE ON spec_status_records BEGIN SELECT RAISE(ABORT, 'spec status records are append only'); END;
CREATE TRIGGER spec_status_records_no_update BEFORE UPDATE ON spec_status_records BEGIN SELECT RAISE(ABORT, 'spec status records are append only'); END;
CREATE TRIGGER thesis_history_no_delete BEFORE DELETE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
CREATE TRIGGER thesis_history_no_replace BEFORE INSERT ON thesis_history WHEN EXISTS (SELECT 1 FROM thesis_history WHERE instrument = NEW.instrument AND sequence = NEW.sequence) BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
CREATE TRIGGER thesis_history_no_update BEFORE UPDATE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;
CREATE TRIGGER tool_call_ledger_no_delete BEFORE DELETE ON tool_call_ledger BEGIN SELECT RAISE(ABORT, 'tool call ledger is append only'); END;
CREATE TRIGGER tool_call_ledger_no_update BEFORE UPDATE ON tool_call_ledger BEGIN SELECT RAISE(ABORT, 'tool call ledger is append only'); END;
CREATE TRIGGER wake_triggers_no_delete BEFORE DELETE ON wake_triggers BEGIN SELECT RAISE(ABORT, 'wake triggers are append only'); END;
CREATE TRIGGER wake_triggers_no_update BEFORE UPDATE ON wake_triggers BEGIN SELECT RAISE(ABORT, 'wake triggers are append only'); END;
CREATE TRIGGER web_collected_items_no_delete BEFORE DELETE ON web_collected_items BEGIN SELECT RAISE(ABORT, 'web collected items are append only'); END;
CREATE TRIGGER web_collected_items_no_update BEFORE UPDATE ON web_collected_items BEGIN SELECT RAISE(ABORT, 'web collected items are append only'); END;
