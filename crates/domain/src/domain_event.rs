use crate::{
    AccountId, ClientOrderId, DisclosureKey, FxPair, InformationSource, InstrumentId, KnownMarket,
    ProposalId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    PortfolioSyncRecorded(AccountId),
    TransactionsSyncRecorded(AccountId),
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
    BrokerConnectionChanged,
    SettingsChanged,
    PolicyChanged,
    OrderSwitchChanged,
    WatchlistChanged,
    ThesisUpdated(InstrumentId),
    SelfCheckChanged,
    MarketSessionChanged(KnownMarket),
    SignalRaised(InstrumentId),
    SignalCleared(InstrumentId),
    EstimateSubmitted(InstrumentId),
    EstimateResolved(InstrumentId),
    ValuationSnapshotTaken(AccountId),
    OrderRejected(ClientOrderId),
    OrderOutcomeResolved(ClientOrderId),
    OrderFilled(AccountId),
    OrderCancelled(AccountId),
    OrderModified(AccountId),
    CollectionFinished(InformationSource),
    NewsClusterGrew(InstrumentId),
    AiBudgetChanged,
    AccountSelectionChanged,
    StoredDataChanged,
    FxRateUpdated(FxPair),
    DisclosureTriggerRecorded(DisclosureKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DomainEventKind {
    PortfolioSyncRecorded,
    TransactionsSyncRecorded,
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
    BrokerConnectionChanged,
    SettingsChanged,
    PolicyChanged,
    OrderSwitchChanged,
    WatchlistChanged,
    ThesisUpdated,
    SelfCheckChanged,
    MarketSessionChanged,
    SignalRaised,
    SignalCleared,
    EstimateSubmitted,
    EstimateResolved,
    ValuationSnapshotTaken,
    OrderRejected,
    OrderOutcomeResolved,
    OrderFilled,
    OrderCancelled,
    OrderModified,
    CollectionFinished,
    NewsClusterGrew,
    AiBudgetChanged,
    AccountSelectionChanged,
    StoredDataChanged,
    FxRateUpdated,
    DisclosureTriggerRecorded,
}

impl DomainEvent {
    #[must_use]
    pub const fn kind(&self) -> DomainEventKind {
        match self {
            Self::PortfolioSyncRecorded(_) => DomainEventKind::PortfolioSyncRecorded,
            Self::TransactionsSyncRecorded(_) => DomainEventKind::TransactionsSyncRecorded,
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
            Self::BrokerConnectionChanged => DomainEventKind::BrokerConnectionChanged,
            Self::SettingsChanged => DomainEventKind::SettingsChanged,
            Self::PolicyChanged => DomainEventKind::PolicyChanged,
            Self::OrderSwitchChanged => DomainEventKind::OrderSwitchChanged,
            Self::WatchlistChanged => DomainEventKind::WatchlistChanged,
            Self::ThesisUpdated(_) => DomainEventKind::ThesisUpdated,
            Self::SelfCheckChanged => DomainEventKind::SelfCheckChanged,
            Self::MarketSessionChanged(_) => DomainEventKind::MarketSessionChanged,
            Self::SignalRaised(_) => DomainEventKind::SignalRaised,
            Self::SignalCleared(_) => DomainEventKind::SignalCleared,
            Self::EstimateSubmitted(_) => DomainEventKind::EstimateSubmitted,
            Self::EstimateResolved(_) => DomainEventKind::EstimateResolved,
            Self::ValuationSnapshotTaken(_) => DomainEventKind::ValuationSnapshotTaken,
            Self::OrderRejected(_) => DomainEventKind::OrderRejected,
            Self::OrderOutcomeResolved(_) => DomainEventKind::OrderOutcomeResolved,
            Self::OrderFilled(_) => DomainEventKind::OrderFilled,
            Self::OrderCancelled(_) => DomainEventKind::OrderCancelled,
            Self::OrderModified(_) => DomainEventKind::OrderModified,
            Self::CollectionFinished(_) => DomainEventKind::CollectionFinished,
            Self::NewsClusterGrew(_) => DomainEventKind::NewsClusterGrew,
            Self::AiBudgetChanged => DomainEventKind::AiBudgetChanged,
            Self::AccountSelectionChanged => DomainEventKind::AccountSelectionChanged,
            Self::StoredDataChanged => DomainEventKind::StoredDataChanged,
            Self::FxRateUpdated(_) => DomainEventKind::FxRateUpdated,
            Self::DisclosureTriggerRecorded(_) => DomainEventKind::DisclosureTriggerRecorded,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, ClientOrderId, Disclosure, DomainEvent, DomainEventKind, FxPair,
        InformationSource, InstrumentId, KnownMarket, NormalizedUrl, ProposalId, ReceiptNo,
        UntrustedText,
    };
    use std::collections::{BTreeMap, BTreeSet};
    fn cases_0() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let account = AccountId::new(17);
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
        vec![
            (
                DomainEvent::PortfolioSyncRecorded(account.clone()),
                DomainEventKind::PortfolioSyncRecorded,
                "PortfolioSyncRecorded",
            ),
            (
                DomainEvent::TransactionsSyncRecorded(account),
                DomainEventKind::TransactionsSyncRecorded,
                "TransactionsSyncRecorded",
            ),
            (
                DomainEvent::PriceUpdated(instrument.clone()),
                DomainEventKind::PriceUpdated,
                "PriceUpdated",
            ),
            (
                DomainEvent::PriceMoved(instrument),
                DomainEventKind::PriceMoved,
                "PriceMoved",
            ),
            (
                DomainEvent::DisclosureArrived(disclosure),
                DomainEventKind::DisclosureArrived,
                "DisclosureArrived",
            ),
        ]
    }
    fn cases_5() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        let proposal = ProposalId::new(ClientOrderId::try_new("proposal-7").expect("가짜 제안 키"));
        vec![
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
                DomainEvent::ProposalExpired(proposal.clone()),
                DomainEventKind::ProposalExpired,
                "ProposalExpired",
            ),
            (
                DomainEvent::ProposalApproved(proposal.clone()),
                DomainEventKind::ProposalApproved,
                "ProposalApproved",
            ),
            (
                DomainEvent::ProposalRejected(proposal),
                DomainEventKind::ProposalRejected,
                "ProposalRejected",
            ),
        ]
    }
    fn cases_10() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let proposal = ProposalId::new(ClientOrderId::try_new("proposal-7").expect("가짜 제안 키"));
        let order = ClientOrderId::try_new("order-7").expect("가짜 주문 키");
        vec![
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
            (
                DomainEvent::BrokerConnectionChanged,
                DomainEventKind::BrokerConnectionChanged,
                "BrokerConnectionChanged",
            ),
            (
                DomainEvent::SettingsChanged,
                DomainEventKind::SettingsChanged,
                "SettingsChanged",
            ),
        ]
    }
    fn cases_15() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        vec![
            (
                DomainEvent::PolicyChanged,
                DomainEventKind::PolicyChanged,
                "PolicyChanged",
            ),
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
        ]
    }
    fn cases_20() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let market = KnownMarket::Kr;
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        vec![
            (
                DomainEvent::MarketSessionChanged(market),
                DomainEventKind::MarketSessionChanged,
                "MarketSessionChanged",
            ),
            (
                DomainEvent::SignalRaised(instrument.clone()),
                DomainEventKind::SignalRaised,
                "SignalRaised",
            ),
            (
                DomainEvent::SignalCleared(instrument.clone()),
                DomainEventKind::SignalCleared,
                "SignalCleared",
            ),
            (
                DomainEvent::EstimateSubmitted(instrument.clone()),
                DomainEventKind::EstimateSubmitted,
                "EstimateSubmitted",
            ),
            (
                DomainEvent::EstimateResolved(instrument),
                DomainEventKind::EstimateResolved,
                "EstimateResolved",
            ),
        ]
    }
    fn cases_25() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let account = AccountId::new(17);
        let order = ClientOrderId::try_new("order-7").expect("가짜 주문 키");
        vec![
            (
                DomainEvent::ValuationSnapshotTaken(account.clone()),
                DomainEventKind::ValuationSnapshotTaken,
                "ValuationSnapshotTaken",
            ),
            (
                DomainEvent::OrderRejected(order.clone()),
                DomainEventKind::OrderRejected,
                "OrderRejected",
            ),
            (
                DomainEvent::OrderOutcomeResolved(order),
                DomainEventKind::OrderOutcomeResolved,
                "OrderOutcomeResolved",
            ),
            (
                DomainEvent::OrderFilled(account.clone()),
                DomainEventKind::OrderFilled,
                "OrderFilled",
            ),
            (
                DomainEvent::OrderCancelled(account),
                DomainEventKind::OrderCancelled,
                "OrderCancelled",
            ),
        ]
    }
    fn cases_30() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let account = AccountId::new(17);
        let source_kind = InformationSource::Web;
        let instrument = InstrumentId::try_new("instrument-7").expect("가짜 종목 ID");
        vec![
            (
                DomainEvent::OrderModified(account),
                DomainEventKind::OrderModified,
                "OrderModified",
            ),
            (
                DomainEvent::CollectionFinished(source_kind),
                DomainEventKind::CollectionFinished,
                "CollectionFinished",
            ),
            (
                DomainEvent::NewsClusterGrew(instrument),
                DomainEventKind::NewsClusterGrew,
                "NewsClusterGrew",
            ),
            (
                DomainEvent::AiBudgetChanged,
                DomainEventKind::AiBudgetChanged,
                "AiBudgetChanged",
            ),
            (
                DomainEvent::AccountSelectionChanged,
                DomainEventKind::AccountSelectionChanged,
                "AccountSelectionChanged",
            ),
        ]
    }
    fn cases_35() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        let pair = FxPair::UsdKrw;
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
        vec![
            (
                DomainEvent::StoredDataChanged,
                DomainEventKind::StoredDataChanged,
                "StoredDataChanged",
            ),
            (
                DomainEvent::FxRateUpdated(pair),
                DomainEventKind::FxRateUpdated,
                "FxRateUpdated",
            ),
            (
                DomainEvent::DisclosureTriggerRecorded(disclosure),
                DomainEventKind::DisclosureTriggerRecorded,
                "DisclosureTriggerRecorded",
            ),
        ]
    }
    fn cases() -> Vec<(DomainEvent, DomainEventKind, &'static str)> {
        [
            cases_0(),
            cases_5(),
            cases_10(),
            cases_15(),
            cases_20(),
            cases_25(),
            cases_30(),
            cases_35(),
        ]
        .concat()
    }
    fn expected_debug(event: &DomainEvent) -> String {
        match event {
            DomainEvent::PortfolioSyncRecorded(id) => format!("PortfolioSyncRecorded({id:?})"),
            DomainEvent::TransactionsSyncRecorded(id) => {
                format!("TransactionsSyncRecorded({id:?})")
            }
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
            DomainEvent::BrokerConnectionChanged => "BrokerConnectionChanged".to_owned(),
            DomainEvent::SettingsChanged => "SettingsChanged".to_owned(),
            DomainEvent::PolicyChanged => "PolicyChanged".to_owned(),
            DomainEvent::OrderSwitchChanged => "OrderSwitchChanged".to_owned(),
            DomainEvent::WatchlistChanged => "WatchlistChanged".to_owned(),
            DomainEvent::ThesisUpdated(id) => format!("ThesisUpdated({id:?})"),
            DomainEvent::SelfCheckChanged => "SelfCheckChanged".to_owned(),
            DomainEvent::MarketSessionChanged(id) => format!("MarketSessionChanged({id:?})"),
            DomainEvent::SignalRaised(id) => format!("SignalRaised({id:?})"),
            DomainEvent::SignalCleared(id) => format!("SignalCleared({id:?})"),
            DomainEvent::EstimateSubmitted(id) => format!("EstimateSubmitted({id:?})"),
            DomainEvent::EstimateResolved(id) => format!("EstimateResolved({id:?})"),
            DomainEvent::ValuationSnapshotTaken(id) => format!("ValuationSnapshotTaken({id:?})"),
            DomainEvent::OrderRejected(id) => format!("OrderRejected({id:?})"),
            DomainEvent::OrderOutcomeResolved(id) => format!("OrderOutcomeResolved({id:?})"),
            DomainEvent::OrderFilled(id) => format!("OrderFilled({id:?})"),
            DomainEvent::OrderCancelled(id) => format!("OrderCancelled({id:?})"),
            DomainEvent::OrderModified(id) => format!("OrderModified({id:?})"),
            DomainEvent::CollectionFinished(id) => format!("CollectionFinished({id:?})"),
            DomainEvent::NewsClusterGrew(id) => format!("NewsClusterGrew({id:?})"),
            DomainEvent::AiBudgetChanged => "AiBudgetChanged".to_owned(),
            DomainEvent::AccountSelectionChanged => "AccountSelectionChanged".to_owned(),
            DomainEvent::StoredDataChanged => "StoredDataChanged".to_owned(),
            DomainEvent::FxRateUpdated(id) => format!("FxRateUpdated({id:?})"),
            DomainEvent::DisclosureTriggerRecorded(id) => {
                format!("DisclosureTriggerRecorded({id:?})")
            }
        }
    }
    #[test]
    fn every_variant_has_its_own_kind_and_only_id_debug() {
        let cases = cases();
        let mut kinds = BTreeSet::new();
        for (event, expected, name) in &cases {
            assert_eq!(event.kind(), *expected, "변형별 종류 계약: {name}");
            assert!(kinds.insert(*expected), "종류는 일대일: {name}");
            assert_eq!(event.clone(), *event, "복제는 ID를 보존: {name}");
            assert_eq!(
                format!("{event:?}"),
                expected_debug(event),
                "Debug는 ID와 종류만"
            );
        }
        assert_eq!(kinds.len(), cases.len(), "모든 종류가 서로 다름");
    }
    #[test]
    fn distinct_identifiers_remain_distinct_events() {
        assert_ne!(
            DomainEvent::PortfolioSyncRecorded(AccountId::new(1)),
            DomainEvent::PortfolioSyncRecorded(AccountId::new(2)),
            "계좌 조회 키 보존"
        );
        assert_ne!(
            DomainEvent::ProposalCreated(ProposalId::new(
                ClientOrderId::try_new("first").expect("키")
            )),
            DomainEvent::ProposalCreated(ProposalId::new(
                ClientOrderId::try_new("second").expect("키")
            )),
            "제안 조회 키 보존"
        );
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ContractMismatch {
        DuplicateRow,
        Payload,
        Pending,
        Kind,
        Coverage,
        Owner,
    }
    fn enum_schema(source: &str, marker: &str) -> BTreeMap<String, String> {
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
            .map(|line| {
                let declaration = line.trim_end_matches(',');
                declaration.split_once('(').map_or_else(
                    || (declaration.to_owned(), "없음".to_owned()),
                    |(name, payload)| {
                        (
                            name.to_owned(),
                            payload.strip_suffix(')').expect("튜플 필드 끝").to_owned(),
                        )
                    },
                )
            })
            .collect()
    }
    fn document_rows(document: &str) -> Vec<[String; 8]> {
        document
            .split_once("### 3.5 ")
            .expect("3.5 절")
            .1
            .split_once("### 3.6 ")
            .expect("3.6 절")
            .0
            .lines()
            .filter(|line| line.starts_with("| `"))
            .map(|line| {
                line.trim_matches('|')
                    .split('|')
                    .map(|column| column.trim().trim_matches('`').to_owned())
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("이벤트 표 8열")
            })
            .collect()
    }
    fn documented_schema(rows: &[[String; 8]]) -> BTreeMap<String, String> {
        let mut payloads = BTreeMap::new();
        for [
            name,
            payload,
            _producer,
            _subscriber,
            _reason,
            state,
            _owner,
            _why,
        ] in rows
        {
            if state == "아니오" {
                payloads.extend([(name.clone(), payload.clone())]);
            }
        }
        payloads
    }
    fn pending_contract(rows: &[[String; 8]]) -> Result<(), ContractMismatch> {
        let expected = BTreeMap::from([
            ("AiJobFinished".to_owned(), "AI-17".to_owned()),
            ("BriefingFinished".to_owned(), "J-03".to_owned()),
            ("SourceStatusChanged".to_owned(), "N-01".to_owned()),
        ]);
        let mut actual = BTreeMap::new();
        for [
            name,
            _payload,
            _producer,
            _subscriber,
            _reason,
            state,
            owner,
            _why,
        ] in rows
        {
            if state == "정의 대기" {
                actual.extend([(name.clone(), owner.clone())]);
            }
        }
        if actual != expected {
            return Err(ContractMismatch::Pending);
        }
        for [
            _name,
            _payload,
            producer,
            subscriber,
            reason,
            state,
            owner,
            why,
        ] in rows
        {
            if producer.is_empty() || subscriber.is_empty() || reason.is_empty() {
                return Err(ContractMismatch::Owner);
            }
            match state.as_str() {
                "아니오" if owner == "-" && why == "-" => {}
                "정의 대기" if owner != "-" && !why.is_empty() && why != "-" => {}
                _ => return Err(ContractMismatch::Owner),
            }
        }
        Ok(())
    }
    fn check_contract(source: &str, document: &str) -> Result<(), ContractMismatch> {
        let rows = document_rows(document);
        let names = rows
            .iter()
            .map(
                |[
                    name,
                    _payload,
                    _producer,
                    _subscriber,
                    _reason,
                    _state,
                    _owner,
                    _why,
                ]| name,
            )
            .collect::<BTreeSet<_>>();
        if names.len() != rows.len() {
            return Err(ContractMismatch::DuplicateRow);
        }
        pending_contract(&rows)?;
        let variants = enum_schema(source, "pub enum DomainEvent {");
        if variants != documented_schema(&rows) {
            return Err(ContractMismatch::Payload);
        }
        let kinds = enum_schema(source, "pub enum DomainEventKind {");
        if variants.keys().collect::<BTreeSet<_>>() != kinds.keys().collect::<BTreeSet<_>>()
            || kinds.values().any(|payload| payload != "없음")
        {
            return Err(ContractMismatch::Kind);
        }
        let covered = cases()
            .into_iter()
            .map(|(_, _, name)| name.to_owned())
            .collect::<BTreeSet<_>>();
        if variants.keys().cloned().collect::<BTreeSet<_>>() != covered {
            return Err(ContractMismatch::Coverage);
        }
        Ok(())
    }
    #[test]
    fn architecture_table_matches_payloads_kinds_and_fixed_pending_contract() {
        let source = std::fs::read_to_string("src/domain_event.rs").expect("이벤트 소스");
        let document =
            std::fs::read_to_string("../../docs/architecture.md").expect("아키텍처 문서");
        assert_eq!(
            check_contract(&source, &document),
            Ok(()),
            "문서의 전체 행·페이로드·종류·정의 대기 계약"
        );
    }
    fn replace_row(document: &str, name: &str, replacement: &str) -> String {
        let row = document
            .lines()
            .find(|line| line.starts_with(&format!("| `{name}` |")))
            .expect("기존 행");
        document.replacen(row, replacement, 1)
    }
    #[test]
    fn rejects_all_five_review_mutations() {
        let source = std::fs::read_to_string("src/domain_event.rs").expect("이벤트 소스");
        let document =
            std::fs::read_to_string("../../docs/architecture.md").expect("아키텍처 문서");
        let first = document
            .lines()
            .find(|line| line.starts_with("| `PortfolioSyncRecorded` |"))
            .expect("확정 행");
        let duplicate = document.replacen(first, &format!("{first}\n{first}"), 1);
        assert_eq!(
            check_contract(&source, &duplicate),
            Err(ContractMismatch::DuplicateRow)
        );
        let payload = document.replacen(
            "| `PortfolioSyncRecorded` | `AccountId` |",
            "| `PortfolioSyncRecorded` | `InstrumentId` |",
            1,
        );
        assert_eq!(
            check_contract(&source, &payload),
            Err(ContractMismatch::Payload)
        );
        let missing_pending = replace_row(&document, "AiJobFinished", "");
        assert_eq!(
            check_contract(&source, &missing_pending),
            Err(ContractMismatch::Pending)
        );
        let global_id = document.replacen(
            "| `SettingsChanged` | 없음 |",
            "| `SettingsChanged` | `AccountId` |",
            1,
        );
        assert_eq!(
            check_contract(&source, &global_id),
            Err(ContractMismatch::Payload)
        );
        let code_payload = source.replacen(
            "PortfolioSyncRecorded(AccountId)",
            "PortfolioSyncRecorded(InstrumentId)",
            1,
        );
        assert_eq!(
            check_contract(&code_payload, &document),
            Err(ContractMismatch::Payload),
            "테스트 인자를 맞춰도 선언 필드와 문서가 다르면 실패"
        );
    }
    fn replace_column(document: &str, name: &str, column: usize, value: &str) -> String {
        let row = document
            .lines()
            .find(|line| line.starts_with(&format!("| `{name}` |")))
            .expect("기존 행");
        let mut columns = row
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        *columns.get_mut(column).expect("열 번호") = value;
        replace_row(document, name, &format!("| {} |", columns.join(" | ")))
    }
    #[test]
    fn rejects_missing_metadata_and_invalid_waiting_reasons() {
        let source = std::fs::read_to_string("src/domain_event.rs").expect("이벤트 소스");
        let document =
            std::fs::read_to_string("../../docs/architecture.md").expect("아키텍처 문서");
        for column in [2, 3, 4] {
            let changed = replace_column(&document, "PortfolioSyncRecorded", column, "");
            assert_eq!(
                check_contract(&source, &changed),
                Err(ContractMismatch::Owner),
                "발행자·구독자·근거는 필수"
            );
        }
        for (name, column, value) in [
            ("AiJobFinished", 7, ""),
            ("AiJobFinished", 7, "-"),
            ("SettingsChanged", 5, "미확정"),
            ("SettingsChanged", 6, "AI-17"),
        ] {
            let changed = replace_column(&document, name, column, value);
            assert_eq!(
                check_contract(&source, &changed),
                Err(ContractMismatch::Owner),
                "정의 대기 사유와 상태 계약"
            );
        }
    }
    #[test]
    fn rejects_kind_payloads_extra_kinds_and_missing_case_coverage() {
        let source = std::fs::read_to_string("src/domain_event.rs").expect("이벤트 소스");
        let document =
            std::fs::read_to_string("../../docs/architecture.md").expect("아키텍처 문서");
        let extra = source.replacen(
            "pub enum DomainEventKind {",
            "pub enum DomainEventKind {\n    ExtraKind,",
            1,
        );
        assert_eq!(
            check_contract(&extra, &document),
            Err(ContractMismatch::Kind),
            "종류만 추가할 수 없다"
        );
        let kind_payload = source.replacen(
            "    PortfolioSyncRecorded,\n",
            "    PortfolioSyncRecorded(AccountId),\n",
            1,
        );
        assert_eq!(
            check_contract(&kind_payload, &document),
            Err(ContractMismatch::Kind),
            "종류에는 ID 필드가 없다"
        );
        let removed = source
            .replacen("    PriceUpdated(InstrumentId),\n", "", 1)
            .replacen("    PriceUpdated,\n", "", 1);
        let missing_row = replace_row(&document, "PriceUpdated", "");
        assert_eq!(
            check_contract(&removed, &missing_row),
            Err(ContractMismatch::Coverage),
            "코드·문서와 독립 기대값 목록을 함께 대조한다"
        );
    }
}
