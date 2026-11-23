use super::{
    codec::{Fields, invalid},
    rows::{self, RowEvidence, RowTarget},
    tags::{
        get_access_denial, get_broker_capability, get_http_status, get_incomplete_reason,
        get_missing_data, get_read_endpoint, get_request_rejection, get_response_mismatch,
        get_unavailable_cause, put_access_denial, put_broker_capability, put_http_status,
        put_incomplete_reason, put_missing_data, put_read_endpoint, put_request_rejection,
        put_response_mismatch, put_unavailable_cause,
    },
};
use application::{
    CompleteTransactionObservation, IncompleteTransactions, MasterNotReady, ObservedTransaction,
    ObservedTransactionBatch, PortError, PortErrorKind, QuarantinedTransaction, RawTransactionRow,
    RetryDelay, SystemFailure, SystemPort, SystemPortError, TransactionRead,
};
use domain::{
    AccountId, CompletionAttestation, Nanosecond, SettlementWindow, SpanSeconds, TimeSpan,
    TransactionCode, TransactionFilter, TransactionKind, UntrustedText,
};
use rusqlite::{Connection, Transaction};
fn put_filter(fields: &mut Fields, filter: &TransactionFilter) {
    match filter.selected_kinds() {
        None => fields.push(&("all")),
        Some(kinds) => {
            fields.push(&("only"));
            fields.push(&(kinds.len()));
            for kind in kinds {
                fields.push(&(kind.as_code().as_str()));
            }
        }
    }
}
fn filter(fields: &mut Fields) -> rusqlite::Result<TransactionFilter> {
    match fields.take()?.as_str() {
        "all" => Ok(TransactionFilter::all()),
        "only" => {
            let count: usize = fields.number()?;
            if count > 64 {
                return Err(invalid(()));
            }
            TransactionFilter::only(
                (0..count)
                    .map(|_| {
                        TransactionCode::try_new(&fields.take()?)
                            .map(TransactionKind::new)
                            .map_err(invalid)
                    })
                    .collect::<rusqlite::Result<_>>()?,
            )
            .map_err(invalid)
        }
        _ => Err(invalid(())),
    }
}
pub(super) fn put_window(fields: &mut Fields, window: SettlementWindow) {
    fields.put_date(window.from());
    fields.put_date(window.to());
}
pub(super) fn window(fields: &mut Fields) -> rusqlite::Result<SettlementWindow> {
    SettlementWindow::try_new(fields.date()?, fields.date()?).map_err(invalid)
}

fn put_retry(fields: &mut Fields, retry: RetryDelay) {
    match retry {
        RetryDelay::Unknown => fields.push(&"unknown"),
        RetryDelay::Known(span) => {
            fields.push(&"known");
            let (seconds, nanos) = span.as_parts();
            fields.push(&seconds.as_u64());
            fields.push(&nanos.as_u32());
        }
    }
}

fn retry(fields: &mut Fields) -> rusqlite::Result<RetryDelay> {
    match fields.take()?.as_str() {
        "unknown" => Ok(RetryDelay::Unknown),
        "known" => Ok(RetryDelay::Known(
            TimeSpan::try_new(
                SpanSeconds::new(fields.number()?),
                Nanosecond::new(fields.number()?),
            )
            .map_err(invalid)?,
        )),
        _ => Err(invalid(())),
    }
}

fn put_system_error(fields: &mut Fields, error: SystemPortError) {
    fields.push(&match error.port() {
        SystemPort::Repository => "repository",
        SystemPort::AuditLog => "audit-log",
        SystemPort::SecretStore => "secret-store",
        SystemPort::Notifier => "notifier",
    });
    match error.failure() {
        SystemFailure::PermissionDenied => fields.push(&"permission-denied"),
        SystemFailure::Unavailable(delay) => {
            fields.push(&"unavailable");
            put_retry(fields, delay);
        }
        SystemFailure::InvalidData => fields.push(&"invalid-data"),
        SystemFailure::RepositoryIntegrity => fields.push(&"repository-integrity"),
        SystemFailure::SecretIntegrity => fields.push(&"secret-integrity"),
    }
}

fn system_error(fields: &mut Fields) -> rusqlite::Result<SystemPortError> {
    let port = match fields.take()?.as_str() {
        "repository" => SystemPort::Repository,
        "audit-log" => SystemPort::AuditLog,
        "secret-store" => SystemPort::SecretStore,
        "notifier" => SystemPort::Notifier,
        _ => return Err(invalid(())),
    };
    let failure = match fields.take()?.as_str() {
        "permission-denied" => SystemFailure::PermissionDenied,
        "unavailable" => SystemFailure::Unavailable(retry(fields)?),
        "invalid-data" => SystemFailure::InvalidData,
        "repository-integrity" => SystemFailure::RepositoryIntegrity,
        "secret-integrity" => SystemFailure::SecretIntegrity,
        _ => return Err(invalid(())),
    };
    Ok(SystemPortError::new(port, failure))
}

fn put_error(fields: &mut Fields, error: PortError) {
    put_read_endpoint(fields, error.endpoint());
    put_http_status(fields, error.status());
    match error.kind() {
        PortErrorKind::MasterNotReady(reason) => {
            fields.push(&("master-not-ready"));
            match reason {
                MasterNotReady::StatusCoverage => fields.push(&"status-coverage"),
                MasterNotReady::IdentityContinuity => fields.push(&"identity-continuity"),
                MasterNotReady::Storage(system_error) => {
                    fields.push(&"storage");
                    put_system_error(fields, system_error);
                }
                MasterNotReady::Issuer(system_error) => {
                    fields.push(&"issuer");
                    put_system_error(fields, system_error);
                }
                MasterNotReady::MissingInstrument => fields.push(&"missing-instrument"),
            }
        }
        PortErrorKind::AccessDenied(value) => {
            fields.push(&("denied"));
            put_access_denial(fields, value);
        }
        PortErrorKind::Unavailable { cause, retry } => {
            fields.push(&("unavailable"));
            put_unavailable_cause(fields, cause);
            put_retry(fields, retry);
        }
        PortErrorKind::NotFound(value) => {
            fields.push(&("missing"));
            put_missing_data(fields, value);
        }
        PortErrorKind::InvalidResponse(value) => {
            fields.push(&("response"));
            put_response_mismatch(fields, value);
        }
        PortErrorKind::Unsupported(value) => {
            fields.push(&("unsupported"));
            put_broker_capability(fields, value);
        }
        PortErrorKind::InvalidRequest(value) => {
            fields.push(&("request"));
            put_request_rejection(fields, value);
        }
    }
}
fn error(fields: &mut Fields) -> rusqlite::Result<PortError> {
    let endpoint = get_read_endpoint(fields)?;
    let status = get_http_status(fields)?;
    let kind = match fields.take()?.as_str() {
        "master-not-ready" => PortErrorKind::MasterNotReady(match fields.take()?.as_str() {
            "status-coverage" => MasterNotReady::StatusCoverage,
            "identity-continuity" => MasterNotReady::IdentityContinuity,
            "storage" => MasterNotReady::Storage(system_error(fields)?),
            "issuer" => MasterNotReady::Issuer(system_error(fields)?),
            "missing-instrument" => MasterNotReady::MissingInstrument,
            _ => return Err(invalid(())),
        }),
        "denied" => PortErrorKind::AccessDenied(get_access_denial(fields)?),
        "unavailable" => {
            let cause = get_unavailable_cause(fields)?;
            let retry = retry(fields)?;
            PortErrorKind::Unavailable { cause, retry }
        }
        "missing" => PortErrorKind::NotFound(get_missing_data(fields)?),
        "response" => PortErrorKind::InvalidResponse(get_response_mismatch(fields)?),
        "unsupported" => PortErrorKind::Unsupported(get_broker_capability(fields)?),
        "request" => PortErrorKind::InvalidRequest(get_request_rejection(fields)?),
        _ => return Err(invalid(())),
    };
    Ok(PortError::new(endpoint, status, kind))
}

pub(super) fn put(fields: &mut Fields, observation: &Result<TransactionRead, PortError>) {
    match observation {
        Err(error) => {
            fields.push(&("error"));
            put_error(fields, *error);
        }
        Ok(TransactionRead::Complete(value)) => {
            fields.push(&("complete"));
            fields.push(&(value.account_id().as_i64()));
            put_window(fields, value.window());
            put_filter(fields, value.filter());
            fields.push(&(value.rows().observed_rows().len()));
        }
        Ok(TransactionRead::Incomplete(value)) => {
            fields.push(&("incomplete"));
            fields.push(&(value.account.as_i64()));
            put_window(fields, value.window);
            put_filter(fields, &value.filter);
            put_incomplete_reason(fields, value.reason);
            fields.push(&(value.received.len()));
            for batch in &value.received {
                fields.push(&(batch.observed_rows().len()));
            }
            put_quarantines(fields, &value.quarantined);
        }
    }
}
fn put_quarantines(fields: &mut Fields, quarantines: &[QuarantinedTransaction]) {
    fields.push(&quarantines.len());
    for quarantine in quarantines {
        let QuarantinedTransaction::UnknownCurrency(texts) = quarantine;
        fields.push(&texts.len());
        for text in texts {
            fields.push(text.as_untrusted_str());
        }
    }
}
pub(super) fn save(
    tx: &Transaction<'_>,
    entry: i64,
    observation: &Result<TransactionRead, PortError>,
) -> rusqlite::Result<usize> {
    let batches: Vec<&ObservedTransactionBatch> = match observation {
        Err(_) => Vec::new(),
        Ok(TransactionRead::Complete(value)) => vec![value.rows()],
        Ok(TransactionRead::Incomplete(value)) => value.received.iter().collect(),
    };
    let mut count = 0_usize;
    for (page, batch) in batches.iter().enumerate() {
        for (position, row) in batch.observed_rows().iter().enumerate() {
            rows::save(
                tx,
                RowTarget {
                    entry,
                    scope: "observed",
                    page,
                    position,
                },
                row.transaction(),
                RowEvidence {
                    first: None,
                    gone: None,
                    source: Some(row.source().as_str()),
                },
            )?;
            count = count.checked_add(1).ok_or_else(|| invalid(()))?;
        }
    }
    Ok(count)
}
fn batch(
    connection: &Connection,
    entry: i64,
    page: usize,
    count: usize,
) -> rusqlite::Result<ObservedTransactionBatch> {
    if count > 4096 {
        return Err(invalid(()));
    }
    let rows = rows::load(connection, entry, "observed", page, count)?
        .into_iter()
        .map(|row| {
            if row.first_seen.is_some() || row.disappeared_at.is_some() {
                return Err(invalid(()));
            }
            Ok(ObservedTransaction::new(
                row.transaction,
                RawTransactionRow::new(row.source.ok_or_else(|| invalid(()))?),
            ))
        })
        .collect::<rusqlite::Result<_>>()?;
    ObservedTransactionBatch::try_new(rows).map_err(invalid)
}
pub(super) fn get(
    connection: &Connection,
    entry: i64,
    fields: &mut Fields,
) -> rusqlite::Result<(Result<TransactionRead, PortError>, usize)> {
    let kind = fields.take()?;
    if kind == "error" {
        return Ok((Err(error(fields)?), 0));
    }
    let account = AccountId::new(fields.number()?);
    let window = window(fields)?;
    let filter = filter(fields)?;
    match kind.as_str() {
        "complete" => {
            let count = fields.number()?;
            let rows = batch(connection, entry, 0, count)?;
            Ok((
                Ok(TransactionRead::Complete(
                    CompleteTransactionObservation::after_last_page_without_quarantine(
                        account,
                        window,
                        filter,
                        rows,
                        CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                    )
                    .map_err(invalid)?,
                )),
                count,
            ))
        }
        "incomplete" => {
            let reason = get_incomplete_reason(fields)?;
            let pages: usize = fields.number()?;
            let mut received = Vec::new();
            let mut total = 0_usize;
            for page in 0..pages {
                let count: usize = fields.number()?;
                total = total.checked_add(count).ok_or_else(|| invalid(()))?;
                received.push(batch(connection, entry, page, count)?);
            }
            let quarantine_count: usize = fields.number()?;
            let mut quarantined = Vec::new();
            for _ in 0..quarantine_count {
                let count: usize = fields.number()?;
                let mut texts = Vec::new();
                for _ in 0..count {
                    texts.push(UntrustedText::try_new(&fields.take()?).map_err(invalid)?);
                }
                quarantined.push(QuarantinedTransaction::UnknownCurrency(texts));
            }
            Ok((
                Ok(TransactionRead::Incomplete(IncompleteTransactions {
                    account,
                    window,
                    filter,
                    received,
                    quarantined,
                    reason,
                })),
                total,
            ))
        }
        _ => Err(invalid(())),
    }
}

#[cfg(test)]
mod t21_tests {
    use super::{Fields, MasterNotReady, PortError, PortErrorKind, error, put_error};
    use application::{
        HttpStatus, ReadEndpoint, RetryDelay, SystemFailure, SystemPort, SystemPortError,
    };
    use domain::{Nanosecond, SpanSeconds, TimeSpan};

    fn assert_round_trip(reason: MasterNotReady) {
        let expected = PortError::new(
            ReadEndpoint::Instruments,
            HttpStatus::NotReceived,
            PortErrorKind::MasterNotReady(reason),
        );
        let mut fields = Fields::default();
        put_error(&mut fields, expected);
        let mut decoded = Fields::decode(&fields.encode()).expect("decode");
        assert_eq!(error(&mut decoded).expect("error"), expected);
        decoded.finish().expect("complete");
    }

    #[test]
    fn unready_master_reasons_round_trip_and_unknown_reasons_fail() {
        for reason in [
            MasterNotReady::StatusCoverage,
            MasterNotReady::IdentityContinuity,
            MasterNotReady::MissingInstrument,
        ] {
            let expected = PortError::new(
                ReadEndpoint::Instruments,
                HttpStatus::NotReceived,
                PortErrorKind::MasterNotReady(reason),
            );
            let mut fields = Fields::default();
            put_error(&mut fields, expected);
            let mut decoded = Fields::decode(&fields.encode()).expect("decode");
            assert_eq!(error(&mut decoded).expect("error"), expected);
            decoded.finish().expect("complete");
        }
        let mut fields = Fields::default();
        super::put_read_endpoint(&mut fields, ReadEndpoint::Instruments);
        super::put_http_status(&mut fields, HttpStatus::NotReceived);
        fields.push(&"master-not-ready");
        fields.push(&"unknown");
        let _error = error(&mut fields).expect_err("unknown reason");
    }

    #[test]
    fn system_errors_and_retry_delays_round_trip_without_losing_the_cause() {
        let known_delay = RetryDelay::Known(
            TimeSpan::try_new(SpanSeconds::new(53), Nanosecond::new(17)).expect("delay"),
        );
        for port in [
            SystemPort::Repository,
            SystemPort::AuditLog,
            SystemPort::SecretStore,
            SystemPort::Notifier,
        ] {
            for failure in [
                SystemFailure::PermissionDenied,
                SystemFailure::Unavailable(RetryDelay::Unknown),
                SystemFailure::Unavailable(known_delay),
                SystemFailure::InvalidData,
                SystemFailure::RepositoryIntegrity,
                SystemFailure::SecretIntegrity,
            ] {
                let system = SystemPortError::new(port, failure);
                assert_round_trip(MasterNotReady::Storage(system));
                assert_round_trip(MasterNotReady::Issuer(system));
            }
        }
    }
}
