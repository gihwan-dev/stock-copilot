use domain::DomainEvent;
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiMarket {
    Kr,
    Us,
}

impl From<domain::KnownMarket> for UiMarket {
    fn from(value: domain::KnownMarket) -> Self {
        match value {
            domain::KnownMarket::Kr => Self::Kr,
            domain::KnownMarket::Us => Self::Us,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiFxPair {
    UsdKrw,
    KrwUsd,
}

impl From<domain::FxPair> for UiFxPair {
    fn from(value: domain::FxPair) -> Self {
        match value {
            domain::FxPair::UsdKrw => Self::UsdKrw,
            domain::FxPair::KrwUsd => Self::KrwUsd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiDisclosureSource {
    Kind,
    Dart,
    SecEdgar,
}

impl From<domain::DisclosureSource> for UiDisclosureSource {
    fn from(value: domain::DisclosureSource) -> Self {
        match value {
            domain::DisclosureSource::Kind => Self::Kind,
            domain::DisclosureSource::Dart => Self::Dart,
            domain::DisclosureSource::SecEdgar => Self::SecEdgar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiNewsSource {
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    BankOfKorea,
    FederalReserve,
    GoogleNews,
    YahooFinance,
}

impl From<domain::NewsSource> for UiNewsSource {
    fn from(value: domain::NewsSource) -> Self {
        match value {
            domain::NewsSource::Hankyung => Self::Hankyung,
            domain::NewsSource::Maekyung => Self::Maekyung,
            domain::NewsSource::Yonhap => Self::Yonhap,
            domain::NewsSource::Edaily => Self::Edaily,
            domain::NewsSource::BankOfKorea => Self::BankOfKorea,
            domain::NewsSource::FederalReserve => Self::FederalReserve,
            domain::NewsSource::GoogleNews => Self::GoogleNews,
            domain::NewsSource::YahooFinance => Self::YahooFinance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiInformationSource {
    Web,
    News(UiNewsSource),
    Disclosure(UiDisclosureSource),
}

impl From<domain::InformationSource> for UiInformationSource {
    fn from(value: domain::InformationSource) -> Self {
        match value {
            domain::InformationSource::Web => Self::Web,
            domain::InformationSource::News(source) => Self::News(source.into()),
            domain::InformationSource::Disclosure(source) => Self::Disclosure(source.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiDisclosureKey {
    pub source: UiDisclosureSource,
    pub receipt_no: String,
}

impl From<domain::DisclosureKey> for UiDisclosureKey {
    fn from(value: domain::DisclosureKey) -> Self {
        let receipt_no = match value.receipt_no() {
            domain::ReceiptNo::Kind(receipt) => receipt.as_str(),
            domain::ReceiptNo::Dart(receipt) => receipt.as_str(),
            domain::ReceiptNo::SecEdgar(receipt) => receipt.as_str(),
        }
        .to_owned();
        Self {
            source: value.source().into(),
            receipt_no,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiDataSource {
    KindDisclosure,
    DartDisclosureApi,
    DartDisclosureRss,
    DartFundamentals,
    KindCorpList,
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    GoogleNews,
    WebSearch,
    YahooFinance,
    SecEdgar,
    BankOfKorea,
    FederalReserve,
    UsTreasury,
    Fred,
}

impl From<domain::DataSource> for UiDataSource {
    fn from(value: domain::DataSource) -> Self {
        match value {
            domain::DataSource::KindDisclosure => Self::KindDisclosure,
            domain::DataSource::DartDisclosureApi => Self::DartDisclosureApi,
            domain::DataSource::DartDisclosureRss => Self::DartDisclosureRss,
            domain::DataSource::DartFundamentals => Self::DartFundamentals,
            domain::DataSource::KindCorpList => Self::KindCorpList,
            domain::DataSource::Hankyung => Self::Hankyung,
            domain::DataSource::Maekyung => Self::Maekyung,
            domain::DataSource::Yonhap => Self::Yonhap,
            domain::DataSource::Edaily => Self::Edaily,
            domain::DataSource::GoogleNews => Self::GoogleNews,
            domain::DataSource::WebSearch => Self::WebSearch,
            domain::DataSource::YahooFinance => Self::YahooFinance,
            domain::DataSource::SecEdgar => Self::SecEdgar,
            domain::DataSource::BankOfKorea => Self::BankOfKorea,
            domain::DataSource::FederalReserve => Self::FederalReserve,
            domain::DataSource::UsTreasury => Self::UsTreasury,
            domain::DataSource::Fred => Self::Fred,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type, tauri_specta::Event)]
pub enum UiEvent {
    PortfolioSyncRecorded,
    TransactionsSyncRecorded,
    PriceUpdated(String),
    PriceMoved(String),
    DisclosureArrived(UiDisclosureKey),
    NewsActivityIncreased(String),
    ProposalCreated(String),
    ProposalExpired(String),
    ProposalApproved(String),
    ProposalRejected(String),
    ProposalSkipped(String),
    OrderSubmitted(String),
    OrderOutcomeUnknown(String),
    BrokerConnectionChanged,
    SettingsChanged,
    PolicyChanged,
    OrderSwitchChanged,
    WatchlistChanged,
    ThesisUpdated(String),
    SelfCheckChanged,
    MarketSessionChanged(UiMarket),
    SignalRaised(String),
    SignalCleared(String),
    EstimateSubmitted(String),
    EstimateResolved(String),
    ValuationSnapshotTaken,
    OrderRejected(String),
    OrderOutcomeResolved(String),
    OrderFilled,
    OrderCancelled,
    OrderModified,
    CollectionFinished(UiInformationSource),
    NewsClusterGrew(String),
    AiBudgetChanged,
    AccountSelectionChanged,
    StoredDataChanged,
    FxRateUpdated(UiFxPair),
    DisclosureTriggerRecorded(UiDisclosureKey),
    SourceStatusChanged(UiDataSource),
}

impl From<DomainEvent> for UiEvent {
    fn from(value: DomainEvent) -> Self {
        match value {
            DomainEvent::PortfolioSyncRecorded(_) => Self::PortfolioSyncRecorded,
            DomainEvent::TransactionsSyncRecorded(_) => Self::TransactionsSyncRecorded,
            DomainEvent::PriceUpdated(id) => Self::PriceUpdated(id.as_str().to_owned()),
            DomainEvent::PriceMoved(id) => Self::PriceMoved(id.as_str().to_owned()),
            DomainEvent::DisclosureArrived(id) => Self::DisclosureArrived(id.into()),
            DomainEvent::NewsActivityIncreased(id) => {
                Self::NewsActivityIncreased(id.as_str().to_owned())
            }
            DomainEvent::ProposalCreated(id) => {
                Self::ProposalCreated(id.as_client_order_id().as_str().to_owned())
            }
            DomainEvent::ProposalExpired(id) => {
                Self::ProposalExpired(id.as_client_order_id().as_str().to_owned())
            }
            DomainEvent::ProposalApproved(id) => {
                Self::ProposalApproved(id.as_client_order_id().as_str().to_owned())
            }
            DomainEvent::ProposalRejected(id) => {
                Self::ProposalRejected(id.as_client_order_id().as_str().to_owned())
            }
            DomainEvent::ProposalSkipped(id) => {
                Self::ProposalSkipped(id.as_client_order_id().as_str().to_owned())
            }
            DomainEvent::OrderSubmitted(id) => Self::OrderSubmitted(id.as_str().to_owned()),
            DomainEvent::OrderOutcomeUnknown(id) => {
                Self::OrderOutcomeUnknown(id.as_str().to_owned())
            }
            DomainEvent::BrokerConnectionChanged => Self::BrokerConnectionChanged,
            DomainEvent::SettingsChanged => Self::SettingsChanged,
            DomainEvent::PolicyChanged => Self::PolicyChanged,
            DomainEvent::OrderSwitchChanged => Self::OrderSwitchChanged,
            DomainEvent::WatchlistChanged => Self::WatchlistChanged,
            DomainEvent::ThesisUpdated(id) => Self::ThesisUpdated(id.as_str().to_owned()),
            DomainEvent::SelfCheckChanged => Self::SelfCheckChanged,
            DomainEvent::MarketSessionChanged(id) => Self::MarketSessionChanged(id.into()),
            DomainEvent::SignalRaised(id) => Self::SignalRaised(id.as_str().to_owned()),
            DomainEvent::SignalCleared(id) => Self::SignalCleared(id.as_str().to_owned()),
            DomainEvent::EstimateSubmitted(id) => Self::EstimateSubmitted(id.as_str().to_owned()),
            DomainEvent::EstimateResolved(id) => Self::EstimateResolved(id.as_str().to_owned()),
            DomainEvent::ValuationSnapshotTaken(_) => Self::ValuationSnapshotTaken,
            DomainEvent::OrderRejected(id) => Self::OrderRejected(id.as_str().to_owned()),
            DomainEvent::OrderOutcomeResolved(id) => {
                Self::OrderOutcomeResolved(id.as_str().to_owned())
            }
            DomainEvent::OrderFilled(_) => Self::OrderFilled,
            DomainEvent::OrderCancelled(_) => Self::OrderCancelled,
            DomainEvent::OrderModified(_) => Self::OrderModified,
            DomainEvent::CollectionFinished(id) => Self::CollectionFinished(id.into()),
            DomainEvent::NewsClusterGrew(id) => Self::NewsClusterGrew(id.as_str().to_owned()),
            DomainEvent::AiBudgetChanged => Self::AiBudgetChanged,
            DomainEvent::AccountSelectionChanged => Self::AccountSelectionChanged,
            DomainEvent::StoredDataChanged => Self::StoredDataChanged,
            DomainEvent::FxRateUpdated(id) => Self::FxRateUpdated(id.into()),
            DomainEvent::DisclosureTriggerRecorded(id) => {
                Self::DisclosureTriggerRecorded(id.into())
            }
            DomainEvent::SourceStatusChanged(source) => Self::SourceStatusChanged(source.into()),
        }
    }
}

#[cfg(test)]
mod tests;
