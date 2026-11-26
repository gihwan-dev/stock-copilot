use application::resolve_estimates::ResolutionDataIssue;
use application::{
    AccessDenial, AiRunId, BrokerCapability, HttpStatus, MasterNotReady, MissingData, PortError,
    PortErrorKind, ReadEndpoint, ReasoningEffort, RequestRejection, ResponseMismatch, RetryDelay,
    SystemFailure, SystemPort, SystemPortError, UnavailableCause,
};
use domain::{
    AdjustmentBasis, AiProbability, AiProbabilityInput, CalendarDate, CalendarYear, Calibration,
    CalibrationError, CalibrationEstimateId, CalibrationFact, CalibrationInstant,
    CalibrationInterval, CalibrationKey, CalibrationModel, CalibrationOutcome, CandlePeriod,
    CostEligibleTask, Currency, DailyVolatility, DartReceiptNo, DayNumber, Disclosure,
    EstimateMetadata, EstimateNarrative, EstimateOutcome, EstimatePrompt, EstimateTask, Evidence,
    EvidenceBundleId, EvidenceRef, EvidenceUrl, Horizon, HorizonSelection, InstrumentId,
    KindReceiptNo, KnownMarket, ModelIdentityPart, Money, MonthNumber, Nanosecond, NewsSource,
    NormalizedUrl, NumericInput, OffsetSeconds, OutcomeFailure, OutcomeUnavailable, Price,
    PromptHash, Quantity, Ratio, ReceiptNo, RiskAppetite, SavedCalibrationFact,
    SavedCostEligibleTask, SavedEstimateOutcome, SavedEstimateTask, SavedOutcomeAdjustment,
    SavedOutcomeEvidence, SavedVolatility, ScorecardPromptId, SecEdgarReceiptNo, ShareCount,
    ShareRatio, SignalSubmission, SignalTaskKey, SpanSeconds, Symbol, TimeInterval, TimeSpan,
    Timestamp, ToolReference, UnixSeconds, UntrustedText, Volatility, VolatilitySamples,
};

pub(crate) fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

pub(crate) struct Encoder(String);

impl Encoder {
    pub(crate) fn new() -> Self {
        Self(String::new())
    }
    pub(crate) fn push(&mut self, value: &str) {
        self.0.push_str(&value.len().to_string());
        self.0.push(':');
        self.0.push_str(value);
    }
    pub(crate) fn push_usize(&mut self, value: usize) {
        self.push(&value.to_string());
    }
    pub(crate) fn push_i64(&mut self, value: i64) {
        self.push(&value.to_string());
    }
    pub(crate) fn push_u64(&mut self, value: u64) {
        self.push(&value.to_string());
    }
    pub(crate) fn push_u32(&mut self, value: u32) {
        self.push(&value.to_string());
    }
    pub(crate) fn push_i32(&mut self, value: i32) {
        self.push(&value.to_string());
    }
    pub(crate) fn push_u8(&mut self, value: u8) {
        self.push(&value.to_string());
    }
    pub(crate) fn finish(self) -> String {
        self.0
    }
}

pub(crate) struct Decoder<'input>(&'input str);

impl<'input> Decoder<'input> {
    pub(crate) const fn new(raw: &'input str) -> Self {
        Self(raw)
    }
    pub(crate) fn next(&mut self) -> rusqlite::Result<&'input str> {
        let (length, rest) = self.0.split_once(':').ok_or_else(|| invalid(()))?;
        let size = length.parse::<usize>().map_err(invalid)?;
        let value = rest.get(..size).ok_or_else(|| invalid(()))?;
        self.0 = rest.get(size..).ok_or_else(|| invalid(()))?;
        Ok(value)
    }
    pub(crate) fn next_usize(&mut self) -> rusqlite::Result<usize> {
        self.next()?.parse::<usize>().map_err(invalid)
    }
    pub(crate) fn next_i64(&mut self) -> rusqlite::Result<i64> {
        self.next()?.parse::<i64>().map_err(invalid)
    }
    pub(crate) fn next_u64(&mut self) -> rusqlite::Result<u64> {
        self.next()?.parse::<u64>().map_err(invalid)
    }
    pub(crate) fn next_u32(&mut self) -> rusqlite::Result<u32> {
        self.next()?.parse::<u32>().map_err(invalid)
    }
    pub(crate) fn next_i32(&mut self) -> rusqlite::Result<i32> {
        self.next()?.parse::<i32>().map_err(invalid)
    }
    pub(crate) fn next_u8(&mut self) -> rusqlite::Result<u8> {
        self.next()?.parse::<u8>().map_err(invalid)
    }
    pub(crate) fn finish(self) -> rusqlite::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid(()))
        }
    }
}

pub(crate) fn put_currency(out: &mut Encoder, currency: Currency) {
    out.push(match currency {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    });
}
pub(crate) fn take_currency(input: &mut Decoder<'_>) -> rusqlite::Result<Currency> {
    match input.next()? {
        "KRW" => Ok(Currency::Krw),
        "USD" => Ok(Currency::Usd),
        _ => Err(invalid(())),
    }
}
pub(crate) fn put_known_market(out: &mut Encoder, market: KnownMarket) {
    out.push(match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    });
}
pub(crate) fn take_known_market(input: &mut Decoder<'_>) -> rusqlite::Result<KnownMarket> {
    match input.next()? {
        "KR" => Ok(KnownMarket::Kr),
        "US" => Ok(KnownMarket::Us),
        _ => Err(invalid(())),
    }
}
pub(crate) fn put_timestamp(out: &mut Encoder, value: Timestamp) {
    out.push_i64(*value.utc_seconds().as_i64());
    out.push_u32(*value.nanosecond().as_u32());
    out.push_i32(*value.offset_seconds().as_i32());
}
pub(crate) fn take_timestamp(input: &mut Decoder<'_>) -> rusqlite::Result<Timestamp> {
    let seconds = UnixSeconds::new(input.next_i64()?);
    let nanos = Nanosecond::new(input.next_u32()?);
    let offset = OffsetSeconds::new(input.next_i32()?);
    Timestamp::from_parts(seconds, nanos, offset).map_err(invalid)
}
pub(crate) fn put_date(out: &mut Encoder, value: CalendarDate) {
    out.push_i32(*value.year().as_i32());
    out.push_u8(*value.month().as_u8());
    out.push_u8(*value.day().as_u8());
}
pub(crate) fn take_date(input: &mut Decoder<'_>) -> rusqlite::Result<CalendarDate> {
    let year = CalendarYear::new(input.next_i32()?);
    let month = MonthNumber::new(input.next_u8()?);
    let day = DayNumber::new(input.next_u8()?);
    CalendarDate::from_parts(year, month, day).map_err(invalid)
}
pub(crate) fn put_interval(out: &mut Encoder, value: TimeInterval) {
    put_timestamp(out, value.start());
    put_timestamp(out, value.end());
}
pub(crate) fn take_interval(input: &mut Decoder<'_>) -> rusqlite::Result<TimeInterval> {
    let start = take_timestamp(input)?;
    let end = take_timestamp(input)?;
    TimeInterval::try_new(start, end).map_err(invalid)
}
pub(crate) fn put_money(out: &mut Encoder, value: Money) {
    put_currency(out, value.currency());
    out.push(&value.amount().as_decimal().to_string());
}
pub(crate) fn take_money(input: &mut Decoder<'_>) -> rusqlite::Result<Money> {
    let currency = take_currency(input)?;
    let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
    Money::parse(&numeric, currency).map_err(invalid)
}
pub(crate) fn put_price(out: &mut Encoder, value: Price) {
    put_currency(out, value.as_money().currency());
    out.push(&value.as_money().amount().as_decimal().to_string());
}
pub(crate) fn take_price(input: &mut Decoder<'_>) -> rusqlite::Result<Price> {
    let currency = take_currency(input)?;
    let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
    Price::parse(&numeric, currency).map_err(invalid)
}
pub(crate) fn put_quantity(out: &mut Encoder, value: Quantity) {
    out.push(&value.as_decimal().to_string());
}
pub(crate) fn take_quantity(input: &mut Decoder<'_>) -> rusqlite::Result<Quantity> {
    let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
    Quantity::parse(&numeric).map_err(invalid)
}
pub(crate) fn put_ratio(out: &mut Encoder, value: Ratio) {
    out.push(&value.as_decimal().to_string());
}
pub(crate) fn take_ratio(input: &mut Decoder<'_>) -> rusqlite::Result<Ratio> {
    let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
    Ratio::parse(&numeric).map_err(invalid)
}
pub(crate) fn put_horizon(out: &mut Encoder, value: Horizon) {
    out.push(match value.selection() {
        HorizonSelection::Hours => "hours",
        HorizonSelection::FewDays => "few_days",
        HorizonSelection::WeeksToMonths => "weeks_to_months",
        HorizonSelection::HalfYearOrMore => "half_year_or_more",
    });
}
pub(crate) fn take_horizon(input: &mut Decoder<'_>) -> rusqlite::Result<Horizon> {
    let selection = match input.next()? {
        "hours" => HorizonSelection::Hours,
        "few_days" => HorizonSelection::FewDays,
        "weeks_to_months" => HorizonSelection::WeeksToMonths,
        "half_year_or_more" => HorizonSelection::HalfYearOrMore,
        _ => return Err(invalid(())),
    };
    Ok(Horizon::new(selection))
}
pub(crate) fn put_appetite(out: &mut Encoder, value: RiskAppetite) {
    out.push(match value {
        RiskAppetite::Conservative => "conservative",
        RiskAppetite::Neutral => "neutral",
        RiskAppetite::Aggressive => "aggressive",
    });
}
pub(crate) fn take_appetite(input: &mut Decoder<'_>) -> rusqlite::Result<RiskAppetite> {
    match input.next()? {
        "conservative" => Ok(RiskAppetite::Conservative),
        "neutral" => Ok(RiskAppetite::Neutral),
        "aggressive" => Ok(RiskAppetite::Aggressive),
        _ => Err(invalid(())),
    }
}
fn put_volatility(out: &mut Encoder, value: &Volatility) {
    let saved = value.saved_state();
    put_ratio(out, saved.daily.as_ratio());
    out.push(saved.instrument.as_str());
    put_date(out, saved.as_of);
    put_timestamp(out, *saved.adjustment_basis.as_timestamp());
    out.push_usize(*saved.samples.as_usize());
}
fn take_volatility(input: &mut Decoder<'_>) -> rusqlite::Result<Volatility> {
    let daily = DailyVolatility::try_new(take_ratio(input)?).map_err(invalid)?;
    let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let as_of = take_date(input)?;
    let adjustment_basis = AdjustmentBasis::new(take_timestamp(input)?);
    let samples = VolatilitySamples::try_new(input.next_usize()?).map_err(invalid)?;
    Volatility::restore_from_saved_state(SavedVolatility {
        daily,
        instrument,
        as_of,
        adjustment_basis,
        samples,
    })
    .map_err(invalid)
}
pub(super) fn put_estimate_task(out: &mut Encoder, value: &EstimateTask) {
    let saved = value.saved_state();
    out.push(saved.symbol.as_str());
    put_known_market(out, saved.market);
    put_price(out, saved.reference_price);
    put_timestamp(out, saved.reference_at);
    put_timestamp(out, saved.created_at);
    put_volatility(out, &saved.volatility);
    put_horizon(out, saved.horizon);
    put_appetite(out, saved.appetite);
    put_ratio(out, saved.target);
    put_ratio(out, saved.stop);
    put_ratio(out, saved.base_probability);
}
pub(super) fn take_saved_estimate_task(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<SavedEstimateTask> {
    let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
    let market = take_known_market(input)?;
    let reference_price = take_price(input)?;
    let reference_at = take_timestamp(input)?;
    let created_at = take_timestamp(input)?;
    let volatility = take_volatility(input)?;
    let horizon = take_horizon(input)?;
    let appetite = take_appetite(input)?;
    let target = take_ratio(input)?;
    let stop = take_ratio(input)?;
    let base_probability = take_ratio(input)?;
    Ok(SavedEstimateTask {
        symbol,
        market,
        reference_price,
        reference_at,
        created_at,
        volatility,
        horizon,
        appetite,
        target,
        stop,
        base_probability,
    })
}
pub(super) fn encode_task_key(task: &SignalTaskKey) -> String {
    let mut out = Encoder::new();
    out.push("task-key-v1");
    put_estimate_task(&mut out, task.task());
    out.finish()
}
pub(super) fn decode_task_key(payload: &str) -> rusqlite::Result<SignalTaskKey> {
    let mut input = Decoder::new(payload);
    if input.next()? != "task-key-v1" {
        return Err(invalid(()));
    }
    let saved = take_saved_estimate_task(&mut input)?;
    input.finish()?;
    SignalTaskKey::restore_from_saved_state(saved).map_err(invalid)
}
pub(super) fn encode_cost_eligible_task(task: &CostEligibleTask) -> String {
    let saved = task.saved_state();
    let mut out = Encoder::new();
    out.push("eligible-task-v1");
    put_estimate_task(&mut out, task.task());
    put_ratio(&mut out, saved.cost_delta);
    out.finish()
}
pub(super) fn decode_cost_eligible_task(payload: &str) -> rusqlite::Result<CostEligibleTask> {
    let mut input = Decoder::new(payload);
    if input.next()? != "eligible-task-v1" {
        return Err(invalid(()));
    }
    let saved_task = take_saved_estimate_task(&mut input)?;
    let cost_delta = take_ratio(&mut input)?;
    input.finish()?;
    CostEligibleTask::restore_from_saved_state(SavedCostEligibleTask {
        task: saved_task,
        cost_delta,
    })
    .map_err(invalid)
}
pub(crate) fn put_calibration_key(out: &mut Encoder, key: &CalibrationKey) {
    out.push(key.model().provider().as_str());
    out.push(key.model().name().as_str());
    out.push(key.model().version().as_str());
    put_horizon(out, key.horizon());
}
pub(crate) fn take_calibration_key(input: &mut Decoder<'_>) -> rusqlite::Result<CalibrationKey> {
    let provider = ModelIdentityPart::try_new(input.next()?).map_err(invalid)?;
    let name = ModelIdentityPart::try_new(input.next()?).map_err(invalid)?;
    let version = ModelIdentityPart::try_new(input.next()?).map_err(invalid)?;
    let horizon = take_horizon(input)?;
    Ok(CalibrationKey::new(
        CalibrationModel::new(provider, name, version),
        horizon,
    ))
}
pub(crate) fn encode_calibration_key(key: &CalibrationKey) -> String {
    let mut out = Encoder::new();
    out.push("cal-key-v1");
    put_calibration_key(&mut out, key);
    out.finish()
}
pub(super) fn decode_calibration_key(payload: &str) -> rusqlite::Result<CalibrationKey> {
    let mut input = Decoder::new(payload);
    if input.next()? != "cal-key-v1" {
        return Err(invalid(()));
    }
    let key = take_calibration_key(&mut input)?;
    input.finish()?;
    Ok(key)
}
pub(crate) fn put_ai_probability(out: &mut Encoder, value: AiProbability) {
    out.push(
        &value
            .into_probability()
            .as_decimal()
            .mantissa()
            .unsigned_abs()
            .to_string(),
    );
}
pub(crate) fn take_ai_probability(input: &mut Decoder<'_>) -> rusqlite::Result<AiProbability> {
    let parsed = AiProbabilityInput::try_new(input.next()?).map_err(invalid)?;
    AiProbability::parse(&parsed).map_err(invalid)
}
pub(crate) fn put_news_source(out: &mut Encoder, source: &NewsSource) {
    out.push(match source {
        NewsSource::Hankyung => "hankyung",
        NewsSource::Maekyung => "maekyung",
        NewsSource::Yonhap => "yonhap",
        NewsSource::Edaily => "edaily",
        NewsSource::SegyeFinance => "segye_finance",
        NewsSource::BankOfKorea => "bank_of_korea",
        NewsSource::FederalReserve => "federal_reserve",
        NewsSource::GoogleNews => "google_news",
        NewsSource::YahooFinance => "yahoo_finance",
    });
}
pub(crate) fn take_news_source(input: &mut Decoder<'_>) -> rusqlite::Result<NewsSource> {
    match input.next()? {
        "hankyung" => Ok(NewsSource::Hankyung),
        "maekyung" => Ok(NewsSource::Maekyung),
        "yonhap" => Ok(NewsSource::Yonhap),
        "edaily" => Ok(NewsSource::Edaily),
        "segye_finance" => Ok(NewsSource::SegyeFinance),
        "bank_of_korea" => Ok(NewsSource::BankOfKorea),
        "federal_reserve" => Ok(NewsSource::FederalReserve),
        "google_news" => Ok(NewsSource::GoogleNews),
        "yahoo_finance" => Ok(NewsSource::YahooFinance),
        _ => Err(invalid(())),
    }
}
pub(crate) fn put_evidence(out: &mut Encoder, evidence: &Evidence) {
    out.push_usize(evidence.as_items().len());
    for item in evidence.as_items() {
        match item {
            EvidenceRef::Web(url) => {
                out.push("web");
                out.push(url.as_url().as_str());
            }
            EvidenceRef::News(source, url) => {
                out.push("news");
                put_news_source(out, source);
                out.push(url.as_url().as_str());
            }
            EvidenceRef::Disclosure(key, url) => {
                out.push("disclosure");
                match key.receipt_no() {
                    ReceiptNo::Dart(value) => {
                        out.push("dart");
                        out.push(value.as_str());
                    }
                    ReceiptNo::Kind(value) => {
                        out.push("kind");
                        out.push(value.as_str());
                    }
                    ReceiptNo::SecEdgar(value) => {
                        out.push("sec");
                        out.push(value.as_str());
                    }
                }
                out.push(url.as_url().as_str());
            }
            EvidenceRef::Tool(value) => {
                out.push("tool");
                out.push(value.as_str());
            }
        }
    }
}
pub(crate) fn take_evidence(input: &mut Decoder<'_>) -> rusqlite::Result<Evidence> {
    let count = input.next_usize()?;
    if !(1..=8).contains(&count) {
        return Err(invalid(()));
    }
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let entry = match input.next()? {
            "web" => {
                let normalized = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
                EvidenceRef::Web(EvidenceUrl::try_new(normalized).map_err(invalid)?)
            }
            "news" => {
                let source = take_news_source(input)?;
                let normalized = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
                EvidenceRef::News(source, EvidenceUrl::try_new(normalized).map_err(invalid)?)
            }
            "tool" => EvidenceRef::Tool(ToolReference::try_new(input.next()?).map_err(invalid)?),
            "disclosure" => {
                let receipt = match input.next()? {
                    "dart" => {
                        ReceiptNo::Dart(DartReceiptNo::try_new(input.next()?).map_err(invalid)?)
                    }
                    "kind" => {
                        ReceiptNo::Kind(KindReceiptNo::try_new(input.next()?).map_err(invalid)?)
                    }
                    "sec" => ReceiptNo::SecEdgar(
                        SecEdgarReceiptNo::try_new(input.next()?).map_err(invalid)?,
                    ),
                    _ => return Err(invalid(())),
                };
                let normalized = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
                let url = EvidenceUrl::try_new(normalized).map_err(invalid)?;
                let disclosure = Disclosure::new(
                    receipt,
                    url.as_url().clone(),
                    UntrustedText::try_new("").map_err(invalid)?,
                    None,
                );
                EvidenceRef::Disclosure(disclosure.key().clone(), url)
            }
            _ => return Err(invalid(())),
        };
        items.push(entry);
    }
    Evidence::try_new(items).map_err(invalid)
}
pub(super) fn put_signal_submission(out: &mut Encoder, value: &SignalSubmission) {
    put_estimate_task(out, value.task_key().task());
    put_calibration_key(out, value.calibration_key());
    put_ai_probability(out, value.probability());
    match value.evidence() {
        Some(evidence) => {
            out.push("some");
            put_evidence(out, evidence);
        }
        None => out.push("none"),
    }
    put_timestamp(out, value.submitted_at());
}
pub(super) fn take_signal_submission(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<SignalSubmission> {
    let task_key = SignalTaskKey::restore_from_saved_state(take_saved_estimate_task(input)?)
        .map_err(invalid)?;
    let cal_key = take_calibration_key(input)?;
    let probability = take_ai_probability(input)?;
    let evidence = match input.next()? {
        "some" => Some(take_evidence(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let submitted_at = take_timestamp(input)?;
    SignalSubmission::create(task_key, cal_key, probability, evidence, submitted_at)
        .map_err(invalid)
}
pub(super) fn encode_signal_submission(value: &SignalSubmission) -> String {
    let mut out = Encoder::new();
    out.push("submission-v1");
    put_signal_submission(&mut out, value);
    out.finish()
}
pub(super) fn decode_signal_submission(payload: &str) -> rusqlite::Result<SignalSubmission> {
    let mut input = Decoder::new(payload);
    if input.next()? != "submission-v1" {
        return Err(invalid(()));
    }
    let value = take_signal_submission(&mut input)?;
    input.finish()?;
    Ok(value)
}
fn put_effort(out: &mut Encoder, value: ReasoningEffort) {
    out.push(match value {
        ReasoningEffort::Default => "default",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    });
}
fn take_effort(input: &mut Decoder<'_>) -> rusqlite::Result<ReasoningEffort> {
    match input.next()? {
        "default" => Ok(ReasoningEffort::Default),
        "low" => Ok(ReasoningEffort::Low),
        "medium" => Ok(ReasoningEffort::Medium),
        "high" => Ok(ReasoningEffort::High),
        "xhigh" => Ok(ReasoningEffort::XHigh),
        "max" => Ok(ReasoningEffort::Max),
        _ => Err(invalid(())),
    }
}
pub(super) fn encode_submission_record_body(
    submission: &SignalSubmission,
    narrative: &EstimateNarrative,
    metadata: &EstimateMetadata<ReasoningEffort>,
) -> String {
    let mut out = Encoder::new();
    out.push("submission-record-v1");
    put_signal_submission(&mut out, submission);
    out.push(narrative.rationale.as_untrusted_str());
    out.push(narrative.counterargument.as_untrusted_str());
    put_effort(&mut out, metadata.effort);
    out.push(metadata.cli_version.as_str());
    out.push(metadata.prompt.id.as_str());
    out.push(metadata.prompt.version.as_str());
    out.push(metadata.prompt.hash.as_str());
    out.push(metadata.evidence_bundle.as_str());
    out.finish()
}
pub(super) fn decode_submission_record_body(
    payload: &str,
) -> rusqlite::Result<(
    SignalSubmission,
    EstimateNarrative,
    EstimateMetadata<ReasoningEffort>,
)> {
    let mut input = Decoder::new(payload);
    if input.next()? != "submission-record-v1" {
        return Err(invalid(()));
    }
    let submission = take_signal_submission(&mut input)?;
    let rationale = UntrustedText::try_new(input.next()?).map_err(invalid)?;
    let counterargument = UntrustedText::try_new(input.next()?).map_err(invalid)?;
    let effort = take_effort(&mut input)?;
    let cli_version = ModelIdentityPart::try_new(input.next()?).map_err(invalid)?;
    let prompt_id = ScorecardPromptId::try_new(input.next()?).map_err(invalid)?;
    let prompt_version = ModelIdentityPart::try_new(input.next()?).map_err(invalid)?;
    let prompt_hash = PromptHash::try_new(input.next()?).map_err(invalid)?;
    let evidence_bundle = EvidenceBundleId::try_new(input.next()?).map_err(invalid)?;
    input.finish()?;
    Ok((
        submission,
        EstimateNarrative {
            rationale,
            counterargument,
        },
        EstimateMetadata {
            effort,
            cli_version,
            prompt: EstimatePrompt {
                id: prompt_id,
                version: prompt_version,
                hash: prompt_hash,
            },
            evidence_bundle,
        },
    ))
}
fn put_outcome_evidence(out: &mut Encoder, value: &SavedOutcomeEvidence) {
    out.push(value.instrument.as_str());
    put_interval(out, value.interval);
    put_horizon(out, value.horizon);
    put_timestamp(out, value.at);
    match value.period {
        None => out.push("none"),
        Some(CandlePeriod::Daily) => out.push("daily"),
        Some(CandlePeriod::Minute) => out.push("minute"),
    }
    out.push(value.adjustment.instrument.as_str());
    put_timestamp(out, value.adjustment.reference_at);
    put_timestamp(out, *value.adjustment.basis.as_timestamp());
    out.push_usize(value.adjustment.ratios.len());
    for ratio in &value.adjustment.ratios {
        out.push_u64(*ratio.old_shares().as_u64());
        out.push_u64(*ratio.new_shares().as_u64());
    }
}
fn take_outcome_evidence(input: &mut Decoder<'_>) -> rusqlite::Result<SavedOutcomeEvidence> {
    let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let interval = take_interval(input)?;
    let horizon = take_horizon(input)?;
    let at = take_timestamp(input)?;
    let period = match input.next()? {
        "none" => None,
        "daily" => Some(CandlePeriod::Daily),
        "minute" => Some(CandlePeriod::Minute),
        _ => return Err(invalid(())),
    };
    let adj_instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let reference_at = take_timestamp(input)?;
    let basis = AdjustmentBasis::new(take_timestamp(input)?);
    let ratio_count = input.next_usize()?;
    let mut ratios = Vec::with_capacity(ratio_count);
    for _ in 0..ratio_count {
        let old = ShareCount::try_new(input.next_u64()?).map_err(invalid)?;
        let new = ShareCount::try_new(input.next_u64()?).map_err(invalid)?;
        ratios.push(ShareRatio::new(old, new));
    }
    Ok(SavedOutcomeEvidence {
        instrument,
        interval,
        horizon,
        at,
        period,
        adjustment: SavedOutcomeAdjustment {
            instrument: adj_instrument,
            reference_at,
            basis,
            ratios,
        },
    })
}
fn put_outcome_failure(out: &mut Encoder, value: OutcomeFailure) {
    out.push(match value {
        OutcomeFailure::Stop => "stop",
        OutcomeFailure::Expired => "expired",
        OutcomeFailure::Ambiguous => "ambiguous",
    });
}
fn take_outcome_failure(input: &mut Decoder<'_>) -> rusqlite::Result<OutcomeFailure> {
    match input.next()? {
        "stop" => Ok(OutcomeFailure::Stop),
        "expired" => Ok(OutcomeFailure::Expired),
        "ambiguous" => Ok(OutcomeFailure::Ambiguous),
        _ => Err(invalid(())),
    }
}
fn put_outcome_unavailable(out: &mut Encoder, value: OutcomeUnavailable) {
    out.push(match value {
        OutcomeUnavailable::Calendar => "calendar",
        OutcomeUnavailable::Adjustment => "adjustment",
        OutcomeUnavailable::InvalidCandle => "invalid_candle",
        OutcomeUnavailable::ConflictingCandles => "conflicting_candles",
        OutcomeUnavailable::MissingCandles => "missing_candles",
    });
}
fn take_outcome_unavailable(input: &mut Decoder<'_>) -> rusqlite::Result<OutcomeUnavailable> {
    match input.next()? {
        "calendar" => Ok(OutcomeUnavailable::Calendar),
        "adjustment" => Ok(OutcomeUnavailable::Adjustment),
        "invalid_candle" => Ok(OutcomeUnavailable::InvalidCandle),
        "conflicting_candles" => Ok(OutcomeUnavailable::ConflictingCandles),
        "missing_candles" => Ok(OutcomeUnavailable::MissingCandles),
        _ => Err(invalid(())),
    }
}
pub(super) fn encode_outcome(outcome: &EstimateOutcome) -> String {
    let mut out = Encoder::new();
    out.push("outcome-v1");
    match outcome.saved_state() {
        SavedEstimateOutcome::Success(evidence) => {
            out.push("success");
            put_outcome_evidence(&mut out, &evidence);
        }
        SavedEstimateOutcome::Failure { evidence, reason } => {
            out.push("failure");
            put_outcome_failure(&mut out, reason);
            put_outcome_evidence(&mut out, &evidence);
        }
        SavedEstimateOutcome::Ongoing(evidence) => {
            out.push("ongoing");
            put_outcome_evidence(&mut out, &evidence);
        }
        SavedEstimateOutcome::Unavailable(reason) => {
            out.push("unavailable");
            put_outcome_unavailable(&mut out, reason);
        }
    }
    out.finish()
}
pub(crate) fn decode_outcome(payload: &str) -> rusqlite::Result<EstimateOutcome> {
    let mut input = Decoder::new(payload);
    if input.next()? != "outcome-v1" {
        return Err(invalid(()));
    }
    let saved = match input.next()? {
        "success" => SavedEstimateOutcome::Success(take_outcome_evidence(&mut input)?),
        "failure" => {
            let reason = take_outcome_failure(&mut input)?;
            let evidence = take_outcome_evidence(&mut input)?;
            SavedEstimateOutcome::Failure { evidence, reason }
        }
        "ongoing" => SavedEstimateOutcome::Ongoing(take_outcome_evidence(&mut input)?),
        "unavailable" => SavedEstimateOutcome::Unavailable(take_outcome_unavailable(&mut input)?),
        _ => return Err(invalid(())),
    };
    input.finish()?;
    EstimateOutcome::restore_from_saved_state(saved).map_err(invalid)
}
pub(crate) fn encode_calibration(calibration: &Calibration) -> String {
    let mut out = Encoder::new();
    out.push("calibration-v1");
    put_calibration_key(&mut out, calibration.key());
    out.push_usize(calibration.facts().len());
    for fact in calibration.facts() {
        let est = fact.estimate();
        out.push(est.id().as_str());
        out.push(est.instrument().as_str());
        put_calibration_key(&mut out, est.key());
        out.push_i64(*est.interval().start().utc_seconds().as_i64());
        out.push_u32(*est.interval().start().nanosecond().as_u32());
        out.push_i64(*est.interval().end().utc_seconds().as_i64());
        out.push_u32(*est.interval().end().nanosecond().as_u32());
        put_ai_probability(&mut out, est.predicted_probability());
        out.push(match fact.outcome() {
            CalibrationOutcome::Miss => "miss",
            CalibrationOutcome::Hit => "hit",
        });
    }
    out.finish()
}
pub(crate) fn decode_calibration(payload: &str) -> rusqlite::Result<Calibration> {
    let mut input = Decoder::new(payload);
    if input.next()? != "calibration-v1" {
        return Err(invalid(()));
    }
    let key = take_calibration_key(&mut input)?;
    let count = input.next_usize()?;
    if count > 65_536 {
        return Err(invalid(()));
    }
    let mut facts = Vec::with_capacity(count);
    for _ in 0..count {
        let id = CalibrationEstimateId::try_new(input.next()?).map_err(invalid)?;
        let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
        let fact_key = take_calibration_key(&mut input)?;
        let start = CalibrationInstant::from_parts(
            UnixSeconds::new(input.next_i64()?),
            Nanosecond::new(input.next_u32()?),
        )
        .map_err(invalid)?;
        let end = CalibrationInstant::from_parts(
            UnixSeconds::new(input.next_i64()?),
            Nanosecond::new(input.next_u32()?),
        )
        .map_err(invalid)?;
        let interval = CalibrationInterval::try_new(start, end).map_err(invalid)?;
        let predicted_probability = take_ai_probability(&mut input)?;
        let outcome = match input.next()? {
            "miss" => CalibrationOutcome::Miss,
            "hit" => CalibrationOutcome::Hit,
            _ => return Err(invalid(())),
        };
        facts.push(CalibrationFact::restore_from_saved_state(
            SavedCalibrationFact {
                id,
                instrument,
                key: fact_key,
                interval,
                predicted_probability,
                outcome,
            },
        ));
    }
    input.finish()?;
    Calibration::try_new(key, &facts).map_err(invalid)
}
pub(super) fn encode_run_id(run: &AiRunId) -> &str {
    run.as_str()
}
pub(super) fn decode_run_id(raw: &str) -> rusqlite::Result<AiRunId> {
    AiRunId::try_new(raw).map_err(invalid)
}
fn put_retry_delay(out: &mut Encoder, retry: RetryDelay) {
    match retry {
        RetryDelay::Unknown => out.push("unknown"),
        RetryDelay::Known(span) => {
            let (seconds, nanos) = span.as_parts();
            out.push("known");
            out.push_u64(*seconds.as_u64());
            out.push_u32(*nanos.as_u32());
        }
    }
}
fn put_system_error(out: &mut Encoder, error: SystemPortError) {
    out.push(match error.port() {
        SystemPort::Repository => "repository",
        SystemPort::AuditLog => "audit_log",
        SystemPort::SecretStore => "secret_store",
        SystemPort::Notifier => "notifier",
    });
    match error.failure() {
        SystemFailure::PermissionDenied => out.push("permission_denied"),
        SystemFailure::Unavailable(retry) => {
            out.push("unavailable");
            put_retry_delay(out, retry);
        }
        SystemFailure::InvalidData => out.push("invalid_data"),
        SystemFailure::RepositoryIntegrity => out.push("repository_integrity"),
        SystemFailure::SecretIntegrity => out.push("secret_integrity"),
    }
}
fn put_read_endpoint(out: &mut Encoder, endpoint: ReadEndpoint) {
    out.push(match endpoint {
        ReadEndpoint::Accounts => "accounts",
        ReadEndpoint::Holdings => "holdings",
        ReadEndpoint::Transactions => "transactions",
        ReadEndpoint::Quotes => "quotes",
        ReadEndpoint::Candles => "candles",
        ReadEndpoint::BuyingPower => "buying_power",
        ReadEndpoint::FxRate => "fx_rate",
        ReadEndpoint::OrderFxRates => "order_fx_rates",
        ReadEndpoint::SellableQuantity => "sellable_quantity",
        ReadEndpoint::OpenOrders => "open_orders",
        ReadEndpoint::ClosedOrders => "closed_orders",
        ReadEndpoint::Order => "order",
        ReadEndpoint::StockWarnings => "stock_warnings",
        ReadEndpoint::Instruments => "instruments",
        ReadEndpoint::Orderbook => "orderbook",
        ReadEndpoint::PriceLimits => "price_limits",
        ReadEndpoint::CommissionRate => "commission_rate",
        ReadEndpoint::IndicatorQuotes => "indicator_quotes",
        ReadEndpoint::IndicatorCandles => "indicator_candles",
        ReadEndpoint::Rankings => "rankings",
        ReadEndpoint::StockInvestorFlow => "stock_investor_flow",
        ReadEndpoint::IndicatorInvestorFlow => "indicator_investor_flow",
        ReadEndpoint::SectorComposition => "sector_composition",
        ReadEndpoint::Calendar => "calendar",
        ReadEndpoint::InvestorTradingRankings => "investor_trading_rankings",
        ReadEndpoint::Sectors => "sectors",
        ReadEndpoint::Sector => "sector",
        ReadEndpoint::SectorRankings => "sector_rankings",
        ReadEndpoint::SectorEtfs => "sector_etfs",
        ReadEndpoint::ProgramTrades => "program_trades",
        ReadEndpoint::ShortSelling => "short_selling",
        ReadEndpoint::CreditTrades => "credit_trades",
        ReadEndpoint::SecuritiesLending => "securities_lending",
    });
}
fn put_http_status(out: &mut Encoder, status: HttpStatus) {
    out.push(match status {
        HttpStatus::NotReceived => "not_received",
        HttpStatus::Informational => "informational",
        HttpStatus::Success => "success",
        HttpStatus::Redirect => "redirect",
        HttpStatus::BadRequest => "bad_request",
        HttpStatus::Unauthorized => "unauthorized",
        HttpStatus::Forbidden => "forbidden",
        HttpStatus::NotFound => "not_found",
        HttpStatus::Conflict => "conflict",
        HttpStatus::UriTooLong => "uri_too_long",
        HttpStatus::UnsupportedMediaType => "unsupported_media_type",
        HttpStatus::Unprocessable => "unprocessable",
        HttpStatus::RateLimited => "rate_limited",
        HttpStatus::OtherClientError => "other_client_error",
        HttpStatus::ServerError => "server_error",
    });
}
fn put_broker_capability(out: &mut Encoder, capability: BrokerCapability) {
    out.push(match capability {
        BrokerCapability::DailyCandles => "daily_candles",
        BrokerCapability::MinuteCandles => "minute_candles",
        BrokerCapability::AdjustedHistory => "adjusted_history",
        BrokerCapability::RawHistory => "raw_history",
        BrokerCapability::SellableQuantity => "sellable_quantity",
        BrokerCapability::OpenOrders => "open_orders",
        BrokerCapability::ClosedOrders => "closed_orders",
        BrokerCapability::Order => "order",
        BrokerCapability::StockWarnings => "stock_warnings",
        BrokerCapability::Instruments => "instruments",
        BrokerCapability::Orderbook => "orderbook",
        BrokerCapability::PriceLimits => "price_limits",
        BrokerCapability::CommissionRate => "commission_rate",
        BrokerCapability::IndicatorQuotes => "indicator_quotes",
        BrokerCapability::IndicatorCandles => "indicator_candles",
        BrokerCapability::Rankings => "rankings",
        BrokerCapability::StockInvestorFlow => "stock_investor_flow",
        BrokerCapability::IndicatorInvestorFlow => "indicator_investor_flow",
        BrokerCapability::SectorComposition => "sector_composition",
        BrokerCapability::Calendar => "calendar",
        BrokerCapability::InvestorTradingRankings => "investor_trading_rankings",
        BrokerCapability::Sectors => "sectors",
        BrokerCapability::Sector => "sector",
        BrokerCapability::SectorRankings => "sector_rankings",
        BrokerCapability::SectorEtfs => "sector_etfs",
        BrokerCapability::ProgramTrades => "program_trades",
        BrokerCapability::ShortSelling => "short_selling",
        BrokerCapability::CreditTrades => "credit_trades",
        BrokerCapability::SecuritiesLending => "securities_lending",
    });
}
fn put_master_not_ready(out: &mut Encoder, cause: MasterNotReady) {
    match cause {
        MasterNotReady::StatusCoverage => out.push("status_coverage"),
        MasterNotReady::IdentityContinuity => out.push("identity_continuity"),
        MasterNotReady::Storage(err) => {
            out.push("storage");
            put_system_error(out, err);
        }
        MasterNotReady::Issuer(err) => {
            out.push("issuer");
            put_system_error(out, err);
        }
        MasterNotReady::MissingInstrument => out.push("missing_instrument"),
    }
}
fn put_request_rejection(out: &mut Encoder, rejection: RequestRejection) {
    out.push(match rejection {
        RequestRejection::MalformedRequest => "malformed_request",
        RequestRejection::MissingAccountHeader => "missing_account_header",
        RequestRejection::InvalidToken => "invalid_token",
        RequestRejection::EdgeBlocked => "edge_blocked",
        RequestRejection::UnsupportedQuery => "unsupported_query",
        RequestRejection::UnsupportedContentType => "unsupported_content_type",
        RequestRejection::BatchLimitExceeded => "batch_limit_exceeded",
    });
}
fn put_port_error_kind(out: &mut Encoder, kind: PortErrorKind) {
    match kind {
        PortErrorKind::MasterNotReady(cause) => {
            out.push("master_not_ready");
            put_master_not_ready(out, cause);
        }
        PortErrorKind::AccessDenied(cause) => {
            out.push("access_denied");
            out.push(match cause {
                AccessDenial::InvalidCredentials => "invalid_credentials",
                AccessDenial::IpNotAllowed => "ip_not_allowed",
                AccessDenial::TokenRevokedRepeated => "token_revoked_repeated",
                AccessDenial::AccountNotFound => "account_not_found",
                AccessDenial::Forbidden => "forbidden",
                AccessDenial::LoginUserNotFound => "login_user_not_found",
            });
        }
        PortErrorKind::Unavailable { cause, retry } => {
            out.push("unavailable");
            out.push(match cause {
                UnavailableCause::RateLimited => "rate_limited",
                UnavailableCause::Maintenance => "maintenance",
                UnavailableCause::ServerError => "server_error",
                UnavailableCause::Network => "network",
                UnavailableCause::ExpiredToken => "expired_token",
            });
            put_retry_delay(out, retry);
        }
        PortErrorKind::NotFound(missing) => {
            out.push("not_found");
            out.push(match missing {
                MissingData::Order => "order",
                MissingData::Stock => "stock",
                MissingData::ExchangeRate => "exchange_rate",
                MissingData::Sector => "sector",
            });
        }
        PortErrorKind::InvalidResponse(mismatch) => {
            out.push("invalid_response");
            out.push(match mismatch {
                ResponseMismatch::Schema => "schema",
                ResponseMismatch::Validation => "validation",
            });
        }
        PortErrorKind::Unsupported(capability) => {
            out.push("unsupported");
            put_broker_capability(out, capability);
        }
        PortErrorKind::InvalidRequest(rejection) => {
            out.push("invalid_request");
            put_request_rejection(out, rejection);
        }
    }
}
fn put_port_error(out: &mut Encoder, error: PortError) {
    put_read_endpoint(out, error.endpoint());
    put_http_status(out, error.status());
    put_port_error_kind(out, error.kind());
}
pub(super) fn encode_resolution_issue(issue: ResolutionDataIssue) -> String {
    let mut out = Encoder::new();
    out.push("issue-v1");
    match issue {
        ResolutionDataIssue::Incomplete(reason) => {
            out.push("incomplete");
            put_outcome_unavailable(&mut out, reason);
        }
        ResolutionDataIssue::Read(error) => {
            out.push("read");
            put_port_error(&mut out, error);
        }
        ResolutionDataIssue::Storage(error) => {
            out.push("storage");
            put_system_error(&mut out, error);
        }
        ResolutionDataIssue::Calibration(error) => {
            out.push("calibration");
            out.push(match error {
                CalibrationError::InvalidModelIdentity => "invalid_model_identity",
                CalibrationError::TooManyFacts => "too_many_facts",
                CalibrationError::InvalidEstimateId => "invalid_estimate_id",
                CalibrationError::ConflictingEstimate => "conflicting_estimate",
                CalibrationError::MismatchedCombination => "mismatched_combination",
                CalibrationError::MissingCalibration => "missing_calibration",
                CalibrationError::StaleCalibration => "stale_calibration",
            });
        }
    }
    let _check_span = TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(0));
    out.finish()
}
