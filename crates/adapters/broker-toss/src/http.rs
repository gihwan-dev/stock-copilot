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
    body: Vec<u8>,
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
    pub fn body(&self) -> &[u8] {
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
        match std::str::from_utf8(&self.body) {
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

impl TossHttpClient {
    pub fn new(endpoint: &TossEndpoint) -> Result<Self, TossHttpError> {
        let origin =
            Url::parse(&endpoint.origin).map_err(|_error| TossHttpError::InvalidEndpoint)?;
        let mut builder = Client::builder()
            .https_only(endpoint.mode == EndpointMode::Production)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .http1_only();
        if endpoint.mode == EndpointMode::Loopback {
            builder = builder.dns_resolver(Arc::new(LoopbackResolver));
        }
        let client = builder
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
        let mut authorization =
            HeaderValue::try_from([b"Bearer ", token.as_exposed_secret()].concat().as_slice())
                .map_err(|_error| TossHttpError::InvalidCredential)?;
        authorization.set_sensitive(true);
        let builder = self.client.get(url).header(AUTHORIZATION, authorization);
        let authenticated = match request.account() {
            Some(account) => {
                let mut value = HeaderValue::try_from(account.as_exposed_secret())
                    .map_err(|_error| TossHttpError::InvalidCredential)?;
                value.set_sensitive(true);
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
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_error| TossHttpError::Transport)?
    {
        if body.len().saturating_add(chunk.len()) > 8_388_608 {
            return Err(TossHttpError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(RawResponse {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LoopbackResolver, RawResponse, ResponseHeaders, TossHttpClient, TossHttpError,
        classify_status,
    };
    use crate::endpoint::EndpointMode;
    use crate::{ReadRequest, TossEndpoint, map_toss_read_error, map_toss_token_error};
    use application::{HttpStatus, ReadEndpoint, RetryDelay, SecretValue};
    use reqwest::dns::Resolve;

    #[test]
    fn product_configuration_rejects_plaintext_before_connecting() {
        let endpoint = TossEndpoint {
            origin: "http://127.0.0.1:1".to_owned(),
            mode: EndpointMode::Production,
        };
        let client = TossHttpClient::new(&endpoint).expect("client");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        assert_eq!(
            runtime
                .block_on(client.get(
                    &ReadRequest::new(crate::ReadRoute::Accounts),
                    &SecretValue::new(b"fake".to_vec())
                ))
                .expect_err("plaintext"),
            TossHttpError::Transport
        );
        assert_eq!(
            TossHttpClient::new(&TossEndpoint {
                origin: "invalid origin".to_owned(),
                mode: EndpointMode::Production
            })
            .expect_err("invalid"),
            TossHttpError::InvalidEndpoint
        );
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
        for body in [
            b"{\"error\":{\"code\":\"invalid-token\"}}".to_vec(),
            b"{\"error\":\"invalid_client\"}".to_vec(),
            vec![255],
            Vec::new(),
        ] {
            let response = RawResponse {
                status: 401,
                headers: ResponseHeaders::default(),
                body,
            };
            let parsed = response.error_body();
            assert_eq!(
                response.read_error(ReadEndpoint::Accounts, RetryDelay::Unknown),
                map_toss_read_error(
                    ReadEndpoint::Accounts,
                    HttpStatus::Unauthorized,
                    parsed.clone(),
                    RetryDelay::Unknown
                )
            );
            assert_eq!(
                response.token_error(ReadEndpoint::Accounts, RetryDelay::Unknown),
                map_toss_token_error(
                    ReadEndpoint::Accounts,
                    HttpStatus::Unauthorized,
                    parsed,
                    RetryDelay::Unknown
                )
            );
        }
        assert_eq!(
            format!("{:?}", ResponseHeaders::default()),
            "ResponseHeaders(<redacted>)"
        );
    }
}
