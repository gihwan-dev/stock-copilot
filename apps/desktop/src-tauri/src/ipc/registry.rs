use tauri::Runtime;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

use super::{app_version, chat, open_external, order_approval};

#[must_use]
pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[expect(
        trivial_casts,
        reason = "#[specta::specta] 확장이 명령 함수를 fn 포인터로 as 변환한다 (ADR 0016)"
    )]
    let commands = collect_commands![
        app_version::app_version::<tauri::Wry>,
        chat::send_chat_message::<tauri::Wry>,
        chat::get_chat_history::<tauri::Wry>,
        chat::resubscribe_chat_turn::<tauri::Wry>,
        chat::cancel_chat_turn::<tauri::Wry>,
        open_external::open_external::<tauri::Wry>,
        order_approval::request_order_approval::<tauri::Wry>
    ];
    Builder::<R>::new()
        .commands(commands)
        .events(collect_events![super::event::UiEvent])
        .error_handling(ErrorHandlingMode::Result)
}
