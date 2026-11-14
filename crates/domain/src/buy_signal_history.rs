use crate::{
    BuySignal, MarketCalendar, NoBuyQuantity, ResolvedOrderInstrument, TimeInterval, Timestamp,
};

#[derive(Clone, PartialEq, Eq)]
pub struct BuySignalRecord {
    instrument: ResolvedOrderInstrument,
    interval: TimeInterval,
    raised_at: Timestamp,
}
impl std::fmt::Debug for BuySignalRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BuySignalRecord(..)")
    }
}
impl BuySignalRecord {
    pub fn from_signal(
        signal: &BuySignal,
        raised_at: Timestamp,
        calendars: &[MarketCalendar],
    ) -> Result<Self, NoBuyQuantity> {
        let task = signal.facts().task();
        crate::SignalTiming::new(raised_at, calendars)
            .check(task, signal.facts().submission().submitted_at())
            .or(Err(NoBuyQuantity::Indeterminate))?;
        crate::estimate_outcome::buy_signal_interval_from(task, raised_at, calendars)
            .or(Err(NoBuyQuantity::Indeterminate))
            .map(|interval| Self {
                instrument: task.reference().instrument().clone(),
                interval,
                raised_at,
            })
    }
    #[must_use]
    pub const fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn interval(&self) -> TimeInterval {
        self.interval
    }
    #[must_use]
    pub const fn raised_at(&self) -> Timestamp {
        self.raised_at
    }
}

pub(crate) fn check_buy_history(
    signal: &BuySignal,
    now: Timestamp,
    records: &[BuySignalRecord],
) -> Result<(), NoBuyQuantity> {
    if records.len() > 65_536 {
        return Err(NoBuyQuantity::Indeterminate);
    }
    if records.iter().any(|record| record.raised_at > now) {
        return Err(NoBuyQuantity::Indeterminate);
    }
    let instrument = signal.facts().task().reference().instrument();
    if records.iter().any(|record| {
        record.instrument.market() == instrument.market()
            && record.instrument.symbol() == instrument.symbol()
            && now >= record.interval.start()
            && now < record.interval.end()
    }) {
        return Err(NoBuyQuantity::RepeatedSignal);
    }
    Ok(())
}
