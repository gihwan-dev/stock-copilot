# A-12: D-47을 사용하는 추정 제출

- 상태: 초안
- 기준: 994c25a191e0ab08e2c8e9f1f26cfa2de7be3f4d

## 결정

ADR 0115의 EstimateAssignments<AiRunId>와 EstimateSubmissionRecord<AiRunId, ReasoningEffort>를 그대로 사용한다. application에서 발급·검증·계산 규칙을 재정의하지 않는다. 배치 전체를 domain consume으로 검증하고 순수 다음 상태를 만든다. 원장의 lookup 결과와 기존 SignalSubmission, 근거·반론·메타데이터로 완전한 기록을 만든다. 거래 상태나 신호 조건은 받지 않는다. 빈 배치는 IO 없이 성공한다.

기존 EstimateRepository의 append_submission은 실행 배정·단일 소비와 완전한 기록을 원자적으로 저장하지 못하므로 별도의 최소 EstimateSubmissionRepository 포트를 추가한다. assignments로 읽은 이전 원장 전체와 다음 원장 및 기록을 commit_submission에 전달한다. 어댑터는 이전 원장 일치 확인·다음 원장 교체·기록 추가를 단일 트랜잭션으로 처리한다. Available→Consumed ID 집합과 추가 기록의 ID·실행 집합이 정확히 일대일인지 확인하고 실패·충돌 시 아무것도 쓰지 않는다. 이 포트는 기존 저장 포트의 의미를 바꾸지 않는다. 충돌은 ConcurrentUpdate로 반환하며 자동 재제출하지 않는다. 호출자는 최신 원장에서 다시 시도할 수 있다.

각 기록의 과제에 봉인된 변동성의 InstrumentId로 저장 성공 뒤 EstimateSubmitted를 정확히 한 번 발행한다. DeliveryReport를 반환한다. 구독자 실패는 저장 실패가 아니며 저장 취소·재발행하지 않는다. 프로세스 종료를 가로지르는 영속 이벤트 전달은 ADR 0065의 범위가 아니다.

EstimateSubmissionInput은 application 조립 입력이며 MCP DTO가 아니다. 실행 ID는 신뢰한 실행 컨텍스트에서, SignalSubmission의 CalibrationKey 모델은 실제 응답 모델 관측에서, 메타데이터는 러너·프롬프트 로더에서 전달해야 한다. UI·AI 입력이 이를 정하지 않는다. 도구 경계에서 확률은 기존 AiProbability 파서만 사용한다. Missing은 출처 없음이며 External은 원장 검증 완료를 뜻하지 않는다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| S-18 | 새 포트의 원자 비교 교체와 완전한 기록 영속 저장·종목별 조회 구현, ID와 실행의 일대일 검증, 재시작 복원 |
| E-01, E-02, S-09 | 코드 발급 경로와 신뢰한 실행 컨텍스트 연결. AI에는 task_id·p·근거·반론·출처만 허용 |
| AI-04, AI-07, AI-16, AI-17 | 실제 응답 모델·추론 강도·CLI 버전·프롬프트 ID/버전/해시·증거 묶음 전달 |
| E-06 | 기록의 Missing을 messages.ts에서 출처 없음으로 표시. External을 검증된 출처로 표시하지 않음 |
| A-13, A-15 | 이벤트 종목 키로 완전한 기록 조회. 거래하지 않은 기록도 판정 |

## 검증

프로젝트 신뢰·훅 정의 신뢰·현재 세션 적용은 확인 불가. 지시문의 진행 예외 사용. frozen install과 lefthook install 성공. 새 의존성·기존 테스트 변경·실제 키·계좌·API 접근 없음. 테스트를 먼저 작성해 공개 진입점 부재 컴파일 실패를 확인했다(동시에 fixture의 Market import 누락도 있었고 보정했다).

## 손 변형

최종 제품 소스와 보강한 통합 테스트를 mktemp 임시 복사본에 넣어 기준 9개 통과 후 서로 다른 변형을 실제 실행했다. 컴파일된 26개는 전부 검출되었고, 컴파일 불가 2개는 집계에서 제외한다. 세부 로그는 target/A-12/hand-*.log와 hand-results.json에 보존했다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | empty inversion | caught | tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one, tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes, tests::delivery_failure_preserves_success_without_republishing, tests::invalid_record_rolls_back_entire_batch, tests::complete_records_are_saved_before_exactly_one_event_each, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 2 | all submissions ignored | caught | tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::simultaneous_different_payloads_accept_only_one, tests::duplicate_after_success_never_records_or_publishes_again, tests::invalid_record_rolls_back_entire_batch, tests::delivery_failure_preserves_success_without_republishing, tests::complete_records_are_saved_before_exactly_one_event_each, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 3 | empty fails | caught | tests::empty_batch_has_no_effect |
| 4 | skip consumption | caught | tests::simultaneous_different_payloads_accept_only_one, tests::duplicate_after_success_never_records_or_publishes_again, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 5 | consume repeated first ID | caught | tests::duplicate_after_success_never_records_or_publishes_again, tests::delivery_failure_preserves_success_without_republishing, tests::simultaneous_different_payloads_accept_only_one, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::invalid_record_rolls_back_entire_batch, tests::complete_records_are_saved_before_exactly_one_event_each, tests::invalid_batches_leave_every_record_and_assignment_untouched |
| 6 | drop last request | caught | tests::invalid_record_rolls_back_entire_batch, tests::complete_records_are_saved_before_exactly_one_event_each |
| 7 | reverse record order | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 8 | rationale overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 9 | counterargument overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 10 | effort overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 11 | CLI version overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 12 | prompt version overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 13 | prompt id overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 14 | bundle overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 15 | prompt hash overwritten | caught | tests::complete_records_are_saved_before_exactly_one_event_each |
| 16 | wrong expected state | caught | tests::complete_records_are_saved_before_exactly_one_event_each, tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::invalid_record_rolls_back_entire_batch, tests::delivery_failure_preserves_success_without_republishing |
| 17 | consumption not saved | caught | tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one |
| 18 | records not saved | caught | tests::complete_records_are_saved_before_exactly_one_event_each, tests::delivery_failure_preserves_success_without_republishing, tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::invalid_record_rolls_back_entire_batch |
| 19 | conflict accepted | caught | tests::simultaneous_different_payloads_accept_only_one |
| 20 | success emits no reports | caught | tests::delivery_failure_preserves_success_without_republishing, tests::complete_records_are_saved_before_exactly_one_event_each, tests::duplicate_after_success_never_records_or_publishes_again, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::simultaneous_different_payloads_accept_only_one |
| 21 | event omitted | caught | tests::delivery_failure_preserves_success_without_republishing, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::complete_records_are_saved_before_exactly_one_event_each, tests::duplicate_after_success_never_records_or_publishes_again, tests::simultaneous_different_payloads_accept_only_one |
| 22 | delivery report discarded | caught | tests::delivery_failure_preserves_success_without_republishing |
| 23 | storage error changed | caught | tests::read_failure_preserves_exact_cause_and_never_commits_or_publishes |
| 24 | assignment error changed | caught | tests::invalid_batches_leave_every_record_and_assignment_untouched, tests::duplicate_after_success_never_records_or_publishes_again |
| 25 | record error changed | caught | tests::invalid_record_rolls_back_entire_batch |
| 26 | event duplicated | caught | tests::delivery_failure_preserves_success_without_republishing, tests::duplicate_after_success_never_records_or_publishes_again, tests::complete_records_are_saved_before_exactly_one_event_each, tests::failed_commit_does_not_consume_or_publish_and_can_retry, tests::simultaneous_different_payloads_accept_only_one |
| 27 | whole execute empty success | unviable |  |
| 28 | whole execute single success | unviable |  |

공식 mutants-diff는 2개를 선택했으나 application 패키지만 실행하여 testkit-ports의 통합 테스트를 누락했고 두 execute 대체 변형을 미검출로 보고했다. 테스트를 application으로 옮기거나 crate 밖을 테스트 진입점으로 등록하는 대신 기존 규칙을 보존하고 별도의 --test-package testkit-ports 자동 실행으로 보완 검증한다. target-outside-member 규칙을 변경하지 않았다.
