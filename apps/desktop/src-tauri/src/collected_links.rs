use crate::news_tracking::MatchName;
use application::{
    CollectedCount, CollectedEntry, CollectedItemReads, CollectedKind, CollectedTimes,
    InformationKey, InformationLinks, LinkProbe, NewsDataSettings, NewsDataSettingsRepository,
    PublicationTime, StockAlias, SystemPortError, WriteCompleted,
};
use domain::{InformationSource, InstrumentId, NormalizedUrl, Timestamp, UntrustedText};
use sources::NewsLinkHints;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod relink_tests;

pub trait TitleLinker: Send + Sync {
    fn instruments_in(&self, title: &UntrustedText, aliases: &[StockAlias]) -> Vec<InstrumentId>;

    fn instruments_named(&self, _title: &UntrustedText, _names: &[MatchName]) -> Vec<InstrumentId> {
        Vec::new()
    }

    fn probe_named(&self, _title: &UntrustedText, _names: &[MatchName]) -> NameProbe {
        NameProbe::Quiet
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProbe {
    Quiet,
    NeedsContext,
    GroupOnly,
}

#[derive(Debug, Default)]
struct ProbeCounts {
    name_seen: AtomicU32,
    matched: AtomicU32,
    needs_context: AtomicU32,
    group_only: AtomicU32,
    boundary: AtomicU32,
}

fn bump(counter: &AtomicU32) {
    let _previous = counter.fetch_add(1, Ordering::Relaxed);
}

#[derive(Clone)]
pub struct CollectedLinker(pub Arc<dyn TitleLinker>);

impl fmt::Debug for CollectedLinker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CollectedLinker")
    }
}

pub struct LinkingTimes<'a, S> {
    store: &'a S,
    linker: Option<&'a dyn TitleLinker>,
    names: Option<&'a [MatchName]>,
    hints: Option<&'a NewsLinkHints>,
    linked: AtomicU32,
    probe: ProbeCounts,
}

impl<S> fmt::Debug for LinkingTimes<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkingTimes")
            .field("linking", &self.linker.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a, S> LinkingTimes<'a, S> {
    #[must_use]
    pub const fn new(store: &'a S, linker: Option<&'a dyn TitleLinker>) -> Self {
        Self {
            store,
            linker,
            names: None,
            hints: None,
            linked: AtomicU32::new(0),
            probe: ProbeCounts {
                name_seen: AtomicU32::new(0),
                matched: AtomicU32::new(0),
                needs_context: AtomicU32::new(0),
                group_only: AtomicU32::new(0),
                boundary: AtomicU32::new(0),
            },
        }
    }

    #[must_use]
    pub const fn with_names(
        mut self,
        names: &'a [MatchName],
        hints: Option<&'a NewsLinkHints>,
    ) -> Self {
        self.names = Some(names);
        self.hints = hints;
        self
    }

    #[must_use]
    pub fn linked(&self) -> u32 {
        self.linked.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn probe(&self) -> LinkProbe {
        let read = |counter: &AtomicU32| CollectedCount::new(counter.load(Ordering::Relaxed));
        LinkProbe {
            name_seen: read(&self.probe.name_seen),
            matched: read(&self.probe.matched),
            needs_context: read(&self.probe.needs_context),
            group_only: read(&self.probe.group_only),
            boundary: read(&self.probe.boundary),
        }
    }
}

pub trait TitleProbe: Send + Sync {
    fn inspect(&self, entry: &CollectedEntry);
}

impl<S: Sync> TitleProbe for LinkingTimes<'_, S> {
    fn inspect(&self, entry: &CollectedEntry) {
        let (Some(linker), Some(names)) = (self.linker, self.names) else {
            return;
        };
        let title = title_of(entry);
        let text = title.as_untrusted_str();
        if !names.iter().any(|name| text.contains(name.name.as_str())) {
            return;
        }
        bump(&self.probe.name_seen);
        if !linker.instruments_named(title, names).is_empty() {
            bump(&self.probe.matched);
            return;
        }
        match linker.probe_named(title, names) {
            NameProbe::Quiet => bump(&self.probe.boundary),
            NameProbe::NeedsContext => bump(&self.probe.needs_context),
            NameProbe::GroupOnly => bump(&self.probe.group_only),
        }
    }
}

const fn url_of(entry: &CollectedEntry) -> &NormalizedUrl {
    match entry {
        CollectedEntry::News(item) => item.collected().url(),
        CollectedEntry::Disclosure(item) => item.collected().url(),
    }
}

pub async fn relink_recent<S: CollectedItemReads + InformationLinks>(
    store: &S,
    linker: &dyn TitleLinker,
    names: &[MatchName],
    since: Timestamp,
) -> Result<u32, SystemPortError> {
    if names.is_empty() {
        return Ok(0);
    }
    let mut changed = 0_u32;
    for kind in [CollectedKind::News, CollectedKind::Disclosure] {
        for item in store.collected_since(kind, since).await? {
            let found = linker.instruments_named(title_of(&item.entry), names);
            let mut merged: BTreeSet<InstrumentId> = item.instruments.iter().cloned().collect();
            let before = merged.len();
            merged.extend(found);
            let Some(key) = key_of(&item.entry) else {
                continue;
            };
            if merged.len() == before {
                continue;
            }
            let all: Vec<InstrumentId> = merged.into_iter().collect();
            let WriteCompleted = store.set_instruments(&key, &all).await?;
            changed = changed.saturating_add(1);
        }
    }
    Ok(changed)
}

fn key_of(entry: &CollectedEntry) -> Option<InformationKey> {
    match entry {
        CollectedEntry::News(item) => match item.collected().source() {
            InformationSource::News(source) => Some(InformationKey::News(
                source.clone(),
                item.collected().url().clone(),
            )),
            InformationSource::Web | InformationSource::Disclosure(_) => None,
        },
        CollectedEntry::Disclosure(item) => Some(InformationKey::Disclosure(item.key().clone())),
    }
}

const fn title_of(entry: &CollectedEntry) -> &UntrustedText {
    match entry {
        CollectedEntry::News(item) => item.collected().title(),
        CollectedEntry::Disclosure(item) => item.collected().title(),
    }
}

fn named(
    linker: &dyn TitleLinker,
    entry: &CollectedEntry,
    names: &[MatchName],
    hints: Option<&NewsLinkHints>,
) -> Vec<InstrumentId> {
    let mut found: BTreeSet<InstrumentId> = linker
        .instruments_named(title_of(entry), names)
        .into_iter()
        .collect();
    if let Some(known) = hints {
        found.extend(known.take(url_of(entry)));
    }
    found.into_iter().collect()
}

impl<S> LinkingTimes<'_, S>
where
    S: InformationLinks + NewsDataSettingsRepository,
{
    async fn link(&self, entry: &CollectedEntry) -> Result<(), SystemPortError> {
        let (Some(linker), Some(key)) = (self.linker, key_of(entry)) else {
            return Ok(());
        };
        let instruments = if let Some(names) = self.names {
            named(linker, entry, names, self.hints)
        } else {
            let settings = self
                .store
                .load()
                .await?
                .unwrap_or_else(NewsDataSettings::initial);
            linker.instruments_in(title_of(entry), settings.aliases())
        };
        if instruments.is_empty() {
            return Ok(());
        }
        let WriteCompleted = self.store.set_instruments(&key, &instruments).await?;
        let _previous = self.linked.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

impl<S> CollectedTimes for LinkingTimes<'_, S>
where
    S: CollectedTimes + InformationLinks + NewsDataSettingsRepository,
{
    async fn record_times(
        &self,
        entry: &CollectedEntry,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let recorded = self
            .store
            .record_times(entry, published_at, collected_at)
            .await?;
        self.link(entry).await?;
        Ok(recorded)
    }
}
