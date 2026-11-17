# ADR 0120: A-25 전체 검사를 구간 계획으로 실행

- 상태: 확정 (2026-11-17)


## 결정

RunFullInspection은 A-22 FullInspectionRepository와 D-46 FullInspectionPlan을 연결하는 application 진입점이다. A-22와 A-07의 공개 계약은 유지한다. A-07의 내부 sync_window_record가 저장을 마친 동일 관측과 이벤트 전달 보고서를 반환하고, 기존 sync_window는 전달 보고서만 반환한다. 저장소 최신값을 다시 읽어서 다른 실행의 기록을 성공 증거로 잘못 쓰지 않는다.

개설일 포트가 없으므로 None을 넘긴다. domain이 0001-01-01부터 계획한다. 구간 상한은 InspectionRangeDays 인자로 받으며 실서비스 기본값이나 요청 간격을 여기서 정하지 않는다. 날짜 계산은 domain에만 있다. 실행은 순차이며 각 완전 수신·대조·저장 성공만 Succeeded다. 저장된 Failed/Partial/AccessBlocked는 Failed로 보고한다. 접근 차단을 제외한 관측 실패에서는 남은 구간을 처리한다. 저장 포트 오류는 즉시 반환한다. 전체 기록을 남기지 않은 실행은 다음 호출에서 다시 계획 전체를 실행한다.

완료 시계가 시작보다 과거면 ClockReversed다. completion_at의 결과가 있어야 CAS 저장하고 completed_at을 그대로 쓴다. 완료값 생성 뒤 시계를 다시 읽지 않는다. DayMismatch는 저장 없이 반환하며 다음 호출은 새 오늘로 다시 계획한다. FullInspectionReport는 성공·오류 결과와 구간별 DeliveryReport를 함께 반환한다. 결과가 미완료·자정 변경·저장 오류·후속 구간 오류여도 이미 발행한 보고서는 버리지 않는다. 접근 차단은 해당 구간의 관측을 저장·실패 보고한 직후 나머지 구간을 중단하고 완료를 기록하지 않는다. 일반 Failed·Partial은 원래처럼 나머지 구간을 처리한다. 두 번째 일일 판정은 앞선 시간 검증과 completion_at이 보장하므로 중복 계산을 제거한다.

중복·외부 구간 보고는 FullInspectionProgress의 영구 Invalid 계약으로 닫힌다. 실행기는 불변 계획을 한 번씩 순회하고 요청한 창을 보존하는 A-07 관측만 보고하므로 정상 실행에서 이 두 오류를 주입할 공개 입력이 없다. 새 테스트는 domain의 보고 거부와 완료값 생성 실패를 직접 확인한다. 이 테스트에 실행과 무관한 저장 가짜는 두지 않는다.

## 작업 기준

| 항목 | 상태 | 근거 |
|---|---|---|
| D-46 계획과 구간 동기화 | 충족 | 1·2·2028 구간 호출/저장 검증 |
| 모든 구간 성공 후 정확한 표본 저장 | 충족 | 서로 다른 시작·조회·완료 표본 테스트 |
| 한 구간 실패와 전체 재시도 | 충족 | 첫·마지막 구간 실패 뒤 2개 구간 재시도 |
| 날짜 변경 완료 거부 | 충족 | KST 자정 DayMismatch |
| 중복·범위 밖 보고 | 충족 | 공개 domain 계약 Invalid, 실행기는 구조상 한 번씩 보고 |
| 계좌 개설일 | 정의 대기 | 획득 포트 없음; domain 모름 정책 적용 |
| 실제 요청 상한·비용·간격 | 정의 대기 | T-06 후속 실측·설계 필요 |

## 인계

| 받는 작업 | 내용 |
|---|---|
| T-06 후속 | 모름 정책의 긴 기간 전체 순회 비용, 실제 구간 상한·요청 간격, 전체 수신 자료 재사용 가능성 실측 |
| S-03 | 거래 관측 원자 저장, 전체 완료 시각 CAS·실패 보존 실제 어댑터 검증 |
| U-08 / A-22 후속 | 계좌별 실행 직렬화와 예약 호출 연결. 일일 필요 판정과 CAS는 동시 중복 조회 배제 잠금이 아님 |
| 메인 지정 후속 | 계좌 개설일 획득 포트와 계좌 연결 입력 출처 확정 |
| H-47 | application 변경분 변이에서 testkit-ports 통합 테스트 선택 확인 |

## 검증

실제 검사·리뷰 및 손 변형 결과는 아래와 docs/reviews/A-25.md에 기록한다. 새 포트·외부 의존성·기존 테스트 변경·검사 끄기 없음. 프로젝트 신뢰와 세션 훅 신뢰는 미확인이고 Git 훅 설치는 성공했다. 실제 키·계좌·API 접근은 없다.

## 리뷰 후 평가와 추가 인계

2차 비판의 중요 두 건은 접근 차단 즉시 중단과 성공·실패 양쪽의 전달 보고 보존으로 수정했다. 3차 비판은 rate_limiter 기존 회귀를 직접 실행하여 재시도 소진 뒤 다음 호출에도 Retry-After 대기가 유지됨을 확인했다. 따라서 일반 실패 뒤의 순회가 실제 HTTP 요청을 즉시 보내는 API-2 위반이라는 해석은 기각한다. 가짜2028회 순회 사실과 비용 위험은 남으므로 T-06 후속이 일반 실패 중단 정책과 비용을 함께 평가한다.

U-08/U-11/A-10은 FullInspectionReport.outcome이 전체 검사 결과이고 latest_sync는 마지막 구간의 관측이라는 점을 구별해야 한다. 접근 차단의 원인은 상세 저장 기록에서 조회하며 전체 실행의 Incomplete만 보고 자동 재시도 여부를 정하지 않는다. 기존 TransactionSchedule의 완료만 기록하는 API를 전체 완료 사실 없이 연결하지 않는다.

3차 합성 표본 시작t·구간t+59·완료t+1은 같은 KST 날짜에서 성공한다. 현재 계약은 시작보다 이른 완료를 거부하고 실제 완료 Clock 표본을 그대로 보존한다. 마지막 구간 기록 시각까지 하한으로 삼는 정책은 U-08/S-03의 시계 역행·직렬화 설계로 인계한다. 일일 완료 날짜가 틀린 반례는 확인되지 않았다. 긴 전체 실행과30분 최근 창 동기화의 계좌별 직렬화·CAS 충돌 역시 U-08/S-03이 연결 단계에서 검증한다.

최종 제품 소스의 자동 변경분 변이는8개 중 미검출4·컴파일불가4·종료1이며 통합 테스트가 선택되지 않았다. 같은 소스에서 application/testkit-ports를 모두 지정한 보완 자동 변이는 검출4·컴파일불가4·생존0·시간초과0·종료0이다. 원 명령의 실패를 통과로 표현하지 않는다. H-47에 선택 문제를 인계한다.

## 최종 소스 손 변형

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | 완전응답 실패 | 컴파일 성공·검출 | repository_errors_preserve_failure_and_do_not_mark_completion, all_ranges_save_exact_completion_and_skip_today, reports_survive_incomplete_midnight_and_completion_save_failure, reports_survive_later_sync_failure_and_success, day_mismatch_retry_covers_the_new_today, failed_range_never_completes_and_next_run_retries_every_range, midnight_rejects_completion_and_exact_sample_is_not_resampled, blocked_account_stops_after_one_range_and_retries_next_run |
| 2 | 불완전응답 성공 | 컴파일 성공·검출 | matching_partial_and_access_denied_never_mark_completion |
| 3 | 접근차단 후 순회 지속 | 컴파일 성공·검출 | blocked_account_stops_after_one_range_and_retries_next_run |
| 4 | 전체 보고 삭제 | 컴파일 성공·검출 | blocked_account_stops_after_one_range_and_retries_next_run, reports_survive_later_sync_failure_and_success, reports_survive_incomplete_midnight_and_completion_save_failure |
| 5 | 오류 보고만 삭제 | 컴파일 성공·검출 | reports_survive_incomplete_midnight_and_completion_save_failure, blocked_account_stops_after_one_range_and_retries_next_run, reports_survive_later_sync_failure_and_success |
| 6 | 성공 보고만 삭제 | 컴파일 성공·검출 | reports_survive_later_sync_failure_and_success |
| 7 | 전달 보고 중복 | 컴파일 성공·검출 | reports_survive_later_sync_failure_and_success, reports_survive_incomplete_midnight_and_completion_save_failure, blocked_account_stops_after_one_range_and_retries_next_run |
| 8 | 전달 보고 삭제 | 컴파일 불가·집계 제외 | 해당 없음 |
| 9 | 오늘 완료 오류 | 컴파일 성공·검출 | all_ranges_save_exact_completion_and_skip_today |
| 10 | 첫 구간 누락 | 컴파일 성공·검출 | reports_survive_incomplete_midnight_and_completion_save_failure, repository_errors_preserve_failure_and_do_not_mark_completion, reports_survive_later_sync_failure_and_success, midnight_rejects_completion_and_exact_sample_is_not_resampled, day_mismatch_retry_covers_the_new_today, reversed_clock_and_future_completion_fail_closed, failed_range_never_completes_and_next_run_retries_every_range, matching_partial_and_access_denied_never_mark_completion, all_ranges_save_exact_completion_and_skip_today, blocked_account_stops_after_one_range_and_retries_next_run |
| 11 | 마지막 구간 누락 | 컴파일 성공·검출 | failed_range_never_completes_and_next_run_retries_every_range, reports_survive_incomplete_midnight_and_completion_save_failure, matching_partial_and_access_denied_never_mark_completion, reversed_clock_and_future_completion_fail_closed, all_ranges_save_exact_completion_and_skip_today, repository_errors_preserve_failure_and_do_not_mark_completion, day_mismatch_retry_covers_the_new_today, reports_survive_later_sync_failure_and_success, blocked_account_stops_after_one_range_and_retries_next_run, midnight_rejects_completion_and_exact_sample_is_not_resampled |
| 12 | 첫 구간만 실행 | 컴파일 성공·검출 | failed_range_never_completes_and_next_run_retries_every_range, all_ranges_save_exact_completion_and_skip_today, blocked_account_stops_after_one_range_and_retries_next_run, reports_survive_later_sync_failure_and_success |
| 13 | 성공 보고를 실패로 | 컴파일 성공·검출 | repository_errors_preserve_failure_and_do_not_mark_completion, reports_survive_later_sync_failure_and_success, failed_range_never_completes_and_next_run_retries_every_range, day_mismatch_retry_covers_the_new_today, all_ranges_save_exact_completion_and_skip_today, midnight_rejects_completion_and_exact_sample_is_not_resampled, reports_survive_incomplete_midnight_and_completion_save_failure, blocked_account_stops_after_one_range_and_retries_next_run |
| 14 | 중복 보고 | 컴파일 성공·검출 | reports_survive_incomplete_midnight_and_completion_save_failure, matching_partial_and_access_denied_never_mark_completion, reports_survive_later_sync_failure_and_success, incomplete_wrong_scope_and_store_failure_never_complete, all_ranges_save_exact_completion_and_skip_today, reversed_clock_and_future_completion_fail_closed, midnight_rejects_completion_and_exact_sample_is_not_resampled, failed_range_never_completes_and_next_run_retries_every_range, day_mismatch_retry_covers_the_new_today, repository_errors_preserve_failure_and_do_not_mark_completion, blocked_account_stops_after_one_range_and_retries_next_run |
| 15 | 진행 초기화 | 컴파일 불가·집계 제외 | 해당 없음 |
| 16 | 완료 표본 시작값 | 컴파일 성공·검출 | day_mismatch_retry_covers_the_new_today, reversed_clock_and_future_completion_fail_closed, midnight_rejects_completion_and_exact_sample_is_not_resampled, reports_survive_incomplete_midnight_and_completion_save_failure |
| 17 | 저장 표본 재조회 | 컴파일 성공·검출 | midnight_rejects_completion_and_exact_sample_is_not_resampled, reports_survive_incomplete_midnight_and_completion_save_failure |
| 18 | CAS 이전값 삭제 | 컴파일 성공·검출 | all_ranges_save_exact_completion_and_skip_today |
| 19 | CAS 이전값 현재로 | 컴파일 성공·검출 | midnight_rejects_completion_and_exact_sample_is_not_resampled, day_mismatch_retry_covers_the_new_today, all_ranges_save_exact_completion_and_skip_today |
| 20 | 저장 계좌 변경 | 컴파일 성공·검출 | all_ranges_save_exact_completion_and_skip_today, day_mismatch_retry_covers_the_new_today, midnight_rejects_completion_and_exact_sample_is_not_resampled |
| 21 | 저장 시각 시작값 | 컴파일 성공·검출 | midnight_rejects_completion_and_exact_sample_is_not_resampled |
| 22 | 반환 시각 시작값 | 컴파일 성공·검출 | midnight_rejects_completion_and_exact_sample_is_not_resampled |
| 23 | 역행 비교 반전 | 컴파일 성공·검출 | midnight_rejects_completion_and_exact_sample_is_not_resampled, day_mismatch_retry_covers_the_new_today, reversed_clock_and_future_completion_fail_closed, reports_survive_incomplete_midnight_and_completion_save_failure |
| 24 | 동일 시각 거부 | 컴파일 성공·검출 | repository_errors_preserve_failure_and_do_not_mark_completion, reports_survive_incomplete_midnight_and_completion_save_failure, reports_survive_later_sync_failure_and_success, all_ranges_save_exact_completion_and_skip_today, matching_partial_and_access_denied_never_mark_completion, day_mismatch_retry_covers_the_new_today, failed_range_never_completes_and_next_run_retries_every_range, incomplete_wrong_scope_and_store_failure_never_complete, blocked_account_stops_after_one_range_and_retries_next_run |
| 25 | 완료 조회 계좌 변경 | 컴파일 성공·검출 | all_ranges_save_exact_completion_and_skip_today |
| 26 | 구간 조회 계좌 변경 | 컴파일 불가·집계 제외 | 해당 없음 |

최종 제품 소스에서26개를 실제 시도했고 컴파일 성공23개 모두 검출했다. 컴파일 불가3개는 세지 않는다. 원복 대조군12개 통과. 앞선 소스의22개 및 리뷰 보강2개와 합산하지 않는다.
