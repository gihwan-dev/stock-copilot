use crate::estimates::codec::{Decoder, Encoder, put_timestamp, take_timestamp};
use crate::{Database, sql::information as sql};
use application::{
    CollectedCount, CollectedEntry, CollectedItemHandle, CollectedItemReads, CollectedKind,
    CollectedRetention, CollectedTimes, DisclosureRepository, InformationKey, InformationLinks,
    InformationSearch, NewsRepository, ObservedCollectedItem, PublicationTime, RetentionCutoff,
    SearchLimit, SystemPortError, WriteCompleted,
};
use domain::Timestamp;
use domain::{
    DartReceiptNo, Disclosure, DisclosureKey, DisclosureSource, InformationSource, InstrumentId,
    KindReceiptNo, NewsItem, NewsSource, NormalizedUrl, ReceiptNo, SecEdgarReceiptNo,
    UntrustedText,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct InformationStore<'database> {
    database: &'database Database,
}
impl<'database> InformationStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
fn news_source_tag(source: &NewsSource) -> &'static str {
    match source {
        NewsSource::Hankyung => "hankyung",
        NewsSource::Maekyung => "maekyung",
        NewsSource::Yonhap => "yonhap",
        NewsSource::Edaily => "edaily",
        NewsSource::SegyeFinance => "segye_finance",
        NewsSource::BankOfKorea => "bank_of_korea",
        NewsSource::FederalReserve => "federal_reserve",
        NewsSource::GoogleNews => "google_news",
        NewsSource::YahooFinance => "yahoo_finance",
    }
}
fn decode_news_source(source: &str) -> rusqlite::Result<NewsSource> {
    match source {
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
pub(crate) fn disclosure_source_tag(source: &DisclosureSource) -> &'static str {
    match source {
        DisclosureSource::Dart => "dart",
        DisclosureSource::Kind => "kind",
        DisclosureSource::SecEdgar => "sec_edgar",
    }
}
pub(crate) fn receipt_text(receipt: &ReceiptNo) -> &str {
    match receipt {
        ReceiptNo::Dart(value) => value.as_str(),
        ReceiptNo::Kind(value) => value.as_str(),
        ReceiptNo::SecEdgar(value) => value.as_str(),
    }
}
pub(crate) fn receipt(source: &str, value: &str) -> rusqlite::Result<ReceiptNo> {
    match source {
        "dart" => DartReceiptNo::try_new(value)
            .map(ReceiptNo::Dart)
            .map_err(invalid),
        "kind" => KindReceiptNo::try_new(value)
            .map(ReceiptNo::Kind)
            .map_err(invalid),
        "sec_edgar" => SecEdgarReceiptNo::try_new(value)
            .map(ReceiptNo::SecEdgar)
            .map_err(invalid),
        _ => Err(invalid(())),
    }
}
fn key_parts(key: &InformationKey) -> (&'static str, &'static str, &str) {
    match key {
        InformationKey::News(source, url) => ("news", news_source_tag(source), url.as_str()),
        InformationKey::Disclosure(disclosure_key) => (
            "disclosure",
            disclosure_source_tag(&disclosure_key.source()),
            receipt_text(disclosure_key.receipt_no()),
        ),
    }
}
fn key_from_entry(entry: &CollectedEntry) -> rusqlite::Result<InformationKey> {
    match entry {
        CollectedEntry::News(item) => match item.collected().source() {
            InformationSource::News(source) => Ok(InformationKey::News(
                source.clone(),
                item.collected().url().clone(),
            )),
            InformationSource::Web | InformationSource::Disclosure(_) => Err(invalid(())),
        },
        CollectedEntry::Disclosure(item) => Ok(InformationKey::Disclosure(item.key().clone())),
    }
}
#[must_use]
pub fn citation_digest(entry: &CollectedEntry) -> Option<application::CitationDigest> {
    use sha2::{Digest, Sha256};
    let key = key_from_entry(entry).ok()?;
    let (category, source, external_id) = key_parts(&key);
    let item = content(entry);
    let mut hash = Sha256::new();
    for field in [
        category,
        source,
        external_id,
        item.url().as_str(),
        item.title().as_untrusted_str(),
    ]
    .into_iter()
    .chain(item.body().map(UntrustedText::as_untrusted_str))
    {
        hash.update(u64::try_from(field.len()).ok()?.to_be_bytes());
        hash.update(field.as_bytes());
    }
    let hex = hash
        .finalize()
        .iter()
        .flat_map(|byte| [byte >> 4_u8, byte & 0x0f_u8])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect::<String>();
    application::CitationDigest::try_new(&hex)
}

fn content(entry: &CollectedEntry) -> &domain::CollectedItem {
    match entry {
        CollectedEntry::News(item) => item.collected(),
        CollectedEntry::Disclosure(item) => item.collected(),
    }
}
struct Stored {
    id: i64,
    entry: CollectedEntry,
}
fn decode(row: &Row<'_>) -> rusqlite::Result<Stored> {
    let category = row.get::<_, String>(1)?;
    let source = row.get::<_, String>(2)?;
    let external_id = row.get::<_, String>(3)?;
    let url = NormalizedUrl::try_new(&row.get::<_, String>(4)?).map_err(invalid)?;
    let title = UntrustedText::try_new(&row.get::<_, String>(5)?).map_err(invalid)?;
    let body = row
        .get::<_, Option<String>>(6)?
        .map(|value| UntrustedText::try_new(&value).map_err(invalid))
        .transpose()?;
    let entry = match category.as_str() {
        "news" => {
            if external_id != url.as_str() {
                return Err(invalid(()));
            }
            CollectedEntry::News(NewsItem::new(
                decode_news_source(&source)?,
                url,
                title,
                body,
            ))
        }
        "disclosure" => CollectedEntry::Disclosure(Disclosure::new(
            receipt(&source, &external_id)?,
            url,
            title,
            body,
        )),
        _ => return Err(invalid(())),
    };
    Ok(Stored {
        id: row.get(0)?,
        entry,
    })
}
fn find(connection: &Connection, key: &InformationKey) -> rusqlite::Result<Option<Stored>> {
    let (kind, source, id) = key_parts(key);
    connection
        .query_row(sql::BY_KEY, params![kind, source, id], decode)
        .optional()
}
fn all(connection: &Connection) -> rusqlite::Result<Vec<Stored>> {
    connection
        .prepare(sql::ALL_ITEMS)?
        .query_map([], decode)?
        .collect()
}
fn tokens(text: &str) -> String {
    let mut grams = Vec::new();
    let folded = text.to_ascii_lowercase();
    for (first, second) in folded.chars().zip(folded.chars().skip(1)) {
        let mut gram = String::from("u");
        gram.push_str(&u32::from(first).to_string());
        gram.push('v');
        gram.push_str(&u32::from(second).to_string());
        grams.push(gram);
    }
    grams.join(" ")
}
fn insert_index(tx: &Transaction<'_>, id: i64, entry: &CollectedEntry) -> rusqlite::Result<()> {
    let item = content(entry);
    tx.execute(
        sql::INSERT_INDEX,
        params![
            id,
            tokens(item.title().as_untrusted_str()),
            item.body()
                .map_or_else(String::new, |value| tokens(value.as_untrusted_str()))
        ],
    )?;
    Ok(())
}
fn save(tx: &Transaction<'_>, entry: &CollectedEntry) -> rusqlite::Result<WriteCompleted> {
    let key = key_from_entry(entry)?;
    let (category, source, external_id) = key_parts(&key);
    let item = content(entry);
    let title = item.title().as_untrusted_str();
    let body = item.body().map(UntrustedText::as_untrusted_str);
    let id = if let Some(stored) = find(tx, &key)? {
        tx.execute(
            sql::UPDATE_ITEM,
            params![item.url().as_str(), title, body, stored.id],
        )?;
        tx.execute(sql::DELETE_INDEX, [stored.id])?;
        stored.id
    } else {
        tx.execute(
            sql::INSERT_ITEM,
            params![
                category,
                source,
                external_id,
                item.url().as_str(),
                title,
                body
            ],
        )?;
        tx.last_insert_rowid()
    };
    insert_index(tx, id, entry)?;
    Ok(WriteCompleted)
}
fn linked(
    connection: &Connection,
    instrument: &InstrumentId,
) -> rusqlite::Result<Vec<CollectedEntry>> {
    connection
        .prepare(sql::BY_INSTRUMENT)?
        .query_map([instrument.as_str()], decode)?
        .map(|row| row.map(|stored| stored.entry))
        .collect()
}
fn contains(entry: &CollectedEntry, query: &str) -> bool {
    let item = content(entry);
    let folded_query = query.to_ascii_lowercase();
    item.title()
        .as_untrusted_str()
        .to_ascii_lowercase()
        .contains(&folded_query)
        || item.body().is_some_and(|body| {
            body.as_untrusted_str()
                .to_ascii_lowercase()
                .contains(&folded_query)
        })
}
fn search(
    connection: &Connection,
    query: &UntrustedText,
    limit: SearchLimit,
) -> rusqlite::Result<Vec<CollectedEntry>> {
    let source = query.as_untrusted_str();
    if source.is_empty() {
        return Ok(Vec::new());
    }
    if source.chars().count() == 1 {
        return Ok(all(connection)?
            .into_iter()
            .filter(|item| contains(&item.entry, source))
            .take(*limit.as_usize())
            .map(|item| item.entry)
            .collect());
    }
    let mut phrase = String::from("\"");
    phrase.push_str(&tokens(source));
    phrase.push('"');
    let ids = connection
        .prepare(sql::SEARCH_INDEX)?
        .query_map(
            params![phrase, i64::try_from(*limit.as_usize()).map_err(invalid)?],
            |row| row.get::<_, i64>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ids.into_iter()
        .map(|id| {
            let item = connection.query_row(sql::BY_ID, [id], decode)?;
            if !contains(&item.entry, source) {
                return Err(invalid(()));
            }
            Ok(item.entry)
        })
        .collect()
}
impl NewsRepository for InformationStore<'_> {
    async fn store(&self, item: &NewsItem) -> Result<WriteCompleted, SystemPortError> {
        let entry = CollectedEntry::News(item.clone());
        self.database.write(move |tx| save(tx, &entry)).await
    }
    async fn for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<NewsItem>, SystemPortError> {
        let id = instrument.clone();
        self.database
            .read(move |connection| {
                Ok(linked(connection, &id)?
                    .into_iter()
                    .filter_map(|item| match item {
                        CollectedEntry::News(news) => Some(news),
                        CollectedEntry::Disclosure(_) => None,
                    })
                    .collect())
            })
            .await
    }
}
impl DisclosureRepository for InformationStore<'_> {
    async fn store_disclosure(&self, item: &Disclosure) -> Result<WriteCompleted, SystemPortError> {
        let entry = CollectedEntry::Disclosure(item.clone());
        self.database.write(move |tx| save(tx, &entry)).await
    }
    async fn disclosure(&self, key: &DisclosureKey) -> Result<Option<Disclosure>, SystemPortError> {
        let identity = InformationKey::Disclosure(key.clone());
        self.database
            .read(move |connection| match find(connection, &identity)? {
                Some(Stored {
                    entry: CollectedEntry::Disclosure(item),
                    ..
                }) => Ok(Some(item)),
                None => Ok(None),
                Some(_) => Err(invalid(())),
            })
            .await
    }
    async fn disclosures_for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<Disclosure>, SystemPortError> {
        let id = instrument.clone();
        self.database
            .read(move |connection| {
                Ok(linked(connection, &id)?
                    .into_iter()
                    .filter_map(|item| match item {
                        CollectedEntry::Disclosure(disclosure) => Some(disclosure),
                        CollectedEntry::News(_) => None,
                    })
                    .collect())
            })
            .await
    }
}
impl InformationLinks for InformationStore<'_> {
    async fn set_instruments(
        &self,
        item: &InformationKey,
        instruments: &[InstrumentId],
    ) -> Result<WriteCompleted, SystemPortError> {
        let identity = item.clone();
        let unique = instruments.iter().cloned().collect::<BTreeSet<_>>();
        self.database
            .write(move |tx| {
                let (category, source, id) = key_parts(&identity);
                tx.execute(sql::DELETE_LINKS, params![category, source, id])?;
                for instrument in unique {
                    tx.execute(
                        sql::INSERT_LINK,
                        params![category, source, id, instrument.as_str()],
                    )?;
                }
                Ok(WriteCompleted)
            })
            .await
    }
}
impl InformationSearch for InformationStore<'_> {
    async fn search(
        &self,
        query: &UntrustedText,
        limit: SearchLimit,
    ) -> Result<Vec<CollectedEntry>, SystemPortError> {
        let search_query = query.clone();
        self.database
            .read(move |connection| search(connection, &search_query, limit))
            .await
    }
    async fn rebuild_index(&self) -> Result<WriteCompleted, SystemPortError> {
        self.database
            .write(move |tx| {
                let items = all(tx)?;
                tx.execute(sql::DELETE_ALL_INDEX, [])?;
                for item in items {
                    insert_index(tx, item.id, &item.entry)?;
                }
                Ok(WriteCompleted)
            })
            .await
    }
}

fn encode_moment(at: Timestamp) -> String {
    let mut out = Encoder::new();
    put_timestamp(&mut out, at);
    out.finish()
}

fn decode_moment(raw: &str) -> rusqlite::Result<Timestamp> {
    let mut input = Decoder::new(raw);
    let at = take_timestamp(&mut input)?;
    input.finish()?;
    Ok(at)
}

const fn category_tag(kind: CollectedKind) -> &'static str {
    match kind {
        CollectedKind::News => "news",
        CollectedKind::Disclosure => "disclosure",
    }
}

const fn handle_prefix(kind: CollectedKind) -> &'static str {
    match kind {
        CollectedKind::News => "n-",
        CollectedKind::Disclosure => "d-",
    }
}

fn handle_of(kind: CollectedKind, id: i64) -> rusqlite::Result<CollectedItemHandle> {
    let mut text = String::from(handle_prefix(kind));
    text.push_str(&id.to_string());
    CollectedItemHandle::try_new(&text).map_err(invalid)
}

fn parse_handle(handle: &CollectedItemHandle) -> Option<(CollectedKind, i64)> {
    let raw = handle.as_str();
    let (kind, digits) = if let Some(rest) = raw.strip_prefix(handle_prefix(CollectedKind::News)) {
        (CollectedKind::News, rest)
    } else {
        (
            CollectedKind::Disclosure,
            raw.strip_prefix(handle_prefix(CollectedKind::Disclosure))?,
        )
    };
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse::<i64>().ok().map(|id| (kind, id))
}

struct ObservedRow {
    stored: Stored,
    published: Option<String>,
    collected: String,
}

fn decode_observed(row: &Row<'_>) -> rusqlite::Result<ObservedRow> {
    Ok(ObservedRow {
        stored: decode(row)?,
        published: row.get(7)?,
        collected: row.get(8)?,
    })
}

fn observed(
    connection: &Connection,
    kind: CollectedKind,
    row: ObservedRow,
) -> rusqlite::Result<ObservedCollectedItem> {
    let key = key_from_entry(&row.stored.entry)?;
    let (category, source, external_id) = key_parts(&key);
    let instruments = connection
        .prepare(sql::LINKED_INSTRUMENTS)?
        .query_map(params![category, source, external_id], |link| {
            link.get::<_, String>(0)
        })?
        .map(|raw| {
            let value = raw?;
            InstrumentId::try_new(&value).map_err(invalid)
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ObservedCollectedItem {
        handle: handle_of(kind, row.stored.id)?,
        entry: row.stored.entry,
        published_at: match row.published {
            Some(raw) => PublicationTime::Known(decode_moment(&raw)?),
            None => PublicationTime::NotProvided,
        },
        collected_at: decode_moment(&row.collected)?,
        instruments,
    })
}

impl CollectedTimes for InformationStore<'_> {
    async fn record_times(
        &self,
        entry: &CollectedEntry,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned = entry.clone();
        self.database
            .write(move |tx| {
                let key = key_from_entry(&owned)?;
                let (category, source, external_id) = key_parts(&key);
                let published = match published_at {
                    PublicationTime::Known(at) => Some(encode_moment(at)),
                    PublicationTime::NotProvided => None,
                };
                match tx.execute(
                    sql::RECORD_TIMES,
                    params![
                        published,
                        encode_moment(collected_at),
                        category,
                        source,
                        external_id
                    ],
                )? {
                    1 => Ok(WriteCompleted),
                    _ => Err(invalid(())),
                }
            })
            .await
    }
}

impl application::CollectedPresenceCheck for InformationStore<'_> {
    async fn presence(
        &self,
        url: &NormalizedUrl,
    ) -> Result<application::CollectedPresence, SystemPortError> {
        let wanted = url.as_str().to_owned();
        let stored: Option<String> = self
            .database
            .read(move |connection| {
                connection
                    .query_row(sql::KNOWN_URL, [wanted], |row| row.get(0))
                    .optional()
            })
            .await?;
        Ok(match stored {
            Some(_) => application::CollectedPresence::Stored,
            None => application::CollectedPresence::New,
        })
    }
}

impl application::CollectedUrlCatalog for InformationStore<'_> {
    async fn find_collected(
        &self,
        url: &domain::ParsedExternalUrl,
    ) -> Result<Option<NormalizedUrl>, SystemPortError> {
        let wanted = String::from(url.canonical_url().as_str());
        let stored: Option<String> = self
            .database
            .read(move |connection| {
                connection
                    .query_row(sql::KNOWN_URL, [wanted], |row| row.get(0))
                    .optional()
            })
            .await?;
        Ok(stored.and_then(|text| NormalizedUrl::try_new(&text).ok()))
    }
}

impl CollectedItemReads for InformationStore<'_> {
    async fn collected_since(
        &self,
        kind: CollectedKind,
        since: Timestamp,
    ) -> Result<Vec<ObservedCollectedItem>, SystemPortError> {
        self.database
            .read(move |connection| {
                let rows = connection
                    .prepare(sql::OBSERVED_BY_CATEGORY)?
                    .query_map([category_tag(kind)], decode_observed)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                let items = rows
                    .into_iter()
                    .map(|row| observed(connection, kind, row))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(items
                    .into_iter()
                    .filter(|item| item.collected_at >= since)
                    .collect())
            })
            .await
    }

    async fn collected_item(
        &self,
        handle: &CollectedItemHandle,
    ) -> Result<Option<ObservedCollectedItem>, SystemPortError> {
        let Some((kind, id)) = parse_handle(handle) else {
            return Ok(None);
        };
        self.database
            .read(move |connection| observed_by_id(connection, kind, id))
            .await
    }
}

fn observed_by_id(
    connection: &Connection,
    kind: CollectedKind,
    id: i64,
) -> rusqlite::Result<Option<ObservedCollectedItem>> {
    let row = connection
        .query_row(
            sql::OBSERVED_BY_ID,
            params![id, category_tag(kind)],
            decode_observed,
        )
        .optional()?;
    row.map(|found| observed(connection, kind, found))
        .transpose()
}

impl InformationStore<'_> {
    pub(crate) async fn citation_in_run(
        &self,
        run: &application::AiRunId,
        handle: &CollectedItemHandle,
    ) -> Result<Option<ObservedCollectedItem>, SystemPortError> {
        let Some((kind, id)) = parse_handle(handle) else {
            return Ok(None);
        };
        let tool = match kind {
            CollectedKind::News => "search_news",
            CollectedKind::Disclosure => "get_disclosures",
        };
        let run_id = run.as_str().to_owned();
        let handle_text = handle.as_str().to_owned();
        self.database
            .read(move |connection| {
                let Some(observed) = observed_by_id(connection, kind, id)? else {
                    return Ok(None);
                };
                let digest = citation_digest(&observed.entry).ok_or_else(|| invalid(()))?;
                let shown: bool = connection.query_row(
                    sql::CITED_RESULT_HANDLE,
                    params![run_id, tool, handle_text, digest.as_str()],
                    |row| row.get(0),
                )?;
                Ok(shown.then_some(observed))
            })
            .await
    }
}

struct TimedNews {
    id: i64,
    source: String,
    external_id: String,
    collected: String,
}

fn expired(row: &TimedNews, cutoffs: &[RetentionCutoff]) -> rusqlite::Result<bool> {
    let source = decode_news_source(&row.source)?;
    let collected = decode_moment(&row.collected)?;
    Ok(cutoffs
        .iter()
        .any(|cutoff| cutoff.source == source && collected < cutoff.before))
}

fn delete_news(tx: &Transaction<'_>, row: &TimedNews) -> rusqlite::Result<()> {
    let category = category_tag(CollectedKind::News);
    tx.execute(
        sql::DELETE_LINKS,
        params![category, row.source, row.external_id],
    )?;
    tx.execute(sql::DELETE_INDEX, params![row.id])?;
    tx.execute(sql::DELETE_TIMES, params![row.id])?;
    tx.execute(sql::DELETE_ITEM, params![row.id])?;
    Ok(())
}

fn timed_news(tx: &Transaction<'_>) -> rusqlite::Result<Vec<TimedNews>> {
    tx.prepare(sql::NEWS_COLLECTED_TIMES)?
        .query_map([category_tag(CollectedKind::News)], |row| {
            Ok(TimedNews {
                id: row.get(0)?,
                source: row.get(1)?,
                external_id: row.get(2)?,
                collected: row.get(3)?,
            })
        })?
        .collect()
}

fn prune_expired(
    tx: &Transaction<'_>,
    cutoffs: &[RetentionCutoff],
) -> rusqlite::Result<CollectedCount> {
    let mut removed = 0_u32;
    for row in timed_news(tx)? {
        if expired(&row, cutoffs)? {
            delete_news(tx, &row)?;
            removed = removed.saturating_add(1);
        }
    }
    Ok(CollectedCount::new(removed))
}

impl CollectedRetention for InformationStore<'_> {
    async fn prune_news(
        &self,
        cutoffs: &[RetentionCutoff],
    ) -> Result<CollectedCount, SystemPortError> {
        let owned = cutoffs.to_vec();
        self.database
            .write(move |tx| prune_expired(tx, &owned))
            .await
    }
}
