use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read;
use std::sync::Arc;

use application::{DataAvailability, InformationError, ResponseMismatch, SecretValue};
use domain::{
    CalendarDate, DisplayInstrumentName, InstrumentId, KnownMarket, MasterInstrumentFacts, Symbol,
    UntrustedText,
};
use flate2::read::DeflateDecoder;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::dart_disclosures::{DartCorpCode, map_dart_status_code, parse_yyyymmdd};
use crate::{
    HttpDataSource, SourceCredential, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    collapse_whitespace, http_error_to_information, http_status_to_error,
};

const MAX_XML_UNCOMPRESSED_BYTES: usize = 67_108_864;
const ZIP_EOCD_SIG: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
const ZIP_CD_SIG: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
const ZIP_LFH_SIG: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartCorpRecord {
    pub corp_code: DartCorpCode,
    pub corp_name: UntrustedText,
    pub corp_eng_name: Option<UntrustedText>,
    pub stock_code: Option<Symbol>,
    pub modify_date: CalendarDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartMatchFailureReason {
    MissingInDart,
    AmbiguousStockCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedDartInstrument {
    pub instrument_id: InstrumentId,
    pub symbol: Symbol,
    pub corp_code: DartCorpCode,
    pub corp_name: UntrustedText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmatchedDartInstrument {
    pub instrument_id: InstrumentId,
    pub symbol: Symbol,
    pub master_name: DisplayInstrumentName,
    pub reason: DartMatchFailureReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartCorpReconciliation {
    pub matched: Vec<MatchedDartInstrument>,
    pub unmatched: Vec<UnmatchedDartInstrument>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DartCorpCodeMap {
    by_corp_code: BTreeMap<DartCorpCode, DartCorpRecord>,
    by_stock_code: BTreeMap<Symbol, DartCorpCode>,
    ambiguous_symbols: BTreeSet<Symbol>,
    invalid_stock_code_count: usize,
}

impl DartCorpCodeMap {
    #[must_use]
    pub fn from_records(records: Vec<DartCorpRecord>, invalid_stock_code_count: usize) -> Self {
        let mut by_corp_code = BTreeMap::new();
        for record in records {
            by_corp_code.insert(record.corp_code.clone(), record);
        }
        let mut map = Self {
            by_corp_code,
            by_stock_code: BTreeMap::new(),
            ambiguous_symbols: BTreeSet::new(),
            invalid_stock_code_count,
        };
        map.rebuild_indexes();
        map
    }

    pub fn merge_updates(&mut self, incoming: Vec<DartCorpRecord>) {
        for record in incoming {
            let should_replace = self
                .by_corp_code
                .get(&record.corp_code)
                .is_none_or(|existing| record.modify_date >= existing.modify_date);
            if should_replace {
                self.by_corp_code.insert(record.corp_code.clone(), record);
            }
        }
        self.rebuild_indexes();
    }

    fn rebuild_indexes(&mut self) {
        self.by_stock_code.clear();
        self.ambiguous_symbols.clear();
        let mut counts: BTreeMap<Symbol, Vec<DartCorpCode>> = BTreeMap::new();
        for record in self.by_corp_code.values() {
            if let Some(symbol) = &record.stock_code {
                counts
                    .entry(symbol.clone())
                    .or_default()
                    .push(record.corp_code.clone());
            }
        }
        for (symbol, codes) in counts {
            if let [single] = codes.as_slice() {
                self.by_stock_code.insert(symbol, single.clone());
            } else if codes.len() > 1_usize {
                self.ambiguous_symbols.insert(symbol);
            }
        }
    }

    #[must_use]
    pub fn corp_code_for_symbol(&self, symbol: &Symbol) -> Option<&DartCorpCode> {
        self.by_stock_code.get(symbol)
    }

    #[must_use]
    pub fn record_for_corp_code(&self, code: &DartCorpCode) -> Option<&DartCorpRecord> {
        self.by_corp_code.get(code)
    }

    #[must_use]
    pub const fn ambiguous_symbols(&self) -> &BTreeSet<Symbol> {
        &self.ambiguous_symbols
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_corp_code.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_corp_code.is_empty()
    }

    #[must_use]
    pub const fn invalid_stock_code_count(&self) -> usize {
        self.invalid_stock_code_count
    }

    #[must_use]
    pub fn reconcile_with_master(&self, master: &MasterInstrumentFacts) -> DartCorpReconciliation {
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for instrument in master.instruments() {
            let Ok(quote) = instrument.quote_instrument() else {
                continue;
            };
            if *quote.market() != KnownMarket::Kr {
                continue;
            }
            let row = instrument.row();
            if self.ambiguous_symbols.contains(&row.symbol) {
                unmatched.push(UnmatchedDartInstrument {
                    instrument_id: row.instrument_id.clone(),
                    symbol: row.symbol.clone(),
                    master_name: row.name.clone(),
                    reason: DartMatchFailureReason::AmbiguousStockCode,
                });
                continue;
            }
            if let Some(corp_code) = self.by_stock_code.get(&row.symbol)
                && let Some(record) = self.by_corp_code.get(corp_code)
            {
                matched.push(MatchedDartInstrument {
                    instrument_id: row.instrument_id.clone(),
                    symbol: row.symbol.clone(),
                    corp_code: corp_code.clone(),
                    corp_name: record.corp_name.clone(),
                });
            } else {
                unmatched.push(UnmatchedDartInstrument {
                    instrument_id: row.instrument_id.clone(),
                    symbol: row.symbol.clone(),
                    master_name: row.name.clone(),
                    reason: DartMatchFailureReason::MissingInDart,
                });
            }
        }
        DartCorpReconciliation { matched, unmatched }
    }
}

fn schema_error() -> InformationError {
    InformationError::InvalidResponse(ResponseMismatch::Schema)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, InformationError> {
    let slice = bytes
        .get(offset..offset.checked_add(2_usize).ok_or_else(schema_error)?)
        .ok_or_else(schema_error)?;
    let pair: [u8; 2] = slice.try_into().map_err(|_error| schema_error())?;
    Ok(u16::from_le_bytes(pair))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, InformationError> {
    let slice = bytes
        .get(offset..offset.checked_add(4_usize).ok_or_else(schema_error)?)
        .ok_or_else(schema_error)?;
    let quad: [u8; 4] = slice.try_into().map_err(|_error| schema_error())?;
    Ok(u32::from_le_bytes(quad))
}

fn find_eocd_offset(archive: &[u8]) -> Result<usize, InformationError> {
    if archive.len() < 22_usize {
        return Err(schema_error());
    }
    let min_pos = archive.len().saturating_sub(65_557_usize);
    let max_pos = archive.len().saturating_sub(22_usize);
    for pos in (min_pos..=max_pos).rev() {
        if archive.get(pos..pos.saturating_add(4_usize)) == Some(&ZIP_EOCD_SIG) {
            return Ok(pos);
        }
    }
    Err(schema_error())
}

fn decompress_zip_slice(
    compressed: &[u8],
    compression_code: u16,
    declared_uncompressed: usize,
) -> Result<Vec<u8>, InformationError> {
    if declared_uncompressed > MAX_XML_UNCOMPRESSED_BYTES {
        return Err(schema_error());
    }
    match compression_code {
        0_u16 => {
            if compressed.len() > MAX_XML_UNCOMPRESSED_BYTES {
                return Err(schema_error());
            }
            Ok(compressed.to_vec())
        }
        8_u16 => {
            let limit = u64::try_from(MAX_XML_UNCOMPRESSED_BYTES.saturating_add(1_usize))
                .map_err(|_error| schema_error())?;
            let mut decoder = DeflateDecoder::new(compressed).take(limit);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|_error| schema_error())?;
            if out.len() > MAX_XML_UNCOMPRESSED_BYTES {
                return Err(schema_error());
            }
            Ok(out)
        }
        _ => Err(schema_error()),
    }
}

fn read_local_entry_data(
    archive: &[u8],
    local_offset: usize,
    compressed_size: usize,
) -> Result<&[u8], InformationError> {
    if archive.get(local_offset..local_offset.saturating_add(4_usize)) != Some(&ZIP_LFH_SIG) {
        return Err(schema_error());
    }
    let name_len = usize::from(read_u16_le(archive, local_offset.saturating_add(26_usize))?);
    let extra_len = usize::from(read_u16_le(archive, local_offset.saturating_add(28_usize))?);
    let data_start = local_offset
        .checked_add(30_usize)
        .and_then(|pos| pos.checked_add(name_len))
        .and_then(|pos| pos.checked_add(extra_len))
        .ok_or_else(schema_error)?;
    let data_end = data_start
        .checked_add(compressed_size)
        .ok_or_else(schema_error)?;
    archive.get(data_start..data_end).ok_or_else(schema_error)
}

pub fn extract_corp_code_xml(payload: &[u8]) -> Result<Vec<u8>, InformationError> {
    if payload.is_empty() {
        return Err(schema_error());
    }
    if payload.get(0_usize..4_usize) != Some(&ZIP_LFH_SIG) {
        return Ok(payload.to_vec());
    }
    let eocd = find_eocd_offset(payload)?;
    let entry_count = usize::from(read_u16_le(payload, eocd.saturating_add(10_usize))?);
    let cd_offset = usize::try_from(read_u32_le(payload, eocd.saturating_add(16_usize))?)
        .map_err(|_error| schema_error())?;
    let mut cursor = cd_offset;
    for _index in 0_usize..entry_count {
        if payload.get(cursor..cursor.saturating_add(4_usize)) != Some(&ZIP_CD_SIG) {
            return Err(schema_error());
        }
        let compression_code = read_u16_le(payload, cursor.saturating_add(10_usize))?;
        let compressed_size =
            usize::try_from(read_u32_le(payload, cursor.saturating_add(20_usize))?)
                .map_err(|_error| schema_error())?;
        let uncompressed_size =
            usize::try_from(read_u32_le(payload, cursor.saturating_add(24_usize))?)
                .map_err(|_error| schema_error())?;
        let name_len = usize::from(read_u16_le(payload, cursor.saturating_add(28_usize))?);
        let extra_len = usize::from(read_u16_le(payload, cursor.saturating_add(30_usize))?);
        let comment_len = usize::from(read_u16_le(payload, cursor.saturating_add(32_usize))?);
        let local_offset = usize::try_from(read_u32_le(payload, cursor.saturating_add(42_usize))?)
            .map_err(|_error| schema_error())?;
        let name_start = cursor.checked_add(46_usize).ok_or_else(schema_error)?;
        let name_end = name_start.checked_add(name_len).ok_or_else(schema_error)?;
        let name_bytes = payload.get(name_start..name_end).ok_or_else(schema_error)?;
        if let Ok(name) = std::str::from_utf8(name_bytes)
            && name.to_ascii_lowercase().ends_with(".xml")
        {
            let compressed = read_local_entry_data(payload, local_offset, compressed_size)?;
            return decompress_zip_slice(compressed, compression_code, uncompressed_size);
        }
        cursor = name_end
            .checked_add(extra_len)
            .and_then(|pos| pos.checked_add(comment_len))
            .ok_or_else(schema_error)?;
    }
    Err(schema_error())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorpXmlField {
    None,
    Status,
    CorpCode,
    CorpName,
    CorpEngName,
    StockCode,
    ModifyDate,
}

#[derive(Default)]
struct CorpRowBuilder {
    corp_code: String,
    corp_name: String,
    corp_eng_name: String,
    stock_code: String,
    modify_date: String,
}

impl CorpRowBuilder {
    fn append(&mut self, field: CorpXmlField, text: &str) {
        match field {
            CorpXmlField::CorpCode => self.corp_code.push_str(text),
            CorpXmlField::CorpName => self.corp_name.push_str(text),
            CorpXmlField::CorpEngName => self.corp_eng_name.push_str(text),
            CorpXmlField::StockCode => self.stock_code.push_str(text),
            CorpXmlField::ModifyDate => self.modify_date.push_str(text),
            CorpXmlField::None | CorpXmlField::Status => {}
        }
    }

    fn finish(self, invalid_stock_codes: &mut usize) -> Result<DartCorpRecord, InformationError> {
        let corp_code = DartCorpCode::try_new(&self.corp_code)?;
        let clean_name = collapse_whitespace(&self.corp_name);
        if clean_name.is_empty() {
            return Err(schema_error());
        }
        let corp_name = UntrustedText::try_new(&clean_name).map_err(|_error| schema_error())?;
        let clean_eng = collapse_whitespace(&self.corp_eng_name);
        let corp_eng_name = if clean_eng.is_empty() {
            None
        } else {
            Some(UntrustedText::try_new(&clean_eng).map_err(|_error| schema_error())?)
        };
        let raw_stock = self.stock_code.trim();
        let stock_code = if raw_stock.is_empty() {
            None
        } else if let Ok(symbol) = Symbol::try_new(raw_stock) {
            Some(symbol)
        } else {
            *invalid_stock_codes = invalid_stock_codes.saturating_add(1_usize);
            None
        };
        let modify_date = parse_yyyymmdd(&self.modify_date)?;
        Ok(DartCorpRecord {
            corp_code,
            corp_name,
            corp_eng_name,
            stock_code,
            modify_date,
        })
    }
}

struct CorpXmlParser {
    saw_result_root: bool,
    status_text: String,
    current: Option<CorpRowBuilder>,
    field: CorpXmlField,
    records: Vec<DartCorpRecord>,
    invalid_stock_codes: usize,
}

impl CorpXmlParser {
    fn new() -> Self {
        Self {
            saw_result_root: false,
            status_text: String::new(),
            current: None,
            field: CorpXmlField::None,
            records: Vec::new(),
            invalid_stock_codes: 0_usize,
        }
    }

    fn on_start(&mut self, tag: &str) -> Result<(), InformationError> {
        let lower = tag.to_ascii_lowercase();
        if !self.saw_result_root {
            if lower != "result" {
                return Err(schema_error());
            }
            self.saw_result_root = true;
            return Ok(());
        }
        if self.current.is_none() {
            if lower == "list" {
                self.current = Some(CorpRowBuilder::default());
            } else if lower == "status" {
                self.field = CorpXmlField::Status;
            }
            return Ok(());
        }
        self.field = match lower.as_str() {
            "corp_code" => CorpXmlField::CorpCode,
            "corp_name" => CorpXmlField::CorpName,
            "corp_eng_name" => CorpXmlField::CorpEngName,
            "stock_code" => CorpXmlField::StockCode,
            "modify_date" => CorpXmlField::ModifyDate,
            _ => CorpXmlField::None,
        };
        Ok(())
    }

    fn on_end(&mut self, tag: &str) -> Result<(), InformationError> {
        if tag.eq_ignore_ascii_case("list")
            && let Some(builder) = self.current.take()
        {
            self.records
                .push(builder.finish(&mut self.invalid_stock_codes)?);
        }
        self.field = CorpXmlField::None;
        Ok(())
    }

    fn on_text(&mut self, text: &str) {
        if self.field == CorpXmlField::Status {
            self.status_text.push_str(text);
        } else if let Some(builder) = self.current.as_mut() {
            builder.append(self.field, text);
        }
    }
}

pub fn parse_corp_code_xml(xml_bytes: &[u8]) -> Result<DartCorpCodeMap, InformationError> {
    let text = std::str::from_utf8(xml_bytes)
        .map_err(|_error| schema_error())?
        .trim_start_matches('\u{feff}')
        .trim();
    if text.is_empty() {
        return Err(schema_error());
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut parser = CorpXmlParser::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => parser.on_start(start.name().as_ref())?,
            Ok(Event::End(end)) => parser.on_end(end.name().as_ref())?,
            Ok(Event::Text(value)) => parser.on_text(value.as_ref()),
            Ok(Event::CData(value)) => parser.on_text(value.as_ref()),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_error) => return Err(schema_error()),
        }
    }
    if !parser.saw_result_root || parser.current.is_some() {
        return Err(schema_error());
    }
    if !parser.status_text.trim().is_empty() {
        let _availability = map_dart_status_code(&parser.status_text)?;
    }
    Ok(DartCorpCodeMap::from_records(
        parser.records,
        parser.invalid_stock_codes,
    ))
}

pub struct DartCorpMapClient {
    client: Arc<SourceHttpClient>,
    api_key: Option<Arc<SecretValue>>,
}

impl fmt::Debug for DartCorpMapClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DartCorpMapClient")
            .field("configured", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl DartCorpMapClient {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>, api_key: Option<Arc<SecretValue>>) -> Self {
        Self { client, api_key }
    }

    pub async fn fetch_corp_code_map(
        &self,
    ) -> Result<DataAvailability<DartCorpCodeMap>, InformationError> {
        let Some(secret) = &self.api_key else {
            return Ok(DataAvailability::NotConfigured);
        };
        let credential = SourceCredential::new(
            application::CredentialSource::Dart,
            SecretValue::new(secret.as_exposed_secret().to_vec()),
        );
        let fetch = SourceFetch::new(
            HttpDataSource::DartDisclosureApi,
            "https://opendart.fss.or.kr/api/corpCode.xml",
        )
        .with_credential(credential);
        let outcome =
            self.client.fetch(&fetch).await.map_err(|error| {
                http_error_to_information(HttpDataSource::DartDisclosureApi, error)
            })?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(DataAvailability::NoData),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(
                    HttpDataSource::DartDisclosureApi,
                    status,
                ));
            }
        };
        let xml_bytes = extract_corp_code_xml(payload.body())?;
        let map = parse_corp_code_xml(&xml_bytes)?;
        if map.is_empty() {
            Ok(DataAvailability::NoData)
        } else {
            Ok(DataAvailability::Available(map))
        }
    }
}
