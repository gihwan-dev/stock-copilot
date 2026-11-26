use domain::ClientOrderId;
use mcp_server::build_object_schema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::future::Future;
use tauri::{AppHandle, Runtime};

use super::{CommandError, Handler, ipc_handler};
use crate::ipc::error::{IpcError, IpcErrorKind};
use crate::ipc::event::UiEvent;
use crate::ipc::event_bridge::UiEventBridge;
use crate::ipc::order_approval::OrderApprovalRequest;
use crate::ipc::{
    ai_settings, app_version, briefing, chat, data_status, instruments, news_settings, portfolio,
    public_ip, risk_editor, schedule, scorecard, self_check, settings, signals, system_data,
    thesis, toss, transactions, updates,
};

type Fixed = &'static [(&'static str, &'static str)];

pub(super) struct Route<R: Runtime> {
    pub(super) command: &'static str,
    pub(super) description: &'static str,
    pub(super) properties: Map<String, Value>,
    pub(super) required: &'static [&'static str],
    pub(super) fixed: Fixed,
    pub(super) handler: Handler<R>,
    pub(super) accepts: fn(&Value) -> bool,
}

pub(super) struct Input {
    properties: Vec<(&'static str, Value)>,
    required: &'static [&'static str],
    fixed: Fixed,
}

const fn fixed(pairs: Fixed) -> Input {
    Input {
        properties: Vec::new(),
        required: &[],
        fixed: pairs,
    }
}

const fn fields(
    properties: Vec<(&'static str, Value)>,
    required: &'static [&'static str],
) -> Input {
    Input {
        properties,
        required,
        fixed: &[],
    }
}

fn accepts<Req: DeserializeOwned>(value: &Value) -> bool {
    serde_json::from_value::<Req>(value.clone()).is_ok()
}

fn ipc<R, Req, Resp, E, F, Fut>(
    command: &'static str,
    description: &'static str,
    input: Input,
    call: F,
) -> Route<R>
where
    R: Runtime,
    Req: DeserializeOwned + Send + 'static,
    Resp: Serialize + 'static,
    E: CommandError + 'static,
    F: Fn(AppHandle<R>, Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Resp, E>> + Send + 'static,
{
    Route {
        command,
        description,
        properties: input
            .properties
            .into_iter()
            .map(|(name, schema)| (name.to_owned(), schema))
            .collect(),
        required: input.required,
        fixed: input.fixed,
        handler: ipc_handler(call),
        accepts: accepts::<Req>,
    }
}

fn one_of(values: &[&str]) -> Value {
    let mut schema = Map::new();
    let _type = schema.insert(String::from("type"), Value::from("string"));
    let _values = schema.insert(
        String::from("enum"),
        Value::Array(values.iter().map(|value| Value::from(*value)).collect()),
    );
    Value::Object(schema)
}

fn text(min: u64, max: u64) -> Value {
    let mut schema = Map::new();
    let _type = schema.insert(String::from("type"), Value::from("string"));
    let _min = schema.insert(String::from("minLength"), Value::from(min));
    let _max = schema.insert(String::from("maxLength"), Value::from(max));
    Value::Object(schema)
}

fn integer(min: u64, max: u64) -> Value {
    let mut schema = Map::new();
    let _type = schema.insert(String::from("type"), Value::from("integer"));
    let _min = schema.insert(String::from("minimum"), Value::from(min));
    let _max = schema.insert(String::from("maximum"), Value::from(max));
    Value::Object(schema)
}

fn list(item: Value, max: u64) -> Value {
    let mut schema = Map::new();
    let _type = schema.insert(String::from("type"), Value::from("array"));
    let _items = schema.insert(String::from("items"), item);
    let _max = schema.insert(String::from("maxItems"), Value::from(max));
    Value::Object(schema)
}

fn object(properties: Vec<(&'static str, Value)>, required: &[&str]) -> Value {
    Value::Object(build_object_schema(
        properties
            .into_iter()
            .map(|(name, schema)| (name.to_owned(), schema))
            .collect(),
        required,
    ))
}

fn symbol() -> Value {
    text(1_u64, 20_u64)
}

fn status_routes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "app_version",
            "Read the installed app version. No side effects.",
            fixed(&[("query", "current")]),
            app_version::app_version::<R>,
        ),
        ipc(
            "get_settings",
            "Read the settings overview: saved profile, order switch state and which keys are configured (never key values). No side effects.",
            fixed(&[("scope", "all")]),
            settings::get_settings::<R>,
        ),
        ipc(
            "get_public_ip",
            "Read this computer's public IP as the app shows it. No side effects.",
            fixed(&[("query", "current")]),
            public_ip::get_public_ip::<R>,
        ),
        ipc(
            "check_toss_connection",
            "Check whether the brokerage connection works, like the connection screen. Calls the broker with the stored keys; returns only the connection state.",
            fixed(&[("query", "current")]),
            toss::check_toss_connection::<R>,
        ),
        ipc(
            "get_update_status",
            "Read the app update status. query=check-now also asks the update server for a newer version; it never installs.",
            fields(
                vec![("query", one_of(&["current", "check-now"]))],
                &["query"],
            ),
            updates::get_update_status::<R>,
        ),
        ipc(
            "get_self_check",
            "Read the self-check results shown in the status bar. No side effects.",
            fixed(&[("scope", "current")]),
            self_check::get_self_check::<R>,
        ),
        ipc(
            "get_status",
            "Read the app status screen (data sources, AI jobs, connection). No side effects.",
            fixed(&[("scope", "current")]),
            self_check::get_status::<R>,
        ),
        ipc(
            "get_data_status",
            "Read when each data job (holdings, transactions, valuations, estimates, scoring, briefings, backup) last succeeded and runs next. No side effects.",
            fixed(&[("scope", "current")]),
            data_status::get_data_status::<R>,
        ),
        ipc(
            "get_chat_history",
            "Read a stored chat session by its id. No side effects.",
            fields(vec![("session_id", text(1_u64, 128_u64))], &["session_id"]),
            chat::get_chat_history::<R>,
        ),
    ]
}

fn portfolio_routes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "get_briefings",
            "Read recent briefings as the briefing screen shows them. No side effects.",
            fixed(&[("scope", "recent")]),
            briefing::get_briefings::<R>,
        ),
        ipc(
            "get_portfolio",
            "Read the portfolio screen for the selected account. No side effects.",
            fixed(&[("scope", "current")]),
            portfolio::get_portfolio::<R>,
        ),
        ipc(
            "get_allocation",
            "Read the allocation screen for the selected account. No side effects.",
            fixed(&[("scope", "current")]),
            portfolio::get_allocation::<R>,
        ),
        ipc(
            "get_transactions",
            "Read recent settled transactions as the transactions screen shows them. No side effects.",
            fixed(&[("scope", "recent")]),
            transactions::get_transactions::<R>,
        ),
        ipc(
            "get_scorecard",
            "Read the estimate scorecard screen. No side effects.",
            fixed(&[("scope", "all")]),
            scorecard::get_scorecard::<R>,
        ),
        ipc(
            "get_instruments",
            "Read the tracked instruments list (holdings and watchlist). No side effects.",
            fixed(&[("scope", "tracked")]),
            instruments::get_instruments::<R>,
        ),
        ipc(
            "get_instrument_detail",
            "Read one instrument's detail screen by symbol. No side effects.",
            fields(vec![("symbol", symbol())], &["symbol"]),
            instruments::get_instrument_detail::<R>,
        ),
        ipc(
            "get_thesis",
            "Read the thesis note screen for one symbol. No side effects.",
            fields(vec![("symbol", symbol())], &["symbol"]),
            thesis::get_thesis::<R>,
        ),
        ipc(
            "get_signals",
            "Read current buy and sell signals with their order proposals. No side effects.",
            fixed(&[("scope", "current")]),
            signals::get_signals::<R>,
        ),
    ]
}

fn settings_reads<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "get_job_schedules",
            "Read the AI job schedules (briefings, estimates, collection). No side effects.",
            fixed(&[("scope", "ai")]),
            schedule::get_job_schedules::<R>,
        ),
        ipc(
            "get_risk_settings",
            "Read the investor profile and order settings editors. No side effects.",
            fixed(&[("scope", "current")]),
            risk_editor::get_risk_settings::<R>,
        ),
        ipc(
            "get_account_selection",
            "Read the brokerage accounts (masked numbers) and which one is selected. No side effects.",
            fixed(&[("scope", "current")]),
            system_data::get_account_selection::<R>,
        ),
        ipc(
            "get_principles",
            "Read the investment principles (note, cash floor, concentration floor). No side effects.",
            fixed(&[("scope", "current")]),
            system_data::get_principles::<R>,
        ),
        ipc(
            "get_news_settings",
            "Read news and data source settings, wake criteria and aliases. Shows only whether optional keys are set, never their values. No side effects.",
            fixed(&[("scope", "current")]),
            news_settings::get_news_settings::<R>,
        ),
        ipc(
            "get_ai_settings",
            "Read per-task AI model and effort settings and the AI budget. No side effects.",
            fixed(&[("scope", "current")]),
            ai_settings::get_ai_settings::<R>,
        ),
    ]
}

fn profile_draft() -> Value {
    object(
        vec![
            (
                "horizon",
                one_of(&["hours", "few-days", "weeks-to-months", "half-year-or-more"]),
            ),
            (
                "risk_appetite",
                one_of(&["conservative", "neutral", "aggressive"]),
            ),
            ("instrument_cap", one_of(&["10", "15", "20", "25", "30"])),
            ("sector_cap", one_of(&["30", "40", "50"])),
            (
                "excluded_types",
                list(one_of(&["etf", "etn", "preferred", "spac", "reit"]), 5_u64),
            ),
        ],
        &[
            "horizon",
            "risk_appetite",
            "instrument_cap",
            "sector_cap",
            "excluded_types",
        ],
    )
}

fn order_draft() -> Value {
    object(
        vec![
            ("market_orders", one_of(&["off", "on"])),
            ("hours_horizon", one_of(&["off", "on"])),
            ("card_expiry", one_of(&["5", "10", "15", "30"])),
            ("order_count_limit", one_of(&["off", "3", "5", "10"])),
        ],
        &[
            "market_orders",
            "hours_horizon",
            "card_expiry",
            "order_count_limit",
        ],
    )
}

fn principles_input() -> Value {
    object(
        vec![
            ("text", text(0_u64, 2_000_u64)),
            ("cash_min_percent", one_of(&["0", "5", "10", "20", "30"])),
            (
                "concentration_min_count",
                one_of(&["3", "4", "5", "6", "8", "10"]),
            ),
        ],
        &["text", "cash_min_percent", "concentration_min_count"],
    )
}

fn risk_writes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "save_investor_profile",
            "Save the investor profile, same as the profile editor. Overwrites horizon, risk appetite, caps and excluded types. Raising risk asks the user for Touch ID.",
            fields(vec![("draft", profile_draft())], &["draft"]),
            risk_editor::save_investor_profile::<R>,
        ),
        ipc(
            "save_order_settings",
            "Save order settings, same as the order settings editor. Cannot turn ordering on or off. Relaxing a setting asks the user for Touch ID.",
            fields(vec![("draft", order_draft())], &["draft"]),
            risk_editor::save_order_settings::<R>,
        ),
        ipc(
            "set_principles",
            "Replace the investment principles note and floors, same as the principles editor.",
            fields(vec![("principles", principles_input())], &["principles"]),
            system_data::set_principles::<R>,
        ),
        ipc(
            "select_account",
            "Switch the selected brokerage account by its list position from get_account_selection (\"1\", \"2\", ...).",
            fields(vec![("account", text(1_u64, 4_u64))], &["account"]),
            system_data::select_account::<R>,
        ),
    ]
}

fn instrument_writes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "add_watchlist",
            "Add a symbol to the watchlist.",
            fields(vec![("symbol", symbol())], &["symbol"]),
            instruments::add_watchlist::<R>,
        ),
        ipc(
            "remove_watchlist",
            "Remove a symbol from the watchlist.",
            fields(vec![("symbol", symbol())], &["symbol"]),
            instruments::remove_watchlist::<R>,
        ),
        ipc(
            "skip_signal",
            "Skip the current signal for a symbol with a short reason, same as the skip button. Does not place or cancel orders.",
            fields(
                vec![("symbol", symbol()), ("reason", text(0_u64, 200_u64))],
                &["symbol", "reason"],
            ),
            signals::skip_signal::<R>,
        ),
    ]
}

fn kst_time() -> Value {
    object(
        vec![
            ("hour", integer(0_u64, 23_u64)),
            ("minute", integer(0_u64, 59_u64)),
        ],
        &["hour", "minute"],
    )
}

const JOBS: [&str; 8] = [
    "morning-briefing",
    "closing-review",
    "pre-us-market-check",
    "estimate-cycle",
    "candidate-discovery",
    "weekly-review",
    "intraday-watch",
    "news-collection",
];

const FEED_SOURCES: [&str; 15] = [
    "KindDisclosure",
    "DartDisclosureApi",
    "DartDisclosureRss",
    "DartFundamentals",
    "Hankyung",
    "Maekyung",
    "Yonhap",
    "Edaily",
    "GoogleNews",
    "YahooFinance",
    "SecEdgar",
    "BankOfKorea",
    "FederalReserve",
    "UsTreasury",
    "Fred",
];

fn source_setting() -> Value {
    object(
        vec![
            ("source", one_of(&FEED_SOURCES)),
            ("state", one_of(&["on", "off"])),
            ("body_reading", one_of(&["on", "off", "locked"])),
            (
                "interval_minutes",
                one_of(&["1", "2", "5", "10", "15", "30", "60"]),
            ),
        ],
        &["source", "state", "body_reading", "interval_minutes"],
    )
}

fn wake_input() -> Value {
    object(
        vec![
            ("daily_move_percent", one_of(&["3", "5", "7", "10"])),
            ("news_per_hour", one_of(&["3", "5", "10", "20"])),
        ],
        &["daily_move_percent", "news_per_hour"],
    )
}

fn news_writes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "set_job_schedule",
            "Turn an AI job on or off and optionally set its KST time, same as the schedule editor.",
            fields(
                vec![
                    ("job", one_of(&JOBS)),
                    ("enabled", one_of(&["on", "off"])),
                    ("time", kst_time()),
                ],
                &["job", "enabled"],
            ),
            schedule::set_job_schedule::<R>,
        ),
        ipc(
            "set_news_source",
            "Change one news or data source: on/off, article body reading and collection interval.",
            fields(vec![("setting", source_setting())], &["setting"]),
            news_settings::set_news_source::<R>,
        ),
        ipc(
            "set_sec_contact",
            "Set or clear (omit contact) the contact the app sends to SEC EDGAR.",
            fields(vec![("contact", text(1_u64, 256_u64))], &[]),
            news_settings::set_sec_contact::<R>,
        ),
        ipc(
            "set_wake_criteria",
            "Change when the app wakes the AI: daily price move percent and news per hour.",
            fields(vec![("wake", wake_input())], &["wake"]),
            news_settings::set_wake_criteria::<R>,
        ),
        ipc(
            "set_aliases",
            "Replace the whole news alias list (alias text to ticker). Send every alias to keep.",
            fields(
                vec![(
                    "aliases",
                    list(
                        object(
                            vec![("alias", text(1_u64, 64_u64)), ("ticker", symbol())],
                            &["alias", "ticker"],
                        ),
                        200_u64,
                    ),
                )],
                &["aliases"],
            ),
            news_settings::set_aliases::<R>,
        ),
    ]
}

fn ai_writes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "set_task_model",
            "Change the AI model and effort for one task, same as the AI settings screen.",
            fields(
                vec![(
                    "setting",
                    object(
                        vec![
                            (
                                "task",
                                one_of(&[
                                    "chat",
                                    "estimate",
                                    "deep-research",
                                    "briefing",
                                    "news-summary",
                                    "collection",
                                ]),
                            ),
                            ("model", one_of(&["opus", "sonnet", "haiku"])),
                            ("effort", one_of(&["low", "medium", "high", "xhigh", "max"])),
                        ],
                        &["task", "model", "effort"],
                    ),
                )],
                &["setting"],
            ),
            ai_settings::set_task_model::<R>,
        ),
        ipc(
            "set_budget_percent",
            "Change the share of the AI subscription budget the app may use.",
            fields(
                vec![("percent", one_of(&["50", "60", "70", "80", "90"]))],
                &["percent"],
            ),
            ai_settings::set_budget_percent::<R>,
        ),
    ]
}

fn data_writes<R: Runtime>() -> Vec<Route<R>> {
    vec![
        ipc(
            "backup_now",
            "Write an encrypted backup into the app's backup folder now.",
            fixed(&[("scope", "current")]),
            system_data::backup_now::<R>,
        ),
        ipc(
            "export_data",
            "Export app data. Opens the save panel so the user picks where the file goes; returns cancelled if they close it.",
            fixed(&[("scope", "current")]),
            system_data::export_data::<R>,
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum AiApprovalStatus {
    #[serde(rename = "awaiting-user-approval")]
    AwaitingUserApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AiApprovalResponse {
    status: AiApprovalStatus,
    proposal_id: String,
}

async fn request_user_approval<R: Runtime>(
    app: AppHandle<R>,
    request: OrderApprovalRequest,
) -> Result<AiApprovalResponse, IpcError> {
    let proposal = ClientOrderId::try_new(&request.proposal_id).map_err(|_invalid| IpcError {
        kind: IpcErrorKind::UserFixable,
    })?;
    UiEventBridge::new(app)
        .emit_ui_event(&UiEvent::AiApprovalRequested(proposal.as_str().to_owned()))
        .map_err(|_unavailable| IpcError {
            kind: IpcErrorKind::Retryable,
        })?;
    Ok(AiApprovalResponse {
        status: AiApprovalStatus::AwaitingUserApproval,
        proposal_id: proposal.as_str().to_owned(),
    })
}

fn approval_routes<R: Runtime>() -> Vec<Route<R>> {
    vec![ipc(
        "request_order_approval",
        "Ask the user to approve an order proposal from get_signals. Only shows the user an approval prompt; the order is sent only after the user confirms in the app's dialog with Touch ID. Never sends an order by itself.",
        fields(vec![("proposal_id", text(1_u64, 64_u64))], &["proposal_id"]),
        request_user_approval::<R>,
    )]
}

pub(super) fn all_routes<R: Runtime>() -> Vec<Route<R>> {
    [
        status_routes::<R>(),
        portfolio_routes::<R>(),
        settings_reads::<R>(),
        risk_writes::<R>(),
        instrument_writes::<R>(),
        news_writes::<R>(),
        ai_writes::<R>(),
        data_writes::<R>(),
        approval_routes::<R>(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
