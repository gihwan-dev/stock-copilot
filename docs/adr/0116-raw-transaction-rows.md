# ADR 0116: A-23 거래내역 관측에 원본 행 전달

- 상태: 확정 (2026-11-17)


## 결정

정상 행은 application의 ObservedTransaction에 Transaction과 RawTransactionRow를 함께 보관한다. RawTransactionRow는 받은 UTF-8 JSON 철자를 그대로 소유하는 불투명 newtype이며 application과 domain은 내용을 해석하지 않는다. Debug는 고정 가림 문자열만 반환한다. JSON 유효성과 행의 의미 대응은 broker-toss 파서가 책임진다. 공개 생성자가 임의 원문과 행의 의미 동등성을 증명하지는 않는다.

ObservedTransactionBatch는 결합된 행 목록만 입력받는다. 기존 domain TransactionBatch는 그 목록에서 생성한 읽기 전용 투영이다. CompleteTransactionObservation도 결합된 배치와 조회 범위로만 생성하며 기존 domain 완전성 검사를 호출한다. 정규화 투영을 외부에서 별도로 주입하거나 변경할 수 없다. 원본만의 나란한 목록은 없다. 읽기 전용 정규화 캐시는 기존 domain 접근과 대조를 유지하기 위한 것으로, domain 변경이나 JSON 의존성 추가는 없다.

TransactionRead::Complete의 내용 타입과 IncompleteTransactions.received의 배치 타입을 바꾼다. TransactionSyncRecord가 보관하는 관측과 TransactionSyncRepository::record_sync 서명은 그대로다. 관측에 원본이 포함되므로 같은 저장 호출로 전달된다. 멀티셋, 사라짐, 충돌 거부와 이벤트 규칙은 바꾸지 않는다. UI나 설정은 추가하지 않는다.

T-06의 ParsedTransaction.source를 직렬화하지 않고 get()의 원문으로 연결한다. 정상/격리 혼합, 중단, 중복 행과 재시작은 기존 경로를 따른다. 원문은 행 단위이며 전체 응답이나 헤더를 저장하지 않는다.

객체 안의 중복 JSON 키 처리 정책은 T-06 후속과 S-03의 확인 항목으로 둔다. 수용·격리·거부나 뒤 값 우선을 이번 작업의 회귀 계약으로 고정하지 않는다. 중복 거래 행의 개수 보존과 객체 안 중복 키의 해석은 다른 문제다.

## S-03 판정

원본이 저장 포트에 도달하지 않던 선행 계약의 막힘은 해소된다. S-03은 관측의 observed_rows에서 원문과 정규화 행을 함께 저장할 수 있다. 실제 SQLite 원자 저장, 재구성 및 영속성 검증은 S-03 구현의 완료 조건이다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| S-03 | 원본과 정규화 행, 상세 관측, 요약 및 상태를 기존 저장 계약에 따라 원자적으로 영속화하고 복원 검증 |
| A-07, T-06 | 부분 자료와 중간403·429 원인 및 RetryDelay를 함께 전달할 표현은 범위 밖으로 유지 |
| T-06 후속, S-03 | 중복 JSON 키의 처리 정책과 재정규화 해석 경로를 확인한다. 저장 당시 정규화와 복원 값의 일치를 검증하며 생성 스키마 직접 파싱과 Value 경유의 차이를 고려한다 |
| H-47 | 승인된 기계적 test-diff가 변경분 변이의 전처리를 막은 결과를 인계한다. 자동 변이는 미실행으로 유지 |

## 검증

대상 crate 검사, 변경분 검사, 자동 변이와 손 변형의 실제 결과는 리뷰 기록과 아래 표에 기록한다. 시작 frozen install과 lefthook install 성공. 저장소 hooks=true는 확인했으나 프로젝트 신뢰, 훅 신뢰, 현재 세션 적용은 확인 불가다. 실제 키·계좌·외부 API는 사용하지 않는다. 로컬 합성 HTTP만 사용한다.

## 승인된 기존 테스트 변경

기존 단언은 보존했다. test-diff 지적13건(기존 테스트4건, 지원 코드9건)은 이 작업의 기계적 타입 변경이다.

| 파일 | 이름 | 전 → 후 |
|---|---|---|
| application/tests/ports.rs | domain import 삭제/새 import 판정 2건 | CompleteTransactionResponse·TransactionBatch 제거 |
| application/tests/ports.rs | received | TransactionBatch → ObservedTransactionBatch, 합성 원문 결합 |
| application/tests/ports.rs | impl BrokerRead for FakeBroker | domain 완전 응답 → application 관측 생성자 |
| broker-toss/src/transaction_read.rs | normal_row_source_keeps_received_spelling_until_port_projection | project 튜플 → 결합된 행의 source(), get → as_str |
| broker-toss/tests/transactions.rs | complete | 반환 타입 → CompleteTransactionObservation |
| testkit-ports/tests/sync_transactions.rs | domain import 삭제/새 import 판정 2건 | CompleteTransactionResponse 제거 |
| testkit-ports/tests/sync_transactions.rs | complete | 관측 생성자와 결합 배치 |
| testkit-ports/tests/sync_transactions.rs | initial | reconcile에 normalized() 전달 |
| testkit-ports/tests/sync_transactions.rs | disappearance_warning_survives_storage_and_lookup | reconcile에 normalized() 전달 |
| testkit-ports/tests/sync_transactions.rs | incomplete_cursor_preserves_received_rows_without_disappearance | received의 fixture 배치에 합성 원문 결합 |
| testkit-ports/tests/transaction_schedule.rs | recent_complete_and_incomplete_preserve_observation_and_completion_boundary | 관측 생성자와 normalized() 인자 |

## 손 변형

최종 구현의 mktemp 복사본에서 한 번에 하나씩 실행했다. 첫 시도의 const 생성자 안 문자 치환20개는 컴파일 실패로 제외했다. 생성자의 불필요한 const 제거 후 다시 실행한20개와 구조 변형8개만 아래에 센다. 원복 대조군도 통과했다. 원본 철자 문자 손실과 행 순서·중복·상한·투영을 검증한다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | 원본 문자 A 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 2 | 원본 문자 B 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 3 | 원본 문자 C 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 4 | 원본 문자 D 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 5 | 원본 문자 E 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 6 | 원본 문자 F 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 7 | 원본 문자 G 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 8 | 원본 문자 H 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 9 | 원본 문자 I 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 10 | 원본 문자 J 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 11 | 원본 문자 K 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 12 | 원본 문자 L 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 13 | 원본 문자 M 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 14 | 원본 문자 N 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 15 | 원본 문자 O 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 16 | 원본 문자 P 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 17 | 원본 문자 Q 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 18 | 원본 문자 R 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 19 | 원본 문자 S 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 20 | 원본 문자 T 소실 | source_is_opaque_and_preserves_every_byte_without_debug_disclosure | 컴파일 성공·검출 |
| 21 | 정규화 순서 반전 | tests::observed_batch_bounds_and_order_are_preserved | 컴파일 성공·검출 |
| 22 | 첫 정상 행 제외 | tests::cancellation_notice_survives_storage_and_lookup, tests::disappearance_warning_survives_storage_and_lookup, tests::domain_history_limit_is_recorded_as_failure_with_the_original_state, tests::duplicate_disappearance_preserves_both_boundaries_and_outside_rows, tests::new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, tests::observed_batch_bounds_and_order_are_preserved | 컴파일 성공·검출 |
| 23 | 4096번째 정상 행 제외 | tests::observed_batch_bounds_and_order_are_preserved | 컴파일 성공·검출 |
| 24 | 정규화 행 전체 소실 | tests::cancellation_notice_survives_storage_and_lookup, tests::disappearance_warning_survives_storage_and_lookup, tests::domain_history_limit_is_recorded_as_failure_with_the_original_state, tests::duplicate_disappearance_preserves_both_boundaries_and_outside_rows, tests::new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, tests::observed_batch_bounds_and_order_are_preserved | 컴파일 성공·검출 |
| 25 | 중복 개수 제한 | tests::cancellation_notice_survives_storage_and_lookup, tests::disappearance_warning_survives_storage_and_lookup, tests::duplicate_disappearance_preserves_both_boundaries_and_outside_rows, tests::new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields, tests::observed_batch_bounds_and_order_are_preserved | 컴파일 성공·검출 |
| 26 | 관측 행 반환 첫 원본 제외 | tests::observed_batch_bounds_and_order_are_preserved, tests::raw_rows_reach_repository_with_duplicates_and_partial_quarantine | 컴파일 성공·검출 |
| 27 | 관측 행 반환 마지막 원본 제외 | tests::observed_batch_bounds_and_order_are_preserved, tests::raw_rows_reach_repository_with_duplicates_and_partial_quarantine | 컴파일 성공·검출 |
| 28 | 완전 응답 정규화 행 소실 | tests::cancellation_notice_survives_storage_and_lookup, tests::disappearance_warning_survives_storage_and_lookup, tests::domain_history_limit_is_recorded_as_failure_with_the_original_state, tests::duplicate_disappearance_preserves_both_boundaries_and_outside_rows, tests::new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields | 컴파일 성공·검출 |

## 최초 자동 변이와 과거 환경 예외

just mutants-diff를 실행했다. 첫 실행은 frontendDist 생성 전이라 테스트 목록 컴파일에서 실패했다. 빌드 후 재실행은 승인된 test-diff13건을 거부하여 변이 실행 전에 종료했다. 자동 변이 통과나 선택0개를 주장하지 않는다. 당시 손 변형28개와 자동 명령의 미실행을 구별한다. 이후 보강까지 포함한 구현자 손 변형은 아래 표의33개다. H-47에 승인된 기계적 test-diff와 변경분 변이 전처리의 연계를 인계한다.

정상 커밋 훅은 check-ts의 Xcode 라이선스 R4 테스트1건만 실패했다. 다른 Node 테스트는 실패0이다. 환경2 당시에는 지시문에 따라 그 뒤 check-ts 단계와 check 본문을 순차로 직접 검사했다. 기존 테스트 변경 예외와 환경 예외는 커밋 메시지에 함께 기록한다. 검사 규칙이나 테스트 자체는 변경하지 않는다.

최초 구현 검사 결과: 정상 훅의 Node 테스트는 R4 단1건만 실패했다. 그 뒤 check-ts 단계(CI 회귀88개 포함), fmt·clippy·deny·arch/source/sql·sqlcipher·migration·capability·generated-files·gen-schema·raw-types·disable-check, 전체 nextest1868개, coverage-check(계측1076개), disable-report·hooks-installed가 통과했다. test-diff는 위 승인13건만 실패했다. 환경 예외 사용: 예. 새 테스트를 구현 전에 실행한 동작 red 증거는 없으며, 손 변형 실패와 원복 통과를 별도의 검증으로 남긴다.

## 리뷰 후 보완과 중복 키 실행 관측

1차 독립 비판은 중복 amount 키가 있는 합성 입력을 현재 T-06 경로가 뒤 값으로 정규화하고 원문에는 두 키를 보존하는 반면, 생성 스키마로 직접 역직렬화하면 duplicate field 오류가 남을 재현했다. append_row는 최초 기준과 같았으므로 A-23 제품 회귀가 아니다. 이 내용은 실행 관측이다. 이후 추가했던 수용·뒤 값 우선 단언의 신규 테스트는 2차 리뷰와 비판에서 정책 고정 위험이 확인되어 병합 전에 제거했다. 기존 master 테스트의 단언을 삭제하거나 완화한 변경은 아니다.

혼합 격리 회귀에는 동등 정규화·다른 철자 두 행에 더해 금액이 다른 정상 행과 격리 원문의 재결합 단언을 추가했다. 원본뿐 아니라 각 위치의 정규화 금액도 독립 기대값으로 확인한다. 상한 회귀는 나중 invalid 행이 있는 경로와 정상4097행만 있는 직접 RowLimit 경로를 각각 검사한다. 두 경로에서 꼬리 원문·금액·건수를 확인한다. 제품 코드는 변경하지 않았다.

아래3개는 보강 테스트를 사용한 구현자 임시 사본의 실제 컴파일·실행 결과다. 변형 전3개와 원복 후3개가 모두 통과했다. 앞선28개 및1차 보강2개와 합쳐 구현자 손 변형33개가 컴파일 후 검출됐다. 독립 비판자의 별도 실행 건수와 합산하지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 29 | 격리 혼합 결과에서만 정상 행 원문을 빈 문자열로 변경 | mixed_quarantine_keeps_raw_pairing (1차 보강 시점) | 컴파일 성공·검출 |
| 30 | 두 번째 이후 배치의 원문만 빈 문자열로 변경 | overflow_tail_keeps_raw_pairing (1차 보강 시점) | 컴파일 성공·검출 |
| 31 | 격리 원문을 빈 객체로 손상 | mixed_quarantine_keeps_raw_pairing | 컴파일 성공·검출 |
| 32 | 격리 혼합에서 첫·마지막 정상 행의 정규화 값만 교환 | mixed_quarantine_keeps_raw_pairing | 컴파일 성공·검출 |
| 33 | 직접 RowLimit 분기에서4097번째 원문을 첫 행 원문으로 교체 | clean_row_limit_keeps_raw_tail_pairing | 컴파일 성공·검출 |

## 환경 예외 종료

환경3 안내로 Xcode 라이선스 문제가 해결된 뒤에는 DEVELOPER_DIR 설정과 R4 사유의 커밋 예외를 종료했다. 앞서 환경 예외로 만든 커밋은 이력에 남기며, 이후에는 정상 훅의 전체 검사를 실행한다. 기존에 사전 승인된 기계적 test-diff 예외는 해당 승인 범위에만 적용한다. 서버 재시작으로 중단된 정상 커밋은 HEAD·인덱스·실행 프로세스를 확인한 뒤 다시 실행했다.

## 타입과 포트의 변경

| 대상 | 전 | 후 |
|---|---|---|
| 정상 행 | Transaction만 전달 | ObservedTransaction이 Transaction과 불투명 RawTransactionRow를 소유 |
| `TransactionRead::Complete` | domain CompleteTransactionResponse | application CompleteTransactionObservation |
| `IncompleteTransactions.received` | `Vec<TransactionBatch>` | `Vec<ObservedTransactionBatch>` |
| TransactionSyncRecord::from_observation | `Result<TransactionRead, PortError>` | 서명 유지, 내부 관측에 결합된 원문 포함 |
| TransactionSyncRepository::record_sync | `record: &TransactionSyncRecord` | 서명 유지, 같은 상세 기록으로 원문 전달 |
| domain 대조 | CompleteTransactionResponse | 동일. application의 normalized() 투영으로 연결 |

3차 비판은 헬퍼 이름의 사소한 문제만 확정했다. expected_krw_amount로 명확히 하고 동등한 두 번째 정상 행의 금액도 직접 단언했다. 후자는 이미 거래 전체 동등성으로 보장되는 내용을 읽기 쉽게 보강한 것이다. 제품 코드 변경과 추가 리뷰 회차는 없다. 보호 생성물 두 파일의 변경은 새 ADR 읽기 등록과 해당 설정 해시의 공식 재생성에 한정하며, 구체적인 사전 승인 범위를 벗어나지 않는다.
