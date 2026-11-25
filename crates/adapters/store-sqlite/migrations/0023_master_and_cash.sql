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
CREATE TABLE snapshot_cash (
    snapshot INTEGER NOT NULL REFERENCES holdings_snapshots(id) ON DELETE CASCADE,
    currency TEXT NOT NULL CHECK (currency IN ('KRW', 'USD')),
    amount TEXT NOT NULL CHECK (instr(amount, char(0)) = 0 AND amount NOT GLOB '*[^0-9.-]*' AND amount GLOB '*[0-9]*' AND amount NOT GLOB '?*-*' AND amount NOT GLOB '*.*.*' AND amount NOT GLOB '.*' AND amount NOT GLOB '-.*' AND amount NOT GLOB '*.'),
    seconds INTEGER NOT NULL,
    nanos INTEGER NOT NULL CHECK (nanos >= 0 AND nanos < 1000000000),
    offset INTEGER NOT NULL,
    PRIMARY KEY (snapshot, currency)
) STRICT;
