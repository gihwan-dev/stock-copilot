use crate::command::McpBridgeConfig;
use application::{ProcessId, SecretValue};
use std::fmt;

type PidCallback = Box<dyn FnOnce(Option<ProcessId>) + Send>;

pub struct SpawnedPidNotice(PidCallback);

impl fmt::Debug for SpawnedPidNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SpawnedPidNotice")
    }
}

impl SpawnedPidNotice {
    #[must_use]
    pub fn new<F>(callback: F) -> Self
    where
        F: FnOnce(Option<ProcessId>) + Send + 'static,
    {
        Self(Box::new(callback))
    }

    pub(crate) fn notify(self, pid: Option<ProcessId>) {
        (self.0)(pid);
    }
}

pub struct RunMcpAttachment {
    bridge: McpBridgeConfig,
    token: SecretValue,
    notice: Option<SpawnedPidNotice>,
}

impl fmt::Debug for RunMcpAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunMcpAttachment")
            .field("server_name", &self.bridge.server_name())
            .field("has_notice", &self.notice.is_some())
            .finish_non_exhaustive()
    }
}

impl RunMcpAttachment {
    #[must_use]
    pub const fn new(bridge: McpBridgeConfig, token: SecretValue) -> Self {
        Self {
            bridge,
            token,
            notice: None,
        }
    }

    #[must_use]
    pub fn with_spawned_notice(mut self, notice: SpawnedPidNotice) -> Self {
        self.notice = Some(notice);
        self
    }

    pub(crate) const fn bridge(&self) -> &McpBridgeConfig {
        &self.bridge
    }

    pub(crate) const fn token(&self) -> &SecretValue {
        &self.token
    }

    pub(crate) const fn take_notice(&mut self) -> Option<SpawnedPidNotice> {
        self.notice.take()
    }
}
