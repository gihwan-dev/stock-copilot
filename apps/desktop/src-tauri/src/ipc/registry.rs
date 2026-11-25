use tauri::Runtime;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

use super::{
    ai_settings, app_version, briefing, chat, instruments, news_settings, open_external,
    order_approval, portfolio, public_ip, risk_editor, schedule, scorecard, self_check, settings,
    signals, system_data, thesis, toss, transactions, updates,
};

#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "IPC 명령 목록은 collect_commands! 한 번에 모두 적어야 한다 (ADR 0016)"
)]
pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[expect(
        trivial_casts,
        reason = "#[specta::specta] 확장이 명령 함수를 fn 포인터로 as 변환한다 (ADR 0016)"
    )]
    let commands = collect_commands![
        app_version::app_version::<tauri::Wry>,
        settings::get_settings::<tauri::Wry>,
        settings::set_order_permission::<tauri::Wry>,
        chat::send_chat_message::<tauri::Wry>,
        chat::get_chat_history::<tauri::Wry>,
        chat::resubscribe_chat_turn::<tauri::Wry>,
        chat::cancel_chat_turn::<tauri::Wry>,
        open_external::open_external::<tauri::Wry>,
        order_approval::request_order_approval::<tauri::Wry>,
        briefing::get_briefings::<tauri::Wry>,
        portfolio::get_portfolio::<tauri::Wry>,
        portfolio::get_allocation::<tauri::Wry>,
        schedule::get_job_schedules::<tauri::Wry>,
        schedule::set_job_schedule::<tauri::Wry>,
        public_ip::get_public_ip::<tauri::Wry>,
        public_ip::copy_text::<tauri::Wry>,
        toss::check_toss_connection::<tauri::Wry>,
        toss::submit_toss_keys::<tauri::Wry>,
        updates::get_update_status::<tauri::Wry>,
        updates::restart_to_update::<tauri::Wry>,
        risk_editor::get_risk_settings::<tauri::Wry>,
        risk_editor::save_investor_profile::<tauri::Wry>,
        risk_editor::save_order_settings::<tauri::Wry>,
        self_check::get_self_check::<tauri::Wry>,
        self_check::get_status::<tauri::Wry>,
        scorecard::get_scorecard::<tauri::Wry>,
        transactions::get_transactions::<tauri::Wry>,
        instruments::get_instruments::<tauri::Wry>,
        instruments::get_instrument_detail::<tauri::Wry>,
        instruments::add_watchlist::<tauri::Wry>,
        instruments::remove_watchlist::<tauri::Wry>,
        thesis::get_thesis::<tauri::Wry>,
        signals::get_signals::<tauri::Wry>,
        signals::skip_signal::<tauri::Wry>,
        system_data::get_account_selection::<tauri::Wry>,
        system_data::select_account::<tauri::Wry>,
        system_data::get_principles::<tauri::Wry>,
        system_data::set_principles::<tauri::Wry>,
        system_data::backup_now::<tauri::Wry>,
        system_data::export_data::<tauri::Wry>,
        system_data::delete_all_data::<tauri::Wry>,
        news_settings::get_news_settings::<tauri::Wry>,
        news_settings::set_news_source::<tauri::Wry>,
        news_settings::set_optional_key::<tauri::Wry>,
        news_settings::set_sec_contact::<tauri::Wry>,
        news_settings::set_wake_criteria::<tauri::Wry>,
        news_settings::set_aliases::<tauri::Wry>,
        ai_settings::get_ai_settings::<tauri::Wry>,
        ai_settings::set_task_model::<tauri::Wry>,
        ai_settings::set_budget_percent::<tauri::Wry>,
        ai_settings::set_claude_config_dir::<tauri::Wry>
    ];
    Builder::<R>::new()
        .commands(commands)
        .events(collect_events![super::event::UiEvent])
        .error_handling(ErrorHandlingMode::Result)
}
