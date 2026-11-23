use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossInstrumentRead};
use application::{
    Clock, InstrumentIdIssuer, InstrumentMappingRepository, InstrumentMasterAuthority,
    InstrumentMasterObservation, InstrumentMasterSession, MappingStoreError, MasterNotReady,
    PortError, PortErrorKind, ReadEndpoint, SecretStore,
};
use broker_toss_schema::types::{ListedStock, StockInfo};
use domain::{
    CommonShare, DisplayInstrumentName, InstrumentId, InstrumentMappingError, InstrumentMaster,
    InstrumentMasterEntry, InstrumentSource, IsinCode, KnownMarket, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MappingAssignment, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, NxtSupport, NxtTrading, Symbol, TradingSuspension,
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

fn normalized_symbol(raw: &str) -> Result<Symbol, PortError> {
    Symbol::try_new(&raw.to_ascii_uppercase()).map_err(|_error| invalid())
}

async fn listed<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &crate::TossAccountRead<S, T, R>,
    market: KnownMarket,
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
        let rows: Vec<ListedStock> =
            serde_json::from_value(response).map_err(|_error| invalid())?;
        if rows.len() > 8192 {
            return Err(invalid());
        }
        for stock in rows {
            let symbol = normalized_symbol(&stock.symbol)?;
            if !symbols.insert(symbol.clone()) {
                return Err(invalid());
            }
            let source = InstrumentSource::new(
                market,
                Symbol::try_new(&stock.symbol).map_err(|_error| invalid())?,
                IsinCode::try_new(&stock.isin_code).map_err(|_error| invalid())?,
            );
            DisplayInstrumentName::try_new(&stock.name).map_err(|_error| invalid())?;
            MasterCode::try_new(&stock.security_type.to_string()).map_err(|_error| invalid())?;
            result.push(Listed {
                exchange,
                stock,
                source,
                symbol,
            });
        }
    }
    Ok(result)
}

fn korean_detail(
    value: Option<broker_toss_schema::types::KrMarketDetail>,
) -> Result<Option<KoreanMasterDetail>, PortError> {
    value
        .map(|detail| {
            let suspension = |state| {
                if state {
                    TradingSuspension::Suspended
                } else {
                    TradingSuspension::Clear
                }
            };
            Ok(KoreanMasterDetail {
                liquidation: if detail.liquidation_trading {
                    LiquidationTrading::Liquidation
                } else {
                    LiquidationTrading::Regular
                },
                krx: suspension(detail.krx_trading_suspended),
                nxt: NxtTrading::try_from_parts(
                    if detail.nxt_supported {
                        NxtSupport::Supported
                    } else {
                        NxtSupport::Unsupported
                    },
                    detail.nxt_trading_suspended.map(suspension),
                )
                .map_err(|_error| invalid())?,
            })
        })
        .transpose()
}

fn prepare(
    listed: &Listed,
    info: StockInfo,
    observed: domain::Timestamp,
) -> Result<Prepared, PortError> {
    if info.symbol != listed.stock.symbol
        || info.isin_code != listed.stock.isin_code
        || info.market.to_string() != listed.exchange
        || info.name != listed.stock.name
        || info.security_type.to_string() != listed.stock.security_type.to_string()
        || info.is_common_share != listed.stock.is_common_share
    {
        return Err(invalid());
    }
    let status = match info.status.to_string().as_str() {
        "SCHEDULED" => ListingStatus::Scheduled,
        "ACTIVE" => ListingStatus::Active,
        "DELISTED" => ListingStatus::Delisted,
        _ => return Err(invalid()),
    };
    let row = MasterInstrumentRow {
        listing_market: MasterCode::try_new(&info.market.to_string())
            .map_err(|_error| invalid())?,
        symbol: listed.symbol.clone(),
        instrument_id: InstrumentId::try_new("unassigned-validation")
            .map_err(|_error| invalid())?,
        name: DisplayInstrumentName::try_new(&info.name).map_err(|_error| invalid())?,
        security_type: MasterSecurityType::from_code(
            MasterCode::try_new(&info.security_type.to_string()).map_err(|_error| invalid())?,
        ),
        currency: crate::market_read::currency(
            &info.currency.to_string(),
            ReadEndpoint::Instruments,
        )?,
        listing_status: status,
        common_share: if info.is_common_share {
            CommonShare::Common
        } else {
            CommonShare::Other
        },
        korean_detail: korean_detail(info.korean_market_detail)?,
        observed_at: observed,
    };
    MasterInstrument::try_new(row.clone()).map_err(|_error| invalid())?;
    Ok(Prepared {
        source: listed.source.clone(),
        row,
    })
}

async fn details<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &crate::TossAccountRead<S, T, R>,
    stocks: &[Listed],
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
        let rows: Vec<StockInfo> = serde_json::from_value(response).map_err(|_error| invalid())?;
        if rows.len() != batch.len() {
            return Err(invalid());
        }
        for row in rows {
            if !batch.iter().any(|listed| listed.stock.symbol == row.symbol) {
                return Err(invalid());
            }
            if details.insert(row.symbol.clone(), row).is_some() {
                return Err(invalid());
            }
        }
    }
    let observed = reader.market_now();
    stocks
        .iter()
        .map(|stock| {
            let info = details.remove(&stock.stock.symbol).ok_or_else(invalid)?;
            prepare(stock, info, observed)
        })
        .collect()
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
            match repository.ensure(source, &candidate).await {
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
    if conflict == domain::InstrumentSourceConflict::IdentityUncertain {
        return Ok(InstrumentMasterEntry::Unavailable {
            source: item.source,
            reason: InstrumentMappingError::IdentityUncertain,
        });
    }
    if item.row.listing_status == ListingStatus::Delisted {
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
    let session = reader.authority.begin(market).await.map_err(not_ready)?;
    let stocks = listed(&reader.reader, market).await?;
    let prepared = details(&reader.reader, &stocks).await?;
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
        entries.push(mapped_entry(&session, &reader.issuer, item, conflict).await?);
    }
    let master = InstrumentMaster::try_new(entries).map_err(|_error| invalid())?;
    Ok(master)
}
