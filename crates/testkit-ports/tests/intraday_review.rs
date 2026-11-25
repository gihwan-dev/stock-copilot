#[cfg(test)]
mod tests {
    use application::{
        AiEvent, InformationError, IntradayReviewOutcome, IntradayReviewSubject, IsolationCheck,
        IsolationFailure, ResetTime, RunInput, RunIntradayReview, RunPurpose,
    };
    use domain::{
        DisclosureKey, InstrumentId, KindReceiptNo, Nanosecond, OffsetSeconds, ReceiptNo,
        Timestamp, UnixSeconds, UntrustedText,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiEventStream, ScriptedAiProvider};

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

    fn clock() -> SteppedClock {
        SteppedClock::new(
            Timestamp::from_parts(
                UnixSeconds::new(1_795_995_600),
                Nanosecond::new(0),
                OffsetSeconds::new(32_400),
            )
            .expect("시각"),
        )
    }

    fn samsung() -> IntradayReviewSubject {
        IntradayReviewSubject::PriceMove(InstrumentId::try_new("005930").expect("종목"))
    }

    fn review(
        provider: &ScriptedAiProvider,
        subject: &IntradayReviewSubject,
    ) -> IntradayReviewOutcome {
        run(RunIntradayReview {
            ai: provider,
            clock: &clock(),
        }
        .execute(subject))
    }

    fn scripted(events: Vec<AiEvent>) -> ScriptedAiProvider {
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(events)));
        provider
    }

    fn only_prompt(provider: &ScriptedAiProvider) -> (RunPurpose, String) {
        let calls = provider.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        let Some(AiProviderCall::Run(spec)) = calls.first() else {
            panic!("실행 호출이 아님: {calls:?}");
        };
        let RunInput::Analysis(prompt) = spec.input() else {
            panic!("분석 실행이 아님");
        };
        (spec.purpose(), String::from(prompt.as_str()))
    }

    #[test]
    fn passed_isolation_and_finished_stream_completes_an_intraday_analysis_run() {
        let provider = scripted(vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TextDelta(UntrustedText::try_new("논거 유지").expect("text")),
        ]);
        assert_eq!(
            review(&provider, &samsung()),
            IntradayReviewOutcome::Completed
        );
        let (purpose, prompt) = only_prompt(&provider);
        assert_eq!(purpose, RunPurpose::IntradayMonitoring);
        assert!(
            prompt.contains("<subject kind=\"price-move\">005930</subject>"),
            "{prompt}"
        );
        assert!(prompt.contains("update_thesis"));
        assert!(prompt.contains("submit_estimate"));
    }

    #[test]
    fn disclosure_subject_carries_only_the_receipt_identifier() {
        let provider = scripted(vec![AiEvent::IsolationChecked(IsolationCheck::Passed)]);
        let key = DisclosureKey::from_receipt(ReceiptNo::Kind(
            KindReceiptNo::try_new("20261113000001").expect("접수 번호"),
        ));
        assert_eq!(
            review(&provider, &IntradayReviewSubject::UrgentDisclosure(key)),
            IntradayReviewOutcome::Completed
        );
        let (_purpose, prompt) = only_prompt(&provider);
        assert!(
            prompt.contains("<subject kind=\"disclosure\">20261113000001</subject>"),
            "{prompt}"
        );
    }

    #[test]
    fn rejection_missing_tools_rate_limits_and_provider_failures_are_not_completions() {
        let cases = [
            (
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    IsolationFailure::McpServers,
                ))],
                IntradayReviewOutcome::IsolationRejected,
            ),
            (
                vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::McpUnavailable,
                ],
                IntradayReviewOutcome::ToolsUnavailable,
            ),
            (
                vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::RateLimited {
                        resets_at: ResetTime::NotProvided,
                    },
                ],
                IntradayReviewOutcome::RateLimited,
            ),
            (Vec::new(), IntradayReviewOutcome::ProviderUnavailable),
        ];
        for (events, expected) in cases {
            assert_eq!(review(&scripted(events), &samsung()), expected);
        }
        let failing = ScriptedAiProvider::default();
        failing
            .script_run()
            .push(Err(InformationError::Unavailable(ResetTime::NotProvided)));
        assert_eq!(
            review(&failing, &samsung()),
            IntradayReviewOutcome::ProviderUnavailable
        );
    }
}
