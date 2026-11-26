#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    OpenMain,
    OpenSettings,
    SyncNow,
    DataStatus,
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

#[must_use]
pub const fn residency_tray_menu() -> [TrayMenuItem; 4] {
    [
        TrayMenuItem {
            action: TrayMenuAction::OpenMain,
            label: "열기",
        },
        TrayMenuItem {
            action: TrayMenuAction::SyncNow,
            label: "지금 동기화",
        },
        TrayMenuItem {
            action: TrayMenuAction::DataStatus,
            label: "데이터 상태",
        },
        TrayMenuItem {
            action: TrayMenuAction::Quit,
            label: "종료",
        },
    ]
}

#[must_use]
pub const fn action_code(action: TrayMenuAction) -> &'static str {
    match action {
        TrayMenuAction::OpenMain => "open",
        TrayMenuAction::OpenSettings => "settings",
        TrayMenuAction::SyncNow => "sync-now",
        TrayMenuAction::DataStatus => "data-status",
        TrayMenuAction::Quit => "quit",
    }
}

#[must_use]
pub fn action_of(code: &str) -> Option<TrayMenuAction> {
    [
        TrayMenuAction::OpenMain,
        TrayMenuAction::OpenSettings,
        TrayMenuAction::SyncNow,
        TrayMenuAction::DataStatus,
        TrayMenuAction::Quit,
    ]
    .into_iter()
    .find(|action| action_code(*action) == code)
}

const ICON_SIZE: u32 = 32;
const BARS: [(u32, u32, u32); 3] = [(5, 11, 19), (13, 19, 12), (21, 27, 4)];
const BAR_BOTTOM: u32 = 28;

#[must_use]
pub fn template_icon_rgba() -> (Vec<u8>, u32, u32) {
    let mut rgba = Vec::new();
    for row in 0..ICON_SIZE {
        for column in 0..ICON_SIZE {
            let filled = BARS.iter().any(|(left, right, top)| {
                column >= *left && column < *right && row >= *top && row < BAR_BOTTOM
            });
            rgba.extend_from_slice(if filled {
                &[0, 0, 0, 255]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    (rgba, ICON_SIZE, ICON_SIZE)
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
