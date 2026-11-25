use crate::provider::{ClaudeCodeProvider, ClaudeEventStream};
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, Clock, CollectionQuery, IsolationCheck,
    ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec, WebCollectionFailure,
    WebCollectionRunner, WebFinding,
};
use domain::{NormalizedUrl, UntrustedText};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const WEB_COLLECTION_JSON_SCHEMA: &str = r#"{"type":"object","properties":{"items":{"type":"array","maxItems":20,"items":{"type":"object","properties":{"url":{"type":"string"},"title":{"type":"string"},"summary":{"type":"string"}},"required":["url","title"],"additionalProperties":false}}},"required":["items"],"additionalProperties":false}"#;
pub const MAX_WEB_FINDINGS: usize = 20_usize;

pub struct ClaudeWebCollectionRunner {
    provider: Arc<ClaudeCodeProvider>,
    clock: Arc<dyn Clock>,
    sequence: AtomicU64,
}

impl fmt::Debug for ClaudeWebCollectionRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeWebCollectionRunner")
            .finish_non_exhaustive()
    }
}

impl ClaudeWebCollectionRunner {
    #[must_use]
    pub fn new(provider: Arc<ClaudeCodeProvider>, clock: Arc<dyn Clock>) -> Self {
        Self {
            provider,
            clock,
            sequence: AtomicU64::new(1_u64),
        }
    }

    fn next_run_id(&self) -> Result<AiRunId, WebCollectionFailure> {
        let seq = self.sequence.fetch_add(1_u64, Ordering::Relaxed);
        let seconds = *self.clock.now().utc_seconds().as_i64();
        AiRunId::try_new(&format!("collect-{seconds}-{seq}"))
            .map_err(|_error| WebCollectionFailure::Unavailable)
    }
}

impl WebCollectionRunner for ClaudeWebCollectionRunner {
    async fn collect(
        &self,
        query: &CollectionQuery,
    ) -> Result<Vec<WebFinding>, WebCollectionFailure> {
        let spec = RunSpec::new(
            self.next_run_id()?,
            RunInput::Collection(query.clone()),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        );
        let mut stream = self
            .provider
            .run(spec)
            .await
            .map_err(|_error| WebCollectionFailure::Unavailable)?;
        drain_collection_stream(&mut stream).await?;
        let output = stream
            .structured_output()
            .ok_or(WebCollectionFailure::Unavailable)?;
        parse_web_findings(output).ok_or(WebCollectionFailure::Unavailable)
    }
}

async fn drain_until_closed(stream: &mut ClaudeEventStream) {
    while let Ok(Some(_event)) = stream.next().await {}
}

async fn drain_collection_stream(
    stream: &mut ClaudeEventStream,
) -> Result<(), WebCollectionFailure> {
    match stream.next().await {
        Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Passed))) => {}
        Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Rejected(_)))) => {
            drain_until_closed(stream).await;
            return Err(WebCollectionFailure::Rejected);
        }
        Ok(Some(_) | None) | Err(_) => return Err(WebCollectionFailure::Unavailable),
    }
    loop {
        match stream.next().await {
            Ok(None) => return Ok(()),
            Ok(Some(event)) => check_collection_event(&event)?,
            Err(_) => return Err(WebCollectionFailure::Unavailable),
        }
    }
}

const fn check_collection_event(event: &AiEvent) -> Result<(), WebCollectionFailure> {
    match event {
        AiEvent::ToolCall { .. } | AiEvent::McpUnavailable | AiEvent::IsolationChecked(_) => {
            Err(WebCollectionFailure::Rejected)
        }
        AiEvent::TextDelta(_)
        | AiEvent::WebSearch { .. }
        | AiEvent::TokenUsage(_)
        | AiEvent::Quota(_)
        | AiEvent::RateLimited { .. }
        | AiEvent::ModelRerouted(_)
        | AiEvent::ActualModel(_)
        | AiEvent::Unknown(_) => Ok(()),
    }
}

#[must_use]
pub fn parse_web_findings(output: &Value) -> Option<Vec<WebFinding>> {
    let items = output.get("items")?.as_array()?;
    Some(
        items
            .iter()
            .filter_map(parse_one_finding)
            .take(MAX_WEB_FINDINGS)
            .collect(),
    )
}

fn parse_one_finding(item: &Value) -> Option<WebFinding> {
    let url = NormalizedUrl::try_new(item.get("url")?.as_str()?).ok()?;
    let title_text = item.get("title")?.as_str()?.trim();
    if title_text.is_empty() {
        return None;
    }
    let title = UntrustedText::try_new(title_text).ok()?;
    let summary = match item.get("summary") {
        None | Some(Value::Null) => None,
        Some(value) => Some(UntrustedText::try_new(value.as_str()?).ok()?),
    };
    Some(WebFinding::new(url, title, summary))
}
