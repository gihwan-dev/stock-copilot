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
