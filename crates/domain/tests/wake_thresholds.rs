#[cfg(test)]
mod tests {
    use domain::{
        Currency, MovePercent, Nanosecond, NewsBurstCount, NumericInput, OffsetSeconds, Price,
        Timestamp, TriggerJudgement, UnixSeconds, WakeThresholds, daily_move_trigger_with,
        news_burst_trigger_with,
    };

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn thresholds(percent: u8, count: u8) -> WakeThresholds {
        WakeThresholds {
            daily_move: MovePercent::try_new(percent).expect("변동"),
            news_per_hour: NewsBurstCount::try_new(count).expect("건수"),
        }
    }

    #[test]
    fn custom_thresholds_change_the_move_and_news_boundaries() {
        let three = thresholds(3, 2);
        assert_eq!(
            daily_move_trigger_with(&price("10000"), &price("10300"), three),
            TriggerJudgement::Triggered
        );
        assert_eq!(
            daily_move_trigger_with(&price("10000"), &price("10299"), three),
            TriggerJudgement::Quiet
        );
        assert_eq!(
            news_burst_trigger_with(&[at(100), at(200)], at(300), three),
            TriggerJudgement::Triggered
        );
        assert_eq!(
            news_burst_trigger_with(&[at(200)], at(300), three),
            TriggerJudgement::Quiet
        );
        assert_eq!(WakeThresholds::default(), thresholds(5, 5));
    }
}
