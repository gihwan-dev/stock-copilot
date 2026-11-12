use tauri::Runtime;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

use super::app_version;

#[must_use]
pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[expect(
        trivial_casts,
        reason = "#[specta::specta] 확장이 명령 함수를 fn 포인터로 as 변환한다 (ADR 0016)"
    )]
    let commands = collect_commands![app_version::app_version::<tauri::Wry>];
    Builder::<R>::new()
        .commands(commands)
        .error_handling(ErrorHandlingMode::Result)
}
