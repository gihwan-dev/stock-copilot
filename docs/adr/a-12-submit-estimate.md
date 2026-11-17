# A-12: D-47을 사용하는 추정 제출

- 상태: 초안
- 기준: 994c25a191e0ab08e2c8e9f1f26cfa2de7be3f4d

## 결정

ADR 0115의 EstimateAssignments<AiRunId>와 EstimateSubmissionRecord<AiRunId, ReasoningEffort>를 그대로 사용한다. application에서 발급·검증·계산 규칙을 재정의하지 않는다. 배치 전체를 domain consume으로 검증하고 순수 다음 상태를 만든다. 원장의 lookup 결과와 기존 SignalSubmission, 근거·반론·메타데이터로 완전한 기록을 만든다. 거래 상태나 신호 조건은 받지 않는다. 빈 배치는 IO 없이 성공한다.

기존 EstimateRepository의 append_submission은 실행 배정·단일 소비와 완전한 기록을 원자적으로 저장하지 못하므로 별도의 최소 EstimateSubmissionRepository 포트를 추가한다. assignments로 읽은 이전 원장 전체와 다음 원장 및 기록을 commit_submission에 전달한다. 어댑터는 이전 원장 일치 확인·다음 원장 교체·기록 추가를 단일 트랜잭션으로 처리한다. Available→Consumed ID 집합과 추가 기록의 ID·실행 집합이 정확히 일대일인지 확인하고 실패·충돌 시 아무것도 쓰지 않는다. 이 포트는 기존 저장 포트의 의미를 바꾸지 않는다. 충돌은 ConcurrentUpdate로 반환하며 자동 재제출하지 않는다. 호출자는 최신 원장에서 다시 시도할 수 있다. 서로 다른 과제의 제출이나 새 발급도 전체 원장 비교에서 충돌할 수 있다. E-02·S-09는 ConcurrentUpdate를 성공으로 삼키거나 원래 제출을 버리지 않고, 같은 실행·과제·확률·시각·메타데이터를 보존한 채 최신 원장에서 내부 재시도한다. AI나 사용자에게 다시 제출시키지 않는다. 같은 과제가 이미 소비됐으면 기존 중복 거부가 적용된다. 재시도는 E-02·S-09의 실행 수명 정책과 연결한다. 자료에 없는 고정 횟수나 A-12 내부 자동 재시도를 임의로 추가하지 않는다.

각 기록의 과제에 봉인된 변동성의 InstrumentId로 저장 성공 뒤 EstimateSubmitted를 정확히 한 번 발행한다. DeliveryReport를 반환한다. 구독자 실패는 저장 실패가 아니며 저장 취소·재발행하지 않는다. 프로세스 종료를 가로지르는 영속 이벤트 전달은 ADR 0065의 범위가 아니다. 배치의 입력 순서대로 기록별 이벤트를 내므로 같은 종목의 서로 다른 기록 두 개는 두 이벤트다. 검증 실패는 배치 전체를 거부하며 유효한 일부만 소비하거나 기록하지 않는다. 이 정책은 ADR 0115의 배치 전이와 일치한다.

EstimateSubmissionInput은 application 조립 입력이며 MCP DTO가 아니다. 실행 ID는 신뢰한 실행 컨텍스트에서, SignalSubmission의 CalibrationKey 모델은 실제 응답 모델 관측에서, 메타데이터는 러너·프롬프트 로더에서 전달해야 한다. UI·AI 입력이 이를 정하지 않는다. 도구 경계에서 확률은 기존 AiProbability 파서만 사용한다. Missing은 출처 없음이며 External은 원장 검증 완료를 뜻하지 않는다.

EstimateSubmissionRepository::submission_records(InstrumentId)는 같은 저장 데이터에서 해당 종목의 완전한 EstimateSubmissionRecord 목록을 저장 순서대로 반환한다. 없는 종목은 빈 목록이다. 근거·반론·출처 상태·AI-6 메타데이터와 원래 SignalSubmission을 모두 보존한다. 이벤트 구독자는 이 포트로 방금 저장한 기록을 다시 읽으며, A-13·A-15는 ADR 0115대로 그 안의 SignalSubmission·EstimateTask를 소비한다. S-18이 실제 조회·영속 저장 어댑터를 구현하고 E-06은 이 기록의 출처 상태와 표시 정보를 사용한다. 기존 EstimateRepository 읽기 포트가 필요한 소비자에게는 같은 기록의 SignalSubmission 투영을 제공하며 legacy append_submission을 다시 호출하지 않는다. 신규 제품 제출 경로는 SubmitEstimate만 사용하고 기존 append_submission을 AI·MCP의 별도 저장 경로로 연결하지 않는다. 기존 trait·테스트는 이번 작업에서 제거하거나 약화하지 않으며 S-18이 이 연결 방침을 검증한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| S-18 | 새 포트의 원자 비교 교체와 완전한 기록 영속 저장. 기존 EstimateRepository::submissions의 같은 저장 데이터 투영 조회, ID와 실행의 일대일 검증, 재시작 복원. submission_records의 완전 기록 조회 구현. 기존 append_submission을 별도 제품 제출 경로로 연결하지 않음 |
| E-01, E-02, S-09 | 코드 발급 경로와 신뢰한 실행 컨텍스트 연결. AI에는 task_id·p·근거·반론·출처만 허용. E-02·S-09가 ConcurrentUpdate의 원래 제출을 보존하고 최신 원장으로 내부 재시도하며 오류를 버리지 않음 |
| AI-04, AI-07, AI-16, AI-17 | 실제 응답 모델·추론 강도·CLI 버전·프롬프트 ID/버전/해시·증거 묶음 전달 |
| E-06 | 기록의 Missing을 messages.ts에서 출처 없음으로 표시. External을 검증된 출처로 표시하지 않음. submission_records에서 읽은 완전 기록의 출처 상태·표시 정보 사용 |
| A-13, A-15 | 이벤트 종목 키로 submission_records 조회 후 기록의 SignalSubmission·EstimateTask 사용. 거래하지 않은 기록도 판정 |

## 검증

프로젝트 신뢰·훅 정의 신뢰·현재 세션 적용은 확인 불가. 지시문의 진행 예외 사용. frozen install과 lefthook install 성공. 새 의존성·기존 테스트 변경·실제 키·계좌·API 접근 없음. 테스트를 먼저 작성해 공개 진입점 부재 컴파일 실패를 확인했다(동시에 fixture의 Market import 누락도 있었고 보정했다).

## 리뷰 보강

1차 Opus의 중요 세 건을 새 Codex가 실행으로 비판했다. 같은 저장 객체의 기존 포트 투영 조회가 가능하며, 서로 다른 과제 충돌 뒤에도 실패 과제는 소비되지 않고 재시도할 수 있었다. 완전 기록 조회는 최소 메서드 submission_records로 정의하고 내부 재시도 책임은 위 인계에 명시했다. 보호 생성물·해시는 원 지시문이 승인했고 번호 없는 초안도 지시한 형식이다.

기존 9개 테스트에서 실제 생존한 M1(첫 기록 종목 고정)·M2(빈 배치 조기 반환 제거)·M7(이벤트 역순)을 보강한 테스트에서 다시 실행해 모두 검출했다. 두 종목과 서로 다른 메타데이터·근거·확률, 호출 이력, TaskMismatch의 원장 전체 보존과 commit 미호출, 동일 실행·다른 실행의 서로 다른 과제 경쟁 및 재시도를 검사한다. master 기존 테스트는 수정하지 않았다. 2차 비판의 External 보존과 기존 기록 이후 선발행 생존을 반영해 Web·News·Disclosure·Tool-only·혼합 출처 및 이벤트 시점의 공개 포트 조회 스냅숏을 추가했다. 동시 충돌 재시도는 원래 입력을 그대로 사용하고 submission·근거·메타데이터 일치를 단언한다. submission_records 이외의 제품 실행 로직은 첫 구현 커밋과 같다.

## 손 변형

현재 제품 소스와 통합 테스트 14개를 mktemp 복사본에 넣고 기준 통과 뒤 33개를 실제 실행했다. 컴파일된 31개 전부 검출, 생존 0이다. 컴파일 불가 2개는 검출 수에서 제외한다. 세부 로그는 원 구현 작업 사본의 target/A-12/fix3-hand-*.log와 fix3-hand-results.json이다. target은 Git 추적 자료가 아니므로 다른 작업 사본에는 복사되지 않는다. 아래 표는 독립 복사본에서 변형을 하나씩 적용한 실행 기록이며 과거 9개·26개 결과와 구별한다.

재현은 같은 소스와 테스트를 임시 복사본에 놓고 무변형 통과를 확인한 뒤, 표의 변경을 하나씩 적용해 다음 명령을 실행한다. 각 변형 뒤 원본으로 되돌리고 컴파일 오류를 검출로 세지 않는다. 32번은 기존 소비 과제가 있으면 커밋 전에 이벤트를 내고 커밋 성공 뒤에는 재발행하지 않는 변형이다. 33번은 기록 생성에 전달하는 SignalSubmission의 evidence만 None으로 바꾼다.

```sh
cargo test --locked -p testkit-ports --test submit_estimate
```

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | 빈 배치 조건 반전 | 컴파일 성공·검출 | tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::invalid_record_rolls_back_entire_batch, tests::task_mismatch_preserves_every_assignment_and_record, tests::source_references_and_status_survive_submission, tests::delivery_failure_preserves_success_without_republishing, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::complete_records_are_saved_before_exactly_one_event_each, tests::empty_batch_has_no_effect, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::duplicate_after_success_never_records_or_publishes_again, tests::invalid_batches_leave_every_record_and_assignment_untouched, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes, tests::simultaneous_different_payloads_accept_only_one |
| 2 | 모든 제출을 빈 성공으로 무시 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::duplicate_after_success_never_records_or_publishes_again, tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes, tests::source_references_and_status_survive_submission, tests::task_mismatch_preserves_every_assignment_and_record, tests::invalid_batches_leave_every_record_and_assignment_untouched, tests::delivery_failure_preserves_success_without_republishing, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::invalid_record_rolls_back_entire_batch, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one |
| 3 | 빈 배치를 실패로 처리 | 컴파일 성공·검출 | tests::empty_batch_has_no_effect |
| 4 | 소비 검증 생략 | 컴파일 성공·검출 | tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 5 | 첫 ID를 두 번 소비 | 컴파일 성공·검출 | tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::duplicate_after_success_never_records_or_publishes_again, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::task_mismatch_preserves_every_assignment_and_record, tests::source_references_and_status_survive_submission, tests::invalid_record_rolls_back_entire_batch, tests::delivery_failure_preserves_success_without_republishing, tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 6 | 마지막 요청 누락 | 컴파일 성공·검출 | tests::invalid_record_rolls_back_entire_batch, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::complete_records_are_saved_before_exactly_one_event_each, tests::task_mismatch_preserves_every_assignment_and_record |
| 7 | 기록 순서 반전 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::complete_record_query_filters_instruments_and_preserves_every_field |
| 8 | 근거를 반론으로 덮어쓰기 | 컴파일 성공·검출 | tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::complete_records_are_saved_before_exactly_one_event_each |
| 9 | 반론을 근거로 덮어쓰기 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 10 | 추론 강도 덮어쓰기 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 11 | CLI 버전 덮어쓰기 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 12 | 프롬프트 버전 덮어쓰기 | 컴파일 성공·검출 | tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::complete_records_are_saved_before_exactly_one_event_each |
| 13 | 프롬프트 ID 덮어쓰기 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 14 | 증거 묶음 ID 덮어쓰기 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 15 | 프롬프트 해시 덮어쓰기 | 컴파일 성공·검출 | tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::complete_records_are_saved_before_exactly_one_event_each |
| 16 | 비교할 이전 원장을 다음 원장으로 교체 | 컴파일 성공·검출 | tests::delivery_failure_preserves_success_without_republishing, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::source_references_and_status_survive_submission, tests::duplicate_after_success_never_records_or_publishes_again, tests::complete_records_are_saved_before_exactly_one_event_each, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::invalid_record_rolls_back_entire_batch, tests::simultaneous_different_payloads_accept_only_one |
| 17 | 소비 상태 저장 누락 | 컴파일 성공·검출 | tests::duplicate_after_success_never_records_or_publishes_again, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one |
| 18 | 기록 저장 누락 | 컴파일 성공·검출 | tests::delivery_failure_preserves_success_without_republishing, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::complete_records_are_saved_before_exactly_one_event_each, tests::duplicate_after_success_never_records_or_publishes_again, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::invalid_record_rolls_back_entire_batch, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::source_references_and_status_survive_submission, tests::simultaneous_different_payloads_accept_only_one |
| 19 | 경쟁 충돌을 성공으로 처리 | 컴파일 성공·검출 | tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one |
| 20 | 성공 보고·이벤트 생략 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each, tests::duplicate_after_success_never_records_or_publishes_again, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::delivery_failure_preserves_success_without_republishing, tests::source_references_and_status_survive_submission, tests::simultaneous_different_payloads_accept_only_one, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state |
| 21 | 이벤트 누락 | 컴파일 성공·검출 | tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::source_references_and_status_survive_submission, tests::complete_records_are_saved_before_exactly_one_event_each, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit, tests::duplicate_after_success_never_records_or_publishes_again, tests::delivery_failure_preserves_success_without_republishing, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one |
| 22 | 전달 실패 보고 버리기 | 컴파일 성공·검출 | tests::delivery_failure_preserves_success_without_republishing |
| 23 | 저장 오류 종류 변경 | 컴파일 성공·검출 | tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes |
| 24 | 배정 오류 종류 변경 | 컴파일 성공·검출 | tests::duplicate_after_success_never_records_or_publishes_again, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 25 | 기록 오류 종류 변경 | 컴파일 성공·검출 | tests::task_mismatch_preserves_every_assignment_and_record, tests::invalid_record_rolls_back_entire_batch |
| 26 | 이벤트 중복 | 컴파일 성공·검출 | tests::complete_record_query_filters_instruments_and_preserves_every_field, tests::distinct_tasks_conflict_without_consumption_and_retry_from_latest_state, tests::simultaneous_different_payloads_accept_only_one, tests::complete_records_are_saved_before_exactly_one_event_each, tests::source_references_and_status_survive_submission, tests::delivery_failure_preserves_success_without_republishing, tests::duplicate_after_success_never_records_or_publishes_again, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit |
| 27 | execute 전체 빈 성공 | 컴파일 불가·집계 제외 | - |
| 28 | execute 전체 단일 성공 | 컴파일 불가·집계 제외 | - |
| 29 | M1 첫 기록 종목 고정 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each |
| 30 | M2 빈 배치 조기 반환 제거 | 컴파일 성공·검출 | tests::empty_batch_has_no_effect |
| 31 | M7 이벤트 발행 순서 반전 | 컴파일 성공·검출 | tests::complete_records_are_saved_before_exactly_one_event_each |
| 32 | M8 기존 기록이 있으면 이번 저장 전에 발행 | 컴파일 성공·검출 | tests::each_event_observes_its_new_record_after_existing_records_and_failed_commit |
| 33 | M9 제출의 출처 참조 제거 | 컴파일 성공·검출 | tests::source_references_and_status_survive_submission |

공식 mutants-diff 최종 재실행은 2개 선택·2개 미검출이다. application만 테스트하여 testkit-ports의 통합 테스트를 누락한다. 동일한 현재 소스·통합 테스트 14개에서 별도의 --test-package testkit-ports 자동 실행은 같은 execute 대체 변형 두 개를 모두 검출했다(33초). 공식 결과를 통과로 바꾸어 보고하지 않는다. target-outside-member나 변이 하네스·테스트 위치·의존 방향은 변경하지 않았다.

| 실행 입력 | SHA-256 |
|---|---|
| crates/application/src/submit_estimate.rs | ad3b730f6c80b13cc4f9922050b2dfd77bb8f9cba5e8fbf80b0a7a197c94c6d6 |
| crates/testkit-ports/tests/submit_estimate.rs | 23b991257e19a31264d364f2ec8d5749c4d13671d763165e0c4ead3e7eb7009e |
