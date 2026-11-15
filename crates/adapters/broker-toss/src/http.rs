use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use application::{HttpStatus, ReadEndpoint, RetryDelay, SecretValue};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Client, Url};

use crate::endpoint::EndpointMode;
use crate::{
    ReadRequest, TossEndpoint, TossErrorBody, TossMappedError, map_toss_read_error,
    map_toss_token_error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossHttpError {
    InvalidEndpoint,
    InvalidCredential,
    Configuration,
    Transport,
    ResponseTooLarge,
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

pub struct TossHttpClient {
    pub(crate) client: Client,
    pub(crate) origin: Url,
}

impl fmt::Debug for TossHttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TossHttpClient(<redacted>)")
    }
}

pub struct RawResponse {
    status: u16,
    headers: ResponseHeaders,
    body: SecretValue,
}

impl fmt::Debug for RawResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawResponse(<redacted>)")
    }
}

#[derive(Default)]
pub struct ResponseHeaders {
    retry_after: Option<String>,
    limit: Option<String>,
    remaining: Option<String>,
    reset: Option<String>,
}

impl fmt::Debug for ResponseHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ResponseHeaders(<redacted>)")
    }
}

fn sensitive_header(value: &SecretValue) -> Result<HeaderValue, TossHttpError> {
    let mut header = HeaderValue::try_from(value.as_exposed_secret())
        .map_err(|_error| TossHttpError::InvalidCredential)?;
    header.set_sensitive(true);
    Ok(header)
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

impl ResponseHeaders {
    #[must_use]
    pub fn retry_after(&self) -> Option<&str> {
        self.retry_after.as_deref()
    }
    #[must_use]
    pub fn limit(&self) -> Option<&str> {
        self.limit.as_deref()
    }
    #[must_use]
    pub fn remaining(&self) -> Option<&str> {
        self.remaining.as_deref()
    }
    #[must_use]
    pub fn reset(&self) -> Option<&str> {
        self.reset.as_deref()
    }
}

impl RawResponse {
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }
    #[must_use]
    pub const fn headers(&self) -> &ResponseHeaders {
        &self.headers
    }
    #[must_use]
    pub const fn body(&self) -> &SecretValue {
        &self.body
    }
    #[must_use]
    pub fn read_error(&self, endpoint: ReadEndpoint, retry: RetryDelay) -> TossMappedError {
        map_toss_read_error(
            endpoint,
            classify_status(self.status),
            self.error_body(),
            retry,
        )
    }
    #[must_use]
    pub fn token_error(&self, endpoint: ReadEndpoint, retry: RetryDelay) -> TossMappedError {
        map_toss_token_error(
            endpoint,
            classify_status(self.status),
            self.error_body(),
            retry,
        )
    }
    fn error_body(&self) -> TossErrorBody {
        match std::str::from_utf8(self.body.as_exposed_secret()) {
            Ok(body) => TossErrorBody::parse(body),
            Err(_error) => TossErrorBody::parse("invalid-json"),
        }
    }
}

fn classify_status(status: u16) -> HttpStatus {
    match status {
        100..=199 => HttpStatus::Informational,
        200..=299 => HttpStatus::Success,
        300..=399 => HttpStatus::Redirect,
        400 => HttpStatus::BadRequest,
        401 => HttpStatus::Unauthorized,
        403 => HttpStatus::Forbidden,
        404 => HttpStatus::NotFound,
        409 => HttpStatus::Conflict,
        414 => HttpStatus::UriTooLong,
        415 => HttpStatus::UnsupportedMediaType,
        422 => HttpStatus::Unprocessable,
        429 => HttpStatus::RateLimited,
        500..=599 => HttpStatus::ServerError,
        _ => HttpStatus::OtherClientError,
    }
}

fn configured_client(mode: EndpointMode) -> reqwest::ClientBuilder {
    let builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .http1_only();
    match mode {
        EndpointMode::Production => builder.https_only(true),
        EndpointMode::Loopback => builder
            .https_only(false)
            .dns_resolver(Arc::new(LoopbackResolver)),
    }
}

impl TossHttpClient {
    pub fn new(endpoint: &TossEndpoint) -> Result<Self, TossHttpError> {
        let origin =
            Url::parse(endpoint.origin()).map_err(|_error| TossHttpError::InvalidEndpoint)?;
        let client = configured_client(endpoint.mode())
            .build()
            .map_err(|_error| TossHttpError::Configuration)?;
        Ok(Self { client, origin })
    }

    pub async fn get(
        &self,
        request: &ReadRequest,
        token: &SecretValue,
    ) -> Result<RawResponse, TossHttpError> {
        let url = request.url(&self.origin);
        let authorization_bytes =
            SecretValue::new([b"Bearer ", token.as_exposed_secret()].concat());
        let authorization = sensitive_header(&authorization_bytes)?;
        let builder = self.client.get(url).header(AUTHORIZATION, authorization);
        let authenticated = match request.account() {
            Some(account) => {
                let value = sensitive_header(account)?;
                builder.header("X-Tossinvest-Account", value)
            }
            None => builder,
        };
        receive(
            authenticated
                .send()
                .await
                .map_err(|_error| TossHttpError::Transport)?,
        )
        .await
    }
}

pub(crate) async fn receive(mut response: reqwest::Response) -> Result<RawResponse, TossHttpError> {
    let status = response.status().as_u16();
    let headers = ResponseHeaders {
        retry_after: header(response.headers(), "Retry-After"),
        limit: header(response.headers(), "X-RateLimit-Limit"),
        remaining: header(response.headers(), "X-RateLimit-Remaining"),
        reset: header(response.headers(), "X-RateLimit-Reset"),
    };
    let mut chunks = Vec::new();
    let mut length = 0_usize;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_error| TossHttpError::Transport)?
    {
        if length.saturating_add(chunk.len()) > 8_388_608 {
            return Err(TossHttpError::ResponseTooLarge);
        }
        length = length.saturating_add(chunk.len());
        chunks.push(SecretValue::new(chunk.to_vec()));
    }
    Ok(RawResponse {
        status,
        headers,
        body: SecretValue::new(
            chunks
                .iter()
                .flat_map(|chunk| chunk.as_exposed_secret().iter().copied())
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LoopbackResolver, RawResponse, ResponseHeaders, TossHttpClient, TossHttpError,
        classify_status,
    };
    use crate::endpoint::EndpointMode;
    use crate::{ReadRequest, TossEndpoint};
    use application::{HttpStatus, ReadEndpoint, RetryDelay, SecretValue};
    use reqwest::dns::Resolve;

    #[test]
    fn product_configuration_rejects_plaintext_before_connecting() {
        let probe = testkit::ConnectionProbe::bind().expect("listener");
        let endpoint = TossEndpoint::local(probe.port().expect("port")).expect("local endpoint");
        let client = TossHttpClient {
            client: super::configured_client(EndpointMode::Production)
                .build()
                .expect("client"),
            origin: reqwest::Url::parse(endpoint.origin()).expect("local URL"),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let outcome = runtime
            .block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    client.get(
                        &ReadRequest::new(crate::ReadRoute::Accounts),
                        &SecretValue::new(b"fake".to_vec()),
                    ),
                )
                .await
            })
            .expect("scheme refusal before I/O wait");
        assert_eq!(outcome.expect_err("plaintext"), TossHttpError::Transport);
        assert_eq!(
            probe.observe().expect("listener status"),
            testkit::ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn actual_builder_has_the_documented_timeouts_and_http_version() {
        for mode in [EndpointMode::Production, EndpointMode::Loopback] {
            let config = format!("{:?}", super::configured_client(mode));
            assert!(config.contains("connect_timeout: 5s"), "{config}");
            assert!(config.contains("timeout: 30s"), "{config}");
            assert!(config.contains("http1_only: true"), "{config}");
        }
    }

    #[test]
    fn resolver_only_returns_loopback_for_the_exact_allowed_name() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let addresses: Vec<_> = runtime
            .block_on(LoopbackResolver.resolve("localhost".parse().expect("name")))
            .expect("loopback")
            .collect();
        assert_eq!(addresses, vec!["127.0.0.1:0".parse().expect("address")]);
        assert!(
            runtime
                .block_on(LoopbackResolver.resolve("other.invalid".parse().expect("name")))
                .is_err()
        );
    }

    #[test]
    fn response_status_and_error_mapping_preserve_the_t04_contract() {
        for (code, expected) in [
            (101, HttpStatus::Informational),
            (204, HttpStatus::Success),
            (302, HttpStatus::Redirect),
            (400, HttpStatus::BadRequest),
            (401, HttpStatus::Unauthorized),
            (403, HttpStatus::Forbidden),
            (404, HttpStatus::NotFound),
            (409, HttpStatus::Conflict),
            (414, HttpStatus::UriTooLong),
            (415, HttpStatus::UnsupportedMediaType),
            (422, HttpStatus::Unprocessable),
            (429, HttpStatus::RateLimited),
            (503, HttpStatus::ServerError),
            (418, HttpStatus::OtherClientError),
        ] {
            assert_eq!(classify_status(code), expected);
        }
    }

    #[test]
    fn raw_error_body_preserves_known_code_and_distinct_diagnostic_ids() {
        let response = RawResponse {
            status: 401,
            headers: ResponseHeaders::default(),
            body: SecretValue::new(br#"{"error":{"code":"invalid-token","requestId":"req-731","referenceId":"ref-419"}}"#.to_vec()),
        };
        let mapped = response.read_error(ReadEndpoint::Accounts, RetryDelay::Unknown);
        assert_eq!(
            mapped.port_error().kind(),
            application::PortErrorKind::InvalidRequest(application::RequestRejection::InvalidToken)
        );
        assert_eq!(mapped.code_recognition(), crate::CodeRecognition::Known);
        assert_eq!(
            mapped
                .diagnostics()
                .request_id()
                .expect("request ID")
                .as_str(),
            "req-731"
        );
        assert_eq!(
            mapped
                .diagnostics()
                .reference_id()
                .expect("reference ID")
                .as_str(),
            "ref-419"
        );
    }

    #[test]
    fn raw_oauth_and_malformed_bodies_keep_independent_error_classification() {
        let token = RawResponse {
            status: 401,
            headers: ResponseHeaders::default(),
            body: SecretValue::new(
                br#"{"error":"invalid_client","requestId":"oauth-51"}"#.to_vec(),
            ),
        };
        assert_eq!(
            token
                .token_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
                .port_error()
                .kind(),
            application::PortErrorKind::AccessDenied(application::AccessDenial::InvalidCredentials)
        );
        for bytes in [vec![255], Vec::new()] {
            let malformed = RawResponse {
                status: 401,
                headers: ResponseHeaders::default(),
                body: SecretValue::new(bytes),
            };
            assert_eq!(
                malformed
                    .read_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
                    .port_error()
                    .kind(),
                application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Schema)
            );
            assert_eq!(
                malformed
                    .read_error(ReadEndpoint::Accounts, RetryDelay::Unknown)
                    .code_recognition(),
                crate::CodeRecognition::Absent
            );
        }
        assert_eq!(
            format!("{:?}", ResponseHeaders::default()),
            "ResponseHeaders(<redacted>)"
        );
    }
    #[test]
    fn secret_headers_are_sensitive_and_reject_line_injection() {
        let header = super::sensitive_header(&SecretValue::new(b"fake-header-913".to_vec()))
            .expect("header");
        assert!(header.is_sensitive());
        assert_eq!(format!("{header:?}"), "Sensitive");
        assert_eq!(
            super::sensitive_header(&SecretValue::new(b"bad\r\nvalue".to_vec()))
                .expect_err("invalid"),
            TossHttpError::InvalidCredential
        );
    }
}
