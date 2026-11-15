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
        if kind == RankingKind::Unknown
            || duration == RankingDuration::Unknown
            || ((kind == RankingKind::TopGainers || kind == RankingKind::TopLosers)
                && duration == RankingDuration::Realtime)
        {
            return Err(PortError::new(
                ReadEndpoint::Rankings,
                HttpStatus::BadRequest,
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
            ));
        }
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
