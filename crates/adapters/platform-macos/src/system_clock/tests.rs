use super::SystemClock;
use application::Clock;
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use std::time::{Duration, UNIX_EPOCH};

fn at(seconds: i64, nanos: u32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(0),
    )
    .expect("시점")
}

#[test]
fn conversion_preserves_fractional_instants_on_both_sides_of_epoch() {
    let clock = SystemClock::new().expect("시계");
    for (duration, positive, negative) in [
        (Duration::new(0, 0), at(0, 0), at(0, 0)),
        (Duration::new(0, 1), at(0, 1), at(-1, 999_999_999)),
        (Duration::new(3, 123), at(3, 123), at(-4, 999_999_877)),
        (Duration::new(7, 0), at(7, 0), at(-7, 0)),
    ] {
        assert_eq!(clock.convert(UNIX_EPOCH + duration), positive);
        assert_eq!(clock.convert(UNIX_EPOCH - duration), negative);
    }
}

#[test]
fn conversion_clamps_at_exact_domain_boundaries() {
    let clock = SystemClock::new().expect("시계");
    for (seconds, nanos, expected) in [
        (-62_135_596_801_i128, 999_999_999, at(-62_135_596_800, 0)),
        (-62_135_596_800, 0, at(-62_135_596_800, 0)),
        (-62_135_596_800, 1, at(-62_135_596_800, 1)),
        (
            253_402_300_799,
            999_999_998,
            at(253_402_300_799, 999_999_998),
        ),
        (
            253_402_300_799,
            999_999_999,
            at(253_402_300_799, 999_999_999),
        ),
        (253_402_300_800, 0, at(253_402_300_799, 999_999_999)),
        (i128::MIN, 0, at(-62_135_596_800, 0)),
        (i128::MAX, 0, at(253_402_300_799, 999_999_999)),
        (0, 1_000_000_000, at(253_402_300_799, 999_999_999)),
    ] {
        assert_eq!(clock.parts(seconds, nanos), expected);
    }
}

#[test]
fn conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee() {
    let clock = SystemClock::new().expect("시계");
    let later = clock.convert(UNIX_EPOCH + Duration::new(13, 91));
    let earlier = clock.convert(UNIX_EPOCH + Duration::new(11, 37));
    assert_eq!(later, at(13, 91));
    assert_eq!(earlier, at(11, 37));
    assert!(earlier < later);
    let _instant = clock.now();
}
