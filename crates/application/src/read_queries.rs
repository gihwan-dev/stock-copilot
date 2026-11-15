use crate::{HttpStatus, PortError, PortErrorKind, ReadEndpoint, RequestRejection};
use domain::{KnownMarket, RankingCautionFilter, RankingDuration, RankingKind, RankingLimit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingQuery {
    kind: RankingKind,
    market: KnownMarket,
    duration: RankingDuration,
    limit: RankingLimit,
    caution: RankingCautionFilter,
}
impl RankingQuery {
    pub fn try_new(
        kind: RankingKind,
        market: KnownMarket,
        duration: RankingDuration,
        limit: RankingLimit,
        caution: RankingCautionFilter,
    ) -> Result<Self, PortError> {
        validate_ranking_period(kind, duration)?;
        Ok(Self {
            kind,
            market,
            duration,
            limit,
            caution,
        })
    }
    #[must_use]
    pub const fn kind(self) -> RankingKind {
        self.kind
    }
    #[must_use]
    pub const fn market(self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn duration(self) -> RankingDuration {
        self.duration
    }
    #[must_use]
    pub const fn limit(self) -> RankingLimit {
        self.limit
    }
    #[must_use]
    pub const fn caution_filter(self) -> RankingCautionFilter {
        self.caution
    }
}

fn ranking_query_error() -> PortError {
    PortError::new(
        ReadEndpoint::Rankings,
        HttpStatus::BadRequest,
        PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
    )
}
fn validate_ranking_period(kind: RankingKind, duration: RankingDuration) -> Result<(), PortError> {
    match kind {
        RankingKind::Unknown => Err(ranking_query_error()),
        RankingKind::TopGainers | RankingKind::TopLosers => match duration {
            RankingDuration::Realtime | RankingDuration::Unknown => Err(ranking_query_error()),
            RankingDuration::Day
            | RankingDuration::Week
            | RankingDuration::Month
            | RankingDuration::ThreeMonths
            | RankingDuration::SixMonths
            | RankingDuration::Year => Ok(()),
        },
        RankingKind::MarketTradingAmount
        | RankingKind::MarketTradingVolume
        | RankingKind::TossTradingAmount
        | RankingKind::TossTradingVolume => match duration {
            RankingDuration::Unknown => Err(ranking_query_error()),
            RankingDuration::Realtime
            | RankingDuration::Day
            | RankingDuration::Week
            | RankingDuration::Month
            | RankingDuration::ThreeMonths
            | RankingDuration::SixMonths
            | RankingDuration::Year => Ok(()),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorRankingQuery {
    kind: RankingKind,
    market: KnownMarket,
    duration: RankingDuration,
}
impl SectorRankingQuery {
    pub fn try_new(
        kind: RankingKind,
        market: KnownMarket,
        duration: RankingDuration,
    ) -> Result<Self, PortError> {
        match kind {
            RankingKind::TopGainers | RankingKind::MarketTradingAmount => {}
            RankingKind::MarketTradingVolume
            | RankingKind::TossTradingAmount
            | RankingKind::TossTradingVolume
            | RankingKind::TopLosers
            | RankingKind::Unknown => return Err(sector_ranking_query_error()),
        }
        match duration {
            RankingDuration::Day
            | RankingDuration::Week
            | RankingDuration::Month
            | RankingDuration::ThreeMonths
            | RankingDuration::SixMonths
            | RankingDuration::Year => {}
            RankingDuration::Realtime | RankingDuration::Unknown => {
                return Err(sector_ranking_query_error());
            }
        }
        Ok(Self {
            kind,
            market,
            duration,
        })
    }
    #[must_use]
    pub const fn kind(self) -> RankingKind {
        self.kind
    }
    #[must_use]
    pub const fn market(self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn duration(self) -> RankingDuration {
        self.duration
    }
}

fn sector_ranking_query_error() -> PortError {
    PortError::new(
        ReadEndpoint::SectorRankings,
        HttpStatus::BadRequest,
        PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
    )
}
