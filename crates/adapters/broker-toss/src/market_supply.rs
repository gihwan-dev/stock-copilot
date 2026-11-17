use crate::market_pages::{self, PageQuery};
use crate::{ReadRoute, RouteId, TokenRenewal, TossAccountRead, TossDate};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{
    CalendarDate, CreditDetail, CreditSide, CreditTrade, KnownMarket, NumericInput, ProgramContext,
    ProgramTrade, ProgramVolume, QuoteInstrument, SecuritiesLending, ShortSelling, SupplyContext,
    SupplyKrw, SupplyRate, SupplyRatio, SupplyShares, SupplySignedShares, Timestamp,
};
use serde_json::Value;

fn invalid(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::invalid(endpoint)
}
fn rejected(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::rejected(endpoint, RequestRejection::UnsupportedQuery)
}
fn required<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a Value, PortError> {
    value.get(key).ok_or_else(|| invalid(endpoint))
}
fn text<'a>(value: &'a Value, key: &str, endpoint: ReadEndpoint) -> Result<&'a str, PortError> {
    crate::market_read::text(value, key, endpoint)
}
fn input(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<NumericInput, PortError> {
    NumericInput::try_new(text(value, key, endpoint)?).map_err(|_error| invalid(endpoint))
}
fn shares(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<SupplyShares, PortError> {
    SupplyShares::parse(&input(value, key, endpoint)?).map_err(|_error| invalid(endpoint))
}
fn krw(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<SupplyKrw, PortError> {
    SupplyKrw::parse(&input(value, key, endpoint)?).map_err(|_error| invalid(endpoint))
}
fn rate(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<SupplyRate, PortError> {
    SupplyRate::parse(&input(value, key, endpoint)?).map_err(|_error| invalid(endpoint))
}
fn stamp(value: &Value, endpoint: ReadEndpoint) -> Result<Timestamp, PortError> {
    crate::market_read::timestamp(text(value, "updatedAt", endpoint)?, endpoint)
}
fn date(value: &str, endpoint: ReadEndpoint) -> Result<CalendarDate, PortError> {
    TossDate::try_from(value)
        .map(TossDate::into_domain)
        .map_err(|_error| invalid(endpoint))
}
fn context(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
    endpoint: ReadEndpoint,
) -> Result<SupplyContext, PortError> {
    SupplyContext::from_response(
        instrument.symbol().clone(),
        date(text(value, "date", endpoint)?, endpoint)?,
        stamp(value, endpoint)?,
        collected,
    )
    .map_err(|_error| invalid(endpoint))
}
fn volume(value: &Value, endpoint: ReadEndpoint) -> Result<ProgramVolume, PortError> {
    ProgramVolume::try_new(
        shares(value, "buyVolume", endpoint)?,
        shares(value, "sellVolume", endpoint)?,
        SupplySignedShares::parse(&input(value, "netBuyVolume", endpoint)?)
            .map_err(|_error| invalid(endpoint))?,
    )
    .map_err(|_error| invalid(endpoint))
}
fn program(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
) -> Result<ProgramTrade, PortError> {
    let endpoint = ReadEndpoint::ProgramTrades;
    let context = ProgramContext::from_response(
        instrument.symbol().clone(),
        date(text(value, "date", endpoint)?, endpoint)?,
        collected,
    )
    .map_err(|_error| invalid(endpoint))?;
    Ok(ProgramTrade::from_response(
        context,
        volume(required(value, "arbitrage", endpoint)?, endpoint)?,
        volume(required(value, "nonArbitrage", endpoint)?, endpoint)?,
    ))
}
fn ratio(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<SupplyRatio, PortError> {
    if required(value, key, endpoint)?.is_null() {
        Ok(SupplyRatio::MissingDenominator)
    } else {
        Ok(SupplyRatio::Reported(rate(value, key, endpoint)?))
    }
}
fn short(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
) -> Result<ShortSelling, PortError> {
    let endpoint = ReadEndpoint::ShortSelling;
    Ok(ShortSelling::from_response(
        context(value, instrument, collected, endpoint)?,
        shares(value, "shortSellingVolume", endpoint)?,
        krw(value, "shortSellingAmount", endpoint)?,
        ratio(value, "shortSellingVolumeRate", endpoint)?,
        ratio(value, "shortSellingAmountRate", endpoint)?,
    ))
}
fn side(value: &Value, endpoint: ReadEndpoint) -> Result<CreditSide, PortError> {
    if value.is_null() {
        return Ok(CreditSide::NoData);
    }
    Ok(CreditSide::Reported(CreditDetail::from_response(
        shares(value, "newQuantity", endpoint)?,
        shares(value, "returnQuantity", endpoint)?,
        shares(value, "balanceQuantity", endpoint)?,
        rate(value, "balanceRate", endpoint)?,
        rate(value, "tradingRate", endpoint)?,
    )))
}
fn credit(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
) -> Result<CreditTrade, PortError> {
    let endpoint = ReadEndpoint::CreditTrades;
    Ok(CreditTrade::from_response(
        context(value, instrument, collected, endpoint)?,
        side(required(value, "marginLoan", endpoint)?, endpoint)?,
        side(required(value, "stockLoan", endpoint)?, endpoint)?,
    ))
}
fn lending(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
) -> Result<SecuritiesLending, PortError> {
    let endpoint = ReadEndpoint::SecuritiesLending;
    Ok(SecuritiesLending::from_response(
        context(value, instrument, collected, endpoint)?,
        shares(value, "executionQuantity", endpoint)?,
        shares(value, "repaymentQuantity", endpoint)?,
        shares(value, "balanceQuantity", endpoint)?,
        krw(value, "balanceAmount", endpoint)?,
    ))
}
fn route(
    instrument: &QuoteInstrument,
    endpoint: ReadEndpoint,
    kind: fn(RouteId) -> ReadRoute,
) -> Result<ReadRoute, PortError> {
    RouteId::new(instrument.symbol().as_str())
        .map(kind)
        .map_err(|_error| rejected(endpoint))
}
pub(crate) async fn programs<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<ProgramTrade>, PortError> {
    if instrument.market() != &KnownMarket::Kr {
        return Err(rejected(ReadEndpoint::ProgramTrades));
    }
    let query = PageQuery {
        route: route(
            instrument,
            ReadEndpoint::ProgramTrades,
            ReadRoute::ProgramTrades,
        )?,
        endpoint: ReadEndpoint::ProgramTrades,
        from,
        through,
        interval: None,
    };
    market_pages::fetch(reader, instrument, query, program).await
}
pub(crate) async fn shorts<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<ShortSelling>, PortError> {
    if instrument.market() != &KnownMarket::Kr {
        return Err(rejected(ReadEndpoint::ShortSelling));
    }
    let query = PageQuery {
        route: route(
            instrument,
            ReadEndpoint::ShortSelling,
            ReadRoute::ShortSelling,
        )?,
        endpoint: ReadEndpoint::ShortSelling,
        from,
        through,
        interval: None,
    };
    market_pages::fetch(reader, instrument, query, short).await
}
pub(crate) async fn credits<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<CreditTrade>, PortError> {
    if instrument.market() != &KnownMarket::Kr {
        return Err(rejected(ReadEndpoint::CreditTrades));
    }
    let query = PageQuery {
        route: route(
            instrument,
            ReadEndpoint::CreditTrades,
            ReadRoute::CreditTrades,
        )?,
        endpoint: ReadEndpoint::CreditTrades,
        from,
        through,
        interval: None,
    };
    market_pages::fetch(reader, instrument, query, credit).await
}
pub(crate) async fn loans<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<SecuritiesLending>, PortError> {
    if instrument.market() != &KnownMarket::Kr {
        return Err(rejected(ReadEndpoint::SecuritiesLending));
    }
    let query = PageQuery {
        route: route(
            instrument,
            ReadEndpoint::SecuritiesLending,
            ReadRoute::SecuritiesLending,
        )?,
        endpoint: ReadEndpoint::SecuritiesLending,
        from,
        through,
        interval: None,
    };
    market_pages::fetch(reader, instrument, query, lending).await
}
