use crate::{
    AuditLog, Clock, SystemPortError, WebCollectionFailure, WebFinding, WebSearchGuard,
    WebSearchGuardError, WriteCompleted,
};
use domain::{
    ApprovedWebSearchQuery, CollectedItem, DisplayInstrumentName, EvidenceBundle,
    EvidenceBundleError, EvidenceItem, EvidenceItemId, EvidenceKind, InformationSource,
    InstrumentId, ResearchTopic, UntrustedText, WebSearchInspection, WebSearchSensitiveContext,
    research_search_plan,
};
use std::collections::BTreeSet;
use std::future::Future;

pub trait ResearchEvidenceSource: Send + Sync {
    fn gather(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<EvidenceItem>, SystemPortError>> + Send;
}

pub trait ResearchWebCollector: Send + Sync {
    fn collect(
        &self,
        queries: &[ApprovedWebSearchQuery],
    ) -> impl Future<Output = Result<Vec<WebFinding>, WebCollectionFailure>> + Send;
}

pub trait ResearchWebStore: Send + Sync {
    fn save(
        &self,
        instrument: &InstrumentId,
        items: &[CollectedItem],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchWebOutcome {
    Collected,
    NoApprovedQuery,
    Failed(WebCollectionFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheredEvidence {
    pub bundle: EvidenceBundle,
    pub web: ResearchWebOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatherEvidenceError {
    Source(SystemPortError),
    Bundle(EvidenceBundleError),
    Guard(WebSearchGuardError),
    Store(SystemPortError),
}

#[derive(Debug)]
pub struct GatherResearchEvidence<'a, Source, Audit: AuditLog, Collector, Store, Time> {
    pub source: &'a Source,
    pub guard: &'a WebSearchGuard<'a, Audit>,
    pub collector: &'a Collector,
    pub store: &'a Store,
    pub clock: &'a Time,
}

fn web_content(item: &CollectedItem) -> UntrustedText {
    let Some(body) = item.body() else {
        return item.title().clone();
    };
    let joined = format!(
        "{}\n{}",
        item.title().as_untrusted_str(),
        body.as_untrusted_str()
    );
    match UntrustedText::try_new(&joined) {
        Ok(text) => text,
        Err(_too_long) => item.title().clone(),
    }
}

impl<Source, Audit, Collector, Store, Time>
    GatherResearchEvidence<'_, Source, Audit, Collector, Store, Time>
where
    Source: ResearchEvidenceSource,
    Audit: AuditLog,
    Collector: ResearchWebCollector,
    Store: ResearchWebStore,
    Time: Clock,
{
    async fn approved(
        &self,
        name: &DisplayInstrumentName,
        topics: &[ResearchTopic],
        context: &WebSearchSensitiveContext,
    ) -> Result<Vec<ApprovedWebSearchQuery>, GatherEvidenceError> {
        let mut approved = Vec::new();
        for candidate in research_search_plan(name, topics) {
            match self
                .guard
                .inspect_code_query(&candidate, context)
                .await
                .map_err(GatherEvidenceError::Guard)?
            {
                WebSearchInspection::Approved(query) => approved.push(query),
                WebSearchInspection::Rejected(_rejected) => {}
            }
        }
        Ok(approved)
    }

    pub async fn execute(
        &self,
        instrument: &InstrumentId,
        name: &DisplayInstrumentName,
        topics: &[ResearchTopic],
        context: &WebSearchSensitiveContext,
    ) -> Result<GatheredEvidence, GatherEvidenceError> {
        let local = self
            .source
            .gather(instrument)
            .await
            .map_err(GatherEvidenceError::Source)?;
        let bundle = EvidenceBundle::try_new(instrument.clone(), local)
            .map_err(GatherEvidenceError::Bundle)?;
        let queries = self.approved(name, topics, context).await?;
        if queries.is_empty() {
            return Ok(GatheredEvidence {
                bundle,
                web: ResearchWebOutcome::NoApprovedQuery,
            });
        }
        let findings = match self.collector.collect(&queries).await {
            Ok(findings) => findings,
            Err(failure) => {
                return Ok(GatheredEvidence {
                    bundle,
                    web: ResearchWebOutcome::Failed(failure),
                });
            }
        };
        let mut urls = BTreeSet::new();
        let items: Vec<CollectedItem> = findings
            .iter()
            .map(WebFinding::collected)
            .filter(|item| {
                item.source() == &InformationSource::Web && urls.insert(item.url().clone())
            })
            .collect();
        self.store
            .save(instrument, &items)
            .await
            .map_err(GatherEvidenceError::Store)?;
        let collected_at = self.clock.now();
        let web = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                EvidenceItemId::try_new(&format!("web-{}", index.saturating_add(1)))
                    .map(|id| {
                        EvidenceItem::new(id, EvidenceKind::Web, collected_at, web_content(item))
                    })
                    .map_err(|_error| GatherEvidenceError::Bundle(EvidenceBundleError::DuplicateId))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GatheredEvidence {
            bundle: bundle.with_web(web).map_err(GatherEvidenceError::Bundle)?,
            web: ResearchWebOutcome::Collected,
        })
    }
}
