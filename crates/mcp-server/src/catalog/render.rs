use application::{MarketToolError, MarketToolReply, SystemPortError};
use domain::{
    Currency, FxPair, FxRate, FxRateKind, Money, Quantity, Ratio, Timestamp, TotalCompleteness,
};
use serde_json::{Map, Value, json};

use crate::tools::{ToolErrorCode, ToolErrorPayload};
use crate::untrusted::{NormalizedUntrustedText, UntrustedTextLimit};

const SECONDS_PER_DAY: i64 = 86_400_i64;

#[must_use]
pub(super) fn currency_code(currency: Currency) -> &'static str {
    match currency {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    }
}

#[must_use]
pub(super) fn money(value: &Money) -> Value {
    json!({
        "amount": value.amount().as_decimal().to_string(),
        "currency": currency_code(value.currency()),
    })
}

#[must_use]
pub(super) fn optional_money(value: Option<&Money>) -> Value {
    value.map_or(Value::Null, money)
}

#[must_use]
pub(super) fn ratio(value: &Ratio) -> Value {
    Value::String(value.as_decimal().to_string())
}

#[must_use]
pub(super) fn optional_ratio(value: Option<&Ratio>) -> Value {
    value.map_or(Value::Null, ratio)
}

#[must_use]
pub(super) fn quantity(value: &Quantity) -> Value {
    Value::String(value.as_decimal().to_string())
}

#[must_use]
pub(super) const fn completeness(value: TotalCompleteness) -> &'static str {
    match value {
        TotalCompleteness::Complete => "complete",
        TotalCompleteness::Incomplete => "incomplete",
    }
}

#[must_use]
pub(super) fn fx_rate(value: &FxRate) -> Value {
    let pair = match value.pair() {
        FxPair::UsdKrw => "USD/KRW",
        FxPair::KrwUsd => "KRW/USD",
    };
    let kind = match value.kind() {
        FxRateKind::Mid => "mid",
        FxRateKind::Buy => "buy",
    };
    json!({
        "pair": pair,
        "kind": kind,
        "rate": money(&value.quote_per_base().as_money()),
        "as_of": timestamp(value.as_of()),
    })
}

#[must_use]
pub(super) fn untrusted_label(raw: &str) -> Value {
    NormalizedUntrustedText::from_raw(raw, UntrustedTextLimit::default()).into_field_object()
}

#[must_use]
pub(super) fn timestamp(value: Timestamp) -> Value {
    let utc = *value.utc_seconds().as_i64();
    let offset = i64::from(*value.offset_seconds().as_i32());
    let local = utc.saturating_add(offset);
    let days = local.div_euclid(SECONDS_PER_DAY);
    let seconds_of_day = local.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day.div_euclid(3_600_i64);
    let minute = seconds_of_day.rem_euclid(3_600_i64).div_euclid(60_i64);
    let second = seconds_of_day.rem_euclid(60_i64);
    let sign = if offset < 0_i64 { '-' } else { '+' };
    let offset_abs = offset.saturating_abs();
    let offset_hour = offset_abs.div_euclid(3_600_i64);
    let offset_minute = offset_abs.rem_euclid(3_600_i64).div_euclid(60_i64);
    Value::String(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{sign}{offset_hour:02}:{offset_minute:02}"
    ))
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days.saturating_add(719_468_i64);
    let era = shifted.div_euclid(146_097_i64);
    let day_of_era = shifted.rem_euclid(146_097_i64);
    let year_of_era = day_of_era
        .saturating_sub(day_of_era.div_euclid(1_460_i64))
        .saturating_add(day_of_era.div_euclid(36_524_i64))
        .saturating_sub(day_of_era.div_euclid(146_096_i64))
        .div_euclid(365_i64);
    let day_of_year = day_of_era.saturating_sub(
        year_of_era
            .saturating_mul(365_i64)
            .saturating_add(year_of_era.div_euclid(4_i64))
            .saturating_sub(year_of_era.div_euclid(100_i64)),
    );
    let month_index = day_of_year
        .saturating_mul(5_i64)
        .saturating_add(2_i64)
        .div_euclid(153_i64);
    let day = day_of_year
        .saturating_sub(
            month_index
                .saturating_mul(153_i64)
                .saturating_add(2_i64)
                .div_euclid(5_i64),
        )
        .saturating_add(1_i64);
    let month = if month_index < 10_i64 {
        month_index.saturating_add(3_i64)
    } else {
        month_index.saturating_sub(9_i64)
    };
    let base_year = year_of_era.saturating_add(era.saturating_mul(400_i64));
    let year = if month <= 2_i64 {
        base_year.saturating_add(1_i64)
    } else {
        base_year
    };
    (year, month, day)
}

#[must_use]
pub(super) fn upstream_error(_error: &SystemPortError) -> ToolErrorPayload {
    ToolErrorPayload::retryable(
        ToolErrorCode::UpstreamUnavailable,
        None,
        "stored data could not be read right now; retry later",
    )
}

#[must_use]
pub(super) fn missing_data() -> ToolErrorPayload {
    ToolErrorPayload::non_retryable(
        ToolErrorCode::DataUnavailable,
        None,
        "no stored data is available yet",
    )
}

pub(super) fn require<V>(reply: Result<Option<V>, SystemPortError>) -> Result<V, ToolErrorPayload> {
    match reply {
        Ok(Some(view)) => Ok(view),
        Ok(None) => Err(missing_data()),
        Err(error) => Err(upstream_error(&error)),
    }
}

#[must_use]
pub(super) fn market_error(error: MarketToolError) -> ToolErrorPayload {
    match error {
        MarketToolError::NotConnected => ToolErrorPayload::non_retryable(
            ToolErrorCode::DataUnavailable,
            None,
            "the brokerage account is not connected in the app, so live market data is not available; do not retry until the user connects it",
        ),
        MarketToolError::NotOffered => ToolErrorPayload::non_retryable(
            ToolErrorCode::DataUnavailable,
            None,
            "the brokerage does not offer this market data for the requested market",
        ),
        MarketToolError::NotFound => ToolErrorPayload::non_retryable(
            ToolErrorCode::DataUnavailable,
            None,
            "the brokerage has no data for this symbol",
        ),
        MarketToolError::Rejected => ToolErrorPayload::non_retryable(
            ToolErrorCode::DataUnavailable,
            None,
            "the brokerage rejected this market data request",
        ),
        MarketToolError::RateLimited => ToolErrorPayload::retryable(
            ToolErrorCode::RateLimited,
            None,
            "the brokerage is limiting requests right now; retry later",
        ),
        MarketToolError::Unavailable => ToolErrorPayload::retryable(
            ToolErrorCode::UpstreamUnavailable,
            None,
            "the brokerage market data service is temporarily unavailable; retry later",
        ),
        MarketToolError::InvalidResponse => ToolErrorPayload::non_retryable(
            ToolErrorCode::InternalError,
            None,
            "the brokerage returned market data the app could not read",
        ),
        MarketToolError::Storage(port_error) => upstream_error(&port_error),
    }
}

pub(super) fn require_market<V>(reply: MarketToolReply<V>) -> Result<V, ToolErrorPayload> {
    match reply {
        Ok(Some(view)) => Ok(view),
        Ok(None) => Err(missing_data()),
        Err(error) => Err(market_error(error)),
    }
}

#[must_use]
pub(super) fn object(entries: Vec<(&'static str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}
