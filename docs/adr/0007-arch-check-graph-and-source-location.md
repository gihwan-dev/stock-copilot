# ADR 0007: `xtask arch-check`의 그래프 검사와 소스 위치 검사

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-04, H-22
- 반영한 문서: 없음 (architecture.md 3.2, 4.6과 harness.md 3.5, 6.2, 6.8을 따르고, 해석이 필요한 곳만 여기 적는다)

## 맥락

architecture.md 3.2는 작업 공간 crate 사이의 간선, 개발 의존, 직접·전이 금지, 외부 crate 위치를 표로 정하고 `xtask arch-check`가 검사한다고 정했다. 4.6과 harness.md 3.5는 같은 명령이 재수출 금지, `impl UserPresence` 위치, `UserPresence`의 별칭 가져오기·재수출, 토스 쓰기 메서드와 주문 경로의 조합, `tossinvest.com` 위치도 본다고 정했다. 표를 코드에 흩어 두면 표 하나를 고칠 때 검사 코드가 바뀌고, 표가 보호되지 않는다. 첫 구현의 리뷰에서 검사 범위(빈 `Cargo.toml`, `target` 이름 폴더), 기능으로만 켜지는 의존, 경로 패키지, 외부 래퍼를 거친 전이, 문자열 조각으로 만든 주문 경로를 이용한 우회가 재현되어 이 ADR에 함께 고쳐 적는다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, `cargo_metadata` 0.23.1, `syn` 2.0.119(`full`, `visit`), `proc-macro2` 1.0.107, `toml` 1.2.1, cargo-nextest 0.9.151.

| 항목 | 명령 | 결과 |
|---|---|---|
| 실제 작업 공간 | `just arch-check` | `arch-check: 위반 0건`, 종료 코드 0. 단 아래 예외 12행을 넣은 뒤다 |
| 외부 위치를 전이로 본 첫 실행 | `cargo run -p xtask -- arch-check` | `src-tauri => cargo_metadata, hyper, hyper-util, proc-macro2, reqwest, socket2, syn, toml` 8건. 경로는 `cargo tree --target all -i <crate>`로 확인했다(결과 절) |
| 위반·대조 예제 54개 | `arch_check_cases_tests::every_case_reports_exactly_its_findings` | 예제마다 기대한 (규칙, 대상) 집합과 정확히 같다 |
| 표 원소 하나씩 빼기 | `arch_check_cases_tests::removing_any_rule_element_breaks_some_case` | `rules.toml`을 TOML 값으로 읽어 모든 배열의 원소(문자열과 표 행) 하나를 뺀 변형 200개 넘게 만들고, 변형마다 예제나 실제 작업 공간의 결과가 바뀌는지 본다. 모두 바뀌었다 |
| 쓰이지 않는 원소 | `write-means`에 `unused_means`를 넣고 같은 테스트 | `rules.source.toss-order.write-means[16] removed`를 보고하며 실패 |
| 오탈자 키 | `arch_check_cases_tests::unknown_rule_keys_are_rejected` | 모르는 키는 규칙 파일 해석 오류 |
| 외부 crate 예제의 출처 | `cargo metadata --config source.crates-io.replace-with=…` | `harness/arch-check/vendor/`를 디렉터리 소스로 바꿔 끼우면 예제의 외부 crate가 레지스트리 출처(`registry+https://github.com/rust-lang/crates.io-index`)로 보인다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 표와 예외 목록은 `harness/arch-check/rules.toml` 하나에 둔다. 모르는 키는 해석 오류다 | 표를 보호 경로(`harness/`)에 모은다. 오탈자 키가 규칙을 조용히 지우지 못한다 | Rust 상수 표 | harness.md 7.1 |
| 그래프는 `cargo metadata --locked --offline --all-features`의 resolve 그래프로 만든다. 간선 종류는 normal, build, dev이고 모르는 종류는 normal로 본다. 모든 대상 플랫폼을 넣는다 | 이름을 바꾼 의존, 대상별 의존, 기능으로만 켜지는 선택적 의존도 그래프에 나타난다 | 기본 기능만, `--filter-platform`(검사가 플랫폼에 묶인다) | architecture.md 3.2 |
| 출처가 없는(경로) 비멤버 패키지가 그래프에 있으면 `path-package`로 실패한다 | 루트 `exclude`나 저장소 밖 경로의 crate는 멤버 검사를 피한다 | 경로 crate를 외부 crate로 취급 | 리뷰 |
| 작업 공간 crate 사이의 전이 그래프는 시작 crate에서 첫 단계만 모든 종류, 그 뒤는 normal·build만 따른다 | crate의 개발 의존은 그 crate의 테스트에만 들어간다 | 모든 단계 dev 포함, dev 제외 | - |
| 외부 crate 위치는 멤버에서 시작해 외부 노드만 거치는 전이로 본다. 첫 단계는 모든 종류, 그 뒤는 normal·build이고 다른 멤버에서 멈춘다. 예외는 (멤버, 외부 crate, 첫 단계 외부 crate) 세 값과 이유로 `external-location-exception`에 적는다 | 외부 래퍼(`mcp-server → 래퍼 → reqwest`)로 위치 규칙을 피하지 못한다. 예외가 경로의 입구까지 고정한다 | 직접 간선만, 멤버 단위 예외 | 리뷰 |
| HTTP 묶음은 HTTP 클라이언트와 TLS·소켓 계열(`hyper`, `h2`, `h3`, `rustls`, `native-tls`, `tokio-rustls`, `socket2` 등)과 `tauri-plugin-http`·`-websocket`·`-upload`다. `tauri-plugin-` 접두사 허용은 이 묶음을 풀지 않는다(규칙마다 따로 판정) | Tauri 플러그인으로 HTTP를 들여오지 못한다 | - | architecture.md 3.2 |
| 제품 bin 그래프는 `src-tauri` 패키지의 normal·build 전이 그래프다 | bin은 개발 의존을 쓸 수 없다 | bin 대상별 그래프 | architecture.md 3.2 |
| 소스 검사는 메타데이터의 멤버 패키지 폴더 아래 모든 파일을 본다. 건너뛰는 것은 다른 멤버 패키지 폴더와 메타데이터의 `target_directory`뿐이다 | 하위 폴더에 빈 `Cargo.toml`을 두거나 폴더 이름을 `target`으로 지어 검사를 피하지 못한다 | `Cargo.toml` 유무, 폴더 이름 | 리뷰 |
| 재수출 금지: `approval-command`·`broker-toss-trade`·`trade-fakes`에서 `pub`·`pub(…)`이 붙은 `use`, `extern crate`, `type` 별칭을 실패시킨다 | 경로를 바꾸거나 타입 별칭으로 `order-execution`의 이름을 내보내는 길을 막는다 | `order_execution` 경로만 검사 | architecture.md 3.2 |
| `UserPresence`: 트레이트 경로의 마지막 이름이 `UserPresence`인 `impl`은 `platform-macos`·`testkit`에서만. `as`로 가져오기와 `pub(…)` `use`로 내보내기는 어디서나 실패. 공개 글롭 `use`는 경로 첫 이름이 `application`(또는 파일 안의 `application` 별칭)이거나 파일이 `application`에 있으면 `user-presence-reexport`로 실패. 매크로 인자는 항목으로 해석해 같은 검사를 하고, 해석되지 않으면 토큰으로 본다 | 이름 기반 검사이므로 이름을 바꾸는 길을 모두 막는다 | 이름 해석(컴파일러 없이 불가) | architecture.md 3.2 |
| `process::Command`(경로, `use` 이름, 별칭, `process` 모듈 별칭, `process::*`, 매크로 토큰)는 `ai-claude-code`, `ai-codex`(architecture.md 2.3, AI CLI를 띄움), `src-tauri`(3.1 `platform/`의 자식 프로세스 정리), `testkit`(3.1 가짜 AI CLI), `xtask`(harness.md 6.8, 하네스가 cargo·clippy를 돌림)에서만 통과한다 | 자식 프로세스로 HTTP 클라이언트(`curl` 등)나 다른 실행 파일을 띄워 그래프 규칙을 피하지 못한다 | 검사 없음 | architecture.md 2.3, 3.1 |
| 토스 쓰기 경로의 방어선은 식별자 검사다. `source.http-write` 표의 crate(`broker-toss`, `sources`)에서는 HTTP 메서드를 정할 수 있는 식별자와 같은 문자열(`write-means`: `post`, `put`, `patch`, `delete`, `POST`…, `Method`, `method`, `method_mut`, `request`, `Request`, `RequestBuilder`, `execute`, `from_bytes`)이 표에 적은 파일에서만 나올 수 있다. `broker-toss`는 토큰 발급 파일 `crates/adapters/broker-toss/src/token.rs` 하나, `sources`는 없음(문서에 GET만 있다). 위반은 `http-write-means` | 경로 문자열을 어떻게 만들든(`"/api/v1/orders?"`, `concat!`, `["ord","ers"].concat()`, 다른 파일의 `fn`) 쓰기 수단 자체가 없으면 주문을 보낼 수 없다. `Method::from_bytes(&[80,79,83,84])`도 `Method`·`from_bytes`로 걸린다 | 경로 문자열 일치(조각으로 나누면 피한다) | 리뷰, architecture.md 3.2, 4.2 |
| 문자열 검사는 표식일 뿐이다. `broker-toss-trade` 밖의 파일에 `orders`가 든 문자열(대소문자 무시)과 쓰기 수단 표식이 함께 있으면 `toss-order-write`로 실패한다. 같은 crate의 `const`·`static`이 표식을 담으면 그 이름(문자열 안의 `{이름}` 포함)도 표식이다. 방어선은 그래프(HTTP 묶음 위치)와 식별자 검사다 | 표 밖 crate(`mcp-server`, `toss-smoke` 등)에서 주문 경로를 쓰는 흔적을 잡는다 | - | 리뷰 |
| `tossinvest.com`(대소문자 무시)은 패키지 폴더의 모든 파일(바이트)에서 찾는다. `apps/desktop/src-tauri/src/main.rs`와 `toss-smoke`만 허용한다 | 녹화 자료나 설정 파일에 주소가 들어가도 잡힌다 | `.rs` 문자열 리터럴만 | harness.md 6.2 |
| 예제는 `harness/arch-check/cases/<이름>/`의 작은 작업 공간, 기대값은 `cases.toml`, 외부 crate 대역은 `vendor/`(디렉터리 소스로 끼워 레지스트리 출처가 되게 한다), 저장소 밖 경로 crate는 `outside/`에 둔다. 테스트는 이것을 `target/harness/` 아래로 복사해 실제 규칙 파일로 검사한다. 표 원소 변형 테스트는 실제 작업 공간도 기대값 0건인 예제로 함께 본다 | 표 원소마다 그것이 없으면 결과가 바뀌는 예제가 있어야 한다. 예외 행은 실제 작업 공간이 덮는다 | 경로 crate 대역(`path-package` 규칙과 부딪힌다) | harness.md 6.8, ADR 0005 |
| 규칙 이름: `unknown-crate`, `path-package`, `allowed-edge`, `dev-edge`, `direct-dependent`, `transitive-dependency`, `product-bin-graph`, `external-location`, `external-allowlist`, `external-feature`, `reexport`, `user-presence-impl`, `user-presence-alias`, `user-presence-reexport`, `toss-order-write`, `toss-host`, `http-write-means`, `process-spawn`, `source-parse` | 예제의 기대값을 규칙 이름과 대상으로 적는다 | - | harness.md 6.8 |
| `just check`는 clippy와 deny 뒤에 `just arch-check`를 돈다. 위반은 규칙 이름과 대상을 표준 오류에 쓰고 종료 코드 1 | `print!`·`exit`가 금지라서 `writeln!(stderr)`와 `ExitCode`를 쓴다 | 테스트로만 실행 | harness.md 1.1 |

## 문서의 모호한 점과 고른 해석 (가장 엄격한 쪽)

| 문서 | 모호한 점 | 구현 |
|---|---|---|
| architecture.md 3.2 "시간 타입 crate" | 어떤 crate인지 정해지지 않았다 | `domain`의 외부 의존 허용 목록은 `rust_decimal` 하나 |
| 같은 표, `rust_decimal`(`maths` 기능) | 기능까지 강제하는지 | 기능은 검사하지 않는다 |
| 개발 의존 표 | 일반 의존으로 허용된 crate를 개발 의존으로 둘 수 있는지 | 개발 의존 표만 본다 |
| "그 밖의 crate: `testkit`"과 "`xtask`는 작업 공간 crate에 의존하지 않는다" | `xtask`의 `testkit` 개발 의존 | 둘 수 없다 |
| 금지 표에서 종류가 없는 "직접 의존할 수 있는 것" | 개발 의존 포함 여부 | normal·build만 |
| 외부 crate 표 `domain` 행 | 개발 의존도 막는지 | normal·build만 막는다 |
| harness.md 3.5 "xtask의 외부 의존은 …이다" | 목록인지 허용 목록인지 | `xtask`의 normal·build 외부 의존 허용 목록 |
| "자동화 플러그인 등", "HTTP 클라이언트" | 범위 | 위 결정 표의 목록 |
| "`tokio` 프로세스 기능", "`testkit`은 클라이언트 기능만" | 기능 이름 | 직접 선언의 `process`·`full`. `testkit`의 `rmcp`는 기본 기능을 끄고 `client`만 |
| "`src-tauri/platform`" | 모듈은 그래프로 구분할 수 없다 | `src-tauri` 패키지 전체 |
| "재수출(`pub use`)" | `pub(crate) use`도 포함하는지 | 포함한다 |
| 외부 crate 위치 표 | 직접 의존인지 전이인지 | 전이. 실제 작업 공간이 걸리는 것은 예외 행으로 적는다 |

## 결과

- 실제 작업 공간에 넣은 위치 예외 12행(전부 `crate = "src-tauri"`): `reqwest`·`hyper`·`hyper-util`·`socket2`는 `tauri`를 거친다(`tauri` 2.12.5의 Android·iOS 대상 의존 `reqwest`, 그리고 `tokio → socket2`). `cargo_metadata`·`toml`·`syn`·`proc-macro2`는 `tauri`와 `tauri-build`를 거친다(`tauri-utils`, `cargo_toml`, 절차 매크로, Linux 대상 `system-deps`). 경로와 이유는 `rules.toml`의 각 행에 있다. 원소 변형 테스트가 행마다 필요한지 확인한다.
- 파일 단위 검사라서 `broker-toss`의 쓰기 수단은 토큰 발급 파일 하나에만 둘 수 있다. 주문 조회는 `get`만 쓴다.
- 원리상 이 검사로 못 막는 것과 덮는 방어선:
  - 문자열로 조립한 호스트(`"tossinvest" + ".com"`): 네트워크 차단(harness.md 6.2의 OS 겹)과 `TossEndpoint` 생성자 주입이 덮는다.
  - 토큰 발급 파일 안에서 조각으로 만든 주문 경로, 다른 crate를 거친 쓰기 도우미(`broker_toss`가 범용 요청 함수를 공개하는 것): 그래프 규칙(HTTP 묶음은 세 crate에만), `broker-toss` 공개 API가 GET과 토큰 발급뿐인지 보는 리뷰, 토큰 발급 파일을 보호된 표에 고정한 것이 덮는다.
  - 공개 `fn` 시그니처로 `order-execution`의 타입이 새는 것: 재수출 검사는 이름 노출만 막는다. `src-tauri`가 `order-execution`에 직접 의존하지 않으므로 그 타입을 이름으로 부를 수 없고(architecture.md 3.2), 리뷰가 공개 API를 본다.
  - `macro_rules!`로 숨긴 코드: H-05의 금지 매크로 검사가 막는다.
- T 묶음으로 넘기는 구조 과제:
  - `reqwest`를 가진 읽기 전용 HTTP crate를 분리해 `broker-toss` 그래프에서 범용 HTTP 클라이언트를 없앤다(GET과 토큰 발급만 내는 좁은 클라이언트).
  - `sources`가 토스 자격 증명·토큰을 받지 못함을 조립 경로 테스트로 고정한다.
  - 1차 `broker-toss-trade`가 주문 생성만 구현하는지(정정·취소·조건주문 없음)는 T-16이 본다.
- 바뀌면 다시 볼 조건: `tauri`를 올릴 때(예외 행이 바뀐다), 토스 API에 주문 경로가 더해질 때, 시간 crate를 정할 때, 토큰 발급 파일의 위치가 바뀔 때.
