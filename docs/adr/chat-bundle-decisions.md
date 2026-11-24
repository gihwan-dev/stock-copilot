# 채팅 코어 묶음 결정 기록

ADR 0173 경량 절차에 따라, 문서가 정하지 않은 세부 선택지를 묶음 단위 표로 남긴다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| C-01 | `AiProvider` 기존 계약을 깨지 않기 위해 세션 저장소(`ChatRepository`), 실행 원장(`ChatRunLedger`), 큐 선점(`ChatQueuePort`), 청크 싱크(`ChatChunkSink`), 중단 신호(`ChatStopSignal`)를 별도 포트로 두고, 첫 턴은 `AiProvider::run`, 저장 이력이 있는 후속 턴은 `AiProvider::resume`으로 실행한다 | `AiProvider` 트레이트 시그니처를 직접 수정하거나 `tokio` 채널을 `application`에 직접 도입 |
| C-01 | `SendChatMessage`에서 분석 실행의 허용 MCP 도구 목록(design 3.1의 20개)과 `AiEvent::WebSearch` 차단을 이중 검사하고, 위반 시 즉시 `interrupt` 후 `ChatFailureReason::ForbiddenToolAttempt`로 종료한다 | MCP 서버와 CLI 격리에만 의존하고 스트림 관찰 검사를 생략 |
| C-01 | 취소(`Cancelled`), 시스템/스트림 중단(`Interrupted`), 한도 초과(`RateLimited`)를 서로 다른 `ChatChunk` 변형으로 분리해 낸다 | 세 상태를 `Failed` 하나로 합침 |
| C-09 | 미리 띄운 채팅 실행을 `WarmChatRuntime` 포트 + `WarmChatReadiness`(분석 실행·격리 통과·MCP 연결을 생성 시점에만 검증하는 불변 타입) + `PrewarmChat`·`SendWarmChatMessage`로 분리하고, 만료(기본 10분) 또는 모델·추론 강도 불일치 시 기존 대기 세션을 폐기하고 콜드 분석 실행으로 자동 폴백한다 | `AiProvider` 내부에 암묵적 풀을 숨겨 유스케이스에서 사전 점검 완료 여부를 검증하지 못하게 함 |
