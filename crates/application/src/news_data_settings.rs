use crate::{
    CollectionCadence, CollectionMinutes, CollectionPlan, CollectionTarget, DeliveryReport,
    EventBus, PlannedCollection, SystemPortError, WriteCompleted,
};
use domain::{
    AiBodyReadSettingError, AiBodyReadToggle, DataSource, DomainEvent, InstrumentId, NewsSource,
    SourceAiBodyReadSettings, WakeThresholds,
};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSwitch {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewsSettingValueError {
    Empty,
    TooLong,
    InvalidCharacter,
    MissingEmail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecContactSetting(String);

impl SecContactSetting {
    pub fn try_new(value: &str) -> Result<Self, NewsSettingValueError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(NewsSettingValueError::Empty);
        }
        if trimmed.len() > 256 {
            return Err(NewsSettingValueError::TooLong);
        }
        if !trimmed.bytes().all(|byte| (0x20..=0x7E).contains(&byte)) {
            return Err(NewsSettingValueError::InvalidCharacter);
        }
        let has_email = trimmed.split(' ').any(|word| {
            word.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
            })
        });
        if !has_email {
            return Err(NewsSettingValueError::MissingEmail);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AliasName(String);

impl AliasName {
    pub fn try_new(value: &str) -> Result<Self, NewsSettingValueError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(NewsSettingValueError::Empty);
        }
        if trimmed.chars().count() > 32 {
            return Err(NewsSettingValueError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(NewsSettingValueError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockAlias {
    pub instrument: InstrumentId,
    pub alias: AliasName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsDataSettings {
    disabled_sources: BTreeSet<DataSource>,
    body_read: SourceAiBodyReadSettings,
    sec_contact: Option<SecContactSetting>,
    cadence_overrides: BTreeMap<CollectionTarget, CollectionMinutes>,
    wake: WakeThresholds,
    aliases: Vec<StockAlias>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsDataSettingsChange {
    Source(DataSource, SourceSwitch),
    BodyRead(NewsSource, AiBodyReadToggle),
    SecContact(Option<SecContactSetting>),
    Cadence(CollectionTarget, Option<CollectionMinutes>),
    Wake(WakeThresholds),
    AddAlias(StockAlias),
    RemoveAlias(AliasName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsDataSettingsError {
    BodyReadLocked(NewsSource),
    DuplicateAlias,
    TooManyAliases,
    AliasMissing,
    CadenceNotAdjustable(CollectionTarget),
    Storage(SystemPortError),
}

impl NewsDataSettings {
    #[must_use]
    pub fn initial() -> Self {
        Self {
            disabled_sources: BTreeSet::new(),
            body_read: SourceAiBodyReadSettings::all_disabled(),
            sec_contact: None,
            cadence_overrides: BTreeMap::new(),
            wake: WakeThresholds::default(),
            aliases: Vec::new(),
        }
    }

    #[must_use]
    pub fn source_switch(&self, source: DataSource) -> SourceSwitch {
        if self.disabled_sources.contains(&source) {
            SourceSwitch::Off
        } else {
            SourceSwitch::On
        }
    }

    #[must_use]
    pub const fn body_read(&self) -> &SourceAiBodyReadSettings {
        &self.body_read
    }

    #[must_use]
    pub const fn sec_contact(&self) -> Option<&SecContactSetting> {
        self.sec_contact.as_ref()
    }

    #[must_use]
    pub fn cadence_override(&self, target: CollectionTarget) -> Option<CollectionMinutes> {
        self.cadence_overrides.get(&target).copied()
    }

    #[must_use]
    pub const fn wake(&self) -> WakeThresholds {
        self.wake
    }

    #[must_use]
    pub fn aliases(&self) -> &[StockAlias] {
        &self.aliases
    }

    pub fn apply(&mut self, change: NewsDataSettingsChange) -> Result<(), NewsDataSettingsError> {
        match change {
            NewsDataSettingsChange::Source(source, SourceSwitch::Off) => {
                self.disabled_sources.insert(source);
            }
            NewsDataSettingsChange::Source(source, SourceSwitch::On) => {
                self.disabled_sources.remove(&source);
            }
            NewsDataSettingsChange::BodyRead(source, toggle) => {
                self.body_read
                    .set(&source, toggle)
                    .map_err(|error| match error {
                        AiBodyReadSettingError::SourceLockedOff(locked) => {
                            NewsDataSettingsError::BodyReadLocked(locked)
                        }
                    })?;
            }
            NewsDataSettingsChange::SecContact(contact) => self.sec_contact = contact,
            NewsDataSettingsChange::Cadence(target, minutes) => {
                self.set_cadence(target, minutes)?;
            }
            NewsDataSettingsChange::Wake(thresholds) => self.wake = thresholds,
            NewsDataSettingsChange::AddAlias(alias) => {
                self.add_alias(alias)?;
            }
            NewsDataSettingsChange::RemoveAlias(name) => {
                let before = self.aliases.len();
                self.aliases.retain(|entry| entry.alias != name);
                if self.aliases.len() == before {
                    return Err(NewsDataSettingsError::AliasMissing);
                }
            }
        }
        Ok(())
    }

    fn set_cadence(
        &mut self,
        target: CollectionTarget,
        minutes: Option<CollectionMinutes>,
    ) -> Result<(), NewsDataSettingsError> {
        if target == CollectionTarget::TreasuryAndFred {
            return Err(NewsDataSettingsError::CadenceNotAdjustable(target));
        }
        match minutes {
            Some(value) => {
                self.cadence_overrides.insert(target, value);
            }
            None => {
                self.cadence_overrides.remove(&target);
            }
        }
        Ok(())
    }

    fn add_alias(&mut self, alias: StockAlias) -> Result<(), NewsDataSettingsError> {
        if self.aliases.iter().any(|entry| entry.alias == alias.alias) {
            return Err(NewsDataSettingsError::DuplicateAlias);
        }
        if self.aliases.len() >= 500 {
            return Err(NewsDataSettingsError::TooManyAliases);
        }
        self.aliases.push(alias);
        Ok(())
    }

    #[must_use]
    pub fn adjusted_plan(&self, plan: &CollectionPlan) -> CollectionPlan {
        CollectionPlan::from_entries(plan.phase(), self.adjust_plan(plan))
    }

    #[must_use]
    pub fn adjust_plan(&self, plan: &CollectionPlan) -> Vec<PlannedCollection> {
        plan.entries()
            .iter()
            .map(|entry| {
                let cadence = match (entry.cadence, self.cadence_override(entry.target)) {
                    (CollectionCadence::Every(_), Some(minutes)) => {
                        CollectionCadence::Every(minutes)
                    }
                    (cadence, Some(_) | None) => cadence,
                };
                PlannedCollection {
                    target: entry.target,
                    cadence,
                }
            })
            .collect()
    }
}

pub trait NewsDataSettingsRepository: Send + Sync {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send;
    fn store(
        &self,
        settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct GetNewsDataSettings<'a, Repository> {
    pub repository: &'a Repository,
}

impl<Repository: NewsDataSettingsRepository> GetNewsDataSettings<'_, Repository> {
    pub async fn execute(&self) -> Result<NewsDataSettings, SystemPortError> {
        Ok(self
            .repository
            .load()
            .await?
            .unwrap_or_else(NewsDataSettings::initial))
    }
}

#[derive(Debug)]
pub struct UpdateNewsDataSettings<'a, Repository> {
    pub repository: &'a Repository,
    pub events: &'a EventBus,
}

impl<Repository: NewsDataSettingsRepository> UpdateNewsDataSettings<'_, Repository> {
    pub async fn execute(
        &self,
        change: NewsDataSettingsChange,
    ) -> Result<(NewsDataSettings, DeliveryReport), NewsDataSettingsError> {
        let loaded = self
            .repository
            .load()
            .await
            .map_err(NewsDataSettingsError::Storage)?;
        let mut settings = loaded.unwrap_or_else(NewsDataSettings::initial);
        settings.apply(change)?;
        let _written: WriteCompleted = self
            .repository
            .store(&settings)
            .await
            .map_err(NewsDataSettingsError::Storage)?;
        let report = self.events.publish(DomainEvent::SettingsChanged);
        Ok((settings, report))
    }
}
