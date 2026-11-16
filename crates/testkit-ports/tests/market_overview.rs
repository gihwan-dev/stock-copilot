#[cfg(test)]
mod tests {
    use application::{
        CredentialSource, DataAvailability, GetMarketOverview, HttpStatus, InformationError,
        MacroFact, MacroObservation, MacroSeriesId, MacroSource, MacroUnit, MarketOverview,
        MetricValue, OverviewData, OverviewEntry, OverviewError, OverviewFreshness,
        OverviewObservation, OverviewRequest, PortError, PortErrorKind, QuoteObservation,
        ReadEndpoint, ResponseMismatch,
    };
    use domain::{
        Amount, AmountInput, CalendarDate, CommonShare, Currency, DisplayInstrumentName, FxPair,
        FxRate, FxRateKind, FxSource, IndicatorCode, InstrumentId, KnownMarket, ListingStatus,
        MarketCalendar, MarketIndicatorQuote, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput, OffsetSeconds, Price,
        Quote, QuoteInstrument, SessionInformation, Symbol, TimeInterval, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        scripted::{
            BrokerReadCall, MacroSeriesCall, MarketInfoCall, ScriptedBrokerRead,
            ScriptedMacroSeries, ScriptedMarketInfo,
        },
    };
    fn run<F: Future<Output = MarketOverview>>(future: F) -> Vec<OverviewEntry> {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value.entries,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    type Query<'a> = GetMarketOverview<
        'a,
        ScriptedBrokerRead,
        ScriptedMarketInfo,
        ScriptedMacroSeries,
        SteppedClock,
    >;
    fn query<'a>(
        broker: &'a ScriptedBrokerRead,
        market: &'a ScriptedMarketInfo,
        macro_series: &'a ScriptedMacroSeries,
        clock: &'a SteppedClock,
    ) -> Query<'a> {
        GetMarketOverview {
            broker,
            market,
            macro_series,
            clock,
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

    fn error(endpoint: ReadEndpoint) -> PortError {
        PortError::new(
            endpoint,
            HttpStatus::ServerError,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        )
    }
    #[test]
    fn indicators_preserve_values_times_and_five_minute_boundary() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(1000));
        let query = query(&broker, &market, &macros, &clock);
        let cases = [
            (Some(at(699)), OverviewFreshness::OlderThanFiveMinutes),
            (Some(at(700)), OverviewFreshness::WithinFiveMinutes),
            (Some(at(701)), OverviewFreshness::WithinFiveMinutes),
            (None, OverviewFreshness::Unknown),
            (Some(at(1001)), OverviewFreshness::Unknown),
        ];
        for (stamp, expected) in cases {
            let value = MarketIndicatorQuote::from_response(
                MasterCode::try_new("KOSPI").expect("코드"),
                &NumericInput::try_new("2345.67").expect("값"),
                stamp,
                at(1002),
            )
            .expect("지표");
            market
                .script_indicator_quotes()
                .push(Ok(vec![value.clone()]));
            assert_eq!(
                run(query.execute(&[OverviewRequest::Indicator(IndicatorCode::Kospi)])),
                vec![OverviewEntry {
                    request: OverviewRequest::Indicator(IndicatorCode::Kospi),
                    result: Ok(OverviewData::Indicators(vec![OverviewObservation {
                        value,
                        collected_at: at(1002),
                        freshness: expected
                    }]))
                }]
            );
        }
        assert_eq!(
            market.calls(),
            vec![MarketInfoCall::IndicatorQuotes(vec![IndicatorCode::Kospi]); 5]
        );
        assert_eq!(broker.calls(), vec![]);
        assert_eq!(macros.calls(), vec![]);
    }
    fn sample_quote(instrument: &QuoteInstrument) -> QuoteObservation {
        let price_quote = Quote::try_new(
            instrument.symbol().clone(),
            Currency::Usd,
            Some(
                Price::parse(
                    &NumericInput::try_new("543.21").expect("가격"),
                    Currency::Usd,
                )
                .expect("가격"),
            ),
            Some(at(1699)),
            at(1777),
        )
        .expect("시세");
        QuoteObservation {
            instrument: instrument.clone(),
            result: Ok(price_quote),
        }
    }
    fn mixed_requests(
        instrument: &QuoteInstrument,
        series: &MacroSeriesId,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Vec<OverviewRequest> {
        vec![
            OverviewRequest::Indicator(IndicatorCode::Kosdaq),
            OverviewRequest::EtfApproximation(instrument.clone()),
            OverviewRequest::Macro {
                series: series.clone(),
                from,
                through,
            },
            OverviewRequest::Fx {
                pair: FxPair::KrwUsd,
                kind: FxRateKind::Buy,
            },
            OverviewRequest::Calendar(KnownMarket::Us),
        ]
    }
    #[test]
    fn mixed_results_preserve_sources_absence_errors_and_request_order() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(2000));
        let query = query(&broker, &market, &macros, &clock);
        let instrument = quote();
        let series = MacroSeriesId::try_new("DGS10").expect("계열");
        let from = at(11).offset_calendar_date();
        let through = at(90000).offset_calendar_date();
        let requests = mixed_requests(&instrument, &series, from, through);
        market
            .script_indicator_quotes()
            .push(Err(error(ReadEndpoint::IndicatorQuotes)));
        let observation = sample_quote(&instrument);
        broker.script_quotes().push(Ok(vec![observation.clone()]));
        macros
            .script_daily_values()
            .push(Ok(DataAvailability::NotConfigured));
        market
            .script_fx_rate()
            .push(Err(error(ReadEndpoint::FxRate)));
        market
            .script_calendar()
            .push(Err(error(ReadEndpoint::Calendar)));
        let expected = vec![
            Err(OverviewError::Broker(error(ReadEndpoint::IndicatorQuotes))),
            Ok(OverviewData::EtfApproximation(vec![OverviewObservation {
                value: observation,
                collected_at: at(1777),
                freshness: OverviewFreshness::OlderThanFiveMinutes,
            }])),
            Ok(OverviewData::Macro(DataAvailability::NotConfigured)),
            Err(OverviewError::Broker(error(ReadEndpoint::FxRate))),
            Err(OverviewError::Broker(error(ReadEndpoint::Calendar))),
        ];
        assert_eq!(
            run(query.execute(&requests)),
            requests
                .iter()
                .cloned()
                .zip(expected)
                .map(|(request, result)| OverviewEntry { request, result })
                .collect::<Vec<_>>()
        );
        assert_eq!(
            broker.calls(),
            vec![BrokerReadCall::Quotes(vec![instrument])]
        );
        assert_eq!(
            macros.calls(),
            vec![MacroSeriesCall::DailyValues(series, from, through)]
        );
        assert_eq!(
            market.calls(),
            vec![
                MarketInfoCall::IndicatorQuotes(vec![IndicatorCode::Kosdaq]),
                MarketInfoCall::FxRate(FxPair::KrwUsd, FxRateKind::Buy, None),
                MarketInfoCall::Calendar(KnownMarket::Us)
            ]
        );
    }
    #[test]
    fn macro_sources_and_every_availability_are_preserved() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(3000));
        let query = query(&broker, &market, &macros, &clock);
        let series = MacroSeriesId::try_new("SP500").expect("계열");
        let day = at(2).offset_calendar_date();
        let request = OverviewRequest::Macro {
            series: series.clone(),
            from: day,
            through: day,
        };
        for source in [
            MacroSource::UsTreasury,
            MacroSource::Fred,
            MacroSource::TossEtf(Symbol::try_new("SPY").expect("심볼")),
        ] {
            let value = MacroObservation::new(
                series.clone(),
                MacroFact::new(
                    MetricValue::new(Amount::parse(&AmountInput::new("4.32")).expect("값")),
                    MacroUnit::Percent,
                    day,
                    source,
                ),
            );
            macros
                .script_daily_values()
                .push(Ok(DataAvailability::Available(vec![value.clone()])));
            assert_eq!(
                run(query.execute(std::slice::from_ref(&request))),
                vec![OverviewEntry {
                    request: request.clone(),
                    result: Ok(OverviewData::Macro(DataAvailability::Available(vec![
                        OverviewObservation {
                            value,
                            collected_at: at(3000),
                            freshness: OverviewFreshness::DailyPolicyPending
                        }
                    ])))
                }]
            );
        }
    }
    #[test]
    fn macro_absence_and_errors_are_distinct() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(3000));
        let query = query(&broker, &market, &macros, &clock);
        let day = at(2).offset_calendar_date();
        let request = OverviewRequest::Macro {
            series: MacroSeriesId::try_new("DGS2").expect("계열"),
            from: day,
            through: day,
        };
        for available in [
            DataAvailability::NoData,
            DataAvailability::NotConfigured,
            DataAvailability::Available(vec![]),
        ] {
            macros.script_daily_values().push(Ok(available.clone()));
            assert_eq!(
                run(query.execute(std::slice::from_ref(&request)))[0].result,
                Ok(OverviewData::Macro(match available {
                    DataAvailability::NoData => DataAvailability::NoData,
                    DataAvailability::NotConfigured => DataAvailability::NotConfigured,
                    DataAvailability::Available(_) => DataAvailability::Available(vec![]),
                }))
            );
        }
        let failure = InformationError::InvalidCredential(CredentialSource::Fred);
        macros.script_daily_values().push(Err(failure));
        assert_eq!(
            run(query.execute(&[request]))[0].result,
            Err(OverviewError::Information(failure))
        );
        assert_eq!(run(query.execute(&[])), vec![]);
    }

    fn sample_fx() -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("1432.1").expect("환율"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(3700), at(5000)).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn sample_calendar() -> MarketCalendar {
        let sessions = [at(0), at(86400), at(172_800)].map(|stamp| {
            MarketSession::try_new(
                stamp.offset_calendar_date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
            )
            .expect("세션")
        });
        let [previous, today, next] = sessions;
        MarketCalendar::try_new(previous, today, next).expect("달력")
    }
    #[test]
    fn fx_calendar_and_etf_failures_retain_original_facts() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(4000));
        let query = query(&broker, &market, &macros, &clock);
        let fx = sample_fx();
        market.script_fx_rate().push(Ok(fx));
        let calendar = sample_calendar();
        market.script_calendar().push(Ok(calendar.clone()));
        let instrument = quote();
        let failed = QuoteObservation {
            instrument: instrument.clone(),
            result: Err(error(ReadEndpoint::Quotes)),
        };
        broker.script_quotes().push(Ok(vec![failed.clone()]));
        broker
            .script_quotes()
            .push(Err(error(ReadEndpoint::Quotes)));
        let requests = [
            OverviewRequest::Fx {
                pair: FxPair::UsdKrw,
                kind: FxRateKind::Mid,
            },
            OverviewRequest::Calendar(KnownMarket::Us),
            OverviewRequest::EtfApproximation(instrument.clone()),
            OverviewRequest::EtfApproximation(instrument),
        ];
        let results = run(query.execute(&requests));
        assert_eq!(
            results
                .iter()
                .map(|entry| &entry.request)
                .collect::<Vec<_>>(),
            requests.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            results
                .into_iter()
                .map(|entry| entry.result)
                .collect::<Vec<_>>(),
            vec![
                Ok(OverviewData::Fx(OverviewObservation {
                    value: fx,
                    collected_at: at(4000),
                    freshness: OverviewFreshness::WithinFiveMinutes
                })),
                Ok(OverviewData::Calendar(Box::new(OverviewObservation {
                    value: calendar,
                    collected_at: at(4000),
                    freshness: OverviewFreshness::Unknown
                }))),
                Ok(OverviewData::EtfApproximation(vec![OverviewObservation {
                    value: failed,
                    collected_at: at(4000),
                    freshness: OverviewFreshness::Unknown
                }])),
                Err(OverviewError::Broker(error(ReadEndpoint::Quotes)))
            ]
        );
    }
    #[test]
    fn fx_freshness_uses_original_as_of_instead_of_receive_time() {
        let broker = ScriptedBrokerRead::default();
        let market = ScriptedMarketInfo::default();
        let macros = ScriptedMacroSeries::default();
        let clock = SteppedClock::new(at(1000));
        let query = query(&broker, &market, &macros, &clock);
        for (as_of, expected) in [
            (at(699), OverviewFreshness::OlderThanFiveMinutes),
            (at(700), OverviewFreshness::WithinFiveMinutes),
            (at(701), OverviewFreshness::WithinFiveMinutes),
        ] {
            let value = FxRate::parse(
                &NumericInput::try_new("0.0007").expect("환율"),
                FxPair::KrwUsd,
                FxRateKind::Buy,
                TimeInterval::try_new(as_of, at(2000)).expect("구간"),
                FxSource::TossOpenApi,
            )
            .expect("환율");
            market.script_fx_rate().push(Ok(value));
            let result = run(query.execute(&[OverviewRequest::Fx {
                pair: FxPair::KrwUsd,
                kind: FxRateKind::Buy,
            }]));
            assert_eq!(
                result[0].result,
                Ok(OverviewData::Fx(OverviewObservation {
                    value,
                    collected_at: at(1000),
                    freshness: expected
                }))
            );
        }
    }
}
