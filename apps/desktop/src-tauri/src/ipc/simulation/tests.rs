use domain::{
    AcquisitionPrice, Allocation, BuyingPower, CommonShare, Currency, DisplayInstrumentName,
    Holding, HoldingObservation, InstrumentId, InvestorProfile, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Portfolio,
    Price, Quantity, SectorAssignments, Symbol, Timestamp, TradingSuspension, UnixSeconds,
};

use super::{
    UiFundKind, UiSimulationOutcome, UiTradeInput, UiTradeMarket, UiTradeSide, parse_trade,
    simulated_view,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::portfolio::UiWeightLevel;

fn krw(text: &str) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
}

fn time() -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(1_000),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn master(symbol: &str, name: &str, kind: &str) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new(symbol).expect("심볼"),
        instrument_id: InstrumentId::try_new(&format!("inst-{symbol}")).expect("ID"),
        name: DisplayInstrumentName::try_new(name).expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new(kind).expect("유형")),
        currency: Currency::Krw,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: time(),
    })
    .expect("마스터")
}

fn masters() -> Vec<MasterInstrument> {
    vec![
        master("005930", "삼성전자", "STOCK"),
        master("035720", "카카오", "STOCK"),
        master("360750", "TIGER 미국S&P500", "ETF"),
    ]
}

fn directory() -> InstrumentDirectory {
    let found = InstrumentDirectory::default();
    found.replace(&masters());
    found
}

fn holding(symbol: &str, quantity: &str, price: &str) -> Holding {
    let found = masters()
        .into_iter()
        .find(|candidate| candidate.row().symbol.as_str() == symbol)
        .expect("마스터");
    Holding::new(
        found.order_instrument().expect("종목"),
        Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"),
        AcquisitionPrice::try_new(krw(price)).expect("원가"),
        HoldingObservation::new(
            Some(Price::try_new(krw(price)).expect("가격")),
            None,
            time(),
        ),
    )
    .expect("보유")
}

fn allocation() -> Allocation {
    let valuation = Portfolio::new(
        vec![
            holding("005930", "100", "72000"),
            holding("035720", "40", "42000"),
        ],
        BuyingPower::new(krw("1120000"), time()),
        BuyingPower::new(
            Money::parse(&NumericInput::try_new("0").expect("입력"), Currency::Usd).expect("금액"),
            time(),
        ),
    )
    .expect("포트폴리오")
    .evaluate(None);
    Allocation::new(&valuation, &SectorAssignments::default())
}

fn input(symbol: &str, side: UiTradeSide, quantity: &str, price: Option<&str>) -> UiTradeInput {
    UiTradeInput {
        symbol: String::from(symbol),
        market: UiTradeMarket::Kr,
        side,
        quantity: String::from(quantity),
        price: price.map(String::from),
    }
}

#[test]
fn an_empty_plan_reports_current_weights_limits_and_room() {
    let view = simulated_view(
        &allocation(),
        &[],
        &directory(),
        &InvestorProfile::default(),
    );
    assert_eq!(view.outcome, UiSimulationOutcome::Simulated);
    let samsung = view
        .instruments
        .iter()
        .find(|row| row.symbol == "005930")
        .expect("삼성전자");
    assert_eq!(samsung.weight_before, "72.00");
    assert_eq!(samsung.weight_after, "72.00");
    assert_eq!(samsung.fund_kind, UiFundKind::SingleSecurity);
    assert_eq!(samsung.limit_percent, "20.00");
    assert_eq!(samsung.level_after, UiWeightLevel::OverCap);
    assert_eq!(
        samsung
            .room_to_limit
            .as_ref()
            .map(|money| money.amount.as_str()),
        Some("-5200000")
    );
}

#[test]
fn a_sell_and_buy_plan_returns_fills_cash_and_new_weights() {
    let before = allocation();
    let lookup = directory();
    let trades = [
        input("005930", UiTradeSide::Sell, "75", None),
        input("360750", UiTradeSide::Buy, "100", Some("23500")),
    ]
    .iter()
    .map(|trade| parse_trade(trade, before.valuation(), &lookup))
    .collect::<Option<Vec<_>>>()
    .expect("매매");
    let view = simulated_view(&before, &trades, &directory(), &InvestorProfile::default());
    assert_eq!(view.outcome, UiSimulationOutcome::Simulated);
    let amounts: Vec<&str> = view
        .fills
        .iter()
        .map(|fill| fill.amount.amount.as_str())
        .collect();
    assert_eq!(amounts, vec!["5400000", "2350000"]);
    let samsung = view
        .instruments
        .iter()
        .find(|row| row.symbol == "005930")
        .expect("삼성전자");
    assert_eq!(samsung.weight_after, "18.00");
    assert_eq!(samsung.level_after, UiWeightLevel::Warning);
    let cash: Vec<&str> = view
        .cash_after
        .iter()
        .map(|line| line.amount.amount.as_str())
        .collect();
    assert_eq!(cash, vec!["4170000"]);
    assert_eq!(
        view.total_after.map(|money| money.amount),
        Some(String::from("10000000"))
    );
    let fund = view
        .instruments
        .iter()
        .find(|row| row.symbol == "360750")
        .expect("지수 ETF");
    assert_eq!(fund.fund_kind, UiFundKind::BroadIndexFund);
    assert_eq!(
        (
            fund.weight_after.as_str(),
            fund.limit_percent.as_str(),
            fund.warning_percent.as_str()
        ),
        ("23.50", "40.00", "20.00")
    );
    assert_eq!(fund.level_after, UiWeightLevel::Warning);
}

#[test]
fn impossible_or_malformed_plans_say_why_without_numbers() {
    let before = allocation();
    let lookup = directory();
    let parse = |trade: &UiTradeInput| parse_trade(trade, before.valuation(), &lookup);
    let oversell = vec![parse(&input("035720", UiTradeSide::Sell, "41", None)).expect("매매")];
    let view = simulated_view(
        &before,
        &oversell,
        &directory(),
        &InvestorProfile::default(),
    );
    assert_eq!(view.outcome, UiSimulationOutcome::SellExceedsHolding);
    assert_eq!(view.problem_symbol.as_deref(), Some("035720"));
    assert_eq!(view.instruments, Vec::new());
    assert_eq!(parse(&input("005930", UiTradeSide::Buy, "1.5", None)), None);
    assert_eq!(
        parse(&input("005930", UiTradeSide::Buy, "1", Some("0"))),
        None
    );
    assert_eq!(
        parse(&input("999999", UiTradeSide::Buy, "1", Some("1000"))),
        None
    );
}
