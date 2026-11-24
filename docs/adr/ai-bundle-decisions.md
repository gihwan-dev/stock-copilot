# ai 묶음 결정

경량 절차(ADR 0173)로 `ai` 묶음을 구현하며 문서가 정하지 않아 정한 것이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| AI-17 | 실행 ID는 `domain::AiJobId`(영숫자와 `_.-`, 1~128바이트)이고 `application::AiRunId`가 이를 감싼다. `AiJobFinished(AiJobId)`를 확정 이벤트로 바꾼다 | 이벤트를 페이로드 없이 두고 최신 기록만 읽기 |
| AI-17 | 종료 기록 계약은 `AiJobRecordStore::store_finished`·`finished(AiJobId)`이고, `RecordAiJobFinished`가 새로 저장했을 때만 이벤트를 낸다. 같은 ID의 기록이 이미 있으면 이벤트를 내지 않는다 | 저장할 때마다 이벤트 |
| AI-17 | 종료 기록 필드는 ID·종류·목적·결과(완료·중단·격리 거절·대기·실패)·종료 시각. 토큰·소요 시간은 AI-08, 프롬프트 해시는 AI-07에서 더한다 | 처음부터 모든 필드 |
| AI-17 | 구독자: 스케줄러(깨움)와 UI 갱신(`UiEvent::AiJobFinished(ID)`). 알림은 구독하지 않는다 | 알림 포함 |
| AI-17 | 자식 프로세스는 기존대로 `std::process` + 읽기 스레드로 띄우고, `kill_on_drop`은 스트림이 쥔 `ChildCancelGuard`가 프로세스 그룹에 SIGKILL을 보내는 것으로 대신한다. tokio `process` 기능을 새로 켜지 않는다 | `tokio::process::Command` 전환 |
| AI-17 | 일회 실행은 stdin을 결과 줄(`/usage` 사전 점검의 0턴 결과 제외)을 받을 때까지 쥐고 있다가 닫는다 | 프롬프트 직후 닫기(이전 동작) |
| AI-17 | 실행이 끝나면 작업 폴더·설정 폴더(MCP 설정과 토큰 포함)를 지우고, CLI 기록 폴더(`~/.claude/projects/<작업 폴더 경로 인코딩>`)를 선언 경로와 실제 경로(`/tmp`→`/private/tmp`) 둘 다로 지운다 | 선언 경로만 |
| AI-17 | MCP 토큰은 설정에 하나만 두고, 실행마다 새 토큰을 만드는 일은 M-01 연결 때로 미룬다 | 지금 실행마다 새 토큰 |
