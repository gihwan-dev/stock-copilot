use crate::{Database, sql::information as sql};
use application::{
    CollectedEntry, DisclosureRepository, InformationKey, InformationLinks, InformationSearch,
    NewsRepository, SearchLimit, SystemPortError, WriteCompleted,
};
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
        "bank_of_korea" => Ok(NewsSource::BankOfKorea),
        "federal_reserve" => Ok(NewsSource::FederalReserve),
        "google_news" => Ok(NewsSource::GoogleNews),
        "yahoo_finance" => Ok(NewsSource::YahooFinance),
        _ => Err(invalid(())),
    }
}
fn disclosure_source_tag(source: &DisclosureSource) -> &'static str {
    match source {
        DisclosureSource::Dart => "dart",
        DisclosureSource::Kind => "kind",
        DisclosureSource::SecEdgar => "sec_edgar",
    }
}
fn receipt_text(receipt: &ReceiptNo) -> &str {
    match receipt {
        ReceiptNo::Dart(value) => value.as_str(),
        ReceiptNo::Kind(value) => value.as_str(),
        ReceiptNo::SecEdgar(value) => value.as_str(),
    }
}
fn receipt(source: &str, value: &str) -> rusqlite::Result<ReceiptNo> {
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
    for (first, second) in text.chars().zip(text.chars().skip(1)) {
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
    item.title().as_untrusted_str().contains(query)
        || item
            .body()
            .is_some_and(|body| body.as_untrusted_str().contains(query))
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
