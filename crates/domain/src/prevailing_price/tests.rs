use crate::{
    CheckResult, CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput, NxtTrading, OffsetSeconds,
    Price, SessionData, SessionInformation, Symbol, TimeInterval, Timestamp, TradingSuspension,
    UnixSeconds,
};

const DAY: i64 = 1_794_960_000;
const KST: i32 = 32_400;

fn kst(day: i64, hour: i64, minute: i64, second: i64) -> Timestamp {
    let seconds = DAY
        .checked_add(day.checked_mul(86_400).expect("일"))
        .and_then(|value| value.checked_add(hour.checked_mul(3_600).expect("시")))
        .and_then(|value| value.checked_add(minute.checked_mul(60).expect("분")))
        .and_then(|value| value.checked_add(second))
        .and_then(|value| value.checked_sub(i64::from(KST)))
        .expect("시각");
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(KST),
    )
    .expect("시각")
}

fn utc(time: Timestamp) -> Timestamp {
    Timestamp::from_parts(time.utc_seconds(), time.nanosecond(), OffsetSeconds::new(0))
        .expect("UTC")
}

fn row(day: i64) -> MarketSession {
    let regular = SessionData::try_new(
        TimeInterval::try_new(kst(day, 9, 0, 0), kst(day, 15, 30, 0)).expect("구간"),
        None,
    )
    .expect("세션");
    MarketSession::try_new(
        kst(day, 9, 0, 0).offset_calendar_date(),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(regular),
            after_market: None,
        },
    )
    .expect("행")
}

fn calendar(day: i64) -> MarketCalendar {
    MarketCalendar::try_new(
        row(day.checked_sub(1).expect("전")),
        row(day),
        row(day.checked_add(1).expect("익일")),
    )
    .expect("달력")
}

fn us_calendar(day: i64) -> MarketCalendar {
    let us_row = |index: i64| {
        let regular = SessionData::try_new(
            TimeInterval::try_new(kst(index, 9, 0, 0), kst(index, 15, 30, 0)).expect("구간"),
            None,
        )
        .expect("세션");
        MarketSession::try_new(
            kst(index, 9, 0, 0).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(regular),
                after_market: None,
            },
        )
        .expect("행")
    };
    MarketCalendar::try_new(
        us_row(day.checked_sub(1).expect("전일")),
        us_row(day),
        us_row(day.checked_add(1).expect("익일")),
    )
    .expect("달력")
}

fn master(currency: Currency) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        instrument_id: InstrumentId::try_new("prevailing-a").expect("ID"),
        symbol: Symbol::try_new("005930").expect("종목"),
        name: DisplayInstrumentName::try_new("고정 종목").expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
        currency,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: kst(0, 8, 0, 0),
    })
    .expect("마스터")
}

fn price(text: &str) -> Price {
    Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
}

fn candle(
    source: &MasterInstrument,
    day: i64,
    at: Timestamp,
    status: crate::CandleStatus,
) -> crate::Candle {
    let price = Price::parse(
        &NumericInput::try_new("71200").expect("입력"),
        source.row().currency,
    )
    .expect("종가");
    let quantity =
        crate::Quantity::parse(&NumericInput::try_new("5").expect("입력")).expect("수량");
    crate::Candle::try_new(
        source.row().instrument_id.clone(),
        crate::CandleTime::try_new(kst(day, 0, 0, 0), crate::CandlePeriod::Daily).expect("시각"),
        crate::PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(at)),
        crate::Ohlcv::try_new(price, price, price, price, quantity).expect("가격"),
        crate::CandleObservation::new(at, status),
    )
    .expect("일봉")
}

#[test]
fn overnight_reference_is_the_finalized_close_from_the_latest_regular_session() {
    let at = utc(kst(2, 1, 0, 0));
    let source = master(Currency::Krw);
    let final_close = candle(&source, 1, at, crate::CandleStatus::Finalized);
    let observation = crate::closed_estimate_reference(
        &source,
        at,
        &calendar(1),
        std::slice::from_ref(&final_close),
    )
    .expect("종가");
    assert_eq!(observation.close_at(), Some(kst(1, 15, 30, 0)));
    let mark = observation.reference();
    assert_eq!(mark.price(), price("71200"));
    assert_eq!(mark.observed_at(), at);
    assert_eq!(mark.observed_at().offset_seconds(), OffsetSeconds::new(KST));
    assert_eq!(mark.check(mark.instrument(), at), CheckResult::Passed);
    for missing in [
        vec![],
        vec![candle(&source, 0, at, crate::CandleStatus::Finalized)],
        vec![candle(&source, 1, at, crate::CandleStatus::InProgress)],
        vec![final_close.clone(), final_close.clone()],
    ] {
        assert!(crate::closed_estimate_reference(&source, at, &calendar(1), &missing).is_none());
    }
    for (time, calendar) in [
        (kst(1, 10, 0, 0), calendar(1)),
        (at, us_calendar(1)),
        (kst(5, 1, 0, 0), calendar(1)),
    ] {
        assert!(
            crate::closed_estimate_reference(
                &source,
                time,
                &calendar,
                std::slice::from_ref(&final_close)
            )
            .is_none()
        );
    }
}

#[test]
fn us_friday_close_on_korean_saturday_uses_fridays_bar_and_monday_intraday_is_monday() {
    let us_row = |index: i64| {
        let segment = SessionData::try_new(
            TimeInterval::try_new(
                kst(index, 23, 30, 0),
                kst(index.checked_add(1).expect("익일"), 6, 0, 0),
            )
            .expect("구간"),
            None,
        )
        .expect("세션");
        MarketSession::try_new(
            kst(index, 12, 0, 0).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(segment),
                after_market: None,
            },
        )
        .expect("달력 행")
    };
    let mut row = master(Currency::Krw).row().clone();
    row.listing_market = MasterCode::try_new("NASDAQ").expect("시장");
    row.symbol = Symbol::try_new("AAPL").expect("심볼");
    row.currency = Currency::Usd;
    row.korean_detail = None;
    let source = MasterInstrument::try_new(row).expect("미국 마스터");
    let calendar = MarketCalendar::try_new(us_row(1), us_row(2), us_row(5)).expect("목 금 월");
    let weekend = kst(3, 10, 0, 0);
    let friday = candle(&source, 2, weekend, crate::CandleStatus::Finalized);
    let saturday = candle(&source, 3, weekend, crate::CandleStatus::Finalized);
    assert!(crate::closed_estimate_reference(&source, weekend, &calendar, &[saturday]).is_none());
    let saved = crate::closed_estimate_reference(&source, weekend, &calendar, &[friday])
        .expect("금요일 종가");
    assert_eq!(saved.close_at(), Some(kst(3, 6, 0, 0)));
    assert_eq!(
        saved.close_at().expect("종가").offset_calendar_date(),
        calendar.today().date()
    );
    let tuesday_korea = kst(6, 1, 0, 0);
    let monday_us = tuesday_korea
        .in_exchange_zone(crate::KnownMarket::Us)
        .expect("거래소 시각");
    assert_eq!(
        monday_us.offset_calendar_date(),
        calendar.next_business_day().date()
    );
    assert_eq!(
        calendar.state_at(monday_us),
        crate::MarketState::RegularMarket
    );
}

#[test]
fn eastern_clock_switches_at_the_two_utc_transition_instants() {
    for (second, expected) in [
        (1_772_953_199_i64, -18_000_i32),
        (1_772_953_200, -14_400_i32),
        (1_793_512_799, -14_400_i32),
        (1_793_512_800, -18_000_i32),
    ] {
        let instant = Timestamp::from_parts(
            UnixSeconds::new(second),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("서울");
        let exchange = instant
            .in_exchange_zone(crate::KnownMarket::Us)
            .expect("뉴욕");
        assert_eq!(instant, exchange);
        assert_eq!(exchange.offset_seconds(), OffsetSeconds::new(expected));
    }
}
