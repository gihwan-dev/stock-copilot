mod candidates;
mod holdings_news;
use crate::{
    broker_link::BrokerMarketTools,
    ports::{PortTypes, Ports},
};
use application::DashboardCacheKey;
use application::{BriefingSource, DashboardSubject, MarketToolReads, market_overview_line_fact};
use domain::{
    DisplayPrecision, InformationSource, KnownMarket, Symbol, SyncOutcome, Timestamp,
    UntrustedText, display_percent,
};
use std::{collections::BTreeSet, sync::Arc};
use tauri::{AppHandle, Manager, Runtime};

pub(super) struct Book {
    pub(super) holdings_scope: String,
    pub(super) candidates_scope: String,
    held: BTreeSet<Symbol>,
    evidence: Vec<BriefingSource>,
}

fn fact(title: impl AsRef<str>, observed_at: Timestamp) -> Option<BriefingSource> {
    Some(BriefingSource {
        title: UntrustedText::try_new(title.as_ref()).ok()?,
        url: None,
        origin: InformationSource::Web,
        observed_at,
    })
}

pub(super) async fn holdings<P: PortTypes>(ports: &Ports<P>) -> Result<Option<Arc<Book>>, ()> {
    let mut accounts = ports.saved_accounts.current();
    if accounts.is_empty() {
        return Ok(None);
    }
    accounts.sort();
    let mut held = BTreeSet::new();
    let mut evidence = Vec::new();
    let mut account_ids = Vec::new();
    for account in accounts {
        let view = application::GetPortfolioView::new(
            ports.portfolios.as_ref(),
            ports.market_data.as_ref(),
        )
        .execute(&account)
        .await
        .map_err(|_error| ())?;
        let application::StoredView::Recorded(snapshot) = view else {
            return Err(());
        };
        let SyncOutcome::Complete(portfolio) = snapshot.outcome() else {
            return Err(());
        };
        account_ids.push(account.as_i64());
        append_holdings(portfolio, snapshot.recorded_at(), &mut held, &mut evidence)?;
    }
    let candidates_scope = serde_json::to_string(&(
        account_ids,
        held.iter().map(Symbol::as_str).collect::<Vec<_>>(),
    ))
    .map_err(|_error| ())?;
    let holdings_scope = valuation_scope(&candidates_scope, &evidence)?;
    Ok(Some(Arc::new(Book {
        holdings_scope,
        candidates_scope,
        held,
        evidence,
    })))
}

pub(super) fn valuation_scope(inventory: &str, evidence: &[BriefingSource]) -> Result<String, ()> {
    serde_json::to_string(&(
        inventory,
        evidence
            .iter()
            .map(|source| {
                (
                    source.title.as_untrusted_str(),
                    *source.observed_at.utc_seconds().as_i64(),
                )
            })
            .collect::<Vec<_>>(),
    ))
    .map_err(|_error| ())
}

fn append_holdings(
    portfolio: &domain::PortfolioValuation,
    recorded_at: Timestamp,
    held: &mut BTreeSet<Symbol>,
    evidence: &mut Vec<BriefingSource>,
) -> Result<(), ()> {
    let allocation = domain::Allocation::new(portfolio, &domain::SectorAssignments::default());
    if let Ok(weights) = allocation.holdings().instruments() {
        for entry in weights.entries() {
            let weight = super::super::portfolio::weight_text(entry.display());
            let qualifier = match entry.display().range() {
                domain::AllocationDisplayRange::BelowUnit => " 미만",
                domain::AllocationDisplayRange::AtLeastUnit
                | domain::AllocationDisplayRange::Zero => "",
            };
            if let Some(source) = fact(
                format!(
                    "토스증권 계좌 내 보유 평가액 비중 · {} {}%{}",
                    entry.key().symbol().as_str(),
                    weight,
                    qualifier
                ),
                recorded_at,
            ) {
                evidence.push(source);
            }
        }
    }
    for valued in portfolio.holdings() {
        let holding = valued.holding();
        let symbol = holding.instrument().symbol();
        held.insert(symbol.clone());
        let mut title = format!("토스증권 보유 내역 {}", symbol.as_str());
        if let Some(report) = holding.reported() {
            let precision = DisplayPrecision::try_new(2).map_err(|_error| ())?;
            let value = report.valuation();
            let _written = std::fmt::Write::write_fmt(
                &mut title,
                format_args!(
                    " 평가액 {} {:?}, 수익률 {}%",
                    value.amount().as_decimal(),
                    value.currency(),
                    display_percent(&report.return_rate(), precision).as_str()
                ),
            );
        }
        if let Some(source) = fact(title, holding.observed_at()) {
            evidence.push(source);
        }
    }
    Ok(())
}

type Input = Option<(DashboardSubject, Vec<BriefingSource>)>;

pub(super) async fn collect<R: Runtime, P: PortTypes>(
    app: &AppHandle<R>,
    ports: &Ports<P>,
    key: DashboardCacheKey,
    book: Option<&Book>,
    now: Timestamp,
) -> Result<Input, ()> {
    if key == DashboardCacheKey::Holdings {
        let Some(value) = book.filter(|current| !current.evidence.is_empty()) else {
            return Ok(None);
        };
        let mut evidence = value.evidence.clone();
        if let (Some(store), Some(directory)) = (
            app.try_state::<Arc<store_sqlite::SqliteRepositories>>(),
            app.try_state::<crate::holdings_link::InstrumentDirectory>(),
        ) {
            let ids = value
                .held
                .iter()
                .filter_map(|symbol| {
                    directory
                        .entry(symbol)
                        .map(|entry| entry.master.row().instrument_id.clone())
                })
                .collect::<BTreeSet<_>>();
            if !ids.is_empty() {
                evidence.extend(holdings_news::recent(store.inner().as_ref(), &ids, now).await);
            }
        }
        return Ok(Some((DashboardSubject::Holdings, evidence)));
    }
    let Some(market) = app
        .try_state::<BrokerMarketTools>()
        .map(|market| market.inner().clone())
    else {
        return Ok(None);
    };
    match key {
        DashboardCacheKey::Economy => economy(&market).await,
        DashboardCacheKey::Candidates => {
            let Some(value) = book else { return Ok(None) };
            candidates::collect(app, ports, &market, &value.held, now).await
        }
        DashboardCacheKey::Holdings => Ok(None),
    }
}

pub(super) async fn economy(market: &impl MarketToolReads) -> Result<Input, ()> {
    let (overview, domestic, us) = tokio::join!(
        market.market_overview(),
        market.sectors(KnownMarket::Kr),
        market.sectors(KnownMarket::Us)
    );
    let incomplete = overview.is_err() || domestic.is_err() || us.is_err();
    let mut evidence: Vec<_> = overview
        .ok()
        .flatten()
        .into_iter()
        .flat_map(|view| view.lines)
        .filter_map(|line| {
            fact(
                format!(
                    "시장 지표 · {}",
                    market_overview_line_fact(&line)?.as_untrusted_str()
                ),
                line.collected_at,
            )
        })
        .collect();
    let precision = DisplayPrecision::try_new(2).map_err(|_error| ())?;
    for (region, read) in [(KnownMarket::Kr, domestic), (KnownMarket::Us, us)] {
        let Ok(Some(sectors)) = read else {
            continue;
        };
        if sectors.market != region {
            continue;
        }
        let label = match region {
            KnownMarket::Kr => "국내",
            KnownMarket::Us => "미국",
        };
        evidence.extend(
            sectors
                .lines
                .iter()
                .filter_map(|row| {
                    let rate = row.change_rate.as_ref()?;
                    fact(
                        format!(
                            "토스증권 {label} 업종별 등락 · {} {}%",
                            row.label.as_str(),
                            display_percent(rate, precision).as_str()
                        ),
                        sectors.collected_at,
                    )
                })
                .take(8),
        );
    }
    if evidence.is_empty() && incomplete {
        return Err(());
    }
    Ok((!evidence.is_empty()).then_some((DashboardSubject::Economy, evidence)))
}
