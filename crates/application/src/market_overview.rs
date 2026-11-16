use crate::{
    BrokerRead, Clock, DataAvailability, InformationError, MacroObservation, MacroSeries,
    MacroSeriesId, MarketInfo, PortError, QuoteObservation,
};
use domain::{
    CalendarDate, FxPair, FxRate, FxRateKind, IndicatorCode, KnownMarket, MarketCalendar,
    MarketIndicatorQuote, Nanosecond, QuoteInstrument, SpanSeconds, TimeSpan, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewRequest {
    Indicator(IndicatorCode),
    EtfApproximation(QuoteInstrument),
    Macro {
        series: MacroSeriesId,
        from: CalendarDate,
        through: CalendarDate,
    },
    Fx {
        pair: FxPair,
        kind: FxRateKind,
    },
    Calendar(KnownMarket),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewFreshness {
    WithinFiveMinutes,
    OlderThanFiveMinutes,
    Unknown,
    DailyPolicyPending,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewObservation<T> {
    pub value: T,
    pub collected_at: Timestamp,
    pub freshness: OverviewFreshness,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewData {
    Indicators(Vec<OverviewObservation<MarketIndicatorQuote>>),
    EtfApproximation(Vec<OverviewObservation<QuoteObservation>>),
    Macro(DataAvailability<Vec<OverviewObservation<MacroObservation>>>),
    Fx(OverviewObservation<FxRate>),
    Calendar(Box<OverviewObservation<MarketCalendar>>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewError {
    Broker(PortError),
    Information(InformationError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewEntry {
    pub request: OverviewRequest,
    pub result: Result<OverviewData, OverviewError>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOverview {
    pub entries: Vec<OverviewEntry>,
}
#[derive(Debug)]
pub struct GetMarketOverview<'a, B, M, S, C> {
    pub broker: &'a B,
    pub market: &'a M,
    pub macro_series: &'a S,
    pub clock: &'a C,
}
impl<B: BrokerRead, M: MarketInfo, S: MacroSeries, C: Clock> GetMarketOverview<'_, B, M, S, C> {
    pub async fn execute(&self, requests: &[OverviewRequest]) -> MarketOverview {
        let mut entries = Vec::new();
        for request in requests {
            let result = self.read(request).await;
            entries.push(OverviewEntry {
                request: request.clone(),
                result,
            });
        }
        MarketOverview { entries }
    }
    async fn read_fx(&self, pair: FxPair, kind: FxRateKind) -> Result<OverviewData, OverviewError> {
        let value = self
            .market
            .fx_rate(pair, kind, None)
            .await
            .map_err(OverviewError::Broker)?;
        let collected_at = self.clock.now();
        Ok(OverviewData::Fx(OverviewObservation {
            freshness: freshness(Some(value.as_of()), collected_at),
            value,
            collected_at,
        }))
    }

    async fn read(&self, request: &OverviewRequest) -> Result<OverviewData, OverviewError> {
        match request {
            OverviewRequest::Indicator(code) => {
                let values = self
                    .market
                    .indicator_quotes(&[*code])
                    .await
                    .map_err(OverviewError::Broker)?;
                let now = self.clock.now();
                Ok(OverviewData::Indicators(
                    values
                        .into_iter()
                        .map(|value| indicator_observation(value, now))
                        .collect(),
                ))
            }
            OverviewRequest::EtfApproximation(instrument) => {
                let values = self
                    .broker
                    .quotes(std::slice::from_ref(instrument))
                    .await
                    .map_err(OverviewError::Broker)?;
                let now = self.clock.now();
                Ok(OverviewData::EtfApproximation(
                    values
                        .into_iter()
                        .map(|value| etf_observation(value, now))
                        .collect(),
                ))
            }
            OverviewRequest::Macro {
                series,
                from,
                through,
            } => {
                let values = self
                    .macro_series
                    .daily_values(series, *from, *through)
                    .await
                    .map_err(OverviewError::Information)?;
                Ok(OverviewData::Macro(macro_observations(
                    values,
                    self.clock.now(),
                )))
            }
            OverviewRequest::Fx { pair, kind } => self.read_fx(*pair, *kind).await,
            OverviewRequest::Calendar(market) => {
                let value = self
                    .market
                    .calendar(*market)
                    .await
                    .map_err(OverviewError::Broker)?;
                Ok(OverviewData::Calendar(Box::new(OverviewObservation {
                    value,
                    collected_at: self.clock.now(),
                    freshness: OverviewFreshness::Unknown,
                })))
            }
        }
    }
}
fn indicator_observation(
    value: MarketIndicatorQuote,
    now: Timestamp,
) -> OverviewObservation<MarketIndicatorQuote> {
    let observation = value.observation();
    OverviewObservation {
        freshness: freshness(observation.server_at(), now),
        collected_at: observation.collected_at(),
        value,
    }
}
fn etf_observation(
    value: QuoteObservation,
    now: Timestamp,
) -> OverviewObservation<QuoteObservation> {
    let (collected_at, state) = match &value.result {
        Ok(quote) => (quote.collected_at(), freshness(quote.timestamp(), now)),
        Err(_) => (now, OverviewFreshness::Unknown),
    };
    OverviewObservation {
        value,
        collected_at,
        freshness: state,
    }
}
fn macro_observations(
    values: DataAvailability<Vec<MacroObservation>>,
    collected_at: Timestamp,
) -> DataAvailability<Vec<OverviewObservation<MacroObservation>>> {
    match values {
        DataAvailability::Available(rows) => DataAvailability::Available(
            rows.into_iter()
                .map(|value| OverviewObservation {
                    value,
                    collected_at,
                    freshness: OverviewFreshness::DailyPolicyPending,
                })
                .collect(),
        ),
        DataAvailability::NotConfigured => DataAvailability::NotConfigured,
        DataAvailability::NoData => DataAvailability::NoData,
    }
}

fn freshness(stamp: Option<Timestamp>, now: Timestamp) -> OverviewFreshness {
    let Some(at) = stamp else {
        return OverviewFreshness::Unknown;
    };
    match (
        at.elapsed_until(now),
        TimeSpan::try_new(SpanSeconds::new(300), Nanosecond::new(0)),
    ) {
        (Ok(age), Ok(limit)) if age > limit => OverviewFreshness::OlderThanFiveMinutes,
        (Ok(_), Ok(_)) => OverviewFreshness::WithinFiveMinutes,
        _ => OverviewFreshness::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{OverviewFreshness, freshness};
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn freshness_obeys_exact_boundary_and_unknown_time() {
        for (stamp, expected) in [
            (None, OverviewFreshness::Unknown),
            (Some(at(1001)), OverviewFreshness::Unknown),
            (Some(at(1000)), OverviewFreshness::WithinFiveMinutes),
            (Some(at(701)), OverviewFreshness::WithinFiveMinutes),
            (Some(at(700)), OverviewFreshness::WithinFiveMinutes),
            (Some(at(699)), OverviewFreshness::OlderThanFiveMinutes),
        ] {
            assert_eq!(freshness(stamp, at(1000)), expected);
        }
    }
}
