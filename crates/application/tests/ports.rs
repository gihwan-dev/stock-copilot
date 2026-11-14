#[cfg(test)]
mod tests {
    use application::{BrokerCapabilities, BrokerCapability, BrokerRead, MarketInfo, PortError};
    use domain::{
        AccountDisplayFact, AccountId, BuyingPower, CalendarDate, Candle, CandlePeriod,
        CompleteTransactionResponse, Currency, FxPair, FxRate, FxRateKind, InstrumentId, Market,
        OrderFxRates, Portfolio, PriceAdjustment, Quote, ResolvedOrderInstrument, SettlementWindow,
        TimeInterval, Timestamp, TransactionFilter,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        task::{Context, Poll, Waker},
    };

    struct UnavailableBroker;

    impl BrokerRead for UnavailableBroker {
        fn accounts(&self) -> impl Future<Output = Result<Vec<AccountId>, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn account_display(
            &self,
            _account: &AccountId,
        ) -> impl Future<Output = Result<AccountDisplayFact, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn portfolio(
            &self,
            _account: &AccountId,
        ) -> impl Future<Output = Result<Portfolio, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn transactions(
            &self,
            _account: &AccountId,
            _window: SettlementWindow,
            _filter: &TransactionFilter,
        ) -> impl Future<Output = Result<CompleteTransactionResponse, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn quote(
            &self,
            _instrument: &ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<Quote, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn candles(
            &self,
            _instrument: (&ResolvedOrderInstrument, &InstrumentId),
            _window: TimeInterval,
            _period: CandlePeriod,
            _adjustment: PriceAdjustment,
        ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn buying_power(
            &self,
            _account: &AccountId,
            _currency: Currency,
        ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
    }

    impl MarketInfo for UnavailableBroker {
        fn calendar(
            &self,
            _market: &Market,
            _date: CalendarDate,
        ) -> impl Future<Output = Result<domain::MarketCalendar, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn fx_rate(
            &self,
            _pair: FxPair,
            _kind: FxRateKind,
            _at: Option<Timestamp>,
        ) -> impl Future<Output = Result<FxRate, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
        fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send {
            std::future::ready(Err(PortError::Unavailable))
        }
    }

    fn ready<F: Future + Send>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match pinned.as_mut().poll(&mut context) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
        .expect("최소 가짜는 즉시 완료한다")
    }

    fn call_every_method<P: BrokerRead + MarketInfo>(port: &P) {
        let account = AccountId::new(7);
        let instant = Timestamp::from_parts(
            domain::UnixSeconds::new(0),
            domain::Nanosecond::new(0),
            domain::OffsetSeconds::new(0),
        )
        .expect("시각");
        let date = instant.offset_calendar_date();
        let window = SettlementWindow::try_new(date, date).expect("결제 창");
        let until = instant
            .checked_add(
                domain::TimeSpan::try_new(domain::SpanSeconds::new(60), domain::Nanosecond::new(0))
                    .expect("기간"),
            )
            .expect("종료 시각");
        let interval = TimeInterval::try_new(instant, until).expect("시간 창");
        let instrument = ResolvedOrderInstrument::from_master_lookup_result(
            domain::Symbol::try_new("AAPL").expect("종목"),
            Market::Us,
        )
        .expect("조회 종목");
        let series = InstrumentId::try_new("series-aapl").expect("시계열 식별");
        let accounts: Result<Vec<AccountId>, PortError> = ready(port.accounts());
        let display: Result<AccountDisplayFact, PortError> = ready(port.account_display(&account));
        let portfolio: Result<Portfolio, PortError> = ready(port.portfolio(&account));
        let transactions: Result<CompleteTransactionResponse, PortError> =
            ready(port.transactions(&account, window, &TransactionFilter::all()));
        let quote: Result<Quote, PortError> = ready(port.quote(&instrument));
        let candles: Result<Vec<Candle>, PortError> = ready(port.candles(
            (&instrument, &series),
            interval,
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ));
        let power: Result<BuyingPower, PortError> =
            ready(port.buying_power(&account, Currency::Usd));
        let calendar: Result<domain::MarketCalendar, PortError> =
            ready(port.calendar(&Market::Us, date));
        let fx: Result<FxRate, PortError> =
            ready(port.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, None));
        let order_fx: Result<OrderFxRates, PortError> = ready(port.order_fx_rates());
        assert_eq!(
            [
                accounts.err(),
                display.err(),
                portfolio.err(),
                transactions.err(),
                quote.err(),
                candles.err(),
                power.err(),
                calendar.err(),
                fx.err(),
                order_fx.err()
            ],
            [Some(PortError::Unavailable); 10]
        );
    }

    #[test]
    fn fake_calls_every_domain_typed_method_with_send_futures() {
        call_every_method(&UnavailableBroker);
    }

    #[test]
    fn capability_set_preserves_supported_and_unsupported_features() {
        let supplied = BTreeSet::from([BrokerCapability::KrLimit, BrokerCapability::MinuteCandles]);
        let capabilities = BrokerCapabilities::new(supplied.clone());
        assert_eq!(capabilities.supported(), &supplied);
        assert_eq!(
            BrokerCapabilities::new(BTreeSet::new()).supported(),
            &BTreeSet::new()
        );
        assert_ne!(PortError::AccessDenied, PortError::Unavailable);
        assert_ne!(PortError::InvalidResponse, PortError::Unavailable);
    }
}
