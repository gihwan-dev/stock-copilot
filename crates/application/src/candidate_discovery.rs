use crate::PortError;
use domain::{
    DiscoveryCandidate, DiscoveryInput, DiscoveryQuota, DiscoverySeed, DiscoverySource, Money,
    SectorAmount, SectorComposition, StockRanking, Symbol, select_discovery_candidates,
};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

pub trait DiscoveryLists: Send + Sync {
    fn rankings(&self) -> impl Future<Output = Result<Vec<StockRanking>, PortError>> + Send;
    fn sectors(&self) -> impl Future<Output = Result<Vec<SectorComposition>, PortError>> + Send;
    fn supply_leaders(&self) -> impl Future<Output = Result<Vec<Symbol>, PortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryList {
    Rankings,
    Sectors,
    Supply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub candidates: Vec<DiscoveryCandidate>,
    pub unavailable: Vec<(DiscoveryList, PortError)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    NoList(Vec<(DiscoveryList, PortError)>),
}

#[derive(Debug)]
pub struct DiscoverCandidates<'a, Lists> {
    pub lists: &'a Lists,
    pub quota: DiscoveryQuota,
}

impl<Lists: DiscoveryLists> DiscoverCandidates<'_, Lists> {
    pub async fn execute(
        &self,
        known: &BTreeSet<Symbol>,
        seed: DiscoverySeed,
    ) -> Result<DiscoveryReport, DiscoveryError> {
        let mut unavailable = Vec::new();
        let sectors = match self.lists.sectors().await {
            Ok(value) => value,
            Err(error) => {
                unavailable.push((DiscoveryList::Sectors, error));
                Vec::new()
            }
        };
        let rankings = match self.lists.rankings().await {
            Ok(value) => value,
            Err(error) => {
                unavailable.push((DiscoveryList::Rankings, error));
                Vec::new()
            }
        };
        let supply = match self.lists.supply_leaders().await {
            Ok(value) => value,
            Err(error) => {
                unavailable.push((DiscoveryList::Supply, error));
                Vec::new()
            }
        };
        if unavailable.len() == 3 {
            return Err(DiscoveryError::NoList(unavailable));
        }
        let inputs = discovery_inputs(&rankings, &sectors, supply);
        Ok(DiscoveryReport {
            candidates: select_discovery_candidates(&inputs, known, self.quota, seed),
            unavailable,
        })
    }
}

fn market_caps(sectors: &[SectorComposition]) -> BTreeMap<Symbol, Money> {
    let mut caps = BTreeMap::new();
    for (symbol, money) in sectors
        .iter()
        .flat_map(SectorComposition::stocks)
        .filter_map(|member| match member.market_capitalization() {
            SectorAmount::Available(money) => Some((member.symbol().clone(), money)),
            SectorAmount::Unavailable => None,
        })
    {
        caps.entry(symbol).or_insert(money);
    }
    caps
}

fn discovery_inputs(
    rankings: &[StockRanking],
    sectors: &[SectorComposition],
    supply: Vec<Symbol>,
) -> Vec<DiscoveryInput> {
    let caps = market_caps(sectors);
    let input = |symbol: Symbol, source| {
        let market_cap = caps.get(&symbol).copied();
        DiscoveryInput {
            symbol,
            source,
            market_cap,
        }
    };
    let ranked = rankings.iter().flat_map(|ranking| {
        ranking.items().iter().map(move |item| {
            (
                item.symbol().clone(),
                DiscoverySource::Ranking(ranking.kind()),
            )
        })
    });
    let grouped = sectors.iter().flat_map(|composition| {
        composition.stocks().iter().map(move |member| {
            (
                member.symbol().clone(),
                DiscoverySource::Sector(composition.sector().clone()),
            )
        })
    });
    let flows = supply
        .into_iter()
        .map(|symbol| (symbol, DiscoverySource::Supply));
    ranked
        .chain(grouped)
        .chain(flows)
        .map(|(symbol, source)| input(symbol, source))
        .collect()
}
