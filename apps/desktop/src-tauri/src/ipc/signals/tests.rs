use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use application::{
    Clock, EventBus, PortError, QuoteObservation, ScorecardView, SignalRevision, SignalSkip,
    SignalSnapshot, SignalSourceFacts, SkipReason, SystemFailure, SystemPort, SystemPortError,
    Watchlist, WatchlistEntry, WatchlistRepository, WatchlistStoreOutcome,
};
use domain::{
    CalendarDate, Candle, CommonShare, Currency, DisplayInstrumentName, InstrumentId,
    IssuedEstimateTaskId, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus,
    MarketCalendar, MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType,
    Nanosecond, NxtTrading, OffsetSeconds, QuoteInstrument, Symbol, TimeInterval, Timestamp,
    TradingSuspension, UnixSeconds,
};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    GetSignalsRequest, MarketCalendars, SignalBoard, SignalsIpcState, SignalsScope,
    SkipSignalRequest, SubmissionRecord, UiMarketState, get_signals, market_state, skip_signal,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;
use crate::ipc::instruments::{InstrumentMarket, InstrumentsIpcState};
use crate::ipc::state::BoxFuture;

const RECORDED_AT: i64 = 1_795_564_800;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
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
        observed_at: at(RECORDED_AT),
    })
    .expect("마스터")
}

fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(application::RetryDelay::Unknown),
    )
}

#[derive(Default)]
struct FakeBoard {
    failing: RwLock<Option<SystemPortError>>,
    skips: RwLock<Vec<SignalSkip>>,
}

impl SignalBoard for FakeBoard {
    fn snapshot(
        &self,
        _instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<SignalSnapshot, SystemPortError>> {
        let failure = *self.failing.read().expect("잠금");
        Box::pin(std::future::ready(failure.map_or(
            Ok(SignalSnapshot {
                revision: SignalRevision::new(0),
                submission: None,
                current: None,
            }),
            Err,
        )))
    }

    fn records(
        &self,
        _instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<SubmissionRecord>, SystemPortError>> {
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    fn source_facts(
        &self,
        _task: IssuedEstimateTaskId,
    ) -> BoxFuture<'_, Result<SignalSourceFacts, SystemPortError>> {
        Box::pin(std::future::ready(Ok(SignalSourceFacts::default())))
    }

    fn latest_skip(
        &self,
        _instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Timestamp>, SystemPortError>> {
        Box::pin(std::future::ready(Ok(None)))
    }

    fn skip(
        &self,
        instrument: InstrumentId,
        reason: Option<SkipReason>,
    ) -> BoxFuture<'_, Result<SignalSkip, SystemPortError>> {
        let skip = SignalSkip {
            instrument,
            reason,
            skipped_at: at(RECORDED_AT),
        };
        self.skips.write().expect("잠금").push(skip.clone());
        Box::pin(std::future::ready(Ok(skip)))
    }

    fn scorecards(&self) -> BoxFuture<'_, Option<ScorecardView>> {
        Box::pin(std::future::ready(None))
    }
}

struct NoCalendars;

impl MarketCalendars for NoCalendars {
    fn calendar_on(
        &self,
        _market: KnownMarket,
        _date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        Box::pin(std::future::ready(Err(PortError::new(
            application::ReadEndpoint::Quotes,
            application::HttpStatus::NotReceived,
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation),
        ))))
    }
}

struct NoMarket;

impl InstrumentMarket for NoMarket {
    fn quotes(
        &self,
        _instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    fn daily(
        &self,
        _instrument: QuoteInstrument,
        _window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        Box::pin(std::future::ready(Ok(Vec::new())))
    }
}

struct OneWatched;

impl WatchlistRepository for OneWatched {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
        std::future::ready(Ok(Watchlist::try_from_entries(vec![WatchlistEntry::new(
            InstrumentId::try_new("inst-1").expect("ID"),
            at(RECORDED_AT),
        )])
        .expect("관심 종목")))
    }

    fn store(
        &self,
        _previous: &Watchlist,
        _next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
        std::future::ready(Ok(WatchlistStoreOutcome::Conflict))
    }
}

use std::future::Future;

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        at(RECORDED_AT)
    }
}

fn state(board: Arc<FakeBoard>) -> SignalsIpcState {
    SignalsIpcState {
        board,
        calendars: Arc::new(NoCalendars),
        clock: Arc::new(FixedClock),
        cached: Arc::default(),
    }
}

fn app_with(board: Arc<FakeBoard>) -> tauri::App<tauri::test::MockRuntime> {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _signals = tauri::Manager::manage(&app, state(board));
    let _instruments = tauri::Manager::manage(
        &app,
        InstrumentsIpcState::new(
            Arc::new(NoMarket),
            Arc::new(OneWatched),
            Arc::new(FixedClock),
            Arc::new(EventBus::default()),
        ),
    );
    let directory = InstrumentDirectory::default();
    directory.replace(&[samsung()]);
    let _names = tauri::Manager::manage(&app, directory);
    app
}

fn list() -> GetSignalsRequest {
    GetSignalsRequest {
        scope: SignalsScope::Current,
    }
}

fn skip(symbol: &str, reason: &str) -> SkipSignalRequest {
    SkipSignalRequest {
        symbol: String::from(symbol),
        reason: String::from(reason),
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
fn tracked_instruments_without_a_current_signal_give_no_cards() {
    let app = app_with(Arc::new(FakeBoard::default()));
    let view =
        tauri::async_runtime::block_on(get_signals(app.handle().clone(), list())).expect("신호");
    assert_eq!(view.cards.len(), 0);
    assert_snapshot("get_signals", &view);
}

#[test]
fn a_signal_store_failure_is_reported_instead_of_an_empty_list() {
    let board = Arc::new(FakeBoard::default());
    *board.failing.write().expect("잠금") = Some(unavailable());
    let app = app_with(board);
    let failed = tauri::async_runtime::block_on(get_signals(app.handle().clone(), list()))
        .expect_err("저장소 오류");
    assert_ne!(failed.kind, IpcErrorKind::Bug);
}

#[test]
fn skipping_records_the_trimmed_reason_and_rejects_multi_line_text() {
    let board = Arc::new(FakeBoard::default());
    let app = app_with(Arc::clone(&board));
    let recorded = tauri::async_runtime::block_on(skip_signal(
        app.handle().clone(),
        skip("005930", "  실적 발표 뒤에 다시 볼게요 "),
    ))
    .expect("넘기기");
    assert_snapshot("skip_signal", &recorded);
    let _no_reason =
        tauri::async_runtime::block_on(skip_signal(app.handle().clone(), skip("005930", "")))
            .expect("이유 없이 넘기기");
    let reasons: Vec<Option<String>> = board
        .skips
        .read()
        .expect("잠금")
        .iter()
        .map(|entry| entry.reason.as_ref().map(|text| text.as_str().to_owned()))
        .collect();
    assert_eq!(
        reasons,
        vec![Some(String::from("실적 발표 뒤에 다시 볼게요")), None]
    );
    let multi = tauri::async_runtime::block_on(skip_signal(
        app.handle().clone(),
        skip("005930", "첫 줄\n둘째 줄"),
    ))
    .expect_err("여러 줄");
    assert_eq!(multi.kind, IpcErrorKind::UserFixable);
    let unknown =
        tauri::async_runtime::block_on(skip_signal(app.handle().clone(), skip("123456", "")))
            .expect_err("모르는 종목");
    assert_eq!(unknown.kind, IpcErrorKind::NoData);
}

#[test]
fn an_unreadable_market_calendar_counts_as_closed_so_no_order_is_offered() {
    let signals = state(Arc::new(FakeBoard::default()));
    assert_eq!(
        tauri::async_runtime::block_on(market_state(&signals, Some(KnownMarket::Kr))),
        UiMarketState::Closed
    );
    assert_eq!(
        tauri::async_runtime::block_on(market_state(&signals, None)),
        UiMarketState::Closed
    );
}

#[test]
fn unmanaged_state_and_unknown_fields_are_refused() {
    let bare = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing = tauri::async_runtime::block_on(get_signals(bare.handle().clone(), list()))
        .expect_err("상태 없음");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
    let _refused = serde_json::from_str::<GetSignalsRequest>(r#"{"scope":"current","x":1}"#)
        .expect_err("모르는 필드");
    let _refused_skip =
        serde_json::from_str::<SkipSignalRequest>(r#"{"symbol":"005930","reason":"","card":"1"}"#)
            .expect_err("모르는 필드");
}
