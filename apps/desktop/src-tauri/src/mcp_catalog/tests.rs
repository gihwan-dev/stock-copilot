use super::{ANALYSIS_MCP_SERVER, CatalogStores, allowed_tool_names, analysis_catalog};
use application::{
    AiRunId, EstimateAssignmentRepository, EstimateCommitOutcome, EstimateSubmissionRepository,
    EventBusBuilder, ReasoningEffort, ScorecardObservationRepository, SystemPortError,
};
use domain::{EstimateAssignments, EstimateSubmissionRecord, InstrumentId, ScorecardObservation};
use mcp_server::ToolRegistry;
use serde_json::Value;
use std::future::{Future, ready};
use std::sync::Arc;
use testkit_ports::tool_read_fakes::LatestThesisRepository;

struct ClosedEstimates;

fn closed() -> SystemPortError {
    SystemPortError::new(
        application::SystemPort::Repository,
        application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
    )
}

impl EstimateAssignmentRepository for ClosedEstimates {
    fn load_assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn commit_assignments(
        &self,
        _previous: &EstimateAssignments<AiRunId>,
        _next: &EstimateAssignments<AiRunId>,
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

impl EstimateSubmissionRepository for ClosedEstimates {
    fn submission_records(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send {
        ready(Err(closed()))
    }

    fn assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn commit_submission(
        &self,
        _previous: &EstimateAssignments<AiRunId>,
        _next: &EstimateAssignments<AiRunId>,
        _records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

impl application::ToolCallHistory for ClosedEstimates {
    fn calls_in_run(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<Vec<application::ToolCallRecord>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }
}

impl application::SettingsRepository for ClosedEstimates {
    fn store_profile(
        &self,
        _profile: &domain::InvestorProfile,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<domain::InvestorProfile>, SystemPortError>> + Send {
        ready(Ok(None))
    }

    fn store_risk_settings(
        &self,
        _settings: &domain::RiskSettings,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn risk_settings(
        &self,
    ) -> impl Future<Output = Result<Option<domain::RiskSettings>, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

impl application::CollectedItemReads for ClosedEstimates {
    fn collected_since(
        &self,
        _kind: application::CollectedKind,
        _since: domain::Timestamp,
    ) -> impl Future<Output = Result<Vec<application::ObservedCollectedItem>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }

    fn collected_item(
        &self,
        _handle: &application::CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<application::ObservedCollectedItem>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }
}

impl application::NewsDataSettingsRepository for ClosedEstimates {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<application::NewsDataSettings>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }

    fn store(
        &self,
        _settings: &application::NewsDataSettings,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

impl application::WatchlistRepository for ClosedEstimates {
    fn load(&self) -> impl Future<Output = Result<application::Watchlist, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn store(
        &self,
        _previous: &application::Watchlist,
        _next: &application::Watchlist,
    ) -> impl Future<Output = Result<application::WatchlistStoreOutcome, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }
}

impl application::AuditLog for ClosedEstimates {
    fn append(
        &self,
        _entry: &application::AuditEntry,
    ) -> impl Future<Output = Result<application::RecordOutcome, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn verify(
        &self,
    ) -> impl Future<Output = Result<application::AuditIntegrity, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

impl application::ChatRepository for ClosedEstimates {
    fn ensure_session(
        &self,
        _session: &application::ChatSessionId,
        _now: domain::Timestamp,
    ) -> impl Future<Output = Result<application::ChatSessionRecord, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn load_session(
        &self,
        _session: &application::ChatSessionId,
    ) -> impl Future<Output = Result<Option<application::ChatSessionRecord>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }

    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<application::ChatSessionSummary>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }

    fn append_entry(
        &self,
        _session: &application::ChatSessionId,
        _entry: &application::StoredChatEntry,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        ready(Err(closed()))
    }

    fn load_user_message(
        &self,
        _session: &application::ChatSessionId,
        _message_id: &application::UserMessageId,
    ) -> impl Future<Output = Result<Option<application::StoredUserMessage>, SystemPortError>> + Send
    {
        ready(Err(closed()))
    }
}

impl application::ChatRunLedger for ClosedEstimates {
    fn record_chat_run(
        &self,
        _entry: &application::ChatRunLedgerEntry,
    ) -> impl Future<Output = Result<application::RecordOutcome, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

struct NoBodies;

impl crate::mcp_sources::ArticleBodies for NoBodies {
    fn fetch(
        &self,
        _source: &domain::NewsSource,
        _url: &domain::NormalizedUrl,
        _settings: &application::NewsDataSettings,
    ) -> impl Future<Output = Result<domain::UntrustedText, application::ArticleFetchFailure>> + Send
    {
        ready(Err(application::ArticleFetchFailure::NotFound))
    }
}

impl ScorecardObservationRepository for ClosedEstimates {
    fn scorecard_observations(
        &self,
    ) -> impl Future<Output = Result<Vec<ScorecardObservation>, SystemPortError>> + Send {
        ready(Err(closed()))
    }
}

fn catalog() -> ToolRegistry {
    let clock = Arc::new(testkit_ports::SteppedClock::new(
        domain::Timestamp::from_parts(
            domain::UnixSeconds::new(1_790_000_000_i64),
            domain::Nanosecond::new(0_u32),
            domain::OffsetSeconds::new(0_i32),
        )
        .expect("시각"),
    ));
    analysis_catalog(
        &clock,
        CatalogStores {
            theses: Arc::new(LatestThesisRepository::default()),
            estimates: Arc::new(ClosedEstimates),
            bodies: Arc::new(NoBodies),
            collector:
                Arc::new(
                    crate::ai_runtime::AiAvailability::<
                        testkit_ports::chat_fakes::ScriptedAiProvider,
                    >::Missing,
                ),
            runs: crate::ai_runtime::RunMetadataBook::new(),
            turns: crate::ai_runtime::ChatTurnBook::new(),
            events: Arc::new(EventBusBuilder::default().build()),
            market: crate::broker_link::BrokerMarketTools::unconnected(),
            portfolio: crate::mcp_portfolio::CatalogPortfolio::default(),
        },
    )
    .expect("제품 카탈로그")
}

#[test]
fn product_catalog_serves_exactly_the_documented_tools_list() {
    let actual = serde_json::to_value(catalog().list_rmcp_tools()).expect("직렬화");
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string("../../../fixtures/mcp/tools_list.json").expect("골든"),
    )
    .expect("골든 JSON");
    assert_eq!(actual, golden);
}

#[test]
fn allowed_tools_are_the_catalog_under_the_app_server_name_only() {
    let registry = catalog();
    let allowed = allowed_tool_names(ANALYSIS_MCP_SERVER, &registry);
    assert_eq!(allowed.len(), registry.tool_names().len());
    assert_eq!(allowed.len(), 20_usize);
    for name in &allowed {
        let tool = name
            .strip_prefix("mcp__stock__")
            .expect("앱 MCP 서버 도구만");
        for forbidden in [
            "order", "approve", "approval", "setting", "secret", "key", "token",
        ] {
            assert!(!tool.contains(forbidden), "{name}");
        }
    }
    for expected in [
        "mcp__stock__get_portfolio",
        "mcp__stock__get_estimate_task",
        "mcp__stock__submit_estimate",
        "mcp__stock__update_thesis",
        "mcp__stock__request_web_research",
        "mcp__stock__read_article",
    ] {
        assert!(allowed.contains(expected), "{expected}");
    }
}

#[test]
fn profile_tool_reads_the_saved_investor_profile_from_settings() {
    use application::ProfileSource;
    let stored = super::StoredProfile(Arc::new(ClosedEstimates));
    assert_eq!(tauri::async_runtime::block_on(stored.profile()), Ok(None));
}
