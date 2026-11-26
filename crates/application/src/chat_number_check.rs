mod derived;

use std::collections::BTreeSet;
use std::future::Future;

use crate::{AiRunId, SystemPortError, ToolPayloadJson};
use domain::UntrustedText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatNumberCheck {
    NoNumbers,
    Verified,
    Computed,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatNumberEvidenceSet {
    NotConfigured,
    Payloads(Vec<ToolPayloadJson>),
}

pub trait ChatNumberEvidence: Send + Sync {
    fn evidence_for_run(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ChatNumberEvidenceSet, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoNumberEvidence;

impl ChatNumberEvidence for NoNumberEvidence {
    fn evidence_for_run(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<ChatNumberEvidenceSet, SystemPortError>> + Send {
        std::future::ready(Ok(ChatNumberEvidenceSet::NotConfigured))
    }
}

pub(crate) static NO_NUMBER_EVIDENCE: NoNumberEvidence = NoNumberEvidence;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AnswerNumber(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberRelevance {
    Checked,
    Trivial,
}

impl AnswerNumber {
    fn from_token(token: &NumberToken) -> Self {
        let digits: String = token.0.chars().filter(|ch| *ch != ',').collect();
        let (whole, fraction) = match digits.split_once('.') {
            Some((whole, fraction)) => {
                (whole.to_owned(), fraction.trim_end_matches('0').to_owned())
            }
            None => (digits.clone(), String::new()),
        };
        let trimmed_whole = whole.trim_start_matches('0');
        let whole_part = if trimmed_whole.is_empty() {
            "0"
        } else {
            trimmed_whole
        };
        if fraction.is_empty() {
            Self(whole_part.to_owned())
        } else {
            Self(format!("{whole_part}.{fraction}"))
        }
    }

    fn relevance(&self) -> NumberRelevance {
        if self.0.len() == 1 {
            NumberRelevance::Trivial
        } else {
            NumberRelevance::Checked
        }
    }
}

struct NumberToken(String);

struct NumberText(String);

impl NumberText {
    fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    fn tokens(&self) -> Vec<NumberToken> {
        let mut chars = self.0.chars().peekable();
        let mut found = Vec::new();
        let mut current = String::new();
        let mut seen_dot = false;
        while let Some(ch) = chars.next() {
            let next_is_digit = chars.peek().is_some_and(char::is_ascii_digit);
            if ch.is_ascii_digit() {
                current.push(ch);
            } else if !current.is_empty() && ch == ',' && next_is_digit && !seen_dot {
                current.push(',');
            } else if !current.is_empty() && ch == '.' && next_is_digit && !seen_dot {
                seen_dot = true;
                current.push('.');
            } else if !current.is_empty() {
                found.push(NumberToken(current.clone()));
                current.clear();
                seen_dot = false;
            }
        }
        if !current.is_empty() {
            found.push(NumberToken(current));
        }
        found
    }

    fn numbers(&self) -> Vec<AnswerNumber> {
        self.tokens().iter().map(AnswerNumber::from_token).collect()
    }
}

#[must_use]
pub fn check_chat_numbers(answer: &UntrustedText, evidence: &[ToolPayloadJson]) -> ChatNumberCheck {
    let claims = derived::claims_in(answer);
    if claims.is_empty() {
        return ChatNumberCheck::NoNumbers;
    }
    let known: BTreeSet<AnswerNumber> = evidence
        .iter()
        .flat_map(|payload| NumberText::new(payload.as_str()).numbers())
        .collect();
    let unmatched: Vec<&derived::Claim> = claims
        .iter()
        .filter(|claim| !known.contains(&claim.number))
        .collect();
    if unmatched.is_empty() {
        return ChatNumberCheck::Verified;
    }
    let book = derived::DerivationBook::new(&known);
    if unmatched
        .iter()
        .all(|claim| book.classify(claim) == derived::Grounding::Derived)
    {
        ChatNumberCheck::Computed
    } else {
        ChatNumberCheck::Unverified
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatNumberCheck, check_chat_numbers};
    use crate::ToolPayloadJson;
    use domain::UntrustedText;

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("글")
    }

    fn payload(raw: &str) -> ToolPayloadJson {
        ToolPayloadJson::try_new(raw).expect("원장")
    }

    #[test]
    fn numbers_matching_the_ledger_are_verified_after_normalizing() {
        let evidence = [payload(r#"{"price":"71200.00","weight":"0.125","qty":12}"#)];
        assert_eq!(
            check_chat_numbers(&text("현재가 71,200원, 비중 0.125, 12주"), &evidence),
            ChatNumberCheck::Verified
        );
    }

    #[test]
    fn a_number_missing_from_the_ledger_is_unverified() {
        let evidence = [payload(r#"{"price":"71200"}"#)];
        assert_eq!(
            check_chat_numbers(&text("현재가 71,200원, 목표 90,000원"), &evidence),
            ChatNumberCheck::Unverified
        );
        assert_eq!(
            check_chat_numbers(&text("현재가 71,200원"), &[]),
            ChatNumberCheck::Unverified
        );
    }

    #[test]
    fn answers_without_meaningful_numbers_are_not_checked() {
        assert_eq!(
            check_chat_numbers(&text("1. 분산을 늘리세요. 2. 현금을 두세요."), &[]),
            ChatNumberCheck::NoNumbers
        );
    }

    #[test]
    fn sentence_punctuation_does_not_join_numbers() {
        let evidence = [payload(r#"{"a":"12","b":"34"}"#)];
        assert_eq!(
            check_chat_numbers(&text("값은 12. 그리고 34, 끝."), &evidence),
            ChatNumberCheck::Verified
        );
    }
}

#[cfg(test)]
mod computed_tests {
    use super::{ChatNumberCheck, check_chat_numbers};
    use crate::ToolPayloadJson;
    use domain::UntrustedText;

    fn check(answer: &str, ledger: &str) -> ChatNumberCheck {
        check_chat_numbers(
            &UntrustedText::try_new(answer).expect("글"),
            &[ToolPayloadJson::try_new(ledger).expect("원장")],
        )
    }

    const HOLDING: &str =
        r#"{"price":"72000","quantity":"250","weight":"0.376","total":"15360000"}"#;

    #[test]
    fn percentages_and_products_of_ledger_numbers_are_computed_not_unverified() {
        assert_eq!(check("비중 37.6%", HOLDING), ChatNumberCheck::Computed);
        assert_eq!(
            check("평가금액 18,000,000원", HOLDING),
            ChatNumberCheck::Computed
        );
        assert_eq!(check("현금 1,536만 원", HOLDING), ChatNumberCheck::Computed);
    }

    #[test]
    fn approximate_suggestions_are_computed() {
        assert_eq!(
            check("약 135주를 팔면 약 970만 원이 생깁니다", HOLDING),
            ChatNumberCheck::Computed
        );
        assert_eq!(
            check("회당 200만 원 정도씩 넣으세요", HOLDING),
            ChatNumberCheck::Computed
        );
    }

    #[test]
    fn a_wrong_fact_stays_unverified_even_when_hedged() {
        assert_eq!(
            check("현재가 약 75,000원", HOLDING),
            ChatNumberCheck::Unverified
        );
        assert_eq!(check("보유 300주", HOLDING), ChatNumberCheck::Unverified);
        assert_eq!(
            check("현재가 약 72,000원", r#"{"price":"71800"}"#),
            ChatNumberCheck::Computed
        );
    }

    #[test]
    fn a_plain_number_with_no_source_is_unverified() {
        assert_eq!(check("135주 매도", HOLDING), ChatNumberCheck::Unverified);
    }

    #[test]
    fn exact_ledger_numbers_stay_verified() {
        assert_eq!(
            check("현재가 72,000원, 250주", HOLDING),
            ChatNumberCheck::Verified
        );
    }
}
