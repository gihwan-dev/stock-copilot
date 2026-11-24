pub(crate) mod row;

use crate::account_read::{MasterErrorMode, invalid, retry_delay, token_error};
use crate::{
    QueryKey, RawResponse, ReadRequest, ReadRoute, RouteId, TokenRenewal, TossAccountRead,
};
use application::{
    Clock, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch,
    SecretStore, SecretValue,
};
use domain::{
    AccountId, BrokerOrder, BrokerOrderId, BrokerOrderSet, ClosedOrderPageCollector,
    ClosedOrderQueryScope, ClosedOrderSet, MasterInstrumentFacts, OpenOrderCollector,
    OpenOrderQueryScope, OrderOpenState, OrderPageBoundary, TimeInterval, Timestamp,
};
use row::{ParsedOrder, RowFailure};
use serde_json::{Value, value::RawValue};
use std::collections::BTreeSet;

const MAX_ORDER_PAGES: u16 = 8192;
const MAX_ORDER_ROWS: usize = 4096;

#[derive(serde::Deserialize)]
struct OrderEnvelope {
    result: OrderPage,
    error: Option<Value>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderPage {
    orders: Vec<Box<RawValue>>,
    has_next: bool,
    next_cursor: Value,
}

fn account_secret(account: &AccountId) -> SecretValue {
    SecretValue::new(account.as_i64().to_string().into_bytes())
}

fn read_failure(response: &RawResponse, endpoint: ReadEndpoint) -> PortError {
    response
        .read_error(endpoint, retry_delay(response.headers().retry_after()))
        .port_error()
}

fn invalid_cursor(response: &RawResponse, pages: &ClosedPages) -> bool {
    response.status() == 400
        && pages.cursor.is_some()
        && response
            .read_error(
                ReadEndpoint::ClosedOrders,
                retry_delay(response.headers().retry_after()),
            )
            .cursor_hint()
            == crate::CursorHint::Invalid
}

fn closed_page_query(account: &AccountId, cursor: Option<&str>) -> ReadRequest {
    let mut query = ReadRequest::new(ReadRoute::Orders)
        .with_account(account_secret(account))
        .with_query(QueryKey::Status, "CLOSED".to_owned())
        .with_query(QueryKey::Limit, "100".to_owned());
    if let Some(next_cursor) = cursor {
        query = query.with_query(QueryKey::Cursor, next_cursor.to_owned());
    }
    query
}

pub(crate) async fn order<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
    order_id: &BrokerOrderId,
    master_errors: MasterErrorMode,
) -> Result<BrokerOrder, PortError> {
    let route_id = RouteId::new(order_id.as_str()).map_err(|_error| {
        PortError::new(
            ReadEndpoint::Order,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
        )
    })?;
    let query = ReadRequest::new(ReadRoute::Order(route_id)).with_account(account_secret(account));
    let body = reader.market_read(query, ReadEndpoint::Order).await?;
    let decoded: broker_toss_schema::types::Order = serde_json::from_value(body)
        .map_err(|_error| invalid(ReadEndpoint::Order, ResponseMismatch::Schema))?;
    if decoded.id != order_id.as_str() {
        return Err(invalid(ReadEndpoint::Order, ResponseMismatch::Validation));
    }
    let parsed = match row::parse_decoded(&decoded, reader.market_master()) {
        Ok(parsed) => parsed,
        Err(RowFailure::Schema) => {
            return Err(invalid(ReadEndpoint::Order, ResponseMismatch::Schema));
        }
        Err(RowFailure::MissingMaster) => {
            return Err(master_errors.missing(
                ReadEndpoint::Order,
                invalid(ReadEndpoint::Order, ResponseMismatch::Validation),
            ));
        }
        Err(RowFailure::Unidentifiable) => {
            return Err(invalid(ReadEndpoint::Order, ResponseMismatch::Validation));
        }
    };
    let observed = reader.market_now();
    BrokerOrder::from_lookup_result(parsed.into_row(account, observed))
        .map_err(|_error| invalid(ReadEndpoint::Order, ResponseMismatch::Validation))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenRowQuality {
    Clean,
    Unidentified,
    SchemaError,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenPageStatus {
    Complete,
    Truncated,
}

fn parse_open_page(body: &[u8]) -> Result<(OrderPage, OpenPageStatus), PortError> {
    let envelope: OrderEnvelope = serde_json::from_slice(body)
        .map_err(|_error| invalid(ReadEndpoint::OpenOrders, ResponseMismatch::Schema))?;
    if envelope.error.is_some_and(|error| !error.is_null()) {
        return Err(invalid(ReadEndpoint::OpenOrders, ResponseMismatch::Schema));
    }
    let page = envelope.result;
    let cursor_present = match &page.next_cursor {
        Value::Null => false,
        Value::String(_) => true,
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            return Err(invalid(ReadEndpoint::OpenOrders, ResponseMismatch::Schema));
        }
    };
    let status = if page.has_next || cursor_present {
        OpenPageStatus::Truncated
    } else {
        OpenPageStatus::Complete
    };
    if page.orders.is_empty() && status == OpenPageStatus::Truncated {
        return Err(invalid(
            ReadEndpoint::OpenOrders,
            ResponseMismatch::Validation,
        ));
    }
    Ok((page, status))
}

fn build_open_set(
    page: OrderPage,
    mut status: OpenPageStatus,
    account: &AccountId,
    master: &MasterInstrumentFacts,
    observed: Timestamp,
) -> Result<BrokerOrderSet, PortError> {
    let mut parsed_rows = Vec::new();
    let mut quality = OpenRowQuality::Clean;
    for raw in page.orders {
        if parsed_rows.len() >= MAX_ORDER_ROWS {
            status = OpenPageStatus::Truncated;
            break;
        }
        match row::parse_raw(raw.get(), master) {
            Ok(parsed) => parsed_rows.push(parsed),
            Err(RowFailure::Schema) => quality = OpenRowQuality::SchemaError,
            Err(RowFailure::MissingMaster | RowFailure::Unidentifiable) => {
                if quality == OpenRowQuality::Clean {
                    quality = OpenRowQuality::Unidentified;
                }
            }
        }
    }
    if parsed_rows.is_empty() && quality == OpenRowQuality::SchemaError {
        return Err(invalid(ReadEndpoint::OpenOrders, ResponseMismatch::Schema));
    }
    let mut collector = OpenOrderCollector::start(OpenOrderQueryScope::unfiltered(account.clone()));
    for parsed in parsed_rows {
        collector.accept_row(parsed.into_row(account, observed));
    }
    if quality != OpenRowQuality::Clean {
        collector.record_unidentified_row();
    }
    match status {
        OpenPageStatus::Truncated => collector.record_incomplete_page(),
        OpenPageStatus::Complete => collector.record_all_pages_received(),
    }
    collector
        .finish(observed)
        .map_err(|_error| invalid(ReadEndpoint::OpenOrders, ResponseMismatch::Validation))
}

pub(crate) async fn open_orders<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
) -> Result<BrokerOrderSet, PortError> {
    let query = ReadRequest::new(ReadRoute::Orders)
        .with_account(account_secret(account))
        .with_query(QueryKey::Status, "OPEN".to_owned());
    let response = reader
        .tokens()
        .get(&query)
        .await
        .map_err(|error| token_error(error, ReadEndpoint::OpenOrders))?;
    if !(200..300).contains(&response.status()) {
        return Err(read_failure(&response, ReadEndpoint::OpenOrders));
    }
    let (page, status) = parse_open_page(response.body().as_exposed_secret())?;
    let observed = reader.market_now();
    build_open_set(page, status, account, reader.market_master(), observed)
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ClosedTerminal {
    #[default]
    Reading,
    Exhausted,
    PageLimit,
    RowLimit,
    MidStreamFailed,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ClosedRowQuality {
    #[default]
    Clean,
    Unidentified,
    SchemaError,
}

#[derive(Default)]
struct ClosedPages {
    cursor: Option<String>,
    seen_cursors: BTreeSet<String>,
    seen_ids: BTreeSet<BrokerOrderId>,
    valid_rows: Vec<ParsedOrder>,
    quality: ClosedRowQuality,
    terminal: ClosedTerminal,
}

enum AppendOutcome {
    Continue,
    Done,
    RepeatedCursor,
    RowLimit,
    EnvelopeError(ResponseMismatch),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageStep {
    Continue,
    Stop,
}

impl ClosedPages {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn has_valid_rows(&self) -> bool {
        !self.valid_rows.is_empty()
    }

    fn fail_or_stop(&mut self, error: PortError) -> Result<PageStep, PortError> {
        if !self.has_valid_rows() {
            return Err(error);
        }
        self.terminal = ClosedTerminal::MidStreamFailed;
        Ok(PageStep::Stop)
    }

    fn push_closed_row(&mut self, raw: &str, reader_master: &MasterInstrumentFacts) {
        match row::parse_raw(raw, reader_master) {
            Ok(parsed)
                if parsed.status.open_state() == OrderOpenState::Closed
                    && self.seen_ids.insert(parsed.order_id.clone()) =>
            {
                self.valid_rows.push(parsed);
            }
            Ok(_) | Err(RowFailure::MissingMaster | RowFailure::Unidentifiable) => {
                if self.quality == ClosedRowQuality::Clean {
                    self.quality = ClosedRowQuality::Unidentified;
                }
            }
            Err(RowFailure::Schema) => {
                self.quality = ClosedRowQuality::SchemaError;
            }
        }
    }

    fn append_page(&mut self, body: &[u8], reader_master: &MasterInstrumentFacts) -> AppendOutcome {
        let Ok(envelope) = serde_json::from_slice::<OrderEnvelope>(body) else {
            return AppendOutcome::EnvelopeError(ResponseMismatch::Schema);
        };
        if envelope.error.is_some_and(|error| !error.is_null()) {
            return AppendOutcome::EnvelopeError(ResponseMismatch::Schema);
        }
        let page = envelope.result;
        let next_cursor = match (page.has_next, page.next_cursor) {
            (false, Value::Null) => None,
            (true, Value::String(next)) if !next.is_empty() => {
                if !self.seen_cursors.insert(next.clone()) {
                    return AppendOutcome::RepeatedCursor;
                }
                Some(next)
            }
            _ => return AppendOutcome::EnvelopeError(ResponseMismatch::Validation),
        };
        for raw in page.orders {
            if self.valid_rows.len() >= MAX_ORDER_ROWS {
                self.terminal = ClosedTerminal::RowLimit;
                return AppendOutcome::RowLimit;
            }
            self.push_closed_row(raw.get(), reader_master);
        }
        self.cursor = next_cursor;
        if self.cursor.is_none() {
            self.terminal = ClosedTerminal::Exhausted;
            AppendOutcome::Done
        } else {
            AppendOutcome::Continue
        }
    }

    fn finish(
        self,
        account: &AccountId,
        evaluation_window: TimeInterval,
        observed: Timestamp,
    ) -> Result<ClosedOrderSet, PortError> {
        let mut collector = ClosedOrderPageCollector::start(ClosedOrderQueryScope::unfiltered(
            account.clone(),
            evaluation_window,
        ));
        for parsed in self.valid_rows {
            collector.accept_row(parsed.into_row(account, observed));
        }
        if self.quality != ClosedRowQuality::Clean {
            collector.record_unidentified_row();
        }
        match self.terminal {
            ClosedTerminal::MidStreamFailed => collector.record_request_failed(),
            ClosedTerminal::PageLimit => collector.record_page_limit_reached(),
            ClosedTerminal::RowLimit => collector.record_row_limit_reached(),
            ClosedTerminal::Exhausted => {
                collector.record_page_boundary(OrderPageBoundary::LastPage);
            }
            ClosedTerminal::Reading => {}
        }
        let proven = collector.finish(observed).map_err(|_error| {
            PortError::new(
                ReadEndpoint::ClosedOrders,
                HttpStatus::NotReceived,
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
            )
        })?;
        Ok(proven.into_closed_order_set())
    }
}

fn process_closed_response(
    pages: &mut ClosedPages,
    restarted: &mut bool,
    response: &RawResponse,
    master: &MasterInstrumentFacts,
) -> Result<PageStep, PortError> {
    if invalid_cursor(response, pages) {
        if *restarted {
            return pages.fail_or_stop(read_failure(response, ReadEndpoint::ClosedOrders));
        }
        *restarted = true;
        pages.reset();
        return Ok(PageStep::Continue);
    }
    if !(200..300).contains(&response.status()) {
        return pages.fail_or_stop(read_failure(response, ReadEndpoint::ClosedOrders));
    }
    match pages.append_page(response.body().as_exposed_secret(), master) {
        AppendOutcome::Continue => Ok(PageStep::Continue),
        AppendOutcome::Done | AppendOutcome::RowLimit => Ok(PageStep::Stop),
        AppendOutcome::RepeatedCursor => {
            if !pages.has_valid_rows() {
                return Err(invalid(
                    ReadEndpoint::ClosedOrders,
                    ResponseMismatch::Validation,
                ));
            }
            pages.terminal = ClosedTerminal::PageLimit;
            Ok(PageStep::Stop)
        }
        AppendOutcome::EnvelopeError(mismatch) => {
            pages.fail_or_stop(invalid(ReadEndpoint::ClosedOrders, mismatch))
        }
    }
}

pub(crate) async fn closed_orders<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
    evaluation_window: TimeInterval,
) -> Result<ClosedOrderSet, PortError> {
    if evaluation_window.start() >= reader.market_now() {
        return Err(PortError::new(
            ReadEndpoint::ClosedOrders,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
        ));
    }
    let mut restarted = false;
    let mut pages = ClosedPages::default();
    for _page in 0_u16..MAX_ORDER_PAGES {
        let query = closed_page_query(account, pages.cursor.as_deref());
        let response = match reader.tokens().get(&query).await {
            Ok(received) => received,
            Err(error) => {
                pages.fail_or_stop(token_error(error, ReadEndpoint::ClosedOrders))?;
                break;
            }
        };
        if process_closed_response(
            &mut pages,
            &mut restarted,
            &response,
            reader.market_master(),
        )? == PageStep::Stop
        {
            break;
        }
    }
    if pages.terminal == ClosedTerminal::Reading {
        if !pages.has_valid_rows() {
            return Err(invalid(
                ReadEndpoint::ClosedOrders,
                ResponseMismatch::Validation,
            ));
        }
        pages.terminal = ClosedTerminal::PageLimit;
    }
    if !pages.has_valid_rows()
        && pages.quality == ClosedRowQuality::SchemaError
        && pages.terminal == ClosedTerminal::Exhausted
    {
        return Err(invalid(
            ReadEndpoint::ClosedOrders,
            ResponseMismatch::Schema,
        ));
    }
    let observed = reader.market_now();
    pages.finish(account, evaluation_window, observed)
}
