#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolError {
    InvalidFormat,
}

impl Symbol {
    pub fn try_new(value: &str) -> Result<Self, SymbolError> {
        if value
            .bytes()
            .next()
            .is_none_or(|byte| !byte.is_ascii_alphanumeric())
            || value.len() > 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
        {
            return Err(SymbolError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Market {
    Kr,
    Us,
    Unknown(UnknownMarketCode),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnknownMarketCode(MarketCode);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarketCode(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketError {
    Unsupported,
}

impl MarketCode {
    pub fn try_new(value: &str) -> Result<Self, MarketError> {
        if value.is_empty()
            || value.len() > 32
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(MarketError::Unsupported);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Market {
    #[must_use]
    pub fn new(value: MarketCode) -> Self {
        match value.as_str() {
            "KR" => Self::Kr,
            "US" => Self::Us,
            _ => Self::Unknown(UnknownMarketCode(value)),
        }
    }

    #[must_use]
    pub fn as_code(&self) -> MarketCode {
        match self {
            Self::Kr => MarketCode(String::from("KR")),
            Self::Us => MarketCode(String::from("US")),
            Self::Unknown(value) => value.0.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Market, MarketCode, MarketError, Symbol, SymbolError};

    #[test]
    fn symbols_accept_alphanumeric_and_ticker_punctuation_without_fixed_length() {
        for value in [
            "0101N0",
            "0153R0",
            "005930",
            "A",
            "aapl",
            "BRK.B",
            "BRK-B",
            "LONG1234567",
            "AAPL261218C00150000",
        ] {
            assert_eq!(
                Symbol::try_new(value).expect("유효한 종목 코드").as_str(),
                value,
                "코드를 보존한다"
            );
        }
    }

    #[test]
    fn symbols_reject_empty_whitespace_controls_unicode_and_other_punctuation() {
        for value in [
            "", " AAPL", "AAPL ", "AA\n", "A\0", "종목", "Ａ", "A_B", "A/B",
        ] {
            assert_eq!(
                Symbol::try_new(value),
                Err(SymbolError::InvalidFormat),
                "잘못된 종목 코드"
            );
        }
    }

    #[test]
    fn symbols_reject_path_segments_and_non_alphanumeric_prefixes() {
        for value in [".", "..", "...", "-", "-.-", ".A", "-A"] {
            assert_eq!(
                Symbol::try_new(value),
                Err(SymbolError::InvalidFormat),
                "경로를 바꾸는 코드는 거부한다"
            );
        }
    }

    #[test]
    fn symbols_enforce_the_length_boundary() {
        let accepted = "A".repeat(32);
        assert_eq!(
            Symbol::try_new(&accepted).expect("32자리").as_str(),
            accepted,
            "32자리는 보존한다"
        );
        assert_eq!(
            Symbol::try_new(&"A".repeat(33)),
            Err(SymbolError::InvalidFormat),
            "33자리는 거부한다"
        );
    }

    #[test]
    fn markets_always_send_known_codes_to_the_known_variants() {
        for (value, expected) in [("KR", Market::Kr), ("US", Market::Us)] {
            let code = MarketCode::try_new(value).expect("알려진 시장 코드");
            assert_eq!(
                Market::new(code),
                expected,
                "알려진 코드는 항상 알려진 변형이다"
            );
            assert_eq!(
                Market::new(expected.as_code()),
                expected,
                "접근자 왕복에서도 표현이 하나다"
            );
        }
    }

    #[test]
    fn market_codes_enforce_the_length_boundary_and_reject_large_inputs() {
        let accepted = "A".repeat(32);
        let code = MarketCode::try_new(&accepted).expect("32자리 외부 코드");
        assert_eq!(
            Market::new(code).as_code().as_str(),
            accepted,
            "32자리는 그대로 보존한다"
        );
        for value in ["A".repeat(33), "A".repeat(10_000_000)] {
            assert_eq!(
                MarketCode::try_new(&value),
                Err(MarketError::Unsupported),
                "상한을 초과하면 거부한다"
            );
        }
    }

    #[test]
    fn markets_use_exhaustive_variants_and_preserve_unknown_country_codes() {
        for (value, expected) in [("KR", Market::Kr), ("US", Market::Us)] {
            let market = Market::new(MarketCode::try_new(value).expect("시장 코드"));
            assert_eq!(market, expected, "알려진 시장은 명시 변형으로 만든다");
            assert_eq!(market.as_code().as_str(), value, "시장 코드를 보존한다");
        }
        for value in ["kr", "NYSE", "JP"] {
            let code = MarketCode::try_new(value).expect("모르는 외부 코드");
            let market = Market::new(code.clone());
            assert_ne!(
                market,
                Market::Kr,
                "모르는 코드를 국내 시장으로 바꾸지 않는다"
            );
            assert_ne!(
                market,
                Market::Us,
                "모르는 코드를 미국 시장으로 바꾸지 않는다"
            );
            assert_eq!(market.as_code(), code, "모르는 코드도 원문을 보존한다");
            assert_eq!(
                market.as_code().as_str(),
                value,
                "모르는 코드도 대소문자를 보존한다"
            );
        }
        for value in ["", "US ", "KR\n", "KR\0", "시장"] {
            assert_eq!(
                MarketCode::try_new(value),
                Err(MarketError::Unsupported),
                "지원하지 않는 시장"
            );
        }
    }
}
