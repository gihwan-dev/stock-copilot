#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolError {
    InvalidFormat,
}

impl Symbol {
    pub fn try_new(value: &str) -> Result<Self, SymbolError> {
        if value.is_empty()
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
pub struct Market(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketError {
    Unsupported,
}

impl Market {
    pub fn try_new(value: &str) -> Result<Self, MarketError> {
        if value != "KR" && value != "US" {
            return Err(MarketError::Unsupported);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Market, MarketError, Symbol, SymbolError};

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
    fn markets_accept_only_supported_country_codes() {
        for value in ["KR", "US"] {
            assert_eq!(
                Market::try_new(value).expect("지원 시장").as_str(),
                value,
                "시장 코드를 보존한다"
            );
        }
        for value in ["", "kr", "NYSE", "JP", "US ", "KR\n"] {
            assert_eq!(
                Market::try_new(value),
                Err(MarketError::Unsupported),
                "지원하지 않는 시장"
            );
        }
    }
}
