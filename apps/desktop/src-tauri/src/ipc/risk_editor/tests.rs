use std::sync::Arc;

use application::RiskChangeConfirmation;
use testkit_ports::memory::MemoryReply;

use super::{
    GetRiskSettingsRequest, SaveInvestorProfileRequest, UiCardExpiry, UiExcludableType,
    UiInstrumentCap, UiOrderCountLimit, UiOrderSettingsDraft, UiProfileDraft, UiProfileHorizon,
    UiProfileRiskAppetite, UiRiskRejection, UiRiskSaveKind, UiSectorCap, UiSwitch, load_risk_view,
    save_order, save_profile,
};
use crate::ipc::state::ScreenState;
use crate::ports::Ports;
use crate::tests::support::TestPorts;

fn with_presence(confirmations: Vec<RiskChangeConfirmation>) -> Ports<TestPorts> {
    let base = crate::tests::support::ports();
    let ports = Ports {
        presence: Arc::new(testkit_ports::ScriptedPresence::new([], confirmations)),
        ..base
    };
    for _ in 0_u8..12_u8 {
        ports
            .secrets
            .script_load()
            .push(MemoryReply::Return(Ok(None)));
        ports.settings.script_profile().push(MemoryReply::Memory);
        ports
            .settings
            .script_risk_settings()
            .push(MemoryReply::Memory);
        ports
            .settings
            .script_store_profile()
            .push(MemoryReply::Memory);
        ports
            .settings
            .script_store_risk_settings()
            .push(MemoryReply::Memory);
    }
    ports
}

fn draft(appetite: UiProfileRiskAppetite, types: Vec<UiExcludableType>) -> UiProfileDraft {
    UiProfileDraft {
        horizon: UiProfileHorizon::WeeksToMonths,
        risk_appetite: appetite,
        instrument_cap: UiInstrumentCap::Twenty,
        sector_cap: UiSectorCap::Forty,
        excluded_types: types,
    }
}

#[test]
fn default_view_matches_the_documented_defaults_and_snapshot() {
    let ports = with_presence(Vec::new());
    let state = ScreenState::from_ports(&ports);
    let view = tauri::async_runtime::block_on(load_risk_view(&state)).expect("기본값");
    assert_eq!(
        view.profile,
        draft(UiProfileRiskAppetite::Neutral, Vec::new())
    );
    assert_eq!(view.order.market_orders, UiSwitch::Off);
    assert_eq!(view.order.hours_horizon, UiSwitch::Off);
    assert_eq!(view.order.card_expiry, UiCardExpiry::Ten);
    assert_eq!(view.order.order_count_limit, UiOrderCountLimit::Off);
    super::super::tests::assert_response_snapshot("get_risk_settings", &view);
}

#[test]
fn more_aggressive_profile_needs_confirmation_and_declined_is_not_saved() {
    let ports = with_presence(vec![RiskChangeConfirmation::Denied]);
    let state = ScreenState::from_ports(&ports);
    let outcome = tauri::async_runtime::block_on(save_profile(
        &state,
        &draft(UiProfileRiskAppetite::Aggressive, Vec::new()),
    ))
    .expect("응답");
    assert_eq!(outcome.kind, UiRiskSaveKind::Declined);
    let view = tauri::async_runtime::block_on(load_risk_view(&state)).expect("다시 읽기");
    assert_eq!(view.profile.risk_appetite, UiProfileRiskAppetite::Neutral);
    super::super::tests::assert_response_snapshot("save_investor_profile", &outcome);
}

#[test]
fn safer_profile_is_saved_without_a_dialog_and_read_back() {
    let ports = with_presence(Vec::new());
    let state = ScreenState::from_ports(&ports);
    let outcome = tauri::async_runtime::block_on(save_profile(
        &state,
        &draft(
            UiProfileRiskAppetite::Conservative,
            vec![UiExcludableType::Etf, UiExcludableType::Reit],
        ),
    ))
    .expect("응답");
    assert_eq!(outcome.kind, UiRiskSaveKind::Saved);
    let view = tauri::async_runtime::block_on(load_risk_view(&state)).expect("다시 읽기");
    assert_eq!(
        view.profile.risk_appetite,
        UiProfileRiskAppetite::Conservative
    );
    assert_eq!(
        view.profile.excluded_types,
        vec![UiExcludableType::Etf, UiExcludableType::Reit]
    );
}

#[test]
fn unsupported_exclusions_and_order_values_are_rejected_before_any_write() {
    let ports = with_presence(Vec::new());
    let state = ScreenState::from_ports(&ports);
    let outcome = tauri::async_runtime::block_on(save_profile(
        &state,
        &draft(UiProfileRiskAppetite::Neutral, vec![UiExcludableType::Spac]),
    ))
    .expect("응답");
    assert_eq!(outcome.kind, UiRiskSaveKind::Rejected);
    assert_eq!(outcome.rejection, UiRiskRejection::UnsupportedExclusion);
    let order = tauri::async_runtime::block_on(save_order(
        &state,
        &UiOrderSettingsDraft {
            market_orders: UiSwitch::Off,
            hours_horizon: UiSwitch::Off,
            card_expiry: UiCardExpiry::Thirty,
            order_count_limit: UiOrderCountLimit::Off,
        },
    ))
    .expect("응답");
    assert_eq!(order.rejection, UiRiskRejection::UnsupportedOrderSetting);
    for call in ports.settings.calls() {
        match call {
            testkit_ports::scripted::SettingsRepositoryCall::StoreProfile(_)
            | testkit_ports::scripted::SettingsRepositoryCall::StoreRiskSettings(_) => {
                panic!("거부는 저장하지 않는다")
            }
            testkit_ports::scripted::SettingsRepositoryCall::Profile()
            | testkit_ports::scripted::SettingsRepositoryCall::RiskSettings() => {}
        }
    }
}

#[test]
fn enabling_market_orders_requires_confirmation_and_unavailable_dialog_keeps_it_off() {
    let ports = with_presence(vec![RiskChangeConfirmation::Unavailable]);
    let state = ScreenState::from_ports(&ports);
    let outcome = tauri::async_runtime::block_on(save_order(
        &state,
        &UiOrderSettingsDraft {
            market_orders: UiSwitch::On,
            hours_horizon: UiSwitch::Off,
            card_expiry: UiCardExpiry::Ten,
            order_count_limit: UiOrderCountLimit::Off,
        },
    ))
    .expect("응답");
    assert_eq!(outcome.kind, UiRiskSaveKind::Unavailable);
    let view = tauri::async_runtime::block_on(load_risk_view(&state)).expect("다시 읽기");
    assert_eq!(view.order.market_orders, UiSwitch::Off);
    super::super::tests::assert_response_snapshot("save_order_settings", &outcome);
}

#[test]
fn requests_refuse_unknown_fields() {
    let _parsed: GetRiskSettingsRequest =
        serde_json::from_str(r#"{"scope":"current"}"#).expect("요청");
    let _extra = serde_json::from_str::<GetRiskSettingsRequest>(r#"{"scope":"current","x":1}"#)
        .expect_err("모르는 필드 거부");
    let _bypass = serde_json::from_str::<SaveInvestorProfileRequest>(
        r#"{"draft":{"horizon":"few-days","risk_appetite":"neutral","instrument_cap":"20","sector_cap":"40","excluded_types":[],"bypass":true}}"#,
    )
    .expect_err("우회 필드 거부");
}
