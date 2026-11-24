use crate::{
    DisplayInstrumentName, InstrumentId, Timestamp, UntrustedText, WebSearchCandidateQuery,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceItemId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceItemIdError {
    Invalid,
}

impl EvidenceItemId {
    pub fn try_new(value: &str) -> Result<Self, EvidenceItemIdError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(EvidenceItemIdError::Invalid);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    Quote,
    Financials,
    Disclosure,
    News,
    Supply,
    Thesis,
    InvestorProfile,
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceItem {
    id: EvidenceItemId,
    kind: EvidenceKind,
    collected_at: Timestamp,
    content: UntrustedText,
}

impl EvidenceItem {
    #[must_use]
    pub const fn new(
        id: EvidenceItemId,
        kind: EvidenceKind,
        collected_at: Timestamp,
        content: UntrustedText,
    ) -> Self {
        Self {
            id,
            kind,
            collected_at,
            content,
        }
    }
    #[must_use]
    pub const fn id(&self) -> &EvidenceItemId {
        &self.id
    }
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        self.kind
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
    #[must_use]
    pub const fn content(&self) -> &UntrustedText {
        &self.content
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceBundleError {
    Empty,
    DuplicateId,
    TooMany,
    NotWeb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    instrument: InstrumentId,
    items: Vec<EvidenceItem>,
}

impl EvidenceBundle {
    pub fn try_new(
        instrument: InstrumentId,
        items: Vec<EvidenceItem>,
    ) -> Result<Self, EvidenceBundleError> {
        if items.is_empty() {
            return Err(EvidenceBundleError::Empty);
        }
        let bundle = Self { instrument, items };
        bundle.validate()?;
        Ok(bundle)
    }
    fn validate(&self) -> Result<(), EvidenceBundleError> {
        if self.items.len() > 256 {
            return Err(EvidenceBundleError::TooMany);
        }
        let mut seen = BTreeSet::new();
        if self.items.iter().all(|item| seen.insert(item.id())) {
            Ok(())
        } else {
            Err(EvidenceBundleError::DuplicateId)
        }
    }
    pub fn with_web(&self, web: Vec<EvidenceItem>) -> Result<Self, EvidenceBundleError> {
        if web.iter().any(|item| item.kind() != EvidenceKind::Web) {
            return Err(EvidenceBundleError::NotWeb);
        }
        let mut items = self.items.clone();
        items.extend(web);
        let bundle = Self {
            instrument: self.instrument.clone(),
            items,
        };
        bundle.validate()?;
        Ok(bundle)
    }
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub fn items(&self) -> &[EvidenceItem] {
        &self.items
    }
    #[must_use]
    pub fn get(&self, id: &EvidenceItemId) -> Option<&EvidenceItem> {
        self.items.iter().find(|item| item.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResearchTopic {
    Earnings,
    Guidance,
    Products,
    Competition,
    Regulation,
    Management,
}

impl ResearchTopic {
    pub const ALL: [Self; 6] = [
        Self::Earnings,
        Self::Guidance,
        Self::Products,
        Self::Competition,
        Self::Regulation,
        Self::Management,
    ];
    fn query(self, name: &DisplayInstrumentName) -> WebSearchCandidateQuery {
        let keyword = match self {
            Self::Earnings => "실적",
            Self::Guidance => "전망",
            Self::Products => "신제품",
            Self::Competition => "경쟁사",
            Self::Regulation => "규제",
            Self::Management => "경영진",
        };
        WebSearchCandidateQuery::new(&format!("{} {keyword}", name.as_str()))
    }
}

#[must_use]
pub fn research_search_plan(
    name: &DisplayInstrumentName,
    topics: &[ResearchTopic],
) -> Vec<WebSearchCandidateQuery> {
    let unique: BTreeSet<ResearchTopic> = topics.iter().copied().collect();
    unique.into_iter().map(|topic| topic.query(name)).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        EvidenceBundle, EvidenceBundleError, EvidenceItem, EvidenceItemId, EvidenceItemIdError,
        EvidenceKind, ResearchTopic, research_search_plan,
    };
    use crate::{
        DisplayInstrumentName, InstrumentId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds,
        UntrustedText,
    };

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn item(id: &str, kind: EvidenceKind) -> EvidenceItem {
        EvidenceItem::new(
            EvidenceItemId::try_new(id).expect("ID"),
            kind,
            at(100),
            UntrustedText::try_new("내용").expect("내용"),
        )
    }
    fn instrument() -> InstrumentId {
        InstrumentId::try_new("A005930").expect("종목")
    }

    #[test]
    fn evidence_ids_are_short_safe_tokens() {
        for bad in ["", "공백 있음", "a/b", &"x".repeat(65)] {
            assert_eq!(
                EvidenceItemId::try_new(bad),
                Err(EvidenceItemIdError::Invalid)
            );
        }
        assert_eq!(
            EvidenceItemId::try_new("web-1_a").expect("ID").as_str(),
            "web-1_a"
        );
    }

    #[test]
    fn bundle_requires_items_with_unique_ids_and_web_only_additions() {
        assert_eq!(
            EvidenceBundle::try_new(instrument(), Vec::new()),
            Err(EvidenceBundleError::Empty)
        );
        assert_eq!(
            EvidenceBundle::try_new(
                instrument(),
                vec![
                    item("q1", EvidenceKind::Quote),
                    item("q1", EvidenceKind::News)
                ]
            ),
            Err(EvidenceBundleError::DuplicateId)
        );
        let bundle = EvidenceBundle::try_new(
            instrument(),
            vec![
                item("q1", EvidenceKind::Quote),
                item("t1", EvidenceKind::Thesis),
            ],
        )
        .expect("묶음");
        assert_eq!(
            bundle.with_web(vec![item("n1", EvidenceKind::News)]),
            Err(EvidenceBundleError::NotWeb)
        );
        assert_eq!(
            bundle.with_web(vec![item("q1", EvidenceKind::Web)]),
            Err(EvidenceBundleError::DuplicateId)
        );
        let full = bundle
            .with_web(vec![item("web-1", EvidenceKind::Web)])
            .expect("웹 추가");
        assert_eq!(full.items().len(), 3);
        assert_eq!(full.instrument(), &instrument());
        let web = EvidenceItemId::try_new("web-1").expect("ID");
        let found = full.get(&web).expect("웹 항목");
        assert_eq!(found.kind(), EvidenceKind::Web);
        assert_eq!(found.collected_at(), at(100));
        assert_eq!(found.content().as_untrusted_str(), "내용");
        let many: Vec<_> = (0_u16..257)
            .map(|index| item(&format!("w{index}"), EvidenceKind::Web))
            .collect();
        assert_eq!(
            EvidenceBundle::try_new(instrument(), many),
            Err(EvidenceBundleError::TooMany)
        );
    }

    #[test]
    fn search_plan_uses_only_the_name_and_topic_keywords_once_each() {
        let name = DisplayInstrumentName::try_new("삼성전자").expect("이름");
        let plan = research_search_plan(
            &name,
            &[
                ResearchTopic::Regulation,
                ResearchTopic::Earnings,
                ResearchTopic::Earnings,
            ],
        );
        assert_eq!(
            plan.iter()
                .map(crate::WebSearchCandidateQuery::as_str)
                .collect::<Vec<_>>(),
            vec!["삼성전자 실적", "삼성전자 규제"]
        );
        assert_eq!(
            research_search_plan(&name, &ResearchTopic::ALL).len(),
            ResearchTopic::ALL.len()
        );
        assert_eq!(research_search_plan(&name, &[]).len(), 0);
    }
}
