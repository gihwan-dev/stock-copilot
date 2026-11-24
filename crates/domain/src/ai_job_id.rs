#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AiJobId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobIdError {
    InvalidFormat,
}

impl AiJobId {
    pub fn try_new(value: &str) -> Result<Self, AiJobIdError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(AiJobIdError::InvalidFormat);
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
    use super::{AiJobId, AiJobIdError};

    #[test]
    fn accepts_run_alphabet_up_to_128_bytes() {
        let longest = "a".repeat(128);
        for value in ["a", "chat-1.run_2", longest.as_str()] {
            assert_eq!(
                AiJobId::try_new(value).expect("실행 ID").as_str(),
                value,
                "ID를 그대로 보존"
            );
        }
    }

    #[test]
    fn rejects_empty_long_and_foreign_bytes() {
        let too_long = "a".repeat(129);
        for value in ["", too_long.as_str(), "a b", "a/b", "실행", "a\n"] {
            assert_eq!(
                AiJobId::try_new(value),
                Err(AiJobIdError::InvalidFormat),
                "형식 밖 값 거부"
            );
        }
    }
}
