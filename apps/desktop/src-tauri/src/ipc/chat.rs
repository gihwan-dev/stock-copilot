use application::ChatSinkClosed;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};

use super::chat_dto::{
    CancelChatTurnRequest, CancelChatTurnResponse, ChatChunkEvent, ChatSessionItemDto,
    GetChatHistoryRequest, GetChatHistoryResponse, ResubscribeChatTurnRequest,
    ResubscribeChatTurnResponse, SendChatMessageRequest, SendChatMessageResponse,
};
use super::chat_hub::{ChatIpcState, ChunkDelivery};
use super::error::IpcError;

fn channel_delivery(channel: Channel<ChatChunkEvent>) -> ChunkDelivery {
    Arc::new(move |event| channel.send(event).map_err(|_error| ChatSinkClosed))
}

#[tauri::command]
#[specta::specta]
pub async fn send_chat_message<R: Runtime>(
    app: AppHandle<R>,
    request: SendChatMessageRequest,
    on_chunk: Channel<ChatChunkEvent>,
) -> Result<SendChatMessageResponse, IpcError> {
    app.state::<ChatIpcState>()
        .send_message(request, channel_delivery(on_chunk))
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_chat_sessions<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ChatSessionItemDto>, IpcError> {
    app.state::<ChatIpcState>().sessions().await
}

#[tauri::command]
#[specta::specta]
pub async fn create_chat_session<R: Runtime>(
    app: AppHandle<R>,
    request: GetChatHistoryRequest,
) -> Result<ChatSessionItemDto, IpcError> {
    app.state::<ChatIpcState>().create(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_chat_history<R: Runtime>(
    app: AppHandle<R>,
    request: GetChatHistoryRequest,
) -> Result<GetChatHistoryResponse, IpcError> {
    app.state::<ChatIpcState>().get_history(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn resubscribe_chat_turn<R: Runtime>(
    app: AppHandle<R>,
    request: ResubscribeChatTurnRequest,
    on_chunk: Channel<ChatChunkEvent>,
) -> Result<ResubscribeChatTurnResponse, IpcError> {
    app.state::<ChatIpcState>()
        .resubscribe(request, channel_delivery(on_chunk))
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_chat_turn<R: Runtime>(
    app: AppHandle<R>,
    request: CancelChatTurnRequest,
) -> Result<CancelChatTurnResponse, IpcError> {
    app.state::<ChatIpcState>().cancel(request).await
}

#[cfg(test)]
mod tests {
    use super::super::chat_dto::{
        ActiveTurnStateDto, CancelChatTurnRequest, CancelOutcomeDto, ChatCardKindDto, ChatChunkDto,
        ChatChunkEvent, ChatExecutionModeDto, ChatFailureReasonDto, ChatModelSelectionDto,
        ChatReasoningEffortDto, ChatSourceDto, ChatSourceKindDto, ChatToolStatusDto,
        ChatTurnStatusDto, GetChatHistoryRequest, ResubscribeChatTurnRequest,
        SendChatMessageRequest,
    };
    use super::super::chat_hub::ChatIpcState;
    use super::channel_delivery;
    use application::{AiEvent, ChatChunk, IsolationCheck, ModelName, ResetTime};
    use domain::{Nanosecond, SpanSeconds, TimeSpan, UntrustedText};
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, mpsc};
    use tauri::ipc::{Channel, InvokeResponseBody};
    use testkit_ports::chat_fakes::{ScriptedAiEventStream, ScriptedStreamStep};

    impl super::super::chat_hub::ChatStreamHub {
        async fn register_turn(
            &self,
            session: String,
            turn: String,
            subscriber: super::ChunkDelivery,
        ) -> Arc<super::super::chat_hub::TurnStopSignal> {
            self.start_turn(session, turn, subscriber)
                .await
                .expect("새 턴")
        }
    }

    const RESPONSE_DIRECTORY: &str = "ipc/responses";

    fn assert_response_snapshot<T: serde::Serialize>(name: &str, value: &T) {
        let path = Path::new(RESPONSE_DIRECTORY).join(format!("{name}.json"));
        let mut actual = serde_json::to_string_pretty(value).expect("serialize response");
        actual.push('\n');
        let recorded = fs::read_to_string(&path).expect("read recorded response");
        assert_eq!(
            recorded,
            actual,
            "{} differs from the serialized response",
            path.display()
        );
    }

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("비신뢰 텍스트")
    }

    fn channel_collector() -> (Channel<ChatChunkEvent>, mpsc::Receiver<String>) {
        let (sender, receiver) = mpsc::channel();
        let channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(payload) = body {
                let _sent = sender.send(payload);
            }
            Ok(())
        });
        (channel, receiver)
    }

    fn sample_send_request(turn: &str, run: &str, msg: &str) -> SendChatMessageRequest {
        SendChatMessageRequest {
            session_id: String::from("session-ipc-1"),
            turn_id: String::from(turn),
            run_id: String::from(run),
            user_message_id: String::from(msg),
            message: String::from("삼성전자 현황 알려 줘"),
            context: None,
            model: ChatModelSelectionDto::Default,
            effort: ChatReasoningEffortDto::Medium,
        }
    }

    #[test]
    fn chat_ipc_requests_reject_unknown_and_order_or_settings_fields() {
        let valid = r#"{"session_id":"s1","turn_id":"t1","run_id":"r1","user_message_id":"m1","message":"안녕","context":null,"model":"Default","effort":"Medium"}"#;
        assert!(
            serde_json::from_str::<SendChatMessageRequest>(valid).is_ok(),
            "정상 채팅 요청 허용"
        );
        for forbidden in [
            r#"{"session_id":"s1","turn_id":"t1","run_id":"r1","user_message_id":"m1","message":"매수","context":null,"model":"Default","effort":"Medium","approved":true}"#,
            r#"{"session_id":"s1","turn_id":"t1","run_id":"r1","user_message_id":"m1","message":"매수","context":null,"model":"Default","effort":"Medium","quantity":"10"}"#,
            r#"{"session_id":"s1","turn_id":"t1","run_id":"r1","user_message_id":"m1","message":"매수","context":null,"model":"Default","effort":"Medium","order_enabled":true}"#,
        ] {
            assert!(
                serde_json::from_str::<SendChatMessageRequest>(forbidden).is_err(),
                "추가 필드 거부: {forbidden}"
            );
        }
        assert!(
            serde_json::from_str::<GetChatHistoryRequest>(r#"{"session_id":"s1","extra":1}"#)
                .is_err(),
            "이력 조회 추가 필드 거부"
        );
        assert!(
            serde_json::from_str::<ResubscribeChatTurnRequest>(
                r#"{"turn_id":"t1","after_sequence":0,"approved":true}"#
            )
            .is_err(),
            "재구독 추가 필드 거부"
        );
        assert!(
            serde_json::from_str::<CancelChatTurnRequest>(r#"{"turn_id":"t1","price":"100"}"#)
                .is_err(),
            "취소 추가 필드 거부"
        );
    }

    #[test]
    fn every_chat_chunk_variant_including_cancel_interrupt_and_rate_limit_serializes() {
        let variants = vec![
            ChatChunkDto::TextDelta {
                text: String::from("조각"),
            },
            ChatChunkDto::ToolCallStarted {
                server: String::from("stock-copilot"),
                tool: String::from("get_portfolio"),
            },
            ChatChunkDto::ToolCallFinished {
                server: String::from("stock-copilot"),
                tool: String::from("get_portfolio"),
                status: ChatToolStatusDto::Succeeded,
            },
            ChatChunkDto::Card {
                kind: ChatCardKindDto::Signal,
                id: String::from("sig-1"),
            },
            ChatChunkDto::Sources {
                items: vec![ChatSourceDto {
                    kind: ChatSourceKindDto::Tool,
                    reference: String::from("get_portfolio"),
                    url: None,
                }],
            },
            ChatChunkDto::Done {
                session_id: String::from("session-ipc-1"),
                turn_id: String::from("turn-1"),
                run_id: String::from("run-1"),
                mode: ChatExecutionModeDto::FreshAnalysis,
                actual_model: Some(String::from("claude-sonnet-5-5")),
                first_token_latency_ms: Some(250),
                finished_at_unix_seconds: String::from("91"),
            },
            ChatChunkDto::Cancelled,
            ChatChunkDto::Interrupted,
            ChatChunkDto::RateLimited {
                resets_at_unix_seconds: Some(String::from("3600")),
            },
            ChatChunkDto::Failed {
                reason: ChatFailureReasonDto::ForbiddenToolAttempt,
            },
        ];
        let encoded = serde_json::to_string(&variants).expect("전체 변형 직렬화");
        for token in [
            "TextDelta",
            "ToolCallStarted",
            "ToolCallFinished",
            "Card",
            "Sources",
            "Done",
            "Cancelled",
            "Interrupted",
            "RateLimited",
            "Failed",
        ] {
            assert!(encoded.contains(token), "변형 누락: {token}");
        }
    }

    fn assert_resubscribe_and_cancel_snapshots(state: &ChatIpcState) {
        let (resub_channel, resub_received) = channel_collector();
        let resub = tauri::async_runtime::block_on(state.resubscribe(
            ResubscribeChatTurnRequest {
                turn_id: String::from("turn-1"),
                after_sequence: 1,
            },
            channel_delivery(resub_channel),
        ))
        .expect("재구독");
        assert_eq!(resub.replayed_count, 2, "놓친 2·3번 조각 2건 재전송");
        assert_eq!(resub_received.try_iter().count(), 2, "재전송 조각 수신");
        assert_response_snapshot("resubscribe_chat_turn", &resub);
        let cancel_resp = tauri::async_runtime::block_on(state.cancel(CancelChatTurnRequest {
            turn_id: String::from("turn-1"),
        }))
        .expect("종료된 턴 취소 응답");
        assert_eq!(cancel_resp.outcome, CancelOutcomeDto::NotRunning, "종료됨");
        assert_response_snapshot("cancel_chat_turn", &cancel_resp);
    }

    #[test]
    fn send_history_resubscribe_and_cancel_match_recorded_snapshots_and_measure_latency() {
        let ports = crate::tests::support::ports();
        let latency =
            TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(420_000_000)).expect("지연");
        ports
            .ai_provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::with_clock(
                vec![
                    ScriptedStreamStep::Event(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                    ScriptedStreamStep::Event(AiEvent::ActualModel(
                        ModelName::try_new("claude-sonnet-5-5").expect("모델"),
                    )),
                    ScriptedStreamStep::AdvanceAndEvent {
                        step: latency,
                        event: AiEvent::TextDelta(text("보유 종목을 확인했습니다.")),
                    },
                ],
                Arc::clone(&ports.clock),
            )));
        let state = ChatIpcState::from_ports(&ports);
        let (channel, received) = channel_collector();
        let send_resp = tauri::async_runtime::block_on(state.send_message(
            sample_send_request("turn-1", "run-1", "msg-1"),
            channel_delivery(channel),
        ))
        .expect("채팅 전송");
        assert_eq!(send_resp.status, ChatTurnStatusDto::Completed, "완료 상태");
        assert_eq!(
            send_resp.first_token_latency_ms,
            Some(420),
            "첫 글자 지연 420ms"
        );
        assert_response_snapshot("send_chat_message", &send_resp);
        assert_eq!(
            received.try_iter().count(),
            3,
            "TextDelta + Sources + Done 3건"
        );
        let history = tauri::async_runtime::block_on(state.get_history(GetChatHistoryRequest {
            session_id: String::from("session-ipc-1"),
        }))
        .expect("이력 조회");
        assert_eq!(history.entries.len(), 2, "사용자 + 어시스턴트 저장분");
        assert_response_snapshot("get_chat_history", &history);
        assert_resubscribe_and_cancel_snapshots(&state);
    }

    #[test]
    fn reload_resubscribes_mid_stream_and_receives_replayed_and_live_chunks() {
        let state = ChatIpcState::from_ports(&crate::tests::support::ports());
        let hub = state.hub();
        let (first_channel, first_received) = channel_collector();
        let _stop = tauri::async_runtime::block_on(hub.register_turn(
            String::from("session-reload"),
            String::from("turn-live"),
            channel_delivery(first_channel),
        ));
        tauri::async_runtime::block_on(
            hub.publish("turn-live", &ChatChunk::TextDelta(text("창 새로고침 전 "))),
        );
        assert_eq!(first_received.try_iter().count(), 1, "첫 조각 전달");
        let active = tauri::async_runtime::block_on(hub.active_turn_for_session("session-reload"))
            .expect("활성 턴 조회");
        assert_eq!(
            (active.turn_id.as_str(), active.last_sequence, active.state),
            ("turn-live", 1, ActiveTurnStateDto::Running),
            "진행 중 턴 상태"
        );
        let (second_channel, second_received) = channel_collector();
        let resub = tauri::async_runtime::block_on(state.resubscribe(
            ResubscribeChatTurnRequest {
                turn_id: String::from("turn-live"),
                after_sequence: 0,
            },
            channel_delivery(second_channel),
        ))
        .expect("새 채널 재구독");
        assert_eq!(
            (resub.replayed_count, resub.state),
            (1, ActiveTurnStateDto::Running),
            "기존 조각 재전송 및 구독 유지"
        );
        tauri::async_runtime::block_on(async {
            hub.publish(
                "turn-live",
                &ChatChunk::ToolCallStarted {
                    server: text("stock-copilot"),
                    tool: text("get_quote"),
                },
            )
            .await;
            hub.publish("turn-live", &ChatChunk::Cancelled).await;
            hub.publish(
                "turn-live",
                &ChatChunk::RateLimited {
                    resets_at: ResetTime::NotProvided,
                },
            )
            .await;
        });
        assert_eq!(
            second_received.try_iter().count(),
            4,
            "재구독한 새 채널이 기존 + 이후 실시간 조각을 모두 수신"
        );
    }

    #[test]
    fn cancel_command_signals_running_turn_stop_handle() {
        let state = ChatIpcState::from_ports(&crate::tests::support::ports());
        let hub = state.hub();
        let (channel, _received) = channel_collector();
        let stop = tauri::async_runtime::block_on(hub.register_turn(
            String::from("session-cancel"),
            String::from("turn-cancel"),
            channel_delivery(channel),
        ));
        let cancel_resp = tauri::async_runtime::block_on(state.cancel(CancelChatTurnRequest {
            turn_id: String::from("turn-cancel"),
        }))
        .expect("취소 신호");
        assert_eq!(
            cancel_resp.outcome,
            CancelOutcomeDto::Signaled,
            "취소 전달됨"
        );
        assert_eq!(
            application::ChatStopSignal::state(stop.as_ref()),
            application::ChatSignalState::Cancelled,
            "코어 중단 신호 반영"
        );
    }
    #[test]
    fn session_commands_persist_empty_chats_and_return_ordered_first_message_titles() {
        let ports = crate::tests::support::ports();
        let state = ChatIpcState::from_ports(&ports);
        let lookup = |id: &str| GetChatHistoryRequest {
            session_id: id.to_owned(),
        };
        tauri::async_runtime::block_on(async {
            assert_eq!(state.sessions().await.expect("목록").len(), 0_usize);
            assert_response_snapshot(
                "create_chat_session",
                &state.create(lookup("session-new")).await.expect("생성"),
            );
            let _legacy = state
                .create(lookup("main-chat"))
                .await
                .expect("기존 기본 대화");
            let mut send = sample_send_request("turn-main", "run-main", "msg-main");
            send.session_id = String::from("main-chat");
            ports
                .ai_provider
                .script_run()
                .push(Ok(ScriptedAiEventStream::from_events(vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::TextDelta(text("기본 대화 답변")),
                ])));
            let (channel, _received) = channel_collector();
            let _reply = state
                .send_message(send, channel_delivery(channel))
                .await
                .expect("기본 대화 전송");
            let listed = state.sessions().await.expect("목록");
            let [main, empty] = listed.as_slice() else {
                panic!("두 대화")
            };
            assert_response_snapshot("list_chat_sessions", &listed);
            assert_eq!(empty.title, None);
            assert_eq!(
                state.create(lookup("main-chat")).await.expect("재시도"),
                main.clone()
            );
            let legacy = state
                .get_history(lookup("main-chat"))
                .await
                .expect("기본 이력");
            assert_eq!(legacy.entries.len(), 2_usize);
            let new = state
                .get_history(lookup("session-new"))
                .await
                .expect("빈 이력");
            assert_eq!(new.entries.len(), 0_usize);
            assert_eq!(new.active_turn, None);
        });
    }

    async fn assert_session_replay(
        state: &ChatIpcState,
        session: &str,
        turn: &str,
        expected: &str,
    ) {
        let history = state
            .get_history(GetChatHistoryRequest {
                session_id: session.to_owned(),
            })
            .await
            .expect("이력");
        assert_eq!(history.active_turn.expect("활성 턴").turn_id, turn);
        let (channel, received) = channel_collector();
        let replay = state
            .resubscribe(
                ResubscribeChatTurnRequest {
                    turn_id: turn.to_owned(),
                    after_sequence: 0,
                },
                channel_delivery(channel),
            )
            .await
            .expect("재구독");
        assert_eq!(replay.replayed_count, 1_u32);
        assert!(received.try_recv().expect("재전송").contains(expected));
    }

    #[test]
    fn concurrent_session_streams_replay_and_cancel_only_the_selected_turn() {
        let state = ChatIpcState::from_ports(&crate::tests::support::ports());
        let hub = state.hub();
        tauri::async_runtime::block_on(async {
            let (left_channel, _) = channel_collector();
            let (right_channel, _) = channel_collector();
            let left = hub
                .register_turn(
                    "main-chat".to_owned(),
                    "turn-a".to_owned(),
                    channel_delivery(left_channel),
                )
                .await;
            let right = hub
                .register_turn(
                    "session-new".to_owned(),
                    "turn-b".to_owned(),
                    channel_delivery(right_channel),
                )
                .await;
            let (duplicate, _) = channel_collector();
            let error = hub
                .start_turn(
                    "session-new".to_owned(),
                    "turn-a".to_owned(),
                    channel_delivery(duplicate),
                )
                .await
                .expect_err("중복 턴");
            assert_eq!(error.kind, super::super::error::IpcErrorKind::UserFixable);
            hub.publish("turn-a", &ChatChunk::TextDelta(text("왼쪽")))
                .await;
            hub.publish("turn-b", &ChatChunk::TextDelta(text("오른쪽")))
                .await;
            for (session, turn, expected) in [
                ("main-chat", "turn-a", "왼쪽"),
                ("session-new", "turn-b", "오른쪽"),
            ] {
                assert_session_replay(&state, session, turn, expected).await;
            }
            let _cancel = state
                .cancel(CancelChatTurnRequest {
                    turn_id: "turn-b".to_owned(),
                })
                .await
                .expect("취소");
            assert_eq!(
                application::ChatStopSignal::state(left.as_ref()),
                application::ChatSignalState::Active
            );
            assert_eq!(
                application::ChatStopSignal::state(right.as_ref()),
                application::ChatSignalState::Cancelled
            );
        });
    }
}
