use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::TossHttpError;

const PRODUCTION_ORIGIN: &str = "https://openapi.tossinvest.com";

#[derive(Debug)]
pub struct TossEndpoint {
    pub(crate) origin: String,
    pub(crate) mode: EndpointMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndpointMode {
    Production,
    Loopback,
}

impl TossEndpoint {
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
    use super::{EndpointMode, PRODUCTION_ORIGIN, TossEndpoint};

    #[test]
    fn local_ports_are_bounded_by_the_type_and_always_make_loopback() {
        for port in [1, 49153, u16::MAX] {
            let endpoint = TossEndpoint::local(port).expect("endpoint");
            assert_eq!(endpoint.mode, EndpointMode::Loopback);
            assert_eq!(endpoint.origin, format!("http://127.0.0.1:{port}"));
            assert!(format!("{endpoint:?}").contains("Loopback"));
        }
        let url = reqwest::Url::parse(PRODUCTION_ORIGIN).expect("fixed origin");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.path(), "/");
        assert_eq!(url.port(), None);
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        assert_eq!(
            PRODUCTION_ORIGIN,
            spec["servers"][0]["url"].as_str().expect("spec origin")
        );
    }
}
