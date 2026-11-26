use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossInstrumentRead};
use application::{
    Clock, InstrumentIdIssuer, InstrumentMappingRepository, InstrumentMasterAuthority,
    InstrumentMasterObservation, InstrumentMasterSession, MappingStoreError, MasterNotReady,
    PortError, PortErrorKind, ReadEndpoint, SecretStore,
};
use broker_toss_schema::types::{KrMarketDetail, ListedStock, StockInfo};
use domain::{
    CommonShare, Currency, DisplayInstrumentName, InstrumentId, InstrumentMappingError,
    InstrumentMaster, InstrumentMasterEntry, InstrumentSource, IsinCode, KnownMarket,
    KoreanMasterDetail, LiquidationTrading, ListingStatus, MappingAssignment, MasterCode,
    MasterInstrument, MasterInstrumentError, MasterInstrumentRow, MasterSecurityType, NxtSupport,
    NxtTrading, Symbol, TradingSuspension,
};
use std::collections::{BTreeMap, BTreeSet};

struct Listed {
    exchange: &'static str,
    stock: ListedStock,
    source: InstrumentSource,
    symbol: Symbol,
}

struct Prepared {
    source: InstrumentSource,
    row: MasterInstrumentRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowPolicy {
    Strict,
    SkipInvalid,
}

struct Rows {
    policy: RowPolicy,
    skipped: BTreeMap<String, usize>,
}

impl Rows {
    const fn new(policy: RowPolicy) -> Self {
        Self {
            policy,
            skipped: BTreeMap::new(),
        }
    }

    fn reject(&mut self, stage: &str) -> Result<(), PortError> {
        match self.policy {
            RowPolicy::Strict => Err(failed(stage)),
            RowPolicy::SkipInvalid => {
                let count = self.skipped.entry(stage.to_owned()).or_insert(0);
                *count = count.saturating_add(1);
                Ok(())
            }
        }
    }

    fn report(&self) {
        for (stage, count) in &self.skipped {
            crate::diagnostics::validation_failed(
                ReadEndpoint::Instruments,
                &format!("skipped {count} instrument rows: {stage}"),
            );
        }
    }
}

fn failed(stage: &str) -> PortError {
    crate::diagnostics::validation_failed(ReadEndpoint::Instruments, stage);
    invalid()
}

fn invalid() -> PortError {
    crate::market_read::invalid(ReadEndpoint::Instruments)
}

fn not_ready(reason: MasterNotReady) -> PortError {
    PortError::new(
        ReadEndpoint::Instruments,
        application::HttpStatus::NotReceived,
        PortErrorKind::MasterNotReady(reason),
    )
}

fn not_ready_after_response(reason: MasterNotReady) -> PortError {
    PortError::new(
        ReadEndpoint::Instruments,
        application::HttpStatus::Success,
        PortErrorKind::MasterNotReady(reason),
    )
}

fn exchanges(market: KnownMarket) -> &'static [&'static str] {
    match market {
        KnownMarket::Kr => &["KOSPI", "KOSDAQ", "KR_ETC"],
        KnownMarket::Us => &["NYSE", "NASDAQ", "AMEX", "US_ETC"],
    }
}

fn decoded_rows<T: serde::de::DeserializeOwned>(
    response: serde_json::Value,
    rows: &mut Rows,
    stage: &str,
) -> Result<Vec<T>, PortError> {
    let values: Vec<serde_json::Value> = serde_json::from_value(response).map_err(|error| {
        crate::diagnostics::decode_failed(ReadEndpoint::Instruments, &error);
        invalid()
    })?;
    let mut decoded = Vec::new();
    for value in values {
        match serde_json::from_value::<T>(value) {
            Ok(row) => decoded.push(row),
            Err(error) => match rows.policy {
                RowPolicy::Strict => {
                    crate::diagnostics::decode_failed(ReadEndpoint::Instruments, &error);
                    return Err(invalid());
                }
                RowPolicy::SkipInvalid => rows.reject(&format!(
                    "{stage} ({})",
                    crate::diagnostics::serde_detail(&error)
                ))?,
            },
        }
    }
    Ok(decoded)
}

fn listed_row(
    exchange: &'static str,
    market: KnownMarket,
    stock: ListedStock,
) -> Result<Listed, &'static str> {
    const SYMBOL: &str = "listed symbol is not a valid symbol";
    let symbol = Symbol::try_new(&stock.symbol.to_ascii_uppercase()).map_err(|_error| SYMBOL)?;
    let source = InstrumentSource::new(
        market,
        Symbol::try_new(&stock.symbol).map_err(|_error| SYMBOL)?,
        IsinCode::try_new(&stock.isin_code).map_err(|_error| "listed ISIN is empty or invalid")?,
    );
    DisplayInstrumentName::try_new(&stock.name)
        .map_err(|_error| "listed name is empty or invalid")?;
    MasterCode::try_new(&stock.security_type.to_string())
        .map_err(|_error| "listed security type is empty or invalid")?;
    Ok(Listed {
        exchange,
        stock,
        source,
        symbol,
    })
}

async fn listed<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &crate::TossAccountRead<S, T, R>,
    market: KnownMarket,
    rows: &mut Rows,
) -> Result<Vec<Listed>, PortError> {
    let mut result = Vec::new();
    let mut symbols = BTreeSet::new();
    for &exchange in exchanges(market) {
        let response = reader
            .market_read(
                ReadRequest::new(ReadRoute::Instruments)
                    .with_query(QueryKey::Market, exchange.to_owned()),
                ReadEndpoint::Instruments,
            )
            .await?;
        let stocks: Vec<ListedStock> =
            decoded_rows(response, rows, "listing row does not match the schema")?;
        if stocks.len() > 8192 {
            return Err(failed("listing exceeds the row limit"));
        }
        for stock in stocks {
            let listed = match listed_row(exchange, market, stock) {
                Ok(listed) => listed,
                Err(stage) => {
                    rows.reject(stage)?;
                    continue;
                }
            };
            if !symbols.insert(listed.symbol.clone()) {
                rows.reject("duplicate listed symbol")?;
                continue;
            }
            result.push(listed);
        }
    }
    Ok(result)
}

fn nxt_trading(
    support: NxtSupport,
    state: Option<TradingSuspension>,
    policy: RowPolicy,
) -> Result<NxtTrading, &'static str> {
    match (NxtTrading::try_from_parts(support, state), policy, support) {
        (Ok(trading), _, _) => Ok(trading),
        (Err(_error), RowPolicy::SkipInvalid, NxtSupport::Unsupported) => {
            Ok(NxtTrading::Unsupported)
        }
        (Err(_error), RowPolicy::SkipInvalid, NxtSupport::Supported) => {
            Ok(NxtTrading::Supported(TradingSuspension::Suspended))
        }
        (Err(_error), RowPolicy::Strict, _) => Err("NXT support and NXT suspension disagree"),
    }
}

fn korean_detail(
    value: Option<KrMarketDetail>,
    market: KnownMarket,
    policy: RowPolicy,
) -> Result<Option<KoreanMasterDetail>, &'static str> {
    let Some(detail) = value else {
        return Ok(None);
    };
    if market == KnownMarket::Us && policy == RowPolicy::SkipInvalid {
        return Ok(None);
    }
    let suspension = |state| {
        if state {
            TradingSuspension::Suspended
        } else {
            TradingSuspension::Clear
        }
    };
    let support = if detail.nxt_supported {
        NxtSupport::Supported
    } else {
        NxtSupport::Unsupported
    };
    Ok(Some(KoreanMasterDetail {
        liquidation: if detail.liquidation_trading {
            LiquidationTrading::Liquidation
        } else {
            LiquidationTrading::Regular
        },
        krx: suspension(detail.krx_trading_suspended),
        nxt: nxt_trading(
            support,
            detail.nxt_trading_suspended.map(suspension),
            policy,
        )?,
    }))
}

fn listing_market(
    listed: &Listed,
    info: &StockInfo,
    market: KnownMarket,
    policy: RowPolicy,
) -> Result<MasterCode, &'static str> {
    let reported = info.market.to_string();
    if reported != listed.exchange
        && (policy == RowPolicy::Strict || !exchanges(market).contains(&reported.as_str()))
    {
        return Err("detail market differs from the listing exchange");
    }
    MasterCode::try_new(&reported).map_err(|_error| "detail market is empty or invalid")
}

fn agrees_with_listing(listed: &Listed, info: &StockInfo) -> bool {
    info.name == listed.stock.name
        && info.security_type.to_string() == listed.stock.security_type.to_string()
        && info.is_common_share == listed.stock.is_common_share
}

const fn master_rule(error: MasterInstrumentError) -> &'static str {
    match error {
        MasterInstrumentError::SymbolMismatch => "symbol does not fit the market symbol rules",
        MasterInstrumentError::CurrencyMismatch => "currency does not fit the market",
        MasterInstrumentError::UnexpectedKoreanDetail => "korean market detail on a foreign row",
        MasterInstrumentError::InvalidCode
        | MasterInstrumentError::UnknownMarket
        | MasterInstrumentError::UnknownSecurityType
        | MasterInstrumentError::DuplicateIdentity
        | MasterInstrumentError::InvalidPriceObservation
        | MasterInstrumentError::InvalidNxtStatus
        | MasterInstrumentError::NotListed
        | MasterInstrumentError::TradingSuspended
        | MasterInstrumentError::LiquidationBuy
        | MasterInstrumentError::MissingTradingStatus => "row rejected by the master rules",
    }
}

fn prepare(
    listed: &Listed,
    info: StockInfo,
    context: (KnownMarket, domain::Timestamp, RowPolicy),
) -> Result<Prepared, &'static str> {
    let (market, observed, policy) = context;
    if info.symbol != listed.stock.symbol {
        return Err("detail symbol differs from the listing");
    }
    if info.isin_code != listed.stock.isin_code {
        return Err("detail ISIN differs from the listing");
    }
    let listing_market = listing_market(listed, &info, market, policy)?;
    if policy == RowPolicy::Strict && !agrees_with_listing(listed, &info) {
        return Err("detail name, security type or share class differs from the listing");
    }
    let status = match info.status.to_string().as_str() {
        "SCHEDULED" => ListingStatus::Scheduled,
        "ACTIVE" => ListingStatus::Active,
        "DELISTED" => ListingStatus::Delisted,
        _ => return Err("detail listing status is unknown"),
    };
    let currency = match info.currency.to_string().as_str() {
        "KRW" => Currency::Krw,
        "USD" => Currency::Usd,
        _ => return Err("detail currency is unknown"),
    };
    let row = MasterInstrumentRow {
        listing_market,
        symbol: listed.symbol.clone(),
        instrument_id: InstrumentId::try_new("unassigned-validation")
            .map_err(|_error| "row rejected by the master rules")?,
        name: DisplayInstrumentName::try_new(&info.name)
            .map_err(|_error| "detail name is empty or invalid")?,
        security_type: MasterSecurityType::from_code(
            MasterCode::try_new(&info.security_type.to_string())
                .map_err(|_error| "detail security type is empty or invalid")?,
        ),
        currency,
        listing_status: status,
        common_share: if info.is_common_share {
            CommonShare::Common
        } else {
            CommonShare::Other
        },
        korean_detail: korean_detail(info.korean_market_detail, market, policy)?,
        observed_at: observed,
    };
    MasterInstrument::try_new(row.clone()).map_err(master_rule)?;
    Ok(Prepared {
        source: listed.source.clone(),
        row,
    })
}

async fn details<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &crate::TossAccountRead<S, T, R>,
    stocks: &[Listed],
    market: KnownMarket,
    rows: &mut Rows,
) -> Result<Vec<Prepared>, PortError> {
    let mut details = BTreeMap::new();
    let mut remaining = stocks;
    while !remaining.is_empty() {
        let (batch, rest) = remaining
            .split_at_checked(remaining.len().min(200))
            .ok_or_else(invalid)?;
        remaining = rest;
        let symbols = batch
            .iter()
            .map(|row| row.stock.symbol.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let response = reader
            .market_read(
                ReadRequest::new(ReadRoute::Stocks).with_query(QueryKey::Symbols, symbols),
                ReadEndpoint::Instruments,
            )
            .await?;
        let infos: Vec<StockInfo> =
            decoded_rows(response, rows, "detail row does not match the schema")?;
        if rows.policy == RowPolicy::Strict && infos.len() != batch.len() {
            return Err(failed("detail count differs from the requested symbols"));
        }
        for info in infos {
            if !batch
                .iter()
                .any(|listed| listed.stock.symbol == info.symbol)
            {
                rows.reject("detail row for a symbol that was not requested")?;
                continue;
            }
            if details.contains_key(&info.symbol) {
                rows.reject("duplicate detail row")?;
                continue;
            }
            let _previous = details.insert(info.symbol.clone(), info);
        }
    }
    let observed = reader.market_now();
    let mut prepared = Vec::new();
    for stock in stocks {
        let Some(info) = details.remove(&stock.stock.symbol) else {
            rows.reject("listed symbol has no detail row")?;
            continue;
        };
        match prepare(stock, info, (market, observed, rows.policy)) {
            Ok(item) => prepared.push(item),
            Err(stage) => rows.reject(stage)?,
        }
    }
    Ok(prepared)
}

async fn map_source<M: InstrumentMappingRepository, I: InstrumentIdIssuer>(
    repository: &M,
    issuer: &I,
    source: &InstrumentSource,
) -> Result<Result<InstrumentId, InstrumentMappingError>, PortError> {
    match repository.resolve(source).await {
        Ok(id) => Ok(Ok(id)),
        Err(MappingStoreError::Storage(error)) => {
            Err(not_ready_after_response(MasterNotReady::Storage(error)))
        }
        Err(MappingStoreError::Mapping(InstrumentMappingError::Missing)) => {
            let candidate = issuer
                .issue()
                .await
                .map_err(|error| not_ready_after_response(MasterNotReady::Issuer(error)))?;
            match repository.ensure(source, candidate).await {
                Ok(MappingAssignment::Issued(id) | MappingAssignment::Existing(id)) => Ok(Ok(id)),
                Err(MappingStoreError::Storage(error)) => {
                    Err(not_ready_after_response(MasterNotReady::Storage(error)))
                }
                Err(MappingStoreError::Mapping(InstrumentMappingError::Missing)) => {
                    Err(not_ready_after_response(MasterNotReady::Storage(
                        application::SystemPortError::new(
                            application::SystemPort::Repository,
                            application::SystemFailure::InvalidData,
                        ),
                    )))
                }
                Err(MappingStoreError::Mapping(reason)) => Ok(Err(reason)),
            }
        }
        Err(MappingStoreError::Mapping(reason)) => Ok(Err(reason)),
    }
}

async fn mapped_entry<M: InstrumentMappingRepository, I: InstrumentIdIssuer>(
    repository: &M,
    issuer: &I,
    mut item: Prepared,
    conflict: domain::InstrumentSourceConflict,
) -> Result<InstrumentMasterEntry, PortError> {
    match conflict {
        domain::InstrumentSourceConflict::IdentityUncertain => {
            return Ok(InstrumentMasterEntry::Unavailable {
                source: item.source,
                reason: InstrumentMappingError::IdentityUncertain,
            });
        }
        domain::InstrumentSourceConflict::Distinct => {}
    }
    match item.row.listing_status {
        ListingStatus::Delisted => {
            let reason = match repository.retire(&item.source).await {
                Ok(_) | Err(MappingStoreError::Mapping(InstrumentMappingError::Retired)) => {
                    InstrumentMappingError::Retired
                }
                Err(MappingStoreError::Mapping(InstrumentMappingError::Missing)) => {
                    InstrumentMappingError::IdentityUncertain
                }
                Err(MappingStoreError::Mapping(reason)) => reason,
                Err(MappingStoreError::Storage(error)) => {
                    return Err(not_ready_after_response(MasterNotReady::Storage(error)));
                }
            };
            return Ok(InstrumentMasterEntry::Unavailable {
                source: item.source,
                reason,
            });
        }
        ListingStatus::Scheduled | ListingStatus::Active => {}
    }
    match map_source(repository, issuer, &item.source).await? {
        Ok(id) => {
            item.row.instrument_id = id;
            let instrument = MasterInstrument::try_new(item.row).map_err(|_error| invalid())?;
            Ok(InstrumentMasterEntry::Available(instrument))
        }
        Err(reason) => Ok(InstrumentMasterEntry::Unavailable {
            source: item.source,
            reason,
        }),
    }
}

pub(crate) async fn fetch<
    S: SecretStore,
    T: Clock,
    R: TokenRenewal,
    A: InstrumentMasterAuthority,
    I: InstrumentIdIssuer,
>(
    reader: &TossInstrumentRead<S, T, R, A, I>,
    market: KnownMarket,
) -> Result<InstrumentMaster, PortError> {
    fetch_parts(
        &reader.reader,
        (&reader.authority, &reader.issuer),
        market,
        RowPolicy::Strict,
    )
    .await
}

pub(crate) async fn fetch_parts<
    S: SecretStore,
    T: Clock,
    R: TokenRenewal,
    A: InstrumentMasterAuthority,
    I: InstrumentIdIssuer,
>(
    reader: &crate::TossAccountRead<S, T, R>,
    (authority, issuer): (&A, &I),
    market: KnownMarket,
    policy: RowPolicy,
) -> Result<InstrumentMaster, PortError> {
    let mut rows = Rows::new(policy);
    let result = assemble(reader, (authority, issuer), market, &mut rows).await;
    rows.report();
    result
}

async fn assemble<
    S: SecretStore,
    T: Clock,
    R: TokenRenewal,
    A: InstrumentMasterAuthority,
    I: InstrumentIdIssuer,
>(
    reader: &crate::TossAccountRead<S, T, R>,
    (authority, issuer): (&A, &I),
    market: KnownMarket,
    rows: &mut Rows,
) -> Result<InstrumentMaster, PortError> {
    let session = authority.begin(market).await.map_err(not_ready)?;
    let stocks = listed(reader, market, rows).await?;
    let prepared = details(reader, &stocks, market, rows).await?;
    if prepared.is_empty() && !rows.skipped.is_empty() {
        return Err(failed("every instrument row was rejected"));
    }
    let observations = prepared
        .iter()
        .map(|item| InstrumentMasterObservation {
            source: item.source.clone(),
            listing_market: item.row.listing_market.clone(),
            listing_status: item.row.listing_status,
            observed_at: item.row.observed_at,
        })
        .collect::<Vec<_>>();
    session
        .verify_observation(&observations)
        .await
        .map_err(not_ready_after_response)?;
    let sources = prepared
        .iter()
        .map(|item| item.source.clone())
        .collect::<Vec<_>>();
    let conflicts = domain::instrument_source_conflicts(&sources);
    let mut entries = Vec::new();
    for (item, conflict) in prepared.into_iter().zip(conflicts) {
        entries.push(mapped_entry(&session, issuer, item, conflict).await?);
    }
    InstrumentMaster::try_new(entries)
        .map_err(|_error| failed("instrument master entries share an identity"))
}
