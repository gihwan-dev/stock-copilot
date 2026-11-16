use crate::{
    database::DatabaseFactory,
    ports::{PortTypes, Ports},
    unavailable::{Unavailable, UnavailableFeed},
};
use std::sync::Arc;
#[derive(Debug)]
pub(super) struct TestPorts;
impl PortTypes for TestPorts {
    type BrokerRead = testkit_ports::scripted::ScriptedBrokerRead;
    type MarketInfo = Unavailable;
    type MacroSeries = Unavailable;
    type NewsFeed = UnavailableFeed;
    type Fundamentals = Unavailable;
    type AiProvider = Unavailable;
    type PortfolioRepository = testkit_ports::memory::MemoryPortfolioRepository;
    type TransactionRepository = Unavailable;
    type MarketDataRepository = Unavailable;
    type NewsRepository = Unavailable;
    type ThesisRepository = Unavailable;
    type EstimateRepository = Unavailable;
    type SignalRepository = Unavailable;
    type ProposalRepository = Unavailable;
    type ValuationRepository = Unavailable;
    type SettingsRepository = testkit_ports::memory::MemorySettingsRepository;
    type AuditLog = Unavailable;
    type SecretStore = testkit_ports::system_fakes::MemorySecretStore;
    type Clock = testkit_ports::SteppedClock;
    type Notifier = Unavailable;
    type UserPresence = testkit_ports::ScriptedPresence;
}
pub(super) fn ports() -> Ports<TestPorts> {
    Ports {
        broker_read: Arc::new(testkit_ports::scripted::ScriptedBrokerRead::default()),
        market_info: Arc::new(Unavailable),
        macro_series: Arc::new(Unavailable),
        news_feeds: Vec::new(),
        fundamentals: Arc::new(Unavailable),
        ai_provider: Arc::new(Unavailable),
        portfolios: Arc::new(testkit_ports::memory::MemoryPortfolioRepository::default()),
        transactions: Arc::new(Unavailable),
        market_data: Arc::new(Unavailable),
        news: Arc::new(Unavailable),
        theses: Arc::new(Unavailable),
        estimates: Arc::new(Unavailable),
        signals: Arc::new(Unavailable),
        proposals: Arc::new(Unavailable),
        valuations: Arc::new(Unavailable),
        settings: Arc::new(testkit_ports::memory::MemorySettingsRepository::default()),
        audit: Arc::new(Unavailable),
        secrets: Arc::new(testkit_ports::system_fakes::MemorySecretStore::default()),
        clock: Arc::new(testkit_ports::SteppedClock::new(at(91))),
        notifier: Arc::new(Unavailable),
        presence: Arc::new(testkit_ports::ScriptedPresence::new([], [])),
        events: application::EventBusBuilder::default().build(),
        saved_accounts: Vec::new(),
        database: Arc::new(DatabaseFactory(Box::new(|_path, _key| {
            Err(failure(application::SystemFailure::InvalidData))
        }))),
    }
}
pub(super) fn failure(reason: application::SystemFailure) -> application::SystemPortError {
    application::SystemPortError::new(application::SystemPort::Repository, reason)
}
pub(super) fn at(seconds: i64) -> domain::Timestamp {
    domain::Timestamp::from_parts(
        domain::UnixSeconds::new(seconds),
        domain::Nanosecond::new(0),
        domain::OffsetSeconds::new(0),
    )
    .expect("시각")
}
