use crate::{
    BriefingContext, BriefingContextSource, BriefingSource, NewsRepository, PortfolioRepository,
    SystemPortError,
};
use domain::{AccountId, BriefingKind, InstrumentId, Symbol, SyncOutcome, Timestamp};
use std::collections::BTreeSet;
use std::fmt::{self, Debug, Formatter};

pub struct HoldingsBriefingContext<'a, Portfolios, News> {
    pub portfolios: &'a Portfolios,
    pub news: &'a News,
    pub accounts: &'a [AccountId],
}

impl<Portfolios, News> Debug for HoldingsBriefingContext<'_, Portfolios, News> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("HoldingsBriefingContext")
            .finish_non_exhaustive()
    }
}

impl<Portfolios: PortfolioRepository, News: NewsRepository>
    HoldingsBriefingContext<'_, Portfolios, News>
{
    async fn held_symbols(&self) -> Result<Vec<Symbol>, SystemPortError> {
        let mut seen = BTreeSet::new();
        for account in self.accounts {
            let Some(record) = self.portfolios.latest(account).await? else {
                continue;
            };
            let portfolio = match record.outcome() {
                SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => value,
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => continue,
            };
            for holding in portfolio.holdings() {
                let _inserted = seen.insert(holding.instrument().symbol().clone());
            }
        }
        Ok(seen.into_iter().take(20).collect())
    }
}

impl<Portfolios: PortfolioRepository, News: NewsRepository> BriefingContextSource
    for HoldingsBriefingContext<'_, Portfolios, News>
{
    async fn context(
        &self,
        _kind: BriefingKind,
        now: Timestamp,
    ) -> Result<BriefingContext, SystemPortError> {
        let instruments = self.held_symbols().await?;
        let mut evidence = Vec::new();
        for symbol in &instruments {
            let Ok(instrument) = InstrumentId::try_new(symbol.as_str()) else {
                continue;
            };
            let items = self.news.for_instrument(&instrument).await?;
            evidence.extend(items.iter().take(5).map(|item| {
                let collected = item.collected();
                BriefingSource {
                    title: collected.title().clone(),
                    url: Some(collected.url().clone()),
                    origin: collected.source().clone(),
                    observed_at: now,
                }
            }));
        }
        Ok(BriefingContext {
            instruments,
            evidence,
            code_facts: Vec::new(),
        })
    }
}
