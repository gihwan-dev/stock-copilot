use application::{
    BriefingContext, BriefingContextSource, PortfolioRepository, RetryDelay, SystemFailure,
    SystemPort, SystemPortError,
};
use domain::{AccountId, BriefingKind, Symbol, SyncOutcome, Timestamp};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug)]
pub struct HoldingsBriefingContext<P> {
    portfolios: Arc<P>,
    accounts: Vec<AccountId>,
}

impl<P> HoldingsBriefingContext<P> {
    #[must_use]
    pub const fn new(portfolios: Arc<P>, accounts: Vec<AccountId>) -> Self {
        Self {
            portfolios,
            accounts,
        }
    }
}

const fn no_holdings() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

impl<P: PortfolioRepository> BriefingContextSource for HoldingsBriefingContext<P> {
    async fn context(
        &self,
        _kind: BriefingKind,
        _now: Timestamp,
    ) -> Result<BriefingContext, SystemPortError> {
        let mut symbols: BTreeSet<Symbol> = BTreeSet::new();
        let mut seen = 0_usize;
        for account in &self.accounts {
            let Some(record) = self.portfolios.latest(account).await? else {
                continue;
            };
            match record.outcome() {
                SyncOutcome::Complete(portfolio) | SyncOutcome::Partial(portfolio) => {
                    seen = seen.saturating_add(1);
                    symbols.extend(
                        portfolio
                            .holdings()
                            .iter()
                            .map(|holding| holding.instrument().symbol().clone()),
                    );
                }
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => {}
            }
        }
        if seen == 0_usize {
            return Err(no_holdings());
        }
        Ok(BriefingContext {
            instruments: symbols.into_iter().collect(),
            evidence: Vec::new(),
            code_facts: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::HoldingsBriefingContext;
    use application::BriefingContextSource;
    use domain::{AccountId, BriefingKind};
    use std::sync::Arc;
    use testkit_ports::memory::MemoryPortfolioRepository;

    #[test]
    fn briefing_waits_for_a_portfolio_snapshot_before_calling_ai() {
        let now = domain::Timestamp::from_parts(
            domain::UnixSeconds::new(1_790_000_000_i64),
            domain::Nanosecond::new(0_u32),
            domain::OffsetSeconds::new(0_i32),
        )
        .expect("시각");
        for accounts in [Vec::new(), vec![AccountId::new(1)]] {
            let portfolios = MemoryPortfolioRepository::default();
            for _account in &accounts {
                portfolios
                    .script_latest()
                    .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
            }
            let context = HoldingsBriefingContext::new(Arc::new(portfolios), accounts);
            assert!(
                tauri::async_runtime::block_on(context.context(BriefingKind::Morning, now))
                    .is_err(),
                "보유 스냅샷이 없으면 문맥을 만들지 않는다"
            );
        }
    }
}
