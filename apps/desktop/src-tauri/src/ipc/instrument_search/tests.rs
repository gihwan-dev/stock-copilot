use domain::{
    CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds, Symbol, Timestamp,
    TradingSuspension, UnixSeconds,
};

use super::{SearchInstrumentsRequest, UiFundKindFilter, search};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::simulation::UiFundKind;

fn master(symbol: &str, name: &str, kind: &str, status: ListingStatus) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new(symbol).expect("심볼"),
        instrument_id: InstrumentId::try_new(&format!("inst-{symbol}")).expect("ID"),
        name: DisplayInstrumentName::try_new(name).expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new(kind).expect("유형")),
        currency: Currency::Krw,
        listing_status: status,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: Timestamp::from_parts(
            UnixSeconds::new(1_000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각"),
    })
    .expect("마스터")
}

fn directory() -> InstrumentDirectory {
    let found = InstrumentDirectory::default();
    found.replace(&[
        master("005930", "삼성전자", "STOCK", ListingStatus::Active),
        master("360750", "TIGER 미국S&P500", "ETF", ListingStatus::Active),
        master("069500", "KODEX 200", "ETF", ListingStatus::Active),
        master("122630", "KODEX 레버리지", "ETF", ListingStatus::Active),
        master("091160", "KODEX 반도체", "ETF", ListingStatus::Active),
        master("999990", "옛 S&P500 ETF", "ETF", ListingStatus::Delisted),
    ]);
    found
}

fn symbols(query: Option<&str>, kind: Option<UiFundKindFilter>) -> Vec<String> {
    search(
        &directory(),
        &SearchInstrumentsRequest {
            query: query.map(String::from),
            fund_kind: kind,
        },
    )
    .matches
    .into_iter()
    .map(|found| found.symbol)
    .collect()
}

#[test]
fn broad_index_funds_can_be_listed_by_kind() {
    assert_eq!(
        symbols(None, Some(UiFundKindFilter::BroadIndexFund)),
        vec!["069500", "360750"]
    );
}

#[test]
fn queries_match_names_without_spaces_or_exact_symbols() {
    assert_eq!(symbols(Some("s&p 500"), None), vec!["360750"]);
    assert_eq!(symbols(Some("005930"), None), vec!["005930"]);
    assert_eq!(
        symbols(Some("KODEX"), Some(UiFundKindFilter::LeveragedOrInverse)),
        vec!["122630"]
    );
    let kinds: Vec<UiFundKind> = search(
        &directory(),
        &SearchInstrumentsRequest {
            query: Some(String::from("반도체")),
            fund_kind: None,
        },
    )
    .matches
    .into_iter()
    .map(|found| found.fund_kind)
    .collect();
    assert_eq!(kinds, vec![UiFundKind::SectorOrThemeFund]);
}

#[test]
fn delisted_instruments_are_never_suggested() {
    assert!(!symbols(Some("S&P500"), None).contains(&String::from("999990")));
}
