pub(crate) const CORRUPT_FX: &str =
    "UPDATE market_fx_rates SET source = 'future' WHERE pair = 'USD/KRW' AND kind = 'mid'";
pub(crate) const CORRUPT_QUOTE: &str =
    "UPDATE market_quotes SET observed_nanos = 1000000000 WHERE instrument = 'asset-one'";
pub(crate) const BLOCK_QUOTE: &str = "CREATE TRIGGER reject_market_quote BEFORE INSERT ON market_quotes BEGIN SELECT RAISE(ABORT, 'rejected'); END";
pub(crate) const CORRUPT_SERVER_TIME: &str =
    "UPDATE market_quotes SET last_server_nanos = 1000000000 WHERE instrument = 'asset-one'";
pub(crate) const CORRUPT_SERVER_ORDER: &str =
    "UPDATE market_quotes SET last_server_seconds = 1 WHERE instrument = 'asset-one'";
pub(crate) const UNBLOCK_QUOTE: &str = "DROP TRIGGER reject_market_quote";
pub(crate) const PARTIAL_SERVER_TIME: &str =
    "UPDATE market_quotes SET last_server_nanos = NULL WHERE instrument = 'asset-one'";
pub(crate) const SERVER_TIME_DISAGREES: &str = "UPDATE market_quotes SET last_server_seconds = observed_seconds + 1 WHERE instrument = 'asset-one'";
pub(crate) const SERVER_TIME_TOO_FAR_AHEAD: &str = "UPDATE market_quotes SET last_server_seconds = collected_seconds + 2, last_server_nanos = collected_nanos + 1 WHERE instrument = 'asset-one'";
pub(crate) const SERVER_TIME_OTHER_OFFSET: &str =
    "UPDATE market_quotes SET last_server_offset = 32400 WHERE instrument = 'asset-one'";
