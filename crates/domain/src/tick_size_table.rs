use crate::{
    Amount, Currency, ExcludedSecurityType, KnownMarket, MasterInstrument, Money, Price,
    QuoteInstrument, TickAlignment, TickRounding, TickSize, TickSizeError,
};
use std::cmp::Ordering;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickSchedule {
    KrStock,
    KrExchangeProduct,
    Us,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickTableSource {
    KrxDraft20230125,
    TossOpenApi,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickTable {
    instrument: QuoteInstrument,
    schedule: TickSchedule,
}
#[derive(Debug, Clone, Copy)]
enum TickBoundary {
    One,
    TwoThousand,
    FiveThousand,
    TwentyThousand,
    FiftyThousand,
    TwoHundredThousand,
    FiveHundredThousand,
}
#[derive(Debug, Clone, Copy)]
enum TickUnit {
    One,
    Five,
    Ten,
    Fifty,
    Hundred,
    FiveHundred,
    Thousand,
    UsSmall,
    UsRegular,
}
const KR_STOCK_DRAFT_20230125: [(TickBoundary, TickUnit); 6] = [
    (TickBoundary::TwoThousand, TickUnit::One),
    (TickBoundary::FiveThousand, TickUnit::Five),
    (TickBoundary::TwentyThousand, TickUnit::Ten),
    (TickBoundary::FiftyThousand, TickUnit::Fifty),
    (TickBoundary::TwoHundredThousand, TickUnit::Hundred),
    (TickBoundary::FiveHundredThousand, TickUnit::FiveHundred),
];
const KR_EXCHANGE_PRODUCT_DRAFT_20230125: [(TickBoundary, TickUnit); 1] =
    [(TickBoundary::TwoThousand, TickUnit::One)];
const US_OPENAPI: [(TickBoundary, TickUnit); 1] = [(TickBoundary::One, TickUnit::UsSmall)];
impl TickBoundary {
    fn compare(self, price: Price) -> Ordering {
        let value = price.as_money().amount().as_decimal();
        let ceiling = match self {
            Self::One => rust_decimal::Decimal::ONE,
            Self::TwoThousand => rust_decimal::Decimal::from(2_000_u32),
            Self::FiveThousand => rust_decimal::Decimal::from(5_000_u32),
            Self::TwentyThousand => rust_decimal::Decimal::from(20_000_u32),
            Self::FiftyThousand => rust_decimal::Decimal::from(50_000_u32),
            Self::TwoHundredThousand => rust_decimal::Decimal::from(200_000_u32),
            Self::FiveHundredThousand => rust_decimal::Decimal::from(500_000_u32),
        };
        value.cmp(&ceiling)
    }
}
impl TickUnit {
    fn tick(self, currency: Currency) -> Result<TickSize, TickSizeError> {
        let amount = match self {
            Self::One => rust_decimal::Decimal::ONE,
            Self::Five => rust_decimal::Decimal::from(5_u32),
            Self::Ten => rust_decimal::Decimal::from(10_u32),
            Self::Fifty => rust_decimal::Decimal::from(50_u32),
            Self::Hundred => rust_decimal::Decimal::from(100_u32),
            Self::FiveHundred => rust_decimal::Decimal::from(500_u32),
            Self::Thousand => rust_decimal::Decimal::from(1_000_u32),
            Self::UsSmall => rust_decimal::Decimal::new(1, 4),
            Self::UsRegular => rust_decimal::Decimal::new(1, 2),
        };
        Price::try_new(Money::new(Amount::new(amount), currency))
            .map(TickSize::from_response)
            .map_err(TickSizeError::NumericValue)
    }
}
impl TickTable {
    pub fn from_master(master: &MasterInstrument) -> Result<Self, TickSizeError> {
        let instrument = master
            .quote_instrument()
            .map_err(|_error| TickSizeError::Unavailable)?;
        let schedule = schedule_for(master, &instrument)?;
        Ok(Self {
            instrument,
            schedule,
        })
    }
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn source(&self) -> TickTableSource {
        match self.schedule {
            TickSchedule::KrStock | TickSchedule::KrExchangeProduct => {
                TickTableSource::KrxDraft20230125
            }
            TickSchedule::Us => TickTableSource::TossOpenApi,
        }
    }
    pub fn tick_size(
        &self,
        instrument: &QuoteInstrument,
        price: Price,
    ) -> Result<TickSize, TickSizeError> {
        if instrument != &self.instrument {
            return Err(TickSizeError::DifferentInstrument);
        }
        if price.as_money().currency() != instrument.currency() {
            return Err(TickSizeError::CurrencyMismatch);
        }
        let (bands, last) = match self.schedule {
            TickSchedule::KrStock => (KR_STOCK_DRAFT_20230125.as_slice(), TickUnit::Thousand),
            TickSchedule::KrExchangeProduct => (
                KR_EXCHANGE_PRODUCT_DRAFT_20230125.as_slice(),
                TickUnit::Five,
            ),
            TickSchedule::Us => (US_OPENAPI.as_slice(), TickUnit::UsRegular),
        };
        bands
            .iter()
            .find(|(boundary, _)| boundary.compare(price) == Ordering::Less)
            .map_or(last, |(_, unit)| *unit)
            .tick(instrument.currency())
    }
}
fn schedule_for(
    master: &MasterInstrument,
    instrument: &QuoteInstrument,
) -> Result<TickSchedule, TickSizeError> {
    let security = master
        .row()
        .security_type
        .known()
        .ok_or(TickSizeError::Unavailable)?;
    if security == ExcludedSecurityType::StockWarrants {
        return Err(TickSizeError::Unavailable);
    }
    if *instrument.market() == KnownMarket::Us {
        return Ok(TickSchedule::Us);
    }
    if !["KOSPI", "KOSDAQ"].contains(&master.row().listing_market.as_str()) {
        return Err(TickSizeError::Unavailable);
    }
    match security {
        ExcludedSecurityType::Stock
        | ExcludedSecurityType::ForeignStock
        | ExcludedSecurityType::DepositaryReceipt => Ok(TickSchedule::KrStock),
        ExcludedSecurityType::Etf
        | ExcludedSecurityType::ForeignEtf
        | ExcludedSecurityType::Etn => Ok(TickSchedule::KrExchangeProduct),
        ExcludedSecurityType::InfrastructureFund
        | ExcludedSecurityType::Reit
        | ExcludedSecurityType::StockWarrants => Err(TickSizeError::Unavailable),
    }
}
pub fn tick_alignment(
    table: &TickTable,
    instrument: &QuoteInstrument,
    price: Price,
) -> Result<TickAlignment, TickSizeError> {
    crate::tick_size::align_fixed(price, table.tick_size(instrument, price)?)
}
pub fn round_to_tick(
    table: &TickTable,
    instrument: &QuoteInstrument,
    price: Price,
    direction: TickRounding,
) -> Result<Price, TickSizeError> {
    let rounded =
        crate::tick_size::round_fixed(price, table.tick_size(instrument, price)?, direction)?;
    if rounded.amount().as_decimal().is_zero() {
        return Err(TickSizeError::BelowFirstTick);
    }
    Price::try_new(rounded).map_err(TickSizeError::NumericValue)
}
#[cfg(test)]
mod tests {
    use crate::orderbook::tests::{instrument, master, price};
    use crate::{
        Amount, Currency, KnownMarket, MasterCode, MasterInstrument, Money, Price, TickAlignment,
        TickRounding, TickSizeError, TickTable, TickTableSource, round_to_tick, tick_alignment,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn table(market: KnownMarket, security: &str) -> TickTable {
        TickTable::from_master(&master(market, security, "stable")).expect("표")
    }
    fn check_case(table: &TickTable, input: &str, down: &str, up: &str) {
        let query = table.instrument();
        let currency = query.currency();
        let candidate = price(input, currency);
        assert_eq!(
            round_to_tick(table, query, candidate, TickRounding::Down),
            Ok(price(down, currency))
        );
        assert_eq!(
            round_to_tick(table, query, candidate, TickRounding::Up),
            Ok(price(up, currency))
        );
        assert_eq!(
            tick_alignment(table, query, price(down, currency)),
            Ok(TickAlignment::OnTick)
        );
        assert_eq!(
            tick_alignment(table, query, price(up, currency)),
            Ok(TickAlignment::OnTick)
        );
    }
    #[test]
    fn stock_bands_use_upper_band_at_every_exact_boundary() {
        let table = table(KnownMarket::Kr, "STOCK");
        assert_eq!(table.instrument(), &instrument(KnownMarket::Kr));
        assert_eq!(table.source(), TickTableSource::KrxDraft20230125);
        for (input, expected) in [
            ("1999", "1"),
            ("2000", "5"),
            ("4999", "5"),
            ("5000", "10"),
            ("19999", "10"),
            ("20000", "50"),
            ("49999", "50"),
            ("50000", "100"),
            ("199999", "100"),
            ("200000", "500"),
            ("499999", "500"),
            ("500000", "1000"),
        ] {
            assert_eq!(
                table
                    .tick_size(table.instrument(), price(input, Currency::Krw))
                    .expect("단위")
                    .price(),
                price(expected, Currency::Krw)
            );
        }
        for (input, down, up) in [
            ("1999.5", "1999", "2000"),
            ("4999", "4995", "5000"),
            ("19999", "19990", "20000"),
            ("49990", "49950", "50000"),
            ("199999", "199900", "200000"),
            ("499999", "499500", "500000"),
            ("500001", "500000", "501000"),
        ] {
            check_case(&table, input, down, up);
        }
    }
    #[test]
    fn etf_etn_and_us_bands_round_across_boundaries() {
        for security in ["ETF", "FOREIGN_ETF", "ETN"] {
            let table = table(KnownMarket::Kr, security);
            assert_eq!(table.source(), TickTableSource::KrxDraft20230125);
            check_case(&table, "1999.5", "1999", "2000");
            check_case(&table, "2001", "2000", "2005");
            assert_eq!(
                table
                    .tick_size(table.instrument(), price("2000", Currency::Krw))
                    .expect("단위")
                    .price(),
                price("5", Currency::Krw)
            );
        }
        let us = table(KnownMarket::Us, "STOCK");
        assert_eq!(us.source(), TickTableSource::TossOpenApi);
        assert_eq!(
            us.tick_size(us.instrument(), price("0.5", Currency::Usd))
                .expect("소액")
                .price(),
            price("0.0001", Currency::Usd)
        );
        assert_eq!(
            us.tick_size(us.instrument(), price("1", Currency::Usd))
                .expect("일반")
                .price(),
            price("0.01", Currency::Usd)
        );
        check_case(&us, "0.99995", "0.9999", "1");
        check_case(&us, "1.001", "1", "1.01");
    }
    #[test]
    fn supported_and_unknown_master_types_are_explicit() {
        for security in [
            "STOCK",
            "FOREIGN_STOCK",
            "DEPOSITARY_RECEIPT",
            "ETF",
            "FOREIGN_ETF",
            "ETN",
        ] {
            TickTable::from_master(&master(KnownMarket::Kr, security, "stable")).expect("표");
        }
        for market in [KnownMarket::Kr, KnownMarket::Us] {
            for security in ["STOCK_WARRANTS", "ELW", "FUTURE"] {
                assert_eq!(
                    TickTable::from_master(&master(market, security, "stable")),
                    Err(TickSizeError::Unavailable)
                );
            }
        }
        for security in ["INFRASTRUCTURE_FUND", "REIT"] {
            assert_eq!(
                TickTable::from_master(&master(KnownMarket::Kr, security, "stable")),
                Err(TickSizeError::Unavailable)
            );
            TickTable::from_master(&master(KnownMarket::Us, security, "stable")).expect("표");
        }
        for listing in ["KOSDAQ", "KR_ETC", "JP"] {
            let mut row = master(KnownMarket::Kr, "STOCK", "stable").row().clone();
            row.listing_market = MasterCode::try_new(listing).expect("시장");
            let result = TickTable::from_master(&MasterInstrument::try_new(row).expect("마스터"));
            assert_eq!(result.is_ok(), listing == "KOSDAQ");
        }
    }
    #[test]
    fn zero_floor_and_different_instrument_are_closed_failures() {
        for (market, input) in [(KnownMarket::Kr, "0.5"), (KnownMarket::Us, "0.00001")] {
            let table = table(market, "STOCK");
            let candidate = price(input, table.instrument().currency());
            assert_eq!(
                round_to_tick(&table, table.instrument(), candidate, TickRounding::Down),
                Err(TickSizeError::BelowFirstTick)
            );
            round_to_tick(&table, table.instrument(), candidate, TickRounding::Up).expect("올림");
        }
        let table = table(KnownMarket::Kr, "STOCK");
        let foreign = master(KnownMarket::Kr, "STOCK", "other")
            .quote_instrument()
            .expect("다른 종목");
        let candidate = price("100", Currency::Krw);
        assert_eq!(
            table.tick_size(&foreign, candidate),
            Err(TickSizeError::DifferentInstrument)
        );
        assert_eq!(
            tick_alignment(&table, &foreign, candidate),
            Err(TickSizeError::DifferentInstrument)
        );
        assert_eq!(
            round_to_tick(&table, &foreign, candidate, TickRounding::Up),
            Err(TickSizeError::DifferentInstrument)
        );
        assert_eq!(
            table.tick_size(table.instrument(), price("100", Currency::Usd)),
            Err(TickSizeError::CurrencyMismatch)
        );
    }
    #[test]
    fn maximum_domestic_price_rounds_down_and_rejects_overflow_above() {
        let table = table(KnownMarket::Kr, "STOCK");
        let candidate = price("79228162514264337593543950335", Currency::Krw);
        assert_eq!(
            round_to_tick(&table, table.instrument(), candidate, TickRounding::Down),
            Ok(price("79228162514264337593543950000", Currency::Krw))
        );
        assert_eq!(
            round_to_tick(&table, table.instrument(), candidate, TickRounding::Up),
            Err(TickSizeError::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn each_market_table_matches_independent_integer_reference_for_4096_cases() {
        for (market, security, kind) in [
            (KnownMarket::Kr, "STOCK", 0_u32),
            (KnownMarket::Kr, "ETF", 1),
            (KnownMarket::Us, "STOCK", 2),
        ] {
            let table = table(market, security);
            let strategy = proptest::prop_oneof![1_u64..=2_000_000, 1_u64..=1_000_000_000_000];
            TestRunner::new(Config::with_cases(testkit::property_cases()))
                .run(&strategy, |coefficient| {
                    reference_case(&table, kind, coefficient)
                })
                .expect("독립 격자");
        }
    }
    fn reference_step(kind: u32, coefficient: u64) -> u64 {
        match kind {
            0 => match coefficient {
                0..=1_999_999_999 => 1_000_000,
                2_000_000_000..=4_999_999_999 => 5_000_000,
                5_000_000_000..=19_999_999_999 => 10_000_000,
                20_000_000_000..=49_999_999_999 => 50_000_000,
                50_000_000_000..=199_999_999_999 => 100_000_000,
                200_000_000_000..=499_999_999_999 => 500_000_000,
                _ => 1_000_000_000,
            },
            1 => {
                if coefficient < 2_000_000_000 {
                    1_000_000
                } else {
                    5_000_000
                }
            }
            _ => {
                if coefficient < 1_000_000 {
                    100
                } else {
                    10_000
                }
            }
        }
    }
    fn reference_price(table: &TickTable, coefficient: u64) -> Price {
        Price::try_new(Money::new(
            Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                i128::from(coefficient),
                6,
            )),
            table.instrument().currency(),
        ))
        .expect("참조 가격")
    }
    fn reference_case(
        table: &TickTable,
        kind: u32,
        coefficient: u64,
    ) -> proptest::test_runner::TestCaseResult {
        let candidate = reference_price(table, coefficient);
        let step = reference_step(kind, coefficient);
        let remainder = coefficient.checked_rem(step).expect("양수 단위");
        let down = coefficient.checked_sub(remainder).expect("내림");
        let up = if remainder == 0 {
            coefficient
        } else {
            down.checked_add(step).expect("올림")
        };
        let expected_down = if down == 0 {
            Err(TickSizeError::BelowFirstTick)
        } else {
            Ok(reference_price(table, down))
        };
        proptest::prop_assert_eq!(
            round_to_tick(table, table.instrument(), candidate, TickRounding::Down),
            expected_down
        );
        proptest::prop_assert_eq!(
            round_to_tick(table, table.instrument(), candidate, TickRounding::Up),
            Ok(reference_price(table, up))
        );
        proptest::prop_assert_eq!(
            tick_alignment(table, table.instrument(), candidate),
            Ok(if remainder == 0 {
                TickAlignment::OnTick
            } else {
                TickAlignment::OffTick
            })
        );
        proptest::prop_assert_eq!(
            down.checked_rem(reference_step(kind, down)).expect("배수"),
            0
        );
        proptest::prop_assert_eq!(up.checked_rem(reference_step(kind, up)).expect("배수"), 0);
        proptest::prop_assert!(down <= coefficient && coefficient <= up);
        proptest::prop_assert!(
            coefficient.checked_sub(down).expect("내림 차이") < step
                && up.checked_sub(coefficient).expect("올림 차이") < step
        );
        Ok(())
    }
}
