# 프로젝트 하네스 (v0.2)

목표: **AI가 잘못된 코드를 작성할 수 없을 만큼 엄격한 환경.**

규칙을 문서로 부탁하지 않는다. 위반하면 빌드, 린트, 훅, CI 중 하나가 실패하게 만든다. 도구로 막지 못하는 것은 9장 "리뷰에서 확인할 항목"에 모으고, 리뷰 단계([workflow.md](workflow.md))에서 본다.

이 문서에는 규칙과 강제 방법만 쓴다. 설정 파일 초안은 리서치 파일에 있다.

| 설정 | 초안 위치 |
|---|---|
| `tsconfig.json` | [R-08](research/R-08-typescript-strictness.md) 7.1 |
| `eslint.config.mjs`, 로컬 규칙 | R-08 7.2, 7.3 |
| `knip.json`, `.dependency-cruiser.cjs` | R-08 7.5 |
| 작업 공간 `[workspace.lints]` | [R-09](research/R-09-rust-strictness.md) 6.1 |
| `clippy.toml` | R-09 6.2 |
| `deny.toml` | R-09 6.3 |
| 검사 스크립트, nextest 설정 | R-09 6.4 |
| xtask 검사 항목 | R-09 6.5 |
| `.claude/settings.json`, 훅 스크립트 | [R-10](research/R-10-agent-harness.md) 3.2 |
| `justfile`, `lefthook.yml`, CI 워크플로, 보호 경로 검증 스크립트 | R-10 3.4 |
| 테스트 네트워크 차단 (`.cargo/config.toml` runner) | [R-15](research/R-15-tauri-testing.md) 2.7 |
| 디자인 시스템 린트 규칙 | [R-14](research/R-14-design-system-to-code.md) 2.2, 3.2 |

결정의 근거와 버린 대안은 [ADR 0003](adr/0003-harness-decisions.md)에 있다.

## 1. 방어 단계

| 단계 | 언제 | 무엇이 막는가 |
|---|---|---|
| 1. 타입 | 작성 중 | 컴파일러. 불가능한 상태를 타입으로 없앤다 |
| 2. 린트 | 파일 편집 직후 | 금지 문법, 금지 가져오기, 주석, 검사 끄기 |
| 3. 구조 검사 | `just check` | crate·폴더 의존 방향, 금지 참조, 금지 파일, 설정 상속 |
| 4. 테스트 | `just check` | 단위, 속성, 계약, 경로 고정, 커버리지 |
| 5. 에이전트 훅 | 도구 호출 전후, 서브에이전트 시작·종료 | 보호 경로 편집 차단, 금지 명령 차단, 편집 직후 검사, 종료 전 검사, 리뷰어 격리 확인 |
| 6. 샌드박스 | 셸 명령 실행 중 | 셸과 그 자식 프로세스의 보호 경로 쓰기 |
| 7. git 훅 | 커밋·푸시 | `just check`, `just check-full` |
| 8. CI | 푸시 뒤, `main` 푸시, 밤 | 사후 탐지: 보호 경로 서명 검사, Linux에서 도는 검사, macOS의 전체 `just check`, 리뷰 기록 검사. 실패하면 알린다 (7.6) |
| 9. 리뷰 | 작업 완료 후 | 적대적 리뷰 → 비판 → 평가. 9장 항목 |

### 1.1 검사 명령

| 명령 | 내용 | 누가 부르는가 |
|---|---|---|
| `just lint-file <경로>` | 바뀐 파일 하나의 형식과 린트 | `PostToolUse` 편집 훅 |
| `just check` | 환경 고정, 형식, 린트, 타입, xtask 구조 검사, 테스트(경로 고정 포함), 커버리지, 생성물 차이, 기존 테스트 변경 검사 | `SubagentStop`(implementer), 커밋 훅, CI `master` 푸시와 밤(macOS). Linux 푸시는 `just linux-check`·`just linux-test` |
| `just check-full` | `just check` + 변이 테스트(기준 브랜치 대비 변경분) + 컨테이너 시각 회귀 + 순환 의존 검사 | 푸시 훅, CI |

`just check`의 공통 규칙:

| 규칙 | 이유 |
|---|---|
| 첫 단계에서 `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, `RUSTDOCFLAGS`, `CLIPPY_CONF_DIR`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_TARGET_DIR`를 지우고 `RUSTFLAGS="-D warnings"`로 고정한다 | 이 변수들로 `deny` 린트를 끄거나 설정을 바꿀 수 있다. `RUSTFLAGS`를 설정하면 `build.rustflags`와 상위 `.cargo/config.toml`의 rustflags도 무시된다 (R-09 Q2) |
| 모든 cargo 명령에 `--locked`를 붙인다 | 잠금 파일 밖의 의존성 변화 차단 |
| Rust 테스트는 `cargo-nextest`로만 돈다 | 네트워크 차단과 테스트 필터를 한곳에서 건다 (R-09 Q9) |
| ESLint에 `--cache`를 쓰지 않는다 | 파일 단위 캐시는 다른 파일의 타입 변경을 반영하지 못한다 (R-08 2.3) |
| 경고는 오류다: ESLint `--max-warnings 0`, Rust `-D warnings` | 경고를 쌓아 두는 길을 없앤다 |

### 1.2 언제 무엇을 돌리는가

전체 `just check`는 로컬 macOS와 macOS CI가 맡는다. 작업 공간에 macOS 전용 crate가 있고 `cfg`와 매니페스트 `[target]` 표를 금지하므로(3.2) Linux에서는 작업 공간 전체를 빌드하지 않는다. Linux CI는 서명 검사와 플랫폼 중립 crate 묶음만 돈다. 플랫폼 중립 묶음은 작업 공간에서 보호된 제외 목록(macOS 전용 crate와 그것에 의존하는 crate)을 뺀 것이다. macOS 러너는 Linux보다 분당 약 10배 비싸서 푸시마다 돌리지 않는다 (R-15 2.6).

| 언제 | 어디서 | 내용 |
|---|---|---|
| 에이전트 훅, 커밋 훅 | 로컬 macOS | `just check` |
| 푸시 훅 | 로컬 macOS | `just check-full` |
| CI 푸시마다 | Linux arm64 | 보호 경로 서명 검사 (7.6), 리뷰 기록 검사, 작업 흐름 정책, 플랫폼 중립 crate 묶음의 형식·린트·deny·xtask 검사·테스트(네트워크 이름공간 안). 컨테이너 시각 회귀는 DS-15에서 더한다 (ADR 0020) |
| CI `master` 푸시와 밤 | macOS | `lefthook install` 뒤 전체 `just check`. `src-tauri` 경로 고정 테스트, 앱 빌드, E2E 연기 테스트, WKWebView 스크린샷 보관(비교하지 않음)은 해당 작업(U-03 등)에서 더한다 |
| CI 밤 | Linux | Linux 전체 묶음, `just mutants-all`, 권고 DB를 받은 뒤 `deny`. 오래 걸리는 속성 테스트와 AI 고정 자료의 CLI 버전 점검은 해당 작업에서 더한다 |

밤 macOS 작업은 그날 `main`에 변경이 있을 때만 돈다.

### 1.3 도구 버전 고정

도구 버전은 저장소에 고정한다 (open-issues I-17). 아래 버전은 리서치에서 실제로 돌려 본 버전이다.

| 도구 | 고정 | 근거 |
|---|---|---|
| Rust 툴체인 | `rust-toolchain.toml`에 1.99.1. Tauri 2.12는 1.90 이상이 필요하다 | R-09, R-05 |
| TypeScript | 타입 검사는 TS 7.0.2(`tsc`), 린트는 TS 6.0 API. npm 별칭으로 함께 설치: `"typescript": "npm:@typescript/typescript6@^6.0.2"`, `"@typescript/native": "npm:typescript@^7.0.2"` (open-issues I-16) | R-08 2.1 |
| Node | 24.22.0. `.node-version`으로 고정 | R-08 6.1 |
| CI 도구 | just 1.59.0, cargo-nextest 0.9.151, cargo-deny 0.20.2, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, lefthook 2.2.2, pnpm 12.18.0(`packageManager`). 목록은 `harness/ci/tools.json` | ADR 0020 |
| `tauri`, `tauri-specta` | `=` 고정 (`tauri::test`가 unstable, tauri-specta가 RC) | R-15 4장 |
| Tailwind | 정확한 버전 고정. 올릴 때 "생성되는 클래스 목록" 스냅샷을 다시 돈다 | R-14 5장 |
| `ra-ap-rustc_lexer` | 정확한 버전 고정 (매주 배포되고 `TokenKind`가 바뀐다) | R-09 Q8 |
| Claude Code | 올릴 때마다 6.8의 하네스 회귀 테스트를 돈다 | R-10 4장 |

## 2. 공통 규칙

| 규칙 | 강제 방법 |
|---|---|
| 주석 금지. 설명은 이름, 타입, 테스트, `docs/`에 쓴다 | TS: 로컬 규칙 `local/no-comments`(JSX 주석, `///` 지시문 포함). Rust: xtask가 `ra-ap-rustc_lexer` 토큰으로 주석(문서 주석 포함)을, `proc-macro2` 토큰 트리로 `#[doc]`를 찾는다. 예외는 보호된 생성 경로 목록뿐이다 |
| 검사 끄기는 꼭 필요한 곳에서만, 정해진 형태로만 한다 | 2.1. 그 밖의 끄는 속성·지시문은 어디에 쓰든 실패한다 (3.2, 4.2) |
| 경고는 오류다 | 1.1 |
| 생성 파일은 손으로 고치지 않는다. 생성 파일로 인정하는 것은 보호된 생성 경로 목록에 있는 것뿐이고, 파일 안의 표식으로 판정하지 않는다 | 다시 생성해서 차이가 나면 실패 |
| 의존성 추가는 허용 목록에 있어야 한다. 목록 변경은 사람 승인 | Rust: `cargo-deny`의 `bans.allow`(전이 crate까지 명시). TS: 허용 목록 검사 스크립트. 목록 파일은 보호 파일 |
| `TODO`, `FIXME`, 미완성 구현 금지 | 주석 금지로 대부분 해결. `todo!`, `unimplemented!`는 `forbid` |
| 죽은 코드 금지 | Rust: 컴파일러 경고, 미사용 의존성은 `cargo-machete`. TS: `noUnusedLocals`, `no-unused-vars`, 파일 사이는 `knip` |
| 테스트 건너뛰기 금지 (`#[ignore]`, `#[ignore = "…"]`, `.skip`, `.only`, `.todo`) | Rust: xtask 금지 속성. TS: 린트 |
| 기존 테스트 삭제·약화 금지 | `xtask test-diff` (7.7) |
| 구현 코드가 테스트를 알아보는 분기 금지 (`cfg(test)` 분기, 테스트 이름·경로 문자열 참조) | xtask. `cfg(test)`는 테스트 모듈 선언에만 허용 |

### 2.1 허용하는 검사 끄기

주석은 금지지만, 규칙을 지킬 방법이 없는 곳에서는 아래 형태의 끄기만 쓸 수 있다 (사용자 결정 2026-11-12).

| 언어 | 허용하는 형태 | 조건 |
|---|---|---|
| TypeScript | `// eslint-disable-next-line <규칙 하나> -- <이유>` | 다음 한 줄에만 적용. 규칙 이름 하나와 이유가 반드시 있다. 파일·블록 단위(`eslint-disable`, `eslint-enable`), 규칙 이름 없는 끄기, 여러 규칙을 한 번에 끄기는 실패 |
| Rust | `#[expect(<린트 하나>, reason = "<이유>")]` | 항목이나 문장 하나에만 붙인다. crate·모듈 단위(`#![expect]`)와 `#[allow]`는 실패. `expect`는 린트가 실제로 걸리지 않으면 스스로 실패하므로 남은 끄기가 생기지 않는다 |

- **끌 수 없는 규칙**이 있다. 목록은 보호 파일이다.
  - Rust: 3.1의 `forbid` 린트 전부(panic 계열, `as` 변환, 산술, 부동소수점, 무시된 결과, `unsafe_code` 등). `forbid`는 `expect`로도 꺼지지 않는다. 끌 수 있는 것은 `pedantic`, `nursery`, `cargo` 그룹에 속한 린트뿐이다. 개별로 `deny`인 rustc·clippy 린트와 `disallowed_methods`·`disallowed_types`·`disallowed_macros`는 컴파일러로는 `expect`가 통하지만 xtask(H-40)가 실패시킨다. 예외는 보호된 파일 허용 목록 하나다: `apps/desktop/src-tauri/src/main.rs`의 `tauri::generate_context!()` 문장에 붙은 `disallowed_*` 세 개(ADR 0005).
  - TypeScript: 넘침 검사와 스크롤·넘침 클래스 규칙(4.4), 금액 변환(`local/no-money-coercion`), 금액을 꺼내는 길(`Reflect`, `prototype`, `call`·`apply`·`bind`, `getOwnPropertySymbols`), 숫자 변환 금지, 경계(`boundaries/*`), 네트워크, 전역 `document`·`navigator`·`location`·저장소 금지, 효과 훅·ref 콜백·`useSyncExternalStore` 금지, 타입 술어·단언 함수 금지, `.catch`와 두 인자 `.then` 금지, 디자인 시스템 화면 코드 금지(4.4), `ban-ts-comment`, 테스트 건너뛰기, 금지 확장자, `local/forbidden-file`, `local/no-comments`, 끄기 형태 규칙. 원칙: 파일 허용 목록으로 예외를 주는 금지는 인라인으로 끌 수 없다. 끌 수 있으면 허용 목록이 의미가 없다 (ADR 0009). `@eslint-community/eslint-comments/no-restricted-disable`로 막는다.
- `@ts-ignore`, `@ts-expect-error`는 계속 금지다. 타입 검사는 끌 수 없다.
- xtask 검사(주석, 원시 타입, 소스 위치, 매니페스트)는 끄는 수단이 없다. 예외는 보호된 허용 목록으로만 둔다.
- 쓰이지 않는 끄기는 오류다(`reportUnusedDisableDirectives`).
- `xtask disable-report`가 저장소의 모든 끄기를 파일, 규칙, 이유와 함께 출력하고 `just check` 끝에 개수를 보여 준다. 변경에 새 끄기가 있으면 적대적 리뷰어가 하나씩 "규칙을 지키는 다른 방법이 없었는지" 판정한다 (RV 항목, workflow 2.3). 리뷰 기록에 끄기 목록이 없으면 리뷰 기록 검사가 실패한다.

## 3. Rust

### 3.1 린트 수준

린트는 루트 `Cargo.toml`의 `[workspace.lints]` 한곳에만 둔다. 모든 crate는 정확히 `lints = { workspace = true }`만 갖는다. crate가 `[lints]` 절을 지우면 cargo는 조용히 모든 린트를 끄므로 xtask가 이것을 검사한다 (R-09 Q2).

| 수준 | 대상 | 이유 |
|---|---|---|
| `forbid` (개별) | 아래 표의 핵심 린트 | 코드 속 `#[allow]`, `#![allow]`, `#[expect]`와 `RUSTFLAGS=-A …`로 끌 수 없는 것은 `forbid`뿐이다. 그래서 2.1의 끄기는 이 린트에 통하지 않는다 |
| `deny` (그룹) | `clippy::pedantic`, `clippy::nursery`, `clippy::cargo` | 그룹을 `forbid`로 두면 뒤의 개별 예외가 무시되고 `forbidden_lint_groups` 경고가 난다 |
| 끔 | `missing_errors_doc`, `missing_panics_doc`(문서 주석 요구), `redundant_pub_crate`(`unreachable_pub`과 반대 요구), `missing_const_for_fn`, `option_if_let_else`, `significant_drop_tightening`, `cargo_common_metadata`, `multiple_crate_versions` | 주석 금지와 충돌하거나 서로 반대를 요구한다 (R-09 6.1) |

`forbid` 린트 (R-09 Q1에서 위반 예제로 확인):

| 목적 | 린트 |
|---|---|
| 안전 | `unsafe_code`. 예외는 `platform-macos` 하나에만 보호된 형태로 둘 수 있고, 둘지는 O-07의 ADR에서 정한다 ([architecture.md](architecture.md) 4.5) |
| panic 계열 | `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `panic_in_result_fn`, `unwrap_in_result`, `get_unwrap`, `exit`, `mem_forget`, `fallible_impl_from` |
| 인덱스 | `indexing_slicing`, `string_slice`. 범위 밖이면 panic하는 표준 메서드는 `disallowed-methods` 목록으로 막는다 |
| `as` 변환 | `as_conversions`, `as_underscore`, `as_pointer_underscore`. `try_from`을 쓴다 |
| 넘치는 산술 | `arithmetic_side_effects`, `integer_division`, `modulo_arithmetic`. `rust_decimal`의 연산자도 잡히므로 `checked_*`만 쓴다 |
| 부동소수점 | `float_arithmetic`, `float_cmp_const`, `lossy_float_literal`, `default_numeric_fallback` (작업 공간 전체) |
| 와일드카드 분기 | `wildcard_enum_match_arm`, `rest_pat_in_fully_bound_structs`. `disallowed-macros`에 `std::matches` |
| 무시된 결과 | rustc `unused_must_use`, `let_underscore_must_use`, `let_underscore_untyped`, `unused_result_ok`, `map_err_ignore` |
| 끄기 속성 | `allow_attributes`(`#[allow]` 금지, `#[expect]`만), `allow_attributes_without_reason`(이유 없는 `#[expect]` 금지) |
| 출력 | `print_stdout`, `print_stderr`, `dbg_macro` |
| 불안정 기능 | `unstable_features` (`#[coverage(off)]` 같은 우회 차단) |

`clippy.toml`(루트 하나)의 `disallowed-methods`, `disallowed-types`, `disallowed-macros`가 막는 것: 범위 밖 panic 메서드, `HashMap`·`HashSet`·`RefCell`·`Rc`·`Mutex`, `mem::drop`·`mem::swap`·`mem::replace`·`mem::take`, `Result::unwrap_or*`, `thread::sleep`, `Decimal::from_f64_retain`·`to_f64`, `include*!`, `print`·`eprint`. 목록 전문은 R-09 6.2.

현재 시각과 환경 변수 읽기는 clippy가 아니라 xtask가 막는다. clippy 목록은 파일별 예외를 둘 수 없고, 예외를 두려면 끄기 속성이 필요하기 때문이다. xtask는 `SystemTime::now`, `Instant::now`, `env::var`, `env::var_os`, `env::vars`, `env::vars_os`와 서드파티 crate의 현재 시각 함수(시간 타입 crate의 `now` 계열, `tokio::time::Instant::now` 등)를 보호된 파일 허용 목록 밖에서 실패시킨다. 처음 목록은 시계 어댑터 파일 하나와 조립 지점(`src-tauri/src/main.rs`)이다. 시각은 `Clock` 포트로 받는다.

`disallowed-types`가 Tauri 상태 관리나 서드파티 경계에서 실제로 막히면 ADR로 조정한다.

### 3.2 검사 끄기를 막는 세 겹

clippy와 rustc만으로는 끄기를 막지 못한다. 세 겹을 함께 쓴다 (R-09 요약 2, Q13).

| 겹 | 막는 것 | 못 막는 것 |
|---|---|---|
| ① 핵심 린트 개별 `forbid` | 그 린트에 대한 `#[allow]`, `#![allow]`, `#[expect]`(E0453), `RUSTFLAGS=-A` | `deny` 수준 린트의 끄기, `--cap-lints` |
| ② xtask 토큰 검사 | 아래 표의 속성·매크로·파일 | 환경 변수 |
| ③ 환경 고정 | `RUSTFLAGS`, `--cap-lints`, `CLIPPY_CONF_DIR`, `RUSTC_WRAPPER` 등. `just check`가 지우고 고정한다(1.1). 셸 훅이 거부하고(7.3), 깨끗한 CI가 다시 돈다 | cargo 설정 파일의 `[env]`·`[alias]`(저장소 `.cargo/`는 xtask H-23, 상위 폴더와 `CARGO_HOME`은 샌드박스 H-35와 CI), `PATH`의 가짜 `cargo`(샌드박스와 CI) (ADR 0005) |

xtask가 실패시키는 것 (`proc-macro2` 토큰 트리를 재귀로 돌아서 매크로 호출 안의 속성도 본다):

| 대상 | 내용 |
|---|---|
| 속성 | `allow`, 2.1 형태가 아닌 `expect`(내부 속성 `#![expect]`, 이유 없음, 린트 여러 개), `warn`, `deny`, `forbid`, `doc`, `path`, `ignore`, `cfg_attr`(전부), `no_mangle`, `export_name`, `link_section`, `automatically_derived`, `mutants::*`, `clippy::*` 도구 속성, `rustfmt::*`. `cfg(test)`가 아닌 모든 `cfg`(`cfg(not(clippy))`, `cfg(any())`, `cfg(coverage)` 포함) |
| 매크로 | `macro_rules!`, `include!`·`include_str!`·`include_bytes!`, `asm!`, `global_asm!` |
| 매니페스트 | `lints = { workspace = true }` 외의 `[lints]`, `[target]`, `[profile]`, `[patch]`, `[replace]`, `[features]` 표 |
| 파일 | 작업 공간 안의 `build.rs`와 proc-macro crate, `crates/` 아래의 `clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `rust-toolchain*`, `.cargo/`. 예외는 Tauri가 요구하는 `apps/desktop/src-tauri/build.rs` 하나이고, 이 파일은 보호 파일이다. 이 파일은 명령 목록을 생성 파일에서 읽으므로 명령을 추가해도 바뀌지 않는다 (5장) |
| `domain` | `f32`, `f64` 식별자 |

생성 crate(5장)만 이 검사에서 제외한다. 제외 여부는 보호된 경로 목록으로만 정한다.

### 3.3 lints가 강제하는 작성 규약

| 규약 | 강제하는 것 |
|---|---|
| 테스트 함수는 `()`를 반환하고 실패는 `expect`로 낸다. `Result`를 반환하는 테스트는 쓰지 않는다 | `panic_in_result_fn`, `clippy.toml`의 `allow-expect-in-tests = true`, `allow-unwrap-in-tests = false` |
| 통합 테스트도 `#[cfg(test)] mod` 안에 쓴다 | `tests_outside_test_module` |
| 소비형 토큰(승인 등)은 `self`를 받는 메서드로 쓴다 | pedantic `needless_pass_by_value` |
| 금액 계산은 `rust_decimal`의 `checked_*`만 쓴다. 통계 함수(`sqrt`, `powd`)도 `Decimal`로 계산한다 (R-11) | `arithmetic_side_effects`, `float_arithmetic` |
| 벽시계 시각과 대기는 `Clock` 포트로 받는다 | 대기는 `disallowed-methods`, 시각 읽기는 xtask (3.1) |
| 컴파일 실패를 기대하는 테스트(`trybuild`)는 `tests/` 폴더에 둔다 | 필터로 뺀 `src/` 안 테스트가 커버리지를 깎는다 (R-09 Q7) |

### 3.4 타입 설계 규칙

리뷰에서 확인하고, 가능한 것은 xtask로 검사한다.

- **원시 타입을 그대로 쓰지 않는다. 모든 값은 전용 타입(newtype)이다.** `domain`, `application`, 포트 crate의 함수 시그니처, 구조체 필드, 열거형 변형 필드에 `String`, `&str`, 정수, 부동소수점, `Decimal`, `Vec<u8>`, 그리고 이들을 담은 `Option`·`Vec`·튜플을 쓰면 `xtask raw-types`가 실패한다.
  - `bool`도 금지다. 매개변수, 반환 타입, 필드 모두 해당한다. 두 가지뿐인 값도 열거형으로 쓴다(`OrderSwitch::On`/`Off`, `Judged::Yes`/`Pending`). 열거형은 나중에 세 번째 경우를 더할 수 있고 호출부에서 뜻이 드러난다. 조건식 안에서 생기는 `bool`(`a == b`, `if`)은 그대로 쓴다. 표준·외부 트레이트 구현(`PartialEq::eq` 등)의 시그니처는 예외다.
  - 허용: newtype을 정의하는 튜플 구조체의 비공개 필드 하나, 그 newtype의 생성자와 `as_*` 접근자.
  - 경계 어댑터(`broker-toss-schema`의 생성 타입, DB 행, IPC DTO, 외부 응답 DTO)는 원시 타입을 가질 수 있다. 어댑터는 값을 받자마자 생성자로 전용 타입으로 바꾸고, 원시 타입을 포트 밖으로 내보내지 않는다. 예외 crate 목록은 보호 파일이다.
  - 타입 별칭(`type Price = Decimal`)은 전용 타입이 아니다. 같은 검사로 막는다.
- 검증은 생성자에서 한 번 한다. 만들어진 값은 항상 유효하다.
- 상태 전이는 타입 상태나 열거형으로 표현한다. `bool`은 시그니처와 필드 어디에도 쓰지 않는다(`xtask raw-types`, `max-fn-params-bools = 0`).
- 금액은 십진수 타입만 쓴다.
- 커버리지에 걸리는 방어 분기는 타입으로 없앤다 (6.3).
- 주문 토큰의 구조(비공개 필드, `Clone` 없음, DB에서 `Validated` 복원 금지, `&mut` 접근자 없음)는 [architecture.md](architecture.md) 4.3을 따른다. 위조 시도는 `trybuild` compile-fail 테스트로 고정한다.

### 3.5 도구

| 도구 | 버전 | 용도 |
|---|---|---|
| `rustfmt` | 툴체인 | 형식 |
| `clippy` | 툴체인 | 3.1 |
| `cargo-deny` | 0.20.2 | 서드파티 허용 목록(전이 포함), 라이선스, 취약점, 출처, 서드파티 빌드 스크립트. 작업 공간 crate 사이의 의존 방향은 맡지 않는다 |
| `cargo-nextest` | 0.9.151 | 테스트 실행 |
| `proptest` | 1.12.0 | 속성 테스트 |
| `cargo-mutants` | 27.1.0 | 변이 테스트 (6.4) |
| `cargo-llvm-cov` | 0.9.1 | 커버리지. crate별로 `-p`, 파일별 기준 `--fail-under-file-lines` |
| `cargo-machete` | 0.9.2 | 미사용 의존성 |
| `trybuild` | 1.0.122 | 주문 토큰 위조가 컴파일되지 않는지 |
| `typify` | 0.8.0 | 토스 스펙의 `components.schemas` → Rust 타입 (5장) |
| `insta` | 1.49.0 | 스냅샷 (명령별 대표 응답, 시나리오 결과) |
| `xtask arch-check` | - | `cargo metadata`의 일반·빌드·개발 의존을 허용 간선 표와 비교 ([architecture.md](architecture.md) 3.2, 4.6). 표에 없는 crate가 나타나면 실패. 제품 바이너리 그래프에 E2E 자동화 플러그인, `testkit`, `trade-fakes`가 있으면 실패. 재수출 금지, `impl UserPresence` 위치, `UserPresence`의 별칭 가져오기(`use … as`)와 다시 내보내기 금지 |
| `xtask` 소스·매니페스트·설정 검사 | - | 3.2 |
| `xtask test-diff` | - | 기존 테스트 삭제·변경 검출 (7.7) |
| `xtask lint-file` | - | 편집 직후 한 파일 검사 |
| `xtask gen-tokens`, `xtask tokens-contrast` | - | 디자인 토큰 생성과 대비 검사 (4.4) |
| `xtask capability-check` | - | 생성된 명령 목록과 capability의 명령 허용 항목(같은 목록에서 생성)이 같은지. `build.rs`가 명령 목록을 생성 파일에서 읽는지. 위험한 권한은 [architecture.md](architecture.md) 3.6의 목록이고 있으면 실패. `tauri.conf.json`에 `app.security.capabilities` 명시 (R-05) |
| `xtask migration-check` | - | 적용된 마이그레이션 불변 (R-06) |
| SQL 검사 | - | `store-sqlite`의 SQL 상수에서 금액 열에 `sum`, `avg`, `ORDER BY`, 산술 금지 (R-06) |
| 문자열 위치 검사 | - | `tossinvest.com`은 조립 루트와 `toss-smoke`에만 (6.2). 공개 IP 확인 서비스 주소는 조립 루트에만 (design 5.1, 서비스는 U-07에서 정함). 토스 주문 경로는 문자열이 아니라 HTTP 메서드와 경로의 조합으로 본다: 쓰기 메서드와 주문·조건주문의 생성·정정·취소 경로는 `broker-toss-trade`에만, 예외는 토큰 발급 (architecture.md 3.2) |
| 시각·환경 변수 검사 | - | 현재 시각 함수(서드파티 포함)와 `env::var`·`env::var_os` 계열이 보호된 파일 허용 목록 밖에 있으면 실패 (3.1) |

xtask의 외부 의존은 `cargo_metadata`, `syn`, `proc-macro2`, `ra-ap-rustc_lexer`, `toml`이다(R-09 6.3). 작업 공간 crate에는 의존하지 않는다(architecture.md 3.2).

cargo-vet은 쓰지 않는다. 버전마다 감사 비용이 커서 개인 앱에 맞지 않는다 (R-09 Q10).

## 4. TypeScript

원칙: **TypeScript를 Rust처럼 쓴다.**

| Rust의 성질 | TypeScript에서 강제하는 방법 |
|---|---|
| `Result`, 예외 없음 | `throw`, `try/catch`, `Promise.reject` 금지. 실패는 직접 정의한 판별 유니온 `Result`(`shared/lib/result.ts`)로 반환 |
| 무시할 수 없는 `Result` | 반환값이 있는 호출을 문장으로 쓰면 오류, 받아서 안 쓰면 오류, `void`로 버리면 오류. 비동기 결과를 버리는 유일한 수단은 `shared/platform/detach` |
| 널 없음 | `strictNullChecks`, `!` 단언 금지, `noUncheckedIndexedAccess` |
| 빠짐없는 `match` | 판별 유니온과 `switch-exhaustiveness-check`. `default` 팔 금지 |
| 기본 불변 | `let` 금지, 배열·객체 변경 금지, `readonly` 강제 |
| 강한 타입 변환 | `as` 금지(`as const`만 허용), `any` 금지, 타입 술어(`x is T`)·단언 함수 금지, 오버로드·`declare`·`namespace`·`declare global`·`.d.ts` 금지, `JSON.parse` 금지, 스키마 `.parse()` 금지(`safeParse`만). `unknown`은 경계에서 Zod로 파싱 |
| newtype | **원시 타입을 그대로 쓰지 않는다.** 타입 자리에 `string`, `number`, `bigint`, `boolean`을 쓸 수 없다. 모든 값은 브랜드 타입이고, 두 가지뿐인 값은 리터럴 유니온(`'on' \| 'off'`)이다 (4.2 "원시 타입" 행). 계산을 막아야 하는 값(`Money`, `Quantity`, `Price`, `Ratio`, `Probability`)은 비공개 `unique symbol` 키를 가진 **불투명 객체**다 (open-issues I-15). 그 밖의 값(ID, 종목 코드, 시각, 개수, 화면 문구)은 Zod 브랜드다 |
| 트레이트와 구조체 | 클래스·상속·`this` 금지. 함수와 타입만 쓴다 |
| 미사용은 오류 | `noUnusedLocals`, `noUnusedParameters`, 무시 패턴 없는 `no-unused-vars`, `knip` |

문자열 브랜드 금액을 쓰지 않는 이유: `string`이라서 `parseFloat(a) + Number(b)`와 `a + b`를 TS 7과 TS 6 모두 오류 없이 통과시킨다 (R-08 2.9).

### 4.1 tsconfig

타입 검사는 `tsc`(TS 7), 린트는 TS 6 API로 한다 (1.3). 같은 위반 예제를 두 컴파일러로 돌려 결과가 같은지 확인한다 (6.8).

켜는 옵션: `strict`와 그 9개(`noImplicitAny`, `strictNullChecks`, `strictFunctionTypes`, `strictBindCallApply`, `strictPropertyInitialization`, `strictBuiltinIteratorReturn`, `noImplicitThis`, `useUnknownInCatchVariables`, `alwaysStrict`)를 명시, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`, `noPropertyAccessFromIndexSignature`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedSideEffectImports`, `verbatimModuleSyntax`, `isolatedModules`, `erasableSyntaxOnly`, `forceConsistentCasingInFileNames`, `noErrorTruncation`.

끄거나 고정하는 옵션: `allowUnreachableCode: false`, `allowUnusedLabels: false`, `libReplacement: false`, `types: []`, `skipLibCheck: false`, `allowJs: false`, `checkJs: false`.

`isolatedDeclarations`는 쓰지 않는다. `noEmit` 앱 설정과 맞지 않고, 같은 효과를 ESLint `explicit-module-boundary-types`로 얻는다.

### 4.2 ESLint

기반: ESLint + `typescript-eslint`의 `strict-type-checked`, `stylistic-type-checked`. 플러그인: `eslint-plugin-functional`, `eslint-plugin-boundaries`, `eslint-plugin-react-hooks`, `@eslint-react/eslint-plugin`, `@eslint-community/eslint-plugin-eslint-comments`, 로컬 규칙 4개(`local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`, `local/no-raw-types`). 아래를 모두 오류로 둔다.

| 분류 | 규칙 |
|---|---|
| 타입 | `no-explicit-any`, 모든 `no-unsafe-*`, `consistent-type-assertions: never`, `no-unsafe-type-assertion`, `no-non-null-assertion`, `strict-boolean-expressions`(허용 옵션 모두 `false`), `explicit-function-return-type`, `explicit-module-boundary-types`, `method-signature-style: property`(메서드 단축 문법은 `strictFunctionTypes`를 피한다) |
| 원시 타입 | `local/no-raw-types`: 타입 자리의 `string`, `number`, `bigint`, `boolean` 키워드(`TSStringKeyword`, `TSNumberKeyword`, `TSBigIntKeyword`, `TSBooleanKeyword`)와 `String`·`Number`·`BigInt`·`Boolean` 타입 참조를 금지한다. 매개변수, 반환 타입, 속성, 제네릭 인자(`Array<string>`, `Record<string, T>`), 유니온 구성원 모두 해당한다. 리터럴 유니온(`'buy' \| 'sell'`)은 허용한다. `boolean` 대신 리터럴 유니온을 쓴다(TS `enum` 문법은 금지라서 유니온이 열거형이다). `true`·`false` 리터럴 타입도 타입 자리에서는 금지다. 조건식 안에서 생기는 불린 값은 그대로 쓴다. 타입 정보로 추론된 타입도 본다: 내보내는 함수의 매개변수·반환 타입과 컴포넌트 props에 원시 타입이 남아 있으면 실패한다. 브랜드는 `shared/lib/brand`의 생성 함수와 `shared/ipc`의 Zod 스키마로만 만든다. 화면 문구는 `UiText` 브랜드이고 `messages.ts` 파일에서만 만든다. `messages.ts`는 문구 키마다 언어별 문장을 갖는 목록이라 다국어를 바로 붙일 수 있다(지금은 한국어만). 화면 코드와 `shared/ds`에 문자열 리터럴 문구와 JSX 글자를 직접 쓰면 실패한다 |
| 거짓 타입 | `no-restricted-syntax`로 `TSTypePredicate`, `TSDeclareFunction`(오버로드), `declare`, `TSModuleDeclaration` 금지 |
| 실패 처리 | `.catch(…)`와 두 인자 `.then(…, onRejected)` 금지(거절을 `Result`로 바꾸는 곳은 `shared/ipc`의 호출 어댑터와 `shared/platform/detach`뿐이고 파일 허용 목록으로 둔다), `functional/no-throw-statements`, `functional/no-try-statements`, `functional/no-promise-reject`, `functional/no-expression-statements`(`ignoreVoid: true`), `no-floating-promises`(`ignoreVoid: false`, `checkThenables: true`), `no-misused-promises`, `no-void`, 무시 패턴 없는 `no-unused-vars` |
| 불변 | `functional/no-let`, `no-var`, `prefer-const`, `functional/immutable-data`, `no-param-reassign`(`props: true`), `no-plusplus`, `functional/type-declaration-immutability`, `functional/prefer-immutable-types`(매개변수만), `Object.assign` 등 변경 함수 금지, `prototype` 접근과 `call`·`apply`·`bind` 금지(`Array.prototype.push.call`이 `immutable-data`를 통과하기 때문), `Object.getOwnPropertySymbols`·`Reflect` 금지(불투명 금액에서 값을 꺼내는 길) |
| 문법 | 클래스·클래스 상속·`this` 금지, `enum`·`namespace` 금지(TS `erasableSyntaxOnly`와 겹침), 기본 내보내기·`export *`·동적 `import` 금지, 중첩 삼항 금지, 반복문 금지(`functional/no-loop-statements`), `no-eval`, `no-new-func`, `no-restricted-globals`(`globalThis`, `window` 등), 복잡도 상한(함수 길이, 파일 길이, 순환 복잡도, 매개변수 개수) |
| `switch` | `switch-exhaustiveness-check`(`allowDefaultCaseForExhaustiveSwitch: false`, `considerDefaultExhaustiveForUnions: false`), `SwitchCase[test=null]` 금지. 이 규칙의 주석 표시 기능은 주석 금지로 쓸 수 없다 |
| React | `useEffect`, `useLayoutEffect`, `useInsertionEffect` 가져오기·별칭·다시 내보내기 금지. `react` 기본·네임스페이스 가져오기, `react/*` 하위 경로 가져오기 금지. `features`에서 `useSyncExternalStore` 금지(`subscribe` 안에서 부수 효과를 실행할 수 있다). `features`에서 `ref` 화살표 함수 금지(마운트 효과 우회, R-14 규칙 11). 훅 규칙. 배열 인덱스 키 금지 |
| 주석 | `local/no-comments`(2.1 형태의 끄기 한 줄만 통과), `reportUnusedDisableDirectives: "error"`, `reportUnusedInlineConfigs: "error"`, `@eslint-community/eslint-comments/no-use`(`eslint-disable-next-line`만 허용), `require-description`, `no-unlimited-disable`, `no-restricted-disable`(끌 수 없는 규칙 목록), 한 줄에 규칙 하나, `ban-ts-comment`(`@ts-ignore`, `@ts-expect-error`, `@ts-nocheck`, `@ts-check` 모두). `tsc`는 린트와 상관없이 `@ts-ignore`를 따르므로 린트 실패가 유일한 방어선이다 |
| 숫자 | 금액 타입에 산술·비교·템플릿·`Number`/`String`/`parseFloat` 변환 금지(`local/no-money-coercion`, `restrict-plus-operands`, `no-base-to-string`). `features`에서 `Number`·`parseFloat`·`parseInt`·`BigInt` 호출, `Math.*`, 단항 `+` 일괄 금지. `shared/ds`에서 숫자 변환은 차트 컴포넌트 파일(보호된 파일 허용 목록)에서 코어가 보낸 좌표 문자열을 바꿀 때만 허용 |
| 파일 종류 | `src`에는 `.ts`, `.tsx`만. `.js`, `.mjs`, `.cjs`, `.d.ts`는 `local/forbidden-file`로 실패 (ESLint는 `files`에 맞지 않는 파일을 아예 검사하지 않으므로 확장자 차단 블록이 필요하다) |
| 네트워크 | `fetch`, `XMLHttpRequest`, `WebSocket` 사용 금지. UI는 IPC로만 통신한다 (R-15 2.7) |
| 테스트 | `.skip`, `.only`, `.todo` 금지. `@tauri-apps/api/mocks` 직접 가져오기는 `shared/ipc/testing` 밖에서 금지 (5장) |
| 경계 | `boundaries/dependencies`, `boundaries/no-unknown-files`, `boundaries/no-unknown-dependencies`로 [architecture.md](architecture.md) 5장 의존 표 강제. 다른 요소의 파일은 `index.ts`로만 가져온다. `@tauri-apps/api` 가져오기는 `shared/ipc`(생성 파일 포함)만 |
| 디자인 시스템 | 4.4 |

**예외는 폴더가 아니라 파일 허용 목록으로만 둔다.** 폴더만 예외로 두면 에이전트가 파일을 그 폴더로 옮겨 규칙을 피한다. 허용 목록은 보호 파일인 ESLint 설정 안에 있고, 목록에 없는 파일이 그 폴더에 생기면 `local/forbidden-file`이 실패시킨다.

| 허용 목록 | 풀어 주는 규칙 | 처음 목록 |
|---|---|---|
| `shared/platform` 파일 목록 | 효과 훅, `useSyncExternalStore`, `no-expression-statements`, ref 콜백 | `index.ts`, `detach.ts`, `useCoreEvent.ts`, `useDocumentTitle.ts` (R-08 7.2) |
| 타입 술어 파일 목록 | `TSTypePredicate` | `shared/lib/money.ts` (`` `${number}` ``로 좁히는 곳 하나) |
| 원시 타입 파일 목록 | `local/no-raw-types` | `shared/lib/brand.ts`, `shared/lib/money.ts`, `shared/lib/format.ts`, `shared/ipc`의 스키마 파일, 생성된 `bindings.ts`. `shared/ds`에서는 제3자 컴포넌트를 감싸는 파일만 사용자 승인으로 추가한다 |
| `throw` 허용 목록 | `no-throw-statements` | 비어 있다 |
| 차트 좌표 변환 파일 목록 | `shared/ds` 안의 숫자 변환 금지 | 비어 있다. 차트 컴포넌트 작업(DS-06, DS-17)이 사용자 승인으로 추가한다 |

`throw` 허용 목록이 비어 있는 이유: 오류 경계는 `react-error-boundary` 의존성으로 해결하고 클래스를 직접 쓰지 않는다. TanStack Router의 `redirect`는 반환 형태로 쓴다. TanStack Query의 `queryFn`은 `Promise<Result>`를 돌려주고 거절하지 않는다. 화면은 `status`와 `data.ok`를 둘 다 본다. 재시도는 코어(Rust)가 맡는다 (R-08 2.6). TanStack Router `notFound()`를 던지지 않고 쓸 수 있는지는 미확인이라, 필요해지면 그때 판단한다.

`useEffect` 없이 하는 방법:

| 필요 | 방법 |
|---|---|
| 데이터 읽기 | TanStack Query `useQuery(queryOptions)` (`features/*/model`) |
| 코어 이벤트 구독 | `shared/platform/useCoreEvent` (`useSyncExternalStore` 기반) |
| 코어 이벤트 → 쿼리 갱신 | `shared/platform`의 구독 하나가 `invalidateQueries`를 부른다 |
| 파생 값 | 렌더링 중 계산, 필요하면 `useMemo`. 상태 초기화는 `key` |
| 사용자 동작 | 이벤트 핸들러, `useMutation` |
| 화면 진입 시 동작 | 라우트 `loader`/`beforeLoad` |
| 포커스, 스크롤 | `autoFocus`, 이벤트 핸들러, `shared/ds` 컴포넌트 안의 ref |
| 타이머, 창 크기, 온라인 상태, 테마·글자 크기 | `shared/platform`의 `useSyncExternalStore` 훅 |
| Canvas 차트 같은 비React 위젯 | `shared/platform`의 전용 래퍼 |

### 4.3 도구

| 도구 | 버전 | 용도 |
|---|---|---|
| `tsc` (TS 7) / TS 6 API | 7.0.2 / 6.0.x | 4.1 |
| ESLint + `typescript-eslint` | 10.14.0 + 8.72.2 | 4.2. oxlint와 Biome은 필요한 플러그인과 타입 인식 로컬 규칙을 돌리지 못해 쓰지 않는다 |
| `eslint-plugin-functional` | 10.0.1 | 불변·실패 처리 |
| `eslint-plugin-boundaries` | 7.4.2 | 폴더 경계 (v7의 새 정책 문법만 쓴다) |
| `dependency-cruiser` | 18.7.2 | 순환 의존 (`just check-full`에서만) |
| `knip` | 6.45.1 | 미사용 파일·내보내기·의존성 |
| Zod | 4.6.7 | 경계 파싱. `safeParse`만 |
| TanStack Query, TanStack Router | 5.107.1, 1.170.46 | 데이터 읽기, 라우팅 (메모리 또는 해시 기록) |
| Vitest + Testing Library | Vitest 5.1.0 | 단위·컴포넌트 테스트 |
| tauri-specta `mockCommands` | =2.0.0-rc.28 | UI 테스트의 가짜 IPC (5장) |
| `eslint-plugin-better-tailwindcss` | 4.11.0 | `shared/ds` 안의 Tailwind 클래스 검사 (4.4) |
| stylelint + `stylelint-declaration-strict-value` | 18.0.0 + 1.13.0 | `shared/ds`의 CSS |
| Storybook | 10.6.7 | 컴포넌트 목록 화면 |
| 형식 도구 | 미정 | 리서치가 Prettier와 Biome 형식 기능을 비교하지 않았다. H-06에서 ADR로 정한다 |

### 4.4 디자인 시스템 강제

목표: 모든 화면이 디자인 시스템 안에서만 만들어진다. `features`와 `app`(화면 코드)은 `shared/ds`의 컴포넌트를 조립만 하고, 모양을 정하는 코드는 `shared/ds`에만 있다.

토큰 방식은 Tailwind v4 `@theme` + `--*: initial`이다. 기본 테마를 지워서 토큰 밖 클래스가 빌드 단계에서 생성되지 않는다 (R-14 2.2). 화면 코드에서는 클래스 자체를 쓸 수 없으므로, R-14 3.2의 클래스 관련 규칙(1~6)은 `shared/ds` 안에만 적용한다.

**화면 코드(`features/**`, `app/**`)에서 금지**

| # | 금지 | 강제 방법 | R-14 3.2 대응 |
|---|---|---|---|
| 1 | 소문자 JSX 태그 전부 (`div`, `span`, `p`, `button`, `svg` 등 예외 없음) | `no-restricted-syntax`: 이름이 소문자로 시작하는 `JSXOpeningElement` | 9번을 포함하고 더 넓다 |
| 2 | `createElement`, `cloneElement`, JSX 런타임 함수 직접 호출 | `no-restricted-imports`, `no-restricted-properties` | - |
| 3 | `className`, `style` 속성 | `no-restricted-syntax`: `JSXAttribute` | 6, 7번을 포함한다 |
| 4 | 스타일 파일(`.css` 등) 가져오기, CSS-in-JS, 클래스 이름 도우미(`cn()` 등) | `no-restricted-imports`, `local/forbidden-file`(스타일 파일은 `shared/ds`에만 존재) | - |
| 5 | UI 라이브러리 직접 가져오기: `@base-ui/*`, `recharts`, `lightweight-charts`, `react-markdown`, `lucide-react` | `no-restricted-imports` | 10번 |
| 6 | `dangerouslySetInnerHTML` (`shared/ds` 포함 전체 금지) | `no-restricted-syntax` | - |
| 7 | 색 리터럴(`#hex`, `rgb(`, `oklch(`), `\d+px` 문자열, `Intl`·`toLocaleString`·`toFixed`(형식 변환은 `shared/lib/format`만), JSX 텍스트의 하이픈 빼기(`-\d`), 이모지 | `no-restricted-syntax`(Literal, TemplateElement, JSXText), `no-restricted-globals`, `no-restricted-properties` | 8, 12, 13, 14번 |
| 8 | `ref` 화살표 함수 | `no-restricted-syntax` (4.2 React 행) | 11번 |

1번과 3번 때문에 화면 코드에서는 디자인 시스템 컴포넌트 말고는 아무것도 그릴 수 없다. 배치도 레이아웃 컴포넌트로만 한다 ([architecture.md](architecture.md) 5.1).

**`shared/ds`의 공개 컴포넌트가 지킬 것**

| # | 규칙 | 강제 방법 |
|---|---|---|
| 9 | 공개 props에 `className`, `style`, 임의 HTML 속성 전달(`...rest`)이 없다 | 로컬 ESLint 규칙: 공개 props 타입 검사 |
| 10 | 간격, 색, 크기, 글자 모양을 받는 props는 토큰 이름의 유니온 타입이다. 숫자나 자유 문자열을 받지 않는다. 금액 표시 props는 코어 DTO(십진수 문자열 + 통화 + 코어가 정한 방향)를 받는다 | 같은 규칙 + 타입 테스트. 토큰 이름이 사라지면 생성된 `tokens.ts` 유니온이 줄어 쓰던 코드가 컴파일 오류가 난다 |
| 11 | 임의 스타일을 받는 범용 컴포넌트(`Box` 류)를 두지 않는다 | 10번으로 막힘. 리뷰에서도 확인 |
| 12 | 컴포넌트마다 목록 화면 항목, 시각 회귀 기준 화면(라이트·다크), 상태별 테스트가 있다 | `xtask ds-check`: 공개 목록과 대조해 빠지면 실패 |

**`shared/ds` 내부 스타일 규칙** (R-14 3.2를 그대로 쓰고, 화면 코드에 이미 적용된 것은 뺐다)

| R-14 # | 규칙 | 도구 |
|---|---|---|
| 1 | 토큰 밖 Tailwind 클래스 금지 | `better-tailwindcss/no-unknown-classes` (`entryPoint` = 생성된 테마 CSS) |
| 2 | 임의 값 `[...]`(임의 변형 포함), 불투명도 수식어 `/N`, `-px` 금지 | `better-tailwindcss/no-restricted-classes` |
| 3 | 숫자를 그대로 받는 유틸리티(`opacity-N`, `z-N`, `duration-N`, `border-N`, `rounded-full`, `grid-cols-N` 등) 금지. 보호된 허용 목록에 있는 것만 예외 | 같은 규칙, 허용 목록 정규식 |
| 4 | `dark:` 변형 금지 (테마는 토큰이 바꾼다) | 같은 규칙 |
| 5 | 가격 색(`up`, `down`, `-soft`)은 가격 컴포넌트 파일에서만 | 같은 규칙, 파일 허용 목록 |
| 6 | 동적 클래스 금지. `className`은 문자열 리터럴이나 `cn()`만 | `better-tailwindcss/no-concatenated-classes` + `no-restricted-syntax` |
| 7 | 인라인 `style` 금지. 필요하면 보호된 파일 허용 목록으로만 (처음에는 비어 있다) | `no-restricted-syntax` |
| 8 | 색 리터럴과 `\d+px` 문자열 금지 | `no-restricted-syntax` |
| 15 | CSS: `color-no-hex`, `declaration-strict-value`, `var()` 이름이 생성 토큰 목록에 있어야 함 | stylelint + xtask |
| 16 | 생성물(`tokens.css`, `tokens.ts`)과 시각 회귀 기준 이미지는 손으로 고치지 않음 | 보호 경로 + 재생성 차이 검사 |
| 17 | 토큰 대비 기준 | `xtask tokens-contrast` (필수 조합 표는 보호 파일) |

R-14는 `shared/ds`에서 3, 5, 7번을 풀어 주자고 했다. 이 문서는 더 엄격하게, 셋 다 보호된 허용 목록으로만 푼다.

**스크롤과 넘침** ([architecture.md](architecture.md) 5.1 "스크롤과 넘침")

글로 쓴 규칙만으로는 넘침을 막지 못한다. 쓸 수 있는 CSS를 줄이는 정적 검사와, 실제로 그려서 재는 넘침 검사를 함께 쓴다.

정적 검사:

| # | 규칙 | 강제 방법 |
|---|---|---|
| 18 | `overflow`, `overflow-x`, `overflow-y`(클래스 `overflow-*`, `truncate`, `line-clamp-*`, `text-ellipsis` 포함)는 보호된 허용 목록 파일에만 있다. 처음 목록: `ScrollArea`, `Text`(말줄임), 모서리 자르기가 필요한 파일 | `better-tailwindcss/no-restricted-classes`의 파일별 설정, stylelint `declaration-property-value-disallowed-list` |
| 19 | 스크롤을 만드는 값(`auto`, `scroll`, `overlay`)은 `ScrollArea` 파일 하나에만 있다. `ScrollArea`는 Base UI ScrollArea만 감싼다 | 같은 규칙 |
| 20 | `position`의 `absolute`·`fixed`·`sticky`(클래스 포함)와 `inset-*`, `top-*` 등은 보호된 허용 목록 파일에만 있다 | 같은 규칙 |
| 21 | `height`·`max-height`·`width`·`max-width`·`min-*`에 길이를 주는 클래스는 보호된 허용 목록(컨트롤, 표 행, 차트, 머리줄, `AppShell`, 대화 상자)에만 있다. 그 밖에서는 `auto`, `100%`, `min-content` 계열과 격자 열 수만 쓴다 | 같은 규칙 |
| 22 | `vh`, `vw`, `dvh`, `svh`, `lvh`, `vmin`, `vmax` 단위와 `h-screen`·`w-screen` 클래스는 `AppShell` 파일에만 있다 | stylelint `unit-disallowed-list`, 클래스 규칙 |
| 23 | `white-space: nowrap`(클래스 `whitespace-nowrap`)은 숫자 표시 컴포넌트와 표 셀, 배지, 버튼 파일에만 있다 | 같은 규칙 |
| 24 | 음수 여백(`-m*`), `translate`, `scale`로 내용을 영역 밖으로 옮기는 클래스는 보호된 허용 목록에만 있다 | 같은 규칙 |
| 25 | 화면 코드(`features`, `app`)는 `ScrollArea`를 가져올 수 없다. 스크롤 영역은 `AppShell`, `Dialog`, `Popover`, `Menu`, `Table`, 차트가 안에 갖는다 | `no-restricted-imports` |
| 26 | 레이아웃 컴포넌트(`Stack`, `Inline`, `Split`, `GridItem`)의 자식 래퍼는 `min-width: 0`, `min-height: 0`을 갖는다. `Text`의 기본 줄바꿈은 `keep-all` + `anywhere`다 | 넘침 검사의 기준 예제(긴 낱말 하나를 넣은 `Inline`)가 통과해야 함 |

넘침 검사(`xtask ds-check`의 일부, Vitest 브라우저 모드): 그린 뒤 DOM을 재서 아래를 모두 확인한다. 하나라도 어기면 실패한다.

| 확인 | 판정 |
|---|---|
| 문서 스크롤 없음 | `documentElement`와 `body`의 `scrollWidth`·`scrollHeight`가 `clientWidth`·`clientHeight`보다 크지 않다 |
| 허가 없는 넘침 없음 | 모든 요소에서 `scrollWidth > clientWidth` 또는 `scrollHeight > clientHeight`이면 그 요소는 `ScrollArea`의 뷰포트(`data-scroll-viewport`)이거나 말줄임 요소(`data-truncate`)여야 한다 |
| 잘린 요소 없음 | 모든 요소의 사각형이, `overflow`가 `visible`이 아닌 가장 가까운 조상의 사각형 안에 있다. 조상이 `ScrollArea` 뷰포트이면 스크롤해서 닿을 수 있으므로 통과 |
| 말줄임에는 전체 글자가 있다 | `data-truncate` 요소는 툴팁으로 전체 글자를 낸다 |
| 같은 방향 스크롤 중첩 없음 | 세로 뷰포트의 조상에 세로 뷰포트가 없다(허용 목록의 `ChatInput` 제외) |
| 창 밖으로 나간 요소 없음 | 포털로 그린 겹침 요소(대화 상자, 팝오버, 메뉴, 툴팁, 토스트)의 사각형이 창 안에 있다 |
| 겹친 조작 요소 없음 | 같은 부모 아래의 버튼·입력·링크의 사각형이 서로 겹치지 않는다 |

검사 대상과 조건:

- 대상: 모든 공개 컴포넌트의 모든 이야기, 그리고 화면마다의 네 상태(로딩·빈·오류·정상).
- 창 크기: 화면은 960×640(최소), 1000, 1180, 1440, 1920 폭에서, 채팅 패널은 최소 폭과 최대 폭에서 잰다. 컴포넌트는 자기가 놓일 수 있는 가장 좁은 폭(격자 3열의 `narrow` 구간 폭)에서도 잰다.
- **부하 내용**: 컴포넌트마다 "부하" 이야기가 반드시 있다. 보호된 부하 자료(`shared/ds/testing/stress.ts`)의 값을 넣는다: 띄어쓰기 없는 40자 종목 이름, 긴 영문 낱말, 조 단위 금액과 소수 8자리 수량, 보유 종목 200행, 근거 문단 20개, 출처 30개, 빈 값. 부하 이야기가 없으면 `ds-check`가 실패한다.
- 글자 크기를 키운 조건(기본의 125%)에서도 한 번 더 잰다.
- 화면 작업(U, C, E 묶음)의 화면 테스트도 같은 넘침 검사 함수를 부른다. 이 함수는 `shared/ds/testing`에 있고 보호 파일이다.

넘침 검사는 끌 수 없는 검사다 (2.1). 넘치는 화면을 고치는 방법은 배치를 바꾸는 것뿐이다.

`no-unknown-classes`는 `entryPoint` 설정이 틀리면 조용히 통과할 수 있다. 위반 예제가 실패하는지를 하네스 자체 테스트로 지킨다 (6.8).

**디자인 시스템 공개 목록 잠금**

- `shared/ds/index.ts`(공개 컴포넌트와 변형의 목록), `design/`(Claude Design 산출물), 토큰 생성물, 시각 회귀 기준 화면, 허용 목록들은 보호 파일이다.
- 새 컴포넌트나 새 변형을 추가하려면 사용자 승인이 필요하다. 에이전트가 화면마다 일회용 컴포넌트를 `shared/ds`에 넣어 규칙을 피하는 것을 막기 위해서다.
- 필요한 컴포넌트가 없으면 에이전트는 멈추고 보고한다. 디자인(Claude Design)에 먼저 추가한 뒤 코드로 옮긴다.
- `features` 안에서 디자인 시스템 컴포넌트만으로 조합한 컴포넌트를 만드는 것은 자유다.
- 제3자 컴포넌트는 반드시 DS 래퍼로 감싼다. `--*: initial`이 기본 테마 클래스에 기대는 스타일을 없애기 때문이다.

## 5. Rust와 TypeScript 사이, 생성물

| 생성물 | 생성 방법 | 규칙 |
|---|---|---|
| IPC 타입, 명령 시그니처, `MockCommandHandlers`, `mockCommands`, 명령 목록 파일, capability 명령 허용 항목 | tauri-specta. 생성 진입점은 `src-tauri`의 `gen-bindings` 바이너리다. `xtask`는 작업 공간 crate에 의존하지 않으므로 생성 진입점이 될 수 없다. 보호된 `build.rs`는 명령 목록 파일을 읽기만 한다 | TS에서 손으로 쓰지 않는다. UI 테스트는 `mockCommands`만 써서, 명령 이름·인자·반환 타입이 바뀌면 `tsc`가 테스트 파일에서 실패한다. 보호 목록에 넣지 않는다. 에이전트가 다시 생성하고, 다시 생성해서 차이가 나면 실패하는 것으로만 지킨다. 위험한 권한은 손으로 쓴 보호 파일에만 있고 `capability-check`가 막는다 |
| 명령별 대표 응답 JSON | Rust 테스트가 `insta` 스냅샷으로 내보낸다 | TS 가짜는 이 JSON을 불러 돌려준다. 십진수 문자열 형식, 열거형 태그가 어긋나면 Rust 스냅샷 차이로 드러난다 |
| 토스 API 모델 | `typify`로 `docs/toss-openapi-*.json`의 `components.schemas`만 생성한다. HTTP 호출은 `broker-toss`(읽기)와 `broker-toss-trade`(주문)에 손으로 쓴다 | 생성물은 보호된 생성 경로의 별도 crate `broker-toss-schema`에 둔다. 이 crate만 주석·속성 검사에서 제외한다. 이 crate의 타입은 `broker-toss`, `broker-toss-trade` 밖으로 나가지 않는다(arch-check). 금액 문자열 → `Decimal` 변환은 `broker-toss`가 `Result`로 한다 |
| 디자인 토큰 | `xtask gen-tokens` (`tokens.json` → `tokens.css`, `tokens.ts`) | 4.4 |
| 라우터 생성 파일 | TanStack Router 파일 기반 생성기를 쓰면 | 생성 경로 목록에 등록한다 |

- 생성물은 `just check`와 CI에서 다시 생성해 `git diff --exit-code`가 0이 아니면 실패한다.
- 생성 경로 목록은 보호 파일이다. 파일 안의 "생성됨" 표식은 판정에 쓰지 않는다.
- 보호하는 생성물: `broker-toss-schema`(주석·속성 검사에서 빠지는 crate), 디자인 토큰 생성물(4.4). IPC 생성물은 보호하지 않는다. 명령 하나를 추가할 때마다 보호 파일을 바꾸는 일을 없애기 위해서다.
- 프레임워크 오류(알 수 없는 명령, 잘못된 인자, ACL 거부)는 자유 형식 문자열로 거부되므로, UI는 오류 값을 스키마로 다시 파싱하고 가짜에도 이 경우를 넣는다 (R-15 2.2).

## 6. 테스트와 에이전트 훅

### 6.1 대상별 기준

| 대상 | 기준 |
|---|---|
| `domain` | 파일별 줄 커버리지 95% 이상, 속성 테스트, 변이 테스트 생존은 보호된 허용 목록에 있는 것만 |
| `order-execution` crate와 `application`의 주문 게이트 모듈 | 파일별 줄 커버리지 100% (6.3), 변이 테스트 기준은 `domain`과 같음, 토큰 위조 `trybuild` 테스트, 경로 고정 테스트 (6.5) |
| `domain`의 신호·주문 계산 모듈 (임계값, 여유분, 보정, 신호 판정, 수량·가격, `validate`, 결과 판정) | 파일별 줄 커버리지 100%, 변이 테스트 대상, 속성 테스트. 방향·수량·가격을 코드가 정한다는 요구(ORD-4, ORD-8, ACC-9)가 이 모듈에 있기 때문이다 |
| `application` | 유스케이스마다 가짜 어댑터로 성공·실패 경로 테스트 |
| 어댑터 | 로컬 가짜 HTTP 서버에 녹화 응답을 올린 계약 테스트. 녹화 자료는 토스 OpenAPI 스펙으로 검증해 스펙과 어긋나면 실패. 실제 네트워크 호출 금지 (6.2) |
| `store-sqlite` | SQL 상수 전부를 마이그레이션된 DB에 `prepare`하는 테스트, 스키마 스냅샷 테스트 (R-06) |
| `mcp-server` | 프로세스 안 duplex 계약 테스트, 두 프로토콜 세대의 JSON 줄 스냅샷, `tools/list` 골든 파일 (R-04) |
| `ai-claude-code` | 실행 파일 경로를 주입받아 가짜 CLI로 녹화 stream-json을 재생. 출력 재생과 대본 MCP 클라이언트 두 모드 (R-15 2.4). `PATH`를 바꾸는 방식은 쓰지 않는다 |
| 코어 시나리오 | 하루치 시나리오를 고정 시계와 `testkit` 가짜로 돌리고 이벤트 순서와 DB 상태를 스냅샷으로 고정 |
| UI | 화면마다 로딩·빈 상태·오류·정상 상태 테스트 (Vitest + Testing Library + `mockCommands`) |
| 하네스 자체 | 6.8 |

- 버그를 고칠 때는 그 버그를 재현하는 테스트를 먼저 추가한다.
- tokio `start_paused`는 순수 가짜만 쓰는 테스트에서만 쓴다. 실제 자식 프로세스, 소켓, SQLite 파일 I/O를 기다리면 시계가 자동으로 앞당겨져 타임아웃이 거짓으로 터진다 (R-15 2.3).

### 6.2 네트워크 차단

테스트는 실제 토스 주소와 외부 인터넷에 닿지 않는다. 세 겹으로 막는다 (R-15 2.7, R-09 Q9).

| 겹 | 방법 |
|---|---|
| 1. 구조 | `broker-toss`는 기본 URL을 `TossEndpoint` 생성자 인자로만 받는다. 실제 주소 생성자는 조립 루트에서만 보인다. 테스트와 `testkit`은 `TossEndpoint::local(addr)`만 만들 수 있다. xtask가 조립 루트(`src-tauri/src/main.rs`)와 `toss-smoke` 밖의 `tossinvest.com` 문자열을 실패시킨다 |
| 2. HTTP 클라이언트 | 제품 클라이언트는 `https_only(true)`. 테스트 클라이언트는 루프백만 허용하는 `dns_resolver`를 주입한다. cfg 분기가 아니라 생성자 주입이다 |
| 3. OS | macOS: 루트 `.cargo/config.toml`의 `target.aarch64-apple-darwin.runner`로 모든 테스트 실행 파일을 `sandbox-exec` 네트워크 차단 프로필 안에서 돌린다. 자식 프로세스(가짜 AI CLI 포함)도 물려받는다. 루프백과 Unix 소켓은 열어 둔다. Linux CI: 컨테이너 `--network none`이나 `unshare -n` |

- 이 차단이 실제로 걸려 있는지 하네스 자체 테스트로 늘 확인한다. "외부 연결이 실패해야 통과하는 테스트"와 "루프백 가짜 서버가 동작해야 통과하는 테스트"를 `just check`와 같은 명령(nextest)으로 돌린다. `sandbox-exec`는 폐기 예정 표시가 있어, 사라지면 이 테스트가 먼저 실패한다.
- nextest가 cargo `runner`를 그대로 쓰는지는 리서치에서 직접 확인하지 않았다. 위 자체 테스트가 실패하면 nextest wrapper script(R-09 6.4, 실험 기능)로 바꾼다.
- Linux의 `--network none`·`unshare -n`은 실행해 보지 않았다. CI 구성 작업에서 같은 자체 테스트로 확인한다.
- DNS 조회는 막히지 않는다. 연결만 막힌다. runner가 build script에 적용되는지는 미확인이다. 작업 공간의 `build.rs`는 보호된 `src-tauri/build.rs` 하나뿐이고(3.2), 서드파티 빌드 스크립트는 cargo-deny 허용 목록으로 관리한다.
- TS 쪽은 `fetch`, `XMLHttpRequest`, `WebSocket` 사용 자체를 린트로 금지한다 (4.2).

### 6.3 커버리지 100%와 방어 분기 (open-issues I-13)

100% 기준은 도달할 수 없는 방어 분기와 충돌한다. 리서치 예제에서 `bind`의 `ProposalMismatch` 반환 1줄이 구조상 도달할 수 없어 98.31%에 멈췄다 (R-09 Q7). 해소 순서:

1. **타입으로 분기 자체를 없앤다.** 예: 승인 토큰이 제안을 소유하게 해서 "토큰과 제안 불일치"를 표현할 수 없게 만든다.
2. 그래도 남는 분기는 **보호된 커버리지 예외 목록**에 파일과 함수 단위로 적는다. xtask가 커버리지 결과의 실행되지 않은 줄을 이 목록과 대조해, 목록 밖이면 실패시킨다. 목록 변경은 사람 승인이다.
3. 코드로 커버리지를 빼는 수단은 쓸 수 없다. `#[coverage(off)]`는 unstable이고 `unstable_features = "forbid"`와 xtask의 `cfg` 금지로 막힌다.

- 분기 커버리지(`--branch`)는 nightly가 필요해 쓰지 않는다. 줄 커버리지와 변이 테스트로 대신한다.
- `tests/` 폴더는 기본으로 커버리지에서 빠진다.

### 6.4 변이 테스트

| 항목 | 규칙 |
|---|---|
| 도구 | `cargo-mutants` 27.1.0, `--test-tool nextest`, `--cap-lints=true` 필수 (없으면 엄격 린트 때문에 정상 변이체가 unviable로 분류된다) |
| 대상 | `domain`(신호·주문 계산 모듈 포함), `order-execution`, `application`의 주문 게이트 모듈 |
| 제외 | 진단 출력을 비교하는 `trybuild` 테스트는 nextest 필터로 뺀다 (`--cap-lints`가 기대 출력을 바꾼다) |
| 언제 | 푸시 훅(`just check-full`): 기준 브랜치 대비 변경분(`--in-diff`). CI 밤: 전체 |
| 생존 허용 | 보호된 허용 목록에 있는 것만. 설정 파일(`.cargo/mutants.toml`)도 보호 파일 |
| 코드로 건너뛰기 | `#[mutants::skip]`, `cfg_attr` 형태 모두 xtask가 실패시킨다 |

승인을 생략하는 변이체가 unviable(컴파일 불가)로 나오는 것은 실패가 아니라 타입이 승인 생략을 표현할 수 없다는 증거다.

### 6.5 주문 경로 고정 테스트

WebDriver가 아니라 Rust에서 `tauri::test` 목 런타임으로 만든다. Touch ID 창은 어떤 E2E 도구로도 자동화할 수 없어서 이 계층이 유일한 자동 고정 지점이다 (R-15 2.0).

| 규칙 | 내용 |
|---|---|
| 조립 | 제품과 **같은** `build_app<R: Runtime>(builder, Ports, ctx)`를 부른다. `main.rs`는 실제 어댑터 생성과 `build_app` 호출만 담는다 |
| 호출 | `get_ipc_response`로 WebView가 보내는 것과 같은 IPC 요청을 넣는다. macOS에서 출처는 `tauri://localhost`다 (`http://tauri.localhost`는 macOS에서 로컬로 판정되지 않는다) |
| 목록 | 명령 목록은 tauri-specta 생성물에서, MCP 도구와 스케줄러 작업은 레지스트리에서 가져온다. 손으로 쓴 목록은 새 명령을 빠뜨린다 |
| 가짜 | 가짜 본인 확인은 `application`의 `UserPresence` 포트 가짜(`testkit`의 `ScriptedPresence`), 가짜 증권사는 `trade-fakes`의 `RecordingBroker`다 (open-issues I-14, [architecture.md](architecture.md) 3.2). 둘 다 `cfg`나 `[features]` 없는 보통 crate라 3.2의 `cfg` 금지와 충돌하지 않고, `src-tauri`의 개발 의존으로만 들어오므로 제품 실행 파일에는 없다. 가짜 증권사는 진짜 `ApprovedOrder`를 받기만 한다 |
| 보호 | 경로 고정 테스트 파일과 주문·본인 확인 가짜는 보호 파일이다 (7.1) |

단언 목록은 [architecture.md](architecture.md) 4.6의 표 (a)~(n) 하나뿐이다. 이 문서에 따로 적지 않는다.

- 본인 확인을 무시하게 바꾼 변이를 넣으면 이 테스트가 실패해야 한다. cargo-mutants로 확인한다.
- `mock_context`는 빈 ACL을 쓴다. 실제 capability 파일까지 검증하려면 `tauri::generate_context!()`를 목 런타임과 함께 써야 하는데, 되는지는 미확인이다(open-issues S-13). 안 되면 capability 검증은 `xtask capability-check`와 밤 E2E가 맡는다.
- 실제 Touch ID(`UserPresence` 실구현)는 자동화할 수 없다. 사용자가 O-15에서 확인한다.

### 6.6 시각 회귀

| 규칙 | 내용 |
|---|---|
| 기준 환경 | 태그를 고정한 Playwright 컨테이너(linux/arm64). 로컬은 arm64 컨테이너, CI는 Linux arm64 러너에서 같은 이미지 태그. 같은 컨테이너는 두 번 모두 바이트까지 같았고, 호스트 macOS와는 글꼴부터 달랐다 (R-15 2.5) |
| 기준 갱신 | 컨테이너 안에서만(`just visual-update`). 호스트에서 만든 `-darwin` 기준 파일이 있으면 xtask가 실패시킨다. 갱신은 사용자 서명 커밋으로만 들어간다 |
| 입력 고정 | 저장소에 묶은 웹 글꼴만 사용(시스템 글꼴 금지), 애니메이션·캐럿 끄기, 고정 시계, 고정 난수, 이야기마다 라이트·다크 |
| 실제 WKWebView 스크린샷 | 밤 macOS 작업에서 보관만 하고 비교하지 않는다 |

비교 엔진은 정하지 않았다. R-14는 WKWebView에 가까운 WebKit을, R-15의 재현성 실험은 Chromium을 썼다. 컨테이너 안 WebKit의 재현성은 확인하지 않았다(open-issues S-16). DS-15에서 실측하고 ADR로 정한다. 엔진에 관한 내용은 이 절에만 적는다.

### 6.7 E2E

E2E는 밤 macOS의 연기 테스트 3~5개뿐이다. WebdriverIO `@wdio/tauri-service`와 앱 안 `tauri-plugin-wdio-webdriver`를 쓴다. 이 플러그인은 HTTP로 앱을 조종할 수 있으므로 별도 바이너리 `e2e-app`에서만 등록하고, 가짜 본인 확인(항상 거부)과 가짜 증권사로 조립한다. 제품 바이너리의 의존 그래프에 자동화 플러그인이 있으면 `arch-check`가 실패한다.

### 6.8 하네스 자체 테스트

규칙마다 "위반 예제"를 두고, 검사가 그 예제에서 실패하는지 `just check`에서 확인한다. 하네스 자체 테스트가 없는 규칙은 없는 것으로 본다.

| 대상 | 방법 |
|---|---|
| ESLint 규칙 | `harness/violations/src/**`에 실제 `src`와 같은 폴더 구조로 예제를 두고, `harness/lint-cases.json`에 예제별 "반드시 보고돼야 하는 규칙 ID"를 적는다. 통과해야 하는 예제(`as const`, `satisfies`)는 문제 0개를 기대한다. 알려진 빈틈은 `KNOWN_GAP`으로 표시해, 나중에 막히면 드러나게 한다. 예제에는 주석을 쓸 수 없으므로 기대값은 목록 파일에 둔다 |
| tsc 옵션 | `harness/tsc-violations/*.ts`마다 기대 오류 코드를 정하고 TS 7과 TS 6 두 컴파일러로 확인한다 |
| 유효 설정 | 대표 파일마다 핵심 규칙의 심각도가 `error`이고 끌 수 없는 규칙 목록이 `no-restricted-disable`에 그대로 있는지 `calculateConfigForFile`로 확인한다. 덮어쓰기 블록이 규칙을 몰래 끄는 것을 막는다 |
| 로컬 규칙 | ESLint `RuleTester` 단위 테스트 |
| Rust 린트와 xtask | 금지 항목마다 위반 예제 crate·파일이 실패하는지 |
| 네트워크 차단 | 6.2의 외부 연결 실패 테스트, 토스로 연결을 시도하는 위반 예제 |
| 디자인 시스템 | 4.4의 위반 예제, Tailwind "생성되는 클래스 목록" 스냅샷 |
| 에이전트 하네스 | R-10 "실행 검증 요약"의 실험 12개(보호 경로 쓰기, 셸 우회, 샌드박스 경로 해석, 종료 훅 3회 차단, 리뷰어 원본 오염, `Agent` 인자 거부)를 Claude Code를 올릴 때마다 다시 돈다 |
| 실제 코드 무결성 | `eslint --max-warnings 0 src`와 `tsc`가 깨끗해야 한다. 규칙이 너무 엄격해 정상 코드를 쓸 수 없는 상태도 이것으로 드러난다 |

위반 예제는 `harness/lint-cases.json` 한 파일이 아니라 검사마다 `harness/<검사>/` 아래에 따로 둔다. 대신 하네스 검사 목록(`harness/check-list/`)이 `just check`의 모든 단계와 그 자체 테스트를 묶는다. `justfile` 전체가 보호된 기대값(`harness/check-list/justfile.json`, just 자신의 해석 결과)과 정확히 같아야 하고, 목록의 각 검사는 실제로 실행되는 자체 테스트를 가져야 한다 (ADR 0019).

### 6.9 요구사항 고정 테스트

[design.md](design.md) 7장의 요구사항 중 "0건", "항상", "바뀌지 않음"처럼 테스트로 판정할 수 있는 것은 아래 테스트로 고정한다. 해당 작업은 [tasks.md](tasks.md)에 있다. 주문 경로의 단언은 6.5(architecture.md 4.6)가 맡는다.

| 요구사항 | 고정하는 테스트 |
|---|---|
| SEC-2 | 가짜 CLI가 init에 다른 도구 목록, 연결 안 된 MCP, 플러그인·스킬·슬래시 명령을 보고하면 실제 요청 0건. 실행 종류별 명령 인자(분석: 내장 도구 0개, 수집: WebSearch 하나와 빈 MCP 설정)가 스냅샷과 같음. 사용자 훅·플러그인을 깐 환경의 실제 CLI 확인은 사용자가 기록한다 |
| SEC-3 | MCP 서버를 띄운 동안 열린 TCP 포트 0개. 토큰 없는 접속, 다른 실행의 토큰, 끝난 실행의 토큰은 거부 |
| SEC-4 | `untrusted_text` 정규화: 제어·폭 0·양방향 문자와 링크가 남지 않음. 도구 설명, 오류 메시지, 필드 이름에 외부 텍스트가 없음. `read_article`이 수집 실행이 가져온 주소(출처 `web`)와 본문 읽기가 켜진 언론사 RSS 도메인 밖의 주소를 거부(경로 고정 테스트). `get_estimate_task`가 코드가 만들지 않은 과제 ID를 거부 |
| SEC-9 | `request_web_research`의 입력 스키마가 주제 목록 열거값과 종목 코드만 받음(`tools/list` 골든 파일). 자유 문장 인자는 거부. 보유 수량·금액·계좌번호를 섞은 검색어를 만드는 속성 테스트에서 수집 실행에 넘어간 검색어 0건. 연도·분기 허용 목록은 통과. 사용자 문장 원문은 저장된 사용자 메시지와 바이트까지 같음. 거부와 통과가 모두 감사 로그에 남음 |
| SEC-10 | capability 금지 권한(`xtask capability-check`), CSP 문자열 스냅샷, AI 마크다운의 원시 HTML·이미지가 그려지지 않음. 수집 목록에 없는 주소는 링크가 아니라 글자로 그려짐 |
| SEC-11 | 가짜 비밀 문자열을 넣고 로그, 오류 값, SQLite 파일, MCP 응답, CLI 인자와 환경 변수에서 0건 |
| SEC-12 | 위험 설정 변경 명령은 네이티브 확인 없이 설정을 바꾸지 못함(architecture 4.6 (i)). 주문 밖에서 macOS 본인 확인 호출 0회(P-9) |
| ORD-2 | 결과 모름 시나리오: 10분 안에는 같은 키·같은 내용으로만 재요청, 10분 뒤와 `maintenance` 뒤에는 재전송 0건, 대조가 하나로 정해지지 않으면 같은 종목 새 주문 차단 |
| ORD-3 | 감사 로그의 중간 수정, 삭제, 꼬리 잘라내기, 머리값 되돌리기를 각각 탐지 |
| ORD-4 | `submit_estimate` 입력 스키마에 방향·수량·가격 필드가 없음(`tools/list` 골든 파일). 그런 필드를 붙인 호출은 거부 |
| ORD-6 | 상한을 넘기면 승인 요청 거부(architecture 4.6 (l)) |
| ORD-7 | 여러 제안을 한 번에 승인하는 명령이 생성된 명령 목록에 없음. 한 번의 확인으로 나가는 주문 1건(architecture 4.6 (f), (n)) |
| ORD-8 | 판정 30건 미만인 모델·기간 조합에서는 어떤 설정 값 조합으로도 여유분이 15%p보다 작거나 매수 비중이 보수 운용 값보다 커지지 않음(속성 테스트). 같은 종목에서 기간이 겹치는 추정은 한 건으로 셈. 모델이 바뀌면 새 모델의 건수로 다시 셈 |
| ACC-1 | 원장에 없는 참조를 담은 구조화 출력은 거부. 자유 서술의 숫자가 원장과 다르면 "검증 안 됨" |
| ACC-2 | SQL 검사(3.5), 금액 열의 정규 문자열 왕복, 부동소수점 없음 |
| ACC-5 | 출처 없는 추정은 신호 0건 |
| ACC-9 | 같은 일봉·1분봉 입력에 같은 판정(속성 테스트). 판정이 끝난 기록은 바뀌지 않음. 제출된 추정 수와 기록된 추정 수가 같음 |
| REL-3 | 고정 시계로 여러 날을 건너뛰어도 놓친 작업은 한 번만 실행 |
| REL-4 | 한도 초과 이벤트는 오류가 아니라 대기 상태가 되고 `resetsAt`에 재개 |
| API-6 | SEC 연락처가 없으면 SEC 요청 0건 |
| UX-5 | 모든 점검이 정상이면 상태 띠 0개. 문제가 여럿이면 앞 순서 하나만. 사용자가 할 일이 없는 상태(design 2.6의 6, 7번)는 상태 띠 0개 |
| UX-6 | 화면 코드의 한국어 문자열과 코어가 보내는 화면 문구를 모아 금지어 목록(MCP, 세션, 프로필, 격리, 큐, 보정, 사전 점검, 토큰 등)으로 검사해 0건. 예외는 고급 > 상태 화면 파일뿐이고, 금지어 목록과 예외 목록은 보호 파일이다 |
| UX-4 | 가짜 토스로 첫 실행을 돌려 키 붙여넣기와 허용 IP 확인 두 조작만으로 주 화면에 닿음 |
| MNT-3 | 6.2의 네트워크 차단. 저장소 픽스처에 계좌번호 형태 문자열 0건 |

## 7. 에이전트 하네스

### 7.1 보호 파일

검사의 동작을 바꿀 수 있는 파일은 전부 보호한다. 기준은 **허용 목록**이다. 에이전트가 쓸 수 있는 경로 목록은 `harness/agent-writable.txt` 한 파일에 두고, 편집 훅과 CI가 같은 파일을 읽는다.

에이전트(implementer)가 쓸 수 있는 경로:

- `crates/*/src/**`, `crates/*/tests/**`, `crates/adapters/*/src/**`, `crates/adapters/*/tests/**`, `apps/desktop/src/**`, `apps/desktop/src-tauri/src/**` (배치는 architecture.md 3.1)
- `prompts/**`, `fixtures/**`
- `docs/**` (아래 보호 문서 제외)
- 루트와 각 crate의 `Cargo.toml`, `apps/desktop/package.json`의 의존성 목록. 새 의존성은 허용 목록(보호)에 있어야 하므로 cargo-deny와 TS 허용 목록 검사에서 걸린다
- 잠금 파일 `Cargo.lock`, `pnpm-lock.yaml`. 내용은 의존성 허용 목록 검사(cargo-deny, TS 허용 목록 검사)가 본다
- IPC 생성물: `bindings.ts`, `MockCommandHandlers`, 명령 목록 파일, capability 명령 허용 항목 파일 (5장)

위 경로 안에 있어도 보호하는 것:

- 경로 고정 테스트 파일, 주문·본인 확인 가짜(`crates/trade-fakes/`, `testkit`의 `ScriptedPresence`) (6.5)
- `crates/xtask/`, `crates/adapters/broker-toss-schema/`(생성 경로)
- UX-6 금지어 목록과 예외 목록 (6.9)
- `shared/ds/index.ts`, `shared/ds/tokens.css`, `shared/ds/tokens.ts`, 시각 회귀 기준 이미지 (4.4)
- 생성 경로 중 IPC 생성물을 뺀 것 (5장)
- `docs/harness.md`, `docs/workflow.md`, `docs/open-issues.md`, 이미 있는 `docs/adr/*` 파일. 새 ADR 파일 추가는 허용한다

그 밖은 전부 보호 대상이다. 대표적인 것:

| 분류 | 경로 |
|---|---|
| Rust 설정 | `Cargo.toml`의 의존성 외 절, `apps/desktop/src-tauri/build.rs`, `tauri.conf.json`과 capability 파일 중 손으로 쓰는 부분(생성되는 명령 허용 항목 밖), `.cargo/`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `rust-toolchain.toml`, `.config/nextest.toml`, nextest wrapper·`sandbox-exec` 프로필, `.cargo/mutants.toml`, `crates/xtask/` |
| TS 설정 | `eslint.config.*`, `eslint/`(로컬 규칙), `tsconfig*.json`, `vitest.config.*`, `knip.*`, `.dependency-cruiser.cjs`, stylelint 설정, `package.json`의 스크립트 |
| 하네스 | `justfile`, `harness/`(위반 예제, 기대 목록, 서명자 목록, 에이전트 쓰기 허용 목록, 검증 스크립트), `lefthook.yml`, `lefthook-local.yml`, `.lefthook-local.yml`, `.github/` |
| 에이전트 설정 | `.codex/**`, `harness/codex-hooks/**`, `.claude/settings.json`, `.claude/settings.local.json`, `.claude/agents/`, `.claude/hooks/`, `.claude/skills/`, `.claude/commands/`, `.mcp.json`, `.worktreeinclude`. `.claude/` 전체가 아니라 하위 경로로 적는다. `.claude/worktrees/`까지 막으면 리뷰어 작업 사본의 쓰기가 막힌다 (R-10 Q11) |
| 규칙 문서 | `AGENTS.md`, `CLAUDE.md`, `docs/harness.md`, `docs/workflow.md`, `docs/open-issues.md`(사용자 확정 결정), 이미 있는 `docs/adr/*` |
| 목록 | 의존성 허용 목록, 생성 경로 목록, 변이 생존 허용 목록, 커버리지 예외 목록, ESLint 파일 허용 목록들, Tailwind 숫자 유틸리티 허용 목록, 대비 검사 조합 표, Linux CI 제외 목록(1.2, `harness/ci/linux/exclusions.json`), 현재 시각·환경 변수 허용 파일 목록(3.1), `unsafe` 예외(있으면, 3.1) |
| 디자인 | `design/` |
| 기타 | `.gitignore` (`.claude/worktrees/` 항목은 사람이 처음에 넣어 둔다) |

수정이 필요하면 에이전트는 작업을 멈추고 사람에게 이유를 보고한다 (8장).

### 7.2 보호 경로를 막는 겹

권한 거부 규칙과 `PreToolUse` 훅은 파일 도구와 셸 리다이렉트만 막는다. `python3 -c "open(...)"` 같은 셸 쓰기는 샌드박스만 막는다 (R-10 Q2, 실측). 그래서 네 겹을 함께 쓴다 (open-issues I-11).

| 겹 | 막는 것 | 못 막는 것 |
|---|---|---|
| ① 권한 `deny` 규칙 `Edit(/경로)` | Edit/Write/NotebookEdit, Claude Code가 아는 셸 파일 명령, 리다이렉트 대상. 이 규칙은 샌드박스 `denyWrite`에 자동으로 합쳐진다 | 스크립트가 직접 여는 파일 |
| ② `PreToolUse` 편집 훅 | 역할별 허용 목록(7.3). 경로를 실제 경로로 풀어 작업 사본 밖과 심볼릭 링크를 거른다 | 셸 안의 간접 쓰기 |
| ③ 엄격 샌드박스 | Bash와 그 자식 프로세스의 쓰기. `sandbox.enabled`, `failIfUnavailable: true`(샌드박스를 못 띄우면 실행하지 않음), `allowUnsandboxedCommands: false`(샌드박스 밖 재실행 요청 차단). `denyWrite`에는 Edit으로는 허용하되 셸로는 막을 경로(의존성 목록 파일)와 루트 패턴을 둔다. 잠금 파일은 `cargo`와 `pnpm`이 셸에서 쓰므로 `denyWrite`에 넣지 않는다 | 내장 파일 도구(①②가 맡음), 훅·MCP 프로세스, **목록에 없는 새 경로** |
| ④ CI (사후 탐지) | 푸시된 커밋 중 기준 커밋의 `harness/agent-writable.txt` 밖 경로를 바꾼 것에 사용자 서명이 없으면 실패하고 알린다 (7.6). 푸시를 막지는 않는다 | 커밋 전 작업 사본 상태, 이미 푸시된 변경 |

방어의 중심은 로컬 겹(①②③)이다. ④는 로컬 겹이 뚫린 것을 뒤늦게 알아내는 수단이다.

샌드박스 쓰기 규칙은 **거부가 이긴다.** `denyWrite: ["."]` 뒤에 `allowWrite: ["./src"]`를 두어도 다시 열리지 않는다 (실측). 그래서 로컬은 허용 목록을 거부 목록으로 근사하고, 정확한 허용 목록 검사는 CI가 맡는다. 목록에 없는 새 최상위 설정 파일을 셸로 만드는 것은 로컬에서 새어 나가고 CI가 잡는다.

배치 규칙:

- 경로 규칙은 프로젝트 `.claude/settings.json`에 둔다. `--settings`로 넘긴 상대 경로는 프로젝트 기준으로 풀리지 않았다 (실측).
- 훅 스크립트는 `.claude/hooks/`에 둔다(샌드박스 기본 보호 경로). 표준 라이브러리만 쓰고 `python3 -I`로 실행한다. 빌드 산출물(`target/` 아래)을 훅이 직접 부르지 않는다. 에이전트가 빌드로 바꿔치기할 수 있다.
- 하네스 상태 파일(검사 통과 기록, 차단 횟수, 격리 스냅샷)은 작업 사본과 임시 디렉터리 밖(`~/.local/state/stock-copilot-harness`)에 둔다. 샌드박스 안에서는 홈에 쓸 수 없으므로 위조되지 않는다.
- 샌드박스 안에서 `cargo`, `pnpm`, `tauri` 빌드가 깨지지 않는지, `allowWrite`에 캐시 경로가 필요한지는 실측해야 한다 (open-issues S-03). 깨져서 `excludedCommands`가 늘면 그 명령은 샌드박스 밖에서 돈다는 뜻이므로 ADR로 다룬다.
- 선택 사항: root 소유의 macOS 관리 설정(`/Library/Application Support/ClaudeCode/managed-settings.json`)에 샌드박스 설정을 두면 저장소 설정으로 샌드박스를 느슨하게 할 수 없다. 이 PC의 모든 프로젝트에 적용되므로 사용자가 정한다.

### 7.3 에이전트 훅

판정 코어는 `.claude/hooks/lib/{edit-guard,bash-guard,paths,verdict}.mjs` 하나다. Codex 어댑터는 `harness/codex-hooks/`다. `.codex/hooks.json`과 `.codex/config.toml`은 보호 경로다. 보호 경로의 기준은 `agent-writable.txt`의 허용 목록 밖이다. 잠금 조각·해시도 검사한다.

| 항목 | Claude Code | Codex 0.168.0 |
|---|---|---|
| 편집 전 보호 경로 | PreToolUse, 잠금 off면 경고 | apply_patch의 모든 대상·이동 전후 경로를 같은 코어로 판정 |
| 금지 셸 | PreToolUse Bash | PreToolUse Bash, 같은 코어 |
| 편집 뒤 린트 | PostToolUse Edit/Write | PostToolUse apply_patch의 모든 대상에 `just lint-file` |
| 설정 변경 | ConfigChange 차단 | 전용 이벤트 없음. 편집·셸 판정과 샌드박스만 적용 |
| 종료 검사 | SubagentStop | Stop에서 `just base=<병합 기준> check`. 적용 여부와 브리프 기준은 메인이 확인 |
| 훅 실행 실패 | onFailure block (Stop 제외) | 실행 자체 실패·시간 초과는 강제 차단을 보장하지 못함 |
| 샌드박스 | 정책 준비, H-20 전 비활성 | 프로젝트 권한 프로필. T3가 샌드박스를 덮으면 미적용 |
| 프로젝트 신뢰 | 작업 공간 신뢰 | 프로젝트 신뢰와 훅 정의 해시 신뢰가 각각 필요 |

`just codex-regression`은 대표 10개 도구 시도를 헤드리스로 확인한다. 실제 시도가 없으면 미확정이다. `just check`에는 토큰 없는 어댑터·판정·설정 테스트만 넣는다. 실측과 한계는 ADR 0021에 있다.

### 7.4 역할과 위임

| 역할 | 실행 | 강제 |
|---|---|---|
| 메인 | Claude, 오케스트레이션과 docs | 기존 편집·셸 가드 |
| 구현·수정·조사·비판 | T3의 새 Codex gpt-6.1-sol, 별도 작업 사본 | Claude 위임 가드가 모델·작업 사본·브리프를 확인 |
| 적대적 리뷰 | 새 Claude Opus 5.5 adversarial-reviewer | 기존 리뷰어 편집 거부·격리·오염 감지 |

Codex 내부 서브에이전트와 재개 도구는 거부한다. 등록된 주 작업 트리는 메인, 나머지 진짜 git worktree는 구현 담당이다. 역할은 모델이 전달한 이름을 믿지 않는다. Claude의 옛 implementer·review-critic 호출은 새 진입 가드가 거부한다. 기존 라이브러리는 회귀 자료와 판정 코어로 남긴다.

### 7.5 리뷰어 격리

Claude 적대적 리뷰어는 기존 `isolation: worktree`와 시작·종료 스냅숏을 쓴다. Codex 비판자는 T3의 별도 작업 사본에서 읽기 전용으로 일한다. T3가 준 권한과 훅 적용을 메인이 확인한다. 위임 역할 이름만으로 읽기 전용을 보장하지 않는다. 원본과 대상 작업 사본의 HEAD·상태를 리뷰 전후 비교한다.

### 7.6 사람 승인의 위조 방지

보호 경로 변경의 승인 표시는 사용자 서명이다. 에이전트가 커밋 메시지에 "승인됨"을 써도 아무 효력이 없다.

| 구성 | 내용 |
|---|---|
| 서명 키 | SSH 서명 키를 Secure Enclave에 두고(Secretive) 쓸 때마다 Touch ID를 요구한다. 키를 내보낼 수 없다 |
| 저장소 설정 | `gpg.format=ssh`, `user.signingkey=<Secretive 공개키>`, `commit.gpgsign=false`. 사용자가 보호 파일을 바꿀 때만 `git commit -S`로 서명한다. 모든 커밋에 서명을 걸면 에이전트 커밋마다 Touch ID 창이 떠서 습관적으로 승인할 위험이 생긴다 |
| 검증 | CI가 `git verify-commit`으로 확인한다. 신뢰하는 키는 `gpg.ssh.allowedSignersFile`의 키뿐이다 |
| 신뢰 자료의 출처 | 허용 서명자 목록(`harness/allowed_signers`), 에이전트 쓰기 허용 목록, 검증 스크립트를 **푸시된 커밋이 아니라 기준 커밋(서명 작업이 마지막으로 성공한 `master` 커밋)에서** 읽는다. 같은 푸시에서 서명자 목록을 바꿔 통과하는 길을 막는다. 잠금 시점(`harness/ci/signatures/lock-point.txt`) 이후의 커밋만 검사하고, 잠금 시점이 없는 동안은 "비활성"을 출력하고 통과한다 (ADR 0020) |
| 워크플로 | 보호 경로 검사는 푸시마다 돈다. 푸시된 코드를 실행하지 않고 git 명령만 쓴다. 푸시 뒤에 도는 사후 탐지이고, 실패하면 알린다 |
| 푸시 | 메인 에이전트만 한다. 서브에이전트는 하지 않는다(7.3의 셸 훅이 거부). 메인은 새 작업의 브리프를 쓰기 전에 `main`의 CI가 통과 상태인지 확인하고, 실패면 멈추고 사용자에게 보고한다([workflow.md](workflow.md) 2.1) |
| 원격 강제 수단 | 공개 저장소다. 규칙 세트는 `master` 삭제·강제 푸시 금지만 쓴다(ADR 0020). 상태 검사 필수·PR 필수·서명 필수는 쓰지 않는다. 에이전트에게 주는 토큰에는 저장소 관리 권한을 주지 않는다 |

남는 빈틈:

- 의존성 목록 파일(`Cargo.toml`, `package.json`)에서 "의존성 절만 바뀌었는지" 검사는 기준 커밋의 xtask를 빌드해 돌린다(H-26에서 만들었다. `xtask manifest-changes`).
- 원격에서 푸시 자체를 막는 수단이 없다. 서명 없는 보호 경로 변경이 `main`에 들어갈 수 있고, CI가 실패로 알린 뒤 사람이 되돌린다.
- CI가 생기기 전(H-26 이전)에는 ④겹이 없다. 로컬 겹(H-20)만으로 운용한다.

### 7.7 기존 테스트 삭제·약화 검출

에이전트 편법의 대부분은 테스트 수정이다. ImpossibleBench에서 Claude 계열 편법의 79% 넘게가 테스트 수정이었다 (R-10 Q6).

| 검사 | 방법 | 어디서 |
|---|---|---|
| 기존 테스트 함수의 삭제·변경 | `xtask test-diff`가 기준 커밋 대비 기존 테스트 함수(Rust `#[test]`, TS 테스트 블록)의 삭제와 본문 변경을 뽑는다. 하나라도 있으면 실패한다. 새 테스트 추가는 허용한다 | `SubagentStop`(implementer), CI |
| 테스트 수 감소 | `cargo nextest list`와 `vitest list`의 개수를 기준 커밋과 비교 | 같음 |
| 승인된 변경 | 기존 테스트를 바꾸는 커밋은 사용자 서명 커밋만 CI를 통과한다 (7.6과 같은 검증) | CI |
| 구현을 베낀 테스트 | 변이 테스트로 드러난다(베낀 테스트는 변이를 죽이지 못한다). 테스트 입력 리터럴을 구현에서 다시 쓰는지 찾는 xtask 검사는 효과가 확인되지 않아 두지 않는다 | `just check-full`, 리뷰 |

기존 테스트가 틀렸다고 판단되면 implementer는 고치지 않고 "막힘:"으로 보고한다.

## 8. 하네스 변경 절차

규칙이 실제 작업을 막는다고 판단되면:

1. 에이전트는 우회하지 않고 멈춘다. 보고 첫 줄은 "막힘:"이다.
2. 어떤 규칙이 어떤 코드를 막는지, 대안이 왜 안 되는지 보고한다.
3. 사람이 규칙을 바꾸기로 하면 ADR을 쓰고 보호 파일을 직접 수정해 서명 커밋으로 넣는다.
4. 바꾼 규칙에 위반 예제를 추가하거나 고친다 (6.8).

리뷰에서 같은 종류의 지적이 두 번 나오면 9장 항목을 도구 규칙으로 옮길 수 있는지 검토한다.

## 9. 리뷰에서 확인할 항목

도구로 막지 못한 우회다. 리뷰어는 변경에 해당하는 항목을 모두 확인하고, 해당 없음도 기록한다.

| # | 항목 | 출처 |
|---|---|---|
| RV-1 | `_ = result;`, `let _x = result;`로 버린 `Result` | R-09 Q1 |
| RV-2 | `let … else`, 열거형 `==` 비교, `if let`으로 빠짐 검사를 피한 분기 | R-09 Q1 |
| RV-3 | `u32::pow`, `i32::abs`, `Iterator::sum`처럼 린트가 잡지 않는 넘침 (`disallowed-methods`에 추가될 때까지) | R-09 4장 |
| RV-4 | `order-execution` 안에서 토큰에 `derive(Clone, Default, Deserialize)`, `From` 구현, `pub` 필드, `pub fn new`, `&mut` 접근자를 더한 것 | R-09 Q4 |
| RV-5 | 같은 `Proposal<Validated>`를 두 번 승인·전송할 수 있는 경로 (타입으로 막을 수 없다. 소비됨 기록과 결정적 `clientOrderId`가 있는지) | R-09 Q12 |
| RV-6 | `main.rs`가 `build_app` 호출과 실제 어댑터 생성 외의 일을 하는지 (경로 고정 테스트가 제품 조립을 고정하는 전제) | R-15 4장 |
| RV-7 | 문자열 조각을 합쳐 만든 색·크기 값 (`` `${hash}ff0000` ``) | R-08 2.11 |
| RV-8 | 화살표가 아닌 ref 콜백(함수 참조)을 마운트 효과처럼 쓴 것 | R-08 2.5, R-14 |
| RV-9 | 던지는 내장 함수·라이브러리 호출 (`new URL`, `decodeURIComponent` 등)을 `Result`로 감싸지 않은 것 | R-08 2.11 |
| RV-10 | `shared/platform` 허용 목록 파일과 `shared/lib/money.ts`의 변경. 이 파일들은 규칙이 풀려 있으므로 리뷰 강도를 높인다 | R-08 4장 |
| RV-11 | tokio `start_paused` 테스트에 실제 프로세스·소켓·파일 I/O가 섞인 것 | R-15 2.3 |
| RV-12 | 스케줄러의 `Instant` 기반 장시간 대기 (벽시계 비교 방식이어야 한다) | R-05 |
| RV-13 | 테스트가 기준이 아니라 구현의 출력을 기대값으로 쓰는 것 | R-10 Q6 |
| RV-14 | 브리프의 "손댈 범위" 밖 변경 | R-10 Q6 |
| RV-18 | `broker-toss`의 HTTP 호출과 공개 API가 GET과 토큰 발급뿐인가. `sources`가 토스 자격 증명·토큰을 받는 길이 없는가. 주문 관련 crate의 타입이 공개 `fn` 시그니처로 새지 않는가. 문자열로 만든 주소·경로로 검사를 피한 곳이 없는가 | ADR 0007 |
| RV-17 | 넘침을 숨겨서 통과시킨 것: 내용을 지우거나 줄인 부하 이야기, 뜻 없이 넓힌 허용 목록, 읽을 수 없을 만큼 좁아진 열, 필요 없는 말줄임 | 4.4 스크롤과 넘침 |
| RV-16 | 새로 생긴 검사 끄기 각각: 이유가 사실인가, 규칙을 지키는 다른 방법이 없었는가. 뜻 없는 이름의 열거형(`Flag::A`/`B`)으로 불린을 흉내 낸 것 | 2.1, 3.4 |
| RV-15 | 검증 없는 newtype, 테스트만을 위한 분기, 오류를 다른 값으로 바꿔 삼킨 것 | workflow.md 2.3 |

하네스 쪽 빈틈은 메인 에이전트가 단계마다 확인한다 ([workflow.md](workflow.md) 2.2, 2.3).

| 빈틈 | 메인의 확인 |
|---|---|
| 셸 훅은 스크립트 파일 실행으로 피할 수 있다 | 샌드박스와 CI가 최종 방어. 리뷰 기록에 남은 명령을 본다 |
| 샌드박스 목록에 없는 새 최상위 파일 | 구현 후 `git status`로 새 파일을 보고 보호 대상이면 반려 |
| `SubagentStop` 훅의 타임아웃 | `PostToolUse(Agent)` 상태가 `passed`가 아니면 반려 |
| 리뷰어가 원본을 되돌리지 않음 | 리뷰 단계마다 원본 `git status`와 HEAD 확인 |

## 10. 실측이 남은 것

ID는 [open-issues.md](open-issues.md) 4.3의 것이다. 하네스 쪽은 해당 작업에서 하네스 자체 테스트로 확인한다.

| 확인할 것 | 실측 ID | 확인할 작업 |
|---|---|---|
| 샌드박스 안에서 `cargo`, `pnpm`, `tauri` 빌드와 테스트가 깨지지 않는지, 필요한 `allowWrite` 캐시 경로 | S-03 | H-35 |
| `--restricted` 격리와 샌드박스 안 구독 인증 (앱의 AI 러너 쪽) | S-04 | AI-12 |
| 서명한 Tauri 앱에서 키체인과 본인 확인 창 (키체인 구현 S-12 전) | S-02 | S-17 |
| Tauri CLI가 루트 가상 작업 공간의 `target/`을 다루는지 | S-12 | H-01 |
| `tauri::generate_context!()`와 `mock_builder()`로 실제 capability를 적용한 경로 고정 테스트 | S-13 | O-13 |
| nextest가 cargo `runner`의 `sandbox-exec`를 그대로 쓰는지 | S-14 | H-18 |
| Linux CI의 `--network none`·`unshare -n` 네트워크 차단 | S-15 | H-16 |
| 시각 회귀 비교 엔진 (컨테이너 안 WebKit 스크린샷의 재현성) | S-16 | DS-15 |
| 대화형 세션에서 `--setting-sources project,local`과 구독 로그인 | S-17 | H-15 |
| 실제 앱에서 `vite/client` 타입을 `types`에 넣어야 하는지 | S-18 | H-06 |
| `eslint-plugin-boundaries` 폐기 예정 경고를 `--max-warnings 0`이 잡는지 | S-19 | H-09 |

S-21(비공개 저장소의 규칙 세트)은 닫았다. 결론: 쓸 수 없다고 보고 기대지 않는다(7.6).
