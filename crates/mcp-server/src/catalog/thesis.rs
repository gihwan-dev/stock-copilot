use std::sync::Arc;

use application::{
    AiRunId, CollectedItemHandle, ThesisRepository, ThesisRunMetadata, ThesisToolPort,
    ThesisUpdateFailure, ToolCallHistory, ToolCallOutcome, ToolName, UpdateThesis,
};
use domain::{
    ChangeDisposition, ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
    CriterionAssessment, CriterionComparison, CriterionKind, CriterionVerdict, Evidence,
    EvidenceRef, EvidenceUrl, HistoryAction, InstrumentId, KillCriterion, NormalizedUrl,
    ProposedChange, Symbol, Thesis, ThesisOperation, ThesisPatch, ThesisStatus, ThesisVersion,
    ToolReference, UntrustedText,
};
use serde_json::{Map, Value, json};

use crate::catalog::render::{object, upstream_error};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorCode, ToolErrorPayload, ToolFuture, ToolInvocation,
    ToolRegistry, ToolRegistryError, ToolSpec, build_object_schema,
};
use crate::untrusted::{NormalizedUntrustedText, UntrustedTextLimit};

const TEXT_CHARS: usize = 1_024_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThesisToolKind {
    Get,
    Update,
}

struct ThesisTool<T, R, H> {
    kind: ThesisToolKind,
    spec: ToolSpec,
    port: Arc<T>,
    repository: Arc<R>,
    history: Arc<H>,
}

fn invalid(field: &str, message: &'static str) -> ToolErrorPayload {
    ToolErrorPayload::invalid_input(Some(field), message)
}

fn text_field(value: &Map<String, Value>, key: &str) -> Result<UntrustedText, ToolErrorPayload> {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|raw| UntrustedText::try_new(raw).ok())
        .ok_or_else(|| invalid(key, "required text field is missing or invalid"))
}

fn claim_id(value: &Map<String, Value>) -> Result<ClaimId, ToolErrorPayload> {
    value
        .get("claim_id")
        .and_then(Value::as_str)
        .and_then(|raw| ClaimId::try_new(raw).ok())
        .ok_or_else(|| {
            invalid(
                "claim_id",
                "claim_id must be 1 to 64 letters, digits, _ or -",
            )
        })
}

fn parse_role(raw: Option<&Value>) -> Result<ClaimRole, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        Some("core") => Ok(ClaimRole::Core),
        Some("supporting") => Ok(ClaimRole::Supporting),
        Some(_) | None => Err(invalid("role", "role must be core or supporting")),
    }
}

fn parse_claim_verdict(raw: Option<&Value>) -> Result<ClaimVerdict, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        Some("intact") => Ok(ClaimVerdict::Intact),
        Some("weakened") => Ok(ClaimVerdict::Weakened),
        Some("broken") => Ok(ClaimVerdict::Broken),
        Some("unverifiable") => Ok(ClaimVerdict::Unverifiable),
        Some(_) | None => Err(invalid("verdict", "verdict is not a claim verdict")),
    }
}

fn parse_criterion_verdict(raw: Option<&Value>) -> Result<CriterionVerdict, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        Some("met") => Ok(CriterionVerdict::Met),
        Some("not_met") => Ok(CriterionVerdict::NotMet),
        Some("unverifiable") => Ok(CriterionVerdict::Unverifiable),
        Some(_) | None => Err(invalid("verdict", "verdict is not a criterion verdict")),
    }
}

fn rejected(_error: &domain::ThesisError) -> ToolErrorPayload {
    ToolErrorPayload::invalid_input(
        Some("changes"),
        "the thesis change was rejected by the app rules; reload with get_thesis and retry",
    )
}

impl<T: ThesisToolPort, R: ThesisRepository, H: ToolCallHistory> ThesisTool<T, R, H> {
    async fn resolve(
        &self,
        run: &AiRunId,
        args: &Map<String, Value>,
    ) -> Result<InstrumentId, ToolErrorPayload> {
        let symbol = args
            .get("symbol")
            .and_then(Value::as_str)
            .and_then(|raw| Symbol::try_new(raw).ok())
            .ok_or_else(|| invalid("symbol", "symbol is not a valid listed code"))?;
        self.port
            .resolve_target(run, &symbol)
            .await
            .map_err(|error| upstream_error(&error))?
            .ok_or_else(|| {
                invalid(
                    "symbol",
                    "symbol is not a holding, watchlist entry or app-selected candidate",
                )
            })
    }

    async fn tool_reference(
        &self,
        run: &AiRunId,
        raw: &str,
    ) -> Result<EvidenceRef, ToolErrorPayload> {
        let reference = ToolReference::try_new(raw).map_err(|_err| {
            invalid("evidence", "tool evidence must look like tool:<request id>")
        })?;
        let calls = self
            .history
            .calls_in_run(run)
            .await
            .map_err(|error| upstream_error(&error))?;
        let suffix = reference.as_str().strip_prefix("tool:");
        let found = calls.iter().any(|call| {
            call.run_id() == run
                && call.outcome() == ToolCallOutcome::Succeeded
                && suffix == Some(call.request_id().as_str())
        });
        if found {
            Ok(EvidenceRef::Tool(reference))
        } else {
            Err(invalid(
                "evidence",
                "tool evidence must reference a successful tool call of this run",
            ))
        }
    }

    async fn evidence_item(
        &self,
        run: &AiRunId,
        instrument: &InstrumentId,
        item: &Value,
    ) -> Result<EvidenceRef, ToolErrorPayload> {
        let kind = item.get("kind").and_then(Value::as_str);
        let raw = item
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("evidence", "evidence value is missing"))?;
        match kind {
            Some("tool") => self.tool_reference(run, raw).await,
            Some("collected") => {
                let handle = CollectedItemHandle::try_new(raw).map_err(|_err| {
                    invalid("evidence", "collected evidence needs an item handle")
                })?;
                self.port
                    .collected_evidence(run, instrument, &handle)
                    .await
                    .map_err(|error| upstream_error(&error))?
                    .ok_or_else(|| invalid("evidence", "handle is not an item the app collected"))
            }
            Some("web") => NormalizedUrl::try_new(raw)
                .ok()
                .and_then(|url| EvidenceUrl::try_new(url).ok())
                .map(EvidenceRef::Web)
                .ok_or_else(|| {
                    invalid("evidence", "web evidence must be an http or https address")
                }),
            Some(_) | None => Err(invalid(
                "evidence",
                "evidence kind must be tool, collected or web",
            )),
        }
    }

    async fn evidence(
        &self,
        run: &AiRunId,
        instrument: &InstrumentId,
        change: &Map<String, Value>,
    ) -> Result<Evidence, ToolErrorPayload> {
        let items = change
            .get("evidence")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("evidence", "every change needs evidence"))?;
        let mut refs = Vec::new();
        for item in items {
            refs.push(self.evidence_item(run, instrument, item).await?);
        }
        Evidence::try_new(refs).map_err(|error| rejected(&error))
    }

    fn operation(change: &Map<String, Value>) -> Result<ThesisOperation, ToolErrorPayload> {
        let id = claim_id(change)?;
        match change.get("op").and_then(Value::as_str) {
            Some("add_claim") => {
                let content = ClaimContent::try_new(
                    text_field(change, "statement")?,
                    text_field(change, "metric")?,
                    text_field(change, "expected")?,
                    text_field(change, "observe_via")?,
                    text_field(change, "refutation")?,
                )
                .map_err(|error| rejected(&error))?;
                Ok(ThesisOperation::AddClaim(Claim::new(
                    id,
                    parse_role(change.get("role"))?,
                    content,
                )))
            }
            Some("judge_claim") => Ok(ThesisOperation::Judge(
                id,
                parse_claim_verdict(change.get("verdict"))?,
            )),
            Some("add_kill_criterion") => {
                let criterion = KillCriterion::new(
                    id,
                    CriterionKind::Narrative(text_field(change, "criterion")?),
                )
                .map_err(|error| rejected(&error))?;
                Ok(ThesisOperation::AddKillCriterion(criterion))
            }
            Some("judge_kill_criterion") => Ok(ThesisOperation::JudgeCriterion(
                id,
                CriterionAssessment::Ai(parse_criterion_verdict(change.get("verdict"))?),
            )),
            Some(_) | None => Err(invalid("op", "op is not a supported thesis operation")),
        }
    }

    async fn patch(
        &self,
        run: &AiRunId,
        instrument: &InstrumentId,
        args: &Map<String, Value>,
        metadata: &ThesisRunMetadata,
    ) -> Result<ThesisPatch, ToolErrorPayload> {
        let changes = args
            .get("changes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("changes", "changes are required"))?;
        let mut proposed = Vec::new();
        for raw in changes {
            let change = raw
                .as_object()
                .ok_or_else(|| invalid("changes", "each change must be an object"))?;
            let operation = Self::operation(change)?;
            let evidence = self.evidence(run, instrument, change).await?;
            let reason = ChangeReason::try_new(
                text_field(change, "reason")?,
                metadata.model.clone(),
                metadata.prompt_version.clone(),
            )
            .map_err(|error| rejected(&error))?;
            proposed.push(ProposedChange::new(operation, evidence, reason));
        }
        ThesisPatch::try_new(proposed).map_err(|error| rejected(&error))
    }

    async fn update(
        &self,
        run: &AiRunId,
        args: &Map<String, Value>,
    ) -> Result<Value, ToolErrorPayload> {
        let instrument = self.resolve(run, args).await?;
        let base = args
            .get("base_version")
            .and_then(Value::as_u64)
            .and_then(|raw| usize::try_from(raw).ok())
            .and_then(|raw| ThesisVersion::try_new(raw).ok())
            .ok_or_else(|| invalid("base_version", "base_version must come from get_thesis"))?;
        let metadata = self
            .port
            .run_metadata(run)
            .await
            .map_err(|error| upstream_error(&error))?;
        let patch = self.patch(run, &instrument, args, &metadata).await?;
        let thesis = UpdateThesis::new(self.repository.as_ref())
            .execute(&instrument, base, patch)
            .await
            .map_err(|failure| match failure {
                ThesisUpdateFailure::Rejected(error) => rejected(&error),
                ThesisUpdateFailure::Storage(error) => upstream_error(&error),
            })?;
        let (applied, needs_review) = new_dispositions(&thesis, base);
        Ok(object(vec![
            ("version", Value::from(*thesis.version().as_usize())),
            ("applied", Value::from(applied)),
            ("needs_review", Value::from(needs_review)),
            (
                "status",
                Value::String(status_name(thesis.status()).to_owned()),
            ),
        ]))
    }

    async fn get(
        &self,
        run: &AiRunId,
        args: &Map<String, Value>,
    ) -> Result<Value, ToolErrorPayload> {
        let instrument = self.resolve(run, args).await?;
        let thesis = UpdateThesis::new(self.repository.as_ref())
            .load(&instrument)
            .await
            .map_err(|error| upstream_error(&error))?;
        render_thesis(&thesis)
    }
}

fn new_dispositions(thesis: &Thesis, base: ThesisVersion) -> (usize, usize) {
    thesis
        .history()
        .iter()
        .filter(|entry| entry.sequence() > base)
        .filter(|entry| match entry.action() {
            HistoryAction::Change(_) => true,
            HistoryAction::Decision(..) => false,
        })
        .fold((0_usize, 0_usize), |(applied, review), entry| {
            match entry.disposition() {
                ChangeDisposition::Applied => (applied.saturating_add(1_usize), review),
                ChangeDisposition::NeedsReview => (applied, review.saturating_add(1_usize)),
            }
        })
}

const fn status_name(status: ThesisStatus) -> &'static str {
    match status {
        ThesisStatus::SuspectedBroken => "suspected_broken",
        ThesisStatus::NoBreakSignal => "no_break_signal",
    }
}

fn untrusted(text: &UntrustedText) -> Result<Value, ToolErrorPayload> {
    let limit = UntrustedTextLimit::try_new(TEXT_CHARS).map_err(|_out_of_range| {
        ToolErrorPayload::non_retryable(
            ToolErrorCode::InternalError,
            None,
            "text limit is misconfigured",
        )
    })?;
    Ok(NormalizedUntrustedText::from_untrusted(text, limit).into_field_object())
}

const fn claim_verdict_name(verdict: ClaimVerdict) -> &'static str {
    match verdict {
        ClaimVerdict::Intact => "intact",
        ClaimVerdict::Weakened => "weakened",
        ClaimVerdict::Broken => "broken",
        ClaimVerdict::Unverifiable => "unverifiable",
    }
}

const fn criterion_verdict_name(verdict: CriterionVerdict) -> &'static str {
    match verdict {
        CriterionVerdict::Met => "met",
        CriterionVerdict::NotMet => "not_met",
        CriterionVerdict::Unverifiable => "unverifiable",
    }
}

const fn comparison_name(comparison: CriterionComparison) -> &'static str {
    match comparison {
        CriterionComparison::AtLeast => "at_least",
        CriterionComparison::AtMost => "at_most",
        CriterionComparison::Equal => "equal",
    }
}

fn render_claims(thesis: &Thesis) -> Result<Vec<Value>, ToolErrorPayload> {
    let mut claims: Vec<_> = thesis.claims().iter().collect();
    claims.sort_by(|left, right| left.claim().id().cmp(right.claim().id()));
    claims
        .into_iter()
        .map(|judged| {
            let claim = judged.claim();
            let role = match claim.role() {
                ClaimRole::Core => "core",
                ClaimRole::Supporting => "supporting",
            };
            Ok(object(vec![
                ("claim_id", Value::String(claim.id().as_str().to_owned())),
                ("role", Value::String(role.to_owned())),
                (
                    "verdict",
                    Value::String(claim_verdict_name(judged.verdict()).to_owned()),
                ),
                ("statement", untrusted(claim.statement())?),
                ("metric", untrusted(claim.metric())?),
                ("expected", untrusted(claim.expected())?),
                ("observe_via", untrusted(claim.observe_via())?),
                ("refutation", untrusted(claim.refutation())?),
            ]))
        })
        .collect()
}

fn render_criteria(thesis: &Thesis) -> Result<Vec<Value>, ToolErrorPayload> {
    let mut criteria: Vec<_> = thesis.kill_criteria().iter().collect();
    criteria.sort_by(|left, right| left.criterion().id().cmp(right.criterion().id()));
    criteria
        .into_iter()
        .map(|judged| {
            let criterion = judged.criterion();
            let detail = match criterion.kind() {
                CriterionKind::Narrative(text) => object(vec![
                    ("kind", json!("narrative")),
                    ("text", untrusted(text)?),
                ]),
                CriterionKind::Machine(condition) => object(vec![
                    ("kind", json!("machine")),
                    (
                        "metric",
                        Value::String(condition.metric().as_str().to_owned()),
                    ),
                    (
                        "comparison",
                        Value::String(comparison_name(condition.comparison()).to_owned()),
                    ),
                    (
                        "threshold",
                        Value::String(condition.threshold().as_decimal().to_string()),
                    ),
                ]),
            };
            Ok(object(vec![
                (
                    "criterion_id",
                    Value::String(criterion.id().as_str().to_owned()),
                ),
                (
                    "verdict",
                    Value::String(criterion_verdict_name(judged.verdict()).to_owned()),
                ),
                ("criterion", detail),
            ]))
        })
        .collect()
}

fn render_thesis(thesis: &Thesis) -> Result<Value, ToolErrorPayload> {
    let pending: Vec<Value> = thesis
        .pending()
        .iter()
        .map(|entry| Value::from(*entry.sequence().as_usize()))
        .collect();
    Ok(object(vec![
        ("version", Value::from(*thesis.version().as_usize())),
        (
            "status",
            Value::String(status_name(thesis.status()).to_owned()),
        ),
        ("claims", Value::Array(render_claims(thesis)?)),
        ("kill_criteria", Value::Array(render_criteria(thesis)?)),
        ("pending_review_sequences", Value::Array(pending)),
    ]))
}

impl<T, R, H> McpTool for ThesisTool<T, R, H>
where
    T: ThesisToolPort + 'static,
    R: ThesisRepository + 'static,
    H: ToolCallHistory + 'static,
{
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move {
            match self.kind {
                ThesisToolKind::Get => self.get(invocation.run_id, invocation.arguments).await,
                ThesisToolKind::Update => {
                    self.update(invocation.run_id, invocation.arguments).await
                }
            }
        })
    }
}

fn symbol_schema() -> Value {
    json!({ "type": "string", "minLength": 1_u64, "maxLength": 12_u64 })
}

fn change_schema() -> Value {
    let short = json!({ "type": "string", "minLength": 1_u64, "maxLength": 256_u64 });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "claim_id", "reason", "evidence"],
        "properties": {
            "op": { "type": "string", "enum": ["add_claim", "judge_claim", "add_kill_criterion", "judge_kill_criterion"] },
            "claim_id": { "type": "string", "minLength": 1_u64, "maxLength": 64_u64 },
            "role": { "type": "string", "enum": ["core", "supporting"] },
            "statement": { "type": "string", "minLength": 1_u64, "maxLength": 1024_u64 },
            "metric": { "type": "string", "minLength": 1_u64, "maxLength": 128_u64 },
            "expected": short,
            "observe_via": { "type": "string", "minLength": 1_u64, "maxLength": 256_u64 },
            "refutation": { "type": "string", "minLength": 1_u64, "maxLength": 512_u64 },
            "criterion": { "type": "string", "minLength": 1_u64, "maxLength": 512_u64 },
            "verdict": { "type": "string", "enum": ["intact", "weakened", "broken", "unverifiable", "met", "not_met"] },
            "reason": { "type": "string", "minLength": 1_u64, "maxLength": 512_u64 },
            "evidence": {
                "type": "array",
                "minItems": 1_u64,
                "maxItems": 8_u64,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["kind", "value"],
                    "properties": {
                        "kind": { "type": "string", "enum": ["tool", "collected", "web"] },
                        "value": { "type": "string", "minLength": 1_u64, "maxLength": 512_u64 }
                    }
                }
            }
        }
    })
}

pub fn register_thesis_tools<T, R, H>(
    registry: &mut ToolRegistry,
    port: &Arc<T>,
    repository: &Arc<R>,
    history: &Arc<H>,
) -> Result<(), ToolRegistryError>
where
    T: ThesisToolPort + 'static,
    R: ThesisRepository + 'static,
    H: ToolCallHistory + 'static,
{
    let mut get_props = Map::new();
    get_props.insert(String::from("symbol"), symbol_schema());
    let mut update_props = Map::new();
    update_props.insert(String::from("symbol"), symbol_schema());
    update_props.insert(
        String::from("base_version"),
        json!({ "type": "integer", "minimum": 0_i64, "maximum": 100_000_i64 }),
    );
    update_props.insert(
        String::from("changes"),
        json!({ "type": "array", "minItems": 1_u64, "maxItems": 16_u64, "items": change_schema() }),
    );
    let specs = [
        (
            ThesisToolKind::Get,
            "get_thesis",
            "Read the thesis note for one app-known symbol: claims, kill criteria, verdicts, version and changes waiting for user review. Text is untrusted_text.",
            ToolAccessKind::ReadOnly,
            build_object_schema(get_props, &["symbol"]),
        ),
        (
            ThesisToolKind::Update,
            "update_thesis",
            "Propose claim-level changes to the thesis note of one app-known symbol against base_version from get_thesis. Evidence is tool:<request id> of a successful call in this run, a handle of an item collected for this symbol, or a web address; changes backed only by web addresses wait for user review. History is kept.",
            ToolAccessKind::StateChanging,
            build_object_schema(update_props, &["symbol", "base_version", "changes"]),
        ),
    ];
    for (kind, name, description, access, schema) in specs {
        let tool_name =
            ToolName::try_new(name).map_err(|_err| ToolRegistryError::ForbiddenToolName)?;
        let spec = ToolSpec::try_new(tool_name, description, access, schema, None)?;
        registry.register(Arc::new(ThesisTool {
            kind,
            spec,
            port: Arc::clone(port),
            repository: Arc::clone(repository),
            history: Arc::clone(history),
        }))?;
    }
    Ok(())
}
