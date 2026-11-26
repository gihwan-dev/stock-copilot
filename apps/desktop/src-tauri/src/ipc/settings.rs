use application::{ClassifiedError, RiskSettingsChangeOutcome, SystemPortError};
use domain::{
    HorizonSelection, HoursPermissionState, InvestorProfile, OrderPermission, RiskAppetite,
    RiskSettings, SavedMarketOrderSetting,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::IpcError;
use super::state::{LoadedSettings, ScreenState, SecretPresence};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetSettingsRequest {
    pub scope: SettingsQueryScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum SettingsQueryScope {
    #[serde(rename = "all")]
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetOrderPermissionRequest {
    pub permission: UiOrderPermission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiOrderPermission {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "on")]
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiToggleState {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "on")]
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiConnectionState {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "needs-key")]
    NeedsKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiOptionalKeyState {
    #[serde(rename = "configured")]
    Configured,
    #[serde(rename = "unconfigured")]
    Unconfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiHorizon {
    #[serde(rename = "hours")]
    Hours,
    #[serde(rename = "few-days")]
    FewDays,
    #[serde(rename = "weeks-to-months")]
    WeeksToMonths,
    #[serde(rename = "half-year-or-more")]
    HalfYearOrMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiRiskAppetite {
    #[serde(rename = "conservative")]
    Conservative,
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "aggressive")]
    Aggressive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSettingsView {
    pub toss_connection: UiConnectionState,
    pub horizon: UiHorizon,
    pub risk_appetite: UiRiskAppetite,
    pub instrument_cap_percent: String,
    pub instrument_warning_percent: String,
    pub sector_cap_percent: String,
    pub excluded_sector_count: u16,
    pub excluded_security_type_count: u16,
    pub order_enabled: UiOrderPermission,
    pub ai_budget_percent: String,
    pub dart_key: UiOptionalKeyState,
    pub fred_key: UiOptionalKeyState,
    pub allow_market_orders: UiToggleState,
    pub allow_hours_horizon: UiToggleState,
    pub card_expiry_minutes: u16,
    pub concentration_warning_threshold: u16,
    pub auto_start_at_login: UiToggleState,
    pub account_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiOrderSwitchOutcomeKind {
    #[serde(rename = "changed")]
    Changed,
    #[serde(rename = "unchanged")]
    Unchanged,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiOrderSwitchOutcome {
    pub kind: UiOrderSwitchOutcomeKind,
    pub order_enabled: UiOrderPermission,
}

fn ratio_to_percent_string(ratio: domain::Ratio) -> String {
    let precision = domain::DisplayPrecision::try_new(0).ok();
    precision.map_or_else(
        || String::from("0"),
        |value| domain::display_percent(&ratio, value).as_str().to_owned(),
    )
}

const fn map_horizon(selection: HorizonSelection) -> UiHorizon {
    match selection {
        HorizonSelection::Hours => UiHorizon::Hours,
        HorizonSelection::FewDays => UiHorizon::FewDays,
        HorizonSelection::WeeksToMonths => UiHorizon::WeeksToMonths,
        HorizonSelection::HalfYearOrMore => UiHorizon::HalfYearOrMore,
    }
}

const fn map_risk_appetite(appetite: RiskAppetite) -> UiRiskAppetite {
    match appetite {
        RiskAppetite::Conservative => UiRiskAppetite::Conservative,
        RiskAppetite::Neutral => UiRiskAppetite::Neutral,
        RiskAppetite::Aggressive => UiRiskAppetite::Aggressive,
    }
}

const fn map_optional_key(presence: SecretPresence) -> UiOptionalKeyState {
    match presence {
        SecretPresence::Configured => UiOptionalKeyState::Configured,
        SecretPresence::NotConfigured => UiOptionalKeyState::Unconfigured,
    }
}

const fn map_order_permission(permission: OrderPermission) -> UiOrderPermission {
    match permission {
        OrderPermission::Disabled => UiOrderPermission::Off,
        OrderPermission::Enabled => UiOrderPermission::On,
    }
}

fn saturating_count(count: usize) -> u16 {
    u16::try_from(count).ok().unwrap_or(u16::MAX)
}

fn to_settings_view(loaded: &LoadedSettings) -> UiSettingsView {
    let fallback = InvestorProfile::default();
    let profile = loaded.profile.as_ref().unwrap_or(&fallback);
    let risk = loaded.risk_settings.unwrap_or_else(|| {
        RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            profile.hours_permission().state(),
            profile.weight_caps(),
            profile.risk_appetite(),
        )
    });
    let caps = risk.weight_caps();
    UiSettingsView {
        toss_connection: match loaded.toss_connected {
            SecretPresence::Configured => UiConnectionState::Connected,
            SecretPresence::NotConfigured => UiConnectionState::NeedsKey,
        },
        horizon: map_horizon(profile.horizon().selection()),
        risk_appetite: map_risk_appetite(risk.risk_appetite()),
        instrument_cap_percent: ratio_to_percent_string(caps.instrument_cap().as_ratio()),
        instrument_warning_percent: ratio_to_percent_string(caps.instrument_warning().as_ratio()),
        sector_cap_percent: ratio_to_percent_string(caps.sector_cap().as_ratio()),
        excluded_sector_count: saturating_count(profile.exclusions().sectors().len()),
        excluded_security_type_count: saturating_count(profile.exclusions().security_types().len()),
        order_enabled: map_order_permission(risk.orders()),
        ai_budget_percent: String::from("70"),
        dart_key: map_optional_key(loaded.dart_key),
        fred_key: map_optional_key(loaded.fred_key),
        allow_market_orders: match risk.market_orders() {
            SavedMarketOrderSetting::Disabled => UiToggleState::Off,
            SavedMarketOrderSetting::Enabled => UiToggleState::On,
        },
        allow_hours_horizon: match risk.hours() {
            HoursPermissionState::Disabled => UiToggleState::Off,
            HoursPermissionState::Enabled => UiToggleState::On,
        },
        card_expiry_minutes: 10,
        concentration_warning_threshold: 5,
        auto_start_at_login: UiToggleState::On,
        account_count: loaded.account_count,
    }
}

fn map_system_error(error: &SystemPortError) -> IpcError {
    IpcError::from(error.class())
}

pub async fn load_settings_view(state: &ScreenState) -> Result<UiSettingsView, IpcError> {
    let loaded = state
        .backend()
        .load_settings()
        .await
        .map_err(|error| map_system_error(&error))?;
    Ok(to_settings_view(&loaded))
}

pub async fn apply_order_permission(
    state: &ScreenState,
    permission: UiOrderPermission,
) -> Result<UiOrderSwitchOutcome, IpcError> {
    let loaded = state
        .backend()
        .load_settings()
        .await
        .map_err(|error| map_system_error(&error))?;
    let fallback = InvestorProfile::default();
    let profile = loaded.profile.as_ref().unwrap_or(&fallback);
    let current = loaded.risk_settings.unwrap_or_else(|| {
        RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            profile.hours_permission().state(),
            profile.weight_caps(),
            profile.risk_appetite(),
        )
    });
    let next_orders = match permission {
        UiOrderPermission::Off => OrderPermission::Disabled,
        UiOrderPermission::On => OrderPermission::Enabled,
    };
    let requested = RiskSettings::new(
        next_orders,
        current.market_orders(),
        current.hours(),
        current.weight_caps(),
        current.risk_appetite(),
    );
    let outcome = state
        .backend()
        .change_risk_settings(requested)
        .await
        .map_err(|error| map_system_error(&error))?;
    let result = match outcome {
        RiskSettingsChangeOutcome::Changed(_) => UiOrderSwitchOutcome {
            kind: UiOrderSwitchOutcomeKind::Changed,
            order_enabled: permission,
        },
        RiskSettingsChangeOutcome::Unchanged => UiOrderSwitchOutcome {
            kind: UiOrderSwitchOutcomeKind::Unchanged,
            order_enabled: map_order_permission(current.orders()),
        },
        RiskSettingsChangeOutcome::Denied => UiOrderSwitchOutcome {
            kind: UiOrderSwitchOutcomeKind::Denied,
            order_enabled: map_order_permission(current.orders()),
        },
        RiskSettingsChangeOutcome::Unavailable | RiskSettingsChangeOutcome::ProfileRejected(_) => {
            UiOrderSwitchOutcome {
                kind: UiOrderSwitchOutcomeKind::Unavailable,
                order_enabled: map_order_permission(current.orders()),
            }
        }
    };
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings<R: Runtime>(
    app: AppHandle<R>,
    request: GetSettingsRequest,
) -> Result<UiSettingsView, IpcError> {
    let GetSettingsRequest {
        scope: SettingsQueryScope::All,
    } = request;
    let view = load_settings_view(app.state::<ScreenState>().inner()).await?;
    Ok(
        match app.try_state::<crate::platform::autostart::LoginItemsState>() {
            Some(login) => UiSettingsView {
                auto_start_at_login: super::autostart::toggle(login.items.desired_state()),
                ..view
            },
            None => view,
        },
    )
}

#[tauri::command]
#[specta::specta]
pub async fn set_order_permission<R: Runtime>(
    app: AppHandle<R>,
    request: SetOrderPermissionRequest,
) -> Result<UiOrderSwitchOutcome, IpcError> {
    apply_order_permission(app.state::<ScreenState>().inner(), request.permission).await
}
