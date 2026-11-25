use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use application::{PortError, SystemFailure, SystemPort, SystemPortError};
use domain::{AccountId, InvestmentPrinciples, MaskedAccountNumber};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    AccountBook, DataCare, ExportTarget, PrinciplesBook, SystemDataIpcState, SystemRequest,
    SystemScope, UiCashMinPercent, UiConcentrationCount, UiExportStatus, UiPrinciples,
    UiPrinciplesInput, backup_now, export_chosen, get_account_selection, get_principles,
    load_accounts, save_principles, select_account_ref,
};
use crate::broker_link::{AccountChoice, SelectionChange};
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;

fn masked(number: &str) -> MaskedAccountNumber {
    MaskedAccountNumber::try_new(number).expect("계좌번호")
}

struct FakeAccounts {
    selected: RwLock<Option<AccountId>>,
}

impl AccountBook for FakeAccounts {
    fn brokerage(&self) -> BoxFuture<'_, Result<Vec<(AccountId, MaskedAccountNumber)>, PortError>> {
        Box::pin(std::future::ready(Ok(vec![
            (AccountId::new(413), masked("00000000731")),
            (AccountId::new(500), masked("00000000999")),
        ])))
    }

    fn selected(&self) -> Option<AccountId> {
        self.selected.read().expect("잠금").clone()
    }

    fn choose(&self, account: AccountId) -> BoxFuture<'_, Result<AccountChoice, PortError>> {
        *self.selected.write().expect("잠금") = Some(account);
        Box::pin(std::future::ready(Ok(AccountChoice::Chosen(
            SelectionChange::Changed,
        ))))
    }
}

#[derive(Default)]
struct FakeCare {
    backups: RwLock<Vec<PathBuf>>,
    exports: RwLock<Vec<(PathBuf, PathBuf)>>,
    failure: RwLock<Option<SystemPortError>>,
}

impl DataCare for FakeCare {
    fn back_up(&self, directory: PathBuf) -> BoxFuture<'_, Result<(), SystemPortError>> {
        self.backups.write().expect("잠금").push(directory);
        let failure = *self.failure.read().expect("잠금");
        Box::pin(std::future::ready(failure.map_or(Ok(()), Err)))
    }

    fn export_to(
        &self,
        staging: PathBuf,
        target: PathBuf,
    ) -> BoxFuture<'_, Result<(), SystemPortError>> {
        self.exports.write().expect("잠금").push((staging, target));
        Box::pin(std::future::ready(Ok(())))
    }

    fn wipe(&self, _data_directory: PathBuf) -> BoxFuture<'_, Result<(), SystemPortError>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

struct FixedTarget(Option<PathBuf>);

impl ExportTarget for FixedTarget {
    fn choose(&self) -> BoxFuture<'_, Option<PathBuf>> {
        Box::pin(std::future::ready(self.0.clone()))
    }
}

#[derive(Default)]
struct FakePrinciples(RwLock<Option<InvestmentPrinciples>>);

impl PrinciplesBook for FakePrinciples {
    fn load(&self) -> BoxFuture<'_, Result<Option<InvestmentPrinciples>, SystemPortError>> {
        Box::pin(std::future::ready(Ok(self.0.read().expect("잠금").clone())))
    }

    fn save(&self, principles: InvestmentPrinciples) -> BoxFuture<'_, Result<(), SystemPortError>> {
        *self.0.write().expect("잠금") = Some(principles);
        Box::pin(std::future::ready(Ok(())))
    }
}

fn state(care: Arc<FakeCare>) -> SystemDataIpcState {
    state_with(care, None)
}

fn state_with(care: Arc<FakeCare>, target: Option<PathBuf>) -> SystemDataIpcState {
    SystemDataIpcState {
        accounts: Arc::new(FakeAccounts {
            selected: RwLock::new(Some(AccountId::new(413))),
        }),
        care,
        target: Arc::new(FixedTarget(target)),
        principles: Arc::new(FakePrinciples::default()),
    }
}

fn assert_snapshot<T: serde::Serialize>(name: &str, value: &T) {
    let path = Path::new("ipc/responses").join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

fn current() -> SystemRequest {
    SystemRequest {
        scope: SystemScope::Current,
    }
}

#[test]
fn accounts_are_listed_by_position_with_the_last_four_digits_and_can_be_switched() {
    let system = state(Arc::new(FakeCare::default()));
    let listed = tauri::async_runtime::block_on(load_accounts(&system)).expect("계좌");
    assert_eq!(listed.selected, "1");
    assert_eq!(listed.accounts[1].number, "0999");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, system.clone());
    let recorded =
        tauri::async_runtime::block_on(get_account_selection(app.handle().clone(), current()))
            .expect("명령");
    assert_snapshot("get_account_selection", &recorded);
    let switched =
        tauri::async_runtime::block_on(select_account_ref(&system, "2")).expect("바꾸기");
    assert_eq!(switched.selected, "2");
    assert_snapshot("select_account", &switched);
    for bad in ["0", "3", "x"] {
        let refused = tauri::async_runtime::block_on(select_account_ref(&system, bad))
            .expect_err("없는 계좌");
        assert_eq!(refused.kind, IpcErrorKind::UserFixable);
    }
}

#[test]
fn principles_round_trip_and_reject_overlong_text() {
    let system = state(Arc::new(FakeCare::default()));
    let initial = tauri::async_runtime::block_on(load_principles_for(&system));
    assert_eq!(initial.text, "");
    assert_eq!(initial.concentration_min_count, UiConcentrationCount::Five);
    let requested = UiPrinciplesInput {
        text: String::from("  한 종목에 몰지 않는다  "),
        cash_min_percent: UiCashMinPercent::Ten,
        concentration_min_count: UiConcentrationCount::Six,
    };
    let saved = tauri::async_runtime::block_on(save_principles(&system, &requested)).expect("저장");
    assert_eq!(saved.text, "한 종목에 몰지 않는다");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, system.clone());
    let reread = tauri::async_runtime::block_on(get_principles(app.handle().clone(), current()))
        .expect("다시 읽기");
    assert_eq!(reread, saved);
    assert_snapshot("get_principles", &reread);
    let long = UiPrinciplesInput {
        text: "가".repeat(4001),
        ..requested
    };
    let refused =
        tauri::async_runtime::block_on(save_principles(&system, &long)).expect_err("너무 긺");
    assert_eq!(refused.kind, IpcErrorKind::UserFixable);
}

async fn load_principles_for(system: &SystemDataIpcState) -> UiPrinciples {
    super::load_principles(system).await.expect("원칙")
}

#[test]
fn backup_goes_to_the_app_backup_folder_and_a_failure_is_reported() {
    let care = Arc::new(FakeCare::default());
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, state(Arc::clone(&care)));
    let done =
        tauri::async_runtime::block_on(backup_now(app.handle().clone(), current())).expect("백업");
    assert_snapshot("backup_now", &done);
    assert!(
        care.backups.read().expect("잠금")[0].ends_with("backups"),
        "백업 폴더"
    );
    *care.failure.write().expect("잠금") = Some(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::InvalidData,
    ));
    let failed = tauri::async_runtime::block_on(backup_now(app.handle().clone(), current()))
        .expect_err("실패");
    assert_eq!(failed.kind, IpcErrorKind::Bug);
}

#[test]
fn unmanaged_state_and_unknown_fields_are_refused() {
    let bare = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing = tauri::async_runtime::block_on(get_principles(bare.handle().clone(), current()))
        .expect_err("상태 없음");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
    let _refused = serde_json::from_str::<SystemRequest>(r#"{"scope":"current","all":true}"#)
        .expect_err("모르는 필드");
}

#[test]
fn export_writes_only_to_the_chosen_file_and_a_cancelled_choice_does_nothing() {
    let care = Arc::new(FakeCare::default());
    let cancelled = tauri::async_runtime::block_on(export_chosen(
        &state_with(Arc::clone(&care), None),
        PathBuf::from("/tmp/staging"),
    ))
    .expect("취소");
    assert_eq!(cancelled.status, UiExportStatus::Cancelled);
    assert!(care.exports.read().expect("잠금").is_empty());
    let chosen = PathBuf::from("/Users/me/Desktop/backup.db");
    let exported = tauri::async_runtime::block_on(export_chosen(
        &state_with(Arc::clone(&care), Some(chosen.clone())),
        PathBuf::from("/tmp/staging"),
    ))
    .expect("내보내기");
    assert_eq!(exported.status, UiExportStatus::Exported);
    assert_eq!(
        *care.exports.read().expect("잠금"),
        vec![(PathBuf::from("/tmp/staging"), chosen)]
    );
    assert!(care.backups.read().expect("잠금").is_empty());
}
