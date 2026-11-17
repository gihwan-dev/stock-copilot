use super::codec::{date_text, invalid, read_date, read_time, time_text};
use crate::sql::transactions as sql;
use domain::{
    AssetClass, AssetClassCode, Cancellation, Currency, IsinCode, Market, MarketCode, Money,
    NumericInput, Quantity, Ratio, Settlement, Symbol, Timestamp, Transaction as DomainTransaction,
    TransactionCode, TransactionData, TransactionKind, TransactionPrice,
};
use rusqlite::{Connection, Row, Transaction, params};
pub(super) struct SavedRow {
    pub source: Option<String>,
    pub transaction: DomainTransaction,
    pub first_seen: Option<Timestamp>,
    pub disappeared_at: Option<Timestamp>,
}
fn input(text: &str) -> rusqlite::Result<NumericInput> {
    NumericInput::try_new(text).map_err(invalid)
}
fn money(text: &str, unit: Currency) -> rusqlite::Result<Money> {
    Money::parse(&input(text)?, unit).map_err(invalid)
}
fn decode(row: &Row<'_>) -> rusqlite::Result<(usize, SavedRow)> {
    let transaction = decode_transaction(row)?;
    Ok((
        usize::try_from(row.get::<_, i64>(1)?).map_err(invalid)?,
        SavedRow {
            source: row.get(21)?,
            transaction,
            first_seen: row
                .get::<_, Option<String>>(2)?
                .as_deref()
                .map(read_time)
                .transpose()?,
            disappeared_at: row
                .get::<_, Option<String>>(3)?
                .as_deref()
                .map(read_time)
                .transpose()?,
        },
    ))
}
fn currency(row: &Row<'_>) -> rusqlite::Result<Currency> {
    Ok(match row.get::<_, String>(6)?.as_str() {
        "KRW" => Currency::Krw,
        "USD" => Currency::Usd,
        _ => return Err(invalid(())),
    })
}
fn optional_money(row: &Row<'_>, unit: Currency, column: usize) -> rusqlite::Result<Option<Money>> {
    row.get::<_, Option<String>>(column)?
        .map(|text| money(&text, unit))
        .transpose()
}
fn decode_transaction(row: &Row<'_>) -> rusqlite::Result<DomainTransaction> {
    let unit = currency(row)?;
    DomainTransaction::try_new(TransactionData {
        kind: TransactionKind::new(
            TransactionCode::try_new(&row.get::<_, String>(4)?).map_err(invalid)?,
        ),
        market: Market::new(MarketCode::try_new(&row.get::<_, String>(5)?).map_err(invalid)?),
        currency: unit,
        symbol: row
            .get::<_, Option<String>>(7)?
            .map(|text| Symbol::try_new(&text).map_err(invalid))
            .transpose()?,
        asset_class: row
            .get::<_, Option<String>>(8)?
            .map(|text| {
                AssetClassCode::try_new(&text)
                    .map(AssetClass::new)
                    .map_err(invalid)
            })
            .transpose()?,
        isin: row
            .get::<_, Option<String>>(9)?
            .map(|text| IsinCode::try_new(&text).map_err(invalid))
            .transpose()?,
        quantity: row
            .get::<_, Option<String>>(10)?
            .map(|text| Quantity::parse(&input(&text)?).map_err(invalid))
            .transpose()?,
        filled_price: optional_money(row, unit, 11)?
            .map(|money_value| TransactionPrice::try_new(money_value).map_err(invalid))
            .transpose()?,
        amount: optional_money(row, unit, 12)?,
        commission: money(&row.get::<_, String>(13)?, unit)?,
        tax: money(&row.get::<_, String>(14)?, unit)?,
        net_amount: optional_money(row, unit, 15)?,
        exchange_rate: row
            .get::<_, Option<String>>(16)?
            .map(|text| Ratio::parse(&input(&text)?).map_err(invalid))
            .transpose()?,
        settlement_date: read_date(&row.get::<_, String>(17)?)?,
        settlement: match row.get::<_, String>(18)?.as_str() {
            "settled" => Settlement::Settled,
            "pending" => Settlement::Pending,
            _ => return Err(invalid(())),
        },
        transaction_time: row
            .get::<_, Option<String>>(19)?
            .as_deref()
            .map(read_time)
            .transpose()?,
        cancellation: match row.get::<_, String>(20)?.as_str() {
            "regular" => Cancellation::Regular,
            "cancelled" => Cancellation::Cancelled,
            _ => return Err(invalid(())),
        },
    })
    .map_err(invalid)
}
#[derive(Clone, Copy)]
pub(super) struct RowTarget<'a> {
    pub entry: i64,
    pub scope: &'a str,
    pub page: usize,
    pub position: usize,
}
#[derive(Clone, Copy)]
pub(super) struct RowEvidence<'a> {
    pub first: Option<Timestamp>,
    pub gone: Option<Timestamp>,
    pub source: Option<&'a str>,
}
pub(super) fn save(
    tx: &Transaction<'_>,
    target: RowTarget<'_>,
    item: &DomainTransaction,
    evidence: RowEvidence<'_>,
) -> rusqlite::Result<()> {
    let data = item.data();
    tx.execute(
        sql::INSERT_ROW,
        params![
            target.entry,
            target.scope,
            i64::try_from(target.page).map_err(invalid)?,
            i64::try_from(target.position).map_err(invalid)?,
            evidence.first.map(time_text),
            evidence.gone.map(time_text),
            data.kind.as_code().as_str(),
            data.market.as_code().as_str(),
            match data.currency {
                Currency::Krw => "KRW",
                Currency::Usd => "USD",
            },
            data.symbol.as_ref().map(Symbol::as_str),
            data.asset_class
                .as_ref()
                .map(|value| value.as_code().as_str().to_owned()),
            data.isin.as_ref().map(IsinCode::as_str),
            data.quantity.map(|value| value.as_decimal().to_string()),
            data.filled_price
                .map(|value| value.as_money().amount().as_decimal().to_string()),
            data.amount
                .map(|value| value.amount().as_decimal().to_string()),
            data.commission.amount().as_decimal().to_string(),
            data.tax.amount().as_decimal().to_string(),
            data.net_amount
                .map(|value| value.amount().as_decimal().to_string()),
            data.exchange_rate
                .map(|value| value.as_decimal().to_string()),
            date_text(data.settlement_date),
            match data.settlement {
                Settlement::Settled => "settled",
                Settlement::Pending => "pending",
            },
            data.transaction_time.map(time_text),
            match data.cancellation {
                Cancellation::Regular => "regular",
                Cancellation::Cancelled => "cancelled",
            }
        ],
    )?;
    if let Some(source) = evidence.source {
        tx.execute(sql::INSERT_SOURCE, params![tx.last_insert_rowid(), source])?;
    }
    Ok(())
}
pub(super) fn load(
    connection: &Connection,
    entry: i64,
    scope: &str,
    page: usize,
    count: usize,
) -> rusqlite::Result<Vec<SavedRow>> {
    let rows = connection
        .prepare(sql::ROWS)?
        .query_map(
            params![entry, scope, i64::try_from(page).map_err(invalid)?],
            decode,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.len() != count
        || rows
            .iter()
            .enumerate()
            .any(|(index, (position, _))| index != *position)
    {
        return Err(invalid(()));
    }
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}
