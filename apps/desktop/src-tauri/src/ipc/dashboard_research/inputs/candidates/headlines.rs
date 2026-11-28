use super::evidence::recent;
use crate::{collected_links::TitleLinker, news_tracking::MatchName};
use application::{
    BriefingSource, Clock, CollectedEntry, CollectedItemReads, CollectedKind, CollectedTimes,
    DataAvailability, NewsDataSettings, NewsDataSettingsRepository, NewsRepository,
    ObservedCollectedItem, PublicationTime, SourceSwitch, TrackedInstrument,
};
use domain::{
    DataSource, InstrumentId, Nanosecond, ParsedExternalUrl, SpanSeconds, TimeSpan, Timestamp,
    UntrustedText,
};
use sources::TargetedHeadlines;
use std::collections::BTreeSet;
use store_sqlite::SqliteRepositories;

pub(super) struct Archive {
    items: Vec<ObservedCollectedItem>,
    settings: Option<NewsDataSettings>,
    failed: bool,
}

impl Archive {
    pub(super) async fn load(database: Option<&SqliteRepositories>, now: Timestamp) -> Self {
        let cutoff = TimeSpan::try_new(SpanSeconds::new(604_800), Nanosecond::new(0))
            .ok()
            .and_then(|span| now.checked_sub(span).ok());
        let (Some(store), Some(since)) = (database, cutoff) else {
            return Self {
                items: Vec::new(),
                settings: None,
                failed: true,
            };
        };
        let (news, disclosures, settings) = tokio::join!(
            store.collected_since(CollectedKind::News, since),
            store.collected_since(CollectedKind::Disclosure, since),
            NewsDataSettingsRepository::load(store)
        );
        let failed = news.is_err() || disclosures.is_err() || settings.is_err();
        Self {
            items: news
                .into_iter()
                .flatten()
                .chain(disclosures.into_iter().flatten())
                .collect(),
            settings: settings
                .ok()
                .map(|setting| setting.unwrap_or_else(NewsDataSettings::initial)),
            failed,
        }
    }

    pub(super) async fn for_candidate(
        &self,
        company: &Company<'_>,
        database: Option<
            &(impl NewsRepository + CollectedTimes + application::CollectedPresenceCheck),
        >,
        reader: Option<&impl TargetedHeadlines>,
        clock: &impl Clock,
    ) -> (Vec<BriefingSource>, bool) {
        let mut saved = self.saved(company, clock.now());
        let (Some(store), Some(client), Some(settings)) =
            (database, reader, self.settings.as_ref())
        else {
            return (saved, self.failed);
        };
        if saved
            .iter()
            .find_map(|source| match &source.origin {
                domain::InformationSource::News(agency) => Some(agency),
                domain::InformationSource::Disclosure(_) | domain::InformationSource::Web => None,
            })
            .is_some()
        {
            return (saved, self.failed);
        }
        let Some(search) = search_for(settings, company.target.market) else {
            return (saved, self.failed);
        };
        let found = match client.target_headlines(search, company.target).await {
            Some(Ok(DataAvailability::Available((items, _)))) => items,
            Some(Err(_)) => return (saved, true),
            None | Some(Ok(DataAvailability::NotConfigured | DataAvailability::NoData)) => {
                return (saved, self.failed);
            }
        };
        let now = clock.now();
        let mut selected = found
            .iter()
            .filter_map(|observation| company.confirmed(observation, now))
            .collect::<Vec<_>>();
        selected.sort_by_key(|(published, _, _, _)| std::cmp::Reverse(*published));
        let mut failure = self.failed;
        for (_, observation, item, fact) in selected {
            if saved.len() >= 3 {
                break;
            }
            if saved.iter().any(|source| source.url == fact.url) {
                continue;
            }
            match retain(store, item, observation).await {
                Ok(()) => saved.push(fact),
                Err(_) => failure = true,
            }
        }
        (saved, failure)
    }

    fn saved(&self, company: &Company<'_>, now: Timestamp) -> Vec<BriefingSource> {
        let mut articles = self
            .items
            .iter()
            .filter(|item| company.linked(item))
            .filter_map(|item| {
                let published = match item.published_at {
                    PublicationTime::Known(at) => at,
                    PublicationTime::NotProvided => return None,
                };
                Some((
                    published,
                    article(&item.entry, item.published_at, item.collected_at, now)?,
                ))
            })
            .collect::<Vec<_>>();
        articles.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        let mut seen = BTreeSet::new();
        articles
            .into_iter()
            .map(|(_, source)| source)
            .filter(|source| seen.insert(source.url.clone()))
            .take(3)
            .collect()
    }
}

async fn retain(
    store: &(impl NewsRepository + CollectedTimes + application::CollectedPresenceCheck),
    item: &domain::NewsItem,
    observation: &application::CollectedObservation,
) -> Result<(), application::SystemPortError> {
    if store.presence(item.collected().url()).await? == application::CollectedPresence::New {
        NewsRepository::store(store, item).await?;
        store
            .record_times(
                observation.entry(),
                observation.published_at(),
                observation.collected_at(),
            )
            .await?;
    }
    Ok(())
}

pub(super) struct Company<'a> {
    pub(super) target: &'a TrackedInstrument,
    pub(super) canonical: Option<&'a InstrumentId>,
    pub(super) linker: Option<&'a dyn TitleLinker>,
    pub(super) names: &'a [MatchName],
}

impl Company<'_> {
    fn named(&self, title: &UntrustedText) -> bool {
        self.linker.is_some_and(|linker| {
            self.target
                .link_id()
                .is_some_and(|id| linker.instruments_named(title, self.names).contains(&id))
        })
    }
    fn confirmed<'a>(
        &self,
        observation: &'a application::CollectedObservation,
        now: Timestamp,
    ) -> Option<(
        Timestamp,
        &'a application::CollectedObservation,
        &'a domain::NewsItem,
        BriefingSource,
    )> {
        let CollectedEntry::News(item) = observation.entry() else {
            return None;
        };
        let PublicationTime::Known(published) = observation.published_at() else {
            return None;
        };
        let fact = article(
            observation.entry(),
            observation.published_at(),
            observation.collected_at(),
            now,
        )?;
        self.named(item.collected().title())
            .then_some((published, observation, item, fact))
    }
    fn linked(&self, item: &ObservedCollectedItem) -> bool {
        if self
            .canonical
            .is_some_and(|id| item.instruments.contains(id))
        {
            return true;
        }
        let title = match &item.entry {
            CollectedEntry::News(news) => news.collected().title(),
            CollectedEntry::Disclosure(disclosure) => disclosure.collected().title(),
        };
        self.target
            .link_id()
            .is_some_and(|id| item.instruments.contains(&id))
            && self.named(title)
    }
}

fn search_for(
    settings: &NewsDataSettings,
    market: domain::KnownMarket,
) -> Option<domain::NewsSource> {
    if market == domain::KnownMarket::Us
        && settings.source_switch(DataSource::YahooFinance) == SourceSwitch::On
    {
        return Some(domain::NewsSource::YahooFinance);
    }
    (settings.source_switch(DataSource::GoogleNews) == SourceSwitch::On)
        .then_some(domain::NewsSource::GoogleNews)
}

pub(super) fn article(
    entry: &CollectedEntry,
    published: PublicationTime,
    observed_at: Timestamp,
    now: Timestamp,
) -> Option<BriefingSource> {
    let PublicationTime::Known(at) = published else {
        return None;
    };
    if at > observed_at || observed_at > now || !recent(at, now) {
        return None;
    }
    let (item, label) = match entry {
        CollectedEntry::News(item) => (item.collected(), "기사 발행"),
        CollectedEntry::Disclosure(item) => (item.collected(), "공시 제출"),
    };
    let parsed = ParsedExternalUrl::from_collected(item.url()).ok()?;
    let date = crate::ipc::briefing::kst_iso(at)?;
    Some(BriefingSource {
        title: UntrustedText::try_new(&format!(
            "{} · {label} {date} · {}",
            parsed.host().as_str(),
            item.title().as_untrusted_str()
        ))
        .ok()?,
        url: Some(item.url().clone()),
        origin: item.source().clone(),
        observed_at,
    })
}

#[cfg(test)]
mod tests;
