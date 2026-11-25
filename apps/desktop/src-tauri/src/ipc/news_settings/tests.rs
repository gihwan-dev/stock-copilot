use std::fs;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, RwLock};

use application::{
    EventBus, NewsDataSettings, NewsDataSettingsRepository, SecretKey, SecretStore, SecretValue,
    SystemPortError, WriteCompleted,
};
use domain::{
    CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds, Symbol, Timestamp,
    TradingSuspension, UnixSeconds,
};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    NewsRequest, NewsScope, NewsSettingsIpcState, UiAliasInput, UiBodyReading, UiDailyMove,
    UiFeedSource, UiInterval, UiKeyPresence, UiNewsPerHour, UiOptionalKey, UiSourceSettingInput,
    UiToggle, UiWakeInput, change_contact, change_source, change_wake, get_news_settings,
    load_news, replace_aliases, store_optional_key,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;

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
        observed_at: at(1_795_564_800),
    })
    .expect("마스터")
}

#[derive(Default)]
struct MemoryNews(RwLock<Option<NewsDataSettings>>);

impl NewsDataSettingsRepository for MemoryNews {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send {
        std::future::ready(Ok(self.0.read().expect("잠금").clone()))
    }

    fn store(
        &self,
        settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        *self.0.write().expect("잠금") = Some(settings.clone());
        std::future::ready(Ok(WriteCompleted))
    }
}

#[derive(Default)]
struct MemorySecrets(RwLock<Vec<SecretKey>>);

impl SecretStore for MemorySecrets {
    fn load(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
        let found = self.0.read().expect("잠금").contains(&key);
        std::future::ready(Ok(found.then(|| SecretValue::new(b"secret".to_vec()))))
    }

    fn store(
        &self,
        key: SecretKey,
        _value: SecretValue,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        self.0.write().expect("잠금").push(key);
        std::future::ready(Ok(WriteCompleted))
    }

    fn remove(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        self.0
            .write()
            .expect("잠금")
            .retain(|stored| *stored != key);
        std::future::ready(Ok(WriteCompleted))
    }
}

fn state() -> NewsSettingsIpcState {
    NewsSettingsIpcState::new(
        Arc::new(MemoryNews::default()),
        Arc::new(MemorySecrets::default()),
        Arc::new(EventBus::default()),
    )
}

fn directory() -> InstrumentDirectory {
    let names = InstrumentDirectory::default();
    names.replace(&[samsung()]);
    names
}

fn assert_snapshot<T: serde::Serialize>(name: &str, value: &T) {
    let path = Path::new("ipc/responses").join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

#[test]
fn initial_settings_list_every_source_with_default_cadence_and_locked_reading() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, state());
    let _names = tauri::Manager::manage(&app, directory());
    let view = tauri::async_runtime::block_on(get_news_settings(
        app.handle().clone(),
        NewsRequest {
            scope: NewsScope::Current,
        },
    ))
    .expect("뉴스 설정");
    assert_eq!(view.feeds.len(), 15);
    let kind = &view.feeds[0];
    assert_eq!(kind.source, UiFeedSource::KindDisclosure);
    assert_eq!(kind.interval_minutes, UiInterval::Two);
    assert_eq!(kind.body_reading, UiBodyReading::Locked);
    let hankyung = view
        .feeds
        .iter()
        .find(|row| row.source == UiFeedSource::Hankyung)
        .expect("한경");
    assert_eq!(hankyung.body_reading, UiBodyReading::Off);
    assert_eq!(view.keys.dart, UiKeyPresence::Unset);
    assert_eq!(view.wake.daily_move_percent, UiDailyMove::Five);
    assert_snapshot("get_news_settings", &view);
}

#[test]
fn source_switch_reading_and_cadence_are_saved_and_locked_reading_is_refused() {
    let news = state();
    let names = directory();
    let changed = tauri::async_runtime::block_on(change_source(
        &news,
        &names,
        UiSourceSettingInput {
            source: UiFeedSource::Hankyung,
            state: UiToggle::Off,
            body_reading: UiBodyReading::On,
            interval_minutes: UiInterval::Fifteen,
        },
    ))
    .expect("바꾸기");
    let hankyung = changed
        .feeds
        .iter()
        .find(|row| row.source == UiFeedSource::Hankyung)
        .expect("한경");
    assert_eq!(hankyung.state, UiToggle::Off);
    assert_eq!(hankyung.body_reading, UiBodyReading::On);
    assert_eq!(hankyung.interval_minutes, UiInterval::Fifteen);
    let refused = tauri::async_runtime::block_on(change_source(
        &news,
        &names,
        UiSourceSettingInput {
            source: UiFeedSource::Yonhap,
            state: UiToggle::On,
            body_reading: UiBodyReading::On,
            interval_minutes: UiInterval::Ten,
        },
    ))
    .expect_err("연합 본문 잠김");
    assert_eq!(refused.kind, IpcErrorKind::UserFixable);
}

#[test]
fn keys_contact_wake_and_aliases_round_trip_and_bad_values_are_refused() {
    let news = state();
    let names = directory();
    let keyed = tauri::async_runtime::block_on(store_optional_key(
        &news,
        &names,
        UiOptionalKey::Dart,
        " dart-key ",
    ))
    .expect("키");
    assert_eq!(keyed.keys.dart, UiKeyPresence::Set);
    let empty_key = tauri::async_runtime::block_on(store_optional_key(
        &news,
        &names,
        UiOptionalKey::Fred,
        "  ",
    ))
    .expect_err("빈 키");
    assert_eq!(empty_key.kind, IpcErrorKind::UserFixable);
    let contact =
        tauri::async_runtime::block_on(change_contact(&news, &names, Some("me@example.com")))
            .expect("연락처");
    assert_eq!(contact.sec_contact.as_deref(), Some("me@example.com"));
    let bad_contact =
        tauri::async_runtime::block_on(change_contact(&news, &names, Some("no-email")))
            .expect_err("메일 없음");
    assert_eq!(bad_contact.kind, IpcErrorKind::UserFixable);
    let woken = tauri::async_runtime::block_on(change_wake(
        &news,
        &names,
        UiWakeInput {
            daily_move_percent: UiDailyMove::Seven,
            news_per_hour: UiNewsPerHour::Ten,
        },
    ))
    .expect("기준");
    assert_eq!(woken.wake.news_per_hour, UiNewsPerHour::Ten);
}

#[test]
fn aliases_are_replaced_as_a_whole_and_unknown_tickers_are_refused() {
    let news = state();
    let names = directory();
    let _contact =
        tauri::async_runtime::block_on(change_contact(&news, &names, Some("me@example.com")))
            .expect("연락처");
    let aliased = tauri::async_runtime::block_on(replace_aliases(
        &news,
        &names,
        &[UiAliasInput {
            alias: String::from("삼전"),
            ticker: String::from("005930"),
        }],
    ))
    .expect("약칭");
    assert_eq!(aliased.aliases.len(), 1);
    assert_eq!(aliased.aliases[0].ticker, "005930");
    let cleared =
        tauri::async_runtime::block_on(replace_aliases(&news, &names, &[])).expect("약칭 비우기");
    assert_eq!(cleared.aliases.len(), 0);
    let unknown = tauri::async_runtime::block_on(replace_aliases(
        &news,
        &names,
        &[UiAliasInput {
            alias: String::from("모름"),
            ticker: String::from("123456"),
        }],
    ))
    .expect_err("모르는 종목");
    assert_eq!(unknown.kind, IpcErrorKind::UserFixable);
    let reread = tauri::async_runtime::block_on(load_news(&news, &names)).expect("다시 읽기");
    assert_eq!(reread.sec_contact.as_deref(), Some("me@example.com"));
}
