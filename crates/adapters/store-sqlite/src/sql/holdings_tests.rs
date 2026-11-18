pub(crate) const BAD_DECIMAL: &str = "UPDATE snapshot_holdings SET quantity = '1e2'";
pub(crate) const NEGATIVE_QUANTITY: &str = "UPDATE snapshot_holdings SET quantity = '-1'";
pub(crate) const FAIL_DELETE: &str = "CREATE TRIGGER fail_snapshot_delete BEFORE DELETE ON holdings_snapshots WHEN OLD.id = 2 BEGIN SELECT RAISE(ABORT, 'injected'); END";

pub(crate) const GAP: &str = "UPDATE migration_ledger SET number = 6 WHERE number = 1";
pub(crate) const FUTURE: &str = "PRAGMA user_version = 6";

pub(crate) const BAD_NAME: &str = "UPDATE migration_ledger SET name = 'wrong.sql' WHERE number = 1";
pub(crate) const STORED_TEXT: &str = "SELECT quantity, acquisition_price, current_price, valuation, profit, return_rate, daily_profit, daily_rate FROM snapshot_holdings";
pub(crate) const OTHER_CORRUPT: &str =
    "INSERT INTO holdings_snapshots(account, seconds, nanos, offset) VALUES (2, 1728572400, 0, 1)";
