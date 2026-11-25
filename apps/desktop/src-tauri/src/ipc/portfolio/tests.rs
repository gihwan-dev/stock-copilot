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
    let mut row = samsung().row().clone();
    if let Some(detail) = row.korean_detail.as_mut() {
        detail.krx = TradingSuspension::Suspended;
    }
    let suspended = InstrumentDirectory::default();
    suspended.replace(&[MasterInstrument::try_new(row).expect("정지 종목")]);
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
