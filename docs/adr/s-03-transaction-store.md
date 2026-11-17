# S-03 거래내역 저장소

- 상태: 초안

## 결정

A-23의 ObservedTransaction을 저장 경계로 사용한다. RawTransactionRow의 UTF-8 문자열은 해석하거나 직렬화하지 않고 transaction_sources.source에 그대로 보존한다. 대응하는 정규화 행은 transaction_rows에 별도로 저장한다. 같은 정규화 행이어도 원본 철자와 위치가 다른 관측은 별개 행으로 남는다. 원문과 정규화의 의미 대응은 수신 어댑터의 책임이며 저장소는 그 대응을 새 정책으로 바꾸지 않는다.

새 마이그레이션 0003_transactions.sql만 추가한다. 모든 테이블은 STRICT다. 정규화 금액·수량·비율은 TEXT와 기존 정규 십진수 CHECK를 사용한다. 금액의 통화는 행에 함께 둔다. 부재 값은 NULL이다. 행 위치는 (entry, scope, page, position)으로 유일하며 거래 필드 동등성으로 중복을 제거하지 않는다.

transaction_entries는 계좌별 추가 순서, 상세/기존 요약 구분과 복원 메타데이터를 보존한다. transaction_rows의 before/after 영역은 정상 상태의 이력과 미결제 집합을 구분하고 observed 영역은 받은 페이지별 정상 행을 보존한다. first_seen과 disappeared_at은 domain의 StoredTransaction 생성자로 검증한다. 사라진 행은 지우지 않는다. 모든 정규화 필드를 domain 생성자로 복원한 뒤 보존한 관측을 before 상태에 다시 적용하고 after 상태 및 요약과 비교한다. 행 수·위치·원본 수를 확인하여 누락과 예상 밖 행을 거부한다.

메타데이터는 버전 1의 길이 접두 UTF-8 필드열이다. 태그는 닫힌 문자열이며 숫자·날짜·시각은 domain 생성자에 전달한다. 구분자·공백·한글·NUL을 가진 격리 원문도 길이로 경계를 보존한다. 정상 원문은 이 메타데이터가 아니라 별도 source 열에 저장한다. 새로운 외부 의존성은 없다. RetryDelay::Known을 정확히 보존하기 위해 TimeSpan에 타입이 있는 초·나노초 성분 접근자 as_parts를 추가한다. Debug 문자열은 저장 형식으로 사용하지 않는다.

TransactionStore는 Database를 공유 차용한다. record_sync는 기존 application 서명을 유지하고 IMMEDIATE 쓰기 트랜잭션에서 계좌, 직전 정상 상태, 마지막 상세 관측 시각을 함께 검사한다. 관측 시각 역행과 기존 요약보다 오래된 쓰기도 거부한다. 저장된 이전 상태에 from_observation을 적용한 기록과 입력을 비교하여 다른 이전 상태에서 만들어진 기록을 거부한다. 상세·원본·정규화·상태·요약은 함께 저장되거나 모두 롤백된다. 같은 시각은 허용하고 마지막 추가 순서가 최신이다. 기존 TransactionRepository::record는 요약과 그 정상 상태를 같은 저장 장치에 기록하며 상세 관측을 발명하지 않는다. 실패·접근 차단 요약은 이전 정상 상태를 유지한다.

FullInspectionRepository는 ADR 0110·0117의 별도 계좌별 CAS 계약을 구현한다. record_completed는 전달받은 completed_at을 그대로 보존하며 Clock을 읽지 않는다. record_sync는 전체 완료 시각을 만들거나 바꾸지 않는다. 현재 record_sync 입력에는 전체 완료값이 없으므로 두 호출의 결합 원자성을 주장하지 않는다. 완료 시각 저장 실패 시 기존 완료값을 보존하며 이미 성공한 구간 관측은 남는다.

상태를 불투명 직렬화로 저장하지 않기 위해 각 기록의 before/after 정규화 스냅샷을 보존한다. 구현은 명확한 복원 검증을 우선한다. 기록당 전체 정상 이력을 복제하는 공간·쓰기 비용이 있으므로 긴 실계좌 이력에 대한 성능 검증을 완료했다고 주장하지 않는다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| T-06 후속, S-03 후속 | 객체 내부 중복 JSON 키의 수용·격리·거부와 재정규화 해석 경로를 결정한다. 직접 스키마 파싱과 Value 경유의 차이를 포함한다. 이번 저장소는 전달받은 원문을 그대로 보존하며 원문에서 정규화하는 새 파서나 정책을 추가하지 않는다. |
| A-25, A-22 후속 | 전체 계획의 계좌·창과 모든 구간 저장 성공을 확인하고 completion_at의 성공값에 있는 completed_at을 기존 record_completed에 전달한다. 저장 전 Clock을 다시 읽어 교체하지 않는다. 현재 record_full_inspection_completed 헬퍼의 재표본 경로를 완료값 연결에 사용하지 않는다. |
| S-03 후속, 성능 검증 담당 | 긴 이력의 스냅샷 복제 비용을 측정하고 필요하면 정규화 행 공유와 체크포인트 구조를 별도 설계한다. 기존 원본·순서·중복·사라짐·CAS 계약을 유지한다. |

## 검증

회귀 테스트를 먼저 추가한 최초 실행은 TransactionStore 공개 타입이 없어 컴파일 실패했다. 프로젝트 신뢰·훅 신뢰·현재 세션 적용은 확인 불가다. 시작 frozen 설치와 lefthook 설치는 성공했다. 실제 키·계좌·외부 API는 사용하지 않았다. 검사·변이·리뷰의 최종 실행 결과는 이후 검증 기록에 추가한다.

## 승인된 기존 테스트 변경

기존 단언의 삭제·완화는 없다. test-diff는 기존 테스트 본문 2건을 지적한다. 고정 개수 변경 승인 (c)를 적용한다. 부정 fixture 상수 2개는 새 마이그레이션 번호와 충돌하지 않도록 준비 변경 승인 (d)를 적용한다.

| 파일 | 테스트·준비 항목 | 전 → 후 |
|---|---|---|
| src/migration_tests.rs | fresh_open_records_clock_and_reopen_preserves_ledger | 원장 개수 2 → 3 |
| src/tests.rs | raw_key_is_independently_readable_and_debug_is_closed | sqlite_schema 객체 개수 7 → 12 |
| src/sql/holdings_tests.rs | GAP | 기존 번호 3과 충돌하므로 손상용 번호 3 → 4 |
| src/sql/holdings_tests.rs | FUTURE | 지원 번호가 3으로 늘어 미래 버전 3 → 4 |

보호 파일 변경은 migrations.lock의 0003 해시, schema.snapshot.sql의 새 테이블, 새 ADR 등록에 따른 .codex/config.toml과 harness/claude-hooks/hashes.json의 공식 재생성이다. 규칙 완화와 검사 끄기는 추가하지 않았다.

## 자동 변이

just mutants-diff 14f608305d95e478bda3a81a64ea8f686e9303a8을 한 번 실행했다. 테스트 목록 컴파일 뒤 사전 승인된 test-diff 2건에서 종료 1로 중단됐다. 자동 변이 자체는 미실행이며 통과나 생존 0으로 보고하지 않는다. H-47에 승인된 기계적 test-diff 변경과 변이 전처리의 연계를 인계한다.

## 손 변형

작업 사본 target 아래 mktemp 임시 사본에서 변형마다 원본 소스를 복원한 뒤 실제 컴파일·nextest를 실행했다. 최초 20개는 모두 컴파일됐고 19개 검출·1개 생존이었다. 20번의 정상 전진 저장 회귀를 보강하고 같은 변형을 재실행해 검출했다. 보강 전·후 대조군은 각각 통과했다. 최종적으로 서로 다른 컴파일 가능 변형 20개가 검출됐으며, 최초 생존을 숨기지 않는다. 실행 로그는 target/S-03/manual-run.log와 recheck-20.log에 있다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | 거래 종류 열과 시장 열 혼동 | transaction_roundtrip_raw_duplicates_disappearance_and_reopen | 컴파일·검출 |
| 2 | 시장 열과 거래 종류 열 혼동 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 3 | 종목 대신 자산 종류 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 4 | 자산 종류 대신 ISIN 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 5 | ISIN 대신 종목 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 6 | 수량 대신 단가 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 7 | 단가 대신 총액 복원 | transaction_cas_rejects_stale_state_clock_and_foreign_account | 컴파일·검출 |
| 8 | 총액 대신 수수료 복원 | transaction_corruption_fails_closed | 컴파일·검출 |
| 9 | 수수료 대신 세금 복원 | transaction_write_failure_rolls_back_details_state_and_summary | 컴파일·검출 |
| 10 | 세금 대신 수수료 복원 | transaction_roundtrip_raw_duplicates_disappearance_and_reopen | 컴파일·검출 |
| 11 | 순금액 대신 총액 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 12 | 환율 대신 수량 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 13 | 결제일 대신 결제 상태 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 14 | 거래 시각 대신 취소 상태 복원 | transaction_roundtrip_raw_duplicates_disappearance_and_reopen | 컴파일·검출 |
| 15 | 정상 거래를 취소로 복원 | transaction_roundtrip_raw_duplicates_disappearance_and_reopen | 컴파일·검출 |
| 16 | 결제 거래를 미결제로 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 17 | 원문을 빈 문자열로 저장 | transaction_cas_rejects_stale_state_clock_and_foreign_account | 컴파일·검출 |
| 18 | 정상 상태 CAS 비교 반전 | transaction_roundtrip_raw_duplicates_disappearance_and_reopen | 컴파일·검출 |
| 19 | 완료 시각 역행 거부 반전 | full_completion_cas_preserves_supplied_timestamp_and_accounts | 컴파일·검출 |
| 20 | 새 완료값 대신 이전 완료값 저장 | completion_advances_and_transaction_sync_keeps_completion_unchanged | 최초 생존 → 회귀 보강 후 컴파일·검출 |

## 최종 소스 손 변형 재실행

glob import를 명시적 import로 정리한 최종 제품 소스와 보강한 테스트를 새 mktemp 사본에 담아 20개 전체를 다시 컴파일·실행했다. 원본 대조군 전·후 모두 통과했고 20개 모두 컴파일 후 검출됐다. 생존·컴파일 불가·시간 초과는 각각 0개다. 최초 실행의 20번 생존 및 회귀 보강 기록은 위에 그대로 둔다. 최종 실행 로그는 target/S-03/manual-run-final.log다.

저장소 전체 nextest는 105개 통과했다. clippy와 source-check·sql-check·migration-check·raw-types·arch-check도 통과했다. 전체 변경분 검사는 별도로 기록하며 이 좁은 검사의 통과로 대신하지 않는다.

## 구현자 자체 읽기 검증 보강

최초 구현 커밋 399654b45d667dae17f1dc67bd615308e5892353의 정상 훅은 전체 nextest 1973개와 커버리지 계측 1103개, coverage-check를 통과했다. 실패는 위 사전 승인된 test-diff 2건뿐이다. 남은 disable-report·disable-report-since·hooks-installed도 직접 실행해 통과한 뒤 그 커밋에 한정한 승인 예외를 사용했다. 환경 예외는 사용하지 않았다. 최초 수동 검사에서 소스 편집과 겹쳐 발생한 H-47 목록 비교 실패는 소스 동결 후 정상 훅에서 해소됐다.

그 뒤 별도 사본에서 정상 상태 시각보다 과거인 실패 관측 시각을 읽는 결함을 재현했다. 작업 브랜치에도 실패 상세와 기존 Complete 요약의 두 회귀를 먼저 추가해 2개 실패를 확인했다. before/after 정상 상태의 마지막 동기화 시각보다 관측 시각이 과거이면 읽기도 거부한다. 쓰기에서 이미 강제한 시각 관계를 읽기에도 적용하며 별도 날짜 정책을 추가하지 않는다. 가드 추가 후 저장소 107개 테스트가 통과했다. 함수 길이 제한은 검증 함수를 분리해 지키며 검사 끄기는 추가하지 않았다. 이 수정은 구현 후 첫 독립 리뷰 전에 반영한다.
