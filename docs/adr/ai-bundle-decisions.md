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
| AI-18 | 프로필은 `(version 1)(allow default)` 위에 주어진 절대 경로마다 `(deny file-read* file-write* (subpath …))`. 경로에 `"`·`\`·제어 문자가 있거나 상대 경로·`/`면 거부해 SBPL 주입을 막는다. 네트워크 규칙은 넣지 않는다(CLI가 구독 API에 닿아야 하고 SBPL은 호스트 단위 허용을 못 한다) | 외부 네트워크 허용 목록 |
| AI-18 | 감싸기는 `/usr/bin/sandbox-exec -p <프로필> <claude> <원래 인자>`이고 환경 변수·작업 폴더는 그대로. `preview_command_plan`이 감싼 명령을 돌려주므로 채팅·일회 실행 모두 같은 경로로 감싼다 | 실행 종류별 선택 |
| AI-18 | 자가 점검은 허용 폴더·거부 폴더의 표지 파일을 같은 프로필(+거부 폴더)로 `/bin/cat`하게 해, 허용 표지만 나오고 종료 코드가 실패면 적용, 거부 표지가 나오면 미적용, 그 밖은 사용 불가로 본다. 적용이 아니면 조립 루트가 겹 없이(`without_sandbox`) 띄운다. 실제 `sandbox-exec` 안에서의 구독 인증 확인은 AI-12(사용자 실측)로 남긴다 | 점검 실패 시 AI 끄기 |
| AI-08 | 종료 기록에 시작·종료 시각(`AiJobTiming`, 소요 시간은 `Timestamp::elapsed_until`)과 토큰 합(`TokenUsage`, 입력·출력·캐시 읽기·캐시 생성)을 둔다. 토큰은 실행 중 `TokenUsage` 사건을 `AiJobUsageMeter`가 포화 덧셈으로 모두 더한다 | 마지막 사건만 쓰기 |
| AI-08 | 조회는 `ListAiJobUsage(from, through)`가 종료 시각 창의 기록을 최근 것부터 돌려준다. 저장은 S-09 전까지 `AiJobRecordStore` 포트와 `testkit-ports` 가짜로만 둔다. 목적별 합계는 AI-10 화면 작업 때 더한다 | 지금 목적별 합계 |
| AI-14 | 수집 실행기는 기존 `WebCollectionRunner` 포트(N-20)를 어댑터의 `ClaudeWebCollectionRunner`가 구현한다. JSON 파싱이 필요해 `application`이 아닌 어댑터에 둔다 | `application`에 serde 추가 |
| AI-14 | 구조화 출력은 `AiEvent`에 변형을 더하지 않고, 결과 줄의 `structured_output`을 `ClaudeEventStream::structured_output()`으로만 넘긴다(사전 점검 0턴 결과 제외, 격리 통과 뒤에만) | `AiEvent::StructuredOutput` 추가 |
| AI-14 | 스키마는 `WEB_COLLECTION_JSON_SCHEMA`(`items[]{url,title,summary?}`, 최대 20). 주소가 `NormalizedUrl`이 아니거나 제목이 비었거나 요약이 문자열이 아니면 그 항목만 버리고, `items` 배열이 없으면 실패(`Unavailable`). 실행 중 `ToolCall`·`McpUnavailable`이 나오면 `Rejected` | 하나라도 틀리면 전체 실패 |
| AI-14 | 수집 실행 ID는 `collect-<시계 초>-<순번>`, 모델은 `Fast`, 추론 강도 `Low` | 설정값 |
| AI-16 | `EnsureVerifiedCliVersion`가 분석 사전 점검 → 관측 버전(`CliVersionSource`, Claude는 사전 점검 init의 `claude_code_version`) → 확인한 버전 목록(`VerifiedCliVersions`)에 있으면 바로 사용, 없으면 수집 사전 점검까지 통과하고 그 사이 버전이 그대로일 때만 목록에 기록한다. 결과가 앞 결과와 다르면 `SelfCheckChanged` 하나를 낸다 | 버전마다 사용자 확인 |
| AI-16 | 실패·버전 모름·저장 실패는 AI만 끈다(`AiGate::Disabled`). 자체 점검에는 4·5번(설치·로그인)이나 6번(`ai_environment`)으로만 남겨 주 화면 띠가 생기지 않는다(로그인 실패는 기존 5번 규칙대로 띠가 생긴다). 점검 호출 자체의 오류는 6번 `CliVersion`으로 본다 | 오류를 4번으로 |
| AI-16 | 목록 저장은 S-09 전까지 포트와 가짜만 둔다. 조립 루트는 앱 시작과 관측 버전이 바뀔 때 부른다 | 매 실행마다 |
| AI-07 | 형식 검증·도구 이름 거부·모든 파일 테스트는 E-04의 `PromptFile`을 그대로 쓰고, `LoadedPrompt::load(작업 폴더, 버전 파일, 본문, 해시)`가 폴더 이름=`task`, 파일 이름=`version`을 더 확인한다. 해시는 `application`에 해시 crate가 없어 로더(어댑터)가 계산한 64자리 소문자 hex `PromptDigest`로 받는다. `PromptRef`(ID·버전·해시)는 `AiJobFinishRecord::with_prompt`로 작업 기록에 남긴다 | `application`에 sha2 추가 |
