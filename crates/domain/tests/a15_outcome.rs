#[cfg(test)]
mod tests {
    fn at(day: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_767_225_600_i64
                    .checked_add(day.checked_mul(86_400).expect("일"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn price(text: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("길이"), currency).expect("가격")
    }
    fn series(values: &[&str]) -> Vec<Candle> {
        values
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let close = price(text, Currency::Usd);
                Candle::try_new(
                    InstrumentId::try_new("sample").expect("식별자"),
                    CandleTime::try_new(
                        at(i64::try_from(index).expect("크기")),
                        CandlePeriod::Daily,
                    )
                    .expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(10_001))),
                    Ohlcv::try_new(
                        close,
                        close,
                        close,
                        close,
                        Quantity::parse(&NumericInput::try_new("0").expect("길이")).expect("수량"),
                    )
                    .expect("OHLCV"),
                    CandleObservation::new(at(10_001), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    use domain::{
        AdjustmentBasis, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        Currency, EstimateContext, EstimateOutcome, EstimateTask, Horizon, HoursPermission,
        InstrumentId, MarketCalendar, Nanosecond, NumericInput, OffsetSeconds, Ohlcv,
        OutcomeAdjustment, OutcomeFailure, OutcomeState, OutcomeUnavailable, Price,
        PriceAdjustment, Quantity, RiskAppetite, SessionData, SessionInformation, Symbol,
        TimeInterval, Timestamp, UnixSeconds, Volatility,
    };
    fn instant(day: i64, minute: i64) -> Timestamp {
        let seconds = at(day)
            .utc_seconds()
            .as_i64()
            .checked_add(minute.checked_mul(60).expect("분"))
            .expect("초");
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(-18000),
        )
        .expect("시각")
    }
    fn task() -> EstimateTask {
        let data = (0_u32..61)
            .map(|index| {
                if index.wrapping_rem(2) == 0 {
                    "100"
                } else {
                    "102"
                }
            })
            .collect::<Vec<_>>();
        let volatility = Volatility::calculate(&series(&data)).expect("변동성");
        let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: Symbol::try_new("ABC").expect("심볼"),
            instrument_id: InstrumentId::try_new("sample").expect("종목"),
            name: domain::DisplayInstrumentName::try_new("테스트 종목").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: instant(61, 900),
        })
        .expect("마스터");
        let reference = master
            .price_observation(price("100", Currency::Usd), instant(61, 900))
            .expect("관측");
        EstimateTask::create(
            EstimateContext::new(reference, instant(61, 900)),
            volatility,
            Horizon::new(domain::HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }
    fn row(day: i64) -> domain::MarketSession {
        domain::MarketSession::try_new(
            instant(day, 900).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(instant(day, 900), instant(day, 904)).expect("구간"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("행")
    }
    fn calendars() -> Vec<MarketCalendar> {
        (61_i64..=66)
            .map(|day| {
                MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("전일")),
                    row(day),
                    row(day.checked_add(1).expect("익일")),
                )
                .expect("달력")
            })
            .collect()
    }
    fn adjustment() -> OutcomeAdjustment {
        OutcomeAdjustment::from_actions(&task(), AdjustmentBasis::new(instant(66, 904)), &[])
            .expect("수정")
    }
    fn candle(
        day: i64,
        minute: i64,
        period: CandlePeriod,
        prices: (&str, &str),
        factor: &OutcomeAdjustment,
    ) -> Candle {
        let (high, low) = prices;
        let time = if period == CandlePeriod::Daily {
            instant(day, 300)
        } else {
            instant(day, minute)
        };
        Candle::try_new(
            InstrumentId::try_new("sample").expect("종목"),
            CandleTime::try_new(time, period).expect("봉 시각"),
            PriceAdjustment::Adjusted(factor.basis()),
            Ohlcv::try_new(
                price(
                    if high == "200" && low == "1" {
                        "100"
                    } else {
                        low
                    },
                    Currency::Usd,
                ),
                price(high, Currency::Usd),
                price(low, Currency::Usd),
                price(low, Currency::Usd),
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(instant(70, 904), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    fn flat(factor: &OutcomeAdjustment, value: &str) -> Vec<Candle> {
        let mut result = (901..=904)
            .map(|minute| candle(61, minute, CandlePeriod::Minute, (value, value), factor))
            .collect::<Vec<_>>();
        result.extend(
            (62..=66).map(|day| candle(day, 0, CandlePeriod::Daily, (value, value), factor)),
        );
        result
    }

    #[test]
    fn missing_minute_is_unavailable_for_resolution_before_or_after_close() {
        let factor = adjustment();
        assert_eq!(
            EstimateOutcome::evaluate_with_complete_data(
                &task(),
                &[],
                &factor,
                &[],
                instant(70, 904)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        let mut bars = flat(&factor, "100");
        bars.retain(|bar| bar.time().timestamp() != instant(61, 902));
        assert_eq!(
            EstimateOutcome::evaluate_with_complete_data(
                &task(),
                &calendars(),
                &factor,
                &bars,
                instant(61, 903)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::MissingCandles)
        );
        assert_eq!(
            EstimateOutcome::evaluate_with_complete_data(
                &task(),
                &calendars(),
                &factor,
                &bars,
                instant(70, 904)
            )
            .state(),
            OutcomeState::Unavailable(OutcomeUnavailable::MissingCandles)
        );
        assert_eq!(
            EstimateOutcome::evaluate(&task(), &calendars(), &factor, &bars, instant(70, 904))
                .state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
    }
    #[test]
    fn strict_outcome_keeps_complete_simultaneous_and_calibration_facts() {
        let factor = adjustment();
        let both = [candle(61, 901, CandlePeriod::Minute, ("200", "1"), &factor)];
        let outcome = EstimateOutcome::evaluate_with_complete_data(
            &task(),
            &calendars(),
            &factor,
            &both,
            instant(70, 904),
        );
        assert_eq!(
            outcome.state(),
            OutcomeState::Failure(OutcomeFailure::Ambiguous)
        );
        let identity = |value: &str| domain::ModelIdentityPart::try_new(value).expect("모델");
        let model =
            domain::CalibrationModel::new(identity("provider"), identity("model"), identity("v1"));
        let probability =
            domain::AiProbability::parse(&domain::AiProbabilityInput::try_new("71").expect("입력"))
                .expect("확률");
        let first = outcome
            .calibration_fact(
                domain::CalibrationEstimateId::try_new("first").expect("식별"),
                model.clone(),
                probability,
            )
            .expect("사실");
        let next = outcome
            .calibration_fact(
                domain::CalibrationEstimateId::try_new("next").expect("식별"),
                model,
                probability,
            )
            .expect("사실");
        let key = first.estimate().key().clone();
        let initial = domain::Calibration::try_new(key.clone(), &[first]).expect("보정표");
        let updated = initial.with_fact(next).expect("갱신");
        assert_ne!(initial, updated);
        assert_eq!(updated.for_combination(&key).judgment_count().as_usize(), 1);
    }
}
