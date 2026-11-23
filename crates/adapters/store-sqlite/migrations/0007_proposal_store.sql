CREATE TABLE proposal_consumptions (
    proposal_id TEXT PRIMARY KEY
) STRICT;
CREATE TRIGGER proposal_consumptions_no_update BEFORE UPDATE ON proposal_consumptions BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
CREATE TRIGGER proposal_consumptions_no_delete BEFORE DELETE ON proposal_consumptions BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
CREATE TRIGGER proposal_consumptions_no_replace BEFORE INSERT ON proposal_consumptions WHEN EXISTS (SELECT 1 FROM proposal_consumptions WHERE proposal_id = NEW.proposal_id) BEGIN SELECT RAISE(ABORT, 'proposal consumption is append only'); END;
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
CREATE INDEX proposal_validation_reports_lookup ON proposal_validation_reports(proposal_id, id);
