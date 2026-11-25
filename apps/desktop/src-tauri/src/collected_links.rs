use application::{
    CollectedEntry, CollectedTimes, InformationKey, InformationLinks, NewsDataSettingsRepository,
    PublicationTime, StockAlias, SystemPortError, WriteCompleted,
};
use domain::{InformationSource, InstrumentId, Timestamp, UntrustedText};
use std::fmt;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub trait TitleLinker: Send + Sync {
    fn instruments_in(&self, title: &UntrustedText, aliases: &[StockAlias]) -> Vec<InstrumentId>;
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
        Self { store, linker }
    }
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

impl<S> LinkingTimes<'_, S>
where
    S: InformationLinks + NewsDataSettingsRepository,
{
    async fn link(&self, entry: &CollectedEntry) -> Result<(), SystemPortError> {
        let (Some(linker), Some(key)) = (self.linker, key_of(entry)) else {
            return Ok(());
        };
        let Some(settings) = self.store.load().await? else {
            return Ok(());
        };
        let instruments = linker.instruments_in(title_of(entry), settings.aliases());
        if instruments.is_empty() {
            return Ok(());
        }
        let WriteCompleted = self.store.set_instruments(&key, &instruments).await?;
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
