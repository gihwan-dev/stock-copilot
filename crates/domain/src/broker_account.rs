use crate::{
    AccountDisplayFact, AccountId, AccountLookupDisplayRow, MaskedAccountNumber, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAccountTypeCode(String);
impl BrokerAccountTypeCode {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownBrokerAccountKind(BrokerAccountTypeCode);
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerAccountKind {
    Brokerage,
    OverseasDerivatives,
    PensionSavings,
    ReshoringInvestment,
    Unknown(UnknownBrokerAccountKind),
}
impl BrokerAccountKind {
    #[must_use]
    pub fn new(code: BrokerAccountTypeCode) -> Self {
        match code.as_str() {
            "BROKERAGE" => Self::Brokerage,
            "OVERSEAS_DERIVATIVES" => Self::OverseasDerivatives,
            "PENSION_SAVINGS" => Self::PensionSavings,
            "RESHORING_INVESTMENT" => Self::ReshoringInvestment,
            _ => Self::Unknown(UnknownBrokerAccountKind(code)),
        }
    }
    #[must_use]
    pub fn as_code(&self) -> BrokerAccountTypeCode {
        match self {
            Self::Brokerage => BrokerAccountTypeCode::new("BROKERAGE"),
            Self::OverseasDerivatives => BrokerAccountTypeCode::new("OVERSEAS_DERIVATIVES"),
            Self::PensionSavings => BrokerAccountTypeCode::new("PENSION_SAVINGS"),
            Self::ReshoringInvestment => BrokerAccountTypeCode::new("RESHORING_INVESTMENT"),
            Self::Unknown(value) => value.0.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerAccountRow {
    account: AccountId,
    kind: BrokerAccountKind,
    suffix: MaskedAccountNumber,
    observed_at: Timestamp,
}
impl BrokerAccountRow {
    #[must_use]
    pub fn from_broker_account_result(
        account: AccountId,
        kind: BrokerAccountKind,
        suffix: MaskedAccountNumber,
        observed_at: Timestamp,
    ) -> Self {
        Self {
            account,
            kind,
            suffix,
            observed_at,
        }
    }
    #[must_use]
    pub const fn account(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub const fn kind(&self) -> &BrokerAccountKind {
        &self.kind
    }
    #[must_use]
    pub const fn masked_number(&self) -> &MaskedAccountNumber {
        &self.suffix
    }
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
    #[must_use]
    pub fn display_fact(&self) -> AccountDisplayFact {
        AccountDisplayFact::from_account_lookup_result(
            AccountLookupDisplayRow::from_broker_account_result(
                self.account.clone(),
                self.suffix.clone(),
            ),
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerAccountSelection<'a> {
    Selected(&'a BrokerAccountRow),
    NoBrokerage,
    Empty,
}
#[must_use]
pub fn select_brokerage_account(rows: &[BrokerAccountRow]) -> BrokerAccountSelection<'_> {
    if rows.is_empty() {
        return BrokerAccountSelection::Empty;
    }
    match rows
        .iter()
        .find(|row| row.kind == BrokerAccountKind::Brokerage)
    {
        Some(row) => BrokerAccountSelection::Selected(row),
        None => BrokerAccountSelection::NoBrokerage,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, BrokerAccountKind, BrokerAccountRow, BrokerAccountSelection,
        BrokerAccountTypeCode, MaskedAccountNumber, Nanosecond, OffsetSeconds, Timestamp,
        UnixSeconds, select_brokerage_account,
    };
    use proptest::{
        arbitrary::any,
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(123),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn row(code: &str, id: i64) -> BrokerAccountRow {
        BrokerAccountRow::from_broker_account_result(
            AccountId::new(id),
            BrokerAccountKind::new(BrokerAccountTypeCode::new(code)),
            MaskedAccountNumber::try_new("12345678901").expect("번호"),
            time(),
        )
    }
    #[test]
    fn schema_kinds_and_unknown_are_preserved() {
        for (code, expected) in [
            ("BROKERAGE", BrokerAccountKind::Brokerage),
            (
                "OVERSEAS_DERIVATIVES",
                BrokerAccountKind::OverseasDerivatives,
            ),
            ("PENSION_SAVINGS", BrokerAccountKind::PensionSavings),
            (
                "RESHORING_INVESTMENT",
                BrokerAccountKind::ReshoringInvestment,
            ),
        ] {
            let value = BrokerAccountKind::new(BrokerAccountTypeCode::new(code));
            assert_eq!(value, expected);
            assert_eq!(value.as_code().as_str(), code);
        }
        for code in ["FUTURE", "brokerage", "", "새 유형"] {
            let kind = BrokerAccountKind::new(BrokerAccountTypeCode::new(code));
            for known in [
                BrokerAccountKind::Brokerage,
                BrokerAccountKind::OverseasDerivatives,
                BrokerAccountKind::PensionSavings,
                BrokerAccountKind::ReshoringInvestment,
            ] {
                assert_ne!(
                    std::mem::discriminant(&kind),
                    std::mem::discriminant(&known)
                );
            }
            assert_eq!(kind.as_code().as_str(), code);
        }
    }
    #[test]
    fn row_accessors_and_display_are_bound_to_the_row() {
        let value = row("BROKERAGE", 42);
        assert_eq!(value.account(), &AccountId::new(42));
        assert_eq!(value.kind(), &BrokerAccountKind::Brokerage);
        assert_eq!(value.observed_at(), time());
        assert_eq!(value.masked_number().to_string(), "8901");
        let (key, suffix) = value.display_fact().into_parts();
        assert_eq!(key, AccountId::new(42));
        assert_eq!(suffix.to_string(), "8901");
        assert!(!format!("{value:?}").contains("1234567"));
    }
    #[test]
    fn selection_has_distinct_empty_and_no_brokerage_outcomes() {
        assert_eq!(select_brokerage_account(&[]), BrokerAccountSelection::Empty);
        assert_eq!(
            select_brokerage_account(&[row("FUTURE", 1)]),
            BrokerAccountSelection::NoBrokerage
        );
    }
    #[test]
    fn selection_is_deterministic_and_follows_response_order() {
        let strategy = proptest::collection::vec(
            (
                proptest::sample::select(vec![
                    "BROKERAGE",
                    "OVERSEAS_DERIVATIVES",
                    "PENSION_SAVINGS",
                    "RESHORING_INVESTMENT",
                    "UNKNOWN",
                ]),
                any::<i64>(),
            ),
            0..30,
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |input| {
                let rows: Vec<_> = input.iter().map(|(code, id)| row(code, *id)).collect();
                let expected = match input.iter().position(|(code, _)| *code == "BROKERAGE") {
                    Some(index) => BrokerAccountSelection::Selected(rows.get(index).expect("행")),
                    None if input.is_empty() => BrokerAccountSelection::Empty,
                    None => BrokerAccountSelection::NoBrokerage,
                };
                prop_assert_eq!(select_brokerage_account(&rows), expected.clone());
                prop_assert_eq!(select_brokerage_account(&rows), expected);
                Ok(())
            })
            .expect("응답 순서");
    }
}
