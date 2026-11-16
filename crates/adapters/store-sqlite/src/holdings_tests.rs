use crate::{
    Database, DatabasePath, HoldingsSnapshot, HoldingsStore, SnapshotHolding,
    tests::{Fixture, clock, key, runtime},
};
use domain::{
    AccountId, AcquisitionPrice, Currency, DailyProfitLoss, Money, Nanosecond, NumericInput,
    OffsetSeconds, Price, Quantity, Ratio, ReportedHolding, Symbol, Timestamp, UnixSeconds,
};
fn at(day: i64, second: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(
            1_728_572_400_i64
                .checked_add(
                    day.checked_mul(86400)
                        .and_then(|value| value.checked_add(second))
                        .expect("범위"),
                )
                .expect("시각"),
        ),
        Nanosecond::new(17),
        OffsetSeconds::new(-18000),
    )
    .expect("시각")
}
struct Fixed(Timestamp);
impl application::Clock for Fixed {
    fn now(&self) -> Timestamp {
        self.0
    }
}
fn number(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("숫자")
}
fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&number(text), currency).expect("금액")
}
fn snapshot(day: i64, second: i64, account: i64) -> HoldingsSnapshot {
    let holdings = [Currency::Krw, Currency::Usd]
        .into_iter()
        .map(|unit| SnapshotHolding {
            symbol: Symbol::try_new("TEST").expect("종목"),
            quantity: Quantity::parse(&number("0.1234567890123456789012345678")).expect("수량"),
            acquisition_price: AcquisitionPrice::try_new(money("23.456789", unit)).expect("취득"),
            current_price: Some(Price::parse(&number("34.567891"), unit).expect("현재가")),
            reported: Some(
                ReportedHolding::new(
                    money("45.678912", unit),
                    money("-56.789123", unit),
                    Ratio::parse(&number("-0.678912")).expect("수익률"),
                )
                .expect("보고")
                .with_daily_profit_loss(DailyProfitLoss::new(
                    money("-78.912345", unit),
                    Ratio::parse(&number("-0.891234")).expect("당일율"),
                ))
                .expect("당일"),
            ),
            observed_at: at(day, second.checked_add(7).expect("관측")),
        })
        .collect();
    HoldingsSnapshot {
        account: AccountId::new(account),
        observed_at: at(day, second),
        holdings,
    }
}
#[test]
fn round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let expected = snapshot(0, 123, 11);
    {
        let database = Database::open(&path, &key(), &clock()).expect("열기");
        let db = HoldingsStore::new(database);
        rt.block_on(db.record_holdings(expected.clone()))
            .expect("저장");
        rt.block_on(db.record_holdings(expected.clone()))
            .expect("재시도");
        let mut conflict = expected.clone();
        conflict.holdings.clear();
        assert_eq!(
            rt.block_on(db.record_holdings(conflict))
                .expect_err("충돌")
                .failure(),
            application::SystemFailure::RepositoryIntegrity
        );
        assert_eq!(
            rt.block_on(db.holdings_history(AccountId::new(12)))
                .expect("다른 계좌"),
            Vec::<HoldingsSnapshot>::new()
        );
    };
    let database = Database::open(&path, &key(), &clock()).expect("다시 열기");
    let db = HoldingsStore::new(database);
    assert_eq!(
        rt.block_on(db.holdings_history(AccountId::new(11)))
            .expect("조회"),
        vec![expected]
    );
}
#[test]
fn empty_and_optional_observations_round_trip() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    let db = HoldingsStore::new(database);
    let mut first = snapshot(0, 0, 1);
    for holding in &mut first.holdings {
        holding.current_price = None;
        holding.reported = None;
    }
    let empty = HoldingsSnapshot {
        account: AccountId::new(1),
        observed_at: at(0, 1),
        holdings: vec![],
    };
    for value in [&first, &empty] {
        rt.block_on(db.record_holdings(value.clone()))
            .expect("저장");
    }
    assert_eq!(
        rt.block_on(db.holdings_history(AccountId::new(1)))
            .expect("조회"),
        vec![first, empty]
    );
}
#[test]
fn compaction_uses_kst_dates_29_30_31_and_last_per_account() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    let db = HoldingsStore::new(database);
    for account in [1, 2] {
        for day in [0, 1, 2] {
            for second in [1, 3600] {
                rt.block_on(db.record_holdings(snapshot(day, second, account)))
                    .expect("保存");
            }
        }
    }
    rt.block_on(db.compact_holdings(&Fixed(at(31, -1))))
        .expect("자정 직전");
    for account in [1, 2] {
        assert_eq!(
            rt.block_on(db.holdings_history(AccountId::new(account)))
                .expect("조회"),
            vec![
                snapshot(0, 3600, account),
                snapshot(1, 1, account),
                snapshot(1, 3600, account),
                snapshot(2, 1, account),
                snapshot(2, 3600, account)
            ]
        );
    }
    rt.block_on(db.compact_holdings(&Fixed(at(31, 0))))
        .expect("자정 직후");
    for account in [1, 2] {
        assert_eq!(
            rt.block_on(db.holdings_history(AccountId::new(account)))
                .expect("조회"),
            vec![
                snapshot(0, 3600, account),
                snapshot(1, 3600, account),
                snapshot(2, 1, account),
                snapshot(2, 3600, account)
            ]
        );
    }
    rt.block_on(db.compact_holdings(&Fixed(at(31, 0))))
        .expect("재실행");
}
#[test]
fn corruption_is_rejected_by_domain_and_decimal_constraint() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    let db = HoldingsStore::new(database);
    rt.block_on(db.record_holdings(snapshot(0, 0, 1)))
        .expect("저장");
    rt.block_on(
        db.database
            .write(|tx| tx.execute(crate::sql::holdings_tests::BAD_DECIMAL, [])),
    )
    .expect_err("십진수 거부");
    rt.block_on(
        db.database
            .write(|tx| tx.execute(crate::sql::holdings_tests::NEGATIVE_QUANTITY, [])),
    )
    .expect("깨진 도메인 값");
    assert_eq!(
        rt.block_on(db.holdings_history(AccountId::new(1)))
            .expect_err("닫힌 실패")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}
#[test]
fn failed_compaction_rolls_back_prior_deletes_and_children() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    let db = HoldingsStore::new(database);
    let originals = vec![snapshot(0, 1, 1), snapshot(0, 2, 1), snapshot(0, 3, 1)];
    for value in &originals {
        rt.block_on(db.record_holdings(value.clone()))
            .expect("저장");
    }
    rt.block_on(
        db.database
            .write(|tx| tx.execute_batch(crate::sql::holdings_tests::FAIL_DELETE)),
    )
    .expect("실패 주입");
    rt.block_on(db.compact_holdings(&Fixed(at(31, 0))))
        .expect_err("실패");
    assert_eq!(
        rt.block_on(db.holdings_history(AccountId::new(1)))
            .expect("원본"),
        originals
    );
}
#[test]
fn failed_snapshot_insert_rolls_back_header_and_previous_items() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    let db = HoldingsStore::new(database);
    let mut duplicate = snapshot(0, 0, 1);
    duplicate.holdings.push(duplicate.holdings[0].clone());
    rt.block_on(db.record_holdings(duplicate))
        .expect_err("중복");
    assert_eq!(
        rt.block_on(db.holdings_history(AccountId::new(1)))
            .expect("원본"),
        Vec::<HoldingsSnapshot>::new()
    );
}
#[test]
fn sql_constants_prepare_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("메모리");
    crate::migration::run(&mut connection, at(0, 0)).expect("마이그레이션");
    for sql in [
        crate::sql::tests::FUTURE,
        crate::sql::holdings::INSERT,
        crate::sql::holdings::HEADERS,
        crate::sql::holdings::FIND,
        crate::sql::holdings::ITEMS,
        crate::sql::holdings::INSERT_ITEM,
        crate::sql::holdings::DELETE,
    ] {
        connection.prepare(sql).expect("준비");
    }
}
