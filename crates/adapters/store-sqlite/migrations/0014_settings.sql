CREATE TABLE settings (
    name TEXT PRIMARY KEY CHECK (name IN ('investor_profile', 'risk_settings', 'toss_spec_hash')),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER settings_no_delete BEFORE DELETE ON settings BEGIN SELECT RAISE(ABORT, 'settings are replaced, not deleted'); END;
