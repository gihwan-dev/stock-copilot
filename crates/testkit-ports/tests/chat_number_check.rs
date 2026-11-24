#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiRunId, ChatModelConfig, ChatNumberCheck, ChatSessionId, ChatTurnId,
        ChatTurnIdentity, IsolationCheck, ModelSelection, ReasoningEffort, RetryDelay,
        SendChatMessage, SendChatOutcome, SendChatRequest, SystemFailure, SystemPort,
        SystemPortError, ToolPayloadJson, UserMessageId,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds, UntrustedText};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        chat_fakes::{
            CollectingChatSink, MemoryChatRepository, MemoryChatRunLedger, MemoryNumberEvidence,
            ScriptedAiEventStream, ScriptedAiProvider, ScriptedChatQueue, ScriptedStopSignal,
        },
    };

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

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("비신뢰 텍스트")
    }

    fn run_id() -> AiRunId {
        AiRunId::try_new("run-n1").expect("실행 ID")
    }

    fn request() -> SendChatRequest {
        SendChatRequest::new(
            ChatTurnIdentity {
                session: ChatSessionId::try_new("session-n").expect("세션"),
                turn: ChatTurnId::try_new("turn-n1").expect("턴"),
                run: run_id(),
            },
            UserMessageId::try_new("msg-n1").expect("메시지"),
            text("삼성전자 지금 얼마야?"),
            None,
            ChatModelConfig {
                model: ModelSelection::Default,
                effort: ReasoningEffort::Medium,
            },
        )
    }

    fn answer_with(
        answer: &str,
        evidence: &MemoryNumberEvidence,
        status: ChatTurnStatusLabel,
    ) -> SendChatOutcome {
        let provider = ScriptedAiProvider::default();
        let mut events = vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TextDelta(text(answer)),
        ];
        if status == ChatTurnStatusLabel::Interrupted {
            events.push(AiEvent::McpUnavailable);
        }
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(events)));
        let clock = SteppedClock::new(
            Timestamp::from_parts(
                UnixSeconds::new(5_000),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        );
        run(SendChatMessage::new(
            &provider,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .with_number_evidence(evidence)
        .execute(
            request(),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ChatTurnStatusLabel {
        Completed,
        Interrupted,
    }

    fn check_of(outcome: &SendChatOutcome) -> Option<ChatNumberCheck> {
        match outcome {
            SendChatOutcome::Completed(summary) => summary.number_check(),
            SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::RateLimited { .. }
            | SendChatOutcome::Failed { .. } => None,
        }
    }

    #[test]
    fn numbers_found_in_the_same_run_ledger_are_verified() {
        let evidence = MemoryNumberEvidence::default();
        evidence.record(
            run_id(),
            ToolPayloadJson::try_new(r#"{"price":"71200"}"#).expect("원장"),
        );
        let outcome = answer_with(
            "지금 71,200원이에요.",
            &evidence,
            ChatTurnStatusLabel::Completed,
        );
        assert_eq!(check_of(&outcome), Some(ChatNumberCheck::Verified));
    }

    #[test]
    fn numbers_from_another_run_or_missing_ledger_are_unverified() {
        let evidence = MemoryNumberEvidence::default();
        evidence.record(
            AiRunId::try_new("run-other").expect("다른 실행"),
            ToolPayloadJson::try_new(r#"{"price":"71200"}"#).expect("원장"),
        );
        let outcome = answer_with(
            "지금 71,200원이에요.",
            &evidence,
            ChatTurnStatusLabel::Completed,
        );
        assert_eq!(check_of(&outcome), Some(ChatNumberCheck::Unverified));
        let broken = MemoryNumberEvidence::default();
        broken.fail_with(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ));
        let failed_read = answer_with(
            "지금 71,200원이에요.",
            &broken,
            ChatTurnStatusLabel::Completed,
        );
        assert_eq!(check_of(&failed_read), Some(ChatNumberCheck::Unverified));
    }

    #[test]
    fn answers_without_numbers_and_unfinished_turns_carry_no_unverified_mark() {
        let evidence = MemoryNumberEvidence::default();
        let outcome = answer_with(
            "분산이 잘 되어 있어요.",
            &evidence,
            ChatTurnStatusLabel::Completed,
        );
        assert_eq!(check_of(&outcome), Some(ChatNumberCheck::NoNumbers));
        let interrupted = answer_with("지금 71,200원", &evidence, ChatTurnStatusLabel::Interrupted);
        assert_eq!(check_of(&interrupted), None);
    }

    #[test]
    fn the_default_sender_skips_the_check() {
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("지금 71,200원이에요.")),
            ])));
        let clock = SteppedClock::new(
            Timestamp::from_parts(
                UnixSeconds::new(6_000),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        );
        let outcome = run(SendChatMessage::new(
            &provider,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request(),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ));
        assert_eq!(check_of(&outcome), None);
    }
}
