use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead, TossDate};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{CalendarDate, Timestamp};
use serde_json::Value;
const PAGE_LIMIT: usize = 64;
const PAGE_SIZE: usize = 100;

pub(crate) struct PageQuery {
    pub route: ReadRoute,
    pub endpoint: ReadEndpoint,
    pub from: CalendarDate,
    pub through: CalendarDate,
    pub interval: Option<&'static str>,
}
fn invalid(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::invalid(endpoint)
}
fn required<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a Value, PortError> {
    value.get(key).ok_or_else(|| invalid(endpoint))
}
fn date(value: &str, endpoint: ReadEndpoint) -> Result<CalendarDate, PortError> {
    TossDate::try_from(value)
        .map(TossDate::into_domain)
        .map_err(|_error| invalid(endpoint))
}
fn format_date(value: CalendarDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year().as_i32(),
        value.month().as_u8(),
        value.day().as_u8()
    )
}
pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal, Context: Sync, Row: Send>(
    reader: &TossAccountRead<S, T, R>,
    context: &Context,
    query: PageQuery,
    parse: fn(&Value, &Context, Timestamp) -> Result<Row, PortError>,
) -> Result<Vec<Row>, PortError> {
    if query.from > query.through {
        return Err(crate::market_read::rejected(
            query.endpoint,
            RequestRejection::UnsupportedQuery,
        ));
    }
    let mut until = query.through;
    let mut result = Vec::new();
    for _page in 0..PAGE_LIMIT {
        let mut request = ReadRequest::new(query.route.clone())
            .with_query(QueryKey::Count, "100".to_owned())
            .with_query(QueryKey::Until, format_date(until));
        if let Some(interval) = query.interval {
            request = request.with_query(QueryKey::Interval, interval.to_owned());
        }
        let value = reader.market_read(request, query.endpoint).await?;
        let rows = required(&value, "records", query.endpoint)?
            .as_array()
            .ok_or_else(|| invalid(query.endpoint))?;
        if rows.len() > PAGE_SIZE {
            return Err(invalid(query.endpoint));
        }
        let collected = reader.market_now();
        let mut previous = None;
        for row in rows {
            let raw = crate::market_read::text(row, "date", query.endpoint)?;
            let day = date(raw, query.endpoint)?;
            if day > until || previous.is_some_and(|last| day >= last) {
                return Err(invalid(query.endpoint));
            }
            previous = Some(day);
            let item = parse(row, context, collected)?;
            if day >= query.from && day <= query.through {
                result.push(item);
            }
        }
        let cursor = required(&value, "nextUntil", query.endpoint)?;
        if cursor.is_null() {
            result.reverse();
            return Ok(result);
        }
        let next = date(
            cursor.as_str().ok_or_else(|| invalid(query.endpoint))?,
            query.endpoint,
        )?;
        let oldest = previous.ok_or_else(|| invalid(query.endpoint))?;
        if next >= until || next >= oldest {
            return Err(invalid(query.endpoint));
        }
        if oldest <= query.from || next < query.from {
            result.reverse();
            return Ok(result);
        }
        until = next;
    }
    Err(invalid(query.endpoint))
}
