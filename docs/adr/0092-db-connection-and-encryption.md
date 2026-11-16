# ADR 0092: S-01 DB 연결과 암호화

- 상태: 확정 (2026-11-16)


## 결정

메인의 확정에 따라 rusqlite =0.42.0, 기본 기능 끔, bundled-sqlcipher·backup·hooks·codec을 사용한다. codec은 SQL 파서를 거치지 않는 안전한 키 전달 API에 필요하며 새 패키지는 추가하지 않는다. 0.41.1은 RUSTSEC-2026-0502 때문에 쓰지 않는다. crates.io의 정식 릴리스 목록에서 0.41.0·0.41.1 다음이 0.42.0임을 확인했고, 배포된 rusqlite 0.42.0 Cargo.toml은 libsqlite3-sys 0.40.0을 요구한다. 실제 받은 libsqlite3-sys 0.40.0은 sqlcipher-src =420.0.0을 고정하므로 수정된 SQLCipher를 번들하는 가장 낮은 정식 릴리스다. 사전 릴리스·git 의존성·취약점 무시는 사용하지 않는다.

libsqlite3-sys 0.40.0의 매니페스트와 build.rs를 확인했다. bundled-sqlcipher는 sqlcipher-src를 켜고 vendored-openssl은 켜지 않는다. macOS 호스트·대상에서 OPENSSL_DIR 또는 OPENSSL_LIB_DIR·OPENSSL_INCLUDE_DIR가 지정되지 않으면 SQLCIPHER_CRYPTO_CC와 Security·CoreFoundation 링크를 사용한다. 외부 환경 변수가 OpenSSL 경로를 지정하면 빌드 선택이 달라질 수 있으므로 의존 그래프 검사와 연결마다 commoncrypto 공급자 검사를 함께 한다.

원시 키는 SecretValue를 빌려 32바이트인지 검사하고 67바이트 용량의 Zeroizing<String>에 x'·64자리 hex·'만 넣는다. cipher_memory_security를 먼저 켜고 읽어 확인한 뒤 rusqlite의 안전한 Connection::key API로 sqlite3_key_v2에 전달한다. SQL 문장을 만들지 않아 파서·VDBE·lookaside에 키 문자열 사본을 만들지 않는다. 모든 SQL 준비를 거부하는 authorizer 아래에서도 키 전달이 성공하는 회귀를 둔다. 실제 배포 소스에서 sqlite3_key_v2의 키 사본은 sqlcipher_cipher_ctx_set_pass → sqlcipher_malloc으로 할당되고, 키 유도 후 sqlcipher_free로 덮어쓰고 해제함을 확인했다. Rust 조립 버퍼도 해제 때 지운다. 비밀 소유자는 조립 루트이며 연결을 연 뒤 SecretValue를 해제한다. Database Debug와 오류에는 외부 오류·SQL·키를 담지 않는다.

키 길이 오류는 SecretIntegrity, 설정 불일치·잘못된 키·손상·내부 코드 실패는 RepositoryIntegrity다. SQLite PERM·READONLY·AUTH는 PermissionDenied, BUSY·LOCKED·NOMEM·INTERRUPT·IOERR·FULL·CANTOPEN·PROTOCOL은 Unavailable(Unknown)이다. 다만 읽기 전용 콜백이 쓰기를 시도해서 받은 기본 SQLITE_READONLY는 내부 API 계약 위반으로 RepositoryIntegrity다. 복구 권한 부족 등의 확장 READONLY 코드는 PermissionDenied를 유지한다. SQLite를 열기 전에 키 길이를 검증하고 표준 파일 API로 생성·쓰기 권한을 확인한다. 쓰기 경로의 부모가 디렉터리인지, 쓰기 권한이 있는지 먼저 확인하고 새 DB는 소유자 전용 0600으로 생성한다. 기존 파일은 절단하지 않는다. 연결 직후 is_readonly로 요청한 접근 모드와 일치하는지 확인해 SQLite의 읽기 전용 대체 열기를 거부한다. 디렉터리를 DB로 지정하거나 부모 구성요소가 파일이면 저장소 배치의 불일치로 RepositoryIntegrity다. 부모 부재는 분리된 볼륨 등 일시 상태일 수 있어 Unavailable을 유지하며, 폴더 자동 생성은 하지 않는다. 알려진 앱 데이터 경로 준비는 S-15 조립 단계의 책임이다. 시작 시 접근 확인이 이후 권한 변경·디스크 부족·모든 실제 쓰기의 성공을 보장하지 않으므로 매 작업의 오류 처리를 유지한다. 스레드 생성의 시스템 오류도 권한과 가용성을 구별한다. 패닉·쓰기 요청 채널 종료는 내부 실패로 닫고 읽기 작업의 런타임 취소는 Unavailable로 구별한다. rusqlite 0.42.0의 기본 busy_timeout은 5000ms이며 즉시 실패한다는 1회차 리뷰 설명은 기각했다.

오류 분류는 경계 어댑터에서 SQLite extended_code의 하위 8비트를 ffi의 기본 상태 상수와 비교하고, macOS OS 오류 번호를 닫힌 SystemFailure로 변환한다. 설치된 SDK의 sys/errno.h에서 EPERM=1·EACCES=13·EROFS=30·ENOENT=2를 확인했다. 이 세 OS 오류는 PermissionDenied, ENOTDIR=20·EISDIR=21은 저장소 배치 불일치로 RepositoryIntegrity, 다른 OS 오류와 OS 번호 없는 오류는 Unavailable이다. 원시 코드는 이 어댑터에만 있고 포트에는 노출하지 않는다. 비소진 외부 열거형의 미공개 변형과 완전 매칭·중복 분기 검사 충돌은 네이티브 상태 해석으로 해결하며 매크로·검사 예외를 추가하지 않는다. 테스트 입력의 1·13·30은 제품 상수를 참조하지 않는 독립 기준값이다.

DB를 열기 전에 -wal·-shm 경로도 확인한다. 존재하면 심볼릭 링크가 아닌 정규 파일이어야 하며, 디렉터리 등 다른 종류는 RepositoryIntegrity다. 선택적인 부속 파일의 ENOENT만 정상 부재로 처리하고, 권한·기타 I/O 실패는 기존 분류를 유지한다. SQLite가 관리하는 부속 파일을 미리 만들거나 삭제하지 않는다. 이 검증은 열기 시점의 배치 확인이며 이후 파일 상태 변화의 모든 쓰기 성공을 보장하지 않는다.

쓰기 연결 하나는 생성 직후 전용 스레드에 이동하고 이후 쓰기·트랜잭션은 모두 그 스레드에서 수행한다. FIFO mpsc 채널과 요청별 Tokio oneshot으로 응답한다. 패닉으로 스레드가 끝나면 현재·대기 요청의 응답 채널이 닫히고 이후 전송도 실패한다. Drop은 Stop 요청과 join으로 닫는다. 종료는 진행 중인 작업을 마친 뒤 이뤄지며 무한히 실행되는 내부 작업의 선점은 제공하지 않는다.

문서에 읽기 수가 별도로 지정되어 있지 않아 읽기 전용 연결 하나를 유지한다. 읽기는 SQLITE_OPEN_READ_ONLY와 query_only=ON을 함께 적용하고 Tokio의 비동기 mutex로 직렬화한 뒤 spawn_blocking에서 실행한다. 콜백 전체를 지연 읽기 트랜잭션으로 감싸 첫 조회 이후 같은 스냅샷을 유지한다. 두 조회 사이에 쓰기를 커밋하는 회귀로 고정한다. 기존 잠금 파일의 Tokio =1.54.1에 rt·sync만 켠다. 네트워크·프로세스 기능을 직접 켜지 않는다. 일반 read/write 요청은 코어 스레드가 DB 응답을 동기 대기하지 않는다. 최종 Database 소유권 해제의 Drop은 Stop 뒤 join으로 수락한 쓰기를 모두 기다리는 동기 종료다. 따라서 U-01 수명 주기 조립과 S-15 DB 열기·교체에서는 최종 소유권을 blocking 작업 등 런타임 스레드 밖에서 해제해야 한다. 비동기 종료 API를 이번에 추가하지 않는다.

응답 future 취소는 이미 수락한 쓰기를 취소하지 않는다. FIFO 장벽 뒤 기록이 남는 회귀로 이 계약을 고정한다. 후속 저장소는 결과 미수신을 실패 응답으로 취급하지 않으며 재시도·중복 기록 계약을 정의한다. 결과 수신자가 사라진 oneshot 전송 실패와 이미 종료한 쓰기 스레드의 Stop 전송 실패는 새 사용자 오류를 만들지 않는다. Drop의 join 패닉은 재전파하지 않으며 현재·대기·후속 요청의 닫힌 오류로 관측된다. 별도 종료 진단은 남은 사소 지적으로 기록한다.

공개 API는 DatabasePath::new·as_path, Database::open·check다. 뒤 저장소 구현용 read와 write는 crate 내부 전용이며 외부에는 임의 SQL·Connection·Transaction을 받는 실행 API를 공개하지 않는다. SQL 문장은 src/sql 아래 상수에 둔다. 키 바이트 표현의 x'·' 구분자는 SQL 문장이 아니므로 connection.rs에 둔다. S-20의 향후 파서·실행 인자 검사는 이번에 구현하지 않는다.

| PRAGMA                 | 적용·기대값                      | 확인                                             |
| ---------------------- | -------------------------------- | ------------------------------------------------ |
| key API                | x'…'로 인코딩한 32바이트 원시 키 | 독립 PRAGMA key 열기와 sqlite_schema 읽기로 확인 |
| cipher_provider        | commoncrypto                     | 문자열 일치                                      |
| cipher_memory_security | ON                               | 문자열 1                                         |
| cipher_use_hmac        | 기본 ON                          | 문자열 1                                         |
| cipher_page_size       | 기본 4096                        | 문자열 4096                                      |
| temp_store             | 2                                | 정수 2                                           |
| journal_mode           | WAL                              | 적용 결과와 재조회가 wal                         |
| foreign_keys           | ON                               | 정수 1                                           |
| synchronous            | FULL                             | 정수 2                                           |
| query_only             | 읽기 연결 ON                     | 정수 1                                           |

SQLCipher 관련 숫자 PRAGMA는 실제 드라이버에서 TEXT로 반환하므로 문자열로 검증한다. 일반 SQLite 숫자 PRAGMA와 구분한다. 암호화된 DB와 열린 WAL의 바이트를 검사하며, 테이블은 테스트용이고 제품 스키마·마이그레이션은 추가하지 않는다.

## 의존성

| 패키지                      | 정확한 버전                                               | 라이선스                                    | 근거                                  |
| --------------------------- | --------------------------------------------------------- | ------------------------------------------- | ------------------------------------- |
| fallible-iterator           | 0.3.0                                                     | MIT OR Apache-2.0                           | rusqlite 필수 전이 또는 직접 드라이버 |
| fallible-streaming-iterator | 0.1.9                                                     | MIT OR Apache-2.0                           | rusqlite 필수 전이 또는 직접 드라이버 |
| libsqlite3-sys              | 0.40.0                                                    | MIT                                         | rusqlite 필수 전이 또는 직접 드라이버 |
| rsqlite-vfs                 | 0.2.0                                                     | MIT                                         | rusqlite 필수 전이 또는 직접 드라이버 |
| rusqlite                    | 0.42.0                                                    | MIT                                         | rusqlite 필수 전이 또는 직접 드라이버 |
| sqlcipher-src               | 420.0.0+sqlcipher-4.20.0-sqlite-3.54.1-libtomcrypt-1.18.2 | MIT AND BSD-3-Clause AND blessing AND WTFPL | rusqlite 필수 전이 또는 직접 드라이버 |
| sqlite-wasm-rs              | 0.6.3                                                     | MIT                                         | rusqlite 필수 전이 또는 직접 드라이버 |
| vcpkg                       | 0.2.15                                                    | MIT OR Apache-2.0                           | rusqlite 필수 전이 또는 직접 드라이버 |

기존 패키지의 직접 의존 추가: application·Tokio =1.54.1(rt, sync)·zeroize =1.9.1(alloc). 테스트는 표준 라이브러리와 제품 의존성만 사용하며 불필요한 testkit 개발 의존성은 추가하지 않는다. deny의 기존 취약점·출처 정책은 유지한다. sqlcipher-src에만 blessing·WTFPL 라이선스를 등록하고 vcpkg 0.2.15 배포본의 셸 파일 세 개는 실제 SHA-256으로 한정 등록한다. libsqlite3-sys 빌드 스크립트와 기존 잠금의 pkg-config 0.3.34(MIT OR Apache-2.0)의 이번 사용도 등록한다. arch-check의 SQLite 위치 제한은 store-sqlite에만 유지한다. rusqlite·Tokio의 절차 매크로 통합과 Tokio의 Tauri 기능 통합으로 보이는 socket2 경로는 해당 경유 경로만 등록한다. store-sqlite는 기존 소켓 API 금지 규칙의 대상에도 추가하며 tokio::net 직접 경로와 Tokio 별칭의 lookup_host, 표준 DNS trait 가져오기·glob 메서드 반례를 검사한다. 이는 확인된 식별자 경로의 보완이며 임의 별칭을 모두 추적하는 AST 분석 완성을 뜻하지 않는다.

주간·일일 변이 카탈로그는 CommonCrypto 전용 crate를 포함하므로 두 실행 작업을 macos-15로 옮긴다. 카탈로그·128분할·일일 계획·시간 제한·결과 집계는 유지한다. Linux용 unshare 대신 기존 nextest의 macOS network-sandbox wrapper가 모든 테스트에 적용된다. Linux 일반 검사는 그대로 Linux에서 수행한다. workflow 정책의 정확한 명령·스냅샷과 기존 명령 기대값 한 곳을 기계적으로 맞추고 새 플랫폼 계약 테스트를 둔다. 단계 이름은 mutants with sandboxed tests로 정정해 테스트 실행 파일과 자식 프로세스의 네트워크 차단 범위를 표시한다. 빌드 전체의 차단은 harness 6.2의 확인된 계약이 아니다. CI 러너 실제 실행은 로컬 검증으로 주장하지 않는다.

대안은 기존 세 crate를 Linux 집합, store-sqlite를 macOS 집합으로 나누는 것이다. 이 경우 러너뿐 아니라 baseline·테스트 패키지 선택·카탈로그 합집합·비중복·집계를 함께 나눠야 한다. 현재는 단일 카탈로그 경로를 유지해 구조를 단순하게 둔다. 공개 저장소 표준 러너는 무료이며 실제 계정의 동시 실행 상향·대기시간·실행시간은 확인하지 않았다. 대기열 지연과 각 작업의 실행 제한 초과는 별개다. H-47에 실제 완료 측정과 필요시 두 집합 분리를 인계한다. [GitHub 한도](https://docs.github.com/en/actions/reference/limits), [과금 정책](https://docs.github.com/en/billing/concepts/product-billing/github-actions).

## 폐기한 고정과 해석 증거

0.41.1은 RUSTSEC-2026-0502 때문에 쓰지 않는다. 이전 조합은 rusqlite =0.41.1 → libsqlite3-sys 0.39.1 → sqlcipher-src =419.0.6이었다. cargo deny가 CVE-2026-119375를 거부했다. 별도 복사본에서 cargo update --offline -p sqlcipher-src --precise 420.0.0을 실행했지만 Cargo는 정확한 =419.0.6 요구와 맞지 않는다며 종료 101을 반환했다. 원본 잠금은 바꾸지 않았다. 메인이 0.41.1 고정을 폐기한 뒤 수정된 번들을 포함하는 가장 낮은 정식 릴리스 0.42.0으로 올렸다.

공식 근거는 https://rustsec.org/advisories/RUSTSEC-2026-0502.html 및 실제 받은 crate의 매니페스트·빌드 스크립트다. 해석 실패 로그는 target/S-01/resolution.log와 update-042.log다.

## 승인된 등록과 기존 검사 갱신

| 파일·검사                                                              | 변경 이유·정확한 전 → 후                                                                                             | 유지한 경계                                                 |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| deny.toml                                                              | 승인된 패키지·라이선스·빌드 스크립트·정확한 해시 등록                                                                | 취약점 무시 없음                                            |
| arch-check/rules.toml·vendor·사례                                      | SQLite 패키지의 위치·기능 통합 경로와 store-sqlite 소켓 금지와 lookup_host·ToSocketAddrs·to_socket_addrs 식별자 등록 | SQLite는 store-sqlite에만, 소켓 반례와 규칙 요소 삭제 검사  |
| source-checks/allowlists.toml                                          | 새 tests.rs의 thread::current·temp_dir만 등록                                                                        | 제품 파일로 확대하지 않음                                   |
| .cargo/mutants.toml·docs/harness.md                                    | store-sqlite를 네 번째 변이 대상으로 등록                                                                            | 기존 세 대상 유지                                           |
| source_check/cargo_config_tests.rs                                     | MUTATION_TARGET_GLOBS 3→4                                                                                            | 기존 assert_eq! 유지                                        |
| source_check/tests.rs                                                  | clock-environment 경로 6→7                                                                                           | 기존 동등 단언·나머지 목록 유지                             |
| ci-plumbing.test.mjs                                                   | macOS 제외 crate 4→5, 인자 8→10 및 변이 실행 명령 갱신                                                               | 정확한 deepEqual·includes 단언 유지                         |
| h47-nightly.test.mjs·h47-plan.test.mjs                                 | 명시적 --file 패턴 3→4, 정확한 단계 이름 갱신                                                                        | 128분할 전체 합집합·비중복 단언 유지                        |
| h42-nightly.test.mjs·mutation-fail-fast.workflow                       | 예전 Linux 변이 명령·러너를 macOS의 정확한 값으로 갱신                                                               | 기존 some/equality 단언과 의도적인 fail-fast=true 반례 유지 |
| nightly.yml·workflow-policy/policy.json                                | CommonCrypto 변이의 macOS 배치·정확한 명령·스냅샷                                                                    | 분할·시간 제한·결과 처리·네트워크 차단 유지                 |
| justfile·check-list                                                    | sqlcipher 단계·사례·스냅샷 등록                                                                                      | 전체·fast 정적·Linux 그래프 검사에 등록                     |
| .codex/config.toml·sandbox/lock-fragment.json·claude-hooks/hashes.json | 추적 경로·보호 파일의 기존 스크립트 재생성                                                                           | 생성물을 손으로 수정하지 않음                               |

제품 소스에 주석·검사 끄기를 추가하지 않았다. 3회차 비판의 Cargo.toml EOF 빈 줄도 제거했다. 테스트용 DB·WAL·SHM·빈 dist는 추적하지 않는다. 자동 변이의 frontendDist 준비는 기존 just frontend-build 절차를 사용한다.

## 검증과 한계

프로젝트 신뢰와 훅 정의 신뢰·현재 세션 적용은 각각 확인 불가다. 시작 pnpm frozen install·lefthook install은 통과했고 Git 훅이 실제 실행됐다. 독립 리뷰와 비판, 실행 한계 및 최종 검사 값은 docs/reviews/S-01.md에 기록한다. Linux CI 실제 실행과 실제 키체인·토스 계좌·API는 검사하지 않았다.

회귀 우선으로 SQL 파서 키 거부, 권한 000 디렉터리, 읽기 중간 커밋의 세 테스트가 수정 전 모두 실패했다. 수정 뒤 스냅샷·원시 키 독립 열기·쓰기 PRAGMA·오류 분류·취소 계약을 검사한다. 잘못된 키와 평문 DB는 닫힌 오류이며 DB와 열린 WAL의 알려진 표식 부재도 확인한다.

최초 구현 94c85d41b5056f3653706a80b7c20812f1ee5fd9의 자동 변이는 25개 중 유효 12 검출·컴파일 불가 13·생존 및 시간 초과 0이었다. 최신 master 병합 22c37d3743e13f05a0893df51d23e87dc0d02030 이후 1회차 수정분은 자동 변이 23개 중 유효 8 검출·컴파일 불가 15·생존 및 시간 초과 0이었다. A-21 병합 뒤 옛 기준을 사용한 실행은 병합으로 받은 FakeBroker 구현 변경 때문에 test-diff가 거부했으며 성공으로 세지 않았다. 2회차 수정분은 07f0c74ac87bc1e2b0f0fa6208b40e30a1d70d5a 기준 자동 변이 9개 중 유효 7 검출·컴파일 불가 2·생존 및 시간 초과 0이었다. 3회차 추가 수정은 4384050b6771d3eb62aa4fbd5d9b53e435a7469d 기준 자동 변이 3개 모두 검출·컴파일 불가와 생존 및 시간 초과 0이었다. 최초 선택 0과 준비 실패·중단 실행은 성공 집계에 넣지 않는다. 어댑터 경로에 대한 files=0·crates/adapters 표시는 하네스의 표시 한계이며 실제 카탈로그·실행 결과와 구분한다.

최신 master dc8f3f950294e9d3d1947b39c256868076d7de15를 --no-ff로 받아 22c37d3743e13f05a0893df51d23e87dc0d02030을 만들었다. 정상 병합 훅은 전체 회귀 1419개와 커버리지 1048개 및 앞선 모든 검사를 통과한 뒤 승인된 test-diff 두 건만 보고했다. 뒤 단계 disable-report·disable-report-since·hooks-installed도 별도로 통과했다. 사용자 지시의 명시적 예외에 따라 이 병합 커밋에만 LEFTHOOK=0을 적용하고 메시지에 사유를 기록했다. 첫 구현 커밋 94c85d4도 같은 승인 두 건만의 예외였다. 코드·테스트·하네스 규칙을 끄는 예외는 추가하지 않았다.

3회차 Opus는 차단 0·중요 0·새 사소 2건, 새 Codex는 부속 파일 충돌을 중요로 올려 차단 0·중요 1·사소 1건으로 판단했다. WAL·SHM 디렉터리 충돌의 회귀 두 개는 수정 전 모두 실패했고, 열기 전 부속 파일 배치 검증으로 수정했다. 최종 소유권 해제는 U-01·S-15에 인계한다. 브리프의 최대 3회차 한도에 따라 이 추가 중요 수정 83a0109c2186be3aa34f4b7388917b2c6d605e6b 뒤 새 독립 재검토는 미실행이며, 작업 완료로 판정하지 않는다. 정상 훅은 전체1479·core coverage1058 통과 후 승인된 test-diff 정확 목록 두 건만 보고했고 뒤 단계도 통과했다. 원 비판 탐침의 두 충돌을 구현 담당이 다시 실행해 RepositoryIntegrity로 교정됐음을 확인했다. 이 실행은 새 독립 리뷰가 아니다.

## 손 변형 검증

최종 제품 소스를 mktemp 복사본으로 옮겨 각 변형의 라이브러리 컴파일과 테스트를 따로 실행한다. 컴파일되지 않은 변형은 검출 수에 넣지 않는다. 원본·복원 대조군도 검사한다. 최종 복사본에서 유효 38개 모두 검출·생존 0·컴파일 불가 0이고 원본·복원 대조군이 통과했다. ENOTDIR 원시 코드의 첫 변형이 생존해 독립 입력 20·21의 오류 계약 회귀를 추가하고 당시 32개 전체를 다시 실행했다. 3회차 추가 수정 뒤 부속 파일 코드·두 접미사·파일 종류·심볼릭 링크·오류 분류 변형 여섯 개를 더해 최종 38개 전체를 다시 실행했고, 원본·복원 대조군은 각각 31개 통과했다.

| 번호 | 바꾼 것                                                                                                                                                                                           | 독립 라이브러리 컴파일 | 잡은 테스트                                                                                   | 결과 |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | --------------------------------------------------------------------------------------------- | ---- |
| 1    | 키 길이 31: `!= 32` → `!= 31`                                                                                                                                                                     | 통과                   | tests::one_read_callback_observes_one_snapshot_across_a_writer_commit                         | 검출 |
| 2    | 키 길이 33: `!= 32` → `!= 33`                                                                                                                                                                     | 통과                   | tests::database_secret_and_failure_debug_remain_fixed                                         | 검출 |
| 3    | 상위 nibble 이동: `byte >> 4_u8` → `byte >> 3_u8`                                                                                                                                                 | 통과                   | tests::raw_key_is_independently_readable_and_debug_is_closed                                  | 검출 |
| 4    | 하위 nibble 마스크: `byte & 15` → `byte & 7`                                                                                                                                                      | 통과                   | tests::raw_key_is_independently_readable_and_debug_is_closed                                  | 검출 |
| 5    | hex 진법: `u32::from(nibble), 16` → `u32::from(nibble), 15`                                                                                                                                       | 통과                   | tests::database_secret_and_failure_debug_remain_fixed                                         | 검출 |
| 6    | 숫자 검증 반전: `actual == expected` → `actual != expected`                                                                                                                                       | 통과                   | connection::tests::numeric_setting_verification_rejects_mismatch_and_missing_rows             | 검출 |
| 7    | 문자 검증 반전: `actual == expected` → `actual != expected`                                                                                                                                       | 통과                   | connection::tests::text_setting_verification_rejects_mismatch_and_missing_rows                | 검출 |
| 8    | 공급자 기대값: `sql::PROVIDER, "commoncrypto"` → `sql::PROVIDER, "openssl"`                                                                                                                       | 통과                   | tests::cancelling_a_response_does_not_cancel_an_accepted_write                                | 검출 |
| 9    | TEMP 디스크: `temp_store = 2` → `temp_store = 1`                                                                                                                                                  | 통과                   | connection::tests::numeric_setting_verification_rejects_mismatch_and_missing_rows             | 검출 |
| 10   | TEMP 검증값: `sql::TEMP_STORE, 2` → `sql::TEMP_STORE, 1`                                                                                                                                          | 통과                   | tests::open_contract::regular_sidecar_files_preserve_a_live_encrypted_database                | 검출 |
| 11   | 외래키 끄기: `foreign_keys = ON` → `foreign_keys = OFF`                                                                                                                                           | 통과                   | tests::open_contract::file_preparation_is_owner_only_and_preserves_existing_encrypted_data    | 검출 |
| 12   | WAL 끄기: `journal_mode = WAL` → `journal_mode = DELETE`                                                                                                                                          | 통과                   | tests::open_contract::readonly_database_file_is_rejected_before_a_successful_health_result    | 검출 |
| 13   | 읽기 연결 쓰기 허용: `Access::Read => OpenFlags::SQLITE_OPEN_READ_ONLY` → `Access::Read => OpenFlags::SQLITE_OPEN_READ_WRITE`                                                                     | 통과                   | tests::encrypts_database_and_live_wal_and_rolls_back                                          | 검출 |
| 14   | 쓰기 연결 읽기 전용: `OpenFlags::SQLITE_OPEN_READ_WRITE.union(OpenFlags::SQLITE_OPEN_CREATE)` → `OpenFlags::SQLITE_OPEN_READ_ONLY`                                                                | 통과                   | tests::encrypts_database_and_live_wal_and_rolls_back                                          | 검출 |
| 15   | query_only 끄기: `query_only = ON` → `query_only = OFF`                                                                                                                                           | 통과                   | tests::one_read_callback_observes_one_snapshot_across_a_writer_commit                         | 검출 |
| 16   | 키 메모리 보호 끄기: `cipher_memory_security = ON` → `cipher_memory_security = OFF`                                                                                                               | 통과                   | tests::one_read_callback_observes_one_snapshot_across_a_writer_commit                         | 검출 |
| 17   | HMAC 기대값: `sql::CIPHER_HMAC, "1"` → `sql::CIPHER_HMAC, "0"`                                                                                                                                    | 통과                   | tests::concurrent_producers_keep_fifo_and_use_one_writer_thread                               | 검출 |
| 18   | 페이지 크기 기대값: `sql::PAGE_SIZE, "4096"` → `sql::PAGE_SIZE, "8192"`                                                                                                                           | 통과                   | tests::open_contract::wal_sidecar_directory_is_a_storage_layout_failure                       | 검출 |
| 19   | 내구성 끄기: `synchronous = FULL` → `synchronous = OFF`                                                                                                                                           | 통과                   | tests::open_contract::shm_sidecar_directory_is_a_storage_layout_failure                       | 검출 |
| 20   | 커밋 대신 롤백: `transaction.commit()?;` → `transaction.rollback()?;`                                                                                                                             | 통과                   | tests::cancelling_a_response_does_not_cancel_an_accepted_write                                | 검출 |
| 21   | 외래키 검증값: `sql::FOREIGN_KEYS, 1` → `sql::FOREIGN_KEYS, 0`                                                                                                                                    | 통과                   | tests::encrypted_reopen_and_pragma_contract                                                   | 검출 |
| 22   | 읽기 보호 검증값: `sql::QUERY_ONLY, 1` → `sql::QUERY_ONLY, 0`                                                                                                                                     | 통과                   | tests::concurrent_producers_keep_fifo_and_use_one_writer_thread                               | 검출 |
| 23   | SQLite 기본 오류 마스크: `PRIMARY_CODE_MASK: i32 = 0xff` → `PRIMARY_CODE_MASK: i32 = 0xf`                                                                                                         | 통과                   | tests::database_secret_and_failure_debug_remain_fixed                                         | 검출 |
| 24   | EPERM 경계 코드: `OPERATION_NOT_PERMITTED: i32 = 1` → `OPERATION_NOT_PERMITTED: i32 = 2`                                                                                                          | 통과                   | failure::tests::thread_creation_io_preserves_permission_and_availability                      | 검출 |
| 25   | EACCES 경계 코드: `ACCESS_DENIED: i32 = 13` → `ACCESS_DENIED: i32 = 14`                                                                                                                           | 통과                   | failure::tests::thread_creation_io_preserves_permission_and_availability                      | 검출 |
| 26   | EROFS 경계 코드: `READ_ONLY_FILESYSTEM: i32 = 30` → `READ_ONLY_FILESYSTEM: i32 = 29`                                                                                                              | 통과                   | failure::tests::thread_creation_io_preserves_permission_and_availability                      | 검출 |
| 27   | 새 DB 소유자 권한: `.mode(0o600)` → `.mode(0o644)`                                                                                                                                                | 통과                   | tests::open_contract::file_preparation_is_owner_only_and_preserves_existing_encrypted_data    | 검출 |
| 28   | 기존 DB 절단: `.truncate(false)` → `.truncate(true)`                                                                                                                                              | 통과                   | tests::open_contract::file_preparation_is_owner_only_and_preserves_existing_encrypted_data    | 검출 |
| 29   | 읽기 전용 쓰기 연결 허용: `(Access::Write, true) => Err(failure::permission())` → `(Access::Write, true) => Ok(())`                                                                               | 통과                   | connection::tests::access_verification_rejects_sqlite_readonly_fallback_and_wrong_reader_mode | 검출 |
| 30   | 부모 권한 검사 반전: `metadata.permissions().readonly()` → `!metadata.permissions().readonly()`                                                                                                   | 통과                   | connection::tests::access_verification_rejects_sqlite_readonly_fallback_and_wrong_reader_mode | 검출 |
| 31   | ENOTDIR 경계 코드: `NOT_A_DIRECTORY: i32 = 20` → `NOT_A_DIRECTORY: i32 = 19`                                                                                                                      | 통과                   | failure::tests::storage_layout_io_errors_preserve_integrity_failure                           | 검출 |
| 32   | EISDIR 경계 코드: `IS_A_DIRECTORY: i32 = 21` → `IS_A_DIRECTORY: i32 = 22`                                                                                                                         | 통과                   | failure::tests::storage_layout_io_errors_preserve_integrity_failure                           | 검출 |
| 33   | 부속 파일 부재 코드: `FILE_NOT_FOUND: i32 = 2` → `FILE_NOT_FOUND: i32 = 3`                                                                                                                        | 통과                   | tests::open_contract::file_preparation_is_owner_only_and_preserves_existing_encrypted_data    | 검출 |
| 34   | WAL 경로 접미사: `"-wal"` → `"-wol"`                                                                                                                                                              | 통과                   | connection::tests::sidecar_paths_must_be_owned_regular_files                                  | 검출 |
| 35   | SHM 경로 접미사: `"-shm"` → `"-sum"`                                                                                                                                                              | 통과                   | connection::tests::sidecar_paths_must_be_owned_regular_files                                  | 검출 |
| 36   | 부속 파일 종류 검증 반전: `if !metadata.is_file()` → `if metadata.is_file()`                                                                                                                      | 통과                   | connection::tests::sidecar_paths_must_be_owned_regular_files                                  | 검출 |
| 37   | 부속 심볼릭 링크 허용: `std::fs::symlink_metadata(&sidecar)` → `std::fs::metadata(&sidecar)`                                                                                                      | 통과                   | connection::tests::sidecar_paths_must_be_owned_regular_files                                  | 검출 |
| 38   | 부속 배치 실패를 가용성 오류로 변경: `if !metadata.is_file() {                     return Err(integrity());` → `if !metadata.is_file() {                     return Err(failure::unavailable());` | 통과                   | connection::tests::sidecar_paths_must_be_owned_regular_files                                  | 검출 |

## 인계

- H-47: macOS 변이의 실제 완주·대기시간을 측정하고 필요시 플랫폼 두 집합 분리를 검토한다.

- S-15: 마이그레이션 실행기·원장·스키마 검증을 DB 열기 절차에 통합. 조립 단계에서 앱 데이터 폴더를 준비하고 열기·교체 시 최종 DB 소유권을 런타임 스레드 밖에서 해제한다.
- U-01: 마지막 DB 소유권의 종료 위치를 정하고 수락한 쓰기 완료와 런타임 응답성을 함께 검증한다. 동기 Drop의 독립 탐침에서 두 쓰기 완료와 약 404ms 런타임 지연이 재현됐으며 실제 UI 지연은 미측정이다.

- S-02·S-03 이후: 내부 read·write API로 SQL 상수와 도메인 매핑을 구현. 응답 취소를 실패로 취급하지 않고 재시도·중복 기록 계약을 정의.
- S-16: 감사 로그 HMAC·키체인 머리값 검증.
- S-20: SQL 상수 파서와 실행 인자 검사 구조 통합.
- 후속 하네스 작업: 어댑터 변이 경로 집계 표시 개선. 현재 실제 변이 선택·실행 결과와는 별개다.
