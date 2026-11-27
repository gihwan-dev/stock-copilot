use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use application::{
    AllocationLabel, BrokerRead, CandleToolLine, CandleWindow, CandlesToolView, Clock,
    DiagnosticEvent, InvestorFlowToolLine, InvestorFlowToolView, MarketInfo,
    MarketOverviewToolView, MarketToolError, MarketToolName, MarketToolReads, OverviewItem,
    OverviewToolLine, OverviewUnit, PortError, QuoteObservation, QuoteToolView, RankingQuery,
    RankingToolKind, RankingToolLine, RankingsToolView, SectorRankingQuery, SectorToolLine,
    SectorsToolView, SessionStatus, SessionToolLine, ToolReadDiagnosis, ToolReadResult,
    ValueProvenance,
};
use application::{BrokerCapability, CandlePriceBasis, PortErrorKind, PricedCandlesView};
use domain::{
    AdjustmentBasis, CalendarDate, Candle, CandlePeriod, CandleStatus, DisplayInstrumentName,
    FxPair, FxRate, FxRateKind, IndicatorCode, IndicatorUnit, InvestorClass, InvestorFlow,
    InvestorTradingRankings, KnownMarket, MarketCalendar, MarketIndicatorQuote, MarketState,
    Nanosecond, OffsetSeconds, Price, PriceAdjustment, QuoteInstrument, RankingBasis,
    RankingCautionFilter, RankingChange, RankingDuration, RankingKind, RankingLimit, Ratio,
    SectorAmount, SectorRanking, SpanSeconds, StockRanking, Symbol, TimeInterval, TimeSpan,
    Timestamp,
};

use crate::broker_link::tool_instrument;
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::state::BoxFuture;

pub trait ToolQuoteSource: Send + Sync {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>>;
    fn daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>>;
    fn adjusted_daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>>;
}

impl<B: BrokerRead + 'static> ToolQuoteSource for B {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        Box::pin(async move { BrokerRead::quotes(self, &instruments).await })
    }

    fn daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        Box::pin(async move {
            BrokerRead::candles(
                self,
                &instrument,
                window,
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            )
            .await
        })
    }

    fn adjusted_daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        Box::pin(async move {
            BrokerRead::candles(
                self,
                &instrument,
                window,
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(window.end())),
            )
            .await
        })
    }
}

pub trait ToolMarketSource: Send + Sync {
    fn indicator_quotes(
        &self,
        codes: Vec<IndicatorCode>,
    ) -> BoxFuture<'_, Result<Vec<MarketIndicatorQuote>, PortError>>;
    fn usd_krw(&self) -> BoxFuture<'_, Result<FxRate, PortError>>;
    fn calendar(&self, market: KnownMarket) -> BoxFuture<'_, Result<MarketCalendar, PortError>>;
    fn rankings(&self, query: RankingQuery) -> BoxFuture<'_, Result<StockRanking, PortError>>;
    fn investor_rankings(
        &self,
        limit: RankingLimit,
    ) -> BoxFuture<'_, Result<InvestorTradingRankings, PortError>>;
    fn sector_rankings(
        &self,
        query: SectorRankingQuery,
    ) -> BoxFuture<'_, Result<SectorRanking, PortError>>;
    fn stock_investor_flow(
        &self,
        instrument: QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> BoxFuture<'_, Result<Vec<InvestorFlow>, PortError>>;
}

impl<M: MarketInfo + 'static> ToolMarketSource for M {
    fn indicator_quotes(
        &self,
        codes: Vec<IndicatorCode>,
    ) -> BoxFuture<'_, Result<Vec<MarketIndicatorQuote>, PortError>> {
        Box::pin(async move { MarketInfo::indicator_quotes(self, &codes).await })
    }

    fn usd_krw(&self) -> BoxFuture<'_, Result<FxRate, PortError>> {
        Box::pin(MarketInfo::fx_rate(
            self,
            FxPair::UsdKrw,
            FxRateKind::Mid,
            None,
        ))
    }

    fn calendar(&self, market: KnownMarket) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        Box::pin(MarketInfo::calendar(self, market))
    }

    fn rankings(&self, query: RankingQuery) -> BoxFuture<'_, Result<StockRanking, PortError>> {
        Box::pin(async move { MarketInfo::rankings(self, &query).await })
    }

    fn investor_rankings(
        &self,
        limit: RankingLimit,
    ) -> BoxFuture<'_, Result<InvestorTradingRankings, PortError>> {
        Box::pin(MarketInfo::investor_trading_rankings(self, limit))
    }

    fn sector_rankings(
        &self,
        query: SectorRankingQuery,
    ) -> BoxFuture<'_, Result<SectorRanking, PortError>> {
        Box::pin(async move { MarketInfo::sector_rankings(self, &query).await })
    }

    fn stock_investor_flow(
        &self,
        instrument: QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> BoxFuture<'_, Result<Vec<InvestorFlow>, PortError>> {
        Box::pin(
            async move { MarketInfo::stock_investor_flow(self, &instrument, from, through).await },
        )
    }
}

#[derive(Clone, Default)]
pub struct BrokerMarketTools {
    quotes: Option<Arc<dyn ToolQuoteSource>>,
    market: Option<Arc<dyn ToolMarketSource>>,
    names: Option<InstrumentDirectory>,
    clock: Option<Arc<dyn Clock>>,
}

impl std::fmt::Debug for BrokerMarketTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerMarketTools")
    }
}

const DAY_SECONDS: u64 = 86_400;
const CALENDAR_SPAN_FACTOR: u64 = 2;
const HOLIDAY_MARGIN_DAYS: u64 = 14;
const FLOW_CALENDAR_DAYS: u64 = 45;
const KST_OFFSET: i32 = 32_400;
const RANKING_LIMIT: usize = 100;

const INDICATORS: [(IndicatorCode, OverviewItem); 4] = [
    (IndicatorCode::Kospi, OverviewItem::Kospi),
    (IndicatorCode::Kosdaq, OverviewItem::Kosdaq),
    (IndicatorCode::KrBond3Y, OverviewItem::KrBond3Y),
    (IndicatorCode::KrBond10Y, OverviewItem::KrBond10Y),
];

const ETF_PROXIES: [(&str, OverviewItem); 5] = [
    ("SPY", OverviewItem::Sp500),
    ("QQQ", OverviewItem::Nasdaq100),
    ("GLD", OverviewItem::Gold),
    ("USO", OverviewItem::CrudeOil),
    ("UUP", OverviewItem::DollarIndex),
];

fn record(tool: MarketToolName, result: ToolReadResult) {
    crate::diagnostics_log::record(&DiagnosticEvent::ToolRead(ToolReadDiagnosis {
        tool,
        result,
    }));
}

fn not_connected(tool: MarketToolName) -> MarketToolError {
    record(tool, ToolReadResult::NotConnected);
    MarketToolError::NotConnected
}

fn failed(tool: MarketToolName, error: PortError) -> MarketToolError {
    record(tool, ToolReadResult::Failed(error));
    MarketToolError::from(error)
}

fn days(count: u64) -> Option<TimeSpan> {
    TimeSpan::try_new(
        SpanSeconds::new(count.saturating_mul(DAY_SECONDS)),
        Nanosecond::new(0),
    )
    .ok()
}

fn lookback(now: Timestamp, count: u64) -> Option<TimeInterval> {
    let start = now.checked_sub(days(count)?).ok()?;
    TimeInterval::try_new(start, now).ok()
}

const fn indicator_unit(unit: IndicatorUnit) -> Option<OverviewUnit> {
    match unit {
        IndicatorUnit::IndexPoints => Some(OverviewUnit::IndexPoints),
        IndicatorUnit::YieldPercent => Some(OverviewUnit::YieldPercent),
        IndicatorUnit::Unknown => None,
    }
}

fn indicator_line(quote: &MarketIndicatorQuote) -> Option<OverviewToolLine> {
    let (_code, item) = INDICATORS
        .iter()
        .find(|(code, _item)| *code == quote.code())?;
    Some(OverviewToolLine {
        item: *item,
        unit: indicator_unit(quote.unit())?,
        value: quote.value().as_ratio(),
        change_rate: None,
        provenance: ValueProvenance::Official,
        collected_at: quote.observation().collected_at(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdjustedFailure {
    ReadRaw,
    Report,
}

const fn adjusted_failure(error: PortError) -> AdjustedFailure {
    match error.kind() {
        PortErrorKind::Unsupported(BrokerCapability::AdjustedHistory)
        | PortErrorKind::InvalidResponse(_) => AdjustedFailure::ReadRaw,
        PortErrorKind::Unsupported(_)
        | PortErrorKind::NotFound(_)
        | PortErrorKind::AccessDenied(_)
        | PortErrorKind::InvalidRequest(_)
        | PortErrorKind::Unavailable { .. }
        | PortErrorKind::MasterNotReady(_) => AdjustedFailure::Report,
    }
}

fn candle_line(candle: &Candle) -> CandleToolLine {
    CandleToolLine {
        opened_at: candle.time().timestamp(),
        open: candle.values().open().as_money(),
        high: candle.values().high().as_money(),
        low: candle.values().low().as_money(),
        close: candle.values().close().as_money(),
        volume: Some(candle.values().volume()),
    }
}

fn price_ratio(price: Price) -> Ratio {
    Ratio::of_amount(price.as_money().amount())
}

fn fx_line(rate: FxRate, collected_at: Timestamp) -> OverviewToolLine {
    OverviewToolLine {
        item: OverviewItem::UsdKrw,
        unit: OverviewUnit::KrwPerUsd,
        value: price_ratio(rate.quote_per_base()),
        change_rate: None,
        provenance: ValueProvenance::Official,
        collected_at,
    }
}

fn etf_line(observation: &QuoteObservation) -> Option<OverviewToolLine> {
    let quote = observation.result.as_ref().ok()?;
    let (_symbol, item) = ETF_PROXIES
        .iter()
        .find(|(symbol, _item)| *symbol == observation.instrument.symbol().as_str())?;
    Some(OverviewToolLine {
        item: *item,
        unit: OverviewUnit::UsdPrice,
        value: price_ratio(quote.last_price()?),
        change_rate: None,
        provenance: ValueProvenance::Approximate,
        collected_at: quote.collected_at(),
    })
}

fn session_status(calendar: &MarketCalendar, now: Timestamp) -> SessionStatus {
    if calendar.today().sessions().is_empty() {
        return SessionStatus::Holiday;
    }
    match calendar.state_at(now) {
        MarketState::PreMarket | MarketState::DayMarket => SessionStatus::PreMarket,
        MarketState::RegularMarket => SessionStatus::Open,
        MarketState::AfterMarket => SessionStatus::AfterHours,
        MarketState::Closed | MarketState::Unknown => SessionStatus::Closed,
    }
}

const fn ranking_kind(kind: RankingToolKind) -> Option<RankingKind> {
    match kind {
        RankingToolKind::TradingAmount => Some(RankingKind::MarketTradingAmount),
        RankingToolKind::Gainers => Some(RankingKind::TopGainers),
        RankingToolKind::Losers => Some(RankingKind::TopLosers),
        RankingToolKind::ForeignNetBuy | RankingToolKind::InstitutionNetBuy => None,
    }
}

const fn change(value: RankingChange) -> Option<Ratio> {
    match value {
        RankingChange::Available(rate) => Some(rate),
        RankingChange::Unavailable => None,
    }
}

const fn amount(value: SectorAmount) -> Option<domain::Money> {
    match value {
        SectorAmount::Available(money) => Some(money),
        SectorAmount::Unavailable => None,
    }
}

fn net_of(
    flow: &InvestorFlow,
    class: InvestorClass,
    close: Option<Price>,
) -> Option<domain::Money> {
    flow.top_level()
        .iter()
        .find(|item| item.investor() == class)
        .and_then(|item| item.net_krw_at(close))
}

impl BrokerMarketTools {
    #[must_use]
    pub fn connected(quotes: Arc<dyn ToolQuoteSource>) -> Self {
        Self {
            quotes: Some(quotes),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_clock(self, clock: Arc<dyn Clock>) -> Self {
        Self {
            clock: Some(clock),
            ..self
        }
    }

    #[must_use]
    pub fn with_market(self, market: Arc<dyn ToolMarketSource>) -> Self {
        Self {
            market: Some(market),
            ..self
        }
    }

    #[must_use]
    pub fn with_names(self, names: InstrumentDirectory) -> Self {
        Self {
            names: Some(names),
            ..self
        }
    }

    #[must_use]
    pub const fn unconnected() -> Self {
        Self {
            quotes: None,
            market: None,
            names: None,
            clock: None,
        }
    }

    fn display_name(&self, symbol: &Symbol, market: KnownMarket) -> Option<DisplayInstrumentName> {
        self.names
            .as_ref()
            .and_then(|names| names.entry(symbol))
            .filter(|entry| entry.market == market)
            .map(|entry| entry.name)
            .or_else(|| DisplayInstrumentName::try_new(symbol.as_str()).ok())
    }

    async fn read_quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> Result<Option<QuoteToolView>, MarketToolError> {
        let tool = MarketToolName::Quote;
        let Some(source) = self.quotes.as_ref() else {
            return Err(not_connected(tool));
        };
        let Some(instrument) = tool_instrument(symbol, market) else {
            return Ok(None);
        };
        let observations = source
            .quotes(vec![instrument])
            .await
            .map_err(|error| failed(tool, error))?;
        let Some(observation) = observations.into_iter().next() else {
            return Ok(None);
        };
        let quote = observation.result.map_err(|error| failed(tool, error))?;
        Ok(quote.last_price().map(|price| QuoteToolView {
            symbol: symbol.clone(),
            market,
            price: price.as_money(),
            change_rate: None,
            collected_at: quote.collected_at(),
        }))
    }

    async fn read_candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: CandleWindow,
    ) -> Result<Option<PricedCandlesView>, MarketToolError> {
        let tool = MarketToolName::Candles;
        let (Some(source), Some(clock)) = (self.quotes.as_ref(), self.clock.as_ref()) else {
            return Err(not_connected(tool));
        };
        let registered = self.names.as_ref().and_then(|directory| {
            let id = directory.instrument_of(market, symbol)?;
            directory.master(&id)?.quote_instrument().ok()
        });
        let Some(instrument) = registered.or_else(|| tool_instrument(symbol, market)) else {
            return Ok(None);
        };
        let now = clock.now();
        let requested = u64::from(window.as_u16());
        let interval = lookback(
            now,
            requested
                .saturating_mul(CALENDAR_SPAN_FACTOR)
                .saturating_add(HOLIDAY_MARGIN_DAYS),
        )
        .ok_or(MarketToolError::InvalidResponse)?;
        let (basis, candles) = match source
            .adjusted_daily_candles(instrument.clone(), interval)
            .await
        {
            Ok(candles) => (CandlePriceBasis::Adjusted, candles),
            Err(error) if adjusted_failure(error) == AdjustedFailure::ReadRaw => (
                CandlePriceBasis::Unadjusted,
                source
                    .daily_candles(instrument, interval)
                    .await
                    .map_err(|failure| failed(tool, failure))?,
            ),
            Err(error) => return Err(failed(tool, error)),
        };
        let finalized: Vec<&Candle> = candles
            .iter()
            .filter(|candle| candle.status() == CandleStatus::Finalized)
            .collect();
        let keep = usize::from(window.as_u16());
        let skip = finalized.len().saturating_sub(keep);
        let lines = finalized
            .iter()
            .skip(skip)
            .map(|candle| candle_line(candle))
            .collect();
        Ok(Some(PricedCandlesView {
            basis,
            view: CandlesToolView {
                symbol: symbol.clone(),
                market,
                collected_at: now,
                lines,
            },
        }))
    }

    async fn read_overview(&self) -> Result<Option<MarketOverviewToolView>, MarketToolError> {
        let tool = MarketToolName::Overview;
        let (Some(quotes), Some(market), Some(clock)) = (
            self.quotes.as_ref(),
            self.market.as_ref(),
            self.clock.as_ref(),
        ) else {
            return Err(not_connected(tool));
        };
        let mut lines = Vec::new();
        let mut sessions = Vec::new();
        let mut first_error = None;
        let mut note = |error: PortError| {
            let mapped = failed(tool, error);
            first_error.get_or_insert(mapped);
        };
        match market
            .indicator_quotes(INDICATORS.iter().map(|(code, _item)| *code).collect())
            .await
        {
            Ok(values) => lines.extend(values.iter().filter_map(indicator_line)),
            Err(error) => note(error),
        }
        match market.usd_krw().await {
            Ok(rate) => lines.push(fx_line(rate, clock.now())),
            Err(error) => note(error),
        }
        let proxies = ETF_PROXIES
            .iter()
            .filter_map(|(symbol, _item)| {
                let code = Symbol::try_new(symbol).ok()?;
                tool_instrument(&code, KnownMarket::Us)
            })
            .collect();
        match quotes.quotes(proxies).await {
            Ok(observations) => lines.extend(observations.iter().filter_map(etf_line)),
            Err(error) => note(error),
        }
        for known in [KnownMarket::Kr, KnownMarket::Us] {
            match market.calendar(known).await {
                Ok(calendar) => {
                    let now = clock.now();
                    sessions.push(SessionToolLine {
                        market: known,
                        status: session_status(&calendar, now),
                        collected_at: now,
                    });
                }
                Err(error) => note(error),
            }
        }
        match first_error {
            Some(error) if lines.is_empty() && sessions.is_empty() => Err(error),
            Some(_) | None => Ok(Some(MarketOverviewToolView { lines, sessions })),
        }
    }

    async fn read_rankings(
        &self,
        kind: RankingToolKind,
        market: KnownMarket,
    ) -> Result<Option<RankingsToolView>, MarketToolError> {
        let tool = MarketToolName::Rankings;
        let Some(source) = self.market.as_ref() else {
            return Err(not_connected(tool));
        };
        let limit =
            RankingLimit::try_new(RANKING_LIMIT).map_err(|_error| MarketToolError::Rejected)?;
        let Some(stock_kind) = ranking_kind(kind) else {
            return self.read_investor_rankings(kind, market, limit).await;
        };
        let query = RankingQuery::try_new(
            stock_kind,
            market,
            RankingDuration::Day,
            limit,
            RankingCautionFilter::Included,
        )
        .map_err(|error| failed(tool, error))?;
        let ranking = source
            .rankings(query)
            .await
            .map_err(|error| failed(tool, error))?;
        let lines = ranking
            .items()
            .iter()
            .filter_map(|item| {
                Some(RankingToolLine {
                    symbol: item.symbol().clone(),
                    name: self.display_name(item.symbol(), market)?,
                    price: None,
                    change_rate: change(item.change_rate()),
                    metric: match item.basis() {
                        RankingBasis::Amount(money) => Some(money),
                        RankingBasis::Shares(_) | RankingBasis::ChangeRate(_) => None,
                    },
                })
            })
            .collect();
        Ok(Some(RankingsToolView {
            kind,
            market,
            collected_at: ranking.observation().collected_at(),
            lines,
        }))
    }

    async fn read_investor_rankings(
        &self,
        kind: RankingToolKind,
        market: KnownMarket,
        limit: RankingLimit,
    ) -> Result<Option<RankingsToolView>, MarketToolError> {
        let tool = MarketToolName::Rankings;
        let Some(source) = self.market.as_ref() else {
            return Err(not_connected(tool));
        };
        if market != KnownMarket::Kr {
            return Err(MarketToolError::NotOffered);
        }
        let rankings = source
            .investor_rankings(limit)
            .await
            .map_err(|error| failed(tool, error))?;
        let group = match kind {
            RankingToolKind::InstitutionNetBuy => rankings.institution(),
            RankingToolKind::ForeignNetBuy
            | RankingToolKind::TradingAmount
            | RankingToolKind::Gainers
            | RankingToolKind::Losers => rankings.foreigner(),
        };
        let lines = group
            .buy()
            .items()
            .iter()
            .filter_map(|item| {
                Some(RankingToolLine {
                    symbol: item.symbol().clone(),
                    name: self.display_name(item.symbol(), market)?,
                    price: None,
                    change_rate: None,
                    metric: Some(item.amount()),
                })
            })
            .collect();
        Ok(Some(RankingsToolView {
            kind,
            market,
            collected_at: group.observation().collected_at(),
            lines,
        }))
    }

    async fn read_sectors(
        &self,
        market: KnownMarket,
    ) -> Result<Option<SectorsToolView>, MarketToolError> {
        let tool = MarketToolName::Sectors;
        let Some(source) = self.market.as_ref() else {
            return Err(not_connected(tool));
        };
        let query =
            SectorRankingQuery::try_new(RankingKind::TopGainers, market, RankingDuration::Day)
                .map_err(|error| failed(tool, error))?;
        let ranking = source
            .sector_rankings(query)
            .await
            .map_err(|error| failed(tool, error))?;
        let lines = ranking
            .items()
            .iter()
            .filter_map(|item| {
                Some(SectorToolLine {
                    label: AllocationLabel::try_new(item.name().as_str()).ok()?,
                    change_rate: change(*item.change_rate()),
                    trading_amount: amount(*item.trading_amount()),
                })
            })
            .collect();
        Ok(Some(SectorsToolView {
            market,
            collected_at: ranking.observation().collected_at(),
            lines,
        }))
    }

    async fn read_investor_flow(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> Result<Option<InvestorFlowToolView>, MarketToolError> {
        let tool = MarketToolName::InvestorFlow;
        let (Some(quotes), Some(source), Some(clock)) = (
            self.quotes.as_ref(),
            self.market.as_ref(),
            self.clock.as_ref(),
        ) else {
            return Err(not_connected(tool));
        };
        if market != KnownMarket::Kr {
            return Err(MarketToolError::NotOffered);
        }
        let Some(instrument) = tool_instrument(symbol, market) else {
            return Ok(None);
        };
        let now = clock.now();
        let kst = OffsetSeconds::new(KST_OFFSET);
        let window = lookback(now, FLOW_CALENDAR_DAYS).ok_or(MarketToolError::InvalidResponse)?;
        let (Ok(from), Ok(through)) = (window.start().date_at_offset(kst), now.date_at_offset(kst))
        else {
            return Err(MarketToolError::InvalidResponse);
        };
        let flows = source
            .stock_investor_flow(instrument.clone(), from, through)
            .await
            .map_err(|error| failed(tool, error))?;
        let closes: BTreeMap<CalendarDate, Price> =
            match quotes.daily_candles(instrument, window).await {
                Ok(candles) => candles
                    .iter()
                    .map(|candle| (candle.time().date(), candle.values().close()))
                    .collect(),
                Err(error) => {
                    let _logged = failed(tool, error);
                    BTreeMap::new()
                }
            };
        let lines = flows
            .iter()
            .filter_map(|flow| {
                let close = closes.get(&flow.date()).copied();
                Some(InvestorFlowToolLine {
                    trading_date: flow.date().start_at_offset(kst).ok()?,
                    foreign_net: net_of(flow, InvestorClass::Foreigner, close),
                    institution_net: net_of(flow, InvestorClass::Institution, close),
                    individual_net: net_of(flow, InvestorClass::Individual, close),
                    short_selling_ratio: None,
                })
            })
            .collect();
        Ok(Some(InvestorFlowToolView {
            symbol: symbol.clone(),
            market,
            collected_at: now,
            lines,
        }))
    }
}

impl MarketToolReads for BrokerMarketTools {
    fn quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<QuoteToolView>, MarketToolError>> + Send {
        self.read_quote(symbol, market)
    }

    fn candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: CandleWindow,
    ) -> impl Future<Output = Result<Option<CandlesToolView>, MarketToolError>> + Send {
        let pending = self.read_candles(symbol, market, window);
        async move { pending.await.map(|found| found.map(|priced| priced.view)) }
    }

    fn priced_candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: CandleWindow,
    ) -> impl Future<Output = Result<Option<PricedCandlesView>, MarketToolError>> + Send {
        self.read_candles(symbol, market, window)
    }

    fn market_overview(
        &self,
    ) -> impl Future<Output = Result<Option<MarketOverviewToolView>, MarketToolError>> + Send {
        self.read_overview()
    }

    fn rankings(
        &self,
        kind: RankingToolKind,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<RankingsToolView>, MarketToolError>> + Send {
        self.read_rankings(kind, market)
    }

    fn sectors(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<SectorsToolView>, MarketToolError>> + Send {
        self.read_sectors(market)
    }

    fn investor_flow(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<InvestorFlowToolView>, MarketToolError>> + Send {
        self.read_investor_flow(symbol, market)
    }
}

#[cfg(test)]
mod tests;
