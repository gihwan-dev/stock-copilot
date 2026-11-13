use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(i64);

impl AccountId {
    #[must_use]
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedAccountNumber(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskedAccountNumberError {
    InvalidFormat,
}

impl MaskedAccountNumber {
    pub fn try_new(value: &str) -> Result<Self, MaskedAccountNumberError> {
        if value.len() < 8 || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(MaskedAccountNumberError::InvalidFormat);
        }
        Ok(Self(
            value.chars().skip(value.len().saturating_sub(4)).collect(),
        ))
    }
}

impl fmt::Display for MaskedAccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountId, MaskedAccountNumber, MaskedAccountNumberError};
    use std::collections::BTreeSet;
    use std::hash::{Hash, Hasher};

    #[test]
    fn account_ids_preserve_the_entire_openapi_signed_int64_range() {
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(
                AccountId::new(value).as_i64(),
                value,
                "명시되지 않은 양수 제한을 추가하지 않는다"
            );
        }
    }

    #[test]
    fn account_ids_support_ordered_and_hashed_identity() {
        let first = AccountId::new(1);
        let second = AccountId::new(2);
        assert_ne!(first, second, "서로 다른 계좌 키");
        assert!(first < second, "계좌 키 순서");
        let keys = BTreeSet::from([second, first.clone(), first.clone()]);
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            vec![first.clone(), AccountId::new(2)],
            "키로 중복 제거와 정렬이 가능하다"
        );
        let mut original = std::hash::DefaultHasher::new();
        let mut cloned = std::hash::DefaultHasher::new();
        first.hash(&mut original);
        first.clone().hash(&mut cloned);
        assert_eq!(original.finish(), cloned.finish(), "같은 키의 해시는 같다");
    }

    #[test]
    fn masked_account_numbers_store_only_the_suffix_and_display_it() {
        for (value, suffix) in [
            ("12345678", "5678"),
            ("98765000123", "0123"),
            ("12345678901234567890", "7890"),
            ("999990000", "0000"),
        ] {
            let account = MaskedAccountNumber::try_new(value).expect("지어낸 계좌번호");
            assert_eq!(account.0, suffix, "저장된 값 자체가 뒤 네 자리다");
            assert_eq!(
                format!("{account}"),
                suffix,
                "Display에 뒤 네 자리만 표시한다"
            );
            assert_eq!(
                format!("{account:?}"),
                format!("MaskedAccountNumber({suffix:?})"),
                "Debug에도 접두사가 없다"
            );
            assert_eq!(
                format!("{account:#?}"),
                format!("MaskedAccountNumber(\n    {suffix:?},\n)"),
                "대체 Debug에도 접두사가 없다"
            );
        }
    }

    #[test]
    fn masked_account_numbers_with_the_same_suffix_are_indistinguishable() {
        assert_eq!(
            MaskedAccountNumber::try_new("98765000123"),
            MaskedAccountNumber::try_new("12345000123"),
            "비교로 접두사를 추측할 수 없다"
        );
        assert_ne!(
            MaskedAccountNumber::try_new("12345678"),
            MaskedAccountNumber::try_new("12345679"),
            "접미사는 보존한다"
        );
    }

    #[test]
    fn masked_account_numbers_reject_short_overlong_and_non_numeric_inputs() {
        for value in [
            "",
            "1234",
            "1234567",
            "123456789012345678901",
            "1234567A",
            "１２３４５６７８",
            " 12345678",
            "12345678 ",
            "12345678\n",
            "12345678\0",
        ] {
            assert_eq!(
                MaskedAccountNumber::try_new(value),
                Err(MaskedAccountNumberError::InvalidFormat),
                "임시 길이와 ASCII 숫자 규칙을 지킨다"
            );
        }
    }

    #[test]
    fn masked_account_numbers_reject_hyphenated_notation_without_normalizing_it() {
        assert_eq!(
            MaskedAccountNumber::try_new("123-45-678901"),
            Err(MaskedAccountNumberError::InvalidFormat),
            "하이픈 표기를 정규화하지 않는다"
        );
    }
}
