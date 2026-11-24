#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiRunId, AuditKind, ChatRepository, ChatSequence, ChatSessionId, ChatSignalState,
        ChatTurnId, ChatTurnStatus, ChatWebResearch, ChatWebResearchFailure,
        ChatWebResearchRequest, CollectionQuery, IsolationCheck, ModelSelection, ReasoningEffort,
        ResetTime, RunInput, RunKind, RunPurpose, StoredChatContent, StoredChatEntry,
        StoredUserMessage, ToolStatus, UserMessageId, WebSearchQueryMode,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds, UntrustedText};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        chat_fakes::{
            AiProviderCall, MemoryChatRepository, MemoryChatRunLedger, ScriptedAiEventStream,
            ScriptedAiProvider, ScriptedStopSignal,
        },
        memory::MemoryReply,
        system_fakes::MemoryAuditLog,
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

    fn session() -> ChatSessionId {
        ChatSessionId::try_new("session-1").expect("세션")
    }

    fn turn(value: &str) -> ChatTurnId {
        ChatTurnId::try_new(value).expect("턴")
    }

    fn user_entry(seq: u64, turn_id: &str, message_id: &str, body: &str) -> StoredChatEntry {
        StoredChatEntry::new(
            ChatSequence::new(seq),
            at(1_000),
            turn(turn_id),
            StoredChatContent::User(StoredUserMessage::from_store_result(
                UserMessageId::try_new(message_id).expect("메시지"),
                text(body),
            )),
        )
    }

    fn seeded_repository() -> MemoryChatRepository {
        let repo = MemoryChatRepository::default();
        let _record = run(repo.ensure_session(&session(), at(1_000))).expect("세션");
        let _first = run(repo.append_entry(
            &session(),
            &user_entry(
                1,
                "turn-1",
                "msg-1",
                "삼성전자 2분기 실적 발표 반응 찾아봐 줘",
            ),
        ))
        .expect("저장");
        let _second = run(repo.append_entry(
            &session(),
            &user_entry(2, "turn-2", "msg-2", "SK하이닉스 소식도 알려줘"),
        ))
        .expect("저장");
        repo
    }

    fn research_request(turn_id: &str) -> ChatWebResearchRequest {
        ChatWebResearchRequest::new(
            session(),
            turn(turn_id),
            AiRunId::try_new("collect-1").expect("실행"),
            ModelSelection::Default,
            ReasoningEffort::Low,
        )
    }

    fn passed_then(events: Vec<AiEvent>) -> ScriptedAiEventStream {
        let mut all = vec![AiEvent::IsolationChecked(IsolationCheck::Passed)];
        all.extend(events);
        ScriptedAiEventStream::from_events(all)
    }

    #[test]
    fn user_mode_sends_stored_original_of_that_turn_as_collection_run() {
        let provider = ScriptedAiProvider::default();
        let repo = seeded_repository();
        let ledger = MemoryChatRunLedger::default();
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(at(2_000));
        provider.script_run().push(Ok(passed_then(vec![
            AiEvent::WebSearch {
                query: text("삼성전자 2분기 실적"),
                action: text("search"),
            },
            AiEvent::TextDelta(text("실적 발표 뒤 ")),
            AiEvent::TextDelta(text("주가가 올랐습니다.")),
        ])));
        let findings = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-1"),
                &ScriptedStopSignal::always_active(),
            ),
        )
        .expect("수집 결과");
        assert_eq!(
            findings.summary().as_untrusted_str(),
            "실적 발표 뒤 주가가 올랐습니다.",
            "수집 결과만 돌려줌"
        );
        let calls = provider.calls();
        let Some(AiProviderCall::Run(spec)) = calls.first() else {
            panic!("수집 실행 호출 기대: {calls:?}");
        };
        assert_eq!(spec.kind(), RunKind::Collection, "수집 실행");
        let RunInput::Collection(CollectionQuery::UserMessage(message)) = spec.input() else {
            panic!("사용자 문장 모드 기대");
        };
        assert_eq!(
            message.text().as_untrusted_str(),
            "삼성전자 2분기 실적 발표 반응 찾아봐 줘",
            "저장된 원문 그대로"
        );
        assert_eq!(message.id().as_str(), "msg-1", "그 턴의 사용자 메시지");
        let entries = ledger.entries();
        assert_eq!(entries.len(), 1, "원장 1건");
        assert_eq!(entries[0].kind(), RunKind::Collection, "원장 실행 종류");
        assert_eq!(entries[0].purpose(), RunPurpose::WebCollection, "원장 목적");
        assert_eq!(entries[0].status(), ChatTurnStatus::Completed, "원장 상태");
        let audited = audit.entries();
        assert_eq!(audited.len(), 1, "감사 1건");
        let AuditKind::WebSearchChecked(record) = audited[0].kind() else {
            panic!("웹 검색 감사 기대");
        };
        assert_eq!(
            record.mode(),
            WebSearchQueryMode::UserDirectMessage,
            "사용자 직접 문장"
        );
    }

    #[test]
    fn missing_turn_message_never_starts_collection_run() {
        let provider = ScriptedAiProvider::default();
        let repo = seeded_repository();
        let ledger = MemoryChatRunLedger::default();
        let audit = MemoryAuditLog::default();
        let clock = SteppedClock::new(at(2_000));
        let outcome = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-9"),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_eq!(
            outcome,
            Err(ChatWebResearchFailure::UserMessageMissing),
            "없는 턴"
        );
        assert!(provider.calls().is_empty(), "수집 실행 없음");
        assert!(audit.entries().is_empty(), "감사 없음");
    }

    #[test]
    fn mcp_tool_call_in_collection_run_is_rejected_and_interrupted() {
        let provider = ScriptedAiProvider::default();
        let repo = seeded_repository();
        let ledger = MemoryChatRunLedger::default();
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(at(2_000));
        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::ToolCall {
                server: text("stock-copilot"),
                tool: text("get_portfolio"),
                args: text("{}"),
                result: text(""),
                status: ToolStatus::Running,
            }])));
        provider.script_interrupt().push(Ok(()));
        let outcome = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-2"),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_eq!(
            outcome,
            Err(ChatWebResearchFailure::ForbiddenToolAttempt),
            "MCP 도구 금지"
        );
        assert!(
            provider.calls().contains(&AiProviderCall::Interrupt(
                AiRunId::try_new("collect-1").expect("실행")
            )),
            "중단 호출"
        );
    }

    #[test]
    fn text_before_isolation_check_is_rejected() {
        let provider = ScriptedAiProvider::default();
        let repo = seeded_repository();
        let ledger = MemoryChatRunLedger::default();
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(at(2_000));
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::TextDelta(text("격리 전 글자")),
            ])));
        provider.script_interrupt().push(Ok(()));
        let outcome = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-1"),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_eq!(
            outcome,
            Err(ChatWebResearchFailure::IsolationNotVerified),
            "격리 확인 전 거부"
        );
    }

    #[test]
    fn rate_limit_and_cancel_are_reported_separately() {
        let provider = ScriptedAiProvider::default();
        let repo = seeded_repository();
        let ledger = MemoryChatRunLedger::default();
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        audit.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(at(2_000));
        let resets_at = ResetTime::Known(at(9_000));
        provider
            .script_run()
            .push(Ok(passed_then(vec![AiEvent::RateLimited { resets_at }])));
        provider.script_interrupt().push(Ok(()));
        let limited = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-1"),
                &ScriptedStopSignal::always_active(),
            ),
        );
        assert_eq!(
            limited,
            Err(ChatWebResearchFailure::RateLimited(resets_at)),
            "한도 초과"
        );
        provider.script_run().push(Ok(passed_then(Vec::new())));
        provider.script_interrupt().push(Ok(()));
        let cancelled = run(
            ChatWebResearch::new(&provider, &repo, &ledger, &audit, &clock).execute(
                &research_request("turn-1"),
                &ScriptedStopSignal::with_sequence(Vec::new(), ChatSignalState::Cancelled),
            ),
        );
        assert_eq!(cancelled, Err(ChatWebResearchFailure::Cancelled), "취소");
        let statuses: Vec<ChatTurnStatus> = ledger
            .entries()
            .iter()
            .map(application::ChatRunLedgerEntry::status)
            .collect();
        assert_eq!(
            statuses,
            vec![
                ChatTurnStatus::RateLimited(resets_at),
                ChatTurnStatus::Cancelled
            ],
            "원장 상태 구분"
        );
    }
}
