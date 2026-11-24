use crate::{Nanosecond, SpanSeconds, TimeError, TimeSpan, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepResearchWindow {
    Open,
    Closed { opens_at: Timestamp },
}

pub fn deep_research_window(now: Timestamp) -> Result<DeepResearchWindow, TimeError> {
    let local = now
        .utc_seconds()
        .as_i64()
        .checked_add(32_400_i64)
        .ok_or(TimeError::Overflow)?
        .rem_euclid(86_400_i64);
    if !(21_600_i64..79_200_i64).contains(&local) {
        return Ok(DeepResearchWindow::Open);
    }
    let wait =
        u64::try_from(79_200_i64.saturating_sub(local)).map_err(|_negative| TimeError::Overflow)?;
    let span = TimeSpan::try_new(SpanSeconds::new(wait), Nanosecond::new(0))?;
    let start = Timestamp::from_parts(now.utc_seconds(), Nanosecond::new(0), now.offset_seconds())?;
    Ok(DeepResearchWindow::Closed {
        opens_at: start.checked_add(span)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{DeepResearchWindow, deep_research_window};
    use crate::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

    fn kst(hour: i64, minute: i64) -> Timestamp {
        let seconds = hour
            .checked_mul(3_600)
            .and_then(|value| value.checked_add(minute.checked_mul(60)?))
            .and_then(|value| value.checked_sub(32_400))
            .and_then(|value| value.checked_add(1_800_000_000 - 1_800_000_000 % 86_400))
            .expect("시각");
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    #[test]
    fn night_from_22_to_06_kst_is_open_and_daytime_waits_for_22() {
        for (hour, minute) in [(22, 0), (23, 59), (0, 0), (3, 30), (5, 59)] {
            assert_eq!(
                deep_research_window(kst(hour, minute)),
                Ok(DeepResearchWindow::Open),
                "{hour}:{minute}"
            );
        }
        for (hour, minute) in [(6, 0), (12, 0), (21, 59)] {
            assert_eq!(
                deep_research_window(kst(hour, minute)),
                Ok(DeepResearchWindow::Closed {
                    opens_at: kst(22, 0)
                }),
                "{hour}:{minute}"
            );
        }
    }
}
