use crate::{FxRateQuery, MarketDataRepository, PortfolioRepository, SystemPortError};
use domain::{
    AccountId, Allocation, FxPair, FxRate, FxRateKind, FxValidity, PortfolioValuation,
    SectorAssignments, SyncOutcome, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredView<Value> {
    Missing,
    Recorded(RecordedView<Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewFreshness {
    DefinitionPending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedView<Value> {
    freshness: ViewFreshness,
    recorded_at: Timestamp,
    outcome: SyncOutcome<Value>,
    exchange_rate: Result<Option<FxRate>, SystemPortError>,
}
impl<Value> RecordedView<Value> {
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
    #[must_use]
    pub const fn outcome(&self) -> &SyncOutcome<Value> {
        &self.outcome
    }
    pub const fn exchange_rate(&self) -> &Result<Option<FxRate>, SystemPortError> {
        &self.exchange_rate
    }
    #[must_use]
    pub const fn freshness(&self) -> ViewFreshness {
        self.freshness
    }
}

#[derive(Debug)]
pub struct GetPortfolioView<'a, Repository, Market> {
    repository: &'a Repository,
    market: &'a Market,
}
impl<'a, Repository: PortfolioRepository, Market: MarketDataRepository>
    GetPortfolioView<'a, Repository, Market>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, market: &'a Market) -> Self {
        Self { repository, market }
    }
    pub async fn execute(
        &self,
        account: &AccountId,
    ) -> Result<StoredView<PortfolioValuation>, SystemPortError> {
        let Some(record) = self.repository.latest(account).await? else {
            return Ok(StoredView::Missing);
        };
        let exchange_rate = match record.outcome() {
            SyncOutcome::Complete(_) | SyncOutcome::Partial(_) => {
                self.market
                    .fx_rate(
                        FxPair::UsdKrw,
                        FxRateKind::Mid,
                        FxRateQuery::At(record.recorded_at()),
                    )
                    .await
            }
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => Ok(None),
        };
        let rate = usable_rate(&exchange_rate, record.recorded_at());
        let outcome = match record.outcome() {
            SyncOutcome::Complete(value) => SyncOutcome::Complete(value.evaluate(rate)),
            SyncOutcome::Partial(value) => SyncOutcome::Partial(value.evaluate(rate)),
            SyncOutcome::Failed => SyncOutcome::Failed,
            SyncOutcome::AccessBlocked => SyncOutcome::AccessBlocked,
        };
        Ok(StoredView::Recorded(RecordedView {
            freshness: ViewFreshness::DefinitionPending,
            recorded_at: record.recorded_at(),
            outcome,
            exchange_rate,
        }))
    }
}

#[derive(Debug)]
pub struct GetAllocationView<'a, Repository, Market> {
    portfolio: GetPortfolioView<'a, Repository, Market>,
}
impl<'a, Repository: PortfolioRepository, Market: MarketDataRepository>
    GetAllocationView<'a, Repository, Market>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, market: &'a Market) -> Self {
        Self {
            portfolio: GetPortfolioView::new(repository, market),
        }
    }
    pub async fn execute(
        &self,
        account: &AccountId,
    ) -> Result<StoredView<Allocation>, SystemPortError> {
        let StoredView::Recorded(view) = self.portfolio.execute(account).await? else {
            return Ok(StoredView::Missing);
        };
        let assignments = SectorAssignments::default();
        let outcome = match view.outcome {
            SyncOutcome::Complete(value) => {
                SyncOutcome::Complete(Allocation::new(&value, &assignments))
            }
            SyncOutcome::Partial(value) => {
                SyncOutcome::Partial(Allocation::new(&value, &assignments))
            }
            SyncOutcome::Failed => SyncOutcome::Failed,
            SyncOutcome::AccessBlocked => SyncOutcome::AccessBlocked,
        };
        Ok(StoredView::Recorded(RecordedView {
            freshness: view.freshness,
            recorded_at: view.recorded_at,
            outcome,
            exchange_rate: view.exchange_rate,
        }))
    }
}

fn usable_rate(lookup: &Result<Option<FxRate>, SystemPortError>, at: Timestamp) -> Option<FxRate> {
    lookup.as_ref().ok().copied().flatten().filter(|rate| {
        rate.pair() == FxPair::UsdKrw
            && rate.kind() == FxRateKind::Mid
            && rate.validity_at(at) == FxValidity::Valid
    })
}

#[cfg(test)]
mod tests {
    use super::usable_rate;
    use crate::{SystemFailure, SystemPort, SystemPortError};
    use domain::{
        FxPair, FxRate, FxRateKind, FxSource, Nanosecond, NumericInput, OffsetSeconds,
        TimeInterval, Timestamp, UnixSeconds,
    };
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn expected_rate(rate: FxRate, timestamp: Timestamp) -> Option<FxRate> {
        match (rate.pair(), rate.kind()) {
            (FxPair::UsdKrw, FxRateKind::Mid) => {
                [at(100), at(173)].contains(&timestamp).then_some(rate)
            }
            (FxPair::UsdKrw, FxRateKind::Buy)
            | (FxPair::KrwUsd, FxRateKind::Mid | FxRateKind::Buy) => None,
        }
    }
    #[test]
    fn only_mid_usd_krw_inside_half_open_validity_is_usable() {
        for (pair, kind) in [
            (FxPair::UsdKrw, FxRateKind::Mid),
            (FxPair::UsdKrw, FxRateKind::Buy),
            (FxPair::KrwUsd, FxRateKind::Mid),
            (FxPair::KrwUsd, FxRateKind::Buy),
        ] {
            let rate = FxRate::parse(
                &NumericInput::try_new("13").expect("입력"),
                pair,
                kind,
                TimeInterval::try_new(at(100), at(200)).expect("구간"),
                FxSource::TossOpenApi,
            )
            .expect("환율");
            for timestamp in [at(99), at(100), at(173), at(200), at(201)] {
                assert_eq!(
                    usable_rate(&Ok(Some(rate)), timestamp),
                    expected_rate(rate, timestamp)
                );
            }
        }
        assert_eq!(usable_rate(&Ok(None), at(123)), None);
        assert_eq!(
            usable_rate(
                &Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::InvalidData
                )),
                at(123)
            ),
            None
        );
    }
}
