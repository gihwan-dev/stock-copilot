use application::{
    BriefingSource, CollectedEntry, CollectedItemReads, CollectedKind, PublicationTime,
};
use domain::{InstrumentId, Nanosecond, SpanSeconds, TimeSpan, Timestamp, UntrustedText};
use std::collections::BTreeSet;

pub(super) async fn recent(
    store: &impl CollectedItemReads,
    instruments: &BTreeSet<InstrumentId>,
    now: Timestamp,
) -> Vec<BriefingSource> {
    let Some(since) = TimeSpan::try_new(SpanSeconds::new(604_800), Nanosecond::new(0))
        .ok()
        .and_then(|span| now.checked_sub(span).ok())
    else {
        return Vec::new();
    };
    let collected = tokio::join!(
        store.collected_since(CollectedKind::News, since),
        store.collected_since(CollectedKind::Disclosure, since)
    );
    let mut facts = <[_; 2]>::from(collected)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|item| item.instruments.iter().any(|id| instruments.contains(id)))
        .filter_map(|item| {
            let PublicationTime::Known(published) = item.published_at else {
                return None;
            };
            if !(since..=now).contains(&published)
                || item.collected_at > now
                || published > item.collected_at
            {
                return None;
            }
            let article = match &item.entry {
                CollectedEntry::News(value) => value.collected(),
                CollectedEntry::Disclosure(value) => value.collected(),
            };
            let at = crate::ipc::briefing::kst_iso(published)?;
            let title = UntrustedText::try_new(&format!(
                "{} · 발표 {at}",
                article.title().as_untrusted_str()
            ))
            .ok()?;
            Some((
                published,
                BriefingSource {
                    title,
                    observed_at: item.collected_at,
                    url: Some(article.url().clone()),
                    origin: article.source().clone(),
                },
            ))
        })
        .collect::<Vec<_>>();
    facts.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut urls = BTreeSet::new();
    facts
        .into_iter()
        .map(|(_, source)| source)
        .filter(|source| urls.insert(source.url.clone()))
        .take(6)
        .collect()
}
