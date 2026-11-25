use std::sync::Arc;

use application::{
    AllocationAxis, PerformancePeriod, PortfolioToolReads, SampleSufficiency, SecretValue,
};
use domain::{
    AccountId, AcquisitionPrice, BuyingPower, CommonShare, Currency, DisplayInstrumentName,
    Holding, HoldingObservation, InstrumentId, KoreanMasterDetail, LiquidationTrading,
    ListingStatus, MaskedAccountNumber, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Portfolio,
    Price, Quantity, Symbol, SyncOutcome, SyncRecord, Timestamp, TradingSuspension, UnixSeconds,
};
use store_sqlite::{Database, DatabasePath, SharedDatabase, SqliteRepositories};
use testkit_ports::memory::{MemoryPortfolioRepository, MemoryReply};

use super::{CatalogPortfolio, PortfolioToolSource, StoredPortfolioTools};
use crate::broker_link::AccountSlot;
use crate::holdings_link::InstrumentDirectory;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("숫자"), currency).expect("금액")
}

fn samsung() -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new("005930").expect("심볼"),
        instrument_id: InstrumentId::try_new("inst-1").expect("ID"),
        name: DisplayInstrumentName::try_new("삼성전자").expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
        currency: Currency::Krw,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: at(1_790_000_000),
    })
    .expect("마스터")
}

fn portfolio() -> Portfolio {
    let holding = Holding::new(
        samsung().order_instrument().expect("종목"),
        Quantity::parse(&NumericInput::try_new("10").expect("숫자")).expect("수량"),
        AcquisitionPrice::try_new(money("60000", Currency::Krw)).expect("평단"),
        HoldingObservation::new(
            Some(
                Price::parse(
                    &NumericInput::try_new("70000").expect("숫자"),
                    Currency::Krw,
                )
                .expect("가격"),
            ),
            None,
            at(1_790_000_000),
        ),
    )
    .expect("보유");
    Portfolio::new(
        vec![holding],
        BuyingPower::new(money("300000", Currency::Krw), at(1_790_000_000)),
        BuyingPower::new(money("0", Currency::Usd), at(1_790_000_000)),
    )
    .expect("포트폴리오")
}

struct Fixture {
    tools: StoredPortfolioTools<MemoryPortfolioRepository, SqliteRepositories>,
    portfolios: Arc<MemoryPortfolioRepository>,
    accounts: AccountSlot,
    shared: SharedDatabase,
    directory: std::path::PathBuf,
}

fn fixture(name: &str) -> Fixture {
    let directory = std::path::PathBuf::from("../../../target/fin-hold")
        .join(format!("{name}-{}", std::process::id()));
    let shared = SharedDatabase::new();
    let path = DatabasePath::new(directory.join("store.db"));
    crate::database::prepare_directory(&path).expect("폴더");
    let clock = testkit_ports::SteppedClock::new(at(1_790_000_000));
    let database = Database::open(&path, &SecretValue::new(vec![9; 32]), &clock).expect("열기");
    assert!(tauri::async_runtime::block_on(shared.install(database)).is_none());
    let portfolios = Arc::new(MemoryPortfolioRepository::default());
    let accounts = AccountSlot::default();
    let names = InstrumentDirectory::default();
    names.replace(&[samsung()]);
    Fixture {
        tools: StoredPortfolioTools::new(
            Arc::clone(&portfolios),
            Arc::new(SqliteRepositories::new(shared.clone())),
            accounts.clone(),
            names,
        ),
        portfolios,
        accounts,
        shared,
        directory,
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _closed = tauri::async_runtime::block_on(self.shared.close());
        let _cleaned = std::fs::remove_dir_all(&self.directory);
    }
}

fn select(fixture: &Fixture, outcome: &SyncOutcome<Portfolio>) {
    let account = AccountId::new(413);
    let _changed = fixture.accounts.replace(vec![account.clone()]);
    fixture.accounts.label(
        &account,
        &MaskedAccountNumber::try_new("00000000731").expect("계좌"),
    );
    for _ in 0_u8..8_u8 {
        fixture
            .portfolios
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(1_790_000_100),
                outcome.clone(),
            )))));
    }
}

#[test]
fn without_a_selected_account_every_tool_reports_no_data() {
    let fixture = fixture("tools-empty");
    tauri::async_runtime::block_on(async {
        assert_eq!(
            PortfolioToolSource::portfolio(&fixture.tools).await,
            Ok(None)
        );
        assert_eq!(
            PortfolioToolSource::allocation(&fixture.tools).await,
            Ok(None)
        );
        assert_eq!(
            PortfolioToolSource::transactions(&fixture.tools).await,
            Ok(None)
        );
        assert_eq!(
            PortfolioToolSource::performance(&fixture.tools, PerformancePeriod::OneMonth).await,
            Ok(None)
        );
        assert_eq!(fixture.tools.held().await, Vec::new());
    });
}

#[test]
fn stored_holdings_feed_the_four_portfolio_tools_with_masked_account_and_names() {
    let fixture = fixture("tools-filled");
    select(&fixture, &SyncOutcome::Complete(portfolio()));
    tauri::async_runtime::block_on(async {
        let view = PortfolioToolSource::portfolio(&fixture.tools)
            .await
            .expect("조회")
            .expect("현황");
        assert_eq!(view.account.to_string(), "0731");
        assert_eq!(view.holdings.len(), 1);
        assert_eq!(view.holdings[0].name.as_str(), "삼성전자");
        assert_eq!(view.holdings[0].symbol.as_str(), "005930");
        assert!(view.holdings[0].weight.is_some(), "비중은 domain 배분에서");
        assert_eq!(view.buying_power.len(), 2);
        let allocation = PortfolioToolSource::allocation(&fixture.tools)
            .await
            .expect("조회")
            .expect("배분");
        assert!(
            allocation
                .lines
                .iter()
                .any(|line| line.axis == AllocationAxis::Currency && line.label.as_str() == "KRW")
        );
        let performance =
            PortfolioToolSource::performance(&fixture.tools, PerformancePeriod::OneMonth)
                .await
                .expect("조회")
                .expect("성과");
        assert_eq!(performance.sample, SampleSufficiency::Insufficient);
        assert_eq!(
            PortfolioToolSource::transactions(&fixture.tools).await,
            Ok(None)
        );
        assert_eq!(
            fixture.tools.held().await,
            vec![Symbol::try_new("005930").expect("종목")]
        );
    });
}

#[test]
fn failed_sync_is_an_unavailable_tool_and_an_unconnected_catalog_stays_closed() {
    let fixture = fixture("tools-failed");
    select(&fixture, &SyncOutcome::Failed);
    tauri::async_runtime::block_on(async {
        let _failed = PortfolioToolSource::portfolio(&fixture.tools)
            .await
            .expect_err("실패 기록");
        let unconnected = CatalogPortfolio::default();
        let _closed = unconnected.portfolio().await.expect_err("연결 전");
        assert_eq!(unconnected.held().await, Vec::new());
    });
}
