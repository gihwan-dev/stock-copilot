use domain::{IdentityChangeEvidence, InstrumentId, InstrumentSource, MappingCorrectionEvidence};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityConfirmationDecision {
    Confirmed,
    Denied,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierChangePrompt {
    instrument_id: InstrumentId,
    previous: InstrumentSource,
    next: InstrumentSource,
    evidence: IdentityChangeEvidence,
}

impl IdentifierChangePrompt {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        previous: InstrumentSource,
        next: InstrumentSource,
        evidence: IdentityChangeEvidence,
    ) -> Self {
        Self {
            instrument_id,
            previous,
            next,
            evidence,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn previous(&self) -> &InstrumentSource {
        &self.previous
    }

    #[must_use]
    pub const fn next(&self) -> &InstrumentSource {
        &self.next
    }

    #[must_use]
    pub const fn evidence(&self) -> &IdentityChangeEvidence {
        &self.evidence
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        InstrumentId,
        InstrumentSource,
        InstrumentSource,
        IdentityChangeEvidence,
    ) {
        (self.instrument_id, self.previous, self.next, self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetirementCorrectionPrompt {
    instrument_id: InstrumentId,
    source: InstrumentSource,
    evidence: MappingCorrectionEvidence,
}

impl RetirementCorrectionPrompt {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        source: InstrumentSource,
        evidence: MappingCorrectionEvidence,
    ) -> Self {
        Self {
            instrument_id,
            source,
            evidence,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn source(&self) -> &InstrumentSource {
        &self.source
    }

    #[must_use]
    pub const fn evidence(&self) -> &MappingCorrectionEvidence {
        &self.evidence
    }

    pub(crate) fn into_parts(self) -> (InstrumentId, InstrumentSource, MappingCorrectionEvidence) {
        (self.instrument_id, self.source, self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCorrectionPrompt {
    instrument_id: InstrumentId,
    previous: InstrumentSource,
    mistaken_next: InstrumentSource,
    evidence: MappingCorrectionEvidence,
}

impl LinkCorrectionPrompt {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        previous: InstrumentSource,
        mistaken_next: InstrumentSource,
        evidence: MappingCorrectionEvidence,
    ) -> Self {
        Self {
            instrument_id,
            previous,
            mistaken_next,
            evidence,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn previous(&self) -> &InstrumentSource {
        &self.previous
    }

    #[must_use]
    pub const fn mistaken_next(&self) -> &InstrumentSource {
        &self.mistaken_next
    }

    #[must_use]
    pub const fn evidence(&self) -> &MappingCorrectionEvidence {
        &self.evidence
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        InstrumentId,
        InstrumentSource,
        InstrumentSource,
        MappingCorrectionEvidence,
    ) {
        (
            self.instrument_id,
            self.previous,
            self.mistaken_next,
            self.evidence,
        )
    }
}

pub trait IdentityConfirmationPrompt: Send + Sync {
    fn confirm_identifier_change(
        &self,
        request: &IdentifierChangePrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;

    fn confirm_retirement_correction(
        &self,
        request: &RetirementCorrectionPrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;

    fn confirm_link_correction(
        &self,
        request: &LinkCorrectionPrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;
}

#[cfg(test)]
mod tests {
    use super::{IdentifierChangePrompt, LinkCorrectionPrompt, RetirementCorrectionPrompt};
    use domain::{
        CalendarDate, CalendarYear, CorporateAction, CorporateActionId, CorporateActionKind,
        DayNumber, Disclosure, DisclosureKey, IdentityChangeEvidence, InstrumentId,
        InstrumentSource, IsinCode, KnownMarket, MappingCorrectionEvidence, MonthNumber,
        NormalizedUrl, ReceiptNo, SecEdgarReceiptNo, Symbol, UntrustedText, UserReason,
    };

    fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("심볼"),
            IsinCode::try_new(isin).expect("ISIN"),
        )
    }

    fn disclosure() -> DisclosureKey {
        Disclosure::new(
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new("0001326801-26-000010").expect("SEC")),
            NormalizedUrl::try_new("https://www.sec.gov/Archives/edgar/data/1/index.htm")
                .expect("URL"),
            UntrustedText::try_new("Symbol change filing").expect("제목"),
            None,
        )
        .key()
        .clone()
    }

    fn reason(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("문구")).expect("사유")
    }

    #[test]
    fn prompt_requests_preserve_every_field_through_accessors_and_into_parts() {
        let id = InstrumentId::try_new("inst-0000000000000001").expect("종목 ID");
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let action = CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(20),
            )
            .expect("날짜"),
            CorporateActionKind::SymbolChange {
                from: Symbol::try_new("FB").expect("이전 심볼"),
                to: Symbol::try_new("META").expect("이후 심볼"),
            },
        )
        .expect("기업행위");
        let change_evidence = IdentityChangeEvidence::DisclosureAndAction {
            disclosure: disclosure(),
            action,
            note: reason("종목 코드 변경 확인"),
        };
        let change = IdentifierChangePrompt::new(
            id.clone(),
            previous.clone(),
            next.clone(),
            change_evidence.clone(),
        );
        assert_eq!(change.instrument_id(), &id);
        assert_eq!(change.previous(), &previous);
        assert_eq!(change.next(), &next);
        assert_eq!(change.evidence(), &change_evidence);
        assert_eq!(
            change.into_parts(),
            (id.clone(), previous.clone(), next.clone(), change_evidence)
        );

        let corr_evidence = MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: disclosure(),
            note: reason("오기록 정정 확인"),
        };
        let retirement =
            RetirementCorrectionPrompt::new(id.clone(), previous.clone(), corr_evidence.clone());
        assert_eq!(retirement.instrument_id(), &id);
        assert_eq!(retirement.source(), &previous);
        assert_eq!(retirement.evidence(), &corr_evidence);
        assert_eq!(
            retirement.into_parts(),
            (id.clone(), previous.clone(), corr_evidence.clone())
        );

        let link = LinkCorrectionPrompt::new(
            id.clone(),
            previous.clone(),
            next.clone(),
            corr_evidence.clone(),
        );
        assert_eq!(link.instrument_id(), &id);
        assert_eq!(link.previous(), &previous);
        assert_eq!(link.mistaken_next(), &next);
        assert_eq!(link.evidence(), &corr_evidence);
        assert_eq!(link.into_parts(), (id, previous, next, corr_evidence));
    }
}
