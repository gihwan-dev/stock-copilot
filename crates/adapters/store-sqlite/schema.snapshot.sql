CREATE INDEX transaction_entries_account_id ON transaction_entries(account, id);
CREATE TABLE holdings_snapshots (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    UNIQUE (account, seconds, nanos)
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
