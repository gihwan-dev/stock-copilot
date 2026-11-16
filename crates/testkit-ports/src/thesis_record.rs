use domain::{HistoryAction, HistoryEntry, Thesis, ThesisError, ThesisPatch, UserDecision};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThesisRecord(Vec<HistoryEntry>);
impl ThesisRecord {
    #[must_use]
    pub fn from_thesis(thesis: &Thesis) -> Self {
        Self(thesis.history().to_vec())
    }
    pub fn restore(&self) -> Result<Thesis, ThesisError> {
        let mut thesis = Thesis::new();
        for entry in &self.0 {
            let base = thesis.version();
            match entry.action() {
                HistoryAction::Change(change) => {
                    thesis
                        .apply_batch(base, ThesisPatch::try_new(vec![change.as_ref().clone()])?)?;
                }
                HistoryAction::Decision(target, UserDecision::Confirm, reason) => {
                    thesis.confirm_by_user(base, *target, reason.clone())?;
                }
                HistoryAction::Decision(target, UserDecision::Reject, reason) => {
                    thesis.reject_by_user(base, *target, reason.clone())?;
                }
            }
        }
        Ok(thesis)
    }
}
