use crate::{AccountId, ClientOrderId, MaskedAccountNumber, Money, ResolvedOrderInstrument};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayInstrumentName(String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayInstrumentNameError {
    InvalidName,
}
impl DisplayInstrumentName {
    pub fn try_new(value: &str) -> Result<Self, DisplayInstrumentNameError> {
        if value.is_empty()
            || value.len() > 256
            || value.chars().any(|character| match character {
                '\u{00ad}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061c}'
                | '\u{06dd}'
                | '\u{070f}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08e2}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{110bd}'
                | '\u{110cd}'
                | '\u{13430}'..='\u{1343f}'
                | '\u{1bca0}'..='\u{1bca3}'
                | '\u{1d173}'..='\u{1d17a}'
                | '\u{e0001}'
                | '\u{e0020}'..='\u{e007f}'
                | '\u{034f}'
                | '\u{115f}'..='\u{1160}'
                | '\u{17b4}'..='\u{17b5}'
                | '\u{180b}'..='\u{180d}'
                | '\u{180f}'
                | '\u{2028}'..='\u{2029}'
                | '\u{2065}'
                | '\u{3164}'
                | '\u{fe00}'..='\u{fe0f}'
                | '\u{ffa0}'
                | '\u{e0100}'..='\u{e01ef}' => true,
                _ => character.is_control(),
            })
        {
            return Err(DisplayInstrumentNameError::InvalidName);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentNameFact {
    instrument: ResolvedOrderInstrument,
    name: DisplayInstrumentName,
}
impl InstrumentNameFact {
    #[must_use]
    pub fn from_master_lookup_result(
        instrument: ResolvedOrderInstrument,
        name: DisplayInstrumentName,
    ) -> Self {
        Self { instrument, name }
    }
    pub(crate) fn into_parts(self) -> (ResolvedOrderInstrument, DisplayInstrumentName) {
        (self.instrument, self.name)
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct AccountLookupDisplayRow {
    account: AccountId,
    suffix: MaskedAccountNumber,
}
impl AccountLookupDisplayRow {
    #[must_use]
    pub(crate) fn from_broker_account_result(
        account: AccountId,
        suffix: MaskedAccountNumber,
    ) -> Self {
        Self { account, suffix }
    }
}
impl fmt::Debug for AccountLookupDisplayRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountLookupDisplayRow")
            .field("suffix", &self.suffix)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct AccountDisplayFact {
    account: AccountId,
    suffix: MaskedAccountNumber,
}
impl AccountDisplayFact {
    #[must_use]
    pub(crate) fn from_account_lookup_result(row: AccountLookupDisplayRow) -> Self {
        Self {
            account: row.account,
            suffix: row.suffix,
        }
    }
    pub(crate) fn into_parts(self) -> (AccountId, MaskedAccountNumber) {
        (self.account, self.suffix)
    }
}
impl fmt::Debug for AccountDisplayFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountDisplayFact")
            .field("suffix", &self.suffix)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientOrderPrefix(String);
impl ClientOrderPrefix {
    pub(crate) fn from_order_id(id: &ClientOrderId) -> Self {
        Self(id.as_str().chars().take(8).collect())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedOrderFee(pub(crate) ClientOrderId, pub(crate) Money);
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimatedOrderFee {
    Unknown,
    Confirmed(ConfirmedOrderFee),
}
impl ConfirmedOrderFee {
    #[must_use]
    pub fn money(&self) -> Money {
        self.1
    }
}
