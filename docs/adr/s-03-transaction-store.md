# S-03 거래내역 저장소

- 상태: 초안

## 결정

A-23의 ObservedTransaction을 저장 경계로 사용한다. RawTransactionRow의 UTF-8 문자열은 해석하거나 직렬화하지 않고 transaction_sources.source에 그대로 보존한다. 대응하는 정규화 행은 transaction_rows에 별도로 저장한다. 같은 정규화 행이어도 원본 철자와 위치가 다른 관측은 별개 행으로 남는다. 원문과 정규화의 의미 대응은 수신 어댑터의 책임이며 저장소는 그 대응을 새 정책으로 바꾸지 않는다.

새 마이그레이션 0003_transactions.sql만 추가한다. 모든 테이블은 STRICT다. 정규화 금액·수량·비율은 TEXT와 기존 정규 십진수 CHECK를 사용한다. 금액의 통화는 행에 함께 둔다. 부재 값은 NULL이다. 행 위치는 (entry, scope, page, position)으로 유일하며 거래 필드 동등성으로 중복을 제거하지 않는다.

transaction_entries는 계좌별 추가 순서, 상세/기존 요약 구분과 복원 메타데이터를 보존한다. transaction_rows의 before/after 영역은 정상 상태의 이력과 미결제 집합을 구분하고 observed 영역은 받은 페이지별 정상 행을 보존한다. first_seen과 disappeared_at은 domain의 StoredTransaction 생성자로 검증한다. 사라진 행은 지우지 않는다. 모든 정규화 필드를 domain 생성자로 복원한 뒤 보존한 관측을 before 상태에 다시 적용하고 after 상태 및 요약과 비교한다. 행 수·위치·원본 수를 확인하여 누락과 예상 밖 행을 거부한다.

메타데이터는 버전 1의 길이 접두 UTF-8 필드열이다. 태그는 닫힌 문자열이며 숫자·날짜·시각은 domain 생성자에 전달한다. 구분자·공백·한글·NUL을 가진 격리 원문도 길이로 경계를 보존한다. 정상 원문은 이 메타데이터가 아니라 별도 source 열에 저장한다. 새로운 외부 의존성은 없다. RetryDelay::Known을 정확히 보존하기 위해 TimeSpan에 타입이 있는 초·나노초 성분 접근자 as_parts를 추가한다. Debug 문자열은 저장 형식으로 사용하지 않는다.

TransactionStore는 Database를 공유 차용한다. record_sync는 기존 application 서명을 유지하고 IMMEDIATE 쓰기 트랜잭션에서 계좌, 직전 정상 상태, 마지막 상세 관측 시각을 함께 검사한다. 관측 시각 역행과 기존 요약보다 오래된 쓰기도 거부한다. 저장된 이전 상태에 from_observation을 적용한 기록과 입력을 비교하여 다른 이전 상태에서 만들어진 기록을 거부한다. 상세·원본·정규화·상태·요약은 함께 저장되거나 모두 롤백된다. 같은 시각은 허용하고 마지막 추가 순서가 최신이다. 기존 TransactionRepository::record는 요약과 그 정상 상태를 같은 저장 장치에 기록하며 상세 관측을 발명하지 않는다. 실패·접근 차단 요약은 이전 정상 상태를 유지한다. legacy Partial은 이전 정상 상태와 같은 값만 허용하며 다른 상태를 전달하면 요약 추가 없이 거부한다. 읽을 때도 Partial의 before/after가 다르면 거부한다.

FullInspectionRepository는 ADR 0110·0117의 별도 계좌별 CAS 계약을 구현한다. record_completed는 전달받은 completed_at을 그대로 보존하며 Clock을 읽지 않는다. record_sync는 전체 완료 시각을 만들거나 바꾸지 않는다. 현재 record_sync 입력에는 전체 완료값이 없으므로 두 호출의 결합 원자성을 주장하지 않는다. 완료 시각 저장 실패 시 기존 완료값을 보존하며 이미 성공한 구간 관측은 남는다.

상태를 불투명 직렬화로 저장하지 않기 위해 각 기록의 before/after 정규화 스냅샷을 보존한다. 구현은 명확한 복원 검증을 우선한다. 기록당 전체 정상 이력을 복제하는 공간·쓰기 비용이 있으므로 긴 실계좌 이력에 대한 성능 검증을 완료했다고 주장하지 않는다.

## 인계

ADR 0091 범용 포트의 오래된 record 성공 계약은 record_sync의 CAS와 구분한다. 과거 요약도 버리지 않고 mode=history로 기록하며 현재 요약과 정상 상태 선택에서는 제외한다. 입력에 Complete/Partial 상태가 있으면 그 과거 상태를 자체 스냅샷으로 보존하고, Failed/AccessBlocked에는 당시 정상 상태를 추측하지 않고 빈 상태를 함께 둔다. history의 before/after는 현재 상태 전이가 아니라 입력 복원용 동일 스냅샷이다. 모든 입력은 계좌와 자체 시각을 검증한다. 최신 요약으로 채택되는 Partial의 이전 상태 동일성 계약은 유지한다. 저장 후 검증은 최신 항목 대신 방금 삽입한 ID를 직접 복원하므로 과거 입력도 검증되고 실패하면 롤백된다. 이 분류는 내부 저장 형식이며 화면에 노출하지 않는다.

내부 testkit-ports 개발 의존성으로 공개 transaction_latest 시나리오를 SQLite에 직접 적용했다. 수정 전 오래된 두 번째 쓰기가 실패했고 수정 후 통과했다. 별도 회귀는 네 결과의 과거 요약4개와 상태행4개 보존, 최신 상세/요약/정상 상태 유지, 과거의 다른 계좌 거부, 오래된 상세 동기화 거부, 과거 저장 중 손상 시 롤백을 확인한다. 외부 패키지와 새 검사 끄기는 추가하지 않는다.

| 받는 작업 | 내용 |
|---|---|
| T-06 후속, S-03 후속 | 객체 내부 중복 JSON 키의 수용·격리·거부와 재정규화 해석 경로를 결정한다. 직접 스키마 파싱과 Value 경유의 차이를 포함한다. 이번 저장소는 전달받은 원문을 그대로 보존하며 원문에서 정규화하는 새 파서나 정책을 추가하지 않는다. |
| U-08, A-22 후속 | A-25(ADR 0120)가 전체 계획과 완료값을 연결한다. 실제 SQLite 조립·계좌별 직렬화와 CAS 충돌을 검증한다. 완료값의 completed_at을 기존 record_completed에 그대로 전달하고 재표본 헬퍼로 교체하지 않는다. |
| S-03 후속, 성능 검증 담당 | 긴 이력의 스냅샷 복제 비용을 측정하고 필요하면 정규화 행 공유와 체크포인트 구조를 별도 설계한다. 기존 원본·순서·중복·사라짐·CAS 계약을 유지한다. |

## 검증

회귀 테스트를 먼저 추가한 최초 실행은 TransactionStore 공개 타입이 없어 컴파일 실패했다. 프로젝트 신뢰·훅 신뢰·현재 세션 적용은 확인 불가다. 시작 frozen 설치와 lefthook 설치는 성공했다. 실제 키·계좌·외부 API는 사용하지 않았다. 검사·변이·리뷰의 최종 실행 결과는 이후 검증 기록에 추가한다.

## 승인된 기존 테스트 변경

기존 단언의 삭제·완화는 없다. test-diff는 기존 테스트 본문 2건을 지적한다. 고정 개수 변경 승인 (c)를 적용한다. 부정 fixture 상수 2개는 새 마이그레이션 번호와 충돌하지 않도록 준비 변경 승인 (d)를 적용한다.

| 파일 | 테스트·준비 항목 | 전 → 후 |
|---|---|---|
| src/migration_tests.rs | fresh_open_records_clock_and_reopen_preserves_ledger | 원장 개수 2 → 3 |
| src/tests.rs | raw_key_is_independently_readable_and_debug_is_closed | sqlite_schema 객체 개수 7 → 13 |
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

## 1차 리뷰 평가와 저장 비용 보강

새 Opus 5.5 리뷰는 정상 상태 CAS 조건만 삭제하는 변형의 테스트 공백과 반복 적재/저장 비용을 중요 추정으로 제기했다. 새 Codex 비판은 비교 조건 삭제 변형이 기존 107개를 통과하고 교차 legacy Complete 회귀에서는 검출됨을 실행으로 확인했다. 현행 CAS 자체는 반례를 거부하므로 사소한 테스트 공백으로 평가하고 해당 회귀를 추가한다. legacy Partial이 정상 상태를 교체하는 실제 계약 결함도 두 회귀에서 먼저 실패시킨 뒤 수정했다. 원문 재파싱 부재는 최신 사용자 지시에 맞고, 유스케이스 직접 연결 테스트 부재는 시나리오 동등성 완료 기준 위반으로 보지 않는다. 제품 연결은 별도 미검증이다.

최신 정상 상태/요약/상세 시각을 한 번 읽은 결과에서 함께 취득한다. 최신 항목이 상세 기록이면 다시 같은 상세를 읽지 않는다. 일반적인 상세 기록 뒤의 record_sync는 저장 후 검증을 포함해 전체 적재 2회이며, 최신 항목이 legacy 요약일 때만 기존 상세를 한 번 더 읽는다. 정규화 행과 원문을 LEFT JOIN으로 함께 읽어 행별 원문 존재 확인과 조회를 제거한다. 새 0003에 account/id 색인을 추가해 계좌별 최신 조회를 지원한다. 이전/이후 상태의 전체 스냅샷을 저장하는 2H 삽입은 남는다.

비판자의 보강 전 단일 debug 측정은 H=2000, R=0에서 적재5회·복원20000행·행별 원본 확인20000회·삽입4000행·1851ms였다. 계측과 빌드 조건이 있는 표본이므로 제품 성능 수치로 사용하지 않는다. 개설일 미상 계획 739937일을 모든 구간별 저장에 연결하면 저장 비용이 커지는 조건부 계산은 사실이지만 현재 실행 연결은 A-25이며 UI 정지 또는 대기 CPU/메모리 위반을 재현한 것은 아니다. 전체 스냅샷의 공간 비용과 실제 연결 후 성능은 S-03 후속·A-25가 측정한다. 이번 보강이 전체 스냅샷 비용을 없앴다고 주장하지 않는다.

## 3차 판정과 통합 경계

3차 비판은 외부 추가 트리거로 형식상 유효한 다른 최신 요약이 삽입되면 Id 검증과 Latest 검증의 결과가 달라짐을 확인했다. 정상 스키마와 공개 포트의 제품 결함으로는 기각한다. 도메인 생성자 검증이 외부에서 만들어 넣은 모든 유효한 대체 자료까지 탐지한다는 보장은 하지 않는다. history 사전 시각 검사 삭제는 후단 읽기 검증이 대신 막아 생존했으며 단독 검출로 세지 않는다. 앞 회차 Partial 쓰기 단독 복원 생존도 같은 이유로 분리해 기록한다.

legacy 동률 분기 >를 >=로 바꾼 변형이 기존 회귀를 통과한 사소한 공백은 같은 시각23의 Failed→AccessBlocked 회귀로 보강한다. 대상5afa0f8b 이후 저장 제품 코드는 바꾸지 않는다. 최대3회차를 완료하고 4차 리뷰를 추가하지 않는다. master d2d12e38bfa556418c53fc7c1bb8b99100b083a0의 A-25·A-12·D-48을 통합하며 포트 서명과 실제 저장 경계는 유지한다. A-25 실행기 추가와 SQLite 조립·실제 API 실행 검증을 구분한다.

구현자 보강 후 H=2000/R=0의 단일 계측 표본은 적재2회·복원8000행·삽입4000행·원문 추가0행·3288ms였다. 이전 비판자의1851ms와 실행 조건·부하가 달라 지연 개선을 주장하지 않는다. 반복 적재와 행별 원문 점 조회는 줄었고 상태 복제 비용은 남는다. 공개 TransactionStore 생성 컴파일은 통과하고 외부 Database::write와 store.database 접근은 각각 E0624·E0616으로 거부되는 자체 경계 실험도 실행했다. 이것을 리뷰어의 NOT_RUN인 타입 우회 실험으로 바꿔 보고하지 않는다.

## 통합 소스의 최종 손 변형 27개

master d2d12e38 통합 및 동률 회귀를 포함한 새 mktemp 사본에서 27개 전부 실제 컴파일·실행했다. 전후 원본 대조군은 통과했고 선정한27개는 모두 검출됐다. 제품 코드·마이그레이션·저장소 테스트의 원본 사본과 현재 파일은 바이트 일치한다. 이는 별도 비판자가 확인한 중복 사전검사 삭제 생존2건을 검출로 세거나 숨기는 주장이 아니다. 그 생존은 반환값·커밋 상태의 중복 방어로 별도 기록한다. 자동 mutants-diff는 앞서 승인 test-diff2건으로 전처리 중단된 NOT_RUN 상태 그대로다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | 거래 종류 열과 시장 열 혼동 | state_only_cas_rejects_legacy_complete_interleaving | 컴파일·검출 |
| 2 | 시장 열과 거래 종류 열 혼동 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 3 | 종목 대신 자산 종류 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 4 | 자산 종류 대신 ISIN 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 5 | ISIN 대신 종목 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 6 | 수량 대신 단가 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 7 | 단가 대신 총액 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 8 | 총액 대신 수수료 복원 | legacy_summary_timestamp_before_preserved_state_fails_closed | 컴파일·검출 |
| 9 | 수수료 대신 세금 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 10 | 세금 대신 수수료 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 11 | 순금액 대신 총액 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 12 | 환율 대신 수량 복원 | normalized_fields_pending_unknown_codes_and_legacy_summary_roundtrip | 컴파일·검출 |
| 13 | 결제일 대신 결제 상태 복원 | historical_legacy_summaries_preserve_inputs_without_replacing_latest_state | 컴파일·검출 |
| 14 | 거래 시각 대신 취소 상태 복원 | state_only_cas_rejects_legacy_complete_interleaving | 컴파일·검출 |
| 15 | 정상 거래를 취소로 복원 | failed_record_timestamp_before_preserved_state_fails_closed | 컴파일·검출 |
| 16 | 결제 거래를 미결제로 복원 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 17 | 원문을 빈 문자열로 저장 | historical_legacy_summaries_preserve_inputs_without_replacing_latest_state | 컴파일·검출 |
| 18 | 정상 상태 CAS 비교 반전 | failed_record_timestamp_before_preserved_state_fails_closed | 컴파일·검출 |
| 19 | 완료 시각 역행 거부 반전 | completion_advances_and_transaction_sync_keeps_completion_unchanged | 컴파일·검출 |
| 20 | 새 완료값 대신 이전 완료값 저장 | completion_advances_and_transaction_sync_keeps_completion_unchanged | 컴파일·검출 |
| 21 | 읽기 시각 검증 OR를 AND로 변경 | legacy_summary_timestamp_before_preserved_state_fails_closed | 컴파일·검출 |
| 22 | 읽기 직전 상태 시각 검증 방향 반전 | state_only_cas_rejects_legacy_complete_interleaving | 컴파일·검출 |
| 23 | 읽기 정상 동시각까지 거부 | partial_pages_quarantine_and_port_errors_roundtrip_without_erasing_state | 컴파일·검출 |
| 24 | 정상 상태 CAS 조건만 삭제 | state_only_cas_rejects_legacy_complete_interleaving | 컴파일·검출 |
| 25 | Partial 읽기 정상 상태 보존 검사 삭제 | changed_state_partial_summary_fails_closed_on_read | 컴파일·검출 |
| 26 | 동시각 legacy 요약을 과거 이력으로 분류 | legacy_equal_timestamp_uses_last_saved_summary | 컴파일·검출 |
| 27 | 최신 조회에서 과거 이력 제외 삭제 | generic_transaction_latest_accepts_out_of_order_summaries | 컴파일·검출 |

실행 로그는 target/S-03/manual-merged-final.log이며 사본 결과와 source.tar 경로는 manual-path.txt에 보존했다. 타입 경계·비용 계측의 이전 대상과 이번 최종 소스 증거는 구분한다.

## A-24 추가 통합

d2d12e38 통합 트리의 정상 훅은 전체2019개·계측1115개와 coverage를 통과했다. 검사 도중 master가 A-24를 포함한 c22117c94038c27d252fdf220d88919605afde54로 전진해, 마지막 test-diff가 이전 MERGE_HEAD를 유효한 비교 기준으로 인정하지 않고 중단됐다. 이를 승인 test-diff2건이나 환경 예외로 넘기지 않았다. S-03 회귀·문서를 보존하고 미완료 병합을 정리한 뒤 새 master를 --no-ff로 다시 받는다. 저장소 제품 코드·마이그레이션·테스트37개는 손 변형 실행 사본과 바이트 일치하며, 추가된 application 평가 진입점과 함께 최종 통합 전체 검사를 다시 수행한다. 실제 결과는 최종 커밋 메시지와 보고에 기록한다.
