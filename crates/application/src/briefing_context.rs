use crate::{
    BriefingContext, BriefingContextSource, BriefingSource, DisclosureRepository, NewsRepository,
    PortfolioRepository, SystemPortError,
};
use domain::{
    AccountId, BriefingKind, CollectedItem, InstrumentId, Symbol, SyncOutcome, Timestamp,
};
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
    pub fn with_disclosures<'a, D: DisclosureRepository>(
        &'a self,
        disclosures: &'a D,
    ) -> impl BriefingContextSource + 'a {
        FilingContext {
            holdings: self,
            disclosures,
        }
    }

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
            evidence.extend(
                items
                    .iter()
                    .take(5)
                    .map(|item| source(item.collected(), now)),
            );
        }
        Ok(BriefingContext {
            instruments,
            evidence,
            code_facts: Vec::new(),
        })
    }
}

fn source(item: &CollectedItem, now: Timestamp) -> BriefingSource {
    BriefingSource {
        title: item.title().clone(),
        url: Some(item.url().clone()),
        origin: item.source().clone(),
        observed_at: now,
    }
}

struct FilingContext<'a, H, D> {
    holdings: &'a H,
    disclosures: &'a D,
}

impl<H: BriefingContextSource, D: DisclosureRepository> BriefingContextSource
    for FilingContext<'_, H, D>
{
    async fn context(
        &self,
        kind: BriefingKind,
        now: Timestamp,
    ) -> Result<BriefingContext, SystemPortError> {
        let mut context = self.holdings.context(kind, now).await?;
        let mut receipts = BTreeSet::new();
        for symbol in &context.instruments {
            let Ok(instrument) = InstrumentId::try_new(symbol.as_str()) else {
                continue;
            };
            let Ok(disclosures) = self
                .disclosures
                .disclosures_for_instrument(&instrument)
                .await
            else {
                continue;
            };
            context.evidence.extend(
                disclosures
                    .iter()
                    .rev()
                    .take(5)
                    .filter(|item| receipts.insert(item.key().clone()))
                    .map(|item| source(item.collected(), now)),
            );
        }
        Ok(context)
    }
}
