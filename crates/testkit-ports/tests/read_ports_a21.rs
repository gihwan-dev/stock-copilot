#[cfg(test)]
mod tests {
    use application::{
        BrokerCapabilities, BrokerCapability, CapabilitySupport, HttpStatus, PortError,
        PortErrorKind, ReadEndpoint, SectorRankingQuery,
    };
    use domain::{
        DisplayInstrumentName, InvestorGroupRow, InvestorTradingRankings, KnownMarket, Nanosecond,
        NumericInput, OffsetSeconds, RankingDuration, RankingKind, RankingLimit, SectorCatalog,
        SectorDepth, SectorDetail, SectorEtfRow, SectorEtfs, SectorId, SectorMetadataRow,
        SectorRanking, SectorRateRow, Symbol, Timestamp, UnixSeconds,
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
    fn sector(value: &str) -> SectorId {
        SectorId::try_new(value).expect("섹터")
    }
    fn limit(value: usize) -> RankingLimit {
        RankingLimit::try_new(value).expect("상한")
    }
    fn metadata() -> SectorMetadataRow {
        SectorMetadataRow {
            id: sector("sector-last"),
            name: DisplayInstrumentName::try_new("고정 섹터").expect("이름"),
            depth: SectorDepth::Major,
            parent: None,
        }
    }
    fn detail() -> SectorDetail {
        let rates = [
            RankingDuration::Day,
            RankingDuration::Week,
            RankingDuration::Month,
            RankingDuration::ThreeMonths,
            RankingDuration::SixMonths,
            RankingDuration::Year,
        ]
        .into_iter()
        .map(|duration| SectorRateRow {
            duration,
            change_rate: (duration == RankingDuration::Day)
                .then(|| NumericInput::try_new("1.7").expect("등락률")),
        })
        .collect();
        SectorDetail::from_response(metadata(), KnownMarket::Us, None, rates, at(37)).expect("상세")
    }
    fn investor() -> InvestorTradingRankings {
        let group = |value| InvestorGroupRow {
            ranked_at: Some(at(value)),
            net_buy: vec![],
            net_sell: vec![],
        };
        InvestorTradingRankings::from_response(group(11), group(23), group(31), limit(11), at(37))
            .expect("투자자 랭킹")
    }
    fn catalog() -> SectorCatalog {
        SectorCatalog::from_response(vec![metadata()], at(43)).expect("전체 계층")
    }
    fn ranking_query(
        kind: RankingKind,
        market: KnownMarket,
        duration: RankingDuration,
    ) -> SectorRankingQuery {
        SectorRankingQuery::try_new(kind, market, duration).expect("질의")
    }
    fn ranking() -> SectorRanking {
        SectorRanking::from_response(
            RankingKind::MarketTradingAmount,
            KnownMarket::Us,
            RankingDuration::Month,
            vec![],
            at(47),
        )
        .expect("랭킹")
    }
    fn etfs() -> SectorEtfs {
        let rows = [("AAA", "첫 ETF"), ("BBB", "둘째 ETF")]
            .into_iter()
            .map(|(symbol, name)| SectorEtfRow {
                symbol: Symbol::try_new(symbol).expect("코드"),
                name: DisplayInstrumentName::try_new(name).expect("이름"),
            })
            .collect();
        SectorEtfs::from_response(sector("sector-last"), KnownMarket::Us, rows, at(53))
            .expect("관련 ETF")
    }
    #[test]
    fn investor_trading_rankings_preserves_success_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::InvestorTradingRankings,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::InvestorTradingRankings),
        );
        let value = investor();
        port.script_investor_trading_rankings().push(Err(error));
        port.script_investor_trading_rankings()
            .push(Ok(value.clone()));
        run(contracts::market_info_investor_trading_rankings(
            &port,
            limit(7),
            Err(error),
        ));
        run(contracts::market_info_investor_trading_rankings(
            &port,
            limit(11),
            Ok(value),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::InvestorTradingRankings(limit(7)),
                MarketInfoCall::InvestorTradingRankings(limit(11))
            ],
            "메서드 전체 기록"
        );
        assert_eq!(
            port.script_investor_trading_rankings().calls(),
            [(limit(7),), (limit(11),)],
            "메서드별 인자 기록"
        );
    }
    #[test]
    fn sectors_preserves_success_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::Sectors,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::Sectors),
        );
        let value = catalog();
        port.script_sectors().push(Err(error));
        port.script_sectors().push(Ok(value.clone()));
        run(contracts::market_info_sectors(&port, Err(error)));
        run(contracts::market_info_sectors(&port, Ok(value)));
        assert_eq!(
            port.calls(),
            [MarketInfoCall::Sectors(), MarketInfoCall::Sectors()],
            "메서드 전체 기록"
        );
        assert_eq!(
            port.script_sectors().calls(),
            [(), ()],
            "메서드별 인자 기록"
        );
    }
    #[test]
    fn sector_preserves_success_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::Sector,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::Sector),
        );
        let value = detail();
        port.script_sector().push(Err(error));
        port.script_sector().push(Ok(value.clone()));
        run(contracts::market_info_sector(
            &port,
            &sector("sector-first"),
            KnownMarket::Kr,
            Err(error),
        ));
        run(contracts::market_info_sector(
            &port,
            &sector("sector-last"),
            KnownMarket::Us,
            Ok(value),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::Sector(sector("sector-first"), KnownMarket::Kr),
                MarketInfoCall::Sector(sector("sector-last"), KnownMarket::Us)
            ],
            "메서드 전체 기록"
        );
        assert_eq!(
            port.script_sector().calls(),
            [
                (sector("sector-first"), KnownMarket::Kr),
                (sector("sector-last"), KnownMarket::Us)
            ],
            "메서드별 인자 기록"
        );
    }
    #[test]
    fn sector_rankings_preserves_success_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::SectorRankings,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::SectorRankings),
        );
        let value = ranking();
        port.script_sector_rankings().push(Err(error));
        port.script_sector_rankings().push(Ok(value.clone()));
        run(contracts::market_info_sector_rankings(
            &port,
            &ranking_query(
                RankingKind::TopGainers,
                KnownMarket::Kr,
                RankingDuration::Year,
            ),
            Err(error),
        ));
        run(contracts::market_info_sector_rankings(
            &port,
            &ranking_query(
                RankingKind::MarketTradingAmount,
                KnownMarket::Us,
                RankingDuration::Month,
            ),
            Ok(value),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::SectorRankings(ranking_query(
                    RankingKind::TopGainers,
                    KnownMarket::Kr,
                    RankingDuration::Year
                )),
                MarketInfoCall::SectorRankings(ranking_query(
                    RankingKind::MarketTradingAmount,
                    KnownMarket::Us,
                    RankingDuration::Month
                ))
            ],
            "메서드 전체 기록"
        );
        assert_eq!(
            port.script_sector_rankings().calls(),
            [
                (ranking_query(
                    RankingKind::TopGainers,
                    KnownMarket::Kr,
                    RankingDuration::Year
                ),),
                (ranking_query(
                    RankingKind::MarketTradingAmount,
                    KnownMarket::Us,
                    RankingDuration::Month
                ),)
            ],
            "메서드별 인자 기록"
        );
    }
    #[test]
    fn sector_etfs_preserves_success_error_arguments_and_order() {
        let port = ScriptedMarketInfo::default();
        let error = PortError::new(
            ReadEndpoint::SectorEtfs,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(BrokerCapability::SectorEtfs),
        );
        let value = etfs();
        port.script_sector_etfs().push(Err(error));
        port.script_sector_etfs().push(Ok(value.clone()));
        run(contracts::market_info_sector_etfs(
            &port,
            &sector("sector-first"),
            KnownMarket::Kr,
            Err(error),
        ));
        run(contracts::market_info_sector_etfs(
            &port,
            &sector("sector-last"),
            KnownMarket::Us,
            Ok(value),
        ));
        assert_eq!(
            port.calls(),
            [
                MarketInfoCall::SectorEtfs(sector("sector-first"), KnownMarket::Kr),
                MarketInfoCall::SectorEtfs(sector("sector-last"), KnownMarket::Us)
            ],
            "메서드 전체 기록"
        );
        assert_eq!(
            port.script_sector_etfs().calls(),
            [
                (sector("sector-first"), KnownMarket::Kr),
                (sector("sector-last"), KnownMarket::Us)
            ],
            "메서드별 인자 기록"
        );
    }
    #[test]
    fn added_capabilities_remain_explicit_closed_values() {
        let capabilities = [
            BrokerCapability::InvestorTradingRankings,
            BrokerCapability::Sectors,
            BrokerCapability::Sector,
            BrokerCapability::SectorRankings,
            BrokerCapability::SectorEtfs,
        ];
        let none = BrokerCapabilities::new(std::collections::BTreeSet::new());
        let all = BrokerCapabilities::new(capabilities.into_iter().collect());
        for capability in capabilities {
            contracts::broker_capabilities(&none, capability, CapabilitySupport::Unsupported);
            contracts::broker_capabilities(&all, capability, CapabilitySupport::Supported);
        }
    }
}
