use std::fmt;
use std::sync::Arc;

use application::{
    AliasName, CollectionCadence, CollectionConditions, CollectionMinutes, CollectionPhase,
    CollectionTarget, DartCallCount, DartDailyUsage, DartKeyState, EventBus, FomcDay,
    GetNewsDataSettings, NewsDataSettings, NewsDataSettingsChange, NewsDataSettingsError,
    NewsDataSettingsRepository, SecContactSetting, SecretKey, SecretStore, SecretValue,
    SourceState, SourceStatus, SourceSwitch, StockAlias, SystemPortError, UpdateNewsDataSettings,
    collection_cadence,
};
use domain::{
    AiBodyReadToggle, DataSource, MovePercent, NewsBurstCount, NewsSource, Symbol, WakeThresholds,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::state::{BoxFuture, SecretPresence};
use crate::holdings_link::InstrumentDirectory;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reach_tests;

pub trait NewsBook: Send + Sync {
    fn load(&self) -> BoxFuture<'_, Result<NewsDataSettings, SystemPortError>>;
    fn update(
        &self,
        change: NewsDataSettingsChange,
    ) -> BoxFuture<'_, Result<NewsDataSettings, NewsDataSettingsError>>;
    fn key_present(&self, key: SecretKey)
    -> BoxFuture<'_, Result<SecretPresence, SystemPortError>>;
    fn store_key(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> BoxFuture<'_, Result<(), SystemPortError>>;
}

struct StoredNews<R, S> {
    repository: Arc<R>,
    secrets: Arc<S>,
    events: Arc<EventBus>,
}

impl<R: NewsDataSettingsRepository + 'static, S: SecretStore + 'static> NewsBook
    for StoredNews<R, S>
{
    fn load(&self) -> BoxFuture<'_, Result<NewsDataSettings, SystemPortError>> {
        Box::pin(async move {
            GetNewsDataSettings {
                repository: self.repository.as_ref(),
            }
            .execute()
            .await
        })
    }

    fn update(
        &self,
        change: NewsDataSettingsChange,
    ) -> BoxFuture<'_, Result<NewsDataSettings, NewsDataSettingsError>> {
        Box::pin(async move {
            UpdateNewsDataSettings {
                repository: self.repository.as_ref(),
                events: &self.events,
            }
            .execute(change)
            .await
            .map(|(settings, _delivery)| settings)
        })
    }

    fn key_present(
        &self,
        key: SecretKey,
    ) -> BoxFuture<'_, Result<SecretPresence, SystemPortError>> {
        Box::pin(async move {
            Ok(match self.secrets.load(key).await? {
                Some(_) => SecretPresence::Configured,
                None => SecretPresence::NotConfigured,
            })
        })
    }

    fn store_key(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> BoxFuture<'_, Result<(), SystemPortError>> {
        Box::pin(async move {
            self.secrets.store(key, value).await?;
            let _delivery = self.events.publish(domain::DomainEvent::SettingsChanged);
            Ok(())
        })
    }
}

pub trait RouteLookup: Send + Sync {
    fn source_status(&self, source: DataSource) -> BoxFuture<'_, Option<SourceStatus>>;
}

#[derive(Clone)]
pub struct NewsSettingsIpcState(Arc<dyn NewsBook>, Option<Arc<dyn RouteLookup>>);

impl fmt::Debug for NewsSettingsIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NewsSettingsIpcState")
    }
}

impl NewsSettingsIpcState {
    #[must_use]
    pub fn new<R: NewsDataSettingsRepository + 'static, S: SecretStore + 'static>(
        repository: Arc<R>,
        secrets: Arc<S>,
        events: Arc<EventBus>,
    ) -> Self {
        Self(
            Arc::new(StoredNews {
                repository,
                secrets,
                events,
            }),
            None,
        )
    }

    #[must_use]
    pub fn with_routes(mut self, routes: Arc<dyn RouteLookup>) -> Self {
        self.1 = Some(routes);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiRouteState {
    #[serde(rename = "unset")]
    Unset,
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "failing")]
    Failing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiDisclosureRoutes {
    pub dart: UiRouteState,
    pub sec: UiRouteState,
}

const fn route_of(state: Option<SourceState>) -> UiRouteState {
    match state {
        Some(SourceState::Healthy | SourceState::NoData) => UiRouteState::Connected,
        Some(SourceState::Failed(_) | SourceState::NotConfigured) => UiRouteState::Failing,
        Some(SourceState::NeverRun) | None => UiRouteState::Waiting,
    }
}

async fn routes(
    state: &NewsSettingsIpcState,
    keys: UiKeys,
    settings: &NewsDataSettings,
) -> UiDisclosureRoutes {
    let lookup = |source| async move {
        match &state.1 {
            Some(routes) => routes
                .source_status(source)
                .await
                .map(|status| status.state()),
            None => None,
        }
    };
    let dart = match keys.dart {
        UiKeyPresence::Unset => UiRouteState::Unset,
        UiKeyPresence::Set => route_of(lookup(DataSource::DartDisclosureApi).await),
    };
    let sec = match settings.sec_contact() {
        None => UiRouteState::Unset,
        Some(_) => route_of(lookup(DataSource::SecEdgar).await),
    };
    UiDisclosureRoutes { dart, sec }
}

const fn never_reached(status: &SourceStatus) -> bool {
    match (status.state(), status.last_success_at()) {
        (SourceState::Failed(_), None) => true,
        (
            SourceState::Failed(_)
            | SourceState::NeverRun
            | SourceState::Healthy
            | SourceState::NoData
            | SourceState::NotConfigured,
            _,
        ) => false,
    }
}

async fn unreachable_sources(lookup: Option<&dyn RouteLookup>) -> Vec<UiFeedSource> {
    let Some(routes) = lookup else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for source in SOURCES {
        if let Some(status) = routes.source_status(data_source(source)).await
            && never_reached(&status)
        {
            found.push(source);
        }
    }
    found
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub enum UiFeedSource {
    KindDisclosure,
    DartDisclosureApi,
    DartDisclosureRss,
    DartFundamentals,
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    SegyeFinance,
    GoogleNews,
    YahooFinance,
    SecEdgar,
    BankOfKorea,
    FederalReserve,
    UsTreasury,
    Fred,
}

const SOURCES: [UiFeedSource; 14] = [
    UiFeedSource::KindDisclosure,
    UiFeedSource::DartDisclosureApi,
    UiFeedSource::DartDisclosureRss,
    UiFeedSource::DartFundamentals,
    UiFeedSource::Hankyung,
    UiFeedSource::Maekyung,
    UiFeedSource::Yonhap,
    UiFeedSource::GoogleNews,
    UiFeedSource::YahooFinance,
    UiFeedSource::SecEdgar,
    UiFeedSource::BankOfKorea,
    UiFeedSource::FederalReserve,
    UiFeedSource::UsTreasury,
    UiFeedSource::Fred,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiToggle {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiBodyReading {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "locked")]
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiInterval {
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "15")]
    Fifteen,
    #[serde(rename = "30")]
    Thirty,
    #[serde(rename = "60")]
    Sixty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiDailyMove {
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "7")]
    Seven,
    #[serde(rename = "10")]
    Ten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiNewsPerHour {
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "20")]
    Twenty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiKeyPresence {
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "unset")]
    Unset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum UiOptionalKey {
    #[serde(rename = "dart")]
    Dart,
    #[serde(rename = "fred")]
    Fred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiSourceSetting {
    pub source: UiFeedSource,
    pub state: UiToggle,
    pub body_reading: UiBodyReading,
    pub interval_minutes: UiInterval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiSourceSettingInput {
    pub source: UiFeedSource,
    pub state: UiToggle,
    pub body_reading: UiBodyReading,
    pub interval_minutes: UiInterval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiKeys {
    pub dart: UiKeyPresence,
    pub fred: UiKeyPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiWake {
    pub daily_move_percent: UiDailyMove,
    pub news_per_hour: UiNewsPerHour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiWakeInput {
    pub daily_move_percent: UiDailyMove,
    pub news_per_hour: UiNewsPerHour,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAliasRow {
    pub alias: String,
    pub ticker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiAliasInput {
    pub alias: String,
    pub ticker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiNewsSettings {
    #[serde(rename = "sources")]
    pub feeds: Vec<UiSourceSetting>,
    pub keys: UiKeys,
    pub sec_contact: Option<String>,
    pub wake: UiWake,
    pub aliases: Vec<UiAliasRow>,
    pub routes: UiDisclosureRoutes,
    pub unreachable: Vec<UiFeedSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum NewsScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct NewsRequest {
    pub scope: NewsScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetNewsSourceRequest {
    pub setting: UiSourceSettingInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetOptionalKeyRequest {
    pub key: UiOptionalKey,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetSecContactRequest {
    pub contact: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetWakeCriteriaRequest {
    pub wake: UiWakeInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetAliasesRequest {
    pub aliases: Vec<UiAliasInput>,
}

const fn data_source(source: UiFeedSource) -> DataSource {
    match source {
        UiFeedSource::KindDisclosure => DataSource::KindDisclosure,
        UiFeedSource::DartDisclosureApi => DataSource::DartDisclosureApi,
        UiFeedSource::DartDisclosureRss => DataSource::DartDisclosureRss,
        UiFeedSource::DartFundamentals => DataSource::DartFundamentals,
        UiFeedSource::Hankyung => DataSource::Hankyung,
        UiFeedSource::Maekyung => DataSource::Maekyung,
        UiFeedSource::Yonhap => DataSource::Yonhap,
        UiFeedSource::Edaily => DataSource::Edaily,
        UiFeedSource::SegyeFinance => DataSource::SegyeFinance,
        UiFeedSource::GoogleNews => DataSource::GoogleNews,
        UiFeedSource::YahooFinance => DataSource::YahooFinance,
        UiFeedSource::SecEdgar => DataSource::SecEdgar,
        UiFeedSource::BankOfKorea => DataSource::BankOfKorea,
        UiFeedSource::FederalReserve => DataSource::FederalReserve,
        UiFeedSource::UsTreasury => DataSource::UsTreasury,
        UiFeedSource::Fred => DataSource::Fred,
    }
}

const fn news_source(source: UiFeedSource) -> Option<NewsSource> {
    match source {
        UiFeedSource::Hankyung => Some(NewsSource::Hankyung),
        UiFeedSource::Maekyung => Some(NewsSource::Maekyung),
        UiFeedSource::Yonhap => Some(NewsSource::Yonhap),
        UiFeedSource::Edaily => Some(NewsSource::Edaily),
        UiFeedSource::SegyeFinance => Some(NewsSource::SegyeFinance),
        UiFeedSource::GoogleNews => Some(NewsSource::GoogleNews),
        UiFeedSource::YahooFinance => Some(NewsSource::YahooFinance),
        UiFeedSource::BankOfKorea => Some(NewsSource::BankOfKorea),
        UiFeedSource::FederalReserve => Some(NewsSource::FederalReserve),
        UiFeedSource::KindDisclosure
        | UiFeedSource::DartDisclosureApi
        | UiFeedSource::DartDisclosureRss
        | UiFeedSource::DartFundamentals
        | UiFeedSource::SecEdgar
        | UiFeedSource::UsTreasury
        | UiFeedSource::Fred => None,
    }
}

const fn target(source: UiFeedSource) -> CollectionTarget {
    match source {
        UiFeedSource::KindDisclosure => CollectionTarget::KindDisclosure,
        UiFeedSource::DartDisclosureApi
        | UiFeedSource::DartDisclosureRss
        | UiFeedSource::DartFundamentals => CollectionTarget::DartList,
        UiFeedSource::Hankyung => CollectionTarget::HankyungRss,
        UiFeedSource::Maekyung
        | UiFeedSource::Yonhap
        | UiFeedSource::Edaily
        | UiFeedSource::SegyeFinance => CollectionTarget::PressRss,
        UiFeedSource::GoogleNews => CollectionTarget::GoogleNews,
        UiFeedSource::YahooFinance => CollectionTarget::YahooFinance,
        UiFeedSource::SecEdgar => CollectionTarget::SecEdgar,
        UiFeedSource::BankOfKorea | UiFeedSource::FederalReserve => {
            CollectionTarget::CentralBankRss
        }
        UiFeedSource::UsTreasury | UiFeedSource::Fred => CollectionTarget::TreasuryAndFred,
    }
}

const fn interval_minutes(interval: UiInterval) -> u16 {
    match interval {
        UiInterval::One => 1,
        UiInterval::Two => 2,
        UiInterval::Five => 5,
        UiInterval::Ten => 10,
        UiInterval::Fifteen => 15,
        UiInterval::Thirty => 30,
        UiInterval::Sixty => 60,
    }
}

const fn ui_interval(minutes: u16) -> UiInterval {
    match minutes {
        0..=1 => UiInterval::One,
        2..=4 => UiInterval::Two,
        5..=9 => UiInterval::Five,
        10..=14 => UiInterval::Ten,
        15..=29 => UiInterval::Fifteen,
        30..=59 => UiInterval::Thirty,
        _ => UiInterval::Sixty,
    }
}

const DEFAULT_CONDITIONS: CollectionConditions = CollectionConditions {
    phase: CollectionPhase::DomesticSession,
    fomc: FomcDay::Regular,
    dart_key: DartKeyState::Configured,
    dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(1)),
};

fn shown_interval(settings: &NewsDataSettings, source: UiFeedSource) -> UiInterval {
    let goal = target(source);
    let minutes = settings.cadence_override(goal).map_or_else(
        || match collection_cadence(goal, &DEFAULT_CONDITIONS) {
            CollectionCadence::Every(every) => *every.as_u16(),
            CollectionCadence::AfterUsClose | CollectionCadence::Off => 60,
        },
        |chosen| *chosen.as_u16(),
    );
    ui_interval(minutes)
}

fn body_reading(settings: &NewsDataSettings, source: UiFeedSource) -> UiBodyReading {
    match news_source(source) {
        Some(NewsSource::Yonhap | NewsSource::GoogleNews | NewsSource::SegyeFinance) | None => {
            UiBodyReading::Locked
        }
        Some(news) => match settings.body_read().toggle_for(&news) {
            AiBodyReadToggle::Enabled => UiBodyReading::On,
            AiBodyReadToggle::Disabled => UiBodyReading::Off,
        },
    }
}

const fn daily_move(percent: u8) -> UiDailyMove {
    match percent {
        0..=3 => UiDailyMove::Three,
        4..=5 => UiDailyMove::Five,
        6..=7 => UiDailyMove::Seven,
        _ => UiDailyMove::Ten,
    }
}

const fn per_hour(count: u8) -> UiNewsPerHour {
    match count {
        0..=3 => UiNewsPerHour::Three,
        4..=5 => UiNewsPerHour::Five,
        6..=10 => UiNewsPerHour::Ten,
        _ => UiNewsPerHour::Twenty,
    }
}

const fn presence(present: SecretPresence) -> UiKeyPresence {
    match present {
        SecretPresence::Configured => UiKeyPresence::Set,
        SecretPresence::NotConfigured => UiKeyPresence::Unset,
    }
}

fn to_view(
    settings: &NewsDataSettings,
    keys: UiKeys,
    routes: UiDisclosureRoutes,
    unreachable: Vec<UiFeedSource>,
    directory: &InstrumentDirectory,
) -> UiNewsSettings {
    UiNewsSettings {
        feeds: SOURCES
            .into_iter()
            .map(|source| UiSourceSetting {
                source,
                state: match settings.source_switch(data_source(source)) {
                    SourceSwitch::On => UiToggle::On,
                    SourceSwitch::Off => UiToggle::Off,
                },
                body_reading: body_reading(settings, source),
                interval_minutes: shown_interval(settings, source),
            })
            .collect(),
        keys,
        sec_contact: settings
            .sec_contact()
            .map(|contact| contact.as_str().to_owned()),
        wake: UiWake {
            daily_move_percent: daily_move(*settings.wake().daily_move.as_u8()),
            news_per_hour: per_hour(*settings.wake().news_per_hour.as_u8()),
        },
        aliases: settings
            .aliases()
            .iter()
            .filter_map(|alias| {
                let entry = directory.by_instrument(&alias.instrument)?;
                Some(UiAliasRow {
                    alias: alias.alias.as_str().to_owned(),
                    ticker: entry.master.row().symbol.as_str().to_owned(),
                })
            })
            .collect(),
        routes,
        unreachable,
    }
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn storage(failure: &SystemPortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

fn rejected(failure: &NewsDataSettingsError) -> IpcError {
    match failure {
        NewsDataSettingsError::Storage(storage_failure) => storage(storage_failure),
        NewsDataSettingsError::BodyReadLocked(_)
        | NewsDataSettingsError::DuplicateAlias
        | NewsDataSettingsError::TooManyAliases
        | NewsDataSettingsError::AliasMissing
        | NewsDataSettingsError::CadenceNotAdjustable(_) => error(IpcErrorKind::UserFixable),
    }
}

async fn keys(state: &NewsSettingsIpcState) -> Result<UiKeys, IpcError> {
    let dart = state
        .0
        .key_present(SecretKey::DartApiKey)
        .await
        .map_err(|failure| storage(&failure))?;
    let fred = state
        .0
        .key_present(SecretKey::FredApiKey)
        .await
        .map_err(|failure| storage(&failure))?;
    Ok(UiKeys {
        dart: presence(dart),
        fred: presence(fred),
    })
}

async fn view(
    state: &NewsSettingsIpcState,
    settings: &NewsDataSettings,
    directory: &InstrumentDirectory,
) -> Result<UiNewsSettings, IpcError> {
    let found = keys(state).await?;
    let paths = routes(state, found, settings).await;
    let unreachable = unreachable_sources(state.1.as_deref()).await;
    Ok(to_view(settings, found, paths, unreachable, directory))
}

async fn apply(
    state: &NewsSettingsIpcState,
    change: NewsDataSettingsChange,
) -> Result<NewsDataSettings, IpcError> {
    state
        .0
        .update(change)
        .await
        .map_err(|failure| rejected(&failure))
}

pub async fn load_news(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
) -> Result<UiNewsSettings, IpcError> {
    let settings = state.0.load().await.map_err(|failure| storage(&failure))?;
    view(state, &settings, directory).await
}

pub async fn change_source(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
    setting: UiSourceSettingInput,
) -> Result<UiNewsSettings, IpcError> {
    let current = state.0.load().await.map_err(|failure| storage(&failure))?;
    let switch = match setting.state {
        UiToggle::On => SourceSwitch::On,
        UiToggle::Off => SourceSwitch::Off,
    };
    let mut settings = apply(
        state,
        NewsDataSettingsChange::Source(data_source(setting.source), switch),
    )
    .await?;
    let reading_changed = setting.body_reading != body_reading(&current, setting.source);
    if reading_changed {
        let news = news_source(setting.source).ok_or(error(IpcErrorKind::UserFixable))?;
        let toggle = match setting.body_reading {
            UiBodyReading::On => AiBodyReadToggle::Enabled,
            UiBodyReading::Off | UiBodyReading::Locked => AiBodyReadToggle::Disabled,
        };
        let updated = apply(state, NewsDataSettingsChange::BodyRead(news, toggle)).await?;
        settings = updated;
    }
    if setting.interval_minutes != shown_interval(&current, setting.source) {
        let minutes = CollectionMinutes::try_new(interval_minutes(setting.interval_minutes))
            .map_err(|_range| error(IpcErrorKind::UserFixable))?;
        settings = apply(
            state,
            NewsDataSettingsChange::Cadence(target(setting.source), Some(minutes)),
        )
        .await?;
    }
    view(state, &settings, directory).await
}

pub async fn store_optional_key(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
    key: UiOptionalKey,
    secret: &str,
) -> Result<UiNewsSettings, IpcError> {
    let trimmed = secret.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(error(IpcErrorKind::UserFixable));
    }
    let slot = match key {
        UiOptionalKey::Dart => SecretKey::DartApiKey,
        UiOptionalKey::Fred => SecretKey::FredApiKey,
    };
    state
        .0
        .store_key(slot, SecretValue::new(trimmed.as_bytes().to_vec()))
        .await
        .map_err(|failure| storage(&failure))?;
    load_news(state, directory).await
}

pub async fn change_contact(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
    contact: Option<&str>,
) -> Result<UiNewsSettings, IpcError> {
    let setting = match contact {
        Some(text) => Some(
            SecContactSetting::try_new(text)
                .map_err(|_invalid| error(IpcErrorKind::UserFixable))?,
        ),
        None => None,
    };
    let settings = apply(state, NewsDataSettingsChange::SecContact(setting)).await?;
    view(state, &settings, directory).await
}

const fn move_value(value: UiDailyMove) -> u8 {
    match value {
        UiDailyMove::Three => 3,
        UiDailyMove::Five => 5,
        UiDailyMove::Seven => 7,
        UiDailyMove::Ten => 10,
    }
}

const fn burst_value(value: UiNewsPerHour) -> u8 {
    match value {
        UiNewsPerHour::Three => 3,
        UiNewsPerHour::Five => 5,
        UiNewsPerHour::Ten => 10,
        UiNewsPerHour::Twenty => 20,
    }
}

pub async fn change_wake(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
    wake: UiWakeInput,
) -> Result<UiNewsSettings, IpcError> {
    let thresholds = WakeThresholds {
        daily_move: MovePercent::try_new(move_value(wake.daily_move_percent))
            .map_err(|_range| error(IpcErrorKind::Bug))?,
        news_per_hour: NewsBurstCount::try_new(burst_value(wake.news_per_hour))
            .map_err(|_range| error(IpcErrorKind::Bug))?,
    };
    let settings = apply(state, NewsDataSettingsChange::Wake(thresholds)).await?;
    view(state, &settings, directory).await
}

fn requested_aliases(
    rows: &[UiAliasInput],
    directory: &InstrumentDirectory,
) -> Result<Vec<StockAlias>, IpcError> {
    rows.iter()
        .map(|row| {
            let symbol = Symbol::try_new(row.ticker.trim())
                .map_err(|_bad| error(IpcErrorKind::UserFixable))?;
            let entry = directory
                .entry(&symbol)
                .ok_or(error(IpcErrorKind::UserFixable))?;
            Ok(StockAlias {
                instrument: entry.master.row().instrument_id.clone(),
                alias: AliasName::try_new(&row.alias)
                    .map_err(|_bad| error(IpcErrorKind::UserFixable))?,
            })
        })
        .collect()
}

pub async fn replace_aliases(
    state: &NewsSettingsIpcState,
    directory: &InstrumentDirectory,
    rows: &[UiAliasInput],
) -> Result<UiNewsSettings, IpcError> {
    let wanted = requested_aliases(rows, directory)?;
    let mut settings = state.0.load().await.map_err(|failure| storage(&failure))?;
    let outdated: Vec<AliasName> = settings
        .aliases()
        .iter()
        .filter(|existing| !wanted.contains(existing))
        .map(|existing| existing.alias.clone())
        .collect();
    for name in outdated {
        settings = apply(state, NewsDataSettingsChange::RemoveAlias(name)).await?;
    }
    let fresh: Vec<StockAlias> = wanted
        .into_iter()
        .filter(|alias| !settings.aliases().contains(alias))
        .collect();
    for alias in fresh {
        settings = apply(state, NewsDataSettingsChange::AddAlias(alias)).await?;
    }
    view(state, &settings, directory).await
}

fn managed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(NewsSettingsIpcState, InstrumentDirectory), IpcError> {
    let state = app
        .try_state::<NewsSettingsIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
    Ok((state, directory))
}

#[tauri::command]
#[specta::specta]
pub async fn get_news_settings<R: Runtime>(
    app: AppHandle<R>,
    request: NewsRequest,
) -> Result<UiNewsSettings, IpcError> {
    let NewsRequest {
        scope: NewsScope::Current,
    } = request;
    let (state, directory) = managed(&app)?;
    load_news(&state, &directory).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_news_source<R: Runtime>(
    app: AppHandle<R>,
    request: SetNewsSourceRequest,
) -> Result<UiNewsSettings, IpcError> {
    let (state, directory) = managed(&app)?;
    change_source(&state, &directory, request.setting).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_optional_key<R: Runtime>(
    app: AppHandle<R>,
    request: SetOptionalKeyRequest,
) -> Result<UiNewsSettings, IpcError> {
    let (state, directory) = managed(&app)?;
    store_optional_key(&state, &directory, request.key, &request.secret).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_sec_contact<R: Runtime>(
    app: AppHandle<R>,
    request: SetSecContactRequest,
) -> Result<UiNewsSettings, IpcError> {
    let (state, directory) = managed(&app)?;
    change_contact(&state, &directory, request.contact.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_wake_criteria<R: Runtime>(
    app: AppHandle<R>,
    request: SetWakeCriteriaRequest,
) -> Result<UiNewsSettings, IpcError> {
    let (state, directory) = managed(&app)?;
    change_wake(&state, &directory, request.wake).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_aliases<R: Runtime>(
    app: AppHandle<R>,
    request: SetAliasesRequest,
) -> Result<UiNewsSettings, IpcError> {
    let (state, directory) = managed(&app)?;
    replace_aliases(&state, &directory, &request.aliases).await
}
