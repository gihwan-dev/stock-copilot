pub(crate) const DELETE: &str = "DELETE FROM briefings";
pub(crate) const CORRUPT: &str = "UPDATE briefings SET payload = '3:bad'";
pub(crate) const ZERO_SEQ: &str =
    "INSERT INTO briefings(id_key, seq, payload) VALUES ('x', 0, 'x')";
