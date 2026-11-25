use super::support::{at, failure, ports};
use application::{PortfolioRepository, SettingsRepository, SystemFailure};
use domain::{
    AccountId, BuyingPower, Currency, InvestorProfile, Money, NumericInput, Portfolio, SyncOutcome,
    SyncRecord,
};
use tauri::Manager;
use testkit_ports::memory::MemoryReply;

#[test]
fn saved_data_is_ready_when_the_window_is_shown_before_any_network_call() {
    tauri::async_runtime::block_on(async {
        let mut ports = ports();
        let account = AccountId::new(401);
        let record = stored_record();
        let profile = InvestorProfile::default();
        ports
            .settings
            .script_store_profile()
            .push(MemoryReply::Memory);
        ports
            .settings
            .store_profile(&profile)
            .await
            .expect("설정 저장");
        ports.settings.script_profile().push(MemoryReply::Memory);
        ports
            .settings
            .script_risk_settings()
            .push(MemoryReply::Return(Err(failure(
                SystemFailure::PermissionDenied,
            ))));
        ports.portfolios.script_record().push(MemoryReply::Memory);
        ports
            .portfolios
            .record(&account, &record)
            .await
            .expect("현황 저장");
        ports.portfolios.script_latest().push(MemoryReply::Memory);
        ports.saved_accounts = crate::broker_link::AccountSlot::with(vec![account.clone()]);
        ports
            .broker_read
            .script_accounts()
            .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
        let refresh = crate::startup::start(&ports, |data| {
            assert_eq!(data.profile, Ok(Some(profile)));
            assert_eq!(
                data.risk_settings,
                Err(failure(SystemFailure::PermissionDenied))
            );
            assert_eq!(data.portfolios.len(), 1);
            assert_eq!(data.portfolios.first().expect("현황").account, account);
            assert_eq!(
                data.portfolios.first().expect("현황").record,
                Ok(Some(record))
            );
            assert_eq!(ports.broker_read.calls(), Vec::new());
            Ok(())
        })
        .await
        .expect("창 표시");
        refresh.await.expect("갱신 작업").expect("갱신 결과");
        assert_eq!(
            ports.broker_read.calls(),
            vec![testkit_ports::scripted::BrokerReadCall::Accounts()]
        );
    });
}

#[test]
fn a_window_error_stops_startup_before_network_refresh() {
    tauri::async_runtime::block_on(async {
        let ports = ports();
        ports
            .settings
            .script_profile()
            .push(MemoryReply::Return(Ok(None)));
        ports
            .settings
            .script_risk_settings()
            .push(MemoryReply::Return(Ok(None)));
        let result = crate::startup::start(&ports, |_data| {
            Err(tauri::Error::AssetNotFound("main".into()))
        })
        .await;
        result.expect_err("창 오류");
        assert_eq!(ports.broker_read.calls(), Vec::new());
    });
}

#[test]
fn build_app_runs_the_same_startup_with_the_mock_runtime_and_port_fakes() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    let ports = ports();
    ports
        .settings
        .script_profile()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .secrets
        .script_load()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .broker_read
        .script_accounts()
        .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    let app = crate::build_app(mock_builder(), ports, context).expect("조립");
    let code = app.run_return(|handle, event| match event {
        tauri::RunEvent::Ready => {
            assert_mock_startup(handle);
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<crate::lifecycle::Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료")
                    .is_some()
            );
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => {}
    });
    assert_eq!(code, 0_i32);
}

fn stored_record() -> SyncRecord<Portfolio> {
    let portfolio = Portfolio::new(
        Vec::new(),
        BuyingPower::new(
            Money::parse(
                &NumericInput::try_new("12345").expect("숫자"),
                Currency::Krw,
            )
            .expect("원화"),
            at(17),
        ),
        BuyingPower::new(
            Money::parse(
                &NumericInput::try_new("678.91").expect("숫자"),
                Currency::Usd,
            )
            .expect("달러"),
            at(23),
        ),
    )
    .expect("저장 보유");
    SyncRecord::new(at(31), SyncOutcome::Complete(portfolio))
}

fn assert_mock_startup(handle: &tauri::AppHandle<tauri::test::MockRuntime>) {
    let snapshot = handle.state::<crate::startup::MainScreenData>();
    assert_eq!(snapshot.profile, Ok(None));
    assert_eq!(snapshot.risk_settings, Ok(None));
    assert_eq!(snapshot.portfolios.len(), 0);
    tauri::async_runtime::block_on(handle.state::<crate::lifecycle::Lifecycle>().shutdown());
    assert!(
        handle
            .state::<crate::lifecycle::Lifecycle>()
            .outcome()
            .try_lock()
            .expect("종료 결과")
            .is_some()
    );
    handle
        .get_webview_window("main")
        .expect("주 창")
        .close()
        .expect("창 닫기");
}

#[test]
fn build_app_forces_initially_visible_windows_to_remain_hidden_during_setup() {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    assert!(context.config().app.windows.first().expect("설정").visible);
    let app = crate::build_app(tauri::test::mock_builder(), ports(), context).expect("조립");
    assert!(!app.config().app.windows.first().expect("설정").visible);
}

#[test]
fn window_presentation_requires_prepared_main_screen_data() {
    let app = tauri::test::mock_app();
    let error = crate::show_prepared_main(app.handle()).expect_err("자료 없이 표시 금지");
    assert_eq!(
        error.to_string(),
        tauri::Error::AssetNotFound("main-screen-data".into()).to_string()
    );
}
