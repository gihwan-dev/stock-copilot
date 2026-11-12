use tauri::{App, Builder, Context, Runtime};

pub fn build_app<R: Runtime>(
    builder: Builder<R>,
    context: Context<R>,
) -> Result<App<R>, tauri::Error> {
    builder.build(context)
}

#[cfg(test)]
mod tests;
