#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, AccountObservation, BrokerCapabilities, BrokerCapability, BrokerRead,
        CapabilitySupport, HoldingsObservation, HttpStatus, IncompleteReason,
        IncompleteTransactions, MarketInfo, MissingData, PortError, PortErrorKind,
        QuarantinedTransaction, QuoteObservation, ReadEndpoint, RequestRejection, ResponseMismatch,
        RetryDelay, TransactionRead, UnavailableCause,
    };
    use domain::{
        AccountDisplayFact, AccountId, AccountLookupDisplayRow, AcquisitionPrice, BuyingPower,
        Cancellation, Candle, CandlePeriod, CompleteTransactionResponse, CompletionAttestation,
        Currency, CurrencyAmounts, FxPair, FxRate, FxRateKind, FxSource, Holding,
        HoldingObservation, Market, MaskedAccountNumber, Money, Nanosecond, NumericInput,
        OffsetSeconds, OrderFxRates, PriceAdjustment, Quantity, Quote, ReportedPortfolio,
        ResolvedOrderInstrument, Settlement, SettlementWindow, SpanSeconds, Symbol, TimeInterval,
        TimeSpan, Timestamp, Transaction, TransactionBatch, TransactionData, TransactionFilter,
        TransactionKind, UnixSeconds, UntrustedText,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn instant() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn interval() -> TimeInterval {
        TimeInterval::try_new(
            instant(),
            instant()
                .checked_add(
                    TimeSpan::try_new(SpanSeconds::new(60), Nanosecond::new(0)).expect("기간"),
                )
                .expect("종료"),
        )
        .expect("시간 창")
    }
    fn money(currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new("0").expect("숫자"), currency).expect("금액")
    }
    fn instrument() -> ResolvedOrderInstrument {
        master_instrument(
            Symbol::try_new("AAPL").expect("종목"),
            domain::InstrumentId::try_new("fixture-aapl").expect("고정 자료 키"),
        )
    }
    fn master_instrument(
        symbol: Symbol,
        instrument_id: domain::InstrumentId,
    ) -> ResolvedOrderInstrument {
        domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id,
            symbol,
            name: domain::DisplayInstrumentName::try_new("고정 자료 종목").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: instant(),
        })
        .expect("마스터")
        .order_instrument()
        .expect("종목 사실")
    }

    fn fx(kind: FxRateKind) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("1375").expect("환율"),
            FxPair::UsdKrw,
            kind,
            interval(),
            FxSource::TossOpenApi,
        )
        .expect("환율 사실")
    }
    fn failure(endpoint: ReadEndpoint) -> PortError {
        PortError::new(
            endpoint,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        )
    }
    fn account() -> AccountObservation {
        let key = AccountId::new(7);
        let display = AccountDisplayFact::from_account_lookup_result(
            AccountLookupDisplayRow::from_broker_account_result(
                key.clone(),
                MaskedAccountNumber::try_new("12345678").expect("가짜 계좌"),
            ),
        );
        AccountObservation::from_broker_result(key, display)
    }
    fn holdings() -> HoldingsObservation {
        let held = Holding::new(
            instrument(),
            Quantity::parse(&NumericInput::try_new("1").expect("수량")).expect("수량"),
            AcquisitionPrice::try_new(money(Currency::Usd)).expect("평단"),
            HoldingObservation::new(None, None, instant()),
        )
        .expect("보유 관측");
        let amounts = CurrencyAmounts::new(money(Currency::Krw), Some(money(Currency::Usd)))
            .expect("통화별 금액");
        HoldingsObservation::from_broker_result(
            vec![held],
            ReportedPortfolio::new(amounts, amounts, amounts, instant()),
        )
    }
    fn received() -> TransactionBatch {
        TransactionBatch::try_new(vec![
            Transaction::try_new(TransactionData {
                kind: TransactionKind::Deposit,
                market: Market::Us,
                currency: Currency::Usd,
                symbol: None,
                asset_class: None,
                isin: None,
                quantity: None,
                filled_price: None,
                amount: Some(money(Currency::Usd)),
                commission: money(Currency::Usd),
                tax: money(Currency::Usd),
                net_amount: Some(money(Currency::Usd)),
                exchange_rate: None,
                settlement_date: instant().offset_calendar_date(),
                settlement: Settlement::Settled,
                transaction_time: Some(instant()),
                cancellation: Cancellation::Regular,
            })
            .expect("거래 행"),
        ])
        .expect("거래 묶음")
    }
    fn quarantined() -> Vec<QuarantinedTransaction> {
        vec![QuarantinedTransaction::UnknownCurrency(vec![
            UntrustedText::try_new("{\"currency\":\"EUR\",").expect("원문 앞부분"),
            UntrustedText::try_new("\"memo\":\"가\u{200b}\"}").expect("원문 뒷부분"),
        ])]
    }
    #[derive(Clone, Copy)]
    enum Scenario {
        Complete,
        Incomplete(IncompleteReason),
        Failed,
    }
    struct FakeBroker(Scenario);
    fn outcome<T>(scenario: Scenario, value: T, endpoint: ReadEndpoint) -> Result<T, PortError> {
        match scenario {
            Scenario::Failed => Err(failure(endpoint)),
            Scenario::Complete | Scenario::Incomplete(_) => Ok(value),
        }
    }
    fn quoted(value: &ResolvedOrderInstrument) -> QuoteObservation {
        let result = if value == &instrument() {
            Ok(
                Quote::try_new(value.symbol().clone(), Currency::Usd, None, None, instant())
                    .expect("현재가 관측"),
            )
        } else {
            Err(PortError::new(
                ReadEndpoint::Quotes,
                HttpStatus::NotFound,
                PortErrorKind::NotFound(MissingData::Stock),
            ))
        };
        QuoteObservation {
            instrument: value.clone(),
            result,
        }
    }
    impl BrokerRead for FakeBroker {
        fn accounts(
            &self,
        ) -> impl Future<Output = Result<Vec<AccountObservation>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![account()], ReadEndpoint::Accounts))
        }
        fn holdings(
            &self,
            _account: &AccountId,
        ) -> impl Future<Output = Result<HoldingsObservation, PortError>> + Send {
            std::future::ready(outcome(self.0, holdings(), ReadEndpoint::Holdings))
        }
        fn transactions(
            &self,
            account: &AccountId,
            window: SettlementWindow,
            filter: &TransactionFilter,
        ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send {
            let response = match self.0 {
                Scenario::Complete => Ok(TransactionRead::Complete(
                    CompleteTransactionResponse::after_last_page_without_quarantine(
                        account.clone(),
                        window,
                        filter.clone(),
                        received(),
                        CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                    )
                    .expect("완전 응답"),
                )),
                Scenario::Incomplete(reason) => {
                    Ok(TransactionRead::Incomplete(IncompleteTransactions {
                        account: account.clone(),
                        window,
                        filter: filter.clone(),
                        received: vec![received()],
                        quarantined: quarantined(),
                        reason,
                    }))
                }
                Scenario::Failed => Err(failure(ReadEndpoint::Transactions)),
            };
            std::future::ready(response)
        }
        fn quotes(
            &self,
            instruments: &[ResolvedOrderInstrument],
        ) -> impl Future<Output = Result<Vec<QuoteObservation>, PortError>> + Send {
            let result = if instruments.is_empty() || instruments.len() > 200 {
                Err(PortError::new(
                    ReadEndpoint::Quotes,
                    HttpStatus::BadRequest,
                    PortErrorKind::InvalidRequest(RequestRejection::BatchLimitExceeded),
                ))
            } else {
                outcome(
                    self.0,
                    instruments.iter().map(quoted).collect(),
                    ReadEndpoint::Quotes,
                )
            };
            std::future::ready(result)
        }
        fn candles(
            &self,
            _instrument: &ResolvedOrderInstrument,
            _window: TimeInterval,
            _period: CandlePeriod,
            _adjustment: PriceAdjustment,
        ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send {
            std::future::ready(outcome(self.0, Vec::new(), ReadEndpoint::Candles))
        }
        fn buying_power(
            &self,
            _account: &AccountId,
            currency: Currency,
        ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                BuyingPower::new(money(currency), instant()),
                ReadEndpoint::BuyingPower,
            ))
        }
    }
    impl MarketInfo for FakeBroker {
        fn fx_rate(
            &self,
            _pair: FxPair,
            kind: FxRateKind,
            _at: Option<Timestamp>,
        ) -> impl Future<Output = Result<FxRate, PortError>> + Send {
            std::future::ready(outcome(self.0, fx(kind), ReadEndpoint::FxRate))
        }
        fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                OrderFxRates::from_broker_fx_result(fx(FxRateKind::Buy), fx(FxRateKind::Mid)),
                ReadEndpoint::OrderFxRates,
            ))
        }
    }
    fn ready<F: Future + Send>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match pinned.as_mut().poll(&mut context) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
        .expect("즉시 완료 가짜")
    }
    fn window() -> SettlementWindow {
        SettlementWindow::try_new(
            instant().offset_calendar_date(),
            instant().offset_calendar_date(),
        )
        .expect("조회 창")
    }

    #[test]
    fn successful_facts_survive_a_separate_cash_failure() {
        let port = FakeBroker(Scenario::Complete);
        let accounts: Vec<AccountObservation> = ready(port.accounts()).expect("계좌");
        let first = accounts.first().expect("첫 계좌");
        assert_eq!(first.account(), &AccountId::new(7));
        assert_eq!(first.display(), account().display());
        let snapshot: HoldingsObservation = ready(port.holdings(first.account())).expect("보유");
        let broken_cash: Result<BuyingPower, PortError> =
            ready(FakeBroker(Scenario::Failed).buying_power(first.account(), Currency::Usd));
        assert_eq!(broken_cash.err(), Some(failure(ReadEndpoint::BuyingPower)));
        assert_eq!(
            snapshot.holdings().first().expect("보유 종목").instrument(),
            &instrument()
        );
        assert_eq!(snapshot.reported().observed_at(), instant());
        let cash: BuyingPower =
            ready(port.buying_power(first.account(), Currency::Krw)).expect("현금 관측");
        assert_eq!(cash.money(), Ok(money(Currency::Krw)));
    }
    fn call_market_and_history<P: BrokerRead + MarketInfo + Send + Sync>(port: &P) {
        let rows: TransactionRead =
            ready(port.transactions(&AccountId::new(7), window(), &TransactionFilter::all()))
                .expect("거래 조회");
        let complete = match rows {
            TransactionRead::Complete(value) => Some(value),
            TransactionRead::Incomplete(_) => None,
        }
        .expect("완전 응답 예상");
        assert_eq!(complete.rows(), &received());
        let quotes: Vec<QuoteObservation> = ready(port.quotes(&[instrument()])).expect("현재가");
        assert_eq!(
            quotes
                .first()
                .expect("첫 종목")
                .result
                .as_ref()
                .expect("시세")
                .symbol(),
            instrument().symbol()
        );
        let candles: Vec<Candle> = ready(port.candles(
            &instrument(),
            interval(),
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect("빈 봉 관측");
        assert!(candles.is_empty(), "빈 응답을 보존한다");
        let rate: FxRate = ready(port.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, Some(instant())))
            .expect("과거 환율");
        assert_eq!(rate, fx(FxRateKind::Mid));
        let order_fx: OrderFxRates = ready(port.order_fx_rates()).expect("환율 쌍");
        assert_eq!(
            order_fx,
            OrderFxRates::from_broker_fx_result(fx(FxRateKind::Buy), fx(FxRateKind::Mid))
        );
    }
    #[test]
    fn every_method_has_a_domain_typed_success_and_send_future() {
        call_market_and_history(&FakeBroker(Scenario::Complete));
    }
    #[test]
    fn incomplete_variants_keep_received_rows_and_exact_untrusted_fragments() {
        for reason in [
            IncompleteReason::QuarantinedRows,
            IncompleteReason::RowLimitExceeded,
            IncompleteReason::PageInterrupted,
        ] {
            let response = ready(FakeBroker(Scenario::Incomplete(reason)).transactions(
                &AccountId::new(7),
                window(),
                &TransactionFilter::all(),
            ))
            .expect("불완전 결과");
            let partial = match response {
                TransactionRead::Incomplete(value) => Some(value),
                TransactionRead::Complete(_) => None,
            }
            .expect("불완전 응답 예상");
            assert_eq!(partial.account, AccountId::new(7));
            assert_eq!(partial.window, window());
            assert_eq!(partial.filter, TransactionFilter::all());
            assert_eq!(partial.reason, reason);
            assert_eq!(partial.received, vec![received()]);
            let QuarantinedTransaction::UnknownCurrency(fragments) =
                partial.quarantined.first().expect("격리 행");
            assert_eq!(
                fragments
                    .iter()
                    .map(UntrustedText::as_untrusted_str)
                    .collect::<String>(),
                "{\"currency\":\"EUR\",\"memo\":\"가\u{200b}\"}"
            );
        }
    }
    #[test]
    fn quote_batch_has_individual_results_and_caller_splits_above_two_hundred() {
        let port = FakeBroker(Scenario::Complete);
        let missing = master_instrument(
            Symbol::try_new("MSFT").expect("종목"),
            domain::InstrumentId::try_new("fixture-msft").expect("고정 자료 키"),
        );
        let batch = ready(port.quotes(&[instrument(), missing.clone()])).expect("종목별 결과");
        assert_eq!(
            batch
                .first()
                .expect("첫 종목")
                .result
                .as_ref()
                .expect("성공")
                .symbol(),
            instrument().symbol()
        );
        assert_eq!(batch.last().expect("끝 종목").instrument, missing);
        assert_eq!(
            batch.last().expect("끝 종목").result.as_ref().err(),
            Some(&PortError::new(
                ReadEndpoint::Quotes,
                HttpStatus::NotFound,
                PortErrorKind::NotFound(MissingData::Stock)
            ))
        );
        assert_eq!(
            ready(port.quotes(&vec![instrument(); 200]))
                .expect("상한")
                .len(),
            200
        );
        let expected = PortError::new(
            ReadEndpoint::Quotes,
            HttpStatus::BadRequest,
            PortErrorKind::InvalidRequest(RequestRejection::BatchLimitExceeded),
        );
        assert_eq!(ready(port.quotes(&vec![instrument(); 201])), Err(expected));
        assert_eq!(ready(port.quotes(&[])), Err(expected));
    }
    #[test]
    fn failures_before_any_data_remain_typed_errors() {
        let port = FakeBroker(Scenario::Failed);
        assert_eq!(
            ready(port.accounts()).err(),
            Some(failure(ReadEndpoint::Accounts))
        );
        assert_eq!(
            ready(port.holdings(&AccountId::new(7))).err(),
            Some(failure(ReadEndpoint::Holdings))
        );
        assert_eq!(
            ready(port.transactions(&AccountId::new(7), window(), &TransactionFilter::all())).err(),
            Some(failure(ReadEndpoint::Transactions))
        );
        assert_eq!(
            ready(port.quotes(&[instrument()])).err(),
            Some(failure(ReadEndpoint::Quotes))
        );
        assert_eq!(
            ready(port.candles(
                &instrument(),
                interval(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw
            ))
            .err(),
            Some(failure(ReadEndpoint::Candles))
        );
        assert_eq!(
            ready(port.buying_power(&AccountId::new(7), Currency::Usd)).err(),
            Some(failure(ReadEndpoint::BuyingPower))
        );
        assert_eq!(
            ready(port.fx_rate(FxPair::UsdKrw, FxRateKind::Mid, None)).err(),
            Some(failure(ReadEndpoint::FxRate))
        );
        assert_eq!(
            ready(port.order_fx_rates()).err(),
            Some(failure(ReadEndpoint::OrderFxRates))
        );
    }
    #[test]
    fn read_capabilities_return_closed_support_decisions() {
        let capabilities = BrokerCapabilities::new(BTreeSet::from([
            BrokerCapability::DailyCandles,
            BrokerCapability::RawHistory,
        ]));
        assert_eq!(
            capabilities.support_for(BrokerCapability::DailyCandles),
            CapabilitySupport::Supported
        );
        assert_eq!(
            capabilities.support_for(BrokerCapability::RawHistory),
            CapabilitySupport::Supported
        );
        assert_eq!(
            capabilities.support_for(BrokerCapability::MinuteCandles),
            CapabilitySupport::Unsupported
        );
        assert_eq!(
            capabilities.support_for(BrokerCapability::AdjustedHistory),
            CapabilitySupport::Unsupported
        );
        assert_eq!(
            BrokerCapabilities::new(BTreeSet::new()).support_for(BrokerCapability::DailyCandles),
            CapabilitySupport::Unsupported
        );
    }
    #[test]
    fn errors_keep_closed_causes_endpoint_status_and_retry_without_bodies() {
        let delay = TimeSpan::try_new(SpanSeconds::new(60), Nanosecond::new(0)).expect("대기");
        for kind in [
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
            PortErrorKind::AccessDenied(AccessDenial::IpNotAllowed),
            PortErrorKind::AccessDenied(AccessDenial::TokenRevokedRepeated),
            PortErrorKind::AccessDenied(AccessDenial::AccountNotFound),
            PortErrorKind::AccessDenied(AccessDenial::Forbidden),
            PortErrorKind::AccessDenied(AccessDenial::LoginUserNotFound),
            PortErrorKind::Unavailable {
                cause: UnavailableCause::RateLimited,
                retry: RetryDelay::Known(delay),
            },
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Maintenance,
                retry: RetryDelay::Unknown,
            },
            PortErrorKind::Unavailable {
                cause: UnavailableCause::ServerError,
                retry: RetryDelay::Unknown,
            },
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
            PortErrorKind::Unavailable {
                cause: UnavailableCause::ExpiredToken,
                retry: RetryDelay::Unknown,
            },
            PortErrorKind::NotFound(MissingData::Stock),
            PortErrorKind::NotFound(MissingData::ExchangeRate),
            PortErrorKind::NotFound(MissingData::Sector),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            PortErrorKind::Unsupported(BrokerCapability::MinuteCandles),
            PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
        ] {
            let error = PortError::new(ReadEndpoint::Quotes, HttpStatus::BadRequest, kind);
            assert_eq!(error.endpoint(), ReadEndpoint::Quotes);
            assert_eq!(error.status(), HttpStatus::BadRequest);
            assert_eq!(error.kind(), kind);
        }
    }
}
