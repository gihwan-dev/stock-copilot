mod request;
mod response;

use application::SecretValue;
use broker_toss::{EndpointMode, TokenError, TokenManager, TokenRenewal, TossEndpoint};
use domain::AccountId;
use order_execution::{ApprovedOrder, BrokerTrade, OrderOutcome, OrderUnknownCause};
use reqwest::dns::{Name, Resolve, Resolving};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Client, Url};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::request::OrderCreateBody;
use crate::response::{SubmitClassification, classify};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossTradeError {
    InvalidEndpoint,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeTokenFailure {
    Unavailable,
}

pub trait TradeAccessToken: Send + Sync {
    fn access_token(&self) -> impl Future<Output = Result<SecretValue, TradeTokenFailure>> + Send;
}

impl<Store, Time, Renewal> TradeAccessToken for TokenManager<Store, Time, Renewal>
where
    Store: application::SecretStore,
    Time: application::Clock,
    Renewal: TokenRenewal,
{
    async fn access_token(&self) -> Result<SecretValue, TradeTokenFailure> {
        self.token()
            .await
            .map(|lease| SecretValue::new(lease.secret().as_exposed_secret().to_vec()))
            .map_err(|_error: TokenError| TradeTokenFailure::Unavailable)
    }
}

impl<Store, Time, Renewal> TradeAccessToken for broker_toss::TossAccountRead<Store, Time, Renewal>
where
    Store: application::SecretStore,
    Time: application::Clock,
    Renewal: TokenRenewal,
{
    fn access_token(&self) -> impl Future<Output = Result<SecretValue, TradeTokenFailure>> + Send {
        self.tokens().access_token()
    }
}

#[derive(Debug)]
struct RefuseNames;

impl Resolve for RefuseNames {
    fn resolve(&self, _name: Name) -> Resolving {
        Box::pin(async move {
            let error: Box<dyn std::error::Error + Send + Sync> =
                Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            Err(error)
        })
    }
}

pub struct TossBrokerTrade<Tokens> {
    client: Client,
    origin: Url,
    tokens: Arc<Tokens>,
}

impl<Tokens> fmt::Debug for TossBrokerTrade<Tokens> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TossBrokerTrade(<redacted>)")
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
            .dns_resolver(Arc::new(RefuseNames)),
    }
}

impl<Tokens: TradeAccessToken> TossBrokerTrade<Tokens> {
    pub fn new(endpoint: &TossEndpoint, tokens: Arc<Tokens>) -> Result<Self, TossTradeError> {
        let origin =
            Url::parse(endpoint.origin()).map_err(|_error| TossTradeError::InvalidEndpoint)?;
        let client = configured_client(endpoint.mode())
            .build()
            .map_err(|_error| TossTradeError::Configuration)?;
        Ok(Self {
            client,
            origin,
            tokens,
        })
    }

    async fn send(&self, order: &ApprovedOrder) -> SubmitClassification {
        let Ok(token) = self.tokens.access_token().await else {
            return SubmitClassification::NotSentAuthentication;
        };
        let body = OrderCreateBody::from_parts(
            order.intent().symbol(),
            order.intent().terms(),
            order.client_order_id(),
            order.large_order_confirmation(),
        );
        let Ok(payload) = serde_json::to_vec(&body) else {
            return SubmitClassification::NotSentInvalidRequest;
        };
        let Some(authorization) = bearer(&token) else {
            return SubmitClassification::NotSentAuthentication;
        };
        let account = account_header(order.intent().account_id());
        let mut url = self.origin.clone();
        url.set_path("/api/v1/orders");
        let sent = self
            .client
            .post(url)
            .header(AUTHORIZATION, authorization)
            .header("X-Tossinvest-Account", account)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(payload)
            .send()
            .await;
        let Ok(response) = sent else {
            return SubmitClassification::Unknown(OrderUnknownCause::Transport);
        };
        let status = response.status().as_u16();
        let Ok(bytes) = response.bytes().await else {
            return SubmitClassification::Unknown(OrderUnknownCause::Transport);
        };
        classify(status, &bytes, order.client_order_id())
    }
}

fn bearer(token: &SecretValue) -> Option<HeaderValue> {
    let bytes = SecretValue::new([b"Bearer ", token.as_exposed_secret()].concat());
    let mut header = HeaderValue::try_from(bytes.as_exposed_secret()).ok()?;
    header.set_sensitive(true);
    Some(header)
}

fn account_header(account: &AccountId) -> HeaderValue {
    let mut header = HeaderValue::from(account.as_i64());
    header.set_sensitive(true);
    header
}

impl<Tokens: TradeAccessToken> BrokerTrade for TossBrokerTrade<Tokens> {
    async fn submit(&self, order: ApprovedOrder) -> OrderOutcome {
        match self.send(&order).await {
            SubmitClassification::Accepted(broker_order_id) => order.into_accepted(broker_order_id),
            SubmitClassification::Rejected(reason) => order.into_rejected(reason),
            SubmitClassification::NotSentAuthentication => {
                order.into_rejected(order_execution::OrderRejectionReason::AuthenticationRequired)
            }
            SubmitClassification::NotSentInvalidRequest => {
                order.into_rejected(order_execution::OrderRejectionReason::InvalidRequest)
            }
            SubmitClassification::Unknown(cause) => order.into_unknown(cause),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EndpointMode, configured_client};

    #[test]
    fn builder_uses_the_read_client_timeouts_and_http_version() {
        for mode in [EndpointMode::Production, EndpointMode::Loopback] {
            let config = format!("{:?}", configured_client(mode));
            assert!(config.contains("connect_timeout: 5s"), "{config}");
            assert!(config.contains("timeout: 30s"), "{config}");
            assert!(config.contains("http1_only: true"), "{config}");
        }
    }
}
