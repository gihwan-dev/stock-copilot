use crate::{Script, script::State};
use application::identity_confirmation_prompt::IdentityConfirmationPrompt;
use application::{
    IdentifierChangePrompt, IdentityConfirmationDecision, IdentityConfirmationOutcome,
    LinkCorrectionPrompt, RetirementCorrectionPrompt,
};
use domain::UserIdentityConfirmation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityConfirmationCall {
    IdentifierChange(Box<IdentifierChangePrompt>),
    RetirementCorrection(RetirementCorrectionPrompt),
    LinkCorrection(LinkCorrectionPrompt),
}

#[derive(Debug)]
pub struct ScriptedIdentityConfirmation {
    identifier_changes: Script<IdentifierChangePrompt, IdentityConfirmationDecision>,
    retirement_corrections: Script<RetirementCorrectionPrompt, IdentityConfirmationDecision>,
    link_corrections: Script<LinkCorrectionPrompt, IdentityConfirmationDecision>,
    calls: State<Vec<IdentityConfirmationCall>>,
}

impl ScriptedIdentityConfirmation {
    pub fn new<
        Changes: IntoIterator<Item = IdentityConfirmationDecision>,
        Retirements: IntoIterator<Item = IdentityConfirmationDecision>,
        Links: IntoIterator<Item = IdentityConfirmationDecision>,
    >(
        identifier_changes: Changes,
        retirement_corrections: Retirements,
        link_corrections: Links,
    ) -> Self {
        Self {
            identifier_changes: Script::new(identifier_changes),
            retirement_corrections: Script::new(retirement_corrections),
            link_corrections: Script::new(link_corrections),
            calls: State::default(),
        }
    }

    #[must_use]
    pub fn calls(&self) -> Vec<IdentityConfirmationCall> {
        self.calls.write().clone()
    }

    #[must_use]
    pub fn identifier_change_calls(&self) -> Vec<IdentifierChangePrompt> {
        self.identifier_changes.calls()
    }

    #[must_use]
    pub fn retirement_correction_calls(&self) -> Vec<RetirementCorrectionPrompt> {
        self.retirement_corrections.calls()
    }

    #[must_use]
    pub fn link_correction_calls(&self) -> Vec<LinkCorrectionPrompt> {
        self.link_corrections.calls()
    }
}

impl IdentityConfirmationPrompt for ScriptedIdentityConfirmation {
    async fn confirm_identifier_change(
        &self,
        request: &IdentifierChangePrompt,
    ) -> IdentityConfirmationDecision {
        let recorded = request.clone();
        self.calls
            .write()
            .push(IdentityConfirmationCall::IdentifierChange(Box::new(
                recorded.clone(),
            )));
        self.identifier_changes.run(recorded).await
    }

    async fn confirm_retirement_correction(
        &self,
        request: &RetirementCorrectionPrompt,
    ) -> IdentityConfirmationDecision {
        let recorded = request.clone();
        self.calls
            .write()
            .push(IdentityConfirmationCall::RetirementCorrection(
                recorded.clone(),
            ));
        self.retirement_corrections.run(recorded).await
    }

    async fn confirm_link_correction(
        &self,
        request: &LinkCorrectionPrompt,
    ) -> IdentityConfirmationDecision {
        let recorded = request.clone();
        self.calls
            .write()
            .push(IdentityConfirmationCall::LinkCorrection(recorded.clone()));
        self.link_corrections.run(recorded).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityConfirmationIssuanceKind {
    IdentifierChange,
    RetirementCorrection,
    LinkCorrection,
}

#[derive(Debug)]
pub struct ScriptedIdentityConfirmationIssuer {
    decisions: Script<IdentityConfirmationIssuanceKind, IdentityConfirmationDecision>,
}

impl ScriptedIdentityConfirmationIssuer {
    pub fn new<Decisions: IntoIterator<Item = IdentityConfirmationDecision>>(
        decisions: Decisions,
    ) -> Self {
        Self {
            decisions: Script::new(decisions),
        }
    }

    pub async fn issue(
        &self,
        kind: IdentityConfirmationIssuanceKind,
    ) -> IdentityConfirmationOutcome<UserIdentityConfirmation> {
        match self.decisions.run(kind).await {
            IdentityConfirmationDecision::Confirmed => IdentityConfirmationOutcome::Confirmed(
                UserIdentityConfirmation::issue_after_user_confirmation(),
            ),
            IdentityConfirmationDecision::Denied => IdentityConfirmationOutcome::Denied,
            IdentityConfirmationDecision::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }

    #[must_use]
    pub fn calls(&self) -> Vec<IdentityConfirmationIssuanceKind> {
        self.decisions.calls()
    }
}
