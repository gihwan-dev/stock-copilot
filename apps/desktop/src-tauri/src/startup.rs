use crate::ports::{PortTypes, Ports};
use application::{PortfolioRepository, SettingsRepository, SystemPortError};
use domain::{AccountId, InvestorProfile, Portfolio, RiskSettings, SyncRecord};

#[derive(Debug)]
pub struct StoredPortfolio {
    pub account: AccountId,
    pub record: Result<Option<SyncRecord<Portfolio>>, SystemPortError>,
}

#[derive(Debug)]
pub struct MainScreenData {
    pub profile: Result<Option<InvestorProfile>, SystemPortError>,
    pub risk_settings: Result<Option<RiskSettings>, SystemPortError>,
    pub portfolios: Vec<StoredPortfolio>,
}

pub async fn prepare<P: PortTypes>(ports: &Ports<P>) -> MainScreenData {
    let profile = ports.settings.profile().await;
    let risk_settings = ports.settings.risk_settings().await;
    let mut portfolios = Vec::new();
    for account in &ports.saved_accounts.current() {
        portfolios.push(StoredPortfolio {
            account: account.clone(),
            record: ports.portfolios.latest(account).await,
        });
    }
    MainScreenData {
        profile,
        risk_settings,
        portfolios,
    }
}

pub async fn start<P: PortTypes, Show: FnOnce(MainScreenData) -> Result<(), tauri::Error>>(
    ports: &Ports<P>,
    show: Show,
) -> Result<
    tauri::async_runtime::JoinHandle<Result<domain::BrokerAccountRows, application::PortError>>,
    tauri::Error,
> {
    use application::BrokerRead;
    let data = prepare(ports).await;
    show(data)?;
    let broker = std::sync::Arc::clone(&ports.broker_read);
    Ok(tauri::async_runtime::spawn(async move {
        broker.accounts().await
    }))
}
