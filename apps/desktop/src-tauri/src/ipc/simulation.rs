use domain::{
    Allocation, AllocationDisplayRange, AllocationInstrument, AssetInstrument, Currency, FundKind,
    InvestorProfile, KnownMarket, Market, NumericInput, PortfolioValuation, Price, Quantity,
    ResolvedOrderInstrument, SectorAssignments, SimulatedTrade, Symbol, TradeSide,
    TradeSimulationError, WholeOrderQuantity,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::IpcError;
use super::portfolio::{
    UiMoney, UiWeightLevel, display_name, level, managed, percent, stored, ui_money, view_outcome,
    weight_text,
};
use super::system_data::{
    SystemDataIpcState, UiCashMinPercent, UiConcentrationCount, UiPrinciples, load_principles,
};
use crate::holdings_link::InstrumentDirectory;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SimulateTradesRequest {
    pub trades: Vec<UiTradeInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiTradeInput {
    pub symbol: String,
    pub market: UiTradeMarket,
    pub side: UiTradeSide,
    pub quantity: String,
    #[serde(default)]
    pub price: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum UiTradeMarket {
    #[serde(rename = "KR")]
    Kr,
    #[serde(rename = "US")]
    Us,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiTradeSide {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiSimulationOutcome {
    #[serde(rename = "simulated")]
    Simulated,
    #[serde(rename = "invalid-trade")]
    InvalidTrade,
    #[serde(rename = "missing-price")]
    MissingPrice,
    #[serde(rename = "sell-exceeds-holding")]
    SellExceedsHolding,
    #[serde(rename = "insufficient-cash")]
    InsufficientCash,
    #[serde(rename = "cash-unknown")]
    CashUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiFundKind {
    #[serde(rename = "single-security")]
    SingleSecurity,
    #[serde(rename = "broad-index-fund")]
    BroadIndexFund,
    #[serde(rename = "leveraged-or-inverse")]
    LeveragedOrInverse,
    #[serde(rename = "sector-or-theme-fund")]
    SectorOrThemeFund,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSimulatedFill {
    pub symbol: String,
    pub side: UiTradeSide,
    pub quantity: String,
    pub price: UiMoney,
    pub amount: UiMoney,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSimulatedInstrument {
    pub symbol: String,
    pub name: String,
    pub fund_kind: UiFundKind,
    pub weight_before: String,
    pub weight_after: String,
    pub limit_percent: String,
    pub warning_percent: String,
    pub level_after: UiWeightLevel,
    pub room_to_limit: Option<UiMoney>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiCashAfter {
    pub currency: String,
    pub amount: UiMoney,
    pub weight: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiPrincipleStatus {
    #[serde(rename = "met")]
    Met,
    #[serde(rename = "below")]
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPrincipleCheck {
    pub holdings_after: String,
    pub minimum_holdings: String,
    pub holdings: UiPrincipleStatus,
    pub cash_percent_after: String,
    pub minimum_cash_percent: String,
    pub cash: UiPrincipleStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiLimitSource {
    #[serde(rename = "user-setting")]
    UserSetting,
    #[serde(rename = "app-default")]
    AppDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiValuationFx {
    pub pair: String,
    pub rate: UiMoney,
    pub basis: String,
    pub as_of: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiIssueKind {
    #[serde(rename = "over-limit")]
    OverLimit,
    #[serde(rename = "holdings-below-minimum")]
    HoldingsBelowMinimum,
    #[serde(rename = "cash-below-floor")]
    CashBelowFloor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSimulationIssue {
    pub kind: UiIssueKind,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiTradeSimulation {
    pub issues: Vec<UiSimulationIssue>,
    pub limits_source: UiLimitSource,
    pub valuation_fx: Option<UiValuationFx>,
    pub outcome: UiSimulationOutcome,
    pub problem_symbol: Option<String>,
    pub fills: Vec<UiSimulatedFill>,
    pub total_after: Option<UiMoney>,
    pub cash_after: Vec<UiCashAfter>,
    pub instruments: Vec<UiSimulatedInstrument>,
    pub principles: Option<UiPrincipleCheck>,
}

pub(crate) const fn ui_kind(kind: FundKind) -> UiFundKind {
    match kind {
        FundKind::SingleSecurity => UiFundKind::SingleSecurity,
        FundKind::BroadIndexFund => UiFundKind::BroadIndexFund,
        FundKind::LeveragedOrInverse => UiFundKind::LeveragedOrInverse,
        FundKind::SectorOrThemeFund => UiFundKind::SectorOrThemeFund,
    }
}

const fn currency_of(market: UiTradeMarket) -> (Market, KnownMarket, Currency) {
    match market {
        UiTradeMarket::Kr => (Market::Kr, KnownMarket::Kr, Currency::Krw),
        UiTradeMarket::Us => (Market::Us, KnownMarket::Us, Currency::Usd),
    }
}

fn resolve(
    symbol: &Symbol,
    market: &(Market, KnownMarket),
    valuation: &PortfolioValuation,
    directory: &InstrumentDirectory,
) -> Option<ResolvedOrderInstrument> {
    if let Some(held) = valuation
        .holdings()
        .iter()
        .map(domain::ValuedHolding::instrument)
        .find(|held| held.symbol() == symbol && held.market() == market.0)
    {
        return Some(held.clone());
    }
    let id = directory.instrument_of(market.1, symbol)?;
    directory.master(&id)?.order_instrument().ok()
}

fn parse_trade(
    input: &UiTradeInput,
    valuation: &PortfolioValuation,
    directory: &InstrumentDirectory,
) -> Option<SimulatedTrade> {
    let (market, known, currency) = currency_of(input.market);
    let symbol = Symbol::try_new(&input.symbol.to_uppercase()).ok()?;
    let instrument = resolve(&symbol, &(market, known), valuation, directory)?;
    let quantity = NumericInput::try_new(&input.quantity)
        .ok()
        .and_then(|text| Quantity::parse(&text).ok())
        .and_then(|value| WholeOrderQuantity::try_new(value).ok())?;
    let price = match &input.price {
        None => None,
        Some(text) => Some(
            NumericInput::try_new(text)
                .ok()
                .and_then(|value| Price::parse(&value, currency).ok())?,
        ),
    };
    let side = match input.side {
        UiTradeSide::Buy => TradeSide::Buy,
        UiTradeSide::Sell => TradeSide::Sell,
    };
    Some(SimulatedTrade::new(instrument, side, quantity, price))
}

fn failure(outcome: UiSimulationOutcome, symbol: Option<&Symbol>) -> UiTradeSimulation {
    UiTradeSimulation {
        issues: Vec::new(),
        limits_source: UiLimitSource::AppDefault,
        valuation_fx: None,
        outcome,
        problem_symbol: symbol.map(|found| found.as_str().to_owned()),
        fills: Vec::new(),
        total_after: None,
        cash_after: Vec::new(),
        instruments: Vec::new(),
        principles: None,
    }
}

fn failure_of(error: &TradeSimulationError) -> UiTradeSimulation {
    match error {
        TradeSimulationError::MissingPrice(found) => {
            failure(UiSimulationOutcome::MissingPrice, Some(found.symbol()))
        }
        TradeSimulationError::PriceCurrencyMismatch(found) => {
            failure(UiSimulationOutcome::InvalidTrade, Some(found.symbol()))
        }
        TradeSimulationError::SellExceedsHolding(found) => failure(
            UiSimulationOutcome::SellExceedsHolding,
            Some(found.symbol()),
        ),
        TradeSimulationError::InsufficientCash(_) => {
            failure(UiSimulationOutcome::InsufficientCash, None)
        }
        TradeSimulationError::CashUnavailable(_) => failure(UiSimulationOutcome::CashUnknown, None),
        TradeSimulationError::Arithmetic
        | TradeSimulationError::InvalidPortfolio
        | TradeSimulationError::TooManyTrades => failure(UiSimulationOutcome::InvalidTrade, None),
    }
}

fn kind_of(instrument: &AllocationInstrument, directory: &InstrumentDirectory) -> FundKind {
    directory
        .entry(instrument.symbol())
        .map_or(FundKind::SingleSecurity, |found| found.master.fund_kind())
}

fn entries_of(
    allocation: &Allocation,
) -> &[domain::AllocationEntry<AssetInstrument, domain::AssetWeight>] {
    match allocation.assets().instruments() {
        Ok(weights) => weights.entries(),
        Err(_unavailable) => &[],
    }
}

fn weight_of(allocation: &Allocation, symbol: &Symbol) -> String {
    entries_of(allocation)
        .iter()
        .find_map(|entry| match entry.key() {
            AssetInstrument::Holding(held) if held.symbol() == symbol => {
                Some(weight_text(entry.display()))
            }
            AssetInstrument::Holding(_) | AssetInstrument::Cash(_) => None,
        })
        .unwrap_or_else(|| String::from("0"))
}

struct ViewInputs<'a> {
    before: &'a Allocation,
    after: &'a Allocation,
    valuation: &'a PortfolioValuation,
    exposures: Option<&'a [(AllocationInstrument, domain::Money)]>,
    directory: &'a InstrumentDirectory,
    profile: &'a InvestorProfile,
}

fn instrument_row(
    inputs: &ViewInputs<'_>,
    held: &AllocationInstrument,
    entry: &domain::AllocationEntry<AssetInstrument, domain::AssetWeight>,
) -> Option<UiSimulatedInstrument> {
    let kind = kind_of(held, inputs.directory);
    let limits = inputs.profile.weight_caps().limits_for(kind);
    let room = inputs
        .exposures
        .and_then(|values| values.iter().find(|(instrument, _)| instrument == held))
        .and_then(|(_, value)| {
            domain::room_to_cap(inputs.valuation, *value, limits.instrument_cap()).ok()
        })
        .and_then(|money| ui_money(&money));
    Some(UiSimulatedInstrument {
        symbol: held.symbol().as_str().to_owned(),
        name: display_name(held.symbol(), inputs.directory),
        fund_kind: ui_kind(kind),
        weight_before: weight_of(inputs.before, held.symbol()),
        weight_after: weight_text(entry.display()),
        limit_percent: percent(&limits.instrument_cap().as_ratio())?,
        warning_percent: percent(&limits.instrument_warning().as_ratio())?,
        level_after: level(entry.weight(), inputs.profile, kind),
        room_to_limit: room,
    })
}

fn instrument_rows(inputs: &ViewInputs<'_>) -> Vec<UiSimulatedInstrument> {
    entries_of(inputs.after)
        .iter()
        .filter(|entry| entry.display().range() != AllocationDisplayRange::Zero)
        .filter_map(|entry| match entry.key() {
            AssetInstrument::Holding(held) => instrument_row(inputs, held, entry),
            AssetInstrument::Cash(_) => None,
        })
        .collect()
}

fn cash_rows(after: &Allocation, valuation: &PortfolioValuation) -> Vec<UiCashAfter> {
    entries_of(after)
        .iter()
        .filter(|entry| entry.display().range() != AllocationDisplayRange::Zero)
        .filter_map(|entry| match entry.key() {
            AssetInstrument::Cash(currency) => {
                let unit = currency.as_currency();
                let amount = valuation.buying_power(unit).money().ok()?;
                Some(UiCashAfter {
                    currency: String::from(match unit {
                        Currency::Krw => "KRW",
                        Currency::Usd => "USD",
                    }),
                    amount: ui_money(&amount)?,
                    weight: weight_text(entry.display()),
                })
            }
            AssetInstrument::Holding(_) => None,
        })
        .collect()
}

fn fill_rows(fills: &[domain::SimulatedFill]) -> Vec<UiSimulatedFill> {
    fills
        .iter()
        .filter_map(|fill| {
            Some(UiSimulatedFill {
                symbol: fill.instrument().symbol().as_str().to_owned(),
                side: match fill.side() {
                    TradeSide::Buy => UiTradeSide::Buy,
                    TradeSide::Sell => UiTradeSide::Sell,
                },
                quantity: fill.quantity().quantity().as_decimal().to_string(),
                price: ui_money(&fill.price().as_money())?,
                amount: ui_money(&fill.amount())?,
            })
        })
        .collect()
}

const fn minimum_holdings(count: UiConcentrationCount) -> usize {
    match count {
        UiConcentrationCount::Three => 3_usize,
        UiConcentrationCount::Four => 4_usize,
        UiConcentrationCount::Five => 5_usize,
        UiConcentrationCount::Six => 6_usize,
        UiConcentrationCount::Eight => 8_usize,
        UiConcentrationCount::Ten => 10_usize,
    }
}

const fn minimum_cash_units(percent: UiCashMinPercent) -> u32 {
    match percent {
        UiCashMinPercent::Zero => 0_u32,
        UiCashMinPercent::Five => 500_u32,
        UiCashMinPercent::Ten => 1_000_u32,
        UiCashMinPercent::Twenty => 2_000_u32,
        UiCashMinPercent::Thirty => 3_000_u32,
    }
}

const fn status(met: std::cmp::Ordering) -> UiPrincipleStatus {
    match met {
        std::cmp::Ordering::Less => UiPrincipleStatus::Below,
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => UiPrincipleStatus::Met,
    }
}

fn units_text(units: u32) -> String {
    let whole = units.checked_div(100_u32).unwrap_or(0_u32);
    let part = units.checked_rem(100_u32).unwrap_or(0_u32);
    format!("{whole}.{part:02}")
}

fn principle_check(after: &Allocation, principles: &UiPrinciples) -> UiPrincipleCheck {
    let live = entries_of(after)
        .iter()
        .filter(|entry| entry.display().range() != AllocationDisplayRange::Zero);
    let holdings = live
        .clone()
        .filter(|entry| matches_holding(entry.key()) == UiPrincipleStatus::Met)
        .count();
    let cash_units = live
        .filter(|entry| matches_holding(entry.key()) == UiPrincipleStatus::Below)
        .map(|entry| entry.display().units().as_u32())
        .fold(0_u32, u32::saturating_add);
    let minimum = minimum_holdings(principles.concentration_min_count);
    let floor = minimum_cash_units(principles.cash_min_percent);
    UiPrincipleCheck {
        holdings_after: holdings.to_string(),
        minimum_holdings: minimum.to_string(),
        holdings: status(holdings.cmp(&minimum)),
        cash_percent_after: units_text(cash_units),
        minimum_cash_percent: units_text(floor),
        cash: status(cash_units.cmp(&floor)),
    }
}

const fn matches_holding(key: &AssetInstrument) -> UiPrincipleStatus {
    match key {
        AssetInstrument::Holding(_) => UiPrincipleStatus::Met,
        AssetInstrument::Cash(_) => UiPrincipleStatus::Below,
    }
}

fn issues_of(
    instruments: &[UiSimulatedInstrument],
    principles: Option<&UiPrincipleCheck>,
) -> Vec<UiSimulationIssue> {
    let over = instruments
        .iter()
        .filter(|row| row.level_after == UiWeightLevel::OverCap)
        .map(|row| UiSimulationIssue {
            kind: UiIssueKind::OverLimit,
            symbol: Some(row.symbol.clone()),
        });
    let principle = principles.into_iter().flat_map(|check| {
        [
            (check.holdings, UiIssueKind::HoldingsBelowMinimum),
            (check.cash, UiIssueKind::CashBelowFloor),
        ]
        .into_iter()
        .filter(|(state, _)| *state == UiPrincipleStatus::Below)
        .map(|(_, kind)| UiSimulationIssue { kind, symbol: None })
    });
    over.chain(principle).collect()
}

fn valuation_fx(before: &Allocation) -> Option<UiValuationFx> {
    let rate = domain::valuation_fx_rate(before.valuation())?;
    Some(UiValuationFx {
        pair: String::from("USD/KRW"),
        rate: ui_money(&rate.quote_per_base().as_money())?,
        basis: String::from("portfolio-valuation"),
        as_of: super::briefing::kst_iso(rate.as_of())?,
    })
}

pub(crate) fn simulated_view(
    before: &Allocation,
    trades: &[SimulatedTrade],
    directory: &InstrumentDirectory,
    (profile, principles): (&InvestorProfile, Option<&UiPrinciples>),
) -> UiTradeSimulation {
    let result = match domain::simulate_trades(before.valuation(), trades) {
        Ok(result) => result,
        Err(error) => return failure_of(&error),
    };
    let after = Allocation::new(result.valuation(), &SectorAssignments::default());
    let exposures = domain::krw_exposures(result.valuation()).ok();
    let inputs = ViewInputs {
        before,
        after: &after,
        valuation: result.valuation(),
        exposures: exposures.as_deref(),
        directory,
        profile,
    };
    let total_after = match result.valuation().assets() {
        domain::Total::Complete(value) => value.value().clone(),
        domain::Total::Incomplete(value) => value.subtotal().clone(),
    }
    .into_parts()
    .0;
    let instruments = instrument_rows(&inputs);
    let checked = principles.map(|found| principle_check(&after, found));
    UiTradeSimulation {
        issues: issues_of(&instruments, checked.as_ref()),
        limits_source: UiLimitSource::AppDefault,
        valuation_fx: valuation_fx(before),
        outcome: UiSimulationOutcome::Simulated,
        problem_symbol: None,
        fills: fill_rows(result.fills()),
        total_after: ui_money(&total_after),
        cash_after: cash_rows(&after, result.valuation()),
        instruments,
        principles: checked,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn simulate_trades<R: Runtime>(
    app: AppHandle<R>,
    request: SimulateTradesRequest,
) -> Result<UiTradeSimulation, IpcError> {
    let (screen, directory) = managed(&app)?;
    let view = stored(screen.backend().load_allocation_view().await)?;
    let allocation = view_outcome(&view)?;
    let Some(trades) = request
        .trades
        .iter()
        .map(|trade| parse_trade(trade, allocation.valuation(), &directory))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(failure(UiSimulationOutcome::InvalidTrade, None));
    };
    let saved = screen
        .backend()
        .load_settings()
        .await
        .ok()
        .and_then(|loaded| loaded.profile);
    let limits_source = match saved {
        Some(_) => UiLimitSource::UserSetting,
        None => UiLimitSource::AppDefault,
    };
    let profile = saved.unwrap_or_else(InvestorProfile::default);
    let principles = match app.try_state::<SystemDataIpcState>() {
        Some(state) => load_principles(state.inner()).await.ok(),
        None => None,
    };
    let simulated = simulated_view(
        allocation,
        &trades,
        &directory,
        (&profile, principles.as_ref()),
    );
    Ok(UiTradeSimulation {
        limits_source,
        ..simulated
    })
}
