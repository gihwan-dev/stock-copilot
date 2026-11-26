use std::sync::Arc;

use application::{
    AllocationAxis, AllocationLabel, AllocationLine, AllocationToolView, BuyingPowerLine,
    ConcentrationLine, GetAllocationView, GetPortfolioView, HoldingToolLine, MarketDataRepository,
    PerformancePeriod, PerformanceSummary, PerformanceToolView, PortfolioRepository,
    PortfolioToolReads, PortfolioToolView, RecordedView, RetryDelay, RiskMetrics,
    SampleSufficiency, StoredView, SystemFailure, SystemPort, SystemPortError,
    TransactionRepository, TransactionToolKind, TransactionToolLine, TransactionsToolView,
};
use domain::{
    AccountId, AllocationCurrency, AllocationDisplay, AllocationDisplayRange, AllocationEntry,
    AllocationSector, AssetCountry, AssetWeight, Currency, DisplayInstrumentName, KnownMarket,
    Market, MaskedAccountNumber, MasterInstrument, NumericInput, PortfolioValuation, Ratio,
    SectorAssignments, Symbol, SyncOutcome, Total, TransactionKind, ValuedHolding,
};

use crate::broker_link::AccountSlot;
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::state::BoxFuture;
use crate::performance_book::PerformanceBook;
use crate::sector_classes::SectorClasses;

#[cfg(test)]
mod tests;

pub struct StoredPortfolioTools<P, S, T = S> {
    portfolios: Arc<P>,
    store: Arc<S>,
    transactions: Arc<T>,
    accounts: AccountSlot,
    directory: InstrumentDirectory,
    sectors: Option<Arc<SectorClasses>>,
    performance: Option<Arc<PerformanceBook>>,
}

impl<P, S, T> std::fmt::Debug for StoredPortfolioTools<P, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoredPortfolioTools(<redacted>)")
    }
}

impl<P, S, T> StoredPortfolioTools<P, S, T> {
    #[must_use]
    pub fn new(
        portfolios: Arc<P>,
        (store, transactions): (Arc<S>, Arc<T>),
        accounts: AccountSlot,
        directory: InstrumentDirectory,
    ) -> Self {
        Self {
            portfolios,
            store,
            transactions,
            accounts,
            directory,
            sectors: None,
            performance: None,
        }
    }

    #[must_use]
    pub fn with_performance(self, performance: Arc<PerformanceBook>) -> Self {
        Self {
            performance: Some(performance),
            ..self
        }
    }

    #[must_use]
    pub fn with_sectors(self, sectors: Arc<SectorClasses>) -> Self {
        Self {
            sectors: Some(sectors),
            ..self
        }
    }
}

const fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

fn recorded<T>(view: StoredView<T>) -> Result<Option<RecordedView<T>>, SystemPortError> {
    match view {
        StoredView::Missing => Ok(None),
        StoredView::Recorded(value) => match value.outcome() {
            SyncOutcome::Complete(_) | SyncOutcome::Partial(_) => Ok(Some(value)),
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => Err(unavailable()),
        },
    }
}

fn outcome_value<T>(view: &RecordedView<T>) -> Option<&T> {
    match view.outcome() {
        SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => Some(value),
        SyncOutcome::Failed | SyncOutcome::AccessBlocked => None,
    }
}

fn units_ratio(display: &AllocationDisplay) -> Option<Ratio> {
    let digits = format!("{:05}", display.units().as_u32());
    let (whole, part) = digits.split_at_checked(digits.len().saturating_sub(4))?;
    let input = NumericInput::try_new(&format!("{whole}.{part}")).ok()?;
    Ratio::parse(&input).ok()
}

const fn span_of(period: PerformancePeriod) -> Option<domain::PerformanceSpan> {
    match period {
        PerformancePeriod::OneMonth => Some(domain::PerformanceSpan::OneMonth),
        PerformancePeriod::ThreeMonths => Some(domain::PerformanceSpan::ThreeMonths),
        PerformancePeriod::SixMonths => Some(domain::PerformanceSpan::SixMonths),
        PerformancePeriod::OneYear => Some(domain::PerformanceSpan::OneYear),
        PerformancePeriod::SinceStart => None,
    }
}

fn value_ratio(value: domain::PerformanceValue) -> Option<Ratio> {
    Ratio::parse(&NumericInput::try_new(&value.to_string()).ok()?).ok()
}

fn metric_ratio(metric: domain::RiskMetric) -> Option<Ratio> {
    match metric {
        domain::RiskMetric::Value(value) => value_ratio(value),
        domain::RiskMetric::InsufficientSamples | domain::RiskMetric::Unavailable(_) => None,
    }
}

fn tool_name(symbol: &Symbol, directory: &InstrumentDirectory) -> Option<DisplayInstrumentName> {
    directory
        .name(symbol)
        .or_else(|| DisplayInstrumentName::try_new(symbol.as_str()).ok())
}

fn instrument_weight(
    weights: &[AllocationEntry<domain::AssetInstrument, AssetWeight>],
    symbol: &Symbol,
) -> Option<Ratio> {
    weights
        .iter()
        .find(|entry| match entry.key() {
            domain::AssetInstrument::Holding(instrument) => instrument.symbol() == symbol,
            domain::AssetInstrument::Cash(_) => false,
        })
        .and_then(|entry| units_ratio(entry.display()))
}

fn holding_line(
    holding: &ValuedHolding,
    weights: &[AllocationEntry<domain::AssetInstrument, AssetWeight>],
    directory: &InstrumentDirectory,
) -> Option<HoldingToolLine> {
    let symbol = holding.instrument().symbol().clone();
    let valuation = holding.valuation();
    Some(HoldingToolLine {
        name: tool_name(&symbol, directory)?,
        quantity: holding.holding().quantity(),
        valuation: valuation.selected().ok(),
        purchase: valuation
            .computed()
            .ok()
            .map(domain::ComputedHolding::purchase),
        profit: valuation
            .selected_profit()
            .ok()
            .map(domain::SelectedValue::value),
        weight: instrument_weight(weights, &symbol),
        symbol,
    })
}

fn complete_total(
    valuation: &PortfolioValuation,
) -> (Option<domain::Money>, Option<domain::FxRate>) {
    match valuation.assets() {
        Total::Complete(total) => {
            let (money, rates, _accuracy) = total.value().clone().into_parts();
            (Some(money), rates.first().copied())
        }
        Total::Incomplete(_) => (None, None),
    }
}

fn label(text: &str) -> Option<AllocationLabel> {
    AllocationLabel::try_new(text).ok()
}

fn axis_lines<K>(
    axis: AllocationAxis,
    entries: &[AllocationEntry<K, AssetWeight>],
    name: impl Fn(&K) -> Option<AllocationLabel>,
) -> Vec<AllocationLine> {
    entries
        .iter()
        .filter(|entry| entry.display().range() != AllocationDisplayRange::Zero)
        .filter_map(|entry| {
            Some(AllocationLine {
                axis,
                label: name(entry.key())?,
                weight: units_ratio(entry.display())?,
            })
        })
        .collect()
}

fn currency_label(currency: AllocationCurrency) -> Option<AllocationLabel> {
    label(match currency.as_currency() {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    })
}

fn country_label(country: &AssetCountry) -> Option<AllocationLabel> {
    label(match country {
        AssetCountry::Market(Market::Kr) => "KR",
        AssetCountry::Market(Market::Us) => "US",
        AssetCountry::Market(Market::Unknown(_)) => "unknown",
        AssetCountry::Cash => "cash",
    })
}

fn sector_label(sector: &AllocationSector) -> Option<AllocationLabel> {
    match sector {
        AllocationSector::Classified(id) => label(id.as_str()),
        AllocationSector::EtfFund => label("etf-fund"),
        AllocationSector::Cash => label("cash"),
        AllocationSector::Unclassified => label("unclassified"),
    }
}

const fn tool_kind(kind: &TransactionKind) -> TransactionToolKind {
    match kind {
        TransactionKind::Buy => TransactionToolKind::Buy,
        TransactionKind::Sell => TransactionToolKind::Sell,
        TransactionKind::Dividend | TransactionKind::Interest => TransactionToolKind::Dividend,
        TransactionKind::Deposit => TransactionToolKind::Deposit,
        TransactionKind::Withdrawal => TransactionToolKind::Withdrawal,
        TransactionKind::Fee => TransactionToolKind::Fee,
        TransactionKind::SecuritiesTransferIn
        | TransactionKind::SecuritiesTransferOut
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Unknown(_) => TransactionToolKind::Other,
    }
}

fn transaction_line(row: &domain::StoredTransaction) -> TransactionToolLine {
    let data = row.transaction().data();
    TransactionToolLine {
        occurred_at: data.transaction_time.unwrap_or_else(|| row.first_seen()),
        kind: tool_kind(&data.kind),
        symbol: data.symbol.clone(),
        quantity: data.quantity,
        amount: data.net_amount.or(data.amount).unwrap_or(data.commission),
    }
}

impl<P, S, T> StoredPortfolioTools<P, S, T>
where
    P: PortfolioRepository,
    S: MarketDataRepository,
    T: TransactionRepository,
{
    fn account(&self) -> Option<(AccountId, MaskedAccountNumber)> {
        let account = self.accounts.current().into_iter().next()?;
        let masked = self.accounts.masked_number(&account)?;
        Some((account, masked))
    }

    async fn read_portfolio(&self) -> Result<Option<PortfolioToolView>, SystemPortError> {
        let Some((account, masked)) = self.account() else {
            return Ok(None);
        };
        let Some(view) = recorded(
            GetAllocationView::new(self.portfolios.as_ref(), self.store.as_ref())
                .execute(&account)
                .await?,
        )?
        else {
            return Ok(None);
        };
        let allocation = outcome_value(&view).ok_or_else(unavailable)?;
        let valuation = allocation.valuation();
        let weights = match allocation.assets().instruments() {
            Ok(value) => value.entries(),
            Err(_reason) => &[],
        };
        let (approximate_total_assets, fx_used) = complete_total(valuation);
        Ok(Some(PortfolioToolView {
            account: masked,
            recorded_at: view.recorded_at(),
            completeness: valuation.assets().completeness(),
            holdings: valuation
                .holdings()
                .iter()
                .filter_map(|holding| holding_line(holding, weights, &self.directory))
                .collect(),
            buying_power: [Currency::Krw, Currency::Usd]
                .into_iter()
                .map(|currency| BuyingPowerLine {
                    currency,
                    amount: valuation.buying_power(currency).money().ok(),
                })
                .collect(),
            approximate_total_assets,
            fx_used,
        }))
    }

    async fn read_allocation(&self) -> Result<Option<AllocationToolView>, SystemPortError> {
        let Some((account, masked)) = self.account() else {
            return Ok(None);
        };
        let plain = GetAllocationView::new(self.portfolios.as_ref(), self.store.as_ref());
        let query = match self.assignments(&account).await {
            Some(assignments) => plain.with_assignments(assignments),
            None => plain,
        };
        let Some(view) = recorded(query.execute(&account).await?)? else {
            return Ok(None);
        };
        let allocation = outcome_value(&view).ok_or_else(unavailable)?;
        let assets = allocation.assets();
        let mut lines = Vec::new();
        if let Ok(weights) = assets.sectors() {
            lines.extend(axis_lines(
                AllocationAxis::Sector,
                weights.entries(),
                sector_label,
            ));
        }
        if let Ok(weights) = assets.countries() {
            lines.extend(axis_lines(
                AllocationAxis::Country,
                weights.entries(),
                country_label,
            ));
        }
        if let Ok(weights) = assets.currencies() {
            lines.extend(axis_lines(
                AllocationAxis::Currency,
                weights.entries(),
                |currency| currency_label(*currency),
            ));
        }
        Ok(Some(AllocationToolView {
            account: masked,
            recorded_at: view.recorded_at(),
            completeness: allocation.valuation().assets().completeness(),
            lines,
            concentration: ConcentrationLine {
                largest_holding_weight: None,
                top_five_weight: None,
                herfindahl_index: None,
            },
        }))
    }

    fn performance_summary(&self, period: PerformancePeriod) -> Option<PerformanceSummary> {
        let span = span_of(period)?;
        self.performance.as_ref()?.summary(span)
    }

    async fn assignments(&self, account: &AccountId) -> Option<SectorAssignments> {
        let sectors = self.sectors.as_ref()?;
        let masters: Vec<MasterInstrument> = match self.portfolios.latest(account).await {
            Ok(Some(record)) => match record.outcome() {
                SyncOutcome::Complete(portfolio) | SyncOutcome::Partial(portfolio) => portfolio
                    .holdings()
                    .iter()
                    .filter_map(|holding| {
                        let market =
                            KnownMarket::try_from_market(&holding.instrument().market()).ok()?;
                        let id = self
                            .directory
                            .instrument_of(market, holding.instrument().symbol())?;
                        self.directory.master(&id)
                    })
                    .collect(),
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => Vec::new(),
            },
            Ok(None) | Err(_) => Vec::new(),
        };
        sectors.assignments(&masters).await
    }

    async fn read_transactions(&self) -> Result<Option<TransactionsToolView>, SystemPortError> {
        let Some((account, masked)) = self.account() else {
            return Ok(None);
        };
        let Some(record) = self.transactions.latest(&account).await? else {
            return Ok(None);
        };
        let rows = match record.outcome() {
            SyncOutcome::Complete(rows) | SyncOutcome::Partial(rows) => rows,
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => return Err(unavailable()),
        };
        Ok(Some(TransactionsToolView {
            account: masked,
            recorded_at: record.recorded_at(),
            lines: rows.active_settled_rows().map(transaction_line).collect(),
        }))
    }

    async fn read_performance(
        &self,
        period: PerformancePeriod,
    ) -> Result<Option<PerformanceToolView>, SystemPortError> {
        let Some((account, masked)) = self.account() else {
            return Ok(None);
        };
        let Some(view) = recorded(
            GetPortfolioView::new(self.portfolios.as_ref(), self.store.as_ref())
                .execute(&account)
                .await?,
        )?
        else {
            return Ok(None);
        };
        let summary = self.performance_summary(period);
        Ok(Some(match summary {
            Some(found) => PerformanceToolView {
                account: masked,
                period,
                computed_at: view.recorded_at(),
                sample: SampleSufficiency::Sufficient,
                time_weighted_return: value_ratio(found.performance.twr()),
                money_weighted_return: found.performance.mwr().ok().and_then(value_ratio),
                benchmarks: Vec::new(),
                risk: RiskMetrics {
                    annualized_volatility: metric_ratio(found.volatility.annual()),
                    max_drawdown: metric_ratio(found.drawdown),
                },
            },
            None => PerformanceToolView {
                account: masked,
                period,
                computed_at: view.recorded_at(),
                sample: SampleSufficiency::Insufficient,
                time_weighted_return: None,
                money_weighted_return: None,
                benchmarks: Vec::new(),
                risk: RiskMetrics {
                    annualized_volatility: None,
                    max_drawdown: None,
                },
            },
        }))
    }
}

type Read<T> = Result<Option<T>, SystemPortError>;

pub trait PortfolioToolSource: Send + Sync {
    fn portfolio(&self) -> BoxFuture<'_, Read<PortfolioToolView>>;
    fn allocation(&self) -> BoxFuture<'_, Read<AllocationToolView>>;
    fn transactions(&self) -> BoxFuture<'_, Read<TransactionsToolView>>;
    fn performance(&self, period: PerformancePeriod) -> BoxFuture<'_, Read<PerformanceToolView>>;
    fn held(&self) -> BoxFuture<'_, Vec<Symbol>>;
}

impl<P, S, T> StoredPortfolioTools<P, S, T>
where
    P: PortfolioRepository,
    S: Send + Sync,
    T: Send + Sync,
{
    async fn held_symbols(&self) -> Vec<Symbol> {
        let Some(account) = self.accounts.current().into_iter().next() else {
            return Vec::new();
        };
        match self.portfolios.latest(&account).await {
            Ok(Some(record)) => match record.outcome() {
                SyncOutcome::Complete(portfolio) | SyncOutcome::Partial(portfolio) => portfolio
                    .holdings()
                    .iter()
                    .map(|holding| holding.instrument().symbol().clone())
                    .collect(),
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => Vec::new(),
            },
            Ok(None) | Err(_) => Vec::new(),
        }
    }
}

impl<P, S, T> PortfolioToolSource for StoredPortfolioTools<P, S, T>
where
    P: PortfolioRepository + 'static,
    S: MarketDataRepository + 'static,
    T: TransactionRepository + 'static,
{
    fn portfolio(&self) -> BoxFuture<'_, Read<PortfolioToolView>> {
        Box::pin(self.read_portfolio())
    }

    fn allocation(&self) -> BoxFuture<'_, Read<AllocationToolView>> {
        Box::pin(self.read_allocation())
    }

    fn transactions(&self) -> BoxFuture<'_, Read<TransactionsToolView>> {
        Box::pin(self.read_transactions())
    }

    fn performance(&self, period: PerformancePeriod) -> BoxFuture<'_, Read<PerformanceToolView>> {
        Box::pin(self.read_performance(period))
    }

    fn held(&self) -> BoxFuture<'_, Vec<Symbol>> {
        Box::pin(self.held_symbols())
    }
}

#[derive(Clone, Default)]
pub struct CatalogPortfolio {
    source: Option<Arc<dyn PortfolioToolSource>>,
    directory: InstrumentDirectory,
}

impl std::fmt::Debug for CatalogPortfolio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CatalogPortfolio")
    }
}

impl CatalogPortfolio {
    #[must_use]
    pub fn connected(source: Arc<dyn PortfolioToolSource>, directory: InstrumentDirectory) -> Self {
        Self {
            source: Some(source),
            directory,
        }
    }

    pub async fn held(&self) -> Vec<Symbol> {
        match self.source.as_ref() {
            Some(source) => source.held().await,
            None => Vec::new(),
        }
    }

    #[must_use]
    pub fn name(&self, symbol: &Symbol) -> Option<DisplayInstrumentName> {
        self.directory.name(symbol)
    }
}

impl PortfolioToolReads for CatalogPortfolio {
    async fn portfolio(&self) -> Read<PortfolioToolView> {
        match self.source.as_ref() {
            Some(source) => source.portfolio().await,
            None => Err(unavailable()),
        }
    }

    async fn allocation(&self) -> Read<AllocationToolView> {
        match self.source.as_ref() {
            Some(source) => source.allocation().await,
            None => Err(unavailable()),
        }
    }

    async fn transactions(&self) -> Read<TransactionsToolView> {
        match self.source.as_ref() {
            Some(source) => source.transactions().await,
            None => Err(unavailable()),
        }
    }

    async fn performance(&self, period: PerformancePeriod) -> Read<PerformanceToolView> {
        match self.source.as_ref() {
            Some(source) => source.performance(period).await,
            None => Err(unavailable()),
        }
    }
}
