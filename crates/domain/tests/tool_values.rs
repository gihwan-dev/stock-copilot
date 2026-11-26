#[cfg(test)]
mod tests {
    use domain::{
        CalendarDate, CalendarYear, Currency, DayNumber, InvestorClass, InvestorFlow,
        MarketFlowRecordInput, MarketFlowRow, MarketIndicatorQuote, MasterCode, Money, MonthNumber,
        Nanosecond, NumericInput, OffsetSeconds, Price, Ratio, StockFlowRecordInput, StockFlowRow,
        Symbol, TimeError, Timestamp, UnixSeconds, WeekPart,
    };

    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_795_564_800),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("time")
    }
    fn code(value: &str) -> MasterCode {
        MasterCode::try_new(value).expect("code")
    }
    fn number(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("number")
    }
    fn krw(value: &str) -> Money {
        Money::parse(&number(value), Currency::Krw).expect("money")
    }
    fn price(value: &str, currency: Currency) -> Price {
        Price::parse(&number(value), currency).expect("price")
    }

    fn stock_flow(buy: &str, sell: &str, net: &str) -> InvestorFlow {
        let now = time();
        InvestorFlow::from_stock_response(
            Symbol::try_new("000660").expect("symbol"),
            StockFlowRecordInput {
                date: now.offset_calendar_date(),
                updated: now,
                rows: vec![StockFlowRow {
                    investor: code("foreigner"),
                    buy: number(buy),
                    sell: number(sell),
                    reported_net: number(net),
                }],
                collected: now,
            },
        )
        .expect("stock flow")
    }

    #[test]
    fn stock_flow_net_shares_are_valued_at_the_krw_close() {
        let flow = stock_flow("300", "100", "200");
        let foreigner = flow
            .top_level()
            .iter()
            .find(|item| item.investor() == InvestorClass::Foreigner)
            .expect("foreigner");
        assert_eq!(
            foreigner.net_krw_at(Some(price("1500", Currency::Krw))),
            Some(krw("300000"))
        );
        let selling = stock_flow("100", "350", "-250");
        assert_eq!(
            selling.top_level()[0].net_krw_at(Some(price("2", Currency::Krw))),
            Some(krw("-500"))
        );
        assert_eq!(foreigner.net_krw_at(None), None);
        assert_eq!(
            foreigner.net_krw_at(Some(price("1500", Currency::Usd))),
            None
        );
    }

    #[test]
    fn market_flow_net_amounts_are_already_krw() {
        let now = time();
        let rows = [
            ("individual", "9", "4"),
            ("foreigner", "4", "9"),
            ("institution", "5", "5"),
            ("otherCorporation", "1", "1"),
        ]
        .into_iter()
        .map(|(name, buy, sell)| MarketFlowRow {
            investor: code(name),
            buy: number(buy),
            sell: number(sell),
        })
        .collect();
        let flow = InvestorFlow::from_indicator_response(
            code("KOSPI"),
            &code("1d"),
            MarketFlowRecordInput {
                date: now.offset_calendar_date(),
                updated: now,
                rows,
                collected: now,
            },
        )
        .expect("market flow");
        let net = |class: InvestorClass| {
            flow.top_level()
                .iter()
                .find(|item| item.investor() == class)
                .and_then(|item| item.net_krw_at(None))
        };
        assert_eq!(net(InvestorClass::Individual), Some(krw("5")));
        assert_eq!(net(InvestorClass::Foreigner), Some(krw("-5")));
    }

    #[test]
    fn indicator_value_exposes_its_number() {
        let quote =
            MarketIndicatorQuote::from_response(code("KOSPI"), &number("2512.34"), None, time())
                .expect("quote");
        assert_eq!(
            quote.value().as_ratio(),
            Ratio::parse(&number("2512.34")).expect("ratio")
        );
        let bond = MarketIndicatorQuote::from_response(
            code("KR_BOND_10Y"),
            &number("-0.25"),
            None,
            time(),
        )
        .expect("bond");
        assert_eq!(
            bond.value().as_ratio(),
            Ratio::parse(&number("-0.25")).expect("ratio")
        );
    }

    #[test]
    fn calendar_date_starts_at_local_midnight() {
        let date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(27),
        )
        .expect("date");
        let start = date
            .start_at_offset(OffsetSeconds::new(32_400))
            .expect("start");
        assert_eq!(*start.utc_seconds().as_i64(), 1_795_705_200_i64);
        assert_eq!(*start.offset_seconds().as_i32(), 32_400_i32);
        assert_eq!(start.offset_calendar_date(), date);
        let utc = date.start_at_offset(OffsetSeconds::new(0)).expect("utc");
        assert_eq!(*utc.utc_seconds().as_i64(), 1_795_737_600_i64);
        assert_eq!(
            date.start_at_offset(OffsetSeconds::new(30)),
            Err(TimeError::InvalidComponent)
        );
    }

    #[test]
    fn ratio_reads_a_money_amount() {
        assert_eq!(
            Ratio::of_amount(krw("1385.20").amount()),
            Ratio::parse(&number("1385.20")).expect("ratio")
        );
    }

    #[test]
    fn calendar_dates_know_weekends() {
        let day = |value: u8| {
            CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(value),
            )
            .expect("date")
        };
        assert_eq!(day(27).week_part(), WeekPart::Weekday);
        assert_eq!(day(28).week_part(), WeekPart::Weekend);
        assert_eq!(day(29).week_part(), WeekPart::Weekend);
        assert_eq!(day(30).week_part(), WeekPart::Weekday);
        assert_eq!(day(23).week_part(), WeekPart::Weekday);
    }
}
