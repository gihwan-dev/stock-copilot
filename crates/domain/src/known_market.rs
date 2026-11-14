use crate::Market;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KnownMarket {
    Kr,
    Us,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownKnownMarket;

impl KnownMarket {
    pub fn try_from_market(market: &Market) -> Result<Self, UnknownKnownMarket> {
        match market {
            Market::Kr => Ok(Self::Kr),
            Market::Us => Ok(Self::Us),
            Market::Unknown(_) => Err(UnknownKnownMarket),
        }
    }
}

impl From<KnownMarket> for Market {
    fn from(value: KnownMarket) -> Self {
        match value {
            KnownMarket::Kr => Self::Kr,
            KnownMarket::Us => Self::Us,
        }
    }
}
