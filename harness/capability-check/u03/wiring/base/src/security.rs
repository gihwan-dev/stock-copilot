use tauri::{App, Runtime, Url, WebviewWindowBuilder, webview::NewWindowResponse};

fn allows_navigation(url: &Url) -> bool {
    url.scheme() == "tauri"
        && url.host_str() == Some("localhost")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

fn new_window_response<R: Runtime>() -> NewWindowResponse<R> {
    NewWindowResponse::Deny
}

fn create_windows<R: Runtime>(app: &App<R>) -> Result<(), Error> {
    for config in &app.config().app.windows {
        WebviewWindowBuilder::from_config(app, config)?
            .on_navigation(allows_navigation)
            .on_new_window(|_, _| new_window_response())
            .build()?;
    }
    Ok(())
}
