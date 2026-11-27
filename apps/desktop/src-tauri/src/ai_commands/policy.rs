#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPolicy {
    Read,
    WriteDirect,
    Confirm,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPolicy {
    pub ipc: &'static str,
    pub policy: AiPolicy,
}

const fn read(ipc: &'static str) -> CommandPolicy {
    CommandPolicy {
        ipc,
        policy: AiPolicy::Read,
    }
}

const fn write(ipc: &'static str) -> CommandPolicy {
    CommandPolicy {
        ipc,
        policy: AiPolicy::WriteDirect,
    }
}

const fn confirm(ipc: &'static str) -> CommandPolicy {
    CommandPolicy {
        ipc,
        policy: AiPolicy::Confirm,
    }
}

const fn never(ipc: &'static str) -> CommandPolicy {
    CommandPolicy {
        ipc,
        policy: AiPolicy::Never,
    }
}

pub const COMMAND_POLICIES: [CommandPolicy; 58] = [
    read("app_version"),
    read("simulate_trades"),
    read("search_instruments"),
    read("get_settings"),
    read("get_chat_history"),
    read("get_briefings"),
    read("get_portfolio"),
    read("get_allocation"),
    read("get_job_schedules"),
    read("get_public_ip"),
    read("check_toss_connection"),
    read("get_update_status"),
    read("get_risk_settings"),
    read("get_self_check"),
    read("get_status"),
    read("get_data_status"),
    read("get_performance"),
    read("get_scorecard"),
    read("get_transactions"),
    read("get_instruments"),
    read("get_instrument_detail"),
    read("get_thesis"),
    read("get_signals"),
    read("get_account_selection"),
    read("get_principles"),
    read("get_news_settings"),
    read("get_ai_settings"),
    write("set_job_schedule"),
    write("save_investor_profile"),
    write("save_order_settings"),
    write("add_watchlist"),
    write("remove_watchlist"),
    write("skip_signal"),
    write("select_account"),
    write("set_principles"),
    write("backup_now"),
    write("export_data"),
    write("set_news_source"),
    write("set_sec_contact"),
    write("set_wake_criteria"),
    write("set_aliases"),
    write("set_task_model"),
    write("set_budget_percent"),
    confirm("request_order_approval"),
    never("set_order_permission"),
    never("set_auto_start"),
    never("list_chat_sessions"),
    never("create_chat_session"),
    never("send_chat_message"),
    never("resubscribe_chat_turn"),
    never("cancel_chat_turn"),
    never("open_external"),
    never("copy_text"),
    never("submit_toss_keys"),
    never("restart_to_update"),
    never("delete_all_data"),
    never("set_optional_key"),
    never("set_claude_config_dir"),
];

#[must_use]
pub fn policy_of(ipc: &str) -> Option<AiPolicy> {
    COMMAND_POLICIES
        .iter()
        .find(|entry| entry.ipc == ipc)
        .map(|entry| entry.policy)
}
