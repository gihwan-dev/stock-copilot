# A-22 자동 거래내역 창과 전체 검사 시각

- 상태: 초안

## 결정

기준 16b4c54080d976006659bd1d6954c9b0a7f11dda. 기존 SyncTransactions와 테스트는 유지하고 TransactionSchedule을 얇은 진입점으로 추가한다. sync_recent은 Clock을 한 번 읽어 domain의 recent_transaction_window에 전달하고 같은 표본 시계를 A-07에 주입한다. 날짜 계산과 일일 판정은 D-45 함수만 한다. 기존 저장 후 이벤트 발행은 A-07에 맡긴다.

FullInspectionRepository는 계좌별 last_completed와 record_completed(account, previous, completed_at)만 제공한다. TransactionSyncRepository와 독립적이다. 실제 어댑터는 previous와 저장값이 일치하는 경우에만 원자적으로 갱신하고 실패 시 기존 값을 보존해야 한다. 이 CAS 계약은 오래 걸린 호출이 새 완료 기록을 덮지 않게 한다. S-03의 구현이나 A-23의 스케줄/실행 잠금을 만들지 않는다.

full_inspection_status는 완료 시각을 읽고 daily_full_inspection으로 오늘의 필요 여부를 반환한다. record_full_inspection_completed는 완료 시점의 Clock과 저장된 시각을 같은 domain 함수로 검사한 뒤 저장한다. 동일 시각은 허용하며 미래 시각은 ClockReversed로 닫힌 실패다. 저장 오류는 원형을 보존한다. 이 메서드는 신뢰된 코어 호출자가 전체 구간 성공 후에만 호출하는 저장 진입점이며 전체 기간 성공을 증명하는 토큰이 아니다. 현재 실제 전체 실행 호출자는 없다. 최근 창 성공·부분 수신·실패는 이 메서드를 호출하지 않는다. 전체 완료 사실 없이 이 메서드를 연결해서는 안 된다.

## 작업 기준

| 항목 | 상태 | 근거 |
|---|---|---|
| 자동 14일 창 | 충족 | sync_recent, KST 자정 전후 독립 기대 창 |
| 오늘 전체 검사 필요 여부 | 충족 | full_inspection_status, 완료일/미완료 재시도 |
| 완료 시각 기록 및 역행 거부 | 충족 | 별도 저장 포트, domain 일일 판정 후 CAS 저장 |
| 전체 기간 실행 하루 한 번 | 정의 대기 | 전체 시작일·분할·완결 증명 domain 계약 없음 |
| 전체 수신 실패 후 실제 재실행 | 정의 대기 | 실행 계약 없음. 오류 전파와 다음 조회가 미완료일 때 Required 판정까지 검사. 실제 저장 상태 보존은 S-03 검증 대상 |

## 인계

| 받는 작업 | 내용 |
|---|---|
| D-45 후속(메인 지정) | 전체 시작일, 응답 상한 분할, 모든 구간 성공의 계약 |
| S-03 | 계좌별 완료 시각 CAS 원자 저장·실패 보존, 미래 자료 복구는 별도 결정. 복구·정정이 없으면 표현 가능한 미래 자료는 시계가 저장 시각에 도달할 때까지 전체 판정이 오류로 남고, KST 표현 범위 밖 자료는 시간 경과로 회복된다고 볼 수 없음. 최근 14일 동기화는 독립적으로 실행 가능 |
| A-23 | 계좌별 직렬화 범위에 시계 표본 취득과 state/latest 조회를 함께 포함. 실행 직렬화·일일 검사 호출 연결. 필요 판정만으로 동시 실행 배제를 주장하지 않음 |
| A-22 후속 | 전체 계약 확정 뒤 모든 구간 완전 수신·저장 성공에만 완료 기록 연결 |

## 검사

새 외부 의존성·기존 테스트 변경·기존 가짜 수정 없음. 프로젝트 신뢰와 세션 훅 신뢰는 확인 불가이며 브리프 승인으로 진행한다. Git 훅 설치 성공. 환경 예외 사용: 예. 정상 훅 재실행은 Xcode 라이선스 R4 한 건만 실패했고 나머지 check-ts와 check 본문은 순차 직접 실행하여 통과했다. 해당 커밋에만 브리프의 LEFTHOOK=0 승인을 적용하고 커밋 메시지에 기록했다. 전체 nextest 1813개, 계측 테스트 1070개와 coverage-check·test-diff·hooks-installed가 통과했다. 전체 just check가 무조건 종료 0이었다고 주장하지 않는다.


## 리뷰 반영

저장소 실패 후 다음 응답을 직접 넣는 대본 테스트는 실제 저장 상태 보존을 증명하지 않는다. 테스트 이름과 위 작업 기준의 근거를 그 범위로 정정했다. 실제 CAS 충돌·원자성·실패 보존은 S-03이 어댑터에서 검증한다. 구독자 실패를 정확히 반환하는 회귀와 미래 완료 시각에 도달하면 CompletedToday로 회복하는 회귀를 추가한다. 마지막 완료 시각의 KST 날짜를 기준으로 삼는 것은 ADR 0105의 확정 계약이며 시작 날짜로 바꾸지 않는다. 자정을 넘는 실제 전체 수신에서 어느 결제일까지 완결해야 하는지는 전체 구간 계약의 인계다.

## 변이 범위의 한계

저장소 명령 just mutants-diff는 4개 선택 중 미검출 1개·컴파일 불가 3개로 종료 1이었다. sync_recent을 빈 성공으로 바꾸는 변형은 application 테스트만 실행하여 testkit-ports 통합 테스트가 제외됐다. 원 명령의 실패를 통과로 바꾸어 보고하지 않는다. 동일 변형을 통합 테스트까지 포함하여 보완 실행하며 실제 결과를 별도로 기록한다. 하네스 선택 문제는 A-07에서도 알려진 H-47 인계다. 가짜를 application에 옮기거나 검사 규칙을 바꾸지 않는다.

## 손 변형

구현 소스와 통합 테스트를 target 아래 mktemp 복사본에 넣어 각각 컴파일 후 실행했다. 최초 20개 모두 컴파일 성공·검출이고 복구 대조군 6개도 통과했다. 다음 표는 실제 실행 로그의 집계이며 컴파일 불가를 검출로 세지 않는다. 리뷰 후 추가 회귀에 대한 보고서 유실 변형은 별도로 기록한다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | 최근 창에 표현 범위 밖 시각 전달 | 컴파일 성공·검출 | recent_range_and_sync_errors_are_preserved, recent_window_midnight_uses_one_clock_sample_and_never_marks_full_completion |
| 2 | 최근 창에 하루 뒤 고정 시각 전달 | 컴파일 성공·검출 | recent_window_midnight_uses_one_clock_sample_and_never_marks_full_completion, recent_range_and_sync_errors_are_preserved |
| 3 | A07 기록 시각을 고정 시각으로 대체 | 컴파일 성공·검출 | recent_window_midnight_uses_one_clock_sample_and_never_marks_full_completion |
| 4 | 일일 판정의 이전 완료 무시 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write, daily_status_midnight_retry_and_accounts, future_completion_and_out_of_range_fail_before_writing |
| 5 | 일일 판정 현재 시각을 범위 밖으로 | 컴파일 성공·검출 | daily_status_midnight_retry_and_accounts, future_completion_and_out_of_range_fail_before_writing, completion_uses_current_clock_and_preserves_previous_for_atomic_write, repository_errors_propagate_and_failed_completion_remains_due |
| 6 | 일일 판정 시각을 과거 고정값으로 | 컴파일 성공·검출 | future_completion_and_out_of_range_fail_before_writing, daily_status_midnight_retry_and_accounts, completion_uses_current_clock_and_preserves_previous_for_atomic_write |
| 7 | 완료 저장 이전 값 제거 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write |
| 8 | 완료 저장 현재 값을 과거로 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write |
| 9 | 완료 역행 검사 이전 값 제거 | 컴파일 성공·검출 | future_completion_and_out_of_range_fail_before_writing |
| 10 | 완료 역행 검사를 자기 시각으로 | 컴파일 성공·검출 | future_completion_and_out_of_range_fail_before_writing |
| 11 | 완료 검사 시각 범위 밖 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write, future_completion_and_out_of_range_fail_before_writing, repository_errors_propagate_and_failed_completion_remains_due |
| 12 | 완료 저장 이전 값을 현재로 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write |
| 13 | 완료 저장 계좌 변경 | 컴파일 성공·검출 | completion_uses_current_clock_and_preserves_previous_for_atomic_write |
| 14 | 완료 조회 계좌 변경 | 컴파일 성공·검출 | daily_status_midnight_retry_and_accounts |
| 15 | 최근 창 요청 계좌 변경 | 컴파일 성공·검출 | recent_window_midnight_uses_one_clock_sample_and_never_marks_full_completion |
| 16 | 최근 창 오류를 역행으로 | 컴파일 성공·검출 | recent_range_and_sync_errors_are_preserved |
| 17 | 일일 상태 출력 반전 | 컴파일 성공·검출 | repository_errors_propagate_and_failed_completion_remains_due, completion_uses_current_clock_and_preserves_previous_for_atomic_write, daily_status_midnight_retry_and_accounts |
| 18 | 최근 창 실패 무시 | 컴파일 성공·검출 | recent_range_and_sync_errors_are_preserved |
| 19 | 완료 판정 오류 무시 | 컴파일 성공·검출 | future_completion_and_out_of_range_fail_before_writing |
| 20 | 완료 저장 결과 성공 합성 | 컴파일 성공·검출 | repository_errors_propagate_and_failed_completion_remains_due |

최초 손 변형 표의 실패 전파 테스트 이름은 실행 당시 이름이다. 리뷰 후 repository_errors_propagate_and_missing_completion_is_required로 이름을 정정했고 단언은 유지했다.


## 최종 보완 변이 결과

원 저장소 명령은 종료1(미검출1·컴파일불가3)로 유지한다. 같은 제품 소스의 임시 복사본에서 cargo mutants에 test-package application과 testkit-ports를 함께 지정한 보완 실행은 4개 중 검출1·컴파일불가3·생존0·시간초과0, 종료0이었다. 이 결과는 원 명령의 통과를 뜻하지 않는다. 원 명령의 생존은 빈 성공을 반환하는 sync_recent이며 기존 통합 테스트는 이를 검출한다. 테스트 선택 문제는 H-47 인계다.

1차 비판의 보고서 유실 변형은 새 회귀 반영 후 실제 컴파일 성공·통합7통과1실패로 검출했다. 제품 소스 원복 후 명시적으로 재빌드한 대조군은8개 통과했다. 첫 원복 대조는 copy2가 보존한 파일 시각 때문에 이전 변형 실행 파일이 남아 실패했고, 제품 소스 바이트 동등 확인·파일 시각 갱신·재빌드한 결과와 구별한다. 로그는 target/A-22/report-mutant.log·report-control2.log다.

2차 비판은 Reconciliation 오류만 빈 성공으로 삼키는 변형이 기존8개를 통과함을 실제 확인했다. 표본보다1초 미래인 정상 상태 회귀를 넣으면 실패하고, 원본은9개 통과했다. 이 회귀와 Complete/Incomplete 수신 경로 회귀를 반영한다. 빈 행의 관측·Complete/Partial 요약·표본 시각·창·전체 완료 미기록을 확인하며 상세 행 대조는 기존 A-07 테스트의 범위다. 제품 소스는 바꾸지 않는다.

| 추가 변형 | 컴파일 | 변형 실행 | 원복 대조 |
|---|---|---|---|
| DeliveryReport를 빈 값으로 대체 | 성공 | 새 보고서 회귀 실패(7통과1실패) | 8통과 |
| Reconciliation 오류만 빈 성공으로 대체 | 성공 | 미래 상태 회귀 실패 | 최종 회귀 포함 대조로 확인 |

수정 커밋 6e5e2274의 정상 훅도 Xcode R4 단 한 건만 실패했다. 남은 전 검사를 직접 순차 실행해 전체 nextest1815개·계측1070개·coverage-check·test-diff·hooks-installed까지 통과했고 해당 커밋만 환경 예외를 사용했다. 이후 최신 master bf9618518c8782cd5e00a5735679c34762ade84a 수신과 두 번째 비판의 사소 회귀 보강을 포함한 최종 검사 결과는 리뷰 기록과 최종 보고에 남긴다.
