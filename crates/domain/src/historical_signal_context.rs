use crate::{
    AcquisitionPrice, BuyingPower, Currency, HistoricalSignalQuantity, Holding, HoldingError,
    HoldingObservation, Market, MasterInstrument, Money, Portfolio, PortfolioError, Price,
    ReportedHolding, ReportedPortfolio, Symbol, Timestamp,
};

#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalHolding {
    pub market: Market,
    pub symbol: Symbol,
    pub quantity: HistoricalSignalQuantity,
    pub acquisition_price: AcquisitionPrice,
    pub current_price: Option<Price>,
    pub reported: Option<ReportedHolding>,
    pub observed_at: Timestamp,
}

impl std::fmt::Debug for HistoricalHolding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoricalHolding").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalBuyingPower {
    pub currency: Currency,
    pub money: Result<Money, PortfolioError>,
    pub observed_at: Timestamp,
}

impl std::fmt::Debug for HistoricalBuyingPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoricalBuyingPower")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalPortfolioFacts {
    pub holdings: Vec<HistoricalHolding>,
    pub krw: HistoricalBuyingPower,
    pub usd: HistoricalBuyingPower,
    pub reported: Option<ReportedPortfolio>,
}

impl std::fmt::Debug for HistoricalPortfolioFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoricalPortfolioFacts")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct HistoricalPortfolio(HistoricalPortfolioFacts, Portfolio);

impl PartialEq for HistoricalPortfolio {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.0;
        let right = &other.0;
        let offset = |value: Timestamp| value.offset_seconds();
        left == right
            && self.1 == other.1
            && offset(left.krw.observed_at) == offset(right.krw.observed_at)
            && offset(left.usd.observed_at) == offset(right.usd.observed_at)
            && left.reported.map(|value| offset(value.observed_at()))
                == right.reported.map(|value| offset(value.observed_at()))
            && left
                .holdings
                .iter()
                .zip(&right.holdings)
                .all(|(first, second)| offset(first.observed_at) == offset(second.observed_at))
    }
}
impl Eq for HistoricalPortfolio {}

impl std::fmt::Debug for HistoricalPortfolio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoricalPortfolio")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalPortfolioError {
    MasterMissingOrAmbiguous,
    Master(crate::MasterInstrumentError),
    Holding(HoldingError),
    Portfolio(PortfolioError),
    ChangedFacts,
}

impl HistoricalBuyingPower {
    fn capture(value: BuyingPower) -> Self {
        Self {
            currency: value.currency(),
            money: value.money(),
            observed_at: value.observed_at(),
        }
    }

    fn replay(self) -> Result<BuyingPower, HistoricalPortfolioError> {
        BuyingPower::from_historical(self.currency, self.money, self.observed_at)
            .map_err(HistoricalPortfolioError::Portfolio)
    }
}

impl HistoricalHolding {
    fn capture(value: &Holding) -> Self {
        Self {
            market: value.instrument().market(),
            symbol: value.instrument().symbol().clone(),
            quantity: HistoricalSignalQuantity::new(value.quantity()),
            acquisition_price: value.acquisition_price(),
            current_price: value.current_price(),
            reported: value.reported(),
            observed_at: value.observed_at(),
        }
    }

    fn replay(&self, masters: &[MasterInstrument]) -> Result<Holding, HistoricalPortfolioError> {
        let mut matches = masters.iter().filter(|master| {
            master.row().symbol == self.symbol
                && master
                    .quote_instrument()
                    .is_ok_and(|quote| Market::from(*quote.market()) == self.market)
        });
        let master = matches
            .next()
            .ok_or(HistoricalPortfolioError::MasterMissingOrAmbiguous)?;
        if matches.next().is_some() {
            return Err(HistoricalPortfolioError::MasterMissingOrAmbiguous);
        }
        let instrument = master
            .order_gate_facts()
            .and_then(|facts| facts.instrument_for(crate::OrderSide::Sell))
            .map_err(HistoricalPortfolioError::Master)?;
        Holding::new(
            instrument,
            self.quantity.quantity(),
            self.acquisition_price,
            HoldingObservation::new(self.current_price, self.reported, self.observed_at),
        )
        .map_err(HistoricalPortfolioError::Holding)
    }
}

impl HistoricalPortfolio {
    #[must_use]
    pub fn capture(value: &Portfolio) -> Self {
        let facts = HistoricalPortfolioFacts {
            holdings: value
                .holdings()
                .iter()
                .map(HistoricalHolding::capture)
                .collect(),
            krw: HistoricalBuyingPower::capture(value.buying_power(Currency::Krw)),
            usd: HistoricalBuyingPower::capture(value.buying_power(Currency::Usd)),
            reported: value.reported(),
        };
        Self(facts, value.clone())
    }

    pub fn from_saved_facts(
        facts: HistoricalPortfolioFacts,
        masters: &[MasterInstrument],
    ) -> Result<Self, HistoricalPortfolioError> {
        let holdings = facts
            .holdings
            .iter()
            .map(|value| value.replay(masters))
            .collect::<Result<Vec<_>, _>>()?;
        let mut portfolio = Portfolio::new(holdings, facts.krw.replay()?, facts.usd.replay()?)
            .map_err(HistoricalPortfolioError::Portfolio)?;
        if let Some(reported) = facts.reported {
            portfolio = portfolio.with_reported(reported);
        }
        if Self::capture(&portfolio).0 != facts {
            return Err(HistoricalPortfolioError::ChangedFacts);
        }
        Ok(Self(facts, portfolio))
    }

    #[must_use]
    pub const fn facts(&self) -> &HistoricalPortfolioFacts {
        &self.0
    }

    pub(crate) const fn portfolio(&self) -> &Portfolio {
        &self.1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalSectorAssignment {
    pub market: Market,
    pub symbol: Symbol,
    pub classification: crate::InstrumentClassification,
    pub membership: Option<crate::SectorMembership>,
    pub reason: Option<crate::SectorAssignmentReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalSecurityType {
    pub market: Market,
    pub symbol: Symbol,
    pub security_type: crate::ExcludedSecurityType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalClassifications {
    pub sectors: Vec<HistoricalSectorAssignment>,
    pub security: Vec<HistoricalSecurityType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalClassificationError {
    InvalidMarket,
    DuplicateInstrument,
    MembershipWithoutReason,
}

impl HistoricalClassifications {
    #[must_use]
    pub fn capture(
        assignments: &crate::SectorAssignments,
        security: &crate::PolicySecurityFacts,
    ) -> Self {
        Self {
            sectors: assignments
                .entries()
                .map(|(identity, classification)| HistoricalSectorAssignment {
                    market: identity.market().clone(),
                    symbol: identity.symbol().clone(),
                    classification: classification.clone(),
                    membership: crate::sector_assignments_membership(assignments, identity),
                    reason: crate::sector_assignments_reason(assignments, identity),
                })
                .collect(),
            security: security
                .entries()
                .map(|(identity, security_type)| HistoricalSecurityType {
                    market: identity.market().clone(),
                    symbol: identity.symbol().clone(),
                    security_type: *security_type,
                })
                .collect(),
        }
    }

    pub(crate) fn replay(
        &self,
    ) -> Result<(crate::SectorAssignments, crate::PolicySecurityFacts), HistoricalClassificationError>
    {
        let mut sectors = std::collections::BTreeMap::new();
        let mut details = std::collections::BTreeMap::new();
        for entry in &self.sectors {
            let identity = historical_identity(&entry.market, &entry.symbol)?;
            if sectors
                .insert(identity.clone(), entry.classification.clone())
                .is_some()
            {
                return Err(HistoricalClassificationError::DuplicateInstrument);
            }
            match (entry.membership, entry.reason) {
                (membership, Some(reason)) => {
                    details.insert(
                        identity,
                        crate::sector_selection::AssignmentDetails { membership, reason },
                    );
                }
                (None, None) => (),
                (Some(_), None) => {
                    return Err(HistoricalClassificationError::MembershipWithoutReason);
                }
            }
        }
        let mut security = std::collections::BTreeMap::new();
        for entry in &self.security {
            let identity = historical_identity(&entry.market, &entry.symbol)?;
            if security.insert(identity, entry.security_type).is_some() {
                return Err(HistoricalClassificationError::DuplicateInstrument);
            }
        }
        Ok((
            crate::SectorAssignments(sectors, details),
            crate::PolicySecurityFacts::from_master_lookup_results(security),
        ))
    }
}

fn historical_identity(
    market: &Market,
    symbol: &Symbol,
) -> Result<crate::AllocationInstrument, HistoricalClassificationError> {
    match market {
        Market::Kr | Market::Us => Ok(crate::AllocationInstrument {
            market: market.clone(),
            symbol: symbol.clone(),
        }),
        Market::Unknown(_) => Err(HistoricalClassificationError::InvalidMarket),
    }
}
