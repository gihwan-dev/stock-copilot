# ADR 0007: `xtask arch-check`의 그래프 검사와 소스 위치 검사

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-04, H-22
- 반영한 문서: 없음 (architecture.md 3.2, 4.6과 harness.md 3.5, 6.2, 6.8을 따르고, 해석이 필요한 곳만 여기 적는다)

## 맥락

architecture.md 3.2는 작업 공간 crate 사이의 간선, 개발 의존, 직접·전이 금지, 외부 crate 위치를 표로 정하고 `xtask arch-check`가 검사한다고 정했다. 4.6과 harness.md 3.5는 같은 명령이 재수출 금지, `impl UserPresence` 위치, `UserPresence`의 별칭 가져오기·재수출, 토스 쓰기 메서드와 주문 경로의 조합, `tossinvest.com` 위치도 본다고 정했다. 표를 코드에 흩어 두면 표 하나를 고칠 때 검사 코드가 바뀌고, 표가 보호되지 않는다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, `cargo_metadata` 0.23.1, `syn` 2.0.119(`full`, `visit`), `proc-macro2` 1.0.107, `toml` 1.2.1, cargo-nextest 0.9.151.

| 항목 | 명령 | 결과 |
|---|---|---|
| 실제 작업 공간 | `just arch-check` | `arch-check: 위반 0건`, 종료 코드 0 |
| 위반·대조 예제 35개 | `arch_check_cases::every_case_reports_exactly_its_findings` | 예제마다 기대한 (규칙, 대상) 집합과 정확히 같다 |
| 표 항목 하나씩 빼기 | `arch_check_cases::removing_any_rule_entry_breaks_some_case` | 100개 넘는 변형 모두에서 어느 예제의 결과가 바뀌었다 |
| 표를 느슨하게 한 경우 | `rules.toml`의 `src-tauri` 간선에 `order-execution`을 더한 뒤 같은 테스트 | `src-tauri-depends-on-order-execution`의 `allowed-edge`가 사라져 실패 |
| 오탈자 키 | `arch_check_cases::unknown_rule_keys_are_rejected` | `transitive-forbiden` 같은 모르는 키는 규칙 파일 해석 오류 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 표와 예외 목록은 `harness/arch-check/rules.toml` 하나에 둔다. 모르는 키는 해석 오류다 | 표를 보호 경로(`harness/`)에 모은다. 오탈자 키가 규칙을 조용히 지우지 못한다 | Rust 상수 표(표 변경이 검사 코드 변경이 된다) | harness.md 7.1 |
| 그래프는 `cargo metadata --locked --offline`의 resolve 그래프로 만든다. 간선 종류는 normal, build, dev이고 모르는 종류는 normal로 본다 | 이름을 바꾼 의존(`package = …`)과 대상별 의존도 그래프에 나타난다 | 매니페스트 직접 해석 | architecture.md 3.2, R-09 Q3 |
| 전이 그래프는 시작 crate에서 첫 단계만 모든 종류, 그 뒤는 normal·build만 따른다 | crate의 개발 의존은 그 crate의 테스트에만 들어가고 의존 crate의 개발 의존은 들어가지 않는다 | 모든 단계에서 dev 포함(실제 그래프보다 넓다), dev 제외(테스트 그래프가 빠진다) | - |
| 제품 bin 그래프는 `src-tauri` 패키지의 normal·build 전이 그래프다 | bin은 개발 의존을 쓸 수 없다. `gen-bindings`도 같은 그래프다 | bin 대상별 그래프(cargo metadata가 대상별 간선을 주지 않는다) | architecture.md 3.2 |
| 규칙 이름: `unknown-crate`, `allowed-edge`, `dev-edge`, `direct-dependent`, `transitive-dependency`, `product-bin-graph`, `external-location`, `external-allowlist`, `external-feature`, `reexport`, `user-presence-impl`, `user-presence-alias`, `user-presence-reexport`, `toss-order-write`, `toss-host`, `source-parse` | 예제의 기대값을 규칙 이름과 대상으로 적는다 | - | harness.md 6.8 |
| 위반·대조 예제는 `harness/arch-check/cases/<이름>/`의 작은 작업 공간, 외부 crate는 `harness/arch-check/vendor/`의 경로 crate, 기대값은 `harness/arch-check/cases.toml`에 둔다. 테스트는 예제를 `target/harness/` 아래로 복사해 실제 규칙 파일로 검사한다 | 예제 폴더와 목록이 어긋나면 실패한다. `full-architecture` 예제가 허용 간선과 개발 의존 표의 모든 항목을 담으므로 허용 항목 하나를 빼면 실패한다 | 예제마다 기대값 파일(주석을 쓸 수 없어 목록 하나가 낫다) | harness.md 6.8, ADR 0005 |
| 소스 검사는 각 작업 공간 패키지 폴더의 파일을 본다. `target`과 다른 패키지 폴더는 건너뛴다. `.rs`는 `syn`으로 해석하고, 해석되지 않으면 `source-parse`로 실패한다 | 해석하지 못한 파일을 통과시키면 검사를 피할 수 있다 | 모듈 트리만 따라가기(모듈에 연결하지 않은 파일이 빠진다) | - |
| 재수출 금지: `approval-command`·`broker-toss-trade`·`trade-fakes`에서는 `pub`·`pub(…)`이 붙은 `use`와 `extern crate`를 전부 실패시킨다 | `use order_execution as x; pub use x::…`처럼 경로를 바꾸면 경로 기반 검사는 피할 수 있다 | `order_execution` 경로만 검사 | architecture.md 3.2 |
| `impl UserPresence`는 트레이트 경로의 마지막 이름이 `UserPresence`인 구현이다. 매크로 인자는 항목으로 해석해 같은 검사를 하고, 해석되지 않으면 토큰에 `impl`과 `UserPresence`가 함께 있으면 실패시킨다. `UserPresence`를 `as`로 가져오는 것은 모든 crate에서, `pub(…)` `use`로 다시 내보내는 것도 모든 crate에서 실패다 | 이름 기반 검사이므로 이름을 바꾸는 길을 모두 막는다 | 이름 해석(컴파일러 없이 불가) | architecture.md 3.2 |
| 토스 주문 경로 조합은 파일 단위로 본다. 파일 안에 쓰기 메서드 표식(식별자나 문자열·바이트 문자열 값이 `POST`·`PUT`·`PATCH`·`DELETE`, 대소문자 무시)과 주문 경로 표식(문자열을 `/`로 나눈 조각에 `orders`나 `conditional-orders`가 있음)이 함께 있으면 실패다. 같은 crate의 `const`·`static`이 표식을 담으면 그 이름(문자열 안의 `{이름}` 포함)도 표식이다. `broker-toss-trade`는 제외다 | 토큰 발급(`POST /oauth2/token`)과 주문 조회(GET)는 표식이 겹치지 않아 통과한다. 경로를 상수와 다른 파일로 나눠도 잡힌다 | 함수 단위(호출 경로를 따라가야 한다), crate 단위(`broker-toss`의 토큰 발급과 주문 조회가 함께 있을 수 없다), 전체 경로 문자열 일치(문자열을 나누면 피한다) | architecture.md 3.2, harness.md 3.5 |
| `tossinvest.com`(대소문자 무시)은 패키지 폴더의 모든 파일(바이트)에서 찾는다. `apps/desktop/src-tauri/src/main.rs`와 `toss-smoke` 패키지만 허용한다 | 녹화 자료나 설정 파일에 주소가 들어가도 잡힌다 | `.rs`의 문자열 리터럴만 | harness.md 6.2 |
| `just check`는 clippy와 deny 뒤에 `just arch-check`(`cargo run --locked --quiet --package xtask -- arch-check`)를 돈다. 위반이 있으면 규칙 이름과 대상을 표준 오류에 쓰고 종료 코드 1로 끝난다 | `print!`·`exit`가 금지라서 `writeln!(stderr)`와 `ExitCode`를 쓴다 | 테스트로만 실행(실제 작업 공간 검사가 테스트 필터에 묶인다) | harness.md 1.1 |

## 문서의 모호한 점과 고른 해석 (가장 엄격한 쪽)

| 문서 | 모호한 점 | 구현 |
|---|---|---|
| architecture.md 3.2 외부 crate 표 "시간 타입 crate" | 어떤 crate인지 정해지지 않았다 | `domain`의 외부 의존 허용 목록은 `rust_decimal` 하나. 시간 crate를 정하면 `rules.toml`에 더한다 |
| 같은 표, `rust_decimal`(`maths` 기능) | 기능까지 강제하는지 | 기능은 검사하지 않는다 |
| 개발 의존 표 | 일반 의존으로 허용된 crate를 개발 의존으로도 둘 수 있는지 | 개발 의존은 개발 의존 표만 본다. `domain`을 개발 의존으로 두는 것도 실패다 |
| 개발 의존 표 "그 밖의 crate: `testkit`"과 "`xtask`는 작업 공간 crate에 의존하지 않는다" | `xtask`가 `testkit`을 개발 의존으로 둘 수 있는지 | 둘 수 없다(`xtask = []`) |
| 금지 표 "직접 의존할 수 있는 것"에 종류가 없는 행 | 개발 의존도 포함하는지 | normal·build만 허용 |
| 외부 crate 표 `domain` 행 | 개발 의존도 막는지 | normal·build만 막는다. 속성 테스트 crate를 개발 의존으로 둘 수 있게 한다 |
| harness.md 3.5 "xtask의 외부 의존은 …이다" | 목록인지 허용 목록인지 | `xtask`의 normal·build 외부 의존 허용 목록으로 본다 |
| "`tauri-plugin-wdio-webdriver` 등 자동화 플러그인" | "등"의 범위 | `tauri-plugin-wdio-webdriver`, `tauri-plugin-playwright`(R-15), `tauri-plugin-webdriver`. 새 플러그인은 목록에 더한다 |
| "HTTP 클라이언트" | crate 목록 | `reqwest`, `hyper`, `hyper-util`, `ureq`, `isahc`, `surf`, `attohttpc`, `curl`, `awc` |
| "`tokio` 프로세스 기능", "`testkit`은 클라이언트 기능만" | 기능 이름 | 직접 선언의 `process`·`full`. `testkit`의 `rmcp`는 기본 기능을 끄고 `client`만 |
| "`src-tauri/platform`" | 모듈은 그래프로 구분할 수 없다 | `src-tauri` 패키지 전체를 허용한다 |
| "재수출(`pub use`)" | `pub(crate) use`도 포함하는지 | 포함한다 |

## 결과

- `rules.toml`, `cases.toml`, 예제 폴더는 `harness/` 아래라서 보호 파일이다. 표를 바꾸면 예제와 기대값도 함께 바꿔야 테스트가 통과한다.
- 파일 단위 조합 검사라서, `broker-toss`의 토큰 발급과 주문 조회는 서로 다른 파일에 둬야 한다. 주문 조회 파일이 이름이 `post`인 함수를 부르거나 상수로 `POST`를 담으면 실패한다.
- 알려진 빈틈: 함수 이름이나 다른 crate를 거치는 쓰기 도우미(`broker_toss::send(Method, path)`를 다른 crate에서 부르기)는 잡지 않는다. `broker-toss` 공개 API가 GET과 토큰 발급뿐이라는 것은 리뷰가 본다. `pub type` 별칭으로 `order-execution`의 타입을 노출하는 것도 이 검사 밖이다. `macro_rules!`는 H-05가 막는다.
- 1차의 `broker-toss-trade`가 주문 생성만 구현하는지(정정·취소·조건주문 경로 없음)는 이 검사가 보지 않는다(T-16).
- 바뀌면 다시 볼 조건: `cargo metadata`가 대상별 간선을 주게 될 때, 토스 API에 주문 경로가 더해질 때, 시간 crate를 정할 때.
