use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use application::{
    CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedPosition, FeedSource,
    InformationError, NewsFeed, NewsMetadata, PublicationTime, ResponseMismatch, Retention,
};
use domain::{
    CalendarDate, CalendarYear, DayNumber, Disclosure, DisclosureSource, MonthNumber, Nanosecond,
    NewsItem, NewsSource, NormalizedUrl, ReceiptNo, SecEdgarReceiptNo, SpanSeconds, Symbol,
    TimeSpan, Timestamp, UntrustedText,
};
use form_urlencoded::Serializer;
use serde::Deserialize;

use crate::{
    HttpDataSource, ParsedFeedItem, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    SourceHttpError, SourcePayload, SourceRegistry, collapse_whitespace, http_error_to_information,
    http_status_to_error, parse_feed, parse_feed_timestamp,
};

const DEFAULT_YAHOO_RETENTION_SECONDS: u64 = 90_u64 * 86_400_u64;
const DEFAULT_SEC_RETENTION_SECONDS: u64 = 365_u64 * 86_400_u64;
const MAX_YAHOO_SYMBOLS: usize = 50;
const MAX_CURSOR_SEEN_ITEMS: usize = 128;
const SEC_TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

#[must_use]
pub fn build_yahoo_headline_url(symbols: &[Symbol]) -> Option<String> {
    let mut unique = Vec::new();
    for symbol in symbols {
        if unique.len() >= MAX_YAHOO_SYMBOLS {
            break;
        }
        let code = symbol.as_str().to_ascii_uppercase();
        if !unique.contains(&code) {
            unique.push(code);
        }
    }
    if unique.is_empty() {
        return None;
    }
    let joined = unique.join(",");
    let query = Serializer::new(String::new())
        .append_pair("s", &joined)
        .append_pair("region", "US")
        .append_pair("lang", "en-US")
        .finish();
    Some(format!(
        "https://feeds.finance.yahoo.com/rss/2.0/headline?{query}"
    ))
}

fn decode_cursor_keys(cursor: &FeedCursor, prefix: &str) -> Vec<String> {
    let FeedCursor::Continue(position) = cursor else {
        return Vec::new();
    };
    position
        .as_untrusted_text()
        .as_untrusted_str()
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
        .take(MAX_CURSOR_SEEN_ITEMS)
        .map(str::to_owned)
        .collect()
}

fn encode_cursor_keys(
    prefix: &str,
    latest: Vec<String>,
    previous: Vec<String>,
) -> Option<FeedCursor> {
    let mut merged = Vec::with_capacity(MAX_CURSOR_SEEN_ITEMS);
    for key in latest.into_iter().chain(previous) {
        if merged.len() >= MAX_CURSOR_SEEN_ITEMS {
            break;
        }
        if !merged.contains(&key) {
            merged.push(key);
        }
    }
    let text = merged
        .into_iter()
        .map(|key| format!("{prefix}{key}"))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return None;
    }
    UntrustedText::try_new(&text)
        .ok()
        .map(|value| FeedCursor::Continue(FeedPosition::new(value)))
}

fn parse_yahoo_item(
    item: &ParsedFeedItem,
    collected_at: Timestamp,
) -> Option<CollectedObservation> {
    if item.title.is_empty() || item.link.is_empty() {
        return None;
    }
    let link = NormalizedUrl::try_new(&item.link).ok()?;
    let title = UntrustedText::try_new(&item.title).ok()?;
    let summary = item
        .summary
        .as_deref()
        .and_then(|text| UntrustedText::try_new(text).ok());
    let news = NewsItem::new(NewsSource::YahooFinance, link, title, summary);
    Some(CollectedObservation::new(
        CollectedEntry::News(news),
        item.published_at,
        collected_at,
    ))
}

#[derive(Debug)]
pub struct YahooFinanceFeed {
    client: Arc<SourceHttpClient>,
    symbols: Vec<Symbol>,
}

impl YahooFinanceFeed {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>, symbols: Vec<Symbol>) -> Self {
        Self { client, symbols }
    }

    #[must_use]
    pub fn feed_url(&self) -> Option<String> {
        build_yahoo_headline_url(&self.symbols)
    }
}

impl NewsFeed for YahooFinanceFeed {
    fn source(&self) -> FeedSource {
        FeedSource::News(NewsSource::YahooFinance)
    }

    fn metadata(&self) -> NewsMetadata {
        let spec = SourceRegistry::spec(HttpDataSource::YahooFinance);
        let retention = TimeSpan::try_new(
            SpanSeconds::new(DEFAULT_YAHOO_RETENTION_SECONDS),
            Nanosecond::new(0_u32),
        )
        .map_or(Retention::NotStored, Retention::For);
        NewsMetadata::new(
            retention,
            spec.default_ai_read(),
            spec.conditional_get(),
            spec.user_agent(),
        )
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let Some(url) = self.feed_url() else {
            return Ok(DataAvailability::NoData);
        };
        let fetch = SourceFetch::new(HttpDataSource::YahooFinance, &url);
        let outcome = self
            .client
            .fetch(&fetch)
            .await
            .map_err(|error| http_error_to_information(HttpDataSource::YahooFinance, error))?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(DataAvailability::NoData),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(HttpDataSource::YahooFinance, status));
            }
        };
        let parsed = parse_feed(payload.body())?;
        let collected_at = self.client.now();
        let previous = decode_cursor_keys(&since, "url=");
        let seen: BTreeSet<&str> = previous.iter().map(String::as_str).collect();
        let mut latest_urls = Vec::new();
        let mut observations = Vec::new();
        let mut valid_count = 0_usize;
        for item in &parsed {
            let Some(obs) = parse_yahoo_item(item, collected_at) else {
                continue;
            };
            valid_count = valid_count.saturating_add(1_usize);
            let CollectedEntry::News(news) = obs.entry() else {
                continue;
            };
            let key = news.collected().url().as_str().to_owned();
            if !latest_urls.contains(&key) {
                latest_urls.push(key.clone());
            }
            if !seen.contains(key.as_str()) {
                observations.push(obs);
            }
        }
        if !parsed.is_empty() && valid_count == 0_usize {
            return Err(schema_error());
        }
        if observations.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let next_cursor = encode_cursor_keys("url=", latest_urls, previous).unwrap_or(since);
        Ok(DataAvailability::Available((observations, next_cursor)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecCik(String);

impl SecCik {
    pub fn try_new(raw: &str) -> Result<Self, InformationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.len() > 10_usize
            || !trimmed.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(schema_error());
        }
        Ok(Self(format!("{trimmed:0>10}")))
    }

    pub fn from_u64(value: u64) -> Result<Self, InformationError> {
        if value > 9_999_999_999_u64 {
            return Err(schema_error());
        }
        Ok(Self(format!("{value:010}")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn unpadded(&self) -> &str {
        let stripped = self.0.trim_start_matches('0');
        if stripped.is_empty() { "0" } else { stripped }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecCompanyTickerRecord {
    pub cik: SecCik,
    pub ticker: Symbol,
    pub title: UntrustedText,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecTickerCikMap {
    by_ticker: BTreeMap<String, SecCompanyTickerRecord>,
}

impl SecTickerCikMap {
    #[must_use]
    pub fn from_records(records: Vec<SecCompanyTickerRecord>) -> Self {
        let mut by_ticker = BTreeMap::new();
        for record in records {
            let key = normalize_us_ticker(record.ticker.as_str());
            by_ticker.entry(key).or_insert(record);
        }
        Self { by_ticker }
    }

    #[must_use]
    pub fn cik_for_symbol(&self, symbol: &Symbol) -> Option<&SecCik> {
        let key = normalize_us_ticker(symbol.as_str());
        self.by_ticker.get(&key).map(|record| &record.cik)
    }

    #[must_use]
    pub fn record_for_symbol(&self, symbol: &Symbol) -> Option<&SecCompanyTickerRecord> {
        let key = normalize_us_ticker(symbol.as_str());
        self.by_ticker.get(&key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_ticker.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_ticker.is_empty()
    }
}

fn normalize_us_ticker(raw: &str) -> String {
    raw.trim().to_ascii_uppercase().replace('.', "-")
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawCikValue {
    Number(u64),
    Text(String),
}

#[derive(Deserialize)]
struct RawTickerRow {
    cik_str: RawCikValue,
    ticker: String,
    title: String,
}

pub fn parse_company_tickers_json(body: &[u8]) -> Result<SecTickerCikMap, InformationError> {
    let raw: BTreeMap<String, RawTickerRow> =
        serde_json::from_slice(body).map_err(|_error| schema_error())?;
    let mut records = Vec::with_capacity(raw.len());
    for row in raw.into_values() {
        let parsed_cik = match row.cik_str {
            RawCikValue::Number(value) => SecCik::from_u64(value),
            RawCikValue::Text(text) => SecCik::try_new(&text),
        };
        let Ok(cik) = parsed_cik else {
            continue;
        };
        let Ok(ticker) = Symbol::try_new(row.ticker.trim()) else {
            continue;
        };
        let cleaned_title = collapse_whitespace(&row.title);
        let Ok(title) = UntrustedText::try_new(&cleaned_title) else {
            continue;
        };
        records.push(SecCompanyTickerRecord { cik, ticker, title });
    }
    if !records.is_empty() {
        return Ok(SecTickerCikMap::from_records(records));
    }
    Err(schema_error())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecFilingEntry {
    pub cik: SecCik,
    pub accession_no: SecEdgarReceiptNo,
    pub form: UntrustedText,
    pub filing_date: Option<CalendarDate>,
    pub report_date: Option<CalendarDate>,
    pub primary_document: Option<String>,
    pub title: UntrustedText,
    pub url: NormalizedUrl,
    pub published_at: PublicationTime,
}

impl SecFilingEntry {
    #[must_use]
    pub fn to_disclosure(&self) -> Disclosure {
        Disclosure::new(
            ReceiptNo::SecEdgar(self.accession_no.clone()),
            self.url.clone(),
            self.title.clone(),
            None,
        )
    }

    #[must_use]
    pub fn to_observation(&self, collected_at: Timestamp) -> CollectedObservation {
        CollectedObservation::new(
            CollectedEntry::Disclosure(self.to_disclosure()),
            self.published_at,
            collected_at,
        )
    }
}

fn parse_iso_date(raw: &str) -> Option<CalendarDate> {
    let trimmed = raw.trim();
    let mut parts = trimmed.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    )
    .ok()
}

fn is_safe_document_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && !name.contains('\\')
        && !name.starts_with('/')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
}

fn build_sec_filing_url(
    cik: &SecCik,
    accession_no: &SecEdgarReceiptNo,
    primary_doc: Option<&str>,
) -> Option<NormalizedUrl> {
    let acc_compact = accession_no.as_str().replace('-', "");
    let cik_num = cik.unpadded();
    let path_doc = primary_doc
        .map(str::trim)
        .filter(|doc| is_safe_document_name(doc));
    let url = match path_doc {
        Some(doc) => {
            format!("https://www.sec.gov/Archives/edgar/data/{cik_num}/{acc_compact}/{doc}")
        }
        None => format!(
            "https://www.sec.gov/Archives/edgar/data/{cik_num}/{acc_compact}/{}-index.htm",
            accession_no.as_str()
        ),
    };
    NormalizedUrl::try_new(&url).ok()
}

#[derive(Deserialize)]
struct RawSubmissionsRoot {
    cik: RawCikValue,
    name: Option<String>,
    filings: RawFilingsBlock,
}

#[derive(Deserialize)]
struct RawFilingsBlock {
    recent: RawRecentFilings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRecentFilings {
    accession_number: Vec<String>,
    form: Vec<String>,
    #[serde(default)]
    filing_date: Vec<String>,
    #[serde(default)]
    report_date: Vec<String>,
    #[serde(default)]
    acceptance_date_time: Vec<String>,
    #[serde(default)]
    primary_document: Vec<String>,
    #[serde(default)]
    primary_doc_description: Vec<String>,
}

fn build_submission_entry(
    cik: &SecCik,
    company_name: Option<&str>,
    recent: &RawRecentFilings,
    idx: usize,
) -> Option<SecFilingEntry> {
    let acc_raw = recent.accession_number.get(idx)?.trim();
    let accession_no = SecEdgarReceiptNo::try_new(acc_raw).ok()?;
    let form_raw = collapse_whitespace(recent.form.get(idx)?);
    if form_raw.is_empty() {
        return None;
    }
    let form = UntrustedText::try_new(&form_raw).ok()?;
    let filing_date = recent
        .filing_date
        .get(idx)
        .and_then(|raw| parse_iso_date(raw));
    let report_date = recent
        .report_date
        .get(idx)
        .and_then(|raw| parse_iso_date(raw));
    let published_at = recent
        .acceptance_date_time
        .get(idx)
        .map_or(PublicationTime::NotProvided, |raw| {
            parse_feed_timestamp(raw.trim())
        });
    let primary_doc = recent
        .primary_document
        .get(idx)
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty());
    let description = recent
        .primary_doc_description
        .get(idx)
        .map(|raw| collapse_whitespace(raw))
        .filter(|raw| !raw.is_empty());
    let url = build_sec_filing_url(cik, &accession_no, primary_doc.as_deref())?;
    let title_text = match (company_name, description.as_deref()) {
        (Some(name), Some(desc)) => format!("{form_raw} - {name} ({desc})"),
        (Some(name), None) => format!("{form_raw} - {name}"),
        (None, Some(desc)) => format!("{form_raw} - {desc}"),
        (None, None) => form_raw,
    };
    let title = UntrustedText::try_new(&title_text).ok()?;
    Some(SecFilingEntry {
        cik: cik.clone(),
        accession_no,
        form,
        filing_date,
        report_date,
        primary_document: primary_doc,
        title,
        url,
        published_at,
    })
}

pub fn parse_company_submissions_json(
    body: &[u8],
) -> Result<Vec<SecFilingEntry>, InformationError> {
    let root: RawSubmissionsRoot = serde_json::from_slice(body).map_err(|_error| schema_error())?;
    let cik = match root.cik {
        RawCikValue::Number(value) => SecCik::from_u64(value)?,
        RawCikValue::Text(text) => SecCik::try_new(&text)?,
    };
    let company_name = root
        .name
        .as_deref()
        .map(collapse_whitespace)
        .filter(|text| !text.is_empty());
    let recent = &root.filings.recent;
    let total = recent.accession_number.len().min(recent.form.len());
    let mut entries = Vec::with_capacity(total);
    for idx in 0..total {
        if let Some(entry) = build_submission_entry(&cik, company_name.as_deref(), recent, idx) {
            entries.push(entry);
        }
    }
    if total > 0_usize && entries.is_empty() {
        return Err(schema_error());
    }
    Ok(entries)
}

fn extract_accession_from_text(text: &str) -> Option<SecEdgarReceiptNo> {
    let bytes = text.as_bytes();
    if bytes.len() < 20_usize {
        return None;
    }
    let last_start = bytes.len().checked_sub(20_usize)?;
    for start in 0..=last_start {
        let end = start.checked_add(20_usize)?;
        let Some(candidate) = text.get(start..end) else {
            continue;
        };
        if let Ok(receipt) = SecEdgarReceiptNo::try_new(candidate) {
            return Some(receipt);
        }
    }
    None
}

fn extract_cik_from_link(link: &str) -> Option<SecCik> {
    let marker = "/Archives/edgar/data/";
    let idx = link.find(marker)?;
    let rest = link.get(idx.checked_add(marker.len())?..)?;
    let segment = rest.split('/').next()?;
    SecCik::try_new(segment).ok()
}

fn parse_atom_entry(item: &ParsedFeedItem) -> Option<SecFilingEntry> {
    let accession_no = item
        .guid
        .as_deref()
        .and_then(extract_accession_from_text)
        .or_else(|| extract_accession_from_text(&item.link))?;
    let cik = extract_cik_from_link(&item.link).or_else(|| {
        accession_no
            .as_str()
            .get(0..10_usize)
            .and_then(|head| SecCik::try_new(head).ok())
    })?;
    let url = NormalizedUrl::try_new(&item.link)
        .ok()
        .or_else(|| build_sec_filing_url(&cik, &accession_no, None))?;
    let form_text = item
        .category
        .as_deref()
        .map(collapse_whitespace)
        .filter(|text| !text.is_empty())
        .or_else(|| {
            item.title
                .split(" - ")
                .next()
                .map(collapse_whitespace)
                .filter(|text| !text.is_empty())
        })?;
    let form = UntrustedText::try_new(&form_text).ok()?;
    let title = UntrustedText::try_new(&item.title).ok()?;
    Some(SecFilingEntry {
        cik,
        accession_no,
        form,
        filing_date: None,
        report_date: None,
        primary_document: None,
        title,
        url,
        published_at: item.published_at,
    })
}

pub fn parse_sec_current_atom(body: &[u8]) -> Result<Vec<SecFilingEntry>, InformationError> {
    let parsed = parse_feed(body)?;
    let mut entries = Vec::with_capacity(parsed.len());
    for item in &parsed {
        if let Some(entry) = parse_atom_entry(item) {
            entries.push(entry);
        }
    }
    if !parsed.is_empty() && entries.is_empty() {
        return Err(schema_error());
    }
    Ok(entries)
}

pub fn build_sec_current_atom_url(
    cik: Option<&SecCik>,
    form_type: Option<&str>,
) -> Result<String, InformationError> {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("action", "getcurrent");
    if let Some(target_cik) = cik {
        serializer.append_pair("CIK", target_cik.as_str());
    }
    if let Some(form) = form_type {
        let trimmed = form.trim();
        if trimmed.is_empty()
            || trimmed.len() > 24_usize
            || !trimmed
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-/ ".contains(&byte))
        {
            return Err(schema_error());
        }
        serializer.append_pair("type", trimmed);
    }
    serializer
        .append_pair("owner", "include")
        .append_pair("count", "40")
        .append_pair("output", "atom");
    let query = serializer.finish();
    Ok(format!("https://www.sec.gov/cgi-bin/browse-edgar?{query}"))
}

#[derive(Debug, Clone)]
pub struct SecEdgarClient {
    client: Arc<SourceHttpClient>,
}

impl SecEdgarClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>) -> Self {
        Self { client }
    }

    async fn fetch_sec_payload(
        &self,
        url: &str,
    ) -> Result<DataAvailability<SourcePayload>, InformationError> {
        let fetch = SourceFetch::new(HttpDataSource::SecEdgar, url);
        let outcome = match self.client.fetch(&fetch).await {
            Ok(outcome) => outcome,
            Err(SourceHttpError::MissingContactUserAgent) => {
                return Ok(DataAvailability::NotConfigured);
            }
            Err(other) => return Err(http_error_to_information(HttpDataSource::SecEdgar, other)),
        };
        match outcome {
            SourceFetchOutcome::Modified(payload) => Ok(DataAvailability::Available(payload)),
            SourceFetchOutcome::NotModified => Ok(DataAvailability::NoData),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                Err(http_status_to_error(HttpDataSource::SecEdgar, status))
            }
        }
    }

    pub async fn fetch_ticker_cik_map(
        &self,
    ) -> Result<DataAvailability<SecTickerCikMap>, InformationError> {
        let payload = match self.fetch_sec_payload(SEC_TICKERS_URL).await? {
            DataAvailability::Available(data) => data,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        let map = parse_company_tickers_json(payload.body())?;
        Ok(DataAvailability::Available(map))
    }

    pub async fn fetch_company_submissions(
        &self,
        cik: &SecCik,
    ) -> Result<DataAvailability<Vec<SecFilingEntry>>, InformationError> {
        let url = format!("https://data.sec.gov/submissions/CIK{}.json", cik.as_str());
        let payload = match self.fetch_sec_payload(&url).await? {
            DataAvailability::Available(data) => data,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        let entries = parse_company_submissions_json(payload.body())?;
        if entries.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        Ok(DataAvailability::Available(entries))
    }

    pub async fn fetch_current_atom(
        &self,
        cik: Option<&SecCik>,
        form_type: Option<&str>,
    ) -> Result<DataAvailability<Vec<SecFilingEntry>>, InformationError> {
        let url = build_sec_current_atom_url(cik, form_type)?;
        let payload = match self.fetch_sec_payload(&url).await? {
            DataAvailability::Available(data) => data,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        let entries = parse_sec_current_atom(payload.body())?;
        if entries.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        Ok(DataAvailability::Available(entries))
    }

    #[must_use]
    pub fn now(&self) -> Timestamp {
        self.client.now()
    }
}

pub(crate) fn sec_feed_metadata() -> NewsMetadata {
    let spec = SourceRegistry::spec(HttpDataSource::SecEdgar);
    let retention = TimeSpan::try_new(
        SpanSeconds::new(DEFAULT_SEC_RETENTION_SECONDS),
        Nanosecond::new(0_u32),
    )
    .map_or(Retention::NotStored, Retention::For);
    NewsMetadata::new(
        retention,
        spec.default_ai_read(),
        spec.conditional_get(),
        spec.user_agent(),
    )
}

fn collect_new_sec_observations(
    entries: &[SecFilingEntry],
    since: FeedCursor,
    collected_at: Timestamp,
) -> DataAvailability<(Vec<CollectedObservation>, FeedCursor)> {
    let previous = decode_cursor_keys(&since, "acc=");
    let seen: BTreeSet<&str> = previous.iter().map(String::as_str).collect();
    let mut latest = Vec::new();
    let mut observations = Vec::new();
    for entry in entries {
        let key = entry.accession_no.as_str().to_owned();
        if !latest.contains(&key) {
            latest.push(key.clone());
        }
        if !seen.contains(key.as_str()) {
            observations.push(entry.to_observation(collected_at));
        }
    }
    if observations.is_empty() {
        return DataAvailability::NoData;
    }
    let next_cursor = encode_cursor_keys("acc=", latest, previous).unwrap_or(since);
    DataAvailability::Available((observations, next_cursor))
}

#[derive(Debug)]
pub struct SecEdgarSubmissionsFeed {
    client: SecEdgarClient,
    cik: SecCik,
}

impl SecEdgarSubmissionsFeed {
    #[must_use]
    pub const fn new(client: SecEdgarClient, cik: SecCik) -> Self {
        Self { client, cik }
    }
}

impl NewsFeed for SecEdgarSubmissionsFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::SecEdgar)
    }

    fn metadata(&self) -> NewsMetadata {
        sec_feed_metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let entries = match self.client.fetch_company_submissions(&self.cik).await? {
            DataAvailability::Available(list) => list,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        Ok(collect_new_sec_observations(
            &entries,
            since,
            self.client.now(),
        ))
    }
}

#[derive(Debug)]
pub struct SecEdgarCurrentAtomFeed {
    client: SecEdgarClient,
    cik: Option<SecCik>,
    form_type: Option<String>,
}

impl SecEdgarCurrentAtomFeed {
    #[must_use]
    pub const fn new(
        client: SecEdgarClient,
        cik: Option<SecCik>,
        form_type: Option<String>,
    ) -> Self {
        Self {
            client,
            cik,
            form_type,
        }
    }
}

impl NewsFeed for SecEdgarCurrentAtomFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::SecEdgar)
    }

    fn metadata(&self) -> NewsMetadata {
        sec_feed_metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let entries = match self
            .client
            .fetch_current_atom(self.cik.as_ref(), self.form_type.as_deref())
            .await?
        {
            DataAvailability::Available(list) => list,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        Ok(collect_new_sec_observations(
            &entries,
            since,
            self.client.now(),
        ))
    }
}
