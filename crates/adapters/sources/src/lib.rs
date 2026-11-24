mod article_reader;
mod endpoint;
mod headers;
mod http;
mod news_rss;
mod rate_limiter;
mod registry;
mod rss;

pub use article_reader::{ArticleReadError, ArticleReader};
pub use endpoint::SourceEndpoint;
pub use headers::{
    BROWSER_USER_AGENT, ConditionalValidators, ContactUserAgent, STANDARD_USER_AGENT,
    ValidatorHeader,
};
pub use http::{
    SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient, SourceHttpError,
    SourcePayload, http_error_to_information, http_status_to_error, outcome_to_attempt,
};
pub use news_rss::{PressMacroSource, RssNewsFeed};
pub use rate_limiter::{SourceWait, TokioSourceWait};
pub use registry::{
    AiReadSettingPolicy, CredentialRequirement, HostAllowance, HttpDataSource, RateLimitBucket,
    SourceRegistry, SourceSpec,
};
pub use rss::{
    ParsedFeedItem, collapse_whitespace, parse_feed, parse_feed_timestamp, strip_html_tags,
};
