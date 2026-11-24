use crate::estimates::codec::{Decoder, Encoder, invalid, put_timestamp, take_timestamp};
use crate::{Database, sql::ai_jobs as sql};
use application::{
    AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRecordWrite, AiJobRun, AiJobTiming,
    PrinciplesRepository, RunKind, RunPurpose, SystemPortError, TokenCount, TokenUsage,
    WriteCompleted,
};
use domain::{
    AiJobId, CashFloor, ConcentrationFloor, InvestmentPrinciples, PolicyText, PrincipleNote,
    Timestamp,
};
use rusqlite::OptionalExtension;
use std::future::Future;

const PRINCIPLES_VERSION: i64 = 1;

const KINDS: [(RunKind, &str); 2] = [
    (RunKind::Collection, "collection"),
    (RunKind::Analysis, "analysis"),
];

const PURPOSES: [(RunPurpose, &str); 15] = [
    (RunPurpose::Chat, "chat"),
    (RunPurpose::MorningBriefing, "morning_briefing"),
    (RunPurpose::IntradayMonitoring, "intraday_monitoring"),
    (RunPurpose::ClosingReview, "closing_review"),
    (RunPurpose::UsMarketCheck, "us_market_check"),
    (RunPurpose::Estimate, "estimate"),
    (RunPurpose::DeepResearch, "deep_research"),
    (RunPurpose::CandidateDiscovery, "candidate_discovery"),
    (RunPurpose::WeeklyReview, "weekly_review"),
    (RunPurpose::NewsClassification, "news_classification"),
    (RunPurpose::NewsSummary, "news_summary"),
    (RunPurpose::WebCollection, "web_collection"),
    (RunPurpose::ThesisReview, "thesis_review"),
    (RunPurpose::ClaimVerification, "claim_verification"),
    (RunPurpose::PairedComparison, "paired_comparison"),
];

const OUTCOMES: [(AiJobOutcome, &str); 5] = [
    (AiJobOutcome::Completed, "completed"),
    (AiJobOutcome::Interrupted, "interrupted"),
    (AiJobOutcome::IsolationRejected, "isolation_rejected"),
    (AiJobOutcome::Waiting, "waiting"),
    (AiJobOutcome::Failed, "failed"),
];

const CASH_FLOORS: [(CashFloor, &str); 5] = [
    (CashFloor::None, "none"),
    (CashFloor::FivePercent, "five"),
    (CashFloor::TenPercent, "ten"),
    (CashFloor::TwentyPercent, "twenty"),
    (CashFloor::ThirtyPercent, "thirty"),
];

const CONCENTRATION_FLOORS: [(ConcentrationFloor, &str); 6] = [
    (ConcentrationFloor::Three, "three"),
    (ConcentrationFloor::Four, "four"),
    (ConcentrationFloor::Five, "five"),
    (ConcentrationFloor::Six, "six"),
    (ConcentrationFloor::Eight, "eight"),
    (ConcentrationFloor::Ten, "ten"),
];

fn tag_of<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> &'static str {
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map_or("", |(_, tag)| tag)
}

fn value_of<T: Copy>(table: &[(T, &'static str)], raw: &str) -> rusqlite::Result<T> {
    table
        .iter()
        .find(|(_, tag)| *tag == raw)
        .map(|(value, _)| *value)
        .ok_or_else(|| invalid(()))
}

fn encode_job(record: &AiJobFinishRecord) -> String {
    let mut out = Encoder::new();
    out.push(record.job().as_str());
    out.push(tag_of(&KINDS, record.kind()));
    out.push(tag_of(&PURPOSES, record.purpose()));
    out.push(tag_of(&OUTCOMES, record.outcome()));
    put_timestamp(&mut out, record.timing().started_at());
    put_timestamp(&mut out, record.timing().finished_at());
    let tokens = record.tokens();
    for count in [
        tokens.input(),
        tokens.output(),
        tokens.cache_read(),
        tokens.cache_creation(),
    ] {
        out.push_u64(*count.as_u64());
    }
    out.finish()
}

fn decode_job(payload: &str) -> rusqlite::Result<AiJobFinishRecord> {
    let mut input = Decoder::new(payload);
    let job = AiJobId::try_new(input.next()?).map_err(invalid)?;
    let kind = value_of(&KINDS, input.next()?)?;
    let purpose = value_of(&PURPOSES, input.next()?)?;
    let outcome = value_of(&OUTCOMES, input.next()?)?;
    let started = take_timestamp(&mut input)?;
    let finished = take_timestamp(&mut input)?;
    let timing = AiJobTiming::try_new(started, finished).map_err(invalid)?;
    let tokens = TokenUsage::new(
        TokenCount::new(input.next_u64()?),
        TokenCount::new(input.next_u64()?),
        TokenCount::new(input.next_u64()?),
        TokenCount::new(input.next_u64()?),
    );
    input.finish()?;
    Ok(AiJobFinishRecord::new(
        AiJobRun::new(job, kind, purpose),
        outcome,
        timing,
        tokens,
    ))
}

fn time_key(at: Timestamp) -> (i64, i64) {
    (
        *at.utc_seconds().as_i64(),
        i64::from(*at.nanosecond().as_u32()),
    )
}

fn decode_rows(
    statement: &mut rusqlite::Statement<'_>,
    params: impl rusqlite::Params,
) -> rusqlite::Result<Vec<AiJobFinishRecord>> {
    let rows = statement.query_map(params, |row| row.get::<_, String>(0))?;
    let mut records = Vec::new();
    for payload in rows {
        records.push(decode_job(&payload?)?);
    }
    Ok(records)
}

#[derive(Debug)]
pub struct AiJobStore<'database> {
    database: &'database Database,
}

impl<'database> AiJobStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl AiJobRecordStore for AiJobStore<'_> {
    fn store_finished(
        &self,
        record: &AiJobFinishRecord,
    ) -> impl Future<Output = Result<AiJobRecordWrite, SystemPortError>> + Send {
        let job = record.job().as_str().to_owned();
        let (seconds, nanos) = time_key(record.finished_at());
        let payload = encode_job(record);
        self.database.write(move |transaction| {
            match transaction.execute(
                sql::INSERT_JOB,
                rusqlite::params![job, seconds, nanos, payload],
            )? {
                1 => Ok(AiJobRecordWrite::Stored),
                0 => Ok(AiJobRecordWrite::AlreadyStored),
                _ => Err(invalid(())),
            }
        })
    }

    fn finished(
        &self,
        job: &AiJobId,
    ) -> impl Future<Output = Result<Option<AiJobFinishRecord>, SystemPortError>> + Send {
        let key = job.as_str().to_owned();
        let expected = job.clone();
        self.database.read(move |connection| {
            let found = connection
                .query_row(sql::FIND_JOB, [key], |row| row.get::<_, String>(0))
                .optional()?
                .map(|payload| decode_job(&payload))
                .transpose()?;
            match found {
                Some(record) if record.job() != &expected => Err(invalid(())),
                other => Ok(other),
            }
        })
    }

    fn finished_between(
        &self,
        from: Timestamp,
        through: Timestamp,
    ) -> impl Future<Output = Result<Vec<AiJobFinishRecord>, SystemPortError>> + Send {
        let (from_seconds, from_nanos) = time_key(from);
        let (through_seconds, through_nanos) = time_key(through);
        self.database.read(move |connection| {
            let mut statement = connection.prepare(sql::JOBS_BETWEEN)?;
            decode_rows(
                &mut statement,
                rusqlite::params![from_seconds, from_nanos, through_seconds, through_nanos],
            )
        })
    }
}

fn encode_principles(principles: &InvestmentPrinciples) -> String {
    let mut out = Encoder::new();
    match principles.note() {
        Some(note) => {
            out.push("some");
            out.push(note.text().as_str());
        }
        None => out.push("none"),
    }
    out.push(tag_of(&CASH_FLOORS, principles.cash()));
    out.push(tag_of(&CONCENTRATION_FLOORS, principles.concentration()));
    out.finish()
}

fn decode_principles(payload: &str) -> rusqlite::Result<InvestmentPrinciples> {
    let mut input = Decoder::new(payload);
    let note = match input.next()? {
        "some" => {
            let text = PolicyText::try_new(input.next()?).map_err(invalid)?;
            Some(PrincipleNote::try_new(&text).map_err(invalid)?)
        }
        "none" => None,
        _ => return Err(invalid(())),
    };
    let cash = value_of(&CASH_FLOORS, input.next()?)?;
    let concentration = value_of(&CONCENTRATION_FLOORS, input.next()?)?;
    input.finish()?;
    Ok(InvestmentPrinciples::new(note, cash, concentration))
}

#[derive(Debug)]
pub struct PrinciplesStore<'database> {
    database: &'database Database,
}

impl<'database> PrinciplesStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl PrinciplesRepository for PrinciplesStore<'_> {
    fn store_principles(
        &self,
        principles: &InvestmentPrinciples,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let payload = encode_principles(principles);
        self.database.write(move |transaction| {
            match transaction.execute(
                sql::UPSERT_PRINCIPLES,
                rusqlite::params![PRINCIPLES_VERSION, payload],
            )? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }

    fn principles(
        &self,
    ) -> impl Future<Output = Result<Option<InvestmentPrinciples>, SystemPortError>> + Send {
        self.database.read(|connection| {
            connection
                .query_row(sql::FIND_PRINCIPLES, [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .optional()?
                .map(|(version, payload)| match version {
                    PRINCIPLES_VERSION => decode_principles(&payload),
                    _ => Err(invalid(())),
                })
                .transpose()
        })
    }
}
