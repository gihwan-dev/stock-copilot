use crate::account_read::invalid;
use crate::{QueryKey, ReadRequest, ReadRoute, RouteId, TokenRenewal, TossAccountRead, TossDate};
use application::{
    Clock, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch,
    SecretStore, SecretValue,
};
use domain::{
    AccountId, BuyingPower, CalendarDate, CommissionRateFact, CommissionValidity, CostRate,
    Currency, KnownMarket, MasterCode, Money, NumericInput, Quantity, Ratio,
    ResolvedOrderInstrument, SellableQuantityFact, StockWarning, StockWarnings,
};

struct ParsedCommission {
    market: KnownMarket,
    rate: CostRate,
    validity: CommissionValidity,
}

fn account_secret(account: &AccountId) -> SecretValue {
    SecretValue::new(account.as_i64().to_string().into_bytes())
}

fn currency_code(currency: Currency) -> &'static str {
    match currency {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    }
}

fn malformed_query(endpoint: ReadEndpoint) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::NotReceived,
        PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
    )
}

fn parse_optional_date(
    raw: Option<&str>,
    endpoint: ReadEndpoint,
) -> Result<Option<CalendarDate>, PortError> {
    raw.map(|text| {
        TossDate::try_from(text)
            .map(TossDate::into_domain)
            .map_err(|_error| invalid(endpoint, ResponseMismatch::Validation))
    })
    .transpose()
}

pub(crate) async fn buying_power<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
    currency: Currency,
) -> Result<BuyingPower, PortError> {
    let query = ReadRequest::new(ReadRoute::BuyingPower)
        .with_account(account_secret(account))
        .with_query(QueryKey::Currency, currency_code(currency).to_owned());
    let body = reader.market_read(query, ReadEndpoint::BuyingPower).await?;
    let decoded: broker_toss_schema::types::BuyingPowerResponse = serde_json::from_value(body)
        .map_err(|error| {
            crate::diagnostics::decode_failed(ReadEndpoint::BuyingPower, &error);
            invalid(ReadEndpoint::BuyingPower, ResponseMismatch::Schema)
        })?;
    crate::diagnostics::staged(
        ReadEndpoint::BuyingPower,
        "buying power rejected",
        buying_power_money(&decoded, currency),
    )
    .map(|money| BuyingPower::new(money, reader.market_now()))
}

fn buying_power_money(
    decoded: &broker_toss_schema::types::BuyingPowerResponse,
    currency: Currency,
) -> Result<Money, PortError> {
    let response_currency = match decoded.currency {
        broker_toss_schema::types::Currency::Krw => Currency::Krw,
        broker_toss_schema::types::Currency::Usd => Currency::Usd,
        broker_toss_schema::types::Currency::Unknown(_) => {
            return Err(invalid(
                ReadEndpoint::BuyingPower,
                ResponseMismatch::Validation,
            ));
        }
    };
    if response_currency != currency {
        return Err(invalid(
            ReadEndpoint::BuyingPower,
            ResponseMismatch::Validation,
        ));
    }
    let input = NumericInput::try_new(&decoded.cash_buying_power)
        .map_err(|_error| invalid(ReadEndpoint::BuyingPower, ResponseMismatch::Validation))?;
    let money = Money::parse(&input, currency)
        .map_err(|_error| invalid(ReadEndpoint::BuyingPower, ResponseMismatch::Validation))?;
    if currency == Currency::Krw && money.amount().as_decimal().normalize().scale() != 0 {
        return Err(invalid(
            ReadEndpoint::BuyingPower,
            ResponseMismatch::Validation,
        ));
    }
    Ok(money)
}

pub(crate) async fn sellable_quantity<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
    instrument: &ResolvedOrderInstrument,
) -> Result<SellableQuantityFact, PortError> {
    let query = ReadRequest::new(ReadRoute::SellableQuantity)
        .with_account(account_secret(account))
        .with_query(QueryKey::Symbol, instrument.symbol().as_str().to_owned());
    let body = reader
        .market_read(query, ReadEndpoint::SellableQuantity)
        .await?;
    let decoded: broker_toss_schema::types::SellableQuantityResponse = serde_json::from_value(body)
        .map_err(|_error| invalid(ReadEndpoint::SellableQuantity, ResponseMismatch::Schema))?;
    let input = NumericInput::try_new(&decoded.sellable_quantity)
        .map_err(|_error| invalid(ReadEndpoint::SellableQuantity, ResponseMismatch::Validation))?;
    let quantity = Quantity::parse(&input)
        .map_err(|_error| invalid(ReadEndpoint::SellableQuantity, ResponseMismatch::Validation))?;
    SellableQuantityFact::from_lookup_result(
        account.clone(),
        instrument.clone(),
        quantity,
        reader.market_now(),
    )
    .map_err(|_error| invalid(ReadEndpoint::SellableQuantity, ResponseMismatch::Validation))
}

fn parse_commission_row(
    row: &broker_toss_schema::types::Commission,
) -> Result<ParsedCommission, PortError> {
    let validation = || invalid(ReadEndpoint::CommissionRate, ResponseMismatch::Validation);
    let market = match row.market_country {
        broker_toss_schema::types::MarketCountry::Kr => KnownMarket::Kr,
        broker_toss_schema::types::MarketCountry::Us => KnownMarket::Us,
        broker_toss_schema::types::MarketCountry::Unknown(_) => return Err(validation()),
    };
    let input = NumericInput::try_new(&row.rate).map_err(|_error| validation())?;
    let ratio = Ratio::parse(&input).map_err(|_error| validation())?;
    let rate = CostRate::try_new(ratio).map_err(|_error| validation())?;
    let start = parse_optional_date(row.start_date.as_deref(), ReadEndpoint::CommissionRate)?;
    let end = parse_optional_date(row.end_date.as_deref(), ReadEndpoint::CommissionRate)?;
    let validity = CommissionValidity::try_new(start, end).map_err(|_error| validation())?;
    Ok(ParsedCommission {
        market,
        rate,
        validity,
    })
}

pub(crate) async fn commission_rate<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    account: &AccountId,
    market: KnownMarket,
) -> Result<CommissionRateFact, PortError> {
    let query = ReadRequest::new(ReadRoute::CommissionRate).with_account(account_secret(account));
    let body = reader
        .market_read(query, ReadEndpoint::CommissionRate)
        .await?;
    let decoded: Vec<broker_toss_schema::types::Commission> = serde_json::from_value(body)
        .map_err(|_error| invalid(ReadEndpoint::CommissionRate, ResponseMismatch::Schema))?;
    let mut seen_kr = false;
    let mut seen_us = false;
    let mut selected = None;
    for item in &decoded {
        let parsed = parse_commission_row(item)?;
        match parsed.market {
            KnownMarket::Kr => {
                if seen_kr {
                    return Err(invalid(
                        ReadEndpoint::CommissionRate,
                        ResponseMismatch::Validation,
                    ));
                }
                seen_kr = true;
            }
            KnownMarket::Us => {
                if seen_us {
                    return Err(invalid(
                        ReadEndpoint::CommissionRate,
                        ResponseMismatch::Validation,
                    ));
                }
                seen_us = true;
            }
        }
        if parsed.market == market {
            selected = Some(parsed);
        }
    }
    let matched = selected
        .ok_or_else(|| invalid(ReadEndpoint::CommissionRate, ResponseMismatch::Validation))?;
    Ok(CommissionRateFact::from_commissions_result(
        market.into(),
        matched.rate,
        matched.validity,
    ))
}

fn parse_stock_warning(
    row: &broker_toss_schema::types::StockWarning,
) -> Result<StockWarning, PortError> {
    let validation = || invalid(ReadEndpoint::StockWarnings, ResponseMismatch::Validation);
    let code = MasterCode::try_new(&row.warning_type.to_string()).map_err(|_error| validation())?;
    if let Some(raw_exchange) = row.exchange.as_deref() {
        let _exchange = MasterCode::try_new(raw_exchange).map_err(|_error| validation())?;
    }
    let start_date = parse_optional_date(row.start_date.as_deref(), ReadEndpoint::StockWarnings)?;
    let end_date = parse_optional_date(row.end_date.as_deref(), ReadEndpoint::StockWarnings)?;
    if let (Some(start), Some(end)) = (start_date, end_date)
        && start > end
    {
        return Err(validation());
    }
    Ok(StockWarning::from_code(code))
}

pub(crate) async fn stock_warnings<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &ResolvedOrderInstrument,
) -> Result<StockWarnings, PortError> {
    let route_id = RouteId::new(instrument.symbol().as_str())
        .map_err(|_error| malformed_query(ReadEndpoint::StockWarnings))?;
    let query = ReadRequest::new(ReadRoute::StockWarnings(route_id));
    let body = reader
        .market_read(query, ReadEndpoint::StockWarnings)
        .await?;
    let decoded: Vec<broker_toss_schema::types::StockWarning> = serde_json::from_value(body)
        .map_err(|_error| invalid(ReadEndpoint::StockWarnings, ResponseMismatch::Schema))?;
    if decoded.len() > 256 {
        return Err(invalid(
            ReadEndpoint::StockWarnings,
            ResponseMismatch::Validation,
        ));
    }
    let mut warnings = Vec::with_capacity(decoded.len());
    for item in &decoded {
        warnings.push(parse_stock_warning(item)?);
    }
    Ok(StockWarnings::from_lookup_result(
        instrument.clone(),
        warnings,
        reader.market_now(),
    ))
}
