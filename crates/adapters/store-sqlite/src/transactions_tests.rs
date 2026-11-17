use crate::{
    Database, DatabasePath, TransactionStore,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    CompleteTransactionObservation, FullInspectionRepository, ObservedTransaction,
    ObservedTransactionBatch, RawTransactionRow, TransactionRead, TransactionRepository,
    TransactionSyncRecord, TransactionSyncRepository,
};
use application::{IncompleteReason, IncompleteTransactions, QuarantinedTransaction};
use domain::{
    AccountId, CalendarDate, CalendarYear, Cancellation, CompletionAttestation, Currency,
    DayNumber, Market, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds, SettledRowSet,
    Settlement, SettlementWindow, Timestamp, Transaction, TransactionData, TransactionFilter,
    TransactionKind, UnixSeconds,
};
fn at(count: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(count),
        Nanosecond::new(17),
        OffsetSeconds::new(32400),
    )
    .expect("시각")
}
fn date(count: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(count),
    )
    .expect("날짜")
}
fn window() -> SettlementWindow {
    SettlementWindow::try_new(date(3), date(16)).expect("창")
}
fn money(text: &str) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
}
fn row() -> Transaction {
    Transaction::try_new(TransactionData {
        kind: TransactionKind::Deposit,
        market: Market::Kr,
        currency: Currency::Krw,
        symbol: None,
        asset_class: None,
        isin: None,
        quantity: None,
        filled_price: None,
        amount: Some(money("101.23")),
        commission: money("2.34"),
        tax: money("3.45"),
        net_amount: Some(money("95.44")),
        exchange_rate: None,
        settlement_date: date(3),
        settlement: Settlement::Settled,
        transaction_time: Some(at(5)),
        cancellation: Cancellation::Regular,
    })
    .expect("거래")
}
fn complete(account: &AccountId, sources: &[&str]) -> TransactionRead {
    TransactionRead::Complete(
        CompleteTransactionObservation::after_last_page_without_quarantine(
            account.clone(),
            window(),
            TransactionFilter::all(),
            ObservedTransactionBatch::try_new(
                sources
                    .iter()
                    .map(|text| {
                        ObservedTransaction::new(row(), RawTransactionRow::new((*text).to_owned()))
                    })
                    .collect(),
            )
            .expect("배치"),
            CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .expect("완전 응답"),
    )
}
async fn disappear_twice(
    store: &TransactionStore<'_>,
    account: &AccountId,
    first: &TransactionSyncRecord,
) {
    let second = TransactionSyncRecord::from_observation(
        first.state(),
        window(),
        at(20),
        Ok(complete(account, &["{\"x\":1}"])),
    );
    store
        .record_sync(account, first.state(), Some(at(10)), &second)
        .await
        .expect("사라짐");
    let third = TransactionSyncRecord::from_observation(
        second.state(),
        window(),
        at(30),
        Ok(complete(account, &[])),
    );
    store
        .record_sync(account, second.state(), Some(at(20)), &third)
        .await
        .expect("두 번째 사라짐");
    let state = store.state(account).await.expect("이력");
    assert_eq!(state.history_rows().len(), 2);
    assert_eq!(
        state
            .history_rows()
            .iter()
            .filter(|record_row| record_row.disappeared_at() == Some(at(20)))
            .count(),
        1
    );
    assert_eq!(
        state
            .history_rows()
            .iter()
            .filter(|record_row| record_row.disappeared_at() == Some(at(30)))
            .count(),
        1
    );
    assert_eq!(store.latest_sync(account).await.expect("상세"), Some(third));
}
fn partial_observation(account: &AccountId) -> TransactionRead {
    TransactionRead::Incomplete(IncompleteTransactions {
        account: account.clone(),
        window: window(),
        filter: TransactionFilter::all(),
        received: vec![
            ObservedTransactionBatch::try_new(vec![ObservedTransaction::new(
                row(),
                RawTransactionRow::new("  {\"same\":1} ".to_owned()),
            )])
            .expect("배치"),
            ObservedTransactionBatch::try_new(vec![]).expect("빈 배치"),
        ],
        quarantined: vec![QuarantinedTransaction::UnknownCurrency(vec![
            domain::UntrustedText::try_new("{\"currency\":\"EUR\"}").expect("격리"),
        ])],
        reason: IncompleteReason::PageInterrupted,
    })
}
fn port_errors() -> [application::PortError; 2] {
    use application::{
        AccessDenial, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RetryDelay,
        UnavailableCause,
    };
    [
        PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::RateLimited,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::RateLimited,
                retry: RetryDelay::Known(
                    domain::TimeSpan::try_new(
                        domain::SpanSeconds::new(7),
                        Nanosecond::new(123_456_789),
                    )
                    .expect("대기"),
                ),
            },
        ),
        PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::Forbidden,
            PortErrorKind::AccessDenied(AccessDenial::Forbidden),
        ),
    ]
}

#[test]
fn generic_transaction_latest_accepts_out_of_order_summaries() {
    use domain::{SyncOutcome, SyncRecord};
    let fixture = Fixture::new();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("DB");
    let store = TransactionStore::new(&database);
    runtime().block_on(async {
        let account = AccountId::new(41);
        let other = AccountId::new(42);
        let older = SyncRecord::new(at(11), SyncOutcome::Failed);
        let newer = SyncRecord::new(at(23), SyncOutcome::AccessBlocked);
        testkit_ports::contracts::scenarios::transaction_latest(
            &store,
            (&account, &other),
            (&older, &newer),
        )
        .await;
    });
}

#[test]
fn historical_legacy_summaries_preserve_inputs_without_replacing_latest_state() {
    use domain::{SyncOutcome, SyncRecord};
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("DB");
    runtime().block_on(async {
        let account = AccountId::new(43);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["original"])),
        );
        let latest = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(30),
            Ok(complete(&account, &[])),
        );
        let store = TransactionStore::new(&db);
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("처음");
        store
            .record_sync(&account, first.state(), Some(at(10)), &latest)
            .await
            .expect("최신");
        for (time, outcome) in [
            (15, SyncOutcome::Complete(first.state().clone())),
            (16, SyncOutcome::Partial(first.state().clone())),
            (17, SyncOutcome::Failed),
            (18, SyncOutcome::AccessBlocked),
        ] {
            store
                .record(&account, &SyncRecord::new(at(time), outcome))
                .await
                .expect("과거 보존");
        }
        assert_eq!(store.state(&account).await.expect("상태"), *latest.state());
        assert_eq!(
            store.latest(&account).await.expect("요약"),
            Some(latest.summary())
        );
        assert_eq!(
            store.latest_sync(&account).await.expect("상세"),
            Some(latest.clone())
        );
        let counts: (i64, i64) = db
            .read(|connection| {
                connection.query_row(crate::sql::transactions_tests::HISTORY_COUNTS, [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
            })
            .await
            .expect("과거 기록 보존");
        assert_eq!(counts, (4, 4));
        history_rejections(&db, &store, &account, &latest).await;
    });
}
#[test]
fn transaction_roundtrip_raw_duplicates_disappearance_and_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let account = AccountId::new(31);
    let db = Database::open(&path, &key(), &clock()).expect("열기");
    let initial = SettledRowSet::new(account.clone());
    let first = TransactionSyncRecord::from_observation(
        &initial,
        window(),
        at(10),
        Ok(complete(&account, &["{ \"x\":1.00 }", "{\"x\":1}"])),
    );
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        store
            .record_sync(&account, &initial, None, &first)
            .await
            .expect("저장");
        assert_eq!(
            store.latest_sync(&account).await.expect("상세"),
            Some(first.clone())
        );
        assert_eq!(
            store.latest(&account).await.expect("요약"),
            Some(first.summary())
        );
        assert_eq!(store.state(&account).await.expect("상태"), *first.state());
        assert_eq!(
            store
                .state(&account)
                .await
                .expect("중복")
                .history_rows()
                .len(),
            2
        );
        disappear_twice(&store, &account, &first).await;
    });
    runtime().block_on(db.close()).expect("닫기");
    let reopened = Database::open(&path, &key(), &clock()).expect("다시 열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&reopened);
        assert_eq!(
            store
                .state(&account)
                .await
                .expect("영속")
                .history_rows()
                .len(),
            2
        );
        assert_eq!(
            store.state(&AccountId::new(32)).await.expect("격리"),
            SettledRowSet::new(AccountId::new(32))
        );
    });
}
#[test]
fn transaction_cas_rejects_stale_state_clock_and_foreign_account() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(41);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("저장");
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect_err("거부");
        store
            .record_sync(&AccountId::new(42), &empty, None, &first)
            .await
            .expect_err("거부");
        let older = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(9),
            Ok(complete(&account, &[])),
        );
        store
            .record_sync(&account, first.state(), Some(at(10)), &older)
            .await
            .expect_err("거부");
        assert_eq!(
            store.latest_sync(&account).await.expect("보존"),
            Some(first)
        );
    });
}
#[test]
fn full_completion_cas_preserves_supplied_timestamp_and_accounts() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(51);
        assert_eq!(store.last_completed(&account).await.expect("처음"), None);
        store
            .record_completed(&account, None, at(11))
            .await
            .expect("완료");
        assert_eq!(
            store.last_completed(&account).await.expect("그대로"),
            Some(at(11))
        );
        store
            .record_completed(&account, None, at(12))
            .await
            .expect_err("거부");
        store
            .record_completed(&account, Some(at(11)), at(10))
            .await
            .expect_err("거부");
        assert_eq!(
            store.last_completed(&account).await.expect("보존"),
            Some(at(11))
        );
        assert_eq!(
            store
                .last_completed(&AccountId::new(52))
                .await
                .expect("격리"),
            None
        );
        store
            .record_completed(&account, Some(at(11)), at(11))
            .await
            .expect("동률");
    });
}

#[test]
fn transaction_write_failure_rolls_back_details_state_and_summary() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let account = AccountId::new(61);
        let store = TransactionStore::new(&db);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["{\"a\":1}"])),
        );
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("최초");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::FAIL_SOURCE))
            .await
            .expect("실패 주입");
        let second = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(20),
            Ok(complete(&account, &["{\"a\":2}"])),
        );
        store
            .record_sync(&account, first.state(), Some(at(10)), &second)
            .await
            .expect_err("거부");
        assert_eq!(
            store.latest_sync(&account).await.expect("상세 보존"),
            Some(first.clone())
        );
        assert_eq!(
            store.latest(&account).await.expect("요약 보존"),
            Some(first.summary())
        );
        assert_eq!(
            store.state(&account).await.expect("상태 보존"),
            *first.state()
        );
    });
}
#[test]
fn transaction_corruption_fails_closed() {
    for statement in [
        crate::sql::transactions_tests::BAD_CURRENCY,
        crate::sql::transactions_tests::BAD_QUANTITY,
        crate::sql::transactions_tests::BAD_TIME,
        crate::sql::transactions_tests::BAD_POSITION,
        crate::sql::transactions_tests::REMOVE_SOURCE,
        crate::sql::transactions_tests::REMOVE_ROW,
        crate::sql::transactions_tests::BAD_HEADER,
        crate::sql::transactions_tests::BAD_MODE,
    ] {
        let fixture = Fixture::new();
        let db =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        runtime().block_on(async {
            let store = TransactionStore::new(&db);
            let account = AccountId::new(71);
            let empty = SettledRowSet::new(account.clone());
            let record = TransactionSyncRecord::from_observation(
                &empty,
                window(),
                at(10),
                Ok(complete(&account, &["{}"])),
            );
            store
                .record_sync(&account, &empty, None, &record)
                .await
                .expect("저장");
            db.write(move |tx| tx.execute_batch(statement))
                .await
                .expect("손상");
            assert!(store.state(&account).await.is_err(), "{statement}");
            assert!(store.latest(&account).await.is_err(), "{statement}");
            assert!(store.latest_sync(&account).await.is_err(), "{statement}");
        });
    }
}
#[test]
fn full_completion_failure_and_corruption_do_not_change_transactions() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(81);
        store
            .record_completed(&account, None, at(11))
            .await
            .expect("처음");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::FAIL_COMPLETION))
            .await
            .expect("실패 주입");
        store
            .record_completed(&account, Some(at(11)), at(12))
            .await
            .expect_err("거부");
        assert_eq!(
            store.last_completed(&account).await.expect("보존"),
            Some(at(11))
        );
        assert_eq!(store.latest(&account).await.expect("별개"), None);
        let other = AccountId::new(82);
        store
            .record_completed(&other, None, at(13))
            .await
            .expect("다른 계좌");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::BAD_COMPLETION))
            .await
            .expect_err("트리거 실패로도 롤백");
        assert_eq!(
            store.last_completed(&account).await.expect("보존"),
            Some(at(11))
        );
    });
}
#[test]
fn full_completion_corruption_fails_closed() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(83);
        store
            .record_completed(&account, None, at(14))
            .await
            .expect("처음");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::BAD_COMPLETION))
            .await
            .expect("손상");
        store.last_completed(&account).await.expect_err("거부");
    });
}
#[test]
fn partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(91);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["{}", "{}"])),
        );
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("처음");
        let partial = partial_observation(&account);
        let record =
            TransactionSyncRecord::from_observation(first.state(), window(), at(20), Ok(partial));
        store
            .record_sync(&account, first.state(), Some(at(10)), &record)
            .await
            .expect("부분");
        assert_eq!(
            store.latest_sync(&account).await.expect("부분 복원"),
            Some(record)
        );
        assert_eq!(
            store.state(&account).await.expect("정상 유지"),
            *first.state()
        );
        let errors = port_errors();
        let mut previous = at(20);
        for (index, error) in errors.into_iter().enumerate() {
            let now = at(i64::try_from(index)
                .expect("번호")
                .checked_add(30)
                .expect("시각"));
            let failure_record =
                TransactionSyncRecord::from_observation(first.state(), window(), now, Err(error));
            store
                .record_sync(&account, first.state(), Some(previous), &failure_record)
                .await
                .expect("오류 저장");
            assert_eq!(
                store.latest_sync(&account).await.expect("오류 복원"),
                Some(failure_record.clone())
            );
            assert_eq!(
                store.latest(&account).await.expect("오류 요약"),
                Some(failure_record.summary())
            );
            assert_eq!(
                store.state(&account).await.expect("정상 유지"),
                *first.state()
            );
            previous = now;
        }
    });
}

fn varied_row() -> Transaction {
    let mut data = row().data().clone();
    data.kind =
        TransactionKind::new(domain::TransactionCode::try_new("FUTURE_KIND").expect("종류"));
    data.market = Market::new(domain::MarketCode::try_new("FUTURE_MARKET").expect("시장"));
    data.symbol = Some(domain::Symbol::try_new("TEST").expect("종목"));
    data.asset_class = Some(domain::AssetClass::new(
        domain::AssetClassCode::try_new("FUTURE_ASSET").expect("자산"),
    ));
    data.isin = Some(domain::IsinCode::try_new("TEST12345").expect("ISIN"));
    data.quantity = Some(
        domain::Quantity::parse(
            &NumericInput::try_new("7.123456789012345678901234567").expect("수량 입력"),
        )
        .expect("수량"),
    );
    data.filled_price = Some(domain::TransactionPrice::try_new(money("8.7654321")).expect("단가"));
    data.exchange_rate = Some(
        domain::Ratio::parse(&NumericInput::try_new("0.123456789").expect("환율 입력"))
            .expect("환율"),
    );
    data.settlement = Settlement::Pending;
    data.transaction_time = None;
    data.cancellation = Cancellation::Cancelled;
    Transaction::try_new(data).expect("서로 다른 필드")
}
#[test]
fn normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let account = AccountId::new(101);
        let store = TransactionStore::new(&db);
        let empty = SettledRowSet::new(account.clone());
        let observed = ObservedTransaction::new(
            varied_row(),
            RawTransactionRow::new(" {\"opaque\":\"원문\\uAC00\",\"number\":1.00} ".to_owned()),
        );
        let observation = TransactionRead::Complete(
            CompleteTransactionObservation::after_last_page_without_quarantine(
                account.clone(),
                window(),
                TransactionFilter::all(),
                ObservedTransactionBatch::try_new(vec![observed]).expect("배치"),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("전체"),
        );
        let record =
            TransactionSyncRecord::from_observation(&empty, window(), at(10), Ok(observation));
        store
            .record_sync(&account, &empty, None, &record)
            .await
            .expect("저장");
        assert_eq!(
            store.latest_sync(&account).await.expect("정규화 복원"),
            Some(record.clone())
        );
        assert_eq!(
            store
                .state(&account)
                .await
                .expect("미결제")
                .pending_rows()
                .as_slice(),
            &[varied_row()]
        );
        let legacy =
            domain::SyncRecord::new(at(20), domain::SyncOutcome::Partial(record.state().clone()));
        store.record(&account, &legacy).await.expect("기존 포트");
        assert_eq!(
            store.latest(&account).await.expect("기존 요약"),
            Some(legacy)
        );
        assert_eq!(
            store.latest_sync(&account).await.expect("상세 보존"),
            Some(record)
        );
    });
}
#[test]
fn all_transaction_runtime_statements_prepare() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("메모리");
    crate::migration::run(&mut connection, at(1)).expect("마이그레이션");
    for statement in [
        crate::sql::transactions::INSERT_ENTRY,
        crate::sql::transactions::LATEST,
        crate::sql::transactions::LATEST_SYNC,
        crate::sql::transactions::INSERT_ROW,
        crate::sql::transactions::ROWS,
        crate::sql::transactions::INSERT_SOURCE,
        crate::sql::transactions::ROW_COUNT,
        crate::sql::transactions::SOURCE_COUNT,
        crate::sql::transactions::COMPLETION,
        crate::sql::transactions::COMPLETE,
    ] {
        connection.prepare(statement).expect("SQL 준비");
    }
}

#[test]
fn completion_advances_and_transaction_sync_keeps_completion_unchanged() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(111);
        store
            .record_completed(&account, None, at(11))
            .await
            .expect("처음");
        store
            .record_completed(&account, Some(at(11)), at(12))
            .await
            .expect("전진");
        assert_eq!(
            store.last_completed(&account).await.expect("정확한 새 값"),
            Some(at(12))
        );
        let empty = SettledRowSet::new(account.clone());
        let record = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(13),
            Ok(complete(&account, &[])),
        );
        store
            .record_sync(&account, &empty, None, &record)
            .await
            .expect("관측");
        assert_eq!(
            store
                .last_completed(&account)
                .await
                .expect("최근 관측과 독립"),
            Some(at(12))
        );
    });
}
#[test]
fn every_error_category_and_equal_observation_clock_preserve_the_contract() {
    use application::{
        BrokerCapability, HttpStatus, MissingData, PortError, PortErrorKind, ReadEndpoint,
        RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
    };
    let errors = [
        PortErrorKind::NotFound(MissingData::Order),
        PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
        PortErrorKind::Unsupported(BrokerCapability::DailyCandles),
        PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
        PortErrorKind::Unavailable {
            cause: UnavailableCause::Network,
            retry: RetryDelay::Unknown,
        },
    ];
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(121);
        let state = SettledRowSet::new(account.clone());
        let mut previous = None;
        for kind in errors {
            let record = TransactionSyncRecord::from_observation(
                &state,
                window(),
                at(10),
                Err(PortError::new(
                    ReadEndpoint::Transactions,
                    HttpStatus::NotReceived,
                    kind,
                )),
            );
            store
                .record_sync(&account, &state, previous, &record)
                .await
                .expect("같은 시각 마지막 저장");
            assert_eq!(
                store.latest_sync(&account).await.expect("오류 복원"),
                Some(record.clone())
            );
            assert_eq!(
                store.latest(&account).await.expect("요약"),
                Some(record.summary())
            );
            previous = Some(at(10));
        }
        let outdated_record = TransactionSyncRecord::from_observation(
            &state,
            window(),
            at(11),
            Ok(complete(&account, &[])),
        );
        store
            .record_sync(&account, &state, None, &outdated_record)
            .await
            .expect_err("상태가 같아도 최신 상세 시각 충돌");
    });
}

fn usd_row() -> Transaction {
    let mut data = varied_row().data().clone();
    data.currency = Currency::Usd;
    let parse = |text: &str| {
        Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Usd).expect("달러")
    };
    data.amount = Some(parse("111.123"));
    data.commission = parse("4.234");
    data.tax = parse("5.345");
    data.net_amount = Some(parse("101.544"));
    data.filled_price = Some(domain::TransactionPrice::try_new(parse("9.876")).expect("단가"));
    data.settlement = Settlement::Settled;
    data.cancellation = Cancellation::Regular;
    Transaction::try_new(data).expect("달러 거래")
}
#[test]
fn usd_values_and_rejected_scope_observation_roundtrip() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(131);
        let empty = SettledRowSet::new(account.clone());
        let response = TransactionRead::Complete(
            CompleteTransactionObservation::after_last_page_without_quarantine(
                account.clone(),
                window(),
                TransactionFilter::all(),
                ObservedTransactionBatch::try_new(vec![ObservedTransaction::new(
                    usd_row(),
                    RawTransactionRow::new("{\"currency\":\"USD\"}".to_owned()),
                )])
                .expect("배치"),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("전체"),
        );
        let first = TransactionSyncRecord::from_observation(&empty, window(), at(10), Ok(response));
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("달러 저장");
        assert_eq!(
            store.latest_sync(&account).await.expect("달러 복원"),
            Some(first.clone())
        );
        let rejected = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(11),
            Ok(complete(&AccountId::new(132), &["{\"account\":132}"])),
        );
        store
            .record_sync(&account, first.state(), Some(at(10)), &rejected)
            .await
            .expect("거부 관측 보존");
        assert_eq!(
            store.latest_sync(&account).await.expect("거부 복원"),
            Some(rejected)
        );
        assert_eq!(
            store.state(&account).await.expect("정상 상태 유지"),
            *first.state()
        );
        assert_eq!(
            store
                .latest_sync(&AccountId::new(132))
                .await
                .expect("계좌 격리"),
            None
        );
    });
}

#[test]
fn failed_record_timestamp_before_preserved_state_fails_closed() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(141);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        store
            .record_sync(&account, &empty, None, &first)
            .await
            .expect("정상");
        let failed = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(20),
            Err(port_errors()[0]),
        );
        store
            .record_sync(&account, first.state(), Some(at(10)), &failed)
            .await
            .expect("실패");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::REVERSE_FAILED_TIME))
            .await
            .expect("유효하지만 역행하는 시각 손상");
        store
            .latest_sync(&account)
            .await
            .expect_err("정상 상태 시각보다 오래된 실패 기록");
    });
}
#[test]
fn legacy_summary_timestamp_before_preserved_state_fails_closed() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(151);
        let empty = SettledRowSet::new(account.clone());
        let first = TransactionSyncRecord::from_observation(
            &empty,
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        let summary =
            domain::SyncRecord::new(at(10), domain::SyncOutcome::Complete(first.state().clone()));
        store
            .record(&account, &summary)
            .await
            .expect("기존 포트 정상 저장");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::REVERSE_FIRST_TIME))
            .await
            .expect("유효하지만 역행하는 시각 손상");
        store
            .latest(&account)
            .await
            .expect_err("정상 상태보다 오래된 요약 시각");
    });
}

fn empty_state(account: &AccountId, at: Timestamp) -> SettledRowSet {
    SettledRowSet::try_restore(
        account.clone(),
        vec![],
        domain::TransactionBatch::try_new(vec![]).expect("빈 배치"),
        Some(at),
    )
    .expect("빈 정상 상태")
}
#[test]
fn state_only_cas_rejects_legacy_complete_interleaving() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(161);
        let first = TransactionSyncRecord::from_observation(
            &SettledRowSet::new(account.clone()),
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        store
            .record_sync(&account, &SettledRowSet::new(account.clone()), None, &first)
            .await
            .expect("X 저장");
        let replacement = domain::SyncRecord::new(
            at(20),
            domain::SyncOutcome::Complete(empty_state(&account, at(20))),
        );
        store
            .record(&account, &replacement)
            .await
            .expect("Y로 교체");
        assert_eq!(
            store.latest_sync(&account).await.expect("상세 시각 유지"),
            Some(first.clone())
        );
        let outdated = TransactionSyncRecord::from_observation(
            first.state(),
            window(),
            at(30),
            Ok(complete(&account, &[])),
        );
        store
            .record_sync(&account, first.state(), Some(at(10)), &outdated)
            .await
            .expect_err("정상 상태만 달라도 CAS 거부");
        assert_eq!(
            store.state(&account).await.expect("Y 보존"),
            empty_state(&account, at(20))
        );
        assert_eq!(
            store.latest(&account).await.expect("요약 보존"),
            Some(replacement)
        );
    });
}
#[test]
fn legacy_partial_rejects_changed_state_and_preserves_normal_state() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(171);
        let first = TransactionSyncRecord::from_observation(
            &SettledRowSet::new(account.clone()),
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        store
            .record_sync(&account, &SettledRowSet::new(account.clone()), None, &first)
            .await
            .expect("정상 저장");
        let changed = domain::SyncRecord::new(
            at(20),
            domain::SyncOutcome::Partial(empty_state(&account, at(20))),
        );
        store
            .record(&account, &changed)
            .await
            .expect_err("불완전 요약이 정상 상태를 바꿀 수 없음");
        assert_eq!(
            store.state(&account).await.expect("정상 보존"),
            *first.state()
        );
        assert_eq!(
            store.latest(&account).await.expect("요약 보존"),
            Some(first.summary())
        );
        let unchanged =
            domain::SyncRecord::new(at(21), domain::SyncOutcome::Partial(first.state().clone()));
        store
            .record(&account, &unchanged)
            .await
            .expect("같은 정상 상태의 불완전 요약");
        assert_eq!(
            store.latest(&account).await.expect("불완전 결과 보존"),
            Some(unchanged)
        );
        assert_eq!(
            store.latest_sync(&account).await.expect("상세 보존"),
            Some(first)
        );
    });
}
#[test]
fn changed_state_partial_summary_fails_closed_on_read() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
    runtime().block_on(async {
        let store = TransactionStore::new(&db);
        let account = AccountId::new(181);
        let first = TransactionSyncRecord::from_observation(
            &SettledRowSet::new(account.clone()),
            window(),
            at(10),
            Ok(complete(&account, &["{}"])),
        );
        store
            .record_sync(&account, &SettledRowSet::new(account.clone()), None, &first)
            .await
            .expect("정상 저장");
        let changed = domain::SyncRecord::new(
            at(20),
            domain::SyncOutcome::Complete(empty_state(&account, at(20))),
        );
        store
            .record(&account, &changed)
            .await
            .expect("완전 요약은 교체 허용");
        db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::PARTIAL_WITH_CHANGED_STATE))
            .await
            .expect("부분 태그 손상");
        store
            .state(&account)
            .await
            .expect_err("부분 요약의 이전 상태 불일치");
    });
}

async fn history_rejections(
    db: &Database,
    store: &TransactionStore<'_>,
    account: &AccountId,
    latest: &TransactionSyncRecord,
) {
    let foreign = domain::SyncRecord::new(
        at(19),
        domain::SyncOutcome::Complete(empty_state(&AccountId::new(44), at(12))),
    );
    store
        .record(account, &foreign)
        .await
        .expect_err("과거도 계좌 검증");
    let stale = TransactionSyncRecord::from_observation(
        latest.state(),
        window(),
        at(29),
        Ok(complete(account, &[])),
    );
    store
        .record_sync(account, latest.state(), Some(at(30)), &stale)
        .await
        .expect_err("오래된 상세 거부");
    db.write(|tx| tx.execute_batch(crate::sql::transactions_tests::CORRUPT_HISTORY))
        .await
        .expect("손상 주입");
    store
        .record(
            account,
            &domain::SyncRecord::new(at(20), domain::SyncOutcome::Failed),
        )
        .await
        .expect_err("과거 저장 재검증");
    let counts_after: (i64, i64) = db
        .read(|connection| {
            connection.query_row(crate::sql::transactions_tests::HISTORY_COUNTS, [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
        })
        .await
        .expect("롤백");
    assert_eq!(counts_after, (4, 4));
}
