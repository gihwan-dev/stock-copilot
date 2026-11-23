use crate::{
    BuySignal, MarketCalendar, NoBuyQuantity, ResolvedOrderInstrument, TimeInterval, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuySignalOccurrenceId(String);
impl BuySignalOccurrenceId {
    pub fn try_new(value: &str) -> Result<Self, NoBuyQuantity> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(NoBuyQuantity::Indeterminate);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct BuySignalRecord {
    id: BuySignalOccurrenceId,
    instrument_id: crate::InstrumentId,
    instrument: ResolvedOrderInstrument,
    originating_task: crate::SignalTaskKey,
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
        id: BuySignalOccurrenceId,
        signal: &BuySignal,
        raised_at: Timestamp,
        calendars: &[MarketCalendar],
    ) -> Result<Self, NoBuyQuantity> {
        Self::from_signal_checked(id, signal, raised_at, calendars)
            .or(Err(NoBuyQuantity::Indeterminate))
    }
    pub fn from_signal_checked(
        id: BuySignalOccurrenceId,
        signal: &BuySignal,
        raised_at: Timestamp,
        calendars: &[MarketCalendar],
    ) -> Result<Self, crate::NoSignalReason> {
        let task = signal.facts().task();
        crate::SignalTiming::new(raised_at, calendars)
            .check(task, signal.facts().submission().submitted_at())?;
        crate::estimate_outcome::buy_signal_interval_from(task, raised_at, calendars)
            .or(Err(crate::NoSignalReason::Unavailable(
                crate::SignalUnavailable::Calendar,
            )))
            .map(|interval| Self {
                id,
                instrument_id: task.volatility().instrument().clone(),
                instrument: task.reference().instrument().clone(),
                originating_task: signal.facts().submission().task_key().clone(),
                interval,
                raised_at,
            })
    }
    #[must_use]
    pub const fn id(&self) -> &BuySignalOccurrenceId {
        &self.id
    }
    #[must_use]
    pub const fn instrument_id(&self) -> &crate::InstrumentId {
        &self.instrument_id
    }
    #[must_use]
    pub const fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn originating_task(&self) -> &crate::SignalTaskKey {
        &self.originating_task
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentBuySignal {
    signal: BuySignal,
    occurrence: BuySignalRecord,
}
impl CurrentBuySignal {
    pub fn try_from_history(
        signal: BuySignal,
        occurrence: crate::HistoricalSignalOccurrence,
    ) -> Result<Self, NoBuyQuantity> {
        Self::try_new(signal, occurrence.into_record())
    }
    pub fn try_new(signal: BuySignal, occurrence: BuySignalRecord) -> Result<Self, NoBuyQuantity> {
        if occurrence.instrument() != signal.facts().task().reference().instrument()
            || occurrence.instrument_id() != signal.facts().task().volatility().instrument()
        {
            return Err(NoBuyQuantity::Indeterminate);
        }
        Ok(Self { signal, occurrence })
    }
    #[must_use]
    pub const fn signal(&self) -> &BuySignal {
        &self.signal
    }
    #[must_use]
    pub const fn occurrence(&self) -> &BuySignalRecord {
        &self.occurrence
    }
    pub fn update(self, signal: BuySignal) -> Result<Self, NoBuyQuantity> {
        Self::try_new(signal, self.occurrence)
    }
}

pub(crate) fn check_buy_history(
    current: &CurrentBuySignal,
    now: Timestamp,
    records: &[BuySignalRecord],
) -> Result<(), NoBuyQuantity> {
    if records.len() > 65_536 {
        return Err(NoBuyQuantity::Indeterminate);
    }
    if current.occurrence.raised_at > now || records.iter().any(|record| record.raised_at > now) {
        return Err(NoBuyQuantity::Indeterminate);
    }
    if records
        .iter()
        .any(|record| record.id == current.occurrence.id && record != &current.occurrence)
    {
        return Err(NoBuyQuantity::Indeterminate);
    }
    let instrument = current.signal.facts().task().reference().instrument();
    let instrument_id = current.signal.facts().task().volatility().instrument();
    if records.iter().any(|record| {
        record.id != current.occurrence.id
            && (record.instrument_id() == instrument_id || record.instrument() == instrument)
            && now >= record.interval.start()
            && now < record.interval.end()
    }) {
        return Err(NoBuyQuantity::RepeatedSignal);
    }
    Ok(())
}
