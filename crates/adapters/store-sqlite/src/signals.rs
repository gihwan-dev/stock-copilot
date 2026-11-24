mod codec;
pub(crate) mod context_codec;
mod draft_ops;
mod signal_ops;

use crate::{Database, sql::signals as sql};
use application::{
    ConditionalDraftOutcome, CurrentDraftOutcome, DraftCheck, DraftStoreCandidate,
    ProposalRepository, SignalCommitOutcome, SignalInvalidationOutcome,
    SignalInvalidationRepository, SignalMutation, SignalRepository, SignalSnapshot,
    SignalSourceFacts, SystemPortError, WriteCompleted,
};
use codec::{decode_sources, encode_master, encode_sources};
use domain::{
    AccountId, Draft, DraftRestoreEvidence, HistoricalSignal, HistoricalSignalOccurrence,
    InstrumentId, IssuedEstimateTaskId, MasterInstrument, Proposal, ProposalId,
    ResolvedOrderInstrument,
};
use draft_ops::{
    OwnedDraftCandidate, OwnedDraftCheck, current_draft_tx, read_draft_tx, store_current_draft_tx,
    store_draft_standalone_tx,
};
use rusqlite::{OptionalExtension, params};
use signal_ops::{
    commit_signal_tx, invalid_data, invalidate_signal_tx, load_occurrences_for, load_snapshot,
};

#[derive(Debug)]
pub struct SignalStore<'database> {
    database: &'database Database,
}

impl<'database> SignalStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn save_master(
        &self,
        master: &MasterInstrument,
    ) -> Result<WriteCompleted, SystemPortError> {
        let instrument_id = master.row().instrument_id.as_str().to_owned();
        let payload = encode_master(master);
        self.database
            .write(move |tx| {
                tx.execute(sql::UPSERT_MASTER, params![instrument_id, payload])?;
                Ok(WriteCompleted)
            })
            .await
    }

    pub async fn save_sources(
        &self,
        task: &IssuedEstimateTaskId,
        facts: &SignalSourceFacts,
    ) -> Result<WriteCompleted, SystemPortError> {
        let task_id = task.as_estimate_id().as_str().to_owned();
        let payload = encode_sources(facts);
        self.database
            .write(move |tx| {
                tx.execute(sql::UPSERT_SOURCES, params![task_id, payload])?;
                Ok(WriteCompleted)
            })
            .await
    }

    pub async fn select_account(
        &self,
        account: &AccountId,
    ) -> Result<WriteCompleted, SystemPortError> {
        let account_id = account.as_i64();
        self.database
            .write(move |tx| {
                tx.execute(sql::UPSERT_SELECTED_ACCOUNT, [account_id])?;
                Ok(WriteCompleted)
            })
            .await
    }
}

impl SignalRepository for SignalStore<'_> {
    async fn snapshot(&self, instrument: &InstrumentId) -> Result<SignalSnapshot, SystemPortError> {
        let owned = instrument.clone();
        self.database
            .read(move |connection| load_snapshot(connection, &owned))
            .await?
    }

    async fn commit(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        mutation: &SignalMutation,
    ) -> Result<SignalCommitOutcome, SystemPortError> {
        let owned_instrument = instrument.clone();
        let owned_expected = expected.clone();
        let owned_mutation = mutation.clone();
        self.database
            .write(move |tx| {
                commit_signal_tx(tx, &owned_instrument, &owned_expected, &owned_mutation)
            })
            .await?
    }

    async fn occurrences(
        &self,
        instrument: &InstrumentId,
        identity: &ResolvedOrderInstrument,
    ) -> Result<Vec<HistoricalSignalOccurrence>, SystemPortError> {
        let owned_instrument = instrument.clone();
        let owned_identity = identity.clone();
        self.database
            .read(move |connection| {
                load_occurrences_for(connection, &owned_instrument, &owned_identity)
            })
            .await?
    }

    async fn sources(
        &self,
        task: &IssuedEstimateTaskId,
    ) -> Result<SignalSourceFacts, SystemPortError> {
        let task_id = task.as_estimate_id().as_str().to_owned();
        self.database
            .read(move |connection| {
                let row: Option<String> = connection
                    .query_row(sql::SOURCES_BY_TASK, [&task_id], |record| record.get(0))
                    .optional()?;
                match row {
                    Some(payload) => Ok(Ok(decode_sources(&payload)?)),
                    None => Ok(Err(invalid_data())),
                }
            })
            .await?
    }

    async fn current(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<HistoricalSignal>, SystemPortError> {
        let snapshot = self.snapshot(instrument).await?;
        Ok(snapshot
            .current
            .into_iter()
            .map(|entry| entry.signal)
            .collect())
    }
}

impl SignalInvalidationRepository for SignalStore<'_> {
    async fn invalidate(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
    ) -> Result<SignalInvalidationOutcome, SystemPortError> {
        let owned_instrument = instrument.clone();
        let owned_expected = expected.clone();
        self.database
            .write(move |tx| invalidate_signal_tx(tx, &owned_instrument, &owned_expected))
            .await?
    }
}

impl ProposalRepository for SignalStore<'_> {
    async fn store_draft(
        &self,
        proposal: &Proposal<Draft>,
        evidence: &DraftRestoreEvidence,
        master: &MasterInstrument,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_proposal = proposal.clone();
        let owned_evidence = evidence.clone();
        let owned_master = master.clone();
        self.database
            .write(move |tx| {
                store_draft_standalone_tx(tx, &owned_proposal, &owned_evidence, &owned_master)
            })
            .await?
    }

    async fn draft(
        &self,
        proposal: &ProposalId,
        master: &MasterInstrument,
    ) -> Result<Option<Proposal<Draft>>, SystemPortError> {
        let owned_id = proposal.clone();
        let owned_master = master.clone();
        self.database
            .read(move |connection| read_draft_tx(connection, &owned_id, &owned_master))
            .await?
    }

    async fn current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        check: DraftCheck<'_>,
    ) -> Result<CurrentDraftOutcome, SystemPortError> {
        let owned_instrument = instrument.clone();
        let owned_expected = expected.clone();
        let owned_check = OwnedDraftCheck {
            account: check.account.clone(),
            at: check.at,
            observation: check.observation.clone(),
            master: check.master.clone(),
        };
        self.database
            .write(move |tx| current_draft_tx(tx, &owned_instrument, &owned_expected, &owned_check))
            .await?
    }

    async fn store_current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        candidate: DraftStoreCandidate<'_>,
        check: DraftCheck<'_>,
    ) -> Result<ConditionalDraftOutcome, SystemPortError> {
        let owned_instrument = instrument.clone();
        let owned_expected = expected.clone();
        let owned_candidate = OwnedDraftCandidate {
            proposal: candidate.proposal.clone(),
            evidence: candidate.evidence.clone(),
        };
        let owned_check = OwnedDraftCheck {
            account: check.account.clone(),
            at: check.at,
            observation: check.observation.clone(),
            master: check.master.clone(),
        };
        self.database
            .write(move |tx| {
                store_current_draft_tx(
                    tx,
                    &owned_instrument,
                    &owned_expected,
                    &owned_candidate,
                    &owned_check,
                )
            })
            .await?
    }
}
