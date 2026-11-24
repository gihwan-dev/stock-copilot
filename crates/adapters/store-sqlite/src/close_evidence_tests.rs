use crate::estimates_tests::master_for;
use crate::sql::{close_evidence_tests as sql, market as market_sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, MarketStore};
use application::{MarketDataRepository, ValuationCloseEvidenceRepository};
use domain::{
    CalendarDate, CalendarYear, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
    Currency, DayNumber, InstrumentId, MonthNumber, Nanosecond, NumericInput, OffsetSeconds, Ohlcv,
    Price, PriceAdjustment, Quantity, Timestamp, UnixSeconds,
};

const DAY_START: i64 = 1_795_532_400;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(day),
    )
    .expect("날짜")
}

fn daily(id: &str, start: i64, collected: i64) -> Candle {
    let number = |text| NumericInput::try_new(text).expect("input");
    let price = |text| Price::parse(&number(text), Currency::Krw).expect("price");
    Candle::try_new(
        InstrumentId::try_new(id).expect("id"),
        CandleTime::try_new(at(start), CandlePeriod::Daily).expect("time"),
        PriceAdjustment::Raw,
        Ohlcv::try_new(
            price("70000"),
            price("71000"),
            price("69000"),
            price("70500"),
            Quantity::parse(&number("1000")).expect("volume"),
        )
        .expect("ohlcv"),
        CandleObservation::new(at(collected), CandleStatus::Finalized),
    )
    .expect("candle")
}

#[test]
fn close_evidence_reads_daily_candles_of_the_mapped_instrument() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = MarketStore::new(&db);
    let instrument = master_for("instrument-a", "005930")
        .order_instrument()
        .expect("instrument");
    let id = InstrumentId::try_new("instrument-a").expect("id");
    let today = daily("instrument-a", DAY_START, DAY_START.saturating_add(60_000));
    let yesterday = daily(
        "instrument-a",
        DAY_START.saturating_sub(86_400),
        DAY_START.saturating_sub(20_000),
    );
    rt.block_on(store.store_candles(&id, &[yesterday, today.clone()]))
        .expect("candles");
    assert_eq!(
        rt.block_on(store.observed_close_evidence(
            &instrument,
            date(25),
            at(DAY_START.saturating_add(90_000))
        ))
        .expect("unmapped"),
        Vec::new()
    );
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::MAP_SAMSUNG)))
        .expect("map");
    let evidence = rt
        .block_on(store.observed_close_evidence(
            &instrument,
            date(25),
            at(DAY_START.saturating_add(90_000)),
        ))
        .expect("evidence");
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].candle(), &today);
    assert_eq!(evidence[0].instrument().saved_state().instrument_id, id);
    assert_eq!(
        rt.block_on(store.observed_close_evidence(
            &instrument,
            date(25),
            at(DAY_START.saturating_add(59_999))
        ))
        .expect("too early"),
        Vec::new()
    );
    assert_eq!(
        rt.block_on(store.observed_close_evidence(
            &instrument,
            date(23),
            at(DAY_START.saturating_add(90_000))
        ))
        .expect("other day"),
        Vec::new()
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn close_evidence_sql_prepares_on_the_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [market_sql::ACTIVE_MAPPING_BY_SYMBOL, sql::MAP_SAMSUNG] {
        connection.prepare(statement).expect(statement);
    }
}
