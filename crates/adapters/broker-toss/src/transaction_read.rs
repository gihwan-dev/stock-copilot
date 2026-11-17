pub(crate) mod row;
use crate::{QueryKey, ReadRequest, ReadRoute, TokenManager, TokenRenewal};
use application::{
    Clock, IncompleteReason, IncompleteTransactions, PortError, QuarantinedTransaction,
    ReadEndpoint, ResponseMismatch, SecretStore, SecretValue, TransactionRead,
};
use domain::{
    AccountId, CompleteTransactionResponse, CompletionAttestation, SettlementWindow,
    TransactionBatch, TransactionFilter, UntrustedText,
};
use serde_json::{Value, value::RawValue};

type Conversion<T> = Result<T, PortError>;
pub(crate) fn invalid() -> PortError {
    crate::account_read::invalid(ReadEndpoint::Transactions, ResponseMismatch::Validation)
}
#[derive(serde::Deserialize)]
struct Envelope {
    result: Page,
    error: Option<Value>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    transactions: Vec<Box<RawValue>>,
    has_next: bool,
    next_cursor: Value,
}
#[derive(serde::Deserialize)]
struct CursorEnvelope {
    result: CursorPage,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPage {
    next_cursor: Option<String>,
}
#[derive(Default)]
struct Pages {
    cursor: Option<String>,
    seen: std::collections::BTreeSet<String>,
    rows: Vec<row::ParsedTransaction>,
    quarantined: Vec<QuarantinedTransaction>,
}
#[derive(Debug)]
enum PageFault {
    Interrupted(PortError),
    RepeatedCursor,
    RowLimit,
}
impl From<PortError> for PageFault {
    fn from(error: PortError) -> Self {
        Self::Interrupted(error)
    }
}
fn transactions_request(
    account: &AccountId,
    filter: &TransactionFilter,
    cursor: Option<&str>,
) -> ReadRequest {
    let mut request = ReadRequest::new(ReadRoute::Transactions)
        .with_account(SecretValue::new(account.as_i64().to_string().into_bytes()))
        .with_query(QueryKey::Limit, "100".to_owned());
    if let Some(kinds) = filter.selected_kinds() {
        request = request.with_query(
            QueryKey::TransactionTypes,
            kinds
                .iter()
                .map(|kind| kind.as_code().as_str().to_owned())
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    if let Some(next_cursor) = cursor {
        request = request.with_query(QueryKey::Cursor, next_cursor.to_owned());
    }
    request
}
impl Pages {
    fn append(
        &mut self,
        body: &[u8],
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Result<(), PageFault> {
        if serde_json::from_slice::<CursorEnvelope>(body).is_ok_and(|envelope| {
            envelope
                .result
                .next_cursor
                .as_ref()
                .is_some_and(|next| self.seen.contains(next))
        }) {
            return Err(PageFault::RepeatedCursor);
        }
        let envelope: Envelope = serde_json::from_slice(body).map_err(|_error| invalid())?;
        if envelope.error.is_some_and(|error| !error.is_null()) {
            return Err(invalid().into());
        }
        let page = envelope.result;
        for source in page.transactions {
            self.append_row(source, window, filter)?;
        }
        if self.rows.len() > 4096 || self.quarantined.len() > 4096 {
            return Err(PageFault::RowLimit);
        }
        self.cursor = match (page.has_next, page.next_cursor) {
            (false, Value::Null) => None,
            (true, Value::String(next)) if !next.is_empty() => {
                self.seen.insert(next.clone());
                Some(next)
            }
            _ => return Err(invalid().into()),
        };
        Ok(())
    }
    fn append_row(
        &mut self,
        source: Box<RawValue>,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Conversion<()> {
        let value: Value = serde_json::from_str(source.get()).map_err(|_error| invalid())?;
        row::validate_filter(&value, filter)?;
        let scope = row::in_scope(&value, window);
        if scope.as_ref().is_ok_and(|included| !included) {
            return Ok(());
        }
        let currency = field(&value, "currency")?.as_str().ok_or_else(invalid)?;
        if currency != "KRW" && currency != "USD" {
            self.quarantined
                .push(QuarantinedTransaction::UnknownCurrency(quarantine(
                    source.get(),
                )?));
            return Ok(());
        }
        scope?;
        let _decoded: broker_toss_schema::types::Transaction =
            serde_json::from_value(value.clone()).map_err(|_error| invalid())?;
        let transaction = row::convert(&value)?;
        self.rows.push(row::ParsedTransaction {
            transaction,
            source,
        });
        Ok(())
    }
    fn incomplete(
        self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
        reason: IncompleteReason,
    ) -> Conversion<TransactionRead> {
        let final_reason = if self.rows.len() > 4096 || self.quarantined.len() > 4096 {
            IncompleteReason::RowLimitExceeded
        } else {
            reason
        };
        let (rows, _sources) = row::project(self.rows);
        let mut remaining = rows.into_iter();
        let mut received = Vec::new();
        loop {
            let batch: Vec<_> = remaining.by_ref().take(4096).collect();
            if batch.is_empty() {
                break;
            }
            received.push(TransactionBatch::try_new(batch).map_err(|_error| invalid())?);
        }
        Ok(TransactionRead::Incomplete(IncompleteTransactions {
            account: account.clone(),
            window,
            filter: filter.clone(),
            received,
            quarantined: self.quarantined,
            reason: final_reason,
        }))
    }
    fn interrupted(
        self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
        error: PortError,
    ) -> Conversion<TransactionRead> {
        if self.rows.is_empty() && self.quarantined.is_empty() {
            return Err(error);
        }
        self.incomplete(account, window, filter, IncompleteReason::PageInterrupted)
    }
    fn finish(
        self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Conversion<TransactionRead> {
        if !self.quarantined.is_empty() {
            return self.incomplete(account, window, filter, IncompleteReason::QuarantinedRows);
        }
        let (rows, _sources) = row::project(self.rows);
        CompleteTransactionResponse::after_last_page_without_quarantine(
            account.clone(),
            window,
            filter.clone(),
            TransactionBatch::try_new(rows).map_err(|_error| invalid())?,
            CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .map(TransactionRead::Complete)
        .map_err(|_error| invalid())
    }
}
pub(crate) async fn fetch<Store: SecretStore, Time: Clock, Renewal: TokenRenewal>(
    tokens: &TokenManager<Store, Time, Renewal>,
    account: &AccountId,
    window: SettlementWindow,
    filter: &TransactionFilter,
) -> Conversion<TransactionRead> {
    let mut restarted = false;
    let mut pages = Pages::default();
    for _page in 0_u16..8192 {
        let response = match tokens
            .get(&transactions_request(
                account,
                filter,
                pages.cursor.as_deref(),
            ))
            .await
        {
            Ok(received) => received,
            Err(error) => {
                return pages.interrupted(
                    account,
                    window,
                    filter,
                    crate::account_read::token_error(error, ReadEndpoint::Transactions),
                );
            }
        };
        if cursor_error(&response, &pages) {
            if restarted {
                return Err(read_failure(&response));
            }
            restarted = true;
            pages = Pages::default();
            continue;
        }
        if !(200..300).contains(&response.status()) {
            let error = read_failure(&response);
            return pages.interrupted(account, window, filter, error);
        }
        match pages.append(response.body().as_exposed_secret(), window, filter) {
            Ok(()) => {}
            Err(PageFault::Interrupted(error)) => {
                return pages.interrupted(account, window, filter, error);
            }
            Err(PageFault::RepeatedCursor) => return Err(invalid()),
            Err(PageFault::RowLimit) => {
                return pages.incomplete(
                    account,
                    window,
                    filter,
                    IncompleteReason::RowLimitExceeded,
                );
            }
        }
        if pages.cursor.is_none() {
            return pages.finish(account, window, filter);
        }
    }
    pages.interrupted(account, window, filter, invalid())
}
fn read_failure(response: &crate::RawResponse) -> PortError {
    response
        .read_error(
            ReadEndpoint::Transactions,
            crate::account_read::retry_delay(response.headers().retry_after()),
        )
        .port_error()
}
fn cursor_error(response: &crate::RawResponse, pages: &Pages) -> bool {
    response.status() == 400
        && pages.cursor.is_some()
        && response
            .read_error(
                ReadEndpoint::Transactions,
                crate::account_read::retry_delay(response.headers().retry_after()),
            )
            .cursor_hint()
            == crate::CursorHint::Invalid
}
pub(crate) fn field<'a>(value: &'a Value, key: &str) -> Conversion<&'a Value> {
    value.get(key).ok_or_else(invalid)
}
fn quarantine(source: &str) -> Conversion<Vec<UntrustedText>> {
    let mut remaining = source;
    let mut chunks = Vec::new();
    while !remaining.is_empty() {
        let mut boundary = remaining.len().min(65536);
        while !remaining.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }
        let (chunk, tail) = remaining.split_at_checked(boundary).ok_or_else(invalid)?;
        chunks.push(UntrustedText::try_new(chunk).map_err(|_error| invalid())?);
        remaining = tail;
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::Pages;
    use crate::TossDate;
    use domain::SettlementWindow;

    #[test]
    fn normal_row_source_keeps_received_spelling_until_port_projection() {
        let source = r#"{ "transactionType":"DEPOSIT", "marketCountry":"KR", "currency":"KRW", "symbol":null,"assetClass":null,"isinCode":null,"quantity":null,"filledPrice":null,"amount":"731.000","commission":"13.0","tax":"17","netAmount":"701","exchangeRate":null,"settlementDate":"2026-09-18","settled":true,"transactionTime":null,"isCancellation":false, "extra":"\uAC00" }"#;
        let body = format!(
            r#"{{"result":{{"transactions":[{source}],"hasNext":false,"nextCursor":null}}}}"#
        );
        let start = TossDate::try_from("2026-09-01")
            .expect("date")
            .into_domain();
        let end = TossDate::try_from("2026-09-30")
            .expect("date")
            .into_domain();
        let mut pages = Pages::default();
        pages
            .append(
                body.as_bytes(),
                SettlementWindow::try_new(start, end).expect("window"),
                &domain::TransactionFilter::all(),
            )
            .expect("page");
        assert_eq!(pages.rows[0].source.get(), source);
        let (rows, sources) = super::row::project(pages.rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(sources[0].get(), source);
        let unknown_source = source.replace("\"KRW\"", "\"EUR\"");
        let unknown_body = format!(
            r#"{{"result":{{"transactions":[{unknown_source}],"hasNext":false,"nextCursor":null}}}}"#
        );
        let mut unknown_pages = Pages::default();
        unknown_pages
            .append(
                unknown_body.as_bytes(),
                SettlementWindow::try_new(start, end).expect("window"),
                &domain::TransactionFilter::all(),
            )
            .expect("page");
        let application::QuarantinedTransaction::UnknownCurrency(chunks) =
            &unknown_pages.quarantined[0];
        assert_eq!(
            chunks
                .iter()
                .map(domain::UntrustedText::as_untrusted_str)
                .collect::<String>(),
            unknown_source
        );
    }
}
