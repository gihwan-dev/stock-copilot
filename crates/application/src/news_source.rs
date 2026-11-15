use crate::{DataAvailability, InformationError};
use domain::{Disclosure, DisclosureSource, NewsItem, TimeSpan, Timestamp};
use std::future::Future;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectedEntry {
    News(NewsItem),
    Disclosure(Disclosure),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSource {
    News(domain::NewsSource),
    Disclosure(DisclosureSource),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedPosition(domain::UntrustedText);
impl FeedPosition {
    #[must_use]
    pub const fn new(value: domain::UntrustedText) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_untrusted_text(&self) -> &domain::UntrustedText {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedCursor {
    Start,
    Continue(FeedPosition),
}
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
    entry: CollectedEntry,
    published_at: PublicationTime,
    collected_at: Timestamp,
}
impl CollectedObservation {
    #[must_use]
    pub const fn new(
        entry: CollectedEntry,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> Self {
        Self {
            entry,
            published_at,
            collected_at,
        }
    }
    #[must_use]
    pub const fn entry(&self) -> &CollectedEntry {
        &self.entry
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

pub trait NewsFeed: Send + Sync {
    fn source(&self) -> FeedSource;
    fn metadata(&self) -> NewsMetadata;
    fn new_items(
        &self,
        since: FeedCursor,
    ) -> impl Future<
        Output = Result<
            DataAvailability<(Vec<CollectedObservation>, FeedCursor)>,
            InformationError,
        >,
    > + Send;
}
