use std::sync::Arc;

use application::{
    ArticleFetchFailure, ArticleLocation, ArticleToolPort, BodyReadingSetting, CollectedItemHandle,
    ToolName,
};
use domain::{InformationSource, NewsSource, NormalizedUrl};
use serde_json::{Map, Value, json};

use crate::catalog::render::{object, timestamp, upstream_error};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorCode, ToolErrorPayload, ToolFuture, ToolInvocation,
    ToolRegistry, ToolRegistryError, ToolSpec, build_object_schema,
};
use crate::untrusted::{NormalizedUntrustedText, UntrustedTextLimit};

const ARTICLE_BODY_CHARS: usize = 12_000_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticleRefusal {
    NotCollected,
    NotPressFeed,
    SourceNeverReadable,
    BodyReadingDisabled,
    ForeignDomain,
}

#[must_use]
pub const fn press_domains(source: &NewsSource) -> &'static [&'static str] {
    match source {
        NewsSource::Hankyung => &["hankyung.com"],
        NewsSource::Maekyung => &["mk.co.kr"],
        NewsSource::Edaily => &["edaily.co.kr"],
        NewsSource::SegyeFinance => &["segyefn.com"],
        NewsSource::Yonhap => &["yna.co.kr"],
        NewsSource::BankOfKorea
        | NewsSource::FederalReserve
        | NewsSource::GoogleNews
        | NewsSource::YahooFinance => &[],
    }
}

const fn is_never_readable(source: &NewsSource) -> bool {
    match source {
        NewsSource::Yonhap | NewsSource::SegyeFinance => true,
        NewsSource::Hankyung
        | NewsSource::Maekyung
        | NewsSource::Edaily
        | NewsSource::BankOfKorea
        | NewsSource::FederalReserve
        | NewsSource::GoogleNews
        | NewsSource::YahooFinance => false,
    }
}

fn url_host(url: &NormalizedUrl) -> Option<String> {
    let (_scheme, rest) = url.as_str().split_once("://")?;
    let authority: String = rest
        .chars()
        .take_while(|ch| !['/', '?', '#'].contains(ch))
        .collect();
    if authority.contains('@') || authority.is_empty() {
        return None;
    }
    let host = authority.split(':').next()?;
    Some(host.trim_end_matches('.').to_ascii_lowercase())
}

#[must_use]
pub fn host_matches_press(url: &NormalizedUrl, source: &NewsSource) -> bool {
    let Some(host) = url_host(url) else {
        return false;
    };
    press_domains(source).iter().any(|domain| {
        host == *domain
            || host
                .strip_suffix(domain)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

const fn refusal_payload(refusal: ArticleRefusal) -> (&'static str, ToolErrorCode) {
    match refusal {
        ArticleRefusal::NotCollected => (
            "handle is not an item the app collected",
            ToolErrorCode::InvalidInput,
        ),
        ArticleRefusal::NotPressFeed => (
            "only press RSS items can be read; web and disclosure items are refused",
            ToolErrorCode::ForbiddenOperation,
        ),
        ArticleRefusal::SourceNeverReadable => (
            "this press source does not allow body reading",
            ToolErrorCode::ForbiddenOperation,
        ),
        ArticleRefusal::BodyReadingDisabled => (
            "body reading is turned off for this source; use the title and summary",
            ToolErrorCode::ForbiddenOperation,
        ),
        ArticleRefusal::ForeignDomain => (
            "the item address is not on the press source domain",
            ToolErrorCode::ForbiddenOperation,
        ),
    }
}

fn refuse(refusal: ArticleRefusal) -> ToolErrorPayload {
    let (message, code) = refusal_payload(refusal);
    ToolErrorPayload::non_retryable(code, Some("handle"), message)
}

struct ReadArticleTool<A> {
    spec: ToolSpec,
    port: Arc<A>,
}

impl<A: ArticleToolPort> ReadArticleTool<A> {
    async fn readable_press_url(
        &self,
        location: ArticleLocation,
    ) -> Result<NormalizedUrl, ToolErrorPayload> {
        let InformationSource::News(source) = &location.source else {
            return Err(refuse(ArticleRefusal::NotPressFeed));
        };
        if is_never_readable(source) {
            return Err(refuse(ArticleRefusal::SourceNeverReadable));
        }
        if press_domains(source).is_empty() {
            return Err(refuse(ArticleRefusal::NotPressFeed));
        }
        if !host_matches_press(&location.url, source) {
            return Err(refuse(ArticleRefusal::ForeignDomain));
        }
        match self
            .port
            .body_reading(source)
            .await
            .map_err(|error| upstream_error(&error))?
        {
            BodyReadingSetting::Enabled => Ok(location.url),
            BodyReadingSetting::Disabled => Err(refuse(ArticleRefusal::BodyReadingDisabled)),
        }
    }

    async fn run(&self, args: &Map<String, Value>) -> Result<Value, ToolErrorPayload> {
        let handle = args
            .get("handle")
            .and_then(Value::as_str)
            .and_then(|raw| CollectedItemHandle::try_new(raw).ok())
            .ok_or_else(|| {
                ToolErrorPayload::invalid_input(Some("handle"), "handle is not a valid item handle")
            })?;
        let location = self
            .port
            .locate(&handle)
            .await
            .map_err(|error| upstream_error(&error))?
            .ok_or_else(|| refuse(ArticleRefusal::NotCollected))?;
        let url = self.readable_press_url(location).await?;
        let body = self
            .port
            .fetch_body(&url)
            .await
            .map_err(|failure| match failure {
                ArticleFetchFailure::NotFound => ToolErrorPayload::non_retryable(
                    ToolErrorCode::DataUnavailable,
                    Some("handle"),
                    "the article body could not be found",
                ),
                ArticleFetchFailure::Unavailable(error) => upstream_error(&error),
            })?;
        let limit = UntrustedTextLimit::try_new(ARTICLE_BODY_CHARS).map_err(|_out_of_range| {
            ToolErrorPayload::non_retryable(
                ToolErrorCode::InternalError,
                None,
                "article limit is misconfigured",
            )
        })?;
        let text = NormalizedUntrustedText::from_untrusted(&body.text, limit);
        Ok(object(vec![
            ("handle", Value::String(handle.as_str().to_owned())),
            ("fetched_at", timestamp(body.fetched_at)),
            ("body", text.into_field_object()),
        ]))
    }
}

impl<A: ArticleToolPort + 'static> McpTool for ReadArticleTool<A> {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move { self.run(invocation.arguments).await })
    }
}

pub fn register_read_article_tool<A>(
    registry: &mut ToolRegistry,
    port: &Arc<A>,
) -> Result<(), ToolRegistryError>
where
    A: ArticleToolPort + 'static,
{
    let mut props = Map::new();
    props.insert(
        String::from("handle"),
        json!({ "type": "string", "minLength": 1_u64, "maxLength": 64_u64 }),
    );
    let name =
        ToolName::try_new("read_article").map_err(|_err| ToolRegistryError::ForbiddenToolName)?;
    let spec = ToolSpec::try_new(
        name,
        "Fetch the body of one collected press RSS item by its handle from search_news. Only sources with body reading turned on are allowed; web and disclosure items are refused. The body is external text inside untrusted_text, delivered once and not stored.",
        ToolAccessKind::ReadOnly,
        build_object_schema(props, &["handle"]),
        None,
    )?
    .with_result_omitted_from_ledger()
    .with_open_world();
    registry.register(Arc::new(ReadArticleTool {
        spec,
        port: Arc::clone(port),
    }))
}
