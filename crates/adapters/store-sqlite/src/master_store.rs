use crate::{SqliteRepositories, not_opened, sql::master as sql};
use application::{SystemPort, SystemPortError, WriteCompleted};
use domain::{
    CommonShare, Currency, DisplayInstrumentName, InstrumentId, KnownMarket, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds, Symbol, Timestamp,
    TradingSuspension, UnixSeconds,
};
use rusqlite::{Row, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMaster {
    pub market: KnownMarket,
    pub refreshed_at: Timestamp,
    pub instruments: Vec<MasterInstrument>,
}

fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

const fn market_code(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}

fn market_of(code: &str) -> rusqlite::Result<KnownMarket> {
    match code {
        "KR" => Ok(KnownMarket::Kr),
        "US" => Ok(KnownMarket::Us),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

const fn suspension_code(value: TradingSuspension) -> &'static str {
    match value {
        TradingSuspension::Clear => "clear",
        TradingSuspension::Suspended => "suspended",
    }
}

fn suspension_of(code: &str) -> rusqlite::Result<TradingSuspension> {
    match code {
        "clear" => Ok(TradingSuspension::Clear),
        "suspended" => Ok(TradingSuspension::Suspended),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn timestamp(row: &Row<'_>, start: usize) -> rusqlite::Result<Timestamp> {
    let next = |offset: usize| {
        start
            .checked_add(offset)
            .ok_or(rusqlite::Error::InvalidQuery)
    };
    Timestamp::from_parts(
        UnixSeconds::new(row.get(start)?),
        Nanosecond::new(row.get(next(1)?)?),
        OffsetSeconds::new(row.get(next(2)?)?),
    )
    .map_err(invalid)
}

fn detail(row: &Row<'_>) -> rusqlite::Result<Option<KoreanMasterDetail>> {
    let columns: (Option<String>, Option<String>, Option<String>) =
        (row.get(8)?, row.get(9)?, row.get(10)?);
    let (Some(liquidation), Some(krx), Some(nxt)) = columns else {
        return Ok(None);
    };
    Ok(Some(KoreanMasterDetail {
        liquidation: match liquidation.as_str() {
            "regular" => LiquidationTrading::Regular,
            "liquidation" => LiquidationTrading::Liquidation,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        krx: suspension_of(&krx)?,
        nxt: match nxt.as_str() {
            "unsupported" => NxtTrading::Unsupported,
            other => NxtTrading::Supported(suspension_of(other)?),
        },
    }))
}

fn decode(row: &Row<'_>) -> rusqlite::Result<MasterInstrument> {
    let text = |index: usize| row.get::<_, String>(index);
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new(&text(0)?).map_err(invalid)?,
        symbol: Symbol::try_new(&text(1)?).map_err(invalid)?,
        instrument_id: InstrumentId::try_new(&text(2)?).map_err(invalid)?,
        name: DisplayInstrumentName::try_new(&text(3)?).map_err(invalid)?,
        security_type: MasterSecurityType::from_code(
            MasterCode::try_new(&text(4)?).map_err(invalid)?,
        ),
        currency: match text(5)?.as_str() {
            "KRW" => Currency::Krw,
            "USD" => Currency::Usd,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        listing_status: match text(6)?.as_str() {
            "scheduled" => ListingStatus::Scheduled,
            "active" => ListingStatus::Active,
            "delisted" => ListingStatus::Delisted,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        common_share: match text(7)?.as_str() {
            "common" => CommonShare::Common,
            "other" => CommonShare::Other,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        korean_detail: detail(row)?,
        observed_at: timestamp(row, 11)?,
    })
    .map_err(invalid)
}

fn insert(
    transaction: &rusqlite::Transaction<'_>,
    market: KnownMarket,
    position: usize,
    instrument: &MasterInstrument,
) -> rusqlite::Result<()> {
    let row = instrument.row();
    let detail = row.korean_detail;
    transaction.execute(
        sql::INSERT_INSTRUMENT,
        params![
            market_code(market),
            i64::try_from(position).map_err(invalid)?,
            row.listing_market.as_str(),
            row.symbol.as_str(),
            row.instrument_id.as_str(),
            row.name.as_str(),
            row.security_type.code().as_str(),
            match row.currency {
                Currency::Krw => "KRW",
                Currency::Usd => "USD",
            },
            match row.listing_status {
                ListingStatus::Scheduled => "scheduled",
                ListingStatus::Active => "active",
                ListingStatus::Delisted => "delisted",
            },
            match row.common_share {
                CommonShare::Common => "common",
                CommonShare::Other => "other",
            },
            detail.map(|value| match value.liquidation {
                LiquidationTrading::Regular => "regular",
                LiquidationTrading::Liquidation => "liquidation",
            }),
            detail.map(|value| suspension_code(value.krx)),
            detail.map(|value| match value.nxt {
                NxtTrading::Unsupported => "unsupported",
                NxtTrading::Supported(state) => suspension_code(state),
            }),
            row.observed_at.utc_seconds().as_i64(),
            row.observed_at.nanosecond().as_u32(),
            row.observed_at.offset_seconds().as_i32()
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct MasterSnapshots<'a>(pub &'a SqliteRepositories);

impl MasterSnapshots<'_> {
    pub async fn save(
        &self,
        market: KnownMarket,
        refreshed_at: Timestamp,
        instruments: Vec<MasterInstrument>,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.0.shared().read().await;
        let database = guard.as_ref().ok_or(not_opened(SystemPort::Repository))?;
        database
            .write(move |transaction| {
                transaction.execute(sql::DELETE_MARKET, [market_code(market)])?;
                for (position, instrument) in instruments.iter().enumerate() {
                    insert(transaction, market, position, instrument)?;
                }
                transaction.execute(
                    sql::UPSERT_REFRESH,
                    params![
                        market_code(market),
                        refreshed_at.utc_seconds().as_i64(),
                        refreshed_at.nanosecond().as_u32(),
                        refreshed_at.offset_seconds().as_i32()
                    ],
                )?;
                Ok(WriteCompleted)
            })
            .await
    }

    pub async fn stored(&self) -> Result<Vec<StoredMaster>, SystemPortError> {
        let guard = self.0.shared().read().await;
        let database = guard.as_ref().ok_or(not_opened(SystemPort::Repository))?;
        database
            .read(|connection| {
                let refreshes = connection
                    .prepare(sql::REFRESHES)?
                    .query_map([], |row| {
                        Ok((market_of(&row.get::<_, String>(0)?)?, timestamp(row, 1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                refreshes
                    .into_iter()
                    .map(|(market, refreshed_at)| {
                        let instruments = connection
                            .prepare(sql::INSTRUMENTS)?
                            .query_map([market_code(market)], decode)?
                            .collect::<rusqlite::Result<Vec<_>>>()?;
                        Ok(StoredMaster {
                            market,
                            refreshed_at,
                            instruments,
                        })
                    })
                    .collect()
            })
            .await
    }
}
