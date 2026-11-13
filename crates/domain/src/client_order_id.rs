#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientOrderId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientOrderIdError {
    InvalidFormat,
}

impl ClientOrderId {
    pub fn try_new(value: &str) -> Result<Self, ClientOrderIdError> {
        if value.is_empty()
            || value.len() > 36
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(ClientOrderIdError::InvalidFormat);
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
    use super::{ClientOrderId, ClientOrderIdError};

    #[test]
    fn client_order_ids_accept_spec_alphabet_and_length_boundaries() {
        for value in ["a", "ABCxyz019-_", "12345678-1234-1234-1234-123456789abc"] {
            assert_eq!(
                ClientOrderId::try_new(value)
                    .expect("중복 방지 키")
                    .as_str(),
                value,
                "키를 보존한다"
            );
        }
    }

    #[test]
    fn client_order_ids_reject_invalid_alphabet_empty_and_overlong_values() {
        for value in [
            "",
            "12345678-1234-1234-1234-123456789abcd",
            "A.B",
            "A/B",
            " A",
            "A ",
            "A\n",
            "A\0",
            "주문",
            "Ａ",
        ] {
            assert_eq!(
                ClientOrderId::try_new(value),
                Err(ClientOrderIdError::InvalidFormat),
                "잘못된 중복 방지 키"
            );
        }
    }
}
