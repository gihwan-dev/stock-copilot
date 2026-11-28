#[cfg(test)]
mod tests {
    use application::{
        AiEvent, BriefingFailure, BriefingOutcome, BriefingSource, DashboardSubject,
        IsolationCheck, RunInput, RunPurpose, research_dashboard_subject,
    };
    use domain::{
        InformationSource, Nanosecond, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiEventStream, ScriptedAiProvider};

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("합성 제공자의 즉시 응답"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시험 시각")
    }
    fn source(title: &str) -> BriefingSource {
        BriefingSource {
            title: UntrustedText::try_new(title).expect("합성 근거"),
            observed_at: at(700),
            origin: InformationSource::Web,
            url: None,
        }
    }
    fn script(ai: &ScriptedAiProvider, body: &str) {
        ai.script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(UntrustedText::try_new(body).expect("합성 본문")),
            ])));
    }

    #[test]
    fn three_independent_runs_receive_only_their_own_dated_evidence_and_one_can_fail() {
        let ai = ScriptedAiProvider::default();
        let kinds = [
            DashboardSubject::Holdings,
            DashboardSubject::Candidate(
                Symbol::try_new("TEST").expect("합성 종목"),
                domain::DisplayInstrumentName::try_new("합성 기업").expect("이름"),
            ),
            DashboardSubject::Economy,
        ];
        let labels = ["합성 보유 비중", "합성 미보유 근거", "합성 업종 등락"];
        let responses = [
            "비중이 높다 [1]",
            "관측 이유만 있다 [1]",
            "업종 변동을 관찰했다 [1]",
        ];
        for ((subject, label), response) in kinds.iter().zip(labels).zip(responses) {
            script(&ai, response);
            let outcome = run(research_dashboard_subject(
                &ai,
                subject,
                &[source(label)],
                at(900),
            ));
            if let DashboardSubject::Candidate(..) = subject {
                assert_eq!(
                    outcome,
                    BriefingOutcome::Failed(BriefingFailure::EmptyResult)
                );
                continue;
            }
            let BriefingOutcome::Completed { sources, .. } = outcome else {
                panic!("독립적인 성공 결과")
            };
            assert_eq!(sources, vec![source(label)]);
        }
        let calls = ai.calls();
        assert_eq!(calls.len(), 3);
        for ((call, label), purpose) in calls.iter().zip(labels).zip([
            RunPurpose::ThesisReview,
            RunPurpose::CandidateDiscovery,
            RunPurpose::DeepResearch,
        ]) {
            let AiProviderCall::Run(spec) = call else {
                panic!("독립 분석 실행")
            };
            assert_eq!(spec.purpose(), purpose);
            let RunInput::Analysis(prompt) = spec.input() else {
                panic!("분석 전용")
            };
            assert!(prompt.as_str().contains(label));
            assert!(prompt.as_str().contains("700"));
            assert!(
                labels
                    .iter()
                    .filter(|other| **other != label)
                    .all(|other| !prompt.as_str().contains(other))
            );
        }
    }

    #[test]
    fn ungrounded_sentences_out_of_range_citations_and_incomplete_candidates_are_rejected() {
        let ai = ScriptedAiProvider::default();
        let candidate = DashboardSubject::Candidate(
            Symbol::try_new("TEST").expect("합성 코드"),
            domain::DisplayInstrumentName::try_new("합성 기업").expect("이름"),
        );
        for invalid in [
            "근거 문장 [1]. 출처 없는 단정.",
            "이유 거래대금 증가.실적 개선이 확실하다 [1]\n위험 변동이 있다 [1]\n확인 조건 재검토 [1]",
            "이유 첫 문장. 출처 없는 단정 [1]\n위험 위험성 [1]\n확인 조건 재검토 [1]",
            "이유 이유가 있다 [1]\n위험 변동이 있다 [1]",
            "이유 이유가 있다 [1]\n위험 [1]\n확인 조건 재검토 [1]",
            "이유 관측했다 [2]\n위험 변동이 있다 [1]\n확인 조건 재검토 [1]",
        ] {
            script(&ai, invalid);
            assert_eq!(
                run(research_dashboard_subject(
                    &ai,
                    &candidate,
                    &[source("근거")],
                    at(900)
                )),
                BriefingOutcome::Failed(BriefingFailure::EmptyResult)
            );
        }
        script(
            &ai,
            "이유: 1.25% 관측했다. [1]\n위험 변동이 있다 [1].\n확인 조건: 실적 재검토 [1]",
        );
        let BriefingOutcome::Completed { sources, .. } = run(research_dashboard_subject(
            &ai,
            &candidate,
            &[source("근거")],
            at(900),
        )) else {
            panic!("근거가 있는 세 항목")
        };
        assert_eq!(sources.len(), 1);
        assert_eq!(
            run(research_dashboard_subject(&ai, &candidate, &[], at(901))),
            BriefingOutcome::Failed(BriefingFailure::EmptyResult)
        );
        assert_eq!(ai.calls().len(), 7);
    }
}
