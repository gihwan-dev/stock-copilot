CREATE TABLE cash_ledger (
    id INTEGER PRIMARY KEY,
    amount TEXT NOT NULL CHECK (length(amount) > 0 AND amount GLOB '[0-9-]*'),
    currency TEXT NOT NULL
) STRICT;

CREATE INDEX cash_ledger_by_currency ON cash_ledger (currency);
