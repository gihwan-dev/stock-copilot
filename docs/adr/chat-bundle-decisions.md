# 채팅 코어 묶음 결정 기록

ADR 0173 경량 절차에 따라, 문서가 정하지 않은 세부 선택지를 묶음 단위 표로 남긴다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| C-01 | `AiProvider` 기존 계약을 깨지 않기 위해 세션 저장소(`ChatRepository`), 실행 원장(`ChatRunLedger`), 큐 선점(`ChatQueuePort`), 청크 싱크(`ChatChunkSink`), 중단 신호(`ChatStopSignal`)를 별도 포트로 두고, 첫 턴은 `AiProvider::run`, 저장 이력이 있는 후속 턴은 `AiProvider::resume`으로 실행한다 | `AiProvider` 트레이트 시그니처를 직접 수정하거나 `tokio` 채널을 `application`에 직접 도입 |
| C-01 | `SendChatMessage`에서 분석 실행의 허용 MCP 도구 목록(design 3.1의 20개)과 `AiEvent::WebSearch` 차단을 이중 검사하고, 위반 시 즉시 `interrupt` 후 `ChatFailureReason::ForbiddenToolAttempt`로 종료한다 | MCP 서버와 CLI 격리에만 의존하고 스트림 관찰 검사를 생략 |
| C-01 | 취소(`Cancelled`), 시스템/스트림 중단(`Interrupted`), 한도 초과(`RateLimited`)를 서로 다른 `ChatChunk` 변형으로 분리해 낸다 | 세 상태를 `Failed` 하나로 합침 |
| C-09 | 미리 띄운 채팅 실행을 `WarmChatRuntime` 포트 + `WarmChatReadiness`(분석 실행·격리 통과·MCP 연결을 생성 시점에만 검증하는 불변 타입) + `PrewarmChat`·`SendWarmChatMessage`로 분리하고, 만료(기본 10분) 또는 모델·추론 강도 불일치 시 기존 대기 세션을 폐기하고 콜드 분석 실행으로 자동 폴백한다 | `AiProvider` 내부에 암묵적 풀을 숨겨 유스케이스에서 사전 점검 완료 여부를 검증하지 못하게 함 |
| C-02 | `src-tauri`의 채팅 IPC를 `send_chat_message`, `get_chat_history`, `resubscribe_chat_turn`, `cancel_chat_turn` 4개 명령과 순번 기반 버퍼 `ChatStreamHub`로 구성하고, `Channel` API 허용은 `src/ipc/chat.rs` 한 파일로 제한한다. `ChatIpcState`는 제네릭 `Ports<P>`에서 채팅 포트만 클론하는 비제네릭 상태(`ChatBackend`)로 조립해 `ProductPorts`와 `TestPorts` 모두 동일 경로로 동작하며 주문·설정 포트에 접근할 수 없다 | `Ports<ProductPorts>`에 직접 묶어 `TestPorts` 조립에서 채팅 명령을 시험하지 못하거나, `Channel`을 여러 모듈에 흩어 놓음 |
| C-10 | `ChatWebResearch` 유스케이스는 세션·턴 ID만 받고(자유 문장 필드 없음) 그 턴에 저장된 사용자 메시지 원문을 `CollectionQuery::UserMessage`로 수집 실행에 넘긴다. 격리 확인 전 이벤트·MCP 도구 호출은 중단 후 거부, 결과는 3만 자로 자른 `UntrustedText`만 돌려주고 원장에 `Collection`/`WebCollection`으로 남긴다. 코드 주제 모드·빈도 제한·MCP 도구 연결은 N-18에 둔다 | 채팅 실행이 넘긴 검색 문장을 받거나, 메시지 ID를 AI가 고르게 함 |
| E-09 | 기본 설정의 투자 성향 저장은 `UpdateInvestorProfile` 하나로 받고, 결과는 기존 `RiskSettingsChangeOutcome`을 재사용한다. 기간을 몇 시간 단위로 새로 바꾸면 `AllowHours`, 비중 상한 완화는 `RelaxWeightCaps`, 위험 성향 상향은 `IncreaseRiskAppetite` 확인을 차례로 받고 하나라도 거부·불가면 아무것도 저장하지 않는다. 몇 시간 단위 기간은 고급 허용(위험 설정의 `hours`)이 켜져 있어야만 받는다. 위험 설정과의 동기화는 저장소의 `store_profile` 계약에 맡긴다. 화면·IPC는 이 묶음 범위 밖 | 투자 성향과 위험 설정을 따로 저장하는 두 유스케이스를 화면이 차례로 부름 |
| C-03 | 대화는 고정 세션 하나(`main-chat`)로 이어 가고, 새로 고침 뒤에는 `get_chat_history`로 저장분을 다시 그린 뒤 실행 중인 턴만 `resubscribe_chat_turn`(순번 0부터)으로 다시 받는다. 응답 없이 남은 사용자 턴은 "끊김"으로 닫는다. 도구 이름은 `shared/ipc` 경계에서 사용자 주제(`ChatToolTopic`)로 바꿔 화면에 내부 이름이 닿지 않는다. 턴·실행·메시지 ID는 WebView가 시각과 난수로 만든다. 보내기·멈추기·링크 열기는 효과 훅 대신 TanStack `useMutation`으로 부른다. 채팅 IPC는 새 허용 목록 항목 없이 기존 `call.ts`·`schema.ts`(`Wire` 브랜드)와 `brand.ts`의 공개 스키마로 짓는다 | 세션 목록 화면, `crypto.randomUUID`(DOM 값 금지), 새 허용 목록 파일 추가 |
