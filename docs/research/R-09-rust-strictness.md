# R-09 Rust 엄격 도구 구성

조사일: 2026-11-11. 실행 환경: macOS 26.5.2 (aarch64), Rust 1.99.1 stable.
모든 실험은 별도 rustup/cargo 홈을 둔 임시 작업 공간에서 돌렸다. 실제 증권 계좌·키를 쓰는 호출은 하지 않았다.

## 1. 요약

1. 현재 안정판은 **rustc 1.99.1 / clippy 0.1.99**(2026-10-13 빌드)다. 이 버전에서 아래 6장의 `[workspace.lints]`·`clippy.toml`·`deny.toml` 초안을 예제 작업 공간(`domain`, `application`, `order-execution`, `broker-toss`, `mcp-server`, `xtask`)에 넣고, fmt → clippy → xtask → cargo-deny → nextest → 커버리지까지 전부 통과하는 것을 확인했다. 위반 예제 70여 개도 돌렸다.
2. 핵심 결론: **`forbid` 수준으로 둔 린트만 `#[allow]`, `#[expect]`, `RUSTFLAGS=-A ...`로 끌 수 없다.** `deny` 수준 린트는 안쪽 `#![allow(x, reason = "...")]`, `#[expect(...)]`, `RUSTFLAGS`, `.cargo/config.toml`로 끌 수 있고, clippy는 이것을 잡지 못한다. `--cap-lints`는 `forbid`까지 무력화한다. 그래서 다음 세 겹이 필요하다. ① 중요한 린트는 하나씩 `forbid`로 둔다. ② `xtask`가 토큰 단위로 속성·주석·매크로를 검사한다. ③ 검사 스크립트가 환경 변수를 비우고 `RUSTFLAGS`를 고정한다.
3. clippy만으로 잡히지 않는 우회가 실제로 있다. `_ = result;`, `let _x = result;`, `matches!`·`let … else`·`==`로 쓴 와일드카드 분기, `pow`·`abs`·`sum`의 오버플로, `writeln!(stdout())`, 로컬 `macro_rules!` 안의 `unwrap()`, `#[cfg(not(clippy))]`, `#[cfg(any())]`, 문서 주석이 그렇다. 앞의 두 개와 분기 우회는 도구로 막지 못해 리뷰 대상으로 남는다. 나머지는 xtask와 `disallowed-*` 설정으로 막힌다.
4. **cargo-deny로는 작업 공간 crate 사이의 의존 방향을 강제할 수 없다.** `allow-workspace = true`이면 작업 공간 crate에 대한 `deny`가 조용히 무시된다. `false`로 바꾸면 허용 목록과 충돌한다. 또 `wrappers`는 직접 부모만 보기 때문에 전이 경로(`mcp-server → broker-toss → order-execution`)를 통과시킨다. 그래서 의존 방향은 `cargo metadata` 기반 `xtask arch-check`가 맡고, cargo-deny는 서드파티 허용 목록·라이선스·취약점·빌드 스크립트를 맡는다.
5. 추천 도구: 금액은 `rust_decimal` 1.43.0, 속성 테스트는 `proptest` 1.12.0, 변이 테스트는 `cargo-mutants` 27.1.0(`--cap-lints=true` 필수), 커버리지는 `cargo-llvm-cov` 0.9.1(`--fail-under-file-lines`), 테스트 실행은 `cargo-nextest` 0.9.151(실험 기능인 wrapper script와 `sandbox-exec`로 외부 네트워크 차단), 토스 타입 생성은 `typify` 0.8.0(스키마만 생성하고 HTTP는 손으로 쓴다).

## 2. 질문별 답

### Q1. 목적별 제한 린트 (rustc 1.99.1 / clippy 0.1.99)

전체 목록은 `clippy-driver rustc -W help`로 뽑았다. restriction 135개, pedantic 145개, nursery 52개다. 아래 표의 "검증"은 예제 작업 공간에서 위반 예제를 넣고 `cargo clippy --all-targets`를 돌린 결과다.

| 목적 | 린트 (설정 수준) | 검증 결과 | 오탐·빈틈 |
|---|---|---|---|
| panic 계열 | `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `panic_in_result_fn`, `unwrap_in_result`, `get_unwrap`, `exit`, `mem_forget`, `fallible_impl_from` (모두 forbid) | 01~05, 33 실패(검출) | `assert!`는 `Result`를 반환하지 않는 함수에서 잡히지 않음(47). 로컬 `macro_rules!` 안의 `unwrap()`이 통과(43). `panic_in_result_fn`은 테스트 예외 설정이 없어 **`Result`를 반환하는 테스트 + `assert!`도 금지**한다. 테스트는 `()`를 반환하고 `expect`를 쓰는 규약이 필요하다 |
| 인덱스 접근 | `indexing_slicing`, `string_slice` (forbid), `disallowed-methods`로 `Vec::remove/insert/drain/split_off`, `slice::split_at/chunks/windows/copy_from_slice`, `str::split_at`, `String::insert/remove/truncate/drain`, `RefCell::borrow*`, `Iterator::step_by` | 06, 07, 32 실패 | 표준 라이브러리에서 범위 밖이면 panic하는 메서드는 린트가 없다. 목록을 직접 유지해야 한다 |
| `as` 변환 | `as_conversions`, `as_underscore`, `as_pointer_underscore` (forbid) + pedantic `cast_*` | 08 실패 | 문서에 "every single use of `as`"를 잡는다고 적혀 있다. `as` 대신 `try_from`을 쓴다 |
| 넘치는 산술 | `arithmetic_side_effects`, `integer_division`, `modulo_arithmetic` (forbid), pedantic `unchecked_time_subtraction` | 09, 31, 58, 60(Decimal `+`), 68(Duration `-`) 실패 | `u32::pow`(49), `i32::abs`(63), `Iterator::sum`(67)은 잡지 않는다. `rust_decimal`의 `+`는 실제로 panic한다(5장 실행 결과). `checked_*`만 쓰게 하려면 Decimal 연산자 금지 규칙이 필요한데, `arithmetic_side_effects`가 Decimal에도 걸리는 것을 확인했다 |
| 부동소수점 | `float_arithmetic`, `float_cmp_const`, `lossy_float_literal`, `default_numeric_fallback` (forbid) | 10 실패 | **타입 선언만 한 `f64` 필드는 통과**(34). `domain`의 `f32`/`f64` 금지는 xtask 식별자 검사와 `Decimal::to_f64` 같은 메서드 금지로 보완한다 |
| 와일드카드 match | `wildcard_enum_match_arm`, `rest_pat_in_fully_bound_structs` (forbid), `unneeded_field_pattern`, pedantic `match_wildcard_for_single_variants` | 11 실패 | **`matches!`(26), `let … else`(70), `kind == Kind::A`(82)는 통과.** `if let`(27, 69)은 `match_like_matches_macro`가 `matches!`를 권해서 결국 같은 우회로 간다. `disallowed-macros`에 `std::matches`를 넣으면 26은 잡힌다(검증함). `let else`와 `==`는 도구로 막을 수 없다 |
| 무시된 Result | rustc `unused_must_use` (forbid), clippy `let_underscore_must_use`, `let_underscore_untyped`, `unused_result_ok`, `map_err_ignore` (forbid), `disallowed-methods`로 `mem::drop`, `Result::unwrap_or*`, `Option::unwrap_or_default` | 12, 14, 15, 44, 50, 71 실패 | **`_ = f();`(13)와 `let _ignored = f();`(16)는 통과.** 이것을 잡는 린트는 확인하지 못했다 |
| allow 속성 | `allow_attributes`, `allow_attributes_without_reason` (forbid) | 17, 18, 21, 64 실패 | clippy 문서에 `allow_attributes`는 바깥 속성만 본다고 적혀 있다. 실제로 **안쪽 `#![allow(deny 수준 린트, reason = "...")]`가 통과**했다(80). `#[expect(deny 수준 린트, reason)]`도 통과했다(20). forbid 린트에 대한 allow/expect는 rustc E0453으로 막힌다(17~19) |
| 출력 매크로 | `print_stdout`, `print_stderr`, `dbg_macro` (forbid), `use_debug`, `disallowed-macros`로 `print`·`eprint` | 23, 24 실패 | `writeln!(std::io::stdout(), …)`는 통과(25). `std::io::stdout/stderr`를 `disallowed-methods`에 넣으면 막히지만 `xtask`처럼 출력이 필요한 crate가 생긴다(4장) |
| 그 밖 | `unsafe_code` forbid (39 실패), `disallowed-types`로 `HashMap/HashSet/RefCell/Rc/Mutex` (57 실패), `tests_outside_test_module`(통합 테스트도 `#[cfg(test)] mod`로 감싸야 한다), `ignore_without_reason`(pedantic) | — | `#[ignore = "이유"]`는 통과(41b). xtask로 막는다 |

실제로 부딪힌 린트 사이의 충돌과 마찰:

- `min_ident_chars`의 임계값은 "이 길이 이하를 금지"한다는 뜻이다. 2로 두면 `id`도 금지된다. 1로 둬야 한 글자만 금지된다.
- `renamed_function_params`는 `Display::fmt`의 인자 이름을 `f`로 쓰라고 요구한다. 그런데 `min_ident_chars`는 `f`를 금지한다. `allowed-idents-below-min-chars = ["f"]`로 풀었다.
- pedantic의 `needless_pass_by_value`는 소비형 토큰 `fn bind(approval: UserApproval, …)`을 "값으로 받지만 소비하지 않는다"고 오류로 낸다. 필드가 `Copy`라서 구조 분해를 해도 같은 오류가 난다. **`impl UserApproval { fn bind(self, …) }`처럼 메서드 수신자로 바꾸면 해결된다.** 4장의 토큰은 모두 `self`를 소비하는 메서드로 설계해야 한다.
- `redundant_pub_crate`(nursery)는 `unreachable_pub`(rustc)와 서로 반대를 요구한다. 그래서 `redundant_pub_crate`를 끈다.
- 그룹 전체를 `forbid`로 두면 뒤에 오는 개별 `allow`(예: `missing_errors_doc`)가 무시되고 오류가 남는다. `forbidden_lint_groups`라는 미래 호환 경고도 나온다. 그래서 그룹은 `deny`, 핵심 개별 린트는 `forbid`로 둔다.

### Q2. 작업 공간 전체 적용과 crate가 끌 수 없게 하기

확인한 사실:

| 시도 | 결과 |
|---|---|
| crate `Cargo.toml`에 `[lints] workspace = true`와 개별 린트를 함께 둠 | cargo 오류: "cannot override `workspace.lints` in `lints`" |
| `workspace = false` | cargo 오류: "`workspace` cannot be false" |
| `[lints]` 절을 지움 | **조용히 통과한다. 모든 린트가 꺼진다.** xtask `lints-not-inherited`가 잡는다 |
| forbid 린트에 `#[allow]`, `#![allow]`, `#[expect]` | E0453 오류 |
| deny 린트에 안쪽 `#![allow(…, reason)]`, `#[expect(…, reason)]` | 통과한다. xtask `banned-attribute`가 잡는다 |
| `RUSTFLAGS='-A clippy::<deny 린트>'`, `cargo clippy -- -A …`, `.cargo/config.toml`의 `build.rustflags` | 통과한다(우회 성공) |
| `RUSTFLAGS='-A clippy::<forbid 린트>'` | 여전히 실패한다(우회 실패) |
| `RUSTFLAGS='--cap-lints=allow'` | **forbid까지 모두 통과한다** |
| crate 폴더에 빈 `clippy.toml` | 루트 설정을 **대체**한다. `disallowed-methods`가 꺼진다. xtask `nested-config`가 잡는다 |
| `CLIPPY_CONF_DIR=<빈 폴더>` | 루트 설정이 무시된다 |

권장 구조:

1. 루트 `[workspace.lints]` 하나만 둔다(6.1). 모든 crate는 정확히 `[lints] workspace = true`만 갖는다. xtask가 toml을 파싱해 확인한다.
2. 핵심 린트는 개별 `forbid`로 둔다.
3. `just check`의 첫 단계에서 `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, `RUSTDOCFLAGS`, `CLIPPY_CONF_DIR`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_TARGET_DIR`를 지우고 `RUSTFLAGS="-D warnings"`로 고정한다. cargo 문서에 따르면 `RUSTFLAGS`를 설정하면 `build.rustflags`와 `target.*.rustflags`는 무시된다. 상위 디렉터리와 `$HOME/.cargo/config.toml`의 rustflags도 같이 무력화된다. 6.4의 `check.sh`로 검증했다.
4. xtask가 `crates/` 아래의 `clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `rust-toolchain*`, `.cargo/`, `build.rs`를 금지한다. 이 파일들은 루트에서 보호 파일로 둔다.
5. 에이전트 훅(R-10)이 위 환경 변수, `--cap-lints`, `cargo clippy -- -A`를 셸 명령에서 거부한다. 최종 관문은 깨끗한 환경의 CI다.

### Q3. crate 의존 방향과 특정 타입 참조 금지

| 수단 | 직접 의존 | 전이 의존 | 작업 공간 crate | 검증 |
|---|---|---|---|---|
| cargo-deny `bans.deny` + `wrappers` | 잡음(`allow-workspace=false`일 때만) | **못 잡음.** `mcp-server → broker-toss → order-execution`가 통과 | `allow-workspace = true`이면 무시된다("unused-wrapper" 경고만). `false`이면 모든 작업 공간 crate를 `allow`에 넣어야 하는데, 그러면 "crate was specified in both allow and deny" 설정 오류가 난다 | 실행함 |
| `xtask arch-check` (`cargo_metadata` 0.23.1) | 허용 간선 표로 검사 | 허용 표가 DAG이면 전이도 표 안에서만 생긴다 | 정확함 | 실행함. `mcp-server -> broker-toss`를 `[arch-edge]`로 잡음 |
| 가시성(`pub(crate)`, 비공개 필드) | — | — | crate 안쪽 생성자 봉인 | trybuild로 확인(Q4) |
| clippy `disallowed-types` | 작업 공간 전체에만 적용된다. crate별로 하려면 crate 폴더 `clippy.toml`이 필요한데, 이것은 루트 설정을 대체한다 | — | — | 실행함 |

추천:

- 작업 공간 그래프는 `xtask arch-check`가 단독으로 책임진다. 일반·빌드 의존 간선을 허용 표와 비교하고, 표에 없는 crate가 나타나면 실패시킨다. 개발 의존은 별도 표로 검사한다.
- "`src-tauri` 안에서는 승인 명령 모듈 하나만 `order-execution`을 참조" 같은 모듈 단위 규칙은 이름 검색으로 검사하면 약하다. `use … as`나 재수출로 피해 갈 수 있다. **그 모듈을 별도 crate(`approval-command`)로 꺼내 그래프 검사로 바꾸는 것**을 추천한다.
- 특정 타입 금지(예: `domain`의 `f64`)는 xtask가 해당 crate 소스에서 원시 타입 식별자를 찾게 한다. `domain`은 십진수·시간 crate 말고는 의존하지 않으므로, 다른 crate가 만든 별칭으로 피해 갈 길이 없다. `to_f64()`처럼 이름 없이 `f64`가 생기는 경로는 `disallowed-methods`로 막는다.

### Q4. 타입 상태와 봉인된 생성자로 승인 없는 전송을 막는 구조

검증한 구조(`order-execution`):

```rust
pub struct UserApproval { proposal: ProposalId }
pub struct ApprovedOrder { proposal: Proposal<Validated> }

pub trait BrokerTrade {
    fn submit(&self, order: ApprovedOrder) -> Result<ProposalId, SubmitError>;
}

fn approve(presence: &dyn UserPresence, proposal: &Proposal<Validated>) -> Result<UserApproval, SubmitError> { … }

impl UserApproval {
    fn bind(self, proposal: Proposal<Validated>) -> Result<ApprovedOrder, SubmitError> { … }
}

pub fn approve_and_submit(presence: &dyn UserPresence, broker: &dyn BrokerTrade,
                          proposal: Proposal<Validated>) -> Result<ProposalId, SubmitError> {
    let approval = approve(presence, &proposal)?;
    let order = approval.bind(proposal)?;
    broker.submit(order)
}
```

- 비공개 필드 하나로 crate 밖에서의 생성은 막힌다. 별도의 `Sealed` 표식 필드는 불필요했다. 오히려 `dead_code`에 걸린다.
- `trybuild` 1.0.122 compile-fail 테스트 5개로 확인했다.
  - 구조체 리터럴로 위조: E0451
  - `Clone` 없음: E0308
  - `Default` 없음: E0277
  - 비공개 `bind` 호출: E0624
  - `Proposal<Validated>`를 리터럴로 위조: E0451
- 변이 테스트에서 "`approve`를 `Ok(Default::default())`로 바꾸기" 같은 승인 생략 변이체는 전부 **unviable(컴파일 불가)**로 나왔다. 타입이 승인 생략을 표현할 수 없다는 증거다.

우회 경로와 막는 방법:

| 우회 | 막는 것 | 확인 |
|---|---|---|
| `unsafe { transmute / mem::zeroed / MaybeUninit }`로 위조 | `unsafe_code = "forbid"`(39) + 모든 crate의 린트 상속 검사 | 실행 |
| `[lints]`가 없는 새 crate에서 `unsafe` | xtask `lints-not-inherited` | 실행 |
| `derive(Clone, Default, Deserialize)`나 `From` 구현 추가 | crate 내부 변경이다. `order-execution` 리뷰, 커버리지 100%, 변이 테스트. trybuild 사례가 다시 컴파일되면 테스트가 실패한다 | 실행 |
| `pub` 필드나 `pub fn new` 추가 | `partial_pub_fields`(일부만 막음) + trybuild 회귀 | 일부 |
| `&mut ApprovedOrder`를 얻어 `mem::swap/replace`로 다른 제안과 바꿔치기 | `&mut` 접근자를 내지 않는다. `disallowed-methods`에 `mem::swap/replace/take` | 설정함 |
| `Proposal<Validated>`를 복제해 같은 제안을 두 번 승인·전송 | **타입으로 막을 수 없다.** Q12 참고 | 분석 |
| proc-macro나 `build.rs`가 코드를 생성 | xtask가 작업 공간 안의 `build.rs`와 proc-macro crate를 금지한다. cargo-deny `[bans.build]`로 서드파티 빌드 스크립트 허용 목록 | 실행 |
| `include!`나 `#[path]`로 검사 밖의 파일을 끌어옴 | `disallowed-macros`(`include*`) + xtask | 실행 |
| feature나 `cfg(test)`로 테스트용 생성자 노출 | xtask가 `cfg(test)` 말고는 모든 `cfg`와 `cfg_attr`를 금지. `features` 표 금지 | 실행 |

### Q5. 십진수 crate 비교

| 항목 | rust_decimal 1.43.0 | bigdecimal 0.4.11 | fastnum 0.7.5 |
|---|---|---|---|
| 표현 | 96비트 정수 + 0~28 배율, 약 28자리 고정 정밀도 | 임의 정밀도(힙 할당) | 고정 크기 십진 부동소수(D128 등) |
| 1/3 결과 (실행) | `0.3333333333333333333333333333` (28자리) | 문자열 102자 | `0.333…`(39자리) |
| 넘침 | 연산자 `+`는 **panic("Addition overflowed")**. `checked_add`는 `None` (실행) | 넘치지 않음(메모리) | 미확인 |
| 반올림 | `RoundingStrategy` 7종(MidpointNearestEven, MidpointAwayFromZero, ToZero 등) | 미확인(이번 조사 범위 밖) | 미확인 |
| 직렬화 | 기본은 문자열. `serde-str`, `serde-float`, `serde-arbitrary-precision`, `serde-with-*` 기능 | serde 기능 있음(미확인 세부) | 미확인 |
| 성능 (곱셈+덧셈 200만 회, release, 1회 측정) | 13.1 ns/연산 | 20.3 ns/연산 | 8.5 ns/연산 |
| 내려받기 수 | 1.74억 | 1.44억 | 1,100만 |

추천: **rust_decimal**. 원화(정수)·달러(소수 넷째 자리)·소수점 주식 수량을 다루기에 28자리는 넉넉하다. `Copy` 타입이고 할당이 없다. 가장 많이 쓰인다. 단, 연산자가 panic하므로 `domain` 안에서는 `checked_*`만 쓰게 한다(`arithmetic_side_effects`가 Decimal 연산자를 잡는 것을 확인했다). 부동소수에서 만드는 `from_f64_retain`은 `disallowed-methods`로 막는다.

### Q6. 속성 테스트와 변이 테스트

- **proptest 1.12.0**(2026-10-10 배포): `proptest!` 매크로 안에서도 위 린트가 모두 통과했다. 예제 2개를 각 256회 기본 실행했고 0.02초가 걸렸다.
- **cargo-mutants 27.1.0**: 예제(`domain` + `order-execution`, 변이체 17개)에서 약 20초가 걸렸다(caught 7, unviable 10, missed 0).
  - **엄격한 린트 아래에서는 `--cap-lints=true`가 필요하다.** 이것이 없으면 `exceeds -> true` 같은 정상 변이체가 "unused variable"로 컴파일에 실패해 unviable로 분류된다. 첫 실행에서 caught 5 / unviable 12였다가, 이 옵션으로 caught 7 / unviable 10이 됐다.
  - `--cap-lints`를 쓰면 trybuild 테스트의 기대 출력이 달라져 기준 실행이 실패했다. 변이 테스트에서는 nextest 필터 `-E 'not test(sealed_constructors)'`로 trybuild를 뺀다.
  - CI 비용 관리: 문서가 PR 기준 diff에 `--in-diff`를 쓰는 방법을 권한다. 그것이 전체 실행을 대체하지는 않는다는 점도 명시한다. `just check-full`에서 전체를, PR에서 `--in-diff`를 돌린다.
  - 건너뛰기 수단: `#[mutants::skip]`, `#[cfg_attr(test, mutants::skip)]`(문서에 "cfg_attr 조건을 평가하지 않는다"고 적혀 있다), 설정 파일의 경로·정규식 필터. 설정 파일 `.cargo/mutants.toml`은 보호 파일로 둔다.

### Q7. 커버리지 측정과 기준 강제

- **cargo-llvm-cov 0.9.1**(안정 툴체인)과 `llvm-tools-preview`로 동작했다.
  - `--fail-under-lines`는 실행 전체 합계를 기준으로 한다. crate별 기준은 `-p <crate>`로 따로 돌리거나 `--fail-under-file-lines`(파일별)를 쓴다. 둘 다 실행해 기준 미달이면 종료 코드 1이 나오는 것을 확인했다.
  - 분기 커버리지 `--branch`는 "unstable"이다(nightly 필요).
- 실측: `domain` 줄 커버리지 100%, `order-execution` 98.31%. 남은 1줄은 `bind`의 `ProposalMismatch` 반환인데, `approve_and_submit` 구조상 도달할 수 없다. **100% 기준은 방어 코드를 둘 수 없게 만든다.** 둘 중 하나를 골라야 한다. ① 불일치를 표현할 수 없게 타입을 바꾼다(승인 토큰이 제안을 소유). ② 기준을 "95% + 변이 생존 0"으로 바꾼다.
- 빈틈:
  - cargo-llvm-cov는 `cfg(coverage)`를 자동으로 켠다. 문서는 `#[coverage(off)]`(unstable)를 안내한다. 이것은 `unstable_features = "forbid"`와 xtask의 cfg 금지로 막힌다.
  - `tests/` 폴더는 기본으로 커버리지에서 빠진다.
  - `src/` 안의 테스트 모듈은 커버리지에 포함된다. 그래서 필터로 건너뛴 테스트가 커버리지를 깎았다(실측 92%). trybuild 같은 테스트는 `tests/`로 옮긴다.

### Q8. 주석 금지

- rustc, clippy, rustfmt 모두 "주석 금지" 린트가 없다. rustc `missing_docs`와 clippy `missing_docs_in_private_items`는 반대로 문서를 요구한다. 그래서 harness.md의 `xtask lint-comments`가 맞다.
- 검증한 구현: `ra-ap-rustc_lexer` 0.184.0의 `tokenize`로 `LineComment`/`BlockComment` 토큰을 찾는다. `///`, `//!`, `/** */` 같은 문서 주석도 이 토큰에 포함된다. 문자열 안의 `//`는 주석으로 잡히지 않는다.
- 문서 속성 `#[doc = "…"]`는 `proc-macro2`로 파싱한 토큰 스트림에서 `#[doc]` 속성으로 잡는다. proc-macro2는 문서 주석을 `#[doc]`로 바꿔 주므로 두 경로가 모두 걸린다(42, 74, 75).
- 생성 파일 예외는 경로 목록으로만 둔다.
- 주의: `ra-ap-rustc_lexer`는 rust-analyzer가 매주 자동 배포하는 crate다. 메이저 버전이 0이고 `TokenKind`가 바뀐다. 실제로 이번에 `ForcedKeywordIdent` 변형이 추가된 것을 보았다. 정확한 버전으로 고정한다.

### Q9. 테스트에서 네트워크 막기와 녹화 응답 계약 테스트

- 운영체제 수준 차단(검증): macOS `sandbox-exec`에 `(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))` 프로필을 준다. 이것을 nextest wrapper script로 모든 테스트 바이너리에 씌웠다.
  - 결과: 외부 `example.com:443` 연결 실패, 루프백 연결 성공.
  - wrapper가 없을 때와 `cargo test`로 돌렸을 때는 외부 연결이 **성공**했다(테스트 실패로 확인).
  - 따라서 `just check`는 반드시 nextest로 돌려야 한다.
  - 한계: `sandbox-exec`는 man 페이지에 "DEPRECATED"로 표시되어 있다. nextest의 wrapper script는 0.9.98에서 들어온 실험 기능이다(`experimental = ["wrapper-scripts"]`).
- 코드 수준: 어댑터가 기본 URL과 HTTP 클라이언트를 주입받게 하고, 테스트는 `127.0.0.1` 목 서버를 쓴다. 후보 버전: wiremock 0.6.5, httpmock 0.9.0, mockito 1.7.2. 이번에는 실행하지 않았다.
- 녹화 응답 계약 테스트 도구(VCR류)의 현재 상태: **미확인**. 이번 조사에서 Rust용 녹화·재생 crate는 검증하지 못했다. 대안으로 `fixtures/`의 비식별 JSON을 생성 타입(Q11)으로 역직렬화하는 테스트를 추천한다. 스펙 변경 시 차이가 드러나고 별도 도구가 필요 없다.

### Q10. 의존성 허용 목록: cargo-deny와 cargo-vet

| 항목 | cargo-deny 0.20.2 | cargo-vet 0.10.3 |
|---|---|---|
| 허용 목록 | `bans.allow`가 비어 있지 않으면 목록 밖의 crate는 전이 의존까지 실패한다(검증: `anyhow` 추가 시 `not-allowed` + `build-script-not-allowed`) | "감사됨" 기록이 없으면 실패. 처음에는 전부 `exemptions`로 시작한다(예제에서 96건) |
| 버전 고정 | crate 이름 단위 | crate+버전 단위(갱신하면 다시 감사) |
| 라이선스·취약점·출처 | 있음(RustSec, 라이선스, 레지스트리·git 출처, 빌드 스크립트) | 없음 |
| 작업 공간 그래프 규칙 | 불충분(Q3) | 없음 |
| 운영 비용 | 낮음. 새 의존성이 들어오면 전이 crate 이름을 추가한다 | 높음. 버전마다 감사 또는 가져온 감사가 필요하다 |

추천: **cargo-deny만 쓴다.** 허용 목록은 전이 crate까지 명시하고 보호 파일로 둔다. 개인 앱이므로 cargo-vet의 버전별 감사는 비용 대비 효과가 낮다. 공급망 공격을 막는 일을 더 엄격하게 하려면 `Cargo.lock` 변경을 사람 승인으로 묶는 편이 싸다.

### Q11. OpenAPI 3.1 스펙에서 Rust 타입 생성 (`docs/toss-openapi-1.2.28.json` 실측)

스펙 특징: `openapi: 3.1.0`, 경로 40개, 스키마 109개. `type: ["string","null"]` 64곳, `oneOf: [{$ref}, {type: null}]` 20곳, 판별자 없는 `oneOf` 1곳(`OrderCreateRequest`), `format: decimal` 122곳, OAuth2 `tokenUrl`이 상대 경로다.

| 도구 | 3.1 입력 | 결과 | 생성물 성질 |
|---|---|---|---|
| progenitor 0.16.0 | 실패: "invalid type: sequence, expected a string"(type 배열) | 3.0으로 내린 사본도 패닉: `assertion failed: response_types.len() <= 1`(엔드포인트별 오류 응답 스키마가 여럿) | — |
| oas3-gen 0.29.0 | 실패: 상대 `tokenUrl`에서 파싱 오류 | `tokenUrl`을 절대 경로로 바꾸면 types 9,927줄, client 1,922줄 생성. **컴파일 오류 1건**(쉼표 구분 enum 배열 쿼리 `transactionTypes`) | `#![allow(...)]`, `//!` 문서, `expect("invalid regex")`, 요청 타입에 `Default` 파생. `decimal`은 `String` |
| typify 0.8.0 (`components.schemas`만 JSON Schema로 추출) | 성공(0.14초, 19,317줄) | **컴파일 성공.** `null` 세션 역직렬화 테스트 통과 | `oneOf` + null → `Option<T>`. 패턴 제약 문자열은 전용 newtype(`HoldingsItemAveragePurchasePrice`). `OrderCreateRequest` → 판별 없는 enum. `#![allow(clippy::…)]`와 `#[doc]` 포함 |
| openapi-generator 7.27.0 (`rust`, Docker) | 성공(경고 425건, 대부분 이름 바꿈) | **컴파일 성공** | null 허용 필드가 `Option<Option<Box<T>>>`. `Default` 구현. `decimal`은 `String`. Java 실행 환경이 필요하다 |

추천: **typify 0.8.0으로 응답·요청 모델만 만들고, HTTP 호출 40개는 `broker-toss`에 손으로 쓴다.**

- 이유: 3.1 원본을 전처리 없이 받는다. 생성물이 컴파일된다. 의존성이 적다(serde, serde_json, chrono, regress). 패턴 검증 newtype이 "경계에서 파싱" 원칙과 맞는다.
- 공통 조건: 어느 도구든 생성물에 `#![allow]`와 문서 주석이 들어간다. 그래서 생성 파일은 **보호된 생성 경로의 별도 crate**(예: `broker-toss-schema`)에 둔다. xtask가 이 경로만 주석·속성 검사에서 제외하고, 다른 crate는 이 crate의 타입을 `broker-toss` 밖으로 내보내지 못하게 한다(arch-check).
- 금액 문자열(`String`)을 `Decimal`로 바꾸는 일은 `broker-toss`의 변환 계층에서 한다. 이 변환은 실패를 `Result`로 반환한다.
- 이 추천이 틀리는 조건: 토스가 WebSocket(AsyncAPI) 타입까지 같은 방식으로 생성해야 하거나, 스펙에 `discriminator`가 많아져 typify의 판별 없는 enum이 잘못된 변형을 고르는 사례가 생길 때.

### Q12. 타입 상태 값을 DB에 저장·복원할 때의 빈틈과 "승인과 전송을 한 호출로"의 약점

빈틈:

1. **복원 생성자가 곧 검증 우회다.** `Proposal<Validated>`를 DB에서 되살리려면 검증을 건너뛰는 생성자가 필요하다. 이것이 생기면 DB를 쓸 수 있는 모든 코드(마이그레이션, 버그, 다른 어댑터)가 "검증됨"을 만들 수 있다.
   - 대책: DB에서는 `Proposal<Draft>`와 검증 보고서만 복원한다. `Validated`는 항상 `validate()`를 다시 거쳐서만 만든다.
   - 저장된 "검증됨" 표시는 UI 힌트로만 쓴다.
   - 꼭 필요하면 키체인 키로 만든 HMAC을 함께 저장해 변조를 감지한다(R-06, R-16과 연결).
2. **복제로 인한 중복 전송.** `Proposal<Validated>: Clone`이면 같은 제안을 두 번 `approve_and_submit`할 수 있다. 토큰이 제안 ID에 묶여 있어도 타입은 "한 번만"을 기억하지 못한다.
   - 대책: ① `Validated`에서 `Clone`을 뺀다. ② 전송 직전에 같은 트랜잭션으로 `proposal_id`의 "소비됨" 행을 고유 제약으로 넣는다. ③ 토스 `clientOrderId`(스펙 설명: 최대 36자, "멱등성 키는 10분간 유효")를 제안 ID에서 결정적으로 만든다.
3. **전송 중 장애.** 응답 전에 앱이 죽거나 시간 초과가 나면 "보냈는지 모름" 상태가 된다. 한 호출 구조는 이것을 표현하지 못한다.
   - 대책: 전송 전에 "전송 중(clientOrderId)" 기록을 먼저 쓴다(write-ahead). 재시작 시 주문 조회로 맞춘다.
4. **승인 후 지연.** Touch ID 창이 떠 있는 동안 시세가 바뀐다. 아키텍처 4.2의 순서대로 본인 확인 **뒤에** 최신 시세·잔고로 다시 검증해야 한다(예제는 그 순서를 따른다).
5. **할 수 없게 되는 기능.** "지금 승인하고 장 시작에 전송", "여러 건 한 번에 승인"은 승인을 저장해야 하므로 이 구조로는 불가능하다. 필요해지면 토스 쪽 예약·조건 주문으로 보내거나 매번 다시 확인한다.
6. **테스트 범위.** 테스트용 생성자를 열지 않으므로 `BrokerTrade` 구현체(`broker-toss`)는 진짜 `ApprovedOrder`를 받는 테스트를 crate 밖에서 만들 수 없다. `testkit`이 `order-execution`에 의존하는 설계(architecture.md 3.2)는 이 원칙과 4.1의 "의존 가능: src-tauri, broker-toss뿐"과 **서로 충돌한다**(5장).

### Q13. clippy가 잡지 못하는 검사 끄기 수단을 잡는 방법

검증한 xtask(약 400줄, 같은 린트를 적용해 통과함) 결과:

| 수단 | clippy/rustc | xtask |
|---|---|---|
| `#[expect(deny 린트, reason)]` | 통과(20) | `banned-attribute` |
| 안쪽 `#![allow(…, reason)]` | 통과(80) | `banned-attribute` |
| `#[cfg_attr(all(), allow(…))]` | `allow_attributes`가 잡음 | `banned-attribute`(cfg_attr 전면 금지) |
| `#[cfg(not(clippy))]`, `#[cfg(any())]` | 통과(22, 66). `unexpected_cfgs`는 알 수 없는 이름만 잡는다 | `cfg(test)` 말고는 모든 `cfg` 금지 |
| `#[doc = …]`, `///` | 통과 | `banned-attribute`, `line-comment` |
| `#[mutants::skip]` | 의존성이 없으면 E0433. 하지만 `cfg_attr` 형태는 컴파일된다 | `banned-attribute` |
| `#[ignore]`, `#[ignore = "이유"]` | 이유가 없을 때만 잡음 | `banned-attribute` |
| `#[path = …]`, `include!` | 파일이 없으면 오류. 파일이 있으면 통과 | `banned-attribute`, `banned-macro` |
| `macro_rules!`로 린트 숨기기 | 통과(43) | `banned-macro` |
| 매크로 호출 내부의 속성(`proptest! { #[ignore] … }`) | 통과 | 잡음(76). 토큰 트리를 재귀로 순회하기 때문이다 |
| `[lints]` 절 삭제, crate 폴더 `clippy.toml`, `.cargo/` | 통과 | `lints-not-inherited`, `nested-config` |
| `RUSTFLAGS`, `--cap-lints`, `CLIPPY_CONF_DIR` | 통과 | 해당 없음. `check.sh`가 환경 변수를 비우고, 훅과 CI가 막는다 |

구현 요점: `syn`으로 파싱하면 매크로 본문 안의 속성을 보지 못한다. 그래서 `proc-macro2` 토큰 스트림에서 `#` 뒤의 `[...]` 그룹을 재귀로 찾았다. 허용하는 속성(`must_use`, `derive`, `test`, `cfg(test)`, `serde(...)` 등)은 금지 목록 방식으로 둔다.

## 3. 선택지 비교표와 추천

| 영역 | 선택지 | 추천 | 이유 | 추천이 틀리는 조건 |
|---|---|---|---|---|
| 린트 끄기 방지 | deny만 / 개별 forbid / 그룹 forbid | 그룹은 deny, 핵심 개별 린트는 forbid, 나머지는 xtask | 그룹 forbid는 개별 예외를 무효로 만들어 쓸 수 없다. deny는 안쪽 allow·expect·RUSTFLAGS로 꺼진다 | rustc가 `forbidden_lint_groups`를 하드 오류로 바꾸면서 그룹 예외 문법을 제공할 때 |
| 의존 방향 | cargo-deny wrappers / xtask(cargo metadata) | xtask | cargo-deny는 작업 공간 crate와 전이 경로를 막지 못한다(실측) | cargo-deny가 작업 공간 간선 규칙을 지원할 때 |
| 의존성 허용 | cargo-deny / cargo-vet | cargo-deny | 허용 목록 + 라이선스 + 취약점 + 빌드 스크립트를 한 도구로 처리하고 운영 비용이 낮다 | 외부 배포나 공동 개발로 공급망 감사가 필요해질 때 |
| 십진수 | rust_decimal / bigdecimal / fastnum | rust_decimal | Copy이고 할당이 없으며 28자리, 가장 많이 쓰인다. 연산자 panic은 린트로 막힌다 | 28자리를 넘는 중간 계산이 필요하거나 fastnum이 성숙해 성능이 중요해질 때 |
| 속성 테스트 | proptest / quickcheck / bolero | proptest | 엄격 린트 아래에서 매크로가 문제없이 통과했다. 축소 기능이 있다 | — |
| 변이 테스트 | cargo-mutants | cargo-mutants(`--cap-lints=true`) | 유일한 성숙 도구다. 타입 안전성 검증에도 유용했다 | — |
| 커버리지 | cargo-llvm-cov | cargo-llvm-cov(`-p`별 + `--fail-under-file-lines`) | 안정 툴체인에서 동작한다 | 분기 커버리지가 필수가 될 때(nightly 필요) |
| 테스트 실행·네트워크 | cargo test / nextest + wrapper | nextest + `sandbox-exec` wrapper | 외부 차단과 루프백 허용을 실측으로 확인했다 | Apple이 `sandbox-exec`를 제거하거나 nextest가 wrapper 기능을 바꿀 때 |
| 토스 타입 생성 | progenitor / oas3-gen / typify / openapi-generator | typify(모델만) | 3.1 원본을 받고 컴파일되며 의존성이 적다 | 위 Q11 조건 |
| 주석 검사 | 정규식 / syn / rustc_lexer | ra-ap-rustc_lexer + proc-macro2 | 문자열 안의 `//`를 오탐하지 않고 문서 주석까지 잡는다 | API 변경이 잦아 유지비가 크면, 고정 버전에 머문다 |
| 미사용 의존성 | rustc `unused_crate_dependencies` / cargo-machete | cargo-machete 0.9.2 | rustc 린트는 lib와 test 대상에서 dev 의존을 오탐했다(`trybuild` unused). machete는 실제로 쓰이지 않던 `syn`을 찾았다 | — |

## 4. 위험과 미확인 항목

위험:

- `sandbox-exec`는 폐기 예정으로 표시되어 있다. nextest wrapper script는 실험 기능이다. 둘 다 바뀌면 네트워크 차단이 조용히 사라질 수 있다. 하네스 자체 테스트(외부 연결 실패를 기대하는 테스트)를 `just check`에 상시 넣어 감시한다.
- `disallowed-methods`의 panic 메서드 목록은 사람이 유지해야 한다. 빠진 메서드(예: `u32::pow`, `i32::abs`, `Iterator::sum`, `Duration` 연산 외 기타)는 통과한다. `pow`/`abs`/`sum`은 다음 개정 때 목록에 추가하고 검증해야 한다(이번 초안에는 없다).
- `_ = result;`, `let _x = result;`, `let … else`, 열거형 `==` 비교는 막을 도구가 없다. 리뷰 체크리스트(R-10)에 넣는다.
- `disallowed-types`에 넣은 `Mutex`·`Rc`·`HashMap`은 Tauri 상태 관리나 서드파티 API 경계에서 마찰을 낼 수 있다. 앱 뼈대(R-05)에서 실제로 막히면 ADR로 조정한다.
- 엄격 린트 때문에 `cargo-mutants`를 `--cap-lints`로 돌리면 컴파일 시점 검사가 꺼진 상태로 변이를 평가하게 된다. 판정에는 영향이 없지만 trybuild 같은 진단 비교 테스트는 빼야 한다.
- `ra-ap-rustc_lexer`는 매주 배포된다. 버전을 고정하지 않으면 xtask가 컴파일되지 않을 수 있다(이번에 실제로 열거형 변형이 추가된 것을 봤다).

미확인:

- clippy 1.99 lint 문서의 개별 "Known problems" 전체 문구. 문서 페이지가 매우 길어 일부만 확인했다. 위 오탐·빈틈은 실행으로 확인한 것이다.
- rust_decimal 연산자 panic을 문서가 명시하는지. 동작은 실행으로 확인했다("Addition overflowed").
- bigdecimal·fastnum의 반올림 모드와 serde 세부 동작.
- Rust용 녹화·재생(VCR) 계약 테스트 crate의 현재 상태.
- `use_debug`가 `format!("{v:?}")`에서는 걸리지 않고 `writeln!` 안의 `{e:?}`에서는 걸린 이유. 동작만 관찰했다.
- 이번 측정은 예제 규모(변이체 17개, 테스트 11개)다. 실제 crate 규모에서의 CI 시간은 미확인이다.
- openapi-generator와 oas3-gen의 생성물에 엄격 린트를 걸었을 때의 위반 수(별도 crate로 격리한다는 전제라 세지 않았다).

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·절 | 바꿀 내용 |
|---|---|
| harness.md 1장 (`just check`) | 첫 단계에서 `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, `RUSTDOCFLAGS`, `CLIPPY_CONF_DIR`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`를 비우고 `RUSTFLAGS="-D warnings"`로 고정한다. 모든 cargo 명령에 `--locked`를 붙인다. 테스트는 `cargo test`가 아니라 nextest로만 돌린다 |
| harness.md 2장 (검사 끄기 금지) | 강제 방법을 "핵심 린트 개별 `forbid` + xtask 토큰 검사"로 구체화한다. 금지 대상에 `cfg_attr` 전체, `cfg(test)` 외 모든 `cfg`(특히 `cfg(not(clippy))`, `cfg(coverage)`), `#[ignore = "…"]`, `#[path]`, `include!`, `macro_rules!`, 작업 공간 안의 `build.rs`·proc-macro crate, crate 폴더의 `clippy.toml`·`.cargo/`를 추가한다 |
| harness.md 3.1 | "`clippy::pedantic` 전체(문서 주석을 요구하는 린트 제외)"를 "pedantic·nursery·cargo는 deny. `missing_errors_doc`, `missing_panics_doc`, `redundant_pub_crate`, `missing_const_for_fn`, `option_if_let_else`, `significant_drop_tightening`, `cargo_common_metadata`, `multiple_crate_versions`는 끔"으로 바꾼다(6.1). 도구로 못 막는 우회 목록(Q1)을 리뷰 항목으로 옮긴다. "`domain`에서 부동소수점"은 `float_arithmetic`을 전역으로 적용하고, `domain`의 `f32`/`f64` 식별자는 xtask로 막는 것으로 바꾼다 |
| harness.md 3.1 (새 규약) | 테스트 함수는 `()`를 반환하고 `expect`를 쓴다(`panic_in_result_fn`). 통합 테스트도 `#[cfg(test)] mod`로 감싼다(`tests_outside_test_module`). 소비형 토큰은 `self`를 받는 메서드로 쓴다(`needless_pass_by_value`) |
| harness.md 3.3 | 후보를 확정한다: cargo-nextest 0.9.151(wrapper script), proptest 1.12.0, cargo-mutants 27.1.0(`--cap-lints=true`, PR은 `--in-diff`), cargo-llvm-cov 0.9.1(`-p`별 + `--fail-under-file-lines`), cargo-machete 0.9.2를 추가한다. cargo-vet은 쓰지 않는다 |
| harness.md 5장 | 토스 타입은 typify로 `components.schemas`만 생성한다. 생성물은 보호된 생성 경로의 별도 crate에 두고, 그 경로만 주석·속성 검사에서 제외한다 |
| harness.md 6장 | `order-execution` 줄 커버리지 100%와 방어 분기가 충돌한다. 타입으로 불일치를 없애거나 기준을 "95% + 변이 생존 0"으로 조정한다. 네트워크 금지 강제 방법을 "nextest wrapper + `sandbox-exec`"로 적고, 하네스 자체 테스트로 외부 연결 실패를 검사한다 |
| harness.md 7.1 | 보호 파일에 `.config/nextest.toml`, wrapper 스크립트, `.cargo/mutants.toml`, `supply-chain/`(쓰는 경우)를 추가한다 |
| architecture.md 3.2 | 의존 검사는 `xtask arch-check`가 단독으로 한다고 명시한다(cargo-deny 불가 근거: Q3). `testkit`이 `order-execution`에 의존하는 줄은 4.1과 충돌하므로 고친다. 예: `order-execution` 전용 가짜 객체는 그 crate 내부 테스트에 둔다 |
| architecture.md 4.1 | "`src-tauri` 안 승인 명령 모듈 하나로 제한"을 `approval-command` crate로 분리해 그래프 검사 대상으로 만든다 |
| architecture.md 4.2 | 추가: `Proposal<Validated>`는 `Clone`을 구현하지 않고 DB에서 복원하지 않는다(항상 `validate` 재실행). 전송 전 "소비됨/전송 중" 기록과 결정적 `clientOrderId`를 둔다. 토큰에 `&mut` 접근자를 내지 않는다. 토큰 소비는 `self` 메서드로 한다 |
| 다른 리서치 | R-06에 "검증 상태를 저장하지 않음 / HMAC"과 소비됨 고유 제약을, R-10에 환경 변수·`--cap-lints`·`cargo clippy -- -A` 차단 훅과 Q1 리뷰 체크리스트를 넘긴다 |

## 6. 검증한 설정 초안

예제 작업 공간에서 `check.sh`(6.4)가 종료 코드 0으로 끝났다(2026-11-11 실행). 출력 요지:

```
cargo deny: advisories ok, bans ok, licenses ok, sources ok
nextest:    11 tests run: 11 passed, 0 skipped
llvm-cov:   domain lib.rs 줄 100.00% / order-execution lib.rs 줄 98.31% (기준 95)
mutants:    17 mutants tested in 20s: 7 caught, 10 unviable
```

### 6.1 작업 공간 `Cargo.toml`의 lints 절

각 crate의 `Cargo.toml`에는 이것만 둔다.

```toml
[lints]
workspace = true
```

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unstable_features = "forbid"
unused_must_use = "forbid"
warnings = { level = "deny", priority = -2 }
future_incompatible = { level = "deny", priority = -1 }
let_underscore = { level = "deny", priority = -1 }
nonstandard_style = { level = "deny", priority = -1 }
rust_2018_idioms = { level = "deny", priority = -1 }
unused = { level = "deny", priority = -1 }
deprecated_in_future = "deny"
elided_lifetimes_in_paths = "deny"
explicit_outlives_requirements = "deny"
macro_use_extern_crate = "deny"
meta_variable_misuse = "deny"
missing_debug_implementations = "deny"
non_ascii_idents = "deny"
redundant_lifetimes = "deny"
single_use_lifetimes = "deny"
trivial_casts = "deny"
trivial_numeric_casts = "deny"
unit_bindings = "deny"
unnameable_types = "deny"
unreachable_pub = "deny"
unused_import_braces = "deny"
unused_lifetimes = "deny"
unused_macro_rules = "deny"
unused_qualifications = "deny"
unexpected_cfgs = { level = "deny", check-cfg = ["cfg(clippy)"] }

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }

missing_errors_doc = "allow"
missing_panics_doc = "allow"
cargo_common_metadata = "allow"
multiple_crate_versions = "allow"
redundant_pub_crate = "allow"
missing_const_for_fn = "allow"
option_if_let_else = "allow"
significant_drop_tightening = "allow"

unwrap_used = "forbid"
expect_used = "forbid"
panic = "forbid"
todo = "forbid"
unimplemented = "forbid"
unreachable = "forbid"
panic_in_result_fn = "forbid"
unwrap_in_result = "forbid"
get_unwrap = "forbid"
indexing_slicing = "forbid"
string_slice = "forbid"
exit = "forbid"
mem_forget = "forbid"
fallible_impl_from = "forbid"

as_conversions = "forbid"
as_underscore = "forbid"
as_pointer_underscore = "forbid"
arithmetic_side_effects = "forbid"
integer_division = "forbid"
modulo_arithmetic = "forbid"
float_arithmetic = "forbid"
float_cmp_const = "forbid"
lossy_float_literal = "forbid"
default_numeric_fallback = "forbid"

wildcard_enum_match_arm = "forbid"
rest_pat_in_fully_bound_structs = "forbid"
unneeded_field_pattern = "deny"

let_underscore_must_use = "forbid"
let_underscore_untyped = "forbid"
unused_result_ok = "forbid"
map_err_ignore = "forbid"
assertions_on_result_states = "forbid"
try_err = "deny"

allow_attributes = "forbid"
allow_attributes_without_reason = "forbid"
cfg_not_test = "forbid"
tests_outside_test_module = "forbid"

print_stdout = "forbid"
print_stderr = "forbid"
dbg_macro = "forbid"
use_debug = "deny"

clone_on_ref_ptr = "deny"
create_dir = "deny"
deref_by_slicing = "deny"
empty_drop = "deny"
empty_enum_variants_with_brackets = "deny"
empty_structs_with_brackets = "deny"
error_impl_error = "deny"
filetype_is_file = "deny"
if_then_some_else_none = "deny"
impl_trait_in_params = "deny"
infinite_loop = "deny"
iter_over_hash_type = "deny"
large_include_file = "deny"
min_ident_chars = "deny"
missing_assert_message = "deny"
mixed_read_write_in_expression = "deny"
mod_module_files = "deny"
multiple_inherent_impl = "deny"
mutex_atomic = "deny"
partial_pub_fields = "deny"
pathbuf_init_then_push = "deny"
pub_without_shorthand = "deny"
rc_buffer = "deny"
rc_mutex = "deny"
redundant_type_annotations = "deny"
ref_patterns = "deny"
renamed_function_params = "deny"
return_and_then = "deny"
same_name_method = "deny"
semicolon_outside_block = "deny"
shadow_reuse = "deny"
shadow_same = "deny"
shadow_unrelated = "deny"
str_to_string = "deny"
string_add = "deny"
string_lit_chars_any = "deny"
suspicious_xor_used_as_pow = "deny"
unnecessary_self_imports = "deny"
unseparated_literal_suffix = "deny"
verbose_file_reads = "deny"
disallowed_methods = "forbid"
disallowed_types = "forbid"
disallowed_macros = "forbid"
```

`unused_crate_dependencies`는 넣지 않는다. 켜 보니 dev 의존(`trybuild`)과 테스트에서만 쓰는 의존을 lib 대상에서 오탐했다. 대신 cargo-machete로 검사한다.

### 6.2 `clippy.toml`

루트 하나만 둔다. crate 폴더에 이 파일이 생기면 루트를 대체하므로 xtask가 금지한다.

```toml
msrv = "1.99"
avoid-breaking-exported-api = false
warn-on-all-wildcard-imports = true
check-private-items = false
allow-unwrap-in-tests = false
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = false
allow-print-in-tests = false
allow-unwrap-in-consts = false
allow-expect-in-consts = false
too-many-lines-threshold = 60
too-many-arguments-threshold = 5
cognitive-complexity-threshold = 15
excessive-nesting-threshold = 4
max-fn-params-bools = 0
absolute-paths-max-segments = 3
min-ident-chars-threshold = 1
allowed-idents-below-min-chars = ["f"]

disallowed-methods = [
  { path = "std::process::exit", reason = "종료는 main의 반환값으로" },
  { path = "std::process::abort", reason = "panic 계열 금지" },
  { path = "std::time::SystemTime::now", reason = "Clock 포트를 쓴다" },
  { path = "std::time::Instant::now", reason = "Clock 포트를 쓴다" },
  { path = "std::thread::sleep", reason = "Clock 포트와 비동기 타이머를 쓴다" },
  { path = "std::env::var", reason = "설정은 조립 루트에서 타입으로 읽는다" },
  { path = "std::env::set_var", reason = "전역 상태 변경 금지" },
  { path = "std::vec::Vec::remove", reason = "범위 밖이면 panic. checked 대안을 쓴다" },
  { path = "std::vec::Vec::insert", reason = "범위 밖이면 panic" },
  { path = "std::vec::Vec::swap_remove", reason = "범위 밖이면 panic" },
  { path = "std::vec::Vec::split_off", reason = "범위 밖이면 panic" },
  { path = "std::vec::Vec::drain", reason = "범위 밖이면 panic" },
  { path = "slice::split_at", reason = "범위 밖이면 panic. split_at_checked" },
  { path = "slice::split_at_mut", reason = "범위 밖이면 panic. split_at_mut_checked" },
  { path = "slice::copy_from_slice", reason = "길이가 다르면 panic" },
  { path = "slice::chunks", reason = "0이면 panic. NonZero를 받는 래퍼를 쓴다" },
  { path = "slice::windows", reason = "0이면 panic" },
  { path = "str::split_at", reason = "경계가 아니면 panic. split_at_checked" },
  { path = "std::string::String::insert", reason = "경계가 아니면 panic" },
  { path = "std::string::String::remove", reason = "경계가 아니면 panic" },
  { path = "std::string::String::truncate", reason = "경계가 아니면 panic" },
  { path = "std::string::String::drain", reason = "경계가 아니면 panic" },
  { path = "std::cell::RefCell::borrow", reason = "런타임 panic. try_borrow" },
  { path = "std::cell::RefCell::borrow_mut", reason = "런타임 panic. try_borrow_mut" },
  { path = "std::iter::Iterator::step_by", reason = "0이면 panic" },
  { path = "std::option::Option::unwrap_or_default", reason = "실패를 기본값으로 숨기지 않는다" },
  { path = "std::result::Result::unwrap_or_default", reason = "오류를 기본값으로 숨기지 않는다" },
  { path = "std::result::Result::unwrap_or", reason = "오류를 숨기지 않는다. match로 처리" },
  { path = "std::result::Result::unwrap_or_else", reason = "오류를 숨기지 않는다. match로 처리" },
  { path = "std::mem::drop", reason = "Result를 버리는 우회로. 스코프로 해제한다" },
  { path = "std::mem::take", reason = "상태를 몰래 비우지 않는다" },
  { path = "std::mem::replace", reason = "소비형 토큰 재사용 우회로" },
  { path = "std::mem::swap", reason = "소비형 토큰 재사용 우회로" },
  { path = "rust_decimal::Decimal::from_f64_retain", reason = "부동소수점에서 금액을 만들지 않는다", allow-invalid = true },
]

disallowed-macros = [
  { path = "std::unreachable", reason = "panic 계열 금지" },
  { path = "std::include", reason = "검사를 피해 코드를 넣는 경로" },
  { path = "std::include_str", reason = "fixtures는 테스트 로더로 읽는다" },
  { path = "std::include_bytes", reason = "fixtures는 테스트 로더로 읽는다" },
  { path = "std::env", reason = "빌드 시점 환경 변수 금지" },
  { path = "std::option_env", reason = "빌드 시점 환경 변수 금지" },
  { path = "std::eprint", reason = "로깅 계층으로" },
  { path = "std::print", reason = "로깅 계층으로" },
]

disallowed-types = [
  { path = "std::collections::HashMap", reason = "순회 순서가 비결정적. BTreeMap" },
  { path = "std::collections::HashSet", reason = "순회 순서가 비결정적. BTreeSet" },
  { path = "std::cell::RefCell", reason = "런타임 대여 panic" },
  { path = "std::rc::Rc", reason = "공유 소유는 설계 검토 대상" },
  { path = "std::sync::Mutex", reason = "poison 처리에서 unwrap을 부른다. 메시지 전달로" },
]
```

검증한 선택 항목(초안에는 넣지 않았다): `disallowed-macros`에 `{ path = "std::matches", … }`를 추가하면 `matches!` 와일드카드 우회(26)가 잡힌다. 다만 xtask 자신도 `matches!`를 쓰고 있어 함께 고쳐야 한다. 넣는 것을 권한다.

### 6.3 `deny.toml`

```toml
[graph]
all-features = true
no-default-features = false
exclude-dev = false
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]

[output]
feature-depth = 1

[advisories]
version = 2
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"
unmaintained = "all"
unsound = "all"
ignore = []

[licenses]
version = 2
confidence-threshold = 0.93
allow = ["Apache-2.0", "Apache-2.0 WITH LLVM-exception", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib"]
exceptions = []

[licenses.private]
ignore = true

[bans]
multiple-versions = "warn"
wildcards = "deny"
allow-wildcard-paths = true
highlight = "all"
workspace-default-features = "allow"
external-default-features = "allow"
allow-workspace = true
allow = [
  { crate = "rust_decimal", reason = "domain 금액" },
  { crate = "proptest", reason = "속성 테스트" },
  { crate = "trybuild", reason = "봉인 생성자 compile-fail 테스트" },
  { crate = "syn", reason = "xtask" },
  { crate = "proc-macro2", reason = "xtask" },
  { crate = "quote", reason = "proc-macro2 전이" },
  { crate = "ra-ap-rustc_lexer", reason = "xtask 주석 검사" },
  { crate = "cargo_metadata", reason = "xtask arch-check" },
  { crate = "toml", reason = "xtask manifest 검사" },
  # 아래는 예제 작업 공간의 전이 의존성 전체(각각 reason = "전이 의존성")
  # serde serde_core serde_derive thiserror thiserror-impl arrayvec autocfg bit-set bit-vec
  # bitflags camino cargo-platform cfg-if chacha20 core_detect cpufeatures errno fastrand fnv
  # getrandom glob itoa libc memchr num-traits once_cell quick-error rand rand_core
  # rand_xorshift regex-syntax rustix rusty-fork semver serde_json serde_spanned target-tuple
  # tempfile termcolor toml_datetime toml_parser toml_writer unarray unicode-ident
  # unicode-properties wait-timeout winnow zmij
]
deny = [
  { crate = "openssl", use-instead = "rustls" },
  { crate = "openssl-sys", use-instead = "rustls" },
]
skip = []
skip-tree = []

[bans.build]
allow-build-scripts = [
  { crate = "proc-macro2" }, { crate = "quote" }, { crate = "serde" }, { crate = "serde_core" },
  { crate = "rust_decimal" }, { crate = "thiserror" }, { crate = "libc" }, { crate = "rustix" },
  { crate = "num-traits" }, { crate = "getrandom" }, { crate = "camino" }, { crate = "target-tuple" },
  { crate = "zmij" }, { crate = "serde_json" }, { crate = "syn" },
]
executables = "deny"
interpreted = "deny"
include-dependencies = true
include-workspace = true
include-archives = true

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

위 초안에서 주석으로 묶은 전이 의존성 목록은 실제로 검증한 파일에서는 한 줄에 하나씩 `{ crate = "…", reason = "전이 의존성" }`으로 적혀 있다. 이 초안은 문서에서 읽기 쉽게 접은 것이다. deny.toml에 넣은 이 주석은 생성물이 아니라 보호 설정 파일의 주석이므로, 주석 금지(Rust 소스 대상)와 무관하다.

작업 공간 crate 사이의 금지 간선(`order-execution`)은 cargo-deny에 넣지 않는다. 넣어 봐야 무시되거나 설정 오류가 난다(Q3).

### 6.4 검사 스크립트와 nextest 설정 (검증본)

```sh
#!/bin/sh
set -eu
cd "$(dirname "$0")"
for v in RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS RUSTDOCFLAGS CLIPPY_CONF_DIR RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_TARGET_DIR; do unset "$v"; done
export RUSTFLAGS="-D warnings"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked
cargo run --quiet --locked -p xtask -- "$PWD"
cargo deny --locked check
cargo nextest run --workspace --locked
cargo llvm-cov nextest --locked -p domain --fail-under-file-lines 95
cargo llvm-cov nextest --locked -p order-execution --fail-under-file-lines 95
```

`check-full`에는 다음을 더한다.

```sh
cargo mutants -p domain -p order-execution --test-tool nextest --cap-lints=true -- -E 'not test(sealed_constructors)'
```

`.config/nextest.toml`:

```toml
experimental = ["wrapper-scripts"]

[scripts.wrapper.no-network]
command = { command-line = "scripts/no-network.sh", relative-to = "workspace-root" }

[[profile.default.scripts]]
filter = "all()"
platform = "cfg(target_os = \"macos\")"
run-wrapper = "no-network"
```

`scripts/no-network.sh`:

```sh
#!/bin/sh
exec /usr/bin/sandbox-exec -p '(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))(allow network-outbound (remote unix-socket))' "$@"
```

### 6.5 xtask 검사 항목 (검증본 요지)

- `source_checks`: `crates/**/*.rs` 전체를 검사한다.
  - `ra-ap-rustc_lexer`로 주석 토큰을 찾는다.
  - `proc-macro2` 토큰 트리를 재귀로 순회해 `#[...]`·`#![...]`의 이름이 `allow`, `expect`, `warn`, `deny`, `forbid`, `doc`, `path`, `ignore`, `cfg_attr`, `no_mangle`, `export_name`, `link_section`, `automatically_derived`, `mutants::*`, `clippy::*`, `rustfmt::*`이거나 `cfg(test)`가 아닌 `cfg`이면 실패시킨다.
  - `macro_rules!`, `include*!`, `asm!`, `global_asm!`도 실패시킨다.
- `manifest_checks`: 작업 공간의 각 패키지가 정확히 `lints = { workspace = true }`만 갖는지 확인한다. `build.rs`와 proc-macro 대상을 금지한다. `[target]`, `[profile]`, `[patch]`, `[replace]`, `[features]` 표를 금지한다.
- `arch_checks`: `cargo_metadata`의 일반·빌드 의존을 허용 간선 표와 비교한다. 표에 없는 crate가 있으면 실패시킨다.
- `config_file_checks`: `crates/` 아래의 `clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `rust-toolchain*`, `build.rs`, `.cargo/`를 금지한다.

## 7. 출처

모두 2026-11-11에 확인했다.

- Rust stable 채널 매니페스트(rust 1.99.1 58cf8f9c3 2026-10-13, clippy-preview 0.1.99): https://static.rust-lang.org/dist/channel-rust-stable.toml
- Cargo `[lints]` 절: https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
- Cargo 설정 계층과 `build.rustflags` 우선순위: https://doc.rust-lang.org/cargo/reference/config.html
- rustc 린트 수준(forbid, `--cap-lints`, expect): https://doc.rust-lang.org/rustc/lints/levels.html
- Clippy 설정 파일 탐색과 `cfg(clippy)`: https://doc.rust-lang.org/clippy/configuration.html
- Clippy 설정 키 목록(`allow-*-in-tests`, `disallowed-*`, `allow-invalid`): https://doc.rust-lang.org/clippy/lint_configuration.html
- Clippy 1.99 린트 목록(`allow_attributes`는 바깥 속성만, `as_conversions`, `arithmetic_side_effects`, `cfg_not_test`): https://clippy.rust-lang.org/rust-1.99.0/index.html
- 린트 전체 목록: 로컬 `clippy-driver rustc -W help`, `rustc -W help` (1.99.1)
- cargo-deny bans 설정(`allow` 허용 목록 의미, `wrappers`, `allow-workspace`, `[bans.build]`): https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html
- cargo-vet 개요: https://mozilla.github.io/cargo-vet/
- cargo-mutants 건너뛰기, 속성, 증분 실행: https://mutants.rs/skip.html , https://mutants.rs/attrs.html , https://mutants.rs/in-diff.html
- cargo-llvm-cov(`--branch` unstable, `--fail-under-*`, `cfg(coverage)`): https://github.com/taiki-e/cargo-llvm-cov
- nextest wrapper scripts(실험 기능, 0.9.98): https://nexte.st/docs/configuration/wrapper-scripts/
- rust_decimal 1.43.0(96비트, 28자리, serde 기능): https://docs.rs/rust_decimal/1.43.0/rust_decimal/
- rust_decimal RoundingStrategy: https://docs.rs/rust_decimal/1.43.0/rust_decimal/enum.RoundingStrategy.html
- rust_decimal Decimal(`checked_add`, `from_f64_retain`, `from_str_exact`): https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html
- crate 버전과 배포일(crates.io API): https://crates.io/api/v1/crates/{progenitor, typify, oas3-gen, rust_decimal, bigdecimal, fastnum, proptest, cargo-mutants, cargo-llvm-cov, cargo-deny, cargo-vet, cargo-nextest, trybuild, wiremock, httpmock, mockito, ra-ap-rustc_lexer, cargo_metadata, cargo-progenitor, cargo-typify}
- openapi-generator 최신 릴리스(v7.27.0, 2026-11-02): https://api.github.com/repos/OpenAPITools/openapi-generator/releases/latest
- `sandbox-exec` 폐기 표시: 로컬 `man sandbox-exec` (macOS 26.5.2)
- 토스 스펙: 저장소 `docs/toss-openapi-1.2.28.json` (`clientOrderId` 멱등성 10분 설명 포함)
- 실행 결과: 예제 작업 공간, 위반 예제, 측정 로그(이 문서의 표에 인용함)
