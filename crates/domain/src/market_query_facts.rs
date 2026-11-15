use crate::market_ranking::ranking_change;
use crate::sector_composition::sector_amount;
use crate::{
    Currency, DisplayInstrumentName, InvestorClass, KnownMarket, MarketDataError,
    MarketObservation, Money, NumericInput, RankingChange, RankingContinuity, RankingDuration,
    RankingKind, RankingLimit, RankingOrdinal, SectorAmount, SectorId, Symbol, Timestamp,
};
use std::collections::BTreeSet;

const SECTOR_PERIODS: [RankingDuration; 6] = [
    RankingDuration::Day,
    RankingDuration::Week,
    RankingDuration::Month,
    RankingDuration::ThreeMonths,
    RankingDuration::SixMonths,
    RankingDuration::Year,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectorDepth {
    Major,
    Middle,
    Minor,
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
    parent: Option<SectorId>,
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
        parent: row.parent,
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
    let Some(parent_id) = &sector.parent else {
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
    pub duration: RankingDuration,
    pub change_rate: Option<NumericInput>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRate {
    duration: RankingDuration,
    change_rate: RankingChange,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorDetail {
    metadata: SectorMetadata,
    market: KnownMarket,
    summary: Option<DisplayInstrumentName>,
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
        if !rate_rows
            .iter()
            .map(|rate_row| rate_row.duration)
            .eq(SECTOR_PERIODS)
        {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        let rates = rate_rows
            .into_iter()
            .map(|rate_row| {
                Ok(SectorRate {
                    duration: rate_row.duration,
                    change_rate: ranking_change(rate_row.change_rate.as_ref())?,
                })
            })
            .collect::<Result<Vec<_>, MarketDataError>>()?;
        Ok(Self {
            metadata: metadata(row)?,
            market,
            summary,
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
    pub const fn summary(&self) -> Option<&DisplayInstrumentName> {
        self.summary.as_ref()
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
    rank: RankingOrdinal,
    sector: SectorId,
    name: DisplayInstrumentName,
    change_rate: RankingChange,
    trading_amount: SectorAmount,
    market_capitalization: SectorAmount,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRanking {
    kind: RankingKind,
    market: KnownMarket,
    duration: RankingDuration,
    items: Vec<SectorRankingItem>,
    continuity: RankingContinuity,
    observation: MarketObservation,
}
impl SectorRanking {
    pub fn from_response(
        kind: RankingKind,
        market: KnownMarket,
        duration: RankingDuration,
        rows: Vec<SectorRankingRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        if kind != RankingKind::TopGainers && kind != RankingKind::MarketTradingAmount {
            return Err(MarketDataError::UnsupportedRanking);
        }
        if !SECTOR_PERIODS.contains(&duration) {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        if rows.len() > 100 {
            return Err(MarketDataError::InvalidCount);
        }
        let mut items = rows
            .into_iter()
            .map(sector_ranking_item)
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by_key(|row| row.rank);
        let continuity =
            crate::market_ranking::ranking_continuity(items.iter().map(|row| row.rank))?;
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
            observation: MarketObservation::collected_only(collected),
        })
    }

    #[must_use]
    pub const fn kind(&self) -> RankingKind {
        self.kind
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn duration(&self) -> RankingDuration {
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
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
fn validate_sector_ranking_order(
    kind: RankingKind,
    items: &[SectorRankingItem],
) -> Result<(), MarketDataError> {
    let values = items.iter().filter_map(|item| {
        if kind == RankingKind::TopGainers {
            match item.change_rate {
                RankingChange::Available(value) => Some(value.as_decimal()),
                RankingChange::Unavailable => None,
            }
        } else {
            match item.trading_amount {
                SectorAmount::Available(value) => Some(value.amount().as_decimal()),
                SectorAmount::Unavailable => None,
            }
        }
    });
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|old| old < value) {
            return Err(MarketDataError::InvalidRank);
        }
        previous = Some(value);
    }
    Ok(())
}
fn sector_ranking_item(row: SectorRankingRow) -> Result<SectorRankingItem, MarketDataError> {
    if row.currency != Currency::Krw {
        return Err(MarketDataError::InvalidCurrency);
    }
    Ok(SectorRankingItem {
        rank: row.rank,
        sector: row.sector,
        name: row.name,
        change_rate: ranking_change(row.change_rate.as_ref())?,
        trading_amount: sector_amount(row.trading_amount.as_ref())?,
        market_capitalization: sector_amount(row.market_capitalization.as_ref())?,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetTradingDirection {
    NetBuy,
    NetSell,
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
    investor: InvestorClass,
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
            foreigner: investor_group(InvestorClass::Foreigner, foreigner, limit, collected)?,
            institution: investor_group(InvestorClass::Institution, institution, limit, collected)?,
            individual: investor_group(InvestorClass::Individual, individual, limit, collected)?,
            limit,
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
}
fn investor_group(
    investor: InvestorClass,
    row: InvestorGroupRow,
    limit: RankingLimit,
    collected: Timestamp,
) -> Result<InvestorRankingGroup, MarketDataError> {
    Ok(InvestorRankingGroup {
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
    let continuity = crate::market_ranking::ranking_continuity(items.iter().map(|row| row.rank))?;
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
    pub const fn parent(&self) -> Option<&SectorId> {
        self.parent.as_ref()
    }
}

impl SectorRate {
    #[must_use]
    pub const fn duration(&self) -> RankingDuration {
        self.duration
    }
    #[must_use]
    pub const fn change_rate(&self) -> &RankingChange {
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
    pub const fn change_rate(&self) -> &RankingChange {
        &self.change_rate
    }
    #[must_use]
    pub const fn trading_amount(&self) -> &SectorAmount {
        &self.trading_amount
    }
    #[must_use]
    pub const fn market_capitalization(&self) -> &SectorAmount {
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
    pub const fn investor(&self) -> InvestorClass {
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
        f.debug_struct("NetTradingRow")
            .field("rank", &self.rank)
            .field("symbol", &self.symbol)
            .field("currency", &self.currency)
            .field("amount", &"[REDACTED]")
            .finish()
    }
}
impl std::fmt::Debug for NetTradingItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetTradingItem")
            .field("rank", &self.rank)
            .field("symbol", &self.symbol)
            .field("amount", &"[REDACTED]")
            .finish()
    }
}
impl std::fmt::Debug for SectorRankingRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectorRankingRow")
            .field("rank", &self.rank)
            .field("sector", &self.sector)
            .field("name", &self.name)
            .field("currency", &self.currency)
            .field("change_rate", &self.change_rate)
            .field("trading_amount", &"[REDACTED]")
            .field("market_capitalization", &"[REDACTED]")
            .finish()
    }
}
#[cfg(test)]
mod tests {
    use super::{
        Currency, DisplayInstrumentName, InvestorClass, InvestorGroupRow, InvestorTradingRankings,
        KnownMarket, MarketDataError, NetTradingDirection, NetTradingRow, NumericInput,
        RankingChange, RankingContinuity, RankingDuration, RankingKind, RankingLimit,
        RankingOrdinal, SectorAmount, SectorCatalog, SectorDepth, SectorDetail, SectorEtfRow,
        SectorEtfs, SectorId, SectorMetadataRow, SectorRanking, SectorRankingRow, SectorRateRow,
        Symbol, investor_group, net_trading_item, net_trading_list, ranking_change, sector_amount,
        sector_ranking_item,
    };
    use crate::Ratio;
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
        assert_eq!(facts.limit(), RankingLimit::try_new(5).expect("개수"));
        for (group, investor, day, buy, sell) in [
            (facts.foreigner(), InvestorClass::Foreigner, 2, "111", "222"),
            (
                facts.institution(),
                InvestorClass::Institution,
                3,
                "333",
                "444",
            ),
            (
                facts.individual(),
                InvestorClass::Individual,
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
                InvestorClass::Foreigner,
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
        assert_eq!(row.parent(), Some(&id("10")));
        assert_eq!(catalog.observation().server_at(), None);
    }
    #[test]
    fn sector_detail_preserves_summary_and_six_rates() {
        let durations = [
            RankingDuration::Day,
            RankingDuration::Week,
            RankingDuration::Month,
            RankingDuration::ThreeMonths,
            RankingDuration::SixMonths,
            RankingDuration::Year,
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
        assert_eq!(detail.summary(), Some(&name("요약")));
        assert_eq!(detail.observation().collected_at(), at(5, 0));
        for (rate, duration) in detail.rates().iter().zip(durations) {
            assert_eq!(rate.duration(), duration);
        }
        assert_eq!(
            detail.rates().get(2).expect("월").change_rate(),
            &RankingChange::Unavailable
        );
        assert_eq!(
            detail.rates().first().expect("일").change_rate(),
            &RankingChange::Available(Ratio::parse(&num("0.01")).expect("비율"))
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
            ranking_change(Some(&num("-1.01"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            ranking_change(Some(&num("bad"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            ranking_change(Some(&num("-1"))),
            Ok(RankingChange::Available(
                Ratio::parse(&num("-1")).expect("전손")
            ))
        );
        assert_eq!(
            sector_amount(Some(&num("-1"))),
            Err(MarketDataError::InvalidNumber)
        );
        assert_eq!(
            sector_amount(Some(&num("bad"))),
            Err(MarketDataError::InvalidNumber)
        );
    }
    #[test]
    fn sector_rankings_preserve_nullable_fields_and_krw_amounts() {
        let facts = SectorRanking::from_response(
            RankingKind::TopGainers,
            KnownMarket::Us,
            RankingDuration::Month,
            vec![ranking_row()],
            at(5, 0),
        )
        .expect("랭킹");
        assert_eq!(facts.kind(), RankingKind::TopGainers);
        assert_eq!(facts.market(), KnownMarket::Us);
        assert_eq!(facts.duration(), RankingDuration::Month);
        assert_eq!(facts.continuity(), RankingContinuity::Gaps);
        assert_eq!(facts.observation().server_at(), None);
        let item = facts.items().first().expect("행");
        assert_eq!(item.rank(), RankingOrdinal::new(2));
        assert_eq!(item.sector(), &id("100"));
        assert_eq!(item.name(), &name("반도체"));
        assert_eq!(
            item.change_rate(),
            &RankingChange::Available(Ratio::parse(&num("0.0179")).expect("비율"))
        );
        for (value, expected) in [
            (item.trading_amount(), "2411"),
            (item.market_capitalization(), "6120"),
        ] {
            let expected_money = super::Money::parse(&num(expected), Currency::Krw).expect("금액");
            assert_eq!(value, &SectorAmount::Available(expected_money));
        }
    }
    #[test]
    fn sector_rankings_preserve_nulls_and_reject_invalid_inputs() {
        let mut row = ranking_row();
        row.change_rate = None;
        row.trading_amount = None;
        row.market_capitalization = None;
        let empty = sector_ranking_item(row).expect("null");
        assert_eq!(empty.change_rate(), &RankingChange::Unavailable);
        assert_eq!(empty.trading_amount(), &SectorAmount::Unavailable);
        assert_eq!(empty.market_capitalization(), &SectorAmount::Unavailable);
        let mut wrong_currency = ranking_row();
        wrong_currency.currency = Currency::Usd;
        assert_eq!(
            sector_ranking_item(wrong_currency),
            Err(MarketDataError::InvalidCurrency)
        );
        assert_eq!(
            SectorRanking::from_response(
                RankingKind::MarketTradingAmount,
                KnownMarket::Kr,
                RankingDuration::Day,
                vec![ranking_row(); 101],
                at(5, 0)
            ),
            Err(MarketDataError::InvalidCount)
        );
        assert_eq!(
            SectorRanking::from_response(
                RankingKind::TopGainers,
                KnownMarket::Kr,
                RankingDuration::Day,
                vec![ranking_row(); 2],
                at(5, 0)
            ),
            Err(MarketDataError::InvalidRank)
        );
        let mut second = ranking_row();
        second.rank = RankingOrdinal::new(3);
        assert_eq!(
            SectorRanking::from_response(
                RankingKind::TopGainers,
                KnownMarket::Kr,
                RankingDuration::Day,
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
    fn debug_redacts_only_money_and_keeps_identifiers_and_summary() {
        let row = net_row(1, "005930", "123456789");
        let row_debug = format!("{row:?}");
        assert!(row_debug.contains("005930"));
        assert!(!row_debug.contains("123456789"));
        let ranking_debug = format!("{:?}", ranking_row());
        assert!(ranking_debug.contains("반도체"));
        assert!(ranking_debug.contains("100"));
        assert!(!ranking_debug.contains("2411"));
        assert!(!ranking_debug.contains("6120"));
        let money = super::Money::parse(&num("123456789"), Currency::Krw).expect("금액");
        assert_eq!(
            format!("{:?}", SectorAmount::Available(money)),
            "Available([REDACTED])"
        );
        assert_eq!(format!("{:?}", SectorAmount::Unavailable), "Unavailable");
        let detail = SectorDetail::from_response(
            meta("10", SectorDepth::Major, None),
            KnownMarket::Kr,
            Some(name("요약 문장")),
            all_rate_rows(),
            at(5, 0),
        )
        .expect("상세");
        assert!(format!("{detail:?}").contains("요약 문장"));
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
        for kind in [RankingKind::TopGainers, RankingKind::MarketTradingAmount] {
            assert_eq!(
                SectorRanking::from_response(
                    kind,
                    KnownMarket::Kr,
                    RankingDuration::Day,
                    vec![first.clone(), second.clone()],
                    at(5, 0)
                ),
                Err(MarketDataError::InvalidRank)
            );
        }
        second.change_rate = None;
        SectorRanking::from_response(
            RankingKind::TopGainers,
            KnownMarket::Kr,
            RankingDuration::Day,
            vec![first, second],
            at(5, 0),
        )
        .expect("비교값 없음 보존");
    }
    fn all_rate_rows() -> Vec<SectorRateRow> {
        [
            RankingDuration::Day,
            RankingDuration::Week,
            RankingDuration::Month,
            RankingDuration::ThreeMonths,
            RankingDuration::SixMonths,
            RankingDuration::Year,
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
                InvestorClass::Foreigner,
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
                    RankingKind::TopGainers,
                    KnownMarket::Kr,
                    RankingDuration::Day,
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
                RankingKind::TopGainers,
                KnownMarket::Kr,
                RankingDuration::Day,
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
                RankingKind::MarketTradingAmount,
                KnownMarket::Kr,
                RankingDuration::Day,
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
    #[test]
    fn review_rank_order_compares_available_values_across_nulls() {
        for kind in [RankingKind::TopGainers, RankingKind::MarketTradingAmount] {
            let mut first = ranking_row();
            first.rank = RankingOrdinal::new(1);
            first.change_rate = Some(num("0.01"));
            first.trading_amount = Some(num("10"));
            let mut missing = ranking_row();
            missing.rank = RankingOrdinal::new(2);
            missing.sector = id("200");
            missing.change_rate = None;
            missing.trading_amount = None;
            let mut last = ranking_row();
            last.rank = RankingOrdinal::new(3);
            last.sector = id("300");
            last.change_rate = Some(num("0.02"));
            last.trading_amount = Some(num("20"));
            assert_eq!(
                SectorRanking::from_response(
                    kind,
                    KnownMarket::Kr,
                    RankingDuration::Day,
                    vec![first.clone(), missing.clone(), last.clone()],
                    at(5, 0)
                ),
                Err(MarketDataError::InvalidRank)
            );
            first.change_rate = Some(num("0.03"));
            first.trading_amount = Some(num("30"));
            let facts = SectorRanking::from_response(
                kind,
                KnownMarket::Kr,
                RankingDuration::Day,
                vec![first, missing, last],
                at(5, 0),
            )
            .expect("null 너머 내림차순");
            assert_eq!(facts.items().len(), 3);
        }
    }
    #[test]
    fn review_detail_rejects_self_parent_for_non_major_depths() {
        for depth in [SectorDepth::Middle, SectorDepth::Minor] {
            assert_eq!(
                SectorDetail::from_response(
                    meta("10", depth, Some("10")),
                    KnownMarket::Kr,
                    None,
                    all_rate_rows(),
                    at(5, 0)
                ),
                Err(MarketDataError::InvalidRange)
            );
        }
    }
    #[test]
    fn review_sector_queries_reject_unsupported_shared_request_values() {
        for kind in [
            RankingKind::MarketTradingVolume,
            RankingKind::TossTradingAmount,
            RankingKind::TossTradingVolume,
            RankingKind::TopLosers,
            RankingKind::Unknown,
        ] {
            assert_eq!(
                SectorRanking::from_response(
                    kind,
                    KnownMarket::Kr,
                    RankingDuration::Day,
                    vec![],
                    at(5, 0)
                ),
                Err(MarketDataError::UnsupportedRanking)
            );
        }
        for duration in [RankingDuration::Realtime, RankingDuration::Unknown] {
            assert_eq!(
                SectorRanking::from_response(
                    RankingKind::TopGainers,
                    KnownMarket::Kr,
                    duration,
                    vec![],
                    at(5, 0)
                ),
                Err(MarketDataError::UnsupportedPeriod)
            );
        }
    }
    #[test]
    fn review_sector_detail_rejects_a_seventh_rate() {
        let mut rows = all_rate_rows();
        rows.push(SectorRateRow {
            duration: RankingDuration::Day,
            change_rate: Some(num("0")),
        });
        assert_eq!(
            SectorDetail::from_response(
                meta("10", SectorDepth::Major, None),
                KnownMarket::Kr,
                None,
                rows,
                at(5, 0)
            ),
            Err(MarketDataError::UnsupportedPeriod)
        );
    }
    #[test]
    fn review_shared_amount_and_change_types_preserve_zero_and_unavailable() {
        assert_eq!(sector_amount(None), Ok(SectorAmount::Unavailable));
        assert_eq!(ranking_change(None), Ok(RankingChange::Unavailable));
        assert_eq!(
            sector_amount(Some(&num("0"))),
            Ok(SectorAmount::Available(crate::Money::new(
                crate::Amount::new(rust_decimal::Decimal::ZERO),
                Currency::Krw
            )))
        );
    }
}
