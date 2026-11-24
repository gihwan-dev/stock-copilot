use crate::platform::notifier::{FilteredNotifier, UnavailableNotificationDelivery};
use crate::ports::PortTypes;
use crate::sqlite_ports::SqliteRepositories;
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
    type ChatRepository = SqliteRepositories;
    type ChatRunLedger = SqliteRepositories;
    type ChatQueuePort = Unavailable;
    type WarmChatRuntime = Unavailable;
    type PortfolioRepository = Unavailable;
    type TransactionRepository = SqliteRepositories;
    type MarketDataRepository = SqliteRepositories;
    type NewsRepository = SqliteRepositories;
    type ThesisRepository = SqliteRepositories;
    type EstimateRepository = SqliteRepositories;
    type SignalRepository = SqliteRepositories;
    type ProposalRepository = SqliteRepositories;
    type ValuationRepository = SqliteRepositories;
    type SettingsRepository = SqliteRepositories;
    type AuditLog = SqliteRepositories;
    type SecretStore = Unavailable;
    type Clock = platform_macos::SystemClock;
    type Notifier = FilteredNotifier<UnavailableNotificationDelivery>;
    type UserPresence =
        platform_macos::MacOsRiskDialogPresence<platform_macos::UnavailableRiskDialogBackend>;
}
