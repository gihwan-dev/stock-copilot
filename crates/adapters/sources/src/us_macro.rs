use std::fmt;
use std::sync::Arc;

use application::{
    CredentialSource, DataAvailability, InformationError, MacroFact, MacroObservation, MacroSeries,
    MacroSeriesId, MacroSource, MacroUnit, MetricValue, ResponseMismatch, SecretValue,
};
use domain::{Amount, AmountInput, CalendarDate, CalendarYear, DayNumber, MonthNumber};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Deserialize;

use crate::{
    HttpDataSource, SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    http_error_to_information, http_status_to_error,
};

const MAX_TREASURY_YEARS: i32 = 5;

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryTenor {
    OneMonth,
    ThreeMonth,
    SixMonth,
    OneYear,
    TwoYear,
    FiveYear,
    TenYear,
    ThirtyYear,
}

impl TreasuryTenor {
    pub const ALL: [Self; 8] = [
        Self::OneMonth,
        Self::ThreeMonth,
        Self::SixMonth,
        Self::OneYear,
        Self::TwoYear,
        Self::FiveYear,
        Self::TenYear,
        Self::ThirtyYear,
    ];

    #[must_use]
    pub const fn series_id(self) -> &'static str {
        match self {
            Self::OneMonth => "UST_1M",
            Self::ThreeMonth => "UST_3M",
            Self::SixMonth => "UST_6M",
            Self::OneYear => "UST_1Y",
            Self::TwoYear => "UST_2Y",
            Self::FiveYear => "UST_5Y",
            Self::TenYear => "UST_10Y",
            Self::ThirtyYear => "UST_30Y",
        }
    }

    #[must_use]
    pub const fn xml_field(self) -> &'static str {
        match self {
            Self::OneMonth => "BC_1MONTH",
            Self::ThreeMonth => "BC_3MONTH",
            Self::SixMonth => "BC_6MONTH",
            Self::OneYear => "BC_1YEAR",
            Self::TwoYear => "BC_2YEAR",
            Self::FiveYear => "BC_5YEAR",
            Self::TenYear => "BC_10YEAR",
            Self::ThirtyYear => "BC_30YEAR",
        }
    }

    fn from_series(series: &MacroSeriesId) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|tenor| tenor.series_id() == series.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FredSeries {
    Sp500,
    NasdaqComposite,
    DowJones,
    Vix,
    BroadDollar,
    WtiCrude,
    BrentCrude,
}

impl FredSeries {
    pub const ALL: [Self; 7] = [
        Self::Sp500,
        Self::NasdaqComposite,
        Self::DowJones,
        Self::Vix,
        Self::BroadDollar,
        Self::WtiCrude,
        Self::BrentCrude,
    ];

    #[must_use]
    pub const fn series_id(self) -> &'static str {
        match self {
            Self::Sp500 => "SP500",
            Self::NasdaqComposite => "NASDAQCOM",
            Self::DowJones => "DJIA",
            Self::Vix => "VIXCLS",
            Self::BroadDollar => "DTWEXBGS",
            Self::WtiCrude => "DCOILWTICO",
            Self::BrentCrude => "DCOILBRENTEU",
        }
    }

    #[must_use]
    pub const fn unit(self) -> MacroUnit {
        match self {
            Self::Sp500
            | Self::NasdaqComposite
            | Self::DowJones
            | Self::Vix
            | Self::BroadDollar => MacroUnit::IndexPoints,
            Self::WtiCrude | Self::BrentCrude => MacroUnit::Usd,
        }
    }

    fn from_series(series: &MacroSeriesId) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|fred| fred.series_id() == series.as_str())
    }
}

#[must_use]
pub fn build_treasury_yield_url(year: CalendarYear) -> String {
    format!(
        "https://home.treasury.gov/resource-center/data-chart-center/interest-rates/pages/xml?data=daily_treasury_yield_curve&field_tdr_date_value={:04}",
        *year.as_i32()
    )
}

#[must_use]
pub fn build_fred_observations_url(
    series: FredSeries,
    from: CalendarDate,
    through: CalendarDate,
) -> String {
    format!(
        "https://api.stlouisfed.org/fred/series/observations?series_id={}&observation_start={}&observation_end={}&file_type=json",
        series.series_id(),
        format_date(from),
        format_date(through),
    )
}

fn format_date(date: CalendarDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        *date.year().as_i32(),
        *date.month().as_u8(),
        *date.day().as_u8()
    )
}

fn parse_iso_date(raw: &str) -> Option<CalendarDate> {
    let date_part = raw.trim().split('T').next()?;
    let mut parts = date_part.split('-');
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

fn parse_decimal(raw: &str) -> Result<Option<Amount>, InformationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed.eq_ignore_ascii_case("N/A") {
        return Ok(None);
    }
    Amount::parse(&AmountInput::new(trimmed))
        .map(Some)
        .map_err(|_error| schema_error())
}

fn within(date: CalendarDate, from: CalendarDate, through: CalendarDate) -> bool {
    from <= date && date <= through
}

fn local_name(raw: &str) -> &str {
    raw.rsplit_once(':').map_or(raw, |(_prefix, local)| local)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreasuryField {
    Other,
    Date,
    Value,
}

#[derive(Debug, Default)]
struct TreasuryRow {
    date: String,
    value: String,
}

#[derive(Debug)]
struct TreasuryParser {
    tenor: TreasuryTenor,
    rows: Vec<TreasuryRow>,
    current: Option<TreasuryRow>,
    field: TreasuryField,
    saw_feed: bool,
}

impl TreasuryParser {
    const fn new(tenor: TreasuryTenor) -> Self {
        Self {
            tenor,
            rows: Vec::new(),
            current: None,
            field: TreasuryField::Other,
            saw_feed: false,
        }
    }

    fn on_start(&mut self, raw: &str) {
        let name = local_name(raw);
        if name == "feed" {
            self.saw_feed = true;
        }
        if name == "properties" {
            self.current = Some(TreasuryRow::default());
        }
        self.field = if name == "NEW_DATE" {
            TreasuryField::Date
        } else if name == self.tenor.xml_field() {
            TreasuryField::Value
        } else {
            TreasuryField::Other
        };
    }

    fn on_end(&mut self, raw: &str) {
        if local_name(raw) == "properties"
            && let Some(row) = self.current.take()
        {
            self.rows.push(row);
        }
        self.field = TreasuryField::Other;
    }

    fn on_text(&mut self, piece: &str) {
        let Some(row) = self.current.as_mut() else {
            return;
        };
        match self.field {
            TreasuryField::Date => row.date.push_str(piece),
            TreasuryField::Value => row.value.push_str(piece),
            TreasuryField::Other => {}
        }
    }
}

pub fn parse_treasury_yield_xml(
    body: &[u8],
    tenor: TreasuryTenor,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<MacroObservation>, InformationError> {
    let text = std::str::from_utf8(body)
        .map_err(|_error| schema_error())?
        .trim_start_matches('\u{feff}')
        .trim();
    if !text.starts_with('<') {
        return Err(schema_error());
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut parser = TreasuryParser::new(tenor);
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => parser.on_start(start.name().as_ref()),
            Ok(Event::End(end)) => parser.on_end(end.name().as_ref()),
            Ok(Event::Text(value)) => parser.on_text(value.as_ref()),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_error) => return Err(schema_error()),
        }
    }
    if !parser.saw_feed || parser.current.is_some() {
        return Err(schema_error());
    }
    let rows = parser.rows;
    let series = MacroSeriesId::try_new(tenor.series_id()).map_err(|_error| schema_error())?;
    let mut observations = Vec::new();
    for row in rows {
        let date = parse_iso_date(&row.date).ok_or_else(schema_error)?;
        if !within(date, from, through) {
            continue;
        }
        let Some(amount) = parse_decimal(&row.value)? else {
            continue;
        };
        observations.push(MacroObservation::new(
            series.clone(),
            MacroFact::new(
                MetricValue::new(amount),
                MacroUnit::Percent,
                date,
                MacroSource::UsTreasury,
            ),
        ));
    }
    observations.sort_by_key(|observation| observation.fact().as_of());
    observations.dedup_by_key(|observation| observation.fact().as_of());
    Ok(observations)
}

#[derive(Deserialize)]
struct WireFredResponse {
    observations: Vec<WireFredObservation>,
}

#[derive(Deserialize)]
struct WireFredObservation {
    date: String,
    value: String,
}

pub fn parse_fred_observations_json(
    body: &[u8],
    series: FredSeries,
    from: CalendarDate,
    through: CalendarDate,
) -> Result<Vec<MacroObservation>, InformationError> {
    let parsed: WireFredResponse = serde_json::from_slice(body).map_err(|_error| schema_error())?;
    let id = MacroSeriesId::try_new(series.series_id()).map_err(|_error| schema_error())?;
    let mut observations = Vec::new();
    for row in parsed.observations {
        let date = parse_iso_date(&row.date).ok_or_else(schema_error)?;
        if !within(date, from, through) {
            continue;
        }
        let Some(amount) = parse_decimal(&row.value)? else {
            continue;
        };
        observations.push(MacroObservation::new(
            id.clone(),
            MacroFact::new(
                MetricValue::new(amount),
                series.unit(),
                date,
                MacroSource::Fred,
            ),
        ));
    }
    observations.sort_by_key(|observation| observation.fact().as_of());
    Ok(observations)
}

#[derive(Clone)]
pub struct UsMacroClient {
    client: Arc<SourceHttpClient>,
    fred_key: Option<Arc<SecretValue>>,
}

impl fmt::Debug for UsMacroClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsMacroClient")
            .field("fred_configured", &self.fred_key.is_some())
            .finish_non_exhaustive()
    }
}

fn wrap(observations: Vec<MacroObservation>) -> DataAvailability<Vec<MacroObservation>> {
    if observations.is_empty() {
        DataAvailability::NoData
    } else {
        DataAvailability::Available(observations)
    }
}

impl UsMacroClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>, fred_key: Option<Arc<SecretValue>>) -> Self {
        Self { client, fred_key }
    }

    async fn fetch_body(
        &self,
        source: HttpDataSource,
        fetch: &SourceFetch,
    ) -> Result<Option<Vec<u8>>, InformationError> {
        let outcome = self
            .client
            .fetch(fetch)
            .await
            .map_err(|error| http_error_to_information(source, error))?;
        match outcome {
            SourceFetchOutcome::Modified(payload) => Ok(Some(payload.body().to_vec())),
            SourceFetchOutcome::NotModified => Ok(None),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                Err(http_status_to_error(source, status))
            }
        }
    }

    async fn treasury(
        &self,
        tenor: TreasuryTenor,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<DataAvailability<Vec<MacroObservation>>, InformationError> {
        let last = *through.year().as_i32();
        let earliest = last.saturating_sub(MAX_TREASURY_YEARS.saturating_sub(1_i32));
        let first = (*from.year().as_i32()).max(earliest);
        let mut all = Vec::new();
        for year in first..=last {
            let url = build_treasury_yield_url(CalendarYear::new(year));
            let fetch = SourceFetch::new(HttpDataSource::UsTreasury, &url);
            if let Some(body) = self.fetch_body(HttpDataSource::UsTreasury, &fetch).await? {
                all.extend(parse_treasury_yield_xml(&body, tenor, from, through)?);
            }
        }
        Ok(wrap(all))
    }

    async fn fred(
        &self,
        series: FredSeries,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<DataAvailability<Vec<MacroObservation>>, InformationError> {
        let Some(secret) = self.fred_key.as_deref() else {
            return Ok(DataAvailability::NotConfigured);
        };
        let url = build_fred_observations_url(series, from, through);
        let copy = SecretValue::new(secret.as_exposed_secret().to_vec());
        let fetch = SourceFetch::new(HttpDataSource::Fred, &url)
            .with_credential(SourceCredential::new(CredentialSource::Fred, copy));
        let Some(body) = self.fetch_body(HttpDataSource::Fred, &fetch).await? else {
            return Ok(DataAvailability::NoData);
        };
        Ok(wrap(parse_fred_observations_json(
            &body, series, from, through,
        )?))
    }
}

impl MacroSeries for UsMacroClient {
    async fn daily_values(
        &self,
        series: &MacroSeriesId,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<DataAvailability<Vec<MacroObservation>>, InformationError> {
        if through < from {
            return Ok(DataAvailability::NoData);
        }
        if let Some(tenor) = TreasuryTenor::from_series(series) {
            return self.treasury(tenor, from, through).await;
        }
        if let Some(fred) = FredSeries::from_series(series) {
            return self.fred(fred, from, through).await;
        }
        Ok(DataAvailability::NoData)
    }
}
