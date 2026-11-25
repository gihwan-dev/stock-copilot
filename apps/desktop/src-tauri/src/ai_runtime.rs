use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, ChatHistory, Clock, ExecutablePath,
    InformationError, IsolationCheck, PeerProcessInspector, ProcessId, ResetTime, RunKind, RunSpec,
    SecretValue, ToolCallLedger,
};
use domain::UntrustedText;
use mcp_server::{
    McpServerConfig, McpServerHandle, PeerInspectorMode, PeerProcessTarget, RunIdentity,
    RunServerDeps, RunServerRequest, RunSocketSpec, ToolLedgerMode, ToolRegistry,
};
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

#[cfg(test)]
mod tests;

type BindParent = Box<dyn FnOnce(Option<ProcessId>) + Send>;

pub struct ParentPidBinder(BindParent);

impl fmt::Debug for ParentPidBinder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ParentPidBinder")
    }
}

impl ParentPidBinder {
    #[must_use]
    pub fn new<F>(bind: F) -> Self
    where
        F: FnOnce(Option<ProcessId>) + Send + 'static,
    {
        Self(Box::new(bind))
    }

    pub fn bind(self, pid: Option<ProcessId>) {
        (self.0)(pid);
    }
}

pub struct McpRunLink {
    pub socket_path: PathBuf,
    pub token: SecretValue,
    pub binder: ParentPidBinder,
}

impl fmt::Debug for McpRunLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpRunLink")
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

pub trait AttachableAi: AiProvider {
    fn run_with_mcp(
        &self,
        spec: RunSpec,
        link: McpRunLink,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send;

    fn resume_with_mcp(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
        link: McpRunLink,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send;

    fn verify_with_mcp(
        &self,
        link: McpRunLink,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send;
}

pub trait McpRunSession: Send + Sync + 'static {
    fn link(&self) -> Result<McpRunLink, InformationError>;
}

pub trait RunMcpHost: Send + Sync {
    type Session: McpRunSession;

    fn open(&self, run: &AiRunId) -> Result<Self::Session, InformationError>;
}

pub struct AttachedStream<S, M> {
    inner: S,
    session: Option<M>,
}

impl<S, M> fmt::Debug for AttachedStream<S, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachedStream")
            .field("mcp_open", &self.session.is_some())
            .finish_non_exhaustive()
    }
}

impl<S: AiEventStream, M: Send> AiEventStream for AttachedStream<S, M> {
    async fn next(&mut self) -> Result<Option<AiEvent>, InformationError> {
        let next = self.inner.next().await;
        match &next {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                self.session = None;
            }
        }
        next
    }
}

pub struct McpAttachedAi<A, H> {
    inner: Arc<A>,
    host: Arc<H>,
}

impl<A, H> fmt::Debug for McpAttachedAi<A, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("McpAttachedAi")
    }
}

impl<A, H> McpAttachedAi<A, H> {
    #[must_use]
    pub const fn new(inner: Arc<A>, host: Arc<H>) -> Self {
        Self { inner, host }
    }
}

impl<A: AttachableAi, H: RunMcpHost> AiProvider for McpAttachedAi<A, H> {
    type Stream = AttachedStream<A::Stream, H::Session>;

    async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
        match spec.kind() {
            RunKind::Collection => Ok(AttachedStream {
                inner: self.inner.run(spec).await?,
                session: None,
            }),
            RunKind::Analysis => {
                let session = self.host.open(spec.id())?;
                let inner = self.inner.run_with_mcp(spec, session.link()?).await?;
                Ok(AttachedStream {
                    inner,
                    session: Some(session),
                })
            }
        }
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        match spec.kind() {
            RunKind::Collection => Ok(AttachedStream {
                inner: self.inner.resume(spec, history, message).await?,
                session: None,
            }),
            RunKind::Analysis => {
                let session = self.host.open(spec.id())?;
                let inner = self
                    .inner
                    .resume_with_mcp(spec, history, message, session.link()?)
                    .await?;
                Ok(AttachedStream {
                    inner,
                    session: Some(session),
                })
            }
        }
    }

    async fn usage(&self) -> Result<AiUsage, InformationError> {
        self.inner.usage().await
    }

    async fn interrupt(&self, run: &AiRunId) -> Result<(), InformationError> {
        self.inner.interrupt(run).await
    }

    async fn verify_isolation(&self, kind: RunKind) -> Result<IsolationCheck, InformationError> {
        match kind {
            RunKind::Collection => self.inner.verify_isolation(kind).await,
            RunKind::Analysis => {
                let probe = AiRunId::try_new("isolation-probe-analysis")
                    .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
                let session = self.host.open(&probe)?;
                self.inner.verify_with_mcp(session.link()?).await
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BridgeLocation(Arc<OnceLock<PathBuf>>);

impl BridgeLocation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, path: PathBuf) -> Result<(), PathBuf> {
        self.0.set(path)
    }

    #[must_use]
    pub fn get(&self) -> Option<&Path> {
        self.0.get().map(PathBuf::as_path)
    }

    fn executable(&self) -> Option<ExecutablePath> {
        ExecutablePath::try_new(self.get()?.to_str()?).ok()
    }
}

pub struct McpServerSession<C: Clock + 'static> {
    handle: Arc<McpServerHandle<C>>,
}

impl<C: Clock + 'static> fmt::Debug for McpServerSession<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpServerSession")
            .field("socket_path", &self.handle.socket_path())
            .finish_non_exhaustive()
    }
}

impl<C: Clock + 'static> McpServerSession<C> {
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        self.handle.socket_path()
    }

    #[must_use]
    pub fn run_kind(&self) -> RunKind {
        self.handle.run_kind()
    }
}

impl<C: Clock + 'static> McpRunSession for McpServerSession<C> {
    fn link(&self) -> Result<McpRunLink, InformationError> {
        let weak: Weak<McpServerHandle<C>> = Arc::downgrade(&self.handle);
        Ok(McpRunLink {
            socket_path: self.handle.socket_path().to_path_buf(),
            token: SecretValue::new(self.handle.token().as_exposed_bytes().to_vec()),
            binder: ParentPidBinder::new(move |pid| bind_parent(&weak, pid)),
        })
    }
}

pub struct McpServerHost<C, P, L> {
    clock: Arc<C>,
    inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    registry: Arc<ToolRegistry>,
    ledger: ToolLedgerMode<L>,
    base_dir: PathBuf,
    bridge: BridgeLocation,
}

impl<C, P, L> fmt::Debug for McpServerHost<C, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpServerHost")
            .field("base_dir", &self.base_dir)
            .field("tool_count", &self.registry.tool_names().len())
            .finish_non_exhaustive()
    }
}

pub struct McpServerHostParts<C, P, L> {
    pub clock: Arc<C>,
    pub inspector: PeerInspectorMode<P>,
    pub config: McpServerConfig,
    pub registry: Arc<ToolRegistry>,
    pub ledger: ToolLedgerMode<L>,
    pub base_dir: PathBuf,
    pub bridge: BridgeLocation,
}

impl<C, P, L> fmt::Debug for McpServerHostParts<C, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpServerHostParts")
            .field("base_dir", &self.base_dir)
            .finish_non_exhaustive()
    }
}

impl<C, P, L> McpServerHost<C, P, L> {
    #[must_use]
    pub fn new(parts: McpServerHostParts<C, P, L>) -> Self {
        Self {
            clock: parts.clock,
            inspector: parts.inspector,
            config: parts.config,
            registry: parts.registry,
            ledger: parts.ledger,
            base_dir: parts.base_dir,
            bridge: parts.bridge,
        }
    }

    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

impl<C, P, L> RunMcpHost for McpServerHost<C, P, L>
where
    C: Clock + 'static,
    P: PeerProcessInspector + 'static,
    L: ToolCallLedger + 'static,
{
    type Session = McpServerSession<C>;

    fn open(&self, run: &AiRunId) -> Result<Self::Session, InformationError> {
        let unavailable = |_error| InformationError::Unavailable(ResetTime::NotProvided);
        let bridge = self.bridge.executable().ok_or_else(missing)?;
        ensure_private_dir(&self.base_dir).map_err(unavailable)?;
        let socket = RunSocketSpec::generate_under(&self.base_dir)
            .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
        let deps = RunServerDeps::new(Arc::clone(&self.clock), self.inspector.clone(), self.config)
            .with_tools(Arc::clone(&self.registry), self.ledger.clone());
        let request = RunServerRequest::new(
            RunIdentity::new(run.clone(), RunKind::Analysis),
            socket,
            PeerProcessTarget::pending_parent(bridge),
            deps,
        );
        let handle = McpServerHandle::start(request)
            .map_err(|_error| InformationError::Unavailable(ResetTime::NotProvided))?;
        Ok(McpServerSession {
            handle: Arc::new(handle),
        })
    }
}

fn bind_parent<C: Clock + 'static>(weak: &Weak<McpServerHandle<C>>, pid: Option<ProcessId>) {
    if let (Some(handle), Some(parent)) = (weak.upgrade(), pid) {
        match handle.bind_expected_parent_pid(parent) {
            Ok(()) | Err(_) => {}
        }
    }
}

fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[derive(Debug)]
pub enum AiAvailability<A> {
    Ready(A),
    Missing,
}

const fn missing() -> InformationError {
    InformationError::Unavailable(ResetTime::NotProvided)
}

impl<A: AiProvider> AiProvider for AiAvailability<A> {
    type Stream = A::Stream;

    async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
        match self {
            Self::Ready(inner) => inner.run(spec).await,
            Self::Missing => Err(missing()),
        }
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        match self {
            Self::Ready(inner) => inner.resume(spec, history, message).await,
            Self::Missing => Err(missing()),
        }
    }

    async fn usage(&self) -> Result<AiUsage, InformationError> {
        match self {
            Self::Ready(inner) => inner.usage().await,
            Self::Missing => Err(missing()),
        }
    }

    async fn interrupt(&self, run: &AiRunId) -> Result<(), InformationError> {
        match self {
            Self::Ready(inner) => inner.interrupt(run).await,
            Self::Missing => Err(missing()),
        }
    }

    async fn verify_isolation(&self, kind: RunKind) -> Result<IsolationCheck, InformationError> {
        match self {
            Self::Ready(inner) => inner.verify_isolation(kind).await,
            Self::Missing => Err(missing()),
        }
    }
}
