use application::{AiProvider, AiRunId, ChatHistory, InformationError, RunSpec};
use src_tauri::ai_runtime::{
    AiAvailability, AttachableAi, BridgeLocation, ChatTurnBook, LateAi, McpAttachedAi, McpRunLink,
    McpServerHost, McpServerHostParts, RunMetadataBook,
};
use src_tauri::audit_queue::AuditEventQueue;
use src_tauri::collected_links::{CollectedLinker, TitleLinker};
use src_tauri::ipc::briefing::BriefingIpcState;
use src_tauri::ipc::open_external::ExternalLinkRuntime;
use src_tauri::mcp_catalog::{
    ANALYSIS_MCP_SERVER, CatalogStores, allowed_tool_names, analysis_catalog,
};
use src_tauri::mcp_sources::ArticleBodies;
use src_tauri::platform::collector::{CollectorFeed, CollectorFeeds, SharedFeed};
use src_tauri::platform::intraday::{BridgedIntradayWatch, IntradayWatchSlot};
use src_tauri::platform::notifier::{
    AsyncRuntimeNotificationDispatch, FilteredNotifier, NotificationDelivery,
    NotificationEventSubscriber, NotificationPreferences, UnavailableNotificationDelivery,
};
use src_tauri::platform::peer_process::PsPeerInspector;
use src_tauri::platform::scheduling::SpecWatchInputs;
use src_tauri::platform::single_instance::{InstanceAcquisition, SocketPath, attach_to_app};
use src_tauri::product_events::{
    BridgedUiEvents, FlushingAuditSink, RecordedTriggerUrgency, UiEventSlot, ui_event_kinds,
};
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
    let repositories = Arc::new(SqliteRepositories::new(SharedDatabase::new()));
    let notifier = Arc::new(FilteredNotifier::new(
        UnavailableNotificationDelivery,
        NotificationPreferences::default(),
    ));
    let ui_events = Arc::new(UiEventSlot::<tauri::Wry>::default());
    let intraday = Arc::new(IntradayWatchSlot::new());
    let (bus, scheduler_events) = product_event_bus(ProductSubscribers {
        notifier: Arc::clone(&notifier),
        audit: Arc::clone(&repositories),
        clock: Arc::clone(&shared_clock),
        ui_events: Arc::clone(&ui_events),
        intraday: Arc::clone(&intraday),
    });
    let events = Arc::new(bus);
    let turns = ChatTurnBook::new();
    let ai = assemble_ai(
        discover_ai_setup(
            &context.config().identifier,
            bundled_bridge(),
            turns.clone(),
        ),
        &shared_clock,
        &events,
        &repositories,
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let ports = product_ports(&repositories, notifier, &shared_clock, events, ai);
    let app = build_app(
        product_builder(scheduler_events, &repositories, ui_events, &shared_clock)
            .manage(intraday)
            .manage(turns),
        ports,
        context,
    )?;
    let socket_path = SocketPath::try_new(
        tauri::Manager::path(&app)
            .app_data_dir()?
            .join("instance.sock"),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let single_instance = attach_to_app(app.handle(), socket_path)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    match single_instance {
        InstanceAcquisition::Primary(_guard) => {
            run_app(app, |handle, code| handle.exit(code.as_i32()));
            Ok(())
        }
        InstanceAcquisition::SecondaryNotified => Ok(()),
    }
}

fn product_ports(
    repositories: &Arc<SqliteRepositories>,
    notifier: Arc<ProductNotifier>,
    shared_clock: &Arc<platform_macos::SystemClock>,
    events: Arc<application::EventBus>,
    ai: ProductAiParts,
) -> Ports<AppPortTypes> {
    Ports::<AppPortTypes> {
        broker_read: Arc::new(Unavailable),
        market_info: Arc::new(Unavailable),
        macro_series: Arc::new(Unavailable),
        fundamentals: Arc::new(Unavailable),
        ai_provider: ai.ai,
        chat_repository: Arc::clone(repositories),
        chat_ledger: Arc::clone(repositories),
        chat_queue: ai.queue,
        warm_chat: Arc::new(Unavailable),
        ai_jobs: ai.jobs,
        portfolios: Arc::new(Unavailable),
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
        secrets: Arc::new(product_secrets()),
        notifier,
        news_feeds: Vec::new(),
        clock: Arc::clone(shared_clock),
        presence: Arc::new(platform_macos::MacOsRiskDialogPresence::unavailable()),
        events,
        saved_accounts: Vec::new(),
        database: Arc::new(database_factory(Arc::clone(shared_clock))),
    }
}

type ProductSecrets = secrets_keychain::KeychainSecretStore<secrets_keychain::SystemKeychain>;

fn product_secrets() -> ProductSecrets {
    let build = if tauri::is_dev() {
        secrets_keychain::BuildKind::Development
    } else {
        secrets_keychain::BuildKind::Release
    };
    let profile = match build {
        secrets_keychain::BuildKind::Development => secrets_keychain::KeychainProfile::development(),
        secrets_keychain::BuildKind::Release => secrets_keychain::KeychainProfile::product_login(),
    };
    secrets_keychain::KeychainSecretStore::new(
        profile,
        secrets_keychain::SystemKeychain,
    )
}

fn database_key_provision() -> DatabaseKeyProvision {
    DatabaseKeyProvision(Box::new(secrets_keychain::generate_database_key))
}

fn product_builder(
    scheduler_events: Arc<scheduler::SchedulerEventBridge>,
    repositories: &Arc<SqliteRepositories>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
    clock: &Arc<platform_macos::SystemClock>,
) -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default()
        .manage(ExternalLinkRuntime::new(
            Arc::new(Unavailable),
            Arc::new(Unavailable),
        ))
        .manage(scheduler_events)
        .manage(Arc::clone(repositories))
        .manage(BriefingIpcState::from_repository(Arc::clone(repositories)))
        .manage(database_key_provision())
        .manage(ui_events)
        .manage(CollectedLinker(Arc::new(AliasTitleLinker)));
    let with_feeds = builder.manage(collector_feeds(clock));
    match spec_watch_inputs(clock) {
        Some(inputs) => with_feeds.manage(inputs),
        None => with_feeds,
    }
}

fn collector_feeds(clock: &Arc<platform_macos::SystemClock>) -> CollectorFeeds {
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    let Ok(http) = sources::SourceHttpClient::new(
        sources::SourceEndpoint::live(),
        shared,
        Arc::new(sources::TokioSourceWait),
        None,
    ) else {
        return CollectorFeeds::default();
    };
    let client = Arc::new(http);
    let mut feeds: Vec<CollectorFeed> = sources::PressMacroSource::ALL
        .into_iter()
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
    CollectorFeeds(feeds)
}

const fn press_target(source: sources::PressMacroSource) -> application::CollectionTarget {
    match source {
        sources::PressMacroSource::Hankyung => application::CollectionTarget::HankyungRss,
        sources::PressMacroSource::Maekyung
        | sources::PressMacroSource::Yonhap
        | sources::PressMacroSource::Edaily => application::CollectionTarget::PressRss,
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
type AppPortTypes =
    ProductPorts<SqliteRepositories, ProductSecrets, ProductAi, ProductQueue, ProductAiJobs>;

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
        self.provider.run(spec).await
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: domain::UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        self.provider.resume(spec, history, message).await
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
        self.provider
            .run_attached(spec, self.attachment(link)?)
            .await
    }

    async fn resume_with_mcp(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: domain::UntrustedText,
        link: McpRunLink,
    ) -> Result<Self::Stream, InformationError> {
        self.provider
            .resume_attached(spec, history, message, self.attachment(link)?)
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
    (runs, collector): (
        &RunMetadataBook,
        &LateAi<ai_claude_code::ClaudeCodeProvider>,
    ),
) -> Option<mcp_server::ToolRegistry> {
    analysis_catalog(
        clock,
        CatalogStores {
            theses: Arc::clone(repositories),
            estimates: Arc::clone(repositories),
            bodies: Arc::new(ProductArticleBodies::new(clock)),
            runs: runs.clone(),
            collector: Arc::new(collector.clone()),
            turns: setup.turns.clone(),
            events: Arc::clone(events),
        },
    )
    .ok()
}

fn claude_ai(
    setup: AiSetup,
    clock: &Arc<platform_macos::SystemClock>,
    events: &Arc<application::EventBus>,
    repositories: &Arc<SqliteRepositories>,
) -> Option<McpAttachedAi<ProductClaude, ProductMcpHost>> {
    let runs = RunMetadataBook::new().with_hasher(Arc::new(store_sqlite::Sha256PromptHasher));
    let collector = LateAi::<ai_claude_code::ClaudeCodeProvider>::new();
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
    .with_collection_json_schema(String::from(ai_claude_code::WEB_COLLECTION_JSON_SCHEMA));
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
    let _bound = collector.bind(Arc::clone(&provider));
    let host = McpServerHost::new(McpServerHostParts {
        clock: Arc::clone(clock),
        inspector: mcp_server::PeerInspectorMode::Configured(Arc::new(PsPeerInspector::default())),
        config: mcp_server::McpServerConfig::default_config().ok()?,
        registry: Arc::new(registry),
        ledger: mcp_server::ToolLedgerMode::Configured(Arc::clone(repositories)),
        base_dir: setup.mcp_base,
        bridge: setup.bridge.clone(),
    });
    Some(
        McpAttachedAi::new(
            Arc::new(ProductClaude {
                provider,
                bridge: setup.bridge,
            }),
            Arc::new(host),
        )
        .with_runs(runs),
    )
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

fn discover_ai_setup(identifier: &str, bridge: BridgeLocation, turns: ChatTurnBook) -> AiSetup {
    let host_env = ai_claude_code::HostEnvironment::from_pairs(std::env::vars());
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
    let executable = match ai_claude_code::find_claude_executable(
        &ai_claude_code::DiscoveryConfig::new(host_env.clone()),
        &ai_claude_code::SystemLoginShellPathResolver,
    ) {
        ai_claude_code::DiscoveryOutcome::Found { path, .. } => Some(path),
        ai_claude_code::DiscoveryOutcome::NotInstalled => None,
    };
    AiSetup {
        executable,
        sandbox: app_data.and_then(|dir| probed_sandbox(&dir, &work_root)),
        host_env,
        mcp_base: temp_root.join("sc-mcp"),
        work_root,
        bridge,
        cli_projects_dir: None,
        turns,
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

type ProductNotifier = FilteredNotifier<UnavailableNotificationDelivery>;

struct ProductSubscribers<D> {
    notifier: Arc<FilteredNotifier<D>>,
    audit: Arc<SqliteRepositories>,
    clock: Arc<platform_macos::SystemClock>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
    intraday: Arc<IntradayWatchSlot>,
}

fn product_event_bus<D: NotificationDelivery + 'static>(
    subscribers: ProductSubscribers<D>,
) -> (application::EventBus, Arc<scheduler::SchedulerEventBridge>) {
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
    (builder.build(), bridge)
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
        let feeds = super::collector_feeds(&clock);
        let sources: Vec<(CollectionTarget, FeedSource)> = feeds
            .0
            .iter()
            .map(|feed| (feed.target, feed.regular.source()))
            .collect();
        assert_eq!(sources.len(), 7);
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
    use src_tauri::platform::notifier::{
        FilteredNotifier, NotificationPreferences, UnavailableNotificationDelivery,
    };
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
            UnavailableNotificationDelivery,
            NotificationPreferences::default(),
        ));
        let events = Arc::new(application::EventBusBuilder::default().build());
        let ai = super::ai_composition_tests::without_claude(&clock, &events, repositories);
        let ports: Ports = super::product_ports(repositories, notifier, &clock, events, ai);
        tauri::test::mock_builder()
            .manage(ports)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("앱")
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
            saved_accounts: Vec::new(),
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
