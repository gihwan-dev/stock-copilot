pub mod bindings;
pub mod database;
pub mod exit;
pub mod ipc;
pub mod lifecycle;
pub mod ports;
pub mod product_ports;
mod security;
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
        window.create = false;
    }
    builder
        .invoke_handler(ipc::registry::specta_builder::<R>().invoke_handler())
        .setup(move |app| {
            ipc::registry::specta_builder::<R>().mount_events(app);
            security::create_windows(app)?;
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
                publish_main(app.handle(), data)
            }))?;
            app.manage(database.status());
            app.manage(Lifecycle::new(refresh, database));
            app.manage(ports);
            Ok(())
        })
        .build(context)
}

fn publish_main<R: Runtime>(
    app: &tauri::AppHandle<R>,
    data: startup::MainScreenData,
) -> Result<(), tauri::Error> {
    if !app.manage(data) {
        return Err(tauri::Error::AssetNotFound(
            "duplicate-main-screen-data".into(),
        ));
    }
    show_prepared_main(app)
}

fn show_prepared_main<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), tauri::Error> {
    if app.try_state::<startup::MainScreenData>().is_none() {
        return Err(tauri::Error::AssetNotFound("main-screen-data".into()));
    }
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| tauri::Error::AssetNotFound("main-window".into()))?;
    window.show()
}

pub fn run_app<R: Runtime, Finish>(app: App<R>, finish: Finish)
where
    Finish: Fn(&tauri::AppHandle<R>, exit::ExitCode) + Send + Sync + 'static,
{
    let finalizer = Arc::new(finish);
    app.run(move |handle, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            exit::request(
                handle,
                &api,
                exit::ExitCode::from_request(code),
                Arc::clone(&finalizer),
            );
        }
    });
}

#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod tests;
