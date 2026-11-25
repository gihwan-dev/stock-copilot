use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead, TossDate};
use application::{Clock, PortError, ReadEndpoint, SecretStore};
use domain::{
    AuctionBoundary, CalendarDate, KnownMarket, MarketCalendar, MarketSession, SessionData,
    SessionInformation, TimeInterval,
};
use serde_json::Value;
const ENDPOINT: ReadEndpoint = ReadEndpoint::Calendar;

pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    market: KnownMarket,
) -> Result<MarketCalendar, PortError> {
    fetch_request(reader, market, None).await
}

pub(crate) async fn fetch_on<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    market: KnownMarket,
    date: CalendarDate,
) -> Result<MarketCalendar, PortError> {
    fetch_request(reader, market, Some(date)).await
}

fn date_text(value: CalendarDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year().as_i32(),
        value.month().as_u8(),
        value.day().as_u8()
    )
}

async fn fetch_request<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    market: KnownMarket,
    date: Option<CalendarDate>,
) -> Result<MarketCalendar, PortError> {
    let route = match market {
        KnownMarket::Kr => ReadRoute::CalendarKr,
        KnownMarket::Us => ReadRoute::CalendarUs,
    };
    let request = match date {
        Some(day) => ReadRequest::new(route).with_query(QueryKey::Date, date_text(day)),
        None => ReadRequest::new(route),
    };
    let value = reader.market_read(request, ENDPOINT).await?;
    fields(&value, &["previousBusinessDay", "today", "nextBusinessDay"])?;
    let previous = day(required(&value, "previousBusinessDay")?, market)?;
    let today = day(required(&value, "today")?, market)?;
    let next = day(required(&value, "nextBusinessDay")?, market)?;
    MarketCalendar::try_new(previous, today, next).map_err(|_error| invalid())
}
fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a Value, PortError> {
    value.get(key).ok_or_else(invalid)
}
fn fields(value: &Value, allowed: &[&str]) -> Result<(), PortError> {
    let object = value.as_object().ok_or_else(invalid)?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid());
    }
    Ok(())
}
fn day(value: &Value, market: KnownMarket) -> Result<MarketSession, PortError> {
    let date = TossDate::try_from(crate::market_read::text(value, "date", ENDPOINT)?)
        .map_err(|_error| invalid())?
        .into_domain();
    let information = match market {
        KnownMarket::Kr => {
            fields(value, &["date", "integrated"])?;
            let integrated = required(value, "integrated")?;
            if integrated.is_null() {
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                }
            } else {
                fields(integrated, &["preMarket", "regularMarket", "afterMarket"])?;
                SessionInformation::Kr {
                    pre_market: session(
                        required(integrated, "preMarket")?,
                        Some("singlePriceAuctionStartTime"),
                    )?,
                    regular_market: session(
                        required(integrated, "regularMarket")?,
                        Some("singlePriceAuctionStartTime"),
                    )?,
                    after_market: session(
                        required(integrated, "afterMarket")?,
                        Some("singlePriceAuctionEndTime"),
                    )?,
                }
            }
        }
        KnownMarket::Us => {
            fields(
                value,
                &[
                    "date",
                    "dayMarket",
                    "preMarket",
                    "regularMarket",
                    "afterMarket",
                ],
            )?;
            SessionInformation::Us {
                day_market: session(required(value, "dayMarket")?, None)?,
                pre_market: session(required(value, "preMarket")?, None)?,
                regular_market: session(required(value, "regularMarket")?, None)?,
                after_market: session(required(value, "afterMarket")?, None)?,
            }
        }
    };
    MarketSession::try_new(date, information).map_err(|_error| invalid())
}
fn session(value: &Value, auction_key: Option<&str>) -> Result<Option<SessionData>, PortError> {
    if value.is_null() {
        return Ok(None);
    }
    let mut allowed = vec!["startTime", "endTime"];
    if let Some(key) = auction_key {
        allowed.push(key);
    }
    fields(value, &allowed)?;
    let start = parse_time(crate::market_read::text(value, "startTime", ENDPOINT)?)?;
    let end = parse_time(crate::market_read::text(value, "endTime", ENDPOINT)?)?;
    let interval = TimeInterval::try_new(start, end).map_err(|_error| invalid())?;
    let auction = match auction_key.and_then(|key| value.get(key).map(|entry| (key, entry))) {
        Some((_, Value::Null)) | None => None,
        Some((key, stamp)) => {
            let at = parse_time(stamp.as_str().ok_or_else(invalid)?)?;
            Some(if key == "singlePriceAuctionEndTime" {
                AuctionBoundary::EndsAt(at)
            } else {
                AuctionBoundary::StartsAt(at)
            })
        }
    };
    SessionData::try_new(interval, auction)
        .map(Some)
        .map_err(|_error| invalid())
}
fn parse_time(value: &str) -> Result<domain::Timestamp, PortError> {
    let time = crate::market_read::timestamp(value, ENDPOINT)?;
    if *time.offset_seconds().as_i32() != 32_400_i32 {
        return Err(invalid());
    }
    Ok(time)
}
