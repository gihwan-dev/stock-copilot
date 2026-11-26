use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, OnceLock};

use application::{
    AuditEntry, AuditKind, AuditLog, Clock, DiagnosticEvent, DiagnosticText, ToolName,
};
use mcp_server::{
    CursorStore, McpTool, PageRequest, PaginationContext, ToolAccessKind, ToolErrorCode,
    ToolErrorPayload, ToolFuture, ToolInvocation, ToolRegistry, ToolRegistryError, ToolSpec,
    build_object_schema,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tauri::{AppHandle, Runtime};

use crate::ai_runtime::ChatTurnBook;
use crate::ipc::error::{IpcError, IpcErrorKind, IpcSyncError};
use crate::ipc::event::UiEvent;
use crate::ipc::event_bridge::UiEventBridge;
use crate::ipc::state::BoxFuture;

mod policy;
mod routes;
mod shape;

#[cfg(test)]
mod tests;

pub use policy::{AiPolicy, COMMAND_POLICIES, CommandPolicy, policy_of};

pub const TOOL_PREFIX: &str = "app_";

#[must_use]
pub fn tool_name_of(command: &str) -> String {
    format!("{TOOL_PREFIX}{command}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandFailure {
    Arguments,
    App(IpcErrorKind),
    Encoding,
}

pub trait CommandError {
    fn kind(&self) -> IpcErrorKind;
}

impl CommandError for IpcError {
    fn kind(&self) -> IpcErrorKind {
        self.kind
    }
}

impl CommandError for IpcSyncError {
    fn kind(&self) -> IpcErrorKind {
        self.kind
    }
}

pub type CommandFuture = BoxFuture<'static, Result<Value, CommandFailure>>;

pub type Handler<R> = Arc<dyn Fn(AppHandle<R>, Value) -> CommandFuture + Send + Sync>;

pub fn ipc_handler<R, Req, Resp, E, F, Fut>(command: F) -> Handler<R>
where
    R: Runtime,
    Req: DeserializeOwned + Send + 'static,
    Resp: Serialize + 'static,
    E: CommandError + 'static,
    F: Fn(AppHandle<R>, Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Resp, E>> + Send + 'static,
{
    Arc::new(move |app, args| match serde_json::from_value::<Req>(args) {
        Ok(request) => {
            let pending = command(app, request);
            Box::pin(async move {
                match pending.await {
                    Ok(response) => {
                        serde_json::to_value(response).map_err(|_error| CommandFailure::Encoding)
                    }
                    Err(error) => Err(CommandFailure::App(error.kind())),
                }
            })
        }
        Err(_error) => Box::pin(std::future::ready(Err(CommandFailure::Arguments))),
    })
}

pub struct AppCommandSlot<R: Runtime>(OnceLock<AppHandle<R>>);

impl<R: Runtime> Default for AppCommandSlot<R> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

impl<R: Runtime> fmt::Debug for AppCommandSlot<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppCommandSlot")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommandBinding {
    Bound,
    AlreadyBound,
}

impl<R: Runtime> AppCommandSlot<R> {
    pub fn bind(&self, handle: AppHandle<R>) -> AppCommandBinding {
        match self.0.set(handle) {
            Ok(()) => AppCommandBinding::Bound,
            Err(_existing) => AppCommandBinding::AlreadyBound,
        }
    }

    fn handle(&self) -> Option<AppHandle<R>> {
        self.0.get().cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCommandRecord {
    pub command: &'static str,
    pub summary: String,
}

pub trait AiCommandJournal: Send + Sync {
    fn record(&self, record: AiCommandRecord) -> BoxFuture<'_, ()>;
}

pub struct AuditedAiJournal<A, C> {
    audit: Arc<A>,
    clock: Arc<C>,
}

impl<A, C> fmt::Debug for AuditedAiJournal<A, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuditedAiJournal")
    }
}

impl<A, C> AuditedAiJournal<A, C> {
    #[must_use]
    pub const fn new(audit: Arc<A>, clock: Arc<C>) -> Self {
        Self { audit, clock }
    }
}

impl<A: AuditLog + 'static, C: Clock + 'static> AuditedAiJournal<A, C> {
    async fn append(&self, tool: ToolName, summary: DiagnosticText) {
        let entry = AuditEntry::new(
            self.clock.now(),
            AuditKind::AiCommandApplied { tool, summary },
        );
        match self.audit.append(&entry).await {
            Ok(_) | Err(_) => {}
        }
    }
}

impl<A: AuditLog + 'static, C: Clock + 'static> AiCommandJournal for AuditedAiJournal<A, C> {
    fn record(&self, record: AiCommandRecord) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let tool_text = tool_name_of(record.command);
            let summary = DiagnosticText::new(&record.summary);
            crate::diagnostics_log::record(&DiagnosticEvent::AiCommandApplied {
                tool: DiagnosticText::new(&tool_text),
                summary: summary.clone(),
            });
            if let Ok(tool) = ToolName::try_new(&tool_text) {
                self.append(tool, summary).await;
            }
        })
    }
}

pub struct AppCommandDeps<R: Runtime, C: Clock> {
    pub app: Arc<AppCommandSlot<R>>,
    pub turns: ChatTurnBook,
    pub journal: Arc<dyn AiCommandJournal>,
    pub cursors: Arc<CursorStore<C>>,
}

impl<R: Runtime, C: Clock> fmt::Debug for AppCommandDeps<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppCommandDeps")
    }
}

struct AppCommandTool<R: Runtime, C: Clock> {
    policy: AiPolicy,
    route: routes::Route<R>,
    spec: ToolSpec,
    deps: Arc<AppCommandDeps<R, C>>,
}

const LIMIT_FIELD: &str = "limit";
const CURSOR_FIELD: &str = "cursor";

fn page_properties(properties: &mut Map<String, Value>) {
    let mut limit = Map::new();
    limit.insert(String::from("type"), Value::from("integer"));
    limit.insert(String::from("minimum"), Value::from(1_u64));
    limit.insert(String::from("maximum"), Value::from(200_u64));
    let mut cursor = Map::new();
    cursor.insert(String::from("type"), Value::from("string"));
    cursor.insert(String::from("minLength"), Value::from(1_u64));
    cursor.insert(String::from("maxLength"), Value::from(128_u64));
    let _previous_limit = properties.insert(String::from(LIMIT_FIELD), Value::Object(limit));
    let _previous_cursor = properties.insert(String::from(CURSOR_FIELD), Value::Object(cursor));
}

fn tool_spec<R: Runtime>(
    policy: AiPolicy,
    route: &routes::Route<R>,
) -> Result<ToolSpec, ToolRegistryError> {
    let mut properties = route.properties.clone();
    let access = match policy {
        AiPolicy::Read => {
            page_properties(&mut properties);
            ToolAccessKind::ReadOnly
        }
        AiPolicy::WriteDirect | AiPolicy::Confirm => ToolAccessKind::StateChanging,
        AiPolicy::Never => return Err(ToolRegistryError::ForbiddenToolName),
    };
    let name = ToolName::try_new(&tool_name_of(route.command))
        .map_err(|_error| ToolRegistryError::ForbiddenToolName)?;
    ToolSpec::try_new(
        name,
        route.description,
        access,
        build_object_schema(properties, route.required),
        None,
    )
}

pub fn register_app_commands<R: Runtime, C: Clock + 'static>(
    registry: &mut ToolRegistry,
    deps: AppCommandDeps<R, C>,
) -> Result<(), ToolRegistryError> {
    let shared = Arc::new(deps);
    let mut routes: BTreeMap<&'static str, routes::Route<R>> = routes::all_routes::<R>()
        .into_iter()
        .map(|route| (route.command, route))
        .collect();
    for entry in COMMAND_POLICIES {
        match (entry.policy, routes.remove(entry.ipc)) {
            (AiPolicy::Never, None) => {}
            (AiPolicy::Never, Some(_)) | (_, None) => {
                return Err(ToolRegistryError::DuplicateToolName);
            }
            (policy, Some(route)) => {
                let spec = tool_spec(policy, &route)?;
                registry.register(Arc::new(AppCommandTool {
                    policy,
                    route,
                    spec,
                    deps: Arc::clone(&shared),
                }))?;
            }
        }
    }
    if routes.is_empty() {
        Ok(())
    } else {
        Err(ToolRegistryError::DuplicateToolName)
    }
}

fn failure_payload(failure: CommandFailure) -> ToolErrorPayload {
    match failure {
        CommandFailure::Arguments | CommandFailure::App(IpcErrorKind::UserFixable) => {
            ToolErrorPayload::invalid_input(None, "the app rejected these arguments")
        }
        CommandFailure::App(IpcErrorKind::Retryable) => ToolErrorPayload::retryable(
            ToolErrorCode::UpstreamUnavailable,
            None,
            "the app could not finish this now; retry later",
        ),
        CommandFailure::App(IpcErrorKind::NoData) => ToolErrorPayload::non_retryable(
            ToolErrorCode::DataUnavailable,
            None,
            "the app has no data for this yet",
        ),
        CommandFailure::App(IpcErrorKind::Bug) | CommandFailure::Encoding => {
            ToolErrorPayload::non_retryable(
                ToolErrorCode::DataUnavailable,
                None,
                "the app could not complete this command",
            )
        }
    }
}

impl<R: Runtime, C: Clock + 'static> AppCommandTool<R, C> {
    fn allow(&self, run: &application::AiRunId) -> Result<(), ToolErrorPayload> {
        match self.policy {
            AiPolicy::Read => Ok(()),
            AiPolicy::WriteDirect | AiPolicy::Confirm => match self.deps.turns.turn_of(run) {
                Some(_turn) => Ok(()),
                None => Err(ToolErrorPayload::non_retryable(
                    ToolErrorCode::ForbiddenOperation,
                    None,
                    "this tool changes app data and only runs in a chat with the user",
                )),
            },
            AiPolicy::Never => Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::UnknownTool,
                None,
                "requested tool is not registered",
            )),
        }
    }

    fn request(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<(Map<String, Value>, PageRequest), ToolErrorPayload> {
        let mut request = arguments.clone();
        let limit = request.remove(LIMIT_FIELD);
        let cursor = request.remove(CURSOR_FIELD);
        let page = PageRequest::from_raw(
            limit.as_ref().and_then(Value::as_u64),
            cursor.as_ref().and_then(Value::as_str),
        )?;
        for (field, value) in self.route.fixed {
            let _previous = request.insert((*field).to_owned(), Value::from(*value));
        }
        Ok((request, page))
    }

    async fn record(&self, arguments: &Map<String, Value>, app: &AppHandle<R>) {
        self.deps
            .journal
            .record(AiCommandRecord {
                command: self.route.command,
                summary: shape::summarize(arguments),
            })
            .await;
        match self.policy {
            AiPolicy::WriteDirect => {
                let event = UiEvent::AiCommandApplied(self.route.command.to_owned());
                match UiEventBridge::new(app.clone()).emit_ui_event(&event) {
                    Ok(()) | Err(_) => {}
                }
            }
            AiPolicy::Read | AiPolicy::Confirm | AiPolicy::Never => {}
        }
    }

    async fn run(&self, invocation: ToolInvocation<'_>) -> Result<Value, ToolErrorPayload> {
        self.allow(invocation.run_id)?;
        let app = self.deps.app.handle().ok_or_else(|| {
            ToolErrorPayload::retryable(
                ToolErrorCode::UpstreamUnavailable,
                None,
                "the app is still starting; retry shortly",
            )
        })?;
        let (request, page) = self.request(invocation.arguments)?;
        let request_value = Value::Object(request);
        if !(self.route.accepts)(&request_value) {
            return Err(failure_payload(CommandFailure::Arguments));
        }
        let fingerprint = format!("{}:{request_value}", self.route.command);
        let value = (self.route.handler)(app.clone(), request_value)
            .await
            .map_err(failure_payload)?;
        let normalized = shape::normalized(value);
        match self.policy {
            AiPolicy::Read => shape::paged(
                normalized,
                &page,
                &PaginationContext {
                    cursors: &self.deps.cursors,
                    run_id: invocation.run_id,
                    tool_name: self.spec.name(),
                    query_fingerprint: &fingerprint,
                },
            ),
            AiPolicy::WriteDirect | AiPolicy::Confirm => {
                self.record(invocation.arguments, &app).await;
                Ok(shape::object(normalized))
            }
            AiPolicy::Never => Err(failure_payload(CommandFailure::Arguments)),
        }
    }
}

impl<R: Runtime, C: Clock + 'static> McpTool for AppCommandTool<R, C> {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(self.run(invocation))
    }
}
