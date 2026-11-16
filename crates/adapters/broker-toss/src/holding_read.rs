use crate::account_read::invalid;
use application::{HoldingsObservation, ReadEndpoint, ResponseMismatch};
use domain::{
    AcquisitionPrice, Currency, CurrencyAmounts, DailyProfitLoss, Holding, HoldingObservation,
    Market, MasterInstrumentFacts, Money, NumericInput, OrderSide, PortfolioDailyProfitLoss, Price,
    Quantity, Ratio, ReportedHolding, ReportedPortfolio, Timestamp,
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
        Some(Value::Null) => None,
        Some(usd_value) => Some(money(usd_value, Currency::Usd)?),
        None => return Err(failure()),
    };
    CurrencyAmounts::new(krw, usd).map_err(|_error| failure())
}
fn holding(
    value: &Value,
    master: &MasterInstrumentFacts,
    observed: Timestamp,
) -> Conversion<Holding> {
    let (market, currency) = match (
        field(value, "/marketCountry")?.as_str(),
        field(value, "/currency")?.as_str(),
    ) {
        (Some("KR"), Some("KRW")) => (Market::Kr, Currency::Krw),
        (Some("US"), Some("USD")) => (Market::Us, Currency::Usd),
        _ => return Err(failure()),
    };
    let symbol = field(value, "/symbol")?.as_str().ok_or_else(failure)?;
    let identified = master
        .instruments()
        .iter()
        .find(|row| {
            row.row().symbol.as_str() == symbol
                && row.row().currency == currency
                && row
                    .quote_instrument()
                    .is_ok_and(|instrument| Market::from(*instrument.market()) == market)
        })
        .ok_or_else(failure)?;
    let instrument = identified
        .order_gate_facts()
        .and_then(|gate| gate.instrument_for(OrderSide::Sell))
        .map_err(|_error| failure())?;
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
    Holding::new(
        instrument,
        quantity,
        acquisition,
        HoldingObservation::new(Some(price), Some(reported), observed),
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
) -> Conversion<HoldingsObservation> {
    let decoded: broker_toss_schema::types::HoldingsOverview =
        serde_json::from_value(value.clone())
            .map_err(|_error| invalid(ReadEndpoint::Holdings, ResponseMismatch::Schema))?;
    let modeled = serde_json::to_value(decoded).map_err(|_error| failure())?;
    validate_numbers(&modeled)?;
    let rows = field(value, "/items")?.as_array().ok_or_else(failure)?;
    let mut seen = std::collections::BTreeSet::new();
    let holdings = rows
        .iter()
        .map(|row| {
            let parsed = holding(row, master, observed)?;
            if !seen.insert((
                parsed.instrument().market(),
                parsed.instrument().symbol().clone(),
            )) {
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
    Ok(HoldingsObservation::from_broker_result(holdings, reported))
}

fn field<'a>(value: &'a Value, pointer: &str) -> Conversion<&'a Value> {
    value.pointer(pointer).ok_or_else(failure)
}
