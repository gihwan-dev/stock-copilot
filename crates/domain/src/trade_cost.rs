use crate::{ClientOrderId, ConfirmedOrderFee, Currency, Market, Money, MoneyError, Ratio};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeCostError {
    InvalidRate,
    UnknownComponent,
    MarketMismatch,
    CurrencyMismatch,
    NegativeAmount,
    Arithmetic(MoneyError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostRate(Ratio);
impl CostRate {
    pub fn try_new(value: Ratio) -> Result<Self, TradeCostError> {
        if !(Decimal::ZERO..=Decimal::ONE).contains(&value.as_decimal()) {
            return Err(TradeCostError::InvalidRate);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn ratio(self) -> Ratio {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionRateFact {
    market: Market,
    rate: CostRate,
}
impl CommissionRateFact {
    #[must_use]
    pub fn from_commissions_result(market: Market, rate: CostRate) -> Self {
        Self { market, rate }
    }
    pub fn estimate_order_fee(
        &self,
        order: ClientOrderId,
        amount: Money,
    ) -> Result<ConfirmedOrderFee, TradeCostError> {
        let currency = match self.market {
            Market::Kr => Currency::Krw,
            Market::Us => Currency::Usd,
            Market::Unknown(_) => return Err(TradeCostError::MarketMismatch),
        };
        if amount.currency() != currency {
            return Err(TradeCostError::CurrencyMismatch);
        }
        if amount.amount().as_decimal().is_sign_negative() {
            return Err(TradeCostError::NegativeAmount);
        }
        let exact = amount
            .checked_mul(&self.rate.ratio())
            .map_err(TradeCostError::Arithmetic)?;
        Ok(ConfirmedOrderFee(
            order,
            crate::trade_cost_arithmetic::ceil_fee(exact),
        ))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeTaxFact {
    market: Market,
    buy: CostRate,
    sell: CostRate,
}
impl TradeTaxFact {
    #[must_use]
    pub fn from_schedule_result(market: Market, buy: CostRate, sell: CostRate) -> Self {
        Self { market, buy, sell }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeCost {
    market: Market,
    rate: Ratio,
}
impl TradeCost {
    pub fn from_facts(
        commission_input: Option<CommissionRateFact>,
        tax_input: Option<TradeTaxFact>,
    ) -> Result<Self, TradeCostError> {
        let commission = commission_input.ok_or(TradeCostError::UnknownComponent)?;
        let tax = tax_input.ok_or(TradeCostError::UnknownComponent)?;
        if commission.market != tax.market
            || matches_known_market(&commission.market) != crate::CheckResult::Passed
        {
            return Err(TradeCostError::MarketMismatch);
        }
        commission
            .rate
            .ratio()
            .checked_add(&commission.rate.ratio())
            .and_then(|rate| rate.checked_add(&tax.buy.ratio()))
            .and_then(|rate| rate.checked_add(&tax.sell.ratio()))
            .map(|rate| Self {
                market: commission.market,
                rate,
            })
            .map_err(TradeCostError::Arithmetic)
    }
    #[must_use]
    pub fn rate(&self) -> Ratio {
        self.rate
    }
    pub(crate) fn market(&self) -> Market {
        self.market.clone()
    }
}

fn matches_known_market(market: &Market) -> crate::CheckResult {
    match market {
        Market::Kr | Market::Us => crate::CheckResult::Passed,
        Market::Unknown(_) => crate::CheckResult::Failed,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ClientOrderId, CommissionRateFact, CostRate, Currency, Market, Money, NumericInput, Ratio,
        TradeCost, TradeCostError, TradeTaxFact,
    };
    use rust_decimal::Decimal;
    fn rate(text: &str) -> CostRate {
        CostRate::try_new(Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율"))
            .expect("비용률")
    }
    #[test]
    fn complete_costs_and_unknown_components_are_distinct() {
        let commission = CommissionRateFact::from_commissions_result(Market::Kr, rate("0.00025"));
        let tax = TradeTaxFact::from_schedule_result(Market::Kr, rate("0"), rate("0.002"));
        let cost =
            TradeCost::from_facts(Some(commission.clone()), Some(tax.clone())).expect("왕복 비용");
        assert_eq!(cost.rate().as_decimal(), Decimal::new(25, 4));
        assert_eq!(
            TradeCost::from_facts(None, Some(tax)),
            Err(TradeCostError::UnknownComponent)
        );
        assert_eq!(
            TradeCost::from_facts(Some(commission.clone()), None),
            Err(TradeCostError::UnknownComponent)
        );
        assert_eq!(
            TradeCost::from_facts(
                Some(commission),
                Some(TradeTaxFact::from_schedule_result(
                    Market::Us,
                    rate("0"),
                    rate("0.0000206")
                ))
            ),
            Err(TradeCostError::MarketMismatch)
        );
        for text in ["-0.01", "1.01"] {
            assert_eq!(
                CostRate::try_new(
                    Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
                ),
                Err(TradeCostError::InvalidRate)
            );
        }
    }
    #[test]
    fn fee_rounds_up_in_currency_minor_units_and_preserves_order_key() {
        for (currency, market, value, expected) in [
            (Currency::Krw, Market::Kr, "10001", "2"),
            (Currency::Usd, Market::Us, "100.01", "0.02"),
        ] {
            let fact = CommissionRateFact::from_commissions_result(market, rate("0.00015"));
            let id = ClientOrderId::try_new("cost-test").expect("키");
            let amount =
                Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액");
            let result = fact
                .estimate_order_fee(id.clone(), amount)
                .expect("예상 수수료");
            assert_eq!(result.0, id);
            assert_eq!(
                result.money(),
                Money::parse(&NumericInput::try_new(expected).expect("입력"), currency)
                    .expect("기대 금액")
            );
        }
    }

    #[test]
    fn fee_rejects_negative_wrong_currency_unknown_market_and_inexact_product() {
        let fact = CommissionRateFact::from_commissions_result(Market::Kr, rate("0.1"));
        let key = ClientOrderId::try_new("fee").expect("키");
        let money = |text, currency| {
            Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
        };
        assert_eq!(
            fact.estimate_order_fee(key.clone(), money("-1", Currency::Krw)),
            Err(TradeCostError::NegativeAmount)
        );
        assert_eq!(
            fact.estimate_order_fee(key.clone(), money("1", Currency::Usd)),
            Err(TradeCostError::CurrencyMismatch)
        );
        assert_eq!(
            fact.estimate_order_fee(
                key.clone(),
                money("0.0000000000000000000000000001", Currency::Krw)
            ),
            Err(TradeCostError::Arithmetic(crate::MoneyError::PrecisionLoss))
        );
        let unknown = Market::new(crate::MarketCode::try_new("OTHER").expect("시장"));
        let unknown_fact = CommissionRateFact::from_commissions_result(unknown.clone(), rate("0"));
        assert_eq!(
            unknown_fact.estimate_order_fee(key, money("1", Currency::Krw)),
            Err(TradeCostError::MarketMismatch)
        );
        assert_eq!(
            TradeCost::from_facts(
                Some(unknown_fact),
                Some(TradeTaxFact::from_schedule_result(
                    unknown,
                    rate("0"),
                    rate("0")
                ))
            ),
            Err(TradeCostError::MarketMismatch)
        );
    }

    #[test]
    fn both_tax_legs_and_zero_commission_are_preserved() {
        let fact = CommissionRateFact::from_commissions_result(Market::Us, rate("0"));
        let cost = TradeCost::from_facts(
            Some(fact.clone()),
            Some(TradeTaxFact::from_schedule_result(
                Market::Us,
                rate("0.001"),
                rate("0.002"),
            )),
        )
        .expect("각 방향 비용");
        assert_eq!(cost.rate(), rate("0.003").ratio());
        let fee = fact
            .estimate_order_fee(
                ClientOrderId::try_new("zero").expect("키"),
                Money::parse(&NumericInput::try_new("100").expect("입력"), Currency::Usd)
                    .expect("금액"),
            )
            .expect("0 수수료");
        assert_eq!(fee.money().amount().as_decimal(), Decimal::ZERO);
    }

    #[test]
    fn fee_ceiling_matches_independent_integer_minor_units() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_i64..=1_000_000, 0_i64..=100_000),
                |(units, numerator)| {
                    for (currency, market, scale) in [
                        (Currency::Krw, Market::Kr, 0_u32),
                        (Currency::Usd, Market::Us, 2_u32),
                    ] {
                        let amount =
                            Money::new(crate::Amount::new(Decimal::new(units, scale)), currency);
                        let fact = CommissionRateFact::from_commissions_result(
                            market,
                            CostRate::try_new(Ratio::new(Decimal::new(numerator, 5)))
                                .expect("요율"),
                        );
                        let fee = fact
                            .estimate_order_fee(
                                ClientOrderId::try_new("oracle").expect("키"),
                                amount,
                            )
                            .expect("수수료");
                        let expected = Decimal::new(
                            (units * numerator + 99_999)
                                .checked_div(100_000)
                                .expect("양의 분모"),
                            scale,
                        );
                        proptest::prop_assert_eq!(fee.money().amount().as_decimal(), expected);
                    }
                    Ok(())
                },
            )
            .expect("정수 올림 기준");
    }
}
