use crate::ipc::briefing::kst_iso;
use crate::ipc::event::UiNewsSource;
use application::{
    CollectedEntry, CollectedItemReads, CollectedKind, PublicationTime, group_same_story,
};
use domain::{
    InformationSource, Nanosecond, NewsSource, SpanSeconds, Symbol, TimeSpan, Timestamp,
    UntrustedText,
};
use serde::Serialize;
use specta::Type;

#[cfg(test)]
mod tests;

pub const RECENT_NEWS_SECONDS: u64 = 7_u64 * 86_400_u64;
pub const RECENT_NEWS_LIMIT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentNews {
    pub title: String,
    pub source: UiNewsSource,
    pub published_at: String,
    pub url: String,
}

struct Headline {
    title: UntrustedText,
    source: NewsSource,
    at: Timestamp,
    url: String,
}

fn window_start(now: Timestamp) -> Option<Timestamp> {
    let span = TimeSpan::try_new(SpanSeconds::new(RECENT_NEWS_SECONDS), Nanosecond::new(0)).ok()?;
    now.checked_sub(span).ok()
}

fn headline(item: application::ObservedCollectedItem, symbol: &Symbol) -> Option<Headline> {
    if !item
        .instruments
        .iter()
        .any(|instrument| instrument.as_str() == symbol.as_str())
    {
        return None;
    }
    let CollectedEntry::News(news) = item.entry else {
        return None;
    };
    let InformationSource::News(source) = news.collected().source().clone() else {
        return None;
    };
    let at = match item.published_at {
        PublicationTime::Known(published) => published,
        PublicationTime::NotProvided => item.collected_at,
    };
    Some(Headline {
        title: news.collected().title().clone(),
        source,
        at,
        url: news.collected().url().as_str().to_owned(),
    })
}

pub async fn recent_news<S: CollectedItemReads>(
    store: &S,
    symbol: &Symbol,
    now: Timestamp,
) -> Vec<UiInstrumentNews> {
    let Some(since) = window_start(now) else {
        return Vec::new();
    };
    let Ok(items) = store.collected_since(CollectedKind::News, since).await else {
        return Vec::new();
    };
    let mut headlines: Vec<Headline> = items
        .into_iter()
        .filter_map(|item| headline(item, symbol))
        .collect();
    headlines.sort_by_key(|line| std::cmp::Reverse(line.at));
    group_same_story(headlines, |line| &line.title, |line| line.at)
        .into_iter()
        .take(RECENT_NEWS_LIMIT)
        .filter_map(|group| {
            Some(UiInstrumentNews {
                title: group.representative.title.as_untrusted_str().to_owned(),
                source: group.representative.source.into(),
                published_at: kst_iso(group.representative.at)?,
                url: group.representative.url,
            })
        })
        .collect()
}
