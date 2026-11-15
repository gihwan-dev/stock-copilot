# S-01 DB 연결과 암호화

- 상태: 초안

## 결정

메인의 확정에 따라 rusqlite =0.42.0, 기본 기능 끔, bundled-sqlcipher·backup·hooks를 사용한다. 0.41.1은 RUSTSEC-2026-0502 때문에 쓰지 않는다. crates.io의 정식 릴리스 목록에서 0.41.0·0.41.1 다음이 0.42.0임을 확인했고, 배포된 rusqlite 0.42.0 Cargo.toml은 libsqlite3-sys 0.40.0을 요구한다. 실제 받은 libsqlite3-sys 0.40.0은 sqlcipher-src =420.0.0을 고정하므로 수정된 SQLCipher를 번들하는 가장 낮은 정식 릴리스다. 사전 릴리스·git 의존성·취약점 무시는 사용하지 않는다.

libsqlite3-sys 0.40.0의 매니페스트와 build.rs를 확인했다. bundled-sqlcipher는 sqlcipher-src를 켜고 vendored-openssl은 켜지 않는다. macOS 호스트·대상에서 OPENSSL_DIR 또는 OPENSSL_LIB_DIR·OPENSSL_INCLUDE_DIR가 지정되지 않으면 SQLCIPHER_CRYPTO_CC와 Security·CoreFoundation 링크를 사용한다. 외부 환경 변수가 OpenSSL 경로를 지정하면 빌드 선택이 달라질 수 있으므로 의존 그래프 검사와 연결마다 commoncrypto 공급자 검사를 함께 한다.


원시 키는 SecretValue를 빌려 32바이트인지 검사하고 84바이트 용량을 미리 확보한 Zeroizing<String>에 고정 접두·접미와 64자리 hex만 넣는다. 추가 할당과 복사 없이 SQLCipher 원시 키 문장을 실행하고 버퍼를 지운다. 비밀 소유자는 조립 루트이며 연결을 연 뒤 SecretValue를 해제한다. 오류에는 외부 오류나 SQL을 담지 않는다. 키 길이 오류는 SecretIntegrity, 연결·조회·쓰기·스레드 실패는 RepositoryIntegrity다. Database Debug는 고정 문자열이다.

쓰기 연결 하나는 생성 직후 전용 스레드에 이동하고 이후 쓰기·트랜잭션은 모두 그 스레드에서 수행한다. FIFO mpsc 채널과 요청별 Tokio oneshot으로 응답한다. 패닉으로 스레드가 끝나면 현재·대기 요청의 응답 채널이 닫히고 이후 전송도 실패한다. Drop은 Stop 요청과 join으로 닫는다. 종료는 진행 중인 작업을 마친 뒤 이뤄지며 무한히 실행되는 내부 작업의 선점은 제공하지 않는다.

문서에 읽기 수가 별도로 지정되어 있지 않아 읽기 전용 연결 하나를 유지한다. 읽기는 SQLITE_OPEN_READ_ONLY와 query_only=ON을 함께 적용하고 Tokio의 비동기 mutex로 직렬화한 뒤 spawn_blocking에서 실행한다. 기존 잠금 파일의 Tokio =1.54.1에 rt·sync만 켠다. 네트워크·프로세스 기능을 직접 켜지 않는다. 코어 스레드가 DB 응답을 동기 대기하지 않는다.

공개 API는 DatabasePath::new·as_path, Database::open·check다. 뒤 저장소 구현용 read와 write는 crate 내부 전용이며 외부에는 임의 SQL·Connection·Transaction을 받는 실행 API를 공개하지 않는다. SQL은 src/sql 아래 상수에 둔다. S-20의 향후 파서·실행 인자 검사는 이번에 구현하지 않는다.

| PRAGMA | 적용·기대값 | 확인 |
|---|---|---|
| key | 32바이트 원시 키 | sqlite_schema 읽기로 키 검증 |
| cipher_provider | commoncrypto | 문자열 일치 |
| cipher_memory_security | ON | 문자열 1 |
| cipher_use_hmac | 기본 ON | 문자열 1 |
| cipher_page_size | 기본 4096 | 문자열 4096 |
| temp_store | 2 | 정수 2 |
| journal_mode | WAL | 적용 결과와 재조회가 wal |
| foreign_keys | ON | 정수 1 |
| synchronous | FULL | 정수 2 |
| query_only | 읽기 연결 ON | 정수 1 |

SQLCipher 관련 숫자 PRAGMA는 실제 드라이버에서 TEXT로 반환하므로 문자열로 검증한다. 일반 SQLite 숫자 PRAGMA와 구분한다. 암호화된 DB와 열린 WAL의 바이트를 검사하며, 테이블은 테스트용이고 제품 스키마·마이그레이션은 추가하지 않는다.

## 의존성

| 패키지 | 정확한 버전 | 라이선스 | 근거 |
|---|---|---|---|
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | rusqlite 필수 전이 또는 직접 드라이버 |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | rusqlite 필수 전이 또는 직접 드라이버 |
| libsqlite3-sys | 0.40.0 | MIT | rusqlite 필수 전이 또는 직접 드라이버 |
| rsqlite-vfs | 0.2.0 | MIT | rusqlite 필수 전이 또는 직접 드라이버 |
| rusqlite | 0.42.0 | MIT | rusqlite 필수 전이 또는 직접 드라이버 |
| sqlcipher-src | 420.0.0+sqlcipher-4.20.0-sqlite-3.54.1-libtomcrypt-1.18.2 | MIT AND BSD-3-Clause AND blessing AND WTFPL | rusqlite 필수 전이 또는 직접 드라이버 |
| sqlite-wasm-rs | 0.6.3 | MIT | rusqlite 필수 전이 또는 직접 드라이버 |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | rusqlite 필수 전이 또는 직접 드라이버 |

기존 패키지의 직접 의존 추가: application·Tokio =1.54.1(rt, sync)·zeroize =1.9.1(alloc). 테스트는 표준 라이브러리와 제품 의존성만 사용하며 불필요한 testkit 개발 의존성은 추가하지 않는다. deny의 기존 취약점·출처 정책은 유지한다. sqlcipher-src에만 blessing·WTFPL 라이선스를 등록하고 vcpkg 0.2.15 배포본의 셸 파일 세 개는 실제 SHA-256으로 한정 등록한다. libsqlite3-sys 빌드 스크립트와 기존 잠금의 pkg-config 0.3.34(MIT OR Apache-2.0)의 이번 사용도 등록한다. arch-check의 SQLite 위치 제한은 store-sqlite에만 유지한다. rusqlite·Tokio의 절차 매크로 통합과 Tokio의 Tauri 기능 통합으로 보이는 socket2 경로는 해당 경유 경로만 등록한다.

## 인계

- S-15: 마이그레이션 실행기·원장·스키마 검증을 DB 열기 절차에 통합.
- S-02·S-03 이후: 내부 read·write API로 SQL 상수와 도메인 매핑을 구현.
- S-16: 감사 로그 HMAC·키체인 머리값 검증.
- S-20: SQL 상수 파서와 실행 인자 검사 구조 통합.

## 검사와 한계

프로젝트 신뢰와 훅 정의 해시 신뢰·현재 세션 적용은 각각 확인 불가다. pnpm frozen install과 lefthook install은 성공했다. 시작 전체 검사는 지시대로 생략했다.

## 손 변형 검증

| 번호 | 바꾼 것 | 독립 라이브러리 컴파일 | 잡은 테스트 | 결과 |
|---|---|---|---|---|
| 1 | 키 길이 31 | 통과 | connection::tests::text_setting_verification_rejects_mismatch_and_missing_rows, worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::queued_requests_are_released_when_worker_exits, tests::worker_panic_closes_current_queued_and_future_requests, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::encrypted_reopen_and_pragma_contract, tests::database_secret_and_failure_debug_remain_fixed, tests::encrypts_database_and_live_wal_and_rolls_back | 검출 |
| 2 | 키 길이 33 | 통과 | connection::tests::text_setting_verification_rejects_mismatch_and_missing_rows, worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypts_database_and_live_wal_and_rolls_back, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::database_secret_and_failure_debug_remain_fixed, tests::worker_panic_closes_current_queued_and_future_requests, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::encrypted_reopen_and_pragma_contract | 검출 |
| 3 | 상위 nibble 이동 | 통과 | tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 4 | 하위 nibble 마스크 | 통과 | tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 5 | hex 진법 | 통과 | tests::encrypts_database_and_live_wal_and_rolls_back, tests::queued_requests_are_released_when_worker_exits, tests::encrypted_reopen_and_pragma_contract, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::worker_panic_closes_current_queued_and_future_requests, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::database_secret_and_failure_debug_remain_fixed | 검출 |
| 6 | 숫자 검증 반전 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, connection::tests::numeric_setting_verification_rejects_mismatch_and_missing_rows, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::worker_panic_closes_current_queued_and_future_requests, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::database_secret_and_failure_debug_remain_fixed, tests::encrypted_reopen_and_pragma_contract, tests::encrypts_database_and_live_wal_and_rolls_back | 검출 |
| 7 | 문자 검증 반전 | 통과 | connection::tests::text_setting_verification_rejects_mismatch_and_missing_rows, worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypts_database_and_live_wal_and_rolls_back, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::queued_requests_are_released_when_worker_exits, tests::encrypted_reopen_and_pragma_contract, tests::worker_panic_closes_current_queued_and_future_requests, tests::database_secret_and_failure_debug_remain_fixed, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::rejects_wrong_key_plaintext_and_invalid_length | 검출 |
| 8 | 공급자 기대값 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::encrypts_database_and_live_wal_and_rolls_back, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypted_reopen_and_pragma_contract, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::database_secret_and_failure_debug_remain_fixed, tests::queued_requests_are_released_when_worker_exits | 검출 |
| 9 | TEMP 디스크 | 통과 | connection::tests::numeric_setting_verification_rejects_mismatch_and_missing_rows, worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::database_secret_and_failure_debug_remain_fixed, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::encrypts_database_and_live_wal_and_rolls_back, tests::encrypted_reopen_and_pragma_contract, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::worker_panic_closes_current_queued_and_future_requests | 검출 |
| 10 | TEMP 검증값 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypts_database_and_live_wal_and_rolls_back, tests::database_secret_and_failure_debug_remain_fixed, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypted_reopen_and_pragma_contract, tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 11 | 외래키 끄기 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::queued_requests_are_released_when_worker_exits, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::database_secret_and_failure_debug_remain_fixed, tests::encrypts_database_and_live_wal_and_rolls_back, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypted_reopen_and_pragma_contract, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::rejects_wrong_key_plaintext_and_invalid_length | 검출 |
| 12 | WAL 끄기 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypted_reopen_and_pragma_contract, tests::database_secret_and_failure_debug_remain_fixed, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::encrypts_database_and_live_wal_and_rolls_back, tests::worker_panic_closes_current_queued_and_future_requests | 검출 |
| 13 | 읽기 연결 쓰기 허용 | 통과 | tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 14 | 쓰기 연결 읽기 전용 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::database_secret_and_failure_debug_remain_fixed, tests::encrypts_database_and_live_wal_and_rolls_back, tests::worker_panic_closes_current_queued_and_future_requests, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::encrypted_reopen_and_pragma_contract, tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 15 | query_only 끄기 | 통과 | tests::raw_key_is_independently_readable_and_debug_is_closed, tests::database_secret_and_failure_debug_remain_fixed, tests::worker_panic_closes_current_queued_and_future_requests, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::encrypted_reopen_and_pragma_contract, tests::encrypts_database_and_live_wal_and_rolls_back, tests::queued_requests_are_released_when_worker_exits, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread | 검출 |
| 16 | 키 메모리 보호 끄기 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypted_reopen_and_pragma_contract, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::database_secret_and_failure_debug_remain_fixed, tests::queued_requests_are_released_when_worker_exits, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypts_database_and_live_wal_and_rolls_back, tests::raw_key_is_independently_readable_and_debug_is_closed | 검출 |
| 17 | HMAC 기대값 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypted_reopen_and_pragma_contract, tests::database_secret_and_failure_debug_remain_fixed, tests::worker_panic_closes_current_queued_and_future_requests, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::encrypts_database_and_live_wal_and_rolls_back | 검출 |
| 18 | 페이지 크기 기대값 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::encrypted_reopen_and_pragma_contract, tests::worker_panic_closes_current_queued_and_future_requests, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::database_secret_and_failure_debug_remain_fixed, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::encrypts_database_and_live_wal_and_rolls_back | 검출 |
| 19 | 내구성 끄기 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::queued_requests_are_released_when_worker_exits, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypts_database_and_live_wal_and_rolls_back, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::encrypted_reopen_and_pragma_contract, tests::database_secret_and_failure_debug_remain_fixed | 검출 |
| 20 | 커밋 대신 롤백 | 통과 | tests::encrypts_database_and_live_wal_and_rolls_back, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread | 검출 |
| 21 | 외래키 검증값 | 통과 | worker::tests::dropping_writer_disconnects_retained_request_endpoint, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread, tests::worker_panic_closes_current_queued_and_future_requests, tests::database_secret_and_failure_debug_remain_fixed, tests::encrypted_reopen_and_pragma_contract, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::queued_requests_are_released_when_worker_exits, tests::encrypts_database_and_live_wal_and_rolls_back | 검출 |
| 22 | 읽기 보호 검증값 | 통과 | tests::encrypts_database_and_live_wal_and_rolls_back, tests::raw_key_is_independently_readable_and_debug_is_closed, tests::queued_requests_are_released_when_worker_exits, tests::worker_panic_closes_current_queued_and_future_requests, tests::encrypted_reopen_and_pragma_contract, tests::database_secret_and_failure_debug_remain_fixed, tests::rejects_wrong_key_plaintext_and_invalid_length, tests::concurrent_producers_keep_fifo_and_use_one_writer_thread | 검출 |

최종 소스 복사본에서 대조군·복원 대조군 통과, 유효 손 변형 22개 검출, 생존 0, 컴파일 불가 0을 실측했다.

보호 파일 추가 등록: deny.toml(패키지·빌드 스크립트), harness/arch-check/rules.toml(기능 통합 경로), harness/source-checks/allowlists.toml(새 tests.rs의 thread::current만), harness/ci/linux/exclusions.json(CommonCrypto 필수 crate의 macOS 검사 분류), justfile(전체·fast 정적 검사에 sqlcipher 검사를 등록). .codex/config.toml·harness/claude-hooks/hashes.json은 기존 생성 스크립트로 갱신한다. 승인된 정확 목록의 기대값만 갱신하고 기존 단언은 유지한다. 소스 검사 끄기는 없으며 Git 커밋의 승인된 test-diff 예외는 아래에 기록한다.

harness/sqlcipher/check.mjs는 cargo tree의 드라이버 버전과 OpenSSL 부재를 확인하고 산출물을 빌드해 독립 Rust Debug 탐침을 실행한다. otool -L로 탐침의 libcrypto·libssl 미링크를 확인한다. Debug 탐침은 SQL·외부 오류를 출력하지 않고 고정 문자열과 닫힌 오류만 단언한다.

## 폐기한 고정과 해석 증거

0.41.1은 RUSTSEC-2026-0502 때문에 쓰지 않는다. 이전 조합은 rusqlite =0.41.1 → libsqlite3-sys 0.39.1 → sqlcipher-src =419.0.6이었다. cargo deny가 CVE-2026-119375를 거부했다. 별도 복사본에서 cargo update --offline -p sqlcipher-src --precise 420.0.0을 실행했지만 Cargo가 420.0.0 후보를 찾고도 정확한 =419.0.6 요구와 맞지 않는다며 종료 101을 반환했다. 그 실험은 원본 잠금을 바꾸지 않았다. 메인이 0.41.1 고정을 폐기한 뒤 0.42.0으로 올려 이 문제를 해소한다.

공식 근거: https://rustsec.org/advisories/RUSTSEC-2026-0502.html 와 배포된 두 crate의 매니페스트·빌드 스크립트. 원문 로그는 target/S-01/resolution.log와 update-042.log다.

## 검사와 기록

최신 master 735ea38189edbf15d34238dd14ac49bb02c9e94f를 --no-ff 정상 훅으로 받아 병합 커밋 7e4e34940e647aa0976fc4e96d735fe7cd61bd88을 만들었다. --autostash로 보관한 S-01 변경을 복원했고 충돌한 생성물 두 개는 master 것을 선택한 뒤 기존 스크립트로 다시 생성했다. master의 기존 테스트·공용 lib.rs를 직접 바꾸지 않았다.

새 검사 단계는 check-list 단계 표와 justfile 스냅샷에 등록하며, 누락 버전·OpenSSL 의존성·OpenSSL 링크를 각각 실패시키는 독립 사례·실행 테스트를 둔다. Linux에서는 의존 그래프만 검사하고 CommonCrypto 네이티브 탐침은 macOS에서 실행한다.

자동 변이의 첫 실행은 frontendDist가 없어 목록 준비에서 실패했으므로 변이 실적으로 세지 않는다. 재실행 전 기존 just frontend-build 절차를 사용하고 추적 파일에 빈 dist를 추가하지 않는다.

## 승인된 기존 테스트 갱신

| 파일·테스트 | 전 → 후 | 이유 |
|---|---|---|
| harness/ci/tests/ci-plumbing.test.mjs: the Linux exclusion list holds exactly the reviewed macOS-only crates and tests | macOS 전용 crate 정확한 목록 4개 → store-sqlite를 더한 정확한 목록 5개 | CommonCrypto를 강제하는 새 저장소의 승인된 CI 등록에 기계적으로 필요한 목록 갱신. 기존 단언은 유지 |

변이 대상 등록: .cargo/mutants.toml의 기존 세 crate 패턴을 유지하고 crates/adapters/store-sqlite/src/**/*.rs만 추가한다. 새 어댑터 코드가 변경분 변이와 전체 변이의 대상에서 빠지지 않도록 한다.

Debug 고정 출력을 일반 nextest와 자동 변이에서도 검사한다. 테스트의 Display 어댑터가 Debug::fmt에 그대로 위임해 출력 문자열을 단언하므로 SQL 조립 매크로를 쓰지 않는다. 별도 하네스 탐침은 rustc --test로 일반·pretty 출력까지 확인한다.

첫 유효 자동 변이 실행은 29개 중 5개 미검출을 보고했다. Debug 출력·숫자 PRAGMA 확인·Writer 종료의 검증은 실패 반례와 남아 있는 요청 endpoint로 보강했다. 서로 겹치지 않는 OpenFlags 비트의 OR를 XOR로 바꾼 두 동등 변이는 동일한 비트 집합을 나타내므로 OpenFlags::union으로 명시한다. URI 등 기본 플래그를 추가하지 않고 기존 READ_ONLY 또는 READ_WRITE·CREATE와 NO_MUTEX만 유지한다.

## 구현 커밋 전 최종 검사

- nextest: 연결·동시성·설정 불일치·종료·Debug 회귀 11개 통과.
- clippy all-targets, source-check, sql-check 통과.
- cargo deny: advisories·bans·licenses·sources 모두 통과.
- CommonCrypto 공급자 및 산출물 OpenSSL 미링크·Debug 탐침 통과.
- check-list·SQLCipher 검사 사례 테스트 13개와 Linux 정확 목록 테스트 통과.
- 자동 변경분 변이: 최종 실행 25개 중 유효 12개 검출, 컴파일 불가 13개, 생존 0, 시간 초과 0. 컴파일 불가는 검출에 세지 않는다.
- 첫 변이 목록 준비와 커밋 훅 fresh-build가 겹쳐 실행 파일이 사라진 실패는 결과로 세지 않았고, 이후 단독 실행했다.
- 초기 자동 선택 0은 성공 근거로 쓰지 않는다. 새 store-sqlite 패턴 등록 뒤 29개를 실제 선택해 미검출을 보강하고, 최종 25개를 실행했다. 하네스의 2단계 crate 경로 표시는 어댑터에 대해 files=0·crates/adapters로 출력되는 한계가 있지만 실제 선택·실행 결과 파일은 store-sqlite의 25개 변이를 확인한다. H-47 병합 후에도 같은 표시 한계가 남아 있으며 선택·검출 집계와 구분한다. 표시 개선은 후속 하네스 작업에 인계한다.

테스트 DB는 시스템 임시 폴더의 작업 전용 하위 폴더에 생성한다. tests.rs의 temp_dir 호출만 보호 목록에 한정 등록하고, Writer 종료 테스트도 같은 Fixture를 재사용한다. 테스트 보조 DB·WAL·SHM·빈 dist는 추적하지 않는다. 최종 임시 경로 보정 후 자동 변이 25개(유효 12 검출·컴파일 불가 13·생존 및 시간 초과 0), 손 변형 22개와 원본·복원 대조군을 다시 실행해 통과했다.

H-47을 포함한 master e2b940598678ecc91d49267a3d4c02ccef4c819c를 구현 커밋 전에 다시 받아 병합 커밋 37b7ec4bfa01ff6b31788cacba1d4f695afb3ed5을 만들었다. 정상 병합 훅이 통과했으며 생성물 두 개만 재생성했다. SQLite 위치 제한 세 항목은 허용·거부 vendor 사례에 등록해 규칙 요소 삭제 검증이 실제로 잡게 한다. 효과가 없는 application 경유 예외는 제거한다.

승인된 정확 목록 갱신: crates/xtask/src/source_check/cargo_config_tests.rs의 MUTATION_TARGET_GLOBS 길이와 항목을 3→4로 바꾸고 store-sqlite 패턴만 추가한다. mutation_targets_are_the_documented_modules의 기존 assert_eq!는 유지한다. .cargo/mutants.toml과 docs/harness.md의 대상 등록에 기계적으로 필요한 기대 목록이다. 별도 scratch에서 이 검사와 arch-check 여섯 회귀(각 규칙 요소 삭제 검증 포함)가 모두 통과했다. H-47 병합 후 e2b940598678ecc91d49267a3d4c02ccef4c819c 기준 자동 변이도 25개 중 유효 12 검출·컴파일 불가 13·생존 및 시간 초과 0으로 통과했다.

test-diff는 승인된 MUTATION_TARGET_GLOBS 3→4 목록 갱신 한 건만 support-modified로 보고했다. 그 밖의 검사까지 확인한 뒤 지시문이 정한 해당 커밋의 LEFTHOOK=0 예외만 적용할 수 있다. 기존 assert_eq!·나머지 목록과 규칙은 유지한다.

H-47 분할 목록 테스트의 명시적 --file 패턴 목록에도 store-sqlite 패턴 하나를 추가한다. 기존 전체 집합·비중복 deepEqual/assert 검증은 유지하며, 실제 설정의 네 번째 대상이 128개 분할의 합집합에서 누락되지 않게 하는 승인된 정확 목록 갱신이다.

보호된 clock-environment 경계의 정확한 기대 경로 집합도 tests.rs 한 경로를 더해 6→7로 갱신한다. source_check::tests::allowlists_hold_only_the_documented_paths의 assert_eq!와 나머지 경로는 유지한다. 이 역시 새 테스트 파일만의 허용 등록에 기계적으로 필요한 승인된 정확 목록 변경이다.

| 승인된 기존 검사 | 전 → 후 | 유지한 단언 |
|---|---|---|
| source_check/cargo_config_tests.rs의 MUTATION_TARGET_GLOBS | 3개 → store-sqlite 포함 4개 | mutation_targets_are_the_documented_modules의 assert_eq! |
| source_check/tests.rs의 clock-environment 기대 경로 | 6개 → 새 tests.rs 포함 7개 | allowlists_hold_only_the_documented_paths의 assert_eq! 및 나머지 경계 집합 |
| ci-plumbing.test.mjs의 macOS 제외 목록·CLI 인자 | 4개→5개, 인자 8개→10개 | 정확한 deepEqual·나머지 정책 |
| h47-nightly.test.mjs의 분할 --file 대상 | 3개→4개 | 128분할 전체 합집합·비중복 검증 |

전체 회귀는 4096회 속성 설정에서 1413개 통과 후 pnpm 경로 누락으로 실패한 한 건을 지정 PATH로 재실행해 통과했다. 커버리지는 같은 4096회 설정에서 1043개 통과·위반 없음이다. 최종 정적 검사·xtask Clippy·disable-report·disable-report-since·hooks-installed·commit-guard가 통과했고 test-diff에는 위 Rust의 승인된 목록 갱신 두 건만 남았다. 구현 커밋은 지시문이 허용한 LEFTHOOK=0 예외와 사유 메시지로 작성한다.
