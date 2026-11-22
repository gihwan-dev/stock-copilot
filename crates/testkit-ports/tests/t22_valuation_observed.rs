#[cfg(test)]
mod tests {
    use application::{EventBus, RecordOutcome, TakeValuationSnapshot, ValuationRepository};
    use domain::{
        AccountId, AcquisitionPrice, AdjustmentBasis, BuyingPower, Candle, CandleObservation,
        CandlePeriod, CandleStatus, CandleTime, CommonShare, Currency, DisplayInstrumentName,
        Holding, HoldingObservation, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Money, Nanosecond, NumericInput, NxtTrading,
        ObservedCloseEvidence, OffsetSeconds, Ohlcv, Portfolio, Price, PriceAdjustment, Quantity,
        SessionData, SessionInformation, SnapshotCalendar, SnapshotError, Symbol, SyncOutcome,
        SyncRecord, TimeInterval, Timestamp, TradingSuspension, UnixSeconds, ValuationSnapshot,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        memory::{
            MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
            MemoryValuationRepository,
        },
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    fn at(day: i64, hour: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                day.checked_mul(86400)
                    .expect("일")
                    .checked_add(hour.checked_mul(3600).expect("시"))
                    .expect("초")
                    .checked_sub(32400)
                    .expect("한국 시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(32400),
        )
        .expect("시각")
    }

    fn segment(day: i64, start: i64, end: i64) -> SessionData {
        SessionData::try_new(
            TimeInterval::try_new(at(day, start), at(day, end)).expect("구간"),
            None,
        )
        .expect("세션 구간")
    }

    fn session(day: i64, market: &Market) -> MarketSession {
        let information = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(segment(day, 9, 15)),
                after_market: Some(segment(day, 15, 20)),
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(segment(day, 9, 15)),
                after_market: None,
            },
        };
        MarketSession::try_new(at(day, 0).offset_calendar_date(), information).expect("장")
    }

    fn calendar() -> SnapshotCalendar {
        let market = |value| {
            MarketCalendar::try_new(session(2, &value), session(3, &value), session(4, &value))
                .expect("시장 달력")
        };
        SnapshotCalendar::new(&market(Market::Kr), &market(Market::Us)).expect("평가 달력")
    }

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }

    fn price(value: &str) -> Price {
        Price::try_new(money(value, Currency::Krw)).expect("가격")
    }

    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }

    fn instrument(now: Timestamp, symbol: &str, id: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new(id).expect("식별자"),
            name: DisplayInstrumentName::try_new("합성 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: now,
        })
        .expect("마스터")
    }

    fn inputs(now: Timestamp) -> (Portfolio, ObservedCloseEvidence) {
        let master = instrument(now, "005930", "synthetic-005930");
        let holding = Holding::new(
            master.order_instrument().expect("보유 종목"),
            quantity("3"),
            AcquisitionPrice::try_new(money("70000", Currency::Krw)).expect("취득가"),
            HoldingObservation::new(Some(price("72000")), None, now),
        )
        .expect("보유");
        let portfolio = Portfolio::new(
            vec![holding],
            BuyingPower::new(money("1000", Currency::Krw), now),
            BuyingPower::new(money("0", Currency::Usd), now),
        )
        .expect("계좌");
        let quote = master.quote_instrument().expect("시세 종목");
        let candle = Candle::try_new(
            quote.instrument_id().clone(),
            CandleTime::try_new(at(3, 0), CandlePeriod::Daily).expect("일봉 날짜"),
            PriceAdjustment::Raw,
            Ohlcv::try_new(
                price("71500"),
                price("73000"),
                price("71000"),
                price("72000"),
                quantity("380"),
            )
            .expect("일봉 값"),
            CandleObservation::new(now, CandleStatus::Finalized),
        )
        .expect("일봉");
        (portfolio, ObservedCloseEvidence::new(quote, candle))
    }

    #[test]
    fn t22_observed_snapshot_records_the_repository_close_evidence() {
        let now = at(3, 16);
        let (portfolio, evidence) = inputs(now);
        let portfolios = MemoryPortfolioRepository::default();
        portfolios
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                now,
                SyncOutcome::Complete(portfolio),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        market.supply_close_evidence(evidence.clone());
        let repository = MemoryValuationRepository::default();
        repository.script_append().push(MemoryReply::Memory);
        let account = AccountId::new(71);
        let report = run(TakeValuationSnapshot::new(
            &portfolios,
            &market,
            &repository,
            &SteppedClock::new(now),
            &EventBus::default(),
        )
        .execute(&account, &calendar()))
        .expect("종가 증거와 관측 저장");
        assert_eq!(report.recorded, RecordOutcome::Recorded);
        assert_eq!(report.snapshot.observed_close_evidence(), &[evidence]);
        assert_eq!(
            report.snapshot.stock_value(Currency::Krw),
            Ok(money("216000", Currency::Krw))
        );
        repository.script_history().push(MemoryReply::Memory);
        let window = TimeInterval::try_new(at(3, 0), at(4, 0)).expect("조회 구간");
        let history = run(repository.history(&account, window)).expect("저장 이력");
        assert_eq!(history, vec![report.snapshot]);
    }

    #[test]
    fn t22_same_market_and_currency_do_not_match_another_symbol() {
        let now = at(3, 16);
        let (portfolio, evidence) = inputs(now);
        let other = instrument(now, "000660", "synthetic-000660")
            .quote_instrument()
            .expect("다른 종목");
        let original = evidence.candle();
        let candle = Candle::try_new(
            other.instrument_id().clone(),
            original.time(),
            original.adjustment(),
            original.values(),
            CandleObservation::new(now, CandleStatus::Finalized),
        )
        .expect("다른 종목의 봉");
        let other_evidence = ObservedCloseEvidence::new(other, candle);
        assert_eq!(
            ValuationSnapshot::observe_with_evidence(
                &calendar(),
                &portfolio.evaluate(None),
                now,
                None,
                &[other_evidence],
            )
            .err(),
            Some(SnapshotError::MissingPrice)
        );
    }

    #[test]
    fn t22_close_evidence_requires_a_finalized_raw_daily_candle() {
        let now = at(3, 16);
        let (portfolio, evidence) = inputs(now);
        let original = evidence.candle();
        for (period, adjustment, status) in [
            (
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
            ),
            (
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(now)),
                CandleStatus::Finalized,
            ),
            (
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
                CandleStatus::InProgress,
            ),
        ] {
            let candle = Candle::try_new(
                original.instrument().clone(),
                CandleTime::try_new(at(3, 0), period).expect("봉 날짜"),
                adjustment,
                original.values(),
                CandleObservation::new(now, status),
            )
            .expect("봉");
            let invalid = ObservedCloseEvidence::new(evidence.instrument().clone(), candle);
            assert_eq!(
                ValuationSnapshot::observe_with_evidence(
                    &calendar(),
                    &portfolio.evaluate(None),
                    now,
                    None,
                    &[invalid],
                )
                .err(),
                Some(SnapshotError::InvalidCandle)
            );
        }
    }
}
