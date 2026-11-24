#[cfg(test)]
mod tests {
    use application::{
        EventBus, EventBusBuilder, EventSubscriber, HttpStatus, PortError, PortErrorKind,
        ReadEndpoint, ResponseMismatch, SubscriberFailure, TrackSubmittedOrders, TrackedOrder,
        TrackedOrderUpdate,
    };
    use domain::{
        AccountId, BrokerOrder, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderRow,
        BrokerOrderSize, BrokerOrderStatus, BrokerOrderTime, BrokerOrderType, BrokerTimeInForce,
        CommonShare, Currency, DisplayInstrumentName, DomainEvent, DomainEventKind, InstrumentId,
        ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType,
        Nanosecond, NumericInput, OffsetSeconds, OrderProgress, OrderProgressChange, OrderSide,
        Quantity, Symbol, Timestamp, UnixSeconds,
    };
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
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

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("수량")).expect("수량")
    }

    fn order(id: &str, status: BrokerOrderStatus, filled: &str) -> BrokerOrder {
        let has_fill = filled != "0";
        BrokerOrder::from_lookup_result(BrokerOrderRow {
            account_id: AccountId::new(7),
            order_id: BrokerOrderId::try_new(id).expect("주문"),
            instrument: MasterInstrument::try_new(MasterInstrumentRow {
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
            .expect("주문 식별"),
            side: OrderSide::Buy,
            status,
            size: BrokerOrderSize::Quantity(quantity("3")),
            filled_quantity: quantity(filled),
            average_filled_price: None,
            filled_amount: BrokerOrderMoney::Absent,
            filled_at: has_fill.then(|| at(210)),
            ordered_at: at(200),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Absent,
            order_type: BrokerOrderType::Market,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: at(950),
        })
        .expect("주문")
    }

    struct Collector(Arc<RwLock<Vec<DomainEvent>>>);

    impl EventSubscriber for Collector {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut guard) => guard.push(event.clone()),
                Err(poisoned) => poisoned.into_inner().push(event.clone()),
            }
            Ok(())
        }
    }

    fn bus() -> (EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let seen = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            BTreeSet::from([
                DomainEventKind::OrderFilled,
                DomainEventKind::OrderCancelled,
            ]),
            Collector(Arc::clone(&seen)),
        );
        (builder.build(), seen)
    }

    fn events(seen: &Arc<RwLock<Vec<DomainEvent>>>) -> Vec<DomainEvent> {
        match seen.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn tracked(id: &str, last_seen: Option<OrderProgress>) -> TrackedOrder {
        TrackedOrder {
            account: AccountId::new(7),
            order_id: BrokerOrderId::try_new(id).expect("주문"),
            last_seen,
        }
    }

    fn progress_of(update: &TrackedOrderUpdate) -> (OrderProgress, OrderProgressChange) {
        match update {
            TrackedOrderUpdate::Refreshed { row, change, .. } => (row.progress(), *change),
            TrackedOrderUpdate::Unavailable { .. } => panic!("조회 성공이어야 함"),
        }
    }

    #[test]
    fn four_states_are_tracked_and_terminal_changes_publish_once() {
        let broker = ScriptedBrokerRead::default();
        broker
            .script_order()
            .push(Ok(order("b1", BrokerOrderStatus::Pending, "0")));
        broker
            .script_order()
            .push(Ok(order("b2", BrokerOrderStatus::PartialFilled, "1")));
        broker
            .script_order()
            .push(Ok(order("b3", BrokerOrderStatus::Filled, "3")));
        broker
            .script_order()
            .push(Ok(order("b4", BrokerOrderStatus::Canceled, "0")));
        let (events_bus, seen) = bus();
        let tracker = TrackSubmittedOrders::new(&broker, &events_bus);
        let updates = run(tracker.refresh(&[
            tracked("b1", None),
            tracked("b2", Some(OrderProgress::Unfilled)),
            tracked("b3", Some(OrderProgress::PartiallyFilled)),
            tracked("b4", Some(OrderProgress::Unfilled)),
            tracked("b5", Some(OrderProgress::Filled)),
        ]));
        let states: Vec<_> = updates.iter().map(progress_of).collect();
        assert_eq!(
            states,
            vec![
                (OrderProgress::Unfilled, OrderProgressChange::Advanced),
                (
                    OrderProgress::PartiallyFilled,
                    OrderProgressChange::Advanced
                ),
                (OrderProgress::Filled, OrderProgressChange::BecameFilled),
                (OrderProgress::Ended, OrderProgressChange::BecameEnded),
            ]
        );
        assert_eq!(
            events(&seen),
            vec![
                DomainEvent::OrderFilled(AccountId::new(7)),
                DomainEvent::OrderCancelled(AccountId::new(7)),
            ]
        );
        assert_eq!(broker.calls().len(), 4);
    }

    #[test]
    fn query_failure_keeps_last_seen_state() {
        let broker = ScriptedBrokerRead::default();
        let error = PortError::new(
            ReadEndpoint::Order,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        broker.script_order().push(Err(error));
        let (events_bus, seen) = bus();
        let updates = run(TrackSubmittedOrders::new(&broker, &events_bus)
            .refresh(&[tracked("b1", Some(OrderProgress::PartiallyFilled))]));
        assert_eq!(
            updates,
            vec![TrackedOrderUpdate::Unavailable {
                order_id: BrokerOrderId::try_new("b1").expect("주문"),
                last_seen: Some(OrderProgress::PartiallyFilled),
                error,
            }]
        );
        assert_eq!(events(&seen), vec![]);
    }
}
