pub mod bindings;
pub mod database;
pub mod ipc;
pub mod lifecycle;
pub mod ports;
pub mod product_ports;
pub mod startup;
pub mod unavailable;

use lifecycle::Lifecycle;
use ports::{PortTypes, Ports};
use std::sync::Arc;
use tauri::{App, Builder, Context, Manager, Runtime};

pub fn build_app<R: Runtime, P: PortTypes>(
    builder: Builder<R>,
    ports: Ports<P>,
    mut context: Context<R>,
) -> Result<App<R>, tauri::Error> {
    for window in &mut context.config_mut().app.windows {
        window.visible = false;
    }
    builder
        .invoke_handler(ipc::registry::specta_builder::<R>().invoke_handler())
        .setup(move |app| {
            let database = tauri::async_runtime::block_on(database::open_database(
                ports.secrets.as_ref(),
                Arc::clone(&ports.database),
                || {
                    app.path()
                        .app_data_dir()
                        .map(|directory| {
                            store_sqlite::DatabasePath::new(directory.join("stock-copilot.db"))
                        })
                        .map_err(|_error| {
                            application::SystemPortError::new(
                                application::SystemPort::Repository,
                                application::SystemFailure::Unavailable(
                                    application::RetryDelay::Unknown,
                                ),
                            )
                        })
                },
            ));
            let refresh = tauri::async_runtime::block_on(startup::start(&ports, |data| {
                app.manage(data);
                if let Some(window) = app.get_webview_window("main") {
                    window.show()?;
                }
                Ok(())
            }))?;
            app.manage(database.status());
            app.manage(Lifecycle::new(refresh, database));
            app.manage(ports);
            Ok(())
        })
        .build(context)
}

pub fn run_app<R: Runtime>(app: App<R>) {
    app.run(|handle, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            request_exit(handle, &api, code);
        }
    });
}

fn request_exit<R: Runtime>(
    handle: &tauri::AppHandle<R>,
    api: &tauri::ExitRequestApi,
    code: Option<i32>,
) {
    let lifecycle = handle.state::<Lifecycle>();
    match lifecycle.outcome().try_lock() {
        Ok(outcome) if outcome.is_some() => return,
        Ok(_outcome) => {}
        Err(_error) => {
            api.prevent_exit();
            return;
        }
    }
    api.prevent_exit();
    let exiting = handle.clone();
    tauri::async_runtime::spawn(async move {
        exiting.state::<Lifecycle>().shutdown().await;
        exiting.exit(code.unwrap_or(0_i32));
    });
}

#[cfg(test)]
mod tests;
