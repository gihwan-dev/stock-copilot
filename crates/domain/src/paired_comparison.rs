use crate::{CalendarDate, InstrumentId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairingRate(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRateError {
    Zero,
}

impl PairingRate {
    pub const DEFAULT: Self = Self(5);
    pub const fn try_new(one_in: u8) -> Result<Self, PairingRateError> {
        if one_in == 0 {
            Err(PairingRateError::Zero)
        } else {
            Ok(Self(one_in))
        }
    }
    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingDecision {
    Pair,
    Skip,
}

#[must_use]
pub fn paired_sample(
    instrument: &InstrumentId,
    day: CalendarDate,
    rate: PairingRate,
) -> PairingDecision {
    let mut state = 0xCBF2_9CE4_8422_2325_u64;
    let text = format!("{}|{day:?}", instrument.as_str());
    for byte in text.bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(0x0100_0000_01B3);
    }
    state ^= state >> 33_u32;
    match state.checked_rem(u64::from(rate.0)) {
        Some(0) => PairingDecision::Pair,
        Some(_) | None => PairingDecision::Skip,
    }
}

#[cfg(test)]
mod tests {
    use super::{PairingDecision, PairingRate, PairingRateError, paired_sample};
    use crate::{CalendarDate, CalendarYear, DayNumber, InstrumentId, MonthNumber};

    fn day(value: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(value),
        )
        .expect("날짜")
    }

    #[test]
    fn rate_one_pairs_everything_and_zero_is_rejected() {
        assert_eq!(PairingRate::try_new(0), Err(PairingRateError::Zero));
        let all = PairingRate::try_new(1).expect("비율");
        let id = InstrumentId::try_new("A005930").expect("종목");
        assert_eq!(paired_sample(&id, day(3), all), PairingDecision::Pair);
        assert_eq!(*PairingRate::DEFAULT.as_u8(), 5);
    }

    #[test]
    fn sampling_is_deterministic_and_roughly_follows_the_rate() {
        let rate = PairingRate::try_new(4).expect("비율");
        let mut paired = 0_u16;
        for index in 0_u16..400 {
            let id = InstrumentId::try_new(&format!("I{index}")).expect("종목");
            let first = paired_sample(&id, day(5), rate);
            assert_eq!(first, paired_sample(&id, day(5), rate));
            if first == PairingDecision::Pair {
                paired = paired.saturating_add(1);
            }
        }
        assert!((60_u16..140).contains(&paired), "{paired}");
    }
}
