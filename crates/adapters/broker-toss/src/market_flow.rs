use crate::market_pages::{self, PageQuery};
use crate::{ReadRoute, RouteId, TokenRenewal, TossAccountRead, TossDate};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{
    CalendarDate, FlowInterval, IndicatorCode, InvestorFlow, KnownMarket, MarketFlowRecordInput,
    MarketFlowRow, MasterCode, NumericInput, QuoteInstrument, StockFlowRecordInput, StockFlowRow,
    Timestamp,
};
use serde_json::Value;

fn invalid(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::invalid(endpoint)
}
fn rejected(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::rejected(endpoint, RequestRejection::UnsupportedQuery)
}
fn text<'a>(value: &'a Value, key: &str, endpoint: ReadEndpoint) -> Result<&'a str, PortError> {
    crate::market_read::text(value, key, endpoint)
}
fn required<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a Value, PortError> {
    value.get(key).ok_or_else(|| invalid(endpoint))
}
fn number(value: &Value, key: &str, endpoint: ReadEndpoint) -> Result<NumericInput, PortError> {
    NumericInput::try_new(text(value, key, endpoint)?).map_err(|_error| invalid(endpoint))
}
fn code(value: &str, endpoint: ReadEndpoint) -> Result<MasterCode, PortError> {
    MasterCode::try_new(value).map_err(|_error| invalid(endpoint))
}
fn row_stock(name: &str, value: &Value) -> Result<StockFlowRow, PortError> {
    let endpoint = ReadEndpoint::StockInvestorFlow;
    Ok(StockFlowRow {
        investor: code(name, endpoint)?,
        buy: number(value, "buyVolume", endpoint)?,
        sell: number(value, "sellVolume", endpoint)?,
        reported_net: number(value, "netBuyVolume", endpoint)?,
    })
}
fn row_market(name: &str, value: &Value) -> Result<MarketFlowRow, PortError> {
    let endpoint = ReadEndpoint::IndicatorInvestorFlow;
    Ok(MarketFlowRow {
        investor: code(name, endpoint)?,
        buy: number(value, "buyAmount", endpoint)?,
        sell: number(value, "sellAmount", endpoint)?,
    })
}
const TOP: [&str; 4] = ["individual", "foreigner", "institution", "otherCorporation"];
const DETAIL: [&str; 7] = [
    "financialInvestment",
    "insurance",
    "trust",
    "privateEquityFund",
    "bank",
    "otherFinancialInstitution",
    "pensionFund",
];
fn day(value: &Value, endpoint: ReadEndpoint) -> Result<CalendarDate, PortError> {
    TossDate::try_from(text(value, "date", endpoint)?)
        .map(TossDate::into_domain)
        .map_err(|_error| invalid(endpoint))
}
fn updated(value: &Value, endpoint: ReadEndpoint) -> Result<Timestamp, PortError> {
    crate::market_read::timestamp(text(value, "updatedAt", endpoint)?, endpoint)
}
fn stock(
    value: &Value,
    instrument: &QuoteInstrument,
    collected: Timestamp,
) -> Result<InvestorFlow, PortError> {
    let endpoint = ReadEndpoint::StockInvestorFlow;
    let _decoded: broker_toss_schema::types::StockInvestorTradingRecord =
        serde_json::from_value(value.clone()).map_err(|_error| invalid(endpoint))?;
    let mut rows = Vec::new();
    for name in TOP {
        if let Some(group) = value.get(name).filter(|entry| !entry.is_null()) {
            rows.push(row_stock(name, group)?);
        }
    }
    if let Some(detail) = value
        .get("institution")
        .and_then(|group| group.get("breakdown"))
        .filter(|entry| !entry.is_null())
    {
        for name in DETAIL {
            rows.push(row_stock(name, required(detail, name, endpoint)?)?);
        }
    }
    InvestorFlow::from_stock_response(
        instrument.symbol().clone(),
        StockFlowRecordInput {
            date: day(value, endpoint)?,
            updated: updated(value, endpoint)?,
            rows,
            collected,
        },
    )
    .map_err(|_error| invalid(endpoint))
}
struct IndicatorContext {
    code: MasterCode,
    interval: MasterCode,
}
fn indicator(
    value: &Value,
    context: &IndicatorContext,
    collected: Timestamp,
) -> Result<InvestorFlow, PortError> {
    let endpoint = ReadEndpoint::IndicatorInvestorFlow;
    let _decoded: broker_toss_schema::types::InvestorTradingRecord =
        serde_json::from_value(value.clone()).map_err(|_error| invalid(endpoint))?;
    let mut rows = Vec::new();
    for name in TOP {
        rows.push(row_market(name, required(value, name, endpoint)?)?);
    }
    let institution = required(value, "institution", endpoint)?;
    let details = required(institution, "breakdown", endpoint)?;
    for name in DETAIL {
        rows.push(row_market(name, required(details, name, endpoint)?)?);
    }
    InvestorFlow::from_indicator_response(
        context.code.clone(),
        &context.interval,
        MarketFlowRecordInput {
            date: day(value, endpoint)?,
            updated: updated(value, endpoint)?,
            rows,
            collected,
        },
    )
    .map_err(|_error| invalid(endpoint))
}
pub(crate) async fn stocks<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<InvestorFlow>, PortError> {
    let endpoint = ReadEndpoint::StockInvestorFlow;
    if instrument.market() != &KnownMarket::Kr {
        return Err(rejected(endpoint));
    }
    let id = RouteId::new(instrument.symbol().as_str()).map_err(|_error| rejected(endpoint))?;
    market_pages::fetch(
        reader,
        instrument,
        PageQuery {
            route: ReadRoute::StockInvestorFlow(id),
            endpoint,
            from,
            through,
            interval: None,
        },
        stock,
    )
    .await
}
pub(crate) async fn indices<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    indicator: IndicatorCode,
    from: CalendarDate,
    through: CalendarDate,
    interval: FlowInterval,
) -> Result<Vec<InvestorFlow>, PortError> {
    let endpoint = ReadEndpoint::IndicatorInvestorFlow;
    let name = match indicator {
        IndicatorCode::Kospi => "KOSPI",
        IndicatorCode::Kosdaq => "KOSDAQ",
        IndicatorCode::KrBond2Y
        | IndicatorCode::KrBond3Y
        | IndicatorCode::KrBond5Y
        | IndicatorCode::KrBond10Y
        | IndicatorCode::KrBond20Y
        | IndicatorCode::KrBond30Y
        | IndicatorCode::Unknown => return Err(rejected(endpoint)),
    };
    let period = match interval {
        FlowInterval::Day => "1d",
        FlowInterval::Week => "1w",
        FlowInterval::Month => "1mo",
        FlowInterval::Year => "1y",
        FlowInterval::Unknown => return Err(rejected(endpoint)),
    };
    let id = RouteId::new(name).map_err(|_error| rejected(endpoint))?;
    let context = IndicatorContext {
        code: code(name, endpoint)?,
        interval: code(period, endpoint)?,
    };
    market_pages::fetch(
        reader,
        &context,
        PageQuery {
            route: ReadRoute::IndicatorInvestorFlow(id),
            endpoint,
            from,
            through,
            interval: Some(period),
        },
        self::indicator,
    )
    .await
}
