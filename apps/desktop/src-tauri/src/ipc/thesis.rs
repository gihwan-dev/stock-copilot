use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use application::{SystemPortError, ThesisEntryTime, ThesisHistoryTimes, ThesisRepository};
use domain::{
    ClaimId, ClaimVerdict, HistoryAction, InstrumentId, Symbol, Thesis, ThesisOperation,
    ThesisVersion, Timestamp, UserDecision,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use super::state::BoxFuture;
use crate::holdings_link::InstrumentDirectory;

#[cfg(test)]
mod tests;

pub trait ThesisNotes: Send + Sync {
    fn thesis(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Thesis>, SystemPortError>>;
    fn times(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<ThesisEntryTime>, SystemPortError>>;
}

struct StoredNotes<R>(Arc<R>);

impl<R: ThesisRepository + ThesisHistoryTimes + 'static> ThesisNotes for StoredNotes<R> {
    fn thesis(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Thesis>, SystemPortError>> {
        Box::pin(async move { ThesisRepository::load(self.0.as_ref(), &instrument).await })
    }

    fn times(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<ThesisEntryTime>, SystemPortError>> {
        Box::pin(async move { self.0.entry_times(&instrument).await })
    }
}

#[derive(Clone)]
pub struct ThesisIpcState(Arc<dyn ThesisNotes>);

impl fmt::Debug for ThesisIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ThesisIpcState")
    }
}

impl ThesisIpcState {
    #[must_use]
    pub fn from_repository<R: ThesisRepository + ThesisHistoryTimes + 'static>(
        repository: Arc<R>,
    ) -> Self {
        Self(Arc::new(StoredNotes(repository)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetThesisRequest {
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiClaimStatus {
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "needs-review")]
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiThesisChange {
    #[serde(rename = "claim-added")]
    ClaimAdded,
    #[serde(rename = "claim-judged")]
    ClaimJudged,
    #[serde(rename = "criterion-added")]
    CriterionAdded,
    #[serde(rename = "criterion-judged")]
    CriterionJudged,
    #[serde(rename = "user-confirmed")]
    UserConfirmed,
    #[serde(rename = "user-rejected")]
    UserRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiClaimVerdict {
    #[serde(rename = "intact")]
    Intact,
    #[serde(rename = "weakened")]
    Weakened,
    #[serde(rename = "broken")]
    Broken,
    #[serde(rename = "unverifiable")]
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiThesisClaim {
    pub id: String,
    pub text: String,
    pub status: UiClaimStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiThesisRevision {
    pub sequence: String,
    pub at: Option<String>,
    pub change: UiThesisChange,
    pub subject: Option<String>,
    pub verdict: Option<UiClaimVerdict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiThesisView {
    pub claims: Vec<UiThesisClaim>,
    pub history: Vec<UiThesisRevision>,
}

const fn verdict(value: ClaimVerdict) -> UiClaimVerdict {
    match value {
        ClaimVerdict::Intact => UiClaimVerdict::Intact,
        ClaimVerdict::Weakened => UiClaimVerdict::Weakened,
        ClaimVerdict::Broken => UiClaimVerdict::Broken,
        ClaimVerdict::Unverifiable => UiClaimVerdict::Unverifiable,
    }
}

fn pending_targets(thesis: &Thesis) -> Vec<ClaimId> {
    thesis
        .pending()
        .iter()
        .filter_map(|entry| match entry.action() {
            HistoryAction::Change(change) => match change.operation() {
                ThesisOperation::AddClaim(claim) => Some(claim.id().clone()),
                ThesisOperation::Judge(id, _) => Some(id.clone()),
                ThesisOperation::AddKillCriterion(_) | ThesisOperation::JudgeCriterion(..) => None,
            },
            HistoryAction::Decision(..) => None,
        })
        .collect()
}

fn statements(thesis: &Thesis) -> BTreeMap<ClaimId, String> {
    let mut known = BTreeMap::new();
    for entry in thesis.history() {
        if let HistoryAction::Change(change) = entry.action()
            && let ThesisOperation::AddClaim(claim) = change.operation()
        {
            let _previous = known.insert(
                claim.id().clone(),
                claim.statement().as_untrusted_str().to_owned(),
            );
        }
    }
    known
}

fn claims(thesis: &Thesis) -> Vec<UiThesisClaim> {
    let pending = pending_targets(thesis);
    let status = |id: &ClaimId| {
        if pending.contains(id) {
            UiClaimStatus::NeedsReview
        } else {
            UiClaimStatus::Confirmed
        }
    };
    let mut rows: Vec<UiThesisClaim> = thesis
        .claims()
        .iter()
        .map(|judged| UiThesisClaim {
            id: judged.claim().id().as_str().to_owned(),
            text: judged.claim().statement().as_untrusted_str().to_owned(),
            status: status(judged.claim().id()),
        })
        .collect();
    for entry in thesis.pending() {
        if let HistoryAction::Change(change) = entry.action()
            && let ThesisOperation::AddClaim(claim) = change.operation()
            && !rows.iter().any(|row| row.id == claim.id().as_str())
        {
            rows.push(UiThesisClaim {
                id: claim.id().as_str().to_owned(),
                text: claim.statement().as_untrusted_str().to_owned(),
                status: UiClaimStatus::NeedsReview,
            });
        }
    }
    rows
}

fn revision(
    action: &HistoryAction,
    known: &BTreeMap<ClaimId, String>,
) -> (UiThesisChange, Option<String>, Option<UiClaimVerdict>) {
    match action {
        HistoryAction::Change(change) => match change.operation() {
            ThesisOperation::AddClaim(claim) => (
                UiThesisChange::ClaimAdded,
                Some(claim.statement().as_untrusted_str().to_owned()),
                None,
            ),
            ThesisOperation::Judge(id, judged) => (
                UiThesisChange::ClaimJudged,
                known.get(id).cloned(),
                Some(verdict(*judged)),
            ),
            ThesisOperation::AddKillCriterion(_) => (UiThesisChange::CriterionAdded, None, None),
            ThesisOperation::JudgeCriterion(..) => (UiThesisChange::CriterionJudged, None, None),
        },
        HistoryAction::Decision(_, UserDecision::Confirm, _) => {
            (UiThesisChange::UserConfirmed, None, None)
        }
        HistoryAction::Decision(_, UserDecision::Reject, _) => {
            (UiThesisChange::UserRejected, None, None)
        }
    }
}

#[must_use]
pub fn to_thesis_view(thesis: &Thesis, times: &[ThesisEntryTime]) -> UiThesisView {
    let known = statements(thesis);
    let recorded: BTreeMap<ThesisVersion, Timestamp> = times
        .iter()
        .map(|time| (time.sequence, time.recorded_at))
        .collect();
    let history = thesis
        .history()
        .iter()
        .rev()
        .map(|entry| {
            let (change, subject, judged) = revision(entry.action(), &known);
            UiThesisRevision {
                sequence: entry.sequence().as_usize().to_string(),
                at: recorded.get(&entry.sequence()).copied().and_then(kst_iso),
                change,
                subject,
                verdict: judged,
            }
        })
        .collect();
    UiThesisView {
        claims: claims(thesis),
        history,
    }
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn storage(failure: &SystemPortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

pub async fn load_thesis(
    state: &ThesisIpcState,
    directory: &InstrumentDirectory,
    symbol: &Symbol,
) -> Result<UiThesisView, IpcError> {
    let entry = directory.entry(symbol).ok_or(error(IpcErrorKind::NoData))?;
    let instrument = entry.master.row().instrument_id.clone();
    let Some(thesis) = state
        .0
        .thesis(instrument.clone())
        .await
        .map_err(|failure| storage(&failure))?
    else {
        return Ok(UiThesisView {
            claims: Vec::new(),
            history: Vec::new(),
        });
    };
    let times = state
        .0
        .times(instrument)
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(to_thesis_view(&thesis, &times))
}

#[tauri::command]
#[specta::specta]
pub async fn get_thesis<R: Runtime>(
    app: AppHandle<R>,
    request: GetThesisRequest,
) -> Result<UiThesisView, IpcError> {
    let symbol =
        Symbol::try_new(&request.symbol).map_err(|_invalid| error(IpcErrorKind::UserFixable))?;
    let state = app
        .try_state::<ThesisIpcState>()
        .map(|managed| managed.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
    load_thesis(&state, &directory, &symbol).await
}
