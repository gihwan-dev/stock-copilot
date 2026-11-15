# T-02 토큰 관리

- 상태: 초안

## 결정

키는 SecretStore의 TossClientId·TossClientSecret만 읽고 issue_token으로 발급한다. 토큰은 SecretValue를 소유하는 비공개 Token을 Arc로 공유하며 영속 저장하지 않는다. 응답 DTO는 원시 SecretValue를 빌려 읽어 토큰 문자열 소유 복사를 만들지 않는다. 전용 TokenLease의 Debug와 관리자의 Debug에는 비밀을 싣지 않는다. 실제 키·계좌·외부 API는 사용하지 않는다.

Clock이 제공하는 발급 요청 시작 시각에 expires_in을 더한다. 응답 수신 지연은 수명을 늘리지 않는다. 60초 전부터 갱신하며 수명 2~60초는 만료 1초 전이 아니라 수명에서 1초를 남기는 여유를 잡아 발급 1초 뒤 갱신한다. 수명 0·1초, 범위 초과, Bearer 이외, 빈 토큰·헤더에 부적합한 문자는 닫힌 InvalidResponse다. 문서가 갱신 여유 수치를 정하지 않아 보수적으로 정했다. 호출자가 token을 주기적으로 호출하는 예약 연결은 J-01에 인계한다.

관리자 한 개를 앱 조립에서 Arc로 공유한다. tokio sync 기능을 추가하고 한 비동기 잠금으로 발급·후속 복구를 직렬화한다. 동시 대기자는 진행 중인 발급 결과를 재사용한다. 실패 회차도 세대 번호를 비교해 이미 대기하던 요청에는 같은 실패를 전달한다. 이후 새 요청은 다시 시도할 수 있다. 잠금은 REST 응답을 기다리는 동안 잡지 않는다. 이전 lease로 거절된 요청은 이미 새 토큰이 있으면 추가 발급하지 않는다.

발급 시도가 시작되면 이전 토큰을 관리자로부터 제거한다. 발급이 성공했지만 응답이 소실됐을 수 있으므로 실패 시에도 이전 토큰을 다시 쓰지 않는다. 연결 상태는 Disconnected·Connected·DuplicateKey다. 발급 → close → reconnect(새 토큰) → resubscribe → reconcile_orders 순서가 끝나고 아직 만료되지 않아야 Connected다. 어느 단계든 실패하면 Disconnected다. 같은 상태는 이벤트를 다시 내지 않는다. 발행 보고서 최신 한 개를 보존해 호출자가 실패 구독자를 확인할 수 있게 한다.

401 token-revoked에는 요청당 재발급/재조회 각각 최대 1회다. 두 번째 revoked는 DuplicateKey와 RevokedRepeated로 닫고 이후 자동 발급도 중단한다. 사용자가 다른 프로그램을 종료하고 연결을 다시 구성하는 시점에 새 관리자를 조립한다. 자동 재발급 무한 반복을 막기 위한 보수적 선택이다.

TokenRenewal은 웹소켓 네 단계의 실행 계약이다. 구현체에 주문 전송 권한을 주지 않으며 마지막 단계는 읽기 주문 재조회다. 실제 웹소켓은 T-17이 구현한다. 새 HTTP 수단이나 주문 경로를 추가하지 않는다. 사용자 설정·화면 상태는 추가하지 않는다.

## 의존성 및 승인 항목

- tokio =1.54.1, default-features=false, 기존 rt/time에 sync만 추가, MIT. 새 패키지와 새 전이 의존성 없음.
- serde는 기존 workspace 버전을 일반 의존성에서도 사용한다. 테스트에 이미 있던 동일 버전·기능이며 새 패키지 없음.
- testkit에 별도 ScriptedHttpServer 도우미를 추가한다. 기존 LocalHttpServer는 수정하지 않는다. 기존 테스트와 단언은 변경하지 않는다.
- 프로젝트 설정 hooks=true와 현재 세션의 프로젝트/훅 신뢰 적용 여부를 분리한다. 세션 적용은 확인할 수 없다. frozen install과 lefthook install을 완료했다.

## 인계

- T-17: TokenRenewal을 구현해 close/reconnect/resubscribe/reconcile_orders를 연결한다. 네 단계 실패는 호출자에게 반환하며 실제 주문은 전송하지 않는다.
- T-03: TokenManager::get은 token/revoked 흐름만 제공한다. 그룹 속도 제한은 TossHttpClient 호출 전의 어댑터 연결부에서 결합한다. 공통 HTTP 구조는 변경하지 않았다.
- T-05·T-16: 조립한 단일 TokenManager를 공유한다. TokenLease는 비밀을 노출해야 하는 경계에서만 사용한다.
- J-01: Clock 기준 token 호출을 예약해 유휴 중에도 만료 전 갱신한다. TokenManager는 벽시계/자체 타이머를 만들지 않는다.

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
