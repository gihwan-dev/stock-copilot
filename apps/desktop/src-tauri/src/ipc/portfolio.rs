use application::{RecordedView, StoredView, SystemPortError};
use domain::{
    Allocation, AllocationDisplayRange, AssetInstrument, Concentration, ConcentrationWarning,
    Currency, DisplayDirection, DisplayPrecision, HoldingReturn, InvestorProfile, Market, Money,
    MoneyChange, PortfolioValuation, Symbol, SyncOutcome, Total, ValuedHolding,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind, IpcSyncError};
use super::state::ScreenState;
use crate::diagnostics_log::DiagnosticsLog;
use crate::holdings_link::InstrumentDirectory;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetPortfolioRequest {
    pub scope: PortfolioScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub enum PortfolioScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiCurrency {
    #[serde(rename = "KRW")]
    Krw,
    #[serde(rename = "USD")]
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiDirection {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "flat")]
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTotalCoverage {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiMoney {
    pub amount: String,
    pub currency: UiCurrency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSignedAmount {
    pub amount: UiMoney,
    pub rate: String,
    pub direction: UiDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTrading {
    #[serde(rename = "trading")]
    Trading,
    #[serde(rename = "suspended")]
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiHolding {
    pub symbol: String,
    pub name: String,
    pub trading: UiTrading,
    pub quantity: String,
    pub value: UiMoney,
    pub profit: UiSignedAmount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiCurrencyValue {
    pub currency: UiCurrency,
    pub holdings: UiMoney,
    pub buying_power: Option<UiMoney>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPortfolioView {
    pub recorded_at: String,
    pub total: UiMoney,
    pub total_coverage: UiTotalCoverage,
    pub exchange_rate_at: Option<String>,
    pub currencies: Vec<UiCurrencyValue>,
    pub holdings: Vec<UiHolding>,
    pub today: Option<UiSignedAmount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiWeightLevel {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "over-cap")]
    OverCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiAllocationLabel {
    #[serde(rename = "holding")]
    Holding,
    #[serde(rename = "cash-krw")]
    CashKrw,
    #[serde(rename = "cash-usd")]
    CashUsd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAllocationRow {
    pub id: String,
    pub kind: UiAllocationLabel,
    pub name: String,
    pub value: UiMoney,
    pub weight: String,
    pub level: UiWeightLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiConcentration {
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "concentrated")]
    Concentrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAllocationView {
    pub instruments: Vec<UiAllocationRow>,
    pub concentration: UiConcentration,
}

const fn ui_currency(currency: Currency) -> UiCurrency {
    match currency {
        Currency::Krw => UiCurrency::Krw,
        Currency::Usd => UiCurrency::Usd,
    }
}

pub(crate) const fn ui_direction(direction: DisplayDirection) -> UiDirection {
    match direction {
        DisplayDirection::Up => UiDirection::Up,
        DisplayDirection::Down => UiDirection::Down,
        DisplayDirection::Flat => UiDirection::Flat,
    }
}

const fn digits_for(currency: Currency) -> u8 {
    match currency {
        Currency::Krw => 0,
        Currency::Usd => 2,
    }
}

pub(crate) fn precision(digits: u8) -> Option<DisplayPrecision> {
    DisplayPrecision::try_new(digits).ok()
}

pub(crate) fn ui_money(money: &Money) -> Option<UiMoney> {
    let shown = domain::display_money(money, precision(digits_for(money.currency()))?);
    Some(UiMoney {
        amount: shown.digits().as_str().to_owned(),
        currency: ui_currency(shown.currency()),
    })
}

pub(crate) fn percent(ratio: &domain::Ratio) -> Option<String> {
    Some(
        domain::display_percent(ratio, precision(2)?)
            .as_str()
            .to_owned(),
    )
}

pub(crate) fn signed(amount: Money, rate: Option<String>) -> Option<UiSignedAmount> {
    let change = domain::display_change(
        &MoneyChange::new(amount),
        precision(digits_for(amount.currency()))?,
    );
    Some(UiSignedAmount {
        amount: UiMoney {
            amount: change.money().digits().as_str().to_owned(),
            currency: ui_currency(change.money().currency()),
        },
        rate: rate?,
        direction: ui_direction(change.direction()),
    })
}

pub(crate) fn return_rate(holding: &ValuedHolding) -> Option<String> {
    let observed = holding.valuation().selected_return().ok()?.value();
    match observed.rate() {
        HoldingReturn::Defined(quotient) => Some(
            domain::display_quotient_percent(&quotient, precision(2)?)
                .0
                .as_str()
                .to_owned(),
        ),
        HoldingReturn::ZeroCost => None,
    }
}

fn holding_row(holding: &ValuedHolding, directory: &InstrumentDirectory) -> Option<UiHolding> {
    let symbol = holding.instrument().symbol();
    let value = holding.valuation().selected().ok()?;
    let profit = holding.valuation().selected_profit().ok()?.value();
    Some(UiHolding {
        symbol: symbol.as_str().to_owned(),
        name: directory.name(symbol).map_or_else(
            || {
                holding.holding().reported_name().map_or_else(
                    || symbol.as_str().to_owned(),
                    |name| name.as_str().to_owned(),
                )
            },
            |name| name.as_str().to_owned(),
        ),
        trading: match directory.entry(symbol).map(|entry| entry.trading) {
            Some(domain::MasterTradingState::Suspended) => UiTrading::Suspended,
            Some(domain::MasterTradingState::Trading) | None => UiTrading::Trading,
        },
        quantity: holding
            .holding()
            .quantity()
            .as_decimal()
            .normalize()
            .to_string(),
        value: ui_money(&value)?,
        profit: signed(
            profit,
            return_rate(holding).or_else(|| Some(String::from("0"))),
        )?,
    })
}

pub(crate) fn display_name(symbol: &Symbol, directory: &InstrumentDirectory) -> String {
    directory.name(symbol).map_or_else(
        || symbol.as_str().to_owned(),
        |name| name.as_str().to_owned(),
    )
}

fn current<T: Clone>(total: &Total<T>) -> (T, UiTotalCoverage) {
    match total {
        Total::Complete(value) => (value.value().clone(), UiTotalCoverage::Complete),
        Total::Incomplete(value) => (value.subtotal().clone(), UiTotalCoverage::Partial),
    }
}

fn currency_row(valuation: &PortfolioValuation, currency: Currency) -> Option<UiCurrencyValue> {
    let (holdings, _coverage) =
        current(valuation.currency_summary(currency).valuation().selected());
    Some(UiCurrencyValue {
        currency: ui_currency(currency),
        holdings: ui_money(&holdings)?,
        buying_power: valuation
            .buying_power(currency)
            .money()
            .ok()
            .and_then(|money| ui_money(&money)),
    })
}

fn today(valuation: &PortfolioValuation) -> Option<UiSignedAmount> {
    let daily = valuation.daily_profit_loss()?;
    let amount = daily.amounts().amount(Currency::Krw)?;
    signed(amount, percent(&daily.rate()))
}

pub(crate) fn view_outcome<T>(view: &RecordedView<T>) -> Result<&T, IpcError> {
    match view.outcome() {
        SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => Ok(value),
        SyncOutcome::Failed => Err(IpcError {
            kind: IpcErrorKind::Retryable,
        }),
        SyncOutcome::AccessBlocked => Err(IpcError {
            kind: IpcErrorKind::UserFixable,
        }),
    }
}

const fn bug() -> IpcError {
    IpcError {
        kind: IpcErrorKind::Bug,
    }
}

#[must_use]
pub fn to_portfolio_view(
    recorded_at: domain::Timestamp,
    valuation: &PortfolioValuation,
    directory: &InstrumentDirectory,
) -> Option<UiPortfolioView> {
    let (assets, total_coverage) = current(valuation.assets());
    let (total, rates, _accuracy) = assets.into_parts();
    let recorded = kst_iso(recorded_at)?;
    Some(UiPortfolioView {
        exchange_rate_at: (!rates.is_empty()).then(|| recorded.clone()),
        recorded_at: recorded,
        total: ui_money(&total)?,
        total_coverage,
        currencies: [Currency::Krw, Currency::Usd]
            .into_iter()
            .filter_map(|currency| currency_row(valuation, currency))
            .collect(),
        holdings: valuation
            .holdings()
            .iter()
            .filter_map(|holding| holding_row(holding, directory))
            .collect(),
        today: today(valuation),
    })
}

pub(crate) fn level(
    weight: &domain::AssetWeight,
    profile: &InvestorProfile,
    kind: domain::FundKind,
) -> UiWeightLevel {
    let limits = profile.weight_caps().limits_for(kind);
    if weight.cmp_profile_weight(&limits.instrument_cap()) == std::cmp::Ordering::Greater {
        UiWeightLevel::OverCap
    } else if weight.cmp_profile_weight(&limits.instrument_warning()) == std::cmp::Ordering::Greater
    {
        UiWeightLevel::Warning
    } else {
        UiWeightLevel::Normal
    }
}

pub(crate) fn weight_text(display: &domain::AllocationDisplay) -> String {
    match display.range() {
        AllocationDisplayRange::Zero => String::from("0"),
        AllocationDisplayRange::BelowUnit => String::from("0.01"),
        AllocationDisplayRange::AtLeastUnit => {
            let units = format!("{:03}", display.units().as_u32());
            units
                .split_at_checked(units.len().saturating_sub(2))
                .map_or_else(
                    || String::from("0"),
                    |(whole, part)| format!("{whole}.{part}"),
                )
        }
    }
}

fn holding_value(valuation: &PortfolioValuation, symbol: &Symbol) -> Option<Money> {
    valuation
        .holdings()
        .iter()
        .find(|holding| holding.instrument().symbol() == symbol)
        .and_then(|holding| holding.valuation().selected().ok())
}

fn allocation_row(
    entry: &domain::AllocationEntry<AssetInstrument, domain::AssetWeight>,
    valuation: &PortfolioValuation,
    directory: &InstrumentDirectory,
    profile: &InvestorProfile,
) -> Option<UiAllocationRow> {
    let (id, kind, name, value) = match entry.key() {
        AssetInstrument::Holding(instrument) => {
            let symbol = instrument.symbol();
            let market = match instrument.market() {
                Market::Kr => "KR",
                Market::Us => "US",
                Market::Unknown(_) => "XX",
            };
            (
                format!("{market}-{}", symbol.as_str()),
                UiAllocationLabel::Holding,
                display_name(symbol, directory),
                holding_value(valuation, symbol)?,
            )
        }
        AssetInstrument::Cash(currency) => {
            let unit = currency.as_currency();
            let kind = match unit {
                Currency::Krw => UiAllocationLabel::CashKrw,
                Currency::Usd => UiAllocationLabel::CashUsd,
            };
            (
                format!("cash-{unit:?}"),
                kind,
                String::new(),
                valuation.buying_power(unit).money().ok()?,
            )
        }
    };
    Some(UiAllocationRow {
        id,
        kind,
        name,
        value: ui_money(&value)?,
        weight: weight_text(entry.display()),
        level: match entry.key() {
            AssetInstrument::Holding(instrument) => level(
                entry.weight(),
                profile,
                directory
                    .entry(instrument.symbol())
                    .map_or(domain::FundKind::SingleSecurity, |found| {
                        found.master.fund_kind()
                    }),
            ),
            AssetInstrument::Cash(_) => UiWeightLevel::Normal,
        },
    })
}

#[must_use]
pub fn to_allocation_view(
    allocation: &Allocation,
    directory: &InstrumentDirectory,
    profile: &InvestorProfile,
) -> UiAllocationView {
    let instruments = match allocation.assets().instruments() {
        Ok(weights) => weights
            .entries()
            .iter()
            .filter(|entry| entry.display().range() != AllocationDisplayRange::Zero)
            .filter_map(|entry| allocation_row(entry, allocation.valuation(), directory, profile))
            .collect(),
        Err(_reason) => Vec::new(),
    };
    let concentration = match Concentration::new(allocation).warning() {
        ConcentrationWarning::Warning => UiConcentration::Concentrated,
        ConcentrationWarning::Clear
        | ConcentrationWarning::NotApplicable
        | ConcentrationWarning::Unavailable => UiConcentration::Balanced,
    };
    UiAllocationView {
        instruments,
        concentration,
    }
}

pub(crate) fn stored<T>(
    loaded: Result<StoredView<T>, SystemPortError>,
) -> Result<RecordedView<T>, IpcError> {
    match loaded.map_err(|error| IpcError::from(application::ClassifiedError::class(&error)))? {
        StoredView::Missing => Err(IpcError {
            kind: IpcErrorKind::NoData,
        }),
        StoredView::Recorded(view) => Ok(view),
    }
}

pub async fn load_portfolio(
    screen: &ScreenState,
    directory: &InstrumentDirectory,
) -> Result<UiPortfolioView, IpcError> {
    let view = stored(screen.backend().load_portfolio_view().await)?;
    let valuation = view_outcome(&view)?;
    to_portfolio_view(view.recorded_at(), valuation, directory).ok_or_else(bug)
}

pub async fn load_allocation(
    screen: &ScreenState,
    directory: &InstrumentDirectory,
) -> Result<UiAllocationView, IpcError> {
    let view = stored(screen.backend().load_allocation_view().await)?;
    let allocation = view_outcome(&view)?;
    let profile = screen
        .backend()
        .load_settings()
        .await
        .ok()
        .and_then(|loaded| loaded.profile)
        .unwrap_or_else(InvestorProfile::default);
    Ok(to_allocation_view(allocation, directory, &profile))
}

pub(crate) fn managed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(&ScreenState, InstrumentDirectory), IpcError> {
    let screen = app.try_state::<ScreenState>().ok_or(IpcError {
        kind: IpcErrorKind::NoData,
    })?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |state| state.inner().clone());
    Ok((screen.inner(), directory))
}

#[tauri::command]
#[specta::specta]
pub async fn get_portfolio<R: Runtime>(
    app: AppHandle<R>,
    request: GetPortfolioRequest,
) -> Result<UiPortfolioView, IpcSyncError> {
    let GetPortfolioRequest {
        scope: PortfolioScope::Current,
    } = request;
    let reason = app
        .try_state::<Option<Arc<DiagnosticsLog>>>()
        .and_then(|state| {
            state
                .inner()
                .as_ref()
                .and_then(|log| log.portfolio_failure())
        });
    let loaded = match managed(&app) {
        Ok((screen, directory)) => load_portfolio(screen, &directory).await,
        Err(error) => Err(error),
    };
    loaded.map_err(|error| IpcSyncError::explained(error, reason))
}

#[tauri::command]
#[specta::specta]
pub async fn get_allocation<R: Runtime>(
    app: AppHandle<R>,
    request: GetPortfolioRequest,
) -> Result<UiAllocationView, IpcError> {
    let GetPortfolioRequest {
        scope: PortfolioScope::Current,
    } = request;
    let (screen, directory) = managed(&app)?;
    load_allocation(screen, &directory).await
}
