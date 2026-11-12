pub mod bindings;
pub mod ipc;

use tauri::{App, Builder, Context, Runtime};

pub fn build_app<R: Runtime>(
    builder: Builder<R>,
    context: Context<R>,
) -> Result<App<R>, tauri::Error> {
    builder
        .invoke_handler(ipc::registry::specta_builder::<R>().invoke_handler())
        .build(context)
}

#[cfg(test)]
mod tests;
