use std::future::Future;

use domain::{Symbol, Timestamp, UntrustedText};

use crate::{AiRunId, SystemPortError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResearchTopic {
    Earnings,
    Guidance,
    Regulation,
    ManagementChange,
    ProductLaunch,
    Litigation,
    IndustryTrend,
    SupplyChain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebResearchToolRequest {
    CodeTopics {
        symbols: Vec<Symbol>,
        topics: Vec<ResearchTopic>,
    },
    StoredUserMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebResearchToolOutcome {
    pub summary: UntrustedText,
    pub collected_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebResearchToolFailure {
    QueryRejected,
    ModeNotAllowed,
    UserMessageMissing,
    CollectionFailed,
    Unavailable(SystemPortError),
}

pub trait WebResearchToolPort: Send + Sync {
    fn known_targets(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<Vec<Symbol>, SystemPortError>> + Send;

    fn request_research(
        &self,
        run: &AiRunId,
        request: &WebResearchToolRequest,
    ) -> impl Future<Output = Result<WebResearchToolOutcome, WebResearchToolFailure>> + Send;
}
