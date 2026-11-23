use crate::account_read::invalid;
use application::{HoldingsObservation, ReadEndpoint, ResponseMismatch};
use domain::{
    AcquisitionPrice, Currency, CurrencyAmounts, DailyProfitLoss, Holding, HoldingObservation,
    KnownMarket, MasterInstrumentFacts, Money, NumericInput, OrderSide, PortfolioDailyProfitLoss,
    Price, Quantity, Ratio, ReportedHolding, ReportedPortfolio, Symbol, Timestamp,
};
use serde_json::Value;

type Conversion<T> = Result<T, application::PortError>;
fn failure() -> application::PortError {
    invalid(ReadEndpoint::Holdings, ResponseMismatch::Validation)
}
fn numeric(value: &Value) -> Conversion<NumericInput> {
    NumericInput::try_new(value.as_str().ok_or_else(failure)?).map_err(|_error| failure())
}
fn money(value: &Value, currency: Currency) -> Conversion<Money> {
    Money::parse(&numeric(value)?, currency).map_err(|_error| failure())
}
fn ratio(value: &Value) -> Conversion<Ratio> {
    Ratio::parse(&numeric(value)?).map_err(|_error| failure())
}
fn amounts(value: &Value) -> Conversion<CurrencyAmounts> {
    let krw = money(field(value, "/krw")?, Currency::Krw)?;
    let usd = match value.get("usd") {
        Some(Value::Null) | None => None,
        Some(usd_value) => Some(money(usd_value, Currency::Usd)?),
    };
    CurrencyAmounts::new(krw, usd).map_err(|_error| failure())
}
struct ParsedHolding {
    market: KnownMarket,
    currency: Currency,
    symbol: Symbol,
    quantity: Quantity,
    acquisition: AcquisitionPrice,
    price: Price,
    reported: ReportedHolding,
}

fn parse_holding(value: &Value) -> Conversion<ParsedHolding> {
    let (market, currency) = match (
        field(value, "/marketCountry")?.as_str(),
        field(value, "/currency")?.as_str(),
    ) {
        (Some("KR"), Some("KRW")) => (KnownMarket::Kr, Currency::Krw),
        (Some("US"), Some("USD")) => (KnownMarket::Us, Currency::Usd),
        _ => return Err(failure()),
    };
    let symbol = Symbol::try_new(field(value, "/symbol")?.as_str().ok_or_else(failure)?)
        .map_err(|_error| failure())?;
    domain::validate_market_symbol(&symbol, market).map_err(|_error| failure())?;
    let quantity =
        Quantity::parse(&numeric(field(value, "/quantity")?)?).map_err(|_error| failure())?;
    let acquisition =
        AcquisitionPrice::try_new(money(field(value, "/averagePurchasePrice")?, currency)?)
            .map_err(|_error| failure())?;
    let price = Price::parse(&numeric(field(value, "/lastPrice")?)?, currency)
        .map_err(|_error| failure())?;
    let reported = ReportedHolding::new(
        money(field(value, "/marketValue/amount")?, currency)?,
        money(field(value, "/profitLoss/amount")?, currency)?,
        ratio(field(value, "/profitLoss/rate")?)?,
    )
    .map_err(|_error| failure())?
    .with_daily_profit_loss(DailyProfitLoss::new(
        money(field(value, "/dailyProfitLoss/amount")?, currency)?,
        ratio(field(value, "/dailyProfitLoss/rate")?)?,
    ))
    .map_err(|_error| failure())?;
    Ok(ParsedHolding {
        market,
        currency,
        symbol,
        quantity,
        acquisition,
        price,
        reported,
    })
}

fn holding(
    parsed: &ParsedHolding,
    master: &MasterInstrumentFacts,
    observed: Timestamp,
    master_errors: crate::account_read::MasterErrorMode,
) -> Conversion<Holding> {
    let identified = master
        .instruments()
        .iter()
        .find(|row| {
            row.row().symbol == parsed.symbol
                && row.row().currency == parsed.currency
                && row
                    .quote_instrument()
                    .is_ok_and(|instrument| *instrument.market() == parsed.market)
        })
        .ok_or_else(|| master_errors.missing(ReadEndpoint::Holdings, failure()))?;
    let instrument = identified
        .order_gate_facts()
        .and_then(|gate| gate.instrument_for(OrderSide::Sell))
        .map_err(|_error| failure())?;
    Holding::new(
        instrument,
        parsed.quantity,
        parsed.acquisition,
        HoldingObservation::new(Some(parsed.price), Some(parsed.reported), observed),
    )
    .map_err(|_error| failure())
}
fn validate_numbers(value: &Value) -> Conversion<()> {
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                match key.as_str() {
                    "name" | "symbol" | "marketCountry" | "currency" => {}
                    _ => validate_numbers(child)?,
                }
            }
        }
        Value::Array(rows) => {
            for row in rows {
                validate_numbers(row)?;
            }
        }
        Value::Null => {}
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            let _number = ratio(value)?;
        }
    }
    Ok(())
}
pub(crate) fn holdings(
    value: &Value,
    master: &MasterInstrumentFacts,
    observed: Timestamp,
    master_errors: crate::account_read::MasterErrorMode,
) -> Conversion<HoldingsObservation> {
    let decoded: broker_toss_schema::types::HoldingsOverview =
        serde_json::from_value(value.clone())
            .map_err(|_error| invalid(ReadEndpoint::Holdings, ResponseMismatch::Schema))?;
    let modeled = serde_json::to_value(decoded).map_err(|_error| failure())?;
    validate_numbers(&modeled)?;
    let rows = field(value, "/items")?.as_array().ok_or_else(failure)?;
    let mut seen = std::collections::BTreeSet::new();
    let parsed = rows
        .iter()
        .map(|row| {
            let parsed = parse_holding(row)?;
            if !seen.insert((parsed.market, parsed.symbol.clone())) {
                return Err(failure());
            }
            Ok(parsed)
        })
        .collect::<Conversion<Vec<_>>>()?;
    let reported = ReportedPortfolio::new(
        amounts(field(value, "/marketValue/amount")?)?,
        amounts(field(value, "/totalPurchaseAmount")?)?,
        amounts(field(value, "/profitLoss/amount")?)?,
        observed,
    )
    .with_daily_profit_loss(PortfolioDailyProfitLoss::new(
        amounts(field(value, "/dailyProfitLoss/amount")?)?,
        ratio(field(value, "/dailyProfitLoss/rate")?)?,
    ))
    .with_return_rate(ratio(field(value, "/profitLoss/rate")?)?);
    let holdings = parsed
        .iter()
        .map(|row| holding(row, master, observed, master_errors))
        .collect::<Conversion<Vec<_>>>()?;
    Ok(HoldingsObservation::from_broker_result(holdings, reported))
}

fn field<'a>(value: &'a Value, pointer: &str) -> Conversion<&'a Value> {
    value.pointer(pointer).ok_or_else(failure)
}
