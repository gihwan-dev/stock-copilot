use std::sync::Arc;

use application::{
    AiRunId, Clock, EstimateAssignmentRepository, EstimateRunContextSource,
    EstimateSubmissionRepository, EstimateTaskView, EventBus, GetEstimateTask,
    GetEstimateTaskError, GetEstimateTaskInput, GetProfile, GetScorecard, GetScorecardError,
    ProfileSource, ProfileView, ReadOnlySettings, ScorecardObservationRepository,
    ScorecardSelection, ScorecardView, SubmitEstimateError, SubmitEstimateTool,
    SubmitEstimateToolError, ToolCallHistory, ToolName,
};
use domain::{
    CalibrationBin, CalibrationEstimateId, CalibrationKey, EstimateConsumption, HorizonSelection,
    HoursPermissionState, RiskAppetite, Scorecard, ScorecardBin, Scorecards,
};
use serde_json::{Map, Value, json};

use crate::catalog::render::{money, object, ratio, timestamp, upstream_error};
use crate::estimate_input::{
    SUBMIT_ESTIMATE_TOOL, parse_submit_estimate_arguments, submit_estimate_schema,
};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorPayload, ToolFuture, ToolInvocation, ToolRegistry,
    ToolRegistryError, ToolSpec, build_object_schema,
};

#[derive(Debug)]
pub struct EstimateToolPorts<A, S, H, X, G, O, C> {
    pub assignments: Arc<A>,
    pub submissions: Arc<S>,
    pub history: Arc<H>,
    pub context: Arc<X>,
    pub settings: Arc<G>,
    pub observations: Arc<O>,
    pub clock: Arc<C>,
    pub events: Arc<EventBus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EstimateToolKind {
    GetTask,
    Submit,
    Profile,
    Scorecard,
}

struct EstimateTool<A, S, H, X, G, O, C> {
    kind: EstimateToolKind,
    spec: ToolSpec,
    ports: Arc<EstimateToolPorts<A, S, H, X, G, O, C>>,
}

fn unknown_task() -> ToolErrorPayload {
    ToolErrorPayload::invalid_input(Some("task_id"), "unknown task_id for this run")
}

fn parse_task_id(args: &Map<String, Value>) -> Result<CalibrationEstimateId, ToolErrorPayload> {
    args.get("task_id")
        .and_then(Value::as_str)
        .and_then(|raw| CalibrationEstimateId::try_new(raw).ok())
        .ok_or_else(unknown_task)
}

const fn horizon_name(selection: HorizonSelection) -> &'static str {
    match selection {
        HorizonSelection::Hours => "hours",
        HorizonSelection::FewDays => "few_days",
        HorizonSelection::WeeksToMonths => "weeks_to_months",
        HorizonSelection::HalfYearOrMore => "half_year_or_more",
    }
}

fn render_task(view: &EstimateTaskView) -> Value {
    let consumption = match view.consumption() {
        EstimateConsumption::Available => "available",
        EstimateConsumption::Consumed => "submitted",
    };
    object(vec![
        (
            "task_id",
            Value::String(view.task_id().as_estimate_id().as_str().to_owned()),
        ),
        ("symbol", Value::String(view.symbol().as_str().to_owned())),
        (
            "market",
            Value::String(view.market().as_code().as_str().to_owned()),
        ),
        (
            "event",
            object(vec![
                ("target_return", Value::String(view.target().to_string())),
                ("stop_loss", Value::String(view.stop().to_string())),
                ("horizon_trading_days", ratio(&view.horizon().as_ratio())),
                (
                    "rule",
                    json!(
                        "success if the target return is touched before the stop loss within the horizon; otherwise failure"
                    ),
                ),
            ]),
        ),
        (
            "base_probability",
            Value::String(view.base_probability().to_string()),
        ),
        ("reference_price", money(&view.reference_price().as_money())),
        ("reference_time", timestamp(view.asked_at())),
        ("status", Value::String(consumption.to_owned())),
    ])
}

fn render_profile(view: &ProfileView) -> Value {
    let (source, profile) = match view {
        ProfileView::Saved(profile) => ("saved", profile),
        ProfileView::Default(profile) => ("default", profile),
    };
    let saved = profile.saved_state();
    let appetite = match saved.risk_appetite {
        RiskAppetite::Conservative => "conservative",
        RiskAppetite::Neutral => "neutral",
        RiskAppetite::Aggressive => "aggressive",
    };
    let hours = match saved.hours_permission {
        HoursPermissionState::Disabled => "disabled",
        HoursPermissionState::Enabled => "enabled",
    };
    let mut sectors: Vec<&str> = saved.sectors.iter().map(domain::SectorId::as_str).collect();
    sectors.sort_unstable();
    let mut security_types: Vec<String> = saved
        .security_types
        .iter()
        .map(|kind| format!("{kind:?}"))
        .collect();
    security_types.sort_unstable();
    object(vec![
        ("source", Value::String(source.to_owned())),
        (
            "horizon",
            Value::String(horizon_name(saved.horizon).to_owned()),
        ),
        ("hours_trading", Value::String(hours.to_owned())),
        ("risk_appetite", Value::String(appetite.to_owned())),
        ("instrument_weight_cap", ratio(&saved.instrument_cap)),
        (
            "instrument_weight_warning",
            ratio(&saved.instrument_warning),
        ),
        ("sector_weight_cap", ratio(&saved.sector_cap)),
        ("excluded_sectors", json!(sectors)),
        ("excluded_security_types", json!(security_types)),
    ])
}

const fn bin_name(bin: CalibrationBin) -> &'static str {
    match bin {
        CalibrationBin::Below50 => "below_50",
        CalibrationBin::From50To60 => "50_to_60",
        CalibrationBin::From60To70 => "60_to_70",
        CalibrationBin::AtLeast70 => "at_least_70",
    }
}

fn render_key(key: &CalibrationKey) -> Value {
    let model = key.model();
    object(vec![
        (
            "provider",
            Value::String(model.provider().as_str().to_owned()),
        ),
        ("model", Value::String(model.name().as_str().to_owned())),
        (
            "version",
            Value::String(model.version().as_str().to_owned()),
        ),
        (
            "horizon",
            Value::String(horizon_name(key.horizon().selection()).to_owned()),
        ),
    ])
}

fn render_card(card: &Scorecard) -> Value {
    match card {
        Scorecard::Pending(pending) => object(vec![
            ("key", render_key(pending.key())),
            ("status", json!("pending_under_30_judged")),
            ("judged", Value::from(pending.judgment_count().as_usize())),
        ]),
        Scorecard::Ready(metrics) => {
            let bins: Vec<Value> = metrics
                .reliability()
                .iter()
                .map(|bin| match bin {
                    ScorecardBin::NoData(empty) => object(vec![
                        ("bin", Value::String(bin_name(*empty).to_owned())),
                        ("count", Value::from(0_u64)),
                    ]),
                    ScorecardBin::Observed(point) => object(vec![
                        ("bin", Value::String(bin_name(point.bin()).to_owned())),
                        ("count", Value::from(point.count().as_usize())),
                        (
                            "mean_prediction",
                            Value::String(point.mean_prediction().as_str().to_owned()),
                        ),
                        (
                            "hit_rate",
                            Value::String(point.hit_rate().as_str().to_owned()),
                        ),
                    ]),
                })
                .collect();
            object(vec![
                ("key", render_key(metrics.key())),
                ("status", json!("ready")),
                ("judged", Value::from(metrics.judgment_count().as_usize())),
                ("brier", Value::String(metrics.brier().as_str().to_owned())),
                ("reliability", Value::Array(bins)),
            ])
        }
    }
}

fn render_cards(cards: &Scorecards) -> Value {
    object(vec![
        (
            "cards",
            Value::Array(cards.cards().iter().map(render_card).collect()),
        ),
        (
            "model_comparisons",
            Value::from(cards.comparison_count().as_usize()),
        ),
    ])
}

fn submit_failure(error: &SubmitEstimateToolError) -> ToolErrorPayload {
    match error {
        SubmitEstimateToolError::Probability(_) => {
            ToolErrorPayload::invalid_input(Some("p"), "p must be an integer from 1 to 99")
        }
        SubmitEstimateToolError::UnknownSource | SubmitEstimateToolError::Sources(_) => {
            ToolErrorPayload::invalid_input(
                Some("sources"),
                "sources must reference successful tool calls of this run",
            )
        }
        SubmitEstimateToolError::Assignment(_) => unknown_task(),
        SubmitEstimateToolError::Submission(_) => ToolErrorPayload::invalid_input(
            Some("task_id"),
            "the estimate could not be recorded for this task",
        ),
        SubmitEstimateToolError::Submit(SubmitEstimateError::Assignment(_)) => {
            ToolErrorPayload::invalid_input(
                Some("task_id"),
                "unknown or already submitted task_id for this run",
            )
        }
        SubmitEstimateToolError::Submit(SubmitEstimateError::Record(_)) => {
            ToolErrorPayload::invalid_input(
                Some("task_id"),
                "the estimate could not be recorded for this task",
            )
        }
        SubmitEstimateToolError::Submit(SubmitEstimateError::Storage(port_error)) => {
            upstream_error(port_error)
        }
        SubmitEstimateToolError::Submit(SubmitEstimateError::ConcurrentUpdate) => {
            ToolErrorPayload::retryable(
                crate::tools::ToolErrorCode::UpstreamUnavailable,
                None,
                "the estimate could not be stored right now",
            )
        }
        SubmitEstimateToolError::Ledger(port_error)
        | SubmitEstimateToolError::Storage(port_error) => upstream_error(port_error),
    }
}

impl<A, S, H, X, G, O, C> EstimateTool<A, S, H, X, G, O, C>
where
    A: EstimateAssignmentRepository,
    S: EstimateSubmissionRepository,
    H: ToolCallHistory,
    X: EstimateRunContextSource,
    G: ProfileSource,
    O: ScorecardObservationRepository,
    C: Clock,
{
    async fn run(
        &self,
        run: &AiRunId,
        args: &Map<String, Value>,
    ) -> Result<Value, ToolErrorPayload> {
        let ports = self.ports.as_ref();
        match self.kind {
            EstimateToolKind::GetTask => {
                let input = GetEstimateTaskInput::new(parse_task_id(args)?);
                let view = GetEstimateTask::new(ports.assignments.as_ref())
                    .execute(run, &input)
                    .await
                    .map_err(|error| match error {
                        GetEstimateTaskError::UnknownTask | GetEstimateTaskError::WrongRun => {
                            unknown_task()
                        }
                        GetEstimateTaskError::Storage(port_error) => upstream_error(&port_error),
                    })?;
                Ok(render_task(&view))
            }
            EstimateToolKind::Submit => self.submit(run, args).await,
            EstimateToolKind::Profile => {
                let view = GetProfile::new(&ReadOnlySettings::new(ports.settings.as_ref()))
                    .execute()
                    .await
                    .map_err(|error| upstream_error(&error))?;
                Ok(render_profile(&view))
            }
            EstimateToolKind::Scorecard => self.scorecard().await,
        }
    }

    async fn submit(
        &self,
        run: &AiRunId,
        args: &Map<String, Value>,
    ) -> Result<Value, ToolErrorPayload> {
        let ports = self.ports.as_ref();
        let arguments = parse_submit_estimate_arguments(args)?;
        let task_id = arguments.task_id.as_str().to_owned();
        let context = ports
            .context
            .estimate_context(run)
            .await
            .map_err(|error| upstream_error(&error))?;
        if &context.run != run {
            return Err(unknown_task());
        }
        SubmitEstimateTool::new(
            ports.submissions.as_ref(),
            ports.history.as_ref(),
            ports.clock.as_ref(),
            ports.events.as_ref(),
        )
        .execute(context, arguments)
        .await
        .map_err(|error| submit_failure(&error))?;
        Ok(object(vec![
            ("task_id", Value::String(task_id)),
            ("recorded", json!("estimate_only_no_order")),
        ]))
    }

    async fn scorecard(&self) -> Result<Value, ToolErrorPayload> {
        let ports = self.ports.as_ref();
        let view = GetScorecard::new(ports.observations.as_ref(), ports.clock.as_ref())
            .execute(ScorecardSelection::All)
            .await
            .map_err(|error| match error {
                GetScorecardError::Storage(port_error) => upstream_error(&port_error),
                GetScorecardError::Calculation(_) => ToolErrorPayload::retryable(
                    crate::tools::ToolErrorCode::UpstreamUnavailable,
                    None,
                    "the scorecard could not be computed from stored records",
                ),
            })?;
        Ok(match view {
            ScorecardView::NoObservations => object(vec![
                ("cards", json!([])),
                ("status", json!("no_judged_estimates_yet")),
            ]),
            ScorecardView::Cards(cards) => render_cards(&cards),
            ScorecardView::Single(card) => {
                object(vec![("cards", Value::Array(vec![render_card(&card)]))])
            }
            ScorecardView::UnknownCombination => object(vec![("cards", json!([]))]),
        })
    }
}

impl<A, S, H, X, G, O, C> McpTool for EstimateTool<A, S, H, X, G, O, C>
where
    A: EstimateAssignmentRepository + 'static,
    S: EstimateSubmissionRepository + 'static,
    H: ToolCallHistory + 'static,
    X: EstimateRunContextSource + 'static,
    G: ProfileSource + 'static,
    O: ScorecardObservationRepository + 'static,
    C: Clock + 'static,
{
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move { self.run(invocation.run_id, invocation.arguments).await })
    }
}

pub fn register_estimate_tools<A, S, H, X, G, O, C>(
    registry: &mut ToolRegistry,
    ports: &Arc<EstimateToolPorts<A, S, H, X, G, O, C>>,
) -> Result<(), ToolRegistryError>
where
    A: EstimateAssignmentRepository + 'static,
    S: EstimateSubmissionRepository + 'static,
    H: ToolCallHistory + 'static,
    X: EstimateRunContextSource + 'static,
    G: ProfileSource + 'static,
    O: ScorecardObservationRepository + 'static,
    C: Clock + 'static,
{
    let mut task_props = Map::new();
    task_props.insert(
        String::from("task_id"),
        json!({ "type": "string", "minLength": 1_u64, "maxLength": 128_u64 }),
    );
    let specs = [
        (
            EstimateToolKind::GetTask,
            "get_estimate_task",
            "Read one estimate task the app prepared for this run: the event (target return before stop loss within the horizon), the base probability, and the reference price and time. Input is task_id only.",
            ToolAccessKind::ReadOnly,
            build_object_schema(task_props, &["task_id"]),
        ),
        (
            EstimateToolKind::Submit,
            SUBMIT_ESTIMATE_TOOL,
            "Submit a probability estimate for one task: task_id, integer p from 1 to 99, rationale, strongest counterargument, and sources as tool:<request id> of successful calls in this run. This records an estimate only; it never creates an order. The app decides any signal.",
            ToolAccessKind::StateChanging,
            submit_estimate_schema(),
        ),
        (
            EstimateToolKind::Profile,
            "get_profile",
            "Read the user's investment profile and principles: horizon, risk appetite, weight caps and exclusions.",
            ToolAccessKind::ReadOnly,
            build_object_schema(Map::new(), &[]),
        ),
        (
            EstimateToolKind::Scorecard,
            "get_scorecard",
            "Read past estimate outcomes computed by the app per model and horizon: judged count, Brier score and reliability. Combinations with fewer than 30 judged estimates are marked pending.",
            ToolAccessKind::ReadOnly,
            build_object_schema(Map::new(), &[]),
        ),
    ];
    for (kind, name, description, access, schema) in specs {
        let tool_name =
            ToolName::try_new(name).map_err(|_err| ToolRegistryError::ForbiddenToolName)?;
        let spec = ToolSpec::try_new(tool_name, description, access, schema, None)?;
        registry.register(Arc::new(EstimateTool {
            kind,
            spec,
            ports: Arc::clone(ports),
        }))?;
    }
    Ok(())
}
