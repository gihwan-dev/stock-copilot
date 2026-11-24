mod article_reader;
mod dart_corp_map;
mod dart_disclosures;
mod dart_fundamentals;
mod endpoint;
mod google_news;
mod headers;
mod http;
mod kind_disclosures;
mod news_rss;
mod rate_limiter;
mod registry;
mod rss;
mod us_sources;

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
pub use dart_fundamentals::{
    DartFundamentalsClient, DartReportCode, DartStatementScope, build_dart_fundamentals_url,
    parse_dart_financial_statements,
};
pub use endpoint::SourceEndpoint;
pub use google_news::{
    GOOGLE_NEWS_RETENTION_SECONDS, GoogleNewsDisplayItem, GoogleNewsFeed, GoogleNewsPruneResult,
    GoogleNewsRetentionStatus, build_google_news_search_url, google_news_retention_status,
    prune_expired_google_news,
};
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
pub use us_sources::{
    SecCik, SecCompanyTickerRecord, SecEdgarClient, SecEdgarCurrentAtomFeed,
    SecEdgarSubmissionsFeed, SecFilingEntry, SecTickerCikMap, YahooFinanceFeed,
    build_sec_current_atom_url, build_yahoo_headline_url, parse_company_submissions_json,
    parse_company_tickers_json, parse_sec_current_atom,
};
