use crate::{CollectionQuery, CollectionTopic, DeliveryReport, EventBus, SystemPortError};
use domain::{CollectedItem, DomainEvent, InformationSource, NormalizedUrl, Symbol, UntrustedText};
use std::collections::BTreeSet;
use std::future::Future;

pub const STOCK_WEB_TOPICS: [CollectionTopic; 3] = [
    CollectionTopic::CompanyNews,
    CollectionTopic::Earnings,
    CollectionTopic::Disclosures,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebCollectionReservation {
    instrument: Symbol,
}

impl WebCollectionReservation {
    #[must_use]
    pub const fn instrument(&self) -> &Symbol {
        &self.instrument
    }

    #[must_use]
    pub fn query(&self) -> CollectionQuery {
        CollectionQuery::CodeTopics {
            instruments: vec![self.instrument.clone()],
            topics: STOCK_WEB_TOPICS.to_vec(),
        }
    }
}

#[must_use]
pub fn plan_stock_web_collection(known: &[Symbol]) -> Vec<WebCollectionReservation> {
    let mut seen = BTreeSet::new();
    known
        .iter()
        .filter(|symbol| seen.insert((*symbol).clone()))
        .take(20)
        .map(|symbol| WebCollectionReservation {
            instrument: symbol.clone(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFinding {
    url: NormalizedUrl,
    title: UntrustedText,
    summary: Option<UntrustedText>,
}

impl WebFinding {
    #[must_use]
    pub const fn new(
        url: NormalizedUrl,
        title: UntrustedText,
        summary: Option<UntrustedText>,
    ) -> Self {
        Self {
            url,
            title,
            summary,
        }
    }

    fn into_collected(self) -> CollectedItem {
        CollectedItem::from_web(self.url, self.title, self.summary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebCollectionFailure {
    Unavailable,
    Rejected,
}

pub trait WebCollectionRunner: Send + Sync {
    fn collect(
        &self,
        query: &CollectionQuery,
    ) -> impl Future<Output = Result<Vec<WebFinding>, WebCollectionFailure>> + Send;
}

pub trait WebCollectionStore: Send + Sync {
    fn save(
        &self,
        instrument: &Symbol,
        items: &[CollectedItem],
    ) -> impl Future<Output = Result<crate::WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockWebCollectionResult {
    instrument: Symbol,
    outcome: StockWebCollectionOutcome,
}

impl StockWebCollectionResult {
    #[must_use]
    pub const fn instrument(&self) -> &Symbol {
        &self.instrument
    }

    #[must_use]
    pub const fn outcome(&self) -> &StockWebCollectionOutcome {
        &self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockWebCollectionOutcome {
    Saved(Vec<CollectedItem>),
    RunFailed(WebCollectionFailure),
    StoreFailed(SystemPortError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockWebCollectionReport {
    pub results: Vec<StockWebCollectionResult>,
    pub delivery: Option<DeliveryReport>,
}

#[derive(Debug)]
pub struct RunStockWebCollection<'a, Runner, Store> {
    pub runner: &'a Runner,
    pub store: &'a Store,
    pub events: &'a EventBus,
}

impl<Runner: WebCollectionRunner, Store: WebCollectionStore>
    RunStockWebCollection<'_, Runner, Store>
{
    async fn one(&self, reservation: &WebCollectionReservation) -> StockWebCollectionOutcome {
        let findings = match self.runner.collect(&reservation.query()).await {
            Ok(findings) => findings,
            Err(failure) => return StockWebCollectionOutcome::RunFailed(failure),
        };
        let mut urls = BTreeSet::new();
        let items: Vec<CollectedItem> = findings
            .into_iter()
            .map(WebFinding::into_collected)
            .filter(|item| {
                item.source() == &InformationSource::Web && urls.insert(item.url().clone())
            })
            .collect();
        match self.store.save(reservation.instrument(), &items).await {
            Ok(_written) => StockWebCollectionOutcome::Saved(items),
            Err(error) => StockWebCollectionOutcome::StoreFailed(error),
        }
    }

    pub async fn run(&self, reservations: &[WebCollectionReservation]) -> StockWebCollectionReport {
        let mut results = Vec::with_capacity(reservations.len());
        for reservation in reservations {
            results.push(StockWebCollectionResult {
                instrument: reservation.instrument().clone(),
                outcome: self.one(reservation).await,
            });
        }
        let saved = results.iter().any(|result| match result.outcome {
            StockWebCollectionOutcome::Saved(_) => true,
            StockWebCollectionOutcome::RunFailed(_) | StockWebCollectionOutcome::StoreFailed(_) => {
                false
            }
        });
        let delivery = saved.then(|| {
            self.events
                .publish(DomainEvent::CollectionFinished(InformationSource::Web))
        });
        StockWebCollectionReport { results, delivery }
    }
}
