use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::TossHttpError;

const PRODUCTION_ORIGIN: &str = "https://openapi.tossinvest.com";

#[derive(Debug)]
pub struct TossEndpoint {
    origin: String,
    mode: EndpointMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndpointMode {
    Production,
    Loopback,
}

impl TossEndpoint {
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }
    pub(crate) const fn mode(&self) -> EndpointMode {
        self.mode
    }
    #[must_use]
    pub fn production() -> Self {
        Self {
            origin: PRODUCTION_ORIGIN.to_owned(),
            mode: EndpointMode::Production,
        }
    }

    pub fn local(port: u16) -> Result<Self, TossHttpError> {
        if port == 0 {
            return Err(TossHttpError::InvalidEndpoint);
        }
        let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        Ok(Self {
            origin: format!("http://{address}"),
            mode: EndpointMode::Loopback,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EndpointMode, TossEndpoint};

    #[test]
    fn local_ports_are_bounded_by_the_type_and_always_make_loopback() {
        for port in [1, 49153, u16::MAX] {
            let endpoint = TossEndpoint::local(port).expect("endpoint");
            assert_eq!(endpoint.mode, EndpointMode::Loopback);
            assert_eq!(endpoint.origin, format!("http://127.0.0.1:{port}"));
            assert!(format!("{endpoint:?}").contains("Loopback"));
        }
    }
}
