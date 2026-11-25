use std::fs;
use std::path::Path;
use std::sync::Arc;

use application::{StoredView, SystemFailure, SystemPort, SystemPortError};
use domain::{
    AccountId, Cancellation, CommonShare, Currency, DisplayInstrumentName, InstrumentId,
    KoreanMasterDetail, LiquidationTrading, ListingStatus, Market, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading,
    OffsetSeconds, Quantity, SettledRowSet, Settlement, StoredTransaction, Symbol, SyncOutcome,
    SyncRecord, Timestamp, TradingSuspension, Transaction, TransactionBatch, TransactionCode,
    TransactionData, TransactionKind, UnixSeconds,
};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    GetTransactionsRequest, TransactionsScope, UiTransactionKind, UiTransactionPresence,
    get_transactions, to_transactions_view,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;
use crate::ipc::portfolio::UiCurrency;
use crate::ipc::state::{BoxFuture, LoadedSettings, ScreenBackend, ScreenState, SecretPresence};

const RECORDED_AT: i64 = 1_795_564_800;
const DAY: i64 = 86_400;

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
        observed_at: at(RECORDED_AT),
    })
    .expect("마스터")
}

fn days_before(days: i64) -> Timestamp {
    at(days
        .checked_mul(DAY)
        .and_then(|span| RECORDED_AT.checked_sub(span))
        .expect("시각 계산"))
}

fn transaction(code: &str, days_ago: i64, cancellation: Cancellation) -> Transaction {
    let traded = match code {
        "BUY" | "SELL" => Some("005930"),
        _ => None,
    };
    Transaction::try_new(TransactionData {
        kind: TransactionKind::new(TransactionCode::try_new(code).expect("코드")),
        market: Market::Kr,
        currency: Currency::Krw,
        symbol: traded.map(|symbol| Symbol::try_new(symbol).expect("심볼")),
        asset_class: None,
        isin: None,
        quantity: traded
            .map(|_| Quantity::parse(&NumericInput::try_new("10").expect("수량")).expect("수량")),
        filled_price: None,
        amount: Some(money("720000", Currency::Krw)),
        commission: money("0", Currency::Krw),
        tax: money("0", Currency::Krw),
        net_amount: None,
        exchange_rate: None,
        settlement_date: days_before(days_ago)
            .date_at_offset(OffsetSeconds::new(32_400))
            .expect("날짜"),
        settlement: Settlement::Settled,
        transaction_time: None,
        cancellation,
    })
    .expect("거래")
}

fn rows() -> SettledRowSet {
    let stored = |transaction, vanished: Option<Timestamp>| {
        StoredTransaction::try_restore(transaction, days_before(10), vanished).expect("기록")
    };
    SettledRowSet::try_restore(
        AccountId::new(413),
        vec![
            stored(transaction("DEPOSIT", 3, Cancellation::Regular), None),
            stored(transaction("BUY", 1, Cancellation::Regular), None),
            stored(
                transaction("SELL", 2, Cancellation::Regular),
                Some(at(RECORDED_AT)),
            ),
            stored(transaction("BUY", 2, Cancellation::Cancelled), None),
            stored(transaction("INTEREST", 4, Cancellation::Regular), None),
        ],
        TransactionBatch::try_new(Vec::new()).expect("미정산"),
        Some(at(RECORDED_AT)),
    )
    .expect("거래 집합")
}

fn directory() -> InstrumentDirectory {
    let names = InstrumentDirectory::default();
    names.replace(&[samsung()]);
    names
}

#[test]
fn transactions_view_orders_newest_first_names_instruments_and_marks_vanished_rows() {
    let record = SyncRecord::new(at(RECORDED_AT), SyncOutcome::Complete(rows()));
    let SyncOutcome::Complete(set) = record.outcome() else {
        panic!("완료");
    };
    let view = to_transactions_view(&record, set, &directory()).expect("화면");
    assert_eq!(view.recorded_at, "2026-11-25T09:00:00+09:00");
    let kinds: Vec<UiTransactionKind> = view.rows.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        vec![
            UiTransactionKind::Buy,
            UiTransactionKind::Sell,
            UiTransactionKind::Deposit,
            UiTransactionKind::Other,
        ]
    );
    assert_eq!(view.rows[0].name.as_deref(), Some("삼성전자"));
    assert_eq!(view.rows[0].quantity.as_deref(), Some("10"));
    assert_eq!(view.rows[0].amount.amount, "720000");
    assert_eq!(view.rows[0].settled_at, "2026-11-24T00:00:00+09:00");
    assert_eq!(view.rows[1].presence, UiTransactionPresence::Vanished);
    assert_eq!(view.rows[2].name, None);
    assert_eq!(view.rows[2].cash, Some(UiCurrency::Krw));
    assert_eq!(view.rows[2].presence, UiTransactionPresence::Present);
}

struct FixedBackend {
    record: Option<SyncRecord<SettledRowSet>>,
    failure: Option<SystemPortError>,
}

fn unused<T: Send + 'static>() -> BoxFuture<'static, Result<T, SystemPortError>> {
    Box::pin(std::future::ready(Err(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::InvalidData,
    ))))
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
        unused()
    }

    fn load_portfolio_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::PortfolioValuation>, SystemPortError>> {
        unused()
    }

    fn load_allocation_view(
        &self,
    ) -> BoxFuture<'_, Result<StoredView<domain::Allocation>, SystemPortError>> {
        unused()
    }

    fn load_transactions(
        &self,
    ) -> BoxFuture<'_, Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>> {
        Box::pin(std::future::ready(match self.failure {
            Some(error) => Err(error),
            None => Ok(self.record.clone()),
        }))
    }

    fn update_profile(
        &self,
        _requested: application::InvestorProfileRequest,
    ) -> BoxFuture<'_, Result<application::RiskSettingsChangeOutcome, SystemPortError>> {
        unused()
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

fn request() -> GetTransactionsRequest {
    GetTransactionsRequest {
        scope: TransactionsScope::Recent,
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
fn transactions_command_answers_the_recorded_view_for_the_typescript_fake() {
    let app = app_with(FixedBackend {
        record: Some(SyncRecord::new(
            at(RECORDED_AT),
            SyncOutcome::Complete(rows()),
        )),
        failure: None,
    });
    let view = tauri::async_runtime::block_on(get_transactions(app.handle().clone(), request()))
        .expect("거래내역");
    assert_snapshot("get_transactions", &view);
}

#[test]
fn missing_failed_blocked_storage_and_unmanaged_states_map_to_closed_errors() {
    let cases = [
        (None, IpcErrorKind::NoData),
        (
            Some(SyncRecord::new(at(RECORDED_AT), SyncOutcome::Failed)),
            IpcErrorKind::Retryable,
        ),
        (
            Some(SyncRecord::new(at(RECORDED_AT), SyncOutcome::AccessBlocked)),
            IpcErrorKind::UserFixable,
        ),
    ];
    for (record, expected) in cases {
        let app = app_with(FixedBackend {
            record,
            failure: None,
        });
        let error =
            tauri::async_runtime::block_on(get_transactions(app.handle().clone(), request()))
                .expect_err("오류");
        assert_eq!(error.kind, expected);
    }
    let app = app_with(FixedBackend {
        record: None,
        failure: Some(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        )),
    });
    let error = tauri::async_runtime::block_on(get_transactions(app.handle().clone(), request()))
        .expect_err("저장소 오류");
    assert_ne!(error.kind, IpcErrorKind::Bug);
    let bare = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing =
        tauri::async_runtime::block_on(get_transactions(bare.handle().clone(), request()))
            .expect_err("상태 없음");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
}

#[test]
fn transactions_request_refuses_unknown_fields() {
    assert_eq!(
        serde_json::from_str::<GetTransactionsRequest>(r#"{"scope":"recent"}"#).expect("요청"),
        request()
    );
    for input in [
        r#"{"scope":"recent","account":"413"}"#,
        r#"{"scope":"all"}"#,
    ] {
        assert!(
            serde_json::from_str::<GetTransactionsRequest>(input).is_err(),
            "{input}"
        );
    }
}
