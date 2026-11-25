use crate::platform::notifier::{FilteredNotifier, UnavailableNotificationDelivery};
use crate::ports::PortTypes;
use crate::unavailable::{Unavailable, UnavailableFeed};
use application::{
    AiProvider, AuditLog, ChatNumberEvidence, ChatQueuePort, ChatRepository, ChatRunLedger,
    EstimateRepository, MarketDataRepository, NewsRepository, ProposalRepository,
    SettingsRepository, SignalRepository, ThesisRepository, TransactionRepository,
    ValuationRepository,
};
use std::marker::PhantomData;
type PortMarker<Store, Secrets, Ai, Queue, Jobs> = fn() -> (Store, Secrets, Ai, Queue, Jobs);
#[derive(Debug)]
pub struct ProductPorts<Store, Secrets, Ai, Queue, Jobs>(
    PhantomData<PortMarker<Store, Secrets, Ai, Queue, Jobs>>,
);
impl<Store, Secrets, Ai, Queue, Jobs> PortTypes for ProductPorts<Store, Secrets, Ai, Queue, Jobs>
where
    Ai: AiProvider + 'static,
    Queue: ChatQueuePort + 'static,
    Jobs: scheduler::AiJobQueue + 'static,
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
    Secrets: application::SecretStore + 'static,
{
    type BrokerRead = Unavailable;
    type MarketInfo = Unavailable;
    type MacroSeries = Unavailable;
    type NewsFeed = UnavailableFeed;
    type Fundamentals = Unavailable;
    type AiProvider = Ai;
    type ChatRepository = Store;
    type ChatRunLedger = Store;
    type ChatQueuePort = Queue;
    type WarmChatRuntime = Unavailable;
    type AiJobQueue = Jobs;
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
    type SecretStore = Secrets;
    type Clock = platform_macos::SystemClock;
    type Notifier = FilteredNotifier<UnavailableNotificationDelivery>;
    type UserPresence =
        platform_macos::MacOsRiskDialogPresence<platform_macos::UnavailableRiskDialogBackend>;
}
