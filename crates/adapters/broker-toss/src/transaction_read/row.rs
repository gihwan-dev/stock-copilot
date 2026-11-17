use crate::transaction_read::{field, invalid};
use application::PortError;
use domain::{
    AssetClass, AssetClassCode, Cancellation, Currency, IsinCode, Market, Money, NumericInput,
    Quantity, Ratio, Settlement, Symbol, Transaction, TransactionCode, TransactionData,
    TransactionKind, TransactionPrice,
};
use serde_json::Value;

type Conversion<T> = Result<T, PortError>;
fn text(value: &Value) -> Conversion<&str> {
    value.as_str().ok_or_else(invalid)
}
fn numeric(value: &Value) -> Conversion<NumericInput> {
    let input = text(value)?;
    if input.len() > 30 {
        return Err(invalid());
    }
    NumericInput::try_new(input).map_err(|_error| invalid())
}
fn optional<T>(
    value: &Value,
    parse: impl FnOnce(&Value) -> Conversion<T>,
) -> Conversion<Option<T>> {
    if value.is_null() {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}
pub(crate) fn convert(value: &Value) -> Conversion<Transaction> {
    let (market, currency) = market_currency(value)?;
    let money = |amount_value: &Value| {
        Money::parse(&numeric(amount_value)?, currency).map_err(|_error| invalid())
    };
    let data = TransactionData {
        kind: TransactionKind::new(
            TransactionCode::try_new(text(field(value, "transactionType")?)?)
                .map_err(|_error| invalid())?,
        ),
        market,
        currency,
        symbol: optional(field(value, "symbol")?, |field_value| {
            Symbol::try_new(text(field_value)?).map_err(|_error| invalid())
        })?,
        asset_class: optional(field(value, "assetClass")?, |field_value| {
            AssetClassCode::try_new(text(field_value)?)
                .map(AssetClass::new)
                .map_err(|_error| invalid())
        })?,
        isin: optional(field(value, "isinCode")?, |field_value| {
            IsinCode::try_new(text(field_value)?).map_err(|_error| invalid())
        })?,
        quantity: optional(field(value, "quantity")?, |field_value| {
            Quantity::parse(&numeric(field_value)?).map_err(|_error| invalid())
        })?,
        filled_price: optional(field(value, "filledPrice")?, |field_value| {
            TransactionPrice::try_new(money(field_value)?).map_err(|_error| invalid())
        })?,
        amount: optional(field(value, "amount")?, money)?,
        commission: money(field(value, "commission")?)?,
        tax: money(field(value, "tax")?)?,
        net_amount: optional(field(value, "netAmount")?, money)?,
        exchange_rate: optional(field(value, "exchangeRate")?, |field_value| {
            Ratio::parse(&numeric(field_value)?).map_err(|_error| invalid())
        })?,
        settlement_date: crate::TossDate::try_from(text(field(value, "settlementDate")?)?)
            .map_err(|_error| invalid())?
            .into_domain(),
        settlement: match field(value, "settled")?.as_bool() {
            Some(true) => Settlement::Settled,
            Some(false) => Settlement::Pending,
            None => return Err(invalid()),
        },
        transaction_time: optional(field(value, "transactionTime")?, |field_value| {
            crate::TossTimestamp::try_from(text(field_value)?)
                .map(crate::TossTimestamp::into_domain)
                .map_err(|_error| invalid())
        })?,
        cancellation: match field(value, "isCancellation")?.as_bool() {
            Some(true) => Cancellation::Cancelled,
            Some(false) => Cancellation::Regular,
            None => return Err(invalid()),
        },
    };
    finish(data)
}

fn finish(data: TransactionData) -> Conversion<Transaction> {
    Transaction::try_new(data).map_err(|_error| invalid())
}

fn market_currency(value: &Value) -> Conversion<(Market, Currency)> {
    match (
        text(field(value, "marketCountry")?)?,
        text(field(value, "currency")?)?,
    ) {
        ("KR", "KRW") => Ok((Market::Kr, Currency::Krw)),
        ("US", "USD") => Ok((Market::Us, Currency::Usd)),
        _ => Err(invalid()),
    }
}

pub(crate) struct ParsedTransaction {
    pub(crate) transaction: Transaction,
    pub(crate) source: Box<serde_json::value::RawValue>,
}
pub(crate) fn project(
    rows: Vec<ParsedTransaction>,
) -> (Vec<Transaction>, Vec<Box<serde_json::value::RawValue>>) {
    rows.into_iter()
        .map(|parsed| (parsed.transaction, parsed.source))
        .unzip()
}
pub(crate) fn in_scope(value: &Value, window: domain::SettlementWindow) -> Conversion<bool> {
    match field(value, "settled")?.as_bool() {
        Some(false) => Ok(true),
        Some(true) => {
            let date = crate::TossDate::try_from(text(field(value, "settlementDate")?)?)
                .map_err(|_error| invalid())?
                .into_domain();
            Ok(date >= window.from() && date <= window.to())
        }
        None => Err(invalid()),
    }
}

pub(crate) fn validate_filter(value: &Value, filter: &domain::TransactionFilter) -> Conversion<()> {
    if let Some(kinds) = filter.selected_kinds() {
        let kind = TransactionKind::new(
            TransactionCode::try_new(text(field(value, "transactionType")?)?)
                .map_err(|_error| invalid())?,
        );
        if !kinds.contains(&kind) {
            return Err(invalid());
        }
    }
    Ok(())
}
