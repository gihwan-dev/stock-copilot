use std::collections::BTreeSet;

use crate::NewsItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    representative: NewsItem,
    members: Vec<NewsItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterError {
    DuplicateUrl,
}

impl Cluster {
    pub fn try_new(representative: NewsItem, members: Vec<NewsItem>) -> Result<Self, ClusterError> {
        let mut urls = BTreeSet::from([representative.collected().url()]);
        for member in &members {
            if !urls.insert(member.collected().url()) {
                return Err(ClusterError::DuplicateUrl);
            }
        }
        Ok(Self {
            representative,
            members,
        })
    }

    #[must_use]
    pub const fn representative(&self) -> &NewsItem {
        &self.representative
    }

    #[must_use]
    pub fn members(&self) -> &[NewsItem] {
        &self.members
    }
}

#[cfg(test)]
mod tests {
    use super::{Cluster, ClusterError};
    use crate::{NewsItem, NewsSource, NormalizedUrl, UntrustedText};

    #[test]
    fn cluster_is_nonempty_and_keeps_the_representative() {
        let leader = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://example.com/1").expect("주소"),
            UntrustedText::try_new("대표").expect("제목"),
            None,
        );
        let member = NewsItem::new(
            NewsSource::Maekyung,
            NormalizedUrl::try_new("https://example.com/2").expect("주소"),
            UntrustedText::try_new("관련").expect("제목"),
            None,
        );
        let cluster = Cluster::try_new(leader.clone(), vec![member.clone()]).expect("묶음");
        assert_eq!(cluster.representative(), &leader);
        assert_eq!(cluster.members(), &[member]);
        assert_eq!(
            Cluster::try_new(leader.clone(), vec![])
                .expect("대표만")
                .members(),
            &[]
        );
        assert_eq!(
            Cluster::try_new(leader.clone(), vec![leader]),
            Err(ClusterError::DuplicateUrl)
        );
    }
    #[test]
    fn cluster_rejects_duplicate_members_even_when_representative_differs() {
        let representative = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://example.com/1").expect("주소"),
            UntrustedText::try_new("대표").expect("제목"),
            None,
        );
        let member = NewsItem::new(
            NewsSource::Maekyung,
            NormalizedUrl::try_new("https://example.com/2").expect("주소"),
            UntrustedText::try_new("관련").expect("제목"),
            None,
        );
        assert_eq!(
            Cluster::try_new(representative, vec![member.clone(), member]),
            Err(ClusterError::DuplicateUrl)
        );
    }
}
