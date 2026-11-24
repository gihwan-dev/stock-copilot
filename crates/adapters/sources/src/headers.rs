use std::fmt;

use application::{ConditionalGet, UserAgentRequirement};
use reqwest::header::HeaderValue;

use crate::{SourceHttpError, SourceSpec};

pub const STANDARD_USER_AGENT: &str = "StockCopilot/0.1 (macOS)";
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone, PartialEq, Eq)]
pub struct ContactUserAgent(String);

impl fmt::Debug for ContactUserAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContactUserAgent(<redacted>)")
    }
}

fn is_valid_email_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|ch| ch == '<' || ch == '>' || ch == '(' || ch == ')');
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }
    let valid_local = local
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._%+-".contains(&byte));
    let valid_domain = domain.split('.').all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    let valid_tld = domain
        .rsplit('.')
        .next()
        .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic()));
    valid_local && valid_domain && valid_tld
}

fn normalize_header_text(value: &str, max_len: usize) -> Result<&str, SourceHttpError> {
    if !value.bytes().all(|byte| (0x20..=0x7E).contains(&byte)) {
        return Err(SourceHttpError::InvalidHeader);
    }
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_len {
        return Err(SourceHttpError::InvalidHeader);
    }
    Ok(trimmed)
}

impl ContactUserAgent {
    pub fn try_new(value: &str) -> Result<Self, SourceHttpError> {
        let trimmed = normalize_header_text(value, 256)?;
        let tokens: Vec<&str> = trimmed.split_ascii_whitespace().collect();
        if tokens.len() < 2 {
            return Err(SourceHttpError::InvalidHeader);
        }
        let email_count = tokens
            .iter()
            .filter(|token| is_valid_email_token(token))
            .count();
        let has_identity_token = tokens.iter().any(|token| {
            !is_valid_email_token(token) && token.bytes().any(|byte| byte.is_ascii_alphabetic())
        });
        if email_count != 1 || !has_identity_token {
            return Err(SourceHttpError::InvalidHeader);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub(crate) fn as_header_value(&self) -> Result<HeaderValue, SourceHttpError> {
        let mut header = HeaderValue::try_from(self.0.as_str())
            .map_err(|_error| SourceHttpError::InvalidHeader)?;
        header.set_sensitive(true);
        Ok(header)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorHeader(String);

impl ValidatorHeader {
    pub fn try_new(value: &str) -> Result<Self, SourceHttpError> {
        let trimmed = normalize_header_text(value, 512)?;
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn as_header_value(&self) -> Result<HeaderValue, SourceHttpError> {
        HeaderValue::try_from(self.0.as_str()).map_err(|_error| SourceHttpError::InvalidHeader)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConditionalValidators {
    etag: Option<ValidatorHeader>,
    last_modified: Option<ValidatorHeader>,
}

impl ConditionalValidators {
    #[must_use]
    pub const fn new(
        etag: Option<ValidatorHeader>,
        last_modified: Option<ValidatorHeader>,
    ) -> Self {
        Self {
            etag,
            last_modified,
        }
    }

    #[must_use]
    pub const fn etag(&self) -> Option<&ValidatorHeader> {
        self.etag.as_ref()
    }

    #[must_use]
    pub const fn last_modified(&self) -> Option<&ValidatorHeader> {
        self.last_modified.as_ref()
    }
}

pub(crate) struct SourceHeaderPlan {
    pub(crate) user_agent: HeaderValue,
    pub(crate) if_none_match: Option<HeaderValue>,
    pub(crate) if_modified_since: Option<HeaderValue>,
}

pub(crate) fn resolve_headers(
    spec: &SourceSpec,
    contact: Option<&ContactUserAgent>,
    validators: Option<&ConditionalValidators>,
) -> Result<SourceHeaderPlan, SourceHttpError> {
    let user_agent = match spec.user_agent() {
        UserAgentRequirement::Standard => HeaderValue::from_static(STANDARD_USER_AGENT),
        UserAgentRequirement::Browser => HeaderValue::from_static(BROWSER_USER_AGENT),
        UserAgentRequirement::Contact => match contact {
            Some(configured) => configured.as_header_value()?,
            None => return Err(SourceHttpError::MissingContactUserAgent),
        },
    };
    let (if_none_match, if_modified_since) = match (spec.conditional_get(), validators) {
        (ConditionalGet::Supported, Some(values)) => (
            match values.etag() {
                Some(etag) => Some(etag.as_header_value()?),
                None => None,
            },
            match values.last_modified() {
                Some(modified) => Some(modified.as_header_value()?),
                None => None,
            },
        ),
        (ConditionalGet::Supported, None) | (ConditionalGet::Unsupported, _) => (None, None),
    };
    Ok(SourceHeaderPlan {
        user_agent,
        if_none_match,
        if_modified_since,
    })
}
