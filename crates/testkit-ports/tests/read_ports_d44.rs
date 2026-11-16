#[cfg(test)]
mod tests {
    use application::{
        BrokerCapabilities, BrokerCapability, CapabilitySupport, HttpStatus, PortError,
        PortErrorKind, ReadEndpoint,
    };
    use domain::{
        CalendarDate, CommonShare, CreditDetail, CreditSide, CreditTrade, Currency,
        DisplayInstrumentName, InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus,
        MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, Nanosecond,
        NumericInput, NxtTrading, OffsetSeconds, ProgramContext, ProgramTrade, ProgramVolume,
        QuoteInstrument, SecuritiesLending, ShortSelling, SupplyContext, SupplyKrw, SupplyRate,
        SupplyRatio, SupplyShares, SupplySignedShares, Symbol, Timestamp, TradingSuspension,
        UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts,
        scripted::{MarketInfoCall, ScriptedMarketInfo},
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
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn date(value: i64) -> CalendarDate {
        at(value).offset_calendar_date()
    }
    fn quote(code: &str) -> QuoteInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: InstrumentId::try_new(code).expect("식별자"),
            symbol: Symbol::try_new(code).expect("종목"),
            name: DisplayInstrumentName::try_new("수급 고정 종목").expect("이름"),
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
            observed_at: at(11),
        })
        .expect("마스터")
        .quote_instrument()
        .expect("시세 식별")
    }
    fn num(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("숫자")
    }
    fn shares(value: &str) -> SupplyShares {
        SupplyShares::parse(&num(value)).expect("주")
    }
    fn krw(value: &str) -> SupplyKrw {
        SupplyKrw::parse(&num(value)).expect("원")
    }
    fn rate(value: &str) -> SupplyRate {
        SupplyRate::parse(&num(value)).expect("비율")
    }
    fn context() -> SupplyContext {
        SupplyContext::from_response(
            Symbol::try_new("000660").expect("종목"),
            date(345_600),
            at(345_713),
            at(345_739),
        )
        .expect("맥락")
    }
    fn program() -> ProgramTrade {
        let first = ProgramVolume::try_new(
            shares("11"),
            shares("17"),
            SupplySignedShares::parse(&num("-6")).expect("순수량"),
        )
        .expect("차익");
        let other = ProgramVolume::try_new(
            shares("29"),
            shares("23"),
            SupplySignedShares::parse(&num("6")).expect("순수량"),
        )
        .expect("비차익");
        ProgramTrade::from_response(
            ProgramContext::from_response(
                Symbol::try_new("000660").expect("종목"),
                date(345_600),
                at(345_739),
            )
            .expect("프로그램 맥락"),
            first,
            other,
        )
    }
    fn short() -> ShortSelling {
        ShortSelling::from_response(
            context(),
            shares("31"),
            krw("4700"),
            SupplyRatio::Reported(rate("0.03215")),
            SupplyRatio::MissingDenominator,
        )
    }
    fn credit() -> CreditTrade {
        CreditTrade::from_response(
            context(),
            CreditSide::Reported(CreditDetail::from_response(
                shares("41"),
                shares("43"),
                shares("53"),
                rate("0.0042"),
                rate("0.09"),
            )),
            CreditSide::NoData,
        )
    }
    fn lending() -> SecuritiesLending {
        SecuritiesLending::from_response(
            context(),
            shares("61"),
            shares("67"),
            shares("71"),
            krw("7300"),
        )
    }

    #[test]
    fn program_trades_preserves_facts_empty_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let first = quote("005930");
        let other = quote("000660");
        let error = PortError::new(
            ReadEndpoint::ProgramTrades,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::ProgramTrades),
        );
        let value = vec![program()];
        port.script_program_trades().push(Err(error));
        port.script_program_trades().push(Ok(value.clone()));
        port.script_program_trades().push(Ok(vec![]));
        run(contracts::market_info_program_trades(
            &port,
            &first,
            date(0),
            date(86_400),
            Err(error),
        ));
        run(contracts::market_info_program_trades(
            &port,
            &other,
            date(259_200),
            date(432_000),
            Ok(value),
        ));
        run(contracts::market_info_program_trades(
            &port,
            &first,
            date(518_400),
            date(604_800),
            Ok(vec![]),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::ProgramTrades(first.clone(), date(0), date(86_400)),
                MarketInfoCall::ProgramTrades(other.clone(), date(259_200), date(432_000)),
                MarketInfoCall::ProgramTrades(first.clone(), date(518_400), date(604_800))
            ],
            "포트 전체의 인자·순서 기록"
        );
        assert_eq!(
            port.script_program_trades().calls(),
            [
                (first.clone(), date(0), date(86_400)),
                (other, date(259_200), date(432_000)),
                (first, date(518_400), date(604_800))
            ],
            "메서드별 인자 기록"
        );
    }

    #[test]
    fn short_selling_preserves_facts_empty_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let first = quote("005930");
        let other = quote("000660");
        let error = PortError::new(
            ReadEndpoint::ShortSelling,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::ShortSelling),
        );
        let value = vec![short()];
        port.script_short_selling().push(Err(error));
        port.script_short_selling().push(Ok(value.clone()));
        port.script_short_selling().push(Ok(vec![]));
        run(contracts::market_info_short_selling(
            &port,
            &first,
            date(0),
            date(86_400),
            Err(error),
        ));
        run(contracts::market_info_short_selling(
            &port,
            &other,
            date(259_200),
            date(432_000),
            Ok(value),
        ));
        run(contracts::market_info_short_selling(
            &port,
            &first,
            date(518_400),
            date(604_800),
            Ok(vec![]),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::ShortSelling(first.clone(), date(0), date(86_400)),
                MarketInfoCall::ShortSelling(other.clone(), date(259_200), date(432_000)),
                MarketInfoCall::ShortSelling(first.clone(), date(518_400), date(604_800))
            ],
            "포트 전체의 인자·순서 기록"
        );
        assert_eq!(
            port.script_short_selling().calls(),
            [
                (first.clone(), date(0), date(86_400)),
                (other, date(259_200), date(432_000)),
                (first, date(518_400), date(604_800))
            ],
            "메서드별 인자 기록"
        );
    }

    #[test]
    fn credit_trades_preserves_facts_empty_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let first = quote("005930");
        let other = quote("000660");
        let error = PortError::new(
            ReadEndpoint::CreditTrades,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::CreditTrades),
        );
        let value = vec![credit()];
        port.script_credit_trades().push(Err(error));
        port.script_credit_trades().push(Ok(value.clone()));
        port.script_credit_trades().push(Ok(vec![]));
        run(contracts::market_info_credit_trades(
            &port,
            &first,
            date(0),
            date(86_400),
            Err(error),
        ));
        run(contracts::market_info_credit_trades(
            &port,
            &other,
            date(259_200),
            date(432_000),
            Ok(value),
        ));
        run(contracts::market_info_credit_trades(
            &port,
            &first,
            date(518_400),
            date(604_800),
            Ok(vec![]),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::CreditTrades(first.clone(), date(0), date(86_400)),
                MarketInfoCall::CreditTrades(other.clone(), date(259_200), date(432_000)),
                MarketInfoCall::CreditTrades(first.clone(), date(518_400), date(604_800))
            ],
            "포트 전체의 인자·순서 기록"
        );
        assert_eq!(
            port.script_credit_trades().calls(),
            [
                (first.clone(), date(0), date(86_400)),
                (other, date(259_200), date(432_000)),
                (first, date(518_400), date(604_800))
            ],
            "메서드별 인자 기록"
        );
    }

    #[test]
    fn securities_lending_preserves_facts_empty_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let first = quote("005930");
        let other = quote("000660");
        let error = PortError::new(
            ReadEndpoint::SecuritiesLending,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::SecuritiesLending),
        );
        let value = vec![lending()];
        port.script_securities_lending().push(Err(error));
        port.script_securities_lending().push(Ok(value.clone()));
        port.script_securities_lending().push(Ok(vec![]));
        run(contracts::market_info_securities_lending(
            &port,
            &first,
            date(0),
            date(86_400),
            Err(error),
        ));
        run(contracts::market_info_securities_lending(
            &port,
            &other,
            date(259_200),
            date(432_000),
            Ok(value),
        ));
        run(contracts::market_info_securities_lending(
            &port,
            &first,
            date(518_400),
            date(604_800),
            Ok(vec![]),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::SecuritiesLending(first.clone(), date(0), date(86_400)),
                MarketInfoCall::SecuritiesLending(other.clone(), date(259_200), date(432_000)),
                MarketInfoCall::SecuritiesLending(first.clone(), date(518_400), date(604_800))
            ],
            "포트 전체의 인자·순서 기록"
        );
        assert_eq!(
            port.script_securities_lending().calls(),
            [
                (first.clone(), date(0), date(86_400)),
                (other, date(259_200), date(432_000)),
                (first, date(518_400), date(604_800))
            ],
            "메서드별 인자 기록"
        );
    }

    #[test]
    fn supply_capabilities_remain_explicit_closed_values() {
        let capabilities = [
            BrokerCapability::ProgramTrades,
            BrokerCapability::ShortSelling,
            BrokerCapability::CreditTrades,
            BrokerCapability::SecuritiesLending,
        ];
        let none = BrokerCapabilities::new(std::collections::BTreeSet::new());
        let all = BrokerCapabilities::new(capabilities.into_iter().collect());
        for capability in capabilities {
            contracts::broker_capabilities(&none, capability, CapabilitySupport::Unsupported);
            contracts::broker_capabilities(&all, capability, CapabilitySupport::Supported);
        }
    }
}
