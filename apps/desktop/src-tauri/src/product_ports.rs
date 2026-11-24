use crate::platform::notifier::{FilteredNotifier, UnavailableNotificationDelivery};
use crate::ports::PortTypes;
use crate::unavailable::{Unavailable, UnavailableFeed};
use application::{
    AuditLog, ChatNumberEvidence, ChatRepository, ChatRunLedger, EstimateRepository,
    MarketDataRepository, NewsRepository, ProposalRepository, SettingsRepository, SignalRepository,
    ThesisRepository, TransactionRepository, ValuationRepository,
};
use std::marker::PhantomData;
#[derive(Debug)]
pub struct ProductPorts<Store>(PhantomData<fn() -> Store>);
impl<Store> PortTypes for ProductPorts<Store>
where
    Store: ChatRepository
        + ChatRunLedger
        + ChatNumberEvidence
        + TransactionRepository
        + MarketDataRepository
        + NewsRepository
        + ThesisRepository
        + EstimateRepository
        + SignalRepository
        + ProposalRepository
        + ValuationRepository
        + SettingsRepository
        + AuditLog
        + 'static,
{
    type BrokerRead = Unavailable;
    type MarketInfo = Unavailable;
    type MacroSeries = Unavailable;
    type NewsFeed = UnavailableFeed;
    type Fundamentals = Unavailable;
    type AiProvider = Unavailable;
    type ChatRepository = Store;
    type ChatRunLedger = Store;
    type ChatQueuePort = Unavailable;
    type WarmChatRuntime = Unavailable;
    type PortfolioRepository = Unavailable;
    type TransactionRepository = Store;
    type MarketDataRepository = Store;
    type NewsRepository = Store;
    type ThesisRepository = Store;
    type EstimateRepository = Store;
    type SignalRepository = Store;
    type ProposalRepository = Store;
    type ValuationRepository = Store;
    type SettingsRepository = Store;
    type AuditLog = Store;
    type SecretStore = Unavailable;
    type Clock = platform_macos::SystemClock;
    type Notifier = FilteredNotifier<UnavailableNotificationDelivery>;
    type UserPresence =
        platform_macos::MacOsRiskDialogPresence<platform_macos::UnavailableRiskDialogBackend>;
}
