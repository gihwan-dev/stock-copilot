use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use application::{
    ChangeRiskSettings, RiskSettingsChangeOutcome, SecretKey, SecretStore, SettingsRepository,
    SystemPortError, TransactionRepository,
};
use domain::{InvestorProfile, RiskSettings, SettledRowSet, SyncRecord};

use crate::ports::{PortTypes, Ports};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPresence {
    Configured,
    NotConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSettings {
    pub profile: Option<InvestorProfile>,
    pub risk_settings: Option<RiskSettings>,
    pub toss_connected: SecretPresence,
    pub dart_key: SecretPresence,
    pub fred_key: SecretPresence,
    pub account_count: u16,
}

pub trait ScreenBackend: Send + Sync {
    fn load_settings(&self) -> BoxFuture<'_, Result<LoadedSettings, SystemPortError>>;
    fn change_risk_settings(
        &self,
        requested: RiskSettings,
    ) -> BoxFuture<'_, Result<RiskSettingsChangeOutcome, SystemPortError>>;
    fn load_portfolio_view(
        &self,
    ) -> BoxFuture<'_, Result<application::StoredView<domain::PortfolioValuation>, SystemPortError>>;
    fn load_allocation_view(
        &self,
    ) -> BoxFuture<'_, Result<application::StoredView<domain::Allocation>, SystemPortError>>;
    fn load_transactions(
        &self,
    ) -> BoxFuture<'_, Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>>;
}

struct PortsBackend<P: PortTypes> {
    settings: Arc<P::SettingsRepository>,
    secrets: Arc<P::SecretStore>,
    portfolios: Arc<P::PortfolioRepository>,
    transactions: Arc<P::TransactionRepository>,
    market_data: Arc<P::MarketDataRepository>,
    presence: Arc<P::UserPresence>,
    events: application::EventBus,
    saved_accounts: Vec<domain::AccountId>,
}

async fn secret_presence<S: SecretStore>(store: &S, key: SecretKey) -> SecretPresence {
    match store.load(key).await {
        Ok(Some(_)) => SecretPresence::Configured,
        Ok(None) | Err(_) => SecretPresence::NotConfigured,
    }
}

fn saturating_account_count(count: usize) -> u16 {
    u16::try_from(count).ok().unwrap_or(u16::MAX)
}

impl<P: PortTypes> ScreenBackend for PortsBackend<P> {
    fn load_settings(&self) -> BoxFuture<'_, Result<LoadedSettings, SystemPortError>> {
        Box::pin(async move {
            let profile = match self.settings.profile().await {
                Ok(value) => value,
                Err(error)
                    if error.failure()
                        == application::SystemFailure::Unavailable(
                            application::RetryDelay::Unknown,
                        ) =>
                {
                    None
                }
                Err(error) => return Err(error),
            };
            let risk_settings = match self.settings.risk_settings().await {
                Ok(value) => value,
                Err(error)
                    if error.failure()
                        == application::SystemFailure::Unavailable(
                            application::RetryDelay::Unknown,
                        ) =>
                {
                    None
                }
                Err(error) => return Err(error),
            };
            let toss_connected =
                secret_presence(self.secrets.as_ref(), SecretKey::TossClientId).await;
            let dart_key = secret_presence(self.secrets.as_ref(), SecretKey::DartApiKey).await;
            let fred_key = secret_presence(self.secrets.as_ref(), SecretKey::FredApiKey).await;
            Ok(LoadedSettings {
                profile,
                risk_settings,
                toss_connected,
                dart_key,
                fred_key,
                account_count: saturating_account_count(self.saved_accounts.len()),
            })
        })
    }

    fn change_risk_settings(
        &self,
        requested: RiskSettings,
    ) -> BoxFuture<'_, Result<RiskSettingsChangeOutcome, SystemPortError>> {
        Box::pin(async move {
            ChangeRiskSettings::new(self.settings.as_ref(), self.presence.as_ref(), &self.events)
                .execute(requested)
                .await
        })
    }

    fn load_portfolio_view(
        &self,
    ) -> BoxFuture<'_, Result<application::StoredView<domain::PortfolioValuation>, SystemPortError>>
    {
        Box::pin(async move {
            let Some(account) = self.saved_accounts.first() else {
                return Ok(application::StoredView::Missing);
            };
            application::GetPortfolioView::new(self.portfolios.as_ref(), self.market_data.as_ref())
                .execute(account)
                .await
        })
    }

    fn load_allocation_view(
        &self,
    ) -> BoxFuture<'_, Result<application::StoredView<domain::Allocation>, SystemPortError>> {
        Box::pin(async move {
            let Some(account) = self.saved_accounts.first() else {
                return Ok(application::StoredView::Missing);
            };
            application::GetAllocationView::new(self.portfolios.as_ref(), self.market_data.as_ref())
                .execute(account)
                .await
        })
    }

    fn load_transactions(
        &self,
    ) -> BoxFuture<'_, Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>> {
        Box::pin(async move {
            let Some(account) = self.saved_accounts.first() else {
                return Ok(None);
            };
            self.transactions.latest(account).await
        })
    }
}

pub struct ScreenState {
    backend: Arc<dyn ScreenBackend>,
}

impl std::fmt::Debug for ScreenState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScreenState(<redacted>)")
    }
}

impl ScreenState {
    #[must_use]
    pub fn from_ports<P: PortTypes>(ports: &Ports<P>) -> Self {
        Self {
            backend: Arc::new(PortsBackend::<P> {
                settings: Arc::clone(&ports.settings),
                secrets: Arc::clone(&ports.secrets),
                portfolios: Arc::clone(&ports.portfolios),
                transactions: Arc::clone(&ports.transactions),
                market_data: Arc::clone(&ports.market_data),
                presence: Arc::clone(&ports.presence),
                events: application::EventBusBuilder::default().build(),
                saved_accounts: ports.saved_accounts.clone(),
            }),
        }
    }

    #[must_use]
    pub fn backend(&self) -> &dyn ScreenBackend {
        self.backend.as_ref()
    }
}
