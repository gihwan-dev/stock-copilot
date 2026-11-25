use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use reqwest::Url;
use reqwest::header::USER_AGENT;

use crate::endpoint::EndpointMode;
use crate::http::configured_client;

const MAX_URL_BYTES: usize = 256;
const MAX_BODY_BYTES: usize = 64;
const CLIENT_NAME: &str = "stock-copilot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicIpError {
    InvalidEndpoint,
    Configuration,
    Transport,
    Status,
    RedirectBlocked,
    ResponseTooLarge,
    InvalidAddress,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PublicIpEndpoint {
    url: Url,
    mode: EndpointMode,
}

impl fmt::Debug for PublicIpEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicIpEndpoint")
    }
}

fn plain_host(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        !host.is_empty()
            && !host.ends_with('.')
            && host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
    })
}

impl PublicIpEndpoint {
    pub fn service(raw: &str) -> Result<Self, PublicIpError> {
        if raw.is_empty() || raw.len() > MAX_URL_BYTES || !raw.is_ascii() {
            return Err(PublicIpError::InvalidEndpoint);
        }
        let url = Url::parse(raw).map_err(|_error| PublicIpError::InvalidEndpoint)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !plain_host(&url)
        {
            return Err(PublicIpError::InvalidEndpoint);
        }
        Ok(Self {
            url,
            mode: EndpointMode::Live,
        })
    }

    pub fn local(port: u16, path: &str) -> Result<Self, PublicIpError> {
        if port == 0 || !path.starts_with('/') || path.len() > MAX_URL_BYTES || !path.is_ascii() {
            return Err(PublicIpError::InvalidEndpoint);
        }
        let url = Url::parse(&format!("http://127.0.0.1:{port}{path}"))
            .map_err(|_error| PublicIpError::InvalidEndpoint)?;
        Ok(Self {
            url,
            mode: EndpointMode::Loopback(port),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PublicIpAddress(IpAddr);

impl fmt::Debug for PublicIpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicIpAddress")
    }
}

const fn usable(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            !(v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation())
        }
        IpAddr::V6(v6) => {
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local())
        }
    }
}

impl PublicIpAddress {
    pub fn parse(body: &[u8]) -> Result<Self, PublicIpError> {
        let text = std::str::from_utf8(body).map_err(|_error| PublicIpError::InvalidAddress)?;
        let address =
            IpAddr::from_str(text.trim_ascii()).map_err(|_error| PublicIpError::InvalidAddress)?;
        if usable(address) {
            Ok(Self(address))
        } else {
            Err(PublicIpError::InvalidAddress)
        }
    }

    #[must_use]
    pub const fn as_ip(&self) -> IpAddr {
        self.0
    }

    #[must_use]
    pub fn to_text(self) -> String {
        self.0.to_string()
    }
}

pub struct PublicIpClient {
    client: reqwest::Client,
    url: Url,
}

impl fmt::Debug for PublicIpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicIpClient")
    }
}

impl PublicIpClient {
    pub fn new(endpoint: PublicIpEndpoint) -> Result<Self, PublicIpError> {
        let client = configured_client(endpoint.mode)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_error| PublicIpError::Configuration)?;
        Ok(Self {
            client,
            url: endpoint.url,
        })
    }

    pub async fn lookup(&self) -> Result<PublicIpAddress, PublicIpError> {
        let mut response = self
            .client
            .get(self.url.clone())
            .header(USER_AGENT, CLIENT_NAME)
            .send()
            .await
            .map_err(|_error| PublicIpError::Transport)?;
        let status = response.status().as_u16();
        match status {
            200..=299 => {}
            300..=399 => return Err(PublicIpError::RedirectBlocked),
            _ => return Err(PublicIpError::Status),
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_error| PublicIpError::Transport)?
        {
            if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                return Err(PublicIpError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        PublicIpAddress::parse(&body)
    }
}

#[cfg(test)]
mod tests;
