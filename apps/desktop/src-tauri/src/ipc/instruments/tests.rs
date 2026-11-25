use std::fs;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, RwLock};

use application::{
    Clock, EventBus, PortError, QuoteObservation, StoredView, SystemFailure, SystemPort,
    SystemPortError, Watchlist, WatchlistEntry, WatchlistRepository, WatchlistStoreOutcome,
};
use domain::{
    AccountId, AcquisitionPrice, BuyingPower, Candle, CandleObservation, CandlePeriod,
    CandleStatus, CandleTime, CommonShare, Currency, DisplayInstrumentName, Holding,
    HoldingObservation, InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus,
    MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, Money, Nanosecond,
    NumericInput, NxtTrading, OffsetSeconds, Ohlcv, Portfolio, Price, PriceAdjustment, Quantity,
    Quote, QuoteInstrument, Symbol, SyncOutcome, SyncRecord, TimeInterval, Timestamp,
    TradingSuspension, UnixSeconds,
};
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::{MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply};

use super::{
    GetInstrumentsRequest, InstrumentMarket, InstrumentRequest, InstrumentsIpcState,
    InstrumentsScope, UiInstrumentOrigin, UiWatchlistOutcome, UiWatchlistRejection, add_watchlist,
    get_instrument_detail, get_instruments, remove_watchlist,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;
use crate::ipc::portfolio::UiDirection;
use crate::ipc::state::{BoxFuture, LoadedSettings, ScreenBackend, ScreenState, SecretPresence};

const RECORDED_AT: i64 = 1_795_564_800;
const KST: i32 = 32_400;
const KST_MIDNIGHT_TODAY: i64 = 1_795_532_400;
const DAY: i64 = 86_400;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn local(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(KST),
    )
    .expect("시각")
}

fn quoted_at() -> Timestamp {
    at(RECORDED_AT.checked_add(3_600).expect("시각"))
}

fn number(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("숫자")
}

fn money(text: &str) -> Money {
    Money::parse(&number(text), Currency::Krw).expect("금액")
}

fn price(text: &str) -> Price {
    Price::parse(&number(text), Currency::Krw).expect("가격")
}

fn master(symbol: &str, id: &str, name: &str, listing: ListingStatus) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new(symbol).expect("심볼"),
        instrument_id: InstrumentId::try_new(id).expect("ID"),
        name: DisplayInstrumentName::try_new(name).expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
        currency: Currency::Krw,
        listing_status: listing,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: at(RECORDED_AT),
    })
    .expect("마스터")
}

fn samsung() -> MasterInstrument {
    master("005930", "inst-1", "삼성전자", ListingStatus::Active)
}

fn hynix() -> MasterInstrument {
    master("000660", "inst-2", "SK하이닉스", ListingStatus::Active)
}

fn delisted() -> MasterInstrument {
    master("999990", "inst-9", "상장폐지", ListingStatus::Delisted)
}

fn directory() -> InstrumentDirectory {
    let names = InstrumentDirectory::default();
    names.replace(&[samsung(), hynix(), delisted()]);
    names
}

fn candle(instrument: &QuoteInstrument, day_start: i64, close: &str) -> Candle {
    let value = price(close);
    Candle::try_new(
        instrument.instrument_id().clone(),
        CandleTime::try_new(local(day_start), CandlePeriod::Daily).expect("일봉"),
        PriceAdjustment::Raw,
        Ohlcv::try_new(
            value,
            value,
            value,
            value,
            Quantity::parse(&number("0")).expect("거래량"),
        )
        .expect("OHLCV"),
        CandleObservation::new(quoted_at(), CandleStatus::Finalized),
    )
    .expect("봉")
}

#[derive(Default)]
struct FakeMarket {
    unpriced: RwLock<Option<()>>,
    daily_calls: RwLock<Vec<Symbol>>,
}

fn port_failure() -> PortError {
    PortError::new(
        application::ReadEndpoint::Quotes,
        application::HttpStatus::NotReceived,
        application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation),
    )
}

impl InstrumentMarket for FakeMarket {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        let unpriced = self.unpriced.read().expect("잠금").is_some();
        Box::pin(async move {
            if unpriced {
                return Err(port_failure());
            }
            Ok(instruments
                .into_iter()
                .filter(|instrument| instrument.symbol().as_str() == "005930")
                .map(|instrument| QuoteObservation {
                    result: Ok(Quote::try_new(
                        instrument.symbol().clone(),
                        Currency::Krw,
                        Some(price("71400")),
                        None,
                        quoted_at(),
                    )
                    .expect("시세")),
                    instrument,
                })
                .collect())
        })
    }

    fn daily(
        &self,
        instrument: QuoteInstrument,
        _window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        self.daily_calls
            .write()
            .expect("잠금")
            .push(instrument.symbol().clone());
        let yesterday = KST_MIDNIGHT_TODAY.checked_sub(DAY).expect("어제");
        Box::pin(std::future::ready(Ok(vec![
            candle(&instrument, KST_MIDNIGHT_TODAY, "71400"),
            candle(&instrument, yesterday, "70000"),
        ])))
    }
}

struct MemoryWatchlist(RwLock<Watchlist>);

impl WatchlistRepository for MemoryWatchlist {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
        std::future::ready(Ok(self.0.read().expect("잠금").clone()))
    }

    fn store(
        &self,
        previous: &Watchlist,
        next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
        let mut current = self.0.write().expect("잠금");
        let outcome = if *current == *previous {
            *current = next.clone();
            WatchlistStoreOutcome::Stored
        } else {
            WatchlistStoreOutcome::Conflict
        };
        std::future::ready(Ok(outcome))
    }
}

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        quoted_at()
    }
}

fn holding() -> Holding {
    Holding::new(
        samsung().order_instrument().expect("종목"),
        Quantity::parse(&number("10")).expect("수량"),
        AcquisitionPrice::try_new(money("65000")).expect("평단"),
        HoldingObservation::new(Some(price("71400")), None, at(RECORDED_AT)),
    )
    .expect("보유")
}

fn portfolio() -> Portfolio {
    Portfolio::new(
        vec![holding()],
        BuyingPower::new(money("5000000"), at(RECORDED_AT)),
        BuyingPower::new(
            Money::parse(&number("0"), Currency::Usd).expect("달러"),
            at(RECORDED_AT),
        ),
    )
    .expect("포트폴리오")
}

struct HoldingsBackend {
    portfolios: MemoryPortfolioRepository,
    market: MemoryMarketDataRepository,
}

fn holdings_backend() -> HoldingsBackend {
    let portfolios = MemoryPortfolioRepository::default();
    let market = MemoryMarketDataRepository::default();
    for _ in 0_u8..8_u8 {
        portfolios
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(RECORDED_AT),
                SyncOutcome::Complete(portfolio()),
            )))));
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
    }
    HoldingsBackend { portfolios, market }
}

fn unused<T: Send + 'static>() -> BoxFuture<'static, Result<T, SystemPortError>> {
    Box::pin(std::future::ready(Err(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::InvalidData,
    ))))
}

impl ScreenBackend for HoldingsBackend {
    fn load_settings(&self) -> BoxFuture<'_, Result<LoadedSettings, SystemPortError>> {
        Box::pin(std::future::ready(Ok(LoadedSettings {
            profile: None,
            risk_settings: None,
            toss_connected: SecretPresence::Configured,
            dart_key: SecretPresence::NotConfigured,
            fred_key: SecretPresence::NotConfigured,
            account_count: 1,
        })))
    }

    fn change_risk_settings(
        &self,
        _requested: domain::RiskSettings,
    ) -> BoxFuture<'_, Result<application::RiskSettingsChangeOutcome, SystemPortError>> {
        unused()
    }

    fn load_portfolio_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::PortfolioValuation>, SystemPortError>> {
        Box::pin(async move {
            application::GetPortfolioView::new(&self.portfolios, &self.market)
                .execute(&AccountId::new(413))
                .await
        })
    }

    fn load_allocation_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::Allocation>, SystemPortError>> {
        unused()
    }

    fn load_transactions(
        &self,
    ) -> BoxFuture<'_, Result<Option<SyncRecord<domain::SettledRowSet>>, SystemPortError>> {
        unused()
    }

    fn update_profile(
        &self,
        _requested: application::InvestorProfileRequest,
    ) -> BoxFuture<'_, Result<application::RiskSettingsChangeOutcome, SystemPortError>> {
        unused()
    }
}

struct Harness {
    app: tauri::App<tauri::test::MockRuntime>,
    market: Arc<FakeMarket>,
    watchlist: Arc<MemoryWatchlist>,
}

fn harness() -> Harness {
    let market = Arc::new(FakeMarket::default());
    let watchlist = Arc::new(MemoryWatchlist(RwLock::new(
        Watchlist::try_from_entries(vec![WatchlistEntry::new(
            InstrumentId::try_new("inst-2").expect("ID"),
            at(RECORDED_AT),
        )])
        .expect("관심 종목"),
    )));
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(
        &app,
        InstrumentsIpcState::new(
            Arc::<FakeMarket>::clone(&market),
            Arc::clone(&watchlist),
            Arc::new(FixedClock),
            Arc::new(EventBus::default()),
        ),
    );
    let _screen = tauri::Manager::manage(&app, ScreenState::new(Arc::new(holdings_backend())));
    let _names = tauri::Manager::manage(&app, directory());
    Harness {
        app,
        market,
        watchlist,
    }
}

fn list_request() -> GetInstrumentsRequest {
    GetInstrumentsRequest {
        scope: InstrumentsScope::Tracked,
    }
}

fn symbol(text: &str) -> InstrumentRequest {
    InstrumentRequest {
        symbol: String::from(text),
    }
}

fn assert_snapshot<T: serde::Serialize>(name: &str, value: &T) {
    let path = Path::new("ipc/responses").join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

#[test]
fn list_puts_holdings_before_watchlist_and_computes_today_from_the_previous_daily_close() {
    let setup = harness();
    let view =
        tauri::async_runtime::block_on(get_instruments(setup.app.handle().clone(), list_request()))
            .expect("목록");
    assert_eq!(view.rows.len(), 2);
    assert_eq!(view.rows[0].symbol, "005930");
    assert_eq!(view.rows[0].origin, UiInstrumentOrigin::Holding);
    let today = view.rows[0].today.as_ref().expect("당일 등락");
    assert_eq!(today.rate, "2.00");
    assert_eq!(today.direction, UiDirection::Up);
    assert_eq!(view.rows[1].symbol, "000660");
    assert_eq!(view.rows[1].origin, UiInstrumentOrigin::Watchlist);
    assert_eq!(view.rows[1].today, None);
    assert_snapshot("get_instruments", &view);
    let _again =
        tauri::async_runtime::block_on(get_instruments(setup.app.handle().clone(), list_request()))
            .expect("다시");
    assert_eq!(setup.market.daily_calls.read().expect("잠금").len(), 1);
}

#[test]
fn list_without_quotes_keeps_rows_and_leaves_today_empty_instead_of_inventing_it() {
    let setup = harness();
    *setup.market.unpriced.write().expect("잠금") = Some(());
    let view =
        tauri::async_runtime::block_on(get_instruments(setup.app.handle().clone(), list_request()))
            .expect("목록");
    assert_eq!(view.rows.len(), 2);
    assert!(view.rows.iter().all(|row| row.today.is_none()));
    let detail = tauri::async_runtime::block_on(get_instrument_detail(
        setup.app.handle().clone(),
        symbol("005930"),
    ))
    .expect_err("시세 없음");
    assert_eq!(detail.kind, IpcErrorKind::Retryable);
}

#[test]
fn detail_carries_quote_today_chart_and_holding() {
    let setup = harness();
    let detail = tauri::async_runtime::block_on(get_instrument_detail(
        setup.app.handle().clone(),
        symbol("005930"),
    ))
    .expect("상세");
    assert_eq!(detail.price.amount, "71400");
    assert_eq!(
        detail.today.as_ref().map(|today| today.rate.as_str()),
        Some("2.00")
    );
    assert_eq!(detail.quoted_at, "2026-11-25T10:00:00+09:00");
    assert_eq!(detail.candles.len(), 2);
    assert_eq!(detail.candles[0].date, "2026-11-24");
    assert_eq!(detail.candles[0].close, "70000");
    let held = detail.holding.as_ref().expect("보유");
    assert_eq!(held.quantity, "10");
    assert_eq!(held.value.amount, "714000");
    assert_snapshot("get_instrument_detail", &detail);
    let unknown = tauri::async_runtime::block_on(get_instrument_detail(
        setup.app.handle().clone(),
        symbol("123456"),
    ))
    .expect_err("모르는 종목");
    assert_eq!(unknown.kind, IpcErrorKind::NoData);
}

#[test]
fn watchlist_add_and_remove_go_through_the_use_case_and_reject_with_codes() {
    let setup = harness();
    let handle = setup.app.handle().clone();
    let added = tauri::async_runtime::block_on(add_watchlist(handle.clone(), symbol("005930")))
        .expect("추가");
    assert_eq!(added.outcome, UiWatchlistOutcome::Applied);
    assert_snapshot("add_watchlist", &added);
    assert_eq!(setup.watchlist.0.read().expect("잠금").entries().len(), 2);
    let unknown = tauri::async_runtime::block_on(add_watchlist(handle.clone(), symbol("123456")))
        .expect("모르는 종목");
    assert_eq!(
        unknown.reason,
        Some(UiWatchlistRejection::UnknownInstrument)
    );
    let inactive = tauri::async_runtime::block_on(add_watchlist(handle.clone(), symbol("999990")))
        .expect("상장폐지");
    assert_eq!(
        inactive.reason,
        Some(UiWatchlistRejection::InactiveInstrument)
    );
    let removed =
        tauri::async_runtime::block_on(remove_watchlist(handle.clone(), symbol("000660")))
            .expect("삭제");
    assert_eq!(removed.outcome, UiWatchlistOutcome::Applied);
    assert_snapshot("remove_watchlist", &removed);
    let again = tauri::async_runtime::block_on(remove_watchlist(handle.clone(), symbol("000660")))
        .expect("이미 없음");
    assert_eq!(again.reason, Some(UiWatchlistRejection::NotInWatchlist));
    let invalid =
        tauri::async_runtime::block_on(add_watchlist(handle, symbol(""))).expect_err("빈 심볼");
    assert_eq!(invalid.kind, IpcErrorKind::UserFixable);
}

#[test]
fn unmanaged_state_answers_no_data_and_requests_refuse_unknown_fields() {
    let bare = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing =
        tauri::async_runtime::block_on(get_instruments(bare.handle().clone(), list_request()))
            .expect_err("상태 없음");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
    assert_eq!(
        serde_json::from_str::<GetInstrumentsRequest>(r#"{"scope":"tracked"}"#).expect("요청"),
        list_request()
    );
    for input in [r#"{"scope":"all"}"#, r#"{"scope":"tracked","account":"1"}"#] {
        let _refused =
            serde_json::from_str::<GetInstrumentsRequest>(input).expect_err("모르는 필드");
    }
    let _refused =
        serde_json::from_str::<InstrumentRequest>(r#"{"symbol":"005930","market":"kr"}"#)
            .expect_err("모르는 필드");
}
