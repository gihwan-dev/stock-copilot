mod digest;
mod normalize;
mod rules;

use std::collections::BTreeSet;
use std::fmt;

use crate::{Amount, MaskedAccountNumber, Quantity};

pub use digest::{WebSearchDigestError, WebSearchQueryDigest};

#[derive(Clone, PartialEq, Eq)]
pub struct WebSearchCandidateQuery(String);

impl WebSearchCandidateQuery {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WebSearchCandidateQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebSearchCandidateQuery(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveContextError {
    InvalidFormat,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SensitivePersonalName(String);

impl SensitivePersonalName {
    pub fn try_new(value: &str) -> Result<Self, SensitiveContextError> {
        if value.chars().any(char::is_control) {
            return Err(SensitiveContextError::InvalidFormat);
        }
        let trimmed = value.trim();
        let alnum_count = trimmed
            .chars()
            .filter(|item| item.is_alphanumeric())
            .count();
        if alnum_count < 2_usize || trimmed.len() > 128_usize {
            return Err(SensitiveContextError::InvalidFormat);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SensitivePersonalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitivePersonalName(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveAccountNumber(String);

impl SensitiveAccountNumber {
    pub fn try_new(value: &str) -> Result<Self, SensitiveContextError> {
        if value.chars().any(char::is_control) {
            return Err(SensitiveContextError::InvalidFormat);
        }
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.len() > 48_usize
            || !trimmed
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-' || byte == b' ')
        {
            return Err(SensitiveContextError::InvalidFormat);
        }
        let digit_count = trimmed.bytes().filter(u8::is_ascii_digit).count();
        if !(4_usize..=32_usize).contains(&digit_count) {
            return Err(SensitiveContextError::InvalidFormat);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SensitiveAccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitiveAccountNumber(<redacted>)")
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct WebSearchSensitiveContext {
    account_numbers: Vec<SensitiveAccountNumber>,
    masked_accounts: Vec<MaskedAccountNumber>,
    holding_quantities: Vec<Quantity>,
    holding_amounts: Vec<Amount>,
    personal_names: Vec<SensitivePersonalName>,
}

impl WebSearchSensitiveContext {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            account_numbers: Vec::new(),
            masked_accounts: Vec::new(),
            holding_quantities: Vec::new(),
            holding_amounts: Vec::new(),
            personal_names: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_account_number(mut self, value: SensitiveAccountNumber) -> Self {
        self.account_numbers.push(value);
        self
    }

    #[must_use]
    pub fn with_masked_account(mut self, value: MaskedAccountNumber) -> Self {
        self.masked_accounts.push(value);
        self
    }

    #[must_use]
    pub fn with_holding_quantity(mut self, value: Quantity) -> Self {
        self.holding_quantities.push(value);
        self
    }

    #[must_use]
    pub fn with_holding_amount(mut self, value: Amount) -> Self {
        self.holding_amounts.push(value);
        self
    }

    #[must_use]
    pub fn with_personal_name(mut self, value: SensitivePersonalName) -> Self {
        self.personal_names.push(value);
        self
    }

    pub(crate) fn account_numbers(&self) -> &[SensitiveAccountNumber] {
        &self.account_numbers
    }

    pub(crate) fn masked_accounts(&self) -> &[MaskedAccountNumber] {
        &self.masked_accounts
    }

    pub(crate) fn holding_quantities(&self) -> &[Quantity] {
        &self.holding_quantities
    }

    pub(crate) fn holding_amounts(&self) -> &[Amount] {
        &self.holding_amounts
    }

    pub(crate) fn personal_names(&self) -> &[SensitivePersonalName] {
        &self.personal_names
    }
}

impl fmt::Debug for WebSearchSensitiveContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSearchSensitiveContext")
            .field("account_numbers", &self.account_numbers.len())
            .field("masked_accounts", &self.masked_accounts.len())
            .field("holding_quantities", &self.holding_quantities.len())
            .field("holding_amounts", &self.holding_amounts.len())
            .field("personal_names", &self.personal_names.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WebSearchRejectionReason {
    EmptyQuery,
    QueryTooLong,
    ControlOrInvisibleCharacter,
    EncodedOrObfuscatedText,
    UrlOrDomainShape,
    MultiDigitNumber,
    SpelledOutNumber,
    MoneyQuantityOrAccountTerm,
    SensitiveContextMatch,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WebSearchQueryText(String);

impl WebSearchQueryText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WebSearchQueryText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebSearchQueryText(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApprovedWebSearchQuery {
    text: WebSearchQueryText,
    digest: WebSearchQueryDigest,
}

impl ApprovedWebSearchQuery {
    #[must_use]
    pub const fn text(&self) -> &WebSearchQueryText {
        &self.text
    }

    #[must_use]
    pub const fn digest(&self) -> &WebSearchQueryDigest {
        &self.digest
    }
}

impl fmt::Debug for ApprovedWebSearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovedWebSearchQuery")
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RejectedWebSearchQuery {
    digest: WebSearchQueryDigest,
    reasons: Vec<WebSearchRejectionReason>,
}

impl RejectedWebSearchQuery {
    #[must_use]
    pub const fn digest(&self) -> &WebSearchQueryDigest {
        &self.digest
    }

    #[must_use]
    pub fn reasons(&self) -> &[WebSearchRejectionReason] {
        &self.reasons
    }
}

impl fmt::Debug for RejectedWebSearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectedWebSearchQuery")
            .field("digest", &self.digest)
            .field("reasons", &self.reasons)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSearchInspection {
    Approved(ApprovedWebSearchQuery),
    Rejected(RejectedWebSearchQuery),
}

#[must_use]
pub fn inspect_web_search_query(
    candidate: &WebSearchCandidateQuery,
    context: &WebSearchSensitiveContext,
) -> WebSearchInspection {
    let query_digest = WebSearchQueryDigest::of_query(candidate);
    let mut reasons = BTreeSet::new();
    let prepared = normalize::prepare_query(candidate, &mut reasons);
    rules::evaluate_rules(&prepared, context, &mut reasons);
    if reasons.is_empty() {
        WebSearchInspection::Approved(ApprovedWebSearchQuery {
            text: WebSearchQueryText(candidate.as_str().to_owned()),
            digest: query_digest,
        })
    } else {
        WebSearchInspection::Rejected(RejectedWebSearchQuery {
            digest: query_digest,
            reasons: reasons.into_iter().collect(),
        })
    }
}
