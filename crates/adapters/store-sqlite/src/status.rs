use crate::estimates::codec::{
    Decoder, Encoder, invalid, put_news_source, put_timestamp, take_news_source, take_timestamp,
};
use crate::{Database, sql::status as sql};
use application::{
    AliasName, CollectionMinutes, CollectionTarget, CredentialSource, HttpLastModified,
    InformationError, NewsDataSettings, NewsDataSettingsChange, NewsDataSettingsRepository,
    ResetTime, ResponseMismatch, RetryDelay, SchemaReviewState, SecContactSetting, SourceState,
    SourceStatus, SourceStatusRepository, SourceSwitch, SpecChangeReason, SpecDocumentSnapshot,
    SpecDriftStatus, SpecFetchError, SpecHash, SpecStatusRecord, SpecVersionLabel, StockAlias,
    SystemPortError, TossSpecBaseline, TossSpecObservation, TossSpecStatusRepository,
    WriteCompleted,
};
use domain::{
    AiBodyReadToggle, DataSource, InstrumentId, MovePercent, Nanosecond, NewsBurstCount,
    NewsSource, SpanSeconds, TimeSpan, Timestamp, WakeThresholds,
};
use rusqlite::OptionalExtension;
use std::future::Future;

const NEWS_VERSION: i64 = 1;

const NEWS_SOURCES: [NewsSource; 8] = [
    NewsSource::Hankyung,
    NewsSource::Maekyung,
    NewsSource::Yonhap,
    NewsSource::Edaily,
    NewsSource::BankOfKorea,
    NewsSource::FederalReserve,
    NewsSource::GoogleNews,
    NewsSource::YahooFinance,
];

const fn source_name(source: DataSource) -> &'static str {
    match source {
        DataSource::KindDisclosure => "kind_disclosure",
        DataSource::DartDisclosureApi => "dart_disclosure_api",
        DataSource::DartDisclosureRss => "dart_disclosure_rss",
        DataSource::DartFundamentals => "dart_fundamentals",
        DataSource::KindCorpList => "kind_corp_list",
        DataSource::Hankyung => "hankyung",
        DataSource::Maekyung => "maekyung",
        DataSource::Yonhap => "yonhap",
        DataSource::Edaily => "edaily",
        DataSource::SegyeFinance => "segye_finance",
        DataSource::GoogleNews => "google_news",
        DataSource::WebSearch => "web_search",
        DataSource::YahooFinance => "yahoo_finance",
        DataSource::SecEdgar => "sec_edgar",
        DataSource::BankOfKorea => "bank_of_korea",
        DataSource::FederalReserve => "federal_reserve",
        DataSource::UsTreasury => "us_treasury",
        DataSource::Fred => "fred",
    }
}

fn parse_source(raw: &str) -> rusqlite::Result<DataSource> {
    DataSource::all()
        .into_iter()
        .find(|source| source_name(*source) == raw)
        .ok_or_else(|| invalid(()))
}

const fn target_name(target: CollectionTarget) -> &'static str {
    match target {
        CollectionTarget::KindDisclosure => "kind_disclosure",
        CollectionTarget::DartList => "dart_list",
        CollectionTarget::PressRss => "press_rss",
        CollectionTarget::HankyungRss => "hankyung_rss",
        CollectionTarget::GoogleNews => "google_news",
        CollectionTarget::YahooFinance => "yahoo_finance",
        CollectionTarget::SecEdgar => "sec_edgar",
        CollectionTarget::CentralBankRss => "central_bank_rss",
        CollectionTarget::TreasuryAndFred => "treasury_and_fred",
    }
}

fn parse_target(raw: &str) -> rusqlite::Result<CollectionTarget> {
    CollectionTarget::ALL
        .into_iter()
        .find(|target| target_name(*target) == raw)
        .ok_or_else(|| invalid(()))
}

fn put_optional_time(out: &mut Encoder, value: Option<Timestamp>) {
    match value {
        Some(at) => {
            out.push("some");
            put_timestamp(out, at);
        }
        None => out.push("none"),
    }
}

fn take_optional_time(input: &mut Decoder<'_>) -> rusqlite::Result<Option<Timestamp>> {
    match input.next()? {
        "some" => take_timestamp(input).map(Some),
        "none" => Ok(None),
        _ => Err(invalid(())),
    }
}

fn put_optional_text(out: &mut Encoder, value: Option<&str>) {
    match value {
        Some(text) => {
            out.push("some");
            out.push(text);
        }
        None => out.push("none"),
    }
}

fn take_optional_text<'input>(
    input: &mut Decoder<'input>,
) -> rusqlite::Result<Option<&'input str>> {
    match input.next()? {
        "some" => input.next().map(Some),
        "none" => Ok(None),
        _ => Err(invalid(())),
    }
}

fn put_information_error(out: &mut Encoder, error: InformationError) {
    match error {
        InformationError::Unavailable(ResetTime::Known(at)) => {
            out.push("unavailable_known");
            put_timestamp(out, at);
        }
        InformationError::Unavailable(ResetTime::NotProvided) => out.push("unavailable"),
        InformationError::InvalidResponse(ResponseMismatch::Schema) => out.push("schema"),
        InformationError::InvalidResponse(ResponseMismatch::Validation) => out.push("validation"),
        InformationError::InvalidCredential(CredentialSource::Dart) => out.push("credential_dart"),
        InformationError::InvalidCredential(CredentialSource::Fred) => out.push("credential_fred"),
    }
}

fn take_information_error(input: &mut Decoder<'_>) -> rusqlite::Result<InformationError> {
    match input.next()? {
        "unavailable_known" => Ok(InformationError::Unavailable(ResetTime::Known(
            take_timestamp(input)?,
        ))),
        "unavailable" => Ok(InformationError::Unavailable(ResetTime::NotProvided)),
        "schema" => Ok(InformationError::InvalidResponse(ResponseMismatch::Schema)),
        "validation" => Ok(InformationError::InvalidResponse(
            ResponseMismatch::Validation,
        )),
        "credential_dart" => Ok(InformationError::InvalidCredential(CredentialSource::Dart)),
        "credential_fred" => Ok(InformationError::InvalidCredential(CredentialSource::Fred)),
        _ => Err(invalid(())),
    }
}

fn encode_source_status(status: &SourceStatus) -> String {
    let mut out = Encoder::new();
    match status.state() {
        SourceState::NeverRun => out.push("never_run"),
        SourceState::Healthy => out.push("healthy"),
        SourceState::NoData => out.push("no_data"),
        SourceState::NotConfigured => out.push("not_configured"),
        SourceState::Failed(error) => {
            out.push("failed");
            put_information_error(&mut out, error);
        }
    }
    put_optional_time(&mut out, status.last_success_at());
    put_optional_time(&mut out, status.last_attempt_at());
    out.finish()
}

fn decode_source_status(source: DataSource, payload: &str) -> rusqlite::Result<SourceStatus> {
    let mut input = Decoder::new(payload);
    let state = match input.next()? {
        "never_run" => SourceState::NeverRun,
        "healthy" => SourceState::Healthy,
        "no_data" => SourceState::NoData,
        "not_configured" => SourceState::NotConfigured,
        "failed" => SourceState::Failed(take_information_error(&mut input)?),
        _ => return Err(invalid(())),
    };
    let last_success = take_optional_time(&mut input)?;
    let last_attempt = take_optional_time(&mut input)?;
    input.finish()?;
    SourceStatus::try_new(source, state, last_success, last_attempt).map_err(invalid)
}

#[derive(Debug)]
pub struct SourceStatusStore<'database> {
    database: &'database Database,
}

impl<'database> SourceStatusStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl SourceStatusRepository for SourceStatusStore<'_> {
    fn find(
        &self,
        source: DataSource,
    ) -> impl Future<Output = Result<Option<SourceStatus>, SystemPortError>> + Send {
        self.database.read(move |connection| {
            connection
                .query_row(sql::FIND_SOURCE, [source_name(source)], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?
                .map(|payload| decode_source_status(source, &payload))
                .transpose()
        })
    }

    fn list_all(&self) -> impl Future<Output = Result<Vec<SourceStatus>, SystemPortError>> + Send {
        self.database.read(|connection| {
            let mut statement = connection.prepare(sql::ALL_SOURCES)?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut statuses = Vec::new();
            for row in rows {
                let (name, payload) = row?;
                statuses.push(decode_source_status(parse_source(&name)?, &payload)?);
            }
            statuses.sort_by_key(SourceStatus::source);
            Ok(statuses)
        })
    }

    fn save(
        &self,
        status: &SourceStatus,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let name = source_name(status.source());
        let payload = encode_source_status(status);
        self.database.write(move |transaction| {
            match transaction.execute(sql::UPSERT_SOURCE, rusqlite::params![name, payload])? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }
}

fn put_snapshot(out: &mut Encoder, snapshot: &SpecDocumentSnapshot) {
    out.push(snapshot.normalized_hash().as_str());
    put_optional_text(out, snapshot.version().map(SpecVersionLabel::as_str));
    put_optional_text(out, snapshot.last_modified().map(HttpLastModified::as_str));
}

fn hash(raw: &str) -> rusqlite::Result<SpecHash> {
    SpecHash::try_new(raw).map_err(invalid)
}

fn label(raw: &str) -> rusqlite::Result<SpecVersionLabel> {
    SpecVersionLabel::try_new(raw).map_err(invalid)
}

fn modified(raw: &str) -> rusqlite::Result<HttpLastModified> {
    HttpLastModified::try_new(raw).map_err(invalid)
}

fn take_snapshot(input: &mut Decoder<'_>) -> rusqlite::Result<SpecDocumentSnapshot> {
    let normalized = hash(input.next()?)?;
    let version = take_optional_text(input)?.map(label).transpose()?;
    let last_modified = take_optional_text(input)?.map(modified).transpose()?;
    Ok(SpecDocumentSnapshot::new(
        normalized,
        version,
        last_modified,
    ))
}

fn put_baseline(out: &mut Encoder, baseline: &TossSpecBaseline) {
    out.push(baseline.raw_spec_hash().as_str());
    out.push(baseline.normalized_spec_hash().as_str());
    out.push(baseline.version_label().as_str());
    put_optional_text(out, baseline.versions_hash().map(SpecHash::as_str));
    put_optional_text(
        out,
        baseline.spec_last_modified().map(HttpLastModified::as_str),
    );
    put_optional_text(
        out,
        baseline
            .versions_last_modified()
            .map(HttpLastModified::as_str),
    );
}

fn take_baseline(input: &mut Decoder<'_>) -> rusqlite::Result<TossSpecBaseline> {
    let raw = hash(input.next()?)?;
    let normalized = hash(input.next()?)?;
    let version = label(input.next()?)?;
    let mut baseline = TossSpecBaseline::new(raw, normalized, version);
    if let Some(value) = take_optional_text(input)? {
        baseline = baseline.with_versions_hash(hash(value)?);
    }
    if let Some(value) = take_optional_text(input)? {
        baseline = baseline.with_spec_last_modified(modified(value)?);
    }
    if let Some(value) = take_optional_text(input)? {
        baseline = baseline.with_versions_last_modified(modified(value)?);
    }
    Ok(baseline)
}

fn put_fetch_error(out: &mut Encoder, error: SpecFetchError) {
    match error {
        SpecFetchError::Network => out.push("network"),
        SpecFetchError::ServerError => out.push("server_error"),
        SpecFetchError::RateLimited(RetryDelay::Unknown) => out.push("rate_limited"),
        SpecFetchError::RateLimited(RetryDelay::Known(span)) => {
            out.push("rate_limited_known");
            let (seconds, nanos) = span.as_parts();
            out.push_u64(*seconds.as_u64());
            out.push_u32(*nanos.as_u32());
        }
        SpecFetchError::AccessDenied => out.push("access_denied"),
        SpecFetchError::InvalidResponse => out.push("invalid_response"),
        SpecFetchError::ResponseTooLarge => out.push("response_too_large"),
    }
}

fn take_fetch_error(input: &mut Decoder<'_>) -> rusqlite::Result<SpecFetchError> {
    match input.next()? {
        "network" => Ok(SpecFetchError::Network),
        "server_error" => Ok(SpecFetchError::ServerError),
        "rate_limited" => Ok(SpecFetchError::RateLimited(RetryDelay::Unknown)),
        "rate_limited_known" => {
            let seconds = SpanSeconds::new(input.next_u64()?);
            let nanos = Nanosecond::new(input.next_u32()?);
            let span = TimeSpan::try_new(seconds, nanos).map_err(invalid)?;
            Ok(SpecFetchError::RateLimited(RetryDelay::Known(span)))
        }
        "access_denied" => Ok(SpecFetchError::AccessDenied),
        "invalid_response" => Ok(SpecFetchError::InvalidResponse),
        "response_too_large" => Ok(SpecFetchError::ResponseTooLarge),
        _ => Err(invalid(())),
    }
}

fn put_status(out: &mut Encoder, status: &SpecDriftStatus) {
    match status {
        SpecDriftStatus::Aligned => out.push("aligned"),
        SpecDriftStatus::DriftDetected { reasons, review } => {
            out.push("drift");
            out.push_usize(reasons.len());
            for reason in reasons {
                out.push(match reason {
                    SpecChangeReason::NormalizedHash => "normalized_hash",
                    SpecChangeReason::VersionsCatalog => "versions_catalog",
                    SpecChangeReason::LastModified => "last_modified",
                });
            }
            out.push(match review {
                SchemaReviewState::NotRequired => "not_required",
                SchemaReviewState::PendingTypeDiffReview => "pending_type_diff_review",
            });
        }
        SpecDriftStatus::CheckFailed(error) => {
            out.push("check_failed");
            put_fetch_error(out, *error);
        }
    }
}

fn take_status(input: &mut Decoder<'_>) -> rusqlite::Result<SpecDriftStatus> {
    match input.next()? {
        "aligned" => Ok(SpecDriftStatus::Aligned),
        "drift" => {
            let count = input.next_usize()?;
            let mut reasons = Vec::new();
            for _ in 0..count {
                reasons.push(match input.next()? {
                    "normalized_hash" => SpecChangeReason::NormalizedHash,
                    "versions_catalog" => SpecChangeReason::VersionsCatalog,
                    "last_modified" => SpecChangeReason::LastModified,
                    _ => return Err(invalid(())),
                });
            }
            let review = match input.next()? {
                "not_required" => SchemaReviewState::NotRequired,
                "pending_type_diff_review" => SchemaReviewState::PendingTypeDiffReview,
                _ => return Err(invalid(())),
            };
            Ok(SpecDriftStatus::DriftDetected { reasons, review })
        }
        "check_failed" => Ok(SpecDriftStatus::CheckFailed(take_fetch_error(input)?)),
        _ => Err(invalid(())),
    }
}

fn encode_spec_record(record: &SpecStatusRecord) -> String {
    let mut out = Encoder::new();
    put_timestamp(&mut out, record.checked_at());
    put_baseline(&mut out, record.baseline());
    match record.observed() {
        Some(observed) => {
            out.push("some");
            put_snapshot(&mut out, observed.spec());
            put_snapshot(&mut out, observed.versions());
        }
        None => out.push("none"),
    }
    put_status(&mut out, record.status());
    out.finish()
}

fn decode_spec_record(payload: &str) -> rusqlite::Result<SpecStatusRecord> {
    let mut input = Decoder::new(payload);
    let checked_at = take_timestamp(&mut input)?;
    let baseline = take_baseline(&mut input)?;
    let observed = match input.next()? {
        "some" => {
            let spec = take_snapshot(&mut input)?;
            let versions = take_snapshot(&mut input)?;
            Some(TossSpecObservation::new(spec, versions))
        }
        "none" => None,
        _ => return Err(invalid(())),
    };
    let status = take_status(&mut input)?;
    input.finish()?;
    Ok(SpecStatusRecord::new(
        checked_at, baseline, observed, status,
    ))
}

#[derive(Debug)]
pub struct SpecStatusStore<'database> {
    database: &'database Database,
}

impl<'database> SpecStatusStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl TossSpecStatusRepository for SpecStatusStore<'_> {
    fn record(
        &self,
        status: &SpecStatusRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let payload = encode_spec_record(status);
        self.database.write(move |transaction| {
            match transaction.execute(sql::INSERT_SPEC, [payload])? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }

    fn latest(
        &self,
    ) -> impl Future<Output = Result<Option<SpecStatusRecord>, SystemPortError>> + Send {
        self.database.read(|connection| {
            connection
                .query_row(sql::LATEST_SPEC, [], |row| row.get::<_, String>(0))
                .optional()?
                .map(|payload| decode_spec_record(&payload))
                .transpose()
        })
    }
}

fn encode_news_settings(settings: &NewsDataSettings) -> String {
    let mut out = Encoder::new();
    let disabled: Vec<DataSource> = DataSource::all()
        .into_iter()
        .filter(|source| settings.source_switch(*source) == SourceSwitch::Off)
        .collect();
    out.push_usize(disabled.len());
    for source in disabled {
        out.push(source_name(source));
    }
    let enabled: Vec<&NewsSource> = NEWS_SOURCES
        .iter()
        .filter(|source| settings.body_read().toggle_for(source) == AiBodyReadToggle::Enabled)
        .collect();
    out.push_usize(enabled.len());
    for source in enabled {
        put_news_source(&mut out, source);
    }
    put_optional_text(
        &mut out,
        settings.sec_contact().map(SecContactSetting::as_str),
    );
    let cadences: Vec<(CollectionTarget, CollectionMinutes)> = CollectionTarget::ALL
        .into_iter()
        .filter_map(|target| {
            settings
                .cadence_override(target)
                .map(|minutes| (target, minutes))
        })
        .collect();
    out.push_usize(cadences.len());
    for (target, minutes) in cadences {
        out.push(target_name(target));
        out.push_u32(u32::from(*minutes.as_u16()));
    }
    let wake = settings.wake();
    out.push_u8(*wake.daily_move.as_u8());
    out.push_u8(*wake.news_per_hour.as_u8());
    out.push_usize(settings.aliases().len());
    for alias in settings.aliases() {
        out.push(alias.instrument.as_str());
        out.push(alias.alias.as_str());
    }
    out.finish()
}

fn apply(settings: &mut NewsDataSettings, change: NewsDataSettingsChange) -> rusqlite::Result<()> {
    settings.apply(change).map_err(invalid)
}

fn decode_news_settings(payload: &str) -> rusqlite::Result<NewsDataSettings> {
    let mut input = Decoder::new(payload);
    let mut settings = NewsDataSettings::initial();
    for _ in 0..input.next_usize()? {
        let source = parse_source(input.next()?)?;
        apply(
            &mut settings,
            NewsDataSettingsChange::Source(source, SourceSwitch::Off),
        )?;
    }
    for _ in 0..input.next_usize()? {
        let source = take_news_source(&mut input)?;
        apply(
            &mut settings,
            NewsDataSettingsChange::BodyRead(source, AiBodyReadToggle::Enabled),
        )?;
    }
    let contact = take_optional_text(&mut input)?
        .map(|raw| SecContactSetting::try_new(raw).map_err(invalid))
        .transpose()?;
    apply(&mut settings, NewsDataSettingsChange::SecContact(contact))?;
    for _ in 0..input.next_usize()? {
        let target = parse_target(input.next()?)?;
        let raw = u16::try_from(input.next_u32()?).map_err(invalid)?;
        let minutes = CollectionMinutes::try_new(raw).map_err(invalid)?;
        apply(
            &mut settings,
            NewsDataSettingsChange::Cadence(target, Some(minutes)),
        )?;
    }
    let wake = WakeThresholds {
        daily_move: MovePercent::try_new(input.next_u8()?).map_err(invalid)?,
        news_per_hour: NewsBurstCount::try_new(input.next_u8()?).map_err(invalid)?,
    };
    apply(&mut settings, NewsDataSettingsChange::Wake(wake))?;
    for _ in 0..input.next_usize()? {
        let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
        let alias = AliasName::try_new(input.next()?).map_err(invalid)?;
        apply(
            &mut settings,
            NewsDataSettingsChange::AddAlias(StockAlias { instrument, alias }),
        )?;
    }
    input.finish()?;
    Ok(settings)
}

fn upgrade_news(version: i64, payload: String) -> rusqlite::Result<String> {
    match version {
        NEWS_VERSION => Ok(payload),
        _ => Err(invalid(())),
    }
}

#[derive(Debug)]
pub struct NewsDataSettingsStore<'database> {
    database: &'database Database,
}

impl<'database> NewsDataSettingsStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl NewsDataSettingsRepository for NewsDataSettingsStore<'_> {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send {
        self.database.read(|connection| {
            connection
                .query_row(sql::FIND_NEWS, [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .optional()?
                .map(|(version, payload)| decode_news_settings(&upgrade_news(version, payload)?))
                .transpose()
        })
    }

    fn store(
        &self,
        settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let payload = encode_news_settings(settings);
        self.database.write(move |transaction| {
            match transaction.execute(sql::UPSERT_NEWS, rusqlite::params![NEWS_VERSION, payload])? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }
}
