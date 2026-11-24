#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AnalysisFailure, ClaimVerifier, DraftResearchCases, ModelName, ModelSelection,
        ResearchAnalyst, ResearchCaseError, ResearchCases, ResearchModels, VerifyResearchClaims,
    };
    use domain::{
        ClaimCheckBasis, ClaimSupport, EvidenceBundle, EvidenceItem, EvidenceItemId, EvidenceKind,
        InstrumentId, Nanosecond, OffsetSeconds, ResearchClaim, ResearchClaimText, ResearchDraft,
        ResearchStance, Timestamp, UnixSeconds, UntrustedText,
    };
    use testkit_ports::Script;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn id(value: &str) -> EvidenceItemId {
        EvidenceItemId::try_new(value).expect("ID")
    }
    fn ledger() -> EvidenceBundle {
        let at = Timestamp::from_parts(
            UnixSeconds::new(10),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        EvidenceBundle::try_new(
            InstrumentId::try_new("A005930").expect("종목"),
            ["quote-1", "web-1"]
                .iter()
                .map(|value| {
                    EvidenceItem::new(
                        id(value),
                        EvidenceKind::Quote,
                        at,
                        UntrustedText::try_new(value).expect("내용"),
                    )
                })
                .collect(),
        )
        .expect("묶음")
    }
    fn claim(text: &str, cites: &[&str]) -> ResearchClaim {
        ResearchClaim::try_new(
            ResearchClaimText::try_new(text).expect("문장"),
            cites.iter().map(|value| id(value)).collect(),
        )
        .expect("주장")
    }
    fn draft(stance: ResearchStance, claims: Vec<ResearchClaim>) -> ResearchDraft {
        ResearchDraft::try_new(stance, claims).expect("초안")
    }
    fn models() -> ResearchModels {
        ResearchModels {
            bull: ModelSelection::Highest,
            bear: ModelSelection::Named(ModelName::try_new("other-model").expect("모델")),
        }
    }

    type DraftReply = Result<ResearchDraft, AnalysisFailure>;
    #[derive(Default)]
    struct Analyst(Script<(ResearchStance, ModelSelection, EvidenceBundle), DraftReply>);
    impl ResearchAnalyst for Analyst {
        async fn draft(
            &self,
            stance: ResearchStance,
            model: &ModelSelection,
            ledger: &EvidenceBundle,
        ) -> DraftReply {
            self.0.run((stance, model.clone(), ledger.clone())).await
        }
    }
    type VerifyReply = Result<ClaimSupport, AnalysisFailure>;
    #[derive(Default)]
    struct Verifier(Script<(ResearchClaim, Vec<EvidenceItem>), VerifyReply>);
    impl ClaimVerifier for Verifier {
        async fn verify(&self, claim: &ResearchClaim, evidence: &[EvidenceItem]) -> VerifyReply {
            self.0.run((claim.clone(), evidence.to_vec())).await
        }
    }

    #[test]
    fn bull_and_bear_cases_see_the_same_ledger_and_never_each_other() {
        let analyst = Analyst::default();
        let bull = draft(ResearchStance::Bull, vec![claim("수요 증가", &["quote-1"])]);
        let bear = draft(ResearchStance::Bear, vec![claim("재고 부담", &["web-1"])]);
        analyst.0.push(Ok(bull.clone()));
        analyst.0.push(Ok(bear.clone()));
        let chosen = models();
        let cases = run(DraftResearchCases {
            analyst: &analyst,
            models: &chosen,
        }
        .execute(&ledger()))
        .expect("초안");
        assert_eq!(cases, ResearchCases { bull, bear });
        assert_eq!(
            analyst.0.calls(),
            vec![
                (ResearchStance::Bull, chosen.bull.clone(), ledger()),
                (ResearchStance::Bear, chosen.bear, ledger()),
            ]
        );
    }

    #[test]
    fn wrong_stance_and_interrupted_analysis_stop_drafting() {
        let analyst = Analyst::default();
        analyst.0.push(Ok(draft(
            ResearchStance::Bear,
            vec![claim("주장", &["quote-1"])],
        )));
        let chosen = models();
        assert_eq!(
            run(DraftResearchCases {
                analyst: &analyst,
                models: &chosen,
            }
            .execute(&ledger())),
            Err(ResearchCaseError::StanceMismatch)
        );
        let paused = Analyst::default();
        paused.0.push(Err(AnalysisFailure::Interrupted));
        assert_eq!(
            run(DraftResearchCases {
                analyst: &paused,
                models: &chosen,
            }
            .execute(&ledger())),
            Err(ResearchCaseError::Analysis(AnalysisFailure::Interrupted))
        );
        assert_eq!(paused.0.calls().len(), 1);
    }

    #[test]
    fn each_claim_is_verified_alone_against_only_its_cited_ledger_items() {
        let cases = ResearchCases {
            bull: draft(
                ResearchStance::Bull,
                vec![
                    claim("수요 증가", &["quote-1"]),
                    claim("없는 근거", &["quote-1", "x-9"]),
                ],
            ),
            bear: draft(ResearchStance::Bear, vec![claim("재고 부담", &["web-1"])]),
        };
        let verifier = Verifier::default();
        verifier.0.push(Ok(ClaimSupport::Supported));
        verifier.0.push(Ok(ClaimSupport::Contradicted));
        let checked = run(VerifyResearchClaims {
            verifier: &verifier,
        }
        .execute(&ledger(), &cases))
        .expect("검증");
        assert_eq!(
            checked
                .iter()
                .map(|item| (item.stance(), item.support(), item.basis()))
                .collect::<Vec<_>>(),
            vec![
                (
                    ResearchStance::Bull,
                    ClaimSupport::Supported,
                    ClaimCheckBasis::Verifier
                ),
                (
                    ResearchStance::Bull,
                    ClaimSupport::Unsupported,
                    ClaimCheckBasis::MissingEvidence
                ),
                (
                    ResearchStance::Bear,
                    ClaimSupport::Contradicted,
                    ClaimCheckBasis::Verifier
                ),
            ]
        );
        let calls = verifier.0.calls();
        assert_eq!(calls.len(), 2);
        let (first_claim, first_evidence) = calls.first().expect("첫 검증");
        assert_eq!(first_claim, &claim("수요 증가", &["quote-1"]));
        assert_eq!(
            first_evidence
                .iter()
                .map(|item| item.id().as_str())
                .collect::<Vec<_>>(),
            vec!["quote-1"]
        );
        let (_, second_evidence) = calls.get(1).expect("둘째 검증");
        assert_eq!(
            second_evidence
                .iter()
                .map(|item| item.id().as_str())
                .collect::<Vec<_>>(),
            vec!["web-1"]
        );
    }

    #[test]
    fn verifier_failure_stops_verification() {
        let cases = ResearchCases {
            bull: draft(ResearchStance::Bull, vec![claim("주장", &["quote-1"])]),
            bear: draft(ResearchStance::Bear, vec![claim("반론", &["web-1"])]),
        };
        let verifier = Verifier::default();
        verifier.0.push(Err(AnalysisFailure::Unavailable));
        assert_eq!(
            run(VerifyResearchClaims {
                verifier: &verifier,
            }
            .execute(&ledger(), &cases)),
            Err(AnalysisFailure::Unavailable)
        );
        assert_eq!(verifier.0.calls().len(), 1);
    }
}
