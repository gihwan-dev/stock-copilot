use crate::estimates::codec::{Decoder, Encoder, invalid, put_timestamp, take_timestamp};
use crate::{Database, sql::jobs as sql};
use application::{
    JobClockTime, JobCode, JobFailureKind, JobHour, JobItemCount, JobMinute, JobRunEntry,
    JobRunResult, JobScheduleStore, JobSummary, JobSwitch, SavedJobSchedule, SystemPortError,
    WriteCompleted,
};
use std::future::Future;

const fn switch_tag(value: JobSwitch) -> &'static str {
    match value {
        JobSwitch::Enabled => "enabled",
        JobSwitch::Disabled => "disabled",
    }
}

fn parse_switch(raw: &str) -> rusqlite::Result<JobSwitch> {
    match raw {
        "enabled" => Ok(JobSwitch::Enabled),
        "disabled" => Ok(JobSwitch::Disabled),
        _ => Err(invalid(())),
    }
}

const fn failure_tag(kind: JobFailureKind) -> &'static str {
    match kind {
        JobFailureKind::PortUnavailable => "port_unavailable",
        JobFailureKind::StorageFailure => "storage_failure",
        JobFailureKind::CalendarUnavailable => "calendar_unavailable",
        JobFailureKind::ExecutionFailed => "execution_failed",
    }
}

fn parse_failure(raw: &str) -> rusqlite::Result<JobFailureKind> {
    match raw {
        "port_unavailable" => Ok(JobFailureKind::PortUnavailable),
        "storage_failure" => Ok(JobFailureKind::StorageFailure),
        "calendar_unavailable" => Ok(JobFailureKind::CalendarUnavailable),
        "execution_failed" => Ok(JobFailureKind::ExecutionFailed),
        _ => Err(invalid(())),
    }
}

fn encode_run(entry: &JobRunEntry) -> String {
    let mut out = Encoder::new();
    put_timestamp(&mut out, entry.scheduled_for);
    put_timestamp(&mut out, entry.evaluated_at);
    match &entry.result {
        JobRunResult::Executed { summary, processed } => {
            out.push("executed");
            out.push(summary.as_str());
            out.push_u32(*processed.as_u32());
        }
        JobRunResult::SkippedMissedWindow => out.push("skipped"),
        JobRunResult::Deferred { resumes_at } => {
            out.push("deferred");
            match resumes_at {
                Some(at) => {
                    out.push("some");
                    put_timestamp(&mut out, *at);
                }
                None => out.push("none"),
            }
        }
        JobRunResult::Failed(kind) => {
            out.push("failed");
            out.push(failure_tag(*kind));
        }
    }
    out.finish()
}

fn decode_run(job: &JobCode, payload: &str) -> rusqlite::Result<JobRunEntry> {
    let mut input = Decoder::new(payload);
    let scheduled_for = take_timestamp(&mut input)?;
    let evaluated_at = take_timestamp(&mut input)?;
    let result = match input.next()? {
        "executed" => JobRunResult::Executed {
            summary: JobSummary::new(input.next()?),
            processed: JobItemCount::new(input.next_u32()?),
        },
        "skipped" => JobRunResult::SkippedMissedWindow,
        "deferred" => JobRunResult::Deferred {
            resumes_at: match input.next()? {
                "some" => Some(take_timestamp(&mut input)?),
                "none" => None,
                _ => return Err(invalid(())),
            },
        },
        "failed" => JobRunResult::Failed(parse_failure(input.next()?)?),
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(JobRunEntry {
        job: job.clone(),
        scheduled_for,
        evaluated_at,
        result,
    })
}

fn time_parts(time: Option<JobClockTime>) -> (Option<i64>, Option<i64>) {
    time.map_or((None, None), |value| {
        (
            Some(i64::from(*value.hour.as_u8())),
            Some(i64::from(*value.minute.as_u8())),
        )
    })
}

fn stored_time(hour: Option<i64>, minute: Option<i64>) -> rusqlite::Result<Option<JobClockTime>> {
    match (hour, minute) {
        (None, None) => Ok(None),
        (Some(stored_hour), Some(stored_minute)) => Ok(Some(JobClockTime {
            hour: JobHour::try_new(u8::try_from(stored_hour).map_err(invalid)?).map_err(invalid)?,
            minute: JobMinute::try_new(u8::try_from(stored_minute).map_err(invalid)?)
                .map_err(invalid)?,
        })),
        (Some(_), None) | (None, Some(_)) => Err(invalid(())),
    }
}

#[derive(Debug)]
pub struct JobScheduleSqlStore<'database> {
    database: &'database Database,
}

impl<'database> JobScheduleSqlStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl JobScheduleStore for JobScheduleSqlStore<'_> {
    fn save_schedule(
        &self,
        schedule: &SavedJobSchedule,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let job = String::from(schedule.job.as_str());
        let switch = switch_tag(schedule.switch);
        let (hour, minute) = time_parts(schedule.time);
        self.database.write(move |transaction| {
            match transaction.execute(
                sql::UPSERT_SETTING,
                rusqlite::params![job, switch, hour, minute],
            )? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }

    fn schedules(
        &self,
    ) -> impl Future<Output = Result<Vec<SavedJobSchedule>, SystemPortError>> + Send {
        self.database.read(|connection| {
            let mut statement = connection.prepare(sql::ALL_SETTINGS)?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })?;
            let mut schedules = Vec::new();
            for row in rows {
                let (job, switch, hour, minute) = row?;
                schedules.push(SavedJobSchedule {
                    job: JobCode::try_new(&job).map_err(invalid)?,
                    switch: parse_switch(&switch)?,
                    time: stored_time(hour, minute)?,
                });
            }
            Ok(schedules)
        })
    }

    fn append_run(
        &self,
        entry: &JobRunEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let job = String::from(entry.job.as_str());
        let payload = encode_run(entry);
        self.database.write(move |transaction| {
            match transaction.execute(sql::APPEND_RECORD, rusqlite::params![job, payload])? {
                1 => {}
                _ => return Err(invalid(())),
            }
            let _trimmed = transaction.execute(sql::TRIM_RECORDS, [&job])?;
            Ok(WriteCompleted)
        })
    }

    fn recent_runs(
        &self,
        job: &JobCode,
    ) -> impl Future<Output = Result<Vec<JobRunEntry>, SystemPortError>> + Send {
        let code = job.clone();
        self.database.read(move |connection| {
            let mut statement = connection.prepare(sql::RECENT_RECORDS)?;
            let rows = statement.query_map([code.as_str()], |row| row.get::<_, String>(0))?;
            let mut runs = Vec::new();
            for row in rows {
                runs.push(decode_run(&code, &row?)?);
            }
            Ok(runs)
        })
    }
}
