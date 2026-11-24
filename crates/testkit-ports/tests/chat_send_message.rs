#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiRunId, AiUsage, BackgroundPreemption, ChatChunk, ChatExecutionMode,
        ChatFailureReason, ChatModelConfig, ChatQueueAcquisition, ChatRepository, ChatRole,
        ChatScreenContext, ChatSessionId, ChatSignalState, ChatSourceKind, ChatTurnId,
        ChatTurnIdentity, ChatTurnStatus, InformationError, IsolationCheck, IsolationFailure,
        ModelName, ModelSelection, ReasoningEffort, ResetTime, RunKind, RunPurpose,
        SendChatMessage, SendChatOutcome, SendChatRequest, StoredChatContent, StoredChatEntry,
        SystemFailure, SystemPort, SystemPortError, TokenCount, TokenUsage, ToolStatus,
        UserMessageId,
    };
    use domain::{
        HorizonSelection, KnownMarket, Nanosecond, OffsetSeconds, SpanSeconds, Symbol, TimeSpan,
        Timestamp, UnixSeconds, UntrustedText,
    };
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        chat_fakes::{
            AiProviderCall, ChatQueueCall, CollectingChatSink, MemoryChatRepository,
            MemoryChatRunLedger, ScriptedAiEventStream, ScriptedAiProvider, ScriptedChatQueue,
            ScriptedStopSignal, ScriptedStreamStep,
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

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("비신뢰 텍스트")
    }

    fn request(
        session: &str,
        turn: &str,
        run_id: &str,
        message_id: &str,
        body: &str,
    ) -> SendChatRequest {
        SendChatRequest::new(
            ChatTurnIdentity {
                session: ChatSessionId::try_new(session).expect("세션 ID"),
                turn: ChatTurnId::try_new(turn).expect("턴 ID"),
                run: AiRunId::try_new(run_id).expect("실행 ID"),
            },
            UserMessageId::try_new(message_id).expect("메시지 ID"),
            text(body),
            Some(ChatScreenContext::new(
                Some(Symbol::try_new("005930").expect("종목")),
                Some(KnownMarket::Kr),
                Some(HorizonSelection::WeeksToMonths),
            )),
            ChatModelConfig {
                model: ModelSelection::Default,
                effort: ReasoningEffort::Medium,
            },
        )
    }

    fn assert_failed_with(outcome: &SendChatOutcome, expected: ChatFailureReason, label: &str) {
        match outcome {
            SendChatOutcome::Failed { reason, .. } => {
                assert_eq!(*reason, expected, "{label}");
            }
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::RateLimited { .. } => panic!("{label}: 실패 기대, 실제 {outcome:?}"),
        }
    }

    fn assert_completed(outcome: &SendChatOutcome, label: &str) {
        match outcome {
            SendChatOutcome::Completed(_) => {}
            SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::RateLimited { .. }
            | SendChatOutcome::Failed { .. } => panic!("{label}: 완료 기대, 실제 {outcome:?}"),
        }
    }

    fn first_turn_steps(latency_step: TimeSpan) -> Vec<ScriptedStreamStep> {
        vec![
            ScriptedStreamStep::Event(AiEvent::IsolationChecked(IsolationCheck::Passed)),
            ScriptedStreamStep::Event(AiEvent::ActualModel(
                ModelName::try_new("claude-sonnet-5-5").expect("모델"),
            )),
            ScriptedStreamStep::Event(AiEvent::ToolCall {
                server: text("stock-copilot"),
                tool: text("get_portfolio"),
                args: text("{}"),
                result: text(""),
                status: ToolStatus::Running,
            }),
            ScriptedStreamStep::Event(AiEvent::ToolCall {
                server: text("stock-copilot"),
                tool: text("get_portfolio"),
                args: text("{}"),
                result: text("{}"),
                status: ToolStatus::Succeeded,
            }),
            ScriptedStreamStep::AdvanceAndEvent {
                step: latency_step,
                event: AiEvent::TextDelta(text("보유 비중을 ")),
            },
            ScriptedStreamStep::Event(AiEvent::TextDelta(text("확인했습니다."))),
            ScriptedStreamStep::Event(AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(120),
                TokenCount::new(45),
                TokenCount::new(10),
                TokenCount::new(0),
            ))),
        ]
    }

    fn assert_first_turn_storage(repo: &MemoryChatRepository) {
        let session_id = ChatSessionId::try_new("session-1").expect("세션");
        let stored = repo.stored_session(&session_id).expect("세션 저장됨");
        assert_eq!(stored.entries().len(), 2, "사용자 + 어시스턴트 2건 저장");
        let user_msg = run(repo.load_user_message(
            &session_id,
            &UserMessageId::try_new("msg-1").expect("메시지"),
        ))
        .expect("조회")
        .expect("사용자 메시지 존재");
        assert_eq!(
            user_msg.text().as_untrusted_str(),
            "내 포트폴리오 균형 어때?",
            "원문 보존"
        );
        let Some(StoredChatContent::Assistant(reply)) =
            stored.entries().get(1).map(StoredChatEntry::content)
        else {
            panic!("어시스턴트 응답 저장 기대");
        };
        assert_eq!(
            reply.text().as_untrusted_str(),
            "보유 비중을 확인했습니다.",
            "비신뢰 텍스트 합산 저장"
        );
        assert_eq!(reply.status(), ChatTurnStatus::Completed, "완료 상태 저장");
        assert_eq!(reply.sources().len(), 1, "도구 출처 1건");
        assert_eq!(
            reply.sources()[0].kind(),
            ChatSourceKind::Tool,
            "도구 출처 종류"
        );
    }

    fn assert_chunk_sequence(chunks: &[ChatChunk]) {
        assert_eq!(chunks.len(), 6, "청크 6건");
        assert_eq!(
            chunks[0],
            ChatChunk::ToolCallStarted {
                server: text("stock-copilot"),
                tool: text("get_portfolio"),
            },
            "첫 청크 ToolCallStarted"
        );
        assert_eq!(
            chunks[1],
            ChatChunk::ToolCallFinished {
                server: text("stock-copilot"),
                tool: text("get_portfolio"),
                status: ToolStatus::Succeeded,
            },
            "둘째 청크 ToolCallFinished"
        );
        assert_eq!(
            chunks[2],
            ChatChunk::TextDelta(text("보유 비중을 ")),
            "셋째 청크 TextDelta"
        );
        assert_eq!(
            chunks[3],
            ChatChunk::TextDelta(text("확인했습니다.")),
            "넷째 청크 TextDelta"
        );
        match &chunks[4] {
            ChatChunk::Sources(list) => assert_eq!(list.len(), 1, "출처 1건"),
            ChatChunk::TextDelta(_)
            | ChatChunk::ToolCallStarted { .. }
            | ChatChunk::ToolCallFinished { .. }
            | ChatChunk::Card(_)
            | ChatChunk::Done(_)
            | ChatChunk::Cancelled
            | ChatChunk::Interrupted
            | ChatChunk::RateLimited { .. }
            | ChatChunk::Failed(_) => panic!("Sources 청크 기대"),
        }
        match &chunks[5] {
            ChatChunk::Done(_) => {}
            ChatChunk::TextDelta(_)
            | ChatChunk::ToolCallStarted { .. }
            | ChatChunk::ToolCallFinished { .. }
            | ChatChunk::Card(_)
            | ChatChunk::Sources(_)
            | ChatChunk::Cancelled
            | ChatChunk::Interrupted
            | ChatChunk::RateLimited { .. }
            | ChatChunk::Failed(_) => panic!("Done 청크 기대"),
        }
    }

    fn assert_first_turn_provider_and_queue(
        provider: &ScriptedAiProvider,
        ledger: &MemoryChatRunLedger,
        queue: &ScriptedChatQueue,
    ) {
        let Some(AiProviderCall::Run(spec)) = provider.calls().into_iter().next() else {
            panic!("Run 호출 기대");
        };
        assert_eq!(spec.kind(), RunKind::Analysis, "분석 실행 고정");
        assert_eq!(spec.purpose(), RunPurpose::Chat, "목적 Chat");
        assert_eq!(ledger.entries().len(), 1, "실행 원장 1건");
        assert_eq!(queue.active_run(), None, "종료 후 큐 슬롯 해제");
        assert_eq!(
            queue.calls(),
            vec![
                ChatQueueCall::Acquire(AiRunId::try_new("run-1").expect("실행")),
                ChatQueueCall::Release(AiRunId::try_new("run-1").expect("실행")),
            ],
            "큐 선점과 해제 쌍"
        );
    }

    #[test]
    fn first_turn_runs_as_analysis_streams_untrusted_text_and_persists_to_repository() {
        let provider = ScriptedAiProvider::default();
        let repo = MemoryChatRepository::default();
        let ledger = MemoryChatRunLedger::default();
        let queue = ScriptedChatQueue::default();
        queue.set_next_acquisition(ChatQueueAcquisition::Acquired {
            preempted_background: BackgroundPreemption::SignaledPause,
        });
        let clock = Arc::new(SteppedClock::new(at(1_000)));
        let latency_step =
            TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(350_000_000)).expect("지연");
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::with_clock(
                first_turn_steps(latency_step),
                Arc::clone(&clock),
            )));
        let mut sink = CollectingChatSink::default();
        let outcome = run(
            SendChatMessage::new(&provider, &repo, &ledger, &queue, clock.as_ref()).execute(
                request(
                    "session-1",
                    "turn-1",
                    "run-1",
                    "msg-1",
                    "내 포트폴리오 균형 어때?",
                ),
                &mut sink,
                &ScriptedStopSignal::always_active(),
            ),
        );
        let SendChatOutcome::Completed(summary) = outcome else {
            panic!("완료 기대: {outcome:?}");
        };
        assert_eq!(summary.mode(), ChatExecutionMode::FreshAnalysis, "새 분석");
        assert_eq!(
            summary.first_token_latency(),
            Some(latency_step),
            "첫 글자 지연"
        );
        assert_eq!(
            summary.actual_model().map(ModelName::as_str),
            Some("claude-sonnet-5-5"),
            "실제 응답 모델"
        );
        assert_first_turn_provider_and_queue(&provider, &ledger, &queue);
        assert_first_turn_storage(&repo);
        assert_chunk_sequence(sink.chunks());
    }

    fn assert_resume_call(provider: &ScriptedAiProvider) {
        let calls = provider.calls();
        let Some(AiProviderCall::Resume {
            spec,
            history,
            message,
        }) = calls.get(1)
        else {
            panic!("두 번째 호출은 resume여야 함");
        };
        assert_eq!(spec.kind(), RunKind::Analysis, "resume도 분석 실행");
        assert_eq!(history.messages().len(), 2, "이전 대화 2건 재주입");
        assert_eq!(history.messages()[0].role(), ChatRole::User, "첫 항목 User");
        assert_eq!(
            history.messages()[0].text().as_untrusted_str(),
            "첫 질문",
            "첫 질문 원문"
        );
        assert_eq!(
            history.messages()[1].role(),
            ChatRole::Assistant,
            "둘째 항목 Assistant"
        );
        assert_eq!(
            history.messages()[1].text().as_untrusted_str(),
            "첫 답변",
            "첫 답변 원문"
        );
        assert_eq!(message.as_untrusted_str(), "후속 질문", "새 질문 전달");
    }

    #[test]
    fn subsequent_turn_reinjects_sqlite_history_via_resume_without_cli_session_state() {
        let provider = ScriptedAiProvider::default();
        let repo = MemoryChatRepository::default();
        let ledger = MemoryChatRunLedger::default();
        let queue = ScriptedChatQueue::default();
        let clock = SteppedClock::new(at(2_000));
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("첫 답변")),
            ])));
        provider
            .script_resume()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("두 번째 답변")),
            ])));
        let use_case = SendChatMessage::new(&provider, &repo, &ledger, &queue, &clock);
        let stop = ScriptedStopSignal::always_active();
        let first = run(use_case.execute(
            request("session-2", "turn-1", "run-1", "msg-1", "첫 질문"),
            &mut CollectingChatSink::default(),
            &stop,
        ));
        assert_completed(&first, "첫 턴 완료");
        let second = run(use_case.execute(
            request("session-2", "turn-2", "run-2", "msg-2", "후속 질문"),
            &mut CollectingChatSink::default(),
            &stop,
        ));
        let SendChatOutcome::Completed(summary) = second else {
            panic!("후속 턴 완료 기대: {second:?}");
        };
        assert_eq!(
            summary.mode(),
            ChatExecutionMode::ResumedFromHistory,
            "저장 대화 재주입"
        );
        assert_resume_call(&provider);
    }

    #[test]
    fn cancellation_emits_distinct_cancelled_chunk_and_interrupts_provider() {
        let clock = SteppedClock::new(at(3_000));
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("부분 응답")),
                AiEvent::TextDelta(text("취소로 중단될 부분")),
            ])));
        provider.script_interrupt().push(Ok(()));
        let stop = ScriptedStopSignal::with_sequence(
            vec![
                ChatSignalState::Active,
                ChatSignalState::Active,
                ChatSignalState::Cancelled,
            ],
            ChatSignalState::Cancelled,
        );
        let mut sink = CollectingChatSink::default();
        let outcome = run(SendChatMessage::new(
            &provider,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-c", "turn-c", "run-c", "msg-c", "질문"),
            &mut sink,
            &stop,
        ));
        match outcome {
            SendChatOutcome::Cancelled(_) => {}
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::RateLimited { .. }
            | SendChatOutcome::Failed { .. } => panic!("취소 결과 기대: {outcome:?}"),
        }
        assert_eq!(
            sink.chunks().last(),
            Some(&ChatChunk::Cancelled),
            "취소는 Cancelled 조각으로 구분"
        );
        assert!(
            provider.calls().contains(&AiProviderCall::Interrupt(
                AiRunId::try_new("run-c").expect("ID")
            )),
            "취소 시 provider.interrupt 호출"
        );
    }

    #[test]
    fn provider_interruption_emits_distinct_interrupted_chunk() {
        let clock = SteppedClock::new(at(3_500));
        let provider_interrupt = ScriptedAiProvider::default();
        provider_interrupt
            .script_run()
            .push(Ok(ScriptedAiEventStream::new(vec![
                ScriptedStreamStep::Event(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                ScriptedStreamStep::Event(AiEvent::TextDelta(text("중단 전"))),
                ScriptedStreamStep::Error(InformationError::Unavailable(ResetTime::NotProvided)),
            ])));
        let mut sink_interrupt = CollectingChatSink::default();
        let interrupted = run(SendChatMessage::new(
            &provider_interrupt,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-i", "turn-i", "run-i", "msg-i", "질문"),
            &mut sink_interrupt,
            &ScriptedStopSignal::always_active(),
        ));
        match interrupted {
            SendChatOutcome::Interrupted(_) => {}
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::RateLimited { .. }
            | SendChatOutcome::Failed { .. } => panic!("중단 결과 기대: {interrupted:?}"),
        }
        assert_eq!(
            sink_interrupt.chunks().last(),
            Some(&ChatChunk::Interrupted),
            "중단은 Interrupted 조각"
        );
    }

    #[test]
    fn rate_limit_emits_distinct_rate_limited_chunk() {
        let clock = SteppedClock::new(at(3_600));
        let provider_limit = ScriptedAiProvider::default();
        let resume_at = ResetTime::Known(at(4_000));
        provider_limit
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::RateLimited {
                    resets_at: resume_at,
                },
            ])));
        let mut sink_limit = CollectingChatSink::default();
        let rate_limited = run(SendChatMessage::new(
            &provider_limit,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-r", "turn-r", "run-r", "msg-r", "질문"),
            &mut sink_limit,
            &ScriptedStopSignal::always_active(),
        ));
        match rate_limited {
            SendChatOutcome::RateLimited { resets_at, .. } => {
                assert_eq!(resets_at, resume_at, "재개 시각 보존");
            }
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::Failed { .. } => panic!("한도 초과 기대: {rate_limited:?}"),
        }
        assert_eq!(
            sink_limit.chunks().last(),
            Some(&ChatChunk::RateLimited {
                resets_at: resume_at
            }),
            "한도 초과는 RateLimited 조각"
        );
    }

    fn assert_forbidden_event_blocked(label: &str, forbidden_event: AiEvent) {
        let clock = SteppedClock::new(at(5_000));
        let provider = ScriptedAiProvider::default();
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                forbidden_event,
                AiEvent::TextDelta(text("실행되면 안 되는 후속 텍스트")),
            ])));
        provider.script_interrupt().push(Ok(()));
        let mut sink = CollectingChatSink::default();
        let outcome = run(SendChatMessage::new(
            &provider,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-sec", "turn-sec", "run-sec", "msg-sec", "주문해 줘"),
            &mut sink,
            &ScriptedStopSignal::always_active(),
        ));
        assert_failed_with(&outcome, ChatFailureReason::ForbiddenToolAttempt, label);
        assert_eq!(
            sink.chunks(),
            &[ChatChunk::Failed(ChatFailureReason::ForbiddenToolAttempt)],
            "{label}: 후속 텍스트 전달 없이 실패 조각"
        );
        assert!(
            provider.calls().contains(&AiProviderCall::Interrupt(
                AiRunId::try_new("run-sec").expect("ID")
            )),
            "{label}: 즉시 interrupt 호출"
        );
    }

    #[test]
    fn rejects_web_search_and_forbidden_order_or_settings_tools_in_chat() {
        assert_forbidden_event_blocked(
            "web-search",
            AiEvent::WebSearch {
                query: text("삼성전자 100주"),
                action: text("search"),
            },
        );
        assert_forbidden_event_blocked(
            "order-tool",
            AiEvent::ToolCall {
                server: text("stock-copilot"),
                tool: text("submit_order"),
                args: text("{}"),
                result: text(""),
                status: ToolStatus::Running,
            },
        );
        assert_forbidden_event_blocked(
            "settings-tool",
            AiEvent::ToolCall {
                server: text("stock-copilot"),
                tool: text("change_risk_settings"),
                args: text("{}"),
                result: text(""),
                status: ToolStatus::Running,
            },
        );
    }

    #[test]
    fn enforces_isolation_first_event_and_blocks_unverified_output() {
        let clock = SteppedClock::new(at(6_000));
        let provider_rejected = ScriptedAiProvider::default();
        provider_rejected
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Rejected(IsolationFailure::McpServers)),
            ])));
        let outcome = run(SendChatMessage::new(
            &provider_rejected,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-iso", "turn-iso", "run-iso", "msg-iso", "질문"),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ));
        assert_failed_with(
            &outcome,
            ChatFailureReason::IsolationRejected(IsolationFailure::McpServers),
            "사전 점검 거절",
        );

        let provider_unverified = ScriptedAiProvider::default();
        provider_unverified
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::TextDelta(text("사전 점검 없는 텍스트")),
            ])));
        provider_unverified.script_interrupt().push(Ok(()));
        let unverified = run(SendChatMessage::new(
            &provider_unverified,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-u", "turn-u", "run-u", "msg-u", "질문"),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ));
        assert_failed_with(
            &unverified,
            ChatFailureReason::IsolationNotVerified,
            "사전 점검 없이 텍스트 차단",
        );
    }

    #[test]
    fn persists_complete_reply_even_when_sink_closes_mid_stream() {
        let clock = SteppedClock::new(at(6_500));
        let provider = ScriptedAiProvider::default();
        let repo = MemoryChatRepository::default();
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(text("새로고침 전 ")),
                AiEvent::TextDelta(text("새로고침 후")),
            ])));
        let mut closing_sink = CollectingChatSink::close_after(1);
        let outcome = run(SendChatMessage::new(
            &provider,
            &repo,
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-rld", "turn-rld", "run-rld", "msg-rld", "질문"),
            &mut closing_sink,
            &ScriptedStopSignal::always_active(),
        ));
        assert_completed(&outcome, "싱크가 닫혀도 코어는 응답을 끝까지 저장");
        let stored = repo
            .stored_session(&ChatSessionId::try_new("session-rld").expect("세션"))
            .expect("저장됨");
        let Some(StoredChatContent::Assistant(reply)) =
            stored.entries().get(1).map(StoredChatEntry::content)
        else {
            panic!("어시스턴트 응답 저장");
        };
        assert_eq!(
            reply.text().as_untrusted_str(),
            "새로고침 전 새로고침 후",
            "창 새로고침 뒤에도 전체 원문이 저장소에 남음"
        );
    }

    #[test]
    fn rejects_empty_message_and_busy_queue() {
        let clock = SteppedClock::new(at(7_000));
        let provider = ScriptedAiProvider::default();
        let repo = MemoryChatRepository::default();
        let ledger = MemoryChatRunLedger::default();
        let queue = ScriptedChatQueue::default();
        let empty_outcome = run(
            SendChatMessage::new(&provider, &repo, &ledger, &queue, &clock).execute(
                request("session-e", "turn-e", "run-e", "msg-e", "   \n\t "),
                &mut CollectingChatSink::default(),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_failed_with(
            &empty_outcome,
            ChatFailureReason::EmptyMessage,
            "빈 메시지 거부",
        );
        assert!(provider.calls().is_empty(), "빈 메시지는 AI 호출 0건");

        queue.set_next_acquisition(ChatQueueAcquisition::Busy);
        let busy_outcome = run(
            SendChatMessage::new(&provider, &repo, &ledger, &queue, &clock).execute(
                request("session-b", "turn-b", "run-b", "msg-b", "질문"),
                &mut CollectingChatSink::default(),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_failed_with(
            &busy_outcome,
            ChatFailureReason::QueueBusy,
            "동시 채팅 1개 제한",
        );
    }

    #[test]
    fn propagates_storage_failure_and_maps_quota_wait_to_rate_limited() {
        let clock = SteppedClock::new(at(7_500));
        let repo_fail = MemoryChatRepository::default();
        repo_fail.fail_next_ensure(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::PermissionDenied,
        ));
        let store_outcome = run(SendChatMessage::new(
            &ScriptedAiProvider::default(),
            &repo_fail,
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-s", "turn-s", "run-s", "msg-s", "질문"),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ));
        assert_failed_with(
            &store_outcome,
            ChatFailureReason::StorageError(SystemFailure::PermissionDenied),
            "저장 실패 전파",
        );

        let provider_quota = ScriptedAiProvider::default();
        let reset = ResetTime::Known(at(8_000));
        provider_quota
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::Quota(AiUsage::Waiting(reset)),
            ])));
        let quota_outcome = run(SendChatMessage::new(
            &provider_quota,
            &MemoryChatRepository::default(),
            &MemoryChatRunLedger::default(),
            &ScriptedChatQueue::default(),
            &clock,
        )
        .execute(
            request("session-q", "turn-q", "run-q", "msg-q", "질문"),
            &mut CollectingChatSink::default(),
            &ScriptedStopSignal::always_active(),
        ));
        match quota_outcome {
            SendChatOutcome::RateLimited { resets_at, .. } => {
                assert_eq!(resets_at, reset, "Quota::Waiting도 한도 초과 조각으로 매핑");
            }
            SendChatOutcome::Completed(_)
            | SendChatOutcome::Cancelled(_)
            | SendChatOutcome::Interrupted(_)
            | SendChatOutcome::Failed { .. } => panic!("한도 초과 기대: {quota_outcome:?}"),
        }
    }
}
