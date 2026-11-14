use crate::{
    LiquidationTrading, ListingStatus, Market, MasterInstrumentError, MasterInstrumentRow,
    NxtTrading, OrderSide, ResolvedOrderInstrument, TradingSuspension,
};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct MasterOrderGateFacts {
    instrument: ResolvedOrderInstrument,
    row: MasterInstrumentRow,
}
pub(crate) fn from_master(
    instrument: ResolvedOrderInstrument,
    row: MasterInstrumentRow,
) -> MasterOrderGateFacts {
    MasterOrderGateFacts { instrument, row }
}
impl fmt::Debug for MasterOrderGateFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterOrderGateFacts")
            .finish_non_exhaustive()
    }
}
impl MasterOrderGateFacts {
    #[must_use]
    pub const fn row(&self) -> &MasterInstrumentRow {
        &self.row
    }
    pub fn instrument_for(
        &self,
        side: OrderSide,
    ) -> Result<ResolvedOrderInstrument, MasterInstrumentError> {
        if self.row.listing_status != ListingStatus::Active {
            return Err(MasterInstrumentError::NotListed);
        }
        if self.row.security_type.known().is_none() {
            return Err(MasterInstrumentError::UnknownSecurityType);
        }
        if self.instrument.market() == Market::Kr {
            let detail = self
                .row
                .korean_detail
                .ok_or(MasterInstrumentError::MissingTradingStatus)?;
            if detail.krx == TradingSuspension::Suspended {
                return Err(MasterInstrumentError::TradingSuspended);
            }
            if side == OrderSide::Buy && detail.liquidation == LiquidationTrading::Liquidation {
                return Err(MasterInstrumentError::LiquidationBuy);
            }
        }
        Ok(self.instrument.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NxtSupport {
    Unsupported,
    Supported,
}
impl NxtTrading {
    pub fn try_from_parts(
        support: NxtSupport,
        suspension: Option<TradingSuspension>,
    ) -> Result<Self, MasterInstrumentError> {
        match (support, suspension) {
            (NxtSupport::Unsupported, None) => Ok(Self::Unsupported),
            (NxtSupport::Supported, Some(state)) => Ok(Self::Supported(state)),
            (NxtSupport::Unsupported, Some(_)) | (NxtSupport::Supported, None) => {
                Err(MasterInstrumentError::InvalidNxtStatus)
            }
        }
    }
}
