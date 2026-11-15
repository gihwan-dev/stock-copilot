use crate::{
    ClockAlignment, Currency, Price, Quantity, QuoteInstrument, TickAlignment, TickSizeError,
    TickTable, Timestamp,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderbookLevelInput {
    pub price: Price,
    pub volume: Quantity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookInput {
    pub instrument: QuoteInstrument,
    pub currency: Currency,
    pub asks: Vec<OrderbookLevelInput>,
    pub bids: Vec<OrderbookLevelInput>,
    pub timestamp: Option<Timestamp>,
    pub collected_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookLevel {
    instrument: QuoteInstrument,
    price: Price,
    volume: Quantity,
}
impl OrderbookLevel {
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub const fn volume(&self) -> Quantity {
        self.volume
    }
    pub fn tick_alignment(&self, table: &TickTable) -> Result<TickAlignment, TickSizeError> {
        crate::tick_alignment(table, &self.instrument, self.price)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestQuote<'a> {
    NoQuote,
    Available(&'a OrderbookLevel),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookInputOrder {
    Empty,
    Single,
    Ascending,
    Descending,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookTimeAlignment {
    Unknown,
    Known(ClockAlignment),
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
    ask_input_order: OrderbookInputOrder,
    bid_input_order: OrderbookInputOrder,
    timestamp: Option<Timestamp>,
    collected_at: Timestamp,
    clock_alignment: OrderbookTimeAlignment,
}
impl Orderbook {
    pub fn try_new(input: OrderbookInput) -> Result<Self, OrderbookError> {
        let OrderbookInput {
            instrument,
            currency,
            mut asks,
            mut bids,
            timestamp,
            collected_at,
        } = input;
        if instrument.currency() != currency
            || asks
                .iter()
                .chain(&bids)
                .any(|level| level.price.as_money().currency() != currency)
        {
            return Err(OrderbookError::CurrencyMismatch);
        }
        let ask_input_order =
            normalize_levels(&mut asks, Ordering::Less, OrderbookError::AskOrder)?;
        let bid_input_order =
            normalize_levels(&mut bids, Ordering::Greater, OrderbookError::BidOrder)?;
        if asks
            .first()
            .zip(bids.first())
            .is_some_and(|(ask, bid)| compare_prices(bid.price, ask.price) != Ordering::Less)
        {
            return Err(OrderbookError::CrossedOrLocked);
        }
        let clock_alignment = orderbook_time_alignment(timestamp, collected_at)?;
        Ok(Self {
            asks: seal_levels(&instrument, asks),
            bids: seal_levels(&instrument, bids),
            instrument,
            ask_input_order,
            bid_input_order,
            timestamp,
            collected_at,
            clock_alignment,
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
    pub const fn ask_input_order(&self) -> OrderbookInputOrder {
        self.ask_input_order
    }
    #[must_use]
    pub const fn bid_input_order(&self) -> OrderbookInputOrder {
        self.bid_input_order
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
    pub const fn clock_alignment(&self) -> OrderbookTimeAlignment {
        self.clock_alignment
    }
}
fn compare_prices(left: Price, right: Price) -> Ordering {
    left.as_money()
        .amount()
        .as_decimal()
        .cmp(&right.as_money().amount().as_decimal())
}
fn normalize_levels(
    levels: &mut [OrderbookLevelInput],
    desired: Ordering,
    error: OrderbookError,
) -> Result<OrderbookInputOrder, OrderbookError> {
    let Some((first, remaining)) = levels.split_first() else {
        return Ok(OrderbookInputOrder::Empty);
    };
    let Some(second) = remaining.first() else {
        return Ok(OrderbookInputOrder::Single);
    };
    let direction = compare_prices(first.price, second.price);
    let original = match direction {
        Ordering::Less => OrderbookInputOrder::Ascending,
        Ordering::Greater => OrderbookInputOrder::Descending,
        Ordering::Equal => return Err(error),
    };
    if levels
        .iter()
        .zip(levels.iter().skip(1))
        .any(|(left, right)| compare_prices(left.price, right.price) != direction)
    {
        return Err(error);
    }
    if direction != desired {
        levels.reverse();
    }
    Ok(original)
}
fn seal_levels(
    instrument: &QuoteInstrument,
    levels: Vec<OrderbookLevelInput>,
) -> Vec<OrderbookLevel> {
    levels
        .into_iter()
        .map(|level| OrderbookLevel {
            instrument: instrument.clone(),
            price: level.price,
            volume: level.volume,
        })
        .collect()
}
fn orderbook_time_alignment(
    timestamp: Option<Timestamp>,
    collected_at: Timestamp,
) -> Result<OrderbookTimeAlignment, OrderbookError> {
    match timestamp {
        None => Ok(OrderbookTimeAlignment::Unknown),
        Some(value) => crate::quote::assess_time(
            value,
            collected_at,
            crate::quote::TimestampAllowance::Observation,
        )
        .map(OrderbookTimeAlignment::Known)
        .map_err(|crate::quote::TimeAssessmentError::TooFarAhead| OrderbookError::FutureTimestamp),
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        BestQuote, ClockAlignment, CommonShare, Currency, DisplayInstrumentName, InstrumentId,
        KnownMarket, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, NumericInput, OffsetSeconds, Orderbook, OrderbookError,
        OrderbookInput, OrderbookInputOrder, OrderbookLevelInput, OrderbookTimeAlignment, Price,
        Quantity, QuoteInstrument, Symbol, TickAlignment, TickSizeError, TickTable, Timestamp,
        UnixSeconds,
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
    pub(crate) fn master(market: KnownMarket, security: &str, identity: &str) -> MasterInstrument {
        let (code, symbol, currency) = match market {
            KnownMarket::Kr => ("KOSPI", "005930", Currency::Krw),
            KnownMarket::Us => ("NASDAQ", "AAPL", Currency::Usd),
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(code).expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new(identity).expect("키"),
            name: DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new(security).expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(0),
        })
        .expect("마스터")
    }
    pub(crate) fn instrument(market: KnownMarket) -> QuoteInstrument {
        master(market, "STOCK", "stable")
            .quote_instrument()
            .expect("시세 식별")
    }
    fn level(value: &str) -> OrderbookLevelInput {
        OrderbookLevelInput {
            price: price(value, Currency::Krw),
            volume: Quantity::parse(&NumericInput::try_new("0.125").expect("입력")).expect("수량"),
        }
    }
    fn book(
        asks: Vec<OrderbookLevelInput>,
        bids: Vec<OrderbookLevelInput>,
    ) -> Result<Orderbook, OrderbookError> {
        Orderbook::try_new(OrderbookInput {
            instrument: instrument(KnownMarket::Kr),
            currency: Currency::Krw,
            asks,
            bids,
            timestamp: Some(at(0)),
            collected_at: at(1),
        })
    }
    #[test]
    fn both_strict_directions_are_normalized_and_original_order_is_preserved() {
        for (asks, ask_order) in [
            (
                vec![level("101"), level("102")],
                OrderbookInputOrder::Ascending,
            ),
            (
                vec![level("102"), level("101")],
                OrderbookInputOrder::Descending,
            ),
        ] {
            for (bids, bid_order) in [
                (
                    vec![level("99"), level("98")],
                    OrderbookInputOrder::Descending,
                ),
                (
                    vec![level("98"), level("99")],
                    OrderbookInputOrder::Ascending,
                ),
            ] {
                let value = book(asks.clone(), bids).expect("호가");
                assert_eq!(value.ask_input_order(), ask_order);
                assert_eq!(value.bid_input_order(), bid_order);
                assert_eq!(
                    value
                        .asks()
                        .iter()
                        .map(crate::OrderbookLevel::price)
                        .collect::<Vec<_>>(),
                    vec![level("101").price, level("102").price]
                );
                assert_eq!(
                    value
                        .bids()
                        .iter()
                        .map(crate::OrderbookLevel::price)
                        .collect::<Vec<_>>(),
                    vec![level("99").price, level("98").price]
                );
                assert_eq!(
                    value.best_ask(),
                    BestQuote::Available(value.asks().first().expect("매도"))
                );
                assert_eq!(
                    value.best_bid(),
                    BestQuote::Available(value.bids().first().expect("매수"))
                );
            }
        }
    }
    #[test]
    fn reviewer_first_ask_regression_is_checked_after_both_input_directions() {
        for asks in [
            vec![level("101"), level("102")],
            vec![level("102"), level("101")],
        ] {
            book(asks.clone(), vec![level("100.5")]).expect("수용");
            for bid in ["101.5", "101", "102"] {
                assert_eq!(
                    book(asks.clone(), vec![level(bid)]),
                    Err(OrderbookError::CrossedOrLocked)
                );
            }
        }
    }
    #[test]
    fn duplicates_nonmonotone_and_currency_mismatches_are_rejected() {
        for levels in [
            vec![level("101"), level("101")],
            vec![level("101"), level("103"), level("102")],
        ] {
            assert_eq!(book(levels.clone(), vec![]), Err(OrderbookError::AskOrder));
            assert_eq!(book(vec![], levels), Err(OrderbookError::BidOrder));
        }
        let usd = OrderbookLevelInput {
            price: price("1", Currency::Usd),
            volume: level("1").volume,
        };
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
                collected_at: at(0)
            }),
            Err(OrderbookError::CurrencyMismatch)
        );
    }
    #[test]
    fn empty_and_single_sides_preserve_identity_volume_and_closed_absence() {
        for asks in [vec![], vec![level("101")]] {
            for bids in [vec![], vec![level("99")]] {
                let value = book(asks.clone(), bids.clone()).expect("호가");
                assert_book_shape(&value, &asks, &bids);
            }
        }
    }
    fn assert_book_shape(
        value: &Orderbook,
        asks: &[OrderbookLevelInput],
        bids: &[OrderbookLevelInput],
    ) {
        assert_eq!(value.instrument(), &instrument(KnownMarket::Kr));
        assert_eq!(value.timestamp(), Some(at(0)));
        assert_eq!(value.collected_at(), at(1));
        assert_eq!(
            value.clock_alignment(),
            OrderbookTimeAlignment::Known(ClockAlignment::Aligned)
        );
        assert_eq!(
            value.ask_input_order(),
            if asks.is_empty() {
                OrderbookInputOrder::Empty
            } else {
                OrderbookInputOrder::Single
            }
        );
        assert_eq!(
            value.bid_input_order(),
            if bids.is_empty() {
                OrderbookInputOrder::Empty
            } else {
                OrderbookInputOrder::Single
            }
        );
        for entry in value.asks().iter().chain(value.bids()) {
            assert_eq!(entry.instrument(), value.instrument());
            assert_eq!(entry.volume(), level("1").volume);
        }
        if asks.is_empty() {
            assert_eq!(value.best_ask(), BestQuote::NoQuote);
        }
        if bids.is_empty() {
            assert_eq!(value.best_bid(), BestQuote::NoQuote);
        }
    }
    #[test]
    fn unknown_time_is_not_aligned_and_small_skew_is_preserved() {
        for (timestamp, expected) in [
            (None, Ok(OrderbookTimeAlignment::Unknown)),
            (
                Some(at(0)),
                Ok(OrderbookTimeAlignment::Known(ClockAlignment::Aligned)),
            ),
            (
                Some(at(2)),
                Ok(OrderbookTimeAlignment::Known(
                    ClockAlignment::AheadWithinTolerance,
                )),
            ),
            (Some(at(3)), Err(OrderbookError::FutureTimestamp)),
        ] {
            let result = Orderbook::try_new(OrderbookInput {
                instrument: instrument(KnownMarket::Kr),
                currency: Currency::Krw,
                asks: vec![],
                bids: vec![],
                timestamp,
                collected_at: at(0),
            });
            assert_eq!(
                result.map(|value| {
                    assert_eq!(value.timestamp(), timestamp);
                    value.clock_alignment()
                }),
                expected
            );
        }
    }
    #[test]
    fn off_tick_levels_are_preserved_and_cannot_use_another_instruments_table() {
        let value = book(vec![level("101.5")], vec![]).expect("단위 외 호가도 보존");
        let table =
            TickTable::from_master(&master(KnownMarket::Kr, "STOCK", "stable")).expect("표");
        let foreign =
            TickTable::from_master(&master(KnownMarket::Kr, "STOCK", "other")).expect("다른 표");
        let entry = value.asks().first().expect("단계");
        assert_eq!(entry.tick_alignment(&table), Ok(TickAlignment::OffTick));
        assert_eq!(
            entry.tick_alignment(&foreign),
            Err(TickSizeError::DifferentInstrument)
        );
    }
    #[test]
    fn accepted_books_have_strict_spreads_for_4096_generated_cases() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(1_u32..100_000, 1_u32..100_000), |(ask, bid)| {
                let result = book(vec![level(&ask.to_string())], vec![level(&bid.to_string())]);
                proptest::prop_assert_eq!(result.is_ok(), bid < ask);
                if let Ok(value) = result {
                    proptest::prop_assert_eq!(
                        value.asks().first().expect("매도").price(),
                        level(&ask.to_string()).price
                    );
                    proptest::prop_assert_eq!(
                        value.bids().first().expect("매수").price(),
                        level(&bid.to_string()).price
                    );
                }
                Ok(())
            })
            .expect("스프레드 속성");
    }
}
