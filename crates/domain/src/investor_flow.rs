use crate::{
    Amount, CalendarDate, IndicatorCode, KnownMarket, MarketDataError, MarketObservation,
    MasterCode, NumericInput, Symbol, Timestamp,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowTradingRow {
    pub investor: MasterCode,
    pub buy: NumericInput,
    pub sell: NumericInput,
    pub reported_net: Option<NumericInput>,
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
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorFlow {
    subject: FlowSubject,
    date: CalendarDate,
    interval: FlowInterval,
    items: Vec<FlowItem>,
    observation: MarketObservation,
    foreign_scope: ForeignInvestorScope,
    finality: FlowFinality,
}
impl InvestorFlow {
    pub fn from_stock_response(
        symbol: Symbol,
        input: FlowRecordInput,
    ) -> Result<Self, MarketDataError> {
        let FlowRecordInput {
            date,
            updated,
            rows,
            collected,
        } = input;
        crate::market_ranking::validate_market_symbol(&symbol, KnownMarket::Kr)?;
        Ok(Self {
            subject: FlowSubject::Stock(symbol),
            date,
            interval: FlowInterval::Day,
            items: flow_items(rows, FlowUnit::Shares)?,
            observation: MarketObservation::from_response(Some(updated), collected)?,
            foreign_scope: ForeignInvestorScope::RegisteredOnly,
            finality: FlowFinality::Unknown,
        })
    }
    pub fn from_indicator_response(
        code: MasterCode,
        raw_interval: &MasterCode,
        input: FlowRecordInput,
    ) -> Result<Self, MarketDataError> {
        let FlowRecordInput {
            date,
            updated,
            rows,
            collected,
        } = input;
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
        Ok(Self {
            subject: FlowSubject::Indicator(code),
            date,
            interval,
            items: flow_items(rows, FlowUnit::Krw)?,
            observation: MarketObservation::from_response(Some(updated), collected)?,
            foreign_scope: ForeignInvestorScope::RegisteredAndUnregistered,
            finality: FlowFinality::Unknown,
        })
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
    pub fn items(&self) -> &[FlowItem] {
        &self.items
    }
}
fn flow_items(rows: Vec<FlowTradingRow>, unit: FlowUnit) -> Result<Vec<FlowItem>, MarketDataError> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for row in rows {
        if !seen.insert(row.investor.as_str().to_owned()) {
            return Err(MarketDataError::InvalidFlow);
        }
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
        if [buy, sell].iter().any(|value| {
            value.as_decimal() < Decimal::ZERO || !value.as_decimal().fract().is_zero()
        }) {
            return Err(MarketDataError::InvalidFlow);
        }
        match (unit, row.reported_net) {
            (FlowUnit::Shares, Some(input)) => {
                if input
                    .parse_amount()
                    .or(Err(MarketDataError::InvalidNumber))?
                    != net
                {
                    return Err(MarketDataError::InvalidFlow);
                }
            }
            (FlowUnit::Shares, None) | (FlowUnit::Krw, Some(_)) => {
                return Err(MarketDataError::InvalidFlow);
            }
            (FlowUnit::Krw, None) => (),
        }
        items.push(FlowItem {
            investor: InvestorClass::from_code(&row.investor),
            raw_investor: row.investor,
            buy: FlowNet::from_amount(buy, unit),
            sell: FlowNet::from_amount(sell, unit),
            net: FlowNet::from_amount(net, unit),
            unit,
        });
    }
    items.sort_by(|left, right| left.raw_investor.as_str().cmp(right.raw_investor.as_str()));
    Ok(items)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowRecordInput {
    pub date: CalendarDate,
    pub updated: Timestamp,
    pub rows: Vec<FlowTradingRow>,
    pub collected: Timestamp,
}

#[cfg(test)]
mod tests {
    use crate::{
        FlowFinality, FlowInterval, FlowRecordInput, FlowTradingRow, FlowUnit,
        ForeignInvestorScope, InvestorClass, InvestorFlow, MasterCode, Nanosecond, NumericInput,
        OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };
    fn number(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("수치")
    }
    fn code(text: &str) -> MasterCode {
        MasterCode::try_new(text).expect("코드")
    }
    fn stamp() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn flow_units_foreign_scope_and_net_are_sealed_from_raw_rows() {
        let date = stamp().offset_calendar_date();
        let input = || FlowTradingRow {
            investor: code("foreigner"),
            buy: number("100"),
            sell: number("120"),
            reported_net: Some(number("-20")),
        };
        let stock = InvestorFlow::from_stock_response(
            Symbol::try_new("005930").expect("종목"),
            FlowRecordInput {
                date,
                updated: stamp(),
                rows: vec![input()],
                collected: stamp(),
            },
        )
        .expect("수급");
        assert_eq!(stock.foreign_scope(), ForeignInvestorScope::RegisteredOnly);
        assert_eq!(stock.interval(), FlowInterval::Day);
        assert_eq!(stock.date(), date);
        assert_eq!(stock.finality(), FlowFinality::Unknown);
        assert_eq!(stock.observation().server_at(), Some(stamp()));
        let item = stock.items().first().expect("행");
        assert_eq!(item.investor(), InvestorClass::Foreigner);
        assert_eq!(item.net().to_string(), "-20");
        assert_eq!(item.unit(), FlowUnit::Shares);
        let mut raw = input();
        raw.reported_net = None;
        let market = InvestorFlow::from_indicator_response(
            code("KOSPI"),
            &code("1w"),
            FlowRecordInput {
                date,
                updated: stamp(),
                rows: vec![raw],
                collected: stamp(),
            },
        )
        .expect("시장 수급");
        assert_eq!(
            market.foreign_scope(),
            ForeignInvestorScope::RegisteredAndUnregistered
        );
        assert_eq!(market.interval(), FlowInterval::Week);
        assert_eq!(market.items().first().expect("행").unit(), FlowUnit::Krw);
    }
    #[test]
    fn invalid_flow_numbers_and_mismatched_reported_net_are_rejected() {
        let date = stamp().offset_calendar_date();
        for (buy, sell, net) in [
            ("1.5", "0", "1.5"),
            ("-1", "0", "-1"),
            ("10", "2", "7"),
            ("bad", "2", "8"),
        ] {
            let row = FlowTradingRow {
                investor: code("individual"),
                buy: number(buy),
                sell: number(sell),
                reported_net: Some(number(net)),
            };
            InvestorFlow::from_stock_response(
                Symbol::try_new("005930").expect("종목"),
                FlowRecordInput {
                    date,
                    updated: stamp(),
                    rows: vec![row],
                    collected: stamp(),
                },
            )
            .expect_err("거부");
        }
    }
    #[test]
    fn investor_catalog_and_intervals_preserve_all_schema_values() {
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
            ("1d", FlowInterval::Day),
            ("1w", FlowInterval::Week),
            ("1mo", FlowInterval::Month),
            ("1y", FlowInterval::Year),
            ("new", FlowInterval::Unknown),
        ] {
            assert_eq!(FlowInterval::from_code(&code(raw)), expected);
        }
    }
    #[test]
    fn flow_duplicate_missing_net_and_unsupported_market_period_are_rejected() {
        let row = || FlowTradingRow {
            investor: code("new"),
            buy: number("1"),
            sell: number("0"),
            reported_net: None,
        };
        let input = || FlowRecordInput {
            date: stamp().offset_calendar_date(),
            updated: stamp(),
            rows: vec![row()],
            collected: stamp(),
        };
        InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), input())
            .expect_err("순수량 누락");
        InvestorFlow::from_indicator_response(code("KR_BOND_2Y"), &code("1d"), input())
            .expect_err("국채 수급");
        InvestorFlow::from_indicator_response(code("KOSDAQ"), &code("new"), input())
            .expect_err("미지 기간");
        let mut duplicate = input();
        duplicate.rows.push(row());
        InvestorFlow::from_indicator_response(code("KOSDAQ"), &code("1d"), duplicate)
            .expect_err("중복");
        let mut reported = input();
        reported.rows.first_mut().expect("행").reported_net = Some(number("1"));
        InvestorFlow::from_indicator_response(code("KOSDAQ"), &code("1d"), reported)
            .expect_err("시장에 수량 순매수");
        let market = InvestorFlow::from_indicator_response(code("KOSDAQ"), &code("1d"), input())
            .expect("시장");
        assert_eq!(
            market.subject(),
            &crate::FlowSubject::Indicator(code("KOSDAQ"))
        );
        let item = market.items().first().expect("행");
        assert_eq!(item.raw_investor().as_str(), "new");
        assert_eq!(item.buy().to_string(), "1");
        assert_eq!(item.sell().to_string(), "0");
        assert_eq!(item.net().to_string(), "1");
    }

    #[test]
    fn flow_parsing_and_future_time_errors_propagate() {
        let row = || FlowTradingRow {
            investor: code("individual"),
            buy: number("1"),
            sell: number("0"),
            reported_net: Some(number("1")),
        };
        let input = || FlowRecordInput {
            date: stamp().offset_calendar_date(),
            updated: stamp(),
            rows: vec![row()],
            collected: stamp(),
        };
        for field in ["sell", "net"] {
            let mut invalid = input();
            let item = invalid.rows.first_mut().expect("행");
            if field == "sell" {
                item.sell = number("bad");
            } else {
                item.reported_net = Some(number("bad"));
            }
            InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), invalid)
                .expect_err("문법");
        }
        let mut future = input();
        future.collected = Timestamp::from_parts(
            UnixSeconds::new(-3),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), future.clone())
            .expect_err("미래");
        future.rows.first_mut().expect("행").reported_net = None;
        InvestorFlow::from_indicator_response(code("KOSPI"), &code("1d"), future)
            .expect_err("미래");
        InvestorFlow::from_stock_response(Symbol::try_new("AAPL").expect("종목"), input())
            .expect_err("국내 한정");
        let mut unsorted = input();
        let mut other = row();
        other.investor = code("foreigner");
        unsorted.rows = vec![other, row()];
        let fact =
            InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), unsorted)
                .expect("정렬");
        assert_eq!(
            fact.items().first().expect("행").investor(),
            InvestorClass::Foreigner
        );
    }

    #[test]
    fn flow_arithmetic_failure_is_closed_before_any_fact_is_published() {
        let input = FlowRecordInput {
            date: stamp().offset_calendar_date(),
            updated: stamp(),
            collected: stamp(),
            rows: vec![FlowTradingRow {
                investor: code("individual"),
                buy: number("-79228162514264337593543950335"),
                sell: number("79228162514264337593543950335"),
                reported_net: None,
            }],
        };
        InvestorFlow::from_stock_response(Symbol::try_new("005930").expect("종목"), input)
            .expect_err("범위 밖 차");
    }
}
