use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{AccessDenial, Clock, HttpStatus, PortError, PortErrorKind, ReadEndpoint};
use domain::{
    AllocationInstrument, CommonShare, Currency, DisplayInstrumentName, InstrumentClassification,
    InstrumentId, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode,
    MasterInstrument, MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput,
    NxtTrading, OffsetSeconds, SectorCatalog, SectorComposition, SectorDepth, SectorId,
    SectorMetadataRow, SectorStockRow, Symbol, Timestamp, TradingSuspension, UnixSeconds,
};

use super::{SectorClasses, SectorFeed};
use crate::ipc::state::BoxFuture;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        at(1_790_000_000)
    }
}

fn master(symbol: &str, kind: &str) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new(symbol).expect("심볼"),
        instrument_id: InstrumentId::try_new(&format!("inst-{symbol}")).expect("ID"),
        name: DisplayInstrumentName::try_new("종목").expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new(kind).expect("유형")),
        currency: Currency::Krw,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: at(1_790_000_000),
    })
    .expect("마스터")
}

fn sector() -> SectorId {
    SectorId::try_new("semiconductor").expect("섹터")
}

enum Catalog {
    Answers,
    Fails,
}

struct Feed {
    catalog: Catalog,
    calls: AtomicUsize,
}

impl Feed {
    fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            calls: AtomicUsize::new(0),
        }
    }
}

fn denied(endpoint: ReadEndpoint) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::NotReceived,
        PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
    )
}

impl SectorFeed for Feed {
    fn catalog(&self) -> BoxFuture<'_, Result<SectorCatalog, PortError>> {
        let _seen = self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = match self.catalog {
            Catalog::Answers => Ok(SectorCatalog::from_response(
                vec![SectorMetadataRow {
                    id: sector(),
                    name: DisplayInstrumentName::try_new("반도체").expect("이름"),
                    depth: SectorDepth::Major,
                    parent: None,
                }],
                at(1_790_000_000),
            )
            .expect("목록")),
            Catalog::Fails => Err(denied(ReadEndpoint::Sectors)),
        };
        Box::pin(std::future::ready(reply))
    }

    fn composition<'a>(
        &'a self,
        sector: &'a SectorId,
        market: KnownMarket,
    ) -> BoxFuture<'a, Result<SectorComposition, PortError>> {
        let reply = SectorComposition::from_stocks_response(
            sector.clone(),
            market,
            vec![SectorStockRow {
                symbol: Symbol::try_new("005930").expect("심볼"),
                currency: Currency::Krw,
                market_capitalization: Some(NumericInput::try_new("1000").expect("시총")),
                trading_amount: None,
            }],
            at(1_790_000_000),
        )
        .map_err(|_error| denied(ReadEndpoint::SectorComposition));
        Box::pin(std::future::ready(reply))
    }
}

fn shared(feed: &Arc<Feed>) -> Arc<dyn SectorFeed> {
    Arc::<Feed>::clone(feed)
}

fn classification(
    classes: &SectorClasses,
    masters: &[MasterInstrument],
) -> Vec<(Symbol, InstrumentClassification)> {
    let assignments = tauri::async_runtime::block_on(classes.assignments(masters)).expect("분류");
    assignments
        .entries()
        .map(|(instrument, class): (&AllocationInstrument, _)| {
            (instrument.symbol().clone(), class.clone())
        })
        .collect()
}

#[test]
fn held_stocks_get_their_major_sector_and_etfs_are_funds() {
    let feed = Arc::new(Feed::new(Catalog::Answers));
    let classes = SectorClasses::new(shared(&feed), Arc::new(Fixed));
    let masters = [master("005930", "STOCK"), master("069500", "ETF")];
    assert_eq!(
        classification(&classes, &masters),
        vec![
            (
                Symbol::try_new("005930").expect("심볼"),
                InstrumentClassification::Sector(sector())
            ),
            (
                Symbol::try_new("069500").expect("심볼"),
                InstrumentClassification::EtfFund
            ),
        ]
    );
    let _again = classification(&classes, &masters);
    assert_eq!(
        feed.calls.load(Ordering::SeqCst),
        1,
        "하루 안에는 다시 받지 않는다"
    );
}

#[test]
fn a_failed_sector_list_still_marks_etfs_and_retries_later() {
    let feed = Arc::new(Feed::new(Catalog::Fails));
    let classes = SectorClasses::new(shared(&feed), Arc::new(Fixed));
    let masters = [master("005930", "STOCK"), master("069500", "ETF")];
    assert_eq!(
        classification(&classes, &masters),
        vec![
            (
                Symbol::try_new("005930").expect("심볼"),
                InstrumentClassification::Unclassified
            ),
            (
                Symbol::try_new("069500").expect("심볼"),
                InstrumentClassification::EtfFund
            ),
        ]
    );
    let _again = classification(&classes, &masters);
    assert_eq!(
        feed.calls.load(Ordering::SeqCst),
        4,
        "실패는 저장하지 않고 다시 시도한다"
    );
}
