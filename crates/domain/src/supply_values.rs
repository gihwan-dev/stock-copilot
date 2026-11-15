use crate::{Amount, MarketDataError, NumericInput};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyShares(Amount);
impl SupplyShares {
    pub(crate) const fn to_amount(self) -> Amount {
        self.0
    }
    pub fn parse(input: &NumericInput) -> Result<Self, MarketDataError> {
        let value = input
            .parse_amount()
            .or(Err(MarketDataError::InvalidNumber))?;
        if value.as_decimal() < Decimal::ZERO || !value.as_decimal().fract().is_zero() {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self(value))
    }
}
impl std::fmt::Display for SupplyShares {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0.as_decimal(), f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplySignedShares(Amount);
impl SupplySignedShares {
    pub(crate) const fn to_amount(self) -> Amount {
        self.0
    }
    pub fn parse(input: &NumericInput) -> Result<Self, MarketDataError> {
        let value = input
            .parse_amount()
            .or(Err(MarketDataError::InvalidNumber))?;
        if !value.as_decimal().fract().is_zero() {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self(value))
    }
}
impl std::fmt::Display for SupplySignedShares {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0.as_decimal(), f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyKrw(Amount);
impl SupplyKrw {
    pub fn parse(input: &NumericInput) -> Result<Self, MarketDataError> {
        let value = input
            .parse_amount()
            .or(Err(MarketDataError::InvalidNumber))?;
        if value.as_decimal() < Decimal::ZERO || !value.as_decimal().fract().is_zero() {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self(value))
    }
}
impl std::fmt::Display for SupplyKrw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0.as_decimal(), f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyRate(Amount);
impl SupplyRate {
    pub fn parse(input: &NumericInput) -> Result<Self, MarketDataError> {
        let value = input
            .parse_amount()
            .or(Err(MarketDataError::InvalidNumber))?;
        if value.as_decimal() < Decimal::ZERO || value.as_decimal() > Decimal::ONE {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self(value))
    }
}
impl std::fmt::Display for SupplyRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0.as_decimal(), f)
    }
}

#[cfg(test)]
mod supply_tests {
    use crate::{
        CreditDetail, CreditSide, CreditTrade, Nanosecond, NumericInput, OffsetSeconds,
        ProgramContext, ProgramTrade, ProgramVolume, SecuritiesLending, ShortSelling,
        SupplyContext, SupplyKrw, SupplyRate, SupplyRatio, SupplyShares, SupplySignedShares,
        Symbol, Timestamp, UnixSeconds,
    };
    fn num(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("입력")
    }
    fn shares(value: &str) -> SupplyShares {
        SupplyShares::parse(&num(value)).expect("주")
    }
    fn krw(value: &str) -> SupplyKrw {
        SupplyKrw::parse(&num(value)).expect("원")
    }
    fn rate(value: &str) -> SupplyRate {
        SupplyRate::parse(&num(value)).expect("비율")
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn symbol() -> Symbol {
        Symbol::try_new("005930").expect("종목")
    }
    fn context() -> SupplyContext {
        SupplyContext::from_response(symbol(), at(0).offset_calendar_date(), at(100), at(200))
            .expect("맥락")
    }
    #[test]
    fn supply_numeric_units_boundaries_and_missing_values() {
        for value in ["0", "7", "79228162514264337593543950335"] {
            assert_eq!(shares(value).to_string(), value);
            assert_eq!(krw(value).to_string(), value);
            assert_eq!(
                SupplySignedShares::parse(&num(value))
                    .expect("부호 수량")
                    .to_string(),
                value
            );
        }
        for value in ["-1", "0.1", "", "1e2", "79228162514264337593543950336"] {
            SupplyShares::parse(&num(value)).expect_err("잘못된 입력 거부");
            SupplyKrw::parse(&num(value)).expect_err("잘못된 입력 거부");
        }
        assert_eq!(
            SupplySignedShares::parse(&num("-9"))
                .expect("순매도")
                .to_string(),
            "-9"
        );
        for value in ["0.1", "", "79228162514264337593543950336"] {
            SupplySignedShares::parse(&num(value)).expect_err("잘못된 입력 거부");
        }
        for value in ["0", "0.03215", "1"] {
            assert_eq!(rate(value).to_string(), value);
        }
        for value in ["-0.01", "1.00001", "", "1e2"] {
            SupplyRate::parse(&num(value)).expect_err("잘못된 입력 거부");
        }
        assert_ne!(
            SupplyRatio::MissingDenominator,
            SupplyRatio::Reported(rate("0"))
        );
    }
    #[test]
    fn supply_context_preserves_dates_and_distinct_observation_clocks() {
        let fact_context = context();
        assert_eq!(fact_context.symbol(), &symbol());
        assert_eq!(fact_context.date(), at(0).offset_calendar_date());
        assert_eq!(fact_context.observation().server_at(), Some(at(100)));
        assert_eq!(fact_context.observation().collected_at(), at(200));
        let program_context =
            ProgramContext::from_response(symbol(), at(0).offset_calendar_date(), at(300))
                .expect("프로그램");
        assert_eq!(program_context.symbol(), &symbol());
        assert_eq!(program_context.date(), at(0).offset_calendar_date());
        assert_eq!(program_context.observation().server_at(), None);
        assert_eq!(program_context.observation().collected_at(), at(300));
        for sym in ["AAPL", "12345"] {
            SupplyContext::from_response(
                Symbol::try_new(sym).expect("심볼"),
                at(0).offset_calendar_date(),
                at(100),
                at(200),
            )
            .expect_err("잘못된 입력 거부");
            ProgramContext::from_response(
                Symbol::try_new(sym).expect("심볼"),
                at(0).offset_calendar_date(),
                at(200),
            )
            .expect_err("잘못된 입력 거부");
        }
        SupplyContext::from_response(symbol(), at(86400).offset_calendar_date(), at(100), at(200))
            .expect_err("잘못된 입력 거부");
        ProgramContext::from_response(symbol(), at(86400).offset_calendar_date(), at(200))
            .expect_err("잘못된 입력 거부");
        SupplyContext::from_response(symbol(), at(0).offset_calendar_date(), at(1000), at(200))
            .expect_err("잘못된 입력 거부");
        SupplyContext::from_response(symbol(), at(100).offset_calendar_date(), at(100), at(100))
            .expect("같은 시각 허용");
        ProgramContext::from_response(symbol(), at(100).offset_calendar_date(), at(100))
            .expect("같은 날짜 허용");
    }
    #[test]
    fn supply_program_validates_reported_net_and_keeps_two_trading_groups() {
        let volume = |second, value, net| {
            ProgramVolume::try_new(
                shares(second),
                shares(value),
                SupplySignedShares::parse(&num(net)).expect("순수량"),
            )
        };
        let first = volume("11", "17", "-6").expect("차익");
        assert_eq!(first.buy().to_string(), "11");
        assert_eq!(first.sell().to_string(), "17");
        assert_eq!(first.net().to_string(), "-6");
        let second = volume("29", "23", "6").expect("비차익");
        let fact_context =
            ProgramContext::from_response(symbol(), at(0).offset_calendar_date(), at(300))
                .expect("맥락");
        let fact = ProgramTrade::from_response(fact_context.clone(), first.clone(), second.clone());
        assert_eq!(fact.context(), &fact_context);
        assert_eq!(fact.arbitrage(), &first);
        assert_eq!(fact.non_arbitrage(), &second);
        volume("11", "17", "6").expect_err("잘못된 입력 거부");
        volume("0", "0", "0").expect("거래 없음");
    }
    #[test]
    fn supply_short_credit_and_lending_preserve_every_distinct_field() {
        let fact_context = context();
        let value = ShortSelling::from_response(
            fact_context.clone(),
            shares("31"),
            krw("4700"),
            SupplyRatio::Reported(rate("0.03215")),
            SupplyRatio::MissingDenominator,
        );
        assert_eq!(value.context(), &fact_context);
        assert_eq!(value.volume().to_string(), "31");
        assert_eq!(value.amount().to_string(), "4700");
        assert_eq!(value.volume_rate(), &SupplyRatio::Reported(rate("0.03215")));
        assert_eq!(value.amount_rate(), &SupplyRatio::MissingDenominator);
        let detail = CreditDetail::from_response(
            shares("41"),
            shares("43"),
            shares("53"),
            rate("0.0042"),
            rate("0.09"),
        );
        assert_eq!(detail.new_quantity().to_string(), "41");
        assert_eq!(detail.return_quantity().to_string(), "43");
        assert_eq!(detail.balance_quantity().to_string(), "53");
        assert_eq!(detail.balance_rate().to_string(), "0.0042");
        assert_eq!(detail.trading_rate().to_string(), "0.09");
        let credit = CreditTrade::from_response(
            fact_context.clone(),
            CreditSide::Reported(detail.clone()),
            CreditSide::NoData,
        );
        assert_eq!(credit.context(), &fact_context);
        assert_eq!(credit.margin_loan(), &CreditSide::Reported(detail));
        assert_eq!(credit.stock_loan(), &CreditSide::NoData);
        let lending = SecuritiesLending::from_response(
            fact_context.clone(),
            shares("61"),
            shares("67"),
            shares("71"),
            krw("7300"),
        );
        assert_eq!(lending.context(), &fact_context);
        assert_eq!(lending.execution_quantity().to_string(), "61");
        assert_eq!(lending.repayment_quantity().to_string(), "67");
        assert_eq!(lending.balance_quantity().to_string(), "71");
        assert_eq!(lending.balance_amount().to_string(), "7300");
    }
}
