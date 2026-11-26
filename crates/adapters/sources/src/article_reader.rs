use std::sync::Arc;

use application::{InformationError, ResponseMismatch};
use domain::{
    AiBodyReadPolicy, CollectedItem, InformationSource, NewsSource, SourceAiBodyReadSettings,
    UntrustedText,
};

use crate::{
    AiReadSettingPolicy, HostAllowance, HttpDataSource, SourceFetch, SourceFetchOutcome,
    SourceHttpClient, SourceRegistry, collapse_whitespace, http_error_to_information,
    http_status_to_error, strip_html_tags,
};

const MAX_ARTICLE_BYTES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticleReadError {
    WebSourceDisallowed,
    DisclosureDisallowed,
    SourceLockedOff(NewsSource),
    AiBodyReadDisabled(NewsSource),
    DisallowedArticleHost,
    FetchFailed(InformationError),
}

fn news_to_http_source(source: &NewsSource) -> HttpDataSource {
    match source {
        NewsSource::Hankyung => HttpDataSource::Hankyung,
        NewsSource::Maekyung => HttpDataSource::Maekyung,
        NewsSource::Yonhap => HttpDataSource::Yonhap,
        NewsSource::Edaily => HttpDataSource::Edaily,
        NewsSource::SegyeFinance => HttpDataSource::SegyeFinance,
        NewsSource::BankOfKorea => HttpDataSource::BankOfKorea,
        NewsSource::FederalReserve => HttpDataSource::FederalReserve,
        NewsSource::GoogleNews => HttpDataSource::GoogleNews,
        NewsSource::YahooFinance => HttpDataSource::YahooFinance,
    }
}

fn truncate_utf8_bytes(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0_usize && !text.is_char_boundary(end) {
        end = end.saturating_sub(1_usize);
    }
    text.get(..end).unwrap_or("")
}

fn validate_article_url(
    http_source: HttpDataSource,
    raw_url: &str,
) -> Result<(), ArticleReadError> {
    let Some(rest) = raw_url.strip_prefix("https://") else {
        return Err(ArticleReadError::DisallowedArticleHost);
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let spec = SourceRegistry::spec(http_source);
    if spec.host_allowance(host) == HostAllowance::Allowed {
        Ok(())
    } else {
        Err(ArticleReadError::DisallowedArticleHost)
    }
}

#[derive(Debug)]
pub struct ArticleReader {
    client: Arc<SourceHttpClient>,
}

impl ArticleReader {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>) -> Self {
        Self { client }
    }

    pub async fn read_transient(
        &self,
        item: &CollectedItem,
        settings: &SourceAiBodyReadSettings,
    ) -> Result<UntrustedText, ArticleReadError> {
        let news_source = match item.source() {
            InformationSource::Web => return Err(ArticleReadError::WebSourceDisallowed),
            InformationSource::Disclosure(_) => return Err(ArticleReadError::DisclosureDisallowed),
            InformationSource::News(source) => source,
        };
        let http_source = news_to_http_source(news_source);
        let spec = SourceRegistry::spec(http_source);
        if spec.ai_read_policy() == AiReadSettingPolicy::LockedOff {
            return Err(ArticleReadError::SourceLockedOff(news_source.clone()));
        }
        if settings.policy_for(news_source) != AiBodyReadPolicy::BodyAllowed {
            return Err(ArticleReadError::AiBodyReadDisabled(news_source.clone()));
        }
        let raw_url = item.url().as_str();
        validate_article_url(http_source, raw_url)?;
        let fetch = SourceFetch::new(http_source, raw_url);
        let outcome = self.client.fetch(&fetch).await.map_err(|error| {
            ArticleReadError::FetchFailed(http_error_to_information(http_source, error))
        })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => {
                return Err(ArticleReadError::FetchFailed(
                    InformationError::InvalidResponse(ResponseMismatch::Schema),
                ));
            }
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(ArticleReadError::FetchFailed(http_status_to_error(
                    http_source,
                    status,
                )));
            }
        };
        let text = std::str::from_utf8(payload.body()).map_err(|_error| {
            ArticleReadError::FetchFailed(InformationError::InvalidResponse(
                ResponseMismatch::Schema,
            ))
        })?;
        let cleaned = collapse_whitespace(&strip_html_tags(text));
        let bounded = truncate_utf8_bytes(&cleaned, MAX_ARTICLE_BYTES);
        UntrustedText::try_new(bounded).map_err(|_error| {
            ArticleReadError::FetchFailed(InformationError::InvalidResponse(
                ResponseMismatch::Schema,
            ))
        })
    }
}
