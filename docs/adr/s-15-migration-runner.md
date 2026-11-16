# S-15 마이그레이션 실행기와 원장

- 상태: 초안

## 결정

DB 열기는 Clock을 받아 쓰기 스레드 초기화가 끝난 뒤에만 성공한다. 쓰기 스레드의 IMMEDIATE 트랜잭션 하나에서 내장 목록과 기존 원장의 번호·파일 이름·SHA-256을 비교하고 미적용 파일을 적용한다. 전체 열기 묶음이 원자적이다. 마이그레이션 SQL 실행 중 SQLite authorizer로 트랜잭션·savepoint 제어를 거부한다. 실행기 소유 BEGIN·COMMIT·실패 시 ROLLBACK은 허용한다. SQL 실행 구간의 원자적 플래그를 콜백과 공유하고 오류 뒤 반드시 해제한다. source의 COMMIT 뒤 실패할 때 partial 테이블이 남는 회귀를 먼저 재현한 뒤 이 경계를 추가했다. 실패하면 앞서 적용한 미적용 파일도 함께 되돌린다. user_version과 원장 길이도 대조해 꼬리 삭제·원장 삭제와 미래 버전을 닫는다. 파일과 원장을 함께 되돌리는 공격의 방지는 이 원장만으로 보장하지 않는다.

파일은 migrations/0001_migration_ledger.sql 규칙을 따른다. sql-check가 include_str 매크로도 금지하므로 SQL 상수에 동일 리터럴을 내장하고 테스트가 실제 파일 바이트와의 일치를 강제한다. migrations.lock은 실제 파일의 SHA-256으로 등록한다. SHA-256은 이미 잠긴 sha2 =0.10.9, default-features=false를 재사용한다. 새 외부 패키지는 없다. domain 내부 의존을 추가해 Clock 반환 Timestamp의 저장 경계 변환만 한다.

원장에는 number INTEGER PRIMARY KEY CHECK(number > 0), name TEXT NOT NULL UNIQUE, sha256 TEXT NOT NULL(64자리 소문자 16진수 CHECK), applied_at INTEGER NOT NULL(Clock의 UTC 초)을 둔다. 테이블은 STRICT다.

스키마는 적용 뒤 실제 sqlite_schema와 pragma_table_list를 읽어 검사한다. 금액 이름은 하네스 이름 조각 규칙을 따른다. 금액 열은 TEXT, currency TEXT NOT NULL, 명시한 정규 십진수 CHECK 식을 요구한다. SQL 파서를 새로 추가하지 않기 위해 지원 DDL을 보수적으로 한정한다. 주석·인용 식별자와 다른 표현의 동등 CHECK는 거부한다. 허용 식은 sql/tests.rs의 GOOD_MONEY 고정 자료와 schema.rs에 있다. NULL 허용 여부는 각 저장소의 모델이 정한다. 금액 산술은 하지 않는다. S-20에서 SQL 파서 기반 검사로 통합한다.

schema.snapshot.sql은 빈 DB 적용 결과다. 테스트는 파일을 읽기만 한다. 명시적 갱신 명령은 현재 단일 마이그레이션에서 `cp crates/adapters/store-sqlite/migrations/0001_migration_ledger.sql crates/adapters/store-sqlite/schema.snapshot.sql`이다. 다음 마이그레이션부터는 적용 후 스키마 추출 명령도 함께 추가한다.

close(self)는 DB 마지막 소유권을 spawn_blocking으로 옮겨 수락한 쓰기를 끝내고 join한다. 쓰기 스레드 패닉·요청 채널 단절은 RepositoryIntegrity로 돌려준다. Drop의 동기 안전망은 유지한다. SQL 실행 API는 crate 내부로 유지한다.

## 승인된 변경

기존 Database::open 호출에 가짜 Clock 인자만 추가하고 기존 단언을 유지한다. 기존 schema count 고정 기대값은 원장과 고유 인덱스로 0에서 2로 갱신한다. S-01 테스트 samples의 문자열 value 열은 금액 이름 규칙에 걸리므로 payload로 바꾼다. 데이터·기대값은 그대로다. 준비 부분 변경 승인 (d)와 정확한 개수 변경 승인 (c)를 적용한다. 생성물은 저장소 스크립트로 재생성한다.

## 검증

target/S-15에 red/green·검사 로그를 보관한다. 프로젝트·세션 훅 신뢰는 미확인이다. Git 훅은 시작 절차에서 설치했다. 독립 리뷰와 변이 결과는 검증이 끝난 뒤 기록한다.

## 인계

- U-01: 사이드카 검사와 실제 열기 사이 경합은 남아 있다. 실제 앱 데이터 폴더 준비, 마지막 Database 소유자의 close 호출 위치와 실제 UI 응답성 검증. close를 쓰지 않는 Drop은 여전히 동기 join이다.
- S-02 이후: 새 파일을 순서대로 등록하고 해당 저장소 테이블·통화·CHECK와 스냅샷을 추가한다.
- S-08: 마이그레이션 직후 authorizer는 해제된다. 감사 로그 정책은 그 뒤 연결에 설치한다.
- S-20: 금액 이름·DDL 검사와 SQL 상수의 파일 내장을 파서로 통합하고 스냅샷 갱신 도구를 확장한다.
- S-16: 원장 전체 되돌리기 방지와 감사 로그의 외부 머리값 검증은 별도다.

## 검사 기록

- 최종 로컬 nextest: 42/42 통과. clippy와 arch-check/source-check/sql-check/migration-check/raw-types/disable-check 통과.
- 최초 회귀 테스트는 실행기와 새 open 서명이 없어서 컴파일 실패한 뒤 구현했다.
- 첫 check-fast는 신규 스냅샷을 추적하기 전 만든 보호 경로 생성물 때문에 실패했다. 추적 후 정상 생성기로 다시 만들었다.
- 두 번째 check-fast의 H-47 원본 변이 목록 비교 중 소스가 바뀌어 실패했다. 최종 검사는 소스를 동결한 상태로 다시 수행한다.
- 자동 `just mutants-diff e86f6fb12d35afda07149a103b9c94b9493ddd16`는 승인된 기존 테스트 변경 때문에 test-diff에서 거부됐다. 변이를 실행했다고 세지 않는다. 검사기는 변경하지 않는다.
- sha2 =0.10.9(default-features=false, MIT OR Apache-2.0)와 기존 domain 내부 의존만 추가했다. sha2 전이 의존은 모두 기존 잠금 항목이다. deny의 기존 sha2 등록을 재사용하고 arch-check 통과를 확인했다.
- 보호 파일 변경은 migrations.lock(새 파일 해시 등록), schema.snapshot.sql(새 스냅샷), .codex/config.toml·harness/sandbox/lock-fragment.json·harness/claude-hooks/hashes.json(새 스냅샷 보호 경로와 정상 생성물)이다. 기존 규칙은 완화하지 않았다.

추가 보호 경계 준비 변경: harness/sqlcipher/debug.rs의 기존 두 open 호출에 FixedClock만 주입하고 Debug·오류 단언은 그대로 유지했다. harness/sqlcipher/check.mjs에는 이 가짜 시계를 컴파일하는 domain rlib 인자만 등록했다. 기존 서명 변경 승인 (b)·준비 부분 승인 (d)에 해당한다.

## 승인된 test-diff 세부

검사 결과 23건: 테스트 20개와 준비 도우미 3개다. 아래 변경은 준비 부분 승인 (d), open 서명 변경 승인 (b), 정확한 개수 변경 승인 (c)에 해당한다. 기존 단언 삭제·완화는 없다. sql/tests.rs의 CREATE_TEST·INSERT_TEST·VALUE_TEST는 value 문자열 열을 payload로 바꿔 기존 가짜 문자열 자료를 보존한다.

| 파일                       | 테스트·도우미                                                                                    | 전 → 후                                                                                                       |
| -------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| src/tests.rs               | `key`                                                                                            | 비공개 가짜 자료 도우미 → crate 내 신규 회귀에서 재사용하는 준비 도우미(본문 그대로)                          |
| src/tests.rs               | `runtime`                                                                                        | 비공개 가짜 자료 도우미 → crate 내 신규 회귀에서 재사용하는 준비 도우미(본문 그대로)                          |
| src/tests.rs               | `tests::cancelling_a_response_does_not_cancel_an_accepted_write`                                 | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::concurrent_producers_keep_fifo_and_use_one_writer_thread`                                | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::database_secret_and_failure_debug_remain_fixed`                                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::encrypted_reopen_and_pragma_contract`                                                    | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::encrypts_database_and_live_wal_and_rolls_back`                                           | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::one_read_callback_observes_one_snapshot_across_a_writer_commit`                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `assert_sidecar_directory_is_integrity`                                                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::a_directory_or_non_directory_parent_is_a_storage_layout_failure`          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::file_preparation_is_owner_only_and_preserves_existing_encrypted_data`     | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::invalid_key_length_creates_no_database_file`                              | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::readonly_database_file_is_rejected_before_a_successful_health_result`     | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::readonly_parent_rejects_new_and_existing_database_with_permission_action` | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests/open_contract.rs | `tests::open_contract::regular_sidecar_files_preserve_a_live_encrypted_database`                 | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::permission_failures_require_permission_action`                                           | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::queued_requests_are_released_when_worker_exits`                                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::raw_key_is_independently_readable_and_debug_is_closed`                                   | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로; sqlite_schema 정확한 개수 0 → 2(원장+고유 인덱스) |
| src/tests.rs               | `tests::rejects_wrong_key_plaintext_and_invalid_length`                                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::sqlite_failures_preserve_permission_availability_and_integrity`                          | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::worker_panic_closes_current_queued_and_future_requests`                                  | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/tests.rs               | `tests::writer_connection_applies_and_verifies_the_complete_pragma_contract`                     | open(path, key) → open(path, key, 가짜 Clock); 단언 그대로                                                    |
| src/worker.rs              | `worker::tests::dropping_writer_disconnects_retained_request_endpoint`                           | 가짜 Writer::open(path,key) → open_migrated(path,key,가짜 시각); 종료 단언 그대로                             |

## 손 변형

최종 종료 API 보강까지 포함한 사본에서 전후 대조군 각각 42개 테스트 통과. 각 변형 전에 store-sqlite를 정리하고 다시 컴파일했다. 컴파일 가능한 28개 전부 테스트 실패로 검출됐고 생존·컴파일 불가·시간 초과는 0이다. 자동 변이의 통과로 대신 표시하지 않는다.

| 번호 | 파일         | 바꾼 것                                                                                                       | 잡은 테스트                                                              | 결과 |
| ---- | ------------ | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ---- |
| 1    | migration.rs | `byte >> 4_u8 -> byte >> 3_u8`                                                                                | `migration_tests::sha256_matches_independent_known_vectors`              | 검출 |
| 2    | migration.rs | `byte & 15 -> byte & 7`                                                                                       | `migration_tests::sha256_matches_independent_known_vectors`              | 검출 |
| 3    | migration.rs | `version > migrations.len() -> version < migrations.len()`                                                    | `migration_tests::connection_sql_constants_prepare_on_migrated_database` | 검출 |
| 4    | migration.rs | `before == 0 && version != 0 -> before == 0 && version == 0`                                                  | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`    | 검출 |
| 5    | migration.rs | `if before == 0 { -> if before != 0 {`                                                                        | `migration_tests::connection_sql_constants_prepare_on_migrated_database` | 검출 |
| 6    | migration.rs | `rows.len() != version -> rows.len() == version`                                                              | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 7    | migration.rs | `before != 0 && rows.is_empty() -> before == 0 && rows.is_empty()`                                            | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 8    | migration.rs | `if index >= rows.len() -> if index > rows.len()`                                                             | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 9    | migration.rs | `if index != 0 { -> if index == 0 {`                                                                          | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 10   | migration.rs | `digest(source), -> digest(name),`                                                                            | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 11   | migration.rs | `at.utc_seconds().as_i64() -> at.utc_seconds().as_i64().checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?` | `migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger`  | 검출 |
| 12   | migration.rs | `transaction.commit() -> transaction.rollback()`                                                              | `migration_tests::embedded_migration_and_saved_schema_match_files`       | 검출 |
| 13   | migration.rs | `*strict != 1 -> *strict == 1`                                                                                | `migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger`  | 검출 |
| 14   | migration.rs | `row.get::<_, i64>(1) -> row.get::<_, i64>(0)`                                                                | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`    | 검출 |
| 15   | migration.rs | `name != expected_name -> name == expected_name`                                                              | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 16   | migration.rs | `*hash != digest(source) -> *hash == digest(source)`                                                          | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 17   | schema.rs    | `words.get(1) != Some(&"text") -> words.get(1) == Some(&"text")`                                              | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 18   | schema.rs    | `&#124;&#124; !currency -> &#124;&#124; currency`                                                             | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 19   | schema.rs    | `!compact.ends_with(&expected) -> compact.ends_with(&expected)`                                               | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 20   | schema.rs    | `if money.is_empty() -> if !money.is_empty()`                                                                 | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 21   | schema.rs    | `symbol.is_uppercase() -> symbol.is_lowercase()`                                                              | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 22   | schema.rs    | `depth == 0 -> depth == 1`                                                                                    | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 23   | schema.rs    | `if symbol == '\'' -> if symbol == 'x'`                                                                       | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 24   | schema.rs    | `if !quoted -> if quoted`                                                                                     | `schema::tests::decimal_schema_requires_type_currency_and_exact_check`   | 검출 |
| 25   | migration.rs | `Authorization::Deny -> Authorization::Allow`                                                                 | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 26   | migration.rs | `guarded.store(true, Ordering::Relaxed) -> guarded.store(false, Ordering::Relaxed)`                           | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 27   | migration.rs | `guarded.store(false, Ordering::Relaxed) -> guarded.store(true, Ordering::Relaxed)`                           | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
| 28   | migration.rs | `if guarded.load(Ordering::Relaxed) { -> if !guarded.load(Ordering::Relaxed) {`                               | `migration_tests::migration_cannot_escape_transaction`                   | 검출 |
