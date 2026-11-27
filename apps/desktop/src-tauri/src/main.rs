use application::{AiProvider, AiRunId, ChatHistory, InformationError, RunSpec};
use src_tauri::ai_commands::{
    AppCommandDeps, AppCommandSlot, AuditedAiJournal, register_app_commands,
};
use src_tauri::ai_runtime::{
    AiAvailability, AttachableAi, BridgeLocation, ChatTurnBook, LateAi, McpAttachedAi, McpRunLink,
    McpServerHost, McpServerHostParts, RunMetadataBook,
};
use src_tauri::audit_queue::AuditEventQueue;
use src_tauri::broker_link::{AccountSelector, BrokerMarketTools};
use src_tauri::collected_links::{CollectedLinker, TitleLinker};
use src_tauri::estimate_flow::{
    BackgroundQuota, CalendarDays, EstimateFlow, EstimateFlowParts, EstimateFlowSlot,
    EstimateRunBook, ProductEstimateFlow, ProductMaterials, UnavailableEstimateFlow,
};
use src_tauri::holdings_link::{
    HoldingsListener, InstrumentDirectory, InstrumentMasterFeed, MasterRefresh, PortsHoldingsSync,
    SavedMaster,
};
use src_tauri::ipc::ai_settings::{AiControls, AiSettingsIpcState, BudgetApplied};
use src_tauri::ipc::briefing::BriefingIpcState;
use src_tauri::ipc::instruments::InstrumentsIpcState;
use src_tauri::ipc::news_settings::NewsSettingsIpcState;
use src_tauri::ipc::open_external::ExternalLinkRuntime;
use src_tauri::ipc::order_approval::OrderApprovalRuntime;
use src_tauri::ipc::public_ip::{LookupFailed, PublicIpLookup, PublicIpRuntime, TextClipboard};
use src_tauri::ipc::scorecard::ScorecardIpcState;
use src_tauri::ipc::self_check::{AiLimit, AiStatusProbe, ClaudeInstall, SelfCheckIpcState};
use src_tauri::ipc::signals::{MarketCalendars, SignalsIpcState};
use src_tauri::ipc::state::BoxFuture;
use src_tauri::ipc::system_data::{DataCare, ExportTarget, SystemDataIpcState};
use src_tauri::ipc::thesis::ThesisIpcState;
use src_tauri::mcp_catalog::{
    ANALYSIS_MCP_SERVER, CatalogStores, allowed_tool_names, analysis_catalog,
};
use src_tauri::mcp_portfolio::{CatalogPortfolio, StoredPortfolioTools};
use src_tauri::mcp_sources::ArticleBodies;
use src_tauri::order_runtime::{LedgerTxPort, SubmissionLedger, forwarding_bus};
use src_tauri::platform::collector::{CollectorFeed, CollectorFeeds, SharedFeed};
use src_tauri::platform::intraday::{BridgedIntradayWatch, IntradayWatchSlot};
use src_tauri::platform::notifier::{
    AsyncRuntimeNotificationDispatch, FilteredNotifier, NotificationDelivery,
    NotificationEventSubscriber, NotificationPreferences,
};
use src_tauri::platform::peer_process::PsPeerInspector;
use src_tauri::platform::resolution::{ResolutionData, ResolutionDataSlot, ResolutionSource};
use src_tauri::platform::scheduling::SpecWatchInputs;
use src_tauri::platform::single_instance::{
    InstanceAcquisition, InstanceSlot, SocketPath, attach_to_app,
};
use src_tauri::platform::system_notification::{
    NotificationHandleSlot, SystemNotificationDelivery,
};
use src_tauri::product_events::{
    BridgedUiEvents, FlushingAuditSink, RecordedTriggerUrgency, UiEventSlot, ui_event_kinds,
};
use src_tauri::signal_flow::{
    BridgedSignalPipeline, FlowReestimate, OrderContexts, ProductSignalData, ProductSignalPipeline,
    ProposalInstruments, SignalDataParts, SignalPipelineSlot,
};
use src_tauri::toss_link::{
    ProbeFailure, ProbeResult, SharedClock, SharedSecrets, TossLink, TossLinkState, TossProbe,
};
use src_tauri::updates;
use src_tauri::updates::tauri_source::{TauriUpdateSource, configured_pubkey};
use src_tauri::{build_app, run_app};
use src_tauri::{
    database::{DatabaseFactory, DatabaseKeyProvision},
    ports::Ports,
    product_ports::ProductPorts,
    unavailable::Unavailable,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use store_sqlite::{SharedDatabase, SqliteRepositories};

fn product_context() -> tauri::Context<tauri::Wry> {
    #[expect(
        clippy::disallowed_macros,
        reason = "tauri::generate_context! expands to include_bytes! and env!"
    )]
    #[expect(
        clippy::disallowed_methods,
        reason = "tauri::generate_context! expands to std::process::exit"
    )]
    #[expect(
        clippy::disallowed_types,
        reason = "tauri::generate_context! expands to std::collections::HashMap"
    )]
    let context = tauri::generate_context!();
    context
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = product_context();
    let clock = platform_macos::SystemClock::new()
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let shared_clock = Arc::new(clock);
    let diagnostics = product_diagnostics(&context.config().identifier, &shared_clock);
    let repositories = Arc::new(SqliteRepositories::new(SharedDatabase::new()));
    let notification_handle = Arc::new(NotificationHandleSlot::<tauri::Wry>::default());
    let notifier = product_notifier(&notification_handle);
    let ui_events = Arc::new(UiEventSlot::<tauri::Wry>::default());
    let intraday = Arc::new(IntradayWatchSlot::new());
    let signals = Arc::new(SignalPipelineSlot::default());
    let (events, scheduler_events) = product_event_bus(ProductSubscribers {
        notifier: Arc::clone(&notifier),
        audit: Arc::clone(&repositories),
        clock: Arc::clone(&shared_clock),
        ui_events: Arc::clone(&ui_events),
        intraday: Arc::clone(&intraday),
        signals: Arc::clone(&signals),
    });
    let toss = product_toss(&shared_clock, &events, &repositories)?;
    let (turns, app_commands) = (ChatTurnBook::new(), Arc::new(AppCommandSlot::default()));
    let (directory, estimate_runs) = (InstrumentDirectory::default(), EstimateRunBook::new());
    let ai = assemble_ai(
        discover_ai_setup(
            &context.config().identifier,
            bundled_bridge(),
            (turns.clone(), Arc::clone(&app_commands)),
        )
        .with_market(market_tools(&toss, &shared_clock, &directory))
        .with_portfolio(catalog_portfolio(&toss, &repositories, &directory))
        .with_estimate_runs(estimate_runs.clone()),
        &shared_clock,
        &events,
        &repositories,
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let self_check = product_self_check(&ai, &repositories);
    let ports = product_ports(&repositories, (notifier, &toss), &shared_clock, events, ai);
    let holdings = holdings_listener(&ports, &toss, &repositories, &directory, &shared_clock);
    let estimates = product_estimate_flow(&ports, &toss, (&directory, estimate_runs), &signals);
    let app = build_app(
        with_estimates(
            product_builder(scheduler_events, &repositories, ui_events, &shared_clock),
            estimates,
        )
        .manage(notification_handle)
        .manage(diagnostics)
        .manage(market_tools(&toss, &shared_clock, &directory))
        .manage(directory)
        .manage(intraday)
        .manage(turns)
        .manage(app_commands)
        .manage(toss_link_state(&toss, holdings.clone(), &shared_clock))
        .manage(holdings)
        .manage(self_check)
        .manage_screens(&toss, &repositories, &shared_clock, &ports),
        ports,
        context,
    )?;
    run_primary(app)
}

fn product_diagnostics(
    identifier: &str,
    clock: &Arc<platform_macos::SystemClock>,
) -> Option<Arc<src_tauri::diagnostics_log::DiagnosticsLog>> {
    let directory = PathBuf::from(std::env::var_os("HOME")?)
        .join("Library")
        .join("Application Support")
        .join(identifier);
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    let log = src_tauri::diagnostics_log::DiagnosticsLog::start(directory, shared);
    let sink: Arc<dyn application::DiagnosticsSink> =
        Arc::<src_tauri::diagnostics_log::DiagnosticsLog>::clone(&log);
    let _installed = broker_toss::install_diagnostics(sink);
    src_tauri::diagnostics_log::install(&log);
    Some(log)
}

fn product_notifier(handle: &Arc<NotificationHandleSlot<tauri::Wry>>) -> Arc<ProductNotifier> {
    Arc::new(FilteredNotifier::new(
        SystemNotificationDelivery::new(Arc::clone(handle)),
        NotificationPreferences::default(),
    ))
}

trait ManageScreens {
    fn manage_screens(
        self,
        toss: &ProductToss,
        repositories: &Arc<SqliteRepositories>,
        clock: &Arc<platform_macos::SystemClock>,
        ports: &Ports<AppPortTypes>,
    ) -> Self;
}

impl ManageScreens for tauri::Builder<tauri::Wry> {
    fn manage_screens(
        self,
        toss: &ProductToss,
        repositories: &Arc<SqliteRepositories>,
        clock: &Arc<platform_macos::SystemClock>,
        ports: &Ports<AppPortTypes>,
    ) -> Self {
        let events = &ports.events;
        self.manage(AiSettingsIpcState::new(
            Arc::clone(repositories),
            Arc::clone(events),
            Arc::new(QueueControls(Arc::clone(&ports.chat_queue))),
        ))
        .manage(InstrumentsIpcState::new(
            Arc::<ProductBroker>::clone(&toss.broker),
            Arc::clone(repositories),
            Arc::<platform_macos::SystemClock>::clone(clock),
            Arc::clone(events),
        ))
        .manage(Arc::clone(&toss.performance))
        .manage(ThesisIpcState::from_repository(Arc::clone(repositories)))
        .manage(
            SignalsIpcState::new(
                Arc::clone(repositories),
                Arc::new(TossResolutionSource(Arc::<ProductBroker>::clone(
                    &toss.broker,
                ))),
                Arc::<platform_macos::SystemClock>::clone(clock),
            )
            .with_proposals(Arc::new(StoredCurrentProposals(Arc::clone(repositories)))),
        )
        .manage(
            NewsSettingsIpcState::new(
                Arc::clone(repositories),
                Arc::clone(&toss.secrets),
                Arc::clone(events),
            )
            .with_routes(Arc::new(StoredRoutes(Arc::clone(repositories)))),
        )
        .manage(SystemDataIpcState::new(
            AccountSelector::new(
                Arc::<ProductBroker>::clone(&toss.broker),
                toss.accounts.clone(),
                Arc::clone(events),
            ),
            toss.accounts.clone(),
            (
                Arc::new(ProductDataCare {
                    repositories: Arc::clone(repositories),
                    secrets: Arc::clone(&toss.secrets),
                    clock: Arc::<platform_macos::SystemClock>::clone(clock),
                    events: Arc::clone(events),
                }),
                Arc::new(SavePanelTarget),
            ),
            Arc::clone(repositories),
        ))
    }
}

const DATABASE_FILES: [&str; 3] = [
    "stock-copilot.db",
    "stock-copilot.db-wal",
    "stock-copilot.db-shm",
];

const WIPED_SECRETS: [application::SecretKey; 7] = [
    application::SecretKey::TossClientId,
    application::SecretKey::TossClientSecret,
    application::SecretKey::TossAccessToken,
    application::SecretKey::DatabaseEncryptionKey,
    application::SecretKey::AuditHmacKey,
    application::SecretKey::DartApiKey,
    application::SecretKey::FredApiKey,
];

struct ProductDataCare {
    repositories: Arc<SqliteRepositories>,
    secrets: Arc<ProductSecrets>,
    clock: Arc<platform_macos::SystemClock>,
    events: Arc<application::EventBus>,
}

fn storage_failure() -> application::SystemPortError {
    application::SystemPortError::new(
        application::SystemPort::Repository,
        application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
    )
}

fn remove_stored_files(data_directory: &Path) -> Result<(), application::SystemPortError> {
    let files = DATABASE_FILES.map(|name| data_directory.join(name));
    for file in files.iter().filter(|file| file.exists()) {
        std::fs::remove_file(file).map_err(|_locked| storage_failure())?;
    }
    let backups = data_directory.join("backups");
    if backups.exists() {
        std::fs::remove_dir_all(&backups).map_err(|_locked| storage_failure())?;
    }
    Ok(())
}

const EXPORT_FILE_NAME: &str = "stock-copilot-backup.db";

struct SavePanelTarget;

impl ExportTarget for SavePanelTarget {
    fn choose(&self) -> BoxFuture<'_, Option<PathBuf>> {
        Box::pin(async move {
            tauri::async_runtime::spawn_blocking(|| {
                platform_macos::pick_save_path(EXPORT_FILE_NAME)
            })
            .await
            .ok()
            .flatten()
        })
    }
}

fn copy_staged_backup(staging: &Path, target: &Path) -> Result<(), application::SystemPortError> {
    let staged = std::fs::read_dir(staging)
        .map_err(|_unreadable| storage_failure())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_file())
        .ok_or_else(storage_failure)?;
    std::fs::copy(&staged, target).map_err(|_unwritable| storage_failure())?;
    std::fs::remove_dir_all(staging).map_err(|_locked| storage_failure())
}

impl DataCare for ProductDataCare {
    fn export_to(
        &self,
        staging: PathBuf,
        target: PathBuf,
    ) -> BoxFuture<'_, Result<(), application::SystemPortError>> {
        Box::pin(async move {
            if staging.exists() {
                std::fs::remove_dir_all(&staging).map_err(|_locked| storage_failure())?;
            }
            self.back_up(staging.clone()).await?;
            copy_staged_backup(&staging, &target)
        })
    }

    fn back_up(
        &self,
        directory: PathBuf,
    ) -> BoxFuture<'_, Result<(), application::SystemPortError>> {
        Box::pin(async move {
            use application::SecretStore as _;
            let key = self
                .secrets
                .load(application::SecretKey::DatabaseEncryptionKey)
                .await?
                .ok_or_else(storage_failure)?;
            self.repositories
                .back_up(directory, &key, self.clock.as_ref())
                .await
                .map(|_outcome| ())
        })
    }

    fn wipe(
        &self,
        data_directory: PathBuf,
    ) -> BoxFuture<'_, Result<(), application::SystemPortError>> {
        Box::pin(async move {
            use application::SecretStore as _;
            let _closed = self.repositories.shared().close().await?;
            remove_stored_files(&data_directory)?;
            for key in WIPED_SECRETS {
                let _removed = self.secrets.remove(key).await?;
            }
            let _delivery = self.events.publish(domain::DomainEvent::StoredDataChanged);
            Ok(())
        })
    }
}

struct QueueControls(Arc<ProductQueue>);

impl AiControls for QueueControls {
    fn apply_budget(
        &self,
        percent: application::ScheduledBudgetPercent,
    ) -> BoxFuture<'_, BudgetApplied> {
        Box::pin(async move {
            let cap = match percent {
                application::ScheduledBudgetPercent::Fifty => 50,
                application::ScheduledBudgetPercent::Sixty => 60,
                application::ScheduledBudgetPercent::Seventy => 70,
                application::ScheduledBudgetPercent::Eighty => 80,
                application::ScheduledBudgetPercent::Ninety => 90,
            };
            match self.0.set_scheduled_cap_percent(cap).await {
                Ok(_transition) => BudgetApplied::Applied,
                Err(_invalid) => BudgetApplied::Refused,
            }
        })
    }

    fn usage(&self) -> BoxFuture<'_, application::AiUsage> {
        Box::pin(self.0.current_usage())
    }
}

struct QueueQuota(Arc<ProductQueue>);

impl BackgroundQuota for QueueQuota {
    fn admission(&self) -> BoxFuture<'_, application::CycleAdmission> {
        Box::pin(async move {
            if self.0.snapshot().await.active_chat.is_some() {
                return application::CycleAdmission::Paused { resumes_at: None };
            }
            match self.0.evaluate_scheduled_quota().await {
                ai_claude_code::QuotaDecision::Allowed => application::CycleAdmission::Admitted,
                ai_claude_code::QuotaDecision::WaitScheduledBudget { .. } => {
                    application::CycleAdmission::Paused { resumes_at: None }
                }
                ai_claude_code::QuotaDecision::WaitRateLimit { resumes_at, .. } => {
                    application::CycleAdmission::Paused {
                        resumes_at: Some(resumes_at),
                    }
                }
            }
        })
    }
}

const ESTIMATE_MODEL_PROVIDER: &str = "claude-code";
const ESTIMATE_MODEL_NAME: &str = "default";
const ESTIMATE_CLI_VERSION: &str = "cli";
const ESTIMATE_PROMPT_ID: &str = "estimate";
const ESTIMATE_PROMPT_VERSION: &str = "v1";

fn estimate_identity() -> Option<application::EstimateRunIdentity> {
    use application::PromptHasher as _;
    let part = |value: &str| domain::ModelIdentityPart::try_new(value).ok();
    let digest = store_sqlite::Sha256PromptHasher
        .digest(&application::estimate_prompt_source())
        .ok()?;
    Some(application::EstimateRunIdentity {
        model: domain::CalibrationModel::new(
            part(ESTIMATE_MODEL_PROVIDER)?,
            part(ESTIMATE_MODEL_NAME)?,
            part(ESTIMATE_CLI_VERSION)?,
        ),
        cli_version: part(ESTIMATE_CLI_VERSION)?,
        prompt: domain::EstimatePrompt {
            id: domain::ScorecardPromptId::try_new(ESTIMATE_PROMPT_ID).ok()?,
            version: part(ESTIMATE_PROMPT_VERSION)?,
            hash: domain::PromptHash::try_new(digest.as_str()).ok()?,
        },
    })
}

struct ToolCandleFeed(Arc<ProductBroker>);

impl src_tauri::broker_link::ToolQuoteSource for ToolCandleFeed {
    fn quotes(
        &self,
        instruments: Vec<domain::QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<application::QuoteObservation>, application::PortError>> {
        Box::pin(async move { application::BrokerRead::quotes(&*self.0, &instruments).await })
    }

    fn daily_candles(
        &self,
        instrument: domain::QuoteInstrument,
        window: domain::TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<domain::Candle>, application::PortError>> {
        Box::pin(async move { self.0.history_daily_candles(&instrument, window).await })
    }

    fn adjusted_daily_candles(
        &self,
        instrument: domain::QuoteInstrument,
        window: domain::TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<domain::Candle>, application::PortError>> {
        Box::pin(async move {
            application::BrokerRead::candles(
                &*self.0,
                &instrument,
                window,
                domain::CandlePeriod::Daily,
                domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(window.end())),
            )
            .await
        })
    }
}

fn market_tools(
    toss: &ProductToss,
    clock: &Arc<platform_macos::SystemClock>,
    directory: &InstrumentDirectory,
) -> BrokerMarketTools {
    BrokerMarketTools::connected(Arc::new(ToolCandleFeed(Arc::clone(&toss.broker))))
        .with_market(Arc::<ProductBroker>::clone(&toss.broker))
        .with_names(directory.clone())
        .with_clock(Arc::<platform_macos::SystemClock>::clone(clock))
}

type ProductCalendars = ResolutionData<TossResolutionSource>;

fn product_calendars(toss: &ProductToss) -> Arc<ProductCalendars> {
    Arc::new(ResolutionData::new(TossResolutionSource(Arc::clone(
        &toss.broker,
    ))))
}

fn product_estimate_flow(
    ports: &Ports<AppPortTypes>,
    toss: &ProductToss,
    (directory, runs): (&InstrumentDirectory, EstimateRunBook),
    signals: &SignalPipelineSlot,
) -> (
    EstimateFlowSlot,
    Arc<ProductCalendars>,
    Option<OrderApprovalRuntime>,
) {
    let calendars = product_calendars(toss);
    let Some(identity) = estimate_identity() else {
        return (
            EstimateFlowSlot(Arc::new(UnavailableEstimateFlow)),
            calendars,
            None,
        );
    };
    let days: Arc<dyn CalendarDays> = Arc::<ProductCalendars>::clone(&calendars);
    let materials = ProductMaterials {
        broker: Arc::clone(&toss.broker),
        settings: Arc::clone(&ports.settings),
        directory: directory.clone(),
        accounts: toss.accounts.clone(),
        days: Arc::clone(&days),
    };
    let flow: Arc<dyn EstimateFlow> = Arc::new(ProductEstimateFlow::new(EstimateFlowParts {
        materials,
        store: Arc::clone(&ports.settings),
        portfolios: Arc::clone(&ports.portfolios),
        ai: Arc::clone(&ports.ai_provider),
        quota: Arc::new(QueueQuota(Arc::clone(&ports.chat_queue))),
        clock: Arc::clone(&ports.clock),
        runs,
        identity,
    }));
    let pipeline = Arc::new(ProductSignalPipeline {
        data: ProductSignalData::new(SignalDataParts {
            broker: Arc::clone(&toss.broker),
            store: Arc::clone(&ports.settings),
            portfolios: Arc::clone(&ports.portfolios),
            directory: directory.clone(),
            accounts: toss.accounts.clone(),
            days,
        }),
        store: Arc::clone(&ports.settings),
        clock: Arc::clone(&ports.clock),
        events: Arc::clone(&ports.events),
        reestimate: FlowReestimate(Arc::clone(&flow)),
    });
    signals.bind(Arc::<ProductPipeline>::clone(&pipeline));
    let orders = order_approval_runtime(ports, toss, Some(pipeline));
    (EstimateFlowSlot(flow), calendars, orders)
}

type ProductPipeline = ProductSignalPipeline<
    ProductBroker,
    SqliteRepositories,
    store_sqlite::SqlitePortfolioStore,
    platform_macos::SystemClock,
>;

struct StoredProposalInstruments(Arc<SqliteRepositories>);

impl ProposalInstruments for StoredProposalInstruments {
    fn instrument_of(
        &self,
        proposal: &domain::ProposalId,
    ) -> BoxFuture<'_, Result<Option<domain::InstrumentId>, application::SystemPortError>> {
        let owned = proposal.clone();
        Box::pin(async move { store_sqlite::proposal_instrument(self.0.as_ref(), &owned).await })
    }
}

struct StoredCurrentProposals(Arc<SqliteRepositories>);

impl src_tauri::ipc::signals::CurrentProposals for StoredCurrentProposals {
    fn current(
        &self,
        instrument: &domain::InstrumentId,
    ) -> BoxFuture<'_, Result<Option<domain::ProposalId>, application::SystemPortError>> {
        let owned = instrument.clone();
        Box::pin(async move { store_sqlite::current_proposal(self.0.as_ref(), &owned).await })
    }
}

struct StoredSubmissionLedger(Arc<SqliteRepositories>);

impl SubmissionLedger for StoredSubmissionLedger {
    fn commit_prepared<'a>(
        &'a self,
        proposal: &'a domain::ProposalId,
        client_order_id: &'a domain::ClientOrderId,
        prepared_at: domain::Timestamp,
    ) -> BoxFuture<'a, Result<application::ConsumptionOutcome, application::SystemPortError>> {
        Box::pin(store_sqlite::commit_prepared_submission(
            self.0.as_ref(),
            proposal,
            client_order_id,
            prepared_at,
        ))
    }
}

type ProductOrderContexts = OrderContexts<
    ProductBroker,
    SqliteRepositories,
    store_sqlite::SqlitePortfolioStore,
    platform_macos::SystemClock,
    StoredProposalInstruments,
>;

type ProductPresence = <AppPortTypes as src_tauri::ports::PortTypes>::UserPresence;

struct ProductApprovalTypes;

impl approval_command::service::ApprovalServiceTypes for ProductApprovalTypes {
    type Broker = ProductBroker;
    type Market = ProductBroker;
    type Settings = SqliteRepositories;
    type Rates = SqliteRepositories;
    type Proposals = SqliteRepositories;
    type Consumption = SqliteRepositories;
    type Reports = SqliteRepositories;
    type Audit = SqliteRepositories;
    type Contexts = ProductOrderContexts;
    type Redrafter = ProductOrderContexts;
    type Freeze = approval_command::ports::NeverFrozen;
    type Clock = platform_macos::SystemClock;
    type Presence = ProductPresence;
    type TxPort = LedgerTxPort;
    type Trade = broker_toss_trade::TossBrokerTrade<ProductBroker>;
}

fn with_estimates(
    builder: tauri::Builder<tauri::Wry>,
    (flow, calendars, runtime): (
        EstimateFlowSlot,
        Arc<ProductCalendars>,
        Option<OrderApprovalRuntime>,
    ),
) -> tauri::Builder<tauri::Wry> {
    let managed = builder.manage(ResolutionDataSlot(calendars)).manage(flow);
    match runtime {
        Some(approvals) => managed.manage(approvals),
        None => managed,
    }
}

fn order_approval_runtime(
    ports: &Ports<AppPortTypes>,
    toss: &ProductToss,
    built: Option<Arc<ProductPipeline>>,
) -> Option<OrderApprovalRuntime> {
    let pipeline = built?;
    let trade = broker_toss_trade::TossBrokerTrade::new(
        &broker_toss::TossEndpoint::production(),
        Arc::clone(&toss.broker),
    )
    .ok()?;
    let contexts = Arc::new(OrderContexts {
        pipeline,
        proposals: Arc::new(StoredProposalInstruments(Arc::clone(&ports.settings))),
    });
    let service = approval_command::service::ApprovalService::new(
        approval_command::service::ApprovalServiceParts::<ProductApprovalTypes> {
            broker: Arc::clone(&toss.broker),
            market: Arc::clone(&toss.broker),
            settings: Arc::clone(&ports.settings),
            rates: Arc::clone(&ports.settings),
            proposals: Arc::clone(&ports.settings),
            consumption: Arc::clone(&ports.settings),
            reports: Arc::clone(&ports.settings),
            audit: Arc::clone(&ports.settings),
            contexts: Arc::clone(&contexts),
            redrafter: contexts,
            freeze: Arc::new(approval_command::ports::NeverFrozen),
            clock: Arc::clone(&ports.clock),
            presence: Arc::clone(&ports.presence),
            tx_port: Arc::new(LedgerTxPort(Arc::new(StoredSubmissionLedger(Arc::clone(
                &ports.settings,
            ))))),
            trade: Arc::new(trade),
            events: forwarding_bus(&ports.events),
        },
    );
    Some(OrderApprovalRuntime::new(Arc::new(service)))
}

struct ProductAiStatus {
    install: ClaudeInstall,
    queue: Arc<ProductQueue>,
}

impl AiStatusProbe for ProductAiStatus {
    fn install(&self) -> ClaudeInstall {
        self.install
    }

    fn limit(&self) -> BoxFuture<'_, AiLimit> {
        Box::pin(async move {
            match self.queue.evaluate_chat_quota().await {
                ai_claude_code::QuotaDecision::WaitRateLimit { resumes_at, .. } => {
                    AiLimit::Waiting(resumes_at)
                }
                ai_claude_code::QuotaDecision::Allowed
                | ai_claude_code::QuotaDecision::WaitScheduledBudget { .. } => AiLimit::Available,
            }
        })
    }
}

fn product_self_check(
    ai: &ProductAiParts,
    repositories: &Arc<SqliteRepositories>,
) -> SelfCheckIpcState {
    let install = match ai.ai.as_ref() {
        AiAvailability::Ready(_) => ClaudeInstall::Installed,
        AiAvailability::Missing => ClaudeInstall::Missing,
    };
    SelfCheckIpcState::new(
        Arc::new(ProductAiStatus {
            install,
            queue: Arc::clone(&ai.queue),
        }),
        Arc::clone(repositories),
    )
}

struct TossResolutionSource(Arc<ProductBroker>);

impl ResolutionSource for TossResolutionSource {
    fn candles<'a>(
        &'a self,
        instrument: &'a domain::QuoteInstrument,
        window: domain::TimeInterval,
        period: domain::CandlePeriod,
        adjustment: domain::PriceAdjustment,
    ) -> BoxFuture<'a, Result<Vec<domain::Candle>, application::PortError>> {
        use application::BrokerRead as _;
        Box::pin(self.0.candles(instrument, window, period, adjustment))
    }

    fn calendar_on(
        &self,
        market: domain::KnownMarket,
        date: domain::CalendarDate,
    ) -> BoxFuture<'_, Result<domain::MarketCalendar, application::PortError>> {
        Box::pin(self.0.calendar_on(market, date))
    }
}

impl MarketCalendars for TossResolutionSource {
    fn calendar_on(
        &self,
        market: domain::KnownMarket,
        date: domain::CalendarDate,
    ) -> BoxFuture<'_, Result<domain::MarketCalendar, application::PortError>> {
        Box::pin(self.0.calendar_on(market, date))
    }
}

const OPEN_COMMAND: &str = "/usr/bin/open";

struct MacUrlOpener;

fn open_url(
    url: &str,
) -> Result<application::ExternalUrlOpened, application::ExternalUrlOpenerError> {
    let status = std::process::Command::new(OPEN_COMMAND)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_error| application::ExternalUrlOpenerError::LaunchFailed)?;
    if status.success() {
        Ok(application::ExternalUrlOpened)
    } else {
        Err(application::ExternalUrlOpenerError::LaunchFailed)
    }
}

impl application::ExternalUrlOpener for MacUrlOpener {
    async fn open(
        &self,
        url: &domain::AuthorizedExternalUrl,
    ) -> Result<application::ExternalUrlOpened, application::ExternalUrlOpenerError> {
        let target = String::from(url.canonical_url().as_str());
        tauri::async_runtime::spawn_blocking(move || open_url(&target))
            .await
            .map_err(|_error| application::ExternalUrlOpenerError::LaunchFailed)?
    }
}

const PUBLIC_IP_SERVICE: &str = "https://api.ipify.org";
const CLIPBOARD_COMMAND: &str = "/usr/bin/pbcopy";

struct ServicePublicIp(Option<sources::PublicIpClient>);

impl PublicIpLookup for ServicePublicIp {
    fn lookup(&self) -> BoxFuture<'_, Result<std::net::IpAddr, LookupFailed>> {
        Box::pin(async move {
            let client = self.0.as_ref().ok_or(LookupFailed)?;
            client
                .lookup()
                .await
                .map(|address| address.as_ip())
                .map_err(|_error| LookupFailed)
        })
    }
}

struct MacClipboard;

fn write_clipboard(text: &str) -> Result<(), LookupFailed> {
    use std::io::Write;
    let mut child = std::process::Command::new(CLIPBOARD_COMMAND)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_error| LookupFailed)?;
    let written = child
        .stdin
        .take()
        .ok_or(LookupFailed)
        .and_then(|mut stdin| {
            stdin
                .write_all(text.as_bytes())
                .map_err(|_error| LookupFailed)
        });
    let status = child.wait().map_err(|_error| LookupFailed)?;
    written?;
    if status.success() {
        Ok(())
    } else {
        Err(LookupFailed)
    }
}

impl TextClipboard for MacClipboard {
    fn copy(&self, text: String) -> BoxFuture<'_, Result<(), LookupFailed>> {
        Box::pin(async move {
            tauri::async_runtime::spawn_blocking(move || write_clipboard(&text))
                .await
                .map_err(|_error| LookupFailed)?
        })
    }
}

fn public_ip_runtime(clock: &Arc<platform_macos::SystemClock>) -> PublicIpRuntime {
    let client = sources::PublicIpEndpoint::service(PUBLIC_IP_SERVICE)
        .and_then(sources::PublicIpClient::new)
        .ok();
    let shared_clock: Arc<dyn application::Clock> =
        Arc::<platform_macos::SystemClock>::clone(clock);
    PublicIpRuntime::new(
        Arc::new(ServicePublicIp(client)),
        Arc::new(MacClipboard),
        shared_clock,
    )
}

const TOSS_RECHECK_SECONDS: u64 = 3_600;

async fn restore_saved_holdings<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(holdings) = tauri::Manager::try_state::<HoldingsListener>(app) {
        let _restored = holdings.inner().restore_saved().await;
    }
    let (Some(ports), Some(repositories)) = (
        tauri::Manager::try_state::<Ports<AppPortTypes>>(app),
        tauri::Manager::try_state::<Arc<SqliteRepositories>>(app),
    ) else {
        return;
    };
    if !ports.saved_accounts.current().is_empty() {
        return;
    }
    let latest = {
        let guard = repositories.shared().read().await;
        match guard.as_ref() {
            Some(database) => store_sqlite::HoldingsStore::new(database)
                .latest_account()
                .await
                .ok()
                .flatten(),
            None => None,
        }
    };
    if let Some(account) = latest
        && ports.saved_accounts.current().is_empty()
        && ports.saved_accounts.replace(vec![account])
            == src_tauri::broker_link::SelectionChange::Changed
    {
        let _delivery = ports
            .events
            .publish(domain::DomainEvent::AccountSelectionChanged);
    }
}

fn start_toss_check(app: &tauri::AppHandle<tauri::Wry>) {
    if let Some(state) = tauri::Manager::try_state::<TossLinkState>(app) {
        let link = state.inner().clone();
        let handle = app.clone();
        let _check = tauri::async_runtime::spawn(async move {
            let _setup = src_tauri::database::wait_for_setup(&handle).await;
            restore_saved_holdings(&handle).await;
            loop {
                let _state = link.backend().check().await;
                tokio::time::sleep(std::time::Duration::from_secs(TOSS_RECHECK_SECONDS)).await;
            }
        });
    }
}

struct ProductMasterFeed<S: application::SecretStore + 'static = ProductSecrets> {
    broker: Arc<ProductBrokerFor<S>>,
    store: Arc<SqliteRepositories>,
}

impl<S: application::SecretStore + 'static> InstrumentMasterFeed for ProductMasterFeed<S> {
    fn fetch(
        &self,
        market: domain::KnownMarket,
    ) -> BoxFuture<'_, Result<domain::InstrumentMaster, application::PortError>> {
        Box::pin(async move {
            self.broker
                .instrument_master(self.store.as_ref(), self.store.as_ref(), market)
                .await
        })
    }

    fn install(&self, facts: domain::MasterInstrumentFacts) {
        self.broker.replace_market_master(facts);
    }

    fn save(&self, master: SavedMaster) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let _saved = store_sqlite::MasterSnapshots(self.store.as_ref())
                .save(master.market, master.refreshed_at, master.instruments)
                .await;
        })
    }

    fn saved(&self) -> BoxFuture<'_, Vec<SavedMaster>> {
        Box::pin(async move {
            store_sqlite::MasterSnapshots(self.store.as_ref())
                .stored()
                .await
                .map_or_else(
                    |_error| Vec::new(),
                    |stored| {
                        stored
                            .into_iter()
                            .map(|master| SavedMaster {
                                market: master.market,
                                refreshed_at: master.refreshed_at,
                                instruments: master.instruments,
                            })
                            .collect()
                    },
                )
        })
    }
}

fn holdings_listener(
    ports: &Ports<AppPortTypes>,
    toss: &ProductToss,
    repositories: &Arc<SqliteRepositories>,
    directory: &InstrumentDirectory,
    clock: &Arc<platform_macos::SystemClock>,
) -> HoldingsListener {
    let shared_clock: Arc<dyn application::Clock> =
        Arc::<platform_macos::SystemClock>::clone(clock);
    let refresh = MasterRefresh::new(
        Arc::new(ProductMasterFeed {
            broker: Arc::clone(&toss.broker),
            store: Arc::clone(repositories),
        }),
        directory.clone(),
        shared_clock,
    );
    HoldingsListener::new(
        Arc::new(AccountSelector::new(
            Arc::clone(&toss.broker),
            ports.saved_accounts.clone(),
            Arc::clone(&ports.events),
        )),
        Arc::new(refresh),
        Arc::new(PortsHoldingsSync::from_ports(ports)),
    )
}

fn installed_bundle() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let bundle = executable.parent()?.parent()?.parent()?;
    bundle
        .extension()
        .filter(|extension| *extension == "app")
        .map(|_app| bundle.to_path_buf())
}

fn prepare_login_start(app: &tauri::App<tauri::Wry>) {
    use src_tauri::platform::autostart::{
        LOGIN_ARGUMENT, LaunchAgentAutostart, LaunchMode, LoginItemsSetup,
    };
    let launched = tauri::Manager::env(app);
    let mode = if launched
        .args_os
        .iter()
        .any(|argument| argument == LOGIN_ARGUMENT)
    {
        LaunchMode::LoginBackground
    } else {
        LaunchMode::Interactive
    };
    let _mode = tauri::Manager::manage(app, mode);
    let found = (
        std::env::var_os("HOME").map(PathBuf::from),
        installed_bundle(),
        tauri::Manager::path(app).app_data_dir().ok(),
    );
    if let (Some(home), Some(bundle), Some(data_dir)) = found {
        let _setup = tauri::Manager::manage(
            app,
            LoginItemsSetup {
                port: LaunchAgentAutostart::in_home(&home, bundle),
                data_dir,
            },
        );
    }
}

fn run_primary(app: tauri::App<tauri::Wry>) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = SocketPath::try_new(
        tauri::Manager::path(&app)
            .app_data_dir()?
            .join("instance.sock"),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let single_instance = attach_to_app(app.handle(), socket_path)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    match single_instance {
        InstanceAcquisition::Primary(guard) => {
            let _managed = tauri::Manager::manage(&app, InstanceSlot::holding(guard));
            let tray = src_tauri::platform::menu_bar::install(&app);
            src_tauri::platform::residency::settle_menu_bar(app.handle(), tray);
            prepare_login_start(&app);
            start_updates(app.handle());
            start_toss_check(app.handle());
            run_app(app, |handle, code| handle.exit(code.as_i32()));
            Ok(())
        }
        InstanceAcquisition::SecondaryNotified => Ok(()),
    }
}

struct ProductToss {
    secrets: Arc<ProductSecrets>,
    broker: Arc<ProductBroker>,
    portfolios: Arc<store_sqlite::SqlitePortfolioStore>,
    accounts: src_tauri::broker_link::AccountSlot,
    sectors: Arc<src_tauri::sector_classes::SectorClasses>,
    performance: Arc<src_tauri::performance_book::PerformanceBook>,
}

fn catalog_portfolio(
    toss: &ProductToss,
    repositories: &Arc<SqliteRepositories>,
    directory: &InstrumentDirectory,
) -> CatalogPortfolio {
    CatalogPortfolio::connected(
        Arc::new(
            StoredPortfolioTools::new(
                Arc::clone(&toss.portfolios),
                (Arc::clone(repositories), Arc::clone(repositories)),
                toss.accounts.clone(),
                directory.clone(),
            )
            .with_sectors(Arc::clone(&toss.sectors))
            .with_performance(Arc::clone(&toss.performance)),
        ),
        directory.clone(),
    )
}

struct TossPerformanceFeed(Arc<ProductBroker>);

impl src_tauri::performance_book::PerformanceFeed for TossPerformanceFeed {
    fn transactions<'a>(
        &'a self,
        account: &'a domain::AccountId,
        window: domain::SettlementWindow,
    ) -> BoxFuture<'a, Result<application::TransactionRead, application::PortError>> {
        use application::BrokerRead as _;
        Box::pin(async move {
            self.0
                .transactions(account, window, &domain::TransactionFilter::all())
                .await
        })
    }

    fn calendar_on(
        &self,
        market: domain::KnownMarket,
        date: domain::CalendarDate,
    ) -> BoxFuture<'_, Result<domain::MarketCalendar, application::PortError>> {
        Box::pin(self.0.calendar_on(market, date))
    }

    fn fx_at(
        &self,
        at: domain::Timestamp,
    ) -> BoxFuture<'_, Result<domain::FxRate, application::PortError>> {
        use application::MarketInfo as _;
        Box::pin(
            self.0
                .fx_rate(domain::FxPair::UsdKrw, domain::FxRateKind::Mid, Some(at)),
        )
    }

    fn buying_power<'a>(
        &'a self,
        account: &'a domain::AccountId,
        currency: domain::Currency,
    ) -> BoxFuture<'a, Result<domain::BuyingPower, application::PortError>> {
        use application::BrokerRead as _;
        Box::pin(self.0.buying_power(account, currency))
    }

    fn daily_candles<'a>(
        &'a self,
        instrument: &'a domain::QuoteInstrument,
        window: domain::TimeInterval,
    ) -> BoxFuture<'a, Result<Vec<domain::Candle>, application::PortError>> {
        Box::pin(self.0.history_daily_candles(instrument, window))
    }
}

fn product_performance(
    broker: &Arc<ProductBroker>,
    clock: &Arc<platform_macos::SystemClock>,
) -> Arc<src_tauri::performance_book::PerformanceBook> {
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    Arc::new(src_tauri::performance_book::PerformanceBook::new(
        Arc::new(src_tauri::performance_book::FeedHistory::new(
            Arc::new(TossPerformanceFeed(Arc::clone(broker))),
            Arc::clone(&shared),
        )),
        shared,
    ))
}

fn product_toss(
    clock: &Arc<platform_macos::SystemClock>,
    events: &Arc<application::EventBus>,
    repositories: &Arc<SqliteRepositories>,
) -> Result<ProductToss, std::io::Error> {
    let secrets = Arc::new(product_secrets());
    let broker = live_toss_read(
        &broker_toss::TossEndpoint::production(),
        Arc::clone(&secrets),
        Arc::clone(clock),
        Arc::clone(events),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let shared_broker = Arc::new(broker);
    let sectors = Arc::new(src_tauri::sector_classes::SectorClasses::new(
        Arc::new(src_tauri::sector_classes::MarketSectorFeed(Arc::clone(
            &shared_broker,
        ))),
        Arc::<platform_macos::SystemClock>::clone(clock),
    ));
    let performance = product_performance(&shared_broker, clock);
    Ok(ProductToss {
        secrets,
        sectors,
        performance,
        broker: shared_broker,
        portfolios: Arc::new(store_sqlite::SqlitePortfolioStore::new(
            repositories.as_ref().clone(),
        )),
        accounts: src_tauri::broker_link::AccountSlot::default(),
    })
}

fn product_ports(
    repositories: &Arc<SqliteRepositories>,
    (notifier, toss): (Arc<ProductNotifier>, &ProductToss),
    shared_clock: &Arc<platform_macos::SystemClock>,
    events: Arc<application::EventBus>,
    ai: ProductAiParts,
) -> Ports<AppPortTypes> {
    Ports::<AppPortTypes> {
        broker_read: Arc::clone(&toss.broker),
        market_info: Arc::clone(&toss.broker),
        macro_series: Arc::new(Unavailable),
        fundamentals: Arc::new(Unavailable),
        ai_provider: ai.ai,
        chat_repository: Arc::clone(repositories),
        chat_ledger: Arc::clone(repositories),
        chat_queue: ai.queue,
        warm_chat: Arc::new(Unavailable),
        ai_jobs: ai.jobs,
        portfolios: Arc::clone(&toss.portfolios),
        transactions: Arc::clone(repositories),
        market_data: Arc::clone(repositories),
        news: Arc::clone(repositories),
        theses: Arc::clone(repositories),
        estimates: Arc::clone(repositories),
        signals: Arc::clone(repositories),
        proposals: Arc::clone(repositories),
        valuations: Arc::clone(repositories),
        settings: Arc::clone(repositories),
        audit: Arc::clone(repositories),
        secrets: Arc::clone(&toss.secrets),
        notifier,
        news_feeds: Vec::new(),
        clock: Arc::clone(shared_clock),
        presence: Arc::new(platform_macos::MacOsPresence::new(
            platform_macos::NativeRiskDialog,
            platform_macos::NativeOrderDialog,
            platform_macos::LocalDeviceOwnerAuth,
        )),
        events,
        saved_accounts: toss.accounts.clone(),
        database: Arc::new(database_factory(Arc::clone(shared_clock))),
    }
}

type ProductSecrets = secrets_keychain::KeychainSecretStore<secrets_keychain::SystemKeychain>;

type ProductBrokerFor<S> = LiveTossRead<S, platform_macos::SystemClock>;
type ProductBroker = ProductBrokerFor<ProductSecrets>;

#[derive(Debug, Default, Clone, Copy)]
struct NoStreamRenewal;

impl broker_toss::TokenRenewal for NoStreamRenewal {
    fn close(&self) -> impl Future<Output = Result<(), broker_toss::TokenError>> + Send {
        std::future::ready(Ok(()))
    }

    fn reconnect(
        &self,
        _token: &application::SecretValue,
    ) -> impl Future<Output = Result<(), broker_toss::TokenError>> + Send {
        std::future::ready(Ok(()))
    }

    fn resubscribe(&self) -> impl Future<Output = Result<(), broker_toss::TokenError>> + Send {
        std::future::ready(Ok(()))
    }

    fn reconcile_orders(
        &self,
        _token: &application::SecretValue,
    ) -> impl Future<Output = Result<(), broker_toss::TokenError>> + Send {
        std::future::ready(Ok(()))
    }
}

type LiveTossRead<S, C> =
    broker_toss::TossAccountRead<SharedSecrets<S>, SharedClock<C>, NoStreamRenewal>;

fn live_toss_read<S, C>(
    endpoint: &broker_toss::TossEndpoint,
    secrets: Arc<S>,
    clock: Arc<C>,
    events: Arc<application::EventBus>,
) -> Result<LiveTossRead<S, C>, broker_toss::TossHttpError>
where
    S: application::SecretStore + 'static,
    C: application::Clock + 'static,
{
    let shared_clock: Arc<dyn application::Clock> = Arc::<C>::clone(&clock);
    let client = broker_toss::TossHttpClient::new(
        endpoint,
        shared_clock,
        Arc::new(broker_toss::TokioRequestWait),
    )?;
    let tokens = broker_toss::TokenManager::new(
        client,
        SharedSecrets(secrets),
        SharedClock(Arc::clone(&clock)),
        NoStreamRenewal,
        events,
    );
    let master = domain::MasterInstrumentFacts::try_new(Vec::new())
        .map_err(|_error| broker_toss::TossHttpError::Configuration)?;
    Ok(
        broker_toss::TossAccountRead::new(tokens, SharedClock(clock), master)
            .showing_unmatched_holdings()
            .with_live_market_sessions(),
    )
}

const fn probe_failure(error: broker_toss::TokenError) -> ProbeFailure {
    use application::PortErrorKind;
    use broker_toss::{TokenError, TossHttpError};
    match error {
        TokenError::MissingCredentials => ProbeFailure::MissingKeys,
        TokenError::Rejected(PortErrorKind::AccessDenied(denial)) => ProbeFailure::Denied(denial),
        TokenError::Rejected(PortErrorKind::InvalidRequest(_))
        | TokenError::Http(TossHttpError::InvalidCredential) => ProbeFailure::RejectedKey,
        TokenError::RevokedRepeated => ProbeFailure::KeyInUse,
        TokenError::Store(failure) => ProbeFailure::Store(failure),
        TokenError::Rejected(_)
        | TokenError::Http(_)
        | TokenError::InvalidResponse
        | TokenError::Renewal
        | TokenError::Superseded => ProbeFailure::Unavailable,
    }
}

struct LiveProbe<S, C>(Arc<LiveTossRead<S, C>>);

impl<S, C> TossProbe for LiveProbe<S, C>
where
    S: application::SecretStore + 'static,
    C: application::Clock + 'static,
{
    fn check(&self) -> BoxFuture<'_, ProbeResult> {
        Box::pin(async move { self.0.connection_status().await.map_err(probe_failure) })
    }

    fn reconnect(&self) -> BoxFuture<'_, ProbeResult> {
        Box::pin(async move { self.0.reconnect().await.map_err(probe_failure) })
    }
}

fn toss_link_state(
    toss: &ProductToss,
    holdings: HoldingsListener,
    clock: &Arc<platform_macos::SystemClock>,
) -> TossLinkState {
    let shared_clock: Arc<dyn application::Clock> =
        Arc::<platform_macos::SystemClock>::clone(clock);
    TossLinkState::new(Arc::new(TossLink::new(
        Arc::clone(&toss.secrets),
        Arc::new(LiveProbe(Arc::clone(&toss.broker))),
        Arc::new(holdings),
        shared_clock,
    )))
}

fn product_secrets() -> ProductSecrets {
    let build = if tauri::is_dev() {
        secrets_keychain::BuildKind::Development
    } else {
        secrets_keychain::BuildKind::Release
    };
    let profile = match build {
        secrets_keychain::BuildKind::Development => {
            secrets_keychain::KeychainProfile::development()
        }
        secrets_keychain::BuildKind::Release => secrets_keychain::KeychainProfile::product_login(),
    };
    secrets_keychain::KeychainSecretStore::new(profile, secrets_keychain::SystemKeychain)
}

fn database_key_provision() -> DatabaseKeyProvision {
    DatabaseKeyProvision(Box::new(secrets_keychain::generate_database_key))
}

fn start_updates(handle: &tauri::AppHandle<tauri::Wry>) {
    let mode = updates::mode_for(
        updates::current_build(),
        configured_pubkey(handle).as_deref(),
    );
    let source: Arc<dyn updates::UpdateSource> = Arc::new(TauriUpdateSource::new(handle.clone()));
    src_tauri::ipc::updates::start(handle, Arc::new(updates::UpdateCenter::new(mode, source)));
}

fn product_builder(
    scheduler_events: Arc<scheduler::SchedulerEventBridge>,
    repositories: &Arc<SqliteRepositories>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
    clock: &Arc<platform_macos::SystemClock>,
) -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(ExternalLinkRuntime::new(
            Arc::clone(repositories),
            Arc::new(MacUrlOpener),
        ))
        .manage(scheduler_events)
        .manage(Arc::clone(repositories))
        .manage(BriefingIpcState::from_repository(Arc::clone(repositories)))
        .manage(database_key_provision())
        .manage(ui_events)
        .manage(CollectedLinker(Arc::new(AliasTitleLinker)))
        .manage(ScorecardIpcState::from_repository(
            Arc::clone(repositories),
            Arc::<platform_macos::SystemClock>::clone(clock),
        ))
        .manage(public_ip_runtime(clock));
    let tracking = src_tauri::news_tracking::NewsTracking::default();
    let hints = sources::NewsLinkHints::default();
    let with_feeds = builder
        .manage(collector_feeds(clock, &tracking, &hints))
        .manage(tracking)
        .manage(hints);
    match spec_watch_inputs(clock) {
        Some(inputs) => with_feeds.manage(inputs),
        None => with_feeds,
    }
}

#[derive(Debug)]
struct LoggedSourceCalls;

impl application::DiagnosticsSink for LoggedSourceCalls {
    fn record(&self, event: &application::DiagnosticEvent) {
        src_tauri::diagnostics_log::record(event);
    }
}

const fn matches_unreachable_press(source: sources::PressMacroSource) -> bool {
    match source {
        sources::PressMacroSource::Edaily | sources::PressMacroSource::SegyeFinance => true,
        sources::PressMacroSource::Hankyung
        | sources::PressMacroSource::Maekyung
        | sources::PressMacroSource::Yonhap
        | sources::PressMacroSource::BankOfKorea
        | sources::PressMacroSource::FederalReserve => false,
    }
}

fn disclosure_feeds(
    client: &Arc<sources::SourceHttpClient>,
    clock: &Arc<platform_macos::SystemClock>,
    tracking: &src_tauri::news_tracking::NewsTracking,
    hints: &sources::NewsLinkHints,
) -> [CollectorFeed; 2] {
    [
        CollectorFeed {
            target: application::CollectionTarget::DartList,
            startup: None,
            regular: SharedFeed::new(sources::DartLatestFeed::new(
                Arc::clone(client),
                Arc::new(tracking.clone()),
                hints.clone(),
            )),
        },
        CollectorFeed {
            target: application::CollectionTarget::SecEdgar,
            startup: None,
            regular: SharedFeed::new(sources::SecTrackedFeed::new(
                sources::SecClientParts {
                    endpoint: sources::SourceEndpoint::live(),
                    clock: Arc::<platform_macos::SystemClock>::clone(clock),
                    wait: Arc::new(sources::TokioSourceWait),
                    diagnostics: Some(Arc::new(LoggedSourceCalls)),
                },
                Arc::new(tracking.clone()),
                Arc::new(tracking.clone()),
                hints.clone(),
            )),
        },
    ]
}

fn collector_feeds(
    clock: &Arc<platform_macos::SystemClock>,
    tracking: &src_tauri::news_tracking::NewsTracking,
    hints: &sources::NewsLinkHints,
) -> CollectorFeeds {
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    let Ok(http) = sources::SourceHttpClient::new(
        sources::SourceEndpoint::live(),
        shared,
        Arc::new(sources::TokioSourceWait),
        None,
    ) else {
        return CollectorFeeds::default();
    };
    let client = Arc::new(http.with_diagnostics(Arc::new(LoggedSourceCalls)));
    let disclosure_feeds = disclosure_feeds(&client, clock, tracking, hints);
    let mut feeds: Vec<CollectorFeed> = sources::PressMacroSource::ALL
        .into_iter()
        .filter(|source| !matches_unreachable_press(*source))
        .map(|source| CollectorFeed {
            target: press_target(source),
            startup: None,
            regular: SharedFeed::new(sources::RssNewsFeed::new(Arc::clone(&client), source)),
        })
        .collect();
    let kind = |window| {
        SharedFeed::new(sources::KindDisclosureFeed::new(
            sources::KindDisclosureClient::new(Arc::clone(&client)),
            window,
        ))
    };
    feeds.push(CollectorFeed {
        target: application::CollectionTarget::KindDisclosure,
        startup: Some(kind(sources::KindFetchWindow::StartupCatchUp)),
        regular: kind(sources::KindFetchWindow::Regular),
    });
    for (target, search) in [
        (
            application::CollectionTarget::GoogleNews,
            sources::TrackedSearch::GoogleNews,
        ),
        (
            application::CollectionTarget::YahooFinance,
            sources::TrackedSearch::YahooFinance,
        ),
    ] {
        feeds.push(CollectorFeed {
            target,
            startup: None,
            regular: SharedFeed::new(sources::TrackedNewsFeed::new(
                Arc::clone(&client),
                search,
                Arc::new(tracking.clone()),
                hints.clone(),
            )),
        });
    }
    feeds.extend(disclosure_feeds);
    CollectorFeeds(feeds)
}

const fn press_target(source: sources::PressMacroSource) -> application::CollectionTarget {
    match source {
        sources::PressMacroSource::Hankyung => application::CollectionTarget::HankyungRss,
        sources::PressMacroSource::Maekyung
        | sources::PressMacroSource::Yonhap
        | sources::PressMacroSource::Edaily
        | sources::PressMacroSource::SegyeFinance => application::CollectionTarget::PressRss,
        sources::PressMacroSource::BankOfKorea | sources::PressMacroSource::FederalReserve => {
            application::CollectionTarget::CentralBankRss
        }
    }
}

fn spec_watch_inputs(clock: &Arc<platform_macos::SystemClock>) -> Option<SpecWatchInputs> {
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    let http = broker_toss::TossHttpClient::new(
        &broker_toss::TossEndpoint::production(),
        shared,
        Arc::new(broker_toss::TokioRequestWait),
    )
    .ok()?;
    let baseline = broker_toss::pinned_spec_baseline().ok()?;
    Some(SpecWatchInputs {
        source: Arc::new(broker_toss::TossSpecClient::new(http)),
        baseline,
    })
}

type ProductMcpHost =
    McpServerHost<platform_macos::SystemClock, PsPeerInspector, SqliteRepositories>;
type ProductAi = AiAvailability<McpAttachedAi<ProductClaude, ProductMcpHost>>;
type ProductQueue = ai_claude_code::AiRunnerQueue<ProductAi>;
type AppPortTypes = ProductPorts<
    SqliteRepositories,
    ProductSecrets,
    ProductAi,
    ProductQueue,
    ProductAiJobs,
    ProductBroker,
    store_sqlite::SqlitePortfolioStore,
>;

const ALLOWED_BUILTIN_PLUGINS: [&str; 4] = [
    "cc-plugin-agents-md",
    "cc-plugin-telemetry",
    "cc-plugin-privacy",
    "cc-plugin-plugin-authoring",
];
const BRIDGE_SOCKET_FLAG: &str = "--socket";
const BRIDGE_FILE_NAME: &str = "mcp-bridge";

struct ProductClaude {
    provider: Arc<ai_claude_code::ClaudeCodeProvider>,
    bridge: BridgeLocation,
    preferences: Arc<SqliteRepositories>,
    config_dir: ai_claude_code::ConfigDirSlot,
}

impl std::fmt::Debug for ProductClaude {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductClaude")
    }
}

fn bridge_config(
    bridge: &Path,
    socket: &Path,
) -> Result<ai_claude_code::McpBridgeConfig, ai_claude_code::CommandBuildError> {
    ai_claude_code::McpBridgeConfig::try_new(
        String::from(ANALYSIS_MCP_SERVER),
        bridge.to_path_buf(),
        vec![
            String::from(BRIDGE_SOCKET_FLAG),
            socket.to_string_lossy().into_owned(),
        ],
    )
}

impl ProductClaude {
    async fn prepare(&self, spec: RunSpec) -> RunSpec {
        use application::AiPreferencesRepository as _;
        match self.preferences.load_ai_preferences().await {
            Ok(Some(stored)) => {
                self.config_dir
                    .set(stored.config_dir().map(|dir| Path::new(dir.as_str())));
                src_tauri::task_models::tuned(&stored, spec)
            }
            Ok(None) => {
                self.config_dir.set(None);
                spec
            }
            Err(_unreadable) => spec,
        }
    }

    fn attachment(
        &self,
        link: McpRunLink,
    ) -> Result<ai_claude_code::RunMcpAttachment, InformationError> {
        let McpRunLink {
            socket_path,
            token,
            binder,
        } = link;
        let location = self.bridge.get().ok_or(InformationError::Unavailable(
            application::ResetTime::NotProvided,
        ))?;
        let bridge = bridge_config(location, &socket_path)
            .map_err(ai_claude_code::CommandBuildError::to_information_error)?;
        Ok(
            ai_claude_code::RunMcpAttachment::new(bridge, token).with_spawned_notice(
                ai_claude_code::SpawnedPidNotice::new(move |pid| binder.bind(pid)),
            ),
        )
    }
}

impl AiProvider for ProductClaude {
    type Stream = ai_claude_code::ClaudeEventStream;

    async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
        let tuned = self.prepare(spec).await;
        self.provider.run(tuned).await
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: domain::UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        let tuned = self.prepare(spec).await;
        self.provider.resume(tuned, history, message).await
    }

    async fn usage(&self) -> Result<application::AiUsage, InformationError> {
        self.provider.usage().await
    }

    async fn interrupt(&self, run: &AiRunId) -> Result<(), InformationError> {
        self.provider.interrupt(run).await
    }

    async fn verify_isolation(
        &self,
        kind: application::RunKind,
    ) -> Result<application::IsolationCheck, InformationError> {
        self.provider.verify_isolation(kind).await
    }
}

impl AttachableAi for ProductClaude {
    async fn run_with_mcp(
        &self,
        spec: RunSpec,
        link: McpRunLink,
    ) -> Result<Self::Stream, InformationError> {
        let tuned = self.prepare(spec).await;
        self.provider
            .run_attached(tuned, self.attachment(link)?)
            .await
    }

    async fn resume_with_mcp(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: domain::UntrustedText,
        link: McpRunLink,
    ) -> Result<Self::Stream, InformationError> {
        let tuned = self.prepare(spec).await;
        self.provider
            .resume_attached(tuned, history, message, self.attachment(link)?)
            .await
    }

    async fn verify_with_mcp(
        &self,
        link: McpRunLink,
    ) -> Result<application::IsolationCheck, InformationError> {
        self.provider
            .verify_isolation_attached(self.attachment(link)?)
            .await
    }
}

struct ProductAiJobs {
    queue: Arc<ProductQueue>,
    grants: tokio::sync::Mutex<BTreeMap<scheduler::ScheduledJobId, AiRunId>>,
}

impl std::fmt::Debug for ProductAiJobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductAiJobs")
    }
}

impl scheduler::AiJobQueue for ProductAiJobs {
    async fn request_background_slot(
        &self,
        job: scheduler::ScheduledJobId,
        _now: domain::Timestamp,
    ) -> Result<scheduler::AiQueueAdmission, scheduler::AiQueueError> {
        use ai_claude_code::ScheduledSlotAdmission as Slot;
        let admission = self
            .queue
            .request_scheduled_slot(job.as_code())
            .await
            .map_err(|_error| scheduler::AiQueueError::Unavailable)?;
        Ok(match admission {
            Slot::Admitted(grant) => {
                let (run, _session, _token) = grant.credentials.into_parts();
                let _previous = self.grants.lock().await.insert(job, run);
                scheduler::AiQueueAdmission::Admitted
            }
            Slot::PausedForChat | Slot::Busy => scheduler::AiQueueAdmission::PausedForChat,
            Slot::BudgetReached { .. } => {
                scheduler::AiQueueAdmission::BudgetReached { resumes_at: None }
            }
            Slot::RateLimited { resumes_at, .. } => scheduler::AiQueueAdmission::RateLimited {
                resumes_at: Some(resumes_at),
            },
        })
    }

    async fn release_background_slot(
        &self,
        job: scheduler::ScheduledJobId,
        _now: domain::Timestamp,
    ) -> Result<(), scheduler::AiQueueError> {
        let Some(run) = self.grants.lock().await.remove(&job) else {
            return Ok(());
        };
        self.queue
            .release_scheduled_slot(&run, ai_claude_code::ScheduledReleaseOutcome::Completed)
            .await
            .map(|_written| ())
            .map_err(|_error| scheduler::AiQueueError::Unavailable)
    }
}

struct AiSetup {
    executable: Option<PathBuf>,
    host_env: ai_claude_code::HostEnvironment,
    work_root: PathBuf,
    mcp_base: PathBuf,
    bridge: BridgeLocation,
    sandbox: Option<ai_claude_code::SandboxLayer>,
    cli_projects_dir: Option<PathBuf>,
    turns: ChatTurnBook,
    market: BrokerMarketTools,
    portfolio: CatalogPortfolio,
    estimate_runs: EstimateRunBook,
    app_commands: Arc<AppCommandSlot<tauri::Wry>>,
}

impl AiSetup {
    fn with_market(self, market: BrokerMarketTools) -> Self {
        Self { market, ..self }
    }

    fn with_estimate_runs(self, estimate_runs: EstimateRunBook) -> Self {
        Self {
            estimate_runs,
            ..self
        }
    }

    fn with_portfolio(self, portfolio: CatalogPortfolio) -> Self {
        Self { portfolio, ..self }
    }
}

impl std::fmt::Debug for AiSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiSetup")
            .field("has_executable", &self.executable.is_some())
            .field("has_sandbox", &self.sandbox.is_some())
            .finish_non_exhaustive()
    }
}

struct ProductAiParts {
    ai: Arc<ProductAi>,
    queue: Arc<ProductQueue>,
    jobs: Arc<ProductAiJobs>,
}

fn assemble_ai(
    setup: AiSetup,
    clock: &Arc<platform_macos::SystemClock>,
    events: &Arc<application::EventBus>,
    repositories: &Arc<SqliteRepositories>,
) -> Result<ProductAiParts, ai_claude_code::QuotaConfigError> {
    let ai = Arc::new(
        claude_ai(setup, clock, events, repositories)
            .map_or(AiAvailability::Missing, AiAvailability::Ready),
    );
    let shared_clock: Arc<dyn application::Clock> =
        Arc::<platform_macos::SystemClock>::clone(clock);
    let queue = Arc::new(ai_claude_code::AiRunnerQueue::with_default_policy(
        Arc::clone(&ai),
        shared_clock,
        Arc::clone(events),
    )?);
    let jobs = Arc::new(ProductAiJobs {
        queue: Arc::clone(&queue),
        grants: tokio::sync::Mutex::new(BTreeMap::new()),
    });
    Ok(ProductAiParts { ai, queue, jobs })
}

#[derive(Debug)]
struct StoredRoutes(Arc<SqliteRepositories>);

impl src_tauri::ipc::news_settings::RouteLookup for StoredRoutes {
    fn source_status(
        &self,
        source: domain::DataSource,
    ) -> std::pin::Pin<Box<dyn Future<Output = Option<application::SourceStatus>> + Send + '_>>
    {
        Box::pin(async move {
            application::SourceStatusRepository::find(self.0.as_ref(), source)
                .await
                .ok()
                .flatten()
        })
    }
}

fn named_matcher(names: &[src_tauri::news_tracking::MatchName]) -> sources::StockMatcher {
    use src_tauri::news_tracking::{MatchNameClarity, MatchNameRole};
    let entries = names
        .iter()
        .filter_map(|name| {
            sources::MatchDictionaryEntry::new(
                name.instrument.clone(),
                &name.name,
                match name.role {
                    MatchNameRole::Official => sources::MatchNameKind::Official,
                    MatchNameRole::Alias => sources::MatchNameKind::Alias,
                },
                match name.clarity {
                    MatchNameClarity::Distinct => sources::MatchNameAmbiguity::Distinct,
                    MatchNameClarity::NeedsContext => sources::MatchNameAmbiguity::CommonNoun,
                },
            )
        })
        .collect();
    sources::StockMatcher::new(entries)
}

#[derive(Debug, Clone, Copy)]
struct AliasTitleLinker;

impl TitleLinker for AliasTitleLinker {
    fn instruments_in(
        &self,
        title: &domain::UntrustedText,
        aliases: &[application::StockAlias],
    ) -> Vec<domain::InstrumentId> {
        let entries = aliases
            .iter()
            .filter_map(|alias| {
                sources::MatchDictionaryEntry::new(
                    alias.instrument.clone(),
                    alias.alias.as_str(),
                    sources::MatchNameKind::Alias,
                    sources::MatchNameAmbiguity::Distinct,
                )
            })
            .collect();
        sources::StockMatcher::new(entries)
            .match_title(title)
            .matched()
            .to_vec()
    }

    fn probe_named(
        &self,
        title: &domain::UntrustedText,
        names: &[src_tauri::news_tracking::MatchName],
    ) -> src_tauri::collected_links::NameProbe {
        let found = named_matcher(names).match_title(title);
        if !found.ambiguous().is_empty() {
            src_tauri::collected_links::NameProbe::NeedsContext
        } else if !found.group_mentions().is_empty() {
            src_tauri::collected_links::NameProbe::GroupOnly
        } else {
            src_tauri::collected_links::NameProbe::Quiet
        }
    }

    fn instruments_named(
        &self,
        title: &domain::UntrustedText,
        names: &[src_tauri::news_tracking::MatchName],
    ) -> Vec<domain::InstrumentId> {
        named_matcher(names).match_title(title).matched().to_vec()
    }
}

struct ProductArticleBodies {
    reader: Option<sources::ArticleReader>,
}

impl std::fmt::Debug for ProductArticleBodies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProductArticleBodies")
    }
}

const fn article_unavailable() -> application::ArticleFetchFailure {
    application::ArticleFetchFailure::Unavailable(application::SystemPortError::new(
        application::SystemPort::Repository,
        application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
    ))
}

impl ProductArticleBodies {
    fn new(clock: &Arc<platform_macos::SystemClock>) -> Self {
        let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
        let reader = sources::SourceHttpClient::new(
            sources::SourceEndpoint::live(),
            shared,
            Arc::new(sources::TokioSourceWait),
            None,
        )
        .ok()
        .map(|client| sources::ArticleReader::new(Arc::new(client)));
        Self { reader }
    }
}

impl ArticleBodies for ProductArticleBodies {
    async fn fetch(
        &self,
        source: &domain::NewsSource,
        url: &domain::NormalizedUrl,
        settings: &application::NewsDataSettings,
    ) -> Result<domain::UntrustedText, application::ArticleFetchFailure> {
        let reader = self.reader.as_ref().ok_or_else(article_unavailable)?;
        let title =
            domain::UntrustedText::try_new("article").map_err(|_error| article_unavailable())?;
        let item = domain::NewsItem::new(source.clone(), url.clone(), title, None);
        match reader
            .read_transient(item.collected(), settings.body_read())
            .await
        {
            Ok(text) => Ok(text),
            Err(sources::ArticleReadError::FetchFailed(_)) => Err(article_unavailable()),
            Err(
                sources::ArticleReadError::WebSourceDisallowed
                | sources::ArticleReadError::DisclosureDisallowed
                | sources::ArticleReadError::SourceLockedOff(_)
                | sources::ArticleReadError::AiBodyReadDisabled(_)
                | sources::ArticleReadError::DisallowedArticleHost,
            ) => Err(application::ArticleFetchFailure::NotFound),
        }
    }
}

fn product_catalog(
    setup: &AiSetup,
    clock: &Arc<platform_macos::SystemClock>,
    events: &Arc<application::EventBus>,
    repositories: &Arc<SqliteRepositories>,
    (runs, collector): (&RunMetadataBook, &LateAi<ProductClaude>),
) -> Option<mcp_server::ToolRegistry> {
    let registry = analysis_catalog(
        clock,
        CatalogStores {
            theses: Arc::new(application::TimedThesisRepository::new(
                Arc::clone(repositories),
                Arc::clone(repositories),
                Arc::clone(clock),
            )),
            estimates: Arc::clone(repositories),
            bodies: Arc::new(ProductArticleBodies::new(clock)),
            runs: runs.clone(),
            collector: Arc::new(collector.clone()),
            turns: setup.turns.clone(),
            events: Arc::clone(events),
            market: setup.market.clone(),
            portfolio: setup.portfolio.clone(),
            estimate_runs: setup.estimate_runs.clone(),
        },
    )
    .ok()?;
    with_app_commands(registry, setup, clock, repositories)
}

fn with_app_commands(
    mut registry: mcp_server::ToolRegistry,
    setup: &AiSetup,
    clock: &Arc<platform_macos::SystemClock>,
    repositories: &Arc<SqliteRepositories>,
) -> Option<mcp_server::ToolRegistry> {
    register_app_commands(
        &mut registry,
        AppCommandDeps {
            app: Arc::clone(&setup.app_commands),
            turns: setup.turns.clone(),
            journal: Arc::new(AuditedAiJournal::new(
                Arc::clone(repositories),
                Arc::clone(clock),
            )),
            cursors: mcp_server::make_cursor_store(Arc::clone(clock)),
        },
    )
    .ok()?;
    Some(registry)
}

fn claude_ai(
    setup: AiSetup,
    clock: &Arc<platform_macos::SystemClock>,
    events: &Arc<application::EventBus>,
    repositories: &Arc<SqliteRepositories>,
) -> Option<McpAttachedAi<ProductClaude, ProductMcpHost>> {
    let runs = RunMetadataBook::new().with_hasher(Arc::new(store_sqlite::Sha256PromptHasher));
    let collector = LateAi::<ProductClaude>::new();
    let config_dir = ai_claude_code::ConfigDirSlot::default();
    let registry = product_catalog(&setup, clock, events, repositories, (&runs, &collector))?;
    let base = ai_claude_code::ClaudeCodeConfig::new(
        setup.executable?,
        setup.work_root,
        String::from(ANALYSIS_MCP_SERVER),
        allowed_tool_names(ANALYSIS_MCP_SERVER, &registry),
        ALLOWED_BUILTIN_PLUGINS
            .into_iter()
            .map(String::from)
            .collect(),
    )?
    .with_host_environment(setup.host_env)
    .with_collection_json_schema(String::from(ai_claude_code::WEB_COLLECTION_JSON_SCHEMA))
    .with_config_dir_slot(config_dir.clone());
    let config = match (setup.sandbox, setup.cli_projects_dir) {
        (Some(layer), None) => base.with_sandbox(layer),
        (None, None) => base.without_sandbox(),
        (Some(layer), Some(dir)) => base.with_sandbox(layer).with_cli_projects_dir(dir),
        (None, Some(dir)) => base.without_sandbox().with_cli_projects_dir(dir),
    };
    let shared_clock: Arc<dyn application::Clock> =
        Arc::<platform_macos::SystemClock>::clone(clock);
    let provider = Arc::new(
        ai_claude_code::ClaudeCodeProvider::new(config).with_clock_and_quota(
            shared_clock,
            ai_claude_code::QuotaPolicy::default_policy().ok()?,
        ),
    );
    let product = Arc::new(ProductClaude {
        provider,
        bridge: setup.bridge.clone(),
        preferences: Arc::clone(repositories),
        config_dir,
    });
    let _bound = collector.bind(Arc::clone(&product));
    let host = McpServerHost::new(McpServerHostParts {
        clock: Arc::clone(clock),
        inspector: mcp_server::PeerInspectorMode::Configured(Arc::new(PsPeerInspector::default())),
        config: mcp_server::McpServerConfig::default_config().ok()?,
        registry: Arc::new(registry),
        ledger: mcp_server::ToolLedgerMode::Configured(Arc::clone(repositories)),
        base_dir: setup.mcp_base,
        bridge: setup.bridge,
    });
    Some(McpAttachedAi::new(product, Arc::new(host)).with_runs(runs))
}

fn bundled_bridge() -> BridgeLocation {
    let location = BridgeLocation::new();
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(BRIDGE_FILE_NAME)))
    {
        let _first = location.set(path);
    }
    location
}

struct DiscoveredCli {
    executable: Option<PathBuf>,
    host_env: ai_claude_code::HostEnvironment,
}

fn child_path(login: Option<&str>, process: Option<&str>, executable: Option<&Path>) -> String {
    let mut entries: Vec<String> = Vec::new();
    let mut push = |entry: &str| {
        if !entry.is_empty() && !entries.iter().any(|known| known == entry) {
            entries.push(String::from(entry));
        }
    };
    if let Some(directory) = executable.and_then(Path::parent).and_then(Path::to_str) {
        push(directory);
    }
    for list in [login, process].into_iter().flatten() {
        for entry in list.split(':') {
            push(entry);
        }
    }
    entries.join(":")
}

fn discover_cli<V, R>(vars: V, resolver: &R, known: Option<Vec<PathBuf>>) -> DiscoveredCli
where
    V: Iterator<Item = (String, String)> + Clone,
    R: ai_claude_code::LoginShellPathResolver,
{
    let process_env = ai_claude_code::HostEnvironment::from_pairs(vars.clone());
    let base = ai_claude_code::DiscoveryConfig::new(process_env.clone());
    let config = match known {
        Some(locations) => base.with_known_locations(locations),
        None => base,
    };
    let executable = match ai_claude_code::find_claude_executable(&config, resolver) {
        ai_claude_code::DiscoveryOutcome::Found { path, .. } => Some(path),
        ai_claude_code::DiscoveryOutcome::NotInstalled => None,
    };
    let login = resolver.resolve_login_shell_path(&process_env);
    let path = child_path(login.as_deref(), process_env.path(), executable.as_deref());
    let host_env = ai_claude_code::HostEnvironment::from_pairs(
        vars.filter(|(key, _value)| key != "PATH")
            .chain([(String::from("PATH"), path)]),
    );
    DiscoveredCli {
        executable,
        host_env,
    }
}

fn discover_ai_setup(
    identifier: &str,
    bridge: BridgeLocation,
    (turns, app_commands): (ChatTurnBook, Arc<AppCommandSlot<tauri::Wry>>),
) -> AiSetup {
    let DiscoveredCli {
        executable,
        host_env,
    } = discover_cli(
        std::env::vars().collect::<Vec<_>>().into_iter(),
        &ai_claude_code::SystemLoginShellPathResolver,
        None,
    );
    let temp_root = host_env
        .tmpdir()
        .and_then(|tmp| std::fs::canonicalize(tmp).ok())
        .unwrap_or_else(|| PathBuf::from("/private/tmp"));
    let app_data = host_env.home().map(|home| {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(identifier)
    });
    let work_root = temp_root.join("stock-copilot-ai");
    AiSetup {
        executable,
        sandbox: app_data.and_then(|dir| probed_sandbox(&dir, &work_root)),
        host_env,
        mcp_base: temp_root.join("sc-mcp"),
        work_root,
        bridge,
        cli_projects_dir: None,
        turns,
        market: BrokerMarketTools::unconnected(),
        portfolio: CatalogPortfolio::default(),
        estimate_runs: EstimateRunBook::new(),
        app_commands,
    }
}

fn probed_sandbox(app_data: &Path, scratch: &Path) -> Option<ai_claude_code::SandboxLayer> {
    std::fs::create_dir_all(app_data).ok()?;
    std::fs::create_dir_all(scratch).ok()?;
    let profile = ai_claude_code::SandboxProfile::try_new(&[app_data.to_path_buf()]).ok()?;
    let layer = ai_claude_code::SandboxLayer::new(
        PathBuf::from(ai_claude_code::SYSTEM_SANDBOX_EXEC),
        profile,
    );
    match ai_claude_code::probe_sandbox_layer(&layer, scratch) {
        ai_claude_code::SandboxProbeOutcome::Applied => Some(layer),
        ai_claude_code::SandboxProbeOutcome::NotApplied
        | ai_claude_code::SandboxProbeOutcome::Unavailable => None,
    }
}

type ProductNotifier = FilteredNotifier<SystemNotificationDelivery<tauri::Wry>>;

struct ProductSubscribers<D> {
    notifier: Arc<FilteredNotifier<D>>,
    audit: Arc<SqliteRepositories>,
    clock: Arc<platform_macos::SystemClock>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
    intraday: Arc<IntradayWatchSlot>,
    signals: Arc<SignalPipelineSlot>,
}

fn product_event_bus<D: NotificationDelivery + 'static>(
    subscribers: ProductSubscribers<D>,
) -> (
    Arc<application::EventBus>,
    Arc<scheduler::SchedulerEventBridge>,
) {
    let bridge = Arc::new(scheduler::SchedulerEventBridge::new());
    let mut builder = application::EventBusBuilder::default();
    builder.subscribe(
        scheduler::SchedulerEventBridge::subscribed_kinds(),
        scheduler::BridgedSchedulerEvents(Arc::clone(&bridge)),
    );
    builder.subscribe(ui_event_kinds(), BridgedUiEvents(subscribers.ui_events));
    builder.subscribe(
        IntradayWatchSlot::subscribed_kinds(),
        BridgedIntradayWatch(subscribers.intraday),
    );
    builder.subscribe(
        SignalPipelineSlot::subscribed_kinds(),
        BridgedSignalPipeline(subscribers.signals),
    );
    builder.subscribe(
        NotificationEventSubscriber::<
            AsyncRuntimeNotificationDispatch<FilteredNotifier<D>>,
            RecordedTriggerUrgency,
        >::subscribed_kinds(),
        NotificationEventSubscriber::new(
            AsyncRuntimeNotificationDispatch::new(subscribers.notifier),
            RecordedTriggerUrgency,
        ),
    );
    let clock: Arc<dyn application::Clock> = subscribers.clock;
    builder.subscribe(
        application::AuditEventRecorder::<FlushingAuditSink<SqliteRepositories>>::kinds(),
        application::AuditEventRecorder::new(
            clock,
            FlushingAuditSink::new(AuditEventQueue::default(), subscribers.audit),
        ),
    );
    (Arc::new(builder.build()), bridge)
}

fn database_factory<C: application::Clock + 'static>(clock: Arc<C>) -> DatabaseFactory {
    DatabaseFactory(Box::new(move |path, key| {
        src_tauri::database::prepare_directory(&path)?;
        store_sqlite::Database::open(&path, &key, clock.as_ref())
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn product_acl_allows_only_main_window_event_subscription() {
        let mut context = super::product_context();
        let authority = context.runtime_authority_mut();
        let local = tauri::ipc::Origin::Local;
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com".parse().expect("원격 출처"),
        };
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_some(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "other", "other", &local)
                    .is_none(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "main", "main", &remote)
                    .is_none(),
                "{command}"
            );
        }
        for command in ["plugin:event|emit", "plugin:event|emit_to"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
        let without_permissions = tauri::runtime_authority!(
            std::collections::BTreeMap::new(),
            tauri::utils::acl::resolved::Resolved::default()
        );
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                without_permissions
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
    }

    #[test]
    fn product_event_permissions_are_individually_required() {
        use std::collections::BTreeMap;
        use tauri::utils::acl::{capability::Capability, manifest::Manifest, resolved::Resolved};

        let manifests: BTreeMap<String, Manifest> = serde_json::from_str(
            &std::fs::read_to_string("gen/schemas/acl-manifests.json").expect("생성된 ACL"),
        )
        .expect("ACL 매니페스트");
        let capability: Capability = serde_json::from_str(
            &std::fs::read_to_string("capabilities/main.json").expect("제품 capability"),
        )
        .expect("제품 capability 형식");
        for (omitted, denied, retained) in [
            (
                "core:event:allow-listen",
                "plugin:event|listen",
                "plugin:event|unlisten",
            ),
            (
                "core:event:allow-unlisten",
                "plugin:event|unlisten",
                "plugin:event|listen",
            ),
        ] {
            let mut edited = capability.clone();
            edited
                .permissions
                .retain(|permission| permission.identifier().get() != omitted);
            assert_eq!(edited.permissions.len() + 1, capability.permissions.len());
            let capabilities = BTreeMap::from([(edited.identifier.clone(), edited)]);
            let resolved = Resolved::resolve(
                &manifests,
                capabilities,
                tauri::utils::platform::Target::MacOS,
            )
            .expect("제품 ACL 해석");
            let authority = tauri::runtime_authority!(BTreeMap::new(), resolved);
            assert!(
                authority
                    .resolve_access(denied, "main", "main", &tauri::ipc::Origin::Local)
                    .is_none(),
                "{omitted}: {denied}"
            );
            assert!(
                authority
                    .resolve_access(retained, "main", "main", &tauri::ipc::Origin::Local)
                    .is_some(),
                "{omitted}: {retained}"
            );
        }
    }

    #[test]
    fn composition_root_constructs_the_fixed_product_endpoint_without_sending_requests() {
        let endpoint = broker_toss::TossEndpoint::production();
        assert!(format!("{endpoint:?}").contains("Production"));
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        let expected_origin = spec["servers"][0]["url"].as_str().expect("origin");
        assert_eq!(
            format!("{endpoint:?}"),
            format!("TossEndpoint {{ origin: {expected_origin:?}, mode: Production }}")
        );
        let client = broker_toss::TossHttpClient::new(
            &endpoint,
            std::sync::Arc::new(testkit_ports::SteppedClock::new(
                broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                    .expect("clock")
                    .into_domain(),
            )),
            std::sync::Arc::new(broker_toss::TokioRequestWait),
        )
        .expect("product configuration");
        assert_eq!(format!("{client:?}"), "TossHttpClient(<redacted>)");
    }

    #[test]
    fn configured_database_is_opened_and_explicitly_closed_after_refresh_cleanup() {
        use src_tauri::database::{DatabaseStatus, open_database};
        use src_tauri::lifecycle::Lifecycle;
        use std::sync::Arc;
        use testkit_ports::{memory::MemoryReply, system_fakes::MemorySecretStore};
        let directory = std::path::PathBuf::from("../../../target/U-01")
            .join(format!("database-lifecycle-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("테스트 폴더");
        let path = store_sqlite::DatabasePath::new(directory.join("nested").join("store.db"));
        let secrets = MemorySecretStore::default();
        secrets.script_load().push(MemoryReply::Return(Ok(Some(
            application::SecretValue::new(vec![83; 32]),
        ))));
        let clock = Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ));
        let factory = Arc::new(super::database_factory(clock));
        tauri::async_runtime::block_on(async {
            let state = open_database(&secrets, factory, || Ok(path)).await;
            assert_eq!(state.status(), DatabaseStatus::Ready);
            let lifecycle =
                Lifecycle::new(tauri::async_runtime::spawn(std::future::pending()), state);
            lifecycle.shutdown().await;
            let guard = lifecycle.outcome().lock().await;
            match &guard.as_ref().expect("종료 결과").database {
                Ok(src_tauri::lifecycle::DatabaseClosure::Closed(_receipt)) => {}
                Ok(src_tauri::lifecycle::DatabaseClosure::NotOpened) => {
                    panic!("열린 DB의 명시적 종료 증거 필요")
                }
                Err(error) => panic!("종료 실패: {error:?}"),
            }
        });
        std::fs::remove_dir_all(directory).expect("테스트 정리");
    }

    #[test]
    fn product_acl_allows_open_external_only_from_main_local_origin() {
        let mut context = super::product_context();
        let authority = context.runtime_authority_mut();
        let local = tauri::ipc::Origin::Local;
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com".parse().expect("원격 출처"),
        };
        assert!(
            authority
                .resolve_access("open_external", "main", "main", &local)
                .is_some(),
            "main 로컬 출처 허용"
        );
        assert!(
            authority
                .resolve_access("open_external", "other", "other", &local)
                .is_none(),
            "다른 창 거부"
        );
        assert!(
            authority
                .resolve_access("open_external", "main", "main", &remote)
                .is_none(),
            "원격 출처 거부"
        );
    }

    #[test]
    fn composition_root_registers_keyless_press_and_kind_feeds_for_the_collector() {
        use application::{CollectionTarget, FeedSource, NewsFeed};
        let clock = std::sync::Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let feeds = super::collector_feeds(
            &clock,
            &src_tauri::news_tracking::NewsTracking::default(),
            &sources::NewsLinkHints::default(),
        );
        let sources: Vec<(CollectionTarget, FeedSource)> = feeds
            .0
            .iter()
            .map(|feed| (feed.target, feed.regular.source()))
            .collect();
        assert_eq!(sources.len(), 10);
        assert!(sources.contains(&(
            CollectionTarget::HankyungRss,
            FeedSource::News(domain::NewsSource::Hankyung)
        )));
        assert!(sources.contains(&(
            CollectionTarget::CentralBankRss,
            FeedSource::News(domain::NewsSource::FederalReserve)
        )));
        let kind = feeds
            .0
            .iter()
            .find(|feed| feed.target == CollectionTarget::KindDisclosure)
            .expect("KIND");
        assert_eq!(
            kind.startup.as_ref().map(NewsFeed::source),
            Some(FeedSource::Disclosure(domain::DisclosureSource::Kind))
        );
    }

    #[test]
    fn composition_root_constructs_sources_client_without_broker_credentials_or_hosts() {
        let clock = std::sync::Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ));
        let client = sources::SourceHttpClient::new(
            sources::SourceEndpoint::live(),
            clock,
            std::sync::Arc::new(sources::TokioSourceWait),
            None,
        )
        .expect("live sources client");
        assert_eq!(format!("{client:?}"), "SourceHttpClient(<redacted>)");
        for spec in sources::SourceRegistry::all() {
            for host in spec.allowed_hosts() {
                assert!(!host.to_ascii_lowercase().contains("toss"), "{host}");
            }
            match spec.credential() {
                sources::CredentialRequirement::None
                | sources::CredentialRequirement::Optional(
                    application::CredentialSource::Dart | application::CredentialSource::Fred,
                ) => {}
            }
        }
    }
}

#[cfg(test)]
mod cli_discovery_tests {
    use super::{child_path, discover_cli};
    use std::path::PathBuf;

    fn fake_cli(directory: &std::path::Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(directory).expect("폴더");
        let path = directory.join("claude");
        std::fs::write(&path, "#!/usr/bin/env node\n").expect("가짜 CLI");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("권한");
        path
    }

    fn vars(path: &str) -> std::vec::IntoIter<(String, String)> {
        vec![
            (String::from("HOME"), String::from("/Users/fake")),
            (String::from("PATH"), String::from(path)),
            (String::from("USER"), String::from("fake")),
            (String::from("TMPDIR"), String::from("/tmp")),
        ]
        .into_iter()
    }

    #[test]
    fn gui_launch_finds_the_cli_through_the_login_shell_and_children_get_that_path() {
        let root =
            PathBuf::from("../../../target/fin-gaps").join(format!("cli-{}", std::process::id()));
        let shell_bin = std::fs::canonicalize({
            let directory = root.join("brew-bin");
            let _cli = fake_cli(&directory);
            directory
        })
        .expect("절대 경로");
        let login = format!("{}:/usr/bin:/bin", shell_bin.display());
        let found = discover_cli(
            vars("/usr/bin:/bin:/usr/sbin:/sbin"),
            &ai_claude_code::StaticPathResolver::new(Some(login)),
            Some(Vec::new()),
        );
        assert_eq!(found.executable, Some(shell_bin.join("claude")));
        let child = found.host_env.path().expect("자식 PATH");
        assert!(
            child.starts_with(&shell_bin.display().to_string()),
            "실행 파일 폴더(node 위치)가 맨 앞: {child}"
        );
        assert!(child.contains("/usr/sbin"), "원래 PATH도 남김: {child}");
        std::fs::remove_dir_all(root).expect("정리");
    }

    #[test]
    fn missing_cli_stays_missing_and_keeps_the_process_path() {
        let found = discover_cli(
            vars("/usr/bin:/bin"),
            &ai_claude_code::StaticPathResolver::new(None),
            Some(Vec::new()),
        );
        assert_eq!(found.executable, None);
        assert_eq!(found.host_env.path(), Some("/usr/bin:/bin"));
    }

    #[test]
    fn child_path_drops_duplicates_and_empty_entries() {
        assert_eq!(
            child_path(
                Some("/opt/homebrew/bin::/usr/bin"),
                Some("/usr/bin:/bin"),
                Some(std::path::Path::new("/opt/homebrew/bin/claude")),
            ),
            "/opt/homebrew/bin:/usr/bin:/bin"
        );
    }

    #[test]
    fn bundled_bridge_is_looked_up_next_to_the_running_executable() {
        let location = super::bundled_bridge();
        let expected = std::env::current_exe()
            .expect("실행 파일")
            .parent()
            .expect("폴더")
            .join(super::BRIDGE_FILE_NAME);
        assert_eq!(location.get(), Some(expected.as_path()));
    }
}

#[cfg(test)]
mod product_bus_tests {
    use super::{ProductSubscribers, product_event_bus};
    use application::{Notification, SystemPortError, WriteCompleted};
    use domain::{DomainEvent, InstrumentId};
    use src_tauri::platform::notifier::{
        FilteredNotifier, NotificationDelivery, NotificationMessage, NotificationPreferences,
        NotificationToggle,
    };
    use src_tauri::product_events::UiEventSlot;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    use store_sqlite::{SharedDatabase, SqliteRepositories};

    type Sent = (Notification, NotificationMessage);

    struct ChannelDelivery(mpsc::Sender<Sent>);

    impl NotificationDelivery for ChannelDelivery {
        fn deliver(
            &self,
            notification: Notification,
            message: NotificationMessage,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let _sent = self.0.send((notification, message));
            std::future::ready(Ok(WriteCompleted))
        }
    }

    fn instrument() -> InstrumentId {
        InstrumentId::try_new("005930").expect("종목")
    }

    #[test]
    fn product_event_bus_turns_each_new_signal_into_one_macos_notification() {
        let (sender, receiver) = mpsc::channel();
        let notifier = Arc::new(FilteredNotifier::new(
            ChannelDelivery(sender),
            NotificationPreferences::default(),
        ));
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let (events, _bridge) = product_event_bus(ProductSubscribers {
            notifier: Arc::clone(&notifier),
            audit: Arc::new(SqliteRepositories::new(SharedDatabase::new())),
            clock,
            ui_events: Arc::new(UiEventSlot::<tauri::Wry>::default()),
            intraday: Arc::new(src_tauri::platform::intraday::IntradayWatchSlot::new()),
            signals: Arc::new(src_tauri::signal_flow::SignalPipelineSlot::default()),
        });
        let _cleared = events.publish(DomainEvent::SignalCleared(instrument()));
        let _raised = events.publish(DomainEvent::SignalRaised(instrument()));
        let (notification, message) = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("새 신호 알림");
        assert_eq!(notification, Notification::NewSignal);
        assert_eq!(message.title, "새 매매 신호");
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "신호 해제는 알리지 않는다"
        );
        notifier
            .set_toggle(Notification::NewSignal, NotificationToggle::Disabled)
            .expect("끄기");
        let _muted = events.publish(DomainEvent::SignalRaised(instrument()));
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "꺼 둔 알림은 보내지 않는다"
        );
    }
}

#[cfg(test)]
mod briefing_store_tests {
    use application::{BriefingOutcome, BriefingRecord, BriefingRepository, SecretValue};
    use domain::{
        BriefingId, BriefingKind, CalendarDate, CalendarYear, DayNumber, MonthNumber, UntrustedText,
    };
    use src_tauri::ipc::briefing::{BriefingIpcState, UiBriefingStatus, load_briefings_view};
    use std::sync::Arc;
    use store_sqlite::{SharedDatabase, SqliteRepositories};

    fn finished_at() -> domain::Timestamp {
        broker_toss::TossTimestamp::try_from("2026-11-24T23:20:00Z")
            .expect("clock")
            .into_domain()
    }

    fn clock() -> testkit_ports::SteppedClock {
        testkit_ports::SteppedClock::new(finished_at())
    }

    #[test]
    fn installed_database_serves_saved_briefings_to_the_briefing_ipc() {
        let directory = std::path::PathBuf::from("../../../target/chat2")
            .join(format!("sqlite-briefings-{}", std::process::id()));
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let state = BriefingIpcState::from_repository(Arc::clone(&repositories));
        tauri::async_runtime::block_on(async {
            let missing = load_briefings_view(&state).await;
            assert!(missing.is_err(), "설치 전에는 읽지 못한다");
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let database =
                store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock())
                    .expect("열기");
            assert!(shared.install(database).await.is_none());
            let date = CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(25),
            )
            .expect("date");
            let record = BriefingRecord {
                id: BriefingId::new(BriefingKind::Morning, date),
                finished_at: finished_at(),
                outcome: BriefingOutcome::Completed {
                    body: UntrustedText::try_new("오늘 브리핑").expect("text"),
                    sources: Vec::new(),
                },
            };
            let _saved = repositories.save(&record).await.expect("저장");
            let view = load_briefings_view(&state).await.expect("조회");
            let latest = view.latest.expect("최근 브리핑");
            assert_eq!(latest.body, "오늘 브리핑");
            assert_eq!(latest.status, UiBriefingStatus::Completed);
            assert_eq!(latest.finished_at, "2026-11-25T08:20:00+09:00");
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }
}

#[cfg(test)]
mod ai_composition_tests {
    use super::{ANALYSIS_MCP_SERVER, AiSetup, ProductAiParts, assemble_ai, bridge_config};
    use ai_claude_code::HostEnvironment;
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AnalysisPrompt, CollectionQuery,
        InformationError, IsolationCheck, ModelSelection, ReasoningEffort, ResetTime, RunInput,
        RunPurpose, RunSpec,
    };
    use serde_json::{Value, json};
    use src_tauri::ai_runtime::{BridgeLocation, ChatTurnBook};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use store_sqlite::{SharedDatabase, SqliteRepositories};
    use testkit::fake_ai_cli::{
        EnvPresence, FakeAiMode, FakeAiScenario, InvocationRecord, NonessentialTrafficFlag,
        PreparedFakeAiCli, ReplayScenario, ReplayTurn, TurnStop,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);
    const BRIDGE: &str = "/Applications/Stock Copilot.app/Contents/Resources/mcp-bridge";

    pub(super) fn without_claude(
        clock: &Arc<platform_macos::SystemClock>,
        events: &Arc<application::EventBus>,
        repositories: &Arc<SqliteRepositories>,
    ) -> ProductAiParts {
        let setup = AiSetup {
            executable: None,
            host_env: HostEnvironment::from_pairs(Vec::<(String, String)>::new()),
            work_root: PathBuf::from("/private/tmp/stock-copilot-ai"),
            mcp_base: PathBuf::from("/private/tmp/sc-mcp"),
            bridge: BridgeLocation::new(),
            sandbox: None,
            cli_projects_dir: None,
            turns: ChatTurnBook::new(),
            market: src_tauri::broker_link::BrokerMarketTools::unconnected(),
            portfolio: src_tauri::mcp_portfolio::CatalogPortfolio::default(),
            estimate_runs: src_tauri::estimate_flow::EstimateRunBook::new(),
            app_commands: Arc::default(),
        };
        assemble_ai(setup, clock, events, repositories).expect("AI 조립")
    }

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/private/tmp").join(format!("sc-fdm-{}-{seq}", std::process::id()));
            std::fs::create_dir_all(&path).expect("폴더");
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            match std::fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn analysis_tools() -> Vec<String> {
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string("../../../fixtures/mcp/tools_list.json").expect("골든"),
        )
        .expect("골든 JSON");
        golden
            .as_array()
            .expect("도구 배열")
            .iter()
            .map(|tool| format!("mcp__stock__{}", tool["name"].as_str().expect("이름")))
            .chain(
                src_tauri::ai_commands::COMMAND_POLICIES
                    .iter()
                    .filter(|entry| entry.policy != src_tauri::ai_commands::AiPolicy::Never)
                    .map(|entry| {
                        format!(
                            "mcp__stock__{}",
                            src_tauri::ai_commands::tool_name_of(entry.ipc)
                        )
                    }),
            )
            .collect()
    }

    fn init_line(tools: &[String], servers: &Value) -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": tools,
            "mcp_servers": servers,
            "model": "claude-opus-5-5",
            "permissionMode": "dontAsk",
            "slash_commands": [],
            "skills": [],
            "plugins": ["cc-plugin-privacy"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.322"
        })
        .to_string()
    }

    fn turn(lines: Vec<String>) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: lines,
            stderr_lines: Vec::new(),
            stop_after: TurnStop::Continue,
        }
    }

    fn scenario(init: String) -> FakeAiScenario {
        let result = |turns: u32| {
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": turns})
                .to_string()
        };
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns: vec![
                    turn(vec![init, result(0)]),
                    turn(vec![
                        json!({
                            "type": "assistant",
                            "message": {"model": "claude-opus-5-5", "content": [{"type": "text", "text": "확인했습니다"}]}
                        })
                        .to_string(),
                        result(1),
                    ]),
                ],
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    fn setup(executable: Option<PathBuf>, root: &Path) -> AiSetup {
        let bridge = BridgeLocation::new();
        assert_eq!(bridge.set(PathBuf::from(BRIDGE)), Ok(()));
        AiSetup {
            executable,
            host_env: HostEnvironment::new_clean(
                String::from("/private/tmp/fake-home"),
                String::from("/usr/bin:/bin"),
                String::from("tester"),
                Some(String::from("tester")),
                String::from("/private/tmp"),
            ),
            work_root: root.join("w"),
            mcp_base: root.join("m"),
            bridge,
            sandbox: None,
            cli_projects_dir: Some(root.join("projects")),
            turns: ChatTurnBook::new(),
            market: src_tauri::broker_link::BrokerMarketTools::unconnected(),
            portfolio: src_tauri::mcp_portfolio::CatalogPortfolio::default(),
            estimate_runs: src_tauri::estimate_flow::EstimateRunBook::new(),
            app_commands: Arc::default(),
        }
    }

    fn parts(setup: AiSetup) -> ProductAiParts {
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let events = Arc::new(application::EventBusBuilder::default().build());
        let repositories = Arc::new(SqliteRepositories::new(SharedDatabase::new()));
        assemble_ai(setup, &clock, &events, &repositories).expect("조립")
    }

    fn spec(id: &str, input: RunInput, purpose: RunPurpose) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("id"),
            input,
            ModelSelection::Default,
            ReasoningEffort::Default,
            purpose,
        )
    }

    async fn drain<S: AiEventStream>(stream: &mut S) -> Vec<AiEvent> {
        let mut events = Vec::new();
        while let Ok(Some(event)) = stream.next().await {
            events.push(event);
        }
        events
    }

    fn only_invocation(prepared: &PreparedFakeAiCli) -> InvocationRecord {
        let invocations = prepared.read_invocations().expect("호출 기록");
        assert_eq!(invocations.len(), 1_usize);
        invocations.into_iter().next().expect("호출 하나")
    }

    fn value_after<'a>(argv: &'a [String], flag: &str) -> Vec<&'a str> {
        argv.iter()
            .zip(argv.iter().skip(1))
            .filter(|(left, _right)| *left == flag)
            .map(|(_left, right)| right.as_str())
            .collect()
    }

    fn assert_common_isolation(argv: &[String]) {
        for flag in [
            "-p",
            "--restricted",
            "--disable-slash-commands",
            "--strict-mcp-config",
            "--no-session-persistence",
        ] {
            assert!(argv.iter().any(|arg| arg == flag), "{flag}");
        }
        assert_eq!(value_after(argv, "--permission-mode"), vec!["dontAsk"]);
        assert_eq!(value_after(argv, "--permission-prompts"), vec!["none"]);
        assert!(argv.iter().all(|arg| arg != "--resume"));
    }

    #[test]
    fn composed_chat_run_keeps_isolation_and_connects_through_a_per_run_server() {
        let root = TempRoot::new();
        let tools = analysis_tools();
        let prepared = PreparedFakeAiCli::prepare(
            &root.0,
            "fake-claude",
            &scenario(init_line(
                &tools,
                &json!([{"name": ANALYSIS_MCP_SERVER, "status": "connected"}]),
            )),
        )
        .expect("가짜 CLI");
        let composed = parts(setup(
            Some(prepared.executable_path().to_path_buf()),
            &root.0,
        ));
        let events = tauri::async_runtime::block_on(async {
            let mut stream = composed
                .ai
                .run(spec(
                    "chat-fin-d",
                    RunInput::Analysis(AnalysisPrompt::try_new("보유 종목").expect("prompt")),
                    RunPurpose::Chat,
                ))
                .await
                .expect("분석 실행");
            drain(&mut stream).await
        });
        assert_eq!(
            events.first(),
            Some(&AiEvent::IsolationChecked(IsolationCheck::Passed))
        );
        assert!(events.iter().any(is_text));
        assert_analysis_invocation(&only_invocation(&prepared), tools, &root.0);
        let leftover = std::fs::read_dir(root.0.join("m"))
            .expect("소켓 기반 폴더")
            .count();
        assert_eq!(leftover, 0_usize, "실행이 끝나면 실행별 소켓 폴더가 없다");
    }

    const fn is_text(event: &AiEvent) -> bool {
        match event {
            AiEvent::TextDelta(_) => true,
            AiEvent::IsolationChecked(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => false,
        }
    }

    fn assert_analysis_invocation(invocation: &InvocationRecord, tools: Vec<String>, root: &Path) {
        assert_common_isolation(&invocation.argv);
        assert_eq!(value_after(&invocation.argv, "--tools"), vec![""]);
        let allowed_at = invocation
            .argv
            .iter()
            .position(|arg| arg == "--allowedTools")
            .expect("--allowedTools");
        let allowed: Vec<String> = invocation
            .argv
            .iter()
            .skip(allowed_at.saturating_add(1))
            .take(tools.len())
            .cloned()
            .collect();
        let mut expected = tools;
        expected.sort();
        assert_eq!(allowed, expected);
        let mcp_config = value_after(&invocation.argv, "--mcp-config");
        assert_eq!(mcp_config.len(), 1_usize);
        assert!(
            mcp_config
                .iter()
                .all(|path| path.starts_with(root.join("w").to_str().expect("경로")))
        );
        assert_eq!(invocation.env_observation.mcp_token, EnvPresence::Present);
        assert_eq!(
            invocation.env_observation.anthropic_api_key,
            EnvPresence::Absent
        );
        assert_eq!(
            invocation.env_observation.disable_nonessential_traffic,
            NonessentialTrafficFlag::Disabled
        );
        assert_eq!(
            invocation
                .stdin_lines
                .first()
                .map(|line| line.contains("/usage")),
            Some(true)
        );
    }

    #[test]
    fn composed_collection_run_has_web_search_only_and_no_app_mcp() {
        let root = TempRoot::new();
        let prepared = PreparedFakeAiCli::prepare(
            &root.0,
            "fake-claude",
            &scenario(init_line(&[String::from("WebSearch")], &json!([]))),
        )
        .expect("가짜 CLI");
        let composed = parts(setup(
            Some(prepared.executable_path().to_path_buf()),
            &root.0,
        ));
        let events = tauri::async_runtime::block_on(async {
            let mut stream = composed
                .ai
                .run(spec(
                    "collect-fin-d",
                    RunInput::Collection(CollectionQuery::CodeTopics {
                        instruments: Vec::new(),
                        topics: Vec::new(),
                    }),
                    RunPurpose::WebCollection,
                ))
                .await
                .expect("수집 실행");
            drain(&mut stream).await
        });
        assert_eq!(
            events.first(),
            Some(&AiEvent::IsolationChecked(IsolationCheck::Passed))
        );
        let invocation = only_invocation(&prepared);
        assert_common_isolation(&invocation.argv);
        assert_eq!(value_after(&invocation.argv, "--tools"), vec!["WebSearch"]);
        assert_eq!(
            value_after(&invocation.argv, "--allowedTools"),
            vec!["WebSearch"]
        );
        assert_eq!(
            value_after(&invocation.argv, "--json-schema").len(),
            1_usize
        );
        assert_eq!(invocation.env_observation.mcp_token, EnvPresence::Absent);
        assert!(
            !root.0.join("m").exists(),
            "수집 실행은 앱 MCP를 열지 않는다"
        );
    }

    #[test]
    fn missing_claude_leaves_ai_unavailable_but_keeps_queue_and_jobs_working() {
        let root = TempRoot::new();
        let composed = parts(setup(None, &root.0));
        tauri::async_runtime::block_on(async {
            assert_eq!(
                composed
                    .ai
                    .run(spec(
                        "chat-missing",
                        RunInput::Analysis(AnalysisPrompt::try_new("x").expect("prompt")),
                        RunPurpose::Chat,
                    ))
                    .await
                    .err(),
                Some(InformationError::Unavailable(ResetTime::NotProvided))
            );
            let at = domain::Timestamp::from_parts(
                domain::UnixSeconds::new(1_790_000_000_i64),
                domain::Nanosecond::new(0_u32),
                domain::OffsetSeconds::new(0_i32),
            )
            .expect("시각");
            assert_eq!(
                scheduler::AiJobQueue::request_background_slot(
                    composed.jobs.as_ref(),
                    scheduler::ScheduledJobId::MorningBriefing,
                    at,
                )
                .await,
                Ok(scheduler::AiQueueAdmission::Admitted)
            );
            assert_eq!(
                scheduler::AiJobQueue::request_background_slot(
                    composed.jobs.as_ref(),
                    scheduler::ScheduledJobId::ClosingReview,
                    at,
                )
                .await,
                Ok(scheduler::AiQueueAdmission::PausedForChat),
                "예약 작업은 한 번에 하나"
            );
            assert_eq!(
                scheduler::AiJobQueue::release_background_slot(
                    composed.jobs.as_ref(),
                    scheduler::ScheduledJobId::MorningBriefing,
                    at,
                )
                .await,
                Ok(())
            );
            assert_eq!(
                scheduler::AiJobQueue::request_background_slot(
                    composed.jobs.as_ref(),
                    scheduler::ScheduledJobId::ClosingReview,
                    at,
                )
                .await,
                Ok(scheduler::AiQueueAdmission::Admitted)
            );
        });
    }

    #[test]
    fn alias_linker_matches_collected_titles_with_the_shared_stock_matcher() {
        use src_tauri::collected_links::TitleLinker;
        let samsung = domain::InstrumentId::try_new("005930").expect("종목");
        let aliases = [application::StockAlias {
            instrument: samsung.clone(),
            alias: application::AliasName::try_new("삼성전자").expect("약칭"),
        }];
        let title = |raw: &str| domain::UntrustedText::try_new(raw).expect("제목");
        assert_eq!(
            super::AliasTitleLinker.instruments_in(&title("삼성전자, 4분기 실적 발표"), &aliases),
            vec![samsung]
        );
        assert_eq!(
            super::AliasTitleLinker.instruments_in(&title("환율 하락"), &aliases),
            Vec::<domain::InstrumentId>::new()
        );
        assert_eq!(
            super::AliasTitleLinker.instruments_in(&title("삼성전자"), &[]),
            Vec::<domain::InstrumentId>::new()
        );
    }

    #[test]
    fn bridge_config_points_the_cli_at_the_bundled_bridge_with_a_token_reference_only() {
        let config = bridge_config(
            Path::new(BRIDGE),
            Path::new("/private/tmp/sc-mcp/r0123/m.sock"),
        )
        .expect("브리지 설정");
        let rendered: Value =
            serde_json::from_str(&config.render_mcp_json(None).expect("렌더")).expect("JSON");
        assert_eq!(
            rendered,
            json!({"mcpServers": {"stock": {
                "type": "stdio",
                "command": BRIDGE,
                "args": ["--socket", "/private/tmp/sc-mcp/r0123/m.sock"],
                "env": {"STOCK_MCP_TOKEN": "${STOCK_MCP_TOKEN}"}
            }}})
        );
    }
}

#[cfg(test)]
mod key_provision_tests {
    use application::SecretValue;
    use src_tauri::database::{DatabaseKeyProvision, DatabaseStatus, open_provisioned_database};
    use src_tauri::lifecycle::{DatabaseClosure, Lifecycle};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemorySecretStore;

    const FAKE_KEY: [u8; 32] = *b"fake-db-key-9c41e7-never-on-disk";

    fn clock() -> Arc<testkit_ports::SteppedClock> {
        Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-25T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ))
    }

    fn provision(count: Arc<AtomicUsize>) -> DatabaseKeyProvision {
        DatabaseKeyProvision(Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(SecretValue::new(FAKE_KEY.to_vec()))
        }))
    }

    async fn launch(
        secrets: &MemorySecretStore,
        count: &Arc<AtomicUsize>,
        path: &std::path::Path,
    ) -> DatabaseStatus {
        let state = open_provisioned_database(
            secrets,
            &provision(Arc::clone(count)),
            Arc::new(super::database_factory(clock())),
            store_sqlite::DatabasePath::new(path.to_path_buf()),
        )
        .await;
        let status = state.status();
        let lifecycle = Lifecycle::new(tauri::async_runtime::spawn(std::future::pending()), state);
        lifecycle.shutdown().await;
        let guard = lifecycle.outcome().lock().await;
        match &guard.as_ref().expect("종료 결과").database {
            Ok(DatabaseClosure::Closed(_receipt)) => {}
            Ok(DatabaseClosure::NotOpened) => panic!("열린 DB의 명시적 종료 필요"),
            Err(error) => panic!("종료 실패: {error:?}"),
        }
        status
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        (0..=haystack.len().saturating_sub(needle.len()))
            .any(|start| haystack.get(start..start.saturating_add(needle.len())) == Some(needle))
    }

    #[test]
    fn first_launch_creates_the_key_and_the_next_launch_reopens_without_key_bytes_on_disk() {
        let directory = std::path::PathBuf::from("../../../target/fin-c")
            .join(format!("product-key-{}", std::process::id()));
        let path = directory.join("data").join("stock-copilot.db");
        let secrets = MemorySecretStore::default();
        for _ in 0..4_usize {
            secrets.script_load().push(MemoryReply::Memory);
        }
        secrets.script_store().push(MemoryReply::Memory);
        let count = Arc::new(AtomicUsize::new(0));
        tauri::async_runtime::block_on(async {
            assert_eq!(launch(&secrets, &count, &path).await, DatabaseStatus::Ready);
            assert_eq!(launch(&secrets, &count, &path).await, DatabaseStatus::Ready);
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "키는 첫 실행에만 만든다");
        let hex: Vec<u8> = FAKE_KEY
            .iter()
            .flat_map(|byte| format!("{byte:02x}").into_bytes())
            .collect();
        let mut scanned = 0_usize;
        for entry in std::fs::read_dir(path.parent().expect("폴더")).expect("목록") {
            let bytes = std::fs::read(entry.expect("항목").path()).expect("파일");
            assert!(!contains(&bytes, &FAKE_KEY), "원시 키가 파일에 있음");
            assert!(!contains(&bytes, &hex), "16진 키가 파일에 있음");
            scanned = scanned.saturating_add(1);
        }
        assert!(scanned >= 1, "DB 파일을 검사함");
        std::fs::remove_dir_all(directory).expect("정리");
    }

    #[test]
    fn product_secrets_use_the_development_keychain_profile_outside_tauri_build() {
        assert_eq!(
            super::product_secrets().profile(),
            &secrets_keychain::KeychainProfile::development()
        );
    }
}

#[cfg(test)]
mod collector_store_tests {
    use application::{
        AiReadPolicy, CollectedEntry, CollectedObservation, ConditionalGet, DataAvailability,
        FeedCursor, FeedPosition, FeedSource, InformationError, NewsFeed, NewsMetadata,
        PublicationTime, Retention, RetryDelay, SecretValue, SourceState, SourceStatusRepository,
        SystemFailure, SystemPort, SystemPortError, UserAgentRequirement,
    };
    use domain::{DataSource, NewsItem, NewsSource, NormalizedUrl, UntrustedText};
    use src_tauri::platform::collector::{AppFeedRunner, FeedRunner};
    use src_tauri::platform::notifier::{FilteredNotifier, NotificationPreferences};
    use src_tauri::platform::system_notification::SystemNotificationDelivery;
    use std::sync::{Arc, RwLock};
    use store_sqlite::{SharedDatabase, SqliteRepositories};

    type Items = DataAvailability<(Vec<CollectedObservation>, FeedCursor)>;

    struct CountingFeed {
        calls: Arc<RwLock<Vec<FeedCursor>>>,
        reply: RwLock<Option<Items>>,
    }

    impl NewsFeed for CountingFeed {
        fn source(&self) -> FeedSource {
            FeedSource::News(NewsSource::Yonhap)
        }
        fn metadata(&self) -> NewsMetadata {
            NewsMetadata::new(
                Retention::NotStored,
                AiReadPolicy::TitleOnly,
                ConditionalGet::Unsupported,
                UserAgentRequirement::Standard,
            )
        }
        fn new_items(
            &self,
            since: FeedCursor,
        ) -> impl Future<Output = Result<Items, InformationError>> + Send {
            self.calls.write().expect("잠금").push(since);
            let reply = self.reply.write().expect("잠금").take();
            std::future::ready(Ok(reply.unwrap_or(DataAvailability::NoData)))
        }
    }

    fn position(value: &str) -> FeedCursor {
        FeedCursor::Continue(FeedPosition::new(
            UntrustedText::try_new(value).expect("텍스트"),
        ))
    }

    fn one_article_feed(calls: &Arc<RwLock<Vec<FeedCursor>>>) -> CountingFeed {
        let collected_at = broker_toss::TossTimestamp::try_from("2026-11-25T00:00:00Z")
            .expect("clock")
            .into_domain();
        let item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://example.com/fin-e").expect("주소"),
            UntrustedText::try_new("수집 기사").expect("제목"),
            None,
        );
        CountingFeed {
            calls: Arc::clone(calls),
            reply: RwLock::new(Some(DataAvailability::Available((
                vec![CollectedObservation::new(
                    CollectedEntry::News(item),
                    PublicationTime::NotProvided,
                    collected_at,
                )],
                position("next"),
            )))),
        }
    }

    type Ports = src_tauri::ports::Ports<super::AppPortTypes>;

    fn mock_app(repositories: &Arc<SqliteRepositories>) -> tauri::App<tauri::test::MockRuntime> {
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let notifier = Arc::new(FilteredNotifier::new(
            SystemNotificationDelivery::<tauri::Wry>::new(Arc::default()),
            NotificationPreferences::default(),
        ));
        let events = Arc::new(application::EventBusBuilder::default().build());
        let ai = super::ai_composition_tests::without_claude(&clock, &events, repositories);
        let toss = super::product_toss(&clock, &events, repositories).expect("토스 조립");
        let ports: Ports =
            super::product_ports(repositories, (notifier, &toss), &clock, events, ai);
        tauri::test::mock_builder()
            .manage(ports)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱")
    }

    #[test]
    fn startup_restore_selects_the_last_synced_account_before_toss_answers() {
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let directory = std::path::PathBuf::from("../../../target/fin-hold")
            .join(format!("restore-{}", std::process::id()));
        let system = platform_macos::SystemClock::new().expect("시계");
        let observed = broker_toss::TossTimestamp::try_from("2026-11-25T07:00:00Z")
            .expect("clock")
            .into_domain();
        tauri::async_runtime::block_on(async {
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let database =
                store_sqlite::Database::open(&path, &SecretValue::new(vec![61; 32]), &system)
                    .expect("열기");
            assert!(shared.install(database).await.is_none());
            let guard = shared.read().await;
            let _written = store_sqlite::HoldingsStore::new(guard.as_ref().expect("열림"))
                .record_holdings(store_sqlite::HoldingsSnapshot {
                    account: domain::AccountId::new(413),
                    observed_at: observed,
                    holdings: Vec::new(),
                })
                .await
                .expect("스냅샷");
        });
        let clock = Arc::new(system);
        let notifier = Arc::new(FilteredNotifier::new(
            SystemNotificationDelivery::<tauri::Wry>::new(Arc::default()),
            NotificationPreferences::default(),
        ));
        let events = Arc::new(application::EventBusBuilder::default().build());
        let ai = super::ai_composition_tests::without_claude(&clock, &events, &repositories);
        let toss = super::product_toss(&clock, &events, &repositories).expect("토스 조립");
        let ports: Ports =
            super::product_ports(&repositories, (notifier, &toss), &clock, events, ai);
        let accounts = ports.saved_accounts.clone();
        let app = tauri::test::mock_builder()
            .manage(ports)
            .manage(Arc::clone(&repositories))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱");
        tauri::async_runtime::block_on(super::restore_saved_holdings(app.handle()));
        assert_eq!(accounts.current(), vec![domain::AccountId::new(413)]);
        let _closed = tauri::async_runtime::block_on(shared.close());
        let _cleaned = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn app_runner_stores_collected_news_and_source_status_only_once_the_database_is_open() {
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let app = mock_app(&repositories);
        let calls = Arc::new(RwLock::new(Vec::new()));
        let runner =
            AppFeedRunner::<tauri::test::MockRuntime, super::AppPortTypes, CountingFeed>::new(
                app.handle().clone(),
                repositories.as_ref().clone(),
                one_article_feed(&calls),
            );
        let directory = std::path::PathBuf::from("../../../target/fin-e")
            .join(format!("collector-{}", std::process::id()));
        tauri::async_runtime::block_on(async {
            assert_eq!(
                runner.collect(FeedCursor::Start).await,
                Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::Unavailable(RetryDelay::Unknown)
                ))
            );
            assert_eq!(*calls.read().expect("잠금"), Vec::<FeedCursor>::new());
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let clock = platform_macos::SystemClock::new().expect("시계");
            let database =
                store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock)
                    .expect("열기");
            assert!(shared.install(database).await.is_none());
            let collected = runner.collect(FeedCursor::Start).await.expect("수집");
            assert_eq!(collected.cursor, position("next"));
            assert_eq!(*collected.news.as_u32(), 1);
            let status = repositories
                .find(DataSource::Yonhap)
                .await
                .expect("상태")
                .expect("기록");
            assert_eq!(status.state(), SourceState::Healthy);
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }

    #[test]
    fn app_runner_records_collection_time_so_news_tools_can_read_the_item() {
        use application::{CollectedItemReads, CollectedKind};
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let app = mock_app(&repositories);
        let calls = Arc::new(RwLock::new(Vec::new()));
        let runner =
            AppFeedRunner::<tauri::test::MockRuntime, super::AppPortTypes, CountingFeed>::new(
                app.handle().clone(),
                repositories.as_ref().clone(),
                one_article_feed(&calls),
            );
        let directory = std::path::PathBuf::from("../../../target/fin-d")
            .join(format!("collector-times-{}", std::process::id()));
        tauri::async_runtime::block_on(async {
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let clock = platform_macos::SystemClock::new().expect("시계");
            let database =
                store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock)
                    .expect("열기");
            assert!(shared.install(database).await.is_none());
            let _collected = runner.collect(FeedCursor::Start).await.expect("수집");
            let collected_at = broker_toss::TossTimestamp::try_from("2026-11-25T00:00:00Z")
                .expect("clock")
                .into_domain();
            let items = repositories
                .collected_since(CollectedKind::News, collected_at)
                .await
                .expect("조회");
            assert_eq!(items.len(), 1_usize);
            let item = items.first().expect("항목");
            assert_eq!(item.collected_at, collected_at);
            assert_eq!(item.published_at, PublicationTime::NotProvided);
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }

    #[test]
    fn daily_valuation_without_market_calendar_fails_without_recording_and_skips_backfill() {
        use scheduler::{DailyValuationPort, JobError};
        use src_tauri::platform::scheduled_ports::AppStoredJobs;
        let repositories = Arc::new(SqliteRepositories::new(SharedDatabase::new()));
        let app = mock_app(&repositories);
        let jobs = AppStoredJobs::<tauri::test::MockRuntime, super::AppPortTypes>::new(
            app.handle().clone(),
            repositories.as_ref().clone(),
        );
        let now = broker_toss::TossTimestamp::try_from("2026-11-25T07:00:00Z")
            .expect("clock")
            .into_domain();
        tauri::async_runtime::block_on(async {
            assert_eq!(
                jobs.record_today(now).await,
                Err(JobError::PortUnavailable(String::from("market-calendar")))
            );
            assert_eq!(jobs.missing_days(now).await, Ok(Vec::new()));
        });
    }
}

#[cfg(test)]
mod briefing_assembly_tests {
    use application::{AiEvent, BriefingOutcome, BriefingRepository, IsolationCheck, SecretValue};
    use domain::UntrustedText;
    use scheduler::{
        AiJobQueue, AiQueueAdmission, AiQueueError, JobRegistry, JobRunOutcome, ScheduledJobId,
        Scheduler,
    };
    use src_tauri::database::DatabaseFactory;
    use src_tauri::platform::scheduling::register_stored_jobs;
    use src_tauri::ports::{PortTypes, Ports};
    use src_tauri::unavailable::{Unavailable, UnavailableFeed};
    use std::sync::Arc;
    use store_sqlite::{SharedDatabase, SqliteRepositories};
    use testkit_ports::chat_fakes::{ScriptedAiEventStream, ScriptedAiProvider};

    #[derive(Debug, Default)]
    struct AdmittingQueue;

    impl AiJobQueue for AdmittingQueue {
        fn request_background_slot(
            &self,
            _job: ScheduledJobId,
            _now: domain::Timestamp,
        ) -> impl Future<Output = Result<AiQueueAdmission, AiQueueError>> + Send {
            std::future::ready(Ok(AiQueueAdmission::Admitted))
        }
        fn release_background_slot(
            &self,
            _job: ScheduledJobId,
            _now: domain::Timestamp,
        ) -> impl Future<Output = Result<(), AiQueueError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    #[derive(Debug)]
    struct AssemblyPorts;
    impl PortTypes for AssemblyPorts {
        type BrokerRead = testkit_ports::scripted::ScriptedBrokerRead;
        type MarketInfo = Unavailable;
        type MacroSeries = Unavailable;
        type NewsFeed = UnavailableFeed;
        type Fundamentals = Unavailable;
        type AiProvider = ScriptedAiProvider;
        type ChatRepository = testkit_ports::chat_fakes::MemoryChatRepository;
        type ChatRunLedger = testkit_ports::chat_fakes::MemoryChatRunLedger;
        type ChatQueuePort = testkit_ports::chat_fakes::ScriptedChatQueue;
        type WarmChatRuntime = testkit_ports::chat_fakes::ScriptedWarmChatRuntime;
        type AiJobQueue = AdmittingQueue;
        type PortfolioRepository = testkit_ports::memory::MemoryPortfolioRepository;
        type TransactionRepository = Unavailable;
        type MarketDataRepository = Unavailable;
        type NewsRepository = Unavailable;
        type ThesisRepository = Unavailable;
        type EstimateRepository = Unavailable;
        type SignalRepository = Unavailable;
        type ProposalRepository = Unavailable;
        type ValuationRepository = Unavailable;
        type SettingsRepository = testkit_ports::memory::MemorySettingsRepository;
        type AuditLog = Unavailable;
        type SecretStore = testkit_ports::system_fakes::MemorySecretStore;
        type Clock = testkit_ports::SteppedClock;
        type Notifier = Unavailable;
        type UserPresence = testkit_ports::ScriptedPresence;
    }

    fn utc(text: &str) -> domain::Timestamp {
        broker_toss::TossTimestamp::try_from(text)
            .expect("clock")
            .into_domain()
    }

    fn ports(
        provider: Arc<ScriptedAiProvider>,
        clock: Arc<testkit_ports::SteppedClock>,
        events: application::EventBus,
    ) -> Ports<AssemblyPorts> {
        Ports {
            broker_read: Arc::new(testkit_ports::scripted::ScriptedBrokerRead::default()),
            market_info: Arc::new(Unavailable),
            macro_series: Arc::new(Unavailable),
            news_feeds: Vec::new(),
            fundamentals: Arc::new(Unavailable),
            ai_provider: provider,
            chat_repository: Arc::new(testkit_ports::chat_fakes::MemoryChatRepository::default()),
            chat_ledger: Arc::new(testkit_ports::chat_fakes::MemoryChatRunLedger::default()),
            chat_queue: Arc::new(testkit_ports::chat_fakes::ScriptedChatQueue::default()),
            warm_chat: Arc::new(testkit_ports::chat_fakes::ScriptedWarmChatRuntime::default()),
            ai_jobs: Arc::new(AdmittingQueue),
            portfolios: Arc::new(testkit_ports::memory::MemoryPortfolioRepository::default()),
            transactions: Arc::new(Unavailable),
            market_data: Arc::new(Unavailable),
            news: Arc::new(Unavailable),
            theses: Arc::new(Unavailable),
            estimates: Arc::new(Unavailable),
            signals: Arc::new(Unavailable),
            proposals: Arc::new(Unavailable),
            valuations: Arc::new(Unavailable),
            settings: Arc::new(testkit_ports::memory::MemorySettingsRepository::default()),
            audit: Arc::new(Unavailable),
            secrets: Arc::new(testkit_ports::system_fakes::MemorySecretStore::default()),
            clock,
            notifier: Arc::new(Unavailable),
            presence: Arc::new(testkit_ports::ScriptedPresence::new([], [])),
            events: Arc::new(events),
            saved_accounts: src_tauri::broker_link::AccountSlot::default(),
            database: Arc::new(DatabaseFactory(Box::new(|_path, _key| {
                Err(application::SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::InvalidData,
                ))
            }))),
        }
    }

    fn assert_morning_executed(report: &scheduler::TickReport) {
        let morning = report
            .records
            .iter()
            .find(|record| record.id == ScheduledJobId::MorningBriefing)
            .expect("아침 브리핑 실행");
        match &morning.outcome {
            JobRunOutcome::Executed(_) => {}
            other @ (JobRunOutcome::SkippedMissedWindow
            | JobRunOutcome::DeferredByAiQueue { .. }
            | JobRunOutcome::Failed(_)) => panic!("실행되지 않음: {other:?}"),
        }
    }

    fn mock_app(
        provider: &Arc<ScriptedAiProvider>,
        clock: &Arc<testkit_ports::SteppedClock>,
        events: application::EventBus,
    ) -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .manage(ports(Arc::clone(provider), Arc::clone(clock), events))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱")
    }

    fn registry(
        app: &tauri::App<tauri::test::MockRuntime>,
        repositories: &SqliteRepositories,
        inbox: Arc<scheduler::IntradayWatchInbox>,
    ) -> JobRegistry {
        let mut registry = JobRegistry::new();
        register_stored_jobs::<tauri::test::MockRuntime, AssemblyPorts>(
            &mut registry,
            app.handle(),
            repositories,
            None,
            inbox,
        )
        .expect("등록");
        registry
    }

    #[test]
    fn admitted_morning_briefing_runs_through_the_scheduler_and_is_stored() {
        let provider = Arc::new(ScriptedAiProvider::default());
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(UntrustedText::try_new("오늘 시장 요약").expect("text")),
            ])));
        let clock = Arc::new(testkit_ports::SteppedClock::new(utc(
            "2026-11-24T23:19:30Z",
        )));
        let shared = SharedDatabase::new();
        let repositories = SqliteRepositories::new(shared.clone());
        let app = mock_app(
            &provider,
            &clock,
            application::EventBusBuilder::default().build(),
        );
        let registry = registry(
            &app,
            &repositories,
            Arc::new(scheduler::IntradayWatchInbox::new()),
        );
        let scheduler =
            Scheduler::new(Arc::clone(&clock), Arc::new(AdmittingQueue), registry).expect("일정");
        let directory = std::path::PathBuf::from("../../../target/fin-e")
            .join(format!("briefing-assembly-{}", std::process::id()));
        tauri::async_runtime::block_on(async {
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let database = store_sqlite::Database::open(
                &path,
                &SecretValue::new(vec![83; 32]),
                clock.as_ref(),
            )
            .expect("열기");
            assert!(shared.install(database).await.is_none());
            let _startup = scheduler.tick_once().await;
            clock
                .advance_to(utc("2026-11-24T23:20:05Z"))
                .expect("08:20 KST");
            let report = scheduler.tick_once().await;
            assert_morning_executed(&report);
            let saved = repositories.recent().await.expect("조회");
            let latest = saved.first().expect("기록");
            assert_eq!(latest.id.kind(), domain::BriefingKind::Morning);
            match &latest.outcome {
                BriefingOutcome::Completed { body, .. } => {
                    assert_eq!(
                        body,
                        &UntrustedText::try_new("오늘 시장 요약").expect("text")
                    );
                }
                BriefingOutcome::Failed(failure) => panic!("실패 기록: {failure:?}"),
            }
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }

    #[test]
    fn price_move_event_wakes_the_intraday_watch_job_and_starts_one_monitoring_run() {
        use application::{AiEvent, RunPurpose};
        use src_tauri::platform::intraday::{BridgedIntradayWatch, IntradayWatchSlot};
        use testkit_ports::chat_fakes::AiProviderCall;
        let provider = Arc::new(ScriptedAiProvider::default());
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
            ])));
        let clock = Arc::new(testkit_ports::SteppedClock::new(utc(
            "2026-11-25T01:00:00Z",
        )));
        let slot = Arc::new(IntradayWatchSlot::new());
        let mut bus = application::EventBusBuilder::default();
        bus.subscribe(
            IntradayWatchSlot::subscribed_kinds(),
            BridgedIntradayWatch(Arc::clone(&slot)),
        );
        let app = mock_app(&provider, &clock, bus.build());
        let inbox = Arc::new(scheduler::IntradayWatchInbox::new());
        let registry = registry(
            &app,
            &SqliteRepositories::new(SharedDatabase::new()),
            Arc::clone(&inbox),
        );
        let scheduler =
            Scheduler::new(Arc::clone(&clock), Arc::new(AdmittingQueue), registry).expect("일정");
        slot.bind(scheduler::IntradayWatchSubscriber::new(
            inbox,
            scheduler.triggers(),
        ));
        let ports = tauri::Manager::state::<Ports<AssemblyPorts>>(&app);
        let _delivery = ports.events.publish(domain::DomainEvent::PriceMoved(
            domain::InstrumentId::try_new("005930").expect("종목"),
        ));
        tauri::async_runtime::block_on(async {
            let report = scheduler.tick_once().await;
            let watch = report
                .records
                .iter()
                .find(|record| record.id == ScheduledJobId::IntradayWatch)
                .expect("장중 감시 실행");
            assert!(matches_executed(&watch.outcome), "{:?}", watch.outcome);
        });
        let purposes: Vec<RunPurpose> = provider
            .calls()
            .iter()
            .filter_map(|call| match call {
                AiProviderCall::Run(spec) => Some(spec.purpose()),
                AiProviderCall::Resume { .. }
                | AiProviderCall::Usage
                | AiProviderCall::Interrupt(_)
                | AiProviderCall::VerifyIsolation(_) => None,
            })
            .collect();
        assert_eq!(purposes, vec![RunPurpose::IntradayMonitoring]);
    }

    fn matches_executed(outcome: &JobRunOutcome) -> bool {
        match outcome {
            JobRunOutcome::Executed(_) => true,
            JobRunOutcome::SkippedMissedWindow
            | JobRunOutcome::DeferredByAiQueue { .. }
            | JobRunOutcome::Failed(_) => false,
        }
    }

    fn settings_control(
        app: &tauri::App<tauri::test::MockRuntime>,
        clock: &Arc<testkit_ports::SteppedClock>,
        store: &Arc<SqliteRepositories>,
    ) -> src_tauri::platform::job_settings::SettingsControl<
        testkit_ports::SteppedClock,
        AdmittingQueue,
        SqliteRepositories,
    > {
        use src_tauri::platform::job_settings::{AppJobSchedules, SettingsControl};
        let scheduler = Arc::new(
            Scheduler::new(
                Arc::clone(clock),
                Arc::new(AdmittingQueue),
                registry(
                    app,
                    store.as_ref(),
                    Arc::new(scheduler::IntradayWatchInbox::new()),
                ),
            )
            .expect("일정"),
        );
        SettingsControl::new(
            scheduler::ScheduleSettings::new(
                scheduler,
                Arc::new(AppJobSchedules::new(Arc::clone(store))),
                application::EventBusBuilder::default().build(),
            ),
            Arc::clone(store),
        )
    }

    async fn install_database(
        shared: &SharedDatabase,
        directory: &std::path::Path,
        clock: &testkit_ports::SteppedClock,
    ) {
        let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
        src_tauri::database::prepare_directory(&path).expect("폴더");
        let database = store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), clock)
            .expect("열기");
        assert!(shared.install(database).await.is_none());
    }

    async fn record_one_skip(store: &Arc<SqliteRepositories>) {
        use scheduler::JobRecordSink;
        use src_tauri::platform::job_settings::job_record_queue;
        let writer = {
            let (sink, writer) = job_record_queue(Arc::clone(store));
            sink.record(&scheduler::JobExecutionRecord {
                id: ScheduledJobId::MorningBriefing,
                scheduled_for: utc("2026-11-24T22:45:00Z"),
                evaluated_at: utc("2026-11-24T22:45:01Z"),
                outcome: JobRunOutcome::SkippedMissedWindow,
            });
            writer
        };
        writer.await.expect("기록 완료");
    }

    #[test]
    fn saved_schedule_change_is_restored_after_restart_and_job_runs_are_persisted() {
        use scheduler::{JobEnablement, JobScheduleChange, KstTimeOfDay};
        use src_tauri::platform::job_settings::ScheduleControl;
        let provider = Arc::new(ScriptedAiProvider::default());
        let clock = Arc::new(testkit_ports::SteppedClock::new(utc(
            "2026-11-24T20:00:00Z",
        )));
        let shared = SharedDatabase::new();
        let store = Arc::new(SqliteRepositories::new(shared.clone()));
        let app = mock_app(
            &provider,
            &clock,
            application::EventBusBuilder::default().build(),
        );
        let directory = std::path::PathBuf::from("../../../target/fin-e")
            .join(format!("job-settings-{}", std::process::id()));
        tauri::async_runtime::block_on(async {
            install_database(&shared, &directory, clock.as_ref()).await;
            let first = settings_control(&app, &clock, &store);
            let changed = first
                .change(JobScheduleChange {
                    id: ScheduledJobId::MorningBriefing,
                    enabled: JobEnablement::Disabled,
                    time: Some(KstTimeOfDay::try_new(7, 45).expect("시각")),
                })
                .await
                .expect("변경");
            assert_eq!(changed.config.enabled, JobEnablement::Disabled);
            let restarted = settings_control(&app, &clock, &store);
            let report = restarted.restore().await.expect("복원");
            assert_eq!(report.applied, vec![ScheduledJobId::MorningBriefing]);
            let morning = restarted
                .configurable_jobs()
                .await
                .into_iter()
                .find(|job| job.id == ScheduledJobId::MorningBriefing)
                .expect("아침 브리핑");
            assert_eq!(morning.config.enabled, JobEnablement::Disabled);
            assert_eq!(
                morning.config.schedule,
                scheduler::ScheduleRule::DailyKst(KstTimeOfDay::try_new(7, 45).expect("시각"))
            );
            record_one_skip(&store).await;
            let runs = restarted
                .recent_runs(ScheduledJobId::MorningBriefing)
                .await
                .expect("기록 조회");
            assert_eq!(runs.len(), 1);
            assert_eq!(
                runs[0].result,
                application::JobRunResult::SkippedMissedWindow
            );
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }
}

#[cfg(test)]
mod toss_assembly_tests {
    use application::SecretValue;
    use src_tauri::broker_link::{AccountSelector, AccountSlot};
    use src_tauri::toss_link::{TossConnectionState, TossKeys, TossLink, TossLinkBackend};
    use std::sync::Arc;
    use testkit::ScriptedHttpServer;
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemorySecretStore;

    const ID_SENTINEL: &str = "client-id-sentinel-7f3a";
    const SECRET_SENTINEL: &str = "client-secret-sentinel-91bc";

    fn http(status: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn token_ok() -> Vec<u8> {
        http(
            "200 OK",
            r#"{"access_token":"fake-access-token","token_type":"Bearer","expires_in":3600}"#,
        )
    }

    fn access_denied() -> Vec<u8> {
        http("403 Forbidden", r#"{"error":"access_denied"}"#)
    }

    fn accounts() -> Vec<u8> {
        http(
            "200 OK",
            r#"{"result":[{"accountNo":"00000000419","accountSeq":27,"accountType":"PENSION_SAVINGS"},{"accountNo":"00000000731","accountSeq":413,"accountType":"BROKERAGE"}]}"#,
        )
    }

    fn keys() -> TossKeys {
        TossKeys::new(
            SecretValue::new(ID_SENTINEL.as_bytes().to_vec()),
            SecretValue::new(SECRET_SENTINEL.as_bytes().to_vec()),
        )
    }

    fn memory_secrets() -> Arc<MemorySecretStore> {
        let secrets = Arc::new(MemorySecretStore::default());
        for _ in 0_u8..24_u8 {
            secrets.script_load().push(MemoryReply::Memory);
            secrets.script_store().push(MemoryReply::Memory);
        }
        secrets
    }

    #[test]
    fn pasted_keys_reach_fake_toss_then_ip_retry_connects_and_selects_the_brokerage_account() {
        let server =
            ScriptedHttpServer::start(vec![access_denied(), token_ok(), accounts(), accounts()])
                .expect("가짜 토스");
        let secrets = memory_secrets();
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let events = Arc::new(application::EventBus::default());
        let broker = Arc::new(
            super::live_toss_read(
                &broker_toss::TossEndpoint::local(server.port()).expect("주소"),
                Arc::clone(&secrets),
                Arc::clone(&clock),
                Arc::clone(&events),
            )
            .expect("어댑터"),
        );
        let slot = AccountSlot::default();
        let link = TossLink::new(
            Arc::clone(&secrets),
            Arc::new(super::LiveProbe(Arc::clone(&broker))),
            Arc::new(AccountSelector::new(
                Arc::clone(&broker),
                slot.clone(),
                events,
            )),
            clock,
        );
        tauri::async_runtime::block_on(async {
            assert_eq!(
                link.submit(keys()).await,
                Ok(TossConnectionState::IpNotAllowed)
            );
            assert_eq!(slot.current(), Vec::new());
            assert_eq!(
                link.submit(keys()).await,
                Ok(TossConnectionState::Connected)
            );
            assert_eq!(slot.current(), vec![domain::AccountId::new(413)]);
            assert_eq!(link.check().await, Ok(TossConnectionState::Connected));
        });
        let requests = String::from_utf8(server.finish().expect("요청")).expect("UTF-8");
        assert_eq!(requests.matches("grant_type=client_credentials").count(), 2);
        assert_eq!(requests.matches("GET /api/v1/accounts").count(), 2);
        assert!(requests.contains(ID_SENTINEL));
        assert!(requests.contains(SECRET_SENTINEL));
        assert!(!format!("{link:?}").contains(SECRET_SENTINEL));
    }

    fn ok(body: &serde_json::Value) -> Vec<u8> {
        http("200 OK", &body.to_string())
    }

    fn listing(rows: &serde_json::Value) -> Vec<u8> {
        ok(&serde_json::json!({ "result": rows }))
    }

    fn kr_listing() -> serde_json::Value {
        serde_json::json!([{"symbol":"005930","name":"삼성전자","securityType":"STOCK","isCommonShare":true,"isinCode":"KR7005930003"}])
    }

    fn kr_details() -> serde_json::Value {
        serde_json::json!([{"symbol":"005930","name":"삼성전자","englishName":"Samsung","isinCode":"KR7005930003","market":"KOSPI","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"KRW","sharesOutstanding":"91","koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":false,"krxTradingSuspended":false,"nxtTradingSuspended":null}}])
    }

    fn us_listing() -> serde_json::Value {
        serde_json::json!([{"symbol":"AAPL","name":"애플","securityType":"STOCK","isCommonShare":true,"isinCode":"US0378331005"}])
    }

    fn us_details() -> serde_json::Value {
        serde_json::json!([{"symbol":"AAPL","name":"애플","englishName":"Apple","isinCode":"US0378331005","market":"NASDAQ","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"USD","sharesOutstanding":"15000","koreanMarketDetail":null}])
    }

    fn holdings() -> Vec<u8> {
        let recorded: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                "../../../crates/adapters/broker-toss/tests/recordings/t05-holdings.json",
            )
            .expect("녹화"),
        )
        .expect("JSON");
        ok(&recorded["body"])
    }

    fn buying_power(currency: &str, amount: &str) -> Vec<u8> {
        ok(&serde_json::json!({"result":{"currency":currency,"cashBuyingPower":amount}}))
    }

    fn brokerage_accounts() -> Vec<u8> {
        http(
            "200 OK",
            r#"{"result":[{"accountNo":"00000000731","accountSeq":413,"accountType":"BROKERAGE"}]}"#,
        )
    }

    fn master_and_holdings_script() -> Vec<Vec<u8>> {
        let empty = serde_json::json!([]);
        vec![
            token_ok(),
            brokerage_accounts(),
            listing(&kr_listing()),
            listing(&empty),
            listing(&empty),
            listing(&kr_details()),
            listing(&empty),
            listing(&us_listing()),
            listing(&empty),
            listing(&empty),
            listing(&us_details()),
            brokerage_accounts(),
            holdings(),
            buying_power("KRW", "5000000"),
            buying_power("USD", "3500.525"),
            ok(
                &serde_json::json!({"result":{"baseCurrency":"USD","quoteCurrency":"KRW","rate":"1381.7","midRate":"1374.2","basisPoint":"54.58","rateChangeType":"UP","validFrom":"2000-01-01T00:00:00+09:00","validUntil":"2100-01-01T00:00:00+09:00"}}),
            ),
        ]
    }

    async fn open_store(directory: &std::path::Path) -> Arc<store_sqlite::SqliteRepositories> {
        let shared = store_sqlite::SharedDatabase::new();
        let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
        src_tauri::database::prepare_directory(&path).expect("폴더");
        let clock = platform_macos::SystemClock::new().expect("시계");
        let database = store_sqlite::Database::open(&path, &SecretValue::new(vec![71; 32]), &clock)
            .expect("열기");
        assert!(shared.install(database).await.is_none());
        Arc::new(store_sqlite::SqliteRepositories::new(shared))
    }

    async fn store_keys(secrets: &MemorySecretStore) {
        for (key, value) in [
            (application::SecretKey::TossClientId, ID_SENTINEL),
            (application::SecretKey::TossClientSecret, SECRET_SENTINEL),
        ] {
            let _stored = application::SecretStore::store(
                secrets,
                key,
                SecretValue::new(value.as_bytes().to_vec()),
            )
            .await
            .expect("키");
        }
    }

    type TestBroker = super::ProductBrokerFor<MemorySecretStore>;

    struct HoldingsParts {
        listener: src_tauri::holdings_link::HoldingsListener,
        names: src_tauri::holdings_link::InstrumentDirectory,
        portfolios: Arc<store_sqlite::SqlitePortfolioStore>,
        slot: AccountSlot,
    }

    fn holdings_parts(
        broker: &Arc<TestBroker>,
        repositories: &Arc<store_sqlite::SqliteRepositories>,
        events: &Arc<application::EventBus>,
    ) -> HoldingsParts {
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let portfolios = Arc::new(store_sqlite::SqlitePortfolioStore::new(
            repositories.as_ref().clone(),
        ));
        let slot = AccountSlot::default();
        let names = src_tauri::holdings_link::InstrumentDirectory::default();
        let shared_clock: Arc<dyn application::Clock> =
            Arc::<platform_macos::SystemClock>::clone(&clock);
        let refresh = src_tauri::holdings_link::MasterRefresh::new(
            Arc::new(super::ProductMasterFeed {
                broker: Arc::clone(broker),
                store: Arc::clone(repositories),
            }),
            names.clone(),
            shared_clock,
        );
        let sync = HoldingsSyncFor {
            store: Arc::clone(repositories),
            broker: Arc::clone(broker),
            portfolios: Arc::clone(&portfolios),
            clock,
            events: Arc::clone(events),
            slot: slot.clone(),
        };
        HoldingsParts {
            listener: src_tauri::holdings_link::HoldingsListener::new(
                Arc::new(src_tauri::toss_link::NoListener),
                Arc::new(refresh),
                Arc::new(sync),
            ),
            names,
            portfolios,
            slot,
        }
    }

    async fn assert_stored_holdings(
        parts: &HoldingsParts,
        repositories: &store_sqlite::SqliteRepositories,
    ) {
        use application::PortfolioRepository;
        let record = parts
            .portfolios
            .latest(&domain::AccountId::new(413))
            .await
            .expect("조회")
            .expect("기록");
        let domain::SyncOutcome::Complete(portfolio) = record.outcome() else {
            panic!("완료여야 함: {:?}", record.outcome());
        };
        assert_eq!(portfolio.holdings().len(), 2);
        let stored = application::GetPortfolioView::new(parts.portfolios.as_ref(), repositories)
            .execute(&domain::AccountId::new(413))
            .await
            .expect("현황");
        let application::StoredView::Recorded(view) = stored else {
            panic!("기록된 현황");
        };
        let domain::SyncOutcome::Complete(valuation) = view.outcome() else {
            panic!("완료된 평가");
        };
        assert_eq!(
            valuation.assets().completeness(),
            domain::TotalCompleteness::Complete,
            "환율이 있어 달러 자산까지 총평가에 들어감"
        );
        let name = |symbol: &str| {
            parts
                .names
                .name(&domain::Symbol::try_new(symbol).expect("심볼"))
                .map(|name| name.as_str().to_owned())
        };
        assert_eq!(name("005930"), Some(String::from("삼성전자")));
        assert_eq!(name("AAPL"), Some(String::from("애플")));
        let history = {
            let guard = repositories.shared().read().await;
            store_sqlite::HoldingsStore::new(guard.as_ref().expect("열림"))
                .holdings_history(domain::AccountId::new(413))
                .await
                .expect("이력")
        };
        assert_eq!(history.len(), 1);
        let saved = store_sqlite::MasterSnapshots(repositories)
            .stored()
            .await
            .expect("저장된 마스터");
        assert_eq!(saved.len(), 2, "국내·미국 마스터가 SQLite에 남음");
        let restarted = store_sqlite::SqlitePortfolioStore::new(repositories.clone());
        let restored = restarted
            .latest(&domain::AccountId::new(413))
            .await
            .expect("복원")
            .expect("재시작 직후 보유");
        let domain::SyncOutcome::Partial(value) = restored.outcome() else {
            panic!("복원 기록: {:?}", restored.outcome());
        };
        assert_eq!(value.holdings().len(), 2);
    }

    #[test]
    fn connected_toss_loads_both_masters_then_stores_converted_holdings_and_names() {
        let server = ScriptedHttpServer::start(master_and_holdings_script()).expect("가짜 토스");
        let secrets = memory_secrets();
        tauri::async_runtime::block_on(store_keys(&secrets));
        let events = Arc::new(application::EventBus::default());
        let broker = Arc::new(
            super::live_toss_read(
                &broker_toss::TossEndpoint::local(server.port()).expect("주소"),
                Arc::clone(&secrets),
                Arc::new(platform_macos::SystemClock::new().expect("시계")),
                Arc::clone(&events),
            )
            .expect("어댑터"),
        );
        let directory = std::path::PathBuf::from("../../../target/fin-hold")
            .join(format!("holdings-{}", std::process::id()));
        tauri::async_runtime::block_on(async {
            let repositories = open_store(&directory).await;
            let parts = holdings_parts(&broker, &repositories, &events);
            let _selected =
                AccountSelector::new(Arc::clone(&broker), parts.slot.clone(), Arc::clone(&events))
                    .select()
                    .await;
            assert_eq!(parts.slot.current(), vec![domain::AccountId::new(413)]);
            assert_eq!(
                parts.listener.refresh_holdings().await,
                src_tauri::holdings_link::RefreshOutcome::Refreshed
            );
            assert_stored_holdings(&parts, &repositories).await;
            let _closed = repositories.shared().close().await;
        });
        let requests = String::from_utf8(server.finish().expect("요청")).expect("UTF-8");
        assert_eq!(requests.matches("GET /api/v1/stocks/all").count(), 7);
        assert_eq!(requests.matches("GET /api/v1/stocks?").count(), 2);
        assert_eq!(requests.matches("GET /api/v1/holdings").count(), 1);
        assert_eq!(requests.matches(" HTTP/1.1\r\n").count(), 16);
        assert_eq!(requests.matches("GET /api/v1/exchange-rate").count(), 1);
        let _cleaned = std::fs::remove_dir_all(&directory);
    }

    struct HoldingsSyncFor {
        store: Arc<store_sqlite::SqliteRepositories>,
        broker: Arc<TestBroker>,
        portfolios: Arc<store_sqlite::SqlitePortfolioStore>,
        clock: Arc<platform_macos::SystemClock>,
        events: Arc<application::EventBus>,
        slot: AccountSlot,
    }

    impl HoldingsSyncFor {
        async fn sync_accounts(&self) {
            let parts = src_tauri::holdings_link::SyncParts {
                broker: self.broker.as_ref(),
                portfolios: self.portfolios.as_ref(),
                clock: self.clock.as_ref(),
                events: &self.events,
                market: self.broker.as_ref(),
                market_data: self.store.as_ref(),
            };
            for account in &self.slot.current() {
                let _report = src_tauri::holdings_link::sync_with_rate(&parts, account)
                    .await
                    .expect("동기화");
            }
        }
    }

    impl src_tauri::holdings_link::HoldingsSync for HoldingsSyncFor {
        fn sync(&self) -> src_tauri::ipc::state::BoxFuture<'_, ()> {
            Box::pin(self.sync_accounts())
        }
    }

    #[test]
    fn without_stored_keys_the_check_needs_keys_and_sends_nothing_to_toss() {
        let server = ScriptedHttpServer::start(Vec::new()).expect("가짜 토스");
        let secrets = memory_secrets();
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let broker = Arc::new(
            super::live_toss_read(
                &broker_toss::TossEndpoint::local(server.port()).expect("주소"),
                Arc::clone(&secrets),
                Arc::clone(&clock),
                Arc::new(application::EventBus::default()),
            )
            .expect("어댑터"),
        );
        let link = TossLink::new(
            secrets,
            Arc::new(super::LiveProbe(broker)),
            Arc::new(src_tauri::toss_link::NoListener),
            clock,
        );
        assert_eq!(
            tauri::async_runtime::block_on(link.check()),
            Ok(TossConnectionState::NeedsKeys)
        );
        assert_eq!(server.finish().expect("요청"), Vec::<u8>::new());
    }

    fn odd_master_and_unmatched_holdings_script() -> Vec<Vec<u8>> {
        let empty = serde_json::json!([]);
        let kr_listing = serde_json::json!([
            {"symbol":"005930","name":"삼성전자","securityType":"STOCK","isCommonShare":true,"isinCode":"KR7005930003"},
            {"symbol":"000660","name":"합성 빈 ISIN","securityType":"STOCK","isCommonShare":true,"isinCode":""},
            {"symbol":"Q500001","name":"합성 ETN","securityType":"ETN","isCommonShare":true,"isinCode":"KRQ500001001"},
            {"symbol":"005930","name":"삼성전자 중복","securityType":"STOCK","isCommonShare":true,"isinCode":"KR7005930003"}
        ]);
        let kr_details = serde_json::json!([
            {"symbol":"005930","name":"삼성전자보통주","englishName":"Samsung","isinCode":"KR7005930003","market":"KOSPI","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"KRW","listDate":null,"delistDate":null,"sharesOutstanding":"91","leverageFactor":null,"koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":false,"krxTradingSuspended":false,"nxtTradingSuspended":false}},
            {"symbol":"Q500001","name":"합성 ETN","englishName":"Synthetic ETN","isinCode":"KRQ500001001","market":"KOSPI","securityType":"ETN","isCommonShare":true,"status":"ACTIVE","currency":"KRW","sharesOutstanding":"1","leverageFactor":"2.0","koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":false,"krxTradingSuspended":false,"nxtTradingSuspended":null}}
        ]);
        let us_listing = serde_json::json!([
            {"symbol":"AAPL","name":"애플","securityType":"STOCK","isCommonShare":true,"isinCode":"US0378331005"},
            {"symbol":"BRK/B","name":"합성 버크셔","securityType":"STOCK","isCommonShare":false,"isinCode":"US0846707026"}
        ]);
        let us_details = serde_json::json!([
            {"symbol":"AAPL","name":"애플","englishName":"Apple","isinCode":"US0378331005","market":"NASDAQ","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"USD","sharesOutstanding":"15000","koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":false,"krxTradingSuspended":false,"nxtTradingSuspended":null}},
            {"symbol":"MSFT","name":"합성 요청 외","englishName":"Unrequested","isinCode":"US5949181045","market":"NASDAQ","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"USD","sharesOutstanding":"1","koreanMarketDetail":null}
        ]);
        let recorded: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                "../../../crates/adapters/broker-toss/tests/recordings/t05-holdings.json",
            )
            .expect("녹화"),
        )
        .expect("JSON");
        let mut holdings = recorded["body"].clone();
        holdings["result"]["items"][1]["symbol"] = serde_json::json!("TSLA");
        vec![
            token_ok(),
            brokerage_accounts(),
            listing(&kr_listing),
            listing(&empty),
            listing(&empty),
            listing(&kr_details),
            listing(&empty),
            listing(&us_listing),
            listing(&empty),
            listing(&empty),
            listing(&us_details),
            brokerage_accounts(),
            ok(&holdings),
            buying_power("KRW", "5000000"),
            buying_power("USD", "3500.525"),
            ok(
                &serde_json::json!({"result":{"baseCurrency":"USD","quoteCurrency":"KRW","rate":"1381.7","midRate":"1374.2","basisPoint":"54.58","rateChangeType":"UP","validFrom":"2000-01-01T00:00:00+09:00","validUntil":"2100-01-01T00:00:00+09:00"}}),
            ),
        ]
    }

    async fn assert_odd_master_holdings(parts: &HoldingsParts) {
        use application::PortfolioRepository;
        let name = |symbol: &str| {
            parts
                .names
                .name(&domain::Symbol::try_new(symbol).expect("심볼"))
                .map(|found| found.as_str().to_owned())
        };
        assert_eq!(name("005930"), Some(String::from("삼성전자보통주")));
        assert_eq!(name("AAPL"), Some(String::from("애플")));
        assert_eq!(name("Q500001"), None);
        assert_eq!(name("MSFT"), None);
        let record = parts
            .portfolios
            .latest(&domain::AccountId::new(413))
            .await
            .expect("조회")
            .expect("기록");
        let domain::SyncOutcome::Complete(portfolio) = record.outcome() else {
            panic!("완료여야 함: {:?}", record.outcome());
        };
        let shown = portfolio
            .holdings()
            .iter()
            .map(|holding| {
                (
                    holding.instrument().symbol().as_str().to_owned(),
                    holding
                        .reported_name()
                        .map(|reported| reported.as_str().to_owned()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            shown,
            vec![
                (String::from("005930"), None),
                (String::from("TSLA"), Some(String::from("Apple Inc."))),
            ]
        );
    }

    #[test]
    fn first_run_with_odd_master_rows_still_shows_every_holding() {
        let server = ScriptedHttpServer::start(odd_master_and_unmatched_holdings_script())
            .expect("가짜 토스");
        let secrets = memory_secrets();
        tauri::async_runtime::block_on(store_keys(&secrets));
        let events = Arc::new(application::EventBus::default());
        let broker = Arc::new(
            super::live_toss_read(
                &broker_toss::TossEndpoint::local(server.port()).expect("주소"),
                Arc::clone(&secrets),
                Arc::new(platform_macos::SystemClock::new().expect("시계")),
                Arc::clone(&events),
            )
            .expect("어댑터"),
        );
        let directory = std::path::PathBuf::from("../../../target/fin-hold")
            .join(format!("odd-master-{}", std::process::id()));
        let _stale = std::fs::remove_dir_all(&directory);
        tauri::async_runtime::block_on(async {
            let repositories = open_store(&directory).await;
            let parts = holdings_parts(&broker, &repositories, &events);
            assert_eq!(
                parts.listener.restore_saved().await,
                src_tauri::holdings_link::RefreshOutcome::Fresh,
                "빈 저장소는 오류가 아니다"
            );
            let _selected =
                AccountSelector::new(Arc::clone(&broker), parts.slot.clone(), Arc::clone(&events))
                    .select()
                    .await;
            assert_eq!(
                parts.listener.refresh_holdings().await,
                src_tauri::holdings_link::RefreshOutcome::Refreshed
            );
            assert_odd_master_holdings(&parts).await;
            let _closed = repositories.shared().close().await;
        });
        let requests = String::from_utf8(server.finish().expect("요청")).expect("UTF-8");
        assert_eq!(requests.matches("GET /api/v1/stocks?").count(), 2);
        assert!(requests.contains("Q500001"));
        assert!(!requests.contains("000660"));
        assert!(!requests.contains("BRK"));
        let _cleaned = std::fs::remove_dir_all(&directory);
    }

    async fn wait_for(
        app: tauri::AppHandle<tauri::test::MockRuntime>,
    ) -> src_tauri::database::SetupWait {
        src_tauri::database::wait_for_setup(&app).await
    }

    #[test]
    fn toss_check_waits_until_setup_has_settled_the_database() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱");
        let handle = app.handle().clone();
        let waited = tauri::async_runtime::block_on(async {
            let waiter = handle.clone();
            let waiting = tauri::async_runtime::spawn(wait_for(waiter));
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert!(!waiting.inner().is_finished(), "설정 전에는 기다린다");
            let _managed =
                tauri::Manager::manage(&handle, src_tauri::database::DatabaseStatus::Ready);
            waiting.await.expect("대기")
        });
        assert_eq!(
            waited,
            src_tauri::database::SetupWait::Settled(src_tauri::database::DatabaseStatus::Ready)
        );
    }
}

#[cfg(test)]
mod chart_candle_assembly_tests {
    use application::SecretValue;
    use src_tauri::ipc::instruments::InstrumentMarket;
    use std::sync::Arc;
    use testkit::ScriptedHttpServer;
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemorySecretStore;

    fn http(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn at(text: &str) -> domain::Timestamp {
        broker_toss::TossTimestamp::try_from(text)
            .expect("시각")
            .into_domain()
    }

    fn candle(stamp: &str, close: &str) -> String {
        format!(
            r#"{{"timestamp":"{stamp}","openPrice":"{close}","highPrice":"{close}","lowPrice":"{close}","closePrice":"{close}","volume":"1200","currency":"KRW"}}"#
        )
    }

    fn kr_day(date: &str) -> String {
        format!(
            r#"{{"date":"{date}","integrated":{{"preMarket":null,"regularMarket":{{"startTime":"{date}T09:00:00+09:00","endTime":"{date}T15:30:00+09:00"}},"afterMarket":null}}}}"#
        )
    }

    fn script() -> Vec<Vec<u8>> {
        vec![
            http(r#"{"access_token":"fake-access-token","token_type":"Bearer","expires_in":3600}"#),
            http(&format!(
                r#"{{"result":{{"candles":[{},{},{}],"nextBefore":null}}}}"#,
                candle("2026-03-23T14:00:00+09:00", "201000"),
                candle("2026-03-20T14:00:00+09:00", "199500"),
                candle("2026-03-19T14:00:00+09:00", "198000"),
            )),
            http(&format!(
                r#"{{"result":{{"previousBusinessDay":{},"today":{},"nextBusinessDay":{}}}}}"#,
                kr_day("2026-03-20"),
                kr_day("2026-03-23"),
                kr_day("2026-03-24"),
            )),
        ]
    }

    async fn secrets() -> Arc<MemorySecretStore> {
        let secrets = Arc::new(MemorySecretStore::default());
        for _ in 0_u8..8_u8 {
            secrets.script_load().push(MemoryReply::Memory);
            secrets.script_store().push(MemoryReply::Memory);
        }
        for (key, value) in [
            (application::SecretKey::TossClientId, "client-id-chart-5c1"),
            (
                application::SecretKey::TossClientSecret,
                "client-secret-chart-8d2",
            ),
        ] {
            let _stored = application::SecretStore::store(
                &*secrets,
                key,
                SecretValue::new(value.as_bytes().to_vec()),
            )
            .await
            .expect("키");
        }
        secrets
    }

    #[test]
    fn production_reader_serves_chart_candles_with_todays_open_candle() {
        let server = ScriptedHttpServer::start(script()).expect("가짜 토스");
        let now = at("2026-03-23T10:00:00+09:00");
        let candles = tauri::async_runtime::block_on(async {
            let broker = super::live_toss_read(
                &broker_toss::TossEndpoint::local(server.port()).expect("주소"),
                secrets().await,
                Arc::new(testkit_ports::SteppedClock::new(now)),
                Arc::new(application::EventBus::default()),
            )
            .expect("어댑터");
            let instrument =
                domain::QuoteInstrument::restore_from_saved_state(domain::SavedQuoteInstrument {
                    market: domain::KnownMarket::Kr,
                    currency: domain::Currency::Krw,
                    symbol: domain::Symbol::try_new("000660").expect("종목"),
                    instrument_id: domain::InstrumentId::try_new("chart-000660").expect("ID"),
                })
                .expect("시세 종목");
            let window =
                domain::TimeInterval::try_new(at("2026-03-18T00:00:00+09:00"), now).expect("창");
            broker.daily(instrument, window).await.expect("차트 일봉")
        });
        let days: Vec<_> = candles
            .iter()
            .map(|candle| (candle.time().timestamp(), candle.status()))
            .collect();
        assert_eq!(
            days,
            vec![
                (
                    at("2026-03-19T00:00:00+09:00"),
                    domain::CandleStatus::Finalized
                ),
                (
                    at("2026-03-20T00:00:00+09:00"),
                    domain::CandleStatus::Finalized
                ),
                (
                    at("2026-03-23T00:00:00+09:00"),
                    domain::CandleStatus::InProgress
                ),
            ]
        );
        let requests = String::from_utf8(server.finish().expect("요청")).expect("UTF-8");
        assert_eq!(
            requests
                .matches("GET /api/v1/market-calendar/KR?date=2026-03-23")
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod holdings_job_assembly_tests {
    use application::{
        AiEvent, BriefingOutcome, BriefingRepository, IsolationCheck, PortfolioRepository,
        SecretValue, ValuationRepository,
    };
    use domain::{
        AccountId, AcquisitionPrice, BuyingPower, CommonShare, Currency, DisplayInstrumentName,
        Holding, HoldingObservation, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading,
        OffsetSeconds, Portfolio, Price, Quantity, SessionData, SessionInformation, Symbol,
        SyncOutcome, SyncRecord, TimeInterval, Timestamp, TradingSuspension, UnixSeconds,
        UntrustedText,
    };
    use scheduler::{AiJobQueue, AiQueueAdmission, AiQueueError, BriefingPort, DailyValuationPort};
    use src_tauri::database::DatabaseFactory;
    use src_tauri::platform::scheduled_ports::AppStoredJobs;
    use src_tauri::ports::{PortTypes, Ports};
    use src_tauri::unavailable::{Unavailable, UnavailableFeed};
    use std::sync::Arc;
    use store_sqlite::{SharedDatabase, SqlitePortfolioStore, SqliteRepositories};
    use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiEventStream, ScriptedAiProvider};

    #[derive(Debug, Default)]
    struct AdmittingQueue;

    impl AiJobQueue for AdmittingQueue {
        fn request_background_slot(
            &self,
            _job: scheduler::ScheduledJobId,
            _now: Timestamp,
        ) -> impl Future<Output = Result<AiQueueAdmission, AiQueueError>> + Send {
            std::future::ready(Ok(AiQueueAdmission::Admitted))
        }
        fn release_background_slot(
            &self,
            _job: scheduler::ScheduledJobId,
            _now: Timestamp,
        ) -> impl Future<Output = Result<(), AiQueueError>> + Send {
            std::future::ready(Ok(()))
        }
    }

    #[derive(Debug)]
    struct GenericJobPorts<Holdings, Store>(std::marker::PhantomData<fn() -> (Holdings, Store)>);
    type HoldingJobPorts = GenericJobPorts<SqlitePortfolioStore, SqliteRepositories>;
    impl<Holdings, Store> PortTypes for GenericJobPorts<Holdings, Store>
    where
        Holdings: PortfolioRepository + 'static,
        Store: application::MarketDataRepository
            + application::NewsRepository
            + ValuationRepository
            + 'static,
    {
        type BrokerRead = testkit_ports::scripted::ScriptedBrokerRead;
        type MarketInfo = testkit_ports::scripted::ScriptedMarketInfo;
        type MacroSeries = Unavailable;
        type NewsFeed = UnavailableFeed;
        type Fundamentals = Unavailable;
        type AiProvider = ScriptedAiProvider;
        type ChatRepository = testkit_ports::chat_fakes::MemoryChatRepository;
        type ChatRunLedger = testkit_ports::chat_fakes::MemoryChatRunLedger;
        type ChatQueuePort = testkit_ports::chat_fakes::ScriptedChatQueue;
        type WarmChatRuntime = testkit_ports::chat_fakes::ScriptedWarmChatRuntime;
        type AiJobQueue = AdmittingQueue;
        type PortfolioRepository = Holdings;
        type TransactionRepository = Unavailable;
        type MarketDataRepository = Store;
        type NewsRepository = Store;
        type ThesisRepository = Unavailable;
        type EstimateRepository = Unavailable;
        type SignalRepository = Unavailable;
        type ProposalRepository = Unavailable;
        type ValuationRepository = Store;
        type SettingsRepository = testkit_ports::memory::MemorySettingsRepository;
        type AuditLog = Unavailable;
        type SecretStore = testkit_ports::system_fakes::MemorySecretStore;
        type Clock = testkit_ports::SteppedClock;
        type Notifier = Unavailable;
        type UserPresence = testkit_ports::ScriptedPresence;
    }

    const BASE_DAY: i64 = 1_794_960_000;

    fn at(day: i64, hour: i64) -> Timestamp {
        let days = day.checked_mul(86_400).expect("일");
        let hours = hour.checked_mul(3_600).expect("시");
        let seconds = days
            .checked_add(hours)
            .and_then(|offset| BASE_DAY.checked_add(offset))
            .and_then(|local| local.checked_sub(32_400))
            .expect("시각");
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn segment(day: i64, start: i64, end: i64) -> SessionData {
        SessionData::try_new(
            TimeInterval::try_new(at(day, start), at(day, end)).expect("구간"),
            None,
        )
        .expect("세션")
    }

    fn session(day: i64, market: &Market) -> MarketSession {
        let information = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(segment(day, 9, 15)),
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(segment(day, 9, 15)),
                after_market: None,
            },
        };
        MarketSession::try_new(at(day, 0).offset_calendar_date(), information).expect("장")
    }

    fn calendar(market: &Market) -> MarketCalendar {
        MarketCalendar::try_new(session(0, market), session(1, market), session(2, market))
            .expect("달력")
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("숫자"), currency).expect("금액")
    }

    fn portfolio(now: Timestamp) -> Portfolio {
        let master = MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("005930").expect("심볼"),
            instrument_id: InstrumentId::try_new("inst-1").expect("ID"),
            name: DisplayInstrumentName::try_new("삼성전자").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: now,
        })
        .expect("마스터");
        let holding = Holding::new(
            master.order_instrument().expect("종목"),
            Quantity::parse(&NumericInput::try_new("3").expect("숫자")).expect("수량"),
            AcquisitionPrice::try_new(money("70000", Currency::Krw)).expect("평단"),
            HoldingObservation::new(
                Some(Price::try_new(money("72000", Currency::Krw)).expect("가격")),
                None,
                now,
            ),
        )
        .expect("보유");
        Portfolio::new(
            vec![holding],
            BuyingPower::new(money("1000", Currency::Krw), now),
            BuyingPower::new(money("0", Currency::Usd), now),
        )
        .expect("포트폴리오")
    }

    struct Assembly {
        app: tauri::App<tauri::test::MockRuntime>,
        provider: Arc<ScriptedAiProvider>,
        repositories: SqliteRepositories,
        shared: SharedDatabase,
        directory: std::path::PathBuf,
    }

    fn assemble(now: Timestamp) -> Assembly {
        let shared = SharedDatabase::new();
        let repositories = SqliteRepositories::new(shared.clone());
        let directory = std::path::PathBuf::from("../../../target/fin-hold")
            .join(format!("jobs-{}", std::process::id()));
        let clock = Arc::new(testkit_ports::SteppedClock::new(now));
        tauri::async_runtime::block_on(async {
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let database = store_sqlite::Database::open(
                &path,
                &SecretValue::new(vec![55; 32]),
                clock.as_ref(),
            )
            .expect("열기");
            assert!(shared.install(database).await.is_none());
        });
        let portfolios = Arc::new(SqlitePortfolioStore::new(repositories.clone()));
        let account = AccountId::new(413);
        tauri::async_runtime::block_on(portfolios.record(
            &account,
            &SyncRecord::new(now, SyncOutcome::Complete(portfolio(now))),
        ))
        .expect("동기화 기록");
        let provider = Arc::new(ScriptedAiProvider::default());
        let ports = job_ports(portfolios, &repositories, &provider, clock, account);
        let app = tauri::test::mock_builder()
            .manage(ports)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱");
        Assembly {
            app,
            provider,
            repositories,
            shared,
            directory,
        }
    }

    fn job_ports(
        portfolios: Arc<SqlitePortfolioStore>,
        repositories: &SqliteRepositories,
        provider: &Arc<ScriptedAiProvider>,
        clock: Arc<testkit_ports::SteppedClock>,
        account: AccountId,
    ) -> Ports<HoldingJobPorts> {
        let market_info = Arc::new(testkit_ports::scripted::ScriptedMarketInfo::default());
        market_info
            .script_calendar()
            .push(Ok(calendar(&Market::Kr)));
        market_info
            .script_calendar()
            .push(Ok(calendar(&Market::Us)));
        let shared_repositories = Arc::new(repositories.clone());
        Ports {
            broker_read: Arc::new(testkit_ports::scripted::ScriptedBrokerRead::default()),
            market_info,
            macro_series: Arc::new(Unavailable),
            news_feeds: Vec::new(),
            fundamentals: Arc::new(Unavailable),
            ai_provider: Arc::clone(provider),
            chat_repository: Arc::new(testkit_ports::chat_fakes::MemoryChatRepository::default()),
            chat_ledger: Arc::new(testkit_ports::chat_fakes::MemoryChatRunLedger::default()),
            chat_queue: Arc::new(testkit_ports::chat_fakes::ScriptedChatQueue::default()),
            warm_chat: Arc::new(testkit_ports::chat_fakes::ScriptedWarmChatRuntime::default()),
            ai_jobs: Arc::new(AdmittingQueue),
            portfolios,
            transactions: Arc::new(Unavailable),
            market_data: Arc::clone(&shared_repositories),
            news: Arc::clone(&shared_repositories),
            theses: Arc::new(Unavailable),
            estimates: Arc::new(Unavailable),
            signals: Arc::new(Unavailable),
            proposals: Arc::new(Unavailable),
            valuations: shared_repositories,
            settings: Arc::new(testkit_ports::memory::MemorySettingsRepository::default()),
            audit: Arc::new(Unavailable),
            secrets: Arc::new(testkit_ports::system_fakes::MemorySecretStore::default()),
            clock,
            notifier: Arc::new(Unavailable),
            presence: Arc::new(testkit_ports::ScriptedPresence::new([], [])),
            events: Arc::new(application::EventBusBuilder::default().build()),
            saved_accounts: src_tauri::broker_link::AccountSlot::with(vec![account]),
            database: Arc::new(DatabaseFactory(Box::new(|_path, _key| {
                Err(application::SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::InvalidData,
                ))
            }))),
        }
    }

    impl Drop for Assembly {
        fn drop(&mut self) {
            let _closed = tauri::async_runtime::block_on(self.shared.close());
            let _cleaned = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn briefing_and_daily_valuation_read_the_holdings_the_portfolio_sync_stored() {
        let now = at(1, 16);
        let assembly = assemble(now);
        for _ in 0_u8..2_u8 {
            assembly
                .provider
                .script_run()
                .push(Ok(ScriptedAiEventStream::from_events(vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::TextDelta(UntrustedText::try_new("보유 종목 요약").expect("요약")),
                ])));
        }
        let jobs = AppStoredJobs::<tauri::test::MockRuntime, HoldingJobPorts>::new(
            assembly.app.handle().clone(),
            assembly.repositories.clone(),
        );
        tauri::async_runtime::block_on(async {
            let _briefing = BriefingPort::run(&jobs, domain::BriefingKind::Morning, now)
                .await
                .expect("브리핑");
            let inputs: Vec<String> = assembly
                .provider
                .calls()
                .iter()
                .filter_map(|call| match call {
                    AiProviderCall::Run(spec) => Some(format!("{:?}", spec.input())),
                    AiProviderCall::Resume { .. }
                    | AiProviderCall::Usage
                    | AiProviderCall::Interrupt(_)
                    | AiProviderCall::VerifyIsolation(_) => None,
                })
                .collect();
            assert!(
                inputs.iter().any(|input| input.contains("005930")),
                "브리핑 맥락에 실제 보유 종목: {inputs:?}"
            );
            let saved = assembly.repositories.recent().await.expect("브리핑 기록");
            match saved.first().map(|record| &record.outcome) {
                Some(BriefingOutcome::Completed { .. }) => {}
                other => panic!("완료된 브리핑이어야 함: {other:?}"),
            }
            jobs.record_today(now).await.expect("일별 평가");
            let window = TimeInterval::try_new(at(0, 0), at(2, 0)).expect("구간");
            let history = assembly
                .repositories
                .history(&AccountId::new(413), window)
                .await
                .expect("평가 이력");
            assert_eq!(history.len(), 1, "보유 저장소의 계좌가 평가됨");
        });
    }
}

#[cfg(test)]
mod estimate_flow_assembly_tests {
    use application::{
        AiEvent, AiProvider, AiRunId, AiUsage, ChatHistory, EstimateRunRegistry,
        EstimateSubmissionRepository, EstimateTaskMaterialSource, EstimateTaskMaterials,
        EstimateTrigger, EventBus, EventBusBuilder, EventSubscriber, InformationError,
        IsolationCheck, RunInput, RunKind, RunSingleEstimate, RunSpec, SecretValue,
        SingleEstimateOutcome, SubscriberFailure, SystemPortError, ToolName, TrackingTarget,
    };
    use domain::{DomainEvent, DomainEventKind, EstimateTask, MarketCalendar, Timestamp};
    use mcp_server::{ToolInvocation, ToolRegistry};
    use serde_json::{Value, json};
    use src_tauri::estimate_flow::EstimateRunBook;
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::sync::{Arc, OnceLock, RwLock};
    use store_sqlite::{SharedDatabase, SqliteRepositories};
    use testkit_ports::SteppedClock;
    use testkit_ports::chat_fakes::ScriptedAiEventStream;
    use testkit_ports::estimate_fixtures::{
        sample_day, sample_estimate_materials, sample_kr_calendars,
    };

    struct Materials;

    impl EstimateTaskMaterialSource for Materials {
        fn materials(
            &self,
            _target: &TrackingTarget,
            _now: Timestamp,
        ) -> impl Future<Output = Result<Option<EstimateTaskMaterials>, SystemPortError>> + Send
        {
            std::future::ready(Ok(sample_estimate_materials()))
        }

        fn calendars(
            &self,
            _task: &EstimateTask,
        ) -> impl Future<Output = Result<Vec<MarketCalendar>, SystemPortError>> + Send {
            std::future::ready(Ok(sample_kr_calendars(60, 70).expect("달력")))
        }
    }

    #[derive(Default)]
    struct Submitted(RwLock<Vec<DomainEvent>>);

    struct Recorder(Arc<Submitted>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            if let Ok(mut guard) = self.0.0.write() {
                guard.push(event.clone());
            }
            Ok(())
        }
    }

    struct McpSubmittingAi {
        registry: OnceLock<ToolRegistry>,
        results: RwLock<Vec<Value>>,
    }

    fn task_id_in(prompt: &str) -> String {
        prompt
            .split_once("task_id=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(id, _)| id.to_owned())
            .expect("과제 ID")
    }

    impl McpSubmittingAi {
        async fn call(&self, run: &AiRunId, name: &str, arguments: &Value) -> Value {
            let registry = self.registry.get().expect("카탈로그");
            let tool = Arc::clone(
                registry
                    .get(&ToolName::try_new(name).expect("이름"))
                    .expect("도구")
                    .1,
            );
            let map = arguments.as_object().cloned().expect("객체");
            tool.invoke(ToolInvocation {
                run_id: run,
                arguments: &map,
            })
            .await
            .expect("도구 결과")
        }

        async fn submit(&self, spec: &RunSpec) {
            let RunInput::Analysis(prompt) = spec.input() else {
                panic!("분석 실행");
            };
            let task = task_id_in(prompt.as_str());
            let read = self
                .call(spec.id(), "get_estimate_task", &json!({ "task_id": task }))
                .await;
            let submitted = self
                .call(
                    spec.id(),
                    "submit_estimate",
                    &json!({
                        "task_id": task,
                        "p": 41_u64,
                        "rationale": "근거 없음",
                        "counterargument": "반대 논거",
                        "sources": []
                    }),
                )
                .await;
            self.results
                .write()
                .expect("기록")
                .extend([read, submitted]);
        }
    }

    impl AiProvider for McpSubmittingAi {
        type Stream = ScriptedAiEventStream;

        async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
            self.submit(&spec).await;
            Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
            ]))
        }

        fn resume(
            &self,
            _spec: RunSpec,
            _history: ChatHistory,
            _message: domain::UntrustedText,
        ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
            std::future::ready(Err(InformationError::Unavailable(
                application::ResetTime::NotProvided,
            )))
        }

        fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
            std::future::ready(Err(InformationError::Unavailable(
                application::ResetTime::NotProvided,
            )))
        }

        fn interrupt(
            &self,
            _run: &AiRunId,
        ) -> impl Future<Output = Result<(), InformationError>> + Send {
            std::future::ready(Ok(()))
        }

        fn verify_isolation(
            &self,
            _kind: RunKind,
        ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
            std::future::ready(Ok(IsolationCheck::Passed))
        }
    }

    fn catalog(
        clock: &Arc<SteppedClock>,
        repositories: &Arc<SqliteRepositories>,
        events: &Arc<EventBus>,
        book: &EstimateRunBook,
    ) -> ToolRegistry {
        src_tauri::mcp_catalog::analysis_catalog(
            clock,
            src_tauri::mcp_catalog::CatalogStores {
                theses: Arc::clone(repositories),
                estimates: Arc::clone(repositories),
                bodies: Arc::new(super::ProductArticleBodies { reader: None }),
                runs: src_tauri::ai_runtime::RunMetadataBook::new(),
                collector: Arc::new(
                    src_tauri::ai_runtime::AiAvailability::<
                        testkit_ports::chat_fakes::ScriptedAiProvider,
                    >::Missing,
                ),
                turns: src_tauri::ai_runtime::ChatTurnBook::new(),
                events: Arc::clone(events),
                market: src_tauri::broker_link::BrokerMarketTools::unconnected(),
                portfolio: src_tauri::mcp_portfolio::CatalogPortfolio::default(),
                estimate_runs: book.clone(),
            },
        )
        .expect("카탈로그")
    }

    fn recording_bus() -> (Arc<EventBus>, Arc<Submitted>) {
        let recorder = Arc::new(Submitted::default());
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::EstimateSubmitted]),
            Recorder(Arc::clone(&recorder)),
        );
        (Arc::new(builder.build()), recorder)
    }

    async fn open(shared: &SharedDatabase, directory: &std::path::Path, clock: &SteppedClock) {
        let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
        src_tauri::database::prepare_directory(&path).expect("폴더");
        let database = store_sqlite::Database::open(&path, &SecretValue::new(vec![71; 32]), clock)
            .expect("열기");
        assert!(shared.install(database).await.is_none());
    }

    #[test]
    fn a_code_issued_task_is_estimated_through_the_product_catalog_and_stored() {
        let directory = std::path::PathBuf::from("../../../target/fin-flow")
            .join(format!("estimate-{}", std::process::id()));
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let clock = Arc::new(SteppedClock::new(sample_day(60).expect("시각")));
        let (events, recorder) = recording_bus();
        let book = EstimateRunBook::new();
        let ai = McpSubmittingAi {
            registry: OnceLock::new(),
            results: RwLock::new(Vec::new()),
        };
        if ai
            .registry
            .set(catalog(&clock, &repositories, &events, &book))
            .is_err()
        {
            panic!("카탈로그는 한 번만");
        }
        let identity = super::estimate_identity().expect("식별");
        let target =
            TrackingTarget::for_holding(domain::InstrumentId::try_new("sample-a").expect("ID"));
        tauri::async_runtime::block_on(async {
            open(&shared, &directory, &clock).await;
            let outcome = RunSingleEstimate {
                materials: &Materials,
                tasks: repositories.as_ref(),
                ai: &ai,
                runs: &book,
                clock: clock.as_ref(),
                identity: &identity,
            }
            .run(&target, &EstimateTrigger::Cycle)
            .await
            .expect("실행");
            assert_eq!(outcome, SingleEstimateOutcome::Submitted);
            let records = repositories
                .submission_records(target.instrument())
                .await
                .expect("제출 기록");
            assert_eq!(records.len(), 1);
            let record = records.first().expect("기록");
            assert_eq!(record.metadata().prompt.hash, identity.prompt.hash);
            assert_eq!(
                *recorder.0.read().expect("이벤트"),
                vec![DomainEvent::EstimateSubmitted(target.instrument().clone())]
            );
            let results = ai.results.read().expect("결과").clone();
            assert_eq!(
                results.last().expect("제출 결과")["recorded"],
                json!("estimate_only_no_order")
            );
            book.release(record.run());
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }
}

#[cfg(test)]
mod news_linking_tests {
    use application::TrackedInstrument;
    use domain::{DisplayInstrumentName, InstrumentId, KnownMarket, Symbol, UntrustedText};
    use src_tauri::collected_links::TitleLinker;
    use src_tauri::holdings_link::InstrumentDirectory;
    use src_tauri::news_tracking::{MatchName, match_names};

    fn tracked(symbol: &str, market: KnownMarket, name: &str) -> TrackedInstrument {
        TrackedInstrument {
            symbol: Symbol::try_new(symbol).expect("종목"),
            market,
            name: DisplayInstrumentName::try_new(name).expect("이름"),
            aliases: Vec::new(),
        }
    }

    fn names() -> Vec<MatchName> {
        match_names(
            &[
                tracked("005930", KnownMarket::Kr, "삼성전자"),
                tracked("000660", KnownMarket::Kr, "SK하이닉스"),
                tracked("AAPL", KnownMarket::Us, "애플"),
            ],
            &[],
            &InstrumentDirectory::default(),
        )
    }

    fn linked(title: &str) -> Vec<String> {
        super::AliasTitleLinker
            .instruments_named(&UntrustedText::try_new(title).expect("제목"), &names())
            .iter()
            .map(|instrument| instrument.as_str().to_owned())
            .collect()
    }

    fn ids(symbols: &[&str]) -> Vec<String> {
        symbols
            .iter()
            .map(|symbol| {
                InstrumentId::try_new(symbol)
                    .expect("종목")
                    .as_str()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn holding_names_and_codes_link_articles_without_any_alias_setup() {
        assert_eq!(linked("삼성전자, 3분기 영업익 10조"), ids(&["005930"]));
        assert_eq!(linked("SK하이닉스 HBM 공급 확대"), ids(&["000660"]));
        assert_eq!(linked("[특징주] 005930 외국인 순매수"), ids(&["005930"]));
        assert_eq!(linked("AAPL shares rise after event"), ids(&["AAPL"]));
        assert_eq!(linked("애플 주가 급등"), ids(&["AAPL"]));
        assert_eq!(linked("[특징주]삼성전자 급등"), ids(&["005930"]));
        assert_eq!(linked("[유]삼성전자 주요사항보고서"), ids(&["005930"]));
    }

    #[test]
    fn group_prefixes_and_short_common_names_do_not_link_unrelated_articles() {
        assert_eq!(
            linked("삼성바이오로직스 신공장 착공"),
            ids(&[]),
            "삼성 계열사"
        );
        assert_eq!(linked("삼성 그룹 인사 발표"), ids(&[]), "그룹명만");
        assert_eq!(linked("애플망고 수입 늘어"), ids(&[]), "다른 단어의 일부");
        assert_eq!(
            linked("올해 애플 신제품 공개 행사"),
            ids(&[]),
            "증권 문맥 없음"
        );
    }
}
