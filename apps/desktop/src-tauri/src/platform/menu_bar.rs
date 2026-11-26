use std::sync::Arc;

use scheduler::{JobTriggers, ScheduledJobId};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Runtime};

use crate::ipc::event::UiEvent;
use crate::ipc::event_bridge::UiEventBridge;
use crate::platform::residency::{begin_quit, show_and_focus_main};
use crate::platform::tray::{
    TrayIconState, TrayMenuAction, action_code, action_of, residency_tray_menu, template_icon_rgba,
};

pub const TRAY_ID: &str = "main";

fn build<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let mut items = Vec::new();
    for item in residency_tray_menu() {
        items.push(MenuItem::with_id(
            app,
            action_code(item.action),
            item.label,
            true,
            None::<&str>,
        )?);
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = items
        .iter()
        .map(|item| {
            let shared: &dyn tauri::menu::IsMenuItem<R> = item;
            shared
        })
        .collect();
    let menu = Menu::with_items(app, &refs)?;
    let (rgba, width, height) = template_icon_rgba();
    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::new_owned(rgba, width, height))
        .icon_as_template(true)
        .tooltip("Stock Copilot")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|handle, event| {
            if let Some(action) = action_of(event.id().as_ref()) {
                let _handled = run_action(handle, action);
            }
        })
        .build(app)?;
    Ok(())
}

#[must_use]
pub fn install<R: Runtime>(app: &App<R>) -> TrayIconState {
    match build(app) {
        Ok(()) => TrayIconState::Visible,
        Err(_error) => TrayIconState::Hidden,
    }
}

pub fn sync_now<R: Runtime>(app: &AppHandle<R>) {
    if let Some(triggers) = app.try_state::<Arc<JobTriggers>>() {
        triggers.trigger(ScheduledJobId::PortfolioSync);
        triggers.trigger(ScheduledJobId::TransactionsSync);
    }
}

pub fn run_action<R: Runtime>(
    app: &AppHandle<R>,
    action: TrayMenuAction,
) -> Result<(), tauri::Error> {
    match action {
        TrayMenuAction::OpenMain | TrayMenuAction::OpenSettings => show_and_focus_main(app),
        TrayMenuAction::SyncNow => {
            sync_now(app);
            Ok(())
        }
        TrayMenuAction::DataStatus => {
            show_and_focus_main(app)?;
            let _sent = UiEventBridge::new(app.clone()).emit_ui_event(&UiEvent::ShowDataStatus);
            Ok(())
        }
        TrayMenuAction::Quit => {
            begin_quit(app);
            app.exit(0);
            Ok(())
        }
    }
}
