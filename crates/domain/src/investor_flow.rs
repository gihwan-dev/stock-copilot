use crate::{
    Amount, CalendarDate, Currency, IndicatorCode, KnownMarket, MarketDataError, MarketObservation,
    MasterCode, Money, NumericInput, Price, Symbol, Timestamp,
};
use rust_decimal::Decimal;
use std::{collections::BTreeSet, fmt};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvestorClass {
    Individual,
    Foreigner,
    Institution,
    OtherCorporation,
    FinancialInvestment,
    Insurance,
    Trust,
    PrivateEquityFund,
    Bank,
    OtherFinancialInstitution,
    PensionFund,
    Unknown,
}
impl InvestorClass {
    #[must_use]
    pub fn from_code(code: &MasterCode) -> Self {
        match code.as_str() {
            "individual" => Self::Individual,
            "foreigner" => Self::Foreigner,
            "institution" => Self::Institution,
            "otherCorporation" => Self::OtherCorporation,
            "financialInvestment" => Self::FinancialInvestment,
            "insurance" => Self::Insurance,
            "trust" => Self::Trust,
            "privateEquityFund" => Self::PrivateEquityFund,
            "bank" => Self::Bank,
            "otherFinancialInstitution" => Self::OtherFinancialInstitution,
            "pensionFund" => Self::PensionFund,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub const fn level(self) -> FlowLevel {
        match self {
            Self::Individual | Self::Foreigner | Self::Institution | Self::OtherCorporation => {
                FlowLevel::TopLevel
            }
            Self::FinancialInvestment
            | Self::Insurance
            | Self::Trust
            | Self::PrivateEquityFund
            | Self::Bank
            | Self::OtherFinancialInstitution
            | Self::PensionFund => FlowLevel::InstitutionDetail,
            Self::Unknown => FlowLevel::Unknown,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowInterval {
    Day,
    Week,
    Month,
    Year,
    Unknown,
}
impl FlowInterval {
    #[must_use]
    pub fn from_code(code: &MasterCode) -> Self {
        match code.as_str() {
            "1d" => Self::Day,
            "1w" => Self::Week,
            "1mo" => Self::Month,
            "1y" => Self::Year,
            _ => Self::Unknown,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowFinality {
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowUnit {
    Shares,
    Krw,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignInvestorScope {
    RegisteredOnly,
    RegisteredAndUnregistered,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowSubject {
    Stock(Symbol),
    Indicator(MasterCode),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedFlowShares(Amount);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedFlowKrw(Amount);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowNet {
    Shares(SignedFlowShares),
    Krw(SignedFlowKrw),
}
impl FlowNet {
    fn amount(self) -> Amount {
        match self {
            Self::Shares(value) => value.0,
            Self::Krw(value) => value.0,
        }
    }
    fn from_amount(amount: Amount, unit: FlowUnit) -> Self {
        match unit {
            FlowUnit::Shares => Self::Shares(SignedFlowShares(amount)),
            FlowUnit::Krw => Self::Krw(SignedFlowKrw(amount)),
        }
    }
}
impl fmt::Display for FlowNet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shares(value) => fmt::Display::fmt(&value.0.as_decimal(), f),
            Self::Krw(value) => fmt::Display::fmt(&value.0.as_decimal(), f),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowItem {
    investor: InvestorClass,
    raw_investor: MasterCode,
    buy: FlowNet,
    sell: FlowNet,
    net: FlowNet,
    unit: FlowUnit,
}
impl FlowItem {
    #[must_use]
    pub const fn level(&self) -> FlowLevel {
        self.investor.level()
    }
    #[must_use]
    pub const fn investor(&self) -> InvestorClass {
        self.investor
    }
    #[must_use]
    pub const fn raw_investor(&self) -> &MasterCode {
        &self.raw_investor
    }
    #[must_use]
    pub const fn buy(&self) -> FlowNet {
        self.buy
    }
    #[must_use]
    pub const fn sell(&self) -> FlowNet {
        self.sell
    }
    #[must_use]
    pub const fn net(&self) -> FlowNet {
        self.net
    }
    #[must_use]
    pub const fn unit(&self) -> FlowUnit {
        self.unit
    }
    #[must_use]
    pub fn net_krw_at(&self, close: Option<Price>) -> Option<Money> {
        match self.net {
            FlowNet::Krw(value) => Some(Money::new(value.0, Currency::Krw)),
            FlowNet::Shares(value) => {
                let price = close.filter(|price| price.as_money().currency() == Currency::Krw)?;
                crate::exact_arithmetic::multiply(value.0, price.as_money().amount())
                    .ok()
                    .map(|amount| Money::new(amount, Currency::Krw))
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowLevel {
    TopLevel,
    InstitutionDetail,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowBalance {
    Matched,
    Mismatched,
    NotApplicable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockFlowRow {
    pub investor: MasterCode,
    pub buy: NumericInput,
    pub sell: NumericInput,
    pub reported_net: NumericInput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFlowRow {
    pub investor: MasterCode,
    pub buy: NumericInput,
    pub sell: NumericInput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockFlowRecordInput {
    pub date: CalendarDate,
    pub updated: Timestamp,
    pub rows: Vec<StockFlowRow>,
    pub collected: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketFlowRecordInput {
    pub date: CalendarDate,
    pub updated: Timestamp,
    pub rows: Vec<MarketFlowRow>,
    pub collected: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowTradingRow {
    investor: MasterCode,
    buy: NumericInput,
    sell: NumericInput,
    reported_net: Option<NumericInput>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowGroups {
    top: Vec<FlowItem>,
    details: Vec<FlowItem>,
    unknown: Vec<FlowItem>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorFlow {
    market: KnownMarket,
    subject: FlowSubject,
    date: CalendarDate,
    interval: FlowInterval,
    groups: FlowGroups,
    observation: MarketObservation,
    foreign_scope: ForeignInvestorScope,
    finality: FlowFinality,
    balance: FlowBalance,
}
impl InvestorFlow {
    pub fn from_stock_response(
        symbol: Symbol,
        input: StockFlowRecordInput,
    ) -> Result<Self, MarketDataError> {
        crate::market_ranking::validate_market_symbol(&symbol, KnownMarket::Kr)?;
        validate_flow_date(input.date, input.updated)?;
        let rows = input
            .rows
            .into_iter()
            .map(|row| FlowTradingRow {
                investor: row.investor,
                buy: row.buy,
                sell: row.sell,
                reported_net: Some(row.reported_net),
            })
            .collect();
        Ok(Self {
            market: KnownMarket::Kr,
            subject: FlowSubject::Stock(symbol),
            date: input.date,
            interval: FlowInterval::Day,
            groups: flow_groups(rows, FlowUnit::Shares)?,
            observation: MarketObservation::from_response(Some(input.updated), input.collected)?,
            foreign_scope: ForeignInvestorScope::RegisteredOnly,
            finality: FlowFinality::Unknown,
            balance: FlowBalance::NotApplicable,
        })
    }
    pub fn from_indicator_response(
        code: MasterCode,
        raw_interval: &MasterCode,
        input: MarketFlowRecordInput,
    ) -> Result<Self, MarketDataError> {
        match IndicatorCode::from_code(&code) {
            IndicatorCode::Kospi | IndicatorCode::Kosdaq => (),
            IndicatorCode::KrBond2Y
            | IndicatorCode::KrBond3Y
            | IndicatorCode::KrBond5Y
            | IndicatorCode::KrBond10Y
            | IndicatorCode::KrBond20Y
            | IndicatorCode::KrBond30Y
            | IndicatorCode::Unknown => return Err(MarketDataError::UnsupportedIndicator),
        }
        let interval = FlowInterval::from_code(raw_interval);
        if interval == FlowInterval::Unknown {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        validate_flow_date(input.date, input.updated)?;
        let rows = input
            .rows
            .into_iter()
            .map(|row| FlowTradingRow {
                investor: row.investor,
                buy: row.buy,
                sell: row.sell,
                reported_net: None,
            })
            .collect();
        let groups = flow_groups(rows, FlowUnit::Krw)?;
        if groups.top.len() != 4 {
            return Err(MarketDataError::InvalidFlow);
        }
        let totals = flow_totals(&groups.top)?;
        let balance = if totals.buy == totals.sell {
            FlowBalance::Matched
        } else {
            FlowBalance::Mismatched
        };
        Ok(Self {
            market: KnownMarket::Kr,
            subject: FlowSubject::Indicator(code),
            date: input.date,
            interval,
            groups,
            observation: MarketObservation::from_response(Some(input.updated), input.collected)?,
            foreign_scope: ForeignInvestorScope::RegisteredAndUnregistered,
            finality: FlowFinality::Unknown,
            balance,
        })
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn subject(&self) -> &FlowSubject {
        &self.subject
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub const fn interval(&self) -> FlowInterval {
        self.interval
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
    #[must_use]
    pub const fn finality(&self) -> FlowFinality {
        self.finality
    }
    #[must_use]
    pub const fn foreign_scope(&self) -> ForeignInvestorScope {
        self.foreign_scope
    }
    #[must_use]
    pub const fn balance(&self) -> FlowBalance {
        self.balance
    }
    #[must_use]
    pub fn top_level(&self) -> &[FlowItem] {
        &self.groups.top
    }
    #[must_use]
    pub fn institution_details(&self) -> &[FlowItem] {
        &self.groups.details
    }
    #[must_use]
    pub fn unknown_items(&self) -> &[FlowItem] {
        &self.groups.unknown
    }
}
fn validate_flow_date(date: CalendarDate, updated: Timestamp) -> Result<(), MarketDataError> {
    let market_date = updated
        .date_at_offset(crate::OffsetSeconds::new(32_400))
        .or(Err(MarketDataError::InvalidFlow))?;
    if date > market_date {
        return Err(MarketDataError::InvalidFlow);
    }
    Ok(())
}
fn flow_groups(rows: Vec<FlowTradingRow>, unit: FlowUnit) -> Result<FlowGroups, MarketDataError> {
    if rows.is_empty() {
        return Err(MarketDataError::InvalidFlow);
    }
    let mut seen = BTreeSet::new();
    let mut groups = FlowGroups {
        top: Vec::new(),
        details: Vec::new(),
        unknown: Vec::new(),
    };
    for row in rows {
        if !seen.insert(row.investor.as_str().to_owned()) {
            return Err(MarketDataError::InvalidFlow);
        }
        let item = flow_item(row, unit)?;
        match item.level() {
            FlowLevel::TopLevel => groups.top.push(item),
            FlowLevel::InstitutionDetail => groups.details.push(item),
            FlowLevel::Unknown => groups.unknown.push(item),
        }
    }
    for group in [&mut groups.top, &mut groups.details, &mut groups.unknown] {
        group.sort_by(|left, right| left.raw_investor.as_str().cmp(right.raw_investor.as_str()));
    }
    validate_institution(&groups)?;
    Ok(groups)
}
fn flow_item(row: FlowTradingRow, unit: FlowUnit) -> Result<FlowItem, MarketDataError> {
    let buy = row
        .buy
        .parse_amount()
        .or(Err(MarketDataError::InvalidNumber))?;
    let sell = row
        .sell
        .parse_amount()
        .or(Err(MarketDataError::InvalidNumber))?;
    let net = buy
        .checked_sub(&sell)
        .or(Err(MarketDataError::InvalidNumber))?;
    if [buy, sell]
        .iter()
        .any(|value| value.as_decimal() < Decimal::ZERO || !value.as_decimal().fract().is_zero())
    {
        return Err(MarketDataError::InvalidFlow);
    }
    if let Some(input) = row.reported_net
        && input
            .parse_amount()
            .or(Err(MarketDataError::InvalidNumber))?
            != net
    {
        return Err(MarketDataError::InvalidFlow);
    }
    Ok(FlowItem {
        investor: InvestorClass::from_code(&row.investor),
        raw_investor: row.investor,
        buy: FlowNet::from_amount(buy, unit),
        sell: FlowNet::from_amount(sell, unit),
        net: FlowNet::from_amount(net, unit),
        unit,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FlowTotals {
    buy: Amount,
    sell: Amount,
    net: Amount,
}
fn flow_totals(items: &[FlowItem]) -> Result<FlowTotals, MarketDataError> {
    let zero = Amount::new(Decimal::ZERO);
    let mut total = FlowTotals {
        buy: zero,
        sell: zero,
        net: zero,
    };
    for item in items {
        total.net = total
            .net
            .checked_add(&item.net.amount())
            .or(Err(MarketDataError::InvalidNumber))?;
        total.buy = total
            .buy
            .checked_add(&item.buy.amount())
            .or(Err(MarketDataError::InvalidNumber))?;
        total.sell = total
            .sell
            .checked_add(&item.sell.amount())
            .or(Err(MarketDataError::InvalidNumber))?;
    }
    Ok(total)
}
fn validate_institution(groups: &FlowGroups) -> Result<(), MarketDataError> {
    if groups.details.is_empty() {
        return Ok(());
    }
    if groups.details.len() != 7 {
        return Err(MarketDataError::InvalidFlow);
    }
    let institution = groups
        .top
        .iter()
        .find(|item| item.investor == InvestorClass::Institution)
        .ok_or(MarketDataError::InvalidFlow)?;
    let total = flow_totals(&groups.details)?;
    if total.buy != institution.buy.amount()
        || total.sell != institution.sell.amount()
        || total.net != institution.net.amount()
    {
        return Err(MarketDataError::InvalidFlow);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{
        FlowBalance, FlowLevel, InvestorClass, InvestorFlow, MarketFlowRecordInput, MarketFlowRow,
        StockFlowRecordInput, StockFlowRow,
    };
    use crate::{
        MasterCode, Nanosecond, NumericInput, OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };
    fn code(text: &str) -> MasterCode {
        MasterCode::try_new(text).expect("코드")
    }
    fn num(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("수치")
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn stock_row(investor: &str, buy: &str, sell: &str, net: &str) -> StockFlowRow {
        StockFlowRow {
            investor: code(investor),
            buy: num(buy),
            sell: num(sell),
            reported_net: num(net),
        }
    }
    fn stock_symbol() -> Symbol {
        Symbol::try_new("005930").expect("종목")
    }
    fn stock(rows: Vec<StockFlowRow>) -> Result<InvestorFlow, crate::MarketDataError> {
        InvestorFlow::from_stock_response(
            stock_symbol(),
            StockFlowRecordInput {
                date: at(0).offset_calendar_date(),
                updated: at(0),
                rows,
                collected: at(0),
            },
        )
    }
    fn market_rows() -> Vec<MarketFlowRow> {
        ["individual", "foreigner", "institution", "otherCorporation"]
            .into_iter()
            .map(|investor| MarketFlowRow {
                investor: code(investor),
                buy: num("7"),
                sell: num("7"),
            })
            .collect()
    }
    fn market(rows: Vec<MarketFlowRow>) -> Result<InvestorFlow, crate::MarketDataError> {
        InvestorFlow::from_indicator_response(
            code("KOSPI"),
            &code("1d"),
            MarketFlowRecordInput {
                date: at(0).offset_calendar_date(),
                updated: at(0),
                rows,
                collected: at(0),
            },
        )
    }
    fn details() -> Vec<StockFlowRow> {
        [
            "financialInvestment",
            "insurance",
            "trust",
            "privateEquityFund",
            "bank",
            "otherFinancialInstitution",
            "pensionFund",
        ]
        .into_iter()
        .map(|investor| stock_row(investor, "1", "0", "1"))
        .collect()
    }
    #[test]
    fn levels_sorting_accessors_and_units_are_independent() {
        let fact = stock(vec![
            stock_row("individual", "10", "12", "-2"),
            stock_row("foreigner", "1", "0", "1"),
            stock_row("zz", "0", "0", "0"),
            stock_row("aa", "0", "0", "0"),
        ])
        .expect("종목");
        assert_eq!(
            fact.top_level()
                .iter()
                .map(|item| item.raw_investor().as_str())
                .collect::<Vec<_>>(),
            vec!["foreigner", "individual"]
        );
        assert_eq!(
            fact.unknown_items()
                .iter()
                .map(|item| item.raw_investor().as_str())
                .collect::<Vec<_>>(),
            vec!["aa", "zz"]
        );
        assert_eq!(
            fact.top_level().first().expect("행").level(),
            FlowLevel::TopLevel
        );
        assert_eq!(
            fact.unknown_items().first().expect("행").level(),
            FlowLevel::Unknown
        );
        assert_eq!(fact.top_level().get(1).expect("행").net().to_string(), "-2");
        assert_eq!(fact.top_level().first().expect("행").buy().to_string(), "1");
        assert_eq!(
            fact.top_level().first().expect("행").sell().to_string(),
            "0"
        );
        assert_eq!(
            fact.top_level().first().expect("행").unit(),
            crate::FlowUnit::Shares
        );
        assert_eq!(fact.institution_details(), []);
        assert_eq!(fact.balance(), FlowBalance::NotApplicable);
        assert_eq!(fact.finality(), crate::FlowFinality::Unknown);
        assert_eq!(
            fact.foreign_scope(),
            crate::ForeignInvestorScope::RegisteredOnly
        );
        assert_eq!(fact.interval(), crate::FlowInterval::Day);
        assert_eq!(fact.date(), at(0).offset_calendar_date());
        assert_eq!(fact.observation().server_at(), Some(at(0)));
        assert_eq!(
            fact.subject(),
            &crate::FlowSubject::Stock(Symbol::try_new("005930").expect("종목"))
        );
    }
    #[test]
    fn market_requires_four_top_categories_and_preserves_balance_difference() {
        market(vec![]).expect_err("빈 응답");
        stock(vec![]).expect_err("빈 응답");
        let mut incomplete = market_rows();
        incomplete.pop().expect("행");
        market(incomplete).expect_err("상위 누락");
        let matched = market(market_rows()).expect("일치");
        assert_eq!(matched.balance(), FlowBalance::Matched);
        assert_eq!(matched.top_level().len(), 4);
        assert_eq!(
            matched.foreign_scope(),
            crate::ForeignInvestorScope::RegisteredAndUnregistered
        );
        assert_eq!(
            matched.top_level().first().expect("행").unit(),
            crate::FlowUnit::Krw
        );
        assert_eq!(
            matched.top_level().first().expect("행").net().to_string(),
            "0"
        );
        let mut mismatch = market_rows();
        mismatch.first_mut().expect("행").buy = num("8");
        assert_eq!(
            market(mismatch).expect("잠정 차이").balance(),
            FlowBalance::Mismatched
        );
        let mut duplicate = market_rows();
        duplicate.push(duplicate.first().expect("행").clone());
        market(duplicate).expect_err("중복");
    }
    #[test]
    fn institution_details_are_separate_and_must_match_all_seven_components() {
        let mut rows = details();
        rows.push(stock_row("institution", "7", "0", "7"));
        let fact = stock(rows.clone()).expect("기관 합계");
        assert_eq!(fact.top_level().len(), 1);
        assert_eq!(fact.institution_details().len(), 7);
        assert_eq!(
            fact.institution_details().first().expect("행").level(),
            FlowLevel::InstitutionDetail
        );
        rows.last_mut().expect("기관").buy = num("8");
        rows.last_mut().expect("기관").reported_net = num("8");
        stock(rows).expect_err("매수 합계 차이");
        let mut wrong_sell = details();
        wrong_sell.push(stock_row("institution", "7", "1", "6"));
        stock(wrong_sell).expect_err("매도 합계 차이");
        stock(details()).expect_err("기관 합계 없음");
        let mut incomplete = details();
        incomplete.pop().expect("세부");
        incomplete.push(stock_row("institution", "6", "0", "6"));
        stock(incomplete).expect_err("세부 여섯 개");
        stock(vec![stock_row("bank", "0", "0", "0")]).expect_err("기관 세부 누락");
        let mut money = market_rows();
        for item in &mut money {
            if item.investor.as_str() == "institution" {
                item.sell = num("0");
            }
        }
        money.extend(details().into_iter().map(|row| MarketFlowRow {
            investor: row.investor,
            buy: row.buy,
            sell: row.sell,
        }));
        assert_eq!(
            market(money)
                .expect("시장 기관 세부")
                .institution_details()
                .len(),
            7
        );
    }
    #[test]
    fn numeric_errors_are_closed_and_reported_stock_net_is_verified() {
        for (buy, sell, net) in [
            ("bad", "0", "0"),
            ("1", "bad", "0"),
            ("1", "0", "bad"),
            ("1", "0", "0"),
            ("-1", "0", "-1"),
            ("1.5", "0", "1.5"),
            (
                "-79228162514264337593543950335",
                "79228162514264337593543950335",
                "0",
            ),
        ] {
            stock(vec![stock_row("individual", buy, sell, net)]).expect_err("수치 검증");
        }
    }
    #[test]
    fn dates_future_observation_and_supported_subjects_are_checked() {
        let make = || MarketFlowRecordInput {
            date: at(0).offset_calendar_date(),
            updated: at(0),
            rows: market_rows(),
            collected: at(0),
        };
        InvestorFlow::from_indicator_response(code("KR_BOND_2Y"), &code("1d"), make())
            .expect_err("국채");
        InvestorFlow::from_indicator_response(code("KOSDAQ"), &code("new"), make())
            .expect_err("기간");
        let mut date = make();
        date.date = at(86400).offset_calendar_date();
        InvestorFlow::from_indicator_response(code("KOSPI"), &code("1d"), date)
            .expect_err("미래 집계일");
        let mut late = make();
        late.updated = at(3);
        InvestorFlow::from_indicator_response(code("KOSPI"), &code("1d"), late)
            .expect_err("미래 서버 시각");
        let input = || StockFlowRecordInput {
            date: at(0).offset_calendar_date(),
            updated: at(0),
            rows: vec![stock_row("individual", "0", "0", "0")],
            collected: at(0),
        };
        InvestorFlow::from_stock_response(Symbol::try_new("AAPL").expect("종목"), input())
            .expect_err("국내 한정");
        let mut stock_date = input();
        stock_date.date = at(86400).offset_calendar_date();
        InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), stock_date)
            .expect_err("미래 집계일");
        let mut future_observation = input();
        future_observation.updated = at(3);
        InvestorFlow::from_stock_response(
            Symbol::try_new("005930").expect("종목"),
            future_observation,
        )
        .expect_err("미래 서버 시각");
    }
    #[test]
    fn all_schema_investors_and_intervals_are_preserved() {
        for (raw, expected) in [
            ("individual", InvestorClass::Individual),
            ("foreigner", InvestorClass::Foreigner),
            ("institution", InvestorClass::Institution),
            ("otherCorporation", InvestorClass::OtherCorporation),
            ("financialInvestment", InvestorClass::FinancialInvestment),
            ("insurance", InvestorClass::Insurance),
            ("trust", InvestorClass::Trust),
            ("privateEquityFund", InvestorClass::PrivateEquityFund),
            ("bank", InvestorClass::Bank),
            (
                "otherFinancialInstitution",
                InvestorClass::OtherFinancialInstitution,
            ),
            ("pensionFund", InvestorClass::PensionFund),
            ("new", InvestorClass::Unknown),
        ] {
            assert_eq!(InvestorClass::from_code(&code(raw)), expected);
        }
        for (raw, expected) in [
            ("1d", crate::FlowInterval::Day),
            ("1w", crate::FlowInterval::Week),
            ("1mo", crate::FlowInterval::Month),
            ("1y", crate::FlowInterval::Year),
            ("new", crate::FlowInterval::Unknown),
        ] {
            assert_eq!(crate::FlowInterval::from_code(&code(raw)), expected);
        }
    }

    #[test]
    fn overflowing_totals_never_publish_a_partial_institution_or_market() {
        let maximum = "79228162514264337593543950335";
        for (buy, sell, net, next_buy, next_sell, next_net) in [
            (maximum, "0", maximum, "1", "0", "1"),
            (maximum, maximum, "0", "1", "1", "0"),
            (
                "0",
                maximum,
                "-79228162514264337593543950335",
                "1",
                "1",
                "0",
            ),
        ] {
            let mut entries = details();
            for entry in &mut entries {
                entry.buy = num("0");
                entry.sell = num("0");
                entry.reported_net = num("0");
            }
            let first = entries.first_mut().expect("세부");
            first.buy = num(buy);
            first.sell = num(sell);
            first.reported_net = num(net);
            let second = entries.get_mut(1).expect("세부");
            second.buy = num(next_buy);
            second.sell = num(next_sell);
            second.reported_net = num(next_net);
            entries.push(stock_row("institution", "0", "0", "0"));
            stock(entries).expect_err("세부 합계 넘침");
        }
        let mut rows = market_rows();
        for row in &mut rows {
            row.buy = num("0");
            row.sell = num("0");
        }
        rows.first_mut().expect("개인").buy = num(maximum);
        rows.get_mut(1).expect("외국인").buy = num("1");
        market(rows).expect_err("상위 합계 넘침");
        let fact = stock(vec![stock_row("individual", "0", "0", "0")]).expect("행");
        assert_eq!(
            fact.top_level().first().expect("행").investor(),
            InvestorClass::Individual
        );
    }

    #[test]
    fn sorting_distinguishes_arbitrary_input_from_simple_reversal_at_every_level() {
        for names in [
            ["individual", "foreigner", "institution"],
            ["foreigner", "institution", "individual"],
            ["institution", "individual", "foreigner"],
            ["foreigner", "individual", "institution"],
        ] {
            let fact = stock(
                names
                    .into_iter()
                    .map(|name| stock_row(name, "0", "0", "0"))
                    .collect(),
            )
            .expect("상위 정렬");
            assert_eq!(
                fact.top_level()
                    .iter()
                    .map(|item| item.raw_investor().as_str())
                    .collect::<Vec<_>>(),
                vec!["foreigner", "individual", "institution"]
            );
        }
        let unknown = stock(
            ["bb", "aa", "cc"]
                .into_iter()
                .map(|name| stock_row(name, "0", "0", "0"))
                .collect(),
        )
        .expect("미지 분류 정렬");
        assert_eq!(
            unknown
                .unknown_items()
                .iter()
                .map(|item| item.raw_investor().as_str())
                .collect::<Vec<_>>(),
            vec!["aa", "bb", "cc"]
        );
        let mut entries = details();
        entries.push(stock_row("institution", "7", "0", "7"));
        let institution = stock(entries).expect("세부 정렬");
        assert_eq!(
            institution
                .institution_details()
                .iter()
                .map(|item| item.raw_investor().as_str())
                .collect::<Vec<_>>(),
            vec![
                "bank",
                "financialInvestment",
                "insurance",
                "otherFinancialInstitution",
                "pensionFund",
                "privateEquityFund",
                "trust"
            ]
        );
    }

    #[test]
    fn d42_equal_net_does_not_excuse_wrong_buy_and_sell_totals() {
        let mut rows = details();
        rows.push(stock_row("institution", "8", "1", "7"));
        assert_eq!(stock(rows), Err(crate::MarketDataError::InvalidFlow));
        let mut exact = details();
        exact.push(stock_row("institution", "7", "0", "7"));
        let fact = stock(exact).expect("정확한 기관 합계");
        let institution = fact.top_level().first().expect("기관");
        assert_eq!(institution.buy().to_string(), "7");
        assert_eq!(institution.sell().to_string(), "0");
        assert_eq!(institution.net().to_string(), "7");
    }
    fn assert_flow_observation(
        fact: &InvestorFlow,
        date: crate::CalendarDate,
        updated: Timestamp,
        collected: Timestamp,
    ) {
        assert_eq!(fact.date(), date);
        let observation = fact.observation();
        let server = observation.server_at().expect("서버");
        assert_eq!(server.utc_seconds(), updated.utc_seconds());
        assert_eq!(server.nanosecond(), updated.nanosecond());
        assert_eq!(server.offset_seconds(), updated.offset_seconds());
        assert_eq!(
            observation.collected_at().utc_seconds(),
            collected.utc_seconds()
        );
        assert_eq!(
            observation.collected_at().nanosecond(),
            collected.nanosecond()
        );
        assert_eq!(
            observation.collected_at().offset_seconds(),
            collected.offset_seconds()
        );
    }
    fn assert_flow_result(
        result: Result<InvestorFlow, crate::MarketDataError>,
        expected: Result<(), crate::MarketDataError>,
        date: crate::CalendarDate,
        updated: Timestamp,
        collected: Timestamp,
    ) {
        assert_eq!(
            result.as_ref().map(|_fact| ()).map_err(|error| *error),
            expected
        );
        if let Ok(fact) = result {
            assert_flow_observation(&fact, date, updated, collected);
        }
    }
    fn assert_flow_date_case(
        seconds: i64,
        nanos: u32,
        expected: Result<(), crate::MarketDataError>,
    ) {
        let date = at(86_400).offset_calendar_date();
        for offset in [-18_000_i32, 0_i32, 32_400_i32, 36_000_i32] {
            let updated = Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(nanos),
                OffsetSeconds::new(offset),
            )
            .expect("표기");
            let collected = Timestamp::from_parts(
                UnixSeconds::new(86_401),
                Nanosecond::new(987_654_321),
                OffsetSeconds::new(-3_600),
            )
            .expect("수집");
            let stock = InvestorFlow::from_stock_response(
                stock_symbol(),
                StockFlowRecordInput {
                    date,
                    updated,
                    rows: vec![stock_row("individual", "17", "5", "12")],
                    collected,
                },
            );
            let market = InvestorFlow::from_indicator_response(
                code("KOSPI"),
                &code("1d"),
                MarketFlowRecordInput {
                    date,
                    updated,
                    rows: market_rows(),
                    collected,
                },
            );
            assert_flow_result(stock, expected, date, updated, collected);
            assert_flow_result(market, expected, date, updated, collected);
        }
    }
    #[test]
    fn deterministic_flow_dates_use_kst_and_preserve_original_observations() {
        for (seconds, nanos, expected) in [
            (
                53_999,
                999_999_999,
                Err(crate::MarketDataError::InvalidFlow),
            ),
            (54_000, 0, Ok(())),
            (55_800, 123_456_789, Ok(())),
        ] {
            assert_flow_date_case(seconds, nanos, expected);
        }
        for seconds in [53_999, 54_000] {
            assert_eq!(
                super::validate_flow_date(at(0).offset_calendar_date(), at(seconds)),
                Ok(())
            );
        }
        assert_eq!(
            super::validate_flow_date(at(86_400).offset_calendar_date(), at(253_402_300_799)),
            Err(crate::MarketDataError::InvalidFlow)
        );
    }
}
