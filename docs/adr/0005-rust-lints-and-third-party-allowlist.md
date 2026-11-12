# ADR 0005: Rust 린트 설정, `just check` 환경 고정, 서드파티 허용 목록

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-02, H-03
- 반영한 문서: 없음 (harness.md 1.1, 2.1, 3.1, 3.2, 3.5와 R-09 6.1~6.3을 따르고, 어긋나는 곳만 여기 적는다)

## 맥락

harness.md 3.1은 `[workspace.lints]`와 `clippy.toml`의 초안을 R-09 6.1, 6.2로 가리킨다. 이 초안은 Tauri가 없는 예제 작업 공간에서 검증됐다. 실제 작업 공간에는 `tauri::generate_context!`가 있고, harness.md 3.1은 "현재 시각과 환경 변수 읽기는 clippy가 아니라 xtask가 막는다", "`disallowed-types`가 Tauri에서 막히면 ADR로 조정한다"고 정했다. `rustfmt.toml`의 내용은 어느 문서에도 없다. `deny.toml`은 R-09 6.3이 예제 작업 공간의 전이 목록만 담고 있다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, clippy 0.1.99, cargo-deny 0.20.2(`cargo install --locked`), just 1.59.0, cargo-nextest 0.9.151.

| 항목 | 명령 | 결과 |
|---|---|---|
| R-09 6.1 그대로 적용 | `just clippy Cargo.toml` | `src-tauri/src/main.rs`의 `generate_context!` 한 줄에서 `disallowed_macros`(`include_bytes!`, `env!`), `disallowed_methods`(`std::process::exit`), `disallowed_types`(`HashMap`) 7건. 매크로 확장 안이라 고칠 코드가 없다 |
| 같은 줄에 `#[expect]` | 위와 같음, 세 린트를 `forbid`로 둔 채 | E0453. `forbid`는 끌 수 없다 |
| 기존 테스트 | `just clippy Cargo.toml` | `src-tauri/src/tests.rs`의 `assert!(built.is_ok())`가 `assertions_on_result_states`(forbid)에 걸림 |
| R-09 6.1의 린트 이름 | `just check` | 1.99.1에 모두 있고 이름이 바뀐 것은 없다. 41개 `forbid` 린트 모두 위반 예제에서 그 이름으로 실패했다 |
| `unstable_features` | 안정판에서 `#![feature(never_type)]` | 린트보다 먼저 E0554가 난다. `RUSTC_BOOTSTRAP=1`이면 `unstable_features`(`-F`)로 실패한다 |
| `CLIPPY_CONF_DIR`가 빈 폴더 | 저장소 안의 빈 폴더 | clippy가 그 폴더의 상위로 올라가 루트 `clippy.toml`을 찾았다. 빈 `clippy.toml`을 둬야 설정이 꺼진다 |
| `RUSTC_WRAPPER=""` | `CARGO_BUILD_RUSTC_WRAPPER=<--cap-lints=allow 래퍼>`와 함께 | 래퍼가 쓰이지 않았다. 빈 값이 설정 파일의 `build.rustc-wrapper`보다 앞선다 |
| `RUSTC_WORKSPACE_WRAPPER` | `cargo clippy` | cargo-clippy가 자기 값으로 덮어써서 우회가 되지 않았다 |
| cargo-deny 허용 목록 | 빈 목록으로 `cargo deny --locked check` | 서드파티 207개 + `tauri`가 `not-allowed`, 빌드 스크립트 34개, MPL-2.0 5개, `bit-vec`의 `crusader.sh`, `cookie`의 `scripts/test.sh`(실행 스크립트) |
| 경로 crate와 취약점 | 가짜 권고 DB로 `cargo deny check advisories` | 경로 crate는 권고와 맞추지 않는다. 레지스트리 crate(`itoa 1.0.19`)는 맞췄다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `[workspace.lints]`는 R-09 6.1 그대로 두되 `disallowed_methods`, `disallowed_types`, `disallowed_macros`만 `forbid`에서 `deny`로 내린다. harness.md 3.1의 `forbid` 표는 그대로다 | `generate_context!` 확장을 고칠 수 없고 `forbid`는 끌 수 없다. 세 린트는 harness.md 3.1의 `forbid` 표에 없다 | 목록에서 `include_bytes!`·`env!`·`exit`·`HashMap`을 빼기(작업 공간 전체가 약해진다), `generate_context!`를 쓰지 않기(Tauri가 요구한다) | 실측 |
| `src-tauri/src/main.rs`의 `let context = tauri::generate_context!();` 한 문장에만 2.1 형태의 `#[expect]` 세 개를 붙인다 | 조립 지점 하나에 갇힌다. `expect`라서 확장이 바뀌어 걸리지 않으면 스스로 실패한다 | crate 단위 끄기 | harness.md 2.1 |
| `clippy.toml`은 R-09 6.2에서 `SystemTime::now`, `Instant::now`, `env::var`를 빼고 `Decimal::to_f64`와 `std::matches`를 더한다 | harness.md 3.1이 시각·환경 변수는 xtask가 파일 허용 목록으로 막는다고 정했고, `to_f64`와 `matches!`를 목록에 적었다 | R-09 6.2 그대로 | harness.md 3.1 |
| `rustfmt.toml`은 `edition`과 `style_edition`을 2024로만 둔다 | 문서에 정한 형식 규칙이 없다. 안정판 옵션만 쓴다 | 비워 두기(편집 훅의 `rustfmt`와 `cargo fmt`가 같은 판을 보게 한다) | - |
| `unstable_features` 위반 예제의 기대값은 E0554다. `#[allow]`·`#![allow]`·`#[expect]` 예제는 다른 린트와 같이 E0453을 기대한다 | 안정판에서는 린트가 나기 전에 컴파일러가 막는다. `forbid` 여부는 E0453 예제가 확인한다 | 예제 빼기 | 실측 |
| `just check`의 환경 고정은 `justfile` 머리의 `unexport`와 `export`로 한다. harness.md 1.1의 목록에 `RUSTC`, `RUSTC_BOOTSTRAP`, `RUSTUP_TOOLCHAIN`, `CARGO_BUILD_RUSTC`, `CARGO_BUILD_RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER`, `CARGO_BUILD_TARGET_DIR`, `CARGO_BUILD_RUSTDOCFLAGS`를 더한다. `RUSTC_WRAPPER`와 `RUSTC_WORKSPACE_WRAPPER`는 지우지 않고 빈 값으로 둔다 | `justfile`의 모든 레시피에 걸린다. 빈 래퍼 값은 설정 파일의 래퍼까지 막는다. 더한 변수는 컴파일러·툴체인·불안정 기능을 바꾼다 | 레시피마다 `env -u`, 별도 스크립트 | harness.md 1.1, R-09 6.4 |
| `CARGO_TARGET_DIR`는 지우기만 하고 고정하지 않는다 | 설정 파일의 `build.target-dir`은 H-01의 `build_output_goes_to_root_target_directory`가 잡는다 | 루트 `target`으로 고정 | harness.md 1.1 |
| clippy 단계는 `just clippy <매니페스트>` 레시피로 빼고 `check`가 부른다 | 하네스 테스트가 같은 레시피와 같은 환경 고정으로 위반 예제 작업 공간을 검사한다 | 테스트가 cargo를 직접 부르기(환경 고정을 확인하지 못한다) | harness.md 6.8 |
| `deny.toml`은 R-09 6.3 구조에 실제 그래프의 서드파티 208개를 한 줄씩 이유와 함께 적는다. 이유는 그 crate를 끌어오는 직접 의존(`tauri`, `tauri-build`, `cargo_metadata`)이다 | `bans.allow`가 비어 있지 않으면 목록 밖 crate가 전이까지 실패한다 | 이름 패턴, 직접 의존만 | R-09 Q10, harness.md 2장 |
| MPL-2.0은 전역 허용이 아니라 `licenses.exceptions`로 `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`에만 허용한다 | Tauri가 끌어온다. 다른 crate에 MPL이 새로 들어오면 실패해야 한다 | `licenses.allow`에 MPL-2.0 | 실측 |
| `allow-build-scripts`에 실제로 빌드 스크립트가 있는 34개(`src-tauri` 포함)를 적는다 | `include-workspace = true`라서 작업 공간의 `build.rs`도 검사한다. `src-tauri/build.rs`는 harness.md 3.2의 유일한 예외다 | `include-workspace = false` | harness.md 3.2 |
| `bit-vec`의 `crusader.sh`, `cookie`의 `scripts/test.sh`는 `bans.build.bypass`에 sha-256과 함께 둔다 | 빌드에 쓰이지 않는 저장소 스크립트다. 내용이 바뀌면 다시 실패한다 | `executables = "allow"` | 실측 |
| 하네스 자체 테스트는 `crates/xtask`의 `cfg(test)` 모듈(`rust_lints`, `third_party`)에 둔다. 위반 예제는 `harness/rust-lints/{forbid,deny,clean}/<린트>.rs`(파일 이름이 기대 린트, `__`가 `::`), `harness/deny/<사례>/`에 둔다. 테스트는 루트의 `[workspace.lints]`, `clippy.toml`, `rust-toolchain.toml`, `deny.toml`을 읽어 `target/harness/` 아래에 따로 된 작업 공간을 만든다 | 예제는 실제 작업 공간 멤버가 아니다. 설정을 복사하지 않고 읽으므로 린트 하나를 빼면 테스트가 실패한다. 주석을 쓸 수 없어 기대값을 파일 이름에 둔다 | 예제 작업 공간을 저장소에 고정(설정이 두 벌이 된다), trybuild | harness.md 6.8 |
| 취약점 검사는 테스트가 만든 가짜 권고 DB(git 저장소)로 확인한다. 폴더 이름 `advisory-db-3157b0e258782691`은 cargo-deny 0.20.2가 기본 주소에서 만드는 이름이다 | 테스트가 네트워크를 쓰지 않는다 | 실제 RustSec DB(네트워크) | harness.md 6.2 |

## 결과

- `src-tauri/src/tests.rs`의 기존 테스트 한 줄을 `assert!(built.is_ok())`에서 `.expect(…)`로 바꿨다. 판정은 같다. 기존 테스트 변경이므로 사용자 확인이 필요하다.
- `generate_context!` 밖에서 `include_bytes!`·`env!`·`exit`·`HashMap`을 쓰면 여전히 실패한다. 다만 2.1 형태로 끌 수 있는 수준이 됐으므로, 끄기는 `xtask disable-report`(H-40)와 리뷰가 본다.
- `concat!(env!(…))`처럼 내장 매크로 인자 안의 `env!`는 `disallowed_macros`가 잡지 못했다(H-01의 `workspace_layout.rs`). xtask 검사(H-05)에서 다룬다.
- `just check`의 `cargo deny`는 권고 DB를 받으려고 네트워크를 쓴다. 테스트 네트워크 차단(6.2)은 nextest 안에만 걸리므로 충돌하지 않는다.
- cargo-deny를 올리면 가짜 권고 DB 폴더 이름을 다시 확인한다.
- 바뀌면 다시 볼 조건: Tauri가 `generate_context!` 대신 다른 조립 방법을 낼 때, clippy가 외부 매크로 확장 안의 `disallowed_*`를 보고하지 않게 될 때, cargo-deny가 경로 crate에도 권고를 맞추게 될 때.
