use std::sync::Arc;

use application::{
    AllocationAxis, AllocationToolView, Clock, PerformanceBenchmark, PerformancePeriod,
    PerformanceToolView, PortfolioToolReads, PortfolioToolView, SampleSufficiency, ToolName,
    TransactionAdmission, TransactionToolFilter, TransactionToolKind, TransactionToolLine,
    TransactionsToolView,
};
use domain::{Currency, Quantity};
use serde_json::{Map, Value, json};

use crate::catalog::render::{
    completeness, currency_code, fx_rate, money, object, optional_money, optional_ratio, quantity,
    ratio, require, timestamp, untrusted_label,
};
use crate::pagination::{CursorStore, PageRequest, PaginationContext, paginate_items};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorCode, ToolErrorPayload, ToolFuture, ToolInvocation,
    ToolRegistry, ToolRegistryError, ToolSpec, build_object_schema,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortfolioToolKind {
    Portfolio,
    Allocation,
    Transactions,
    Performance,
}

impl PortfolioToolKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Portfolio => "get_portfolio",
            Self::Allocation => "get_allocation",
            Self::Transactions => "get_transactions",
            Self::Performance => "get_performance",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Portfolio => {
                "Holdings, weights, profit and loss, and buying power per currency. The cash slot is buying power because the broker reports no deposit balance. Total assets are approximate. Only the last four account digits are shown."
            }
            Self::Allocation => {
                "Weights by sector, country and currency with concentration indicators computed by the app."
            }
            Self::Transactions => {
                "Stored transaction history (trades, dividends, deposits and withdrawals), newest first, paginated with limit and cursor."
            }
            Self::Performance => {
                "Period returns (time-weighted and money-weighted), benchmark returns and risk metrics computed by the app. Risk metrics are withheld when the sample is insufficient."
            }
        }
    }

    fn input_schema(self) -> Map<String, Value> {
        let mut props = Map::new();
        match self {
            Self::Portfolio | Self::Allocation => {}
            Self::Transactions => {
                props.insert(
                    String::from("kind"),
                    json!({ "type": "string", "enum": ["all", "trades", "dividends", "cash_flows"] }),
                );
                props.insert(
                    String::from("limit"),
                    json!({ "type": "integer", "minimum": 1_i64, "maximum": 200_i64 }),
                );
                props.insert(
                    String::from("cursor"),
                    json!({ "type": "string", "minLength": 32_u64, "maxLength": 32_u64 }),
                );
            }
            Self::Performance => {
                props.insert(
                    String::from("period"),
                    json!({ "type": "string", "enum": ["1m", "3m", "6m", "1y", "since_start"] }),
                );
            }
        }
        build_object_schema(props, &[])
    }
}

struct PortfolioTool<P, C: Clock> {
    kind: PortfolioToolKind,
    spec: ToolSpec,
    reads: Arc<P>,
    cursors: Arc<CursorStore<C>>,
}

impl<P, C: Clock> PortfolioTool<P, C> {
    fn try_new(
        kind: PortfolioToolKind,
        reads: Arc<P>,
        cursors: Arc<CursorStore<C>>,
    ) -> Result<Self, ToolRegistryError> {
        let name =
            ToolName::try_new(kind.name()).map_err(|_err| ToolRegistryError::ForbiddenToolName)?;
        let spec = ToolSpec::try_new(
            name,
            kind.description(),
            ToolAccessKind::ReadOnly,
            kind.input_schema(),
            None,
        )?;
        Ok(Self {
            kind,
            spec,
            reads,
            cursors,
        })
    }
}

impl<P: PortfolioToolReads + 'static, C: Clock + 'static> McpTool for PortfolioTool<P, C> {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move {
            match self.kind {
                PortfolioToolKind::Portfolio => {
                    let view = self.reads.portfolio().await;
                    Ok(render_portfolio(require(view)?))
                }
                PortfolioToolKind::Allocation => {
                    let view = self.reads.allocation().await;
                    Ok(render_allocation(&require(view)?))
                }
                PortfolioToolKind::Transactions => self.transactions(&invocation).await,
                PortfolioToolKind::Performance => {
                    let period = parse_period(invocation.arguments.get("period"))?;
                    let view = require(self.reads.performance(period).await)?;
                    matching_period(&view, period).map(render_performance)
                }
            }
        })
    }
}

impl<P: PortfolioToolReads, C: Clock> PortfolioTool<P, C> {
    async fn transactions(
        &self,
        invocation: &ToolInvocation<'_>,
    ) -> Result<Value, ToolErrorPayload> {
        let (filter, filter_name) = parse_filter(invocation.arguments.get("kind"))?;
        let page = PageRequest::from_raw(
            invocation.arguments.get("limit").and_then(Value::as_u64),
            invocation.arguments.get("cursor").and_then(Value::as_str),
        )?;
        let view = require(self.reads.transactions().await)?;
        let items = transaction_items(&view, filter);
        let context = PaginationContext {
            cursors: &self.cursors,
            run_id: invocation.run_id,
            tool_name: self.spec.name(),
            query_fingerprint: filter_name,
        };
        let mut extra = Map::new();
        extra.insert(
            String::from("account_suffix"),
            Value::String(view.account.to_string()),
        );
        extra.insert(String::from("recorded_at"), timestamp(view.recorded_at));
        extra.insert(String::from("kind"), Value::String(filter_name.to_owned()));
        paginate_items(&items, &page, &context, &extra)
    }
}

fn render_portfolio(view: PortfolioToolView) -> Value {
    let mut holdings = view.holdings;
    holdings.sort_by(|left, right| left.symbol.as_str().cmp(right.symbol.as_str()));
    let holding_values: Vec<Value> = holdings
        .iter()
        .map(|line| {
            object(vec![
                ("symbol", Value::String(line.symbol.as_str().to_owned())),
                ("name", untrusted_label(line.name.as_str())),
                ("quantity", quantity(&line.quantity)),
                ("valuation", optional_money(line.valuation.as_ref())),
                ("purchase", optional_money(line.purchase.as_ref())),
                ("profit", optional_money(line.profit.as_ref())),
                ("weight", optional_ratio(line.weight.as_ref())),
            ])
        })
        .collect();
    let mut buying_power = view.buying_power;
    buying_power.sort_by_key(|line| currency_rank(line.currency));
    let buying_values: Vec<Value> = buying_power
        .iter()
        .map(|line| {
            object(vec![
                (
                    "currency",
                    Value::String(currency_code(line.currency).to_owned()),
                ),
                ("amount", optional_money(line.amount.as_ref())),
            ])
        })
        .collect();
    object(vec![
        ("account_suffix", Value::String(view.account.to_string())),
        ("recorded_at", timestamp(view.recorded_at)),
        (
            "completeness",
            Value::String(completeness(view.completeness).to_owned()),
        ),
        ("holdings", Value::Array(holding_values)),
        ("cash_basis", Value::String(String::from("buying_power"))),
        ("buying_power", Value::Array(buying_values)),
        (
            "total_assets",
            object(vec![
                (
                    "value",
                    optional_money(view.approximate_total_assets.as_ref()),
                ),
                ("accuracy", Value::String(String::from("approximate"))),
            ]),
        ),
        (
            "fx_used",
            view.fx_used.as_ref().map_or(Value::Null, fx_rate),
        ),
    ])
}

const fn currency_rank(currency: Currency) -> u8 {
    match currency {
        Currency::Krw => 0_u8,
        Currency::Usd => 1_u8,
    }
}

const fn axis_name(axis: AllocationAxis) -> &'static str {
    match axis {
        AllocationAxis::Sector => "sector",
        AllocationAxis::Country => "country",
        AllocationAxis::Currency => "currency",
    }
}

fn render_allocation(view: &AllocationToolView) -> Value {
    let mut lines = view.lines.clone();
    lines.sort_by(|left, right| {
        left.axis
            .cmp(&right.axis)
            .then_with(|| right.weight.as_decimal().cmp(&left.weight.as_decimal()))
            .then_with(|| left.label.cmp(&right.label))
    });
    let line_values: Vec<Value> = lines
        .iter()
        .map(|line| {
            object(vec![
                ("axis", Value::String(axis_name(line.axis).to_owned())),
                ("label", untrusted_label(line.label.as_str())),
                ("weight", ratio(&line.weight)),
            ])
        })
        .collect();
    object(vec![
        ("account_suffix", Value::String(view.account.to_string())),
        ("recorded_at", timestamp(view.recorded_at)),
        (
            "completeness",
            Value::String(completeness(view.completeness).to_owned()),
        ),
        ("lines", Value::Array(line_values)),
        (
            "concentration",
            object(vec![
                (
                    "largest_holding_weight",
                    optional_ratio(view.concentration.largest_holding_weight.as_ref()),
                ),
                (
                    "top_five_weight",
                    optional_ratio(view.concentration.top_five_weight.as_ref()),
                ),
                (
                    "herfindahl_index",
                    optional_ratio(view.concentration.herfindahl_index.as_ref()),
                ),
            ]),
        ),
    ])
}

fn parse_filter(
    raw: Option<&Value>,
) -> Result<(TransactionToolFilter, &'static str), ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        None | Some("all") => Ok((TransactionToolFilter::All, "all")),
        Some("trades") => Ok((TransactionToolFilter::Trades, "trades")),
        Some("dividends") => Ok((TransactionToolFilter::Dividends, "dividends")),
        Some("cash_flows") => Ok((TransactionToolFilter::CashFlows, "cash_flows")),
        Some(_other) => Err(ToolErrorPayload::invalid_input(
            Some("kind"),
            "kind must be one of all, trades, dividends, cash_flows",
        )),
    }
}

const fn transaction_kind_name(kind: TransactionToolKind) -> &'static str {
    match kind {
        TransactionToolKind::Buy => "buy",
        TransactionToolKind::Sell => "sell",
        TransactionToolKind::Dividend => "dividend",
        TransactionToolKind::Deposit => "deposit",
        TransactionToolKind::Withdrawal => "withdrawal",
        TransactionToolKind::Fee => "fee",
        TransactionToolKind::Other => "other",
    }
}

fn transaction_items(view: &TransactionsToolView, filter: TransactionToolFilter) -> Vec<Value> {
    let mut lines: Vec<&TransactionToolLine> = view
        .lines
        .iter()
        .filter(|line| filter.admits(line.kind) == TransactionAdmission::Admitted)
        .collect();
    lines.sort_by(|left, right| {
        let left_at = (
            *left.occurred_at.utc_seconds().as_i64(),
            *left.occurred_at.nanosecond().as_u32(),
        );
        let right_at = (
            *right.occurred_at.utc_seconds().as_i64(),
            *right.occurred_at.nanosecond().as_u32(),
        );
        right_at
            .cmp(&left_at)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| {
                let left_symbol = left.symbol.as_ref().map_or("", |symbol| symbol.as_str());
                let right_symbol = right.symbol.as_ref().map_or("", |symbol| symbol.as_str());
                left_symbol.cmp(right_symbol)
            })
            .then_with(|| {
                left.amount
                    .amount()
                    .as_decimal()
                    .cmp(&right.amount.amount().as_decimal())
            })
            .then_with(|| {
                let left_qty = left.quantity.as_ref().map(Quantity::as_decimal);
                let right_qty = right.quantity.as_ref().map(Quantity::as_decimal);
                left_qty.cmp(&right_qty)
            })
    });
    lines
        .into_iter()
        .map(|line| {
            object(vec![
                ("occurred_at", timestamp(line.occurred_at)),
                (
                    "kind",
                    Value::String(transaction_kind_name(line.kind).to_owned()),
                ),
                (
                    "symbol",
                    line.symbol.as_ref().map_or(Value::Null, |symbol| {
                        Value::String(symbol.as_str().to_owned())
                    }),
                ),
                (
                    "quantity",
                    line.quantity.as_ref().map_or(Value::Null, quantity),
                ),
                ("amount", money(&line.amount)),
            ])
        })
        .collect()
}

fn parse_period(raw: Option<&Value>) -> Result<PerformancePeriod, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        Some("1m") => Ok(PerformancePeriod::OneMonth),
        Some("3m") => Ok(PerformancePeriod::ThreeMonths),
        Some("6m") => Ok(PerformancePeriod::SixMonths),
        None | Some("1y") => Ok(PerformancePeriod::OneYear),
        Some("since_start") => Ok(PerformancePeriod::SinceStart),
        Some(_other) => Err(ToolErrorPayload::invalid_input(
            Some("period"),
            "period must be one of 1m, 3m, 6m, 1y, since_start",
        )),
    }
}

fn matching_period(
    view: &PerformanceToolView,
    requested: PerformancePeriod,
) -> Result<&PerformanceToolView, ToolErrorPayload> {
    if view.period == requested {
        Ok(view)
    } else {
        Err(ToolErrorPayload::retryable(
            ToolErrorCode::UpstreamUnavailable,
            Some("period"),
            "stored performance does not match the requested period",
        ))
    }
}

const fn period_name(period: PerformancePeriod) -> &'static str {
    match period {
        PerformancePeriod::OneMonth => "1m",
        PerformancePeriod::ThreeMonths => "3m",
        PerformancePeriod::SixMonths => "6m",
        PerformancePeriod::OneYear => "1y",
        PerformancePeriod::SinceStart => "since_start",
    }
}

const fn benchmark_name(benchmark: PerformanceBenchmark) -> &'static str {
    match benchmark {
        PerformanceBenchmark::Kospi => "KOSPI",
        PerformanceBenchmark::Kosdaq => "KOSDAQ",
        PerformanceBenchmark::Sp500 => "S&P 500",
        PerformanceBenchmark::Nasdaq100 => "NASDAQ-100",
    }
}

fn render_performance(view: &PerformanceToolView) -> Value {
    let mut benchmarks = view.benchmarks.clone();
    benchmarks.sort_by_key(|line| line.benchmark);
    let benchmark_values: Vec<Value> = benchmarks
        .iter()
        .map(|line| {
            object(vec![
                (
                    "benchmark",
                    Value::String(benchmark_name(line.benchmark).to_owned()),
                ),
                ("period_return", ratio(&line.period_return)),
            ])
        })
        .collect();
    let (sample, risk) = match view.sample {
        SampleSufficiency::Sufficient => (
            "sufficient",
            object(vec![
                (
                    "annualized_volatility",
                    optional_ratio(view.risk.annualized_volatility.as_ref()),
                ),
                (
                    "max_drawdown",
                    optional_ratio(view.risk.max_drawdown.as_ref()),
                ),
            ]),
        ),
        SampleSufficiency::Insufficient => (
            "insufficient",
            object(vec![
                ("annualized_volatility", Value::Null),
                ("max_drawdown", Value::Null),
            ]),
        ),
    };
    object(vec![
        ("account_suffix", Value::String(view.account.to_string())),
        ("period", Value::String(period_name(view.period).to_owned())),
        ("computed_at", timestamp(view.computed_at)),
        ("sample", Value::String(sample.to_owned())),
        (
            "time_weighted_return",
            optional_ratio(view.time_weighted_return.as_ref()),
        ),
        (
            "money_weighted_return",
            optional_ratio(view.money_weighted_return.as_ref()),
        ),
        ("benchmarks", Value::Array(benchmark_values)),
        ("risk", risk),
    ])
}

pub fn register_portfolio_tools<P, C>(
    registry: &mut ToolRegistry,
    reads: &Arc<P>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<(), ToolRegistryError>
where
    P: PortfolioToolReads + 'static,
    C: Clock + 'static,
{
    for kind in [
        PortfolioToolKind::Portfolio,
        PortfolioToolKind::Allocation,
        PortfolioToolKind::Transactions,
        PortfolioToolKind::Performance,
    ] {
        let tool = PortfolioTool::try_new(kind, Arc::clone(reads), Arc::clone(cursors))?;
        registry.register(Arc::new(tool))?;
    }
    Ok(())
}
