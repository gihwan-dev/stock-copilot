#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstrumentId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentIdError {
    InvalidFormat,
}

impl InstrumentId {
    pub fn try_new(value: &str) -> Result<Self, InstrumentIdError> {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(InstrumentIdError::InvalidFormat);
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
    use super::{InstrumentId, InstrumentIdError};

    #[test]
    fn opaque_instrument_ids_preserve_identity_independently_of_symbols() {
        for value in ["instrument-1", "internal_42", "A", "0153R0"] {
            assert_eq!(
                InstrumentId::try_new(value).expect("내부 식별자").as_str(),
                value,
                "식별자를 보존한다"
            );
        }
    }

    #[test]
    fn instrument_ids_reject_empty_whitespace_and_control_characters() {
        for value in [
            "",
            " ",
            " instrument",
            "instrument ",
            "instrument\n",
            "a\0b",
            "a\tb",
            "a\u{200b}b",
            "종목",
        ] {
            assert_eq!(
                InstrumentId::try_new(value),
                Err(InstrumentIdError::InvalidFormat),
                "안전하지 않은 내부 식별자"
            );
        }
    }
}
