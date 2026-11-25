pub mod ai_runtime;
pub mod audit_queue;
pub mod bindings;
pub mod database;
pub mod exit;
pub mod ipc;
pub mod lifecycle;
pub mod mcp_catalog;
pub mod mcp_sources;
pub mod platform;
pub mod ports;
pub mod product_events;
pub mod product_ports;
mod security;
pub mod sqlite_ports;
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
            let _policy = platform::residency::apply_accessory_policy(app);
            app.manage(platform::residency::ResidencyState::new());
            let opened = open_product_database(app, &ports);
            let status = opened.status();
            let (shared_database, database) = share_database(app, opened);
            let refresh = tauri::async_runtime::block_on(startup::start(&ports, |data| {
                publish_main(app.handle(), data)
            }))?;
            app.manage(ipc::state::ScreenState::from_ports(&ports));
            let chat_state = ipc::chat_hub::ChatIpcState::from_ports(&ports);
            app.manage(ports);
            let _scheduling = platform::scheduling::start_background::<R, P>(app);
            let shutdown_hook = database_shutdown_hook(app, shared_database);
            app.manage(status);
            app.manage(chat_state);
            app.manage(Lifecycle::with_shutdown_hook(
                refresh,
                database,
                shutdown_hook,
            ));
            ensure_default_runtimes(app);
            Ok(())
        })
        .build(context)
}

fn database_path<R: Runtime>(
    app: &App<R>,
) -> Result<store_sqlite::DatabasePath, application::SystemPortError> {
    app.path()
        .app_data_dir()
        .map(|directory| store_sqlite::DatabasePath::new(directory.join("stock-copilot.db")))
        .map_err(|_error| {
            application::SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
            )
        })
}

fn open_product_database<R: Runtime, P: PortTypes>(
    app: &App<R>,
    ports: &Ports<P>,
) -> database::DatabaseState {
    let factory = Arc::clone(&ports.database);
    match (
        app.try_state::<database::DatabaseKeyProvision>(),
        database_path(app),
    ) {
        (Some(provision), Ok(resolved)) => {
            tauri::async_runtime::block_on(database::open_provisioned_database(
                ports.secrets.as_ref(),
                provision.inner(),
                factory,
                resolved,
            ))
        }
        (None, resolved) | (Some(_), resolved @ Err(_)) => tauri::async_runtime::block_on(
            database::open_database(ports.secrets.as_ref(), factory, move || resolved),
        ),
    }
}

fn share_database<R: Runtime>(
    app: &App<R>,
    database: database::DatabaseState,
) -> (
    Option<store_sqlite::SharedDatabase>,
    database::DatabaseState,
) {
    let shared = app
        .try_state::<Arc<store_sqlite::SqliteRepositories>>()
        .map(|state| state.shared().clone());
    let remaining = match (shared.as_ref(), database) {
        (Some(target), database::DatabaseState::Ready(opened)) => {
            let _previous = tauri::async_runtime::block_on(target.install(opened));
            database::DatabaseState::NotConfigured
        }
        (_, other) => other,
    };
    (shared, remaining)
}

fn database_shutdown_hook<R: Runtime>(
    app: &App<R>,
    shared: Option<store_sqlite::SharedDatabase>,
) -> Arc<dyn platform::process_cleanup::ShutdownHook> {
    let inner = prepare_process_shutdown_hook(app);
    match shared {
        Some(database) => Arc::new(sqlite_ports::SharedDatabaseShutdownHook::new(
            inner, database,
        )),
        None => inner,
    }
}

fn prepare_process_shutdown_hook<R: Runtime>(
    app: &App<R>,
) -> Arc<dyn platform::process_cleanup::ShutdownHook> {
    if app
        .try_state::<platform::process_cleanup::ProcessRuntime>()
        .is_none()
    {
        app.manage(platform::process_cleanup::ProcessRuntime::default_in_memory());
    }
    let runtime = app.state::<platform::process_cleanup::ProcessRuntime>();
    let _startup_cleanup = tauri::async_runtime::block_on(runtime.cleanup_orphans_on_startup());
    let background = app
        .try_state::<Arc<platform::background::BackgroundRuntime>>()
        .map(|state| Arc::clone(&state));
    Arc::new(platform::background::BackgroundAwareShutdownHook::new(
        background,
        runtime.shutdown_hook(),
    ))
}

fn ensure_default_runtimes<R: Runtime>(app: &App<R>) {
    if let Some(slot) = app.try_state::<Arc<product_events::UiEventSlot<R>>>() {
        let _binding = slot.bind(ipc::event_bridge::UiEventBridge::new(app.handle().clone()));
    }
    if app
        .try_state::<ipc::open_external::ExternalLinkRuntime>()
        .is_none()
    {
        app.manage(ipc::open_external::ExternalLinkRuntime::unavailable());
    }
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
    window.show()?;
    platform::residency::record_main_shown(app);
    Ok(())
}

pub fn run_app<R: Runtime, Finish>(app: App<R>, finish: Finish)
where
    Finish: Fn(&tauri::AppHandle<R>, exit::ExitCode) + Send + Sync + 'static,
{
    let finalizer = Arc::new(finish);
    app.run(move |handle, event| match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            let _action = platform::residency::handle_close_requested(handle, &label, &api);
        }
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            exit::request(
                handle,
                &api,
                exit::ExitCode::from_request(code),
                Arc::clone(&finalizer),
            );
        }
        tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            let visibility = if has_visible_windows {
                platform::residency::VisibleWindowPresence::SomeVisible
            } else {
                platform::residency::VisibleWindowPresence::NoneVisible
            };
            if let Err(_error) = platform::residency::handle_reopen(handle, visibility) {}
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Ready
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | _ => {}
    });
}

#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod tests;
