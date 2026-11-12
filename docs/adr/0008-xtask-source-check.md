# ADR 0008: xtask 소스·매니페스트·파일·시각 검사 (`xtask source-check`)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-05, H-23, H-37
- 반영한 문서: 없음 (harness.md 2장, 2.1, 3.1, 3.2와 R-09 Q2, Q8, Q13, 6.5를 따르고, 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 2장과 3.2는 주석, 금지 속성·매크로, `domain`의 `f32`/`f64`, 매니페스트 표, 작업 공간의 `build.rs`·proc-macro, 하위 설정 파일, 테스트 건너뛰기를 xtask가 실패시킨다고 정했다. 3.1은 현재 시각·환경 변수 읽기를 보호된 파일 허용 목록 밖에서 실패시킨다고 정했다. ADR 0005의 알려진 빈틈 (a)는 저장소 `.cargo/`를 H-23에 넘겼고, `concat!(env!(…))`처럼 `disallowed_macros`가 놓치는 `env!`를 H-05에 넘겼다. 규칙 이름, 허용 목록의 형식, 시계 어댑터 파일의 위치, `.cargo/`를 어디까지 막을지는 문서에 없다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, `ra-ap-rustc_lexer` 0.184.0, `proc-macro2` 1.0.107(`span-locations`), `toml` 1.2.1, `cargo_metadata` 0.23.1.

| 항목 | 명령 | 결과 |
|---|---|---|
| `ra-ap-rustc_lexer` 고정 버전 | `cargo info ra-ap-rustc_lexer@0.184.0` | R-09 Q8이 검증한 0.184.0이 crates.io에 있다. 새 전이 crate는 `unicode-properties` 0.1.5 하나 |
| 위반·대조 예제 | `cargo nextest run -p xtask source_check` | 8개 테스트 통과. 소스 예제 91건, 작업 공간 예제 33건이 기대 규칙 집합과 정확히 같다 |
| 규칙 하나 빼기 | 금지 속성 표에서 `ignore`를 지우고 같은 명령 | `attribute-ignore.rs`, `attribute-ignore-reason.rs`가 "expected {attribute:ignore}, reported {}"로 실패 |
| 실제 작업 공간 | `cargo run --locked -p xtask -- source-check` | 처음에는 `crates/xtask/src/workspace_layout.rs`의 `concat!(env!("CARGO_MANIFEST_DIR"), …)` 3곳이 `clock-env:env`로 실패. 고친 뒤 위반 없음 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 검사 하나를 `xtask source-check`로 묶고 `just check`에서 cargo-deny 다음, nextest 앞에 돈다 | 작업 공간 한 번 순회로 주석·속성·매크로·시각·매니페스트·파일을 모두 본다 | 규칙마다 명령 | harness.md 1.1 |
| 주석은 `ra-ap-rustc_lexer` `=0.184.0`의 `LineComment`/`BlockComment` 토큰, 나머지는 `proc-macro2` `=1.0.107` 토큰 트리를 재귀로 돈다. `syn`은 쓰지 않는다 | 매크로 호출 안의 속성을 보려면 토큰 트리를 직접 돌아야 한다. `syn`이 필요한 검사가 없다 | `syn` 파싱 | R-09 Q8, Q13 |
| 규칙 이름은 `종류:대상` 문자열이다(`attribute:allow`, `expect:no-reason`, `cfg:test-outside-module`, `cfg:macro`, `macro:include`, `domain-float:f64`, `clock-env:now`, `manifest:table:profile`, `manifest:lints-not-inherited`, `file:build-script`, `file:proc-macro`, `file:nested-config:clippy.toml`, `file:cargo-config`, `comment`, `unparsable`) | 예제의 기대값을 이름으로 비교한다 | 숫자 코드 | harness.md 6.8 |
| 규칙 목록은 테스트가 코드의 규칙 정의(금지 속성·매크로 표, 시각·환경 변수 목록, 매니페스트 표, 하위 설정 파일 목록, `expect` 문제 종류, 단일 규칙)에서 만들고, 위반 예제가 없는 규칙이 있으면 실패한다 | 손으로 쓴 목록은 빠진다 | 손으로 쓴 목록 | 리뷰 |
| 위반·대조 예제는 `harness/source-checks/sources/*.rs`(소스)와 `harness/source-checks/workspaces/<사례>/`(기본 작업 공간 `base` 위에 덮어쓰는 파일)에 두고, 기대 규칙은 `harness/source-checks/cases.toml`에 둔다. 테스트는 보고된 규칙 집합이 기대와 **같은지** 본다 | 예제에 주석을 쓸 수 없다. 같음 비교라서 규칙을 빼도, 오탐이 생겨도 실패한다 | 파일 이름에 기대값 | harness.md 6.8, ADR 0005 |
| 작업 공간 예제의 설정 파일은 `.example` 접미사와 `dot-` 접두사로 저장하고, 테스트가 `target/harness/source-checks/` 로 복사할 때 원래 이름으로 바꾼다 | 저장소 안에 `.cargo/`, `clippy.toml` 이 그대로 있으면 실제 검사가 실패한다 | `harness/`를 검사에서 빼기 | - |
| 허용 목록은 `harness/source-checks/allowlists.toml` 한 파일의 `[[generated]]`, `[[clock-environment]]`, `[[build-script]]` 표다. 항목마다 `path`와 `reason`이 있어야 한다 | 보호 경로(`harness/`) 안의 데이터 파일. 목록 내용은 테스트 `allowlists_hold_only_the_documented_paths`가 고정한다 | 코드 안의 상수 | harness.md 3.1, 7.1 |
| 생성 경로(`crates/adapters/broker-toss-schema`)에서는 주석 검사와 금지 속성 검사만 빠진다. 금지 매크로, `cfg`·`cfg_attr`, 시각·환경 변수 검사와 매니페스트 검사는 그 안에서도 돈다 | harness.md 5장 "이 crate만 주석·속성 검사에서 제외". 생성기가 `include!`나 시계 읽기를 낼 이유가 없다 | `.rs` 토큰 검사 전체 제외 | harness.md 5장, 리뷰 |
| 허용 목록의 경로는 실제로 있어야 한다. 없으면 목록 읽기가 실패한다. 그래서 시계 어댑터 파일은 아직 목록에 없고, 시계 어댑터를 만드는 작업이 사용자 승인으로 넣는다 | 없는 경로를 미리 열어 두면 누구든 그 자리에 파일을 만들어 검사를 피한다 | 예상 경로를 미리 넣기 | 리뷰 |
| 시각·환경 변수 검사는 경로 `::now`로 시작하는 모든 이름(`now_utc`, `now_local`, `now_v7` 등), `::today`, `::elapsed`, `.elapsed()`, `::var`, `::var_os`, `::vars`, `::vars_os`, 매크로 `env!`·`option_env!`, 그리고 `use` 안의 같은 이름·`env as`·`env::*`를 실패시킨다. 타입 이름을 보지 않는다 | 타입 이름을 보지 않으므로 별칭(`use std::time::SystemTime as T; T::now()`)과 서드파티 시간 crate도 걸린다. `UNIX_EPOCH.elapsed()`는 `now` 없이 현재 시각을 읽는다. `env!`은 `concat!` 안에서 clippy가 놓친다(ADR 0005) | `SystemTime::now` 등 경로 목록 비교(별칭이 빠진다) | harness.md 3.1, ADR 0005 |
| `cfg(test)`는 바깥 속성으로, 이름이 `tests`이거나 `_tests`로 끝나는 `mod` 선언 바로 앞(다른 속성과 `pub` 뒤)에만 허용한다(`cfg:test-module-name`). `#![cfg(test)]`, 함수·문장의 `cfg(test)`, `cfg!` 매크로는 실패 | "cfg(test)는 테스트 모듈 선언에만 허용". `#[cfg(test)] mod core { … }`처럼 표준·외부 crate 이름을 가리는 모듈로 테스트 빌드의 동작을 바꿀 수 있다. 그래서 xtask의 테스트 모듈을 `harness_workspace_tests`, `rust_lints_tests`, `third_party_tests`, `workspace_layout_tests`로 바꿨다 | 이름 제한 없음 | harness.md 2장, 리뷰 |
| `expect`는 바깥 속성이고 괄호 안에 린트 경로 하나와 비어 있지 않은(공백만이 아닌) 문자열 `reason` 하나만 있을 때 통과한다. 어떤 린트를 끌 수 있는지, `mod`·`impl`에 붙었는지는 보지 않는다 | 그 판정은 H-40 몫 | - | harness.md 2.1 |
| `#[unsafe(no_mangle)]`처럼 `unsafe(…)`로 감싼 속성은 안쪽 이름으로 판정한다. 경로 속성은 `core`·`std`·`alloc`로 시작하면 마지막 이름으로 판정한다 | 2024 판에서 `no_mangle`, `export_name`, `link_section`은 `unsafe(…)`로 쓴다 | - | - |
| `.cargo` 라는 이름의 폴더(또는 링크)는 저장소 어디에 있든 실패한다. 내용은 보지 않는다 | `[env] CLIPPY_CONF_DIR`·`CLIPPY_ARGS`, `[alias] clippy`, `build.rustflags`를 하나씩 막는 것보다 단순하다 | 루트 `config.toml`만 허용하고 표를 검사 | ADR 0005 (a) |
| 하위 설정 파일(`clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `.rustfmt.toml`, `rust-toolchain`, `rust-toolchain.toml`)은 `crates/`뿐 아니라 저장소 루트 밖 어디서든 실패한다 | `apps/desktop/src-tauri`도 작업 공간 멤버다. rustfmt는 `.rustfmt.toml`도 읽는다 | `crates/` 아래만 | R-09 Q2 |
| 순회는 `cargo metadata`의 `target_directory`(저장소 안이면), 루트의 `.git`·`.claude`·`node_modules`, `apps/desktop/node_modules`만 건너뛴다. 멤버 폴더 안의 `target`·`node_modules` 폴더는 검사한다 | 이름으로 건너뛰면 `src/target/mod.rs`처럼 cargo가 컴파일하는 파일이 빠진다 | 이름으로 어느 깊이든 건너뛰기 | 리뷰 |
| 순회 중 만난 심볼릭 링크는 파일이든 폴더든 그 자체로 실패한다(`file:symlink`) | 링크를 따라가지 않으면 링크 너머의 `.rs`가 검사에서 빠지고 cargo는 컴파일한다 | 링크 따라가기(순환과 저장소 밖 경로) | 리뷰 |
| `cargo metadata`의 모든 target `src_path`(`..`을 글자로 정리한 뒤)가 그 멤버 폴더 밖이면 실패한다(`file:target-outside-member`) | `[lib]`·`[[bin]]` 등의 `path`로 검사하지 않는 폴더의 파일을 컴파일할 수 있다 | - | 리뷰 |
| `build.rs`는 `cargo metadata`의 `custom-build` 대상(`package.build`로 이름을 바꾼 것 포함)과 멤버 폴더 안의 `build.rs` 파일 둘 다로 찾는다 | 이름을 바꾼 빌드 스크립트도 막는다 | 파일 이름만 | - |

## 결과

- `crates/xtask/src/workspace_layout.rs`의 `concat!(env!("CARGO_MANIFEST_DIR"), …)` 3곳을 `harness_workspace::repository_root()`로 바꿨다. 테스트의 판정은 같다. 기존 테스트 파일 변경이므로 사용자 확인이 필요하다.
- R-15 2.7의 네트워크 차단(루트 `.cargo/config.toml`의 `runner`)과 harness.md 6.4의 `.cargo/mutants.toml`을 넣을 때는 이 검사를 함께 바꿔야 한다. 그때 허용 목록에 경로를 더하고 그 파일의 `[env]`·`[alias]`·`build`를 검사하는 것을 권한다.
- 시각 검사는 타입을 모르므로 우리 코드의 `elapsed`, `now`, `var` 같은 이름의 연관 함수도 실패한다(메서드 `.now()`는 통과한다). 이름을 바꿔 피한다.
- xtask 테스트 모듈 네 개의 이름을 바꿨다. 테스트 본문과 단언은 그대로지만 nextest의 테스트 경로가 바뀐다.
- 바뀌면 다시 볼 조건: `ra-ap-rustc_lexer`를 올릴 때(`TokenKind` 변경), Rust 판이 바뀌어 속성 문법이 바뀔 때, 시계 어댑터 자리가 정해질 때.
