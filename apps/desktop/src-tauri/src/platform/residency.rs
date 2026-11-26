use crate::lifecycle::{Lifecycle, LifecyclePhase};
use crate::platform::tray::{TrayIconState, TrayMenuAction};
use std::sync::RwLock;
use tauri::{ActivationPolicy, App, AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCloseAction {
    HideAndKeepRunning,
    AllowClose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleWindowPresence {
    NoneVisible,
    SomeVisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationPolicyOutcome {
    Applied,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainWindowVisibility {
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitMode {
    KeepRunningOnClose,
    Quitting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResidencySnapshot {
    visibility: MainWindowVisibility,
    quit_mode: QuitMode,
    close_intercepted_count: u32,
}

#[derive(Debug)]
pub struct ResidencyState {
    inner: RwLock<ResidencySnapshot>,
}

impl Default for ResidencyState {
    fn default() -> Self {
        Self::new()
    }
}

impl ResidencyState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: RwLock::new(ResidencySnapshot {
                visibility: MainWindowVisibility::Hidden,
                quit_mode: QuitMode::KeepRunningOnClose,
                close_intercepted_count: 0,
            }),
        }
    }

    pub fn record_shown(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.visibility = MainWindowVisibility::Visible;
        }
    }

    pub fn record_hidden_on_close(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.visibility = MainWindowVisibility::Hidden;
            guard.close_intercepted_count = guard.close_intercepted_count.saturating_add(1);
        }
    }

    pub fn begin_quit(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.quit_mode = QuitMode::Quitting;
        }
    }

    #[must_use]
    pub fn main_visibility(&self) -> MainWindowVisibility {
        if let Ok(guard) = self.inner.read() {
            guard.visibility
        } else {
            MainWindowVisibility::Hidden
        }
    }

    #[must_use]
    pub fn quit_mode(&self) -> QuitMode {
        if let Ok(guard) = self.inner.read() {
            guard.quit_mode
        } else {
            QuitMode::Quitting
        }
    }

    #[must_use]
    pub fn close_intercepted_count(&self) -> u32 {
        if let Ok(guard) = self.inner.read() {
            guard.close_intercepted_count
        } else {
            0
        }
    }
}

pub fn begin_quit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<ResidencyState>() {
        state.begin_quit();
    }
}

fn follow_window_in_dock<R: Runtime>(app: &AppHandle<R>, visibility: MainWindowVisibility) {
    if app
        .try_state::<ResidencyPresence>()
        .is_some_and(|presence| *presence.inner() == ResidencyPresence::MenuBarOnly)
    {
        let policy = match visibility {
            MainWindowVisibility::Visible => ActivationPolicy::Regular,
            MainWindowVisibility::Hidden => ActivationPolicy::Accessory,
        };
        if let Err(_unsupported) = app.set_activation_policy(policy) {}
    }
}

pub fn settle_menu_bar<R: Runtime>(app: &AppHandle<R>, tray: TrayIconState) {
    let _managed = app.manage(residency_presence(tray));
    let visibility = app
        .try_state::<ResidencyState>()
        .map_or(MainWindowVisibility::Hidden, |state| {
            state.main_visibility()
        });
    follow_window_in_dock(app, visibility);
}

pub fn record_main_shown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<ResidencyState>() {
        state.record_shown();
    }
    follow_window_in_dock(app, MainWindowVisibility::Visible);
}

pub fn record_main_hidden_on_close<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<ResidencyState>() {
        state.record_hidden_on_close();
    }
    follow_window_in_dock(app, MainWindowVisibility::Hidden);
}

#[must_use]
pub fn close_action<R: Runtime>(label: &str, app: &AppHandle<R>) -> WindowCloseAction {
    if label != "main" {
        return WindowCloseAction::AllowClose;
    }
    if app
        .try_state::<ResidencyState>()
        .is_some_and(|state| state.quit_mode() == QuitMode::Quitting)
    {
        return WindowCloseAction::AllowClose;
    }
    match app.try_state::<Lifecycle>() {
        Some(lifecycle) if lifecycle.phase() == LifecyclePhase::Running => {
            WindowCloseAction::HideAndKeepRunning
        }
        Some(_) | None => WindowCloseAction::AllowClose,
    }
}

pub fn handle_close_requested<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    api: &tauri::CloseRequestApi,
) -> WindowCloseAction {
    let action = close_action(label, app);
    if action == WindowCloseAction::HideAndKeepRunning {
        api.prevent_close();
        if let Some(main_window) = app.get_webview_window(label)
            && let Err(_error) = main_window.hide()
        {}
        record_main_hidden_on_close(app);
    }
    action
}

pub fn show_and_focus_main<R: Runtime>(app: &AppHandle<R>) -> Result<(), tauri::Error> {
    let main_window = app
        .get_webview_window("main")
        .ok_or_else(|| tauri::Error::AssetNotFound("main-window".into()))?;
    main_window.show()?;
    main_window.unminimize()?;
    main_window.set_focus()?;
    record_main_shown(app);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyPresence {
    DockIcon,
    MenuBarOnly,
}

#[must_use]
pub const fn residency_presence(tray: TrayIconState) -> ResidencyPresence {
    match tray {
        TrayIconState::Visible => ResidencyPresence::MenuBarOnly,
        TrayIconState::Hidden => ResidencyPresence::DockIcon,
    }
}

#[must_use]
pub fn apply_residency_policy<R: Runtime>(
    app: &mut App<R>,
    tray: TrayIconState,
) -> ActivationPolicyOutcome {
    app.set_activation_policy(match residency_presence(tray) {
        ResidencyPresence::DockIcon => ActivationPolicy::Regular,
        ResidencyPresence::MenuBarOnly => ActivationPolicy::Accessory,
    });
    ActivationPolicyOutcome::Applied
}

pub fn handle_tray_action<R: Runtime>(
    app: &AppHandle<R>,
    action: TrayMenuAction,
) -> Result<(), tauri::Error> {
    match action {
        TrayMenuAction::OpenMain | TrayMenuAction::OpenSettings => show_and_focus_main(app),
        TrayMenuAction::SyncNow | TrayMenuAction::DataStatus => {
            crate::platform::menu_bar::run_action(app, action)
        }
        TrayMenuAction::Quit => {
            begin_quit(app);
            app.exit(0);
            Ok(())
        }
    }
}

pub fn handle_reopen<R: Runtime>(
    app: &AppHandle<R>,
    visibility: VisibleWindowPresence,
) -> Result<(), tauri::Error> {
    match visibility {
        VisibleWindowPresence::NoneVisible => show_and_focus_main(app),
        VisibleWindowPresence::SomeVisible => Ok(()),
    }
}
