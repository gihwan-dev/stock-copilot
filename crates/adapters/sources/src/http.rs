use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use application::{
    Clock, ConditionalGet, CredentialSource, InformationError, ResetTime, ResponseMismatch,
    SecretValue, SourceAttemptOutcome,
};
use reqwest::Client;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{HeaderMap, IF_MODIFIED_SINCE, IF_NONE_MATCH, USER_AGENT};

use crate::endpoint::{EndpointMode, resolve_wire_url};
use crate::headers::resolve_headers;
use crate::rate_limiter::SourceRateLimiter;
use crate::{
    ConditionalValidators, ContactUserAgent, HttpDataSource, SourceEndpoint, SourceRegistry,
    SourceWait,
};

const MAX_BODY_BYTES: usize = 8_388_608;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceHttpError {
    InvalidEndpoint,
    DisallowedUrl,
    InvalidHeader,
    InvalidCredential,
    MissingContactUserAgent,
    Configuration,
    Transport,
    RedirectBlocked,
    UnexpectedNotModified,
    ResponseTooLarge,
    RateLimited,
}

pub struct SourceCredential {
    source: CredentialSource,
    value: SecretValue,
}

impl fmt::Debug for SourceCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceCredential")
            .field("source", &self.source)
            .field("value", &"<redacted>")
            .finish()
    }
}

impl SourceCredential {
    #[must_use]
    pub const fn new(source: CredentialSource, value: SecretValue) -> Self {
        Self { source, value }
    }

    #[must_use]
    pub const fn source(&self) -> CredentialSource {
        self.source
    }

    #[must_use]
    pub const fn value(&self) -> &SecretValue {
        &self.value
    }
}

pub struct SourceFetch {
    source: HttpDataSource,
    url: String,
    validators: Option<ConditionalValidators>,
    credential: Option<SourceCredential>,
}

impl fmt::Debug for SourceFetch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceFetch")
            .field("source", &self.source)
            .field("has_validators", &self.validators.is_some())
            .field("has_credential", &self.credential.is_some())
            .finish_non_exhaustive()
    }
}

impl SourceFetch {
    #[must_use]
    pub fn new(source: HttpDataSource, url: &str) -> Self {
        Self {
            source,
            url: url.to_owned(),
            validators: None,
            credential: None,
        }
    }

    #[must_use]
    pub fn with_validators(mut self, validators: ConditionalValidators) -> Self {
        self.validators = Some(validators);
        self
    }

    #[must_use]
    pub fn with_credential(mut self, credential: SourceCredential) -> Self {
        self.credential = Some(credential);
        self
    }

    #[must_use]
    pub const fn source(&self) -> HttpDataSource {
        self.source
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourcePayload {
    status: u16,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    content_type: Option<String>,
}

impl fmt::Debug for SourcePayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourcePayload")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .field("has_etag", &self.etag.is_some())
            .field("has_last_modified", &self.last_modified.is_some())
            .finish_non_exhaustive()
    }
}

impl SourcePayload {
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[must_use]
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    #[must_use]
    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFetchOutcome {
    Modified(SourcePayload),
    NotModified,
    HttpFailure {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
}

#[derive(Debug)]
struct LoopbackResolver;

impl Resolve for LoopbackResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            if name.as_str() != "localhost" {
                let error: Box<dyn std::error::Error + Send + Sync> =
                    Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
                return Err(error);
            }
            let addresses: Addrs = Box::new(std::iter::once(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                0,
            )));
            Ok(addresses)
        })
    }
}

pub(crate) fn configured_client(mode: EndpointMode) -> reqwest::ClientBuilder {
    let builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .http1_only();
    match mode {
        EndpointMode::Live => builder.https_only(true),
        EndpointMode::Loopback(_) => builder
            .https_only(false)
            .dns_resolver(Arc::new(LoopbackResolver)),
    }
}

fn read_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn parse_retry_after_seconds(headers: &HeaderMap) -> Option<u64> {
    let value = read_header(headers, "retry-after")?;
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    trimmed.parse::<u64>().ok()
}

async fn read_bounded_body(
    mut response: reqwest::Response,
    status: u16,
) -> Result<SourcePayload, SourceHttpError> {
    let etag = read_header(response.headers(), "etag");
    let last_modified = read_header(response.headers(), "last-modified");
    let content_type = read_header(response.headers(), "content-type");
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_error| SourceHttpError::Transport)?
    {
        if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(SourceHttpError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(SourcePayload {
        status,
        body,
        etag,
        last_modified,
        content_type,
    })
}

pub struct SourceHttpClient {
    client: Client,
    endpoint: SourceEndpoint,
    limiter: SourceRateLimiter,
    contact: Option<ContactUserAgent>,
}

impl fmt::Debug for SourceHttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceHttpClient(<redacted>)")
    }
}

impl SourceHttpClient {
    pub fn new(
        endpoint: SourceEndpoint,
        clock: Arc<dyn Clock>,
        wait: Arc<dyn SourceWait>,
        contact: Option<ContactUserAgent>,
    ) -> Result<Self, SourceHttpError> {
        let client = configured_client(endpoint.mode())
            .build()
            .map_err(|_error| SourceHttpError::Configuration)?;
        Ok(Self {
            client,
            endpoint,
            limiter: SourceRateLimiter::new(clock, wait),
            contact,
        })
    }

    #[must_use]
    pub fn now(&self) -> domain::Timestamp {
        self.limiter.current_timestamp()
    }

    pub async fn fetch(&self, target: &SourceFetch) -> Result<SourceFetchOutcome, SourceHttpError> {
        let spec = SourceRegistry::spec(target.source);
        let headers = resolve_headers(spec, self.contact.as_ref(), target.validators.as_ref())?;
        let wire_url =
            resolve_wire_url(spec, self.endpoint, &target.url, target.credential.as_ref())?;
        let mut budget = self.limiter.lock(spec.bucket()).await;
        self.limiter.admit(&mut budget).await?;
        let mut call = self
            .client
            .get(wire_url)
            .header(USER_AGENT, headers.user_agent);
        if let Some(etag) = headers.if_none_match {
            call = call.header(IF_NONE_MATCH, etag);
        }
        if let Some(modified) = headers.if_modified_since {
            call = call.header(IF_MODIFIED_SINCE, modified);
        }
        let response = match call.send().await {
            Ok(resp) => resp,
            Err(_error) => {
                self.limiter.mark_finished(&mut budget);
                return Err(SourceHttpError::Transport);
            }
        };
        let status = response.status().as_u16();
        let outcome = match status {
            304 => match spec.conditional_get() {
                ConditionalGet::Supported => Ok(SourceFetchOutcome::NotModified),
                ConditionalGet::Unsupported => Err(SourceHttpError::UnexpectedNotModified),
            },
            300..=399 => Err(SourceHttpError::RedirectBlocked),
            200..=299 => read_bounded_body(response, status)
                .await
                .map(SourceFetchOutcome::Modified),
            429 | 503 => {
                let retry_after = parse_retry_after_seconds(response.headers());
                if let Some(seconds) = retry_after {
                    self.limiter.defer_seconds(&mut budget, seconds);
                }
                Ok(SourceFetchOutcome::HttpFailure {
                    status,
                    retry_after_seconds: retry_after,
                })
            }
            _ => Ok(SourceFetchOutcome::HttpFailure {
                status,
                retry_after_seconds: parse_retry_after_seconds(response.headers()),
            }),
        };
        self.limiter.mark_finished(&mut budget);
        outcome
    }
}

#[must_use]
pub fn http_status_to_error(source: HttpDataSource, status: u16) -> InformationError {
    match status {
        401 | 403 => match SourceRegistry::spec(source).credential() {
            crate::CredentialRequirement::Optional(cred) => {
                InformationError::InvalidCredential(cred)
            }
            crate::CredentialRequirement::None => {
                InformationError::Unavailable(ResetTime::NotProvided)
            }
        },
        429 | 500..=599 => InformationError::Unavailable(ResetTime::NotProvided),
        _ => InformationError::InvalidResponse(ResponseMismatch::Schema),
    }
}

#[must_use]
pub fn http_error_to_information(
    source: HttpDataSource,
    error: SourceHttpError,
) -> InformationError {
    match error {
        SourceHttpError::InvalidCredential => match SourceRegistry::spec(source).credential() {
            crate::CredentialRequirement::Optional(cred) => {
                InformationError::InvalidCredential(cred)
            }
            crate::CredentialRequirement::None => {
                InformationError::InvalidResponse(ResponseMismatch::Schema)
            }
        },
        SourceHttpError::Transport | SourceHttpError::RateLimited => {
            InformationError::Unavailable(ResetTime::NotProvided)
        }
        SourceHttpError::InvalidEndpoint
        | SourceHttpError::DisallowedUrl
        | SourceHttpError::InvalidHeader
        | SourceHttpError::MissingContactUserAgent
        | SourceHttpError::Configuration
        | SourceHttpError::RedirectBlocked
        | SourceHttpError::UnexpectedNotModified
        | SourceHttpError::ResponseTooLarge => {
            InformationError::InvalidResponse(ResponseMismatch::Schema)
        }
    }
}

#[must_use]
pub fn outcome_to_attempt(
    source: HttpDataSource,
    result: &Result<SourceFetchOutcome, SourceHttpError>,
) -> SourceAttemptOutcome {
    match result {
        Ok(SourceFetchOutcome::Modified(_) | SourceFetchOutcome::NotModified) => {
            SourceAttemptOutcome::Succeeded
        }
        Ok(SourceFetchOutcome::HttpFailure { status, .. }) => match status {
            401 | 403 => match SourceRegistry::spec(source).credential() {
                crate::CredentialRequirement::Optional(cred) => {
                    SourceAttemptOutcome::Failed(InformationError::InvalidCredential(cred))
                }
                crate::CredentialRequirement::None => SourceAttemptOutcome::Failed(
                    InformationError::Unavailable(ResetTime::NotProvided),
                ),
            },
            429 | 500..=599 => {
                SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::NotProvided))
            }
            _ => SourceAttemptOutcome::Failed(InformationError::InvalidResponse(
                ResponseMismatch::Schema,
            )),
        },
        Err(SourceHttpError::MissingContactUserAgent) => SourceAttemptOutcome::NotConfigured,
        Err(SourceHttpError::InvalidCredential) => {
            match SourceRegistry::spec(source).credential() {
                crate::CredentialRequirement::Optional(cred) => {
                    SourceAttemptOutcome::Failed(InformationError::InvalidCredential(cred))
                }
                crate::CredentialRequirement::None => SourceAttemptOutcome::Failed(
                    InformationError::InvalidResponse(ResponseMismatch::Schema),
                ),
            }
        }
        Err(SourceHttpError::Transport | SourceHttpError::RateLimited) => {
            SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::NotProvided))
        }
        Err(
            SourceHttpError::InvalidEndpoint
            | SourceHttpError::DisallowedUrl
            | SourceHttpError::InvalidHeader
            | SourceHttpError::Configuration
            | SourceHttpError::RedirectBlocked
            | SourceHttpError::UnexpectedNotModified
            | SourceHttpError::ResponseTooLarge,
        ) => SourceAttemptOutcome::Failed(InformationError::InvalidResponse(
            ResponseMismatch::Schema,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{EndpointMode, LoopbackResolver, SourceHttpError, configured_client};
    use reqwest::dns::Resolve;

    #[test]
    fn live_client_configuration_rejects_plaintext_before_connecting() {
        let probe = testkit::ConnectionProbe::bind().expect("probe");
        let port = probe.port().expect("port");
        let client = configured_client(EndpointMode::Live)
            .build()
            .expect("client");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let outcome = rt
            .block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    client.get(format!("http://127.0.0.1:{port}/feed")).send(),
                )
                .await
            })
            .expect("scheme refusal before I/O")
            .map_err(|error| {
                assert!(error.is_builder());
                SourceHttpError::Transport
            });
        assert_eq!(outcome.expect_err("plaintext"), SourceHttpError::Transport);
        assert_eq!(
            probe.observe().expect("probe"),
            testkit::ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn builder_keeps_documented_timeouts_and_http1() {
        for mode in [EndpointMode::Live, EndpointMode::Loopback(8080)] {
            let config = format!("{:?}", configured_client(mode));
            assert!(config.contains("connect_timeout: 5s"), "{config}");
            assert!(config.contains("timeout: 30s"), "{config}");
            assert!(config.contains("http1_only: true"), "{config}");
        }
    }

    #[test]
    fn loopback_resolver_allows_only_localhost() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let addresses: Vec<_> = rt
            .block_on(LoopbackResolver.resolve("localhost".parse().expect("name")))
            .expect("loopback")
            .collect();
        assert_eq!(addresses, vec!["127.0.0.1:0".parse().expect("address")]);
        let _err = rt
            .block_on(LoopbackResolver.resolve("other.invalid".parse().expect("name")))
            .map(Iterator::collect::<Vec<_>>)
            .expect_err("non-localhost");
    }
}
