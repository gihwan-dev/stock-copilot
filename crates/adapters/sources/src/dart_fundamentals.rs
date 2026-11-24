use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use application::{
    CredentialSource, DataAvailability, FinancialFact, FinancialLine, FinancialSource,
    FinancialStatement, FinancialUnit, Fundamentals, InformationError, MetricValue,
    ResponseMismatch, SecretValue,
};
use domain::{Amount, AmountInput, CalendarDate, Currency, Symbol, UntrustedText};
use serde::Deserialize;

use crate::dart_disclosures::map_dart_status_code;
use crate::{
    DartCorpCode, DartCorpCodeMap, HttpDataSource, SourceCredential, SourceFetch,
    SourceFetchOutcome, SourceHttpClient, collapse_whitespace, http_error_to_information,
    http_status_to_error,
};

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartReportCode {
    Q1,
    HalfYear,
    Q3,
    Annual,
}

impl DartReportCode {
    #[must_use]
    pub fn from_period_end(period_end: CalendarDate) -> Option<Self> {
        match *period_end.month().as_u8() {
            3_u8 => Some(Self::Q1),
            6_u8 => Some(Self::HalfYear),
            9_u8 => Some(Self::Q3),
            12_u8 => Some(Self::Annual),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Q1 => "11013",
            Self::HalfYear => "11012",
            Self::Q3 => "11014",
            Self::Annual => "11011",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartStatementScope {
    Consolidated,
    Separate,
}

impl DartStatementScope {
    #[must_use]
    pub const fn as_fs_div(self) -> &'static str {
        match self {
            Self::Consolidated => "CFS",
            Self::Separate => "OFS",
        }
    }
}

#[must_use]
pub fn build_dart_fundamentals_url(
    corp_code: &DartCorpCode,
    business_year: i32,
    report_code: DartReportCode,
    scope: DartStatementScope,
) -> String {
    format!(
        "https://opendart.fss.or.kr/api/fnlttSinglAcntAll.json?corp_code={}&bsns_year={business_year:04}&reprt_code={}&fs_div={}",
        corp_code.as_str(),
        report_code.as_code(),
        scope.as_fs_div(),
    )
}

#[derive(Deserialize)]
struct WireDartFinancialResponse {
    status: String,
    #[serde(default)]
    list: Vec<WireDartFinancialRow>,
}

#[derive(Deserialize)]
struct WireDartFinancialRow {
    #[serde(default)]
    sj_div: String,
    #[serde(default)]
    account_nm: String,
    #[serde(default)]
    thstrm_amount: String,
    #[serde(default)]
    currency: String,
}

fn parse_dart_amount(raw: &str) -> Result<Option<Amount>, InformationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed == "--" {
        return Ok(None);
    }
    let compact: String = trimmed
        .chars()
        .filter(|ch| *ch != ',' && !ch.is_ascii_whitespace())
        .collect();
    let normalized = compact.strip_prefix('+').unwrap_or(&compact);
    if normalized.is_empty() {
        return Ok(None);
    }
    let amount = Amount::parse(&AmountInput::new(normalized)).map_err(|_error| schema_error())?;
    Ok(Some(amount))
}

fn parse_dart_currency(raw: &str) -> Option<Currency> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "" | "KRW" => Some(Currency::Krw),
        "USD" => Some(Currency::Usd),
        _ => None,
    }
}

fn format_line_label(sj_div: &str, account_nm: &str) -> Result<UntrustedText, InformationError> {
    let section = collapse_whitespace(sj_div);
    let account = collapse_whitespace(account_nm);
    if account.is_empty() {
        return Err(schema_error());
    }
    let text = if section.is_empty() {
        account
    } else {
        format!("{section}: {account}")
    };
    UntrustedText::try_new(&text).map_err(|_error| schema_error())
}

pub fn parse_dart_financial_statements(
    symbol: &Symbol,
    period_end: CalendarDate,
    body: &[u8],
) -> Result<DataAvailability<Vec<FinancialStatement>>, InformationError> {
    let parsed: WireDartFinancialResponse =
        serde_json::from_slice(body).map_err(|_error| schema_error())?;
    match map_dart_status_code(&parsed.status)? {
        DataAvailability::Available(()) => {}
        DataAvailability::NoData => return Ok(DataAvailability::NoData),
        DataAvailability::NotConfigured => return Ok(DataAvailability::NotConfigured),
    }
    if parsed.list.is_empty() {
        return Ok(DataAvailability::NoData);
    }
    let mut section_order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, Vec<FinancialLine>> = BTreeMap::new();
    for row in &parsed.list {
        let Some(amount) = parse_dart_amount(&row.thstrm_amount)? else {
            continue;
        };
        let Some(currency) = parse_dart_currency(&row.currency) else {
            continue;
        };
        let label = format_line_label(&row.sj_div, &row.account_nm)?;
        let fact = FinancialFact::new(
            MetricValue::new(amount),
            FinancialUnit::Currency(currency),
            period_end,
            FinancialSource::Dart,
        );
        let section_key = collapse_whitespace(&row.sj_div);
        if !grouped.contains_key(&section_key) {
            section_order.push(section_key.clone());
        }
        grouped
            .entry(section_key)
            .or_default()
            .push(FinancialLine::new(label, fact));
    }
    let mut statements = Vec::with_capacity(section_order.len());
    for section in section_order {
        if let Some(lines) = grouped.remove(&section)
            && !lines.is_empty()
        {
            statements.push(FinancialStatement::new(symbol.clone(), period_end, lines));
        }
    }
    if statements.is_empty() {
        return Ok(DataAvailability::NoData);
    }
    Ok(DataAvailability::Available(statements))
}

#[derive(Clone)]
pub struct DartFundamentalsClient {
    client: Arc<SourceHttpClient>,
    api_key: Option<Arc<SecretValue>>,
    corp_map: Arc<DartCorpCodeMap>,
}

impl fmt::Debug for DartFundamentalsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DartFundamentalsClient")
            .field("configured", &self.api_key.is_some())
            .field("corp_map_len", &self.corp_map.len())
            .finish_non_exhaustive()
    }
}

impl DartFundamentalsClient {
    #[must_use]
    pub const fn new(
        client: Arc<SourceHttpClient>,
        api_key: Option<Arc<SecretValue>>,
        corp_map: Arc<DartCorpCodeMap>,
    ) -> Self {
        Self {
            client,
            api_key,
            corp_map,
        }
    }

    pub async fn fetch_statements_with_scope(
        &self,
        symbol: &Symbol,
        period_end: CalendarDate,
        scope: DartStatementScope,
    ) -> Result<DataAvailability<Vec<FinancialStatement>>, InformationError> {
        let Some(secret) = self.api_key.as_deref() else {
            return Ok(DataAvailability::NotConfigured);
        };
        let Some(corp_code) = self.corp_map.corp_code_for_symbol(symbol) else {
            return Ok(DataAvailability::NoData);
        };
        let Some(report_code) = DartReportCode::from_period_end(period_end) else {
            return Ok(DataAvailability::NoData);
        };
        let url =
            build_dart_fundamentals_url(corp_code, *period_end.year().as_i32(), report_code, scope);
        let cred_copy = SecretValue::new(secret.as_exposed_secret().to_vec());
        let fetch = SourceFetch::new(HttpDataSource::DartFundamentals, &url)
            .with_credential(SourceCredential::new(CredentialSource::Dart, cred_copy));
        let outcome =
            self.client.fetch(&fetch).await.map_err(|error| {
                http_error_to_information(HttpDataSource::DartFundamentals, error)
            })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(DataAvailability::NoData),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(
                    HttpDataSource::DartFundamentals,
                    status,
                ));
            }
        };
        parse_dart_financial_statements(symbol, period_end, payload.body())
    }
}

impl Fundamentals for DartFundamentalsClient {
    async fn statements(
        &self,
        symbol: &Symbol,
        period_end: CalendarDate,
    ) -> Result<DataAvailability<Vec<FinancialStatement>>, InformationError> {
        match self
            .fetch_statements_with_scope(symbol, period_end, DartStatementScope::Consolidated)
            .await?
        {
            DataAvailability::Available(stmts) => Ok(DataAvailability::Available(stmts)),
            DataAvailability::NotConfigured => Ok(DataAvailability::NotConfigured),
            DataAvailability::NoData => {
                if self.corp_map.corp_code_for_symbol(symbol).is_none()
                    || DartReportCode::from_period_end(period_end).is_none()
                {
                    return Ok(DataAvailability::NoData);
                }
                self.fetch_statements_with_scope(symbol, period_end, DartStatementScope::Separate)
                    .await
            }
        }
    }
}
