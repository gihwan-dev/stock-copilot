use crate::InformationError;
use domain::{CollectedItem, TimeSpan, Timestamp};
use std::future::Future;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationTime {
    Known(Timestamp),
    NotProvided,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    For(TimeSpan),
    NotStored,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiReadPolicy {
    TitleOnly,
    TitleAndSummary,
    BodyAllowed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalGet {
    Supported,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAgentRequirement {
    Standard,
    Browser,
    Contact,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsMetadata {
    retention: Retention,
    allow_ai_read: AiReadPolicy,
    supports_conditional_get: ConditionalGet,
    requires_user_agent: UserAgentRequirement,
}
impl NewsMetadata {
    #[must_use]
    pub const fn new(
        retention: Retention,
        allow_ai_read: AiReadPolicy,
        supports_conditional_get: ConditionalGet,
        requires_user_agent: UserAgentRequirement,
    ) -> Self {
        Self {
            retention,
            allow_ai_read,
            supports_conditional_get,
            requires_user_agent,
        }
    }
    #[must_use]
    pub const fn retention(&self) -> Retention {
        self.retention
    }
    #[must_use]
    pub const fn allow_ai_read(&self) -> AiReadPolicy {
        self.allow_ai_read
    }
    #[must_use]
    pub const fn supports_conditional_get(&self) -> ConditionalGet {
        self.supports_conditional_get
    }
    #[must_use]
    pub const fn requires_user_agent(&self) -> UserAgentRequirement {
        self.requires_user_agent
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedObservation {
    item: CollectedItem,
    published_at: PublicationTime,
    collected_at: Timestamp,
}
impl CollectedObservation {
    #[must_use]
    pub const fn new(
        item: CollectedItem,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> Self {
        Self {
            item,
            published_at,
            collected_at,
        }
    }
    #[must_use]
    pub const fn item(&self) -> &CollectedItem {
        &self.item
    }
    #[must_use]
    pub const fn published_at(&self) -> PublicationTime {
        self.published_at
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
}

pub trait NewsSource: Send + Sync {
    fn metadata(&self) -> NewsMetadata;
    fn new_items(
        &self,
    ) -> impl Future<Output = Result<Vec<CollectedObservation>, InformationError>> + Send;
}
