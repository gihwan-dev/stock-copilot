# ADR 0098: S-15 마이그레이션 실행기와 원장

- 상태: 확정 (2026-11-16)


## 결정

DB 열기는 Clock을 받아 쓰기 스레드 초기화가 끝난 뒤에만 성공한다. 쓰기 스레드의 IMMEDIATE 트랜잭션 하나에서 내장 목록과 기존 원장의 번호·파일 이름·SHA-256을 비교하고 미적용 파일을 적용한다. 전체 열기 묶음이 원자적이다. 마이그레이션 SQL 실행 중 SQLite authorizer로 트랜잭션·savepoint 제어를 거부한다. 실행기 소유 BEGIN·COMMIT·실패 시 ROLLBACK은 허용한다. SQL 실행 구간의 원자적 플래그를 콜백과 공유하고 오류 뒤 반드시 해제한다. source의 COMMIT 뒤 실패할 때 partial 테이블이 남는 회귀를 먼저 재현한 뒤 이 경계를 추가했다. 실패하면 앞서 적용한 미적용 파일도 함께 되돌린다. user_version과 원장 길이도 대조해 꼬리 삭제·원장 삭제와 미래 버전을 닫는다. 파일과 원장을 함께 되돌리는 공격의 방지는 이 원장만으로 보장하지 않는다.

파일은 migrations/0001_migration_ledger.sql 규칙을 따른다. sql-check가 include_str 매크로도 금지하므로 SQL 상수에 동일 리터럴을 내장한다. 실행기가 쓰는 단일 MIGRATIONS 목록과 migrations 디렉터리의 모든 이름·파일 바이트를 테스트에서 일대일 대조해 미내장 파일과 누락 파일을 검출한다. migrations.lock은 실제 파일의 SHA-256으로 등록한다. SHA-256은 이미 잠긴 sha2 =0.10.9, default-features=false를 재사용한다. 새 외부 패키지는 없다. domain 내부 의존을 추가해 Clock 반환 Timestamp의 저장 경계 변환만 한다.

원장에는 number INTEGER PRIMARY KEY CHECK(number > 0), name TEXT NOT NULL UNIQUE, sha256 TEXT NOT NULL(64자리 소문자 16진수 CHECK), applied_at INTEGER NOT NULL(Clock의 UTC 초)을 둔다. 테이블은 STRICT다.

스키마는 적용 뒤 실제 sqlite_schema와 pragma_table_list를 읽어 검사한다. 전부 STRICT 규칙은 유지하며 가상·그림자 테이블 예외는 만들지 않는다. 하네스 이름 조각 목록은 금액·수량·비율의 산술 금지 대상 집합이다. 이 목록과 런타임 분류 목록의 동일성을 테스트한다. 이름의 마지막 숫자 단위 조각으로 통화 필요 여부를 정한다. probability·qty·quantity·shares·rate·ratio·pct·percent·weight·threshold는 수량·비율 단위이고 나머지는 금액 단위다. 따라서 tax_rate는 통화가 필요 없지만 tax_rate_amount는 필요하다. 모두 TEXT와 정규 십진수 CHECK가 필요하고 금액 단위에만 currency TEXT NOT NULL을 요구한다.

CamelCase 분할은 분류에만 쓰고 CHECK는 실제 열 이름을 SQLite의 ASCII 대소문자 규칙으로만 대조한다. unitPrice와 unit_price를 합치지 않는다. GLOB의 NUL 종료를 보완하는 instr(열,char(0))=0 조건도 CHECK에 강제한다. SQL 파서를 새로 추가하지 않기 위해 지원 DDL을 보수적으로 한정한다. 본문의 큰따옴표·백틱·대괄호 인용 구문과 실제 주석, 다른 표현의 동등 CHECK는 거부한다. 단일 따옴표 문자열과 두 번 쓴 작은따옴표 이스케이프를 구별해 문자열 안의 주석 기호·인용 기호·괄호는 문자로 처리한다. 해석이 끝나지 않은 문자열과 불균형 괄호는 닫힌 실패다. 테이블 이름은 SQLite RENAME이 만드는 ASCII 인용 이름도 받는다. 허용 식은 sql/tests.rs의 GOOD_MONEY 고정 자료와 schema.rs에 있다. NULL 허용 여부는 각 저장소 모델이 정한다. 금액 산술은 하지 않는다. S-20에서 SQL 파서 기반 검사로 통합한다.

schema.snapshot.sql은 빈 DB 적용 결과다. sqlite_schema에서 내부 객체와 SQL 없는 자동 인덱스를 제외한 테이블·명시적 인덱스·뷰·트리거 정의 전체를 type/name 순으로 읽는다. 자동 UNIQUE 인덱스의 제약은 테이블 DDL에 남는다. 객체 범위를 확인하는 별도 고정 자료가 있다. runtime에서는 STRICT를 테이블에 적용하며 뷰·트리거의 존재 자체를 거부하지 않는다. 테스트는 파일을 읽기만 한다. 명시적 갱신 명령은 현재 단일 마이그레이션에서 `cp crates/adapters/store-sqlite/migrations/0001_migration_ledger.sql crates/adapters/store-sqlite/schema.snapshot.sql`이다. 다음 마이그레이션부터는 적용 후 스키마 추출 명령도 함께 추가한다.

close(self)는 DB 마지막 소유권을 spawn_blocking으로 옮겨 수락한 쓰기를 끝내고 join한다. 쓰기 스레드 패닉·요청 채널 단절은 RepositoryIntegrity로 돌려준다. Drop의 동기 안전망은 유지한다. SQL 실행 API는 crate 내부로 유지한다.

## 승인된 변경

기존 Database::open 호출에 가짜 Clock 인자만 추가하고 기존 단언을 유지한다. 기존 schema count 고정 기대값은 원장과 고유 인덱스로 0에서 2로 갱신한다. S-01 테스트 samples의 문자열 value 열은 금액 이름 규칙에 걸리므로 payload로 바꾼다. 데이터·기대값은 그대로다. 준비 부분 변경 승인 (d)와 정확한 개수 변경 승인 (c)를 적용한다. 생성물은 저장소 스크립트로 재생성한다.

## 검증

target/S-15에 red/green·검사 로그를 보관한다. 프로젝트·세션 훅 신뢰는 미확인이다. Git 훅은 시작 절차에서 설치했다. 독립 리뷰와 변이 결과는 검증이 끝난 뒤 기록한다.

## 인계

- U-01: 사이드카 검사와 실제 열기 사이 경합은 남아 있다. 실제 앱 데이터 폴더 준비, 마지막 Database 소유자의 close 호출 위치와 실제 UI 응답성 검증. close를 쓰지 않는 Drop은 여전히 동기 join이다. 동시 새 DB 열기의 빠른 Unavailable은 기준 S-01에서도 재현됐으므로 중복 열기를 만드는 조립 경로와 재시도 책임도 여기서 정한다.
- S-02 이후: 새 파일을 순서대로 등록하고 해당 저장소 테이블·통화·CHECK와 스냅샷을 추가한다.
- S-08: 마이그레이션 직후 authorizer는 해제된다. 감사 로그 정책은 그 뒤 연결에 설치한다.
- S-20: 금액 이름·DDL 검사와 SQL 상수의 파일 내장을 파서로 통합하고 다중 마이그레이션의 스냅샷 추출 명령을 확장한다. 약어·복수형 이름 정책과 원장 자체 DDL 검증도 함께 검토한다.
- S-05: FTS5는 STRICT를 지원하지 않는다. 전부 STRICT 원칙과 FTS 방식의 문서 충돌은 S-05 전에 결정한다. 이번 실행기에는 예외를 넣지 않았다.
- S-16: 원장 전체 되돌리기 방지와 감사 로그의 외부 머리값 검증은 별도다.

## 최초 구현 검사 기록 (704c748)

- 최초 구현 로컬 nextest: 42/42 통과. clippy와 arch-check/source-check/sql-check/migration-check/raw-types/disable-check 통과.
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

## 최초 구현 손 변형 (704c748)

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

## 1차 리뷰에 따른 수정

새 Opus의 적대적 리뷰와 새 Codex의 독립 비판을 거쳤다. 지적 1·2의 실제 우회, 3의 파일·내장 목록 연결, 4의 스냅샷 객체 범위, 6의 수량·비율 통화 구분, 8의 M7/M17/M13 비등가 회귀 공백을 보완했다. 다중 미적용(두 번째 성공 뒤 세 번째 실패)의 원자성도 실제 시험으로 고정했다. 테이블 RENAME의 인용 이름은 안전한 ASCII 이름 범위에서 지원했다.

수정 전 독립 회귀 9개 중 5개 실패를 확인했다. 단위 접미사의 별도 반례도 먼저 실패시킨 뒤 고쳤다. 현재 수정 사본 nextest는 56개 통과이며 clippy·source-check·sql-check·migration-check도 통과했다. 실행별 근거와 최종 리뷰 판단은 reviews/S-15.md에 남긴다. 아래 최초 구현 28개 변형은 이 수정분의 최종 변이 결과로 대신 세지 않는다.

## 1차 리뷰 수정 검증

수정 구간 704c748 이후 test-diff는 위반이 없다. 자동 mutants-diff 첫 실행은 42개 중 검출 33·생존 7·컴파일 불가 2였다. 비테이블 객체가 정상이라는 runtime 대조군과 SQLite가 받지만 지원하지 않는 빈 이름·비ASCII 이름·각 주석 형식의 독립 회귀를 추가했다. sqlite_schema의 table SQL에서 이미 보장되는 CREATE TABLE 접두사 중복 방어는 제거했다. 빈 이름·ASCII 식별자 제한은 유지했다. 다시 실행한 자동 변이는 선택 36·검출 34·컴파일 불가 2·생존 0·시간 초과 0으로 통과했다. 컴파일 불가 2개는 검출 수에 넣지 않는다.

손 변형의 앞선 시도 중 Plain 변형은 컴파일 불가라 제외했다. 의미 있는 숫자 이름 분류 반전으로 교체한 아래 최종 29개는 모두 다시 컴파일해 검출했고 전후 대조군도 각각 56개 통과했다. 별도로 0002_unembedded.sql을 추가하자 전체 파일·내장 목록 검사가 실패했고, 제거한 대조군은 통과했다. 마지막 NUL 변형은 형식 대조와 실제 고정 자료의 char(0)을 함께 char(1)로 바꿔 실제 NUL 삽입 회귀로 검출한다.

| 번호 | 파일                | 바꾼 것                                                                                                      | 잡은 테스트                                                                    | 결과 |
| ---- | ------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------ | ---- |
| 1    | migration.rs        | `byte >> 4_u8 → byte >> 3_u8`                                                                                | `migration_tests::sha256_matches_independent_known_vectors`                    | 검출 |
| 2    | migration.rs        | `byte & 15 → byte & 7`                                                                                       | `migration_tests::sha256_matches_independent_known_vectors`                    | 검출 |
| 3    | migration.rs        | `version > migrations.len() → version < migrations.len()`                                                    | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 4    | migration.rs        | `before == 0 && version != 0 → before == 0 && version == 0`                                                  | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 5    | migration.rs        | `if before == 0 { → if before != 0 {`                                                                        | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 6    | migration.rs        | `rows.len() != version → rows.len() == version`                                                              | `review_tests::snapshot_contains_index_view_trigger_and_tables`                | 검출 |
| 7    | migration.rs        | `&#124;&#124; (before != 0 && rows.is_empty()) →`                                                            | `review_tests::resetting_both_ledger_and_version_is_not_a_fresh_database`      | 검출 |
| 8    | migration.rs        | `if index >= rows.len() → if index > rows.len()`                                                             | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`          | 검출 |
| 9    | migration.rs        | `if index != 0 { → if index == 0 {`                                                                          | `review_tests::snapshot_contains_index_view_trigger_and_tables`                | 검출 |
| 10   | migration.rs        | `digest(source), → digest(name),`                                                                            | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 11   | migration.rs        | `at.utc_seconds().as_i64() → at.utc_seconds().as_i64().checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?` | `migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger`        | 검출 |
| 12   | migration.rs        | `transaction.commit() → transaction.rollback()`                                                              | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 13   | migration.rs        | `*strict != 1 → *strict == 1`                                                                                | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 14   | migration.rs        | `row.get::<_, i64>(1) → row.get::<_, i64>(0)`                                                                | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`          | 검출 |
| 15   | migration.rs        | `name != expected_name → name == expected_name`                                                              | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 16   | migration.rs        | `*hash != digest(source) → *hash == digest(source)`                                                          | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 17   | migration.rs        | `if prefix != Some(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?) → if prefix.is_none()`        | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 18   | migration.rs        | `guarded.store(false, Ordering::Relaxed) → guarded.store(true, Ordering::Relaxed)`                           | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 19   | migration.rs        | `guarded.store(true, Ordering::Relaxed) → guarded.store(false, Ordering::Relaxed)`                           | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 20   | migration.rs        | `if guarded.load(Ordering::Relaxed) → if !guarded.load(Ordering::Relaxed)`                                   | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`          | 검출 |
| 21   | schema.rs           | `ColumnKind::Scalar => {} → ColumnKind::Scalar => return Err(rusqlite::Error::InvalidQuery)`                 | `review_tests::quantity_and_ratio_checks_do_not_require_currency`              | 검출 |
| 22   | schema.rs           | `MONEY_SEGMENTS.contains(&part.as_str()) → !MONEY_SEGMENTS.contains(&part.as_str())`                         | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 23   | schema.rs           | `.rev()         .find( → .find(`                                                                             | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier` | 검출 |
| 24   | schema.rs           | `SCALAR_SEGMENTS.contains(&unit.as_str()) → !SCALAR_SEGMENTS.contains(&unit.as_str())`                       | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier` | 검출 |
| 25   | schema.rs           | `words.get(1) != Some(&"text") → words.get(1) == Some(&"text")`                                              | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 26   | schema.rs           | `!compact.ends_with(&expected) → compact.ends_with(&expected)`                                               | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 27   | schema.rs           | `previous.is_ascii_digit() → false`                                                                          | `review_tests::digit_to_uppercase_boundary_is_a_numeric_name_segment`          | 검출 |
| 28   | migration.rs        | `sql IS NOT NULL AND name NOT GLOB → type = 'table' AND name NOT GLOB`                                       | `review_tests::snapshot_contains_index_view_trigger_and_tables`                | 검출 |
| 29   | schema.rs, tests.rs | `char(0) → char(1) / char(0) → char(1)`                                                                      | `review_tests::decimal_check_rejects_embedded_nul_bytes`                       | 검출 |

## 2차 리뷰에 따른 수정

2차 Opus 리뷰와 별도 Codex 비판이 인용 구문에 의한 금액 열 검사 우회를 실행으로 확인했다. 본문의 지원하지 않는 인용 문법은 거부하고 단일 따옴표 문자열의 경계와 이스케이프를 추적한다. SQLite RENAME의 안전한 ASCII 인용 테이블 머리말은 계속 지원한다. 정상 음수와 나눗셈 기본값은 실제 주석과 구별한다.

exposure_values가 KRW Money를 반환하는 domain 모델에 맞춰 exposure는 통화를 요구한다. return의 단위는 문서에 확정되지 않았으므로 보수적으로 금액으로 취급하고 명시적 ratio·rate 접미사는 비율로 분류한다. 향후 명칭 정책 통합은 S-20 인계다.

독립 비판의 추가 L-1은 미적용 SQL이 기존 원장을 DELETE·UPDATE하거나 새 행을 트리거로 지워도 성공하는 문제였다. 적용 직전의 원장을 검증하고, 커밋 직전 전체 원장을 다시 읽어 모든 번호·이름·해시·적용 시각을 기대 목록과 대조한다. 기존 시각은 보존하고 새 행은 주입 Clock의 시각과 같아야 한다. 손상된 초기 원장을 후속 SQL이 복구해도 적용 전에 거부한다. 정상 두 번째 마이그레이션과 데이터 변경은 허용한다.

수정 전 새 회귀 7개 중 6개가 실패했다. 최초 수정 후 64개 통과, 변이 보강 후 최종 66개 통과다. 정상 DML·서로 다른 적용 시각·재열기와 원장 7종 변조의 롤백을 시험했다. 자동 mutants-diff bcca5a9 최초 28개는 검출19·생존8·컴파일불가1이었다. 적용 전 검사와 적용 후 검사의 중복 때문에 살아남은 변형은 손상 복구 SQL로 구분했다. 정상 참조 인용과 음수·나눗셈 대조군도 추가했다. 재실행은 선택28·검출27·컴파일불가1·생존0·시간초과0으로 통과했다. 컴파일불가는 검출에 세지 않는다.

## 2차 수정 손 변형

mktemp 사본에서 매 변형마다 store-sqlite를 정리하고 다시 컴파일해 실행했다. 최종 유효 39개 모두 테스트가 검출했고 전후 대조군은 각각 66개 통과했다. 앞선 시도 중 사용하지 않는 함수·변수 오류로 컴파일 불가였던 3개는 제외하고, 행 수만 비교·이름 접미사 변조·소스 바이트 변조의 컴파일 가능한 변형으로 교체해 전부 다시 실행했다. 별도 미내장 파일 추가도 목록 대조에서 검출했다. 로그는 target/S-15/third-mut-*.log와 third-mutation-results.json에 있다.

| 번호 | 파일                | 바꾼 것                                                                                                                     | 잡은 테스트                                                                    | 결과 |
| ---- | ------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ---- |
| 1    | migration.rs        | `byte >> 4_u8 → byte >> 3_u8`                                                                                               | `migration_tests::sha256_matches_independent_known_vectors`                    | 검출 |
| 2    | migration.rs        | `byte & 15 → byte & 7`                                                                                                      | `migration_tests::sha256_matches_independent_known_vectors`                    | 검출 |
| 3    | migration.rs        | `version > migrations.len() → version < migrations.len()`                                                                   | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 4    | migration.rs        | `before == 0 && version != 0 → before == 0 && version == 0`                                                                 | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 5    | migration.rs        | `if before == 0 { → if before != 0 {`                                                                                       | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 6    | migration.rs        | `rows.len() != version → rows.len() == version`                                                                             | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`          | 검출 |
| 7    | migration.rs        | `&#124;&#124; (before != 0 && rows.is_empty()) →`                                                                           | `review_tests::resetting_both_ledger_and_version_is_not_a_fresh_database`      | 검출 |
| 8    | migration.rs        | `if index >= rows.len() → if index > rows.len()`                                                                            | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 9    | migration.rs        | `if index != 0 { → if index == 0 {`                                                                                         | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 10   | migration.rs        | `digest(source), → digest(name),`                                                                                           | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 11   | migration.rs        | `at.utc_seconds().as_i64() → at.utc_seconds().as_i64().checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?`                | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 12   | migration.rs        | `transaction.commit() → transaction.rollback()`                                                                             | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 13   | migration.rs        | `*strict != 1 → *strict == 1`                                                                                               | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 14   | migration.rs        | `row.get::<_, i64>(1) → row.get::<_, i64>(0)`                                                                               | `migration_tests::embedded_migration_and_saved_schema_match_files`             | 검출 |
| 15   | migration.rs        | `row.name.as_str() != *expected_name → row.name.as_str() == *expected_name`                                                 | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 16   | migration.rs        | `row.sha256 != digest(source) → row.sha256 == digest(source)`                                                               | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 17   | migration.rs        | `if prefix != Some(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?) → if prefix.is_none()`                       | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 18   | migration.rs        | `guarded.store(false, Ordering::Relaxed) → guarded.store(true, Ordering::Relaxed)`                                          | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 19   | migration.rs        | `guarded.store(true, Ordering::Relaxed) → guarded.store(false, Ordering::Relaxed)`                                          | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 20   | migration.rs        | `if guarded.load(Ordering::Relaxed) → if !guarded.load(Ordering::Relaxed)`                                                  | `migration_tests::migration_cannot_escape_transaction`                         | 검출 |
| 21   | schema.rs           | `ColumnKind::Scalar => {} → ColumnKind::Scalar => return Err(rusqlite::Error::InvalidQuery)`                                | `review_tests::quantity_and_ratio_checks_do_not_require_currency`              | 검출 |
| 22   | schema.rs           | `MONEY_SEGMENTS.contains(&part.as_str()) → !MONEY_SEGMENTS.contains(&part.as_str())`                                        | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 23   | schema.rs           | `.rev()         .find( → .find(`                                                                                            | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier` | 검출 |
| 24   | schema.rs           | `SCALAR_SEGMENTS.contains(&unit.as_str()) → !SCALAR_SEGMENTS.contains(&unit.as_str())`                                      | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier` | 검출 |
| 25   | schema.rs           | `words.get(1) != Some(&"text") → words.get(1) == Some(&"text")`                                                             | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 26   | schema.rs           | `!compact.ends_with(&expected) → compact.ends_with(&expected)`                                                              | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 27   | schema.rs           | `previous.is_ascii_digit() → false`                                                                                         | `review_tests::digit_to_uppercase_boundary_is_a_numeric_name_segment`          | 검출 |
| 28   | migration.rs        | `sql IS NOT NULL AND name NOT GLOB → type = 'table' AND name NOT GLOB`                                                      | `review_tests::snapshot_contains_index_view_trigger_and_tables`                | 검출 |
| 29   | migration.rs        | `ledger_rows(connection)? == expected → ledger_rows(connection)?.len() == expected.len()`                                   | `third_tests::successful_sql_cannot_commit_a_damaged_ledger`                   | 검출 |
| 30   | migration.rs        | `ledger_rows(connection)? == expected → ledger_rows(connection)? != expected`                                               | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                | 검출 |
| 31   | migration.rs        | `&#124;row&#124; row.applied_at → &#124;_row&#124; *at.utc_seconds().as_i64()`                                              | `third_tests::valid_data_migration_preserves_distinct_application_times`       | 검출 |
| 32   | migration.rs        | `name: String::from(*name) → name: format!("{name}x")`                                                                      | `migration_tests::connection_sql_constants_prepare_on_migrated_database`       | 검출 |
| 33   | migration.rs        | `sha256: digest(source) → sha256: digest(&format!("{source}x"))`                                                            | `review_tests::table_rename_quotes_do_not_change_column_identity`              | 검출 |
| 34   | schema.rs           | `'"' &#124; '`' &#124; '[' => return Err(rusqlite::Error::InvalidQuery) → 'x' => return Err(rusqlite::Error::InvalidQuery)` | `review_tests::quantity_and_ratio_checks_do_not_require_currency`              | 검출 |
| 35   | schema.rs           | `remaining.peek() == Some(&'-') → remaining.peek() == Some(&'x')`                                                           | `review_tests::unsupported_schema_forms_fail_after_sqlite_accepts_them`        | 검출 |
| 36   | schema.rs           | `remaining.peek() == Some(&'*') → remaining.peek() == Some(&'x')`                                                           | `review_tests::unsupported_schema_forms_fail_after_sqlite_accepts_them`        | 검출 |
| 37   | schema.rs           | `depth == 0 → depth == 1`                                                                                                   | `review_tests::camel_case_column_must_check_its_own_identifier`                | 검출 |
| 38   | schema.rs           | `Some(quote) => current.push(quote) → Some(quote) => { current.push(quote); *context = TextContext::Syntax; }`              | `third_tests::single_quoted_text_markers_do_not_change_column_boundaries`      | 검출 |
| 39   | schema.rs, tests.rs | `char(0) → char(1)` / `char(0) → char(1)`                                                                                   | `review_tests::decimal_check_rejects_embedded_nul_bytes`                       | 검출 |

## 3차 리뷰 수정과 최종 보장

3차 독립 리뷰와 비판에서 실행 재현된 헤더 주석 경계 우회·Unicode 식별자·마이그레이션 PRAGMA 상태 변경을 수정했다. 첫 괄호 앞에는 ASCII 헤더의 정확한 세 토큰만 허용한다. 본문은 문자열 밖의 비ASCII 문자를 거부하되 한글 등 문자열 데이터는 그대로 허용한다. 실제 SQLite가 수용한 위반 DDL을 적용과 재검증 양쪽에서 거부하고 롤백을 확인한다. 내부 열 분할기의 불균형 괄호·닫히지 않은 문자열도 직접 시험한다.

마이그레이션 원문 실행 구간에만 PRAGMA를 authorizer로 거부한다. 실행기 소유 user_version·스키마 진단과 기존 연결 설정은 허용한다. CHECK 해제 시도는 실패하고 설정0과 이전 version을 유지하며 잘못된 금액은 거부하고 정상 십진수는 저장된다. 오류 뒤 authorizer 해제와 재실행도 검증한다. 현재 실제 파일은 0001 하나이므로 미래 0002/0003 고정 자료의 결과를 현행 제품 DB 손상으로 주장하지 않는다.

3차 비판은 경계 가드 M2/M3의 시험 공백은 확인했으나 M4의 비동등 주장을 재현하지 못했다. 헤더 차단 뒤의 중복 방어와 개별 제품 결함을 구분한다. 동시 열기에 busy timeout이 없다는 추정도 기각됐다. 실제 값은5000ms이고 비판자의80회는 전부 성공했다. 이전의 간헐적 실패 원인은 확정하지 않고 U-01 인계를 유지한다. 원장 자체의 정확한 DDL 검증은 여전히 S-20 인계이며, 원장 행 완전성 검증과 구분한다.

사용자 지시에 따라 3차의 타당한 필수 지적을 직접 수정하고 회귀·변이 결과를 기록하며 4차 독립 리뷰는 진행하지 않는다. 최종 코드 nextest70/70과 clippy 통과. 자동 mutants-diff adb3fa5는 최종14개 중 검출13·컴파일불가1·생존0·시간초과0으로 통과했다. 수정 전 신규3개 중2개 실패, 문자열 정상 대조군1개 통과를 먼저 확인했다.

## 3차 수정 최종 손 변형

최종 mktemp 사본에서 유효43개를 매번 다시 컴파일해 모두 검출했다. 전후 대조군은 각각70개 통과했고 미내장 파일 추가도 검출했다. 앞선 실험은 같은 파일의 두 변경을 복구하는 스크립트 오류로 마지막 대조군이 실패했으므로 최종 통과 근거로 쓰지 않는다. 사본 복구를 수정한 뒤 아래43개와 전후 대조군을 전부 다시 실행했다. 제품 작업 사본에는 실험 변형을 적용하지 않았다. 로그는 final-mutation-results.json, final-mut-*.log, final-manual-progress.log다.

| 번호 | 파일                | 바꾼 것                                                                                                                      | 잡은 테스트                                                                        | 결과 |
| ---- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ---- |
| 1    | migration.rs        | `byte >> 4_u8 → byte >> 3_u8`                                                                                                | `migration_tests::sha256_matches_independent_known_vectors`                        | 검출 |
| 2    | migration.rs        | `byte & 15 → byte & 7`                                                                                                       | `migration_tests::sha256_matches_independent_known_vectors`                        | 검출 |
| 3    | migration.rs        | `version > migrations.len() → version < migrations.len()`                                                                    | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`              | 검출 |
| 4    | migration.rs        | `before == 0 && version != 0 → before == 0 && version == 0`                                                                  | `migration_tests::connection_sql_constants_prepare_on_migrated_database`           | 검출 |
| 5    | migration.rs        | `if before == 0 { → if before != 0 {`                                                                                        | `final_tests::unicode_literal_data_remains_supported`                              | 검출 |
| 6    | migration.rs        | `rows.len() != version → rows.len() == version`                                                                              | `migration_tests::connection_sql_constants_prepare_on_migrated_database`           | 검출 |
| 7    | migration.rs        | `&#124;&#124; (before != 0 && rows.is_empty()) →`                                                                            | `review_tests::resetting_both_ledger_and_version_is_not_a_fresh_database`          | 검출 |
| 8    | migration.rs        | `if index >= rows.len() → if index > rows.len()`                                                                             | `migration_tests::embedded_migration_and_saved_schema_match_files`                 | 검출 |
| 9    | migration.rs        | `if index != 0 { → if index == 0 {`                                                                                          | `migration_tests::migration_cannot_escape_transaction`                             | 검출 |
| 10   | migration.rs        | `digest(source), → digest(name),`                                                                                            | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`              | 검출 |
| 11   | migration.rs        | `at.utc_seconds().as_i64() → at.utc_seconds().as_i64().checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?`                 | `migration_tests::corrupt_ledger_and_future_database_fail_open`                    | 검출 |
| 12   | migration.rs        | `transaction.commit() → transaction.rollback()`                                                                              | `migration_tests::embedded_migration_and_saved_schema_match_files`                 | 검출 |
| 13   | migration.rs        | `*strict != 1 → *strict == 1`                                                                                                | `migration_tests::migration_cannot_escape_transaction`                             | 검출 |
| 14   | migration.rs        | `row.get::<_, i64>(1) → row.get::<_, i64>(0)`                                                                                | `migration_tests::migration_cannot_escape_transaction`                             | 검출 |
| 15   | migration.rs        | `row.name.as_str() != *expected_name → row.name.as_str() == *expected_name`                                                  | `final_tests::unicode_literal_data_remains_supported`                              | 검출 |
| 16   | migration.rs        | `row.sha256 != digest(source) → row.sha256 == digest(source)`                                                                | `final_tests::unicode_literal_data_remains_supported`                              | 검출 |
| 17   | migration.rs        | `if prefix != Some(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?) → if prefix.is_none()`                        | `review_tests::embedded_sequence_gap_rolls_back_pending_schema`                    | 검출 |
| 18   | migration.rs        | `guarded.store(false, Ordering::Relaxed) → guarded.store(true, Ordering::Relaxed)`                                           | `final_tests::unicode_literal_data_remains_supported`                              | 검출 |
| 19   | migration.rs        | `guarded.store(true, Ordering::Relaxed) → guarded.store(false, Ordering::Relaxed)`                                           | `migration_tests::migration_cannot_escape_transaction`                             | 검출 |
| 20   | migration.rs        | `if guarded.load(Ordering::Relaxed) → if !guarded.load(Ordering::Relaxed)`                                                   | `final_tests::unsupported_header_and_unicode_syntax_fail_apply_and_revalidation`   | 검출 |
| 21   | schema.rs           | `ColumnKind::Scalar => {} → ColumnKind::Scalar => return Err(rusqlite::Error::InvalidQuery)`                                 | `review_tests::quantity_and_ratio_checks_do_not_require_currency`                  | 검출 |
| 22   | schema.rs           | `MONEY_SEGMENTS.contains(&part.as_str()) → !MONEY_SEGMENTS.contains(&part.as_str())`                                         | `review_tests::camel_case_column_must_check_its_own_identifier`                    | 검출 |
| 23   | schema.rs           | `.rev()         .find( → .find(`                                                                                             | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier`     | 검출 |
| 24   | schema.rs           | `SCALAR_SEGMENTS.contains(&unit.as_str()) → !SCALAR_SEGMENTS.contains(&unit.as_str())`                                       | `review_tests::monetary_suffix_still_requires_currency_after_scalar_qualifier`     | 검출 |
| 25   | schema.rs           | `words.get(1) != Some(&"text") → words.get(1) == Some(&"text")`                                                              | `review_tests::camel_case_column_must_check_its_own_identifier`                    | 검출 |
| 26   | schema.rs           | `!compact.ends_with(&expected) → compact.ends_with(&expected)`                                                               | `review_tests::camel_case_column_must_check_its_own_identifier`                    | 검출 |
| 27   | schema.rs           | `previous.is_ascii_digit() → false`                                                                                          | `review_tests::digit_to_uppercase_boundary_is_a_numeric_name_segment`              | 검출 |
| 28   | migration.rs        | `sql IS NOT NULL AND name NOT GLOB → type = 'table' AND name NOT GLOB`                                                       | `review_tests::snapshot_contains_index_view_trigger_and_tables`                    | 검출 |
| 29   | migration.rs        | `ledger_rows(connection)? == expected → ledger_rows(connection)?.len() == expected.len()`                                    | `third_tests::successful_sql_cannot_commit_a_damaged_ledger`                       | 검출 |
| 30   | migration.rs        | `ledger_rows(connection)? == expected → ledger_rows(connection)? != expected`                                                | `migration_tests::failed_pending_migration_rolls_back_ddl_and_ledger`              | 검출 |
| 31   | migration.rs        | `&#124;row&#124; row.applied_at → &#124;_row&#124; *at.utc_seconds().as_i64()`                                               | `third_tests::valid_data_migration_preserves_distinct_application_times`           | 검출 |
| 32   | migration.rs        | `name: String::from(*name) → name: format!("{name}x")`                                                                       | `migration_tests::embedded_migration_and_saved_schema_match_files`                 | 검출 |
| 33   | migration.rs        | `sha256: digest(source) → sha256: digest(&format!("{source}x"))`                                                             | `migration_tests::embedded_migration_and_saved_schema_match_files`                 | 검출 |
| 34   | schema.rs           | `'"' &#124; '`' &#124; '[' => return Err(rusqlite::Error::InvalidQuery) → 'x' => return Err(rusqlite::Error::InvalidQuery)`  | `review_tests::quantity_and_ratio_checks_do_not_require_currency`                  | 검출 |
| 35   | schema.rs           | `remaining.peek() == Some(&'-') → remaining.peek() == Some(&'x')`                                                            | `review_tests::unsupported_schema_forms_fail_after_sqlite_accepts_them`            | 검출 |
| 36   | schema.rs           | `remaining.peek() == Some(&'*') → remaining.peek() == Some(&'x')`                                                            | `review_tests::unsupported_schema_forms_fail_after_sqlite_accepts_them`            | 검출 |
| 37   | schema.rs           | `depth == 0 → depth == 1`                                                                                                    | `review_tests::camel_case_column_must_check_its_own_identifier`                    | 검출 |
| 38   | schema.rs           | `Some(quote) => current.push(quote) → Some(quote) => { current.push(quote); *context = TextContext::Syntax; }`               | `schema::tests::malformed_column_boundaries_fail_closed`                           | 검출 |
| 39   | schema.rs           | `header.split_ascii_whitespace().count() != 3 → header.split_ascii_whitespace().count() < 3`                                 | `final_tests::unsupported_header_and_unicode_syntax_fail_apply_and_revalidation`   | 검출 |
| 40   | schema.rs           | `non_ascii if !non_ascii.is_ascii() → non_ascii if non_ascii.is_ascii()`                                                     | `review_tests::camel_case_column_must_check_its_own_identifier`                    | 검출 |
| 41   | migration.rs        | `&#124; AuthAction::Pragma { .. } → `                                                                                        | `migration_tests::migration_cannot_escape_transaction`                             | 검출 |
| 42   | schema.rs, tests.rs | `char(0) → char(1)` / `char(0) → char(1)`                                                                                    | `review_tests::decimal_check_rejects_embedded_nul_bytes`                           | 검출 |
| 43   | migration.rs        | `&#124; AuthAction::Pragma { .. } → ` / `match action { → match action { AuthAction::Pragma { .. } => Authorization::Allow,` | `final_tests::migration_pragma_cannot_disable_checks_and_owned_pragmas_still_work` | 검출 |
