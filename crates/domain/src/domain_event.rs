use crate::{AccountId, ClientOrderId, DisclosureKey, InstrumentId, KnownMarket, ProposalId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    PortfolioSynced(AccountId),
    HoldingsSyncFailed(AccountId),
    BuyingPowerSyncFailed(AccountId),
    TransactionsSynced(AccountId),
    PriceUpdated(InstrumentId),
    PriceMoved(InstrumentId),
    DisclosureArrived(DisclosureKey),
    NewsActivityIncreased(InstrumentId),
    ProposalCreated(ProposalId),
    ProposalExpired(ProposalId),
    ProposalApproved(ProposalId),
    ProposalRejected(ProposalId),
    ProposalSkipped(ProposalId),
    OrderSubmitted(ClientOrderId),
    OrderOutcomeUnknown(ClientOrderId),
    BrokerAccessBlocked,
    BrokerTokenExpired,
    BrokerUserActionRequired,
    SettingsChanged,
    PolicyChanged,
    OrderSwitchChanged,
    WatchlistChanged,
    ThesisUpdated(InstrumentId),
    SelfCheckChanged,
    MarketSessionChanged(KnownMarket),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainEventKind {
    PortfolioSynced,
    HoldingsSyncFailed,
    BuyingPowerSyncFailed,
    TransactionsSynced,
    PriceUpdated,
    PriceMoved,
    DisclosureArrived,
    NewsActivityIncreased,
    ProposalCreated,
    ProposalExpired,
    ProposalApproved,
    ProposalRejected,
    ProposalSkipped,
    OrderSubmitted,
    OrderOutcomeUnknown,
    BrokerAccessBlocked,
    BrokerTokenExpired,
    BrokerUserActionRequired,
    SettingsChanged,
    PolicyChanged,
    OrderSwitchChanged,
    WatchlistChanged,
    ThesisUpdated,
    SelfCheckChanged,
    MarketSessionChanged,
}

impl DomainEvent {
    #[must_use]
    pub const fn kind(&self) -> DomainEventKind {
        match self {
            Self::PortfolioSynced(_) => DomainEventKind::PortfolioSynced,
            Self::HoldingsSyncFailed(_) => DomainEventKind::HoldingsSyncFailed,
            Self::BuyingPowerSyncFailed(_) => DomainEventKind::BuyingPowerSyncFailed,
            Self::TransactionsSynced(_) => DomainEventKind::TransactionsSynced,
            Self::PriceUpdated(_) => DomainEventKind::PriceUpdated,
            Self::PriceMoved(_) => DomainEventKind::PriceMoved,
            Self::DisclosureArrived(_) => DomainEventKind::DisclosureArrived,
            Self::NewsActivityIncreased(_) => DomainEventKind::NewsActivityIncreased,
            Self::ProposalCreated(_) => DomainEventKind::ProposalCreated,
            Self::ProposalExpired(_) => DomainEventKind::ProposalExpired,
            Self::ProposalApproved(_) => DomainEventKind::ProposalApproved,
            Self::ProposalRejected(_) => DomainEventKind::ProposalRejected,
            Self::ProposalSkipped(_) => DomainEventKind::ProposalSkipped,
            Self::OrderSubmitted(_) => DomainEventKind::OrderSubmitted,
            Self::OrderOutcomeUnknown(_) => DomainEventKind::OrderOutcomeUnknown,
            Self::BrokerAccessBlocked => DomainEventKind::BrokerAccessBlocked,
            Self::BrokerTokenExpired => DomainEventKind::BrokerTokenExpired,
            Self::BrokerUserActionRequired => DomainEventKind::BrokerUserActionRequired,
            Self::SettingsChanged => DomainEventKind::SettingsChanged,
            Self::PolicyChanged => DomainEventKind::PolicyChanged,
            Self::OrderSwitchChanged => DomainEventKind::OrderSwitchChanged,
            Self::WatchlistChanged => DomainEventKind::WatchlistChanged,
            Self::ThesisUpdated(_) => DomainEventKind::ThesisUpdated,
            Self::SelfCheckChanged => DomainEventKind::SelfCheckChanged,
            Self::MarketSessionChanged(_) => DomainEventKind::MarketSessionChanged,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, ClientOrderId, Disclosure, DomainEvent, DomainEventKind, InstrumentId,
        KnownMarket, NormalizedUrl, ProposalId, ReceiptNo, UntrustedText,
    };
    use std::collections::BTreeSet;
    fn cases_0() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let account = AccountId::new(17);
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        vec![
            (
                DomainEvent::PortfolioSynced(account.clone()),
                DomainEventKind::PortfolioSynced,
                "PortfolioSynced",
            ),
            (
                DomainEvent::HoldingsSyncFailed(account.clone()),
                DomainEventKind::HoldingsSyncFailed,
                "HoldingsSyncFailed",
            ),
            (
                DomainEvent::BuyingPowerSyncFailed(account.clone()),
                DomainEventKind::BuyingPowerSyncFailed,
                "BuyingPowerSyncFailed",
            ),
            (
                DomainEvent::TransactionsSynced(account),
                DomainEventKind::TransactionsSynced,
                "TransactionsSynced",
            ),
            (
                DomainEvent::PriceUpdated(instrument),
                DomainEventKind::PriceUpdated,
                "PriceUpdated",
            ),
        ]
    }
    fn cases_1() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        let disclosure = Disclosure::new(
            ReceiptNo::Dart(
                crate::DartReceiptNo::try_new("20261114000001").expect("가짜 접수 번호"),
            ),
            NormalizedUrl::try_new("https://example.com/disclosure").expect("가짜 주소"),
            UntrustedText::try_new("이벤트에 실리지 않는 제목").expect("가짜 제목"),
            None,
        )
        .key()
        .clone();
        let proposal = ProposalId::new(ClientOrderId::try_new("proposal-7").expect("가짜 제안 키"));
        vec![
            (
                DomainEvent::PriceMoved(instrument.clone()),
                DomainEventKind::PriceMoved,
                "PriceMoved",
            ),
            (
                DomainEvent::DisclosureArrived(disclosure),
                DomainEventKind::DisclosureArrived,
                "DisclosureArrived",
            ),
            (
                DomainEvent::NewsActivityIncreased(instrument),
                DomainEventKind::NewsActivityIncreased,
                "NewsActivityIncreased",
            ),
            (
                DomainEvent::ProposalCreated(proposal.clone()),
                DomainEventKind::ProposalCreated,
                "ProposalCreated",
            ),
            (
                DomainEvent::ProposalExpired(proposal),
                DomainEventKind::ProposalExpired,
                "ProposalExpired",
            ),
        ]
    }
    fn cases_2() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let proposal = ProposalId::new(ClientOrderId::try_new("proposal-7").expect("가짜 제안 키"));
        let order = ClientOrderId::try_new("order-7").expect("가짜 주문 키");
        vec![
            (
                DomainEvent::ProposalApproved(proposal.clone()),
                DomainEventKind::ProposalApproved,
                "ProposalApproved",
            ),
            (
                DomainEvent::ProposalRejected(proposal.clone()),
                DomainEventKind::ProposalRejected,
                "ProposalRejected",
            ),
            (
                DomainEvent::ProposalSkipped(proposal),
                DomainEventKind::ProposalSkipped,
                "ProposalSkipped",
            ),
            (
                DomainEvent::OrderSubmitted(order.clone()),
                DomainEventKind::OrderSubmitted,
                "OrderSubmitted",
            ),
            (
                DomainEvent::OrderOutcomeUnknown(order),
                DomainEventKind::OrderOutcomeUnknown,
                "OrderOutcomeUnknown",
            ),
        ]
    }
    fn cases_3() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        vec![
            (
                DomainEvent::BrokerAccessBlocked,
                DomainEventKind::BrokerAccessBlocked,
                "BrokerAccessBlocked",
            ),
            (
                DomainEvent::BrokerTokenExpired,
                DomainEventKind::BrokerTokenExpired,
                "BrokerTokenExpired",
            ),
            (
                DomainEvent::BrokerUserActionRequired,
                DomainEventKind::BrokerUserActionRequired,
                "BrokerUserActionRequired",
            ),
            (
                DomainEvent::SettingsChanged,
                DomainEventKind::SettingsChanged,
                "SettingsChanged",
            ),
            (
                DomainEvent::PolicyChanged,
                DomainEventKind::PolicyChanged,
                "PolicyChanged",
            ),
        ]
    }
    fn cases_4() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        let market = KnownMarket::Kr;
        vec![
            (
                DomainEvent::OrderSwitchChanged,
                DomainEventKind::OrderSwitchChanged,
                "OrderSwitchChanged",
            ),
            (
                DomainEvent::WatchlistChanged,
                DomainEventKind::WatchlistChanged,
                "WatchlistChanged",
            ),
            (
                DomainEvent::ThesisUpdated(instrument),
                DomainEventKind::ThesisUpdated,
                "ThesisUpdated",
            ),
            (
                DomainEvent::SelfCheckChanged,
                DomainEventKind::SelfCheckChanged,
                "SelfCheckChanged",
            ),
            (
                DomainEvent::MarketSessionChanged(market),
                DomainEventKind::MarketSessionChanged,
                "MarketSessionChanged",
            ),
        ]
    }
    fn cases() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        [cases_0(), cases_1(), cases_2(), cases_3(), cases_4()].concat()
    }
    #[test]
    fn every_variant_has_its_own_kind_and_only_id_debug() {
        let cases = cases();
        let mut kinds = BTreeSet::new();
        for (event, expected, name) in &cases {
            assert_eq!(event.kind(), *expected, "변형별 종류 계약: {name}");
            assert!(
                kinds.insert(format!("{expected:?}")),
                "종류는 일대일: {name}"
            );
            assert_eq!(event.clone(), *event, "복제는 ID를 보존: {name}");
            let expected_debug = match event {
                DomainEvent::PortfolioSynced(id) => format!("PortfolioSynced({id:?})"),
                DomainEvent::HoldingsSyncFailed(id) => format!("HoldingsSyncFailed({id:?})"),
                DomainEvent::BuyingPowerSyncFailed(id) => format!("BuyingPowerSyncFailed({id:?})"),
                DomainEvent::TransactionsSynced(id) => format!("TransactionsSynced({id:?})"),
                DomainEvent::PriceUpdated(id) => format!("PriceUpdated({id:?})"),
                DomainEvent::PriceMoved(id) => format!("PriceMoved({id:?})"),
                DomainEvent::DisclosureArrived(id) => format!("DisclosureArrived({id:?})"),
                DomainEvent::NewsActivityIncreased(id) => format!("NewsActivityIncreased({id:?})"),
                DomainEvent::ProposalCreated(id) => format!("ProposalCreated({id:?})"),
                DomainEvent::ProposalExpired(id) => format!("ProposalExpired({id:?})"),
                DomainEvent::ProposalApproved(id) => format!("ProposalApproved({id:?})"),
                DomainEvent::ProposalRejected(id) => format!("ProposalRejected({id:?})"),
                DomainEvent::ProposalSkipped(id) => format!("ProposalSkipped({id:?})"),
                DomainEvent::OrderSubmitted(id) => format!("OrderSubmitted({id:?})"),
                DomainEvent::OrderOutcomeUnknown(id) => format!("OrderOutcomeUnknown({id:?})"),
                DomainEvent::BrokerAccessBlocked => "BrokerAccessBlocked".to_owned(),
                DomainEvent::BrokerTokenExpired => "BrokerTokenExpired".to_owned(),
                DomainEvent::BrokerUserActionRequired => "BrokerUserActionRequired".to_owned(),
                DomainEvent::SettingsChanged => "SettingsChanged".to_owned(),
                DomainEvent::PolicyChanged => "PolicyChanged".to_owned(),
                DomainEvent::OrderSwitchChanged => "OrderSwitchChanged".to_owned(),
                DomainEvent::WatchlistChanged => "WatchlistChanged".to_owned(),
                DomainEvent::ThesisUpdated(id) => format!("ThesisUpdated({id:?})"),
                DomainEvent::SelfCheckChanged => "SelfCheckChanged".to_owned(),
                DomainEvent::MarketSessionChanged(id) => format!("MarketSessionChanged({id:?})"),
            };
            assert_eq!(format!("{event:?}"), expected_debug, "Debug는 ID와 종류만");
        }
        assert_eq!(kinds.len(), cases.len(), "모든 종류가 서로 다름");
    }
    fn enum_names(source: &str, marker: &str) -> BTreeSet<String> {
        source
            .split_once(marker)
            .expect("열거형 선언")
            .1
            .split_once("\n}")
            .expect("열거형 끝")
            .0
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.split(['(', ',']).next().expect("변형 이름").to_owned())
            .collect()
    }
    #[test]
    fn architecture_table_matches_both_closed_enums() {
        let source = std::fs::read_to_string("src/domain_event.rs").expect("이벤트 소스");
        let variants = enum_names(&source, "pub enum DomainEvent {");
        let kinds = enum_names(&source, "pub enum DomainEventKind {");
        let architecture =
            std::fs::read_to_string("../../docs/architecture.md").expect("아키텍처 문서");
        let documented = architecture
            .split_once("### 3.5 ")
            .expect("3.5 절")
            .1
            .split_once("### 3.6 ")
            .expect("3.6 절")
            .0
            .lines()
            .filter(|line| line.starts_with("| `") && line.ends_with("| 아니오 |"))
            .map(|line| line.split('`').nth(1).expect("문서 변형 이름").to_owned())
            .collect::<BTreeSet<_>>();
        let covered = cases()
            .into_iter()
            .map(|(_, _, name)| name.to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(variants, documented, "확정 문서 표와 코드 목록");
        assert_eq!(variants, kinds, "변형과 종류 목록");
        assert_eq!(variants, covered, "모든 변형의 독립 기대값을 테스트");
    }
    #[test]
    fn distinct_identifiers_remain_distinct_events() {
        assert_ne!(
            DomainEvent::PortfolioSynced(AccountId::new(1)),
            DomainEvent::PortfolioSynced(AccountId::new(2)),
            "조회 계좌를 잃지 않는다"
        );
        assert_ne!(
            DomainEvent::ProposalCreated(ProposalId::new(
                ClientOrderId::try_new("first").expect("키")
            )),
            DomainEvent::ProposalCreated(ProposalId::new(
                ClientOrderId::try_new("second").expect("키")
            )),
            "제안 조회 키를 잃지 않는다"
        );
    }
}
