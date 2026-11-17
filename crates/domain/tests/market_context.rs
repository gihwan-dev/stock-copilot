#[cfg(test)]
mod tests {
    use domain::{
        InvestorFlow, InvestorGroupRow, InvestorTradingRankings, KnownMarket,
        MarketFlowRecordInput, MarketFlowRow, MasterCode, Nanosecond, NumericInput, OffsetSeconds,
        ProgramContext, RankingLimit, StockFlowRecordInput, StockFlowRow, SupplyContext, Symbol,
        Timestamp, UnixSeconds,
    };

    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn code(value: &str) -> MasterCode {
        MasterCode::try_new(value).expect("code")
    }
    fn number(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("number")
    }
    fn symbol() -> Symbol {
        Symbol::try_new("005930").expect("symbol")
    }

    #[test]
    fn domestic_supply_contexts_expose_their_market() {
        let now = time();
        let date = now.offset_calendar_date();
        let supply = SupplyContext::from_response(symbol(), date, now, now).expect("supply");
        let program = ProgramContext::from_response(symbol(), date, now).expect("program");
        assert_eq!(supply.market(), KnownMarket::Kr);
        assert_eq!(program.market(), KnownMarket::Kr);
    }

    #[test]
    fn domestic_stock_and_indicator_flows_expose_their_market() {
        let now = time();
        let date = now.offset_calendar_date();
        let stock = InvestorFlow::from_stock_response(
            symbol(),
            StockFlowRecordInput {
                date,
                updated: now,
                rows: vec![StockFlowRow {
                    investor: code("foreigner"),
                    buy: number("11"),
                    sell: number("7"),
                    reported_net: number("4"),
                }],
                collected: now,
            },
        )
        .expect("stock");
        let rows = ["individual", "foreigner", "institution", "otherCorporation"]
            .into_iter()
            .map(|name| MarketFlowRow {
                investor: code(name),
                buy: number("7"),
                sell: number("7"),
            })
            .collect();
        let indicator = InvestorFlow::from_indicator_response(
            code("KOSPI"),
            &code("1d"),
            MarketFlowRecordInput {
                date,
                updated: now,
                rows,
                collected: now,
            },
        )
        .expect("indicator");
        assert_eq!(stock.market(), KnownMarket::Kr);
        assert_eq!(indicator.market(), KnownMarket::Kr);
    }

    #[test]
    fn domestic_investor_ranking_exposes_market_even_with_empty_groups() {
        let group = InvestorGroupRow {
            ranked_at: None,
            net_buy: vec![],
            net_sell: vec![],
        };
        let result = InvestorTradingRankings::from_response(
            group.clone(),
            group.clone(),
            group,
            RankingLimit::try_new(1).expect("limit"),
            time(),
        )
        .expect("rankings");
        assert_eq!(result.market(), KnownMarket::Kr);
    }
}
