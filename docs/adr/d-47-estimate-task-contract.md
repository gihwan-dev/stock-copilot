# D-47: 추정 과제 발급·배정·소비와 제출 기록

- 상태: 초안
- 기준: a8d20344ebdcf8d1044189a97cce531b8b41c0af
- 근거: ACC-9, AI-6, ADR 0027·0046·0051·0071, A-12 정의 대기 분석

## 결정

하나의 저장 원장을 EstimateAssignments로 표현한다. issue만 IssuedEstimateTaskId를 만든다. ID의 외부 표현은 기존 CalibrationEstimateId이며 lookup은 그 외부 값을 원장과 대조한다. 외부 값 파싱은 발급 사실을 만들지 않는다. issue는 코드가 정한 CalibrationEstimateId, CostEligibleTask와 실행을 받는다. ID의 유일성은 원장 전체에서 검사한다. 저장소는 원장 스냅샷의 이전 버전과 다음 버전을 비교해 원자적으로 교체해야 한다. 순수 함수의 호출만으로 동시성이나 영속성이 보장되지는 않는다.

실행과 추론 강도는 각각 Run, Effort 제네릭으로 둔다. 이미 application에 있는 AiRunId와 ReasoningEffort를 그대로 사용하기 위한 것으로 domain에서 같은 타입을 재정의하거나 application에 역의존하지 않는다. 실제 응답 모델은 SignalSubmission의 CalibrationKey.model에 보존한다. 요청 모델이나 라우팅 사건으로 대신 채우지 않는 연결 책임은 A-12·AI-04에 있다.

소비는 이전 원장을 바꾸지 않고 새 원장을 반환한다. 배치 크기가 배정 수를 넘으면 AssignmentExceeded, 그 외 미지 값 UnknownTask, 다른 실행 WrongRun, 같은 과제의 재소비 AlreadyConsumed다. 배치 중간의 실패도 원래 원장을 소비하지 않는다. 빈 배치는 같은 상태다. 배정은 발급과 함께 고정하며 재배정 API는 없다.

제출 기록은 기존 SignalSubmission을 통째로 보존하며 근거와 반론은 UntrustedText다. 발급된 과제와 제출 과제의 값이 다르면 TaskMismatch로 거부한다. 제출의 CalibrationKey 기간을 기존 TradingDayHorizon 정책으로 바꾼 값이 과제 기간과 다르면 HorizonMismatch로 거부한다. 기간의 별도 수치 대응을 새로 만들지 않는다. 기록 생성은 소비나 저장 성공을 뜻하지 않으며 거래 조건도 받지 않는다. 도구 참조만 있거나 출처가 없으면 Missing이다. 이는 한국어 UI의 "출처 없음"에 대응한다. 실제 표시는 E-06의 messages.ts가 담당한다. External은 외부 참조가 제출됐다는 종류 분류이며 원장 검증 완료나 신호의 출처 적격성을 증명하지 않는다. 모든 원본 참조를 보존하고 SignalSourceLedger의 원장 대조를 그대로 사용한다. E-06은 External만으로 검증된 출처라고 표시하지 않는다.

프롬프트 해시는 64자리 ASCII 16진수로 제한하고 소문자로 정규화한다. 같은 해시 바이트의 대소문자 표기는 같은 값이다. 해시 계산은 경계 책임이다. CLI 버전과 프롬프트 버전은 기존 ModelIdentityPart의 검증된 불투명 문자열을 재사용한다. 증거 묶음은 EvidenceBundleId로 항목 ID와 구분한다. 묶음의 경계 문자열 형식은 기존 CalibrationEstimateId 파서를 재사용하되 과제 ID 타입을 노출하지 않고 as_str로만 내보낸다. 다른 ID로 사용하려면 경계에서 명시적으로 다시 파싱해야 한다. 공개 필드를 가진 입력 묶음은 기록 생성 때 소유권을 넘기므로 생성 뒤 기록을 수정하지 못한다.

## 기존 타입 대응

| 사실 | 기존 타입 / 결합 |
|---|---|
| 사건·기준가·시각·변동성 | CostEligibleTask → EstimateTask (D-21·D-22) |
| 원래 확률·출처·제출 시각 | SignalSubmission (AiProbability, Evidence, Timestamp) |
| 실제 모델·기간 | SignalSubmission → CalibrationKey → CalibrationModel / Horizon |
| 외부 제출 식별 표현 | CalibrationEstimateId |
| 실행 ID / 추론 강도 | application::AiRunId / ReasoningEffort를 Run / Effort에 대입 |
| 근거·반론 | UntrustedText |
| 프롬프트 ID | ScorecardPromptId |
| CLI·프롬프트 버전 | ModelIdentityPart |
| 프롬프트 해시 / 증거 묶음 ID | PromptHash / EvidenceBundleId (신규 의미) |

## 공개 계약

| 타입 | 공개 생성·전이·조회 |
|---|---|
| `EstimateAssignments<Run>` | default로 빈 상태, issue로 코드의 과제 발급·배정, lookup으로 외부 식별 표현 대조, assigned로 실행 배정 목록, consume으로 순수 다음 상태 |
| `IssuedEstimateTask<Run>`, IssuedEstimateTaskId | 직접 생성 없음. id·run·task·consumption을 불변 조회하고 발급 ID의 as_estimate_id로 기존 외부 표현을 읽음 |
| EstimateAssignmentError, EstimateConsumption | AlreadyIssued·UnknownTask·WrongRun·AlreadyConsumed·AssignmentExceeded / Available·Consumed |
| `EstimateSubmissionRecord<Run, Effort>` | create에서 과제와 기간 일치 검사. task_id·run·submission·narrative·metadata·source_status 조회 |
| EstimatePrompt, `EstimateMetadata<Effort>`, EstimateNarrative | 필수 필드를 소유하는 입력 묶음. 프롬프트 ID·버전·해시, 추론 강도·CLI 버전·증거 묶음, 근거·반론 |
| PromptHash, EvidenceBundleId | try_new로 각 경계 검증, as_str로 경계 문자열 조회 |
| EstimateRecordError, EstimateMetadataError, EstimateSourceStatus | TaskMismatch·HorizonMismatch / InvalidHash·InvalidBundleId / Missing·External |

## A-12의 아홉 기준

| 기준 | 계약 준비 | 남은 실행 책임 |
|---|---|---|
| 미지 task_id 거부 | lookup / consume | A-12가 저장 원장을 읽고 호출 |
| 다른 실행 거부 | WrongRun | 신뢰한 실행 컨텍스트 전달 |
| 배정 수 이하 | 배정 집합·한 번 소비 | 원자적 상태 교체 |
| 중복·동시 중복 한 번 | AlreadyConsumed·순수 전이 | S-18의 비교 교체와 기록 추가 단일 트랜잭션 |
| 출처 없음 | source_status Missing | E-06 messages.ts 표시 |
| 모든 제출 기록 | EstimateSubmissionRecord | S-18 누락 없는 저장, A-15 판정 연결 |
| AI-6 | 완전한 필수 필드 | 실제 관측 모델·CLI·프롬프트·묶음 연결 |
| 기록 뒤 이벤트 한 번 | 기존 DomainEvent 재사용 가능 | A-12 저장 성공 뒤 발행 |
| 저장 실패 이벤트 없음 | 순수 전이로 이전 상태 보존 가능 | A-12 실패 경로·동시성 통합 테스트 |

## 인계

- A-12: Run=AiRunId, Effort=ReasoningEffort로 연결한다. 실제 응답 모델을 CalibrationKey에 넣는다. 기록 생성·소비·저장을 함께 처리하고 성공 뒤 InstrumentId 이벤트를 한 번 발행한다.
- S-18: 스냅샷 비교 교체와 완전한 기록 추가를 단일 트랜잭션으로 적용한다. 이전 Available에서 다음 Consumed로 바뀐 ID 집합과 추가 기록의 task_id·run 집합이 정확히 일대일인지 확인한다. 기록 생성만으로 소비를 증명한다고 취급하지 않는다. 실패는 소비하지 않는다. 원장 전체 ID 유일성과 재시작 복원을 보장한다. 역직렬화를 domain에 추가하지 않는다. 같은 내용의 다른 발급 ID는 구별되며 내용 중복과 기간 내 반복 제한은 ADR 0057대로 E·D-25에서 적용한다.
- E-01·E-02: 발급 함수는 코드 경로만 호출한다. AI 입력은 lookup과 소비의 외부 키로만 사용한다.
- E-06: Missing을 "출처 없음"으로 표시한다. A-13·A-15: 기록의 기존 SignalSubmission·EstimateTask를 사용한다.

## 검증

세 회차의 독립 리뷰와 비판을 마쳤다. 3차 비판에서 실제 생존한 News·Disclosure 분기 변형을 잡도록 단독 출처 사례와 참조 보존 단언을 추가했다. 도구만 있는 경우의 Missing과 Web·News·Disclosure의 External을 각각 검사한다. External의 의미는 참조 종류이며 신호의 원장 적격성을 대신하지 않는다.

프로젝트 신뢰·훅 정의 신뢰·현재 세션 강제 적용은 확인할 수 없었다. 지시문의 계속 진행 예외를 적용했다. frozen install·lefthook install 성공. 실제 계좌·키·API 접근 없음. 새 의존성·기존 테스트 수정 없음. 테스트 우선 작성 순서는 지키지 못했으며 실행 근거를 별도로 기록한다.

## 손 변형

3차 뒤 테스트를 보강한 최종 소스의 독립 임시 복사본에서 30개를 실제 실행했다. 컴파일 성공 28개는 모두 계약 테스트에 검출됐으며 중복 패턴과 사용하지 않는 변수로 컴파일되지 않은 2개는 집계에서 제외한다. 원본 기준 실행은 계약 테스트 8개 통과였다. News와 Disclosure를 각각 false로 바꾼 두 변형도 컴파일되고 새 단언에 검출됐다. 각 로그는 작업 사본 target/D-47/manual-*.log에 있다.

| 번호 | 바꾼 것 | 실행 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | 중복 발급 any→all | 검출, exit 101 | issuance_and_consumption_are_pure_and_errors_are_distinct, record_rejects_calibration_period_that_differs_from_issued_task, complete_record_preserves_independent_fields_without_trade_input, consumption_is_atomic_and_repeated_consumption_is_rejected, source_absence_includes_tool_only_and_preserves_all_references, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 2 | 발급 키 비교 반전 | 검출, exit 101 | complete_record_preserves_independent_fields_without_trade_input, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments, issuance_and_consumption_are_pure_and_errors_are_distinct, consumption_is_atomic_and_repeated_consumption_is_rejected, source_absence_includes_tool_only_and_preserves_all_references |
| 3 | 발급 즉시 소비 | 검출, exit 101 | arbitrary_consumption_sequences_never_repeat_or_exceed_assignments, consumption_is_atomic_and_repeated_consumption_is_rejected |
| 4 | 조회 키 비교 반전 | 검출, exit 101 | record_rejects_calibration_period_that_differs_from_issued_task, complete_record_preserves_independent_fields_without_trade_input |
| 5 | 배정 실행 반전 | 검출, exit 101 | consumption_is_atomic_and_repeated_consumption_is_rejected, issuance_and_consumption_are_pure_and_errors_are_distinct, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 6 | 상한 등호 | 검출, exit 101 | consumption_is_atomic_and_repeated_consumption_is_rejected |
| 7 | 상한 반전 | 검출, exit 101 | issuance_and_consumption_are_pure_and_errors_are_distinct, consumption_is_atomic_and_repeated_consumption_is_rejected, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 8 | 실행 거부 반전 | 검출, exit 101 | consumption_is_atomic_and_repeated_consumption_is_rejected, issuance_and_consumption_are_pure_and_errors_are_distinct, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 9 | 소비 가능 분기 반전 | 컴파일 불가(집계 제외), exit 101 | - |
| 10 | 소비 상태 누락 | 검출, exit 101 | consumption_is_atomic_and_repeated_consumption_is_rejected, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 11 | 발급 오류 혼동 | 검출, exit 101 | issuance_and_consumption_are_pure_and_errors_are_distinct |
| 12 | 상한 오류 혼동 | 검출, exit 101 | issuance_and_consumption_are_pure_and_errors_are_distinct |
| 13 | 실행 오류 혼동 | 검출, exit 101 | issuance_and_consumption_are_pure_and_errors_are_distinct, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 14 | 소비 오류 혼동 | 검출, exit 101 | consumption_is_atomic_and_repeated_consumption_is_rejected, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 15 | 발급 상태 전이 누락 | 검출, exit 101 | complete_record_preserves_independent_fields_without_trade_input, record_rejects_calibration_period_that_differs_from_issued_task, source_absence_includes_tool_only_and_preserves_all_references, issuance_and_consumption_are_pure_and_errors_are_distinct, consumption_is_atomic_and_repeated_consumption_is_rejected, arbitrary_consumption_sequences_never_repeat_or_exceed_assignments |
| 16 | 소비 접근자 고정 | 검출, exit 101 | arbitrary_consumption_sequences_never_repeat_or_exceed_assignments, consumption_is_atomic_and_repeated_consumption_is_rejected |
| 17 | 해시 긴 값 허용 | 검출, exit 101 | prompt_hash_rejects_bad_length_and_non_hex |
| 18 | 해시 문자 반전 | 검출, exit 101 | prompt_hash_rejects_bad_length_and_non_hex, record_rejects_calibration_period_that_differs_from_issued_task, source_absence_includes_tool_only_and_preserves_all_references, complete_record_preserves_independent_fields_without_trade_input |
| 19 | 과제 결합 반전 | 검출, exit 101 | source_absence_includes_tool_only_and_preserves_all_references, record_rejects_calibration_period_that_differs_from_issued_task, complete_record_preserves_independent_fields_without_trade_input |
| 20 | 해시 원문 소실 | 검출, exit 101 | prompt_hash_rejects_bad_length_and_non_hex, complete_record_preserves_independent_fields_without_trade_input |
| 21 | 실질 출처 소실 | 검출, exit 101 | source_absence_includes_tool_only_and_preserves_all_references |
| 22 | 도구 출처 오인 | 검출, exit 101 | complete_record_preserves_independent_fields_without_trade_input, source_absence_includes_tool_only_and_preserves_all_references |
| 23 | 도구 참조 출처 포함 | 검출, exit 101 | source_absence_includes_tool_only_and_preserves_all_references |
| 24 | 해시 정규화 제거 | 검출, exit 101 | prompt_hash_rejects_bad_length_and_non_hex |
| 25 | 기록 기간 비교 반전 | 검출, exit 101 | record_rejects_calibration_period_that_differs_from_issued_task, complete_record_preserves_independent_fields_without_trade_input, source_absence_includes_tool_only_and_preserves_all_references |
| 26 | 기간 오류 종류 혼동 | 검출, exit 101 | record_rejects_calibration_period_that_differs_from_issued_task |
| 27 | 증거 묶음 식별값 소실 | 컴파일 불가(집계 제외), exit 101 | - |
| 28 | 증거 묶음 오류 혼동 | 검출, exit 101 | evidence_bundle_preserves_its_own_boundary_without_task_id_access |
| 29 | 뉴스 출처 분기만 제거 | 검출, exit 101 | source_absence_includes_tool_only_and_preserves_all_references |
| 30 | 공시 출처 분기만 제거 | 검출, exit 101 | source_absence_includes_tool_only_and_preserves_all_references |

환경 종료 안내 이후에는 DEVELOPER_DIR와 환경 사유의 훅 예외를 사용하지 않는다. 과거 승인된 환경 예외와 최종 정상 훅 검증을 구별하며 최종 병합은 정상 훅으로 확정한다.

최종 변경분 자동 변이: 기준 4433fb28a524c418127ffcafc2f262cf03070bab, 공식 mutants-diff 47개 중 검출 22·컴파일 불가 25·생존 0·시간 초과 0으로 완주했다. News·Disclosure 개별 분기는 위 손 변형 표의 별도 실행으로 확인했다.
