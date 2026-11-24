use crate::estimates::codec::{
    Decoder, Encoder, invalid, put_date, put_timestamp, take_date, take_timestamp,
};
use crate::wake::{put_information_source, take_information_source};
use crate::{Database, sql::briefing as sql};
use application::{
    BriefingFailure, BriefingOutcome, BriefingRecord, BriefingRepository, BriefingSource,
    SystemPortError, WriteCompleted,
};
use domain::{BriefingId, BriefingKind, NormalizedUrl, UntrustedText};
use rusqlite::OptionalExtension;
use std::future::Future;

fn put_id(out: &mut Encoder, id: BriefingId) {
    out.push(match id.kind() {
        BriefingKind::Morning => "morning",
        BriefingKind::ClosingReview => "closing_review",
        BriefingKind::PreUsMarket => "pre_us_market",
    });
    put_date(out, id.date());
}

fn take_id(input: &mut Decoder<'_>) -> rusqlite::Result<BriefingId> {
    let kind = match input.next()? {
        "morning" => BriefingKind::Morning,
        "closing_review" => BriefingKind::ClosingReview,
        "pre_us_market" => BriefingKind::PreUsMarket,
        _ => return Err(invalid(())),
    };
    Ok(BriefingId::new(kind, take_date(input)?))
}

fn id_key(id: BriefingId) -> String {
    let mut out = Encoder::new();
    put_id(&mut out, id);
    out.finish()
}

const fn failure_tag(failure: BriefingFailure) -> &'static str {
    match failure {
        BriefingFailure::ProviderUnavailable => "provider_unavailable",
        BriefingFailure::IsolationRejected => "isolation_rejected",
        BriefingFailure::RateLimited => "rate_limited",
        BriefingFailure::EmptyResult => "empty_result",
        BriefingFailure::PromptRejected => "prompt_rejected",
    }
}

fn parse_failure(raw: &str) -> rusqlite::Result<BriefingFailure> {
    match raw {
        "provider_unavailable" => Ok(BriefingFailure::ProviderUnavailable),
        "isolation_rejected" => Ok(BriefingFailure::IsolationRejected),
        "rate_limited" => Ok(BriefingFailure::RateLimited),
        "empty_result" => Ok(BriefingFailure::EmptyResult),
        "prompt_rejected" => Ok(BriefingFailure::PromptRejected),
        _ => Err(invalid(())),
    }
}

fn encode(record: &BriefingRecord) -> String {
    let mut out = Encoder::new();
    put_id(&mut out, record.id);
    put_timestamp(&mut out, record.finished_at);
    match &record.outcome {
        BriefingOutcome::Completed { body, sources } => {
            out.push("completed");
            out.push(body.as_untrusted_str());
            out.push_usize(sources.len());
            for source in sources {
                out.push(source.title.as_untrusted_str());
                match &source.url {
                    Some(url) => {
                        out.push("some");
                        out.push(url.as_str());
                    }
                    None => out.push("none"),
                }
                put_information_source(&mut out, &source.origin);
                put_timestamp(&mut out, source.observed_at);
            }
        }
        BriefingOutcome::Failed(failure) => {
            out.push("failed");
            out.push(failure_tag(*failure));
        }
    }
    out.finish()
}

fn text(raw: &str) -> rusqlite::Result<UntrustedText> {
    UntrustedText::try_new(raw).map_err(invalid)
}

fn take_source(input: &mut Decoder<'_>) -> rusqlite::Result<BriefingSource> {
    let title = text(input.next()?)?;
    let url = match input.next()? {
        "some" => Some(NormalizedUrl::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let origin = take_information_source(input)?;
    let observed_at = take_timestamp(input)?;
    Ok(BriefingSource {
        title,
        url,
        origin,
        observed_at,
    })
}

fn decode(payload: &str) -> rusqlite::Result<BriefingRecord> {
    let mut input = Decoder::new(payload);
    let id = take_id(&mut input)?;
    let finished_at = take_timestamp(&mut input)?;
    let outcome = match input.next()? {
        "completed" => {
            let body = text(input.next()?)?;
            let count = input.next_usize()?;
            let mut sources = Vec::new();
            for _ in 0..count {
                sources.push(take_source(&mut input)?);
            }
            BriefingOutcome::Completed { body, sources }
        }
        "failed" => BriefingOutcome::Failed(parse_failure(input.next()?)?),
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(BriefingRecord {
        id,
        finished_at,
        outcome,
    })
}

#[derive(Debug)]
pub struct BriefingStore<'database> {
    database: &'database Database,
}

impl<'database> BriefingStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl BriefingRepository for BriefingStore<'_> {
    fn save(
        &self,
        record: &BriefingRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let key = id_key(record.id);
        let payload = encode(record);
        self.database.write(move |transaction| {
            match transaction.execute(sql::UPSERT, rusqlite::params![key, payload])? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }

    fn find(
        &self,
        id: &BriefingId,
    ) -> impl Future<Output = Result<Option<BriefingRecord>, SystemPortError>> + Send {
        let key = id_key(*id);
        let expected = *id;
        self.database.read(move |connection| {
            let found = connection
                .query_row(sql::FIND, [key], |row| row.get::<_, String>(0))
                .optional()?
                .map(|payload| decode(&payload))
                .transpose()?;
            match found {
                Some(record) if record.id != expected => Err(invalid(())),
                other => Ok(other),
            }
        })
    }

    fn recent(&self) -> impl Future<Output = Result<Vec<BriefingRecord>, SystemPortError>> + Send {
        self.database.read(|connection| {
            let mut statement = connection.prepare(sql::RECENT)?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let mut records = Vec::new();
            for payload in rows {
                records.push(decode(&payload?)?);
            }
            Ok(records)
        })
    }
}
