use application::{ClassifiedError, InvestorProfileRequest, RiskSettingsChangeOutcome};
use domain::{
    ExcludedSecurityType, Exclusions, HorizonSelection, HoursPermissionState, InvestorProfile,
    NumericInput, OrderPermission, ProfileWeight, Ratio, RiskAppetite, RiskSettings,
    SavedMarketOrderSetting, WeightCaps,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::state::{LoadedSettings, ScreenState};

const INSTRUMENT_WARNING_PERCENT: &str = "10";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum RiskSettingsScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetRiskSettingsRequest {
    pub scope: RiskSettingsScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiInstrumentCap {
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "15")]
    Fifteen,
    #[serde(rename = "20")]
    Twenty,
    #[serde(rename = "25")]
    TwentyFive,
    #[serde(rename = "30")]
    Thirty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiSectorCap {
    #[serde(rename = "30")]
    Thirty,
    #[serde(rename = "40")]
    Forty,
    #[serde(rename = "50")]
    Fifty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiExcludableType {
    #[serde(rename = "etf")]
    Etf,
    #[serde(rename = "etn")]
    Etn,
    #[serde(rename = "preferred")]
    Preferred,
    #[serde(rename = "spac")]
    Spac,
    #[serde(rename = "reit")]
    Reit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiCardExpiry {
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "15")]
    Fifteen,
    #[serde(rename = "30")]
    Thirty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiOrderCountLimit {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "10")]
    Ten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiProfileHorizon {
    #[serde(rename = "hours")]
    Hours,
    #[serde(rename = "few-days")]
    FewDays,
    #[serde(rename = "weeks-to-months")]
    WeeksToMonths,
    #[serde(rename = "half-year-or-more")]
    HalfYearOrMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiProfileRiskAppetite {
    #[serde(rename = "conservative")]
    Conservative,
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "aggressive")]
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiSwitch {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "on")]
    On,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiProfileDraft {
    pub horizon: UiProfileHorizon,
    pub risk_appetite: UiProfileRiskAppetite,
    pub instrument_cap: UiInstrumentCap,
    pub sector_cap: UiSectorCap,
    pub excluded_types: Vec<UiExcludableType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiOrderSettingsDraft {
    pub market_orders: UiSwitch,
    pub hours_horizon: UiSwitch,
    pub card_expiry: UiCardExpiry,
    pub order_count_limit: UiOrderCountLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiRiskSettingsView {
    pub profile: UiProfileDraft,
    pub order: UiOrderSettingsDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SaveInvestorProfileRequest {
    pub draft: UiProfileDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SaveOrderSettingsRequest {
    pub draft: UiOrderSettingsDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiRiskSaveKind {
    #[serde(rename = "saved")]
    Saved,
    #[serde(rename = "unchanged")]
    Unchanged,
    #[serde(rename = "declined")]
    Declined,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiRiskRejection {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "invalid-profile")]
    InvalidProfile,
    #[serde(rename = "unsupported-exclusion")]
    UnsupportedExclusion,
    #[serde(rename = "unsupported-order-setting")]
    UnsupportedOrderSetting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiRiskSaveOutcome {
    pub kind: UiRiskSaveKind,
    pub rejection: UiRiskRejection,
}

const fn saved_outcome(kind: UiRiskSaveKind) -> UiRiskSaveOutcome {
    UiRiskSaveOutcome {
        kind,
        rejection: UiRiskRejection::None,
    }
}

const fn rejected(rejection: UiRiskRejection) -> UiRiskSaveOutcome {
    UiRiskSaveOutcome {
        kind: UiRiskSaveKind::Rejected,
        rejection,
    }
}

const fn map_outcome(outcome: &RiskSettingsChangeOutcome) -> UiRiskSaveOutcome {
    match outcome {
        RiskSettingsChangeOutcome::Changed(_) => saved_outcome(UiRiskSaveKind::Saved),
        RiskSettingsChangeOutcome::Unchanged => saved_outcome(UiRiskSaveKind::Unchanged),
        RiskSettingsChangeOutcome::Denied => saved_outcome(UiRiskSaveKind::Declined),
        RiskSettingsChangeOutcome::Unavailable => saved_outcome(UiRiskSaveKind::Unavailable),
        RiskSettingsChangeOutcome::ProfileRejected(_) => rejected(UiRiskRejection::InvalidProfile),
    }
}

const fn horizon_selection(horizon: UiProfileHorizon) -> HorizonSelection {
    match horizon {
        UiProfileHorizon::Hours => HorizonSelection::Hours,
        UiProfileHorizon::FewDays => HorizonSelection::FewDays,
        UiProfileHorizon::WeeksToMonths => HorizonSelection::WeeksToMonths,
        UiProfileHorizon::HalfYearOrMore => HorizonSelection::HalfYearOrMore,
    }
}

const fn ui_horizon(selection: HorizonSelection) -> UiProfileHorizon {
    match selection {
        HorizonSelection::Hours => UiProfileHorizon::Hours,
        HorizonSelection::FewDays => UiProfileHorizon::FewDays,
        HorizonSelection::WeeksToMonths => UiProfileHorizon::WeeksToMonths,
        HorizonSelection::HalfYearOrMore => UiProfileHorizon::HalfYearOrMore,
    }
}

const fn appetite(value: UiProfileRiskAppetite) -> RiskAppetite {
    match value {
        UiProfileRiskAppetite::Conservative => RiskAppetite::Conservative,
        UiProfileRiskAppetite::Neutral => RiskAppetite::Neutral,
        UiProfileRiskAppetite::Aggressive => RiskAppetite::Aggressive,
    }
}

const fn ui_appetite(value: RiskAppetite) -> UiProfileRiskAppetite {
    match value {
        RiskAppetite::Conservative => UiProfileRiskAppetite::Conservative,
        RiskAppetite::Neutral => UiProfileRiskAppetite::Neutral,
        RiskAppetite::Aggressive => UiProfileRiskAppetite::Aggressive,
    }
}

const fn instrument_cap_text(cap: UiInstrumentCap) -> &'static str {
    match cap {
        UiInstrumentCap::Ten => "10",
        UiInstrumentCap::Fifteen => "15",
        UiInstrumentCap::Twenty => "20",
        UiInstrumentCap::TwentyFive => "25",
        UiInstrumentCap::Thirty => "30",
    }
}

const fn sector_cap_text(cap: UiSectorCap) -> &'static str {
    match cap {
        UiSectorCap::Thirty => "30",
        UiSectorCap::Forty => "40",
        UiSectorCap::Fifty => "50",
    }
}

fn percent_weight(text: &str) -> Option<ProfileWeight> {
    let input = NumericInput::try_new(text).ok()?;
    ProfileWeight::parse_percent(&input).ok()
}

fn matches_percent(ratio: Ratio, text: &str) -> bool {
    percent_weight(text).is_some_and(|weight| weight.as_ratio() == ratio)
}

fn ui_instrument_cap(ratio: Ratio) -> UiInstrumentCap {
    [
        UiInstrumentCap::Ten,
        UiInstrumentCap::Fifteen,
        UiInstrumentCap::Twenty,
        UiInstrumentCap::TwentyFive,
        UiInstrumentCap::Thirty,
    ]
    .into_iter()
    .find(|cap| matches_percent(ratio, instrument_cap_text(*cap)))
    .unwrap_or(UiInstrumentCap::Twenty)
}

fn ui_sector_cap(ratio: Ratio) -> UiSectorCap {
    [UiSectorCap::Thirty, UiSectorCap::Forty, UiSectorCap::Fifty]
        .into_iter()
        .find(|cap| matches_percent(ratio, sector_cap_text(*cap)))
        .unwrap_or(UiSectorCap::Forty)
}

fn security_types(types: &[UiExcludableType]) -> Option<Vec<ExcludedSecurityType>> {
    let mut mapped = Vec::new();
    for kind in types {
        let next: &[ExcludedSecurityType] = match kind {
            UiExcludableType::Etf => &[ExcludedSecurityType::Etf, ExcludedSecurityType::ForeignEtf],
            UiExcludableType::Etn => &[ExcludedSecurityType::Etn],
            UiExcludableType::Reit => &[ExcludedSecurityType::Reit],
            UiExcludableType::Preferred | UiExcludableType::Spac => return None,
        };
        for value in next {
            if !mapped.contains(value) {
                mapped.push(*value);
            }
        }
    }
    Some(mapped)
}

fn ui_excluded(types: &[ExcludedSecurityType]) -> Vec<UiExcludableType> {
    let mut mapped = Vec::new();
    for kind in types {
        let next = match kind {
            ExcludedSecurityType::Etf | ExcludedSecurityType::ForeignEtf => UiExcludableType::Etf,
            ExcludedSecurityType::Etn => UiExcludableType::Etn,
            ExcludedSecurityType::Reit => UiExcludableType::Reit,
            ExcludedSecurityType::Stock
            | ExcludedSecurityType::ForeignStock
            | ExcludedSecurityType::DepositaryReceipt
            | ExcludedSecurityType::InfrastructureFund
            | ExcludedSecurityType::StockWarrants => continue,
        };
        if !mapped.contains(&next) {
            mapped.push(next);
        }
    }
    mapped
}

const fn market_switch(setting: SavedMarketOrderSetting) -> UiSwitch {
    match setting {
        SavedMarketOrderSetting::Enabled => UiSwitch::On,
        SavedMarketOrderSetting::Disabled => UiSwitch::Off,
    }
}

const fn hours_switch(state: HoursPermissionState) -> UiSwitch {
    match state {
        HoursPermissionState::Enabled => UiSwitch::On,
        HoursPermissionState::Disabled => UiSwitch::Off,
    }
}

fn current_risk(loaded: &LoadedSettings, profile: &InvestorProfile) -> RiskSettings {
    loaded.risk_settings.unwrap_or_else(|| {
        RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            profile.hours_permission().state(),
            profile.weight_caps(),
            profile.risk_appetite(),
        )
    })
}

#[must_use]
pub fn to_risk_view(loaded: &LoadedSettings) -> UiRiskSettingsView {
    let fallback = InvestorProfile::default();
    let profile = loaded.profile.as_ref().unwrap_or(&fallback);
    let risk = current_risk(loaded, profile);
    let caps = profile.weight_caps();
    UiRiskSettingsView {
        profile: UiProfileDraft {
            horizon: ui_horizon(profile.horizon().selection()),
            risk_appetite: ui_appetite(profile.risk_appetite()),
            instrument_cap: ui_instrument_cap(caps.instrument_cap().as_ratio()),
            sector_cap: ui_sector_cap(caps.sector_cap().as_ratio()),
            excluded_types: ui_excluded(profile.exclusions().security_types()),
        },
        order: UiOrderSettingsDraft {
            market_orders: market_switch(risk.market_orders()),
            hours_horizon: hours_switch(risk.hours()),
            card_expiry: UiCardExpiry::Ten,
            order_count_limit: UiOrderCountLimit::Off,
        },
    }
}

fn profile_request(
    draft: &UiProfileDraft,
    current: &InvestorProfile,
) -> Result<InvestorProfileRequest, UiRiskRejection> {
    let types =
        security_types(&draft.excluded_types).ok_or(UiRiskRejection::UnsupportedExclusion)?;
    let exclusions = Exclusions::try_new(current.exclusions().sectors().to_vec(), types)
        .map_err(|_error| UiRiskRejection::InvalidProfile)?;
    let cap = percent_weight(instrument_cap_text(draft.instrument_cap))
        .ok_or(UiRiskRejection::InvalidProfile)?;
    let warning =
        percent_weight(INSTRUMENT_WARNING_PERCENT).ok_or(UiRiskRejection::InvalidProfile)?;
    let sector =
        percent_weight(sector_cap_text(draft.sector_cap)).ok_or(UiRiskRejection::InvalidProfile)?;
    let weight_caps = WeightCaps::try_new(cap, warning, sector)
        .map_err(|_error| UiRiskRejection::InvalidProfile)?;
    Ok(InvestorProfileRequest {
        horizon: horizon_selection(draft.horizon),
        risk_appetite: appetite(draft.risk_appetite),
        exclusions,
        weight_caps,
    })
}

fn map_error(error: &application::SystemPortError) -> IpcError {
    IpcError::from(error.class())
}

pub async fn load_risk_view(state: &ScreenState) -> Result<UiRiskSettingsView, IpcError> {
    let loaded = state
        .backend()
        .load_settings()
        .await
        .map_err(|error| map_error(&error))?;
    Ok(to_risk_view(&loaded))
}

pub async fn save_profile(
    state: &ScreenState,
    draft: &UiProfileDraft,
) -> Result<UiRiskSaveOutcome, IpcError> {
    let loaded = state
        .backend()
        .load_settings()
        .await
        .map_err(|error| map_error(&error))?;
    let fallback = InvestorProfile::default();
    let current = loaded.profile.as_ref().unwrap_or(&fallback);
    let request = match profile_request(draft, current) {
        Ok(request) => request,
        Err(rejection) => return Ok(rejected(rejection)),
    };
    let outcome = state
        .backend()
        .update_profile(request)
        .await
        .map_err(|error| map_error(&error))?;
    Ok(map_outcome(&outcome))
}

pub async fn save_order(
    state: &ScreenState,
    draft: &UiOrderSettingsDraft,
) -> Result<UiRiskSaveOutcome, IpcError> {
    if draft.card_expiry != UiCardExpiry::Ten || draft.order_count_limit != UiOrderCountLimit::Off {
        return Ok(rejected(UiRiskRejection::UnsupportedOrderSetting));
    }
    let loaded = state
        .backend()
        .load_settings()
        .await
        .map_err(|error| map_error(&error))?;
    let fallback = InvestorProfile::default();
    let profile = loaded.profile.as_ref().unwrap_or(&fallback);
    let current = current_risk(&loaded, profile);
    let market = match draft.market_orders {
        UiSwitch::On => SavedMarketOrderSetting::Enabled,
        UiSwitch::Off => SavedMarketOrderSetting::Disabled,
    };
    let hours = match draft.hours_horizon {
        UiSwitch::On => HoursPermissionState::Enabled,
        UiSwitch::Off => HoursPermissionState::Disabled,
    };
    let requested = RiskSettings::new(
        current.orders(),
        market,
        hours,
        current.weight_caps(),
        current.risk_appetite(),
    );
    let outcome = state
        .backend()
        .change_risk_settings(requested)
        .await
        .map_err(|error| map_error(&error))?;
    Ok(map_outcome(&outcome))
}

fn screen_state<R: Runtime>(app: &AppHandle<R>) -> Result<&ScreenState, IpcError> {
    app.try_state::<ScreenState>()
        .map(|state| state.inner())
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })
}

#[tauri::command]
#[specta::specta]
pub async fn get_risk_settings<R: Runtime>(
    app: AppHandle<R>,
    request: GetRiskSettingsRequest,
) -> Result<UiRiskSettingsView, IpcError> {
    let GetRiskSettingsRequest {
        scope: RiskSettingsScope::Current,
    } = request;
    load_risk_view(screen_state(&app)?).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_investor_profile<R: Runtime>(
    app: AppHandle<R>,
    request: SaveInvestorProfileRequest,
) -> Result<UiRiskSaveOutcome, IpcError> {
    save_profile(screen_state(&app)?, &request.draft).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_order_settings<R: Runtime>(
    app: AppHandle<R>,
    request: SaveOrderSettingsRequest,
) -> Result<UiRiskSaveOutcome, IpcError> {
    save_order(screen_state(&app)?, &request.draft).await
}

#[cfg(test)]
mod tests;
