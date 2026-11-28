use std::fs;
use std::path::Path;
use std::sync::Arc;

use application::{StoredView, SystemFailure, SystemPort, SystemPortError};
use domain::{
    AccountId, AcquisitionPrice, BuyingPower, CommonShare, Currency, CurrencyAmounts,
    DailyProfitLoss, DisplayInstrumentName, Holding, HoldingObservation, InstrumentId,
    KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading,
    OffsetSeconds, Portfolio, PortfolioDailyProfitLoss, Price, Quantity, Ratio, ReportedHolding,
    ReportedPortfolio, Symbol, SyncOutcome, SyncRecord, Timestamp, TradingSuspension, UnixSeconds,
};
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::{MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply};

use super::{
    GetPortfolioRequest, PortfolioScope, UiCurrency, UiDirection, UiTotalCoverage, get_allocation,
    get_portfolio, to_portfolio_view,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::{BoxFuture, LoadedSettings, ScreenBackend, ScreenState, SecretPresence};

const RECORDED_AT: i64 = 1_795_564_800;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn number(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("숫자")
}

fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&number(text), currency).expect("금액")
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

fn holding() -> Holding {
    let reported = ReportedHolding::new(
        money("7200000", Currency::Krw),
        money("700000", Currency::Krw),
        Ratio::parse(&number("0.1077")).expect("수익률"),
    )
    .expect("보고")
    .with_daily_profit_loss(DailyProfitLoss::new(
        money("-100000", Currency::Krw),
        Ratio::parse(&number("-0.0141")).expect("당일"),
    ))
    .expect("당일 손익");
    Holding::new(
        samsung().order_instrument().expect("종목"),
        Quantity::parse(&number("100")).expect("수량"),
        AcquisitionPrice::try_new(money("65000", Currency::Krw)).expect("평단"),
        HoldingObservation::new(
            Some(Price::parse(&number("72000"), Currency::Krw).expect("가격")),
            Some(reported),
            at(RECORDED_AT),
        ),
    )
    .expect("보유")
}

fn portfolio() -> Portfolio {
    let amounts = |text| CurrencyAmounts::new(money(text, Currency::Krw), None).expect("금액");
    Portfolio::new(
        vec![holding()],
        BuyingPower::new(money("5000000", Currency::Krw), at(RECORDED_AT)),
        BuyingPower::new(money("0", Currency::Usd), at(RECORDED_AT)),
    )
    .expect("포트폴리오")
    .with_reported(
        ReportedPortfolio::new(
            amounts("7200000"),
            amounts("6500000"),
            amounts("700000"),
            at(RECORDED_AT),
        )
        .with_daily_profit_loss(PortfolioDailyProfitLoss::new(
            amounts("-100000"),
            Ratio::parse(&number("-0.0141")).expect("당일"),
        )),
    )
}

fn directory() -> InstrumentDirectory {
    let names = InstrumentDirectory::default();
    names.replace(&[samsung()]);
    names
}

#[test]
fn portfolio_view_carries_names_totals_and_signed_profit_from_domain_values() {
    let view = to_portfolio_view(at(RECORDED_AT), &portfolio().evaluate(None), &directory())
        .expect("화면");
    assert_eq!(view.recorded_at, "2026-11-25T09:00:00+09:00");
    assert_eq!(view.total.currency, UiCurrency::Krw);
    assert_eq!(view.total.amount, "12200000");
    assert_eq!(view.total_coverage, UiTotalCoverage::Complete);
    assert_eq!(view.holdings.len(), 1);
    let row = &view.holdings[0];
    assert_eq!(row.name, "삼성전자");
    assert_eq!(row.symbol, "005930");
    assert_eq!(row.quantity, "100");
    assert_eq!(row.value.amount, "7200000");
    assert_eq!(row.profit.amount.amount, "700000");
    assert_eq!(row.profit.rate, "10.77");
    assert_eq!(row.profit.direction, UiDirection::Up);
    let today = view.today.as_ref().expect("오늘 손익");
    assert_eq!(today.direction, UiDirection::Down);
    assert_eq!(today.rate, "-1.41");
    let unnamed = to_portfolio_view(
        at(RECORDED_AT),
        &portfolio().evaluate(None),
        &InstrumentDirectory::default(),
    )
    .expect("화면");
    assert_eq!(unnamed.holdings[0].name, "005930");
    assert_eq!(view.holdings[0].trading, super::UiTrading::Trading);
    let mut halted_row = samsung().row().clone();
    if let Some(detail) = halted_row.korean_detail.as_mut() {
        detail.krx = TradingSuspension::Suspended;
    }
    let suspended = InstrumentDirectory::default();
    suspended.replace(&[MasterInstrument::try_new(halted_row).expect("정지 종목")]);
    let halted =
        to_portfolio_view(at(RECORDED_AT), &portfolio().evaluate(None), &suspended).expect("화면");
    assert_eq!(halted.holdings[0].trading, super::UiTrading::Suspended);
    assert_eq!(halted.holdings[0].name, "삼성전자");
}

struct FixedBackend {
    portfolios: MemoryPortfolioRepository,
    market: MemoryMarketDataRepository,
    failure: Option<SystemPortError>,
}

fn backend(outcome: Option<&SyncOutcome<Portfolio>>) -> FixedBackend {
    let portfolios = MemoryPortfolioRepository::default();
    let market = MemoryMarketDataRepository::default();
    for _ in 0_u8..4_u8 {
        portfolios.script_latest().push(MemoryReply::Return(Ok(
            outcome.map(|value| SyncRecord::new(at(RECORDED_AT), value.clone()))
        )));
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
    }
    FixedBackend {
        portfolios,
        market,
        failure: None,
    }
}

impl ScreenBackend for FixedBackend {
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
        Box::pin(std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::InvalidData,
        ))))
    }

    fn load_portfolio_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::PortfolioValuation>, SystemPortError>> {
        Box::pin(async move {
            if let Some(error) = self.failure {
                return Err(error);
            }
            application::GetPortfolioView::new(&self.portfolios, &self.market)
                .execute(&AccountId::new(413))
                .await
        })
    }

    fn load_allocation_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::Allocation>, SystemPortError>> {
        Box::pin(async move {
            application::GetAllocationView::new(&self.portfolios, &self.market)
                .execute(&AccountId::new(413))
                .await
        })
    }

    fn load_transactions(
        &self,
    ) -> BoxFuture<'_, Result<Option<SyncRecord<domain::SettledRowSet>>, SystemPortError>> {
        Box::pin(std::future::ready(Ok(None)))
    }

    fn update_profile(
        &self,
        _requested: application::InvestorProfileRequest,
    ) -> BoxFuture<'_, Result<application::RiskSettingsChangeOutcome, SystemPortError>> {
        Box::pin(std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::InvalidData,
        ))))
    }
}

fn app_with(backend: FixedBackend) -> tauri::App<tauri::test::MockRuntime> {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _screen = tauri::Manager::manage(&app, ScreenState::new(Arc::new(backend)));
    let _names = tauri::Manager::manage(&app, directory());
    app
}

fn request() -> GetPortfolioRequest {
    GetPortfolioRequest {
        scope: PortfolioScope::Current,
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
fn portfolio_commands_answer_recorded_views_for_the_typescript_fake() {
    let app = app_with(backend(Some(&SyncOutcome::Complete(portfolio()))));
    let view = tauri::async_runtime::block_on(get_portfolio(app.handle().clone(), request()))
        .expect("현황");
    assert_snapshot("get_portfolio", &view);
    let allocation =
        tauri::async_runtime::block_on(get_allocation(app.handle().clone(), request()))
            .expect("비중");
    assert_eq!(allocation.instruments.len(), 2);
    assert_snapshot("get_allocation", &allocation);
}

#[test]
fn missing_failed_blocked_and_unmanaged_states_map_to_closed_errors() {
    let cases = [
        (None, IpcErrorKind::NoData),
        (Some(SyncOutcome::Failed), IpcErrorKind::Retryable),
        (Some(SyncOutcome::AccessBlocked), IpcErrorKind::UserFixable),
    ];
    for (outcome, expected) in cases {
        let app = app_with(backend(outcome.as_ref()));
        let error = tauri::async_runtime::block_on(get_portfolio(app.handle().clone(), request()))
            .expect_err("오류");
        assert_eq!(error.kind, expected);
    }
    let mut failing = backend(None);
    failing.failure = Some(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(application::RetryDelay::Unknown),
    ));
    let app = app_with(failing);
    let error = tauri::async_runtime::block_on(get_portfolio(app.handle().clone(), request()))
        .expect_err("저장소 오류");
    assert_ne!(error.kind, IpcErrorKind::Bug);
    let bare = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing = tauri::async_runtime::block_on(get_portfolio(bare.handle().clone(), request()))
        .expect_err("상태 없음");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
}

#[test]
fn portfolio_request_refuses_unknown_fields() {
    assert_eq!(
        serde_json::from_str::<GetPortfolioRequest>(r#"{"scope":"current"}"#).expect("요청"),
        request()
    );
    for input in [
        r#"{"scope":"current","account":"413"}"#,
        r#"{"scope":"all"}"#,
    ] {
        assert!(
            serde_json::from_str::<GetPortfolioRequest>(input).is_err(),
            "{input}"
        );
    }
}

struct FixedClock;
impl application::Clock for FixedClock {
    fn now(&self) -> Timestamp {
        at(RECORDED_AT)
    }
}

#[test]
fn a_failed_portfolio_carries_the_last_sync_failure_reason() {
    use application::DiagnosticsSink;
    let log = crate::diagnostics_log::DiagnosticsLog::start(
        testkit::canonical_temp_dir()
            .expect("임시 폴더")
            .join(format!("sc-diag-portfolio-{}", std::process::id())),
        Arc::new(FixedClock),
    );
    log.record(&application::DiagnosticEvent::Sync(
        application::SyncDiagnosis {
            job: application::SyncJob::Portfolio,
            result: application::SyncResult::Failed,
            errors: vec![application::PortError::new(
                application::ReadEndpoint::Holdings,
                application::HttpStatus::Success,
                application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Schema),
            )],
        },
    ));
    let cases = [
        (
            Some(SyncOutcome::Failed),
            IpcErrorKind::Retryable,
            Some(crate::ipc::error::IpcFailureReason::ResponseMismatch),
        ),
        (
            Some(SyncOutcome::AccessBlocked),
            IpcErrorKind::UserFixable,
            None,
        ),
        (None, IpcErrorKind::NoData, None),
    ];
    for (outcome, kind, reason) in cases {
        let app = app_with(backend(outcome.as_ref()));
        let _log = tauri::Manager::manage(&app, Some(Arc::clone(&log)));
        let error = tauri::async_runtime::block_on(get_portfolio(app.handle().clone(), request()))
            .expect_err("오류");
        assert_eq!((error.kind, error.reason), (kind, reason));
    }
    let app = app_with(backend(Some(&SyncOutcome::Failed)));
    let error = tauri::async_runtime::block_on(get_portfolio(app.handle().clone(), request()))
        .expect_err("기록 없음");
    assert_eq!((error.kind, error.reason), (IpcErrorKind::Retryable, None));
    assert_eq!(
        serde_json::to_string(&error).expect("직렬화"),
        r#"{"kind":"retryable","reason":null}"#
    );
}

fn allocation_master(symbol: &str, kind: &str) -> MasterInstrument {
    let mut row = samsung().row().clone();
    row.symbol = Symbol::try_new(symbol).expect("심볼");
    row.instrument_id = InstrumentId::try_new(&format!("inst-{symbol}")).expect("ID");
    row.security_type = MasterSecurityType::from_code(MasterCode::try_new(kind).expect("유형"));
    MasterInstrument::try_new(row).expect("마스터")
}

fn allocation_holding(master: &MasterInstrument, value: &str) -> Holding {
    Holding::new(
        master.order_instrument().expect("종목"),
        Quantity::parse(&number("1")).expect("수량"),
        AcquisitionPrice::try_new(money(value, Currency::Krw)).expect("취득 단가"),
        HoldingObservation::new(
            Some(Price::parse(&number(value), Currency::Krw).expect("시세")),
            None,
            at(RECORDED_AT),
        ),
    )
    .expect("보유")
}

fn sector_service(members: &[MasterInstrument]) -> Arc<crate::sector_classes::SectorClasses> {
    use crate::sector_classes::{MarketSectorFeed, SectorClasses};
    use domain::{
        KnownMarket, SectorCatalog, SectorComposition, SectorDepth, SectorId, SectorMetadataRow,
        SectorStockRow,
    };
    use testkit_ports::scripted::ScriptedMarketInfo;
    let sector = SectorId::try_new("10").expect("업종");
    let market = Arc::new(ScriptedMarketInfo::default());
    market
        .script_sectors()
        .push(Ok(SectorCatalog::from_response(
            vec![SectorMetadataRow {
                id: sector.clone(),
                name: DisplayInstrumentName::try_new("반도체").expect("업종명"),
                depth: SectorDepth::Major,
                parent: None,
            }],
            at(RECORDED_AT),
        )
        .expect("카탈로그")));
    let stocks = members
        .iter()
        .map(|master| SectorStockRow {
            symbol: master.row().symbol.clone(),
            currency: Currency::Krw,
            market_capitalization: None,
            trading_amount: None,
        })
        .collect();
    market
        .script_sector_composition()
        .push(Ok(SectorComposition::from_stocks_response(
            sector,
            KnownMarket::Kr,
            stocks,
            at(RECORDED_AT),
        )
        .expect("구성 종목")));
    Arc::new(SectorClasses::new(
        Arc::new(MarketSectorFeed(market)),
        Arc::new(FixedClock),
    ))
}

fn assert_mixed_rows(view: &super::UiAllocationView) {
    use super::{UiAllocationLabel as Kind, UiWeightLevel};
    let rows: Vec<_> = view
        .sectors
        .iter()
        .map(|row| {
            (
                row.kind,
                row.name.as_str(),
                row.value.currency,
                row.value.amount.as_str(),
                row.weight.as_str(),
                row.level,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                Kind::Sector,
                "반도체",
                UiCurrency::Krw,
                "400",
                "40.00",
                Some(UiWeightLevel::Normal)
            ),
            (Kind::EtfFund, "", UiCurrency::Krw, "300", "30.00", None),
            (Kind::Cash, "", UiCurrency::Krw, "200", "20.00", None),
            (
                Kind::Unclassified,
                "",
                UiCurrency::Krw,
                "100",
                "10.00",
                None
            ),
        ]
    );
}

#[test]
fn allocation_command_groups_holdings_fund_unknown_and_cash_against_total_assets() {
    use crate::sector_classes::SectorRefresh;
    use tauri::Manager;
    let masters = [
        samsung(),
        allocation_master("000660", "STOCK"),
        allocation_master("069500", "ETF"),
        allocation_master("012345", "STOCK"),
    ];
    let holdings = masters
        .iter()
        .zip(["150", "250", "300", "100"])
        .map(|(master, value)| allocation_holding(master, value))
        .collect();
    let portfolio = Portfolio::new(
        holdings,
        BuyingPower::new(money("200", Currency::Krw), at(RECORDED_AT)),
        BuyingPower::new(money("0", Currency::Usd), at(RECORDED_AT)),
    )
    .expect("포트폴리오");
    let app = app_with(backend(Some(&SyncOutcome::Complete(portfolio))));
    app.state::<InstrumentDirectory>().replace(&masters[..3]);
    let service = sector_service(&masters[..2]);
    let _registered = app.manage(Arc::clone(&service));
    tauri::async_runtime::block_on(async {
        let _ready = service.assignments(&masters[..1]).await.expect("업종 응답");
        let view = get_allocation(app.handle().clone(), request())
            .await
            .expect("자산배분");
        assert_eq!(view.sector_refresh, SectorRefresh::Settled);
        assert_eq!(view.instruments.len(), 5);
        assert_mixed_rows(&view);
    });
}

#[test]
fn a_positive_allocation_smaller_than_one_display_unit_carries_its_bound() {
    use super::{UiAllocationLabel, UiWeightRange};
    let portfolio = Portfolio::new(
        vec![allocation_holding(&samsung(), "1")],
        BuyingPower::new(money("99999", Currency::Krw), at(RECORDED_AT)),
        BuyingPower::new(money("0", Currency::Usd), at(RECORDED_AT)),
    )
    .expect("포트폴리오");
    let app = app_with(backend(Some(&SyncOutcome::Complete(portfolio))));
    let view = tauri::async_runtime::block_on(get_allocation(app.handle().clone(), request()))
        .expect("비중");
    for (rows, kind) in [
        (&view.instruments, UiAllocationLabel::Holding),
        (&view.sectors, UiAllocationLabel::Unclassified),
    ] {
        let tiny = rows.iter().find(|row| row.kind == kind).expect("소액 보유");
        assert_eq!(tiny.value.amount, "1");
        assert_eq!(tiny.weight_range, UiWeightRange::BelowUnit);
    }
}
