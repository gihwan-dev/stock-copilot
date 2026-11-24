use crate::platform::notifier::{FilteredNotifier, UnavailableNotificationDelivery};
use crate::ports::PortTypes;
use crate::unavailable::{Unavailable, UnavailableFeed};
#[derive(Debug)]
pub struct ProductPorts;
impl PortTypes for ProductPorts {
    type BrokerRead = Unavailable;
    type MarketInfo = Unavailable;
    type MacroSeries = Unavailable;
    type NewsFeed = UnavailableFeed;
    type Fundamentals = Unavailable;
    type AiProvider = Unavailable;
    type ChatRepository = Unavailable;
    type ChatRunLedger = Unavailable;
    type ChatQueuePort = Unavailable;
    type WarmChatRuntime = Unavailable;
    type PortfolioRepository = Unavailable;
    type TransactionRepository = Unavailable;
    type MarketDataRepository = Unavailable;
    type NewsRepository = Unavailable;
    type ThesisRepository = Unavailable;
    type EstimateRepository = Unavailable;
    type SignalRepository = Unavailable;
    type ProposalRepository = Unavailable;
    type ValuationRepository = Unavailable;
    type SettingsRepository = Unavailable;
    type AuditLog = Unavailable;
    type SecretStore = Unavailable;
    type Clock = platform_macos::SystemClock;
    type Notifier = FilteredNotifier<UnavailableNotificationDelivery>;
    type UserPresence = platform_macos::UnavailablePresence;
}
