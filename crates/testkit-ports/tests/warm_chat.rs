#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiRunId, ChatChunk, ChatExecutionMode, ChatFailureReason, ChatModelConfig,
        ChatSessionId, ChatTurnId, ChatTurnIdentity, IsolationCheck, IsolationFailure,
        McpConnectionState, ModelSelection, PrewarmChat, PrewarmChatOutcome, ReasoningEffort,
        ResetTime, RunKind, SendChatMessage, SendChatOutcome, SendChatRequest, SendWarmChatMessage,
        ToolStatus, UserMessageId, WarmChatId, WarmChatProbe, WarmChatRejection, WarmChatRequest,
    };
    use domain::{
        Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        chat_fakes::{
            AiProviderCall, CollectingChatSink, MemoryChatRepository, MemoryChatRunLedger,
            ScriptedAiEventStream, ScriptedAiProvider, ScriptedChatQueue, ScriptedPrewarmReply,
            ScriptedStopSignal, ScriptedStreamStep, ScriptedWarmChatRuntime, WarmChatRuntimeCall,
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

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn span(seconds: u64, millis: u32) -> TimeSpan {
        TimeSpan::try_new(
            SpanSeconds::new(seconds),
            Nanosecond::new(millis.saturating_mul(1_000_000)),
        )
        .expect("시간 간격")
    }

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("비신뢰 텍스트")
    }

    fn default_config() -> ChatModelConfig {
        ChatModelConfig {
            model: ModelSelection::Default,
            effort: ReasoningEffort::Medium,
        }
    }

    fn chat_request(run_id: &str, config: ChatModelConfig) -> SendChatRequest {
        SendChatRequest::new(
            ChatTurnIdentity {
                session: ChatSessionId::try_new("warm-session").expect("세션"),
                turn: ChatTurnId::try_new("turn-1").expect("턴"),
                run: AiRunId::try_new(run_id).expect("실행"),
            },
            UserMessageId::try_new("msg-1").expect("메시지"),
            text("오늘 시장 요약해 줘"),
            None,
            config,
        )
    }

    fn ready_probe(id: &str, config: ChatModelConfig) -> WarmChatProbe {
        WarmChatProbe::new(
            WarmChatId::try_new(id).expect("대기 ID"),
            RunKind::Analysis,
            IsolationCheck::Passed,
            McpConnectionState::Connected,
            config,
        )
    }

    struct WarmEnv {
        clock: Arc<SteppedClock>,
        warm: ScriptedWarmChatRuntime,
        provider: ScriptedAiProvider,
        repo: MemoryChatRepository,
        ledger: MemoryChatRunLedger,
        queue: ScriptedChatQueue,
    }

    impl WarmEnv {
        fn new(seconds: i64) -> Self {
            let clock = Arc::new(SteppedClock::new(at(seconds)));
            let warm = ScriptedWarmChatRuntime::with_clock(Arc::clone(&clock));
            Self {
                clock,
                warm,
                provider: ScriptedAiProvider::default(),
                repo: MemoryChatRepository::default(),
                ledger: MemoryChatRunLedger::default(),
                queue: ScriptedChatQueue::default(),
            }
        }

        fn prewarm(&self, run_id: &str, config: ChatModelConfig) -> PrewarmChatOutcome {
            run(
                PrewarmChat::new(&self.warm, self.clock.as_ref()).execute(WarmChatRequest::new(
                    AiRunId::try_new(run_id).expect("ID"),
                    config,
                )),
            )
        }

        fn send(&self, request: SendChatRequest, sink: &mut CollectingChatSink) -> SendChatOutcome {
            let sender = SendChatMessage::new(
                &self.provider,
                &self.repo,
                &self.ledger,
                &self.queue,
                self.clock.as_ref(),
            );
            let use_case = SendWarmChatMessage::with_default_max_age(sender, &self.warm);
            run(use_case.execute(request, sink, &ScriptedStopSignal::always_active()))
        }
    }

    const fn matches_run_call(call: Option<&AiProviderCall>) -> bool {
        match call {
            Some(AiProviderCall::Run(_)) => true,
            Some(
                AiProviderCall::Resume { .. }
                | AiProviderCall::Usage
                | AiProviderCall::Interrupt(_)
                | AiProviderCall::VerifyIsolation(_),
            )
            | None => false,
        }
    }

    #[test]
    fn prewarm_excludes_isolation_and_mcp_wait_from_first_token_latency() {
        let env = WarmEnv::new(10_000);
        let preflight_and_mcp_wait = span(1, 500);
        let generation_latency = span(0, 250);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: Some(preflight_and_mcp_wait),
            result: Ok(ready_probe("warm-1", default_config())),
        });
        let PrewarmChatOutcome::Ready(session) = env.prewarm("prewarm-1", default_config()) else {
            panic!("미리 띄운 세션 준비 기대");
        };
        assert_eq!(session.id().as_str(), "warm-1", "준비된 세션 ID");
        env.warm
            .script_send_on_warm()
            .push(Ok(ScriptedAiEventStream::with_clock(
                vec![ScriptedStreamStep::AdvanceAndEvent {
                    step: generation_latency,
                    event: AiEvent::TextDelta(text("바로 첫 글자를 냅니다.")),
                }],
                Arc::clone(&env.clock),
            )));
        let mut sink = CollectingChatSink::default();
        let outcome = env.send(chat_request("run-warm-1", default_config()), &mut sink);
        let SendChatOutcome::Completed(summary) = outcome else {
            panic!("완료 기대: {outcome:?}");
        };
        assert_eq!(
            summary.mode(),
            ChatExecutionMode::WarmPreverified,
            "미리 점검된 경로로 실행"
        );
        assert_eq!(
            summary.first_token_latency(),
            Some(generation_latency),
            "첫 글자 시간에 사전 점검·MCP 연결 대기(1.5초)가 들어가지 않음"
        );
        assert!(env.provider.calls().is_empty(), "콜드 경로 미호출");
        assert!(env.warm.ready_session().is_none(), "사용된 대기 세션 소비");
    }

    #[test]
    fn prewarm_rejects_and_discards_collection_failed_isolation_or_disconnected_mcp() {
        let cases = [
            (
                WarmChatProbe::new(
                    WarmChatId::try_new("bad-kind").expect("ID"),
                    RunKind::Collection,
                    IsolationCheck::Passed,
                    McpConnectionState::Connected,
                    default_config(),
                ),
                WarmChatRejection::WrongRunKind,
            ),
            (
                WarmChatProbe::new(
                    WarmChatId::try_new("bad-iso").expect("ID"),
                    RunKind::Analysis,
                    IsolationCheck::Rejected(IsolationFailure::Tools),
                    McpConnectionState::Connected,
                    default_config(),
                ),
                WarmChatRejection::IsolationRejected(IsolationFailure::Tools),
            ),
            (
                WarmChatProbe::new(
                    WarmChatId::try_new("bad-mcp").expect("ID"),
                    RunKind::Analysis,
                    IsolationCheck::Passed,
                    McpConnectionState::Disconnected,
                    default_config(),
                ),
                WarmChatRejection::McpNotConnected,
            ),
        ];
        for (probe, expected) in cases {
            let env = WarmEnv::new(11_000);
            let id = probe.id().clone();
            env.warm.script_prewarm().push(ScriptedPrewarmReply {
                wait_before_ready: None,
                result: Ok(probe),
            });
            env.warm.script_discard().push(Ok(()));
            let outcome = env.prewarm("prewarm-bad", default_config());
            assert_eq!(
                outcome,
                PrewarmChatOutcome::Rejected(expected),
                "사전 조건 실패 거절"
            );
            assert!(
                env.warm.ready_session().is_none(),
                "실패 세션은 대기 슬롯 미저장"
            );
            assert!(
                env.warm.calls().contains(&WarmChatRuntimeCall::Discard(id)),
                "실패 세션 즉시 폐기"
            );
        }
    }

    #[test]
    fn stale_warm_session_is_discarded_and_falls_back_to_cold_analysis() {
        let env = WarmEnv::new(12_000);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-stale", default_config())),
        });
        env.warm.script_discard().push(Ok(()));
        let _ready = env.prewarm("prewarm-stale", default_config());
        env.clock.advance_by(span(601, 0)).expect("시각 진행");

        env.provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("콜드 폴백 답변")),
            ])));
        let outcome = env.send(
            chat_request("run-cold-1", default_config()),
            &mut CollectingChatSink::default(),
        );
        let SendChatOutcome::Completed(summary) = outcome else {
            panic!("폴백 완료 기대: {outcome:?}");
        };
        assert_eq!(
            summary.mode(),
            ChatExecutionMode::FreshAnalysis,
            "만료 세션은 콜드 분석 실행으로 폴백"
        );
        assert!(
            env.warm.calls().contains(&WarmChatRuntimeCall::Discard(
                WarmChatId::try_new("warm-stale").expect("ID")
            )),
            "만료 세션 폐기"
        );
        assert_eq!(env.provider.calls().len(), 1, "콜드 provider.run 1회 호출");
    }

    #[test]
    fn model_mismatch_discards_warm_session_and_falls_back_to_cold_analysis() {
        let env = WarmEnv::new(13_000);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-default", default_config())),
        });
        env.warm.script_discard().push(Ok(()));
        let _ready = env.prewarm("prewarm-m", default_config());

        env.provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("고성능 모델 콜드 답변")),
            ])));
        let outcome = env.send(
            chat_request(
                "run-high",
                ChatModelConfig {
                    model: ModelSelection::Highest,
                    effort: ReasoningEffort::High,
                },
            ),
            &mut CollectingChatSink::default(),
        );
        let SendChatOutcome::Completed(summary) = outcome else {
            panic!("폴백 완료 기대: {outcome:?}");
        };
        assert_eq!(
            summary.mode(),
            ChatExecutionMode::FreshAnalysis,
            "폴백 모드"
        );
        assert!(
            env.warm.calls().contains(&WarmChatRuntimeCall::Discard(
                WarmChatId::try_new("warm-default").expect("ID")
            )),
            "모델 불일치 세션 폐기"
        );
        assert!(
            matches_run_call(env.provider.calls().first()),
            "콜드 실행 호출"
        );
    }

    #[test]
    fn warm_stream_blocks_forbidden_tools() {
        let env = WarmEnv::new(14_000);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-sec", default_config())),
        });
        let _ready = env.prewarm("prewarm-sec", default_config());
        env.warm
            .script_send_on_warm()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::ToolCall {
                    server: text("stock-copilot"),
                    tool: text("submit_order"),
                    args: text("{}"),
                    result: text(""),
                    status: ToolStatus::Running,
                },
            ])));
        env.provider.script_interrupt().push(Ok(()));
        let outcome = env.send(
            chat_request("run-sec", default_config()),
            &mut CollectingChatSink::default(),
        );
        match outcome {
            SendChatOutcome::Failed { reason, .. } => {
                assert_eq!(
                    reason,
                    ChatFailureReason::ForbiddenToolAttempt,
                    "금지 도구 차단"
                );
            }
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::RateLimited { .. } => panic!("실패 기대: {outcome:?}"),
        }
    }

    #[test]
    fn warm_stream_preserves_rate_limit_chunk() {
        let env = WarmEnv::new(14_500);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-rl", default_config())),
        });
        let _ready_rl = env.prewarm("prewarm-rl", default_config());
        let reset = ResetTime::Known(at(15_000));
        env.warm
            .script_send_on_warm()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::RateLimited { resets_at: reset },
            ])));
        let mut sink_rl = CollectingChatSink::default();
        let outcome_rl = env.send(chat_request("run-rl", default_config()), &mut sink_rl);
        assert_eq!(
            sink_rl.chunks().last(),
            Some(&ChatChunk::RateLimited { resets_at: reset }),
            "미리 띄운 경로에서도 한도 초과 조각 구분"
        );
        match outcome_rl {
            SendChatOutcome::RateLimited { resets_at, .. } => {
                assert_eq!(resets_at, reset, "재개 시각");
            }
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::Failed { .. } => panic!("한도 초과 기대: {outcome_rl:?}"),
        }
    }

    #[test]
    fn prewarm_discards_previous_ready_session_when_replaced() {
        let env = WarmEnv::new(16_000);
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-old", default_config())),
        });
        env.warm.script_prewarm().push(ScriptedPrewarmReply {
            wait_before_ready: None,
            result: Ok(ready_probe("warm-new", default_config())),
        });
        env.warm.script_discard().push(Ok(()));
        let _first = env.prewarm("p-1", default_config());
        let second = env.prewarm("p-2", default_config());
        let PrewarmChatOutcome::Ready(session) = second else {
            panic!("교체 성공 기대: {second:?}");
        };
        assert_eq!(session.id().as_str(), "warm-new", "새 세션 저장");
        assert!(
            env.warm.calls().contains(&WarmChatRuntimeCall::Discard(
                WarmChatId::try_new("warm-old").expect("ID")
            )),
            "기존 대기 세션 정리"
        );
    }
}
