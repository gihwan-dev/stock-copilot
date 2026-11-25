pub(crate) const UPDATE_OCCURRENCE: &str =
    "UPDATE signal_occurrences SET occurrence_payload = 'bad'";
pub(crate) const DELETE_OCCURRENCE: &str = "DELETE FROM signal_occurrences";
pub(crate) const UPDATE_DISCARDED: &str = "UPDATE signal_discarded_submissions SET task_id = 'bad'";
pub(crate) const DELETE_DISCARDED: &str = "DELETE FROM signal_discarded_submissions";
pub(crate) const DELETE_DRAFT: &str = "DELETE FROM proposal_drafts";
pub(crate) const MUTATE_DRAFT_EXPIRY: &str = "UPDATE proposal_drafts SET expires_seconds = 1";
pub(crate) const REVERT_DRAFT_STATE: &str =
    "UPDATE proposal_drafts SET state = 'current' WHERE state = 'superseded'";
pub(crate) const INSERT_CORRUPT_STATE: &str = "INSERT INTO signal_states(instrument, revision, current_payload) VALUES ('corrupt-sig', 1, 'bad')";
pub(crate) const INSERT_CORRUPT_DRAFT: &str = "INSERT INTO proposal_drafts(proposal_id, mode, instrument, account, expires_seconds, expires_nanos, expires_offset, state, snapshot_payload, evidence_payload) VALUES ('corrupt-draft', 'standalone', 'inst', 1, 100, 0, 0, 'current', NULL, 'bad')";
pub(crate) const INSERT_DRAFT_ROW: &str = "INSERT INTO proposal_drafts(proposal_id, mode, instrument, account, expires_seconds, expires_nanos, expires_offset, state, snapshot_payload, evidence_payload) VALUES (?1, 'standalone', ?2, 1, 0, 0, 0, 'current', NULL, '{}')";
