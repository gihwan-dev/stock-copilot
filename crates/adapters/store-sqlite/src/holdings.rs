use crate::{Database, failure, sql::holdings as sql};
use application::{Clock, SystemPortError, WriteCompleted};
use domain::{
    AccountId, AcquisitionPrice, Currency, DailyProfitLoss, Holding, Money, Nanosecond,
    NumericInput, OffsetSeconds, Price, Quantity, Ratio, ReportedHolding, Symbol, Timestamp,
    UnixSeconds,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotHolding {
    pub symbol: Symbol,
    pub quantity: Quantity,
    pub acquisition_price: AcquisitionPrice,
    pub current_price: Option<Price>,
    pub reported: Option<ReportedHolding>,
    pub observed_at: Timestamp,
}
impl From<&Holding> for SnapshotHolding {
    fn from(holding: &Holding) -> Self {
        Self {
            symbol: holding.instrument().symbol().clone(),
            quantity: holding.quantity(),
            acquisition_price: holding.acquisition_price(),
            current_price: holding.current_price(),
            reported: holding.reported(),
            observed_at: holding.observed_at(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingsSnapshot {
    pub account: AccountId,
    pub observed_at: Timestamp,
    pub holdings: Vec<SnapshotHolding>,
}

struct Header {
    id: i64,
    account: AccountId,
    at: Timestamp,
}
fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
fn input(value: &str) -> rusqlite::Result<NumericInput> {
    NumericInput::try_new(value).map_err(invalid)
}
fn money(value: &str, currency: Currency) -> rusqlite::Result<Money> {
    Money::parse(&input(value)?, currency).map_err(invalid)
}
fn timestamp(row: &Row<'_>, start: usize) -> rusqlite::Result<Timestamp> {
    Timestamp::from_parts(
        UnixSeconds::new(row.get(start)?),
        Nanosecond::new(row.get(start.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?)?),
        OffsetSeconds::new(row.get(start.checked_add(2).ok_or(rusqlite::Error::InvalidQuery)?)?),
    )
    .map_err(invalid)
}
fn header(row: &Row<'_>) -> rusqlite::Result<Header> {
    Ok(Header {
        id: row.get(0)?,
        account: AccountId::new(row.get(1)?),
        at: timestamp(row, 2)?,
    })
}
fn decode(row: &Row<'_>) -> rusqlite::Result<SnapshotHolding> {
    let currency = match row.get::<_, String>(1)?.as_str() {
        "KRW" => Currency::Krw,
        "USD" => Currency::Usd,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let quantity = Quantity::parse(&input(&row.get::<_, String>(2)?)?).map_err(invalid)?;
    let acquisition_price =
        AcquisitionPrice::try_new(money(&row.get::<_, String>(3)?, currency)?).map_err(invalid)?;
    let current_price = row
        .get::<_, Option<String>>(4)?
        .map(|value| Price::try_new(money(&value, currency)?).map_err(invalid))
        .transpose()?;
    let reported = match (
        row.get::<_, Option<String>>(5)?,
        row.get::<_, Option<String>>(6)?,
        row.get::<_, Option<String>>(7)?,
        row.get::<_, Option<String>>(8)?,
        row.get::<_, Option<String>>(9)?,
    ) {
        (None, None, None, None, None) => None,
        (Some(valuation), Some(profit), Some(rate), daily, daily_rate) => {
            let report = ReportedHolding::new(
                money(&valuation, currency)?,
                money(&profit, currency)?,
                Ratio::parse(&input(&rate)?).map_err(invalid)?,
            )
            .map_err(invalid)?;
            Some(match (daily, daily_rate) {
                (None, None) => report,
                (Some(amount), Some(day_rate)) => report
                    .with_daily_profit_loss(DailyProfitLoss::new(
                        money(&amount, currency)?,
                        Ratio::parse(&input(&day_rate)?).map_err(invalid)?,
                    ))
                    .map_err(invalid)?,
                _ => return Err(rusqlite::Error::InvalidQuery),
            })
        }
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(SnapshotHolding {
        symbol: Symbol::try_new(&row.get::<_, String>(0)?).map_err(invalid)?,
        quantity,
        acquisition_price,
        current_price,
        reported,
        observed_at: timestamp(row, 10)?,
    })
}
fn load(connection: &Connection, header: &Header) -> rusqlite::Result<HoldingsSnapshot> {
    let holdings = connection
        .prepare(sql::ITEMS)?
        .query_map([header.id], decode)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(HoldingsSnapshot {
        account: header.account.clone(),
        observed_at: header.at,
        holdings,
    })
}
fn currency(value: Currency) -> &'static str {
    match value {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    }
}
fn save_item(
    transaction: &Transaction<'_>,
    id: i64,
    position: usize,
    item: &SnapshotHolding,
) -> rusqlite::Result<()> {
    let unit = item.acquisition_price.money().currency();
    if item
        .current_price
        .is_some_and(|price| price.as_money().currency() != unit)
        || item
            .reported
            .is_some_and(|report_value| report_value.valuation().currency() != unit)
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let report = item.reported;
    let daily = report.and_then(ReportedHolding::daily_profit_loss);
    transaction.execute(
        sql::INSERT_ITEM,
        params![
            id,
            i64::try_from(position).map_err(invalid)?,
            item.symbol.as_str(),
            currency(unit),
            item.quantity.as_decimal().to_string(),
            item.acquisition_price
                .money()
                .amount()
                .as_decimal()
                .to_string(),
            item.current_price
                .map(|price| price.as_money().amount().as_decimal().to_string()),
            report.map(|report_value| report_value.valuation().amount().as_decimal().to_string()),
            report.map(|report_value| report_value.profit().amount().as_decimal().to_string()),
            report.map(|report_value| report_value.return_rate().as_decimal().to_string()),
            daily.map(|daily_value| daily_value.amount().amount().as_decimal().to_string()),
            daily.map(|daily_value| daily_value.rate().as_decimal().to_string()),
            item.observed_at.utc_seconds().as_i64(),
            item.observed_at.nanosecond().as_u32(),
            item.observed_at.offset_seconds().as_i32()
        ],
    )?;
    Ok(())
}
#[derive(Debug)]
pub struct HoldingsStore {
    pub(crate) database: Database,
}
impl HoldingsStore {
    #[must_use]
    pub const fn new(database: Database) -> Self {
        Self { database }
    }
    pub async fn record_holdings(
        &self,
        snapshot: HoldingsSnapshot,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.database
            .write(move |transaction| {
                let at = snapshot.observed_at;
                let found = transaction
                    .query_row(
                        sql::FIND,
                        params![
                            snapshot.account.as_i64(),
                            at.utc_seconds().as_i64(),
                            at.nanosecond().as_u32()
                        ],
                        header,
                    )
                    .optional()?;
                if let Some(existing) = found {
                    return identical(&load(transaction, &existing)?, &snapshot);
                }
                transaction.execute(
                    sql::INSERT,
                    params![
                        snapshot.account.as_i64(),
                        at.utc_seconds().as_i64(),
                        at.nanosecond().as_u32(),
                        at.offset_seconds().as_i32()
                    ],
                )?;
                let id = transaction.last_insert_rowid();
                for (position, item) in snapshot.holdings.iter().enumerate() {
                    save_item(transaction, id, position, item)?;
                }
                Ok(WriteCompleted)
            })
            .await
    }
    pub async fn holdings_history(
        &self,
        account: AccountId,
    ) -> Result<Vec<HoldingsSnapshot>, SystemPortError> {
        self.database
            .read(move |connection| {
                let headers = connection
                    .prepare(sql::HEADERS)?
                    .query_map([], header)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                headers
                    .iter()
                    .filter(|entry| entry.account == account)
                    .map(|entry| load(connection, entry))
                    .collect()
            })
            .await
    }
    pub async fn compact_holdings(
        &self,
        clock: &dyn Clock,
    ) -> Result<WriteCompleted, SystemPortError> {
        let today = clock
            .now()
            .date_at_offset(OffsetSeconds::new(32_400))
            .map_err(|_error| failure::integrity())?;
        self.database
            .write(move |transaction| compact(transaction, today))
            .await
    }
}

fn identical(
    stored: &HoldingsSnapshot,
    incoming: &HoldingsSnapshot,
) -> rusqlite::Result<WriteCompleted> {
    if stored != incoming {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(WriteCompleted)
}
fn remove_previous(transaction: &Transaction<'_>, previous: Option<i64>) -> rusqlite::Result<()> {
    if let Some(id) = previous {
        transaction.execute(sql::DELETE, [id])?;
    }
    Ok(())
}

fn compact(
    transaction: &Transaction<'_>,
    today: domain::CalendarDate,
) -> rusqlite::Result<WriteCompleted> {
    let headers = transaction
        .prepare(sql::HEADERS)?
        .query_map([], header)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut latest = BTreeMap::new();
    for entry in &headers {
        let date = entry
            .at
            .date_at_offset(OffsetSeconds::new(32_400))
            .map_err(invalid)?;
        if *date.days_until(today).map_err(invalid)?.as_i64() < 30 {
            continue;
        }
        remove_previous(
            transaction,
            latest.insert((entry.account.clone(), date), entry.id),
        )?;
    }
    Ok(WriteCompleted)
}
