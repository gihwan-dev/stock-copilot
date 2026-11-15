#[cfg(test)]
mod tests {
    use application::{
        HttpStatus, PortError, PortErrorKind, RankingQuery, ReadEndpoint, RequestRejection,
    };
    use domain::{KnownMarket, RankingCautionFilter, RankingDuration, RankingKind, RankingLimit};

    fn accepted_periods(kind: RankingKind) -> Vec<RankingDuration> {
        use RankingDuration::{Day, Month, Realtime, SixMonths, ThreeMonths, Week, Year};
        match kind {
            RankingKind::Unknown => vec![],
            RankingKind::TopGainers | RankingKind::TopLosers => {
                vec![Day, Week, Month, ThreeMonths, SixMonths, Year]
            }
            RankingKind::MarketTradingAmount
            | RankingKind::MarketTradingVolume
            | RankingKind::TossTradingAmount
            | RankingKind::TossTradingVolume => {
                vec![Realtime, Day, Week, Month, ThreeMonths, SixMonths, Year]
            }
        }
    }
    fn assert_request(
        kind: RankingKind,
        duration: RankingDuration,
        market: KnownMarket,
        limit: RankingLimit,
        caution: RankingCautionFilter,
    ) {
        let actual = RankingQuery::try_new(kind, market, duration, limit, caution);
        if accepted_periods(kind).contains(&duration) {
            let query = actual.expect("지원 조회");
            assert_eq!(query.kind(), kind);
            assert_eq!(query.market(), market);
            assert_eq!(query.duration(), duration);
            assert_eq!(query.limit(), limit);
            assert_eq!(query.caution_filter(), caution);
        } else {
            assert_eq!(
                actual,
                Err(PortError::new(
                    ReadEndpoint::Rankings,
                    HttpStatus::BadRequest,
                    PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery)
                ))
            );
        }
    }
    #[test]
    fn ranking_request_matches_domain_supported_combinations_and_preserves_every_field() {
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
                assert_request(
                    kind,
                    duration,
                    KnownMarket::Kr,
                    RankingLimit::try_new(7).expect("개수"),
                    RankingCautionFilter::Included,
                );
                assert_request(
                    kind,
                    duration,
                    KnownMarket::Us,
                    RankingLimit::try_new(93).expect("개수"),
                    RankingCautionFilter::Excluded,
                );
            }
        }
    }
}
