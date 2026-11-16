#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapabilities, BrokerCapability, BrokerRead, CapabilitySupport,
        HoldingsObservation, HttpStatus, IncompleteReason, IncompleteTransactions, MarketInfo,
        MissingData, PortError, PortErrorKind, QuarantinedTransaction, QuoteObservation,
        ReadEndpoint, RequestRejection, ResponseMismatch, RetryDelay, TransactionRead,
        UnavailableCause,
    };
    use domain::{
        AccountId, AcquisitionPrice, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BrokerOrder, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice,
        BrokerOrderRow, BrokerOrderSet, BrokerOrderSize, BrokerOrderStatus, BrokerOrderTime,
        BrokerOrderType, BrokerTimeInForce, BuyingPower, CalendarDate, Cancellation, Candle,
        CandlePeriod, ClosedOrderCompleteness, ClosedOrderIncomplete, ClosedOrderSet,
        CommissionRateFact, CommissionValidity, CommonShare, CompleteTransactionResponse,
        CompletionAttestation, CostRate, Currency, CurrencyAmounts, DisplayInstrumentName,
        FlowInterval, FxPair, FxRate, FxRateKind, FxSource, Holding, HoldingObservation,
        IndicatorCode, IndicatorUnit, InstrumentId, InvestorFlow, KnownMarket, ListingStatus,
        Market, MarketCalendar, MarketIndicatorCandle, MarketIndicatorQuote, MarketSession,
        MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, Money, Nanosecond, NumericInput, OffsetSeconds, OpenOrderCoverage,
        OrderFxRates, OrderSide, Orderbook, OrderbookInput, PriceAdjustment, PriceLimitRange,
        PriceLimits, PriceLimitsInput, Quantity, Quote, QuoteInstrument, RankingCautionFilter,
        RankingDuration, RankingKind, RankingLimit, RankingResponseInput, Ratio, ReportedPortfolio,
        ResolvedOrderInstrument, SectorComposition, SectorCompositionScope, SectorId,
        SellableQuantityFact, SessionInformation, Settlement, SettlementWindow, SpanSeconds,
        StockRanking, StockWarning, StockWarnings, Symbol, TimeInterval, TimeSpan, Timestamp,
        Transaction, TransactionBatch, TransactionData, TransactionFilter, TransactionKind,
        UnixSeconds, UntrustedText,
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
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: InstrumentId::try_new("fixture-aapl").expect("ID"),
            symbol: Symbol::try_new("AAPL").expect("종목"),
            name: DisplayInstrumentName::try_new("고정 자료 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: instant(),
        })
        .expect("마스터")
        .order_instrument()
        .expect("주문 식별")
    }
    fn master_instrument(symbol: Symbol, instrument_id: InstrumentId) -> QuoteInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id,
            symbol,
            name: DisplayInstrumentName::try_new("고정 자료 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: instant(),
        })
        .expect("마스터")
        .quote_instrument()
        .expect("종목 사실")
    }

    fn quote_instrument() -> QuoteInstrument {
        master_instrument(
            Symbol::try_new("AAPL").expect("종목"),
            InstrumentId::try_new("fixture-aapl").expect("ID"),
        )
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
    fn account() -> BrokerAccountRow {
        let key = AccountId::new(7);
        BrokerAccountRow::from_broker_account_result(
            key,
            BrokerAccountTypeCode::new("BROKERAGE"),
            BrokerAccountNumberInput::new("12345678".to_owned()),
            instant(),
        )
        .expect("가짜 계좌")
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
    fn quoted(value: &QuoteInstrument) -> QuoteObservation {
        let result = if value == &quote_instrument() {
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
        fn accounts(&self) -> impl Future<Output = Result<BrokerAccountRows, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                BrokerAccountRows::try_new(vec![account()]).expect("계좌 행"),
                ReadEndpoint::Accounts,
            ))
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
            instruments: &[QuoteInstrument],
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
            _instrument: &QuoteInstrument,
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
        fn sellable_quantity(
            &self,
            account: &AccountId,
            instrument: &ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<SellableQuantityFact, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                SellableQuantityFact::from_lookup_result(
                    account.clone(),
                    instrument.clone(),
                    Quantity::parse(&NumericInput::try_new("17").expect("수량")).expect("수량"),
                    instant(),
                )
                .expect("매도 가능 사실"),
                ReadEndpoint::SellableQuantity,
            ))
        }
        fn open_orders(
            &self,
            account: &AccountId,
        ) -> impl Future<Output = Result<BrokerOrderSet, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                BrokerOrderSet::from_open_lookup_result(
                    account.clone(),
                    vec![],
                    OpenOrderCoverage::Incomplete,
                    instant(),
                )
                .expect("미체결 집합"),
                ReadEndpoint::OpenOrders,
            ))
        }
        fn closed_orders(
            &self,
            account: &AccountId,
            _evaluation_window: TimeInterval,
        ) -> impl Future<Output = Result<ClosedOrderSet, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                ClosedOrderSet::new(
                    account.clone(),
                    vec![],
                    interval(),
                    a20_observed(),
                    ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages),
                )
                .expect("종료 집합"),
                ReadEndpoint::ClosedOrders,
            ))
        }
        fn order(
            &self,
            account: &AccountId,
            order_id: &BrokerOrderId,
        ) -> impl Future<Output = Result<BrokerOrder, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                a20_order(account, order_id),
                ReadEndpoint::Order,
            ))
        }
        fn stock_warnings(
            &self,
            instrument: &ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<StockWarnings, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                StockWarnings::from_lookup_result(
                    instrument.clone(),
                    vec![StockWarning::InvestmentRisk],
                    instant(),
                ),
                ReadEndpoint::StockWarnings,
            ))
        }
        fn instruments(
            &self,
            market: KnownMarket,
        ) -> impl Future<Output = Result<MasterInstrumentFacts, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                MasterInstrumentFacts::try_new(vec![a20_master(market)]).expect("마스터 목록"),
                ReadEndpoint::Instruments,
            ))
        }
        fn orderbook(
            &self,
            instrument: &QuoteInstrument,
        ) -> impl Future<Output = Result<Orderbook, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                Orderbook::try_new(OrderbookInput {
                    instrument: instrument.clone(),
                    currency: instrument.currency(),
                    asks: vec![],
                    bids: vec![],
                    timestamp: None,
                    collected_at: instant(),
                })
                .expect("호가 없음"),
                ReadEndpoint::Orderbook,
            ))
        }
        fn price_limits(
            &self,
            instrument: &QuoteInstrument,
        ) -> impl Future<Output = Result<PriceLimits, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                PriceLimits::try_new(PriceLimitsInput {
                    instrument: instrument.clone(),
                    currency: Currency::Usd,
                    lower: None,
                    upper: None,
                    timestamp: instant(),
                    collected_at: instant(),
                })
                .expect("미국 제한 없음"),
                ReadEndpoint::PriceLimits,
            ))
        }
        fn commission_rate(
            &self,
            _account: &AccountId,
            market: KnownMarket,
        ) -> impl Future<Output = Result<CommissionRateFact, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                CommissionRateFact::from_commissions_result(
                    market.into(),
                    CostRate::try_new(
                        Ratio::parse(&NumericInput::try_new("0.0017").expect("비율"))
                            .expect("비율"),
                    )
                    .expect("수수료율"),
                    CommissionValidity::try_new(None, None).expect("기간"),
                ),
                ReadEndpoint::CommissionRate,
            ))
        }
    }
    impl MarketInfo for FakeBroker {
        fn program_trades(
            &self,
            _instrument: &QuoteInstrument,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<Vec<domain::ProgramTrade>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::ProgramTrades))
        }
        fn short_selling(
            &self,
            _instrument: &QuoteInstrument,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<Vec<domain::ShortSelling>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::ShortSelling))
        }
        fn credit_trades(
            &self,
            _instrument: &QuoteInstrument,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<Vec<domain::CreditTrade>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::CreditTrades))
        }
        fn securities_lending(
            &self,
            _instrument: &QuoteInstrument,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<Vec<domain::SecuritiesLending>, PortError>> + Send
        {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::SecuritiesLending))
        }

        fn investor_trading_rankings(
            &self,
            limit: RankingLimit,
        ) -> impl Future<Output = Result<domain::InvestorTradingRankings, PortError>> + Send
        {
            let group = || domain::InvestorGroupRow {
                net_buy: vec![],
                net_sell: vec![],
                ranked_at: None,
            };
            std::future::ready(outcome(
                self.0,
                domain::InvestorTradingRankings::from_response(
                    group(),
                    group(),
                    group(),
                    limit,
                    instant(),
                )
                .expect("투자자 랭킹"),
                ReadEndpoint::InvestorTradingRankings,
            ))
        }
        fn sectors(&self) -> impl Future<Output = Result<domain::SectorCatalog, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                domain::SectorCatalog::from_response(vec![], instant()).expect("목록"),
                ReadEndpoint::Sectors,
            ))
        }
        fn sector(
            &self,
            sector: &SectorId,
            market: KnownMarket,
        ) -> impl Future<Output = Result<domain::SectorDetail, PortError>> + Send {
            let rates = [
                RankingDuration::Day,
                RankingDuration::Week,
                RankingDuration::Month,
                RankingDuration::ThreeMonths,
                RankingDuration::SixMonths,
                RankingDuration::Year,
            ]
            .into_iter()
            .map(|duration| domain::SectorRateRow {
                duration,
                change_rate: None,
            })
            .collect();
            std::future::ready(outcome(
                self.0,
                domain::SectorDetail::from_response(
                    domain::SectorMetadataRow {
                        id: sector.clone(),
                        name: DisplayInstrumentName::try_new("분류").expect("이름"),
                        depth: domain::SectorDepth::Major,
                        parent: None,
                    },
                    market,
                    None,
                    rates,
                    instant(),
                )
                .expect("상세"),
                ReadEndpoint::Sector,
            ))
        }
        fn sector_rankings(
            &self,
            query: &application::SectorRankingQuery,
        ) -> impl Future<Output = Result<domain::SectorRanking, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                domain::SectorRanking::from_response(
                    query.kind(),
                    query.market(),
                    query.duration(),
                    vec![],
                    instant(),
                )
                .expect("섹터 랭킹"),
                ReadEndpoint::SectorRankings,
            ))
        }
        fn sector_etfs(
            &self,
            sector: &SectorId,
            market: KnownMarket,
        ) -> impl Future<Output = Result<domain::SectorEtfs, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                domain::SectorEtfs::from_response(sector.clone(), market, vec![], instant())
                    .expect("ETF 목록"),
                ReadEndpoint::SectorEtfs,
            ))
        }
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
        fn indicator_quotes(
            &self,
            codes: &[IndicatorCode],
        ) -> impl Future<Output = Result<Vec<MarketIndicatorQuote>, PortError>> + Send {
            let rows = codes
                .iter()
                .copied()
                .map(a20_indicator_quote)
                .collect::<Result<Vec<_>, _>>();
            std::future::ready(
                rows.and_then(|value| outcome(self.0, value, ReadEndpoint::IndicatorQuotes)),
            )
        }
        fn indicator_candles(
            &self,
            _code: IndicatorCode,
            _window: TimeInterval,
            _period: CandlePeriod,
        ) -> impl Future<Output = Result<Vec<MarketIndicatorCandle>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::IndicatorCandles))
        }
        fn rankings(
            &self,
            query: &application::RankingQuery,
        ) -> impl Future<Output = Result<StockRanking, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                StockRanking::from_response(RankingResponseInput {
                    kind: a20_ranking_kind(query.kind()),
                    market: query.market(),
                    duration: a20_ranking_duration(query.duration()),
                    limit: query.limit(),
                    rows: vec![],
                    ranked_at: None,
                    collected: instant(),
                    caution_filter: query.caution_filter(),
                })
                .expect("랭킹"),
                ReadEndpoint::Rankings,
            ))
        }
        fn stock_investor_flow(
            &self,
            _instrument: &QuoteInstrument,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::StockInvestorFlow))
        }
        fn indicator_investor_flow(
            &self,
            _code: IndicatorCode,
            _from: CalendarDate,
            _through: CalendarDate,
            _interval: FlowInterval,
        ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
            std::future::ready(outcome(self.0, vec![], ReadEndpoint::IndicatorInvestorFlow))
        }
        fn sector_composition(
            &self,
            sector: &SectorId,
            market: KnownMarket,
        ) -> impl Future<Output = Result<SectorComposition, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                SectorComposition::from_stocks_response(sector.clone(), market, vec![], instant())
                    .expect("섹터 구성"),
                ReadEndpoint::SectorComposition,
            ))
        }
        fn calendar(
            &self,
            market: KnownMarket,
        ) -> impl Future<Output = Result<MarketCalendar, PortError>> + Send {
            std::future::ready(outcome(
                self.0,
                a20_calendar(market),
                ReadEndpoint::Calendar,
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
        let accounts: BrokerAccountRows = ready(port.accounts()).expect("계좌");
        let first = accounts.rows().first().expect("첫 계좌");
        assert_eq!(first.account(), &AccountId::new(7));
        assert_eq!(first.display_fact(), account().display_fact());
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
        let quotes: Vec<QuoteObservation> =
            ready(port.quotes(&[quote_instrument()])).expect("현재가");
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
            &quote_instrument(),
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
            InstrumentId::try_new("fixture-msft").expect("고정 자료 키"),
        );
        let batch =
            ready(port.quotes(&[quote_instrument(), missing.clone()])).expect("종목별 결과");
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
            ready(port.quotes(&vec![quote_instrument(); 200]))
                .expect("상한")
                .len(),
            200
        );
        let expected = PortError::new(
            ReadEndpoint::Quotes,
            HttpStatus::BadRequest,
            PortErrorKind::InvalidRequest(RequestRejection::BatchLimitExceeded),
        );
        assert_eq!(
            ready(port.quotes(&vec![quote_instrument(); 201])),
            Err(expected)
        );
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
            ready(port.quotes(&[quote_instrument()])).err(),
            Some(failure(ReadEndpoint::Quotes))
        );
        assert_eq!(
            ready(port.candles(
                &quote_instrument(),
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
    #[test]
    fn a20_broker_contract_methods_return_domain_results_and_send_futures() {
        let port = FakeBroker(Scenario::Failed);
        let account = AccountId::new(19);
        let stock = instrument();
        let quote = quote_instrument();
        let id = BrokerOrderId::try_new("fixture-order-73").expect("주문 키");
        let sellable_quantity: Result<SellableQuantityFact, PortError> =
            ready(port.sellable_quantity(&account, &stock));
        assert_eq!(
            sellable_quantity,
            Err(failure(ReadEndpoint::SellableQuantity))
        );
        let open_orders: Result<BrokerOrderSet, PortError> = ready(port.open_orders(&account));
        assert_eq!(open_orders, Err(failure(ReadEndpoint::OpenOrders)));
        let closed_orders: Result<ClosedOrderSet, PortError> =
            ready(port.closed_orders(&account, interval()));
        assert_eq!(closed_orders, Err(failure(ReadEndpoint::ClosedOrders)));
        let order: Result<BrokerOrder, PortError> = ready(port.order(&account, &id));
        assert_eq!(order, Err(failure(ReadEndpoint::Order)));
        let stock_warnings: Result<StockWarnings, PortError> = ready(port.stock_warnings(&stock));
        assert_eq!(stock_warnings, Err(failure(ReadEndpoint::StockWarnings)));
        let instruments: Result<MasterInstrumentFacts, PortError> =
            ready(port.instruments(KnownMarket::Us));
        assert_eq!(instruments, Err(failure(ReadEndpoint::Instruments)));
        let orderbook: Result<Orderbook, PortError> = ready(port.orderbook(&quote));
        assert_eq!(orderbook, Err(failure(ReadEndpoint::Orderbook)));
        let price_limits: Result<PriceLimits, PortError> = ready(port.price_limits(&quote));
        assert_eq!(price_limits, Err(failure(ReadEndpoint::PriceLimits)));
        let commission_rate: Result<CommissionRateFact, PortError> =
            ready(port.commission_rate(&account, KnownMarket::Us));
        assert_eq!(commission_rate, Err(failure(ReadEndpoint::CommissionRate)));
    }
    #[test]
    fn a20_market_contract_methods_return_domain_results_and_send_futures() {
        let port = FakeBroker(Scenario::Failed);
        let quote = a20_master(KnownMarket::Kr)
            .quote_instrument()
            .expect("국내 시세 식별");
        let sector = SectorId::try_new("fixture-sector").expect("섹터");
        let date = instant().offset_calendar_date();
        let query = application::RankingQuery::try_new(
            RankingKind::TopLosers,
            KnownMarket::Us,
            RankingDuration::Week,
            RankingLimit::try_new(37).expect("개수"),
            RankingCautionFilter::Excluded,
        )
        .expect("랭킹 쿼리");
        let indicator_quotes: Result<Vec<MarketIndicatorQuote>, PortError> =
            ready(port.indicator_quotes(&[IndicatorCode::Kospi, IndicatorCode::KrBond3Y]));
        assert_eq!(
            indicator_quotes,
            Err(failure(ReadEndpoint::IndicatorQuotes))
        );
        let indicator_candles: Result<Vec<MarketIndicatorCandle>, PortError> =
            ready(port.indicator_candles(IndicatorCode::Kosdaq, interval(), CandlePeriod::Minute));
        assert_eq!(
            indicator_candles,
            Err(failure(ReadEndpoint::IndicatorCandles))
        );
        let rankings: Result<StockRanking, PortError> = ready(port.rankings(&query));
        assert_eq!(rankings, Err(failure(ReadEndpoint::Rankings)));
        let stock_investor_flow: Result<Vec<InvestorFlow>, PortError> =
            ready(port.stock_investor_flow(&quote, date, date));
        assert_eq!(
            stock_investor_flow,
            Err(failure(ReadEndpoint::StockInvestorFlow))
        );
        let indicator_investor_flow: Result<Vec<InvestorFlow>, PortError> = ready(
            port.indicator_investor_flow(IndicatorCode::Kospi, date, date, FlowInterval::Week),
        );
        assert_eq!(
            indicator_investor_flow,
            Err(failure(ReadEndpoint::IndicatorInvestorFlow))
        );
        let sector_composition: Result<SectorComposition, PortError> =
            ready(port.sector_composition(&sector, KnownMarket::Kr));
        assert_eq!(
            sector_composition,
            Err(failure(ReadEndpoint::SectorComposition))
        );
        let calendar: Result<MarketCalendar, PortError> = ready(port.calendar(KnownMarket::Us));
        assert_eq!(calendar, Err(failure(ReadEndpoint::Calendar)));
    }
    fn a20_features() -> Vec<(BrokerCapability, ReadEndpoint)> {
        vec![
            (
                BrokerCapability::SellableQuantity,
                ReadEndpoint::SellableQuantity,
            ),
            (BrokerCapability::OpenOrders, ReadEndpoint::OpenOrders),
            (BrokerCapability::ClosedOrders, ReadEndpoint::ClosedOrders),
            (BrokerCapability::Order, ReadEndpoint::Order),
            (BrokerCapability::StockWarnings, ReadEndpoint::StockWarnings),
            (BrokerCapability::Instruments, ReadEndpoint::Instruments),
            (BrokerCapability::Orderbook, ReadEndpoint::Orderbook),
            (BrokerCapability::PriceLimits, ReadEndpoint::PriceLimits),
            (
                BrokerCapability::CommissionRate,
                ReadEndpoint::CommissionRate,
            ),
            (
                BrokerCapability::IndicatorQuotes,
                ReadEndpoint::IndicatorQuotes,
            ),
            (
                BrokerCapability::IndicatorCandles,
                ReadEndpoint::IndicatorCandles,
            ),
            (BrokerCapability::Rankings, ReadEndpoint::Rankings),
            (
                BrokerCapability::StockInvestorFlow,
                ReadEndpoint::StockInvestorFlow,
            ),
            (
                BrokerCapability::IndicatorInvestorFlow,
                ReadEndpoint::IndicatorInvestorFlow,
            ),
            (
                BrokerCapability::SectorComposition,
                ReadEndpoint::SectorComposition,
            ),
            (BrokerCapability::Calendar, ReadEndpoint::Calendar),
        ]
    }
    #[test]
    fn a20_capabilities_and_new_endpoint_errors_preserve_closed_classification() {
        for (capability, endpoint) in a20_features() {
            assert_eq!(
                BrokerCapabilities::new(BTreeSet::from([capability])).support_for(capability),
                CapabilitySupport::Supported
            );
            assert_eq!(
                BrokerCapabilities::new(BTreeSet::new()).support_for(capability),
                CapabilitySupport::Unsupported
            );
            assert_eq!(
                application::classify_port_error(PortError::new(
                    endpoint,
                    HttpStatus::BadRequest,
                    PortErrorKind::Unsupported(capability)
                )),
                application::ErrorClass::Bug(application::BugDiagnostic::UnsupportedCapability(
                    capability
                ))
            );
        }
        assert_eq!(
            application::classify_port_error(PortError::new(
                ReadEndpoint::Order,
                HttpStatus::NotFound,
                PortErrorKind::NotFound(MissingData::Order)
            )),
            application::ErrorClass::NoData(MissingData::Order)
        );
    }
    fn a20_order(account: &AccountId, id: &BrokerOrderId) -> BrokerOrder {
        BrokerOrder::from_lookup_result(BrokerOrderRow {
            account_id: account.clone(),
            order_id: id.clone(),
            instrument: instrument(),
            side: OrderSide::Sell,
            status: BrokerOrderStatus::Canceled,
            size: BrokerOrderSize::Quantity(
                Quantity::parse(&NumericInput::try_new("13").expect("수량")).expect("수량"),
            ),
            filled_quantity: Quantity::parse(&NumericInput::try_new("0").expect("미체결"))
                .expect("수량"),
            average_filled_price: None,
            filled_amount: BrokerOrderMoney::Absent,
            filled_at: None,
            ordered_at: instant(),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Absent,
            order_type: BrokerOrderType::Market,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: instant(),
        })
        .expect("종료 주문")
    }
    fn a20_calendar(market: KnownMarket) -> MarketCalendar {
        let info = match market {
            KnownMarket::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
            KnownMarket::Us => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        };
        let first = instant().offset_calendar_date();
        let second = instant()
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(86_400), Nanosecond::new(0)).expect("하루"),
            )
            .expect("둘째")
            .offset_calendar_date();
        let third = instant()
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(172_800), Nanosecond::new(0)).expect("이틀"),
            )
            .expect("셋째")
            .offset_calendar_date();
        MarketCalendar::try_new(
            MarketSession::try_new(first, info).expect("전일"),
            MarketSession::try_new(second, info).expect("당일"),
            MarketSession::try_new(third, info).expect("다음 날"),
        )
        .expect("달력")
    }
    #[test]
    fn a20_success_keeps_bound_facts_incomplete_orders_and_unknown_server_times() {
        let port = FakeBroker(Scenario::Complete);
        let account = AccountId::new(73);
        let stock = instrument();
        let quote = quote_instrument();
        let id = BrokerOrderId::try_new("fixture-37").expect("주문 ID");
        let sellable = ready(port.sellable_quantity(&account, &stock)).expect("매도가능");
        assert_eq!(sellable.account_id(), &account);
        assert_eq!(sellable.instrument(), &stock);
        assert_eq!(
            sellable.quantity(),
            Quantity::parse(&NumericInput::try_new("17").expect("수량")).expect("수량")
        );
        assert_eq!(sellable.observed_at(), instant());
        let open = ready(port.open_orders(&account)).expect("미체결");
        assert_eq!(open.account_id(), &account);
        assert_eq!(open.coverage(), OpenOrderCoverage::Incomplete);
        let closed = ready(port.closed_orders(&account, interval())).expect("종료");
        assert_eq!(closed.account_id(), &account);
        assert_eq!(closed.covered(), interval());
        assert_eq!(closed.observed_at(), a20_observed());
        assert_eq!(
            closed.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
        let order = ready(port.order(&account, &id)).expect("단건");
        assert_eq!(order.account_id(), &account);
        assert_eq!(order.order_id(), &id);
        assert_eq!(order.instrument(), &stock);
        let warnings = ready(port.stock_warnings(&stock)).expect("경고");
        assert_eq!(warnings.instrument(), &stock);
        assert_eq!(warnings.warnings(), &[StockWarning::InvestmentRisk]);
        assert_eq!(
            ready(port.instruments(KnownMarket::Us))
                .expect("마스터")
                .instruments(),
            &[a20_master(KnownMarket::Us)]
        );
        let book = ready(port.orderbook(&quote)).expect("호가");
        assert_eq!(book.instrument(), &quote);
        assert_eq!(book.timestamp(), None);
        assert_eq!(book.collected_at(), instant());
        let limits = ready(port.price_limits(&quote)).expect("가격 제한");
        assert_eq!(limits.instrument(), &quote);
        assert_eq!(limits.range(), PriceLimitRange::Unlimited);
        let _commission: CommissionRateFact =
            ready(port.commission_rate(&account, KnownMarket::Us)).expect("수수료율");
    }
    #[test]
    fn a20_market_success_keeps_units_and_request_identity() {
        let port = FakeBroker(Scenario::Complete);
        let quote = a20_master(KnownMarket::Kr)
            .quote_instrument()
            .expect("국내 시세 식별");
        let indicators = ready(port.indicator_quotes(&[IndicatorCode::Kospi])).expect("지수");
        assert_eq!(
            indicators.first().expect("지수 행").code(),
            IndicatorCode::Kospi
        );
        assert_eq!(
            indicators.first().expect("지수 행").unit(),
            IndicatorUnit::IndexPoints
        );
        assert_eq!(
            ready(port.indicator_candles(IndicatorCode::Kospi, interval(), CandlePeriod::Daily))
                .expect("빈 봉"),
            []
        );
        let query = application::RankingQuery::try_new(
            RankingKind::TopLosers,
            KnownMarket::Us,
            RankingDuration::Week,
            RankingLimit::try_new(37).expect("개수"),
            RankingCautionFilter::Excluded,
        )
        .expect("요청");
        let ranked = ready(port.rankings(&query)).expect("랭킹");
        assert_eq!(ranked.kind(), query.kind());
        assert_eq!(ranked.market(), query.market());
        assert_eq!(ranked.duration(), query.duration());
        assert_eq!(ranked.limit(), query.limit());
        assert_eq!(ranked.caution_filter(), query.caution_filter());
        let day = instant().offset_calendar_date();
        assert_eq!(
            ready(port.stock_investor_flow(&quote, day, day)).expect("빈 수급"),
            []
        );
        assert_eq!(
            ready(port.indicator_investor_flow(IndicatorCode::Kospi, day, day, FlowInterval::Week))
                .expect("빈 시장 수급"),
            []
        );
        let sector = SectorId::try_new("fixture-19").expect("섹터 ID");
        let composition = ready(port.sector_composition(&sector, KnownMarket::Us)).expect("구성");
        assert_eq!(composition.sector(), &sector);
        assert_eq!(composition.market(), KnownMarket::Us);
        assert_eq!(
            composition.scope(),
            SectorCompositionScope::IncludingDescendants
        );
        let calendar = ready(port.calendar(KnownMarket::Us)).expect("달력");
        assert_eq!(calendar.today().market(), Market::Us);
        assert_eq!(calendar.today().sessions(), []);
    }
    #[test]
    fn a20_distinct_ranking_requests_keep_their_response_identity() {
        let port = FakeBroker(Scenario::Complete);
        for (kind, duration, market, count, caution) in [
            (
                RankingKind::MarketTradingVolume,
                RankingDuration::Day,
                KnownMarket::Kr,
                19,
                RankingCautionFilter::Included,
            ),
            (
                RankingKind::TopLosers,
                RankingDuration::Week,
                KnownMarket::Us,
                37,
                RankingCautionFilter::Excluded,
            ),
            (
                RankingKind::TossTradingAmount,
                RankingDuration::Realtime,
                KnownMarket::Us,
                61,
                RankingCautionFilter::Included,
            ),
        ] {
            let query = application::RankingQuery::try_new(
                kind,
                market,
                duration,
                RankingLimit::try_new(count).expect("개수"),
                caution,
            )
            .expect("유효 조회");
            let response = ready(port.rankings(&query)).expect("응답");
            assert_eq!(response.kind(), kind);
            assert_eq!(response.duration(), duration);
            assert_eq!(response.market(), market);
            assert_eq!(response.limit(), query.limit());
            assert_eq!(response.caution_filter(), caution);
        }
    }

    fn a20_ranking_kind(kind: RankingKind) -> MasterCode {
        MasterCode::try_new(match kind {
            RankingKind::MarketTradingAmount => "MARKET_TRADING_AMOUNT",
            RankingKind::MarketTradingVolume => "MARKET_TRADING_VOLUME",
            RankingKind::TossTradingAmount => "TOSS_SECURITIES_TRADING_AMOUNT",
            RankingKind::TossTradingVolume => "TOSS_SECURITIES_TRADING_VOLUME",
            RankingKind::TopGainers => "TOP_GAINERS",
            RankingKind::TopLosers => "TOP_LOSERS",
            RankingKind::Unknown => "UNKNOWN",
        })
        .expect("종류 코드")
    }
    fn a20_ranking_duration(duration: RankingDuration) -> MasterCode {
        MasterCode::try_new(match duration {
            RankingDuration::Realtime => "realtime",
            RankingDuration::Day => "1d",
            RankingDuration::Week => "1w",
            RankingDuration::Month => "1mo",
            RankingDuration::ThreeMonths => "3mo",
            RankingDuration::SixMonths => "6mo",
            RankingDuration::Year => "1y",
            RankingDuration::Unknown => "unknown",
        })
        .expect("기간 코드")
    }
    fn a20_observed() -> Timestamp {
        instant()
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(71), Nanosecond::new(0)).expect("관측 간격"),
            )
            .expect("응답 관측 시각")
    }
    #[test]
    fn a20_evaluation_request_does_not_fabricate_response_coverage_or_observation() {
        let start = instant()
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(11), Nanosecond::new(0)).expect("시작 간격"),
            )
            .expect("평가 시작");
        let end = instant()
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(23), Nanosecond::new(0)).expect("종료 간격"),
            )
            .expect("평가 끝");
        let evaluation = TimeInterval::try_new(start, end).expect("평가 구간");
        let account = AccountId::new(83);
        let response = ready(FakeBroker(Scenario::Complete).closed_orders(&account, evaluation))
            .expect("부분 조회");
        assert_eq!(response.account_id(), &account);
        assert_eq!(response.covered(), interval());
        assert_eq!(response.observed_at(), a20_observed());
        assert_ne!(response.covered(), evaluation);
        assert_ne!(response.observed_at(), evaluation.end());
        assert_eq!(
            domain::net_purchase_amount(
                &response,
                evaluation,
                domain::NetPurchaseScope::Currency(Currency::Usd)
            ),
            Err(domain::NetPurchaseError::Incomplete(
                domain::NetPurchaseIncomplete::Query(ClosedOrderIncomplete::MissingPages)
            ))
        );
    }
    #[test]
    fn a20_indicator_batches_preserve_requested_order_duplicates_and_units() {
        let requested = [
            IndicatorCode::KrBond3Y,
            IndicatorCode::Kosdaq,
            IndicatorCode::Kospi,
            IndicatorCode::KrBond3Y,
        ];
        let rows =
            ready(FakeBroker(Scenario::Complete).indicator_quotes(&requested)).expect("지표 묶음");
        assert_eq!(rows.len(), 4);
        for (row, (code, unit)) in rows.iter().zip([
            (IndicatorCode::KrBond3Y, IndicatorUnit::YieldPercent),
            (IndicatorCode::Kosdaq, IndicatorUnit::IndexPoints),
            (IndicatorCode::Kospi, IndicatorUnit::IndexPoints),
            (IndicatorCode::KrBond3Y, IndicatorUnit::YieldPercent),
        ]) {
            assert_eq!(row.code(), code);
            assert_eq!(row.unit(), unit);
        }
        assert_eq!(
            ready(FakeBroker(Scenario::Complete).indicator_quotes(&[IndicatorCode::Unknown])),
            Err(PortError::new(
                ReadEndpoint::IndicatorQuotes,
                HttpStatus::BadRequest,
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery)
            ))
        );
    }

    fn a20_indicator_quote(code: IndicatorCode) -> Result<MarketIndicatorQuote, PortError> {
        let (raw, number) = match code {
            IndicatorCode::Kospi => ("KOSPI", "2571"),
            IndicatorCode::Kosdaq => ("KOSDAQ", "731"),
            IndicatorCode::KrBond2Y => ("KR_BOND_2Y", "2.3"),
            IndicatorCode::KrBond3Y => ("KR_BOND_3Y", "3.7"),
            IndicatorCode::KrBond5Y => ("KR_BOND_5Y", "4.1"),
            IndicatorCode::KrBond10Y => ("KR_BOND_10Y", "4.3"),
            IndicatorCode::KrBond20Y => ("KR_BOND_20Y", "4.7"),
            IndicatorCode::KrBond30Y => ("KR_BOND_30Y", "5.1"),
            IndicatorCode::Unknown => {
                return Err(PortError::new(
                    ReadEndpoint::IndicatorQuotes,
                    HttpStatus::BadRequest,
                    PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
                ));
            }
        };
        let raw_code = MasterCode::try_new(raw).map_err(|_error| a20_indicator_error())?;
        let numeric = NumericInput::try_new(number).map_err(|_error| a20_indicator_error())?;
        MarketIndicatorQuote::from_response(raw_code, &numeric, None, instant())
            .map_err(|_error| a20_indicator_error())
    }
    fn a20_indicator_error() -> PortError {
        PortError::new(
            ReadEndpoint::IndicatorQuotes,
            HttpStatus::Success,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        )
    }
    fn a20_master(market: KnownMarket) -> MasterInstrument {
        let (listing, symbol, currency, id) = match market {
            KnownMarket::Kr => ("KOSPI", "005930", Currency::Krw, "fixture-kr-19"),
            KnownMarket::Us => ("NASDAQ", "AAPL", Currency::Usd, "fixture-us-37"),
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(listing).expect("시장"),
            instrument_id: InstrumentId::try_new(id).expect("안정적 ID"),
            symbol: Symbol::try_new(symbol).expect("종목"),
            name: DisplayInstrumentName::try_new("고정 자료 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: instant(),
        })
        .expect("시장별 마스터")
    }
    #[test]
    fn a20_master_fee_and_calendar_keep_both_requested_markets() {
        let port = FakeBroker(Scenario::Complete);
        for market in [KnownMarket::Kr, KnownMarket::Us] {
            let master = ready(port.instruments(market)).expect("마스터");
            assert_eq!(master.instruments(), &[a20_master(market)]);
            let fee = ready(port.commission_rate(&AccountId::new(89), market)).expect("수수료");
            let expected = CommissionRateFact::from_commissions_result(
                market.into(),
                CostRate::try_new(
                    Ratio::parse(&NumericInput::try_new("0.0017").expect("요율")).expect("요율"),
                )
                .expect("수수료율"),
                CommissionValidity::try_new(None, None).expect("기간"),
            );
            assert_eq!(fee, expected);
            let calendar = ready(port.calendar(market)).expect("달력");
            assert_eq!(calendar.today().market(), market.into());
        }
    }
    #[test]
    fn a21_methods_keep_domain_types_send_errors_and_request_identity() {
        let sector = SectorId::try_new("sector-21").expect("식별자");
        let query = application::SectorRankingQuery::try_new(
            RankingKind::MarketTradingAmount,
            KnownMarket::Us,
            RankingDuration::SixMonths,
        )
        .expect("요청");
        let limit = RankingLimit::try_new(37).expect("개수");
        let port = FakeBroker(Scenario::Complete);
        let investors: domain::InvestorTradingRankings =
            ready(port.investor_trading_rankings(limit)).expect("투자자");
        assert_eq!(investors.limit(), limit);
        assert_eq!(
            investors.foreigner().investor(),
            domain::InvestorClass::Foreigner
        );
        assert_eq!(
            investors.institution().investor(),
            domain::InvestorClass::Institution
        );
        assert_eq!(
            investors.individual().investor(),
            domain::InvestorClass::Individual
        );
        let catalog: domain::SectorCatalog = ready(port.sectors()).expect("목록");
        assert_eq!(catalog.sectors(), &[]);
        let detail: domain::SectorDetail =
            ready(port.sector(&sector, KnownMarket::Kr)).expect("상세");
        assert_eq!(detail.metadata().id(), &sector);
        assert_eq!(detail.market(), KnownMarket::Kr);
        let ranking: domain::SectorRanking = ready(port.sector_rankings(&query)).expect("랭킹");
        assert_eq!(ranking.kind(), RankingKind::MarketTradingAmount);
        assert_eq!(ranking.market(), KnownMarket::Us);
        assert_eq!(ranking.duration(), RankingDuration::SixMonths);
        let etfs: domain::SectorEtfs =
            ready(port.sector_etfs(&sector, KnownMarket::Us)).expect("ETF");
        assert_eq!(etfs.sector(), &sector);
        assert_eq!(etfs.market(), KnownMarket::Us);
        assert_eq!(etfs.observation().collected_at(), instant());
        let failed = FakeBroker(Scenario::Failed);
        assert_eq!(
            ready(failed.investor_trading_rankings(limit)),
            Err(failure(ReadEndpoint::InvestorTradingRankings))
        );
        assert_eq!(ready(failed.sectors()), Err(failure(ReadEndpoint::Sectors)));
        assert_eq!(
            ready(failed.sector(&sector, KnownMarket::Kr)),
            Err(failure(ReadEndpoint::Sector))
        );
        assert_eq!(
            ready(failed.sector_rankings(&query)),
            Err(failure(ReadEndpoint::SectorRankings))
        );
        assert_eq!(
            ready(failed.sector_etfs(&sector, KnownMarket::Us)),
            Err(failure(ReadEndpoint::SectorEtfs))
        );
    }
    #[test]
    fn a21_sector_detail_and_related_etfs_keep_both_markets_and_distinct_ids() {
        let port = FakeBroker(Scenario::Complete);
        for sector in [
            SectorId::try_new("sector-21").expect("첫 ID"),
            SectorId::try_new("sector-93").expect("다른 ID"),
        ] {
            for market in [KnownMarket::Kr, KnownMarket::Us] {
                let detail = ready(port.sector(&sector, market)).expect("상세");
                assert_eq!(detail.metadata().id(), &sector);
                assert_eq!(detail.market(), market);
                let etfs = ready(port.sector_etfs(&sector, market)).expect("ETF");
                assert_eq!(etfs.sector(), &sector);
                assert_eq!(etfs.market(), market);
            }
        }
    }
    #[test]
    fn d44_supply_ports_preserve_empty_success_and_endpoint_errors() {
        let instrument = a20_master(KnownMarket::Kr)
            .quote_instrument()
            .expect("국내 종목");
        let date = instant().offset_calendar_date();
        let good = FakeBroker(Scenario::Complete);
        let bad = FakeBroker(Scenario::Failed);
        assert_eq!(
            ready(good.program_trades(&instrument, date, date)).expect("빈 자료"),
            vec![]
        );
        assert_eq!(
            ready(bad.program_trades(&instrument, date, date)),
            Err(failure(ReadEndpoint::ProgramTrades))
        );
        assert_eq!(
            ready(good.short_selling(&instrument, date, date)).expect("빈 자료"),
            vec![]
        );
        assert_eq!(
            ready(bad.short_selling(&instrument, date, date)),
            Err(failure(ReadEndpoint::ShortSelling))
        );
        assert_eq!(
            ready(good.credit_trades(&instrument, date, date)).expect("빈 자료"),
            vec![]
        );
        assert_eq!(
            ready(bad.credit_trades(&instrument, date, date)),
            Err(failure(ReadEndpoint::CreditTrades))
        );
        assert_eq!(
            ready(good.securities_lending(&instrument, date, date)).expect("빈 자료"),
            vec![]
        );
        assert_eq!(
            ready(bad.securities_lending(&instrument, date, date)),
            Err(failure(ReadEndpoint::SecuritiesLending))
        );
    }
}
