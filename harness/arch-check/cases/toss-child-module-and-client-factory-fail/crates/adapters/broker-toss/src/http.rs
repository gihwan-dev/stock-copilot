impl self::TossHttpClient { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } }
