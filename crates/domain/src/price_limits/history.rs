use crate::{
    ExcludedSecurityType, KnownMarket, LiquidationTrading, ListingStatus, MasterInstrument, Price,
    PriceLimitCheck, PriceLimitsError, Ratio,
};
use rust_decimal::Decimal;
pub(super) fn check_adjusted_move(
    instrument: &MasterInstrument,
    date: crate::CalendarDate,
    previous: Price,
    current: Price,
) -> Result<PriceLimitCheck, PriceLimitsError> {
    let row = instrument.row();
    if previous.as_money().currency() != row.currency
        || current.as_money().currency() != row.currency
    {
        return Err(PriceLimitsError::CurrencyMismatch);
    }
    let quote = instrument
        .quote_instrument()
        .or(Err(PriceLimitsError::MissingLimits))?;
    match quote.market() {
        KnownMarket::Us => return Ok(PriceLimitCheck::Unlimited),
        KnownMarket::Kr => {}
    }
    if (
        *date.year().as_i32(),
        *date.month().as_u8(),
        *date.day().as_u8(),
    ) < (2015_i32, 6_u8, 15_u8)
    {
        return Err(PriceLimitsError::MissingLimits);
    }
    if !["KOSPI", "KOSDAQ"].contains(&row.listing_market.as_str())
        || row.security_type.known() != Some(ExcludedSecurityType::Stock)
        || row.listing_status != ListingStatus::Active
        || row
            .korean_detail
            .is_none_or(|detail| detail.liquidation != LiquidationTrading::Regular)
    {
        return Err(PriceLimitsError::MissingLimits);
    }
    let lower = crate::exact_arithmetic::compare_positive_price_product(
        previous,
        Ratio::new(Decimal::new(7_i64, 1_u32)),
        current,
    );
    let upper = crate::exact_arithmetic::compare_positive_price_product(
        previous,
        Ratio::new(Decimal::new(13_i64, 1_u32)),
        current,
    );
    if lower.is_lt() {
        Ok(PriceLimitCheck::BelowLower)
    } else if upper.is_gt() {
        Ok(PriceLimitCheck::AboveUpper)
    } else {
        Ok(PriceLimitCheck::Within)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
        LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Price,
        PriceLimitCheck, PriceLimits, PriceLimitsError, Symbol, Timestamp, TradingSuspension,
        UnixSeconds,
    };
    fn row() -> MasterInstrumentRow {
        MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("market"),
            symbol: Symbol::try_new("005930").expect("symbol"),
            instrument_id: InstrumentId::try_new("history-guard").expect("id"),
            name: DisplayInstrumentName::try_new("합성 종목").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("type"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("time"),
        }
    }
    fn date() -> crate::CalendarDate {
        crate::CalendarDate::from_parts(
            crate::CalendarYear::new(2026),
            crate::MonthNumber::new(3),
            crate::DayNumber::new(25),
        )
        .expect("date")
    }
    fn price(value: &str, unit: Currency) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("input"), unit).expect("price")
    }
    #[test]
    fn regular_stock_daily_moves_include_both_exact_boundaries() {
        for market in ["KOSPI", "KOSDAQ"] {
            let mut input = row();
            input.listing_market = MasterCode::try_new(market).expect("market");
            let master = MasterInstrument::try_new(input).expect("master");
            for (value, expected) in [
                ("69.99999999999999999999999999", PriceLimitCheck::BelowLower),
                ("70", PriceLimitCheck::Within),
                ("100", PriceLimitCheck::Within),
                ("130", PriceLimitCheck::Within),
                ("130.0000000000000000000000001", PriceLimitCheck::AboveUpper),
            ] {
                assert_eq!(
                    PriceLimits::check_adjusted_move(
                        &master,
                        date(),
                        price("100", Currency::Krw),
                        price(value, Currency::Krw)
                    ),
                    Ok(expected)
                );
            }
        }
    }
    #[test]
    fn price_extremes_and_unrepresentable_fractional_bounds_are_exact() {
        let master = MasterInstrument::try_new(row()).expect("master");
        for (previous, current, expected) in [
            (
                "79228162514264337593543950335",
                "79228162514264337593543950335",
                PriceLimitCheck::Within,
            ),
            (
                "0.0000000000000000000000000001",
                "0.0000000000000000000000000001",
                PriceLimitCheck::Within,
            ),
            (
                "0.0000000000000000000000000005",
                "0.0000000000000000000000000003",
                PriceLimitCheck::BelowLower,
            ),
            (
                "0.0000000000000000000000000005",
                "0.0000000000000000000000000004",
                PriceLimitCheck::Within,
            ),
            (
                "0.0000000000000000000000000005",
                "0.0000000000000000000000000006",
                PriceLimitCheck::Within,
            ),
            (
                "0.0000000000000000000000000005",
                "0.0000000000000000000000000007",
                PriceLimitCheck::AboveUpper,
            ),
        ] {
            assert_eq!(
                PriceLimits::check_adjusted_move(
                    &master,
                    date(),
                    price(previous, Currency::Krw),
                    price(current, Currency::Krw)
                ),
                Ok(expected)
            );
        }
    }
    #[test]
    fn unsupported_market_type_and_status_never_receive_a_default_band() {
        let inputs = [
            {
                let mut item = row();
                item.listing_market = MasterCode::try_new("KR_ETC").expect("market");
                item
            },
            {
                let mut item = row();
                item.listing_market = MasterCode::try_new("FUTURE_MARKET").expect("market");
                item.korean_detail = None;
                item
            },
            {
                let mut item = row();
                item.security_type =
                    MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("type"));
                item
            },
            {
                let mut item = row();
                item.security_type =
                    MasterSecurityType::from_code(MasterCode::try_new("NEW_KIND").expect("type"));
                item
            },
            {
                let mut item = row();
                item.listing_status = ListingStatus::Scheduled;
                item
            },
            {
                let mut item = row();
                item.listing_status = ListingStatus::Delisted;
                item
            },
            {
                let mut item = row();
                item.korean_detail = None;
                item
            },
            {
                let mut item = row();
                item.korean_detail.as_mut().expect("detail").liquidation =
                    LiquidationTrading::Liquidation;
                item
            },
        ];
        for input in inputs {
            let master = MasterInstrument::try_new(input).expect("master");
            assert_eq!(
                PriceLimits::check_adjusted_move(
                    &master,
                    date(),
                    price("100", Currency::Krw),
                    price("100", Currency::Krw)
                ),
                Err(PriceLimitsError::MissingLimits)
            );
        }
    }
    #[test]
    fn currency_mismatch_is_rejected_before_market_policy() {
        let master = MasterInstrument::try_new(row()).expect("master");
        for (before, after) in [
            (Currency::Usd, Currency::Krw),
            (Currency::Krw, Currency::Usd),
            (Currency::Usd, Currency::Usd),
        ] {
            assert_eq!(
                PriceLimits::check_adjusted_move(
                    &master,
                    date(),
                    price("100", before),
                    price("100", after)
                ),
                Err(PriceLimitsError::CurrencyMismatch)
            );
        }
    }
    #[test]
    fn us_market_is_unlimited_after_currency_validation() {
        let mut input = row();
        input.listing_market = MasterCode::try_new("NASDAQ").expect("market");
        input.symbol = Symbol::try_new("AAPL").expect("symbol");
        input.currency = Currency::Usd;
        input.korean_detail = None;
        let master = MasterInstrument::try_new(input).expect("master");
        for value in ["1", "1000000"] {
            assert_eq!(
                PriceLimits::check_adjusted_move(
                    &master,
                    date(),
                    price("100", Currency::Usd),
                    price(value, Currency::Usd)
                ),
                Ok(PriceLimitCheck::Unlimited)
            );
        }
        assert_eq!(
            PriceLimits::check_adjusted_move(
                &master,
                date(),
                price("100", Currency::Usd),
                price("100", Currency::Krw)
            ),
            Err(PriceLimitsError::CurrencyMismatch)
        );
    }
    #[test]
    fn historical_policy_starts_at_the_verified_rule_date() {
        let master = MasterInstrument::try_new(row()).expect("master");
        for (day, expected) in [
            (14_u8, Err(PriceLimitsError::MissingLimits)),
            (15_u8, Ok(PriceLimitCheck::Within)),
        ] {
            let effective = crate::CalendarDate::from_parts(
                crate::CalendarYear::new(2015),
                crate::MonthNumber::new(6),
                crate::DayNumber::new(day),
            )
            .expect("date");
            assert_eq!(
                PriceLimits::check_adjusted_move(
                    &master,
                    effective,
                    price("100", Currency::Krw),
                    price("100", Currency::Krw)
                ),
                expected
            );
        }
    }
}
