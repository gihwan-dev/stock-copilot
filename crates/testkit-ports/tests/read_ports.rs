#[cfg(test)]
mod tests {
    use application::{
        HttpStatus, PortError, PortErrorKind, RankingQuery, ReadEndpoint, ResponseMismatch,
    };
    use domain::{
        AccountId, BrokerOrderId, CandlePeriod, CommonShare, Currency, DisplayInstrumentName,
        FlowInterval, FxPair, FxRateKind, IndicatorCode, InstrumentId, KnownMarket, ListingStatus,
        MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, Nanosecond,
        OffsetSeconds, PriceAdjustment, QuoteInstrument, RankingCautionFilter, RankingDuration,
        RankingKind, RankingLimit, ResolvedOrderInstrument, SectorId, SettlementWindow, Symbol,
        TimeInterval, Timestamp, TransactionFilter, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts,
        scripted::{BrokerReadCall, MarketInfoCall, ScriptedBrokerRead, ScriptedMarketInfo},
    };
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn window() -> TimeInterval {
        TimeInterval::try_new(at(11), at(29)).expect("구간")
    }
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: InstrumentId::try_new("fixture-instrument").expect("ID"),
            symbol: Symbol::try_new("AAPL").expect("종목"),
            name: DisplayInstrumentName::try_new("고정 종목 이름").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(17),
        })
        .expect("마스터")
    }
    fn quote() -> QuoteInstrument {
        master().quote_instrument().expect("시세 식별")
    }
    fn order_instrument() -> ResolvedOrderInstrument {
        master().order_instrument().expect("주문 식별")
    }
    fn ranking() -> RankingQuery {
        RankingQuery::try_new(
            RankingKind::TopLosers,
            KnownMarket::Us,
            RankingDuration::Month,
            RankingLimit::try_new(7).expect("상한"),
            RankingCautionFilter::Excluded,
        )
        .expect("질의")
    }
    #[test]
    fn broker_read_accounts_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Accounts,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_accounts().push(Err(error));
        run(contracts::broker_read_accounts(&port, Err(error)));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Accounts()],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_holdings_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Holdings,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_holdings().push(Err(error));
        run(contracts::broker_read_holdings(
            &port,
            &AccountId::new(173),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Holdings(AccountId::new(173))],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_transactions_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_transactions().push(Err(error));
        run(contracts::broker_read_transactions(
            &port,
            &AccountId::new(173),
            SettlementWindow::try_new(
                at(86_411).offset_calendar_date(),
                at(172_823).offset_calendar_date(),
            )
            .expect("결제 창"),
            &TransactionFilter::all(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Transactions(
                AccountId::new(173),
                SettlementWindow::try_new(
                    at(86_411).offset_calendar_date(),
                    at(172_823).offset_calendar_date()
                )
                .expect("결제 창"),
                TransactionFilter::all()
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_quotes_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Quotes,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_quotes().push(Err(error));
        run(contracts::broker_read_quotes(
            &port,
            &[quote(), quote()],
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Quotes([quote(), quote()].to_vec())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_candles_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Candles,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_candles().push(Err(error));
        run(contracts::broker_read_candles(
            &port,
            &quote(),
            (window(), CandlePeriod::Minute, PriceAdjustment::Raw),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Candles(
                quote(),
                window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_buying_power_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::BuyingPower,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_buying_power().push(Err(error));
        run(contracts::broker_read_buying_power(
            &port,
            &AccountId::new(173),
            Currency::Krw,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::BuyingPower(
                AccountId::new(173),
                Currency::Krw
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_sellable_quantity_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::SellableQuantity,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_sellable_quantity().push(Err(error));
        run(contracts::broker_read_sellable_quantity(
            &port,
            &AccountId::new(173),
            &order_instrument(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::SellableQuantity(
                AccountId::new(173),
                order_instrument()
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_open_orders_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::OpenOrders,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_open_orders().push(Err(error));
        run(contracts::broker_read_open_orders(
            &port,
            &AccountId::new(173),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::OpenOrders(AccountId::new(173))],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_closed_orders_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::ClosedOrders,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_closed_orders().push(Err(error));
        run(contracts::broker_read_closed_orders(
            &port,
            &AccountId::new(173),
            window(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::ClosedOrders(AccountId::new(173), window())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_order_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Order,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_order().push(Err(error));
        run(contracts::broker_read_order(
            &port,
            &AccountId::new(173),
            &BrokerOrderId::try_new("fixture-order-59").expect("주문 키"),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Order(
                AccountId::new(173),
                BrokerOrderId::try_new("fixture-order-59").expect("주문 키")
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_stock_warnings_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::StockWarnings,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_stock_warnings().push(Err(error));
        run(contracts::broker_read_stock_warnings(
            &port,
            &order_instrument(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::StockWarnings(order_instrument())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_instruments_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Instruments,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_instruments().push(Err(error));
        run(contracts::broker_read_instruments(
            &port,
            KnownMarket::Us,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Instruments(KnownMarket::Us)],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_orderbook_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Orderbook,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_orderbook().push(Err(error));
        run(contracts::broker_read_orderbook(
            &port,
            &quote(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Orderbook(quote())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_price_limits_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::PriceLimits,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_price_limits().push(Err(error));
        run(contracts::broker_read_price_limits(
            &port,
            &quote(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::PriceLimits(quote())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn broker_read_commission_rate_preserves_error_and_call() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::CommissionRate,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_commission_rate().push(Err(error));
        run(contracts::broker_read_commission_rate(
            &port,
            &AccountId::new(173),
            KnownMarket::Us,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::CommissionRate(
                AccountId::new(173),
                KnownMarket::Us
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_fx_rate_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::FxRate,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_fx_rate().push(Err(error));
        run(contracts::market_info_fx_rate(
            &port,
            FxPair::KrwUsd,
            FxRateKind::Buy,
            Some(at(37)),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::FxRate(
                FxPair::KrwUsd,
                FxRateKind::Buy,
                Some(at(37))
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_order_fx_rates_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::OrderFxRates,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_order_fx_rates().push(Err(error));
        run(contracts::market_info_order_fx_rates(&port, Err(error)));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::OrderFxRates()],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_indicator_quotes_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::IndicatorQuotes,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_indicator_quotes().push(Err(error));
        run(contracts::market_info_indicator_quotes(
            &port,
            &[
                IndicatorCode::Kosdaq,
                IndicatorCode::Kospi,
                IndicatorCode::Kosdaq,
            ],
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::IndicatorQuotes(
                [
                    IndicatorCode::Kosdaq,
                    IndicatorCode::Kospi,
                    IndicatorCode::Kosdaq
                ]
                .to_vec()
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_indicator_candles_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::IndicatorCandles,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_indicator_candles().push(Err(error));
        run(contracts::market_info_indicator_candles(
            &port,
            IndicatorCode::KrBond3Y,
            window(),
            CandlePeriod::Minute,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::IndicatorCandles(
                IndicatorCode::KrBond3Y,
                window(),
                CandlePeriod::Minute
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_rankings_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::Rankings,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_rankings().push(Err(error));
        run(contracts::market_info_rankings(
            &port,
            &ranking(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::Rankings(ranking())],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_stock_investor_flow_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::StockInvestorFlow,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_stock_investor_flow().push(Err(error));
        run(contracts::market_info_stock_investor_flow(
            &port,
            &quote(),
            at(86_411).offset_calendar_date(),
            at(172_823).offset_calendar_date(),
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::StockInvestorFlow(
                quote(),
                at(86_411).offset_calendar_date(),
                at(172_823).offset_calendar_date()
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_indicator_investor_flow_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::IndicatorInvestorFlow,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_indicator_investor_flow().push(Err(error));
        run(contracts::market_info_indicator_investor_flow(
            &port,
            IndicatorCode::KrBond3Y,
            (
                at(86_411).offset_calendar_date(),
                at(172_823).offset_calendar_date(),
            ),
            FlowInterval::Week,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::IndicatorInvestorFlow(
                IndicatorCode::KrBond3Y,
                at(86_411).offset_calendar_date(),
                at(172_823).offset_calendar_date(),
                FlowInterval::Week
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_sector_composition_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::SectorComposition,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_sector_composition().push(Err(error));
        run(contracts::market_info_sector_composition(
            &port,
            &SectorId::try_new("fixture-sector-61").expect("섹터"),
            KnownMarket::Us,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::SectorComposition(
                SectorId::try_new("fixture-sector-61").expect("섹터"),
                KnownMarket::Us
            )],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn market_info_calendar_preserves_error_and_call() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::Calendar,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        port.script_calendar().push(Err(error));
        run(contracts::market_info_calendar(
            &port,
            KnownMarket::Us,
            Err(error),
        ));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::Calendar(KnownMarket::Us)],
            "인자 및 호출 순서 보존"
        );
    }
    #[test]
    fn duplicate_quote_slots_keep_partial_success_and_failure() {
        let port = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Quotes,
            HttpStatus::NotFound,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        let observed = domain::Quote::try_new(
            Symbol::try_new("AAPL").expect("종목"),
            Currency::Usd,
            None,
            Some(at(13)),
            at(17),
        )
        .expect("시세");
        let expected = vec![
            application::QuoteObservation {
                instrument: quote(),
                result: Ok(observed),
            },
            application::QuoteObservation {
                instrument: quote(),
                result: Err(error),
            },
        ];
        port.script_quotes().push(Ok(expected.clone()));
        run(contracts::broker_read_quotes(
            &port,
            &[quote(), quote()],
            Ok(expected),
        ));
        assert_eq!(
            port.calls(),
            [BrokerReadCall::Quotes(vec![quote(), quote()])],
            "중복 요청 슬롯과 원래 순서 보존"
        );
    }
}
