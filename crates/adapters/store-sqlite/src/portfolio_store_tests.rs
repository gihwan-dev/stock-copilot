use crate::{
    Database, DatabasePath, HoldingsStore, SharedDatabase, SqlitePortfolioStore,
    SqliteRepositories,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    InstrumentIdIssuer, InstrumentMappingRepository, InstrumentMasterAuthority,
    InstrumentMasterSession, PortfolioRepository,
};
use domain::{
    AccountId, BuyingPower, Currency, InstrumentSource, IsinCode, KnownMarket, MappingAssignment,
    Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio, Symbol, SyncOutcome, SyncRecord,
    Timestamp, UnixSeconds,
};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
}

fn holding() -> domain::Holding {
    let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
        listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
        instrument_id: domain::InstrumentId::try_new("fixture-hold").expect("ID"),
        symbol: Symbol::try_new("AAPL").expect("심볼"),
        name: domain::DisplayInstrumentName::try_new("애플").expect("이름"),
        security_type: domain::MasterSecurityType::from_code(
            domain::MasterCode::try_new("STOCK").expect("종류"),
        ),
        currency: Currency::Usd,
        listing_status: domain::ListingStatus::Active,
        common_share: domain::CommonShare::Common,
        korean_detail: None,
        observed_at: at(1_790_000_000),
    })
    .expect("마스터");
    domain::Holding::new(
        master.order_instrument().expect("종목"),
        domain::Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
        domain::AcquisitionPrice::try_new(money("11", Currency::Usd)).expect("원가"),
        domain::HoldingObservation::new(
            Some(
                domain::Price::parse(&NumericInput::try_new("17").expect("입력"), Currency::Usd)
                    .expect("가격"),
            ),
            None,
            at(1_790_000_001),
        ),
    )
    .expect("보유")
}

fn record(seconds: i64, outcome: SyncOutcome<Portfolio>) -> SyncRecord<Portfolio> {
    SyncRecord::new(at(seconds), outcome)
}

fn portfolio() -> Portfolio {
    let power = |text, currency| BuyingPower::new(money(text, currency), at(1_790_000_002));
    Portfolio::new(
        vec![holding()],
        power("137", Currency::Krw),
        power("29", Currency::Usd),
    )
    .expect("포트폴리오")
}

#[test]
fn portfolio_store_persists_holdings_and_serves_the_latest_record_per_account() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let store = SqlitePortfolioStore::new(SqliteRepositories::new(shared.clone()));
    let account = AccountId::new(7);
    runtime().block_on(async {
        let complete = record(1_790_000_010, SyncOutcome::Complete(portfolio()));
        let _closed = store.record(&account, &complete).await.expect_err("미설치");
        assert_eq!(store.latest(&account).await, Ok(None));
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        store.record(&account, &complete).await.expect("저장");
        assert_eq!(store.latest(&account).await, Ok(Some(complete.clone())));
        assert_eq!(store.latest(&AccountId::new(8)).await, Ok(None));
        let failed = record(1_790_000_020, SyncOutcome::Failed);
        store.record(&account, &failed).await.expect("실패 기록");
        assert_eq!(store.latest(&account).await, Ok(Some(failed)));
        let guard = shared.read().await;
        let history = HoldingsStore::new(guard.as_ref().expect("열림"))
            .holdings_history(account.clone())
            .await
            .expect("이력");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].observed_at, at(1_790_000_010));
        assert_eq!(history[0].holdings.len(), 1);
        assert_eq!(history[0].holdings[0].symbol.as_str(), "AAPL");
    });
}

#[test]
fn master_authority_and_issuer_follow_the_installed_database() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    let source = InstrumentSource::new(
        KnownMarket::Kr,
        Symbol::try_new("005930").expect("심볼"),
        IsinCode::try_new("KR7005930003").expect("ISIN"),
    );
    runtime().block_on(async {
        let _closed = repositories
            .begin(KnownMarket::Kr)
            .await
            .expect_err("미설치");
        let _missing = repositories.issue().await.expect_err("미설치");
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        let session = repositories.begin(KnownMarket::Kr).await.expect("세션");
        session.verify_observation(&[]).await.expect("확인");
        let issued = repositories.issue().await.expect("발급");
        let first = match session.ensure(&source, issued).await.expect("대응") {
            MappingAssignment::Issued(id) => id,
            MappingAssignment::Existing(id) => panic!("새 대응이어야 함: {id:?}"),
        };
        assert_eq!(session.resolve(&source).await, Ok(first.clone()));
        let again = repositories.issue().await.expect("발급");
        assert_eq!(
            session.ensure(&source, again).await,
            Ok(MappingAssignment::Existing(first))
        );
    });
}
