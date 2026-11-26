use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use application::{
    AiReadPolicy, CollectedEntry, CollectedObservation, CredentialSource, DataAvailability,
    FeedCursor, FeedSource, InformationError, NewsFeed, NewsMetadata, PublicationTime, ResetTime,
    ResponseMismatch, Retention, SecretValue,
};
use domain::{
    CalendarDate, CalendarYear, DartReceiptNo, DayNumber, Disclosure, DisclosureSource,
    MonthNumber, Nanosecond, NormalizedUrl, ReceiptNo, SpanSeconds, Symbol, TimeSpan,
    UntrustedText,
};
use serde::Deserialize;

use crate::{
    HttpDataSource, SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    SourceRegistry, collapse_whitespace, http_error_to_information, http_status_to_error,
};

const DEFAULT_RETENTION_SECONDS: u64 = 365_u64 * 86_400_u64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DartCorpCode(String);

impl DartCorpCode {
    pub fn try_new(value: &str) -> Result<Self, InformationError> {
        let trimmed = value.trim();
        if trimmed.len() != 8_usize || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InformationError::InvalidResponse(ResponseMismatch::Schema));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DartCorpClass {
    Kospi,
    Kosdaq,
    Konex,
    Etc,
    Unknown(String),
}

impl DartCorpClass {
    fn parse(raw: &str) -> Self {
        match raw.trim() {
            "Y" => Self::Kospi,
            "K" => Self::Kosdaq,
            "N" => Self::Konex,
            "E" => Self::Etc,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartAmendmentKind {
    EntryCorrection,
    AttachmentCorrection,
    AttachmentAddition,
    ChangeRegistration,
    TermsConfirmed,
    OtherCorrection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartCorrectionStatus {
    Original,
    Amended(DartAmendmentKind),
    SupersededByCorrection,
    Withdrawn,
    AmendedAndSuperseded(DartAmendmentKind),
    AmendedAndWithdrawn(DartAmendmentKind),
}

impl DartCorrectionStatus {
    #[must_use]
    pub const fn is_amendment(self) -> bool {
        match self {
            Self::Amended(_) | Self::AmendedAndSuperseded(_) | Self::AmendedAndWithdrawn(_) => true,
            Self::Original | Self::SupersededByCorrection | Self::Withdrawn => false,
        }
    }

    #[must_use]
    pub const fn is_withdrawn(self) -> bool {
        match self {
            Self::Withdrawn | Self::AmendedAndWithdrawn(_) => true,
            Self::Original
            | Self::Amended(_)
            | Self::SupersededByCorrection
            | Self::AmendedAndSuperseded(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DartCorrectionGroupKey {
    corp_code: DartCorpCode,
    base_report_name: String,
}

impl DartCorrectionGroupKey {
    #[must_use]
    pub const fn corp_code(&self) -> &DartCorpCode {
        &self.corp_code
    }

    #[must_use]
    pub fn base_report_name(&self) -> &str {
        &self.base_report_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDisclosureEntry {
    disclosure: Disclosure,
    corp_code: DartCorpCode,
    corp_name: UntrustedText,
    stock_code: Option<Symbol>,
    corp_class: DartCorpClass,
    receipt_date: CalendarDate,
    filer_name: UntrustedText,
    base_report_name: UntrustedText,
    correction: DartCorrectionStatus,
}

impl DartDisclosureEntry {
    #[must_use]
    pub const fn disclosure(&self) -> &Disclosure {
        &self.disclosure
    }

    #[must_use]
    pub const fn corp_code(&self) -> &DartCorpCode {
        &self.corp_code
    }

    #[must_use]
    pub const fn corp_name(&self) -> &UntrustedText {
        &self.corp_name
    }

    #[must_use]
    pub const fn stock_code(&self) -> Option<&Symbol> {
        self.stock_code.as_ref()
    }

    #[must_use]
    pub const fn corp_class(&self) -> &DartCorpClass {
        &self.corp_class
    }

    #[must_use]
    pub const fn receipt_date(&self) -> CalendarDate {
        self.receipt_date
    }

    #[must_use]
    pub const fn filer_name(&self) -> &UntrustedText {
        &self.filer_name
    }

    #[must_use]
    pub const fn base_report_name(&self) -> &UntrustedText {
        &self.base_report_name
    }

    #[must_use]
    pub const fn correction(&self) -> DartCorrectionStatus {
        self.correction
    }

    #[must_use]
    pub fn correction_group_key(&self) -> DartCorrectionGroupKey {
        DartCorrectionGroupKey {
            corp_code: self.corp_code.clone(),
            base_report_name: self.base_report_name.as_untrusted_str().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDisclosurePage {
    entries: Vec<DartDisclosureEntry>,
    page_no: u32,
    total_page: u32,
    total_count: u64,
}

impl DartDisclosurePage {
    #[must_use]
    pub fn entries(&self) -> &[DartDisclosureEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn page_no(&self) -> u32 {
        self.page_no
    }

    #[must_use]
    pub const fn total_page(&self) -> u32 {
        self.total_page
    }

    #[must_use]
    pub const fn total_count(&self) -> u64 {
        self.total_count
    }
}

#[derive(Deserialize)]
struct WireDartListResponse {
    status: String,
    #[serde(default)]
    page_no: Option<u32>,
    #[serde(default)]
    total_page: Option<u32>,
    #[serde(default)]
    total_count: Option<u64>,
    #[serde(default)]
    list: Vec<WireDartRow>,
}

#[derive(Deserialize)]
struct WireDartRow {
    corp_cls: String,
    corp_name: String,
    corp_code: String,
    #[serde(default)]
    stock_code: String,
    report_nm: String,
    rcept_no: String,
    #[serde(default)]
    flr_nm: String,
    rcept_dt: String,
    #[serde(default)]
    rm: String,
}

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

pub(crate) fn format_yyyymmdd(date: CalendarDate) -> String {
    format!(
        "{:04}{:02}{:02}",
        date.year().as_i32(),
        date.month().as_u8(),
        date.day().as_u8()
    )
}

pub(crate) fn parse_yyyymmdd(raw: &str) -> Result<CalendarDate, InformationError> {
    let trimmed = raw.trim();
    if trimmed.len() != 8_usize || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(schema_error());
    }
    let year = trimmed
        .get(0_usize..4_usize)
        .and_then(|slice| slice.parse::<i32>().ok())
        .ok_or_else(schema_error)?;
    let month = trimmed
        .get(4_usize..6_usize)
        .and_then(|slice| slice.parse::<u8>().ok())
        .ok_or_else(schema_error)?;
    let day = trimmed
        .get(6_usize..8_usize)
        .and_then(|slice| slice.parse::<u8>().ok())
        .ok_or_else(schema_error)?;
    CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    )
    .map_err(|_error| schema_error())
}

fn strip_amendment_prefix(text: &str) -> Option<(DartAmendmentKind, &str)> {
    const PREFIXES: [(&str, DartAmendmentKind); 6] = [
        ("[기재정정]", DartAmendmentKind::EntryCorrection),
        ("[첨부정정]", DartAmendmentKind::AttachmentCorrection),
        ("[첨부추가]", DartAmendmentKind::AttachmentAddition),
        ("[변경등록]", DartAmendmentKind::ChangeRegistration),
        ("[발행조건확정]", DartAmendmentKind::TermsConfirmed),
        ("[정정]", DartAmendmentKind::OtherCorrection),
    ];
    let trimmed = text.trim_start();
    for (prefix, kind) in PREFIXES {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some((kind, rest.trim_start()));
        }
    }
    None
}

#[must_use]
pub fn parse_correction_info(report_name: &str, remarks: &str) -> (String, DartCorrectionStatus) {
    let mut remainder = report_name.trim();
    let mut first_amendment = None;
    while let Some((kind, rest)) = strip_amendment_prefix(remainder) {
        if first_amendment.is_none() {
            first_amendment = Some(kind);
        }
        remainder = rest;
    }
    let base_name = collapse_whitespace(remainder);
    let has_withdrawn = remarks.contains('철');
    let has_superseded = remarks.contains('정');
    let status = match (first_amendment, has_withdrawn, has_superseded) {
        (Some(kind), true, _) => DartCorrectionStatus::AmendedAndWithdrawn(kind),
        (Some(kind), false, true) => DartCorrectionStatus::AmendedAndSuperseded(kind),
        (Some(kind), false, false) => DartCorrectionStatus::Amended(kind),
        (None, true, _) => DartCorrectionStatus::Withdrawn,
        (None, false, true) => DartCorrectionStatus::SupersededByCorrection,
        (None, false, false) => DartCorrectionStatus::Original,
    };
    (base_name, status)
}

pub(crate) fn map_dart_status_code(status: &str) -> Result<DataAvailability<()>, InformationError> {
    match status.trim() {
        "000" => Ok(DataAvailability::Available(())),
        "013" => Ok(DataAvailability::NoData),
        "010" | "011" | "012" => Err(InformationError::InvalidCredential(CredentialSource::Dart)),
        "020" => Err(InformationError::Unavailable(ResetTime::NotProvided)),
        _ => Err(schema_error()),
    }
}

fn parse_wire_row(row: &WireDartRow) -> Result<DartDisclosureEntry, InformationError> {
    let receipt = DartReceiptNo::try_new(row.rcept_no.trim()).map_err(|_error| schema_error())?;
    let url = NormalizedUrl::try_new(&format!(
        "https://dart.fss.or.kr/dsaf001/main.do?rcpNo={}",
        receipt.as_str()
    ))
    .map_err(|_error| schema_error())?;
    let corp_code = DartCorpCode::try_new(&row.corp_code)?;
    let corp_name_clean = collapse_whitespace(&row.corp_name);
    let report_clean = collapse_whitespace(&row.report_nm);
    if corp_name_clean.is_empty() || report_clean.is_empty() {
        return Err(schema_error());
    }
    let title_text = format!("{corp_name_clean} - {report_clean}");
    let title = UntrustedText::try_new(&title_text).map_err(|_error| schema_error())?;
    let corp_name = UntrustedText::try_new(&corp_name_clean).map_err(|_error| schema_error())?;
    let filer_name = UntrustedText::try_new(&collapse_whitespace(&row.flr_nm))
        .map_err(|_error| schema_error())?;
    let (base_name_raw, correction) = parse_correction_info(&report_clean, &row.rm);
    let base_report_name =
        UntrustedText::try_new(&base_name_raw).map_err(|_error| schema_error())?;
    let stock_trimmed = row.stock_code.trim();
    let stock_code = if stock_trimmed.is_empty() {
        None
    } else {
        Some(Symbol::try_new(stock_trimmed).map_err(|_error| schema_error())?)
    };
    let receipt_date = parse_yyyymmdd(&row.rcept_dt)?;
    let disclosure = Disclosure::new(ReceiptNo::Dart(receipt), url, title, None);
    Ok(DartDisclosureEntry {
        disclosure,
        corp_code,
        corp_name,
        stock_code,
        corp_class: DartCorpClass::parse(&row.corp_cls),
        receipt_date,
        filer_name,
        base_report_name,
        correction,
    })
}

#[must_use]
pub fn group_dart_amendments(
    entries: &[DartDisclosureEntry],
) -> BTreeMap<DartCorrectionGroupKey, Vec<DartReceiptNo>> {
    let mut grouped: BTreeMap<DartCorrectionGroupKey, Vec<DartReceiptNo>> = BTreeMap::new();
    for entry in entries {
        if let ReceiptNo::Dart(receipt) = entry.disclosure().key().receipt_no() {
            grouped
                .entry(entry.correction_group_key())
                .or_default()
                .push(receipt.clone());
        }
    }
    grouped
}

pub struct DartDisclosureClient {
    client: Arc<SourceHttpClient>,
    api_key: Option<Arc<SecretValue>>,
}

impl fmt::Debug for DartDisclosureClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DartDisclosureClient")
            .field("configured", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl DartDisclosureClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>, api_key: Option<Arc<SecretValue>>) -> Self {
        Self { client, api_key }
    }

    pub(crate) fn credential(&self) -> Option<SourceCredential> {
        self.api_key.as_ref().map(|secret| {
            SourceCredential::new(
                CredentialSource::Dart,
                SecretValue::new(secret.as_exposed_secret().to_vec()),
            )
        })
    }

    pub async fn fetch_page(
        &self,
        begin: CalendarDate,
        end: CalendarDate,
        page_no: u32,
    ) -> Result<DataAvailability<DartDisclosurePage>, InformationError> {
        let Some(credential) = self.credential() else {
            return Ok(DataAvailability::NotConfigured);
        };
        let page = page_no.max(1_u32);
        let url = format!(
            "https://opendart.fss.or.kr/api/list.json?bgn_de={}&end_de={}&page_no={page}&page_count=100",
            format_yyyymmdd(begin),
            format_yyyymmdd(end)
        );
        let fetch =
            SourceFetch::new(HttpDataSource::DartDisclosureApi, &url).with_credential(credential);
        let outcome =
            self.client.fetch(&fetch).await.map_err(|error| {
                http_error_to_information(HttpDataSource::DartDisclosureApi, error)
            })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(DataAvailability::NoData),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(
                    HttpDataSource::DartDisclosureApi,
                    status,
                ));
            }
        };
        let parsed: WireDartListResponse =
            serde_json::from_slice(payload.body()).map_err(|_error| schema_error())?;
        match map_dart_status_code(&parsed.status)? {
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::Available(()) => {}
        }
        let mut entries = Vec::with_capacity(parsed.list.len());
        for row in &parsed.list {
            entries.push(parse_wire_row(row)?);
        }
        Ok(DataAvailability::Available(DartDisclosurePage {
            entries,
            page_no: parsed.page_no.unwrap_or(page),
            total_page: parsed.total_page.unwrap_or(1_u32),
            total_count: parsed.total_count.unwrap_or(0_u64),
        }))
    }

    pub async fn fetch_date_as_feed(
        &self,
        begin: CalendarDate,
        end: CalendarDate,
    ) -> Result<DataAvailability<Vec<CollectedObservation>>, InformationError> {
        let page = match self.fetch_page(begin, end, 1_u32).await? {
            DataAvailability::Available(data) => data,
            DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => return Ok(DataAvailability::NoData),
        };
        let collected_at = self.client.now();
        let observations = page
            .entries
            .into_iter()
            .map(|entry| {
                CollectedObservation::new(
                    CollectedEntry::Disclosure(entry.disclosure),
                    PublicationTime::NotProvided,
                    collected_at,
                )
            })
            .collect();
        Ok(DataAvailability::Available(observations))
    }
}

pub(crate) fn dart_feed_metadata() -> NewsMetadata {
    let spec = SourceRegistry::spec(HttpDataSource::DartDisclosureApi);
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

#[derive(Debug)]
pub struct DartDisclosureFeed {
    dart: DartDisclosureClient,
    date: CalendarDate,
}

impl DartDisclosureFeed {
    #[must_use]
    pub const fn new(dart: DartDisclosureClient, date: CalendarDate) -> Self {
        Self { dart, date }
    }
}

impl NewsFeed for DartDisclosureFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::Dart)
    }

    fn metadata(&self) -> NewsMetadata {
        dart_feed_metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        match self.dart.fetch_date_as_feed(self.date, self.date).await? {
            DataAvailability::Available(items) => Ok(DataAvailability::Available((items, since))),
            DataAvailability::NotConfigured => Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => Ok(DataAvailability::NoData),
        }
    }
}
