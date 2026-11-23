use crate::{
    AccountId, Draft, EstimateRestoreError, HistoricalOrderTerms, MasterInstrument,
    MasterInstrumentError, OrderIntent, OrderIntentError, Price, PriceObservation, Proposal,
    ProposalError, ProposalId, ProposalLifetime, ProposalPriceRange, StoredEstimateEvidence,
    TimeError, Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredPriceIdentity {
    Identified,
    Unidentified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredPriceObservation {
    pub price: Price,
    pub observed_at: Timestamp,
    pub identity: StoredPriceIdentity,
}

impl StoredPriceObservation {
    #[must_use]
    pub fn from_price_observation(value: &PriceObservation) -> Self {
        Self {
            price: value.price(),
            observed_at: value.observed_at(),
            identity: if value.instrument_id().is_some() {
                StoredPriceIdentity::Identified
            } else {
                StoredPriceIdentity::Unidentified
            },
        }
    }
    pub(crate) fn replay(
        self,
        current: &MasterInstrument,
        side: crate::OrderSide,
    ) -> Result<PriceObservation, DraftRestoreError> {
        match self.identity {
            StoredPriceIdentity::Identified => current
                .price_observation_for(side, self.price, self.observed_at)
                .map_err(DraftRestoreError::Master),
            StoredPriceIdentity::Unidentified => {
                let instrument = current
                    .order_gate_facts()
                    .and_then(|facts| facts.instrument_for(side))
                    .map_err(DraftRestoreError::Master)?;
                PriceObservation::from_quote_parts(instrument, self.price, self.observed_at)
                    .map_err(DraftRestoreError::Time)
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DraftOriginalFacts {
    pub id: ProposalId,
    pub account: AccountId,
    pub terms: HistoricalOrderTerms,
    pub created_at: Timestamp,
    pub lifetime: ProposalLifetime,
}

impl std::fmt::Debug for DraftOriginalFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftOriginalFacts").finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DraftRestoreEvidence {
    pub estimate: StoredEstimateEvidence,
    pub id: ProposalId,
    pub account: AccountId,
    pub terms: HistoricalOrderTerms,
    pub created_at: Timestamp,
    pub lifetime: ProposalLifetime,
    pub expires_at: Timestamp,
    pub card: StoredPriceObservation,
    pub expected_card: StoredPriceObservation,
    pub original: DraftOriginalFacts,
}

impl std::fmt::Debug for DraftRestoreEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftRestoreEvidence")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftRestoreError {
    Estimate(EstimateRestoreError),
    Master(MasterInstrumentError),
    Time(TimeError),
    Intent(OrderIntentError),
    Proposal(ProposalError),
    ExpiryMismatch,
    ChangedRange,
    ChangedDraft,
}

impl DraftRestoreEvidence {
    pub fn capture(
        original: &Proposal<Draft>,
        estimate: StoredEstimateEvidence,
        current: &MasterInstrument,
        lifetime: ProposalLifetime,
        card: &PriceObservation,
    ) -> Result<Self, DraftRestoreError> {
        let saved = Self {
            estimate,
            id: original.id().clone(),
            account: original.intent().account_id().clone(),
            terms: original.intent().terms(),
            created_at: original.created_at(),
            lifetime,
            expires_at: original.expires_at(),
            card: StoredPriceObservation::from_price_observation(card),
            expected_card: StoredPriceObservation::from_price_observation(
                original.price_range().card(),
            ),
            original: DraftOriginalFacts {
                id: original.id().clone(),
                account: original.intent().account_id().clone(),
                terms: original.intent().terms(),
                created_at: original.created_at(),
                lifetime,
            },
        };
        let rebuilt = saved.restore(current)?;
        if rebuilt.intent() != original.intent() || rebuilt.price_range() != original.price_range()
        {
            return Err(DraftRestoreError::ChangedDraft);
        }
        Ok(saved)
    }

    pub fn restore(
        &self,
        current: &MasterInstrument,
    ) -> Result<Proposal<Draft>, DraftRestoreError> {
        if self.id != self.original.id
            || self.account != self.original.account
            || self.terms != self.original.terms
            || self.created_at != self.original.created_at
            || self.created_at.offset_seconds() != self.original.created_at.offset_seconds()
            || self.lifetime != self.original.lifetime
        {
            return Err(DraftRestoreError::ChangedDraft);
        }
        if self.card != self.expected_card
            || self.card.observed_at.offset_seconds()
                != self.expected_card.observed_at.offset_seconds()
        {
            return Err(DraftRestoreError::ChangedRange);
        }
        let task = self
            .estimate
            .replay(current)
            .map_err(DraftRestoreError::Estimate)?;
        let side = self.terms.side();
        let card = self.card.replay(current, side)?;
        let instrument = card.instrument().clone();
        let range = ProposalPriceRange::from_estimate_task(task.range_facts(), card)
            .map_err(DraftRestoreError::Proposal)?;
        let intent = OrderIntent::new(
            self.account.clone(),
            instrument,
            self.id.as_client_order_id().clone(),
            self.terms.into_order_terms(),
        )
        .map_err(DraftRestoreError::Intent)?;
        let draft = Proposal::new(
            self.id.clone(),
            intent,
            self.created_at,
            range,
            self.lifetime,
        )
        .map_err(DraftRestoreError::Proposal)?;
        if draft.expires_at() != self.expires_at
            || draft.expires_at().offset_seconds() != self.expires_at.offset_seconds()
        {
            return Err(DraftRestoreError::ExpiryMismatch);
        }
        Ok(draft)
    }
}
