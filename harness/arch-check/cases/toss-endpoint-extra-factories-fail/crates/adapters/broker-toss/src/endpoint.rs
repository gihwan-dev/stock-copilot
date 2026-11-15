const PRODUCTION_ORIGIN: &str = "https://openapi.tossinvest.com";
pub struct TossEndpoint { origin: String, mode: EndpointMode }
enum EndpointMode { Production, Loopback }
impl TossEndpoint {
    pub fn production() -> Self {
        impl TossEndpoint { pub const MAKE: fn() -> TossEndpoint = || TossEndpoint { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production }; }
        Self { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production }
    }
    pub fn any(origin: &str) -> Self { Self { origin: origin.to_owned(), mode: EndpointMode::Loopback } }
}
