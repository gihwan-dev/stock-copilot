use crate::{Database, sql::instrument_mapping as sql};
use application::{
    InstrumentIdIssuer, InstrumentMappingHistoryRepository, InstrumentMappingRepository,
    MappingStoreError, SystemPortError,
};
use domain::{
    CalendarDate, CalendarYear, CorporateAction, CorporateActionId, CorporateActionKind,
    DartReceiptNo, DayNumber, DisclosureKey, IndependentInstrumentIdIssuer, InstrumentId,
    InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource,
    InstrumentSourceConflict, IsinCode, IssuedInstrumentId, IssuerSequence, KindReceiptNo,
    KnownMarket, MappingAssignment, MappingHistoryEvent, MonthNumber, Nanosecond, OffsetSeconds,
    PersistedIssuerCursor, ReceiptNo, SecEdgarReceiptNo, ShareCount, ShareRatio, Symbol, Timestamp,
    UnixSeconds, UntrustedText, UserReason, instrument_source_conflicts,
    verify_persisted_mapping_ledger,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::{BTreeMap, BTreeSet};

const STATUS_ACTIVE: &str = "active";
const STATUS_SUPERSEDED: &str = "superseded";
const STATUS_RETIRED: &str = "retired";

const EVENT_IDENTIFIER_CHANGED: &str = "identifier_changed";
const EVENT_RETIREMENT_CORRECTED: &str = "retirement_corrected";
const EVENT_LINK_CORRECTED: &str = "link_corrected";

const ACTION_SYMBOL_CHANGE: &str = "symbol_change";
const ACTION_SPLIT: &str = "split";
const ACTION_REVERSE_SPLIT: &str = "reverse_split";

const DISCLOSURE_DART: &str = "DART";
const DISCLOSURE_KIND: &str = "KIND";
const DISCLOSURE_SEC_EDGAR: &str = "SEC_EDGAR";

#[derive(Debug)]
pub struct InstrumentMappingStore<'database> {
    database: &'database Database,
}

impl<'database> InstrumentMappingStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

#[derive(Debug)]
pub struct InstrumentIdIssuerStore<'database> {
    database: &'database Database,
}

impl<'database> InstrumentIdIssuerStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

enum IdPresence {
    Present,
    Absent,
}

#[derive(Clone, Copy)]
enum TerminalRequirement {
    AllowPartialSlice,
    RequireEveryIdTerminal,
}

#[derive(Clone, Copy)]
pub(crate) enum StoredStatus {
    Active,
    Superseded(i64),
    Retired,
}

impl StoredStatus {
    const fn into_domain(self) -> InstrumentMappingStatus {
        match self {
            Self::Active => InstrumentMappingStatus::Active,
            Self::Superseded(_) => InstrumentMappingStatus::Superseded,
            Self::Retired => InstrumentMappingStatus::Retired,
        }
    }
}

pub(crate) struct StoredRecord {
    pub(crate) source: InstrumentSource,
    pub(crate) id: InstrumentId,
    pub(crate) status: StoredStatus,
}

impl StoredRecord {
    fn to_domain(&self) -> InstrumentMappingRecord {
        InstrumentMappingRecord::new(
            self.source.clone(),
            self.id.clone(),
            self.status.into_domain(),
        )
    }

    fn into_domain(self) -> InstrumentMappingRecord {
        InstrumentMappingRecord::new(self.source, self.id, self.status.into_domain())
    }
}

pub(crate) struct StoredEvent {
    pub(crate) seq: i64,
    pub(crate) event: MappingHistoryEvent,
}

fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

const fn market_tag(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}

fn decode_market(raw: &str) -> rusqlite::Result<KnownMarket> {
    match raw {
        "KR" => Ok(KnownMarket::Kr),
        "US" => Ok(KnownMarket::Us),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_status(raw: &str, superseded_event_seq: Option<i64>) -> rusqlite::Result<StoredStatus> {
    match (raw, superseded_event_seq) {
        (STATUS_ACTIVE, None) => Ok(StoredStatus::Active),
        (STATUS_RETIRED, None) => Ok(StoredStatus::Retired),
        (STATUS_SUPERSEDED, Some(seq)) if seq > 0 => Ok(StoredStatus::Superseded(seq)),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_row(row: &Row<'_>) -> rusqlite::Result<StoredRecord> {
    let raw_id: String = row.get(0)?;
    let raw_market: String = row.get(1)?;
    let raw_symbol: String = row.get(2)?;
    let raw_isin: String = row.get(3)?;
    let raw_status: String = row.get(4)?;
    let superseded_event_seq: Option<i64> = row.get(5)?;

    let id = InstrumentId::try_new(&raw_id).map_err(invalid)?;
    let market = decode_market(&raw_market)?;
    let symbol = Symbol::try_new(&raw_symbol).map_err(invalid)?;
    let isin = IsinCode::try_new(&raw_isin).map_err(invalid)?;
    let status = decode_status(&raw_status, superseded_event_seq)?;

    Ok(StoredRecord {
        source: InstrumentSource::new(market, symbol, isin),
        id,
        status,
    })
}

fn decode_disclosure(source: &str, receipt: &str) -> rusqlite::Result<DisclosureKey> {
    let receipt_no = match source {
        DISCLOSURE_DART => ReceiptNo::Dart(DartReceiptNo::try_new(receipt).map_err(invalid)?),
        DISCLOSURE_KIND => ReceiptNo::Kind(KindReceiptNo::try_new(receipt).map_err(invalid)?),
        DISCLOSURE_SEC_EDGAR => {
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new(receipt).map_err(invalid)?)
        }
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(DisclosureKey::from_receipt(receipt_no))
}

fn decode_share_count(raw: &str) -> rusqlite::Result<ShareCount> {
    if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let parsed = raw.parse::<u64>().map_err(invalid)?;
    ShareCount::try_new(parsed).map_err(invalid)
}

fn decode_timestamp(seconds: i64, nanos: i64, offset: i64) -> rusqlite::Result<Timestamp> {
    let nano = u32::try_from(nanos).map_err(invalid)?;
    let off = i32::try_from(offset).map_err(invalid)?;
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nano),
        OffsetSeconds::new(off),
    )
    .map_err(invalid)
}

struct ActionColumns {
    kind: Option<String>,
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
    from_symbol: Option<String>,
    to_symbol: Option<String>,
    from_count: Option<String>,
    to_count: Option<String>,
}

impl ActionColumns {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            kind: row.get(10)?,
            year: row.get(11)?,
            month: row.get(12)?,
            day: row.get(13)?,
            from_symbol: row.get(14)?,
            to_symbol: row.get(15)?,
            from_count: row.get(16)?,
            to_count: row.get(17)?,
        })
    }

    fn require_absent(&self) -> rusqlite::Result<()> {
        if self.kind.is_some()
            || self.year.is_some()
            || self.month.is_some()
            || self.day.is_some()
            || self.from_symbol.is_some()
            || self.to_symbol.is_some()
            || self.from_count.is_some()
            || self.to_count.is_some()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }

    fn decode_symbol_change(
        self,
        previous: &InstrumentSource,
        next: &InstrumentSource,
    ) -> rusqlite::Result<CorporateActionKind> {
        if self.from_count.is_some() || self.to_count.is_some() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let (Some(raw_from), Some(raw_to)) = (self.from_symbol, self.to_symbol) else {
            return Err(rusqlite::Error::InvalidQuery);
        };
        let from = Symbol::try_new(&raw_from).map_err(invalid)?;
        let to = Symbol::try_new(&raw_to).map_err(invalid)?;
        if from.as_str().eq_ignore_ascii_case(to.as_str())
            || !from
                .as_str()
                .eq_ignore_ascii_case(previous.symbol().as_str())
            || !to.as_str().eq_ignore_ascii_case(next.symbol().as_str())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(CorporateActionKind::SymbolChange { from, to })
    }

    fn decode_split_ratio(self) -> rusqlite::Result<ShareRatio> {
        if self.from_symbol.is_some() || self.to_symbol.is_some() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let (Some(raw_from), Some(raw_to)) = (self.from_count, self.to_count) else {
            return Err(rusqlite::Error::InvalidQuery);
        };
        Ok(ShareRatio::new(
            decode_share_count(&raw_from)?,
            decode_share_count(&raw_to)?,
        ))
    }

    fn decode(
        self,
        instrument_id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
    ) -> rusqlite::Result<CorporateAction> {
        let (Some(raw_kind), Some(raw_year), Some(raw_month), Some(raw_day)) =
            (self.kind.clone(), self.year, self.month, self.day)
        else {
            return Err(rusqlite::Error::InvalidQuery);
        };
        let year = i32::try_from(raw_year).map_err(invalid)?;
        let month = u8::try_from(raw_month).map_err(invalid)?;
        let day = u8::try_from(raw_day).map_err(invalid)?;
        let date = CalendarDate::from_parts(
            CalendarYear::new(year),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .map_err(invalid)?;
        let kind = match raw_kind.as_str() {
            ACTION_SYMBOL_CHANGE => self.decode_symbol_change(previous, next)?,
            ACTION_SPLIT => CorporateActionKind::Split(self.decode_split_ratio()?),
            ACTION_REVERSE_SPLIT => CorporateActionKind::ReverseSplit(self.decode_split_ratio()?),
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        CorporateAction::try_new(CorporateActionId::new(instrument_id.clone()), date, kind)
            .map_err(invalid)
    }
}

struct EventHeader {
    instrument_id: InstrumentId,
    market: KnownMarket,
    previous: InstrumentSource,
    disclosure: DisclosureKey,
    note: UserReason,
    timestamp: Timestamp,
}

fn decode_event_header(row: &Row<'_>) -> rusqlite::Result<EventHeader> {
    let raw_id: String = row.get(2)?;
    let raw_market: String = row.get(3)?;
    let raw_prev_symbol: String = row.get(4)?;
    let raw_prev_isin: String = row.get(5)?;
    let raw_disclosure_source: String = row.get(8)?;
    let raw_disclosure_receipt: String = row.get(9)?;
    let raw_note: String = row.get(18)?;
    let recorded_seconds: i64 = row.get(19)?;
    let recorded_nanos: i64 = row.get(20)?;
    let recorded_offset: i64 = row.get(21)?;

    let instrument_id = InstrumentId::try_new(&raw_id).map_err(invalid)?;
    let market = decode_market(&raw_market)?;
    let previous = InstrumentSource::new(
        market,
        Symbol::try_new(&raw_prev_symbol).map_err(invalid)?,
        IsinCode::try_new(&raw_prev_isin).map_err(invalid)?,
    );
    let disclosure = decode_disclosure(&raw_disclosure_source, &raw_disclosure_receipt)?;
    let note = UserReason::try_new(UntrustedText::try_new(&raw_note).map_err(invalid)?)
        .map_err(invalid)?;
    let timestamp = decode_timestamp(recorded_seconds, recorded_nanos, recorded_offset)?;
    Ok(EventHeader {
        instrument_id,
        market,
        previous,
        disclosure,
        note,
        timestamp,
    })
}

fn decode_next_source(
    market: KnownMarket,
    raw_next_symbol: Option<String>,
    raw_next_isin: Option<String>,
) -> rusqlite::Result<InstrumentSource> {
    let (Some(next_sym), Some(next_isin)) = (raw_next_symbol, raw_next_isin) else {
        return Err(rusqlite::Error::InvalidQuery);
    };
    Ok(InstrumentSource::new(
        market,
        Symbol::try_new(&next_sym).map_err(invalid)?,
        IsinCode::try_new(&next_isin).map_err(invalid)?,
    ))
}

fn reject_unchanged_source(
    left: &InstrumentSource,
    right: &InstrumentSource,
) -> rusqlite::Result<()> {
    if left
        .symbol()
        .as_str()
        .eq_ignore_ascii_case(right.symbol().as_str())
        && left
            .isin()
            .as_str()
            .eq_ignore_ascii_case(right.isin().as_str())
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn decode_event_body(
    event_kind: &str,
    header: EventHeader,
    raw_next_symbol: Option<String>,
    raw_next_isin: Option<String>,
    action_columns: ActionColumns,
) -> rusqlite::Result<MappingHistoryEvent> {
    match event_kind {
        EVENT_IDENTIFIER_CHANGED => {
            let next = decode_next_source(header.market, raw_next_symbol, raw_next_isin)?;
            reject_unchanged_source(&header.previous, &next)?;
            let action = action_columns.decode(&header.instrument_id, &header.previous, &next)?;
            Ok(MappingHistoryEvent::IdentifierChanged {
                instrument_id: header.instrument_id,
                previous: header.previous,
                next,
                disclosure: header.disclosure,
                action: Box::new(action),
                note: header.note,
                confirmed_at: header.timestamp,
            })
        }
        EVENT_RETIREMENT_CORRECTED => {
            if raw_next_symbol.is_some() || raw_next_isin.is_some() {
                return Err(rusqlite::Error::InvalidQuery);
            }
            action_columns.require_absent()?;
            Ok(MappingHistoryEvent::RetirementCorrected {
                instrument_id: header.instrument_id,
                source: header.previous,
                disclosure: header.disclosure,
                note: header.note,
                corrected_at: header.timestamp,
            })
        }
        EVENT_LINK_CORRECTED => {
            action_columns.require_absent()?;
            let unlinked_next = decode_next_source(header.market, raw_next_symbol, raw_next_isin)?;
            reject_unchanged_source(&header.previous, &unlinked_next)?;
            Ok(MappingHistoryEvent::LinkCorrected {
                instrument_id: header.instrument_id,
                restored_previous: header.previous,
                unlinked_next,
                disclosure: header.disclosure,
                note: header.note,
                corrected_at: header.timestamp,
            })
        }
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_event_row(row: &Row<'_>) -> rusqlite::Result<StoredEvent> {
    let seq: i64 = row.get(0)?;
    if seq <= 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let event_kind: String = row.get(1)?;
    let header = decode_event_header(row)?;
    let raw_next_symbol: Option<String> = row.get(6)?;
    let raw_next_isin: Option<String> = row.get(7)?;
    let action_columns = ActionColumns::read(row)?;
    let event = decode_event_body(
        &event_kind,
        header,
        raw_next_symbol,
        raw_next_isin,
        action_columns,
    )?;
    Ok(StoredEvent { seq, event })
}

fn encode_sequence_hex(sequence: IssuerSequence) -> String {
    let mut output = String::with_capacity(16);
    for byte in sequence.as_u64().to_be_bytes() {
        for nibble in [byte >> 4_u8, byte & 15_u8] {
            if let Some(digit) = char::from_digit(u32::from(nibble), 16) {
                output.push(digit);
            }
        }
    }
    output
}

fn decode_sequence_hex(raw: &str) -> rusqlite::Result<IssuerSequence> {
    if raw.len() != 16
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let parsed = u64::from_str_radix(raw, 16).map_err(invalid)?;
    IssuerSequence::try_new(parsed).map_err(invalid)
}

#[derive(Default)]
pub(crate) struct MigrationSchemaTracker {
    objects: BTreeMap<(String, String), (String, String)>,
}

impl MigrationSchemaTracker {
    fn from_migrations() -> Self {
        let mut tracker = Self::default();
        for (_, source) in crate::migration::MIGRATIONS {
            tracker.apply_script(source);
        }
        tracker
    }

    pub(crate) fn apply_script(&mut self, source: &str) {
        for raw in source.split(";\n") {
            let statement = raw.trim();
            let cleaned = statement.strip_suffix(';').unwrap_or(statement);
            if !cleaned.is_empty() {
                self.apply(cleaned);
            }
        }
    }

    fn rename_table(&mut self, old_name: &str, new_name: &str) {
        let old_lower = old_name.to_ascii_lowercase();
        let new_lower = new_name.to_ascii_lowercase();
        if let Some((_, ddl)) = self
            .objects
            .remove(&(String::from("table"), old_lower.clone()))
        {
            self.objects.insert(
                (String::from("table"), new_lower.clone()),
                (new_lower.clone(), ddl),
            );
        }
        for (owner, _) in self.objects.values_mut() {
            if *owner == old_lower {
                owner.clone_from(&new_lower);
            }
        }
    }

    fn apply(&mut self, statement: &str) {
        let words: Vec<&str> = statement.split_whitespace().collect();
        match words.as_slice() {
            ["CREATE", "TABLE", table_token, ..] => {
                let table = table_token.split('(').next().unwrap_or(table_token);
                self.objects.insert(
                    (String::from("table"), table.to_ascii_lowercase()),
                    (table.to_ascii_lowercase(), statement.to_owned()),
                );
            }
            ["CREATE", "INDEX", name, "ON", target, ..]
            | ["CREATE", "UNIQUE", "INDEX", name, "ON", target, ..] => {
                let table = target.split('(').next().unwrap_or(target);
                self.objects.insert(
                    (String::from("index"), name.to_ascii_lowercase()),
                    (table.to_ascii_lowercase(), statement.to_owned()),
                );
            }
            ["CREATE", "TRIGGER", name, ..] => {
                if let Some(table) = words
                    .iter()
                    .zip(words.iter().skip(1))
                    .find(|(left, _)| left.eq_ignore_ascii_case("ON"))
                    .map(|(_, right)| *right)
                {
                    self.objects.insert(
                        (String::from("trigger"), name.to_ascii_lowercase()),
                        (table.to_ascii_lowercase(), statement.to_owned()),
                    );
                }
            }
            ["ALTER", "TABLE", old_name, "RENAME", "TO", new_name] => {
                self.rename_table(old_name, new_name);
            }
            ["DROP", "TABLE", table_name] => {
                let lower = table_name.to_ascii_lowercase();
                self.objects.retain(|_, (owner, _)| *owner != lower);
            }
            _ => {}
        }
    }

    pub(crate) fn mapping_objects(&self) -> BTreeSet<String> {
        self.objects
            .values()
            .filter(|(owner, _)| {
                [
                    "instrument_mappings",
                    "instrument_mapping_events",
                    "instrument_id_issuer",
                ]
                .contains(&owner.as_str())
            })
            .map(|(_, ddl)| ddl.clone())
            .collect()
    }

    pub(crate) fn all_triggers(&self) -> BTreeSet<String> {
        self.objects
            .iter()
            .filter(|((kind, _), _)| kind == "trigger")
            .map(|(_, (_, ddl))| ddl.clone())
            .collect()
    }
}

fn validate_sql_set(
    connection: &Connection,
    query: &str,
    expected: &BTreeSet<String>,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(query)?;
    let actual_rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let actual: BTreeSet<String> = actual_rows
        .iter()
        .map(|entry| entry.trim().to_owned())
        .collect();
    if actual_rows.len() != expected.len() || actual != *expected {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn validate_mapping_objects(connection: &Connection) -> rusqlite::Result<()> {
    let expected = MigrationSchemaTracker::from_migrations().mapping_objects();
    validate_sql_set(connection, sql::SCHEMA_OBJECTS, &expected)
}

pub(crate) fn validate_all_triggers(connection: &Connection) -> rusqlite::Result<()> {
    let expected = MigrationSchemaTracker::from_migrations().all_triggers();
    validate_sql_set(connection, sql::ALL_TRIGGERS, &expected)
}

pub(crate) fn validate_schema(connection: &Connection) -> rusqlite::Result<()> {
    validate_mapping_objects(connection)?;
    validate_all_triggers(connection)
}

pub(crate) fn read_issuer_cursor(
    connection: &Connection,
) -> rusqlite::Result<PersistedIssuerCursor> {
    let mut statement = connection.prepare(sql::ISSUER_CURSOR)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    match rows.as_slice() {
        [] => Ok(PersistedIssuerCursor::Initial),
        [(1, raw)] => Ok(PersistedIssuerCursor::LastIssued(decode_sequence_hex(raw)?)),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub(crate) fn validate_issuer_cursor(connection: &Connection) -> rusqlite::Result<()> {
    let _cursor = read_issuer_cursor(connection)?;
    Ok(())
}

fn load_all_events(connection: &Connection) -> rusqlite::Result<Vec<StoredEvent>> {
    let mut statement = connection.prepare(sql::ALL_EVENTS)?;
    let events = statement
        .query_map([], decode_event_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (index, entry) in events.iter().enumerate() {
        let expected_seq =
            i64::try_from(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?)
                .map_err(invalid)?;
        if entry.seq != expected_seq {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(events)
}

fn load_events_for_id(
    connection: &Connection,
    id: &InstrumentId,
) -> rusqlite::Result<Vec<MappingHistoryEvent>> {
    let mut statement = connection.prepare(sql::EVENTS_FOR_ID)?;
    let stored = statement
        .query_map([id.as_str()], decode_event_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut previous_seq = 0_i64;
    let mut events = Vec::with_capacity(stored.len());
    for entry in stored {
        if entry.seq <= previous_seq {
            return Err(rusqlite::Error::InvalidQuery);
        }
        previous_seq = entry.seq;
        events.push(entry.event);
    }
    Ok(events)
}

#[derive(Default)]
struct RecordSetValidator<'a> {
    seen_exact: BTreeSet<(KnownMarket, Symbol, IsinCode)>,
    seen_normalized_per_id: BTreeSet<(&'a InstrumentId, KnownMarket, String, String)>,
    id_market: BTreeMap<&'a InstrumentId, KnownMarket>,
    terminal_counts: BTreeMap<&'a InstrumentId, usize>,
    symbols: BTreeMap<(KnownMarket, String), &'a InstrumentId>,
    isins: BTreeMap<String, &'a InstrumentId>,
}

impl<'a> RecordSetValidator<'a> {
    fn observe(&mut self, record: &'a StoredRecord) -> rusqlite::Result<()> {
        let exact = (
            record.source.market(),
            record.source.symbol().clone(),
            record.source.isin().clone(),
        );
        if !self.seen_exact.insert(exact) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let norm_symbol = record.source.symbol().as_str().to_ascii_uppercase();
        let norm_isin = record.source.isin().as_str().to_ascii_uppercase();
        if !self.seen_normalized_per_id.insert((
            &record.id,
            record.source.market(),
            norm_symbol.clone(),
            norm_isin.clone(),
        )) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(existing_market) = self.id_market.insert(&record.id, record.source.market())
            && existing_market != record.source.market()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(existing_owner) = self
            .symbols
            .insert((record.source.market(), norm_symbol), &record.id)
            && *existing_owner != record.id
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(existing_owner) = self.isins.insert(norm_isin, &record.id)
            && *existing_owner != record.id
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let counter = self.terminal_counts.entry(&record.id).or_insert(0);
        match record.status {
            StoredStatus::Active | StoredStatus::Retired => {
                *counter = counter
                    .checked_add(1)
                    .ok_or(rusqlite::Error::InvalidQuery)?;
                if *counter > 1 {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            StoredStatus::Superseded(_) => {}
        }
        Ok(())
    }
}

fn validate_records(
    records: &[StoredRecord],
    requirement: TerminalRequirement,
) -> rusqlite::Result<()> {
    let mut validator = RecordSetValidator::default();
    for record in records {
        validator.observe(record)?;
    }
    match requirement {
        TerminalRequirement::AllowPartialSlice => Ok(()),
        TerminalRequirement::RequireEveryIdTerminal => {
            if validator.terminal_counts.values().any(|count| *count != 1) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(())
        }
    }
}

type SupersededLinkKey<'a> = (i64, &'a InstrumentId, KnownMarket, &'a Symbol, &'a IsinCode);

fn expected_superseded_links(
    events: &[StoredEvent],
) -> rusqlite::Result<BTreeSet<SupersededLinkKey<'_>>> {
    let mut active_links_by_id: BTreeMap<&InstrumentId, Vec<(i64, &InstrumentSource)>> =
        BTreeMap::new();
    for entry in events {
        match &entry.event {
            MappingHistoryEvent::IdentifierChanged {
                instrument_id,
                previous,
                ..
            } => {
                active_links_by_id
                    .entry(instrument_id)
                    .or_default()
                    .push((entry.seq, previous));
            }
            MappingHistoryEvent::LinkCorrected { instrument_id, .. } => {
                let popped = active_links_by_id.get_mut(instrument_id).and_then(Vec::pop);
                if popped.is_none() {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            MappingHistoryEvent::RetirementCorrected { .. } => {}
        }
    }
    Ok(active_links_by_id
        .into_iter()
        .flat_map(|(id, links)| {
            links.into_iter().map(move |(seq, source)| {
                (seq, id, source.market(), source.symbol(), source.isin())
            })
        })
        .collect())
}

pub(crate) fn verify_superseded_event_links(
    records: &[StoredRecord],
    events: &[StoredEvent],
) -> rusqlite::Result<()> {
    let expected_links = expected_superseded_links(events)?;
    let mut actual_links = BTreeSet::new();
    for record in records {
        match record.status {
            StoredStatus::Superseded(seq) => {
                let inserted = actual_links.insert((
                    seq,
                    &record.id,
                    record.source.market(),
                    record.source.symbol(),
                    record.source.isin(),
                ));
                if !inserted {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            StoredStatus::Active | StoredStatus::Retired => {}
        }
    }
    if actual_links != expected_links {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn validate_ledger(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(sql::ALL)?;
    let records = statement
        .query_map([], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let events = load_all_events(connection)?;
    let domain_records: Vec<InstrumentMappingRecord> =
        records.iter().map(StoredRecord::to_domain).collect();
    let domain_events: Vec<MappingHistoryEvent> =
        events.iter().map(|entry| entry.event.clone()).collect();
    verify_persisted_mapping_ledger(&domain_records, &domain_events).map_err(invalid)?;
    verify_superseded_event_links(&records, &events)
}

pub(crate) fn validate_on_open(connection: &Connection) -> rusqlite::Result<()> {
    validate_schema(connection)?;
    validate_ledger(connection)?;
    validate_issuer_cursor(connection)
}

fn load_for_source(
    connection: &Connection,
    source: &InstrumentSource,
) -> rusqlite::Result<Vec<StoredRecord>> {
    let mut statement = connection.prepare(sql::BY_SOURCE)?;
    let records = statement
        .query_map(
            params![
                market_tag(source.market()),
                source.symbol().as_str(),
                source.isin().as_str(),
            ],
            decode_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    validate_records(&records, TerminalRequirement::AllowPartialSlice)?;
    Ok(records)
}

fn load_for_source_or_id(
    connection: &Connection,
    source: &InstrumentSource,
    candidate: &InstrumentId,
) -> rusqlite::Result<Vec<StoredRecord>> {
    let mut statement = connection.prepare(sql::BY_SOURCE_OR_ID)?;
    let records = statement
        .query_map(
            params![
                market_tag(source.market()),
                source.symbol().as_str(),
                source.isin().as_str(),
                candidate.as_str(),
            ],
            decode_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    validate_records(&records, TerminalRequirement::AllowPartialSlice)?;
    Ok(records)
}

fn load_for_id(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<Vec<StoredRecord>> {
    let mut statement = connection.prepare(sql::BY_ID)?;
    let records = statement
        .query_map(params![id.as_str()], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    validate_records(&records, TerminalRequirement::RequireEveryIdTerminal)?;
    Ok(records)
}

fn lookup_in_candidates<'a>(
    records: &'a [StoredRecord],
    source: &InstrumentSource,
) -> Result<&'a StoredRecord, InstrumentMappingError> {
    if let Some(entry) = records.iter().find(|entry| entry.source == *source) {
        return Ok(entry);
    }
    let mut sources: Vec<InstrumentSource> =
        records.iter().map(|entry| entry.source.clone()).collect();
    sources.push(source.clone());
    match instrument_source_conflicts(&sources).last() {
        Some(InstrumentSourceConflict::IdentityUncertain) => {
            Err(InstrumentMappingError::IdentityUncertain)
        }
        Some(InstrumentSourceConflict::Distinct) | None => Err(InstrumentMappingError::Missing),
    }
}

fn resolve_in_candidates(
    records: &[StoredRecord],
    source: &InstrumentSource,
) -> Result<InstrumentId, InstrumentMappingError> {
    let entry = lookup_in_candidates(records, source)?;
    match entry.status {
        StoredStatus::Active => Ok(entry.id.clone()),
        StoredStatus::Superseded(_) => Err(InstrumentMappingError::Superseded),
        StoredStatus::Retired => Err(InstrumentMappingError::Retired),
    }
}

fn mapping_id_presence(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<IdPresence> {
    let mut statement = connection.prepare(sql::ID_EXISTS)?;
    let found: Option<i64> = statement
        .query_row([id.as_str()], |row| row.get(0))
        .optional()?;
    match found {
        Some(_) => Ok(IdPresence::Present),
        None => Ok(IdPresence::Absent),
    }
}

fn persist_issuer_cursor(
    transaction: &Transaction<'_>,
    previous: PersistedIssuerCursor,
    next: IssuerSequence,
) -> rusqlite::Result<()> {
    let encoded_next = encode_sequence_hex(next);
    let affected = match previous {
        PersistedIssuerCursor::Initial => {
            transaction.execute(sql::INSERT_ISSUER_CURSOR, [encoded_next.as_str()])?
        }
        PersistedIssuerCursor::LastIssued(prev_seq) => {
            let encoded_prev = encode_sequence_hex(prev_seq);
            transaction.execute(
                sql::UPDATE_ISSUER_CURSOR,
                params![encoded_next.as_str(), encoded_prev.as_str()],
            )?
        }
    };
    if affected != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn issue_next(transaction: &Transaction<'_>) -> rusqlite::Result<IssuedInstrumentId> {
    let previous = read_issuer_cursor(transaction)?;
    let mut issuer = IndependentInstrumentIdIssuer::restore(previous, &[]);
    loop {
        let candidate = issuer.issue().map_err(invalid)?;
        match mapping_id_presence(transaction, candidate.as_instrument_id())? {
            IdPresence::Present => {}
            IdPresence::Absent => {
                match issuer.last_issued() {
                    PersistedIssuerCursor::LastIssued(next_seq) => {
                        persist_issuer_cursor(transaction, previous, next_seq)?;
                    }
                    PersistedIssuerCursor::Initial => return Err(rusqlite::Error::InvalidQuery),
                }
                return Ok(candidate);
            }
        }
    }
}

fn ensure_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
    candidate: &IssuedInstrumentId,
) -> rusqlite::Result<Result<MappingAssignment, InstrumentMappingError>> {
    let records = load_for_source_or_id(transaction, source, candidate.as_instrument_id())?;
    match resolve_in_candidates(&records, source) {
        Ok(existing_id) => return Ok(Ok(MappingAssignment::Existing(existing_id))),
        Err(InstrumentMappingError::Missing) => {}
        Err(error) => return Ok(Err(error)),
    }
    if records
        .iter()
        .any(|entry| entry.id == *candidate.as_instrument_id())
    {
        return Ok(Err(InstrumentMappingError::IdAlreadyUsed));
    }
    let issued_id = candidate.as_instrument_id().clone();
    let inserted = transaction.execute(
        sql::INSERT,
        params![
            issued_id.as_str(),
            market_tag(source.market()),
            source.symbol().as_str(),
            source.isin().as_str(),
            STATUS_ACTIVE,
        ],
    )?;
    if inserted != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Ok(MappingAssignment::Issued(issued_id)))
}

fn retire_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
) -> rusqlite::Result<Result<InstrumentId, InstrumentMappingError>> {
    let records = load_for_source(transaction, source)?;
    let existing = match lookup_in_candidates(&records, source) {
        Ok(record) => record,
        Err(error) => return Ok(Err(error)),
    };
    match existing.status {
        StoredStatus::Retired => Ok(Ok(existing.id.clone())),
        StoredStatus::Superseded(_) => Ok(Err(InstrumentMappingError::Superseded)),
        StoredStatus::Active => {
            let updated = transaction.execute(
                sql::RETIRE,
                params![
                    existing.id.as_str(),
                    market_tag(source.market()),
                    source.symbol().as_str(),
                    source.isin().as_str(),
                ],
            )?;
            if updated != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(Ok(existing.id.clone()))
        }
    }
}

fn map_result<T>(
    outcome: Result<Result<T, InstrumentMappingError>, SystemPortError>,
) -> Result<T, MappingStoreError> {
    match outcome {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(MappingStoreError::Mapping(error)),
        Err(error) => Err(MappingStoreError::Storage(error)),
    }
}

impl InstrumentIdIssuer for InstrumentIdIssuerStore<'_> {
    async fn issue(&self) -> Result<IssuedInstrumentId, SystemPortError> {
        self.database.write(issue_next).await
    }
}

impl InstrumentMappingRepository for InstrumentMappingStore<'_> {
    async fn resolve(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .read(move |connection| {
                    let records = load_for_source(connection, &owned_source)?;
                    Ok(resolve_in_candidates(&records, &owned_source))
                })
                .await,
        )
    }

    async fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentMappingRecord, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .read(move |connection| {
                    let records = load_for_source(connection, &owned_source)?;
                    Ok(lookup_in_candidates(&records, &owned_source).map(StoredRecord::to_domain))
                })
                .await,
        )
    }

    async fn for_id(
        &self,
        id: &InstrumentId,
    ) -> Result<Vec<InstrumentMappingRecord>, MappingStoreError> {
        let owned_id = id.clone();
        map_result(
            self.database
                .read(move |connection| {
                    let records = load_for_id(connection, &owned_id)?;
                    if records.is_empty() {
                        return Ok(Err(InstrumentMappingError::Missing));
                    }
                    Ok(Ok(records
                        .into_iter()
                        .map(StoredRecord::into_domain)
                        .collect()))
                })
                .await,
        )
    }

    async fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: IssuedInstrumentId,
    ) -> Result<MappingAssignment, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .write(move |transaction| ensure_mapping(transaction, &owned_source, &candidate))
                .await,
        )
    }

    async fn retire(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .write(move |transaction| retire_mapping(transaction, &owned_source))
                .await,
        )
    }
}

impl InstrumentMappingHistoryRepository for InstrumentMappingStore<'_> {
    async fn history(&self) -> Result<Vec<MappingHistoryEvent>, SystemPortError> {
        self.database
            .read(|connection| {
                Ok(load_all_events(connection)?
                    .into_iter()
                    .map(|entry| entry.event)
                    .collect())
            })
            .await
    }

    async fn history_for_id(
        &self,
        id: &InstrumentId,
    ) -> Result<Vec<MappingHistoryEvent>, SystemPortError> {
        let owned_id = id.clone();
        self.database
            .read(move |connection| load_events_for_id(connection, &owned_id))
            .await
    }
}
