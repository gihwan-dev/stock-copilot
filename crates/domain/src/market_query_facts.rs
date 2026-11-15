use crate::{
    Currency, DisplayInstrumentName, KnownMarket, MarketDataError, MarketObservation, Money,
    NumericInput, RankingContinuity, RankingLimit, RankingOrdinal, Ratio, SectorId, Symbol,
    Timestamp,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectorDepth {
    Major,
    Middle,
    Minor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectorDuration {
    Day,
    Week,
    Month,
    ThreeMonths,
    SixMonths,
    Year,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorRankingKind {
    TopGainers,
    MarketTradingAmount,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorQueryScope {
    DirectMembersOnly,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorRankingHierarchy {
    MajorAndMiddle,
}
#[derive(Clone, PartialEq, Eq)]
pub enum QueryValue<Value> {
    Unavailable,
    Available(Value),
}
impl<Value> std::fmt::Debug for QueryValue<Value> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Unavailable",
            Self::Available(_) => "Available([REDACTED])",
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorMetadataRow {
    pub id: SectorId,
    pub name: DisplayInstrumentName,
    pub depth: SectorDepth,
    pub parent: Option<SectorId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorMetadata {
    id: SectorId,
    name: DisplayInstrumentName,
    depth: SectorDepth,
    parent: QueryValue<SectorId>,
}
fn metadata(row: SectorMetadataRow) -> Result<SectorMetadata, MarketDataError> {
    if (row.depth == SectorDepth::Major && row.parent.is_some())
        || (row.depth != SectorDepth::Major && row.parent.is_none())
        || row.parent.as_ref() == Some(&row.id)
    {
        return Err(MarketDataError::InvalidRange);
    }
    Ok(SectorMetadata {
        id: row.id,
        name: row.name,
        depth: row.depth,
        parent: row
            .parent
            .map_or(QueryValue::Unavailable, QueryValue::Available),
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorCatalog {
    sectors: Vec<SectorMetadata>,
    observation: MarketObservation,
}
impl SectorCatalog {
    pub fn from_response(
        rows: Vec<SectorMetadataRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let sectors = rows
            .into_iter()
            .map(metadata)
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = BTreeSet::new();
        for sector in &sectors {
            if !seen.insert(sector.id.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
        }
        for sector in &sectors {
            validate_sector_parent(sector, &sectors)?;
        }
        Ok(Self {
            sectors,
            observation: MarketObservation::collected_only(collected),
        })
    }

    #[must_use]
    pub fn sectors(&self) -> &[SectorMetadata] {
        &self.sectors
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
fn validate_sector_parent(
    sector: &SectorMetadata,
    sectors: &[SectorMetadata],
) -> Result<(), MarketDataError> {
    let QueryValue::Available(parent_id) = &sector.parent else {
        return Ok(());
    };
    let parent = sectors
        .iter()
        .find(|candidate| &candidate.id == parent_id)
        .ok_or(MarketDataError::InvalidRange)?;
    if depth_relation(parent.depth, sector.depth) == DepthRelation::Invalid {
        return Err(MarketDataError::InvalidRange);
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepthRelation {
    Adjacent,
    Invalid,
}
fn depth_relation(parent: SectorDepth, child: SectorDepth) -> DepthRelation {
    match (parent, child) {
        (SectorDepth::Major, SectorDepth::Middle) | (SectorDepth::Middle, SectorDepth::Minor) => {
            DepthRelation::Adjacent
        }
        (SectorDepth::Major, SectorDepth::Major | SectorDepth::Minor)
        | (SectorDepth::Middle, SectorDepth::Major | SectorDepth::Middle)
        | (SectorDepth::Minor, SectorDepth::Major | SectorDepth::Middle | SectorDepth::Minor) => {
            DepthRelation::Invalid
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRateRow {
    pub duration: SectorDuration,
    pub change_rate: Option<NumericInput>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRate {
    duration: SectorDuration,
    change_rate: QueryValue<Ratio>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorDetail {
    metadata: SectorMetadata,
    market: KnownMarket,
    summary: QueryValue<DisplayInstrumentName>,
    rates: Vec<SectorRate>,
    observation: MarketObservation,
}
impl SectorDetail {
    pub fn from_response(
        row: SectorMetadataRow,
        market: KnownMarket,
        summary: Option<DisplayInstrumentName>,
        rate_rows: Vec<SectorRateRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let expected = [
            SectorDuration::Day,
            SectorDuration::Week,
            SectorDuration::Month,
            SectorDuration::ThreeMonths,
            SectorDuration::SixMonths,
            SectorDuration::Year,
        ];
        if rate_rows
            .iter()
            .map(|rate_row| rate_row.duration)
            .collect::<Vec<_>>()
            != expected
        {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        let rates = rate_rows
            .into_iter()
            .map(|rate_row| {
                Ok(SectorRate {
                    duration: rate_row.duration,
                    change_rate: query_ratio(rate_row.change_rate.as_ref())?,
                })
            })
            .collect::<Result<Vec<_>, MarketDataError>>()?;
        Ok(Self {
            metadata: metadata(row)?,
            market,
            summary: summary.map_or(QueryValue::Unavailable, QueryValue::Available),
            rates,
            observation: MarketObservation::collected_only(collected),
        })
    }

    #[must_use]
    pub const fn metadata(&self) -> &SectorMetadata {
        &self.metadata
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn summary(&self) -> &QueryValue<DisplayInstrumentName> {
        &self.summary
    }
    #[must_use]
    pub fn rates(&self) -> &[SectorRate] {
        &self.rates
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
fn query_ratio(input: Option<&NumericInput>) -> Result<QueryValue<Ratio>, MarketDataError> {
    input.map_or(Ok(QueryValue::Unavailable), |raw| {
        let value = Ratio::parse(raw).or(Err(MarketDataError::InvalidNumber))?;
        if value.as_decimal() < rust_decimal::Decimal::NEGATIVE_ONE {
            return Err(MarketDataError::InvalidNumber);
        }
        Ok(QueryValue::Available(value))
    })
}
fn query_krw(input: Option<&NumericInput>) -> Result<QueryValue<Money>, MarketDataError> {
    input.map_or(Ok(QueryValue::Unavailable), |raw| {
        let value = Money::parse(raw, Currency::Krw).or(Err(MarketDataError::InvalidNumber))?;
        if value.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            return Err(MarketDataError::InvalidNumber);
        }
        Ok(QueryValue::Available(value))
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorEtfRow {
    pub symbol: Symbol,
    pub name: DisplayInstrumentName,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorEtf {
    symbol: Symbol,
    name: DisplayInstrumentName,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorEtfs {
    sector: SectorId,
    market: KnownMarket,
    etfs: Vec<SectorEtf>,
    scope: SectorQueryScope,
    observation: MarketObservation,
}
impl SectorEtfs {
    pub fn from_response(
        sector: SectorId,
        market: KnownMarket,
        rows: Vec<SectorEtfRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let mut seen = BTreeSet::new();
        let mut etfs = Vec::new();
        for row in rows {
            crate::market_ranking::validate_market_symbol(&row.symbol, market)?;
            if !seen.insert(row.symbol.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
            etfs.push(SectorEtf {
                symbol: row.symbol,
                name: row.name,
            });
        }
        Ok(Self {
            sector,
            market,
            etfs,
            scope: SectorQueryScope::DirectMembersOnly,
            observation: MarketObservation::collected_only(collected),
        })
    }

    #[must_use]
    pub const fn sector(&self) -> &SectorId {
        &self.sector
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub fn etfs(&self) -> &[SectorEtf] {
        &self.etfs
    }
    #[must_use]
    pub const fn scope(&self) -> SectorQueryScope {
        self.scope
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct SectorRankingRow {
    pub rank: RankingOrdinal,
    pub sector: SectorId,
    pub name: DisplayInstrumentName,
    pub change_rate: Option<NumericInput>,
    pub trading_amount: Option<NumericInput>,
    pub market_capitalization: Option<NumericInput>,
    pub currency: Currency,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRankingItem {
    currency: Currency,
    rank: RankingOrdinal,
    sector: SectorId,
    name: DisplayInstrumentName,
    change_rate: QueryValue<Ratio>,
    trading_amount: QueryValue<Money>,
    market_capitalization: QueryValue<Money>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRanking {
    kind: SectorRankingKind,
    market: KnownMarket,
    duration: SectorDuration,
    items: Vec<SectorRankingItem>,
    continuity: RankingContinuity,
    scope: SectorQueryScope,
    hierarchy: SectorRankingHierarchy,
    observation: MarketObservation,
}
impl SectorRanking {
    #[must_use]
    pub const fn hierarchy(&self) -> SectorRankingHierarchy {
        self.hierarchy
    }

    pub fn from_response(
        kind: SectorRankingKind,
        market: KnownMarket,
        duration: SectorDuration,
        rows: Vec<SectorRankingRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        if rows.len() > 100 {
            return Err(MarketDataError::InvalidCount);
        }
        let mut items = rows
            .into_iter()
            .map(sector_ranking_item)
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by_key(|row| row.rank);
        let continuity = ranking_continuity(items.iter().map(|row| row.rank))?;
        validate_sector_ranking_order(kind, &items)?;
        let mut seen = BTreeSet::new();
        for item in &items {
            if !seen.insert(item.sector.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
        }
        Ok(Self {
            kind,
            market,
            duration,
            items,
            continuity,
            scope: SectorQueryScope::DirectMembersOnly,
            hierarchy: SectorRankingHierarchy::MajorAndMiddle,
            observation: MarketObservation::collected_only(collected),
        })
    }

    #[must_use]
    pub const fn kind(&self) -> SectorRankingKind {
        self.kind
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn duration(&self) -> SectorDuration {
        self.duration
    }
    #[must_use]
    pub fn items(&self) -> &[SectorRankingItem] {
        &self.items
    }
    #[must_use]
    pub const fn continuity(&self) -> RankingContinuity {
        self.continuity
    }
    #[must_use]
    pub const fn scope(&self) -> SectorQueryScope {
        self.scope
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
fn validate_sector_ranking_order(
    kind: SectorRankingKind,
    items: &[SectorRankingItem],
) -> Result<(), MarketDataError> {
    for (old, new) in items.iter().zip(items.iter().skip(1)) {
        let comparison = match kind {
            SectorRankingKind::TopGainers => {
                if let (QueryValue::Available(left), QueryValue::Available(right)) =
                    (&old.change_rate, &new.change_rate)
                {
                    Some(left.cmp(right))
                } else {
                    None
                }
            }
            SectorRankingKind::MarketTradingAmount => {
                if let (QueryValue::Available(left), QueryValue::Available(right)) =
                    (&old.trading_amount, &new.trading_amount)
                {
                    Some(left.amount().as_decimal().cmp(&right.amount().as_decimal()))
                } else {
                    None
                }
            }
        };
        if comparison == Some(std::cmp::Ordering::Less) {
            return Err(MarketDataError::InvalidRank);
        }
    }
    Ok(())
}
fn sector_ranking_item(row: SectorRankingRow) -> Result<SectorRankingItem, MarketDataError> {
    if row.currency != Currency::Krw {
        return Err(MarketDataError::InvalidCurrency);
    }
    Ok(SectorRankingItem {
        currency: row.currency,
        rank: row.rank,
        sector: row.sector,
        name: row.name,
        change_rate: query_ratio(row.change_rate.as_ref())?,
        trading_amount: query_krw(row.trading_amount.as_ref())?,
        market_capitalization: query_krw(row.market_capitalization.as_ref())?,
    })
}
fn ranking_continuity(
    ranks: impl Iterator<Item = RankingOrdinal>,
) -> Result<RankingContinuity, MarketDataError> {
    let mut previous = RankingOrdinal::new(0);
    let mut continuity = RankingContinuity::Continuous;
    for (expected, rank) in (1..).zip(ranks) {
        if rank <= previous {
            return Err(MarketDataError::InvalidRank);
        }
        if rank != RankingOrdinal::new(expected) {
            continuity = RankingContinuity::Gaps;
        }
        previous = rank;
    }
    Ok(continuity)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingInvestor {
    Foreigner,
    Institution,
    Individual,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetTradingDirection {
    NetBuy,
    NetSell,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvestorRankingScope {
    KrxAndNxtIncludingEtfs,
}
#[derive(Clone, PartialEq, Eq)]
pub struct NetTradingRow {
    pub rank: RankingOrdinal,
    pub symbol: Symbol,
    pub currency: Currency,
    pub amount: NumericInput,
}
#[derive(Clone, PartialEq, Eq)]
pub struct NetTradingItem {
    rank: RankingOrdinal,
    symbol: Symbol,
    amount: Money,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetTradingList {
    direction: NetTradingDirection,
    items: Vec<NetTradingItem>,
    continuity: RankingContinuity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorGroupRow {
    pub ranked_at: Option<Timestamp>,
    pub net_buy: Vec<NetTradingRow>,
    pub net_sell: Vec<NetTradingRow>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorRankingGroup {
    level: crate::FlowLevel,
    finality: crate::FlowFinality,
    investor: RankingInvestor,
    buy: NetTradingList,
    sell: NetTradingList,
    observation: MarketObservation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorTradingRankings {
    foreigner: InvestorRankingGroup,
    institution: InvestorRankingGroup,
    individual: InvestorRankingGroup,
    limit: RankingLimit,
    scope: InvestorRankingScope,
}
impl InvestorTradingRankings {
    pub fn from_response(
        foreigner: InvestorGroupRow,
        institution: InvestorGroupRow,
        individual: InvestorGroupRow,
        limit: RankingLimit,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        Ok(Self {
            foreigner: investor_group(RankingInvestor::Foreigner, foreigner, limit, collected)?,
            institution: investor_group(
                RankingInvestor::Institution,
                institution,
                limit,
                collected,
            )?,
            individual: investor_group(RankingInvestor::Individual, individual, limit, collected)?,
            limit,
            scope: InvestorRankingScope::KrxAndNxtIncludingEtfs,
        })
    }

    #[must_use]
    pub const fn foreigner(&self) -> &InvestorRankingGroup {
        &self.foreigner
    }
    #[must_use]
    pub const fn institution(&self) -> &InvestorRankingGroup {
        &self.institution
    }
    #[must_use]
    pub const fn individual(&self) -> &InvestorRankingGroup {
        &self.individual
    }
    #[must_use]
    pub const fn limit(&self) -> RankingLimit {
        self.limit
    }
    #[must_use]
    pub const fn scope(&self) -> InvestorRankingScope {
        self.scope
    }
}
fn investor_group(
    investor: RankingInvestor,
    row: InvestorGroupRow,
    limit: RankingLimit,
    collected: Timestamp,
) -> Result<InvestorRankingGroup, MarketDataError> {
    Ok(InvestorRankingGroup {
        level: crate::FlowLevel::TopLevel,
        finality: crate::FlowFinality::Unknown,
        investor,
        buy: net_trading_list(NetTradingDirection::NetBuy, row.net_buy, limit)?,
        sell: net_trading_list(NetTradingDirection::NetSell, row.net_sell, limit)?,
        observation: MarketObservation::from_response(row.ranked_at, collected)?,
    })
}
fn net_trading_list(
    direction: NetTradingDirection,
    rows: Vec<NetTradingRow>,
    limit: RankingLimit,
) -> Result<NetTradingList, MarketDataError> {
    if rows.len() > *limit.as_usize() {
        return Err(MarketDataError::InvalidCount);
    }
    let mut items = rows
        .into_iter()
        .map(net_trading_item)
        .collect::<Result<Vec<_>, _>>()?;
    items.sort_by_key(|row| row.rank);
    let continuity = ranking_continuity(items.iter().map(|row| row.rank))?;
    let mut seen = BTreeSet::new();
    for item in &items {
        if !seen.insert(item.symbol.clone()) {
            return Err(MarketDataError::DuplicateInstrument);
        }
    }
    for (old, new) in items.iter().zip(items.iter().skip(1)) {
        if old.amount.amount().as_decimal() < new.amount.amount().as_decimal() {
            return Err(MarketDataError::InvalidRank);
        }
    }
    Ok(NetTradingList {
        direction,
        items,
        continuity,
    })
}
fn net_trading_item(row: NetTradingRow) -> Result<NetTradingItem, MarketDataError> {
    if row.currency != Currency::Krw {
        return Err(MarketDataError::InvalidCurrency);
    }
    crate::market_ranking::validate_market_symbol(&row.symbol, KnownMarket::Kr)?;
    let amount =
        Money::parse(&row.amount, Currency::Krw).or(Err(MarketDataError::InvalidNumber))?;
    let value = amount.amount().as_decimal();
    if value <= rust_decimal::Decimal::ZERO || !value.fract().is_zero() {
        return Err(MarketDataError::InvalidNumber);
    }
    Ok(NetTradingItem {
        rank: row.rank,
        symbol: row.symbol,
        amount,
    })
}

impl SectorMetadata {
    #[must_use]
    pub const fn id(&self) -> &SectorId {
        &self.id
    }
    #[must_use]
    pub const fn name(&self) -> &DisplayInstrumentName {
        &self.name
    }
    #[must_use]
    pub const fn depth(&self) -> SectorDepth {
        self.depth
    }
    #[must_use]
    pub const fn parent(&self) -> &QueryValue<SectorId> {
        &self.parent
    }
}

impl SectorRate {
    #[must_use]
    pub const fn duration(&self) -> SectorDuration {
        self.duration
    }
    #[must_use]
    pub const fn change_rate(&self) -> &QueryValue<Ratio> {
        &self.change_rate
    }
}

impl SectorEtf {
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn name(&self) -> &DisplayInstrumentName {
        &self.name
    }
}

impl SectorRankingItem {
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    #[must_use]
    pub const fn rank(&self) -> RankingOrdinal {
        self.rank
    }
    #[must_use]
    pub const fn sector(&self) -> &SectorId {
        &self.sector
    }
    #[must_use]
    pub const fn name(&self) -> &DisplayInstrumentName {
        &self.name
    }
    #[must_use]
    pub const fn change_rate(&self) -> &QueryValue<Ratio> {
        &self.change_rate
    }
    #[must_use]
    pub const fn trading_amount(&self) -> &QueryValue<Money> {
        &self.trading_amount
    }
    #[must_use]
    pub const fn market_capitalization(&self) -> &QueryValue<Money> {
        &self.market_capitalization
    }
}

impl NetTradingItem {
    #[must_use]
    pub const fn rank(&self) -> RankingOrdinal {
        self.rank
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn amount(&self) -> Money {
        self.amount
    }
}

impl NetTradingList {
    #[must_use]
    pub const fn direction(&self) -> NetTradingDirection {
        self.direction
    }
    #[must_use]
    pub fn items(&self) -> &[NetTradingItem] {
        &self.items
    }
    #[must_use]
    pub const fn continuity(&self) -> RankingContinuity {
        self.continuity
    }
}

impl InvestorRankingGroup {
    #[must_use]
    pub const fn level(&self) -> crate::FlowLevel {
        self.level
    }
    #[must_use]
    pub const fn finality(&self) -> crate::FlowFinality {
        self.finality
    }

    #[must_use]
    pub const fn investor(&self) -> RankingInvestor {
        self.investor
    }
    #[must_use]
    pub const fn buy(&self) -> &NetTradingList {
        &self.buy
    }
    #[must_use]
    pub const fn sell(&self) -> &NetTradingList {
        &self.sell
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}

impl std::fmt::Debug for NetTradingRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NetTradingRow([REDACTED])")
    }
}
impl std::fmt::Debug for NetTradingItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NetTradingItem([REDACTED])")
    }
}
impl std::fmt::Debug for SectorRankingRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SectorRankingRow([REDACTED])")
    }
}
#[cfg(test)]
mod tests {
    use super::{
        Currency, DisplayInstrumentName, InvestorGroupRow, InvestorRankingScope,
        InvestorTradingRankings, KnownMarket, MarketDataError, NetTradingDirection, NetTradingRow,
        NumericInput, QueryValue, RankingContinuity, RankingInvestor, RankingLimit, RankingOrdinal,
        Ratio, SectorCatalog, SectorDepth, SectorDetail, SectorDuration, SectorEtfRow, SectorEtfs,
        SectorId, SectorMetadataRow, SectorQueryScope, SectorRanking, SectorRankingKind,
        SectorRankingRow, SectorRateRow, Symbol, investor_group, net_trading_item,
        net_trading_list, query_krw, query_ratio, sector_ranking_item,
    };
    use crate::valuation_snapshot::tests::at;
    fn num(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("숫자")
    }
    fn name(text: &str) -> DisplayInstrumentName {
        DisplayInstrumentName::try_new(text).expect("이름")
    }
    fn id(text: &str) -> SectorId {
        SectorId::try_new(text).expect("섹터")
    }
    fn meta(text: &str, depth: SectorDepth, parent: Option<&str>) -> SectorMetadataRow {
        SectorMetadataRow {
            id: id(text),
            name: name(text),
            depth,
            parent: parent.map(id),
        }
    }
    fn ranking_row() -> SectorRankingRow {
        SectorRankingRow {
            rank: RankingOrdinal::new(2),
            sector: id("100"),
            name: name("반도체"),
            change_rate: Some(num("0.0179")),
            trading_amount: Some(num("2411")),
            market_capitalization: Some(num("6120")),
            currency: Currency::Krw,
        }
    }
    fn net_row(rank: usize, symbol: &str, amount: &str) -> NetTradingRow {
        NetTradingRow {
            rank: RankingOrdinal::new(rank),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            currency: Currency::Krw,
            amount: num(amount),
        }
    }
    fn group(day: i64, buy: &str, sell: &str) -> InvestorGroupRow {
        InvestorGroupRow {
            ranked_at: Some(at(day, 0)),
            net_buy: vec![net_row(1, "005930", buy)],
            net_sell: vec![net_row(3, "000660", sell)],
        }
    }
    #[test]
    fn investor_groups_keep_independent_times_directions_units_and_gaps() {
        let facts = InvestorTradingRankings::from_response(
            group(2, "111", "222"),
            group(3, "333", "444"),
            group(4, "555", "666"),
            RankingLimit::try_new(5).expect("개수"),
            at(6, 0),
        )
        .expect("랭킹");
        assert_eq!(facts.scope(), InvestorRankingScope::KrxAndNxtIncludingEtfs);
        assert_eq!(facts.limit(), RankingLimit::try_new(5).expect("개수"));
        for (group, investor, day, buy, sell) in [
            (
                facts.foreigner(),
                RankingInvestor::Foreigner,
                2,
                "111",
                "222",
            ),
            (
                facts.institution(),
                RankingInvestor::Institution,
                3,
                "333",
                "444",
            ),
            (
                facts.individual(),
                RankingInvestor::Individual,
                4,
                "555",
                "666",
            ),
        ] {
            assert_eq!(group.investor(), investor);
            assert_eq!(group.observation().server_at(), Some(at(day, 0)));
            assert_eq!(group.observation().collected_at(), at(6, 0));
            assert_eq!(group.buy().direction(), NetTradingDirection::NetBuy);
            assert_eq!(group.sell().direction(), NetTradingDirection::NetSell);
            assert_eq!(group.buy().continuity(), RankingContinuity::Continuous);
            assert_eq!(group.sell().continuity(), RankingContinuity::Gaps);
            let first = group.buy().items().first().expect("매수");
            let last = group.sell().items().first().expect("매도");
            assert_eq!(first.symbol().as_str(), "005930");
            assert_eq!(last.symbol().as_str(), "000660");
            assert_eq!(first.rank(), RankingOrdinal::new(1));
            assert_eq!(last.rank(), RankingOrdinal::new(3));
            assert_eq!(first.amount().amount().as_decimal().to_string(), buy);
            assert_eq!(last.amount().amount().as_decimal().to_string(), sell);
            assert_eq!(first.amount().currency(), Currency::Krw);
        }
        assert!(!format!("{facts:?}").contains("666"));
    }
    #[test]
    fn investor_empty_groups_preserve_unavailable_time() {
        let empty = InvestorGroupRow {
            ranked_at: None,
            net_buy: vec![],
            net_sell: vec![],
        };
        let facts = InvestorTradingRankings::from_response(
            empty.clone(),
            empty.clone(),
            empty,
            RankingLimit::try_new(1).expect("개수"),
            at(6, 0),
        )
        .expect("빈 응답");
        assert_eq!(facts.individual().observation().server_at(), None);
    }
    #[test]
    fn investor_invalid_values_order_counts_currency_and_identity_are_closed() {
        let limit = RankingLimit::try_new(2).expect("개수");
        for amount in ["0", "-1", "1.1", "bad"] {
            assert_eq!(
                net_trading_list(
                    NetTradingDirection::NetBuy,
                    vec![net_row(1, "005930", amount)],
                    limit
                ),
                Err(MarketDataError::InvalidNumber)
            );
        }
        for rows in [
            vec![net_row(0, "005930", "1")],
            vec![net_row(1, "005930", "1"), net_row(1, "000660", "2")],
            vec![net_row(1, "005930", "1"), net_row(2, "000660", "2")],
        ] {
            assert_eq!(
                net_trading_list(NetTradingDirection::NetBuy, rows, limit),
                Err(MarketDataError::InvalidRank)
            );
        }
        assert_eq!(
            net_trading_list(
                NetTradingDirection::NetBuy,
                vec![net_row(1, "005930", "2"), net_row(2, "005930", "1")],
                limit
            ),
            Err(MarketDataError::DuplicateInstrument)
        );
        assert_eq!(
            net_trading_list(
                NetTradingDirection::NetBuy,
                vec![net_row(1, "005930", "1"); 3],
                limit
            ),
            Err(MarketDataError::InvalidCount)
        );
        let mut row = net_row(1, "005930", "1");
        row.currency = Currency::Usd;
        assert_eq!(net_trading_item(row), Err(MarketDataError::InvalidCurrency));
        assert_eq!(
            net_trading_item(net_row(1, "AAPL", "1")),
            Err(MarketDataError::InvalidSymbol)
        );
        assert_eq!(
            investor_group(
                RankingInvestor::Foreigner,
                group(9, "1", "2"),
                limit,
                at(2, 0)
            ),
            Err(MarketDataError::FutureTimestamp)
        );
    }
    #[test]
    fn sector_metadata_hierarchy_summary_and_all_six_rates_are_preserved() {
        let catalog = SectorCatalog::from_response(
            vec![
                meta("10", SectorDepth::Major, None),
                meta("100", SectorDepth::Middle, Some("10")),
                meta("101", SectorDepth::Minor, Some("100")),
            ],
            at(5, 0),
        )
        .expect("목록");
        let row = catalog.sectors().get(1).expect("중분류");
        assert_eq!(row.id(), &id("100"));
        assert_eq!(row.name(), &name("100"));
        assert_eq!(row.depth(), SectorDepth::Middle);
        assert_eq!(row.parent(), &QueryValue::Available(id("10")));
        assert_eq!(catalog.observation().server_at(), None);
    }
    #[test]
    fn sector_detail_preserves_summary_and_six_rates() {
        let durations = [
            SectorDuration::Day,
            SectorDuration::Week,
            SectorDuration::Month,
            SectorDuration::ThreeMonths,
            SectorDuration::SixMonths,
            SectorDuration::Year,
        ];
        let rates = durations
            .into_iter()
            .zip([
                Some("0.01"),
                Some("-0.02"),
                None,
                Some("0.04"),
                Some("0.05"),
                Some("0.06"),
            ])
            .map(|(duration, value)| SectorRateRow {
                duration,
                change_rate: value.map(num),
            })
            .collect();
        let detail = SectorDetail::from_response(
            meta("10", SectorDepth::Major, None),
            KnownMarket::Us,
            Some(name("요약")),
            rates,
            at(5, 0),
        )
        .expect("상세");
        assert_eq!(detail.metadata().id(), &id("10"));
        assert_eq!(detail.market(), KnownMarket::Us);
        assert_eq!(detail.summary(), &QueryValue::Available(name("요약")));
        assert_eq!(detail.observation().collected_at(), at(5, 0));
        for (rate, duration) in detail.rates().iter().zip(durations) {
            assert_eq!(rate.duration(), duration);
        }
        assert_eq!(
            detail.rates().get(2).expect("월").change_rate(),
            &QueryValue::Unavailable
        );
        assert_eq!(
            detail.rates().first().expect("일").change_rate(),
            &QueryValue::Available(Ratio::parse(&num("0.01")).expect("비율"))
        );
    }
    #[test]
    fn sector_metadata_and_rates_reject_invalid_hierarchy_and_periods() {
        for rows in [
            vec![meta("10", SectorDepth::Major, Some("20"))],
            vec![meta("10", SectorDepth::Middle, None)],
            vec![meta("10", SectorDepth::Middle, Some("10"))],
            vec![meta("10", SectorDepth::Middle, Some("20"))],
            vec![
                meta("10", SectorDepth::Major, None),
                meta("10", SectorDepth::Major, None),
            ],
            vec![
                meta("10", SectorDepth::Major, None),
                meta("20", SectorDepth::Minor, Some("10")),
            ],
        ] {
            SectorCatalog::from_response(rows, at(5, 0)).expect_err("거부");
        }
        SectorDetail::from_response(
            meta("10", SectorDepth::Major, None),
            KnownMarket::Kr,
            None,
            vec![],
            at(5, 0),
        )
        .expect_err("기간");
        assert_eq!(
            query_ratio(Some(&num("-1.01"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            query_ratio(Some(&num("bad"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            query_ratio(Some(&num("-1"))),
            Ok(QueryValue::Available(
                Ratio::parse(&num("-1")).expect("전손")
            ))
        );
        assert_eq!(
            query_krw(Some(&num("-1"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            query_krw(Some(&num("bad"))),
            Err(MarketDataError::InvalidNumber)
        );
    }
    #[test]
    fn sector_rankings_preserve_nullable_fields_krw_and_direct_scope() {
        let facts = SectorRanking::from_response(
            SectorRankingKind::TopGainers,
            KnownMarket::Us,
            SectorDuration::Month,
            vec![ranking_row()],
            at(5, 0),
        )
        .expect("랭킹");
        assert_eq!(
            facts.hierarchy(),
            crate::SectorRankingHierarchy::MajorAndMiddle
        );
        assert_eq!(facts.kind(), SectorRankingKind::TopGainers);
        assert_eq!(facts.market(), KnownMarket::Us);
        assert_eq!(facts.duration(), SectorDuration::Month);
        assert_eq!(facts.continuity(), RankingContinuity::Gaps);
        assert_eq!(facts.scope(), SectorQueryScope::DirectMembersOnly);
        assert_eq!(facts.observation().server_at(), None);
        let item = facts.items().first().expect("행");
        assert_eq!(item.rank(), RankingOrdinal::new(2));
        assert_eq!(item.sector(), &id("100"));
        assert_eq!(item.name(), &name("반도체"));
        assert_eq!(
            item.change_rate(),
            &QueryValue::Available(Ratio::parse(&num("0.0179")).expect("비율"))
        );
        for (value, expected) in [
            (item.trading_amount(), "2411"),
            (item.market_capitalization(), "6120"),
        ] {
            let expected_money = super::Money::parse(&num(expected), Currency::Krw).expect("금액");
            assert_eq!(value, &QueryValue::Available(expected_money));
        }
    }
    #[test]
    fn sector_rankings_preserve_nulls_and_reject_invalid_inputs() {
        let mut row = ranking_row();
        row.change_rate = None;
        row.trading_amount = None;
        row.market_capitalization = None;
        let empty = sector_ranking_item(row).expect("null");
        assert_eq!(empty.change_rate(), &QueryValue::Unavailable);
        assert_eq!(empty.trading_amount(), &QueryValue::Unavailable);
        assert_eq!(empty.market_capitalization(), &QueryValue::Unavailable);
        let mut wrong_currency = ranking_row();
        wrong_currency.currency = Currency::Usd;
        assert_eq!(
            sector_ranking_item(wrong_currency),
            Err(MarketDataError::InvalidCurrency)
        );
        assert_eq!(
            SectorRanking::from_response(
                SectorRankingKind::MarketTradingAmount,
                KnownMarket::Kr,
                SectorDuration::Day,
                vec![ranking_row(); 101],
                at(5, 0)
            ),
            Err(MarketDataError::InvalidCount)
        );
        assert_eq!(
            SectorRanking::from_response(
                SectorRankingKind::TopGainers,
                KnownMarket::Kr,
                SectorDuration::Day,
                vec![ranking_row(); 2],
                at(5, 0)
            ),
            Err(MarketDataError::InvalidRank)
        );
        let mut second = ranking_row();
        second.rank = RankingOrdinal::new(3);
        assert_eq!(
            SectorRanking::from_response(
                SectorRankingKind::TopGainers,
                KnownMarket::Kr,
                SectorDuration::Day,
                vec![ranking_row(), second],
                at(5, 0)
            ),
            Err(MarketDataError::DuplicateInstrument)
        );
    }
    #[test]
    fn sector_etfs_keep_response_order_without_lookthrough_or_fabricated_time() {
        let rows = vec![
            SectorEtfRow {
                symbol: Symbol::try_new("SOXX").expect("심볼"),
                name: name("ETF A"),
            },
            SectorEtfRow {
                symbol: Symbol::try_new("SMH").expect("심볼"),
                name: name("ETF B"),
            },
        ];
        let facts = SectorEtfs::from_response(id("100"), KnownMarket::Us, rows.clone(), at(5, 0))
            .expect("ETF");
        assert_eq!(facts.sector(), &id("100"));
        assert_eq!(facts.market(), KnownMarket::Us);
        assert_eq!(facts.scope(), SectorQueryScope::DirectMembersOnly);
        assert_eq!(facts.observation().server_at(), None);
        for (fact, row) in facts.etfs().iter().zip(&rows) {
            assert_eq!(fact.symbol(), &row.symbol);
            assert_eq!(fact.name(), &row.name);
        }
        assert_eq!(
            SectorEtfs::from_response(
                id("100"),
                KnownMarket::Us,
                vec![rows[0].clone(); 2],
                at(5, 0)
            ),
            Err(MarketDataError::DuplicateInstrument)
        );
        assert_eq!(
            SectorEtfs::from_response(id("100"), KnownMarket::Kr, rows, at(5, 0)),
            Err(MarketDataError::InvalidSymbol)
        );
    }
    #[test]
    fn debug_redacts_amount_rows_optional_values_and_group_finality() {
        let row = net_row(1, "005930", "123456789");
        assert_eq!(format!("{row:?}"), "NetTradingRow([REDACTED])");
        assert_eq!(
            format!("{:?}", ranking_row()),
            "SectorRankingRow([REDACTED])"
        );
        let money = super::Money::parse(&num("123456789"), Currency::Krw).expect("금액");
        assert_eq!(
            format!("{:?}", QueryValue::Available(money)),
            "Available([REDACTED])"
        );
        assert_eq!(
            format!("{:?}", QueryValue::<super::Money>::Unavailable),
            "Unavailable"
        );
        let fact = investor_group(
            RankingInvestor::Individual,
            group(2, "1", "2"),
            RankingLimit::try_new(1).expect("개수"),
            at(5, 0),
        )
        .expect("투자자");
        assert_eq!(fact.level(), crate::FlowLevel::TopLevel);
        assert_eq!(fact.finality(), crate::FlowFinality::Unknown);
        assert_eq!(
            sector_ranking_item(ranking_row()).expect("행").currency(),
            Currency::Krw
        );
    }
    #[test]
    fn sector_ranking_rejects_known_basis_reversals_without_erasing_nulls() {
        let mut first = ranking_row();
        first.rank = RankingOrdinal::new(1);
        first.change_rate = Some(num("0.01"));
        first.trading_amount = Some(num("10"));
        let mut second = ranking_row();
        second.rank = RankingOrdinal::new(2);
        second.sector = id("200");
        second.change_rate = Some(num("0.02"));
        second.trading_amount = Some(num("20"));
        for kind in [
            SectorRankingKind::TopGainers,
            SectorRankingKind::MarketTradingAmount,
        ] {
            assert_eq!(
                SectorRanking::from_response(
                    kind,
                    KnownMarket::Kr,
                    SectorDuration::Day,
                    vec![first.clone(), second.clone()],
                    at(5, 0)
                ),
                Err(MarketDataError::InvalidRank)
            );
        }
        second.change_rate = None;
        SectorRanking::from_response(
            SectorRankingKind::TopGainers,
            KnownMarket::Kr,
            SectorDuration::Day,
            vec![first, second],
            at(5, 0),
        )
        .expect("비교값 없음 보존");
    }
    fn all_rate_rows() -> Vec<SectorRateRow> {
        [
            SectorDuration::Day,
            SectorDuration::Week,
            SectorDuration::Month,
            SectorDuration::ThreeMonths,
            SectorDuration::SixMonths,
            SectorDuration::Year,
        ]
        .into_iter()
        .map(|duration| SectorRateRow {
            duration,
            change_rate: Some(num("0.01")),
        })
        .collect()
    }
    #[test]
    fn detail_rejects_invalid_rate_payloads_and_invalid_metadata() {
        let mut rates = all_rate_rows();
        rates.first_mut().expect("일").change_rate = Some(num("bad"));
        assert_eq!(
            SectorDetail::from_response(
                meta("10", SectorDepth::Major, None),
                KnownMarket::Kr,
                None,
                rates,
                at(5, 0)
            ),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            SectorDetail::from_response(
                meta("10", SectorDepth::Major, Some("10")),
                KnownMarket::Kr,
                None,
                all_rate_rows(),
                at(5, 0)
            ),
            Err(MarketDataError::InvalidRange)
        );
    }
    #[test]
    fn each_investor_group_rejects_invalid_buy_and_sell_payloads() {
        for index in 0..3 {
            let mut groups = [
                group(2, "111", "222"),
                group(3, "333", "444"),
                group(4, "555", "666"),
            ];
            groups
                .get_mut(index)
                .expect("그룹")
                .net_buy
                .first_mut()
                .expect("매수")
                .amount = num("bad");
            let [foreigner, institution, individual] = groups;
            assert_eq!(
                InvestorTradingRankings::from_response(
                    foreigner,
                    institution,
                    individual,
                    RankingLimit::try_new(5).expect("개수"),
                    at(6, 0)
                ),
                Err(MarketDataError::InvalidNumber)
            );
        }
        let mut invalid = group(2, "111", "222");
        invalid.net_sell.first_mut().expect("매도").amount = num("bad");
        assert_eq!(
            investor_group(
                RankingInvestor::Foreigner,
                invalid,
                RankingLimit::try_new(5).expect("개수"),
                at(6, 0)
            ),
            Err(MarketDataError::InvalidNumber)
        );
    }
    #[test]
    fn ranking_rejects_each_invalid_numeric_field_and_preserves_missing_amount_basis() {
        let mut invalid_rate = ranking_row();
        invalid_rate.change_rate = Some(num("bad"));
        let mut invalid_amount = ranking_row();
        invalid_amount.trading_amount = Some(num("bad"));
        let mut invalid_capitalization = ranking_row();
        invalid_capitalization.market_capitalization = Some(num("bad"));
        for row in [invalid_rate, invalid_amount, invalid_capitalization] {
            assert_eq!(
                SectorRanking::from_response(
                    SectorRankingKind::TopGainers,
                    KnownMarket::Kr,
                    SectorDuration::Day,
                    vec![row],
                    at(5, 0)
                ),
                Err(MarketDataError::InvalidNumber)
            );
        }
        let mut wrong_unit = ranking_row();
        wrong_unit.currency = Currency::Usd;
        assert_eq!(
            SectorRanking::from_response(
                SectorRankingKind::TopGainers,
                KnownMarket::Kr,
                SectorDuration::Day,
                vec![wrong_unit],
                at(5, 0)
            ),
            Err(MarketDataError::InvalidCurrency)
        );
        let mut first = ranking_row();
        first.rank = RankingOrdinal::new(1);
        let mut second = ranking_row();
        second.rank = RankingOrdinal::new(2);
        second.sector = id("200");
        second.trading_amount = None;
        assert_eq!(
            SectorRanking::from_response(
                SectorRankingKind::MarketTradingAmount,
                KnownMarket::Kr,
                SectorDuration::Day,
                vec![first, second],
                at(5, 0)
            )
            .expect("미제공 보존")
            .items()
            .len(),
            2
        );
    }
    #[test]
    fn investor_lists_accept_multiple_descending_values_at_the_limit() {
        let list = net_trading_list(
            NetTradingDirection::NetBuy,
            vec![net_row(1, "005930", "222"), net_row(2, "000660", "111")],
            RankingLimit::try_new(2).expect("개수"),
        )
        .expect("두 행");
        assert_eq!(list.items().len(), 2);
        assert_eq!(list.continuity(), RankingContinuity::Continuous);
    }
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(testkit::property_cases()))]
        #[test]
        fn net_amount_round_trip_and_gaps_are_independent(value in 1_u64..100_000_000,rank in 1_usize..100) {
            let text=value.to_string();let list=net_trading_list(NetTradingDirection::NetSell,vec![net_row(rank,"005930",&text)],RankingLimit::try_new(100).expect("개수")).expect("목록");
            proptest::prop_assert_eq!(list.items().first().expect("행").amount().amount().as_decimal().to_string(),text);
            proptest::prop_assert_eq!(list.continuity(),if rank==1 {RankingContinuity::Continuous} else {RankingContinuity::Gaps});
        }
    }
}
