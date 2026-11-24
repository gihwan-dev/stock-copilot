pub(crate) const UPDATE_SKIP: &str = "UPDATE signal_skips SET reason = NULL";
pub(crate) const DELETE_SKIP: &str = "DELETE FROM signal_skips";
pub(crate) const MULTILINE_SKIP: &str = "INSERT INTO signal_skips(instrument, reason, skipped_seconds, skipped_nanos, skipped_offset) VALUES ('a', 'x' || char(10) || 'y', 0, 0, 0)";
pub(crate) const DUPLICATE_WATCH: &str = "INSERT INTO watchlist_entries(position, instrument, added_seconds, added_nanos, added_offset) VALUES (150, 'instrument-a', 0, 0, 0)";
pub(crate) const OVER_CAPACITY: &str = "INSERT INTO watchlist_entries(position, instrument, added_seconds, added_nanos, added_offset) VALUES (200, 'instrument-z', 0, 0, 0)";
pub(crate) const CORRUPT_WATCH: &str = "INSERT INTO watchlist_entries(position, instrument, added_seconds, added_nanos, added_offset) VALUES (199, '', 0, 0, 0)";
