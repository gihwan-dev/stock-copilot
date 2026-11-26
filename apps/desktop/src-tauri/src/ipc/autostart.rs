use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::settings::UiToggleState;
use crate::platform::autostart::{AutostartPort, AutostartState, LoginItems, LoginItemsState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum UiAutoStartChoice {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetAutoStartRequest {
    pub state: UiAutoStartChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAutoStartView {
    pub auto_start_at_login: UiToggleState,
}

#[must_use]
pub const fn toggle(state: AutostartState) -> UiToggleState {
    match state {
        AutostartState::Enabled => UiToggleState::On,
        AutostartState::Disabled => UiToggleState::Off,
    }
}

pub fn change_auto_start<P: AutostartPort>(
    items: &crate::platform::autostart::AutostartController<P>,
    events: &application::EventBus,
    choice: UiAutoStartChoice,
) -> Result<UiAutoStartView, IpcError> {
    let next = match choice {
        UiAutoStartChoice::On => AutostartState::Enabled,
        UiAutoStartChoice::Off => AutostartState::Disabled,
    };
    items
        .apply(next, events)
        .map(|report| UiAutoStartView {
            auto_start_at_login: toggle(report.state),
        })
        .map_err(|_error| IpcError {
            kind: IpcErrorKind::Retryable,
        })
}

fn managed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<
    (
        std::sync::Arc<LoginItems>,
        std::sync::Arc<application::EventBus>,
    ),
    IpcError,
> {
    app.try_state::<LoginItemsState>()
        .map(|state| {
            (
                std::sync::Arc::clone(&state.items),
                std::sync::Arc::clone(&state.events),
            )
        })
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })
}

#[tauri::command]
#[specta::specta]
pub async fn set_auto_start<R: Runtime>(
    app: AppHandle<R>,
    request: SetAutoStartRequest,
) -> Result<UiAutoStartView, IpcError> {
    let (items, events) = managed(&app)?;
    change_auto_start(items.as_ref(), events.as_ref(), request.state)
}

#[cfg(test)]
mod tests;
