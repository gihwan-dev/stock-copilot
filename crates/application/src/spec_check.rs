use crate::{
    DeliveryReport, EventBus, IdentifierError, RetryDelay, SystemPortError, WriteCompleted,
};
use domain::{DomainEvent, Timestamp};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecHash(String);

impl SpecHash {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecVersionLabel(String);

impl SpecVersionLabel {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b' ')
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpLastModified(String);

impl HttpLastModified {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.len() > 128
            || !trimmed
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecChangeReason {
    NormalizedHash,
    VersionsCatalog,
    LastModified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaReviewState {
    NotRequired,
    PendingTypeDiffReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecFetchError {
    Network,
    ServerError,
    RateLimited(RetryDelay),
    AccessDenied,
    InvalidResponse,
    ResponseTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDocumentSnapshot {
    normalized_hash: SpecHash,
    version: Option<SpecVersionLabel>,
    last_modified: Option<HttpLastModified>,
}

impl SpecDocumentSnapshot {
    #[must_use]
    pub const fn new(
        normalized_hash: SpecHash,
        version: Option<SpecVersionLabel>,
        last_modified: Option<HttpLastModified>,
    ) -> Self {
        Self {
            normalized_hash,
            version,
            last_modified,
        }
    }

    #[must_use]
    pub const fn normalized_hash(&self) -> &SpecHash {
        &self.normalized_hash
    }

    #[must_use]
    pub const fn version(&self) -> Option<&SpecVersionLabel> {
        self.version.as_ref()
    }

    #[must_use]
    pub const fn last_modified(&self) -> Option<&HttpLastModified> {
        self.last_modified.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossSpecBaseline {
    raw_spec_hash: SpecHash,
    normalized_spec_hash: SpecHash,
    version_label: SpecVersionLabel,
    versions_hash: Option<SpecHash>,
    spec_last_modified: Option<HttpLastModified>,
    versions_last_modified: Option<HttpLastModified>,
}

impl TossSpecBaseline {
    #[must_use]
    pub const fn new(
        raw_spec_hash: SpecHash,
        normalized_spec_hash: SpecHash,
        version_label: SpecVersionLabel,
    ) -> Self {
        Self {
            raw_spec_hash,
            normalized_spec_hash,
            version_label,
            versions_hash: None,
            spec_last_modified: None,
            versions_last_modified: None,
        }
    }

    #[must_use]
    pub fn with_versions_hash(mut self, hash: SpecHash) -> Self {
        self.versions_hash = Some(hash);
        self
    }

    #[must_use]
    pub fn with_spec_last_modified(mut self, value: HttpLastModified) -> Self {
        self.spec_last_modified = Some(value);
        self
    }

    #[must_use]
    pub fn with_versions_last_modified(mut self, value: HttpLastModified) -> Self {
        self.versions_last_modified = Some(value);
        self
    }

    #[must_use]
    pub const fn raw_spec_hash(&self) -> &SpecHash {
        &self.raw_spec_hash
    }

    #[must_use]
    pub const fn normalized_spec_hash(&self) -> &SpecHash {
        &self.normalized_spec_hash
    }

    #[must_use]
    pub const fn version_label(&self) -> &SpecVersionLabel {
        &self.version_label
    }

    #[must_use]
    pub const fn versions_hash(&self) -> Option<&SpecHash> {
        self.versions_hash.as_ref()
    }

    #[must_use]
    pub const fn spec_last_modified(&self) -> Option<&HttpLastModified> {
        self.spec_last_modified.as_ref()
    }

    #[must_use]
    pub const fn versions_last_modified(&self) -> Option<&HttpLastModified> {
        self.versions_last_modified.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossSpecObservation {
    spec: SpecDocumentSnapshot,
    versions: SpecDocumentSnapshot,
}

impl TossSpecObservation {
    #[must_use]
    pub const fn new(spec: SpecDocumentSnapshot, versions: SpecDocumentSnapshot) -> Self {
        Self { spec, versions }
    }

    #[must_use]
    pub const fn spec(&self) -> &SpecDocumentSnapshot {
        &self.spec
    }

    #[must_use]
    pub const fn versions(&self) -> &SpecDocumentSnapshot {
        &self.versions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecDriftStatus {
    Aligned,
    DriftDetected {
        reasons: Vec<SpecChangeReason>,
        review: SchemaReviewState,
    },
    CheckFailed(SpecFetchError),
}

impl SpecDriftStatus {
    #[must_use]
    pub const fn review_state(&self) -> SchemaReviewState {
        match self {
            Self::Aligned | Self::CheckFailed(_) => SchemaReviewState::NotRequired,
            Self::DriftDetected { review, .. } => *review,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecStatusRecord {
    checked_at: Timestamp,
    baseline: TossSpecBaseline,
    observed: Option<TossSpecObservation>,
    status: SpecDriftStatus,
}

impl SpecStatusRecord {
    #[must_use]
    pub const fn new(
        checked_at: Timestamp,
        baseline: TossSpecBaseline,
        observed: Option<TossSpecObservation>,
        status: SpecDriftStatus,
    ) -> Self {
        Self {
            checked_at,
            baseline,
            observed,
            status,
        }
    }

    #[must_use]
    pub const fn checked_at(&self) -> Timestamp {
        self.checked_at
    }

    #[must_use]
    pub const fn baseline(&self) -> &TossSpecBaseline {
        &self.baseline
    }

    #[must_use]
    pub const fn observed(&self) -> Option<&TossSpecObservation> {
        self.observed.as_ref()
    }

    #[must_use]
    pub const fn status(&self) -> &SpecDriftStatus {
        &self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecCheckReport {
    pub record: SpecStatusRecord,
    pub written: WriteCompleted,
    pub delivery: Option<DeliveryReport>,
}

pub trait TossSpecSource: Send + Sync {
    fn fetch(&self) -> impl Future<Output = Result<TossSpecObservation, SpecFetchError>> + Send;
}

pub trait TossSpecStatusRepository: Send + Sync {
    fn record(
        &self,
        status: &SpecStatusRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;

    fn latest(
        &self,
    ) -> impl Future<Output = Result<Option<SpecStatusRecord>, SystemPortError>> + Send;
}

#[must_use]
pub fn evaluate_spec_drift(
    baseline: &TossSpecBaseline,
    previous_aligned: Option<&TossSpecObservation>,
    observed: &TossSpecObservation,
) -> SpecDriftStatus {
    let mut reasons = Vec::new();
    if observed.spec().normalized_hash() != baseline.normalized_spec_hash() {
        reasons.push(SpecChangeReason::NormalizedHash);
    }
    let expected_versions_hash = baseline
        .versions_hash()
        .or_else(|| previous_aligned.map(|item| item.versions().normalized_hash()));
    let versions_hash_changed = expected_versions_hash
        .is_some_and(|expected| expected != observed.versions().normalized_hash());
    let versions_label_changed = observed
        .versions()
        .version()
        .is_some_and(|current| current != baseline.version_label());
    if versions_hash_changed || versions_label_changed {
        reasons.push(SpecChangeReason::VersionsCatalog);
    }
    let expected_spec_lm = baseline.spec_last_modified().or_else(|| {
        let item = previous_aligned?;
        item.spec().last_modified()
    });
    let spec_lm_changed = expected_spec_lm.is_some_and(|expected| {
        observed
            .spec()
            .last_modified()
            .is_some_and(|actual| actual != expected)
    });
    let expected_versions_lm = baseline.versions_last_modified().or_else(|| {
        let item = previous_aligned?;
        item.versions().last_modified()
    });
    let versions_lm_changed = expected_versions_lm.is_some_and(|expected| {
        observed
            .versions()
            .last_modified()
            .is_some_and(|actual| actual != expected)
    });
    if spec_lm_changed || versions_lm_changed {
        reasons.push(SpecChangeReason::LastModified);
    }
    if reasons.is_empty() {
        SpecDriftStatus::Aligned
    } else {
        SpecDriftStatus::DriftDetected {
            reasons,
            review: SchemaReviewState::PendingTypeDiffReview,
        }
    }
}

#[derive(Debug)]
pub struct CheckTossSpec<'a, Source, Repository, Time> {
    source: &'a Source,
    repository: &'a Repository,
    clock: &'a Time,
    events: &'a EventBus,
}

impl<'a, Source, Repository, Time> CheckTossSpec<'a, Source, Repository, Time>
where
    Source: TossSpecSource,
    Repository: TossSpecStatusRepository,
    Time: crate::Clock,
{
    #[must_use]
    pub const fn new(
        source: &'a Source,
        repository: &'a Repository,
        clock: &'a Time,
        events: &'a EventBus,
    ) -> Self {
        Self {
            source,
            repository,
            clock,
            events,
        }
    }

    pub async fn check(
        &self,
        baseline: &TossSpecBaseline,
    ) -> Result<SpecCheckReport, SystemPortError> {
        let previous = self.repository.latest().await?;
        let previous_aligned = previous
            .as_ref()
            .filter(|record| record.status() == &SpecDriftStatus::Aligned)
            .and_then(SpecStatusRecord::observed);
        let (observed, status) = match self.source.fetch().await {
            Ok(observation) => {
                let evaluated = evaluate_spec_drift(baseline, previous_aligned, &observation);
                (Some(observation), evaluated)
            }
            Err(error) => (None, SpecDriftStatus::CheckFailed(error)),
        };
        let record = SpecStatusRecord::new(self.clock.now(), baseline.clone(), observed, status);
        let written = self.repository.record(&record).await?;
        let changed = match previous {
            Some(prior) => prior.status() != record.status(),
            None => record.status() != &SpecDriftStatus::Aligned,
        };
        let delivery = changed.then(|| self.events.publish(DomainEvent::SelfCheckChanged));
        Ok(SpecCheckReport {
            record,
            written,
            delivery,
        })
    }
}
