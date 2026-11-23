CREATE TABLE instrument_mappings (
    instrument_id TEXT PRIMARY KEY CHECK (instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'),
    market TEXT NOT NULL CHECK (market IN ('KR', 'US')),
    symbol TEXT NOT NULL CHECK (instr(symbol, char(0)) = 0 AND length(symbol) BETWEEN 1 AND 32 AND symbol GLOB '[A-Za-z0-9]*' AND symbol NOT GLOB '*[^A-Za-z0-9.-]*'),
    isin TEXT NOT NULL CHECK (instr(isin, char(0)) = 0 AND length(isin) BETWEEN 1 AND 64 AND isin NOT GLOB '*[^!-~]*'),
    status TEXT NOT NULL CHECK (status IN ('active', 'retired')),
    UNIQUE (market, symbol, isin)
) STRICT;
CREATE UNIQUE INDEX instrument_mappings_symbol_key ON instrument_mappings(market, upper(symbol));
CREATE UNIQUE INDEX instrument_mappings_isin_key ON instrument_mappings(upper(isin));
CREATE TRIGGER instrument_mappings_no_delete BEFORE DELETE ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'instrument mappings cannot be deleted'); END;
CREATE TRIGGER instrument_mappings_retire_only BEFORE UPDATE ON instrument_mappings WHEN OLD.status != 'active' OR NEW.status != 'retired' OR NEW.instrument_id != OLD.instrument_id OR NEW.market != OLD.market OR NEW.symbol != OLD.symbol OR NEW.isin != OLD.isin BEGIN SELECT RAISE(ABORT, 'instrument mapping can only transition from active to retired'); END;
CREATE TRIGGER instrument_mappings_no_replace BEFORE INSERT ON instrument_mappings WHEN EXISTS (SELECT 1 FROM instrument_mappings WHERE instrument_id = NEW.instrument_id OR (market = NEW.market AND upper(symbol) = upper(NEW.symbol)) OR upper(isin) = upper(NEW.isin)) BEGIN SELECT RAISE(ABORT, 'instrument mapping cannot replace existing history'); END;
CREATE TRIGGER instrument_mappings_insert_active_only BEFORE INSERT ON instrument_mappings WHEN NEW.status != 'active' BEGIN SELECT RAISE(ABORT, 'instrument mapping must start active'); END;
