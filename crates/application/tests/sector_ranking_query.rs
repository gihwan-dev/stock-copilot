#[cfg(test)]
mod tests {
    use application::{
        HttpStatus, PortError, PortErrorKind, ReadEndpoint, RequestRejection, SectorRankingQuery,
    };
    use domain::{KnownMarket, RankingDuration, RankingKind};
    #[test]
    fn sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity() {
        for kind in [
            RankingKind::MarketTradingAmount,
            RankingKind::MarketTradingVolume,
            RankingKind::TossTradingAmount,
            RankingKind::TossTradingVolume,
            RankingKind::TopGainers,
            RankingKind::TopLosers,
            RankingKind::Unknown,
        ] {
            for duration in [
                RankingDuration::Realtime,
                RankingDuration::Day,
                RankingDuration::Week,
                RankingDuration::Month,
                RankingDuration::ThreeMonths,
                RankingDuration::SixMonths,
                RankingDuration::Year,
                RankingDuration::Unknown,
            ] {
                assert_request(kind, duration, KnownMarket::Kr);
                assert_request(kind, duration, KnownMarket::Us);
            }
        }
    }
    fn assert_request(kind: RankingKind, duration: RankingDuration, market: KnownMarket) {
        let actual = SectorRankingQuery::try_new(kind, market, duration);
        match (kind, duration) {
            (
                RankingKind::TopGainers | RankingKind::MarketTradingAmount,
                RankingDuration::Day
                | RankingDuration::Week
                | RankingDuration::Month
                | RankingDuration::ThreeMonths
                | RankingDuration::SixMonths
                | RankingDuration::Year,
            ) => {
                let query = actual.expect("지원 조합");
                assert_eq!(query.kind(), kind);
                assert_eq!(query.market(), market);
                assert_eq!(query.duration(), duration);
            }
            _ => assert_eq!(
                actual,
                Err(PortError::new(
                    ReadEndpoint::SectorRankings,
                    HttpStatus::BadRequest,
                    PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery)
                ))
            ),
        }
    }
}
