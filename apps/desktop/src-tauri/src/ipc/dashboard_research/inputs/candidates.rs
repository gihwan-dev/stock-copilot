mod evidence;
mod headlines;
#[cfg(test)]
mod tests;

use super::fact;
use crate::{
    broker_link::BrokerMarketTools,
    holdings_link::InstrumentDirectory,
    ports::{PortTypes, Ports},
};
use application::{
    BriefingSource, CandleWindow, Clock, DashboardSubject, MarketToolError, MarketToolReads,
    RankingToolKind, RankingsToolView, TrackedInstrument,
};
use domain::{KnownMarket, Symbol, Timestamp};
use std::{collections::BTreeSet, sync::Arc};
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};

struct Prospect {
    target: TrackedInstrument,
    rank: BriefingSource,
    position: usize,
}

type Inputs = Option<(DashboardSubject, Vec<BriefingSource>)>;
type RankRead = Result<Option<RankingsToolView>, MarketToolError>;

fn ranked(
    read: RankRead,
    region: KnownMarket,
    held: &BTreeSet<Symbol>,
    at: Timestamp,
) -> Result<Vec<Prospect>, ()> {
    let Some(view) = read.map_err(|_error| ())? else {
        return Ok(Vec::new());
    };
    if view.kind != RankingToolKind::TradingAmount
        || view.market != region
        || !evidence::recent(view.collected_at, at)
    {
        return Ok(Vec::new());
    }
    let mut seen = held.clone();
    Ok(view
        .lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if !seen.insert(row.symbol.clone()) {
                return None;
            }
            let position = index.saturating_add(1);
            let amount = row.metric.map_or_else(String::new, |value| {
                format!(" · 거래대금 {}", evidence::money(value))
            });
            let rank = fact(
                format!(
                    "토스증권 {} 거래대금 {position}위 · {} ({}){amount}",
                    evidence::region(region),
                    row.name.as_str(),
                    row.symbol.as_str()
                ),
                view.collected_at,
            )?;
            Some(Prospect {
                target: TrackedInstrument {
                    symbol: row.symbol,
                    name: row.name,
                    market: region,
                    aliases: Vec::new(),
                },
                rank,
                position,
            })
        })
        .take(2)
        .collect())
}

fn both_markets(
    kr: Result<Vec<Prospect>, ()>,
    us: Result<Vec<Prospect>, ()>,
) -> Result<(Vec<Prospect>, bool), ()> {
    let partial = kr.is_err() || us.is_err();
    if kr.is_err() && us.is_err() {
        return Err(());
    }
    let mut candidates: Vec<_> = kr
        .into_iter()
        .flatten()
        .chain(us.into_iter().flatten())
        .collect();
    candidates.sort_by_key(|row| row.position);
    Ok((candidates, partial))
}

pub(super) async fn collect<R: Runtime, P: PortTypes>(
    app: &AppHandle<R>,
    ports: &Ports<P>,
    market: &BrokerMarketTools,
    held: &BTreeSet<Symbol>,
    now: Timestamp,
) -> Result<Inputs, ()> {
    let (kr, us) = tokio::join!(
        market.rankings(RankingToolKind::TradingAmount, KnownMarket::Kr),
        market.rankings(RankingToolKind::TradingAmount, KnownMarket::Us)
    );
    let clock = ports.clock.as_ref();
    let observed = clock.now();
    let (prospects, mut failed) = both_markets(
        ranked(kr, KnownMarket::Kr, held, observed),
        ranked(us, KnownMarket::Us, held, observed),
    )?;
    let store = app.try_state::<Arc<SqliteRepositories>>();
    let client = app.try_state::<Arc<sources::SourceHttpClient>>();
    let directory = app
        .try_state::<InstrumentDirectory>()
        .as_deref()
        .map_or_else(InstrumentDirectory::default, Clone::clone);
    let linker = app.try_state::<crate::collected_links::CollectedLinker>();
    let records = headlines::Archive::load(store.as_deref().map(Arc::as_ref), now).await;
    let mut best = None;
    for prospect in prospects {
        let target = &prospect.target;
        let id = directory.instrument_of(target.market, &target.symbol);
        let names =
            crate::news_tracking::match_names(std::slice::from_ref(target), &[], &directory);
        let company = headlines::Company {
            target,
            canonical: id.as_ref(),
            linker: linker.as_deref().map(|value| value.0.as_ref()),
            names: &names,
        };
        let (prices, bad_market) = readings(market, target, clock).await;
        let (articles, bad_news) = records
            .for_candidate(
                &company,
                store.as_deref().map(Arc::as_ref),
                client.as_deref(),
                clock,
            )
            .await;
        failed |= bad_market || bad_news;
        if let Some(choice) = choice(prospect, prices, articles)
            && best.as_ref().is_none_or(|(score, _, _)| choice.0 > *score)
        {
            best = Some(choice);
        }
    }
    match best {
        Some((_, subject, evidence_rows)) => Ok(Some((subject, evidence_rows))),
        None if failed => Err(()),
        None => Ok(None),
    }
}

fn choice(
    prospect: Prospect,
    mut prices: Vec<BriefingSource>,
    articles: Vec<BriefingSource>,
) -> Option<(usize, DashboardSubject, Vec<BriefingSource>)> {
    if prices.is_empty() && articles.is_empty() {
        return None;
    }
    let score = prices
        .len()
        .saturating_add(usize::from(!articles.is_empty()));
    let mut facts = vec![prospect.rank];
    facts.append(&mut prices);
    facts.extend(articles);
    Some((
        score,
        DashboardSubject::Candidate(prospect.target.symbol, prospect.target.name),
        facts,
    ))
}

async fn readings(
    market: &impl MarketToolReads,
    target: &TrackedInstrument,
    clock: &impl Clock,
) -> (Vec<BriefingSource>, bool) {
    let Ok(window) = CandleWindow::try_new(5) else {
        return (Vec::new(), true);
    };
    let bars = market
        .priced_candles(&target.symbol, target.market, window)
        .await;
    let flow = if target.market == KnownMarket::Kr {
        market.investor_flow(&target.symbol, target.market).await
    } else {
        Ok(None)
    };
    let failed = bars.is_err() || flow.is_err();
    let now = clock.now();
    let evidence_rows = [
        bars.ok()
            .flatten()
            .and_then(|view| evidence::candles(&view, target, now)),
        flow.ok()
            .flatten()
            .and_then(|view| evidence::flows(&view, target, now)),
    ]
    .into_iter()
    .flatten()
    .collect();
    (evidence_rows, failed)
}
