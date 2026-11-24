use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use application::{
    AiReadPolicy, CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedPosition,
    FeedSource, InformationError, NewsFeed, NewsMetadata, PublicationTime, ResponseMismatch,
    Retention,
};
use domain::{
    CalendarDate, DartReceiptNo, Disclosure, DisclosureKey, DisclosureSource, InstrumentId,
    KindReceiptNo, KnownMarket, MasterInstrumentFacts, Nanosecond, NormalizedUrl, ReceiptNo,
    SpanSeconds, Symbol, TimeSpan, UntrustedText,
};

use crate::{
    DartCorpCodeMap, DartDisclosureClient, HttpDataSource, ParsedFeedItem, SourceFetch,
    SourceFetchOutcome, SourceHttpClient, SourceRegistry, collapse_whitespace,
    http_error_to_information, http_status_to_error, parse_feed,
};

const DEFAULT_RETENTION_SECONDS: u64 = 365_u64 * 86_400_u64;
const MAX_SEEN_RECEIPTS: usize = 512;
const KIND_REGULAR_URL: &str = "https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&repIsuSrtCd=&mktTpCd=0&searchCorpName=&currentPageSize=100";
const KIND_STARTUP_CATCHUP_URL: &str = "https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&repIsuSrtCd=&mktTpCd=0&searchCorpName=&currentPageSize=500";
const DART_TODAY_RSS_URL: &str = "https://dart.fss.or.kr/api/todayRSS.xml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFetchWindow {
    Regular,
    StartupCatchUp,
}

impl KindFetchWindow {
    #[must_use]
    pub const fn feed_url(self) -> &'static str {
        match self {
            Self::Regular => KIND_REGULAR_URL,
            Self::StartupCatchUp => KIND_STARTUP_CATCHUP_URL,
        }
    }

    #[must_use]
    pub const fn page_size(self) -> u16 {
        match self {
            Self::Regular => 100_u16,
            Self::StartupCatchUp => 500_u16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KindMarket {
    Kospi,
    Kosdaq,
    Konex,
    Unknown,
}

impl KindMarket {
    fn from_listing_code(code: &str) -> Self {
        match code.trim() {
            "KOSPI" => Self::Kospi,
            "KOSDAQ" => Self::Kosdaq,
            "KONEX" => Self::Konex,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindDisclosureEntry {
    disclosure: Disclosure,
    receipt_no: KindReceiptNo,
    market: KindMarket,
    corp_name: UntrustedText,
    category: Option<UntrustedText>,
    published_at: PublicationTime,
}

impl KindDisclosureEntry {
    #[must_use]
    pub const fn disclosure(&self) -> &Disclosure {
        &self.disclosure
    }

    #[must_use]
    pub const fn receipt_no(&self) -> &KindReceiptNo {
        &self.receipt_no
    }

    #[must_use]
    pub const fn market(&self) -> KindMarket {
        self.market
    }

    #[must_use]
    pub const fn corp_name(&self) -> &UntrustedText {
        &self.corp_name
    }

    #[must_use]
    pub const fn category(&self) -> Option<&UntrustedText> {
        self.category.as_ref()
    }

    #[must_use]
    pub const fn published_at(&self) -> PublicationTime {
        self.published_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpCodeCandidate {
    pub symbol: Symbol,
    pub instrument_id: Option<InstrumentId>,
    pub market: KindMarket,
    pub official_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpCodeResolution {
    Matched(CorpCodeCandidate),
    Ambiguous(Vec<CorpCodeCandidate>),
    Unmatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindCorpMatchFailureReason {
    Unmatched,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedKindDisclosure {
    pub receipt_no: KindReceiptNo,
    pub corp_name: UntrustedText,
    pub symbol: Symbol,
    pub instrument_id: Option<InstrumentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmatchedKindDisclosure {
    pub receipt_no: KindReceiptNo,
    pub corp_name: UntrustedText,
    pub market: KindMarket,
    pub reason: KindCorpMatchFailureReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCorpResolutionReport {
    pub matched: Vec<MatchedKindDisclosure>,
    pub unmatched: Vec<UnmatchedKindDisclosure>,
}

#[must_use]
pub fn normalize_corp_name(raw: &str) -> String {
    let mut cleaned = collapse_whitespace(raw);
    for token in ["(주)", "㈜", "주식회사", "(유)", "유한회사"] {
        cleaned = cleaned.replace(token, "");
    }
    cleaned
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Default, Clone)]
pub struct CorpNameCodeResolver {
    by_normalized_name: BTreeMap<String, Vec<CorpCodeCandidate>>,
}

impl CorpNameCodeResolver {
    #[must_use]
    pub fn from_master(master: &MasterInstrumentFacts, dart_map: Option<&DartCorpCodeMap>) -> Self {
        let mut resolver = Self::default();
        for instrument in master.instruments() {
            let Ok(quote) = instrument.quote_instrument() else {
                continue;
            };
            if *quote.market() != KnownMarket::Kr {
                continue;
            }
            let row = instrument.row();
            let candidate = CorpCodeCandidate {
                symbol: row.symbol.clone(),
                instrument_id: Some(row.instrument_id.clone()),
                market: KindMarket::from_listing_code(row.listing_market.as_str()),
                official_name: row.name.as_str().to_owned(),
            };
            resolver.insert_candidate(row.name.as_str(), candidate);
        }
        if let Some(map) = dart_map {
            resolver.supplement_with_dart(master, map);
        }
        resolver
    }

    fn supplement_with_dart(&mut self, master: &MasterInstrumentFacts, map: &DartCorpCodeMap) {
        let mut master_by_symbol: BTreeMap<Symbol, (InstrumentId, KindMarket)> = BTreeMap::new();
        for instrument in master.instruments() {
            if let Ok(quote) = instrument.quote_instrument()
                && *quote.market() == KnownMarket::Kr
            {
                let row = instrument.row();
                master_by_symbol.insert(
                    row.symbol.clone(),
                    (
                        row.instrument_id.clone(),
                        KindMarket::from_listing_code(row.listing_market.as_str()),
                    ),
                );
            }
        }
        for (symbol, (instrument_id, market)) in master_by_symbol {
            if let Some(corp_code) = map.corp_code_for_symbol(&symbol)
                && let Some(record) = map.record_for_corp_code(corp_code)
            {
                let dart_name = record.corp_name.as_untrusted_str();
                let candidate = CorpCodeCandidate {
                    symbol,
                    instrument_id: Some(instrument_id),
                    market,
                    official_name: dart_name.to_owned(),
                };
                self.insert_candidate(dart_name, candidate);
            }
        }
    }

    fn insert_candidate(&mut self, name: &str, candidate: CorpCodeCandidate) {
        let key = normalize_corp_name(name);
        if key.is_empty() {
            return;
        }
        let entries = self.by_normalized_name.entry(key).or_default();
        if !entries
            .iter()
            .any(|existing| existing.symbol == candidate.symbol)
        {
            entries.push(candidate);
        }
    }

    #[must_use]
    pub fn resolve(&self, corp_name: &str, market: KindMarket) -> CorpCodeResolution {
        let key = normalize_corp_name(corp_name);
        let Some(candidates) = self.by_normalized_name.get(&key) else {
            return CorpCodeResolution::Unmatched;
        };
        if let [single] = candidates.as_slice() {
            return CorpCodeResolution::Matched(single.clone());
        }
        if market != KindMarket::Unknown {
            let market_filtered: Vec<CorpCodeCandidate> = candidates
                .iter()
                .filter(|item| item.market == market)
                .cloned()
                .collect();
            if let [single] = market_filtered.as_slice() {
                return CorpCodeResolution::Matched(single.clone());
            }
        }
        CorpCodeResolution::Ambiguous(candidates.clone())
    }

    #[must_use]
    pub fn resolve_entries(&self, entries: &[KindDisclosureEntry]) -> KindCorpResolutionReport {
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for entry in entries {
            match self.resolve(entry.corp_name().as_untrusted_str(), entry.market()) {
                CorpCodeResolution::Matched(candidate) => {
                    matched.push(MatchedKindDisclosure {
                        receipt_no: entry.receipt_no().clone(),
                        corp_name: entry.corp_name().clone(),
                        symbol: candidate.symbol,
                        instrument_id: candidate.instrument_id,
                    });
                }
                CorpCodeResolution::Ambiguous(_) => {
                    unmatched.push(UnmatchedKindDisclosure {
                        receipt_no: entry.receipt_no().clone(),
                        corp_name: entry.corp_name().clone(),
                        market: entry.market(),
                        reason: KindCorpMatchFailureReason::Ambiguous,
                    });
                }
                CorpCodeResolution::Unmatched => {
                    unmatched.push(UnmatchedKindDisclosure {
                        receipt_no: entry.receipt_no().clone(),
                        corp_name: entry.corp_name().clone(),
                        market: entry.market(),
                        reason: KindCorpMatchFailureReason::Unmatched,
                    });
                }
            }
        }
        KindCorpResolutionReport { matched, unmatched }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptCorrespondence {
    MatchedExchangeFiling,
    DirectDartFilingDifferent,
    Mismatch,
}

#[must_use]
pub fn kind_dart_receipt_correspondence(
    kind: &KindReceiptNo,
    dart: &DartReceiptNo,
    market: KindMarket,
) -> ReceiptCorrespondence {
    let kind_bytes = kind.as_str().as_bytes();
    let dart_bytes = dart.as_str().as_bytes();
    let same_date = kind_bytes.get(0_usize..8_usize) == dart_bytes.get(0_usize..8_usize);
    let same_tail = kind_bytes.get(9_usize..14_usize) == dart_bytes.get(9_usize..14_usize);
    if !same_date || !same_tail {
        return ReceiptCorrespondence::Mismatch;
    }
    let kind_ninth = kind_bytes.get(8_usize).copied();
    let dart_ninth = dart_bytes.get(8_usize).copied();
    if kind_ninth != Some(b'0') {
        return ReceiptCorrespondence::Mismatch;
    }
    match (dart_ninth, market) {
        (Some(b'0'), _) => ReceiptCorrespondence::DirectDartFilingDifferent,
        (Some(b'8'), KindMarket::Kospi | KindMarket::Unknown)
        | (Some(b'9'), KindMarket::Kosdaq | KindMarket::Unknown) => {
            ReceiptCorrespondence::MatchedExchangeFiling
        }
        _ => ReceiptCorrespondence::Mismatch,
    }
}

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

fn extract_query_param(url: &str, target_key: &str) -> Option<String> {
    let (_head, query_and_frag) = url.split_once('?')?;
    let query = query_and_frag
        .split_once('#')
        .map_or(query_and_frag, |(before, _)| before);
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=')
            && key.eq_ignore_ascii_case(target_key)
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

fn parse_kind_title(raw_title: &str) -> (KindMarket, Option<String>) {
    let trimmed = raw_title.trim();
    let (market, rest) = if let Some(after) = trimmed.strip_prefix("[유]") {
        (KindMarket::Kospi, after.trim_start())
    } else if let Some(after) = trimmed.strip_prefix("[코]") {
        (KindMarket::Kosdaq, after.trim_start())
    } else if let Some(after) = trimmed.strip_prefix("[넥]") {
        (KindMarket::Konex, after.trim_start())
    } else {
        (KindMarket::Unknown, trimmed)
    };
    let inferred_corp = rest
        .split_whitespace()
        .next()
        .filter(|word| !word.is_empty())
        .map(str::to_owned);
    (market, inferred_corp)
}

fn parse_kind_item(item: &ParsedFeedItem) -> Option<KindDisclosureEntry> {
    if item.title.is_empty() || item.link.is_empty() {
        return None;
    }
    let acpt_no = extract_query_param(&item.link, "acptNo")?;
    let receipt_no = KindReceiptNo::try_new(&acpt_no).ok()?;
    let url = NormalizedUrl::try_new(&item.link).ok()?;
    let title = UntrustedText::try_new(&item.title).ok()?;
    let (market, inferred_corp) = parse_kind_title(&item.title);
    let raw_corp = item.author.as_deref().or(inferred_corp.as_deref())?;
    let corp_clean = collapse_whitespace(raw_corp);
    if corp_clean.is_empty() {
        return None;
    }
    let corp_name = UntrustedText::try_new(&corp_clean).ok()?;
    let category = item
        .category
        .as_deref()
        .and_then(|value| UntrustedText::try_new(value).ok());
    let disclosure = Disclosure::new(ReceiptNo::Kind(receipt_no.clone()), url, title, None);
    Some(KindDisclosureEntry {
        disclosure,
        receipt_no,
        market,
        corp_name,
        category,
        published_at: item.published_at,
    })
}

pub fn parse_kind_rss(bytes: &[u8]) -> Result<Vec<KindDisclosureEntry>, InformationError> {
    let items = parse_feed(bytes)?;
    let mut entries = Vec::with_capacity(items.len());
    for item in &items {
        if let Some(entry) = parse_kind_item(item) {
            entries.push(entry);
        }
    }
    if !items.is_empty() && entries.is_empty() {
        return Err(schema_error());
    }
    Ok(entries)
}

#[derive(Debug)]
pub struct KindDisclosureClient {
    client: Arc<SourceHttpClient>,
}

impl KindDisclosureClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>) -> Self {
        Self { client }
    }

    pub async fn fetch_entries(
        &self,
        window: KindFetchWindow,
    ) -> Result<Vec<KindDisclosureEntry>, InformationError> {
        let fetch = SourceFetch::new(HttpDataSource::KindDisclosure, window.feed_url());
        let outcome =
            self.client.fetch(&fetch).await.map_err(|error| {
                http_error_to_information(HttpDataSource::KindDisclosure, error)
            })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(Vec::new()),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(HttpDataSource::KindDisclosure, status));
            }
        };
        parse_kind_rss(payload.body())
    }
}

fn decode_seen_receipts(cursor: &FeedCursor) -> Vec<String> {
    let FeedCursor::Continue(position) = cursor else {
        return Vec::new();
    };
    position
        .as_untrusted_text()
        .as_untrusted_str()
        .lines()
        .filter_map(|line| line.strip_prefix("rcp="))
        .filter(|value| !value.is_empty())
        .take(MAX_SEEN_RECEIPTS)
        .map(str::to_owned)
        .collect()
}

fn encode_seen_receipts(latest: Vec<String>, previous: Vec<String>) -> Option<FeedCursor> {
    let mut merged = Vec::with_capacity(MAX_SEEN_RECEIPTS);
    for code in latest.into_iter().chain(previous) {
        if merged.len() >= MAX_SEEN_RECEIPTS {
            break;
        }
        if !merged.contains(&code) {
            merged.push(code);
        }
    }
    let body = merged
        .into_iter()
        .map(|code| format!("rcp={code}"))
        .collect::<Vec<_>>()
        .join("\n");
    if body.is_empty() {
        return None;
    }
    UntrustedText::try_new(&body)
        .ok()
        .map(|text| FeedCursor::Continue(FeedPosition::new(text)))
}

#[derive(Debug)]
pub struct KindDisclosureFeed {
    kind: KindDisclosureClient,
    window: KindFetchWindow,
}

impl KindDisclosureFeed {
    #[must_use]
    pub const fn new(kind: KindDisclosureClient, window: KindFetchWindow) -> Self {
        Self { kind, window }
    }
}

impl NewsFeed for KindDisclosureFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::Kind)
    }

    fn metadata(&self) -> NewsMetadata {
        let spec = SourceRegistry::spec(HttpDataSource::KindDisclosure);
        let retention = TimeSpan::try_new(
            SpanSeconds::new(DEFAULT_RETENTION_SECONDS),
            Nanosecond::new(0_u32),
        )
        .map_or(Retention::NotStored, Retention::For);
        NewsMetadata::new(
            retention,
            AiReadPolicy::TitleAndSummary,
            spec.conditional_get(),
            spec.user_agent(),
        )
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let previous = decode_seen_receipts(&since);
        let seen: BTreeSet<&str> = previous.iter().map(String::as_str).collect();
        let entries = self.kind.fetch_entries(self.window).await?;
        let collected_at = self.kind.client.now();
        let mut latest_receipts = Vec::new();
        let mut observations = Vec::new();
        for entry in entries {
            let code = entry.receipt_no.as_str().to_owned();
            if !latest_receipts.contains(&code) {
                latest_receipts.push(code.clone());
            }
            if seen.contains(code.as_str()) {
                continue;
            }
            observations.push(CollectedObservation::new(
                CollectedEntry::Disclosure(entry.disclosure),
                entry.published_at,
                collected_at,
            ));
        }
        if observations.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let next_cursor = encode_seen_receipts(latest_receipts, previous).unwrap_or(since);
        Ok(DataAvailability::Available((observations, next_cursor)))
    }
}

fn parse_dart_rss_item(
    item: &ParsedFeedItem,
    collected_at: domain::Timestamp,
) -> Option<CollectedObservation> {
    if item.title.is_empty() || item.link.is_empty() {
        return None;
    }
    let raw_rcp = extract_query_param(&item.link, "rcpNo")?;
    let receipt = DartReceiptNo::try_new(&raw_rcp).ok()?;
    let url = NormalizedUrl::try_new(&item.link).ok()?;
    let title = UntrustedText::try_new(&item.title).ok()?;
    let disclosure = Disclosure::new(ReceiptNo::Dart(receipt), url, title, None);
    Some(CollectedObservation::new(
        CollectedEntry::Disclosure(disclosure),
        item.published_at,
        collected_at,
    ))
}

#[derive(Debug)]
pub struct DartRssClient {
    client: Arc<SourceHttpClient>,
}

impl DartRssClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>) -> Self {
        Self { client }
    }

    pub async fn fetch_today(&self) -> Result<Vec<CollectedObservation>, InformationError> {
        let fetch = SourceFetch::new(HttpDataSource::DartDisclosureRss, DART_TODAY_RSS_URL);
        let outcome =
            self.client.fetch(&fetch).await.map_err(|error| {
                http_error_to_information(HttpDataSource::DartDisclosureRss, error)
            })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(Vec::new()),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(
                    HttpDataSource::DartDisclosureRss,
                    status,
                ));
            }
        };
        let items = parse_feed(payload.body())?;
        let collected_at = self.client.now();
        let mut observations = Vec::with_capacity(items.len());
        for item in &items {
            if let Some(obs) = parse_dart_rss_item(item, collected_at) {
                observations.push(obs);
            }
        }
        if !items.is_empty() && observations.is_empty() {
            return Err(schema_error());
        }
        Ok(observations)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartFallbackCoverage {
    RssOnly,
    ApiOnly,
    RssAndApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomesticDisclosureRoute {
    KindPrimary,
    DartFallback(DartFallbackCoverage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomesticDisclosureBatch {
    pub route: DomesticDisclosureRoute,
    pub observations: Vec<CollectedObservation>,
    pub kind_error: Option<InformationError>,
}

#[derive(Debug)]
pub struct DomesticDisclosureCollector {
    kind: KindDisclosureClient,
    dart_rss: DartRssClient,
    dart_api: Option<DartDisclosureClient>,
}

impl DomesticDisclosureCollector {
    #[must_use]
    pub const fn new(
        kind: KindDisclosureClient,
        dart_rss: DartRssClient,
        dart_api: Option<DartDisclosureClient>,
    ) -> Self {
        Self {
            kind,
            dart_rss,
            dart_api,
        }
    }

    pub async fn collect(
        &self,
        window: KindFetchWindow,
        fallback_date: CalendarDate,
    ) -> Result<DomesticDisclosureBatch, InformationError> {
        match self.kind.fetch_entries(window).await {
            Ok(entries) => {
                let collected_at = self.kind.client.now();
                let observations = entries
                    .into_iter()
                    .map(|entry| {
                        CollectedObservation::new(
                            CollectedEntry::Disclosure(entry.disclosure),
                            entry.published_at,
                            collected_at,
                        )
                    })
                    .collect();
                Ok(DomesticDisclosureBatch {
                    route: DomesticDisclosureRoute::KindPrimary,
                    observations,
                    kind_error: None,
                })
            }
            Err(kind_error) => self.collect_dart_fallback(fallback_date, kind_error).await,
        }
    }

    async fn collect_dart_fallback(
        &self,
        fallback_date: CalendarDate,
        kind_error: InformationError,
    ) -> Result<DomesticDisclosureBatch, InformationError> {
        let mut by_key: BTreeMap<DisclosureKey, CollectedObservation> = BTreeMap::new();
        let mut rss_succeeded = false;
        let mut api_succeeded = false;
        let mut last_error = kind_error;

        match self.dart_rss.fetch_today().await {
            Ok(rss_items) => {
                rss_succeeded = true;
                merge_disclosure_observations(&mut by_key, rss_items);
            }
            Err(err) => {
                last_error = err;
            }
        }

        if let Some(dart_api) = &self.dart_api {
            match dart_api
                .fetch_date_as_feed(fallback_date, fallback_date)
                .await
            {
                Ok(DataAvailability::Available(api_items)) => {
                    api_succeeded = true;
                    merge_disclosure_observations(&mut by_key, api_items);
                }
                Ok(DataAvailability::NoData) => {
                    api_succeeded = true;
                }
                Ok(DataAvailability::NotConfigured) => {}
                Err(err) => {
                    last_error = err;
                }
            }
        }

        let coverage = match (rss_succeeded, api_succeeded) {
            (true, true) => DartFallbackCoverage::RssAndApi,
            (true, false) => DartFallbackCoverage::RssOnly,
            (false, true) => DartFallbackCoverage::ApiOnly,
            (false, false) => return Err(last_error),
        };
        Ok(DomesticDisclosureBatch {
            route: DomesticDisclosureRoute::DartFallback(coverage),
            observations: by_key.into_values().collect(),
            kind_error: Some(kind_error),
        })
    }
}

fn merge_disclosure_observations(
    by_key: &mut BTreeMap<DisclosureKey, CollectedObservation>,
    items: Vec<CollectedObservation>,
) {
    for obs in items {
        if let CollectedEntry::Disclosure(disclosure) = obs.entry() {
            let key = disclosure.key().clone();
            by_key.entry(key).or_insert(obs);
        }
    }
}
