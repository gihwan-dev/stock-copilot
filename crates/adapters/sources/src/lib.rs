mod endpoint;
mod headers;
mod http;
mod rate_limiter;
mod registry;

pub use endpoint::SourceEndpoint;
pub use headers::{
    BROWSER_USER_AGENT, ConditionalValidators, ContactUserAgent, STANDARD_USER_AGENT,
    ValidatorHeader,
};
pub use http::{
    SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient, SourceHttpError,
    SourcePayload, outcome_to_attempt,
};
pub use rate_limiter::{SourceWait, TokioSourceWait};
pub use registry::{
    AiReadSettingPolicy, CredentialRequirement, HostAllowance, HttpDataSource, RateLimitBucket,
    SourceRegistry, SourceSpec,
};
