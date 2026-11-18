CREATE INDEX transaction_entries_account_id ON transaction_entries(account, id);
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
    CHECK ((observed_seconds IS NULL AND observed_nanos IS NULL AND observed_offset IS NULL) OR (observed_seconds IS NOT NULL AND observed_nanos IS NOT NULL AND observed_offset IS NOT NULL))
) STRICT;
CREATE TABLE migration_ledger (
    number INTEGER PRIMARY KEY CHECK (number > 0),
    name TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    applied_at INTEGER NOT NULL
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
