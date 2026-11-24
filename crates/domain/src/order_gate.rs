use crate::{
    BrokerAccountKeySelection, BrokerAccountRows, BrokerOrderSet, BuyingPower, CheckResult,
    CommissionRateFact, Currency, Draft, Market, MarketCalendar, MarketOrderPolicy, MarketState,
    MarketTradingStatus, MasterInstrument, Money, Nanosecond, OppositeOpenOrder,
    OrderExpectedAmount, OrderFxRates, OrderIntent, OrderKrwEstimate, OrderSide, OrderTerms,
    PolicyBuyDecision, PolicyCandidate, PolicySecurityFacts, PortfolioValuation, PriceLimitCheck,
    PriceLimits, PriceObservation, Proposal, SavedMarketOrderSetting, SectorAssignments,
    SellOrderQuantity, SellableQuantityDecision, SellableQuantityFact, SpanSeconds,
    StockWarningDecision, StockWarnings, TickAlignment, TickTable, TimeSpan, Timestamp,
    TradeCostError, Validated, ValidationReport, ValidationReportResults, check_sellable_quantity,
    check_stock_warnings, market_state_at, opposite_open_order, select_brokerage_account_by_id,
    tick_alignment,
};

#[derive(Debug)]
pub struct OrderGateFacts<'a> {
    pub master: &'a MasterInstrument,
    pub accounts: &'a BrokerAccountRows,
    pub calendars: &'a [MarketCalendar],
    pub market_order_policy: &'a MarketOrderPolicy,
    pub valuation: &'a PortfolioValuation,
    pub assignments: &'a SectorAssignments,
    pub security_facts: &'a PolicySecurityFacts,
    pub buying_power: Option<BuyingPower>,
    pub sellable_quantity: Option<&'a SellableQuantityFact>,
    pub open_orders: Option<&'a BrokerOrderSet>,
    pub stock_warnings: Option<&'a StockWarnings>,
    pub price_limits: Option<&'a PriceLimits>,
    pub commission_rate: Option<&'a CommissionRateFact>,
    pub order_fx_rates: OrderFxRates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateCheckDecision {
    Passed,
    Rejected,
}

pub fn validate_proposal(
    proposal: Proposal<Draft>,
    now: Timestamp,
    current: PriceObservation,
    facts: &OrderGateFacts<'_>,
) -> Result<Proposal<Validated>, ValidationReport> {
    proposal.validate_with_facts(now, current, facts)
}

#[must_use]
pub fn recheck_intent_gate(
    intent: &OrderIntent,
    now: Timestamp,
    current: &PriceObservation,
    facts: &OrderGateFacts<'_>,
) -> (GateCheckDecision, ValidationReport) {
    let base_obs = current.check(intent.instrument(), now);
    let price_observation = check_observation_identity(base_obs, current, facts.master);
    let results = ValidationReportResults {
        instrument: check_instrument(intent, facts.master),
        market_hours: check_market_hours(intent, facts.calendars, now),
        available_funds_or_quantity: check_available_funds_or_quantity(intent, current, now, facts),
        opposite_pending_order: check_opposite_pending_order(intent, facts.open_orders),
        buy_warnings: check_buy_warnings(intent, facts.stock_warnings),
        principles: check_principles(intent, current, now, facts),
        order_price: check_order_price(intent, current, facts.master, facts.price_limits),
        market_order_permission: check_market_order_permission(intent, facts.market_order_policy),
        account_match: check_account_match(intent, facts.accounts),
        price_observation,
        expiry: CheckResult::Passed,
        price_range: CheckResult::Passed,
    };
    let passed = [
        results.instrument,
        results.market_hours,
        results.available_funds_or_quantity,
        results.opposite_pending_order,
        results.buy_warnings,
        results.principles,
        results.order_price,
        results.market_order_permission,
        results.account_match,
        results.price_observation,
    ]
    .into_iter()
    .all(|item| item == CheckResult::Passed);
    let decision = if passed {
        GateCheckDecision::Passed
    } else {
        GateCheckDecision::Rejected
    };
    (decision, ValidationReport::from_results(results))
}

pub(crate) fn evaluate_report(
    proposal: &Proposal<Draft>,
    now: Timestamp,
    current: &PriceObservation,
    facts: &OrderGateFacts<'_>,
) -> ValidationReport {
    let intent = proposal.raw_intent();
    let base = proposal.report(now, current);
    let price_observation = check_observation_identity(
        base.result(crate::ValidationCheck::PriceObservation),
        current,
        facts.master,
    );
    ValidationReport::from_results(ValidationReportResults {
        instrument: check_instrument(intent, facts.master),
        market_hours: check_market_hours(intent, facts.calendars, now),
        available_funds_or_quantity: check_available_funds_or_quantity(intent, current, now, facts),
        opposite_pending_order: check_opposite_pending_order(intent, facts.open_orders),
        buy_warnings: check_buy_warnings(intent, facts.stock_warnings),
        principles: check_principles(intent, current, now, facts),
        order_price: check_order_price(intent, current, facts.master, facts.price_limits),
        market_order_permission: check_market_order_permission(intent, facts.market_order_policy),
        account_match: check_account_match(intent, facts.accounts),
        price_observation,
        expiry: base.result(crate::ValidationCheck::Expiry),
        price_range: base.result(crate::ValidationCheck::PriceRange),
    })
}

fn check_observation_identity(
    base: CheckResult,
    current: &PriceObservation,
    master: &MasterInstrument,
) -> CheckResult {
    if base != CheckResult::Passed {
        return base;
    }
    match current.instrument_id() {
        Some(observed_id) if observed_id != &master.row().instrument_id => CheckResult::Failed,
        Some(_) | None => CheckResult::Passed,
    }
}

fn check_instrument(intent: &OrderIntent, master: &MasterInstrument) -> CheckResult {
    let Ok(gate_facts) = master.order_gate_facts() else {
        return CheckResult::Failed;
    };
    let Ok(resolved) = gate_facts.instrument_for(intent.side()) else {
        return CheckResult::Failed;
    };
    if &resolved == intent.instrument() && master.quote_instrument().is_ok() {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegularCutoffRequirement {
    Required,
    NotRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderExecutionKind {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuyTotalOverflow;

fn requires_us_regular_cutoff(terms: &OrderTerms) -> RegularCutoffRequirement {
    match terms {
        OrderTerms::UsMarketAmountBuy { .. } => RegularCutoffRequirement::Required,
        OrderTerms::UsMarketSell { quantity } => {
            if quantity.quantity().as_decimal().normalize().scale() > 0 {
                RegularCutoffRequirement::Required
            } else {
                RegularCutoffRequirement::NotRequired
            }
        }
        OrderTerms::KrLimit { .. }
        | OrderTerms::KrMarket { .. }
        | OrderTerms::UsLimit { .. }
        | OrderTerms::UsMarketBuy { .. } => RegularCutoffRequirement::NotRequired,
    }
}

fn order_execution_kind(terms: &OrderTerms) -> OrderExecutionKind {
    match terms {
        OrderTerms::KrLimit { .. } | OrderTerms::UsLimit { .. } => OrderExecutionKind::Limit,
        OrderTerms::KrMarket { .. }
        | OrderTerms::UsMarketBuy { .. }
        | OrderTerms::UsMarketSell { .. }
        | OrderTerms::UsMarketAmountBuy { .. } => OrderExecutionKind::Market,
    }
}

fn check_us_regular_cutoff(calendars: &[MarketCalendar], now: Timestamp) -> CheckResult {
    let Ok(one_hour) = TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0)) else {
        return CheckResult::Failed;
    };
    let mut found_cutoff = None;
    for calendar in calendars {
        if calendar.today().market() != Market::Us {
            continue;
        }
        for segment in calendar.sessions() {
            if segment.state() != MarketState::RegularMarket {
                continue;
            }
            let interval = segment.session().interval();
            if now < interval.start() || now >= interval.end() {
                continue;
            }
            let Ok(cutoff) = interval.end().checked_sub(one_hour) else {
                return CheckResult::Failed;
            };
            if found_cutoff.is_some_and(|existing| existing != cutoff) {
                return CheckResult::Unknown;
            }
            found_cutoff = Some(cutoff);
        }
    }
    match found_cutoff {
        Some(cutoff) if now < cutoff => CheckResult::Passed,
        Some(_) | None => CheckResult::Failed,
    }
}

fn check_market_hours(
    intent: &OrderIntent,
    calendars: &[MarketCalendar],
    now: Timestamp,
) -> CheckResult {
    let state = market_state_at(calendars, &intent.market(), now);
    match state.trading_status() {
        MarketTradingStatus::Unknown => CheckResult::Unknown,
        MarketTradingStatus::Closed => CheckResult::Failed,
        MarketTradingStatus::Open => {
            if requires_us_regular_cutoff(intent.terms()) == RegularCutoffRequirement::Required {
                if state != MarketState::RegularMarket {
                    return CheckResult::Failed;
                }
                return check_us_regular_cutoff(calendars, now);
            }
            if order_execution_kind(intent.terms()) == OrderExecutionKind::Market
                && state != MarketState::RegularMarket
            {
                return CheckResult::Failed;
            }
            CheckResult::Passed
        }
    }
}

fn order_currency(intent: &OrderIntent) -> Option<Currency> {
    match intent.market() {
        Market::Kr => Some(Currency::Krw),
        Market::Us => Some(Currency::Usd),
        Market::Unknown(_) => None,
    }
}

fn order_notional(intent: &OrderIntent, current: &PriceObservation) -> Option<Money> {
    let expected_currency = order_currency(intent)?;
    let money = match *intent.terms() {
        OrderTerms::KrLimit {
            quantity, price, ..
        } => price.price().checked_mul(&quantity.quantity()).ok()?,
        OrderTerms::UsLimit {
            quantity, price, ..
        } => price.price().checked_mul(&quantity.quantity()).ok()?,
        OrderTerms::KrMarket { quantity, .. } | OrderTerms::UsMarketBuy { quantity } => {
            current.price().checked_mul(&quantity.quantity()).ok()?
        }
        OrderTerms::UsMarketSell { quantity } => {
            current.price().checked_mul(&quantity.quantity()).ok()?
        }
        OrderTerms::UsMarketAmountBuy { amount } => amount.money(),
    };
    (money.currency() == expected_currency).then_some(money)
}

fn check_buy_funds(
    intent: &OrderIntent,
    current: &PriceObservation,
    now: Timestamp,
    buying_power: Option<BuyingPower>,
    commission: Option<&CommissionRateFact>,
) -> CheckResult {
    let Some(cash) = buying_power else {
        return CheckResult::Unknown;
    };
    let Some(expected_currency) = order_currency(intent) else {
        return CheckResult::Failed;
    };
    if cash.currency() != expected_currency {
        return CheckResult::Failed;
    }
    let Ok(available) = cash.money() else {
        return CheckResult::Failed;
    };
    let Some(notional) = order_notional(intent, current) else {
        return CheckResult::Failed;
    };
    let Ok(required) = required_buy_total(intent, notional, now, commission) else {
        return CheckResult::Failed;
    };
    if available.amount().as_decimal() >= required.amount().as_decimal() {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}

fn required_buy_total(
    intent: &OrderIntent,
    notional: Money,
    now: Timestamp,
    commission: Option<&CommissionRateFact>,
) -> Result<Money, BuyTotalOverflow> {
    let Some(rate_fact) = commission else {
        return Ok(notional);
    };
    if rate_fact.market() != &intent.market()
        || rate_fact.validity().covers(now, now) != CheckResult::Passed
    {
        return Ok(notional);
    }
    match rate_fact.estimate_amount(intent.client_order_id().clone(), notional) {
        Ok(confirmed) => notional
            .checked_add(&confirmed.money())
            .map_err(|_err| BuyTotalOverflow),
        Err(TradeCostError::Arithmetic(_)) => Err(BuyTotalOverflow),
        Err(_) => Ok(notional),
    }
}

fn requested_sell_quantity(intent: &OrderIntent) -> Option<SellOrderQuantity> {
    match *intent.terms() {
        OrderTerms::KrLimit {
            side: OrderSide::Sell,
            quantity,
            ..
        }
        | OrderTerms::KrMarket {
            side: OrderSide::Sell,
            quantity,
        }
        | OrderTerms::UsLimit {
            side: OrderSide::Sell,
            quantity,
            ..
        } => SellOrderQuantity::try_new(quantity.quantity()).ok(),
        OrderTerms::UsMarketSell { quantity } => Some(quantity),
        OrderTerms::KrLimit {
            side: OrderSide::Buy,
            ..
        }
        | OrderTerms::KrMarket {
            side: OrderSide::Buy,
            ..
        }
        | OrderTerms::UsLimit {
            side: OrderSide::Buy,
            ..
        }
        | OrderTerms::UsMarketBuy { .. }
        | OrderTerms::UsMarketAmountBuy { .. } => None,
    }
}

fn check_sell_quantity(
    intent: &OrderIntent,
    sellable: Option<&SellableQuantityFact>,
    valuation: &PortfolioValuation,
) -> CheckResult {
    let Some(fact) = sellable else {
        return CheckResult::Unknown;
    };
    let Some(requested) = requested_sell_quantity(intent) else {
        return CheckResult::Failed;
    };
    if check_sellable_quantity(fact, intent.account_id(), intent.instrument(), requested)
        != SellableQuantityDecision::Allowed
    {
        return CheckResult::Failed;
    }
    let Some(held) = valuation
        .holdings()
        .iter()
        .find(|entry| entry.instrument() == intent.instrument())
    else {
        return CheckResult::Failed;
    };
    if requested.quantity() <= held.holding().quantity() {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}

fn check_available_funds_or_quantity(
    intent: &OrderIntent,
    current: &PriceObservation,
    now: Timestamp,
    facts: &OrderGateFacts<'_>,
) -> CheckResult {
    match intent.side() {
        OrderSide::Buy => check_buy_funds(
            intent,
            current,
            now,
            facts.buying_power,
            facts.commission_rate,
        ),
        OrderSide::Sell => check_sell_quantity(intent, facts.sellable_quantity, facts.valuation),
    }
}

fn check_opposite_pending_order(
    intent: &OrderIntent,
    open_orders: Option<&BrokerOrderSet>,
) -> CheckResult {
    let Some(orders) = open_orders else {
        return match intent.market() {
            Market::Us => CheckResult::Passed,
            Market::Kr | Market::Unknown(_) => CheckResult::Unknown,
        };
    };
    match opposite_open_order(
        orders,
        intent.account_id(),
        intent.instrument(),
        intent.side(),
    ) {
        OppositeOpenOrder::Absent | OppositeOpenOrder::NotApplicable => CheckResult::Passed,
        OppositeOpenOrder::Present | OppositeOpenOrder::DifferentAccount => CheckResult::Failed,
        OppositeOpenOrder::Incomplete => CheckResult::Unknown,
    }
}

fn check_buy_warnings(intent: &OrderIntent, warnings: Option<&StockWarnings>) -> CheckResult {
    if intent.side() == OrderSide::Sell {
        return CheckResult::Passed;
    }
    let Some(fact) = warnings else {
        return CheckResult::Unknown;
    };
    match check_stock_warnings(fact, intent.instrument(), intent.side()) {
        StockWarningDecision::Clear | StockWarningDecision::NotApplicable => CheckResult::Passed,
        StockWarningDecision::Blocked | StockWarningDecision::DifferentInstrument => {
            CheckResult::Failed
        }
    }
}

fn check_principles(
    intent: &OrderIntent,
    current: &PriceObservation,
    now: Timestamp,
    facts: &OrderGateFacts<'_>,
) -> CheckResult {
    if intent.side() == OrderSide::Sell {
        return match crate::Policy::check_sell() {
            crate::PolicySellDecision::Allowed => CheckResult::Passed,
        };
    }
    let Some(notional) = order_notional(intent, current) else {
        return CheckResult::Failed;
    };
    let expected = OrderExpectedAmount::from_order(
        notional,
        crate::OrderAmountBasis::Fixed,
        facts.order_fx_rates,
        now,
    );
    let requested_krw = match expected.krw_estimate() {
        OrderKrwEstimate::Native(money) => money,
        OrderKrwEstimate::Converted(converted) => converted.into_parts().0,
        OrderKrwEstimate::Unavailable(_) => return CheckResult::Unknown,
    };
    let Ok(candidate) = PolicyCandidate::new(intent.instrument(), requested_krw) else {
        return CheckResult::Failed;
    };
    match facts.market_order_policy.policy().check_buy(
        facts.valuation,
        facts.assignments,
        facts.security_facts,
        &candidate,
    ) {
        PolicyBuyDecision::Pass => CheckResult::Passed,
        PolicyBuyDecision::Reduce(_) | PolicyBuyDecision::Block(_) => CheckResult::Failed,
    }
}

fn check_order_price(
    intent: &OrderIntent,
    current: &PriceObservation,
    master: &MasterInstrument,
    price_limits: Option<&PriceLimits>,
) -> CheckResult {
    let Ok(quote_instrument) = master.quote_instrument() else {
        return CheckResult::Failed;
    };
    let limit_price = match *intent.terms() {
        OrderTerms::KrLimit { price, .. } => Some(price.price()),
        OrderTerms::UsLimit { price, .. } => Some(price.price()),
        OrderTerms::KrMarket { .. }
        | OrderTerms::UsMarketBuy { .. }
        | OrderTerms::UsMarketSell { .. }
        | OrderTerms::UsMarketAmountBuy { .. } => None,
    };
    if let Some(price) = limit_price {
        let Ok(table) = TickTable::from_master(master) else {
            return CheckResult::Failed;
        };
        if tick_alignment(&table, &quote_instrument, price) != Ok(TickAlignment::OnTick) {
            return CheckResult::Failed;
        }
    }
    let checked_price = limit_price.unwrap_or_else(|| current.price());
    match price_limits {
        Some(limits) => match limits.check(&quote_instrument, checked_price) {
            Ok(PriceLimitCheck::Within | PriceLimitCheck::Unlimited) => CheckResult::Passed,
            Ok(PriceLimitCheck::BelowLower | PriceLimitCheck::AboveUpper) | Err(_) => {
                CheckResult::Failed
            }
        },
        None => match intent.market() {
            Market::Us if limit_price.is_none() => CheckResult::Passed,
            Market::Kr | Market::Us | Market::Unknown(_) => CheckResult::Unknown,
        },
    }
}

fn check_market_order_permission(intent: &OrderIntent, policy: &MarketOrderPolicy) -> CheckResult {
    if order_execution_kind(intent.terms()) != OrderExecutionKind::Market {
        return CheckResult::Passed;
    }
    match policy.setting() {
        SavedMarketOrderSetting::Enabled => CheckResult::Passed,
        SavedMarketOrderSetting::Disabled => CheckResult::Failed,
    }
}

fn check_account_match(intent: &OrderIntent, accounts: &BrokerAccountRows) -> CheckResult {
    match select_brokerage_account_by_id(accounts, intent.account_id()) {
        BrokerAccountKeySelection::Selected(_) => CheckResult::Passed,
        BrokerAccountKeySelection::Empty
        | BrokerAccountKeySelection::NotFound
        | BrokerAccountKeySelection::NotBrokerage => CheckResult::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::{OrderGateFacts, validate_proposal};
    use crate::{
        AccountId, AcquisitionPrice, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderRow,
        BrokerOrderSet, BrokerOrderSize, BrokerOrderStatus, BrokerOrderTime, BrokerOrderType,
        BrokerTimeInForce, BuyingPower, CalendarDate, CalendarYear, CheckResult, ClientOrderId,
        CommissionRateFact, CommissionValidity, CommonShare, CostRate, Currency, DayNumber,
        DisplayInstrumentName, EstimateRangeFacts, FxPair, FxRate, FxRateKind, FxSource, Holding,
        HoldingObservation, InstrumentClassification, InstrumentId, InvestorProfile,
        KoreanMasterDetail, KrOrderPrice, LiquidationTrading, ListingStatus, Market,
        MarketCalendar, MarketOrderPolicy, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money, MonthNumber,
        Nanosecond, NumericInput, NxtTrading, OffsetSeconds, OpenOrderCoverage, OrderFxRates,
        OrderIntent, OrderSide, OrderTerms, Policy, PolicySecurityFacts, Portfolio,
        PortfolioValuation, Price, PriceLimits, PriceLimitsInput, PriceObservation, Proposal,
        ProposalId, ProposalLifetime, ProposalPriceRange, ProposalVolatility, Quantity, Ratio,
        SavedMarketOrderPolicy, SavedMarketOrderSetting, SectorAssignment, SectorAssignments,
        SectorId, SellOrderQuantity, SellableQuantityFact, SessionData, SessionInformation,
        SpanSeconds, StockWarning, StockWarnings, Symbol, TimeInterval, TimeSpan, Timestamp,
        TradingDayHorizon, TradingSuspension, UnixSeconds, UsdOrderAmount, ValidationCheck,
        WholeOrderQuantity, default_market_order_policy, restore_market_order_policy,
    };

    fn at_utc(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn at_kst(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("한국 시각")
    }

    fn at_et(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(-14_400),
        )
        .expect("미국 시각")
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }

    fn price(text: &str, currency: Currency) -> Price {
        Price::try_new(money(text, currency)).expect("가격")
    }

    fn qty(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }

    fn whole(text: &str) -> WholeOrderQuantity {
        WholeOrderQuantity::try_new(qty(text)).expect("정수 수량")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(4),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn kr_master_with(
        symbol: &str,
        id: &str,
        status: ListingStatus,
        krx: TradingSuspension,
        liquidation: LiquidationTrading,
    ) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new(id).expect("식별자"),
            name: DisplayInstrumentName::try_new("테스트 국내").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: status,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation,
                krx,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: at_kst(1_775_088_000),
        })
        .expect("국내 마스터")
    }

    fn us_master(symbol: &str, id: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new(id).expect("식별자"),
            name: DisplayInstrumentName::try_new("테스트 미국").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at_et(1_775_049_000),
        })
        .expect("미국 마스터")
    }

    fn session_data(start: Timestamp, end: Timestamp) -> SessionData {
        SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None).expect("세션")
    }

    fn kr_calendar() -> MarketCalendar {
        let day =
            |number: u8, pre: Option<(i64, i64)>, reg: (i64, i64), after: Option<(i64, i64)>| {
                MarketSession::try_new(
                    date(number),
                    SessionInformation::Kr {
                        pre_market: pre.map(|(from, to)| session_data(at_kst(from), at_kst(to))),
                        regular_market: Some(session_data(at_kst(reg.0), at_kst(reg.1))),
                        after_market: after
                            .map(|(from, to)| session_data(at_kst(from), at_kst(to))),
                    },
                )
                .expect("국내 일별")
            };
        MarketCalendar::try_new(
            day(1, None, (1_775_001_600, 1_775_025_000), None),
            day(
                2,
                Some((1_775_084_400, 1_775_088_000)),
                (1_775_088_000, 1_775_111_400),
                Some((1_775_111_400, 1_775_127_600)),
            ),
            day(3, None, (1_775_174_400, 1_775_197_800), None),
        )
        .expect("국내 달력")
    }

    fn us_calendar() -> MarketCalendar {
        let day =
            |number: u8, pre: Option<(i64, i64)>, reg: (i64, i64), after: Option<(i64, i64)>| {
                MarketSession::try_new(
                    date(number),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: pre.map(|(from, to)| session_data(at_et(from), at_et(to))),
                        regular_market: Some(session_data(at_et(reg.0), at_et(reg.1))),
                        after_market: after.map(|(from, to)| session_data(at_et(from), at_et(to))),
                    },
                )
                .expect("미국 일별")
            };
        MarketCalendar::try_new(
            day(1, None, (1_774_962_600, 1_774_986_000), None),
            day(
                2,
                Some((1_775_030_400, 1_775_049_000)),
                (1_775_049_000, 1_775_072_400),
                Some((1_775_072_400, 1_775_086_800)),
            ),
            day(3, None, (1_775_135_400, 1_775_158_800), None),
        )
        .expect("미국 달력")
    }

    fn brokerage_accounts(account_id: AccountId, code: &str) -> BrokerAccountRows {
        BrokerAccountRows::try_new(vec![
            BrokerAccountRow::from_broker_account_result(
                account_id,
                BrokerAccountTypeCode::new(code),
                BrokerAccountNumberInput::new("123456781234".to_owned()),
                at_utc(1_775_005_200),
            )
            .expect("계좌 행"),
        ])
        .expect("계좌 목록")
    }

    fn build_valuation(
        holdings: Vec<Holding>,
        krw_cash: &str,
        usd_cash: &str,
        now: Timestamp,
        fx: Option<FxRate>,
    ) -> PortfolioValuation {
        let portfolio = Portfolio::new(
            holdings,
            BuyingPower::new(money(krw_cash, Currency::Krw), now),
            BuyingPower::new(money(usd_cash, Currency::Usd), now),
        )
        .expect("포트폴리오");
        portfolio.evaluate(fx)
    }

    fn fx_rate(value: &str, kind: FxRateKind, now: Timestamp) -> FxRate {
        let start = now
            .checked_sub(TimeSpan::try_new(SpanSeconds::new(60), Nanosecond::new(0)).expect("구간"))
            .expect("시작");
        let end = now
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(600), Nanosecond::new(0)).expect("구간"),
            )
            .expect("끝");
        FxRate::parse(
            &NumericInput::try_new(value).expect("입력"),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(start, end).expect("유효 구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }

    fn build_draft(
        master: &MasterInstrument,
        account_id: AccountId,
        terms: OrderTerms,
        obs: PriceObservation,
        now: Timestamp,
    ) -> Proposal<crate::Draft> {
        let instrument = master
            .order_gate_facts()
            .expect("게이트")
            .instrument_for(terms.side())
            .expect("종목");
        let id = ProposalId::new(ClientOrderId::try_new("o01-proposal-1").expect("키"));
        let intent = OrderIntent::new(
            account_id,
            instrument.clone(),
            id.as_client_order_id().clone(),
            terms,
        )
        .expect("의도");
        let range = ProposalPriceRange::from_estimate_task(
            EstimateRangeFacts {
                instrument,
                reference: obs.price(),
                volatility: ProposalVolatility::try_new(ratio("0.05")).expect("변동성"),
                horizon: TradingDayHorizon::try_new(ratio("4")).expect("기간"),
            },
            obs,
        )
        .expect("범위");
        Proposal::new(id, intent, now, range, ProposalLifetime::default()).expect("초안")
    }

    fn assignments_for(master: &MasterInstrument) -> (SectorAssignments, PolicySecurityFacts) {
        let inst = master.order_instrument().expect("종목");
        let facts = MasterInstrumentFacts::try_new(vec![master.clone()]).expect("목록");
        let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
            &inst,
            InstrumentClassification::Sector(SectorId::try_new("TECH").expect("섹터")),
        )])
        .expect("섹터");
        (assignments, facts.policy_security_facts())
    }

    fn kr_limits(master: &MasterInstrument, low: &str, high: &str, now: Timestamp) -> PriceLimits {
        PriceLimits::try_new(PriceLimitsInput {
            instrument: master.quote_instrument().expect("시세 종목"),
            currency: Currency::Krw,
            lower: Some(price(low, Currency::Krw)),
            upper: Some(price(high, Currency::Krw)),
            timestamp: now,
            collected_at: now,
        })
        .expect("가격 제한")
    }

    fn us_limits(master: &MasterInstrument, now: Timestamp) -> PriceLimits {
        PriceLimits::try_new(PriceLimitsInput {
            instrument: master.quote_instrument().expect("시세 종목"),
            currency: Currency::Usd,
            lower: None,
            upper: None,
            timestamp: now,
            collected_at: now,
        })
        .expect("미국 가격 제한")
    }

    struct KrBuyFixture {
        master: MasterInstrument,
        accounts: BrokerAccountRows,
        calendars: Vec<MarketCalendar>,
        policy: MarketOrderPolicy,
        valuation: PortfolioValuation,
        assignments: SectorAssignments,
        security_facts: PolicySecurityFacts,
        buying_power: BuyingPower,
        open_orders: BrokerOrderSet,
        warnings: StockWarnings,
        limits: PriceLimits,
        now: Timestamp,
        obs: PriceObservation,
        draft: Proposal<crate::Draft>,
    }

    impl KrBuyFixture {
        fn new() -> Self {
            let now = at_kst(1_775_091_600);
            let master = kr_master_with(
                "005930",
                "kr-005930",
                ListingStatus::Active,
                TradingSuspension::Clear,
                LiquidationTrading::Regular,
            );
            let account = AccountId::new(1001);
            let accounts = brokerage_accounts(account.clone(), "BROKERAGE");
            let calendars = vec![kr_calendar()];
            let policy = default_market_order_policy(Policy::new(InvestorProfile::default()));
            let valuation = build_valuation(Vec::new(), "10000000", "0", now, None);
            let (assignments, security_facts) = assignments_for(&master);
            let buying_power = BuyingPower::new(money("10000000", Currency::Krw), now);
            let open_orders = BrokerOrderSet::from_open_lookup_result(
                account.clone(),
                Vec::new(),
                OpenOrderCoverage::Complete,
                now,
            )
            .expect("미체결");
            let warnings = StockWarnings::from_lookup_result(
                master.order_instrument().expect("종목"),
                Vec::new(),
                now,
            );
            let limits = kr_limits(&master, "50000", "90000", now);
            let obs = master
                .price_observation(price("70000", Currency::Krw), now)
                .expect("관측");
            let draft = build_draft(
                &master,
                account,
                OrderTerms::KrLimit {
                    side: OrderSide::Buy,
                    quantity: whole("10"),
                    price: KrOrderPrice::try_new(price("70000", Currency::Krw)).expect("지정가"),
                },
                obs.clone(),
                now,
            );
            Self {
                master,
                accounts,
                calendars,
                policy,
                valuation,
                assignments,
                security_facts,
                buying_power,
                open_orders,
                warnings,
                limits,
                now,
                obs,
                draft,
            }
        }

        fn facts(&self) -> OrderGateFacts<'_> {
            OrderGateFacts {
                master: &self.master,
                accounts: &self.accounts,
                calendars: &self.calendars,
                market_order_policy: &self.policy,
                valuation: &self.valuation,
                assignments: &self.assignments,
                security_facts: &self.security_facts,
                buying_power: Some(self.buying_power),
                sellable_quantity: None,
                open_orders: Some(&self.open_orders),
                stock_warnings: Some(&self.warnings),
                price_limits: Some(&self.limits),
                commission_rate: None,
                order_fx_rates: OrderFxRates::missing(),
            }
        }
    }

    #[test]
    fn kr_limit_buy_passes_when_all_twelve_checks_pass() {
        let fixture = KrBuyFixture::new();
        let validated = validate_proposal(
            fixture.draft.clone(),
            fixture.now,
            fixture.obs.clone(),
            &fixture.facts(),
        )
        .expect("모든 게이트 통과");
        assert_eq!(
            validated.intent().symbol().as_str(),
            "005930",
            "검증된 제안의 종목 보존"
        );
    }

    #[test]
    fn kr_buy_rejects_instrument_and_closed_hours() {
        let fixture = KrBuyFixture::new();
        let suspended = kr_master_with(
            "005930",
            "kr-005930",
            ListingStatus::Active,
            TradingSuspension::Suspended,
            LiquidationTrading::Regular,
        );
        let suspended_facts = OrderGateFacts {
            master: &suspended,
            ..fixture.facts()
        };
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(fixture.now, &fixture.obs, &suspended_facts)
                .result(ValidationCheck::Instrument),
            CheckResult::Failed,
            "거래정지 종목은 거부"
        );
        let closed_now = at_kst(1_775_080_800);
        let closed_obs = fixture
            .master
            .price_observation(price("70000", Currency::Krw), closed_now)
            .expect("관측");
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(closed_now, &closed_obs, &fixture.facts())
                .result(ValidationCheck::MarketHours),
            CheckResult::Failed,
            "장 마감 시각은 거부"
        );
    }

    #[test]
    fn kr_buy_rejects_funds_and_fee_shortfall() {
        let fixture = KrBuyFixture::new();
        let low_bp = BuyingPower::new(money("699999", Currency::Krw), fixture.now);
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        buying_power: Some(low_bp),
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Failed,
            "매수가능금액 부족은 거부"
        );
        let fee_rate = CommissionRateFact::from_commissions_result(
            Market::Kr,
            CostRate::try_new(ratio("0.01")).expect("수수료율"),
            CommissionValidity::try_new(None, None).expect("유효"),
        );
        let exact_bp = BuyingPower::new(money("700000", Currency::Krw), fixture.now);
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        buying_power: Some(exact_bp),
                        commission_rate: Some(&fee_rate),
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Failed,
            "수수료 포함 시 매수가능금액 부족은 거부"
        );
    }

    #[test]
    fn kr_buy_rejects_opposite_order_and_buy_warnings() {
        let fixture = KrBuyFixture::new();
        let opposite = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(1001),
            vec![BrokerOrderRow {
                account_id: AccountId::new(1001),
                order_id: BrokerOrderId::try_new("opp-1").expect("주문 ID"),
                instrument: fixture.master.order_instrument().expect("종목"),
                side: OrderSide::Sell,
                status: BrokerOrderStatus::Pending,
                order_type: BrokerOrderType::Limit,
                time_in_force: BrokerTimeInForce::Day,
                price: BrokerOrderPrice::Known(price("70000", Currency::Krw)),
                size: BrokerOrderSize::Quantity(qty("1")),
                filled_quantity: qty("0"),
                average_filled_price: None,
                filled_amount: BrokerOrderMoney::Absent,
                filled_at: None,
                ordered_at: fixture.now,
                canceled_at: BrokerOrderTime::Absent,
                observed_at: fixture.now,
            }],
            OpenOrderCoverage::Complete,
            fixture.now,
        )
        .expect("반대 미체결");
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        open_orders: Some(&opposite),
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::OppositePendingOrder),
            CheckResult::Failed,
            "국내 반대 방향 미체결은 거부"
        );
        let blocked_warnings = StockWarnings::from_lookup_result(
            fixture.master.order_instrument().expect("종목"),
            vec![StockWarning::Overheated],
            fixture.now,
        );
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        stock_warnings: Some(&blocked_warnings),
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::BuyWarnings),
            CheckResult::Failed,
            "매수 유의사항 종목은 거부"
        );
    }

    #[test]
    fn kr_buy_rejects_account_limits_and_principles() {
        let fixture = KrBuyFixture::new();
        let wrong_account = brokerage_accounts(AccountId::new(9999), "BROKERAGE");
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        accounts: &wrong_account,
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::AccountMatch),
            CheckResult::Failed,
            "계좌 불일치는 거부"
        );
        let narrow_limits = kr_limits(&fixture.master, "50000", "69000", fixture.now);
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        price_limits: Some(&narrow_limits),
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::OrderPrice),
            CheckResult::Failed,
            "상한가 초과는 거부"
        );
        let small_valuation = build_valuation(Vec::new(), "1000000", "0", fixture.now, None);
        assert_eq!(
            fixture
                .draft
                .evaluate_gate(
                    fixture.now,
                    &fixture.obs,
                    &OrderGateFacts {
                        valuation: &small_valuation,
                        ..fixture.facts()
                    }
                )
                .result(ValidationCheck::Principles),
            CheckResult::Failed,
            "종목 비중 20% 초과는 거부"
        );
    }

    struct UsFixture {
        master: MasterInstrument,
        account: AccountId,
        accounts: BrokerAccountRows,
        calendars: Vec<MarketCalendar>,
        enabled_policy: MarketOrderPolicy,
        disabled_policy: MarketOrderPolicy,
        assignments: SectorAssignments,
        security_facts: PolicySecurityFacts,
        before_cutoff: Timestamp,
        exact_cutoff: Timestamp,
        fx_rates: OrderFxRates,
        mid_fx: FxRate,
        warnings: StockWarnings,
        limits: PriceLimits,
        obs: PriceObservation,
        cutoff_obs: PriceObservation,
    }

    impl UsFixture {
        fn new() -> Self {
            let master = us_master("AAPL", "us-aapl");
            let account = AccountId::new(2002);
            let accounts = brokerage_accounts(account.clone(), "BROKERAGE");
            let calendars = vec![us_calendar()];
            let (assignments, security_facts) = assignments_for(&master);
            let enabled_policy = restore_market_order_policy(SavedMarketOrderPolicy {
                policy: Policy::new(InvestorProfile::default()),
                market_orders: SavedMarketOrderSetting::Enabled,
            });
            let disabled_policy =
                default_market_order_policy(Policy::new(InvestorProfile::default()));
            let before_cutoff = at_et(1_775_068_799);
            let exact_cutoff = at_et(1_775_068_800);
            let buy_fx = fx_rate("1400", FxRateKind::Buy, before_cutoff);
            let mid_fx = fx_rate("1390", FxRateKind::Mid, before_cutoff);
            let fx_rates = OrderFxRates::from_broker_fx_result(buy_fx, mid_fx);
            let warnings = StockWarnings::from_lookup_result(
                master.order_instrument().expect("종목"),
                Vec::new(),
                before_cutoff,
            );
            let limits = us_limits(&master, before_cutoff);
            let obs = master
                .price_observation(price("200", Currency::Usd), before_cutoff)
                .expect("관측");
            let cutoff_obs = master
                .price_observation(price("200", Currency::Usd), exact_cutoff)
                .expect("경계 관측");
            Self {
                master,
                account,
                accounts,
                calendars,
                enabled_policy,
                disabled_policy,
                assignments,
                security_facts,
                before_cutoff,
                exact_cutoff,
                fx_rates,
                mid_fx,
                warnings,
                limits,
                obs,
                cutoff_obs,
            }
        }

        fn facts<'a>(
            &'a self,
            valuation: &'a PortfolioValuation,
            buying_power: Option<BuyingPower>,
            sellable: Option<&'a SellableQuantityFact>,
            warnings: Option<&'a StockWarnings>,
        ) -> OrderGateFacts<'a> {
            OrderGateFacts {
                master: &self.master,
                accounts: &self.accounts,
                calendars: &self.calendars,
                market_order_policy: &self.enabled_policy,
                valuation,
                assignments: &self.assignments,
                security_facts: &self.security_facts,
                buying_power,
                sellable_quantity: sellable,
                open_orders: None,
                stock_warnings: warnings,
                price_limits: Some(&self.limits),
                commission_rate: None,
                order_fx_rates: self.fx_rates,
            }
        }
    }

    #[test]
    fn us_amount_buy_enforces_one_hour_regular_cutoff_and_market_permission() {
        let fixture = UsFixture::new();
        let valuation = build_valuation(
            Vec::new(),
            "100000000",
            "10000",
            fixture.before_cutoff,
            Some(fixture.mid_fx),
        );
        let draft = build_draft(
            &fixture.master,
            fixture.account.clone(),
            OrderTerms::UsMarketAmountBuy {
                amount: UsdOrderAmount::try_new(money("100", Currency::Usd)).expect("금액"),
            },
            fixture.obs.clone(),
            fixture.before_cutoff,
        );
        let bp = Some(BuyingPower::new(
            money("500", Currency::Usd),
            fixture.before_cutoff,
        ));
        let facts_ok = fixture.facts(&valuation, bp, None, Some(&fixture.warnings));
        assert!(
            validate_proposal(
                draft.clone(),
                fixture.before_cutoff,
                fixture.obs.clone(),
                &facts_ok
            )
            .is_ok(),
            "종료 1시간 1초 전 미국 금액 주문 통과"
        );
        assert_eq!(
            draft
                .evaluate_gate(
                    fixture.before_cutoff,
                    &fixture.obs,
                    &OrderGateFacts {
                        market_order_policy: &fixture.disabled_policy,
                        ..facts_ok
                    }
                )
                .result(ValidationCheck::MarketOrderPermission),
            CheckResult::Failed,
            "시장가 비허용 시 거부"
        );
        assert_eq!(
            draft
                .evaluate_gate(fixture.exact_cutoff, &fixture.cutoff_obs, &facts_ok)
                .result(ValidationCheck::MarketHours),
            CheckResult::Failed,
            "정규장 종료 정확히 1시간 전은 금액 주문 거부"
        );
    }

    fn us_holding(fixture: &UsFixture) -> Holding {
        Holding::new(
            fixture.master.order_instrument().expect("종목"),
            qty("1.5"),
            AcquisitionPrice::try_new(money("150", Currency::Usd)).expect("평균가"),
            HoldingObservation::new(
                Some(price("200", Currency::Usd)),
                None,
                fixture.before_cutoff,
            ),
        )
        .expect("보유")
    }

    #[test]
    fn us_fractional_sell_enforces_one_hour_cutoff_and_sellable_quantity() {
        let fixture = UsFixture::new();
        let inst = fixture.master.order_instrument().expect("종목");
        let valuation = build_valuation(
            vec![us_holding(&fixture)],
            "0",
            "0",
            fixture.before_cutoff,
            Some(fixture.mid_fx),
        );
        let sellable = SellableQuantityFact::from_lookup_result(
            fixture.account.clone(),
            inst.clone(),
            qty("1.5"),
            fixture.before_cutoff,
        )
        .expect("매도가능");
        let draft = build_draft(
            &fixture.master,
            fixture.account.clone(),
            OrderTerms::UsMarketSell {
                quantity: SellOrderQuantity::try_new(qty("0.5")).expect("소수 수량"),
            },
            fixture.obs.clone(),
            fixture.before_cutoff,
        );
        let sell_facts = fixture.facts(&valuation, None, Some(&sellable), None);
        assert!(
            validate_proposal(
                draft.clone(),
                fixture.before_cutoff,
                fixture.obs.clone(),
                &sell_facts
            )
            .is_ok(),
            "종료 1시간 1초 전 소수 매도 통과"
        );
        assert_eq!(
            draft
                .evaluate_gate(fixture.exact_cutoff, &fixture.cutoff_obs, &sell_facts)
                .result(ValidationCheck::MarketHours),
            CheckResult::Failed,
            "정규장 종료 정확히 1시간 전은 소수 매도 거부"
        );
        let low = SellableQuantityFact::from_lookup_result(
            fixture.account.clone(),
            inst,
            qty("0.4"),
            fixture.before_cutoff,
        )
        .expect("매도가능 부족");
        let low_facts = fixture.facts(&valuation, None, Some(&low), None);
        assert_eq!(
            draft
                .evaluate_gate(fixture.before_cutoff, &fixture.obs, &low_facts)
                .result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Failed,
            "매도가능수량 초과는 거부"
        );
    }

    #[test]
    fn tick_misalignment_and_unknown_inputs_fail_closed() {
        let fixture = KrBuyFixture::new();
        let bad_tick = build_draft(
            &fixture.master,
            AccountId::new(1001),
            OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: whole("1"),
                price: KrOrderPrice::try_new(price("70050", Currency::Krw)).expect("가격"),
            },
            fixture.obs.clone(),
            fixture.now,
        );
        assert_eq!(
            bad_tick
                .evaluate_gate(fixture.now, &fixture.obs, &fixture.facts())
                .result(ValidationCheck::OrderPrice),
            CheckResult::Failed,
            "호가 단위 불일치는 거부"
        );
        let unknown_facts = OrderGateFacts {
            calendars: &[],
            buying_power: None,
            open_orders: None,
            stock_warnings: None,
            price_limits: None,
            ..fixture.facts()
        };
        let report = bad_tick.evaluate_gate(fixture.now, &fixture.obs, &unknown_facts);
        assert_eq!(
            report.result(ValidationCheck::MarketHours),
            CheckResult::Unknown,
            "달력 부재는 Unknown"
        );
        assert_eq!(
            report.result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Unknown,
            "매수가능금액 부재는 Unknown"
        );
        assert_eq!(
            report.result(ValidationCheck::OppositePendingOrder),
            CheckResult::Unknown,
            "국내 미체결 부재는 Unknown"
        );
        assert_eq!(
            report.result(ValidationCheck::BuyWarnings),
            CheckResult::Unknown,
            "유의사항 부재는 Unknown"
        );
        assert_eq!(
            validate_proposal(bad_tick, fixture.now, fixture.obs.clone(), &unknown_facts)
                .expect_err("거부"),
            report,
            "Unknown 항목은 통과하지 못한다"
        );
    }

    #[test]
    fn sell_rejects_missing_holding_and_observation_identity_mismatch() {
        let fixture = KrBuyFixture::new();
        let sell_draft = build_draft(
            &fixture.master,
            AccountId::new(1001),
            OrderTerms::KrLimit {
                side: OrderSide::Sell,
                quantity: whole("1"),
                price: KrOrderPrice::try_new(price("70000", Currency::Krw)).expect("지정가"),
            },
            fixture.obs.clone(),
            fixture.now,
        );
        let sellable = SellableQuantityFact::from_lookup_result(
            AccountId::new(1001),
            fixture.master.order_instrument().expect("종목"),
            qty("5"),
            fixture.now,
        )
        .expect("매도가능");
        let facts = OrderGateFacts {
            buying_power: None,
            sellable_quantity: Some(&sellable),
            stock_warnings: None,
            ..fixture.facts()
        };
        assert_eq!(
            sell_draft
                .evaluate_gate(fixture.now, &fixture.obs, &facts)
                .result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Failed,
            "보유하지 않은 종목 매도는 거부"
        );
        let other_id_master = kr_master_with(
            "005930",
            "kr-different-id",
            ListingStatus::Active,
            TradingSuspension::Clear,
            LiquidationTrading::Regular,
        );
        let mismatched_obs = other_id_master
            .price_observation(price("70000", Currency::Krw), fixture.now)
            .expect("다른 ID 관측");
        assert_eq!(
            sell_draft
                .evaluate_gate(fixture.now, &mismatched_obs, &facts)
                .result(ValidationCheck::PriceObservation),
            CheckResult::Failed,
            "같은 심볼이라도 내부 ID가 다르면 거부"
        );
    }
}
