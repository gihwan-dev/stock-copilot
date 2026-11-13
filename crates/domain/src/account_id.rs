use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct AccountId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountIdError {
    InvalidFormat,
}

impl AccountId {
    pub fn try_new(value: &str) -> Result<Self, AccountIdError> {
        if value.len() <= 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AccountIdError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_masked(&self) -> String {
        self.0
            .chars()
            .skip(self.0.len().saturating_sub(4))
            .collect()
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_masked())
    }
}

impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_masked())
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountId, AccountIdError};

    #[test]
    fn account_display_and_debug_expose_only_last_four_digits() {
        for (value, suffix) in [
            ("98765000123", "0123"),
            ("00001234567", "4567"),
            ("91234", "1234"),
            ("999990000", "0000"),
        ] {
            let account = AccountId::try_new(value).expect("가상 계좌번호");
            assert_eq!(
                account.as_masked(),
                suffix,
                "표시 접근자는 뒤 네 자리만 반환한다"
            );
            assert_eq!(
                format!("{account}"),
                suffix,
                "Display는 뒤 네 자리만 반환한다"
            );
            assert_eq!(
                format!("{account:?}"),
                suffix,
                "Debug는 뒤 네 자리만 반환한다"
            );
            assert_eq!(
                format!("{account:#?}"),
                suffix,
                "대체 Debug도 뒤 네 자리만 반환한다"
            );
        }
    }

    #[test]
    fn accounts_reject_short_non_numeric_and_untrimmed_values() {
        for value in [
            "",
            "1",
            "1234",
            "123-45678",
            "12345A",
            "１２３４５",
            " 12345",
            "12345 ",
            "12345\n",
            "12345\0",
        ] {
            assert_eq!(
                AccountId::try_new(value),
                Err(AccountIdError::InvalidFormat),
                "잘못된 계좌 형식"
            );
        }
    }

    #[test]
    fn accounts_reject_hyphenated_notation_without_normalizing_it() {
        assert_eq!(
            AccountId::try_new("123-45-678901"),
            Err(AccountIdError::InvalidFormat),
            "하이픈 표기는 숫자로 정규화하지 않고 거부한다"
        );
    }

    #[test]
    fn accounts_with_same_suffix_keep_distinct_identity() {
        assert_ne!(
            AccountId::try_new("98765000123"),
            AccountId::try_new("12345000123"),
            "마스킹 전 식별자는 다르다"
        );
    }
}
