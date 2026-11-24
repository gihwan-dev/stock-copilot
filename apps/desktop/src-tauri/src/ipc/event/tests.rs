use super::{UiDisclosureKey, UiDisclosureSource, UiEvent};
use domain::{AccountId, ClientOrderId, DomainEvent, InstrumentId, ProposalId};

#[test]
fn account_events_never_serialize_the_account_key() {
    for account in [17, 9_223_372_036_854_775_807] {
        for (event, expected) in [
            (
                DomainEvent::PortfolioSyncRecorded(AccountId::new(account)),
                "PortfolioSyncRecorded",
            ),
            (
                DomainEvent::TransactionsSyncRecorded(AccountId::new(account)),
                "TransactionsSyncRecorded",
            ),
            (
                DomainEvent::ValuationSnapshotTaken(AccountId::new(account)),
                "ValuationSnapshotTaken",
            ),
            (
                DomainEvent::OrderFilled(AccountId::new(account)),
                "OrderFilled",
            ),
            (
                DomainEvent::OrderCancelled(AccountId::new(account)),
                "OrderCancelled",
            ),
            (
                DomainEvent::OrderModified(AccountId::new(account)),
                "OrderModified",
            ),
        ] {
            assert_eq!(
                serde_json::to_value(UiEvent::from(event)).expect("event"),
                serde_json::json!(expected),
                "account key must be absent"
            );
        }
    }
}

#[test]
fn non_account_identifiers_keep_their_exact_strings() {
    let instrument = InstrumentId::try_new("instrument-23").expect("instrument");
    let proposal = ProposalId::new(ClientOrderId::try_new("proposal-47").expect("proposal"));
    let order = ClientOrderId::try_new("order-89").expect("order");
    for (event, expected) in [
        (
            DomainEvent::PriceUpdated(instrument),
            serde_json::json!({"PriceUpdated":"instrument-23"}),
        ),
        (
            DomainEvent::ProposalCreated(proposal),
            serde_json::json!({"ProposalCreated":"proposal-47"}),
        ),
        (
            DomainEvent::OrderSubmitted(order),
            serde_json::json!({"OrderSubmitted":"order-89"}),
        ),
    ] {
        assert_eq!(
            serde_json::to_value(UiEvent::from(event)).expect("event"),
            expected,
            "ID preservation"
        );
    }
    assert_eq!(
        serde_json::to_value(UiDisclosureKey {
            source: UiDisclosureSource::Dart,
            receipt_no: "20261117000031".into()
        })
        .expect("key"),
        serde_json::json!({"source":"Dart","receipt_no":"20261117000031"}),
        "distinct fields"
    );
}

#[test]
fn global_event_has_only_its_kind() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::AccountSelectionChanged)).expect("event"),
        serde_json::json!("AccountSelectionChanged"),
        "global event"
    );
}

#[test]
fn event_contract_group_0() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::PortfolioSyncRecorded(
            AccountId::new(971)
        )))
        .expect("event"),
        serde_json::json!("PortfolioSyncRecorded"),
        "PortfolioSyncRecorded"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::TransactionsSyncRecorded(
            AccountId::new(971)
        )))
        .expect("event"),
        serde_json::json!("TransactionsSyncRecorded"),
        "TransactionsSyncRecorded"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::PriceUpdated(
            InstrumentId::try_new("instrument-2").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"PriceUpdated":"instrument-2"}),
        "PriceUpdated"
    );
}

#[test]
fn event_contract_group_3() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::PriceMoved(
            InstrumentId::try_new("instrument-3").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"PriceMoved":"instrument-3"}),
        "PriceMoved"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::NewsActivityIncreased(
            InstrumentId::try_new("instrument-5").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"NewsActivityIncreased":"instrument-5"}),
        "NewsActivityIncreased"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ProposalCreated(
            ProposalId::new(ClientOrderId::try_new("proposal-6").expect("id"))
        )))
        .expect("event"),
        serde_json::json!({"ProposalCreated":"proposal-6"}),
        "ProposalCreated"
    );
}

#[test]
fn event_contract_group_6() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ProposalExpired(
            ProposalId::new(ClientOrderId::try_new("proposal-7").expect("id"))
        )))
        .expect("event"),
        serde_json::json!({"ProposalExpired":"proposal-7"}),
        "ProposalExpired"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ProposalApproved(
            ProposalId::new(ClientOrderId::try_new("proposal-8").expect("id"))
        )))
        .expect("event"),
        serde_json::json!({"ProposalApproved":"proposal-8"}),
        "ProposalApproved"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ProposalRejected(
            ProposalId::new(ClientOrderId::try_new("proposal-9").expect("id"))
        )))
        .expect("event"),
        serde_json::json!({"ProposalRejected":"proposal-9"}),
        "ProposalRejected"
    );
}

#[test]
fn event_contract_group_9() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ProposalSkipped(
            ProposalId::new(ClientOrderId::try_new("proposal-10").expect("id"))
        )))
        .expect("event"),
        serde_json::json!({"ProposalSkipped":"proposal-10"}),
        "ProposalSkipped"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderSubmitted(
            ClientOrderId::try_new("order-11").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"OrderSubmitted":"order-11"}),
        "OrderSubmitted"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderOutcomeUnknown(
            ClientOrderId::try_new("order-12").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"OrderOutcomeUnknown":"order-12"}),
        "OrderOutcomeUnknown"
    );
}

#[test]
fn event_contract_group_12() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::BrokerConnectionChanged)).expect("event"),
        serde_json::json!("BrokerConnectionChanged"),
        "BrokerConnectionChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::SettingsChanged)).expect("event"),
        serde_json::json!("SettingsChanged"),
        "SettingsChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::PolicyChanged)).expect("event"),
        serde_json::json!("PolicyChanged"),
        "PolicyChanged"
    );
}

#[test]
fn event_contract_group_15() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderSwitchChanged)).expect("event"),
        serde_json::json!("OrderSwitchChanged"),
        "OrderSwitchChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::WatchlistChanged)).expect("event"),
        serde_json::json!("WatchlistChanged"),
        "WatchlistChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ThesisUpdated(
            InstrumentId::try_new("instrument-18").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"ThesisUpdated":"instrument-18"}),
        "ThesisUpdated"
    );
}

#[test]
fn event_contract_group_18() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::SelfCheckChanged)).expect("event"),
        serde_json::json!("SelfCheckChanged"),
        "SelfCheckChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::MarketSessionChanged(
            domain::KnownMarket::Us
        )))
        .expect("event"),
        serde_json::json!({"MarketSessionChanged":"Us"}),
        "MarketSessionChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::SignalRaised(
            InstrumentId::try_new("instrument-21").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"SignalRaised":"instrument-21"}),
        "SignalRaised"
    );
}

#[test]
fn event_contract_group_21() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::SignalCleared(
            InstrumentId::try_new("instrument-22").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"SignalCleared":"instrument-22"}),
        "SignalCleared"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::EstimateSubmitted(
            InstrumentId::try_new("instrument-23").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"EstimateSubmitted":"instrument-23"}),
        "EstimateSubmitted"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::EstimateResolved(
            InstrumentId::try_new("instrument-24").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"EstimateResolved":"instrument-24"}),
        "EstimateResolved"
    );
}

#[test]
fn event_contract_group_24() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::ValuationSnapshotTaken(
            AccountId::new(971)
        )))
        .expect("event"),
        serde_json::json!("ValuationSnapshotTaken"),
        "ValuationSnapshotTaken"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderRejected(
            ClientOrderId::try_new("order-26").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"OrderRejected":"order-26"}),
        "OrderRejected"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderOutcomeResolved(
            ClientOrderId::try_new("order-27").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"OrderOutcomeResolved":"order-27"}),
        "OrderOutcomeResolved"
    );
}

#[test]
fn event_contract_group_27() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderFilled(AccountId::new(971))))
            .expect("event"),
        serde_json::json!("OrderFilled"),
        "OrderFilled"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderCancelled(AccountId::new(
            971
        ))))
        .expect("event"),
        serde_json::json!("OrderCancelled"),
        "OrderCancelled"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::OrderModified(AccountId::new(
            971
        ))))
        .expect("event"),
        serde_json::json!("OrderModified"),
        "OrderModified"
    );
}

#[test]
fn event_contract_group_30() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::NewsClusterGrew(
            InstrumentId::try_new("instrument-32").expect("id")
        )))
        .expect("event"),
        serde_json::json!({"NewsClusterGrew":"instrument-32"}),
        "NewsClusterGrew"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::AiBudgetChanged)).expect("event"),
        serde_json::json!("AiBudgetChanged"),
        "AiBudgetChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::AccountSelectionChanged)).expect("event"),
        serde_json::json!("AccountSelectionChanged"),
        "AccountSelectionChanged"
    );
}

#[test]
fn event_contract_group_33() {
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::StoredDataChanged)).expect("event"),
        serde_json::json!("StoredDataChanged"),
        "StoredDataChanged"
    );
    assert_eq!(
        serde_json::to_value(UiEvent::from(DomainEvent::FxRateUpdated(
            domain::FxPair::KrwUsd
        )))
        .expect("event"),
        serde_json::json!({"FxRateUpdated":"KrwUsd"}),
        "FxRateUpdated"
    );
}

#[test]
fn uimarket_retains_each_closed_kind() {
    assert_eq!(
        serde_json::to_value(super::UiMarket::from(domain::KnownMarket::Kr)).expect("kind"),
        serde_json::json!("Kr"),
        "Kr"
    );
    assert_eq!(
        serde_json::to_value(super::UiMarket::from(domain::KnownMarket::Us)).expect("kind"),
        serde_json::json!("Us"),
        "Us"
    );
}

#[test]
fn uifxpair_retains_each_closed_kind() {
    assert_eq!(
        serde_json::to_value(super::UiFxPair::from(domain::FxPair::UsdKrw)).expect("kind"),
        serde_json::json!("UsdKrw"),
        "UsdKrw"
    );
    assert_eq!(
        serde_json::to_value(super::UiFxPair::from(domain::FxPair::KrwUsd)).expect("kind"),
        serde_json::json!("KrwUsd"),
        "KrwUsd"
    );
}

#[test]
fn uidisclosuresource_retains_each_closed_kind() {
    assert_eq!(
        serde_json::to_value(UiDisclosureSource::from(domain::DisclosureSource::Kind))
            .expect("kind"),
        serde_json::json!("Kind"),
        "Kind"
    );
    assert_eq!(
        serde_json::to_value(UiDisclosureSource::from(domain::DisclosureSource::Dart))
            .expect("kind"),
        serde_json::json!("Dart"),
        "Dart"
    );
    assert_eq!(
        serde_json::to_value(UiDisclosureSource::from(domain::DisclosureSource::SecEdgar))
            .expect("kind"),
        serde_json::json!("SecEdgar"),
        "SecEdgar"
    );
}

#[test]
fn uinewssource_retains_each_closed_kind() {
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::Hankyung))
            .expect("kind"),
        serde_json::json!("Hankyung"),
        "Hankyung"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::Maekyung))
            .expect("kind"),
        serde_json::json!("Maekyung"),
        "Maekyung"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::Yonhap)).expect("kind"),
        serde_json::json!("Yonhap"),
        "Yonhap"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::Edaily)).expect("kind"),
        serde_json::json!("Edaily"),
        "Edaily"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::BankOfKorea))
            .expect("kind"),
        serde_json::json!("BankOfKorea"),
        "BankOfKorea"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(
            domain::NewsSource::FederalReserve
        ))
        .expect("kind"),
        serde_json::json!("FederalReserve"),
        "FederalReserve"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::GoogleNews))
            .expect("kind"),
        serde_json::json!("GoogleNews"),
        "GoogleNews"
    );
    assert_eq!(
        serde_json::to_value(super::UiNewsSource::from(domain::NewsSource::YahooFinance))
            .expect("kind"),
        serde_json::json!("YahooFinance"),
        "YahooFinance"
    );
}

#[test]
fn collection_sources_and_disclosure_receipts_keep_distinct_fields() {
    for (source, expected) in [
        (
            domain::InformationSource::Web,
            serde_json::json!({"CollectionFinished":"Web"}),
        ),
        (
            domain::InformationSource::News(domain::NewsSource::Edaily),
            serde_json::json!({"CollectionFinished":{"News":"Edaily"}}),
        ),
        (
            domain::InformationSource::Disclosure(domain::DisclosureSource::Kind),
            serde_json::json!({"CollectionFinished":{"Disclosure":"Kind"}}),
        ),
    ] {
        assert_eq!(
            serde_json::to_value(UiEvent::from(DomainEvent::CollectionFinished(source)))
                .expect("event"),
            expected,
            "source"
        );
    }
}

#[test]
fn disclosure_receipts_keep_distinct_fields() {
    for (receipt, source, text) in [
        (
            domain::ReceiptNo::Dart(
                domain::DartReceiptNo::try_new("20261117000031").expect("dart"),
            ),
            "Dart",
            "20261117000031",
        ),
        (
            domain::ReceiptNo::Kind(
                domain::KindReceiptNo::try_new("20261117000047").expect("kind"),
            ),
            "Kind",
            "20261117000047",
        ),
        (
            domain::ReceiptNo::SecEdgar(
                domain::SecEdgarReceiptNo::try_new("0001234567-26-000089").expect("sec"),
            ),
            "SecEdgar",
            "0001234567-26-000089",
        ),
    ] {
        let key = domain::Disclosure::new(
            receipt,
            domain::NormalizedUrl::try_new("https://example.com/d").expect("url"),
            domain::UntrustedText::try_new("title stays private").expect("title"),
            None,
        )
        .key()
        .clone();
        assert_eq!(
            serde_json::to_value(UiEvent::from(DomainEvent::DisclosureArrived(key.clone())))
                .expect("event"),
            serde_json::json!({"DisclosureArrived":{"source":source,"receipt_no":text}}),
            "arrival"
        );
        assert_eq!(
            serde_json::to_value(UiEvent::from(DomainEvent::DisclosureTriggerRecorded(key)))
                .expect("event"),
            serde_json::json!({"DisclosureTriggerRecorded":{"source":source,"receipt_no":text}}),
            "trigger"
        );
    }
}

#[test]
fn uidatasource_and_source_status_changed_retain_every_closed_kind() {
    for (source, expected) in [
        (domain::DataSource::KindDisclosure, "KindDisclosure"),
        (domain::DataSource::DartDisclosureApi, "DartDisclosureApi"),
        (domain::DataSource::DartDisclosureRss, "DartDisclosureRss"),
        (domain::DataSource::DartFundamentals, "DartFundamentals"),
        (domain::DataSource::KindCorpList, "KindCorpList"),
        (domain::DataSource::Hankyung, "Hankyung"),
        (domain::DataSource::Maekyung, "Maekyung"),
        (domain::DataSource::Yonhap, "Yonhap"),
        (domain::DataSource::Edaily, "Edaily"),
        (domain::DataSource::GoogleNews, "GoogleNews"),
        (domain::DataSource::WebSearch, "WebSearch"),
        (domain::DataSource::YahooFinance, "YahooFinance"),
        (domain::DataSource::SecEdgar, "SecEdgar"),
        (domain::DataSource::BankOfKorea, "BankOfKorea"),
        (domain::DataSource::FederalReserve, "FederalReserve"),
        (domain::DataSource::UsTreasury, "UsTreasury"),
        (domain::DataSource::Fred, "Fred"),
    ] {
        assert_eq!(
            serde_json::to_value(super::UiDataSource::from(source)).expect("source"),
            serde_json::json!(expected),
            "{expected}"
        );
        assert_eq!(
            serde_json::to_value(UiEvent::from(DomainEvent::SourceStatusChanged(source)))
                .expect("event"),
            serde_json::json!({"SourceStatusChanged": expected}),
            "{expected}"
        );
    }
}
