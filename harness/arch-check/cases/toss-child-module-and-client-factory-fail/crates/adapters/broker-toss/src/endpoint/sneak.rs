impl super::TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: super::EndpointMode::Production } } }
