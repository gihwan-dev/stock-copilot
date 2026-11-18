use crate::{
    Database, DatabasePath, MarketStore,
    tests::{Fixture, clock, key, runtime},
};
use application::{CacheWriteOutcome, FxRateQuery, MarketDataRepository};
use domain::{
    Currency, FxPair, FxRate, FxRateKind, FxSource, InstrumentId, Nanosecond, NumericInput,
    OffsetSeconds, Price, Quote, Symbol, TimeInterval, Timestamp, UnixSeconds,
};

fn at(seconds: i64, nanos: u32, offset: i32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .expect("time")
}
fn fx(value: &str, pair: FxPair, kind: FxRateKind, seconds: i64, end: i64) -> FxRate {
    FxRate::parse(
        &NumericInput::try_new(value).expect("input"),
        pair,
        kind,
        TimeInterval::try_new(at(seconds, 17, 32400), at(end, 23, -18000)).expect("window"),
        FxSource::TossOpenApi,
    )
    .expect("rate")
}
fn quote(value: &str, seconds: i64) -> Quote {
    Quote::try_new(
        Symbol::try_new("AAPL").expect("symbol"),
        Currency::Usd,
        Some(
            Price::parse(&NumericInput::try_new(value).expect("input"), Currency::Usd)
                .expect("price"),
        ),
        Some(at(seconds.checked_sub(1).expect("observation"), 71, -18000)),
        at(seconds, 13, 32400),
    )
    .expect("quote")
}
#[test]
fn fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let mid = fx("1374.25", FxPair::UsdKrw, FxRateKind::Mid, 10, 70);
    let buy = fx("1391.75", FxPair::UsdKrw, FxRateKind::Buy, 12, 82);
    let inverse = fx("0.00072", FxPair::KrwUsd, FxRateKind::Mid, 14, 94);
    {
        let database = Database::open(&path, &key(), &clock()).expect("open");
        let store = MarketStore::new(&database);
        for value in [mid, buy, inverse] {
            assert_eq!(
                rt.block_on(store.store_fx_rate(&value)),
                Ok(CacheWriteOutcome::Changed)
            );
        }
        assert_eq!(
            rt.block_on(store.store_fx_rate(&mid)),
            Ok(CacheWriteOutcome::Unchanged)
        );
        let older = fx("1500.4", FxPair::UsdKrw, FxRateKind::Mid, 9, 69);
        assert_eq!(
            rt.block_on(store.store_fx_rate(&older)),
            Ok(CacheWriteOutcome::OlderIgnored)
        );
        assert_eq!(
            rt.block_on(store.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::Latest)),
            Ok(Some(mid))
        );
        assert_eq!(
            rt.block_on(store.fx_rate(FxPair::UsdKrw, FxRateKind::Buy, FxRateQuery::Latest)),
            Ok(Some(buy))
        );
        assert_eq!(
            rt.block_on(store.fx_rate(FxPair::KrwUsd, FxRateKind::Mid, FxRateQuery::Latest)),
            Ok(Some(inverse))
        );
        assert_eq!(
            rt.block_on(store.fx_rate(
                FxPair::UsdKrw,
                FxRateKind::Mid,
                FxRateQuery::At(at(9, 0, 0))
            )),
            Ok(None)
        );
    };
    let database = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = MarketStore::new(&database);
    let loaded = rt
        .block_on(store.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::Latest))
        .expect("load")
        .expect("rate");
    assert_eq!(loaded, mid);
    assert_eq!(loaded.as_of().offset_seconds().as_i32(), &32_400_i32);
    assert_eq!(loaded.valid_until().offset_seconds().as_i32(), &-18_000_i32);
}
#[test]
fn quotes_reject_older_observations_and_preserve_instrument_isolation() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = MarketStore::new(&database);
    let first = InstrumentId::try_new("asset-one").expect("id");
    let second = InstrumentId::try_new("asset-two").expect("id");
    let newest = quote("192.41", 40);
    let older = quote("181.12", 39);
    let changed = quote("193.82", 40);
    assert_eq!(
        rt.block_on(store.store_quote(&first, &newest)),
        Ok(CacheWriteOutcome::Changed)
    );
    assert_eq!(
        rt.block_on(store.store_quote(&first, &newest)),
        Ok(CacheWriteOutcome::Unchanged)
    );
    assert_eq!(
        rt.block_on(store.store_quote(&first, &older)),
        Ok(CacheWriteOutcome::OlderIgnored)
    );
    assert_eq!(rt.block_on(store.quote(&first)), Ok(Some(newest)));
    assert_eq!(rt.block_on(store.quote(&second)), Ok(None));
    assert_eq!(
        rt.block_on(store.store_quote(&second, &older)),
        Ok(CacheWriteOutcome::Changed)
    );
    assert_eq!(
        rt.block_on(store.store_quote(&first, &changed)),
        Ok(CacheWriteOutcome::Changed)
    );
    assert_eq!(rt.block_on(store.quote(&first)), Ok(Some(changed)));
    assert_eq!(rt.block_on(store.quote(&second)), Ok(Some(older)));
}

fn candle(id: &str, seconds: i64, adjustment: domain::PriceAdjustment) -> domain::Candle {
    let number = |text| NumericInput::try_new(text).expect("input");
    let price = |text| Price::parse(&number(text), Currency::Usd).expect("price");
    domain::Candle::try_new(
        InstrumentId::try_new(id).expect("id"),
        domain::CandleTime::try_new(at(seconds, 0, 0), domain::CandlePeriod::Minute).expect("time"),
        adjustment,
        domain::Ohlcv::try_new(
            price("10.2"),
            price("13.4"),
            price("9.1"),
            price("12.3"),
            domain::Quantity::parse(&number("456.7")).expect("volume"),
        )
        .expect("ohlcv"),
        domain::CandleObservation::new(
            at(seconds.checked_add(60).expect("collection"), 19, 0),
            domain::CandleStatus::Finalized,
        ),
    )
    .expect("candle")
}
#[test]
fn candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = MarketStore::new(&database);
    let id = InstrumentId::try_new("asset-one").expect("id");
    let raw = domain::PriceAdjustment::Raw;
    let adjusted = domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(at(1, 23, 0)));
    let raw_first = candle("asset-one", 120, raw);
    let raw_last = candle("asset-one", 180, raw);
    let adjusted_value = candle("asset-one", 120, adjusted);
    let other = candle("asset-two", 240, raw);
    rt.block_on(store.store_candles(
        &id,
        &[raw_last.clone(), raw_first.clone(), adjusted_value.clone()],
    ))
    .expect("candles");
    let query = TimeInterval::try_new(at(120, 0, 0), at(240, 0, 0)).expect("window");
    assert_eq!(
        rt.block_on(store.candles(&id, query, domain::CandlePeriod::Minute, raw)),
        Ok(vec![raw_first.clone(), raw_last.clone()])
    );
    assert_eq!(
        rt.block_on(store.candles(&id, query, domain::CandlePeriod::Minute, adjusted)),
        Ok(vec![adjusted_value])
    );
    rt.block_on(store.store_candles(&id, &[candle("asset-one", 240, raw), other]))
        .expect_err("mismatched instrument");
    let longer = TimeInterval::try_new(at(120, 0, 0), at(300, 0, 0)).expect("window");
    assert_eq!(
        rt.block_on(store.candles(&id, longer, domain::CandlePeriod::Minute, raw)),
        Ok(vec![raw_first, raw_last])
    );
}

#[test]
fn corrupt_rows_fail_closed_without_affecting_other_keys() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = MarketStore::new(&database);
    let first = InstrumentId::try_new("asset-one").expect("id");
    let second = InstrumentId::try_new("asset-two").expect("id");
    let buy = fx("1378.5", FxPair::UsdKrw, FxRateKind::Buy, 15, 80);
    rt.block_on(store.store_quote(&first, &quote("191.1", 30)))
        .expect("quote");
    rt.block_on(store.store_quote(&second, &quote("199.3", 31)))
        .expect("quote");
    rt.block_on(store.store_fx_rate(&fx("1351.1", FxPair::UsdKrw, FxRateKind::Mid, 10, 70)))
        .expect("rate");
    rt.block_on(store.store_fx_rate(&buy)).expect("rate");
    rt.block_on(database.write(|tx| {
        tx.execute(crate::sql::market_tests::CORRUPT_FX, [])?;
        tx.execute(crate::sql::market_tests::CORRUPT_QUOTE, [])?;
        Ok(())
    }))
    .expect("corrupt fixture");
    assert_eq!(
        rt.block_on(store.quote(&first))
            .expect_err("invalid time")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        rt.block_on(store.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::Latest))
            .expect_err("invalid source")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        rt.block_on(store.quote(&second)),
        Ok(Some(quote("199.3", 31)))
    );
    assert_eq!(
        rt.block_on(store.fx_rate(FxPair::UsdKrw, FxRateKind::Buy, FxRateQuery::Latest)),
        Ok(Some(buy))
    );
    assert_eq!(
        rt.block_on(store.store_fx_rate(&fx("1411.1", FxPair::UsdKrw, FxRateKind::Mid, 20, 90)))
            .expect_err("invalid previous row")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}
#[test]
fn failure_after_delete_rolls_back_the_old_quote() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = MarketStore::new(&database);
    let id = InstrumentId::try_new("asset-one").expect("id");
    let first = quote("191.1", 30);
    rt.block_on(store.store_quote(&id, &first)).expect("first");
    rt.block_on(database.write(|tx| tx.execute_batch(crate::sql::market_tests::BLOCK_QUOTE)))
        .expect("trigger");
    rt.block_on(store.store_quote(&id, &quote("200.9", 40)))
        .expect_err("injected failure");
    assert_eq!(rt.block_on(store.quote(&id)), Ok(Some(first)));
}

#[test]
fn new_fx_values_and_time_publish_events_only_after_commit() {
    use application::{EventBusBuilder, EventSubscriber, SubscriberFailure};
    use domain::{DomainEvent, DomainEventKind};
    use std::sync::mpsc;
    struct Observer(mpsc::Sender<DomainEvent>);
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = MarketStore::new(&database);
    let (sender, received) = mpsc::channel();
    let mut builder = EventBusBuilder::default();
    builder.subscribe(
        std::iter::once(DomainEventKind::FxRateUpdated).collect(),
        Observer(sender),
    );
    let events = builder.build();
    let use_case = application::StoreFxRate::new(&store, &events);
    for (text, seconds, end, expected) in [
        ("1301", 10, 70, 1),
        ("1301", 10, 80, 0),
        ("1402", 9, 60, 0),
        ("1302", 10, 70, 1),
        ("1302", 11, 70, 1),
    ] {
        rt.block_on(use_case.execute(fx(text, FxPair::UsdKrw, FxRateKind::Mid, seconds, end)))
            .expect("store");
        assert_eq!(received.try_iter().count(), expected, "{text} {seconds}");
    }
    assert_eq!(
        rt.block_on(store.fx_rate(
            FxPair::UsdKrw,
            FxRateKind::Mid,
            FxRateQuery::At(at(10, 17, 0))
        )),
        Ok(Some(fx("1302", FxPair::UsdKrw, FxRateKind::Mid, 10, 70)))
    );
}
#[test]
fn all_market_sql_constants_prepare_on_the_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migration");
    for sql in [
        crate::sql::market::QUOTE,
        crate::sql::market::DELETE_QUOTE,
        crate::sql::market::INSERT_QUOTE,
        crate::sql::market::FX_RATES,
        crate::sql::market::DELETE_FX,
        crate::sql::market::INSERT_FX,
        crate::sql::market::CANDLES,
        crate::sql::market::DELETE_CANDLE,
        crate::sql::market::INSERT_CANDLE,
    ] {
        connection.prepare(sql).expect("prepare");
    }
}
