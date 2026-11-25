use domain::{
    CalendarDate, Cancellation, Currency, SettledRowSet, StoredTransaction, SyncOutcome,
    SyncRecord, TransactionKind,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use super::portfolio::{UiCurrency, UiMoney, display_name, ui_money};
use super::state::ScreenState;
use crate::holdings_link::InstrumentDirectory;

#[cfg(test)]
mod tests;

const ROW_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetTransactionsRequest {
    pub scope: TransactionsScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum TransactionsScope {
    #[serde(rename = "recent")]
    Recent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTransactionKind {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
    #[serde(rename = "dividend")]
    Dividend,
    #[serde(rename = "deposit")]
    Deposit,
    #[serde(rename = "withdrawal")]
    Withdrawal,
    #[serde(rename = "fee")]
    Fee,
    #[serde(rename = "other")]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTransactionPresence {
    #[serde(rename = "present")]
    Present,
    #[serde(rename = "vanished")]
    Vanished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiTransactionRow {
    pub id: String,
    pub settled_at: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub cash: Option<UiCurrency>,
    pub kind: UiTransactionKind,
    pub quantity: Option<String>,
    pub amount: UiMoney,
    pub presence: UiTransactionPresence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiTransactionsView {
    pub recorded_at: String,
    pub rows: Vec<UiTransactionRow>,
}

const fn kind(value: &TransactionKind) -> UiTransactionKind {
    match value {
        TransactionKind::Buy => UiTransactionKind::Buy,
        TransactionKind::Sell => UiTransactionKind::Sell,
        TransactionKind::Dividend => UiTransactionKind::Dividend,
        TransactionKind::Deposit => UiTransactionKind::Deposit,
        TransactionKind::Withdrawal => UiTransactionKind::Withdrawal,
        TransactionKind::Fee => UiTransactionKind::Fee,
        TransactionKind::SecuritiesTransferIn
        | TransactionKind::SecuritiesTransferOut
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Interest
        | TransactionKind::Unknown(_) => UiTransactionKind::Other,
    }
}

const fn cash(currency: Currency) -> UiCurrency {
    match currency {
        Currency::Krw => UiCurrency::Krw,
        Currency::Usd => UiCurrency::Usd,
    }
}

fn date_iso(date: CalendarDate) -> String {
    format!(
        "{:04}-{:02}-{:02}T00:00:00+09:00",
        date.year().as_i32(),
        date.month().as_u8(),
        date.day().as_u8()
    )
}

fn row(
    index: usize,
    stored: &StoredTransaction,
    directory: &InstrumentDirectory,
) -> Option<UiTransactionRow> {
    let data = stored.transaction().data();
    let settled_at = data
        .transaction_time
        .and_then(kst_iso)
        .unwrap_or_else(|| date_iso(data.settlement_date));
    let amount = data.net_amount.or(data.amount).unwrap_or(data.commission);
    Some(UiTransactionRow {
        id: format!("{}-{index}", date_iso(data.settlement_date)),
        settled_at,
        symbol: data
            .symbol
            .as_ref()
            .map(|symbol| symbol.as_str().to_owned()),
        name: data
            .symbol
            .as_ref()
            .map(|symbol| display_name(symbol, directory)),
        cash: match data.symbol {
            Some(_) => None,
            None => Some(cash(data.currency)),
        },
        kind: kind(&data.kind),
        quantity: data
            .quantity
            .map(|quantity| quantity.as_decimal().normalize().to_string()),
        amount: ui_money(&amount)?,
        presence: match stored.disappeared_at() {
            Some(_) => UiTransactionPresence::Vanished,
            None => UiTransactionPresence::Present,
        },
    })
}

#[must_use]
pub fn to_transactions_view(
    record: &SyncRecord<SettledRowSet>,
    rows: &SettledRowSet,
    directory: &InstrumentDirectory,
) -> Option<UiTransactionsView> {
    let mut history: Vec<&StoredTransaction> = rows
        .history_rows()
        .iter()
        .filter(|stored| stored.transaction().data().cancellation == Cancellation::Regular)
        .collect();
    history.sort_by(|left, right| {
        let key = |stored: &StoredTransaction| {
            let data = stored.transaction().data();
            (data.settlement_date, data.transaction_time)
        };
        key(right).cmp(&key(left))
    });
    let mut converted = Vec::new();
    for (index, stored) in history.into_iter().take(ROW_LIMIT).enumerate() {
        converted.push(row(index, stored, directory)?);
    }
    Some(UiTransactionsView {
        recorded_at: kst_iso(record.recorded_at())?,
        rows: converted,
    })
}

pub async fn load_transactions(
    screen: &ScreenState,
    directory: &InstrumentDirectory,
) -> Result<UiTransactionsView, IpcError> {
    let record = screen
        .backend()
        .load_transactions()
        .await
        .map_err(|error| IpcError::from(application::ClassifiedError::class(&error)))?
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })?;
    let rows = match record.outcome() {
        SyncOutcome::Complete(rows) | SyncOutcome::Partial(rows) => rows,
        SyncOutcome::Failed => {
            return Err(IpcError {
                kind: IpcErrorKind::Retryable,
            });
        }
        SyncOutcome::AccessBlocked => {
            return Err(IpcError {
                kind: IpcErrorKind::UserFixable,
            });
        }
    };
    to_transactions_view(&record, rows, directory).ok_or(IpcError {
        kind: IpcErrorKind::Bug,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_transactions<R: Runtime>(
    app: AppHandle<R>,
    request: GetTransactionsRequest,
) -> Result<UiTransactionsView, IpcError> {
    let GetTransactionsRequest {
        scope: TransactionsScope::Recent,
    } = request;
    let screen = app.try_state::<ScreenState>().ok_or(IpcError {
        kind: IpcErrorKind::NoData,
    })?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |state| state.inner().clone());
    load_transactions(screen.inner(), &directory).await
}
