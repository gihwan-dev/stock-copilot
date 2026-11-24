# ADR M-01 초안: MCP 서버 뼈대, 실행별 Unix 소켓, 토큰 인증

- 상태: 초안 (사용자·메인 확인 대기)
- 날짜: 2026-11-24
- 결정한 사람: 구현 담당 초안(메인 A-1·B-1·C-1 및 1-A·2-A·3-A 승인 반영)
- 관련 작업: M-01 (연관 O-07, M-02~M-07, H-64, H-68, H-70)
- 반영한 파일: `crates/application/src/peer_process.rs`, `crates/application/src/lib.rs`, `crates/testkit-ports/src/system_fakes.rs`, `crates/mcp-server/**`, `deny.toml`, `harness/arch-check/rules.toml`, `harness/source-checks/allowlists.toml`, `crates/xtask/src/source_check/tests.rs`, `.cargo/mutants.toml`, `crates/xtask/src/source_check/cargo_config_tests.rs`, `crates/xtask/src/mutants_check/test_packages.rs`, `harness/ci/diff-mutants-plan.mjs`, `harness/ci/tests/h47-nightly.test.mjs`, `Cargo.lock`

## 맥락

설계(SEC-3, R-04, R-16, ADR 0002)는 앱 내부 MCP 서버를 TCP/HTTP 포트가 아닌 로컬 Unix 도메인 소켓(`0700` 폴더 안 `0600` 소켓, 경로 104바이트 이내)으로만 열고, 실행마다 추측 불가능한 토큰과 상대 프로세스 검증(`uid`·`pid`·부모 PID·실행 파일 경로)을 겹쳐 보호하도록 정했다.

이번 M-01은 도구 등록(M-02+)이나 stdio 브리지(M-07)를 당기지 않고, 아래 경계를 먼저 고정하는 최소 뼈대를 구현한다.

- 공식 SDK `rmcp =3.9.0`을 최소 기능(`default-features = false`, `server` + `transport-async-rw`)만으로 연결해 MCP `2026-07-28`(`server/discover`)과 `2025-06-18`(`initialize`) 두 세대를 모두 지원한다.
- TCP·HTTP 리스너 의존성을 전혀 들이지 않고, Unix 소켓만 연다.
- `mcp-server`는 `unsafe`·`cfg` 분기·외부 프로세스 실행을 쓰지 않는다. 상대 프로세스의 부모 PID·실행 파일 경로 조회는 `application::PeerProcessInspector` 플랫폼 중립 포트로 분리하고 닫힌 실패를 강제한다.

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `rmcp = { version = "=3.9.0", default-features = false, features = ["server", "transport-async-rw"] }`만 사용 | ADR 0002와 작업 행이 확정한 공식 SDK. `2026-07-28`과 `2025-06-18`을 모두 처리하면서 `reqwest`·`hyper`·`axum`·`tower`·`http`·`sse-stream`·`rmcp-macros`를 모두 배제한다 | 기본 기능(`macros`, `base64`, `server`) 유지; Streamable HTTP 서버(`127.0.0.1`) | ADR 0002, R-04, 메인 A-1 |
| `application::PeerProcessInspector` 플랫폼 중립 포트 신설 + `PeerInspectorMode` 닫힌 실패 + 부모 PID 사후 1회 결합(`PeerProcessTarget::pending_parent` → `McpServerHandle::bind_expected_parent_pid`) | `mcp-server`에 `unsafe`·`cfg` 분기·`ps` 같은 외부 프로세스 실행을 금지하고, 포트 미제공·부모 PID 미결합·조회 실패·제한 시간 초과·부모 PID 불일치·실행 파일 경로 불일치 모두 연결을 즉시 거부한다. `architecture.md` 2.3 순서(토큰·소켓 생성 → CLI 기동 → CLI PID 확보)를 깨지 않도록 1회 한정 결합을 제공한다 | `mcp-server` 내부 `libc`/`libproc` FFI; `ps` 실행(`source.process` 위반); `uid`만 확인; 시작 시점 부모 PID 고정 강제 | 메인 B-1, R-16, M-01 1차 리뷰 지적 2 |
| `0700` 실행별 폴더를 `DirBuilder::new().mode(0o700).create`로 원자 생성 후 `0600` 소켓 바인딩 + 조상 전체 심볼릭 링크·권한·소유자 검사 | 기존 폴더나 비정상 종료 후 남은 소켓이 있으면 `AlreadyExists`로 즉시 거부(`LeftoverPathRefused`)해 임의 삭제·재사용을 막는다. 기반 폴더와 모든 조상 경로에서 심볼릭 링크, sticky 없는 그룹/타인 쓰기(`0o022`), 본인/루트 외 소유자를 거부해 `rename` 가로채기를 막는다 | `create_dir_all` + 기존 소켓 `remove_file` 후 재바인딩; 마지막 폴더만 심볼릭 링크 확인; `libc::getuid` 호출 | SEC-3, R-04, R-16, M-01 1차 리뷰 지적 4 |
| 256비트 OS CSPRNG 토큰(`getrandom`)을 `application::SecretValue`로 감싼 `RunAuthToken`으로 보관하고 `subtle::ConstantTimeEq`로 비교 | `Display`·`Serialize`·`Clone` 부재 및 고정 `Debug` 출력(`RunAuthToken(<redacted>)`)으로 로그·오류 노출을 차단한다. 인증 첫 줄은 1바이트씩 상한(`AuthLineLimit`)과 제한 시간(`auth_timeout`) 안에서 `Zeroizing<Vec<u8>>`에만 읽어 비교 직후 지우며, 파이프라인된 JSON-RPC 바이트를 잃지 않는다 | `BufReader::read_line`(무제한 할당·내부 8KiB 버퍼에 토큰 잔류); `String` 평문 비교(`==`) | SEC-3, SEC-10, R-04, R-16 |
| 만료·폐기 검사를 `ExpiryGuardedReader`/`ExpiryGuardedWriter` 전송 경계와 유휴 만료 감시(`wait_for_expiry`), `ServerHandler` 요청, 피어·인증 대기에 모두 적용하고 `McpServerHandle`만 공개 진입점으로 축소, `RunKind::Collection` 거부 | `rmcp` 초기화 전·`discover` 이후 `ping`과 기본 알림·유휴 슬롯 점유까지 만료·폐기 시 즉시 차단한다. 내부 핸들러·스트림 함수를 `pub(crate)`로 닫아 TCP나 피어/인증 우회 호출을 컴파일 단계에서 막는다 | 인증 시점 1회만 만료 검사; `ServerHandler` 일부 메서드만 검사; 내부 스트림 함수 `pub` 공개 | M-01 완료 기준, design 3.1, architecture 2.2·2.4, M-01 1차 리뷰 지적 3·6 |
| `mcp-server`를 `.cargo/mutants.toml` 자동 변이 대상에 편입하고 `test_packages.rs`·`cargo_config_tests.rs`·`diff-mutants-plan.mjs`·`h47-nightly.test.mjs`를 동기화 | 권한 경계 코드의 회귀를 자동 변이로 감시한다. `crates/mcp-server/tests/server_security.rs`에만 `clock-env:temp_dir`을 허용해 macOS 104바이트 소켓 경로 한도 안의 짧은 경로를 테스트한다 | 자동 변이 제외; 작업 트리 안 긴 경로 사용 | 메인 C-1, 3-A |

## 추가·활성화된 외부 패키지 표

### `Cargo.lock` 신규 추가 및 `deny.toml` `bans.allow` 등록 목록

| 패키지 | 버전 | 도입 경로 | 라이선스 | `Cargo.lock` 신규 | `deny.toml` `bans.allow` 추가 | `build.rs` 여부 및 하는 일 |
|---|---|---|---|---|---|---|
| `rmcp` | `3.9.0` | `mcp-server -> rmcp` | `Apache-2.0` | 예 | 예(`rmcp@3.9.0`) | 있음 (`allow-build-scripts` 등록, sha256 `31b905abf91292bab290271e993e347e06a1944e6305f742f69dbd01e6b89349`). `CARGO_MANIFEST_DIR/../..`에 `.githooks`와 `.git`이 모두 있을 때만 `git config core.hooksPath .githooks`를 실행하므로 레지스트리 의존 빌드에서는 무동작 |
| `chrono` | `0.4.45` | `rmcp -> chrono`, `rmcp -> schemars -> chrono` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`chrono@0.4.45`) | 없음 |
| `const-oid` | `0.10.3` | `rmcp -> sha2 0.11.1 -> digest 0.11.3 -> const-oid` | `Apache-2.0 OR MIT` | 예 | 예(`const-oid@0.10.3`) | 없음 |
| `futures` | `0.3.34` | `rmcp -> futures` | `MIT OR Apache-2.0` | 예 | 예(`futures@0.3.34`) | 없음 |
| `futures-executor` | `0.3.34` | `rmcp -> futures -> futures-executor` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`futures-executor@0.3.34`) | 없음 |
| `futures-io` | `0.3.34` | `rmcp -> futures -> futures-io` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`futures-io@0.3.34`) | 없음 |
| `futures-macro` | `0.3.34` | `rmcp -> futures -> futures-util -> futures-macro` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`futures-macro@0.3.34`) | 없음 (proc-macro) |
| `futures-sink` | `0.3.34` | `rmcp -> futures / tokio-util -> futures-sink` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`futures-sink@0.3.34`) | 없음 |
| `hybrid-array` | `0.4.15` | `rmcp -> sha2 0.11.1 -> digest 0.11.3 -> block-buffer/crypto-common -> hybrid-array` | `MIT OR Apache-2.0` | 예 | 예(`hybrid-array@0.4.15`) | 없음 |
| `pastey` | `0.2.3` | `rmcp(server) -> pastey` | `MIT OR Apache-2.0` | 예 | 예(`pastey@0.2.3`) | 없음 (proc-macro; `mcp-server` 직접 사용 금지) |
| `ref-cast` | `1.0.27` | `rmcp -> schemars 1.2.3 -> ref-cast` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`ref-cast@1.0.27`) | 있음 (`allow-build-scripts` 등록, sha256 `095bc1870b75dd5c80a68bddaae03d2de9f817e134b026ebf3f63e0ff81cf5ca`). `OUT_DIR/private.rs` 생성 및 `rustc --version`으로 버전 cfg 출력만 수행 |
| `ref-cast-impl` | `1.0.27` | `rmcp -> schemars 1.2.3 -> ref-cast -> ref-cast-impl` | `MIT OR Apache-2.0` | 아니오(기존 잠금) | 예(`ref-cast-impl@1.0.27`) | 없음 (proc-macro) |
| `tokio-macros` | `2.7.2` | `rmcp -> tokio(macros) -> tokio-macros` | `MIT` | 예 | 예(`tokio-macros@2.7.2`) | 없음 (proc-macro) |
| `tokio-util` | `0.7.20` | `rmcp(transport-async-rw) -> tokio-util` | `MIT` | 아니오(기존 잠금) | 예(`tokio-util@0.7.20`) | 없음 |
| `tracing-attributes` | `0.1.31` | `rmcp -> tracing -> tracing-attributes` | `MIT` | 예 | 예(`tracing-attributes@0.1.31`) | 없음 (proc-macro) |
| `block-buffer` | `0.12.1` | `rmcp -> sha2 0.11.1 -> digest 0.11.3 -> block-buffer` | `MIT OR Apache-2.0` | 예(추가 버전) | 이미 허용됨 | 없음 |
| `crypto-common` | `0.2.2` | `rmcp -> sha2 0.11.1 -> digest 0.11.3 -> crypto-common` | `MIT OR Apache-2.0` | 예(추가 버전) | 이미 허용됨 | 없음 |
| `digest` | `0.11.3` | `rmcp -> sha2 0.11.1 -> digest` | `MIT OR Apache-2.0` | 예(추가 버전) | 이미 허용됨 | 없음 |
| `schemars_derive` | `1.2.3` | `rmcp -> schemars 1.2.3 -> schemars_derive` | `MIT` | 예(추가 버전) | 이미 허용됨 | 없음 (proc-macro) |
| `serde_derive_internals` | `0.30.0` | `rmcp -> schemars 1.2.3 -> schemars_derive -> serde_derive_internals` | `MIT OR Apache-2.0` | 예(추가 버전) | 이미 허용됨 | 없음 |
| `sha2` | `0.11.1` | `rmcp -> sha2` | `MIT OR Apache-2.0` | 예(추가 버전) | 이미 허용됨 | 없음 |

### `RUSTSEC-2026-0543` (`chrono`) 권고 무시 사유와 다시 볼 조건

- **무시 사유**: `rmcp =3.9.0`이 `chrono ^0.4.38`에 무조건(`optional = false`) 의존하고 `schemars =1.2.3`도 `chrono =0.4.45`에 의존하므로 기능 비활성화로 제거할 수 없다. `RUSTSEC-2026-0543`은 유지 중단 권고(`unmaintained`)일 뿐 보안 취약점이나 unsound 권고가 아니며, `mcp-server` 코드는 `chrono`를 직접 의존하거나 호출하지 않는다(시각 판정은 모두 `application::Clock`과 `domain::Timestamp`를 사용한다).
- **다시 볼 조건**:
  1. `rmcp` 후속 버전이 `chrono` 의존을 제거(`time`/`jiff` 전환 또는 선택 기능화)하면 즉시 `rmcp` 업그레이드를 검토하고 `RUSTSEC-2026-0543` 예외와 `chrono@0.4.45` 허용을 제거한다.
  2. `chrono`에 보안 취약점(`vulnerability`) 또는 `unsound` 권고가 새로 등록되면 예외를 유지하지 않고 즉시 대응한다.

## 정의 대기

문서가 아직 확정하지 않은 정책 세부는 아래의 보수적 기본값으로 구현했다.

| 항목 | 현재 안전한 기본값 | 선택지 | 사용자·운영 영향 |
|---|---|---|---|
| 1. 인증 첫 줄 최대 길이 (`AuthLineLimit`) | `256`바이트(유효 범위 `70..=1024`; `"AUTH "` 5 + 64 hex + `\n` 1 = 70) | A) `256`바이트 유지 / B) 정확히 `70`바이트 고정 | A는 향후 인증 헤더 확장 여유를 두면서도 메모리 남용을 막는다. B는 프로토콜 길이를 1바이트 오차도 없이 고정한다. |
| 2. 인증 첫 줄 제한 시간 (`auth_timeout`) | `5`초(상한 `30`초) | A) `5`초 유지 / B) `1~2`초로 단축 | A는 느린 초기 기동 환경의 거짓 실패를 줄인다. B는 지연 연결 점유 시간을 더 줄인다. |
| 3. 상대 프로세스 조회 제한 시간 (`peer_inspection_timeout`) | `2`초(상한 `10`초) | A) `2`초 유지 / B) `500ms`로 단축 | A는 OS 프로세스 테이블 일시 지연에도 안정적이다. B는 조회 지연 시 더 빨리 닫힌 실패한다. |
| 4. 실행당 최대 동시 연결 수 (`MaxConcurrentConnections`) | `4`(유효 범위 `1..=16`) | A) `4` 유지 / B) `1`(실행당 브리지 단일 연결 엄격 강제) | A는 CLI가 stdio 브리지를 재연결하는 과도기 겹침을 허용한다. B는 동시 연결을 완전히 1개로 묶지만 재연결 경합 시 일시 거부가 생길 수 있다. |
| 5. 토큰 만료 정책 (`TokenTtl`) | `30`분(`1800`초, 상한 `86400`초) 절대 수명 기본값. 만료 시각 이상이면 신규 인증 + 이미 열린 연결의 후속 요청(`ping`·알림 포함) 및 유휴 연결 모두 차단 | A) 절대 수명 유지(장시간·사전 기동 채팅은 AI-02/C-09에서 더 긴 `TokenTtl` 전달) + 실행 종료 시 즉시 폐기 / B) R-04:70 기준 유휴 타임아웃 30분 + 절대 상한 병용 | A는 단순하고 토큰 노출 창을 고정한다. B는 30분을 넘는 활성 채팅을 끊지 않으면서 유휴 노출을 막는다(AI-02·C-09에서 결정). |
| 6. 소켓 디렉터리·파일 이름 형식 | `<base_dir>/r<16자리 hex>/m.sock` (`0700` 폴더 안 `0600` 소켓, 전체 경로 ≤ `103`바이트 = macOS `sun_path` 104바이트 NUL 포함 한도 이내) | A) 난수 하위 폴더 `r<16hex>/m.sock` 유지 / B) 앱 런타임 고정 루트(예: `/tmp/sc-<uid>/`) 정책을 별도 규정 | A는 25바이트 접미사만으로 경로 길이를 최소화한다. B는 macOS `Application Support` 같은 긴 경로를 피하는 운영 경로 규칙을 앱 부트스트랩에서 고정한다. |
| 7. 인증 첫 줄 개행·공백 규칙 | 정확히 `"AUTH <64소문자hex>\n"`(LF만 허용, CRLF·앞뒤 임의 공백 거부) | A) LF 전용 엄격 형식 유지 / B) CRLF(`\r\n`) 허용 | A는 M-07 `mcp-bridge`와 단순·엄격 계약을 고정해 줄 스머글링을 막는다. B는 외부 수동 테스트 도구 호환성을 넓힌다. |

## 인계

1. **실제 macOS 상대 프로세스 조회 구현 전 닫힌 실패 (O-07)**: **실제 조회 구현(`platform-macos`의 `PeerProcessInspector` 어댑터)이 들어오기 전에는 MCP 서버가 모든 연결을 거부한다(의도된 닫힌 실패).** O-07에서 `platform-macos`의 `unsafe` 예외(macOS `libproc` `proc_pidinfo`/`proc_pidpath` 등) 허용 여부를 결정한 뒤 `PeerProcessInspector` 구현을 연결한다.
2. **`pastey` 전용 `external-location`·`BANNED_MACROS` 잠금 (H-70)**: 이번 작업에서는 `pastey =0.2.3`을 `rmcp =3.9.0` 전이 의존성으로 `deny.toml` `bans.allow`에만 등록했고 `mcp-server`에서는 `pastey`·`rmcp::paste!`를 쓰지 않는다. ADR 0162의 권고에 따라 `pastey` 직접 의존을 막는 `external-location`과 `BANNED_MACROS` 잠금은 **H-70**에서 추가한다.
3. **후속 MCP 도구·브리지·변이 통합**: M-02~M-06에서 `McpSkeletonHandler`에 읽기·제안 도구 라우터와 호출 감사 기록을 연결하고, M-07에서 `crates/mcp-bridge`가 환경 변수 토큰으로 `AUTH <token>\n`을 전송하도록 맞춘다. `H-68` 병합 시 `.cargo/mutants.toml`·`test_packages.rs`·`cargo_config_tests.rs`·`diff-mutants-plan.mjs`·`h47-nightly.test.mjs`의 변이 대상 목록을 단일 원천으로 합친다.
