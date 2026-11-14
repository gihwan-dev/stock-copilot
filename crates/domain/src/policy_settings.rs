use crate::{AllocationInstrument, ExcludedSecurityType};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyText(String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyTextError {
    TooLong,
}
impl PolicyText {
    pub fn try_new(text: &str) -> Result<Self, PolicyTextError> {
        if text.len() > 16_384 {
            return Err(PolicyTextError::TooLong);
        }
        Ok(Self(text.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyPrinciple(PolicyText);
impl PolicyPrinciple {
    #[must_use]
    pub const fn from_missing_contract(description: PolicyText) -> Self {
        Self(description)
    }
    #[must_use]
    pub const fn description(&self) -> &PolicyText {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolicyConcentration {
    Warning,
    Clear,
    #[default]
    NotApplicable,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyHoldingCount(usize);
impl PolicyHoldingCount {
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySecurityFacts(BTreeMap<AllocationInstrument, ExcludedSecurityType>);
impl Default for PolicySecurityFacts {
    fn default() -> Self {
        Self::from_master_lookup_results(BTreeMap::new())
    }
}
impl PolicySecurityFacts {
    pub(crate) fn from_master_lookup_results(
        values: BTreeMap<AllocationInstrument, ExcludedSecurityType>,
    ) -> Self {
        Self(values)
    }
    #[must_use]
    pub fn security_type(&self, instrument: &AllocationInstrument) -> Option<ExcludedSecurityType> {
        self.0.get(instrument).copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::{PolicySecurityFacts, PolicyText, PolicyTextError};
    #[test]
    fn context_preserves_korean_text_and_bounds_without_echoing_errors() {
        assert_eq!(
            PolicyText::try_new("장기 투자").expect("문맥").as_str(),
            "장기 투자"
        );
        assert_eq!(
            PolicyText::try_new(&"가".repeat(6000)),
            Err(PolicyTextError::TooLong)
        );
        assert_eq!(
            PolicyText::try_new(&"a".repeat(16_384))
                .expect("최대")
                .as_str()
                .len(),
            16_384
        );
        assert_eq!(
            PolicySecurityFacts::default(),
            PolicySecurityFacts::from_master_lookup_results(std::collections::BTreeMap::new())
        );
    }
}
