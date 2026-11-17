pub(crate) const FAIL_SOURCE: &str = "CREATE TRIGGER fail_transaction_source BEFORE INSERT ON transaction_sources BEGIN SELECT RAISE(ABORT, 'injected'); END";
pub(crate) const FAIL_COMPLETION: &str = "CREATE TRIGGER fail_transaction_completion AFTER UPDATE ON transaction_full_inspections BEGIN SELECT RAISE(ABORT, 'injected'); END";
pub(crate) const BAD_CURRENCY: &str = "UPDATE transaction_rows SET currency = 'EUR'";
pub(crate) const BAD_QUANTITY: &str = "UPDATE transaction_rows SET quantity = '-9'";
pub(crate) const BAD_TIME: &str = "UPDATE transaction_rows SET transaction_time = 'broken'";
pub(crate) const BAD_POSITION: &str = "UPDATE transaction_rows SET position = 10";
pub(crate) const REMOVE_SOURCE: &str = "DELETE FROM transaction_sources";
pub(crate) const REMOVE_ROW: &str = "DELETE FROM transaction_rows WHERE scope = 'after'";
pub(crate) const BAD_HEADER: &str = "UPDATE transaction_entries SET payload = '1:x'";
pub(crate) const BAD_MODE: &str = "UPDATE transaction_entries SET mode = 'summary'";
pub(crate) const BAD_COMPLETION: &str =
    "UPDATE transaction_full_inspections SET completed_at = 'broken'";

pub(crate) const REVERSE_FAILED_TIME: &str = "UPDATE transaction_entries SET payload = substr(payload, 1, 3) || '1:9' || substr(payload, 8) WHERE id = 2";
pub(crate) const REVERSE_FIRST_TIME: &str = "UPDATE transaction_entries SET payload = substr(payload, 1, 3) || '1:9' || substr(payload, 8) WHERE id = 1";
pub(crate) const PARTIAL_WITH_CHANGED_STATE: &str = "UPDATE transaction_entries SET payload = replace(payload, '8:complete', '7:partial') WHERE mode = 'summary'";
pub(crate) const HISTORY_COUNTS: &str = "SELECT (SELECT COUNT(*) FROM transaction_entries WHERE mode = 'history'), (SELECT COUNT(*) FROM transaction_rows JOIN transaction_entries ON entry = transaction_entries.id WHERE mode = 'history')";
pub(crate) const CORRUPT_HISTORY: &str = "CREATE TRIGGER corrupt_history AFTER INSERT ON transaction_entries WHEN NEW.mode = 'history' BEGIN UPDATE transaction_entries SET payload = 'broken' WHERE id = NEW.id; END";
