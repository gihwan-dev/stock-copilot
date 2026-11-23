mod codec;

use crate::{Database, sql::thesis as sql};
use application::{SystemPortError, ThesisRepository, WriteCompleted};
use domain::{HistoryAction, HistoryEntry, InstrumentId, Thesis, ThesisPatch, UserDecision};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

#[derive(Debug)]
pub struct ThesisStore<'database> {
    database: &'database Database,
}
impl<'database> ThesisStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
fn invalid<E>(_: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
fn load(connection: &Connection, instrument: &InstrumentId) -> rusqlite::Result<Option<Thesis>> {
    let exists: Option<String> = connection
        .query_row(sql::NOTE, [instrument.as_str()], |row| row.get(0))
        .optional()?;
    let rows = connection
        .prepare(sql::HISTORY)?
        .query_map([instrument.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let Some(stored_instrument) = exists else {
        return if rows.is_empty() {
            Ok(None)
        } else {
            Err(invalid(()))
        };
    };
    if InstrumentId::try_new(&stored_instrument).map_err(invalid)? != *instrument {
        return Err(invalid(()));
    }
    let mut thesis = Thesis::new();
    for (sequence, payload) in rows {
        let base = thesis.version();
        let expected = base
            .as_usize()
            .checked_add(1)
            .and_then(|number| i64::try_from(number).ok())
            .ok_or_else(|| invalid(()))?;
        if sequence != expected {
            return Err(invalid(()));
        }
        let action = codec::decode(&payload)?;
        match action {
            HistoryAction::Change(change) => thesis
                .apply_batch(base, ThesisPatch::try_new(vec![*change]).map_err(invalid)?)
                .map_err(invalid)?,
            HistoryAction::Decision(target, UserDecision::Confirm, reason) => thesis
                .confirm_by_user(base, target, reason)
                .map_err(invalid)?,
            HistoryAction::Decision(target, UserDecision::Reject, reason) => thesis
                .reject_by_user(base, target, reason)
                .map_err(invalid)?,
        }
        let entry = thesis.history().last().ok_or_else(|| invalid(()))?;
        if codec::encode(entry.action()) != payload {
            return Err(invalid(()));
        }
    }
    Ok(Some(thesis))
}
fn store(
    transaction: &Transaction<'_>,
    instrument: &InstrumentId,
    thesis: &Thesis,
) -> rusqlite::Result<()> {
    let existing = load(transaction, instrument)?;
    let start = if let Some(old) = &existing {
        if !thesis.history().starts_with(old.history()) {
            return Err(invalid(()));
        }
        old.history().len()
    } else {
        transaction.execute(sql::INSERT_NOTE, [instrument.as_str()])?;
        0
    };
    for entry in thesis.history().iter().skip(start) {
        transaction.execute(
            sql::INSERT_HISTORY,
            params![
                instrument.as_str(),
                i64::try_from(*entry.sequence().as_usize()).map_err(invalid)?,
                codec::encode(entry.action())
            ],
        )?;
    }
    Ok(())
}
fn replay(history: Vec<HistoryEntry>) -> rusqlite::Result<Thesis> {
    let mut thesis = Thesis::new();
    for entry in history {
        let base = thesis.version();
        match entry.action() {
            HistoryAction::Change(change) => thesis
                .apply_batch(
                    base,
                    ThesisPatch::try_new(vec![change.as_ref().clone()]).map_err(invalid)?,
                )
                .map_err(invalid)?,
            HistoryAction::Decision(target, UserDecision::Confirm, reason) => thesis
                .confirm_by_user(base, *target, reason.clone())
                .map_err(invalid)?,
            HistoryAction::Decision(target, UserDecision::Reject, reason) => thesis
                .reject_by_user(base, *target, reason.clone())
                .map_err(invalid)?,
        }
        if thesis.history().last() != Some(&entry) {
            return Err(invalid(()));
        }
    }
    Ok(thesis)
}
impl ThesisRepository for ThesisStore<'_> {
    async fn store(
        &self,
        instrument: &InstrumentId,
        thesis: &Thesis,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_instrument = instrument.clone();
        let history = thesis.history().to_vec();
        self.database
            .write(move |transaction| {
                store(transaction, &owned_instrument, &replay(history)?)?;
                Ok(WriteCompleted)
            })
            .await
    }
    async fn load(&self, instrument: &InstrumentId) -> Result<Option<Thesis>, SystemPortError> {
        let owned_instrument = instrument.clone();
        self.database
            .read(move |connection| load(connection, &owned_instrument))
            .await
    }
}
