use std::sync::Arc;

use application::{
    CandlePriceBasis, CandleToolLine, CandleWindow, Clock, MarketOverviewToolView, MarketToolReads,
    OverviewItem, OverviewUnit, RankingToolKind, RankingsToolView, SectorsToolView, SessionStatus,
    ToolName, ValueProvenance,
};
use domain::{KnownMarket, Symbol};
use serde_json::{Map, Value, json};

use crate::catalog::render::{
    money, object, optional_money, optional_ratio, quantity, ratio, require_market, timestamp,
    untrusted_label,
};
use crate::pagination::{CursorStore, PageRequest, PaginationContext, paginate_items};
use crate::tools::{
    McpTool, ToolAccessKind, ToolErrorPayload, ToolFuture, ToolInvocation, ToolRegistry,
    ToolRegistryError, ToolSpec, build_object_schema,
};

const DEFAULT_CANDLE_DAYS: u16 = 60_u16;
const DEFAULT_LIST_LIMIT: usize = 50_usize;
const MAX_LIST_LIMIT: u64 = 100_u64;
const MAX_FLOW_DAYS: usize = 60_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketToolKind {
    Quote,
    Candles,
    Overview,
    Rankings,
    Sectors,
    InvestorFlow,
}

impl MarketToolKind {
    const ALL: [Self; 6] = [
        Self::Quote,
        Self::Candles,
        Self::Overview,
        Self::Rankings,
        Self::Sectors,
        Self::InvestorFlow,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Quote => "get_quote",
            Self::Candles => "get_candles",
            Self::Overview => "get_market_overview",
            Self::Rankings => "get_rankings",
            Self::Sectors => "get_sectors",
            Self::InvestorFlow => "get_investor_flow",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Quote => {
                "Latest brokerage price for one listed symbol with the time it was collected."
            }
            Self::Candles => {
                "Recent finalized daily candles for one symbol from the brokerage, newest first, paginated with limit and cursor. Prices are adjusted for splits and other corporate actions when adjusted is true and are raw brokerage prices when it is false. Includes the collection time."
            }
            Self::Overview => {
                "KOSPI, KOSDAQ, Korean 3Y and 10Y government bond yields, USD/KRW and KR/US market session status, plus S&P 500, Nasdaq 100, gold, crude oil and the dollar approximated by US ETF prices in USD. Each value states whether it is official or an ETF-based approximation and when it was collected. Values that could not be read are left out. US Treasury yields have no source and are listed under not_provided instead of a value."
            }
            Self::Rankings => {
                "Today's brokerage rankings by trading amount or price change for KR or US, or domestic foreign or institution net buying in KRW, with the collection time."
            }
            Self::Sectors => {
                "Today's brokerage sector ranking by price change with trading amount, with the collection time."
            }
            Self::InvestorFlow => {
                "Daily foreign, institutional and individual net buying for one domestic symbol over recent trading days, in KRW estimated as net shares times that day's close (null when the close is unknown), with the collection time."
            }
        }
    }

    fn input_schema(self) -> Map<String, Value> {
        let mut props = Map::new();
        let symbol = json!({ "type": "string", "minLength": 1_u64, "maxLength": 12_u64 });
        let market = json!({ "type": "string", "enum": ["KR", "US"] });
        let required: &[&str] = match self {
            Self::Quote => {
                props.insert(String::from("symbol"), symbol);
                props.insert(String::from("market"), market);
                &["symbol"]
            }
            Self::Candles => {
                props.insert(String::from("symbol"), symbol);
                props.insert(String::from("market"), market);
                props.insert(
                    String::from("days"),
                    json!({ "type": "integer", "minimum": 1_i64, "maximum": 250_i64 }),
                );
                props.insert(
                    String::from("limit"),
                    json!({ "type": "integer", "minimum": 1_i64, "maximum": 200_i64 }),
                );
                props.insert(
                    String::from("cursor"),
                    json!({ "type": "string", "minLength": 32_u64, "maxLength": 32_u64 }),
                );
                &["symbol"]
            }
            Self::Overview => &[],
            Self::Rankings => {
                props.insert(
                    String::from("kind"),
                    json!({
                        "type": "string",
                        "enum": ["trading_amount", "gainers", "losers", "foreign_net_buy", "institution_net_buy"]
                    }),
                );
                props.insert(String::from("market"), market);
                props.insert(String::from("limit"), list_limit_schema());
                &["kind"]
            }
            Self::Sectors => {
                props.insert(String::from("market"), market);
                props.insert(String::from("limit"), list_limit_schema());
                &[]
            }
            Self::InvestorFlow => {
                props.insert(String::from("symbol"), symbol);
                props.insert(
                    String::from("market"),
                    json!({ "type": "string", "enum": ["KR"] }),
                );
                &["symbol"]
            }
        };
        build_object_schema(props, required)
    }
}

fn list_limit_schema() -> Value {
    json!({ "type": "integer", "minimum": 1_i64, "maximum": 100_i64 })
}

fn parse_list_limit(raw: Option<&Value>) -> Result<usize, ToolErrorPayload> {
    match raw.and_then(Value::as_u64) {
        None => Ok(DEFAULT_LIST_LIMIT),
        Some(value) if (1_u64..=MAX_LIST_LIMIT).contains(&value) => {
            usize::try_from(value).map_err(|_overflow| {
                ToolErrorPayload::invalid_input(Some("limit"), "limit must be between 1 and 100")
            })
        }
        Some(_out_of_range) => Err(ToolErrorPayload::invalid_input(
            Some("limit"),
            "limit must be between 1 and 100",
        )),
    }
}

struct MarketTool<M, C: Clock> {
    kind: MarketToolKind,
    spec: ToolSpec,
    reads: Arc<M>,
    cursors: Arc<CursorStore<C>>,
}

impl<M: MarketToolReads + 'static, C: Clock + 'static> McpTool for MarketTool<M, C> {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
        Box::pin(async move {
            let args = invocation.arguments;
            match self.kind {
                MarketToolKind::Quote => {
                    let symbol = parse_symbol(args.get("symbol"))?;
                    let market = parse_market(args.get("market"))?;
                    let view = require_market(self.reads.quote(&symbol, market).await)?;
                    Ok(object(vec![
                        ("symbol", Value::String(view.symbol.as_str().to_owned())),
                        ("market", Value::String(market_code(view.market).to_owned())),
                        ("price", money(&view.price)),
                        ("change_rate", optional_ratio(view.change_rate.as_ref())),
                        ("collected_at", timestamp(view.collected_at)),
                    ]))
                }
                MarketToolKind::Candles => self.candles(&invocation).await,
                MarketToolKind::Overview => self.overview().await,
                MarketToolKind::Rankings => {
                    let kind = parse_ranking_kind(args.get("kind"))?;
                    let market = parse_market(args.get("market"))?;
                    let limit = parse_list_limit(args.get("limit"))?;
                    let view = require_market(self.reads.rankings(kind, market).await)?;
                    Ok(render_rankings(&view, limit))
                }
                MarketToolKind::Sectors => {
                    let market = parse_market(args.get("market"))?;
                    let limit = parse_list_limit(args.get("limit"))?;
                    let view = require_market(self.reads.sectors(market).await)?;
                    Ok(render_sectors(&view, limit))
                }
                MarketToolKind::InvestorFlow => {
                    let symbol = parse_symbol(args.get("symbol"))?;
                    let market = parse_domestic_market(args.get("market"))?;
                    let view = require_market(self.reads.investor_flow(&symbol, market).await)?;
                    Ok(render_investor_flow(&view))
                }
            }
        })
    }
}

impl<M: MarketToolReads, C: Clock> MarketTool<M, C> {
    async fn overview(&self) -> Result<Value, ToolErrorPayload> {
        let primary = require_market(self.reads.market_overview().await);
        if primary.as_ref().is_ok_and(|view| {
            view.lines.iter().any(|line| {
                matches!(line.item, OverviewItem::Kospi | OverviewItem::Kosdaq)
                    && line.change_rate.is_some()
            })
        }) {
            return primary.map(|view| render_overview(&view));
        }
        let sectors = require_market(self.reads.sectors(KnownMarket::Kr).await);
        let mut output = match primary {
            Ok(view) => render_overview(&view),
            Err(error) => {
                if !sectors.as_ref().is_ok_and(|view| !view.lines.is_empty()) {
                    return Err(error);
                }
                let mut empty = render_overview(&MarketOverviewToolView {
                    lines: Vec::new(),
                    sessions: Vec::new(),
                });
                empty["overview_error"] = error.to_json_value();
                empty
            }
        };
        output["kr_sector_ranking"] = match sectors {
            Ok(view) => render_sectors(&view, DEFAULT_LIST_LIMIT),
            Err(error) => error.to_json_value(),
        };
        Ok(output)
    }

    async fn candles(&self, invocation: &ToolInvocation<'_>) -> Result<Value, ToolErrorPayload> {
        let args = invocation.arguments;
        let symbol = parse_symbol(args.get("symbol"))?;
        let market = parse_market(args.get("market"))?;
        let window = parse_window(args.get("days"))?;
        let page = PageRequest::from_raw(
            args.get("limit").and_then(Value::as_u64),
            args.get("cursor").and_then(Value::as_str),
        )?;
        let priced = require_market(
            self.reads
                .priced_candles(&symbol, market, window.clone())
                .await,
        )?;
        let view = priced.view;
        let mut lines: Vec<&CandleToolLine> = view.lines.iter().collect();
        lines.sort_by(|left, right| {
            let left_at = *left.opened_at.utc_seconds().as_i64();
            let right_at = *right.opened_at.utc_seconds().as_i64();
            right_at.cmp(&left_at)
        });
        let items: Vec<Value> = lines
            .into_iter()
            .map(|line| {
                object(vec![
                    ("opened_at", timestamp(line.opened_at)),
                    ("open", money(&line.open)),
                    ("high", money(&line.high)),
                    ("low", money(&line.low)),
                    ("close", money(&line.close)),
                    ("volume", line.volume.as_ref().map_or(Value::Null, quantity)),
                ])
            })
            .collect();
        let fingerprint = format!(
            "{}|{}|{}|{}",
            symbol.as_str(),
            market_code(market),
            window.as_u16(),
            match priced.basis {
                CandlePriceBasis::Adjusted => "adjusted",
                CandlePriceBasis::Unadjusted => "raw",
            }
        );
        let context = PaginationContext {
            cursors: &self.cursors,
            run_id: invocation.run_id,
            tool_name: self.spec.name(),
            query_fingerprint: &fingerprint,
        };
        let extra = candle_extra(&view, priced.basis);
        paginate_items(&items, &page, &context, &extra)
    }
}

fn candle_extra(
    view: &application::CandlesToolView,
    basis: CandlePriceBasis,
) -> Map<String, Value> {
    let mut extra = Map::new();
    extra.insert(
        String::from("symbol"),
        Value::String(view.symbol.as_str().to_owned()),
    );
    extra.insert(
        String::from("market"),
        Value::String(market_code(view.market).to_owned()),
    );
    extra.insert(String::from("collected_at"), timestamp(view.collected_at));
    extra.insert(
        String::from("adjusted"),
        Value::Bool(match basis {
            CandlePriceBasis::Adjusted => true,
            CandlePriceBasis::Unadjusted => false,
        }),
    );
    extra
}

fn parse_symbol(raw: Option<&Value>) -> Result<Symbol, ToolErrorPayload> {
    raw.and_then(Value::as_str)
        .and_then(|text| Symbol::try_new(text).ok())
        .ok_or_else(|| {
            ToolErrorPayload::invalid_input(Some("symbol"), "symbol is not a valid listed code")
        })
}

fn parse_market(raw: Option<&Value>) -> Result<KnownMarket, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        None | Some("KR") => Ok(KnownMarket::Kr),
        Some("US") => Ok(KnownMarket::Us),
        Some(_other) => Err(ToolErrorPayload::invalid_input(
            Some("market"),
            "market must be KR or US",
        )),
    }
}

fn parse_domestic_market(raw: Option<&Value>) -> Result<KnownMarket, ToolErrorPayload> {
    match parse_market(raw)? {
        KnownMarket::Kr => Ok(KnownMarket::Kr),
        KnownMarket::Us => Err(ToolErrorPayload::invalid_input(
            Some("market"),
            "investor flow is available for domestic symbols only",
        )),
    }
}

fn parse_window(raw: Option<&Value>) -> Result<CandleWindow, ToolErrorPayload> {
    let days = match raw.and_then(Value::as_u64) {
        None => DEFAULT_CANDLE_DAYS,
        Some(value) => u16::try_from(value).map_err(|_overflow| {
            ToolErrorPayload::invalid_input(Some("days"), "days must be between 1 and 250")
        })?,
    };
    CandleWindow::try_new(days).map_err(|_err| {
        ToolErrorPayload::invalid_input(Some("days"), "days must be between 1 and 250")
    })
}

fn parse_ranking_kind(raw: Option<&Value>) -> Result<RankingToolKind, ToolErrorPayload> {
    match raw.and_then(Value::as_str) {
        Some("trading_amount") => Ok(RankingToolKind::TradingAmount),
        Some("gainers") => Ok(RankingToolKind::Gainers),
        Some("losers") => Ok(RankingToolKind::Losers),
        Some("foreign_net_buy") => Ok(RankingToolKind::ForeignNetBuy),
        Some("institution_net_buy") => Ok(RankingToolKind::InstitutionNetBuy),
        Some(_) | None => Err(ToolErrorPayload::invalid_input(
            Some("kind"),
            "kind is not a supported ranking",
        )),
    }
}

const fn market_code(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}

const fn ranking_kind_name(kind: RankingToolKind) -> &'static str {
    match kind {
        RankingToolKind::TradingAmount => "trading_amount",
        RankingToolKind::Gainers => "gainers",
        RankingToolKind::Losers => "losers",
        RankingToolKind::ForeignNetBuy => "foreign_net_buy",
        RankingToolKind::InstitutionNetBuy => "institution_net_buy",
    }
}

const fn overview_item_name(item: OverviewItem) -> &'static str {
    match item {
        OverviewItem::Kospi => "kospi",
        OverviewItem::Kosdaq => "kosdaq",
        OverviewItem::KrBond3Y => "kr_bond_3y",
        OverviewItem::KrBond10Y => "kr_bond_10y",
        OverviewItem::UsdKrw => "usd_krw",
        OverviewItem::Sp500 => "sp500",
        OverviewItem::Nasdaq100 => "nasdaq100",
        OverviewItem::UsTreasury2Y => "us_treasury_2y",
        OverviewItem::UsTreasury10Y => "us_treasury_10y",
        OverviewItem::CrudeOil => "crude_oil",
        OverviewItem::Gold => "gold",
        OverviewItem::DollarIndex => "dollar_index",
    }
}

const fn only_approximate(item: OverviewItem) -> bool {
    match item {
        OverviewItem::Sp500
        | OverviewItem::Nasdaq100
        | OverviewItem::CrudeOil
        | OverviewItem::Gold
        | OverviewItem::DollarIndex => true,
        OverviewItem::Kospi
        | OverviewItem::Kosdaq
        | OverviewItem::KrBond3Y
        | OverviewItem::KrBond10Y
        | OverviewItem::UsdKrw
        | OverviewItem::UsTreasury2Y
        | OverviewItem::UsTreasury10Y => false,
    }
}

const fn provenance_name(item: OverviewItem, provenance: ValueProvenance) -> &'static str {
    if only_approximate(item) {
        return "approximate";
    }
    match provenance {
        ValueProvenance::Official => "official",
        ValueProvenance::Approximate => "approximate",
    }
}

const fn unit_name(unit: OverviewUnit) -> &'static str {
    match unit {
        OverviewUnit::IndexPoints => "index_points",
        OverviewUnit::YieldPercent => "yield_percent",
        OverviewUnit::KrwPerUsd => "krw_per_usd",
        OverviewUnit::UsdPrice => "usd_price",
    }
}

const fn session_name(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::PreMarket => "pre_market",
        SessionStatus::Open => "open",
        SessionStatus::AfterHours => "after_hours",
        SessionStatus::Closed => "closed",
        SessionStatus::Holiday => "holiday",
    }
}

const NOT_PROVIDED: [OverviewItem; 2] = [OverviewItem::UsTreasury2Y, OverviewItem::UsTreasury10Y];

fn render_overview(view: &MarketOverviewToolView) -> Value {
    let mut lines = view.lines.clone();
    lines.sort_by_key(|line| line.item);
    let line_values: Vec<Value> = lines
        .iter()
        .map(|line| {
            object(vec![
                (
                    "item",
                    Value::String(overview_item_name(line.item).to_owned()),
                ),
                ("unit", Value::String(unit_name(line.unit).to_owned())),
                ("value", ratio(&line.value)),
                ("change_rate", optional_ratio(line.change_rate.as_ref())),
                (
                    "provenance",
                    Value::String(provenance_name(line.item, line.provenance).to_owned()),
                ),
                ("collected_at", timestamp(line.collected_at)),
            ])
        })
        .collect();
    let mut sessions = view.sessions.clone();
    sessions.sort_by_key(|line| market_code(line.market));
    let session_values: Vec<Value> = sessions
        .iter()
        .map(|line| {
            object(vec![
                ("market", Value::String(market_code(line.market).to_owned())),
                (
                    "status",
                    Value::String(session_name(line.status).to_owned()),
                ),
                ("collected_at", timestamp(line.collected_at)),
            ])
        })
        .collect();
    let not_provided: Vec<Value> = NOT_PROVIDED
        .iter()
        .filter(|item| lines.iter().all(|line| line.item != **item))
        .map(|item| {
            object(vec![
                ("item", Value::String(overview_item_name(*item).to_owned())),
                ("reason", Value::String(String::from("no_source"))),
            ])
        })
        .collect();
    object(vec![
        ("indicators", Value::Array(line_values)),
        ("sessions", Value::Array(session_values)),
        ("not_provided", Value::Array(not_provided)),
    ])
}

fn render_rankings(view: &RankingsToolView, limit: usize) -> Value {
    let lines: Vec<Value> = view
        .lines
        .iter()
        .take(limit)
        .zip(1_u64..)
        .map(|(line, rank)| {
            object(vec![
                ("rank", Value::from(rank)),
                ("symbol", Value::String(line.symbol.as_str().to_owned())),
                ("name", untrusted_label(line.name.as_str())),
                ("price", optional_money(line.price.as_ref())),
                ("change_rate", optional_ratio(line.change_rate.as_ref())),
                ("metric", optional_money(line.metric.as_ref())),
            ])
        })
        .collect();
    object(vec![
        (
            "kind",
            Value::String(ranking_kind_name(view.kind).to_owned()),
        ),
        ("market", Value::String(market_code(view.market).to_owned())),
        ("collected_at", timestamp(view.collected_at)),
        ("items", Value::Array(lines)),
    ])
}

fn render_sectors(view: &SectorsToolView, limit: usize) -> Value {
    let lines: Vec<Value> = view
        .lines
        .iter()
        .take(limit)
        .zip(1_u64..)
        .map(|(line, rank)| {
            object(vec![
                ("rank", Value::from(rank)),
                ("sector", untrusted_label(line.label.as_str())),
                ("change_rate", optional_ratio(line.change_rate.as_ref())),
                (
                    "trading_amount",
                    optional_money(line.trading_amount.as_ref()),
                ),
            ])
        })
        .collect();
    object(vec![
        ("market", Value::String(market_code(view.market).to_owned())),
        ("collected_at", timestamp(view.collected_at)),
        ("items", Value::Array(lines)),
    ])
}

fn render_investor_flow(view: &application::InvestorFlowToolView) -> Value {
    let mut lines = view.lines.clone();
    lines.sort_by(|left, right| {
        let left_at = *left.trading_date.utc_seconds().as_i64();
        let right_at = *right.trading_date.utc_seconds().as_i64();
        right_at.cmp(&left_at)
    });
    let values: Vec<Value> = lines
        .iter()
        .take(MAX_FLOW_DAYS)
        .map(|line| {
            object(vec![
                ("trading_date", timestamp(line.trading_date)),
                ("foreign_net", optional_money(line.foreign_net.as_ref())),
                (
                    "institution_net",
                    optional_money(line.institution_net.as_ref()),
                ),
                (
                    "individual_net",
                    optional_money(line.individual_net.as_ref()),
                ),
                (
                    "short_selling_ratio",
                    optional_ratio(line.short_selling_ratio.as_ref()),
                ),
            ])
        })
        .collect();
    object(vec![
        ("symbol", Value::String(view.symbol.as_str().to_owned())),
        ("market", Value::String(market_code(view.market).to_owned())),
        ("collected_at", timestamp(view.collected_at)),
        ("items", Value::Array(values)),
    ])
}

pub fn register_market_tools<M, C>(
    registry: &mut ToolRegistry,
    reads: &Arc<M>,
    cursors: &Arc<CursorStore<C>>,
) -> Result<(), ToolRegistryError>
where
    M: MarketToolReads + 'static,
    C: Clock + 'static,
{
    for kind in MarketToolKind::ALL {
        let name =
            ToolName::try_new(kind.name()).map_err(|_err| ToolRegistryError::ForbiddenToolName)?;
        let spec = ToolSpec::try_new(
            name,
            kind.description(),
            ToolAccessKind::ReadOnly,
            kind.input_schema(),
            None,
        )?;
        let tool = MarketTool {
            kind,
            spec,
            reads: Arc::clone(reads),
            cursors: Arc::clone(cursors),
        };
        registry.register(Arc::new(tool))?;
    }
    Ok(())
}
