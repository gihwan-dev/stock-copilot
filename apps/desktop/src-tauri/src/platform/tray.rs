#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    OpenMain,
    OpenSettings,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconState {
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayMenuItem {
    pub action: TrayMenuAction,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayError {
    Unavailable,
}

#[must_use]
pub const fn default_tray_menu() -> [TrayMenuItem; 3] {
    [
        TrayMenuItem {
            action: TrayMenuAction::OpenMain,
            label: "열기",
        },
        TrayMenuItem {
            action: TrayMenuAction::OpenSettings,
            label: "설정",
        },
        TrayMenuItem {
            action: TrayMenuAction::Quit,
            label: "종료",
        },
    ]
}

pub trait TrayController: Send + Sync {
    fn install(&self, items: &[TrayMenuItem]) -> Result<TrayIconState, TrayError>;
    fn state(&self) -> TrayIconState;
}

#[derive(Debug, Default)]
pub struct UnavailableTrayController;

impl TrayController for UnavailableTrayController {
    fn install(&self, _items: &[TrayMenuItem]) -> Result<TrayIconState, TrayError> {
        Err(TrayError::Unavailable)
    }

    fn state(&self) -> TrayIconState {
        TrayIconState::Hidden
    }
}
