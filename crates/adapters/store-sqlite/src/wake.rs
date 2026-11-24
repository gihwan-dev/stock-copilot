use crate::estimates::codec::{Decoder, Encoder, invalid, put_news_source, take_news_source};
use crate::information::{disclosure_source_tag, receipt, receipt_text};
use crate::{Database, sql::wake as sql};
use application::{
    Clock, DisclosureCardFact, DisclosureTriggerLog, ResearchWebStore, SystemPortError,
    TriggerWrite, WakeTriggerRepository, WebCollectionStore, WriteCompleted,
};
use domain::{
    CollectedItem, DisclosureAction, DisclosureGrade, DisclosureKey, DisclosureSource,
    InformationSource, InstrumentId, Nanosecond, NormalizedUrl, OffsetSeconds, Symbol, Timestamp,
    UnixSeconds, UntrustedText, WakeTrigger,
};
use std::fmt::{self, Debug, Formatter};
use std::future::Future;

const KOREA_OFFSET: i32 = 32_400;

pub struct WakeTriggerStore<'store> {
    database: &'store Database,
    clock: &'store dyn Clock,
}

impl Debug for WakeTriggerStore<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WakeTriggerStore").finish_non_exhaustive()
    }
}

impl<'store> WakeTriggerStore<'store> {
    #[must_use]
    pub const fn new(database: &'store Database, clock: &'store dyn Clock) -> Self {
        Self { database, clock }
    }
}

const fn grade_tag(grade: DisclosureGrade) -> &'static str {
    match grade {
        DisclosureGrade::Urgent => "urgent",
        DisclosureGrade::High => "high",
        DisclosureGrade::Normal => "normal",
        DisclosureGrade::Low => "low",
    }
}

fn parse_grade(raw: &str) -> rusqlite::Result<DisclosureGrade> {
    match raw {
        "urgent" => Ok(DisclosureGrade::Urgent),
        "high" => Ok(DisclosureGrade::High),
        "normal" => Ok(DisclosureGrade::Normal),
        "low" => Ok(DisclosureGrade::Low),
        _ => Err(invalid(())),
    }
}

const fn action_tag(action: DisclosureAction) -> &'static str {
    match action {
        DisclosureAction::WakeAndNotify => "wake_and_notify",
        DisclosureAction::Wake => "wake",
        DisclosureAction::Briefing => "briefing",
        DisclosureAction::CollectOnly => "collect_only",
    }
}

struct TriggerRow {
    kind: &'static str,
    instrument: String,
    disclosure_source: Option<&'static str>,
    receipt: Option<String>,
    grade: Option<&'static str>,
    action: Option<&'static str>,
}

fn trigger_row(trigger: &WakeTrigger) -> TriggerRow {
    match trigger {
        WakeTrigger::Disclosure {
            key,
            instrument,
            grade,
            action,
        } => TriggerRow {
            kind: "disclosure",
            instrument: instrument.as_str().to_owned(),
            disclosure_source: Some(disclosure_source_tag(&key.source())),
            receipt: Some(receipt_text(key.receipt_no()).to_owned()),
            grade: Some(grade_tag(*grade)),
            action: Some(action_tag(*action)),
        },
        WakeTrigger::PriceMove(instrument) => TriggerRow {
            kind: "price_move",
            instrument: instrument.as_str().to_owned(),
            disclosure_source: None,
            receipt: None,
            grade: None,
            action: None,
        },
        WakeTrigger::NewsBurst(instrument) => TriggerRow {
            kind: "news_burst",
            instrument: instrument.as_str().to_owned(),
            disclosure_source: None,
            receipt: None,
            grade: None,
            action: None,
        },
    }
}

fn korean_day(at: Timestamp) -> Result<i64, SystemPortError> {
    let date = at
        .date_at_offset(OffsetSeconds::new(KOREA_OFFSET))
        .map_err(|_error| crate::failure::integrity())?;
    let year = i64::from(*date.year().as_i32());
    let month = i64::from(*date.month().as_u8());
    let day = i64::from(*date.day().as_u8());
    Ok(year
        .saturating_mul(10_000)
        .saturating_add(month.saturating_mul(100))
        .saturating_add(day))
}

fn decode_time(seconds: i64, raw_nanos: i64, raw_offset: i64) -> rusqlite::Result<Timestamp> {
    let nanos = u32::try_from(raw_nanos).map_err(invalid)?;
    let offset = i32::try_from(raw_offset).map_err(invalid)?;
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .map_err(invalid)
}

fn decode_fact(row: &rusqlite::Row<'_>) -> rusqlite::Result<DisclosureCardFact> {
    let source = row.get::<_, String>(0)?;
    let receipt_no = row.get::<_, String>(1)?;
    let instrument = InstrumentId::try_new(&row.get::<_, String>(2)?).map_err(invalid)?;
    let grade = parse_grade(&row.get::<_, String>(3)?)?;
    let recorded_at = decode_time(row.get(4)?, row.get(5)?, row.get(6)?)?;
    let url = row
        .get::<_, Option<String>>(7)?
        .ok_or_else(|| invalid(()))?;
    let title = row
        .get::<_, Option<String>>(8)?
        .ok_or_else(|| invalid(()))?;
    Ok(DisclosureCardFact {
        key: DisclosureKey::from_receipt(receipt(&source, &receipt_no)?),
        instrument,
        title: UntrustedText::try_new(&title).map_err(invalid)?,
        url: NormalizedUrl::try_new(&url).map_err(invalid)?,
        grade,
        recorded_at,
    })
}

impl WakeTriggerRepository for WakeTriggerStore<'_> {
    fn record(
        &self,
        trigger: &WakeTrigger,
    ) -> impl Future<Output = Result<TriggerWrite, SystemPortError>> + Send {
        let at = self.clock.now();
        let row = trigger_row(trigger);
        let day = korean_day(at);
        let database = self.database;
        async move {
            let recorded_day = day?;
            let seconds = *at.utc_seconds().as_i64();
            let nanos = i64::from(*at.nanosecond().as_u32());
            let offset = i64::from(*at.offset_seconds().as_i32());
            database
                .write(move |transaction| {
                    match transaction.execute(
                        sql::INSERT_TRIGGER,
                        rusqlite::params![
                            row.kind,
                            row.instrument,
                            row.disclosure_source,
                            row.receipt,
                            row.grade,
                            row.action,
                            recorded_day,
                            seconds,
                            nanos,
                            offset
                        ],
                    )? {
                        1 => Ok(TriggerWrite::Recorded),
                        0 => Ok(TriggerWrite::AlreadyRecorded),
                        _ => Err(invalid(())),
                    }
                })
                .await
        }
    }
}

impl DisclosureTriggerLog for WakeTriggerStore<'_> {
    fn recorded_since(
        &self,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<DisclosureCardFact>, SystemPortError>> + Send {
        let seconds = *since.utc_seconds().as_i64();
        let nanos = i64::from(*since.nanosecond().as_u32());
        self.database.read(move |connection| {
            connection
                .prepare(sql::DISCLOSURES_SINCE)?
                .query_map(rusqlite::params![seconds, nanos], decode_fact)?
                .collect()
        })
    }
}

#[derive(Debug)]
pub struct WebItemStore<'database> {
    database: &'database Database,
}

impl<'database> WebItemStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn items_for_symbol(
        &self,
        instrument: &Symbol,
    ) -> Result<Vec<CollectedItem>, SystemPortError> {
        self.items("symbol", instrument.as_str().to_owned()).await
    }

    pub async fn items_for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<CollectedItem>, SystemPortError> {
        self.items("instrument", instrument.as_str().to_owned())
            .await
    }

    async fn items(
        &self,
        kind: &'static str,
        subject: String,
    ) -> Result<Vec<CollectedItem>, SystemPortError> {
        self.database
            .read(move |connection| {
                let mut statement = connection.prepare(sql::WEB_ITEMS)?;
                let rows = statement.query_map(rusqlite::params![kind, subject], |row| {
                    row.get::<_, String>(0)
                })?;
                let mut items = Vec::new();
                for payload in rows {
                    items.push(decode_item(&payload?)?);
                }
                Ok(items)
            })
            .await
    }

    async fn save_items(
        &self,
        kind: &'static str,
        subject: String,
        payloads: Vec<String>,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.database
            .write(move |transaction| {
                let mut inserted = 0_usize;
                for payload in &payloads {
                    inserted = inserted.saturating_add(transaction.execute(
                        sql::INSERT_WEB_ITEM,
                        rusqlite::params![kind, subject, payload],
                    )?);
                }
                if inserted == payloads.len() {
                    Ok(WriteCompleted)
                } else {
                    Err(invalid(()))
                }
            })
            .await
    }
}

fn parse_disclosure_source(raw: &str) -> rusqlite::Result<DisclosureSource> {
    match raw {
        "dart" => Ok(DisclosureSource::Dart),
        "kind" => Ok(DisclosureSource::Kind),
        "sec_edgar" => Ok(DisclosureSource::SecEdgar),
        _ => Err(invalid(())),
    }
}

pub(crate) fn put_information_source(out: &mut Encoder, source: &InformationSource) {
    match source {
        InformationSource::Web => out.push("web"),
        InformationSource::News(news) => {
            out.push("news");
            put_news_source(out, news);
        }
        InformationSource::Disclosure(disclosure) => {
            out.push("disclosure");
            out.push(disclosure_source_tag(disclosure));
        }
    }
}

pub(crate) fn take_information_source(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<InformationSource> {
    match input.next()? {
        "web" => Ok(InformationSource::Web),
        "news" => Ok(InformationSource::News(take_news_source(input)?)),
        "disclosure" => Ok(InformationSource::Disclosure(parse_disclosure_source(
            input.next()?,
        )?)),
        _ => Err(invalid(())),
    }
}

fn encode_item(item: &CollectedItem) -> String {
    let mut out = Encoder::new();
    put_information_source(&mut out, item.source());
    out.push(item.url().as_str());
    out.push(item.title().as_untrusted_str());
    match item.body() {
        Some(body) => {
            out.push("some");
            out.push(body.as_untrusted_str());
        }
        None => out.push("none"),
    }
    out.finish()
}

fn decode_item(payload: &str) -> rusqlite::Result<CollectedItem> {
    let mut input = Decoder::new(payload);
    let source = take_information_source(&mut input)?;
    let url = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
    let title = UntrustedText::try_new(input.next()?).map_err(invalid)?;
    let body = match input.next()? {
        "some" => Some(UntrustedText::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(CollectedItem::from_saved_source(source, url, title, body))
}

impl WebCollectionStore for WebItemStore<'_> {
    fn save(
        &self,
        instrument: &Symbol,
        items: &[CollectedItem],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let payloads = items.iter().map(encode_item).collect();
        self.save_items("symbol", instrument.as_str().to_owned(), payloads)
    }
}

impl ResearchWebStore for WebItemStore<'_> {
    fn save(
        &self,
        instrument: &InstrumentId,
        items: &[CollectedItem],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let payloads = items.iter().map(encode_item).collect();
        self.save_items("instrument", instrument.as_str().to_owned(), payloads)
    }
}
