use crate::{
    AdjustmentBasis, Candle, CandlePeriod, CandleStatus, EstimateContext, Market, MarketCalendar,
    MarketState, MasterInstrument, PriceAdjustment, Timestamp,
};

#[must_use]
pub fn closed_estimate_reference(
    master: &MasterInstrument,
    at: Timestamp,
    calendar: &MarketCalendar,
    candles: &[Candle],
) -> Option<EstimateContext> {
    let instrument = master.quote_instrument().ok()?;
    if calendar.today().market() != Market::from(*instrument.market())
        || calendar.state_at(at) != MarketState::Closed
    {
        return None;
    }
    let (close, session_date) = [
        calendar.previous_business_day(),
        calendar.today(),
        calendar.next_business_day(),
    ]
    .into_iter()
    .flat_map(|row| {
        row.sessions()
            .iter()
            .filter(|segment| segment.state() == MarketState::RegularMarket)
            .map(move |segment| (segment.session().interval().end(), row.date()))
    })
    .filter(|(end, _date)| *end <= at)
    .max()?;
    let mut matching = candles.iter().filter(|candle| {
        candle.instrument() == &master.row().instrument_id
            && candle.time().date() == session_date
            && candle.time().period() == CandlePeriod::Daily
            && candle.status() == CandleStatus::Finalized
            && candle.adjustment() == PriceAdjustment::Adjusted(AdjustmentBasis::new(at))
    });
    let candle = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let local = at.in_exchange_zone(*instrument.market()).ok()?;
    let mark = master
        .price_observation(candle.values().close(), local)
        .ok()?;
    Some(
        EstimateContext::new(mark, local)
            .with_close(Some(close.in_exchange_zone(*instrument.market()).ok()?)),
    )
}

#[cfg(test)]
mod tests;
