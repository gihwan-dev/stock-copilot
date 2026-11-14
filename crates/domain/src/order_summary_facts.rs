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
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
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
pub struct AccountDisplayFact {
    account: AccountId,
    suffix: MaskedAccountNumber,
}
impl AccountDisplayFact {
    #[must_use]
    pub fn from_account_lookup_result(account: AccountId, suffix: MaskedAccountNumber) -> Self {
        Self { account, suffix }
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
