#[cfg(test)]
mod tests {
    use application::{
        HttpStatus, PortError, PortErrorKind, ReadEndpoint, ReconcileOrdersError,
        ReconcileSubmittedOrders, ResponseMismatch, SubmissionLedger, SystemPortError,
    };
    use domain::{
        AccountId, BrokerOrderCoverage, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice,
        BrokerOrderRow, BrokerOrderSet, BrokerOrderSize, BrokerOrderStatus, BrokerOrderTime,
        BrokerOrderType, BrokerTimeInForce, ClientOrderId, ClosedOrderPageCollector,
        ClosedOrderQueryScope, CommonShare, Currency, DisplayInstrumentName, InstrumentId,
        ListingStatus, LocalSubmission, LocalSubmissionResult, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput, OffsetSeconds,
        OpenOrderCoverage, OrderPageBoundary, OrderSide, OrderTerms, Quantity, ReconciliationIssue,
        ReconciliationStatus, ResolvedOrderInstrument, Symbol, TimeInterval, Timestamp,
        UnixSeconds, WholeOrderQuantity,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::scripted::ScriptedBrokerRead;

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
        TimeInterval::try_new(at(100), at(900)).expect("구간")
    }

    fn instrument() -> ResolvedOrderInstrument {
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
        .order_instrument()
        .expect("주문 식별")
    }

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("수량")).expect("수량")
    }

    fn row(id: &str, status: BrokerOrderStatus) -> BrokerOrderRow {
        let filled = status == BrokerOrderStatus::Filled;
        BrokerOrderRow {
            account_id: AccountId::new(7),
            order_id: BrokerOrderId::try_new(id).expect("주문"),
            instrument: instrument(),
            side: OrderSide::Buy,
            status,
            size: BrokerOrderSize::Quantity(quantity("3")),
            filled_quantity: quantity(if filled { "3" } else { "0" }),
            average_filled_price: None,
            filled_amount: BrokerOrderMoney::Absent,
            filled_at: filled.then(|| at(210)),
            ordered_at: at(200),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Absent,
            order_type: BrokerOrderType::Market,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: at(950),
        }
    }

    fn terms() -> OrderTerms {
        OrderTerms::UsMarketBuy {
            quantity: WholeOrderQuantity::try_new(quantity("3")).expect("주문 수량"),
        }
    }

    fn submission(client: &str, result: LocalSubmissionResult) -> LocalSubmission {
        LocalSubmission {
            client_order_id: ClientOrderId::try_new(client).expect("키"),
            instrument: instrument(),
            terms: terms(),
            prepared_at: at(190),
            result,
        }
    }

    fn accepted(id: &str) -> LocalSubmissionResult {
        LocalSubmissionResult::Accepted(BrokerOrderId::try_new(id).expect("주문번호"))
    }

    struct FixedLedger(Vec<LocalSubmission>);

    impl SubmissionLedger for FixedLedger {
        fn submissions_between(
            &self,
            _window: TimeInterval,
        ) -> impl Future<Output = Result<Vec<LocalSubmission>, SystemPortError>> + Send {
            std::future::ready(Ok(self.0.clone()))
        }
    }

    fn broker(closed: BrokerOrderCoverage) -> ScriptedBrokerRead {
        let broker = ScriptedBrokerRead::default();
        broker
            .script_open_orders()
            .push(Ok(BrokerOrderSet::from_open_lookup_result(
                AccountId::new(7),
                vec![row("b2", BrokerOrderStatus::Pending)],
                OpenOrderCoverage::Complete,
                at(950),
            )
            .expect("미체결")));
        let mut collector = ClosedOrderPageCollector::start(ClosedOrderQueryScope::unfiltered(
            AccountId::new(7),
            window(),
        ));
        let _intake = collector.accept_row(row("b1", BrokerOrderStatus::Filled));
        if closed == BrokerOrderCoverage::Complete {
            collector.record_page_boundary(OrderPageBoundary::LastPage);
        } else {
            collector.record_page_limit_reached();
        }
        broker
            .script_closed_orders()
            .push(Ok(collector.finish(at(950)).expect("종료 주문")));
        broker
    }

    fn ledger() -> FixedLedger {
        FixedLedger(vec![
            submission("c1", accepted("b1")),
            submission("c2", accepted("b2")),
            submission("c3", accepted("b3")),
            submission("c4", LocalSubmissionResult::Unknown),
            submission("c5", LocalSubmissionResult::Rejected),
        ])
    }

    #[test]
    fn complete_broker_history_reports_only_the_missing_accepted_order() {
        let ledger = ledger();
        let broker = broker(BrokerOrderCoverage::Complete);
        let clock = SteppedClock::new(at(960));
        let report = run(ReconcileSubmittedOrders::new(&ledger, &broker, &clock)
            .execute(&AccountId::new(7), window()))
        .expect("대조");
        assert_eq!(report.status(), ReconciliationStatus::Mismatched);
        assert_eq!(
            report.issues(),
            &[ReconciliationIssue::AcceptedButMissing {
                client_order_id: ClientOrderId::try_new("c3").expect("키"),
                broker_order_id: BrokerOrderId::try_new("b3").expect("주문번호"),
            }]
        );
        assert_eq!(report.checked_at(), at(960));
    }

    #[test]
    fn incomplete_broker_history_never_claims_missing_orders() {
        let ledger = ledger();
        let broker = broker(BrokerOrderCoverage::Incomplete);
        let clock = SteppedClock::new(at(960));
        let report = run(ReconcileSubmittedOrders::new(&ledger, &broker, &clock)
            .execute(&AccountId::new(7), window()))
        .expect("대조");
        assert_eq!(report.status(), ReconciliationStatus::Incomplete);
        assert_eq!(report.issues(), &[]);
    }

    #[test]
    fn broker_failure_is_reported_as_error() {
        let ledger = ledger();
        let broker = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::OpenOrders,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        broker.script_open_orders().push(Err(error));
        let clock = SteppedClock::new(at(960));
        assert_eq!(
            run(ReconcileSubmittedOrders::new(&ledger, &broker, &clock)
                .execute(&AccountId::new(7), window())),
            Err(ReconcileOrdersError::Broker(error))
        );
    }
}
