use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use application::{PortError, PrinciplesRepository, SystemPortError};
use domain::{
    AccountId, CashFloor, ConcentrationFloor, InvestmentPrinciples, MaskedAccountNumber,
    PolicyText, PrincipleNote,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::state::BoxFuture;
use crate::broker_link::{AccountChoice, AccountSelector, AccountSlot};

#[cfg(test)]
mod tests;

pub trait AccountBook: Send + Sync {
    fn brokerage(&self) -> BoxFuture<'_, Result<Vec<(AccountId, MaskedAccountNumber)>, PortError>>;
    fn selected(&self) -> Option<AccountId>;
    fn choose(&self, account: AccountId) -> BoxFuture<'_, Result<AccountChoice, PortError>>;
}

pub trait DataCare: Send + Sync {
    fn back_up(&self, directory: PathBuf) -> BoxFuture<'_, Result<(), SystemPortError>>;
    fn wipe(&self, data_directory: PathBuf) -> BoxFuture<'_, Result<(), SystemPortError>>;
}

pub trait PrinciplesBook: Send + Sync {
    fn load(&self) -> BoxFuture<'_, Result<Option<InvestmentPrinciples>, SystemPortError>>;
    fn save(&self, principles: InvestmentPrinciples) -> BoxFuture<'_, Result<(), SystemPortError>>;
}

struct SelectorBook<B> {
    selector: AccountSelector<B>,
    slot: AccountSlot,
}

impl<B: application::BrokerRead + 'static> AccountBook for SelectorBook<B> {
    fn brokerage(&self) -> BoxFuture<'_, Result<Vec<(AccountId, MaskedAccountNumber)>, PortError>> {
        Box::pin(self.selector.brokerage_accounts())
    }

    fn selected(&self) -> Option<AccountId> {
        self.slot.current().first().cloned()
    }

    fn choose(&self, account: AccountId) -> BoxFuture<'_, Result<AccountChoice, PortError>> {
        Box::pin(async move { self.selector.choose(&account).await })
    }
}

struct StoredPrinciples<R>(Arc<R>);

impl<R: PrinciplesRepository + 'static> PrinciplesBook for StoredPrinciples<R> {
    fn load(&self) -> BoxFuture<'_, Result<Option<InvestmentPrinciples>, SystemPortError>> {
        Box::pin(async move { self.0.principles().await })
    }

    fn save(&self, principles: InvestmentPrinciples) -> BoxFuture<'_, Result<(), SystemPortError>> {
        Box::pin(async move {
            application::UpdateInvestmentPrinciples::new(self.0.as_ref())
                .execute(&principles)
                .await
                .map(|_written| ())
        })
    }
}

#[derive(Clone)]
pub struct SystemDataIpcState {
    accounts: Arc<dyn AccountBook>,
    care: Arc<dyn DataCare>,
    principles: Arc<dyn PrinciplesBook>,
}

impl fmt::Debug for SystemDataIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SystemDataIpcState")
    }
}

impl SystemDataIpcState {
    #[must_use]
    pub fn new<B, R>(
        selector: AccountSelector<B>,
        slot: AccountSlot,
        care: Arc<dyn DataCare>,
        principles: Arc<R>,
    ) -> Self
    where
        B: application::BrokerRead + 'static,
        R: PrinciplesRepository + 'static,
    {
        Self {
            accounts: Arc::new(SelectorBook { selector, slot }),
            care,
            principles: Arc::new(StoredPrinciples(principles)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum SystemScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SystemRequest {
    pub scope: SystemScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SelectAccountRequest {
    pub account: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiCashMinPercent {
    #[serde(rename = "0")]
    Zero,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "20")]
    Twenty,
    #[serde(rename = "30")]
    Thirty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiConcentrationCount {
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "6")]
    Six,
    #[serde(rename = "8")]
    Eight,
    #[serde(rename = "10")]
    Ten,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPrinciples {
    pub text: String,
    pub cash_min_percent: UiCashMinPercent,
    pub concentration_min_count: UiConcentrationCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiPrinciplesInput {
    pub text: String,
    pub cash_min_percent: UiCashMinPercent,
    pub concentration_min_count: UiConcentrationCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetPrinciplesRequest {
    pub principles: UiPrinciplesInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAccountRow {
    pub id: String,
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAccountSelection {
    pub selected: String,
    pub accounts: Vec<UiAccountRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiBackupStatus {
    #[serde(rename = "backed-up")]
    BackedUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiBackupResult {
    pub status: UiBackupStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiExportStatus {
    #[serde(rename = "exported")]
    Exported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiExportResult {
    pub status: UiExportStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiDeleteStatus {
    #[serde(rename = "deleted")]
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiDeleteResult {
    pub status: UiDeleteStatus,
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn storage(failure: &SystemPortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

fn broker(failure: &PortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

fn account_ref(position: usize) -> String {
    position.saturating_add(1).to_string()
}

pub async fn load_accounts(state: &SystemDataIpcState) -> Result<UiAccountSelection, IpcError> {
    let rows = state
        .accounts
        .brokerage()
        .await
        .map_err(|failure| broker(&failure))?;
    let selected = state.accounts.selected();
    let chosen = rows
        .iter()
        .position(|(account, _)| Some(account) == selected.as_ref())
        .ok_or(error(IpcErrorKind::NoData))?;
    Ok(UiAccountSelection {
        selected: account_ref(chosen),
        accounts: rows
            .iter()
            .enumerate()
            .map(|(position, (_, number))| UiAccountRow {
                id: account_ref(position),
                number: number.to_string(),
            })
            .collect(),
    })
}

pub async fn select_account_ref(
    state: &SystemDataIpcState,
    reference: &str,
) -> Result<UiAccountSelection, IpcError> {
    let rows = state
        .accounts
        .brokerage()
        .await
        .map_err(|failure| broker(&failure))?;
    let position = reference
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
        .ok_or(error(IpcErrorKind::UserFixable))?;
    let (account, _) = rows
        .get(position)
        .cloned()
        .ok_or(error(IpcErrorKind::UserFixable))?;
    match state
        .accounts
        .choose(account)
        .await
        .map_err(|failure| broker(&failure))?
    {
        AccountChoice::Chosen(_) => load_accounts(state).await,
        AccountChoice::NotAvailable => Err(error(IpcErrorKind::UserFixable)),
    }
}

const fn cash_floor(value: UiCashMinPercent) -> CashFloor {
    match value {
        UiCashMinPercent::Zero => CashFloor::None,
        UiCashMinPercent::Five => CashFloor::FivePercent,
        UiCashMinPercent::Ten => CashFloor::TenPercent,
        UiCashMinPercent::Twenty => CashFloor::TwentyPercent,
        UiCashMinPercent::Thirty => CashFloor::ThirtyPercent,
    }
}

const fn ui_cash(value: CashFloor) -> UiCashMinPercent {
    match value {
        CashFloor::None => UiCashMinPercent::Zero,
        CashFloor::FivePercent => UiCashMinPercent::Five,
        CashFloor::TenPercent => UiCashMinPercent::Ten,
        CashFloor::TwentyPercent => UiCashMinPercent::Twenty,
        CashFloor::ThirtyPercent => UiCashMinPercent::Thirty,
    }
}

const fn concentration_floor(value: UiConcentrationCount) -> ConcentrationFloor {
    match value {
        UiConcentrationCount::Three => ConcentrationFloor::Three,
        UiConcentrationCount::Four => ConcentrationFloor::Four,
        UiConcentrationCount::Five => ConcentrationFloor::Five,
        UiConcentrationCount::Six => ConcentrationFloor::Six,
        UiConcentrationCount::Eight => ConcentrationFloor::Eight,
        UiConcentrationCount::Ten => ConcentrationFloor::Ten,
    }
}

const fn ui_concentration(value: ConcentrationFloor) -> UiConcentrationCount {
    match value {
        ConcentrationFloor::Three => UiConcentrationCount::Three,
        ConcentrationFloor::Four => UiConcentrationCount::Four,
        ConcentrationFloor::Five => UiConcentrationCount::Five,
        ConcentrationFloor::Six => UiConcentrationCount::Six,
        ConcentrationFloor::Eight => UiConcentrationCount::Eight,
        ConcentrationFloor::Ten => UiConcentrationCount::Ten,
    }
}

#[must_use]
pub fn to_ui_principles(principles: &InvestmentPrinciples) -> UiPrinciples {
    UiPrinciples {
        text: principles
            .note()
            .map_or_else(String::new, |note| note.text().as_str().to_owned()),
        cash_min_percent: ui_cash(principles.cash()),
        concentration_min_count: ui_concentration(principles.concentration()),
    }
}

pub fn from_ui_principles(requested: &UiPrinciplesInput) -> Result<InvestmentPrinciples, IpcError> {
    let note = if requested.text.trim().is_empty() {
        None
    } else {
        let text = PolicyText::try_new(&requested.text)
            .map_err(|_long| error(IpcErrorKind::UserFixable))?;
        Some(PrincipleNote::try_new(&text).map_err(|_invalid| error(IpcErrorKind::UserFixable))?)
    };
    Ok(InvestmentPrinciples::new(
        note,
        cash_floor(requested.cash_min_percent),
        concentration_floor(requested.concentration_min_count),
    ))
}

pub async fn load_principles(state: &SystemDataIpcState) -> Result<UiPrinciples, IpcError> {
    let stored = state
        .principles
        .load()
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(match stored {
        Some(found) => to_ui_principles(&found),
        None => to_ui_principles(&InvestmentPrinciples::default()),
    })
}

pub async fn save_principles(
    state: &SystemDataIpcState,
    requested: &UiPrinciplesInput,
) -> Result<UiPrinciples, IpcError> {
    let principles = from_ui_principles(requested)?;
    state
        .principles
        .save(principles.clone())
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(to_ui_principles(&principles))
}

fn managed<R: Runtime>(app: &AppHandle<R>) -> Result<SystemDataIpcState, IpcError> {
    app.try_state::<SystemDataIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))
}

fn data_directory<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, IpcError> {
    app.path()
        .app_data_dir()
        .map_err(|_missing| error(IpcErrorKind::Retryable))
}

#[tauri::command]
#[specta::specta]
pub async fn get_account_selection<R: Runtime>(
    app: AppHandle<R>,
    request: SystemRequest,
) -> Result<UiAccountSelection, IpcError> {
    let SystemRequest {
        scope: SystemScope::Current,
    } = request;
    load_accounts(&managed(&app)?).await
}

#[tauri::command]
#[specta::specta]
pub async fn select_account<R: Runtime>(
    app: AppHandle<R>,
    request: SelectAccountRequest,
) -> Result<UiAccountSelection, IpcError> {
    select_account_ref(&managed(&app)?, &request.account).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_principles<R: Runtime>(
    app: AppHandle<R>,
    request: SystemRequest,
) -> Result<UiPrinciples, IpcError> {
    let SystemRequest {
        scope: SystemScope::Current,
    } = request;
    load_principles(&managed(&app)?).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_principles<R: Runtime>(
    app: AppHandle<R>,
    request: SetPrinciplesRequest,
) -> Result<UiPrinciples, IpcError> {
    save_principles(&managed(&app)?, &request.principles).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_now<R: Runtime>(
    app: AppHandle<R>,
    request: SystemRequest,
) -> Result<UiBackupResult, IpcError> {
    let SystemRequest {
        scope: SystemScope::Current,
    } = request;
    let state = managed(&app)?;
    let directory = data_directory(&app)?.join("backups");
    state
        .care
        .back_up(directory)
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(UiBackupResult {
        status: UiBackupStatus::BackedUp,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn export_data<R: Runtime>(
    app: AppHandle<R>,
    request: SystemRequest,
) -> Result<UiExportResult, IpcError> {
    let SystemRequest {
        scope: SystemScope::Current,
    } = request;
    let state = managed(&app)?;
    let directory = app
        .path()
        .download_dir()
        .map_err(|_missing| error(IpcErrorKind::Retryable))?
        .join("Stock Copilot");
    state
        .care
        .back_up(directory)
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(UiExportResult {
        status: UiExportStatus::Exported,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn delete_all_data<R: Runtime>(
    app: AppHandle<R>,
    request: SystemRequest,
) -> Result<UiDeleteResult, IpcError> {
    let SystemRequest {
        scope: SystemScope::Current,
    } = request;
    let state = managed(&app)?;
    state
        .care
        .wipe(data_directory(&app)?)
        .await
        .map_err(|failure| storage(&failure))?;
    app.request_restart();
    Ok(UiDeleteResult {
        status: UiDeleteStatus::Deleted,
    })
}
