use crate::InstrumentId;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuerSequenceError {
    Zero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IssuerSequence(u64);

impl IssuerSequence {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn try_new(value: u64) -> Result<Self, IssuerSequenceError> {
        if value == 0 {
            Err(IssuerSequenceError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedIssuerCursor {
    Initial,
    LastIssued(IssuerSequence),
}

#[derive(Debug, PartialEq, Eq)]
pub struct IssuedInstrumentId(InstrumentId);

impl IssuedInstrumentId {
    #[must_use]
    pub const fn as_instrument_id(&self) -> &InstrumentId {
        &self.0
    }

    pub(crate) fn into_instrument_id(self) -> InstrumentId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentIdIssueError {
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdAdoption {
    Adopted,
    AlreadyKnown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationStatus {
    Reserved,
    Available,
}

#[derive(Debug, PartialEq, Eq)]
pub struct IndependentInstrumentIdIssuer {
    next: Option<IssuerSequence>,
    reserved: BTreeSet<InstrumentId>,
}

impl Default for IndependentInstrumentIdIssuer {
    fn default() -> Self {
        Self::new()
    }
}

impl IndependentInstrumentIdIssuer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: Some(IssuerSequence::new(1)),
            reserved: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn from_existing(existing: &[InstrumentId]) -> Self {
        Self::restore(PersistedIssuerCursor::Initial, existing)
    }

    #[must_use]
    pub fn restore(cursor: PersistedIssuerCursor, existing: &[InstrumentId]) -> Self {
        let next = match cursor {
            PersistedIssuerCursor::Initial => Some(IssuerSequence::new(1)),
            PersistedIssuerCursor::LastIssued(last) => last.next(),
        };
        let mut issuer = Self {
            next,
            reserved: BTreeSet::new(),
        };
        for id in existing {
            let _adopted = issuer.adopt_existing(id.clone());
        }
        issuer
    }

    #[must_use]
    pub const fn last_issued(&self) -> PersistedIssuerCursor {
        match self.next {
            Some(next_seq) => match IssuerSequence::try_new(next_seq.0.saturating_sub(1)) {
                Ok(previous) => PersistedIssuerCursor::LastIssued(previous),
                Err(IssuerSequenceError::Zero) => PersistedIssuerCursor::Initial,
            },
            None => PersistedIssuerCursor::LastIssued(IssuerSequence::new(u64::MAX)),
        }
    }

    pub fn adopt_existing(&mut self, id: InstrumentId) -> IdAdoption {
        if self.reserved.insert(id) {
            IdAdoption::Adopted
        } else {
            IdAdoption::AlreadyKnown
        }
    }

    #[must_use]
    pub fn reservation(&self, id: &InstrumentId) -> ReservationStatus {
        if self.reserved.contains(id) {
            ReservationStatus::Reserved
        } else {
            ReservationStatus::Available
        }
    }

    pub fn issue(&mut self) -> Result<IssuedInstrumentId, InstrumentIdIssueError> {
        while let Some(sequence) = self.next {
            self.next = sequence.next();
            let candidate = InstrumentId::from_issued_sequence(sequence);
            if self.reserved.insert(candidate.clone()) {
                return Ok(IssuedInstrumentId(candidate));
            }
        }
        Err(InstrumentIdIssueError::Exhausted)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IdAdoption, IndependentInstrumentIdIssuer, InstrumentIdIssueError, IssuerSequence,
        ReservationStatus,
    };
    use crate::InstrumentId;

    #[test]
    fn default_issuer_starts_at_first_opaque_sequence_and_exhausts_at_max() {
        let mut fresh = IndependentInstrumentIdIssuer::default();
        let first = fresh.issue().expect("첫 발급");
        assert_eq!(first.as_instrument_id().as_str(), "inst-0000000000000001");
        assert_eq!(
            fresh.reservation(first.as_instrument_id()),
            ReservationStatus::Reserved
        );
        assert_eq!(first.into_instrument_id().as_str(), "inst-0000000000000001");

        let mut at_max = IndependentInstrumentIdIssuer {
            next: Some(IssuerSequence::new(u64::MAX)),
            reserved: std::collections::BTreeSet::new(),
        };
        let last = at_max.issue().expect("마지막 순번 발급");
        assert_eq!(last.as_instrument_id().as_str(), "inst-ffffffffffffffff");
        assert_eq!(at_max.issue(), Err(InstrumentIdIssueError::Exhausted));

        let max_id = InstrumentId::try_new("inst-ffffffffffffffff").expect("최대 ID");
        let mut blocked = IndependentInstrumentIdIssuer {
            next: Some(IssuerSequence::new(u64::MAX)),
            reserved: std::collections::BTreeSet::new(),
        };
        assert_eq!(blocked.adopt_existing(max_id), IdAdoption::Adopted);
        assert_eq!(blocked.issue(), Err(InstrumentIdIssueError::Exhausted));
    }
}
