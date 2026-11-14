use crate::{
    AccountDisplayFact, AccountId, AccountLookupDisplayRow, MaskedAccountNumber,
    MaskedAccountNumberError, Timestamp,
};
use std::collections::BTreeSet;

pub struct BrokerAccountNumberInput(String);
impl BrokerAccountNumberInput {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }
}
impl std::fmt::Debug for BrokerAccountNumberInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerAccountNumberInput(..)")
    }
}
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
    pub fn from_broker_account_result(
        account: AccountId,
        kind: BrokerAccountTypeCode,
        number: BrokerAccountNumberInput,
        observed_at: Timestamp,
    ) -> Result<Self, MaskedAccountNumberError> {
        let BrokerAccountNumberInput(raw_number) = number;
        let suffix = MaskedAccountNumber::try_new(&raw_number)?;
        Ok(Self {
            account,
            kind: BrokerAccountKind::new(kind),
            suffix,
            observed_at,
        })
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
pub struct BrokerAccountRows(Vec<BrokerAccountRow>);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerAccountRowsError {
    DuplicateAccount,
}
impl BrokerAccountRows {
    pub fn try_new(rows: Vec<BrokerAccountRow>) -> Result<Self, BrokerAccountRowsError> {
        let mut keys = BTreeSet::new();
        for row in &rows {
            if !keys.insert(row.account.clone()) {
                return Err(BrokerAccountRowsError::DuplicateAccount);
            }
        }
        Ok(Self(rows))
    }
    #[must_use]
    pub fn rows(&self) -> &[BrokerAccountRow] {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerAccountSelection<'a> {
    Selected(&'a BrokerAccountRow),
    NoBrokerage,
    Empty,
}
#[must_use]
pub fn select_brokerage_account(rows: &BrokerAccountRows) -> BrokerAccountSelection<'_> {
    if rows.0.is_empty() {
        return BrokerAccountSelection::Empty;
    }
    match rows
        .0
        .iter()
        .find(|row| row.kind == BrokerAccountKind::Brokerage)
    {
        Some(row) => BrokerAccountSelection::Selected(row),
        None => BrokerAccountSelection::NoBrokerage,
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerAccountKeySelection<'a> {
    Selected(&'a BrokerAccountRow),
    Empty,
    NotFound,
    NotBrokerage,
}
#[must_use]
pub fn select_brokerage_account_by_id<'a>(
    rows: &'a BrokerAccountRows,
    account: &AccountId,
) -> BrokerAccountKeySelection<'a> {
    if rows.0.is_empty() {
        return BrokerAccountKeySelection::Empty;
    }
    match rows.0.iter().find(|row| row.account == *account) {
        Some(row) if row.kind == BrokerAccountKind::Brokerage => {
            BrokerAccountKeySelection::Selected(row)
        }
        Some(_row) => BrokerAccountKeySelection::NotBrokerage,
        None => BrokerAccountKeySelection::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, BrokerAccountKind, BrokerAccountNumberInput, BrokerAccountRow,
        BrokerAccountRows, BrokerAccountSelection, BrokerAccountTypeCode, Nanosecond,
        OffsetSeconds, Timestamp, UnixSeconds, select_brokerage_account,
    };
    use proptest::{
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
            BrokerAccountTypeCode::new(code),
            BrokerAccountNumberInput::new("12345678901".to_owned()),
            time(),
        )
        .expect("계좌 행")
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
        assert!(!format!("{value:?}").contains("12345678901"));
        let (key, suffix) = value.display_fact().into_parts();
        assert_eq!(key, AccountId::new(42));
        assert_eq!(suffix.to_string(), "8901");
        assert!(!format!("{value:?}").contains("1234567"));
    }
    #[test]
    fn selection_has_distinct_empty_and_no_brokerage_outcomes() {
        assert_eq!(
            select_brokerage_account(&BrokerAccountRows::try_new(vec![]).expect("목록")),
            BrokerAccountSelection::Empty
        );
        assert_eq!(
            select_brokerage_account(
                &BrokerAccountRows::try_new(vec![row("FUTURE", 1)]).expect("목록")
            ),
            BrokerAccountSelection::NoBrokerage
        );
    }
    #[test]
    fn selection_is_deterministic_and_follows_response_order() {
        let strategy = proptest::collection::vec(
            proptest::sample::select(vec![
                "BROKERAGE",
                "OVERSEAS_DERIVATIVES",
                "PENSION_SAVINGS",
                "RESHORING_INVESTMENT",
                "UNKNOWN",
            ]),
            0..30,
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |input| {
                let rows: Vec<_> = input
                    .iter()
                    .enumerate()
                    .map(|(index, code)| {
                        row(
                            code,
                            i64::try_from(index)
                                .expect("키 범위")
                                .checked_neg()
                                .expect("내림차순 키"),
                        )
                    })
                    .collect();
                let list = BrokerAccountRows::try_new(rows).expect("목록");
                let expected = match input.iter().position(|code| *code == "BROKERAGE") {
                    Some(index) => {
                        BrokerAccountSelection::Selected(list.rows().get(index).expect("행"))
                    }
                    None if input.is_empty() => BrokerAccountSelection::Empty,
                    None => BrokerAccountSelection::NoBrokerage,
                };
                prop_assert_eq!(select_brokerage_account(&list), expected.clone());
                prop_assert_eq!(select_brokerage_account(&list), expected);
                Ok(())
            })
            .expect("응답 순서");
    }
    #[test]
    fn duplicate_account_keys_are_rejected() {
        assert_eq!(
            BrokerAccountRows::try_new(vec![row("BROKERAGE", 1), row("FUTURE", 1)]),
            Err(crate::BrokerAccountRowsError::DuplicateAccount)
        );
    }
    #[test]
    fn explicit_selection_requires_a_listed_brokerage() {
        let rows =
            BrokerAccountRows::try_new(vec![row("BROKERAGE", 2), row("FUTURE", 3)]).expect("목록");
        assert_eq!(
            crate::select_brokerage_account_by_id(&rows, &AccountId::new(2)),
            crate::BrokerAccountKeySelection::Selected(rows.rows().first().expect("첫 행"))
        );
        assert_eq!(
            crate::select_brokerage_account_by_id(&rows, &AccountId::new(3)),
            crate::BrokerAccountKeySelection::NotBrokerage
        );
        assert_eq!(
            crate::select_brokerage_account_by_id(&rows, &AccountId::new(4)),
            crate::BrokerAccountKeySelection::NotFound
        );
        assert_eq!(
            crate::select_brokerage_account_by_id(
                &BrokerAccountRows::try_new(vec![]).expect("빈 목록"),
                &AccountId::new(2)
            ),
            crate::BrokerAccountKeySelection::Empty
        );
    }
    #[test]
    fn raw_number_is_validated_in_the_row_and_input_debug_is_redacted() {
        assert_eq!(
            format!("{:?}", BrokerAccountNumberInput::new("12345678".to_owned())),
            "BrokerAccountNumberInput(..)"
        );
        for number in ["", "1234", "1234-5678"] {
            assert_eq!(
                BrokerAccountRow::from_broker_account_result(
                    AccountId::new(1),
                    BrokerAccountTypeCode::new("BROKERAGE"),
                    BrokerAccountNumberInput::new(number.to_owned()),
                    time()
                ),
                Err(crate::MaskedAccountNumberError::InvalidFormat)
            );
        }
    }
}
