use std::future::Future;

use domain::{DisclosureSource, InformationSource, Symbol, Timestamp, UntrustedText};

use crate::SystemPortError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CollectedItemHandle(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolQueryTextError {
    Empty,
    TooLong,
    InvalidCharacter,
}

impl CollectedItemHandle {
    pub fn try_new(raw: &str) -> Result<Self, ToolQueryTextError> {
        if raw.is_empty() {
            return Err(ToolQueryTextError::Empty);
        }
        if raw.len() > 64_usize {
            return Err(ToolQueryTextError::TooLong);
        }
        if !raw
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ToolQueryTextError::InvalidCharacter);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsKeyword(String);

impl NewsKeyword {
    pub fn try_new(raw: &str) -> Result<Self, ToolQueryTextError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ToolQueryTextError::Empty);
        }
        if trimmed.chars().count() > 40_usize {
            return Err(ToolQueryTextError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ToolQueryTextError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookbackDays(u16);

impl LookbackDays {
    pub const fn try_new(days: u16) -> Result<Self, ToolQueryTextError> {
        if days == 0_u16 || days > 90_u16 {
            return Err(ToolQueryTextError::TooLong);
        }
        Ok(Self(days))
    }

    #[must_use]
    pub const fn as_u16(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedItemQuery {
    pub symbol: Option<Symbol>,
    pub keyword: Option<NewsKeyword>,
    pub lookback: LookbackDays,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsToolLine {
    pub handle: CollectedItemHandle,
    pub source: InformationSource,
    pub published_at: Timestamp,
    pub collected_at: Timestamp,
    pub title: UntrustedText,
    pub summary: Option<UntrustedText>,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureToolLine {
    pub handle: CollectedItemHandle,
    pub source: DisclosureSource,
    pub filed_at: Timestamp,
    pub collected_at: Timestamp,
    pub title: UntrustedText,
    pub symbols: Vec<Symbol>,
}

pub trait NewsToolReads: Send + Sync {
    fn search_news(
        &self,
        query: &CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<NewsToolLine>, SystemPortError>> + Send;

    fn disclosures(
        &self,
        query: &CollectedItemQuery,
    ) -> impl Future<Output = Result<Vec<DisclosureToolLine>, SystemPortError>> + Send;
}
