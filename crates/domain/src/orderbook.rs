#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookInput {
    pub instrument: QuoteInstrument,
    pub currency: Currency,
    pub asks: Vec<OrderbookLevel>,
    pub bids: Vec<OrderbookLevel>,
    pub timestamp: Option<Timestamp>,
    pub collected_at: Timestamp,
    pub tick_size: TickSizeAvailability,
}
use crate::{
    ClockAlignment, Currency, Price, Quantity, QuoteInstrument, TickSizeAvailability, Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderbookLevel {
    price: Price,
    volume: Quantity,
}
impl OrderbookLevel {
    #[must_use]
    pub const fn new(price: Price, volume: Quantity) -> Self {
        Self { price, volume }
    }
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub const fn volume(&self) -> Quantity {
        self.volume
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestQuote<'a> {
    NoQuote,
    Available(&'a OrderbookLevel),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookError {
    CurrencyMismatch,
    AskOrder,
    BidOrder,
    CrossedOrLocked,
    FutureTimestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orderbook {
    instrument: QuoteInstrument,
    asks: Vec<OrderbookLevel>,
    bids: Vec<OrderbookLevel>,
    timestamp: Option<Timestamp>,
    collected_at: Timestamp,
    clock_alignment: ClockAlignment,
    tick_size: TickSizeAvailability,
}
impl Orderbook {
    pub fn try_new(input: OrderbookInput) -> Result<Self, OrderbookError> {
        let OrderbookInput {
            instrument,
            currency,
            asks,
            bids,
            timestamp,
            collected_at,
            tick_size,
        } = input;
        if instrument.currency() != currency
            || asks
                .iter()
                .chain(&bids)
                .any(|level| level.price.as_money().currency() != currency)
        {
            return Err(OrderbookError::CurrencyMismatch);
        }
        if let TickSizeAvailability::Reported(tick) = tick_size
            && tick.price().as_money().currency() != currency
        {
            return Err(OrderbookError::CurrencyMismatch);
        }
        validate_level_order(&asks, std::cmp::Ordering::Less, OrderbookError::AskOrder)?;
        validate_level_order(&bids, std::cmp::Ordering::Greater, OrderbookError::BidOrder)?;
        if asks.first().zip(bids.first()).is_some_and(|(ask, bid)| {
            bid.price.as_money().amount().as_decimal() >= ask.price.as_money().amount().as_decimal()
        }) {
            return Err(OrderbookError::CrossedOrLocked);
        }
        let clock_alignment = timestamp
            .map_or(Ok(ClockAlignment::Aligned), |time| {
                crate::quote::assess_time(
                    time,
                    collected_at,
                    crate::quote::TimestampAllowance::Observation,
                )
            })
            .map_err(|crate::quote::TimeAssessmentError::TooFarAhead| {
                OrderbookError::FutureTimestamp
            })?;
        Ok(Self {
            instrument,
            asks,
            bids,
            timestamp,
            collected_at,
            clock_alignment,
            tick_size,
        })
    }
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }
    #[must_use]
    pub fn asks(&self) -> &[OrderbookLevel] {
        &self.asks
    }
    #[must_use]
    pub fn bids(&self) -> &[OrderbookLevel] {
        &self.bids
    }
    #[must_use]
    pub fn best_ask(&self) -> BestQuote<'_> {
        self.asks
            .first()
            .map_or(BestQuote::NoQuote, BestQuote::Available)
    }
    #[must_use]
    pub fn best_bid(&self) -> BestQuote<'_> {
        self.bids
            .first()
            .map_or(BestQuote::NoQuote, BestQuote::Available)
    }
    #[must_use]
    pub const fn timestamp(&self) -> Option<Timestamp> {
        self.timestamp
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
    #[must_use]
    pub const fn clock_alignment(&self) -> ClockAlignment {
        self.clock_alignment
    }
    #[must_use]
    pub const fn tick_size(&self) -> TickSizeAvailability {
        self.tick_size
    }
}

fn validate_level_order(
    levels: &[OrderbookLevel],
    order: std::cmp::Ordering,
    error: OrderbookError,
) -> Result<(), OrderbookError> {
    let mut prior: Option<crate::Amount> = None;
    for level in levels {
        let amount = level.price.as_money().amount();
        if prior.is_some_and(|previous| previous.as_decimal().cmp(&amount.as_decimal()) != order) {
            return Err(error);
        }
        prior = Some(amount);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        BestQuote, ClockAlignment, CommonShare, Currency, DisplayInstrumentName, InstrumentId,
        KnownMarket, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, NumericInput, OffsetSeconds, Orderbook, OrderbookError,
        OrderbookInput, OrderbookLevel, Price, Quantity, QuoteInstrument, Symbol, TickSize,
        TickSizeAvailability, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};

    pub(crate) fn price(value: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("가격")
    }
    pub(crate) fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn instrument(market: KnownMarket) -> QuoteInstrument {
        let (code, symbol, currency) = match market {
            KnownMarket::Kr => ("KOSPI", "005930", Currency::Krw),
            KnownMarket::Us => ("NASDAQ", "AAPL", Currency::Usd),
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(code).expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new("stable").expect("키"),
            name: DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(0),
        })
        .expect("마스터")
        .quote_instrument()
        .expect("시세 식별")
    }
    fn level(value: &str) -> OrderbookLevel {
        OrderbookLevel::new(
            price(value, Currency::Krw),
            Quantity::parse(&NumericInput::try_new("0.125").expect("입력")).expect("수량"),
        )
    }
    fn book(
        asks: Vec<OrderbookLevel>,
        bids: Vec<OrderbookLevel>,
    ) -> Result<Orderbook, OrderbookError> {
        Orderbook::try_new(OrderbookInput {
            instrument: instrument(KnownMarket::Kr),
            currency: Currency::Krw,
            asks,
            bids,
            timestamp: Some(at(0)),
            collected_at: at(1),
            tick_size: TickSizeAvailability::Unavailable,
        })
    }
    #[test]
    fn sealed_levels_and_books_preserve_facts_and_empty_sides() {
        let ask = level("101");
        let bid = level("99");
        assert_eq!(ask.price(), price("101", Currency::Krw));
        assert_eq!(
            ask.volume().as_decimal(),
            rust_decimal::Decimal::new(125, 3)
        );
        for asks in [vec![], vec![ask, level("102")]] {
            for bids in [vec![], vec![bid, level("98")]] {
                let value = book(asks.clone(), bids.clone()).expect("호가");
                assert_eq!(value.instrument(), &instrument(KnownMarket::Kr));
                assert_eq!(value.asks(), asks);
                assert_eq!(value.bids(), bids);
                assert_eq!(
                    value.best_ask(),
                    asks.first()
                        .map_or(BestQuote::NoQuote, BestQuote::Available)
                );
                assert_eq!(
                    value.best_bid(),
                    bids.first()
                        .map_or(BestQuote::NoQuote, BestQuote::Available)
                );
                assert_eq!(value.timestamp(), Some(at(0)));
                assert_eq!(value.collected_at(), at(1));
                assert_eq!(value.clock_alignment(), ClockAlignment::Aligned);
                assert_eq!(value.tick_size(), TickSizeAvailability::Unavailable);
            }
        }
    }
    #[test]
    fn unsorted_duplicate_crossed_and_currency_mismatches_are_rejected() {
        for asks in [
            vec![level("102"), level("101")],
            vec![level("101"), level("101")],
        ] {
            assert_eq!(book(asks, vec![]), Err(OrderbookError::AskOrder));
        }
        for bids in [
            vec![level("98"), level("99")],
            vec![level("99"), level("99")],
        ] {
            assert_eq!(book(vec![], bids), Err(OrderbookError::BidOrder));
        }
        for bid in ["101", "102"] {
            assert_eq!(
                book(vec![level("101")], vec![level(bid)]),
                Err(OrderbookError::CrossedOrLocked)
            );
        }
        let usd = OrderbookLevel::new(price("1", Currency::Usd), level("1").volume());
        assert_eq!(
            book(vec![usd], vec![]),
            Err(OrderbookError::CurrencyMismatch)
        );
        assert_eq!(
            book(vec![], vec![usd]),
            Err(OrderbookError::CurrencyMismatch)
        );
        assert_eq!(
            Orderbook::try_new(OrderbookInput {
                instrument: instrument(KnownMarket::Kr),
                currency: Currency::Usd,
                asks: vec![],
                bids: vec![],
                timestamp: None,
                collected_at: at(0),
                tick_size: TickSizeAvailability::Unavailable
            }),
            Err(OrderbookError::CurrencyMismatch)
        );
    }
    #[test]
    fn nullable_server_time_skew_and_response_ticks_remain_distinct() {
        let tick = TickSize::from_response(price("0.01", Currency::Usd));
        for timestamp in [None, Some(at(0)), Some(at(2))] {
            let value = Orderbook::try_new(OrderbookInput {
                instrument: instrument(KnownMarket::Us),
                currency: Currency::Usd,
                asks: vec![],
                bids: vec![],
                timestamp,
                collected_at: at(0),
                tick_size: TickSizeAvailability::Reported(tick),
            })
            .expect("호가");
            assert_eq!(value.timestamp(), timestamp);
            assert_eq!(value.tick_size(), TickSizeAvailability::Reported(tick));
            assert_eq!(
                value.clock_alignment(),
                if timestamp == Some(at(2)) {
                    ClockAlignment::AheadWithinTolerance
                } else {
                    ClockAlignment::Aligned
                }
            );
        }
        assert_eq!(
            Orderbook::try_new(OrderbookInput {
                instrument: instrument(KnownMarket::Kr),
                currency: Currency::Krw,
                asks: vec![],
                bids: vec![],
                timestamp: None,
                collected_at: at(0),
                tick_size: TickSizeAvailability::Reported(tick)
            }),
            Err(OrderbookError::CurrencyMismatch)
        );
        assert_eq!(
            Orderbook::try_new(OrderbookInput {
                instrument: instrument(KnownMarket::Kr),
                currency: Currency::Krw,
                asks: vec![],
                bids: vec![],
                timestamp: Some(at(3)),
                collected_at: at(0),
                tick_size: TickSizeAvailability::Unavailable
            }),
            Err(OrderbookError::FutureTimestamp)
        );
    }
    #[test]
    fn accepted_books_have_strict_spreads_for_4096_generated_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u32..100_000, 1_u32..100_000), |(ask, bid)| {
                let result = book(vec![level(&ask.to_string())], vec![level(&bid.to_string())]);
                proptest::prop_assert_eq!(result.is_ok(), bid < ask);
                if let Ok(value) = result {
                    proptest::prop_assert_eq!(
                        value.best_ask(),
                        BestQuote::Available(&level(&ask.to_string()))
                    );
                    proptest::prop_assert_eq!(
                        value.best_bid(),
                        BestQuote::Available(&level(&bid.to_string()))
                    );
                }
                Ok(())
            })
            .expect("스프레드 속성");
    }
}
