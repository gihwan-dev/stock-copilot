# T-02 토큰 관리

- 상태: 초안

## 결정

키는 SecretStore의 TossClientId·TossClientSecret만 읽고 issue_token으로 발급한다. 토큰은 SecretValue를 소유하는 비공개 Token을 Arc로 공유하며 영속 저장하지 않는다. 응답의 토큰 필드는 전용 serde visitor가 SecretValue로 즉시 받는다. 이스케이프를 디코딩한 문자열도 같은 비밀 타입으로 들어가며, 소유 String을 받는 경계는 그 바이트 할당을 SecretValue로 즉시 옮긴다. Bearer 타입은 닫힌 열거형으로 디코딩한다. 전용 TokenLease의 Debug와 관리자의 Debug에는 비밀을 싣지 않는다. 실제 키·계좌·외부 API는 사용하지 않는다.

Clock이 제공하는 발급 요청 시작 시각에 expires_in을 더한다. 응답 수신 지연은 수명을 늘리지 않는다. 60초 전부터 갱신하며 수명 2~60초는 만료 1초 전이 아니라 수명에서 1초를 남기는 여유를 잡아 발급 1초 뒤 갱신한다. 수명 0·1초, 범위 초과, Bearer 이외, 빈 토큰·헤더에 부적합한 문자는 닫힌 InvalidResponse다. 문서가 갱신 여유 수치를 정하지 않아 보수적으로 정했다. 호출자가 token을 주기적으로 호출하는 예약 연결은 J-01에 인계한다.

관리자 한 개를 앱 조립에서 Arc로 공유한다. A-10 전역 EventBus도 Arc로 주입해 다른 유스케이스와 동일 버스를 공유한다. tokio sync 기능을 추가하고 한 비동기 잠금으로 발급·후속 복구를 직렬화한다. 동시 대기자는 진행 중인 발급 결과를 재사용한다. 성공한 발급을 기다린 세대는 갱신 시점에 들어섰어도 실제 만료 전까지 그 결과를 공유한다. 실패 회차도 세대 번호를 비교해 이미 대기하던 요청에는 같은 실패를 전달한다. 이후 새 요청은 다시 시도할 수 있다. 잠금은 일반 GET 응답을 기다리는 동안 잡지 않는다. 토큰 발급 REST와 네 갱신 단계는 잠금 안에서 직렬화되므로 상태/발행 보고 조회도 그 완료를 기다린다. 이전 lease로 거절된 요청은 이미 새 토큰이 있으면 추가 발급하지 않는다.

발급 시도가 시작되면 이전 토큰을 관리자로부터 제거한다. 발급이 성공했지만 응답이 소실됐을 수 있으므로 실패 시에도 이전 토큰을 다시 쓰지 않는다. 연결 상태는 Disconnected·Connected·DuplicateKey다. 발급 → close → reconnect(새 토큰) → resubscribe → reconcile_orders 순서가 끝나고 아직 만료되지 않아야 Connected다. 어느 단계든 실패하면 Disconnected다. 같은 상태는 이벤트를 다시 내지 않는다. 발행 보고서 최신 한 개를 보존해 호출자가 실패 구독자를 확인할 수 있게 한다.

401 token-revoked에는 그 요청이 직접 수행하는 재발급이 최대1회다. 계획 갱신이나 다른 요청의 발급 결과를 재사용한 것은 자체 재발급에 포함하지 않는다. 재사용한 토큰이 다시 revoked여도 아직 자체 발급하지 않은 요청은 발급1회를 수행할 수 있다. 자체 재발급 뒤 다시 인증 실패하면 요청을 닫는다. ADR 0074의 T-02 인계에 따라 invalid-token도 동일한 1회 재발급을 수행한다. 새 토큰으로도 invalid-token이면 닫힌 Rejected(InvalidToken)와 Disconnected로 끝내며 중복 키로 오분류하지 않는다. 자체 재발급 뒤 현재 토큰이 다시 revoked면 DuplicateKey와 RevokedRepeated로 닫고 이후 자동 발급도 중단한다. 연결 조정자는 다른 프로그램 종료 후의 재점검에 retry_connection을 호출해 같은 관리자를 회복할 수 있다. 일반 token/get 호출이 이 정지를 자동 해제하지 않는다. 재조립이나 앱 재시작을 사용자에게 요구하지 않는다. 자동 재발급 무한 반복을 막기 위한 보수적 선택이다.

TokenRenewal은 웹소켓 네 단계의 실행 계약이다. 구현체에 주문 전송 권한을 주지 않으며 마지막 단계는 읽기 주문 재조회다. reconcile_orders는 새 SecretValue 참조를 직접 받아 같은 관리자의 잠금을 재진입하거나 토큰을 따로 복사/저장할 필요가 없다. 실제 웹소켓은 T-17이 구현한다. 새 HTTP 수단이나 주문 경로를 추가하지 않는다. 사용자 설정·화면 상태는 추가하지 않는다.

## 의존성 및 승인 항목

- tokio =1.54.1, default-features=false, 기존 rt/time에 sync만 추가, MIT. 새 패키지와 새 전이 의존성 없음.
- serde는 기존 workspace 버전을 일반 의존성에서도 사용한다. 테스트에 이미 있던 동일 버전·기능이며 새 패키지 없음.
- testkit에 별도 ScriptedHttpServer 도우미를 추가한다. 기존 LocalHttpServer는 수정하지 않는다. 기존 테스트와 단언은 변경하지 않는다.
- 프로젝트 설정 hooks=true와 현재 세션의 프로젝트/훅 신뢰 적용 여부를 분리한다. 세션 적용은 확인할 수 없다. frozen install과 lefthook install을 완료했다.

## 인계

- T-17: TokenRenewal을 구현해 close/reconnect/resubscribe/reconcile_orders를 연결한다. 마지막 단계는 인자로 빌려 받은 새 토큰으로 직접 GET을 실행하고 같은 관리자에 재진입하지 않는다. 네 단계 실패는 호출자에게 반환하며 실제 주문은 전송하지 않는다.
- T-03: TokenManager::get은 token/revoked 흐름만 제공한다. 그룹 속도 제한은 TossHttpClient 호출 전의 어댑터 연결부에서 결합한다. AUTH 발급과 최초 GET, 공유 토큰 재조회 및 자체 재발급 뒤 재조회 각각이 전송되기 전에 제한기를 거쳐야 한다. 공통 HTTP 구조는 변경하지 않았다.
- T-05·T-16: 조립한 단일 TokenManager를 공유한다. TokenLease는 비밀을 노출해야 하는 경계에서만 사용한다.
- J-01·T-05·U-07: Clock 기준 token 호출을 예약해 유휴 중에도 만료 전 갱신한다. 반복 revoked 뒤에는 기존 연결 점검 흐름에서 retry_connection을 사용해 앱 재시작 없이 회복한다. access_denied/허용 IP 오류 뒤 일반 호출을 자동 재시도하지 않고, 안내가 떠 있는 동안에만 1분에 1회 이하로 발급을 재점검한다(REL-1). 이 호출 예약/화면 상태 조건은 연결 조정자의 책임이며 T-03의 일반 그룹 속도 제한과 구분한다. TokenManager는 벽시계/자체 타이머를 만들지 않는다.

## 검증

미구현 모듈 E0583 red를 먼저 확인했다. 루프백 서버와 가짜 Clock으로 동시 발급, 갱신 경계, 만료 전·정각·직후, 실패/회복, revoked 중단, 비밀 Debug, 갱신 순서를 검증한다. 최종 검사·리뷰·손 변형 결과는 아래에 실제 실행 결과만 기록한다.

## 손 변형 결과

별도 mktemp 복사본에서 대조군과 원복 대조군이 각각 8개 테스트를 통과했다. 아래 22개는 모두 실제 컴파일 후 테스트 실패로 검출했다. 생존·컴파일 불가·시간 초과 0건이다. 원시 결과는 target/T-02/manual-results.json 및 manual-logs에 있다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | Bearer 반전 | tests::revoked_retry_can_recover_and_other_statuses_do_not_reissue, tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation | 검출 |
| 2 | 빈 토큰 검사 반전 | tests::revoked_retry_can_recover_and_other_statuses_do_not_reissue, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 3 | 최소 수명 완화 | tests::malformed_tokens_are_closed_errors_and_never_become_credentials | 검출 |
| 4 | 수명 오류 | tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted, tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 5 | 갱신 여유 59 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 6 | 갱신 여유 61 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 7 | 갱신시각 덧셈 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 8 | 캐시 경계 반전 | tests::expiry_failures_and_recovery_publish_only_changed_state, tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted | 검출 |
| 9 | 갱신 정각 제외 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 10 | 성공 상태 오지정 | tests::expiry_failures_and_recovery_publish_only_changed_state, tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted | 검출 |
| 11 | 실패 상태 오지정 | tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 12 | 동일 상태 알림 | tests::expiry_failures_and_recovery_publish_only_changed_state, tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted | 검출 |
| 13 | 이벤트 종류 오지정 | tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 14 | 닫기 누락 | tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed, tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation | 검출 |
| 15 | 재연결 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 16 | 구독 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 17 | 주문 재조회 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 18 | revoked 상태 코드 | tests::revoked_reissues_once_then_stops_all_requests, tests::revoked_retry_can_recover_and_other_statuses_do_not_reissue | 검출 |
| 19 | 토큰 필드 교환 | tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted, tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 20 | 키 필드 교환 | tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted | 검출 |
| 21 | 재발급 응답 폐기 | tests::revoked_retry_can_recover_and_other_statuses_do_not_reissue | 검출 |
| 22 | 중복 키 차단 누락 | tests::revoked_reissues_once_then_stops_all_requests | 검출 |

## 하네스 정합화

기존 testkit LocalHttpServer의 impl에 도우미를 추가한 최초 변경은 test-diff가 거부했다. 기존 파일을 완전히 원복하고 새 ScriptedHttpServer 파일로 분리했으며 test-diff 위반 0을 확인했다. 새 토큰 테스트의 발급 횟수는 /oauth2/token HTTP/1.1 경로로 센다. POST 메서드 자체는 기존 T-18 테스트가 고정한다. 구조 검사는 POST뿐 아니라 일반 변수 이름 request 및 같은 문자열도 쓰기 수단으로 예약하므로 새 시험의 일반 조회 변수/캡처 표기도 lookup/captured로 명확히 했다. 새 시험 파일의 쓰기 수단 표기와 순서 기록 orders 문자열 조합을 주문 쓰기로 보는 구조 규칙을 그대로 유지하기 위한 배치이며 검사 끄기/허용 범위 확대는 없다.

## 리뷰 1 수정

비판에서 실행 확정한 A(차단), 1·B·7(중요)를 각각 회귀 테스트 red로 재현했다. 최초 8통과·4실패를 확인한 뒤 수정해 12개를 통과했다. 취소 시도 정리, 성공 세대 공유, 최신 lease 비교, JSON 이스케이프를 수정했다.

취소로 발급 future가 버려져도 시도 범위의 Drop 정리가 Renewal 오류와 완료 세대를 남기고 Disconnected 전이를 한 번 발행한다. 기존 대기자는 그 취소 실패를 공유하며 이후 새 호출은 회복할 수 있다. 갱신 단계 성공 결과를 반영하기 전에 완료 표식을 세워 구독자 패닉 중에 취소 정리를 재발행하지 않는다. 패닉을 회복하거나 삼키는 경로는 추가하지 않는다.

두 번째 revoked 응답도 현재 lease와 비교한다. 이미 계획 갱신으로 교체된 lease의 오래된 응답은 Superseded로 요청만 닫고 최신 토큰·연결 상태를 건드리지 않는다. 이전 회차에서는 세 번째 재조회를 금지했으나 2차 메인 결정으로 대체한다. 공유 토큰 재사용을 자체 재발급으로 세지 않으며, 자체 발급 뒤 현재 lease가 다시 revoked인 경우에만 DuplicateKey로 닫는다.

3번 지적의 앱 재시작 필수 주장은 기각됐다. 남은 조립 불확실성은 같은 인스턴스의 명시적 retry_connection 계약과 T-05/J-01/T-17/U-07 인계로 보완한다. 새 UI 절차나 설정을 만들지 않는다. 2번의 요청 간 누적 제한은 구체 브리프와 달라 기각한다. 4번의 5xx 비발급 보장도 자료가 없어 기각하며 기존 보수적 폐기 규칙을 유지한다.

후속 새 API·서명 변경은 이 브랜치의 새 테스트만 수정한다. master의 기존 테스트 준비/단언 변경은 0건이며 승인 예외를 쓰지 않는다. 동시성 반례 서버는 새 ScriptedHttpServer 안에서 특정 응답을 보류/해제하는 테스트 전용 계약으로 추가한다. 기존 단발 서버는 그대로 둔다.

## 수정 회차 전체 검사 재시도

최초 정상 훅은 기존 Git 훅 시험 commit_guard_ignores_build_outputs_listed_in_the_real_gitignore의 하위 commit-guard가 signal 9로 종료해 실패했다. 전체 1456개 중 1251개 실행 시점에 1250통과·1실패였고 205개는 미실행이다. 원인은 확정하지 않았다. 제품/하네스 소스와 단언을 바꾸지 않고 CARGO_BUILD_JOBS=2로 같은 시험을 격리 실행해 1/1 통과했다(그 실행에서 나머지 316개는 선택하지 않음). 같은 빌드 병렬도2로 정상 커밋 훅 전체를 다시 실행하며, 전체 시험 선택과 속성 반복 수/검사 규칙은 줄이지 않는다. T-18의 기존 정상 훅 기록에도 같은 signal9 관측과 병렬도2 재실행이 있으나 이번 실패의 원인 증명으로 취급하지 않는다.

## 리뷰 수정 뒤 손 변형 결과

1차 수정65a6d575의 핵심 소스와 당시 복사본 원복 소스의 SHA-256이 같다: `5c242883812a9d48b4b3752f59298e24c0e28fbafb116e91e1f0af2bfa3138e5`. 별도 mktemp 복사본의 대조군/원복 대조군은 각각 18개 수용 테스트를 통과했다. 아래 35개는 모두 실제 컴파일 후 테스트 실패로 검출했고 생존·컴파일 불가·시간 초과는 0이다. 최초 22개 표는 최초 구현 회차 기록이며 아래1차 수정 증거와 합산하지 않는다. 변형 복사본의 컴파일에만 cargo-mutants와 같은 cap-lints 모드를 사용했다. 제품 검사와 보호 규칙은 끄지 않았다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | Bearer 분류 변조 | tests::invalid_token_also_reissues_once_as_required_by_t04_handoff, tests::equivalent_json_escapes_preserve_token_and_bearer_type | 검출 |
| 2 | 빈 토큰 검사 반전 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::connection_changes_use_the_same_bus_as_other_application_publishers | 검출 |
| 3 | 최소 수명 완화 | tests::malformed_tokens_are_closed_errors_and_never_become_credentials | 검출 |
| 4 | 수명 오류 | tests::one_second_before_expiry_can_still_be_shared_by_the_waiting_generation, tests::queued_requests_share_success_even_when_refresh_becomes_due_during_issuance | 검출 |
| 5 | 갱신 여유 59 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token, tests::obsolete_revoked_retry_does_not_disconnect_a_newer_successful_token | 검출 |
| 6 | 갱신 여유 61 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 7 | 갱신시각 덧셈 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 8 | 캐시 경계 반전 | tests::expiry_failures_and_recovery_publish_only_changed_state, tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token | 검출 |
| 9 | 갱신 정각 제외 | tests::refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token, tests::obsolete_revoked_retry_does_not_disconnect_a_newer_successful_token | 검출 |
| 10 | 성공 상태 오지정 | tests::connection_changes_use_the_same_bus_as_other_application_publishers, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 11 | 실패 상태 오지정 | tests::expiry_instant_and_after_expiry_share_failure_and_never_return_expired_token, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 12 | 동일 상태 알림 | tests::expiry_failures_and_recovery_publish_only_changed_state, tests::connection_changes_use_the_same_bus_as_other_application_publishers | 검출 |
| 13 | 이벤트 종류 오지정 | tests::connection_changes_use_the_same_bus_as_other_application_publishers, tests::expiry_failures_and_recovery_publish_only_changed_state | 검출 |
| 14 | 닫기 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 15 | 재연결 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 16 | 구독 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed | 검출 |
| 17 | 주문 재조회 누락 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation, tests::expiry_instant_and_after_expiry_share_failure_and_never_return_expired_token | 검출 |
| 18 | revoked 상태 코드 | tests::revoked_reissues_once_then_stops_all_requests, tests::repeated_invalid_token_closes_request_without_reporting_duplicate_key | 검출 |
| 19 | 토큰 필드 교환 | tests::one_second_before_expiry_can_still_be_shared_by_the_waiting_generation, tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation | 검출 |
| 20 | 키 필드 교환 | tests::simultaneous_requests_share_one_issue_and_secret_debug_is_redacted | 검출 |
| 21 | 재발급 응답 폐기 | tests::invalid_token_also_reissues_once_as_required_by_t04_handoff, tests::revoked_retry_can_recover_and_other_statuses_do_not_reissue | 검출 |
| 22 | 중복 키 차단 누락 | tests::connection_coordinator_can_recover_duplicate_key_without_restarting_manager, tests::revoked_reissues_once_then_stops_all_requests | 검출 |
| 23 | 대기자 성공 유효 경계 변조 | tests::one_second_before_expiry_can_still_be_shared_by_the_waiting_generation, tests::queued_requests_share_success_even_when_refresh_becomes_due_during_issuance | 검출 |
| 24 | 취소 정리 조건 반전 | tests::queued_requests_share_success_even_when_refresh_becomes_due_during_issuance, tests::expiry_instant_and_after_expiry_share_failure_and_never_return_expired_token | 검출 |
| 25 | 정상 완료 표식 누락 | tests::one_second_before_expiry_can_still_be_shared_by_the_waiting_generation, tests::expiry_instant_and_after_expiry_share_failure_and_never_return_expired_token | 검출 |
| 26 | 오래된 응답 비교 반전 | tests::repeated_invalid_token_closes_request_without_reporting_duplicate_key, tests::connection_coordinator_can_recover_duplicate_key_without_restarting_manager | 검출 |
| 27 | 디코딩 토큰 폐기 | tests::revoked_reissues_once_then_stops_all_requests, tests::connection_coordinator_can_recover_duplicate_key_without_restarting_manager | 검출 |
| 28 | 주문 재조회 토큰 교환 | tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation | 검출 |
| 29 | 명시 복구 차단 조건 반전 | tests::revoked_reissues_once_then_stops_all_requests, tests::connection_coordinator_can_recover_duplicate_key_without_restarting_manager | 검출 |
| 30 | 오래된 오류를 중복 키로 오분류 | tests::obsolete_revoked_retry_does_not_disconnect_a_newer_successful_token | 검출 |
| 31 | invalid-token을 중복 키로 오분류 | tests::repeated_invalid_token_closes_request_without_reporting_duplicate_key | 검출 |
| 32 | invalid-token 인식 반전 | tests::invalid_token_also_reissues_once_as_required_by_t04_handoff, tests::repeated_invalid_token_closes_request_without_reporting_duplicate_key | 검출 |
| 33 | 반복 invalid-token 상태 오류 | tests::repeated_invalid_token_closes_request_without_reporting_duplicate_key | 검출 |
| 34 | 취소 공유 오류 누락 | tests::cancelled_renewal_disconnects_once_shares_failure_and_can_recover | 검출 |
| 35 | 취소 완료 세대 누락 | tests::cancelled_renewal_disconnects_once_shares_failure_and_can_recover | 검출 |

## 리뷰 2 수정

메인은 계획 갱신 토큰 재사용을 요청 자체 재발급으로 세지 않는다고 확정했다. acquire는 Shared/Issued를 반환하고, get은 요청 자체의 Issued 성공 뒤에만 예산을 소진한다. 공유한 토큰이 다시 거절되면 예산이 남아 발급할 수 있다. 다른 발급 세대가 계속 생기는 동안에는 그 결과를 공유하지만 같은 거부 토큰을 재사용하지 않으며, 요청 자체 발급을 반복하지 않는다. 최초 획득은 인증 실패 복구 예산과 별개다.

자체 발급 뒤 인증 실패에서 실제 다른 토큰이 있으면 Superseded로 요청만 닫는다. 현재 토큰이면 폐기와 닫힌 실패를 기록하고 상태를 전이한다. 다른 실패가 이미 캐시를 비웠으면 그 실패를 그대로 공유하며 새 토큰 교체로 오분류하지 않는다. expired-token401은 ADR0074의 이번 인계 범위 밖으로 기록만 하고 제품 정책을 추가하지 않는다.
