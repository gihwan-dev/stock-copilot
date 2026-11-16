use crate::{QueryKey, ReadRequest, ReadRoute, TokenManager, TokenRenewal};
use application::{
    Clock, IncompleteReason, IncompleteTransactions, PortError, QuarantinedTransaction,
    ReadEndpoint, ResponseMismatch, SecretStore, SecretValue, TransactionRead,
};
use domain::{
    AccountId, CompleteTransactionResponse, CompletionAttestation, SettlementWindow,
    TransactionBatch, TransactionFilter, UntrustedText,
};
use serde_json::Value;

type Conversion<T> = Result<T, PortError>;
pub(crate) fn invalid() -> PortError {
    crate::account_read::invalid(ReadEndpoint::Transactions, ResponseMismatch::Validation)
}
#[derive(Default)]
struct Pages {
    cursor: Option<String>,
    seen: std::collections::BTreeSet<String>,
    rows: Vec<domain::Transaction>,
    quarantined: Vec<QuarantinedTransaction>,
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
    fn append(&mut self, body: &Value, window: SettlementWindow) -> Conversion<()> {
        if body.get("error").is_some_and(|error| !error.is_null()) {
            return Err(invalid());
        }
        let page = field(body, "result")?;
        let _decoded: broker_toss_schema::types::PaginatedTransactionResponse =
            serde_json::from_value(page.clone()).map_err(|_error| invalid())?;
        for raw in field(page, "transactions")?
            .as_array()
            .ok_or_else(invalid)?
        {
            if field(raw, "currency")?.as_str() != Some("KRW")
                && field(raw, "currency")?.as_str() != Some("USD")
            {
                let serialized = raw.to_string();
                self.quarantined
                    .push(QuarantinedTransaction::UnknownCurrency(vec![
                        UntrustedText::try_new(&serialized).map_err(|_error| invalid())?,
                    ]));
                bound(self.quarantined.len())?;
                continue;
            }
            let row = crate::transaction_row::convert(raw)?;
            let data = row.data();
            if data.settlement == domain::Settlement::Pending
                || (data.settlement_date >= window.from() && data.settlement_date <= window.to())
            {
                self.rows.push(row);
            }
            if self.rows.len() > 4096 {
                return Err(invalid());
            }
        }
        self.cursor = match (
            field(page, "hasNext")?.as_bool(),
            field(page, "nextCursor")?,
        ) {
            (Some(false), Value::Null) => None,
            (Some(true), Value::String(next))
                if !next.is_empty() && self.seen.insert(next.clone()) =>
            {
                Some(next.clone())
            }
            _ => return Err(invalid()),
        };
        Ok(())
    }
    fn finish(
        self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Conversion<TransactionRead> {
        let rows = TransactionBatch::try_new(self.rows).map_err(|_error| invalid())?;
        if !self.quarantined.is_empty() {
            return Ok(TransactionRead::Incomplete(IncompleteTransactions {
                account: account.clone(),
                window,
                filter: filter.clone(),
                received: vec![rows],
                quarantined: self.quarantined,
                reason: IncompleteReason::QuarantinedRows,
            }));
        }
        CompleteTransactionResponse::after_last_page_without_quarantine(
            account.clone(),
            window,
            filter.clone(),
            rows,
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
        let response = tokens
            .get(&transactions_request(
                account,
                filter,
                pages.cursor.as_deref(),
            ))
            .await
            .map_err(|error| crate::account_read::token_error(error, ReadEndpoint::Transactions))?;
        if response.status() == 400
            && pages.cursor.is_some()
            && serde_json::from_slice::<Value>(response.body().as_exposed_secret()).is_ok_and(
                |body| {
                    body.pointer("/error/code").and_then(Value::as_str) == Some("invalid-request")
                },
            )
        {
            if restarted {
                return Err(invalid());
            }
            restarted = true;
            pages = Pages::default();
            continue;
        }
        if !(200..300).contains(&response.status()) {
            return Err(response
                .read_error(
                    ReadEndpoint::Transactions,
                    crate::account_read::retry_delay(response.headers().retry_after()),
                )
                .port_error());
        }
        let body: Value = serde_json::from_slice(response.body().as_exposed_secret())
            .map_err(|_error| invalid())?;
        pages.append(&body, window)?;
        if pages.cursor.is_none() {
            return pages.finish(account, window, filter);
        }
    }
    Err(invalid())
}
pub(crate) fn field<'a>(value: &'a Value, key: &str) -> Conversion<&'a Value> {
    value.get(key).ok_or_else(invalid)
}

fn bound(count: usize) -> Conversion<()> {
    if count > 4096 { Err(invalid()) } else { Ok(()) }
}
