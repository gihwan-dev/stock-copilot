use std::future::Future;

use domain::{InformationSource, NewsSource, NormalizedUrl, Timestamp, UntrustedText};

use crate::{CollectedItemHandle, SystemPortError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleLocation {
    pub source: InformationSource,
    pub url: NormalizedUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyReadingSetting {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleBody {
    pub text: UntrustedText,
    pub fetched_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticleFetchFailure {
    NotFound,
    Unavailable(SystemPortError),
}

pub trait ArticleToolPort: Send + Sync {
    fn locate(
        &self,
        handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<ArticleLocation>, SystemPortError>> + Send;

    fn body_reading(
        &self,
        source: &NewsSource,
    ) -> impl Future<Output = Result<BodyReadingSetting, SystemPortError>> + Send;

    fn fetch_body(
        &self,
        url: &NormalizedUrl,
    ) -> impl Future<Output = Result<ArticleBody, ArticleFetchFailure>> + Send;
}
