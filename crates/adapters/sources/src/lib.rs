mod article_reader;
mod dart_corp_map;
mod dart_disclosures;
mod endpoint;
mod headers;
mod http;
mod kind_disclosures;
mod news_rss;
mod rate_limiter;
mod registry;
mod rss;

pub use article_reader::{ArticleReadError, ArticleReader};
pub use dart_corp_map::{
    DartCorpCodeMap, DartCorpMapClient, DartCorpReconciliation, DartCorpRecord,
    DartMatchFailureReason, MatchedDartInstrument, UnmatchedDartInstrument, extract_corp_code_xml,
    parse_corp_code_xml,
};
pub use dart_disclosures::{
    DartAmendmentKind, DartCorpClass, DartCorpCode, DartCorrectionGroupKey, DartCorrectionStatus,
    DartDisclosureClient, DartDisclosureEntry, DartDisclosureFeed, DartDisclosurePage,
    group_dart_amendments, parse_correction_info,
};
pub use endpoint::SourceEndpoint;
pub use headers::{
    BROWSER_USER_AGENT, ConditionalValidators, ContactUserAgent, STANDARD_USER_AGENT,
    ValidatorHeader,
};
pub use http::{
    SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient, SourceHttpError,
    SourcePayload, http_error_to_information, http_status_to_error, outcome_to_attempt,
};
pub use kind_disclosures::{
    CorpCodeCandidate, CorpCodeResolution, CorpNameCodeResolver, DartFallbackCoverage,
    DartRssClient, DomesticDisclosureBatch, DomesticDisclosureCollector, DomesticDisclosureRoute,
    KindCorpMatchFailureReason, KindCorpResolutionReport, KindDisclosureClient,
    KindDisclosureEntry, KindDisclosureFeed, KindFetchWindow, KindMarket, MatchedKindDisclosure,
    ReceiptCorrespondence, UnmatchedKindDisclosure, kind_dart_receipt_correspondence,
    normalize_corp_name, parse_kind_rss,
};
pub use news_rss::{PressMacroSource, RssNewsFeed};
pub use rate_limiter::{SourceWait, TokioSourceWait};
pub use registry::{
    AiReadSettingPolicy, CredentialRequirement, HostAllowance, HttpDataSource, RateLimitBucket,
    SourceRegistry, SourceSpec,
};
pub use rss::{
    ParsedFeedItem, collapse_whitespace, parse_feed, parse_feed_timestamp, strip_html_tags,
};
