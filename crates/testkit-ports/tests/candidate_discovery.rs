#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        DiscoverCandidates, DiscoveryError, DiscoveryList, DiscoveryLists, HttpStatus, PortError,
        PortErrorKind, ReadEndpoint, RetryDelay, UnavailableCause,
    };
    use domain::{
        CapBand, Currency, DiscoveryQuota, DiscoverySeed, DiscoverySlots, DiscoverySource,
        KnownMarket, MasterCode, Nanosecond, NumericInput, OffsetSeconds, RankingCautionFilter,
        RankingKind, RankingLimit, RankingOrdinal, RankingResponseInput, RankingRow,
        SectorComposition, SectorId, SectorStockRow, StockRanking, Symbol, Timestamp, UnixSeconds,
    };
    use testkit_ports::Script;

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
    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn symbol(value: &str) -> Symbol {
        Symbol::try_new(value).expect("심볼")
    }
    fn number(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("숫자")
    }
    fn ranking(symbols: &[&str]) -> StockRanking {
        StockRanking::from_response(RankingResponseInput {
            kind: MasterCode::try_new("MARKET_TRADING_AMOUNT").expect("코드"),
            market: KnownMarket::Kr,
            duration: MasterCode::try_new("1d").expect("기간"),
            limit: RankingLimit::try_new(10).expect("개수"),
            rows: symbols
                .iter()
                .enumerate()
                .map(|(index, value)| RankingRow {
                    rank: RankingOrdinal::new(index.saturating_add(1)),
                    symbol: symbol(value),
                    currency: Currency::Krw,
                    volume: number("10"),
                    amount: number(&format!("{}", 1000_usize.saturating_sub(index))),
                    change_rate: None,
                })
                .collect(),
            ranked_at: None,
            collected: at(),
            caution_filter: RankingCautionFilter::Excluded,
        })
        .expect("랭킹")
    }
    fn sector(rows: &[(&str, Option<&str>)]) -> SectorComposition {
        SectorComposition::from_stocks_response(
            SectorId::try_new("semis").expect("섹터"),
            KnownMarket::Kr,
            rows.iter()
                .map(|(value, cap)| SectorStockRow {
                    symbol: symbol(value),
                    currency: Currency::Krw,
                    market_capitalization: cap.map(number),
                    trading_amount: None,
                })
                .collect(),
            at(),
        )
        .expect("섹터 구성")
    }
    fn unavailable(endpoint: ReadEndpoint) -> PortError {
        PortError::new(
            endpoint,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        )
    }

    #[derive(Default)]
    struct Lists {
        rankings: Script<(), Result<Vec<StockRanking>, PortError>>,
        sectors: Script<(), Result<Vec<SectorComposition>, PortError>>,
        supply: Script<(), Result<Vec<Symbol>, PortError>>,
    }
    impl DiscoveryLists for Lists {
        async fn rankings(&self) -> Result<Vec<StockRanking>, PortError> {
            self.rankings.run(()).await
        }
        async fn sectors(&self) -> Result<Vec<SectorComposition>, PortError> {
            self.sectors.run(()).await
        }
        async fn supply_leaders(&self) -> Result<Vec<Symbol>, PortError> {
            self.supply.run(()).await
        }
    }
    fn quota() -> DiscoveryQuota {
        DiscoveryQuota {
            large: DiscoverySlots::new(1),
            mid: DiscoverySlots::new(2),
            small: DiscoverySlots::new(2),
        }
    }

    #[test]
    fn candidates_come_from_code_lists_with_sources_and_sector_market_caps() {
        let lists = Lists::default();
        lists
            .rankings
            .push(Ok(vec![ranking(&["000001", "000002", "000009"])]));
        lists.sectors.push(Ok(vec![sector(&[
            ("000001", Some("20000000000000")),
            ("000002", Some("30000000000000")),
            ("000003", Some("2000000000000")),
            ("000009", None),
        ])]));
        lists
            .supply
            .push(Ok(vec![symbol("000004"), symbol("000003")]));
        let known = BTreeSet::from([symbol("000009")]);
        let report = run(DiscoverCandidates {
            lists: &lists,
            quota: quota(),
        }
        .execute(&known, DiscoverySeed::new(3)))
        .expect("발굴");
        assert_eq!(report.unavailable, Vec::new());
        let mut chosen: Vec<_> = report
            .candidates
            .iter()
            .map(|candidate| (candidate.symbol().clone(), candidate.band()))
            .collect();
        chosen.sort();
        assert_eq!(
            chosen,
            vec![
                (symbol("000001"), CapBand::Large),
                (symbol("000003"), CapBand::Mid),
                (symbol("000004"), CapBand::Small),
            ]
        );
        let first = report
            .candidates
            .iter()
            .find(|candidate| candidate.symbol() == &symbol("000001"))
            .expect("1");
        assert_eq!(
            first.sources(),
            &[
                DiscoverySource::Ranking(RankingKind::MarketTradingAmount),
                DiscoverySource::Sector(SectorId::try_new("semis").expect("섹터")),
            ]
        );
        let mid = report
            .candidates
            .iter()
            .find(|candidate| candidate.symbol() == &symbol("000003"))
            .expect("3");
        assert_eq!(mid.sources().len(), 2);
    }

    #[test]
    fn failed_lists_are_reported_and_all_failing_is_an_error() {
        let lists = Lists::default();
        let ranking_error = unavailable(ReadEndpoint::Rankings);
        let sector_error = unavailable(ReadEndpoint::SectorComposition);
        lists.rankings.push(Err(ranking_error));
        lists.sectors.push(Err(sector_error));
        lists.supply.push(Ok(vec![symbol("000004")]));
        let report = run(DiscoverCandidates {
            lists: &lists,
            quota: quota(),
        }
        .execute(&BTreeSet::new(), DiscoverySeed::new(1)))
        .expect("일부");
        assert_eq!(
            report.unavailable,
            vec![
                (DiscoveryList::Sectors, sector_error),
                (DiscoveryList::Rankings, ranking_error)
            ]
        );
        assert_eq!(report.candidates.len(), 1);
        let none = Lists::default();
        let supply_error = unavailable(ReadEndpoint::StockInvestorFlow);
        none.rankings.push(Err(ranking_error));
        none.sectors.push(Err(sector_error));
        none.supply.push(Err(supply_error));
        assert_eq!(
            run(DiscoverCandidates {
                lists: &none,
                quota: quota(),
            }
            .execute(&BTreeSet::new(), DiscoverySeed::new(1))),
            Err(DiscoveryError::NoList(vec![
                (DiscoveryList::Sectors, sector_error),
                (DiscoveryList::Rankings, ranking_error),
                (DiscoveryList::Supply, supply_error),
            ]))
        );
    }
}
