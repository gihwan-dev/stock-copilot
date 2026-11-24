pub(crate) use crate::estimates::codec::Encoder;
use crate::estimates::codec::{put_appetite, put_ratio, put_timestamp};
use application::{
    AuditKind, OrderConfirmation, RiskChangeConfirmation, WebSearchAuditOutcome,
    WebSearchAuditRecord, WebSearchQueryMode,
};
use domain::{RiskChange, Timestamp, WebSearchRejectionReason};

pub(crate) fn encode_timestamp(at: Timestamp) -> String {
    let mut out = Encoder::new();
    put_timestamp(&mut out, at);
    out.finish()
}

const fn order_confirmation(outcome: OrderConfirmation) -> &'static str {
    match outcome {
        OrderConfirmation::Confirmed => "confirmed",
        OrderConfirmation::Denied => "denied",
        OrderConfirmation::Unavailable => "unavailable",
    }
}

const fn risk_confirmation(outcome: RiskChangeConfirmation) -> &'static str {
    match outcome {
        RiskChangeConfirmation::Confirmed => "confirmed",
        RiskChangeConfirmation::Denied => "denied",
        RiskChangeConfirmation::Unavailable => "unavailable",
    }
}

fn put_risk_change(out: &mut Encoder, change: &RiskChange) {
    match change {
        RiskChange::EnableOrders => out.push("enable_orders"),
        RiskChange::AllowMarketOrders => out.push("allow_market_orders"),
        RiskChange::AllowHours => out.push("allow_hours"),
        RiskChange::RelaxWeightCaps(caps) => {
            out.push("relax_weight_caps");
            put_ratio(out, caps.instrument_cap().as_ratio());
            put_ratio(out, caps.instrument_warning().as_ratio());
            put_ratio(out, caps.sector_cap().as_ratio());
        }
        RiskChange::IncreaseRiskAppetite(appetite) => {
            out.push("increase_risk_appetite");
            put_appetite(out, *appetite);
        }
    }
}

const fn rejection_reason(reason: WebSearchRejectionReason) -> &'static str {
    match reason {
        WebSearchRejectionReason::EmptyQuery => "empty_query",
        WebSearchRejectionReason::QueryTooLong => "query_too_long",
        WebSearchRejectionReason::ControlOrInvisibleCharacter => "control_or_invisible_character",
        WebSearchRejectionReason::EncodedOrObfuscatedText => "encoded_or_obfuscated_text",
        WebSearchRejectionReason::UrlOrDomainShape => "url_or_domain_shape",
        WebSearchRejectionReason::MultiDigitNumber => "multi_digit_number",
        WebSearchRejectionReason::SpelledOutNumber => "spelled_out_number",
        WebSearchRejectionReason::MoneyQuantityOrAccountTerm => "money_quantity_or_account_term",
        WebSearchRejectionReason::SensitiveContextMatch => "sensitive_context_match",
    }
}

fn put_web_search(out: &mut Encoder, record: &WebSearchAuditRecord) {
    out.push(match record.mode() {
        WebSearchQueryMode::CodeGenerated => "code_generated",
        WebSearchQueryMode::UserDirectMessage => "user_direct_message",
    });
    out.push(record.digest().as_str());
    match record.outcome() {
        WebSearchAuditOutcome::Approved => out.push("approved"),
        WebSearchAuditOutcome::BypassedForUserMessage => out.push("bypassed_for_user_message"),
        WebSearchAuditOutcome::Rejected(reasons) => {
            out.push("rejected");
            out.push_usize(reasons.len());
            for reason in reasons {
                out.push(rejection_reason(*reason));
            }
        }
    }
}

pub(crate) fn encode_kind(kind: &AuditKind) -> (&'static str, String) {
    let mut out = Encoder::new();
    let tag = match kind {
        AuditKind::EstimateSubmitted(instrument) => {
            out.push(instrument.as_str());
            "estimate_submitted"
        }
        AuditKind::SignalRaised(instrument) => {
            out.push(instrument.as_str());
            "signal_raised"
        }
        AuditKind::ValidationRecorded(proposal) => {
            out.push(proposal.as_client_order_id().as_str());
            "validation_recorded"
        }
        AuditKind::ApprovalRequested(proposal) => {
            out.push(proposal.as_client_order_id().as_str());
            "approval_requested"
        }
        AuditKind::PresenceRecorded { proposal, outcome } => {
            out.push(proposal.as_client_order_id().as_str());
            out.push(order_confirmation(*outcome));
            "presence_recorded"
        }
        AuditKind::ProposalSkipped(proposal) => {
            out.push(proposal.as_client_order_id().as_str());
            "proposal_skipped"
        }
        AuditKind::SubmissionPrepared(order) => {
            out.push(order.as_str());
            "submission_prepared"
        }
        AuditKind::SubmissionResponseRecorded(order) => {
            out.push(order.as_str());
            "submission_response_recorded"
        }
        AuditKind::RiskSettingsChanged(change) => {
            put_risk_change(&mut out, change);
            "risk_settings_changed"
        }
        AuditKind::RiskChangeConfirmationRecorded { change, outcome } => {
            put_risk_change(&mut out, change);
            out.push(risk_confirmation(*outcome));
            "risk_change_confirmation_recorded"
        }
        AuditKind::WebSearchChecked(record) => {
            put_web_search(&mut out, record);
            "web_search_checked"
        }
    };
    (tag, out.finish())
}
