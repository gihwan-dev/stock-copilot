use crate::{
    AcquisitionPrice, BuyingPower, Currency, FxRate, Holding, HoldingObservation, Money, Portfolio,
    PortfolioValuation, Price, Quantity, ResolvedOrderInstrument, Total, WholeOrderQuantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedTrade {
    instrument: ResolvedOrderInstrument,
    side: TradeSide,
    quantity: WholeOrderQuantity,
    price: Option<Price>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeSimulationError {
    MissingPrice(ResolvedOrderInstrument),
    PriceCurrencyMismatch(ResolvedOrderInstrument),
    SellExceedsHolding(ResolvedOrderInstrument),
    InsufficientCash(Currency),
    CashUnavailable(Currency),
    Arithmetic,
    InvalidPortfolio,
    TooManyTrades,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedFill {
    instrument: ResolvedOrderInstrument,
    side: TradeSide,
    quantity: WholeOrderQuantity,
    price: Price,
    amount: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeSimulation {
    valuation: PortfolioValuation,
    fills: Vec<SimulatedFill>,
}

impl SimulatedTrade {
    #[must_use]
    pub const fn new(
        instrument: ResolvedOrderInstrument,
        side: TradeSide,
        quantity: WholeOrderQuantity,
        price: Option<Price>,
    ) -> Self {
        Self {
            instrument,
            side,
            quantity,
            price,
        }
    }
}

impl SimulatedFill {
    #[must_use]
    pub const fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn side(&self) -> TradeSide {
        self.side
    }
    #[must_use]
    pub const fn quantity(&self) -> WholeOrderQuantity {
        self.quantity
    }
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub const fn amount(&self) -> Money {
        self.amount
    }
}

impl TradeSimulation {
    #[must_use]
    pub const fn valuation(&self) -> &PortfolioValuation {
        &self.valuation
    }
    #[must_use]
    pub fn fills(&self) -> &[SimulatedFill] {
        &self.fills
    }
}

struct Book {
    holdings: Vec<Holding>,
    krw: Money,
    usd: Money,
    observed_at: crate::Timestamp,
}

impl Book {
    fn cash_mut(&mut self, currency: Currency) -> &mut Money {
        match currency {
            Currency::Krw => &mut self.krw,
            Currency::Usd => &mut self.usd,
        }
    }

    fn apply(&mut self, trade: &SimulatedTrade) -> Result<SimulatedFill, TradeSimulationError> {
        let held = self
            .holdings
            .iter()
            .find(|holding| holding.instrument() == &trade.instrument)
            .cloned();
        let price = trade
            .price
            .or_else(|| held.as_ref().and_then(Holding::current_price))
            .ok_or_else(|| TradeSimulationError::MissingPrice(trade.instrument.clone()))?;
        let currency = price.as_money().currency();
        if market_currency(&trade.instrument) != Some(currency) {
            return Err(TradeSimulationError::PriceCurrencyMismatch(
                trade.instrument.clone(),
            ));
        }
        let amount = price
            .checked_mul(&trade.quantity.quantity())
            .map_err(|_overflow| TradeSimulationError::Arithmetic)?;
        let held_quantity = match held.as_ref() {
            Some(holding) => holding.quantity(),
            None => Quantity::try_new(rust_decimal::Decimal::ZERO)
                .map_err(|_negative| TradeSimulationError::Arithmetic)?,
        };
        let next_decimal = match trade.side {
            TradeSide::Buy => held_quantity
                .as_decimal()
                .checked_add(trade.quantity.quantity().as_decimal()),
            TradeSide::Sell => held_quantity
                .as_decimal()
                .checked_sub(trade.quantity.quantity().as_decimal()),
        }
        .ok_or(TradeSimulationError::Arithmetic)?;
        let next_quantity = Quantity::try_new(next_decimal).map_err(|_negative| {
            TradeSimulationError::SellExceedsHolding(trade.instrument.clone())
        })?;
        let cash = self.cash_mut(currency);
        let next_cash = match trade.side {
            TradeSide::Buy => cash.checked_sub(&amount),
            TradeSide::Sell => cash.checked_add(&amount),
        }
        .map_err(|_overflow| TradeSimulationError::Arithmetic)?;
        if next_cash.amount().as_decimal().is_sign_negative() {
            return Err(TradeSimulationError::InsufficientCash(currency));
        }
        *cash = next_cash;
        self.holdings
            .retain(|holding| holding.instrument() != &trade.instrument);
        self.replace(held.as_ref(), trade, price, next_quantity)?;
        Ok(SimulatedFill {
            instrument: trade.instrument.clone(),
            side: trade.side,
            quantity: trade.quantity,
            price,
            amount,
        })
    }

    fn replace(
        &mut self,
        held: Option<&Holding>,
        trade: &SimulatedTrade,
        price: Price,
        quantity: Quantity,
    ) -> Result<(), TradeSimulationError> {
        if quantity.as_decimal().is_zero() {
            return Ok(());
        }
        let acquisition = match held {
            Some(holding) => holding.acquisition_price(),
            None => AcquisitionPrice::try_new(price.as_money())
                .map_err(|_invalid| TradeSimulationError::Arithmetic)?,
        };
        let observed_at = held.map_or(self.observed_at, Holding::observed_at);
        let rebuilt = Holding::new(
            trade.instrument.clone(),
            quantity,
            acquisition,
            HoldingObservation::new(Some(price), None, observed_at),
        )
        .map_err(|_mismatch| {
            TradeSimulationError::PriceCurrencyMismatch(trade.instrument.clone())
        })?;
        let named = match held.and_then(Holding::reported_name) {
            Some(name) => rebuilt.with_reported_name(name.clone()),
            None => rebuilt,
        };
        self.holdings.push(named);
        Ok(())
    }
}

fn market_currency(instrument: &ResolvedOrderInstrument) -> Option<Currency> {
    match instrument.market() {
        crate::Market::Kr => Some(Currency::Krw),
        crate::Market::Us => Some(Currency::Usd),
        crate::Market::Unknown(_) => None,
    }
}

pub fn valuation_fx_rate(valuation: &PortfolioValuation) -> Option<FxRate> {
    let rates = match valuation.assets() {
        Total::Complete(value) => value.value().clone(),
        Total::Incomplete(value) => value.subtotal().clone(),
    }
    .into_parts()
    .1;
    rates.into_iter().next()
}

fn asset_total(valuation: &PortfolioValuation) -> Money {
    match valuation.assets() {
        Total::Complete(value) => value.value().clone(),
        Total::Incomplete(value) => value.subtotal().clone(),
    }
    .into_parts()
    .0
}

pub fn krw_exposures(
    valuation: &PortfolioValuation,
) -> Result<Vec<(crate::AllocationInstrument, Money)>, crate::AllocationError> {
    let rates = match valuation.assets() {
        Total::Complete(value) => value.value().clone(),
        Total::Incomplete(value) => value.subtotal().clone(),
    }
    .into_parts()
    .1;
    crate::allocation::policy_krw_holding_values(valuation, &rates)
}

pub fn room_to_cap(
    valuation: &PortfolioValuation,
    value_krw: Money,
    cap: crate::ProfileWeight,
) -> Result<Money, crate::MoneyError> {
    let limit = asset_total(valuation).checked_mul(&cap.as_ratio())?;
    limit.checked_sub(&value_krw)
}

pub fn simulate_trades(
    valuation: &PortfolioValuation,
    trades: &[SimulatedTrade],
) -> Result<TradeSimulation, TradeSimulationError> {
    if trades.len() > 20_usize {
        return Err(TradeSimulationError::TooManyTrades);
    }
    let krw_power = valuation.buying_power(Currency::Krw);
    let usd_power = valuation.buying_power(Currency::Usd);
    let mut book = Book {
        holdings: valuation
            .holdings()
            .iter()
            .map(|valued| valued.holding().clone())
            .collect(),
        krw: krw_power
            .money()
            .map_err(|_unknown| TradeSimulationError::CashUnavailable(Currency::Krw))?,
        usd: usd_power
            .money()
            .map_err(|_unknown| TradeSimulationError::CashUnavailable(Currency::Usd))?,
        observed_at: krw_power.observed_at(),
    };
    let fills = trades
        .iter()
        .map(|trade| book.apply(trade))
        .collect::<Result<Vec<_>, _>>()?;
    let portfolio = Portfolio::new(
        book.holdings,
        BuyingPower::new(book.krw, krw_power.observed_at()),
        BuyingPower::new(book.usd, usd_power.observed_at()),
    )
    .map_err(|_invalid| TradeSimulationError::InvalidPortfolio)?;
    Ok(TradeSimulation {
        valuation: portfolio.evaluate(valuation_fx_rate(valuation)),
        fills,
    })
}

#[cfg(test)]
mod tests {
    use super::{SimulatedTrade, TradeSide, TradeSimulationError, simulate_trades};
    use crate::{
        AcquisitionPrice, Allocation, AssetInstrument, BuyingPower, Currency, Holding,
        HoldingObservation, Market, Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio,
        PortfolioValuation, Price, Quantity, ResolvedOrderInstrument, SectorAssignments, Symbol,
        Timestamp, UnixSeconds, WholeOrderQuantity,
    };

    fn input(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("입력")
    }

    fn krw(text: &str) -> Money {
        Money::parse(&input(text), Currency::Krw).expect("금액")
    }

    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn instrument(symbol: &str) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("심볼"),
            Market::Kr,
        )
        .expect("종목")
    }

    fn holding(symbol: &str, quantity: &str, price: &str) -> Holding {
        Holding::new(
            instrument(symbol),
            Quantity::parse(&input(quantity)).expect("수량"),
            AcquisitionPrice::try_new(krw(price)).expect("원가"),
            HoldingObservation::new(
                Some(Price::try_new(krw(price)).expect("가격")),
                None,
                time(),
            ),
        )
        .expect("보유")
    }

    fn valuation() -> PortfolioValuation {
        Portfolio::new(
            vec![holding("AAA", "10", "1000"), holding("BBB", "5", "2000")],
            BuyingPower::new(krw("10000"), time()),
            BuyingPower::new(Money::new(krw("0").amount(), Currency::Usd), time()),
        )
        .expect("포트폴리오")
        .evaluate(None)
    }

    fn trade(symbol: &str, side: TradeSide, quantity: &str, price: Option<&str>) -> SimulatedTrade {
        SimulatedTrade::new(
            instrument(symbol),
            side,
            WholeOrderQuantity::try_new(Quantity::parse(&input(quantity)).expect("수량"))
                .expect("정수 수량"),
            price.map(|value| Price::try_new(krw(value)).expect("가격")),
        )
    }

    fn weight_units(valuation: &PortfolioValuation, symbol: &str) -> Option<u32> {
        let allocation = Allocation::new(valuation, &SectorAssignments::default());
        let instruments = allocation.assets().instruments().clone().expect("구성");
        instruments
            .entries()
            .iter()
            .find_map(|entry| match entry.key() {
                AssetInstrument::Holding(held) if held.symbol().as_str() == symbol => {
                    Some(entry.display().units().as_u32())
                }
                AssetInstrument::Holding(_) | AssetInstrument::Cash(_) => None,
            })
    }

    #[test]
    fn selling_and_buying_moves_cash_and_weights_like_the_screen() {
        let before = valuation();
        match weight_units(&before, "AAA") {
            Some(3333 | 3334) => {}
            other => panic!("셋으로 나눈 비중 {other:?}"),
        }
        let result = simulate_trades(
            &before,
            &[
                trade("AAA", TradeSide::Sell, "10", None),
                trade("CCC", TradeSide::Buy, "3", Some("5000")),
            ],
        )
        .expect("모의 매매");
        assert_eq!(weight_units(result.valuation(), "AAA"), None);
        assert_eq!(weight_units(result.valuation(), "CCC"), Some(5000));
        assert_eq!(
            result.valuation().buying_power(Currency::Krw).money(),
            Ok(krw("5000"))
        );
        let amounts: Vec<Money> = result
            .fills()
            .iter()
            .map(super::SimulatedFill::amount)
            .collect();
        assert_eq!(amounts, vec![krw("10000"), krw("15000")]);
    }

    #[test]
    fn impossible_trades_are_reported_instead_of_guessed() {
        let before = valuation();
        assert_eq!(
            simulate_trades(&before, &[trade("BBB", TradeSide::Sell, "6", None)]),
            Err(TradeSimulationError::SellExceedsHolding(instrument("BBB")))
        );
        assert_eq!(
            simulate_trades(&before, &[trade("CCC", TradeSide::Buy, "1", None)]),
            Err(TradeSimulationError::MissingPrice(instrument("CCC")))
        );
        assert_eq!(
            simulate_trades(&before, &[trade("AAA", TradeSide::Buy, "11", None)]),
            Err(TradeSimulationError::InsufficientCash(Currency::Krw))
        );
        assert_eq!(
            simulate_trades(&before, &[]).map(|result| result.fills().len()),
            Ok(0)
        );
    }

    #[test]
    fn room_to_cap_is_the_amount_left_before_the_limit() {
        let before = valuation();
        let exposures = super::krw_exposures(&before).expect("노출");
        let (_, value) = exposures
            .iter()
            .find(|(held, _)| held.symbol().as_str() == "AAA")
            .expect("AAA");
        let cap = crate::ProfileWeight::parse_percent(&input("20")).expect("한도");
        assert_eq!(super::room_to_cap(&before, *value, cap), Ok(krw("-4000")));
    }

    #[test]
    fn buying_an_existing_holding_keeps_its_cost_basis() {
        let result = simulate_trades(&valuation(), &[trade("BBB", TradeSide::Buy, "5", None)])
            .expect("모의 매매");
        let bought = result
            .valuation()
            .holdings()
            .iter()
            .find(|held| held.instrument().symbol().as_str() == "BBB")
            .expect("보유");
        assert_eq!(
            bought.holding().quantity(),
            Quantity::parse(&input("10")).expect("수량")
        );
        assert_eq!(bought.holding().acquisition_price().money(), krw("2000"));
        assert_eq!(weight_units(result.valuation(), "BBB"), Some(6667));
    }
}
