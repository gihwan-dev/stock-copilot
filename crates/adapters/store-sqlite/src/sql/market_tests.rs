pub(crate) const CORRUPT_FX: &str =
    "UPDATE market_fx_rates SET source = 'future' WHERE pair = 'USD/KRW' AND kind = 'mid'";
pub(crate) const CORRUPT_QUOTE: &str =
    "UPDATE market_quotes SET observed_nanos = 1000000000 WHERE instrument = 'asset-one'";
pub(crate) const BLOCK_QUOTE: &str = "CREATE TRIGGER reject_market_quote BEFORE INSERT ON market_quotes BEGIN SELECT RAISE(ABORT, 'rejected'); END";
