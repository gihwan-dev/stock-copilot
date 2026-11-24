pub(crate) const UPDATE_TRIGGER: &str = "UPDATE wake_triggers SET instrument = 'x'";
pub(crate) const DELETE_TRIGGER: &str = "DELETE FROM wake_triggers";
pub(crate) const DISCLOSURE_WITHOUT_RECEIPT: &str = "INSERT INTO wake_triggers(kind, instrument, recorded_day, recorded_seconds, recorded_nanos, recorded_offset) VALUES ('disclosure', 'a', 20261125, 0, 0, 0)";
pub(crate) const PRICE_WITH_GRADE: &str = "INSERT INTO wake_triggers(kind, instrument, grade, recorded_day, recorded_seconds, recorded_nanos, recorded_offset) VALUES ('price_move', 'a', 'high', 20261125, 0, 0, 0)";
pub(crate) const TRIGGER_COUNT: &str = "SELECT count(*) FROM wake_triggers";
pub(crate) const UPDATE_WEB: &str = "UPDATE web_collected_items SET subject = 'x'";
pub(crate) const DELETE_WEB: &str = "DELETE FROM web_collected_items";
pub(crate) const UNKNOWN_SUBJECT: &str =
    "INSERT INTO web_collected_items(subject_kind, subject, payload) VALUES ('other', 'a', 'x')";
pub(crate) const CORRUPT_WEB: &str = "INSERT INTO web_collected_items(subject_kind, subject, payload) VALUES ('symbol', 'BAD', '3:bad')";
