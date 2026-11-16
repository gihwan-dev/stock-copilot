use crate::database::DatabaseFactory;
use application::user_presence::UserPresence;
use application::{
    AiProvider, AuditLog, BrokerRead, Clock, EstimateRepository, EventBus, Fundamentals,
    MacroSeries, MarketDataRepository, MarketInfo, NewsFeed, NewsRepository, Notifier,
    PortfolioRepository, ProposalRepository, SecretStore, SettingsRepository, SignalRepository,
    ThesisRepository, TransactionRepository, ValuationRepository,
};
use domain::AccountId;
use std::sync::Arc;
pub trait PortTypes: Send + Sync + 'static {
    type BrokerRead: BrokerRead + 'static;
    type MarketInfo: MarketInfo + 'static;
    type MacroSeries: MacroSeries + 'static;
    type NewsFeed: NewsFeed + 'static;
    type Fundamentals: Fundamentals + 'static;
    type AiProvider: AiProvider + 'static;
    type PortfolioRepository: PortfolioRepository + 'static;
    type TransactionRepository: TransactionRepository + 'static;
    type MarketDataRepository: MarketDataRepository + 'static;
    type NewsRepository: NewsRepository + 'static;
    type ThesisRepository: ThesisRepository + 'static;
    type EstimateRepository: EstimateRepository + 'static;
    type SignalRepository: SignalRepository + 'static;
    type ProposalRepository: ProposalRepository + 'static;
    type ValuationRepository: ValuationRepository + 'static;
    type SettingsRepository: SettingsRepository + 'static;
    type AuditLog: AuditLog + 'static;
    type SecretStore: SecretStore + 'static;
    type Clock: Clock + 'static;
    type Notifier: Notifier + 'static;
    type UserPresence: UserPresence + 'static;
}
pub struct Ports<P: PortTypes> {
    pub broker_read: Arc<P::BrokerRead>,
    pub market_info: Arc<P::MarketInfo>,
    pub macro_series: Arc<P::MacroSeries>,
    pub news_feeds: Vec<Arc<P::NewsFeed>>,
    pub fundamentals: Arc<P::Fundamentals>,
    pub ai_provider: Arc<P::AiProvider>,
    pub portfolios: Arc<P::PortfolioRepository>,
    pub transactions: Arc<P::TransactionRepository>,
    pub market_data: Arc<P::MarketDataRepository>,
    pub news: Arc<P::NewsRepository>,
    pub theses: Arc<P::ThesisRepository>,
    pub estimates: Arc<P::EstimateRepository>,
    pub signals: Arc<P::SignalRepository>,
    pub proposals: Arc<P::ProposalRepository>,
    pub valuations: Arc<P::ValuationRepository>,
    pub settings: Arc<P::SettingsRepository>,
    pub audit: Arc<P::AuditLog>,
    pub secrets: Arc<P::SecretStore>,
    pub clock: Arc<P::Clock>,
    pub notifier: Arc<P::Notifier>,
    pub presence: Arc<P::UserPresence>,
    pub events: EventBus,
    pub saved_accounts: Vec<AccountId>,
    pub database: Arc<DatabaseFactory>,
}
impl<P: PortTypes> std::fmt::Debug for Ports<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ports(<redacted>)")
    }
}
