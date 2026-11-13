use super::{ChangeDisposition, ThesisError};
use crate::{Disclosure, DisclosureKey, NewsSource, NormalizedUrl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUrl(NormalizedUrl);
impl EvidenceUrl {
    pub fn try_new(value: NormalizedUrl) -> Result<Self, ThesisError> {
        if value.as_str().len() > 2048 {
            return Err(ThesisError::ReferenceTooLong);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_url(&self) -> &NormalizedUrl {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReference(String);
impl ToolReference {
    pub fn try_new(value: &str) -> Result<Self, ThesisError> {
        if value.len() > 128 {
            return Err(ThesisError::InvalidToolReference);
        }
        let suffix = value
            .strip_prefix("tool:")
            .ok_or(ThesisError::InvalidToolReference)?;
        if suffix.is_empty()
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-#/.".contains(&byte))
        {
            return Err(ThesisError::InvalidToolReference);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRef {
    Web(EvidenceUrl),
    News(NewsSource, EvidenceUrl),
    Disclosure(DisclosureKey, EvidenceUrl),
    Tool(ToolReference),
}
impl EvidenceRef {
    pub fn from_disclosure(item: &Disclosure) -> Result<Self, ThesisError> {
        Ok(Self::Disclosure(
            item.key().clone(),
            EvidenceUrl::try_new(item.collected().url().clone())?,
        ))
    }
    #[must_use]
    pub const fn disclosure_key(&self) -> Option<&DisclosureKey> {
        match self {
            Self::Disclosure(key, _) => Some(key),
            Self::Web(_) | Self::News(_, _) | Self::Tool(_) => None,
        }
    }
    fn url(&self) -> Option<&EvidenceUrl> {
        match self {
            Self::Web(url) | Self::News(_, url) | Self::Disclosure(_, url) => Some(url),
            Self::Tool(_) => None,
        }
    }
    fn compare(&self, other: &Self) -> Result<(), ThesisError> {
        if self == other {
            return Err(ThesisError::DuplicateSource);
        }
        if let (Self::Disclosure(left, _), Self::Disclosure(right, _)) = (self, other)
            && left == right
        {
            return Err(ThesisError::DuplicateSource);
        }
        if let (Some(left), Some(right)) = (self.url(), other.url())
            && left == right
        {
            return Err(ThesisError::ConflictingSource);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence(Box<[EvidenceRef]>);
impl Evidence {
    pub fn try_new(items: Vec<EvidenceRef>) -> Result<Self, ThesisError> {
        if items.is_empty() {
            return Err(ThesisError::EmptyEvidence);
        }
        if items.len() > 8 {
            return Err(ThesisError::TooManySources);
        }
        for (index, item) in items.iter().enumerate() {
            for previous in items.iter().take(index) {
                item.compare(previous)?;
            }
        }
        Ok(Self(items.into_boxed_slice()))
    }
    #[must_use]
    pub fn as_items(&self) -> &[EvidenceRef] {
        &self.0
    }
    pub(super) fn disposition(&self) -> ChangeDisposition {
        if self.0.iter().any(|item| match item {
            EvidenceRef::News(_, _) | EvidenceRef::Disclosure(_, _) => true,
            EvidenceRef::Web(_) | EvidenceRef::Tool(_) => false,
        }) {
            ChangeDisposition::Applied
        } else {
            ChangeDisposition::NeedsReview
        }
    }
}
