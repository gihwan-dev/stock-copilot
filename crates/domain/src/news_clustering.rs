use std::collections::BTreeSet;

use crate::{DomainEvent, InstrumentId, NewsItem, NormalizedUrl, Timestamp, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedNews {
    item: NewsItem,
    published_at: Timestamp,
}

impl TimedNews {
    #[must_use]
    pub const fn new(item: NewsItem, published_at: Timestamp) -> Self {
        Self { item, published_at }
    }

    #[must_use]
    pub const fn item(&self) -> &NewsItem {
        &self.item
    }

    #[must_use]
    pub const fn published_at(&self) -> Timestamp {
        self.published_at
    }

    fn url(&self) -> &NormalizedUrl {
        self.item.collected().url()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TitleBigram(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleBigrams(BTreeSet<TitleBigram>);

impl TitleBigrams {
    #[must_use]
    pub fn from_title(title: &UntrustedText) -> Self {
        let chars: Vec<char> = title
            .as_untrusted_str()
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        let pairs = chars
            .iter()
            .zip(chars.iter().skip(1))
            .map(|(first, second)| TitleBigram([*first, *second].iter().collect()))
            .collect();
        Self(pairs)
    }

    #[must_use]
    pub fn similarity(&self, other: &Self) -> TitleSimilarity {
        let shared = self.0.intersection(&other.0).count();
        let union = self.0.union(&other.0).count();
        if union == 0 {
            return TitleSimilarity::Distinct;
        }
        match (shared.checked_mul(100), union.checked_mul(45)) {
            (Some(left), Some(right)) if left >= right => TitleSimilarity::Similar,
            (Some(_) | None, Some(_) | None) => TitleSimilarity::Distinct,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSimilarity {
    Similar,
    Distinct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockNewsCluster {
    instrument: InstrumentId,
    representative: TimedNews,
    representative_bigrams: TitleBigrams,
    members: Vec<TimedNews>,
}

impl StockNewsCluster {
    fn start(instrument: InstrumentId, representative: TimedNews) -> Self {
        let representative_bigrams =
            TitleBigrams::from_title(representative.item().collected().title());
        Self {
            instrument,
            representative,
            representative_bigrams,
            members: Vec::new(),
        }
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub const fn representative(&self) -> &TimedNews {
        &self.representative
    }

    #[must_use]
    pub fn members(&self) -> &[TimedNews] {
        &self.members
    }

    fn contains_url(&self, url: &NormalizedUrl) -> ClusterMembership {
        if self.representative.url() == url || self.members.iter().any(|member| member.url() == url)
        {
            ClusterMembership::Contains
        } else {
            ClusterMembership::Absent
        }
    }

    fn accepts(&self, candidate: &TimedNews, bigrams: &TitleBigrams) -> ClusterMembership {
        let within = within_window(self.representative.published_at, candidate.published_at)
            == ClusterMembership::Contains;
        let similar = self.representative_bigrams.similarity(bigrams) == TitleSimilarity::Similar;
        let room = self.members.len() < 255;
        if within && similar && room {
            ClusterMembership::Contains
        } else {
            ClusterMembership::Absent
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClusterMembership {
    Contains,
    Absent,
}

fn within_window(first: Timestamp, second: Timestamp) -> ClusterMembership {
    let close = first
        .utc_seconds()
        .as_i64()
        .checked_sub(*second.utc_seconds().as_i64())
        .and_then(i64::checked_abs)
        .is_some_and(|gap| gap <= 172_800);
    if close {
        ClusterMembership::Contains
    } else {
        ClusterMembership::Absent
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterDecision {
    DuplicateUrl,
    Joined(InstrumentId),
    Started(InstrumentId),
}

impl ClusterDecision {
    #[must_use]
    pub fn event(&self) -> Option<DomainEvent> {
        match self {
            Self::Joined(instrument) => Some(DomainEvent::NewsClusterGrew(instrument.clone())),
            Self::DuplicateUrl | Self::Started(_) => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewsClusterBook {
    clusters: Vec<StockNewsCluster>,
}

impl NewsClusterBook {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            clusters: Vec::new(),
        }
    }

    #[must_use]
    pub fn clusters(&self) -> &[StockNewsCluster] {
        &self.clusters
    }

    pub fn clusters_for<'book>(
        &'book self,
        instrument: &'book InstrumentId,
    ) -> impl Iterator<Item = &'book StockNewsCluster> {
        self.clusters
            .iter()
            .filter(move |cluster| &cluster.instrument == instrument)
    }

    pub fn admit(&mut self, instrument: &InstrumentId, candidate: TimedNews) -> ClusterDecision {
        if self
            .clusters_for(instrument)
            .any(|cluster| cluster.contains_url(candidate.url()) == ClusterMembership::Contains)
        {
            return ClusterDecision::DuplicateUrl;
        }
        let bigrams = TitleBigrams::from_title(candidate.item().collected().title());
        let target = self.clusters.iter_mut().find(|cluster| {
            &cluster.instrument == instrument
                && cluster.accepts(&candidate, &bigrams) == ClusterMembership::Contains
        });
        if let Some(cluster) = target {
            cluster.members.push(candidate);
            return ClusterDecision::Joined(instrument.clone());
        }
        self.clusters
            .push(StockNewsCluster::start(instrument.clone(), candidate));
        ClusterDecision::Started(instrument.clone())
    }
}

#[must_use]
pub fn dedupe_by_url(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.collected().url().clone()))
        .collect()
}
