#[cfg(test)]
mod tests {
    use application::{
        EventBusBuilder, HttpStatus, PortError, PortErrorKind, ReadEndpoint, ResponseMismatch,
        SystemPortError, WriteCompleted,
    };
    use approval_command::execute::SubmissionAuditStatus;
    use approval_command::ports::{
        PreSubmitCommitOutcome, PreSubmitTransactionPort, PreparedSubmissionRecord,
        SubmissionResponseRecord, SubmissionResponseStatus, SymbolFreezePort, SymbolFreezeStatus,
    };
    use approval_command::unknown::{
        UnconfirmedOrderFreeze, UnknownOrderReconciler, UnknownReconcileOutcome,
    };
    use domain::{
        AccountId, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderRow,
        BrokerOrderSet, BrokerOrderSize, BrokerOrderStatus, BrokerOrderTime, BrokerOrderType,
        BrokerTimeInForce, ClientOrderId, ClosedOrderPageCollector, ClosedOrderQueryScope,
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, ListingStatus, MasterCode,
        MasterInstrument, MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput,
        OffsetSeconds, OpenOrderCoverage, OrderPageBoundary, OrderSide, OrderTerms, Price,
        Quantity, ResolvedOrderInstrument, Symbol, TimeInterval, Timestamp, UnixSeconds,
        UnknownOrderFacts, UsOrderPrice, WholeOrderQuantity,
    };
    use std::future::Future;
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::scripted::ScriptedBrokerRead;
    use testkit_ports::system_fakes::MemoryAuditLog;

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

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("수량")).expect("수량")
    }

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("가격"), Currency::Usd).expect("가격")
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

    fn row(id: &str, ordered_at: i64) -> BrokerOrderRow {
        BrokerOrderRow {
            account_id: AccountId::new(7),
            order_id: BrokerOrderId::try_new(id).expect("주문"),
            instrument: instrument(),
            side: OrderSide::Buy,
            status: BrokerOrderStatus::Pending,
            size: BrokerOrderSize::Quantity(quantity("3")),
            filled_quantity: quantity("0"),
            average_filled_price: None,
            filled_amount: BrokerOrderMoney::Absent,
            filled_at: None,
            ordered_at: at(ordered_at),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Known(price("190")),
            order_type: BrokerOrderType::Limit,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: at(950),
        }
    }

    fn facts() -> UnknownOrderFacts {
        UnknownOrderFacts {
            instrument: instrument(),
            terms: OrderTerms::UsLimit {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(quantity("3")).expect("주문 수량"),
                price: UsOrderPrice::try_new(price("190")).expect("지정가"),
            },
            sent_at: at(200),
        }
    }

    fn window() -> TimeInterval {
        TimeInterval::try_new(at(100), at(900)).expect("구간")
    }

    fn broker(open_rows: Vec<BrokerOrderRow>) -> ScriptedBrokerRead {
        let broker = ScriptedBrokerRead::default();
        broker
            .script_open_orders()
            .push(Ok(BrokerOrderSet::from_open_lookup_result(
                AccountId::new(7),
                open_rows,
                OpenOrderCoverage::Complete,
                at(950),
            )
            .expect("미체결")));
        let mut collector = ClosedOrderPageCollector::start(ClosedOrderQueryScope::unfiltered(
            AccountId::new(7),
            window(),
        ));
        collector.record_page_boundary(OrderPageBoundary::LastPage);
        broker
            .script_closed_orders()
            .push(Ok(collector.finish(at(950)).expect("종료 주문")));
        broker
    }

    #[derive(Default)]
    struct RecordingTx(RwLock<Vec<SubmissionResponseRecord>>);

    impl PreSubmitTransactionPort for RecordingTx {
        fn commit_consumption_and_submission(
            &self,
            _record: &PreparedSubmissionRecord,
        ) -> impl Future<Output = Result<PreSubmitCommitOutcome, SystemPortError>> + Send {
            std::future::ready(Ok(PreSubmitCommitOutcome::Committed))
        }

        fn record_submission_outcome(
            &self,
            record: &SubmissionResponseRecord,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            match self.0.write() {
                Ok(mut guard) => guard.push(record.clone()),
                Err(poisoned) => poisoned.into_inner().push(record.clone()),
            }
            std::future::ready(Ok(WriteCompleted))
        }
    }

    fn audit() -> MemoryAuditLog {
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        audit
    }

    fn key() -> ClientOrderId {
        ClientOrderId::try_new("order-o19").expect("키")
    }

    #[test]
    fn single_match_resolves_and_releases_the_symbol() {
        let broker = broker(vec![row("b1", 210), row("b0", 150)]);
        let tx = RecordingTx::default();
        let audit = audit();
        let clock = SteppedClock::new(at(960));
        let events = EventBusBuilder::default().build();
        let freeze = UnconfirmedOrderFreeze::new();
        freeze.freeze(AccountId::new(7), instrument(), key());
        let outcome = run(UnknownOrderReconciler {
            broker: &broker,
            tx_port: &tx,
            audit: &audit,
            clock: &clock,
            events: &events,
            freeze: &freeze,
        }
        .reconcile(&AccountId::new(7), &key(), &facts(), window()))
        .expect("대조");
        assert_eq!(
            outcome,
            UnknownReconcileOutcome::Resolved {
                broker_order_id: BrokerOrderId::try_new("b1").expect("주문번호"),
                audit_status: SubmissionAuditStatus::Recorded,
                delivery: application::DeliveryReport::default(),
            }
        );
        assert_eq!(
            run(freeze.freeze_status(&AccountId::new(7), &instrument())),
            Ok(SymbolFreezeStatus::Unfrozen)
        );
        let recorded = match tx.0.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        assert_eq!(
            recorded
                .iter()
                .map(|record| record.status.clone())
                .collect::<Vec<_>>(),
            vec![SubmissionResponseStatus::Accepted {
                broker_order_id: BrokerOrderId::try_new("b1").expect("주문번호"),
            }]
        );
    }

    #[test]
    fn ambiguous_or_missing_match_freezes_the_same_symbol() {
        for rows in [vec![], vec![row("b1", 210), row("b2", 220)]] {
            let broker = broker(rows);
            let tx = RecordingTx::default();
            let audit = audit();
            let clock = SteppedClock::new(at(960));
            let events = EventBusBuilder::default().build();
            let freeze = UnconfirmedOrderFreeze::new();
            let outcome = run(UnknownOrderReconciler {
                broker: &broker,
                tx_port: &tx,
                audit: &audit,
                clock: &clock,
                events: &events,
                freeze: &freeze,
            }
            .reconcile(&AccountId::new(7), &key(), &facts(), window()))
            .expect("대조");
            assert_eq!(outcome, UnknownReconcileOutcome::Unconfirmed);
            assert_eq!(
                run(freeze.freeze_status(&AccountId::new(7), &instrument())),
                Ok(SymbolFreezeStatus::Frozen),
                "확인 불가면 같은 종목 새 주문 차단"
            );
        }
    }

    #[test]
    fn query_failure_freezes_and_reports_the_error() {
        let broker = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::OpenOrders,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        broker.script_open_orders().push(Err(error));
        let mut collector = ClosedOrderPageCollector::start(ClosedOrderQueryScope::unfiltered(
            AccountId::new(7),
            window(),
        ));
        collector.record_page_boundary(OrderPageBoundary::LastPage);
        broker
            .script_closed_orders()
            .push(Ok(collector.finish(at(950)).expect("종료 주문")));
        let tx = RecordingTx::default();
        let audit = audit();
        let clock = SteppedClock::new(at(960));
        let events = EventBusBuilder::default().build();
        let freeze = UnconfirmedOrderFreeze::new();
        assert_eq!(
            run(UnknownOrderReconciler {
                broker: &broker,
                tx_port: &tx,
                audit: &audit,
                clock: &clock,
                events: &events,
                freeze: &freeze,
            }
            .reconcile(&AccountId::new(7), &key(), &facts(), window())),
            Err(error)
        );
        assert_eq!(
            run(freeze.freeze_status(&AccountId::new(7), &instrument())),
            Ok(SymbolFreezeStatus::Frozen)
        );
    }
}
