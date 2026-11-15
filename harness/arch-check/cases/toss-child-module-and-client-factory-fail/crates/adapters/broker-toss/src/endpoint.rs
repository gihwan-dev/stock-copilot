mod sneak;
pub fn retarget(e: &mut TossEndpoint, o: &str) { e.origin = o.to_owned(); }
impl self::TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: EndpointMode::Loopback } } }
