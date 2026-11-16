# A-07 거래내역 동기화

- 상태: 초안
- 기준: 4a587ef3df592cf56a207739a7a59ad620e0056d
- 관련 작업: A-07, D-08, D-44, S-03, T-06, A-10

## 결정

명시적 결제일 창을 받는 `SyncTransactions::sync_window`를 application에 둔다. 날짜 창을 생성하는 진입점과 하루 한 번 전체 검사 실행은 아래 정의 대기가 해소된 뒤 연결한다. 이 변경만으로 A-07 전체 완료를 뜻하지 않는다. 사용자 화면이나 설정은 추가하지 않는다.

`Clock`에서 얻은 시각과 기존 `SettledRowSet::reconcile`만 사용한다. 완전 응답의 계좌·창·필터가 요청과 같은지 확인한 뒤 대조하며 멀티셋이나 창 계산을 다시 구현하지 않는다. 불완전 응답은 수신 페이지와 격리 자료를 그대로 보존하고 사라짐 대조를 호출하지 않는다. 포트 오류도 보존하며 이전 행 상태를 없애지 않는다. 브로커 호출 전의 저장 상태 계좌 불일치와 역행 시각은 수신 관측이 없는 사전조건 오류로 저장·이벤트 없이 거부한다. 응답 수신 뒤 범위 불일치·대조 오류는 이전 상태, 받은 응답과 거부 이유를 함께 저장한 뒤 실패 결과 이벤트를 한 번 낸다. 저장 실패는 이벤트 없이 호출자에게 반환한다.

`TransactionSyncRecord`에 시각, 요청한 결제일 창, 마지막 정상 행 상태, 원래 관측 결과, 대조 결과 또는 거부 이유를 함께 저장한다. 대조 결과에는 기존 도메인 경고·변경·취소 후보가 남는다. 포트 오류의 종류를 보존하므로 접근 차단과 일시적 실패를 성공으로 바꾸지 않는다. 기록 저장 성공 뒤 `TransactionsSyncRecorded(AccountId)`를 한 번 발행하며 `DeliveryReport`를 호출자에게 반환한다. 구독자 실패는 저장 결과를 되돌리지 않고 재발행하지 않는다. 이는 영속 이벤트 로그나 프로세스 종료를 넘는 정확히 한 번 전달의 보장이 아니다(ADR 0065).

## 저장 포트

기존 `TransactionRepository`는 `SyncRecord<SettledRowSet>`의 최신 결과만 조회한다. `Failed`·`AccessBlocked`에는 이전 행 상태가 없고, 수신한 불완전 페이지·격리 원문·대조 경고를 함께 보존할 수 없다. 기존 포트와 가짜·테스트를 변경하는 대신 `TransactionRepository`를 상위 트레이트로 요구하는 최소 확장 `TransactionSyncRepository`를 추가한다. 별도 저장소를 만드는 계약이 아니다. 같은 어댑터 객체가 두 조회를 구현한다. `record_sync`는 상세 기록과 `record.summary()`를 기존 저장 포트의 최신 결과에 한 트랜잭션으로 반영해야 한다. `summary()`는 정상 대조를 Complete, 불완전 수신을 Partial(직전 정상 상태), 접근 거부를 AccessBlocked, 그 밖의 포트 실패와 수신 후 거부를 Failed로 보존한다. A-16의 기존 latest 조회도 같은 저장 완료 결과를 읽는다.

- `state(account)`는 마지막 정상 행 상태를 반환하며 최초에는 해당 계좌의 빈 `SettledRowSet`을 반환한다. 직전 기록이 실패나 불완전이어도 정상 상태를 유지한다.
- `latest_sync(account)`는 마지막 상세 관측 기록을 반환하며 처음에는 None이다. 이벤트 구독자는 이 경로로 경고·격리·접근 차단·실패 원인·요청 창을 다시 읽는다.
- `record_sync(account, previous, record)`는 관측 기록 추가와 상태 갱신을 한 트랜잭션으로 처리한다. 현재 상태가 `previous`와 같은지 검사하고 다르면 저장 없이 기존 `SystemPortError`를 반환한다. 호출 성공은 전체 저장 완료를 뜻한다. 계좌별 동시 실행에서 오래된 상태의 덮어쓰기를 허용하지 않는다.
- 실패·불완전 관측에서도 `record.state`는 `previous`와 같다. 완전 응답에서만 새 대조 상태로 바꾼다. S-03 어댑터는 이 계약을 구현해야 한다. 테스트용 구현은 이 작업의 통합 테스트 파일 안에만 둔다.

## 정의 대기

`SettlementWindow::try_new(from, to)`는 이미 정한 날짜의 순서만 검증한다. 현재 공개 domain API에는 현재 시각에서 최근 14일 창을 반환하는 정책 함수가 없다. `Timestamp::checked_sub`·`date_at_offset`는 기본 연산이며 이를 조합해 13일/14일과 KST 오프셋을 유스케이스에서 정하면 지시문의 재구현 금지를 어긴다. 공개 API를 우회하거나 application에 초 단위 상수를 추가하지 않는다.

전체 검사의 시작 결제일과 완전 응답 상한을 넘을 때의 구간 분할도 domain 계약이 필요하다(ADR 0034). 일별 전체 검사 완료일은 실패·불완전 시 갱신하지 않는 방향으로 별도 저장 계약을 정해야 한다. 이 정의가 없는 상태에서 하루 1회 검사를 했다고 보고하지 않는다.

D-44는 수급 타입 작업이다. 지시문이 가리킨 KST 기준은 ADR 0090과 수급 날짜 검증 안에는 있으나 거래내역 창 생성 계약은 아니다. 날짜 두 개를 호출자가 임의로 넘기는 API로 최종 14일 정책을 대신했다고 주장하지 않는다.

## 인계

| 받는 작업 | 필요한 일 |
|---|---|
| D-08, D-44 | 메인이 날짜 정책의 담당을 확정한다. domain에 끝값 포함 여부를 정한 최근 14일 KST 창 함수와 전체 검사 창·분할 계약을 정의한다. 현재 D-44 완료 기준을 임의로 바꾸지 않는다. |
| A-07 | 위 계약 뒤 자동 창 생성, 계좌별 일일 전체 검사 완료 기록과 재시도, 하루 한 번·KST 자정 경계 테스트를 연결한다. |
| S-03 | 원본 재정규화 보존과 확장 포트의 상세 기록·기존 요약 투영·직전 상태 비교를 원자적으로 저장하고 실패 이후 정상 상태와 상세 결과 조회를 구현한다. |
| T-06 | 페이지·격리·상한 초과에서 완전 응답을 발명하지 않는다. 완전한 결제일 구간의 응답을 제공한다. |
| A-10, U-11 | 저장 결과 재조회로 실패·부분 수신·접근 차단과 대조 경고·사라진 행을 표시한다. |
| H-47 | 변경분 변이가 application만 시험해 testkit-ports의 유스케이스 통합 테스트를 빠뜨리는 선택 문제를 보완한다. 기존 변이 소속 패키지의 테스트도 유지해야 한다. |

## 검증

최초 새 통합 테스트 8개가 통과했고, 1차 리뷰 수정에서 경고 재조회·시계 역행·서로 다른 실패 창·엇갈린 두 동기화의 CAS 거부를 추가했다. 수정분은 별도 검사와 재리뷰 대상으로 남긴다. 구현 전 공개 API 부재로 컴파일 실패를 먼저 확인했다. 기존 테스트와 의존성·검사 규칙은 변경하지 않았다. 승인된 test-diff 변경은 0건이다. 세션의 프로젝트 신뢰와 훅 정의 신뢰 적용은 확인할 수 없으며 정상 Git 커밋 훅 결과를 별도로 남긴다.

저장소 명령 `just mutants-diff 4a587ef3df592cf56a207739a7a59ad620e0056d`는 종료 1이며 선택한 15개 모두 미검출이다. 실행기가 `--no-config`를 지정하고 변이 소속 패키지의 테스트만 실행해 testkit-ports의 통합 테스트가 빠진다. 검사 소스나 대상 목록을 바꾸지 않고 H-47에 인계한다. 임시 사본에서 같은 파일에 `--test-package application --test-package testkit-ports`를 지정한 보완 실행은 종료 0, 15개 중 14개 검출·컴파일 불가 1개·생존 0개·시간 초과 0개다. 원래 명령의 실패와 보완 결과를 구별한다.

구현 커밋 57a278a3391a4b99c5a114dd9faf37363e7461e5의 정상 변경분 훅은 296개 테스트를 통과했다. 문서 커밋 35f7484fe070e8efc1bc057b397ce24f20e702fb의 정상 훅은 전체 검사로 승격되어 1594개 테스트와 커버리지용 1059개 테스트·coverage-check·test-diff·구조 검사를 통과했다. 이후 커밋의 검사 결과를 소급해 주장하지 않는다.

날짜 창 생성·일별 전체 검사 테스트는 정의 대기로 NOT_RUN이다. 새 코드 외의 기존 구현에 대한 결과를 이 작업의 완료 증거로 사용하지 않는다.

## 손 변형

`mktemp -d` 사본의 원본 대조군 8개가 통과한 뒤 변형마다 원본에서 한 부분만 바꿔 같은 테스트를 실제 실행했다. 22개 시도 중 컴파일 성공 20개 모두 검출, 컴파일 실패 2개는 제외했다. 로그와 실행 스크립트는 작업 사본의 `target/A-07/`에 있다. 아래 검출은 해당 통합 테스트 실행의 실제 실패다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | stored account equality | 컴파일 성공·검출 | duplicate_disappearance_preserves_both_boundaries_and_outside_rows, broker_failure_and_access_denial_are_recorded_without_erasing_state, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, rejects_wrong_stored_account_and_clock_reversal, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, broker_failure_and_access_denial_are_recorded_without_erasing_state, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, rejects_wrong_stored_account_and_clock_reversal, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance |
| 2 | reject equal time | 컴파일 성공·검출 | new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
| 3 | reverse clock ordering | 컴파일 성공·검출 | rejects_wrong_response_account_window_or_filter_without_save_or_event, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance, broker_failure_and_access_denial_are_recorded_without_erasing_state, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, incomplete_responses_must_match_all_requested_scope_fields, rejects_wrong_response_account_window_or_filter_without_save_or_event, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance, broker_failure_and_access_denial_are_recorded_without_erasing_state, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, incomplete_responses_must_match_all_requested_scope_fields |
| 4 | complete account equality | 컴파일 성공·검출 | rejects_wrong_response_account_window_or_filter_without_save_or_event, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing, rejects_wrong_response_account_window_or_filter_without_save_or_event, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing |
| 5 | complete window equality | 컴파일 성공·검출 | rejects_wrong_response_account_window_or_filter_without_save_or_event, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, rejects_wrong_response_account_window_or_filter_without_save_or_event, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
| 6 | complete filter equality | 컴파일 성공·검출 | rejects_wrong_response_account_window_or_filter_without_save_or_event, failed_save_emits_nothing, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, rejects_wrong_response_account_window_or_filter_without_save_or_event, failed_save_emits_nothing, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
| 7 | complete scope conjunction | 컴파일 성공·검출 | rejects_wrong_response_account_window_or_filter_without_save_or_event, rejects_wrong_response_account_window_or_filter_without_save_or_event |
| 8 | incomplete account equality | 컴파일 성공·검출 | incomplete_cursor_preserves_received_rows_without_disappearance, incomplete_responses_must_match_all_requested_scope_fields, incomplete_cursor_preserves_received_rows_without_disappearance, incomplete_responses_must_match_all_requested_scope_fields |
| 9 | incomplete window equality | 컴파일 성공·검출 | incomplete_cursor_preserves_received_rows_without_disappearance, incomplete_responses_must_match_all_requested_scope_fields, incomplete_cursor_preserves_received_rows_without_disappearance, incomplete_responses_must_match_all_requested_scope_fields |
| 10 | incomplete filter equality | 컴파일 성공·검출 | incomplete_responses_must_match_all_requested_scope_fields, incomplete_cursor_preserves_received_rows_without_disappearance, incomplete_responses_must_match_all_requested_scope_fields, incomplete_cursor_preserves_received_rows_without_disappearance |
| 11 | incomplete scope conjunction | 컴파일 성공·검출 | incomplete_responses_must_match_all_requested_scope_fields, incomplete_responses_must_match_all_requested_scope_fields |
| 12 | drop reconciled state | 컴파일 성공·검출 | duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
| 13 | persist wrong observation | 컴파일 실패·집계 제외 | 해당 없음 |
| 14 | persist previous timestamp | 컴파일 성공·검출 | duplicate_disappearance_preserves_both_boundaries_and_outside_rows, duplicate_disappearance_preserves_both_boundaries_and_outside_rows |
| 15 | wrong event account | 컴파일 성공·검출 | new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, broker_failure_and_access_denial_are_recorded_without_erasing_state, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, broker_failure_and_access_denial_are_recorded_without_erasing_state, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows |
| 16 | wrong event kind | 컴파일 성공·검출 | broker_failure_and_access_denial_are_recorded_without_erasing_state, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, incomplete_cursor_preserves_received_rows_without_disappearance, broker_failure_and_access_denial_are_recorded_without_erasing_state, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, incomplete_cursor_preserves_received_rows_without_disappearance |
| 17 | skip saving | 컴파일 성공·검출 | new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance, broker_failure_and_access_denial_are_recorded_without_erasing_state, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, failed_save_emits_nothing, incomplete_cursor_preserves_received_rows_without_disappearance, broker_failure_and_access_denial_are_recorded_without_erasing_state |
| 18 | skip event | 컴파일 실패·집계 제외 | 해당 없음 |
| 19 | wrong expected state | 컴파일 성공·검출 | new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, failed_save_emits_nothing, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, failed_save_emits_nothing, duplicate_disappearance_preserves_both_boundaries_and_outside_rows |
| 20 | read clock twice | 컴파일 성공·검출 | new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, broker_failure_and_access_denial_are_recorded_without_erasing_state, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, broker_failure_and_access_denial_are_recorded_without_erasing_state, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows |
| 21 | drop reconciliation evidence | 컴파일 성공·검출 | duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
| 22 | wrong response filter request | 컴파일 성공·검출 | incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, broker_failure_and_access_denial_are_recorded_without_erasing_state, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, incomplete_cursor_preserves_received_rows_without_disappearance, duplicate_disappearance_preserves_both_boundaries_and_outside_rows, broker_failure_and_access_denial_are_recorded_without_erasing_state, new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields |
