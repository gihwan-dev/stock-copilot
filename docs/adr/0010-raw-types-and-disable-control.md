# ADR 0010: 원시 타입 검사(`xtask raw-types`)와 Rust 끄기 통제(`xtask disable-check`, `disable-report`)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-39, H-40(Rust 쪽)
- 반영한 문서: 없음 (harness.md 2.1, 3.4, 6.8과 architecture.md 3.1~3.3, ADR 0005 알려진 빈틈 (c), ADR 0008을 따르고, 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 3.4는 `domain`·`application`·포트 crate의 시그니처와 필드에 원시 타입과 `bool`을 쓰면 `xtask raw-types`가 실패한다고 정했다. 허용은 newtype의 비공개 필드 하나와 그 생성자·`as_*` 접근자, 표준·외부 트레이트 구현의 `bool`이다. 무엇이 생성자인지, 어떤 트레이트가 예외인지, 포트 crate가 무엇인지는 적혀 있지 않다. harness.md 2.1과 ADR 0005 (c)는 `#[expect]`로 끌 수 있는 것을 `pedantic`·`nursery`·`cargo` 그룹 소속 린트로 좁히고, 개별 `deny` 린트와 `disallowed_*`는 `main.rs`의 `generate_context!` 문장 하나만 허용 목록으로 둔다고 정했다. 어떤 린트가 세 그룹 소속인지, `expect`를 어디에 붙일 수 있는지는 적혀 있지 않다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, clippy 0.1.99 (58cf8f9c35 2026-10-13), `syn` 2.0.119(`full`, `visit`), `proc-macro2` 1.0.107(`span-locations`), cargo-nextest 0.9.151.

| 항목 | 명령 | 결과 |
|---|---|---|
| 그룹 소속 목록 | `$(rustup which clippy-driver) -W help` | rustc 그룹 13개, clippy 그룹 10개. 세 그룹 소속 린트 202개 |
| 세 그룹과 개별 설정의 겹침 | `[workspace.lints.clippy]`와 비교 | `fallible_impl_from`(nursery, `forbid`)과 `allow`로 둔 8개(`missing_errors_doc` 등)가 겹친다 |
| 실제 작업 공간 | `just raw-types`, `just disable-check` | 둘 다 위반 없음 |
| 실제 작업 공간 보고 | `just disable-report` | `main.rs` 4, 8, 12줄의 `disallowed_macros`·`disallowed_methods`·`disallowed_types` 3건 |
| 예제 | `cargo nextest run -p xtask raw_types disable_control` | 원시 타입 예제 62개와 모듈 트리 예제 1개, 끄기 예제 40개가 기대 규칙 집합과 같다 |
| 규칙 하나 빼기 | `CStr`와 (`PartialEq`, `ne`)를 코드 목록에서 지우고 같은 명령 | `os-strings.rs`와 `clean-standard-trait-bool.rs`가 기대와 달라 실패 |
| 데이터 원소 하나 빼기 | `removing_any_*_breaks_some_test` | `crates.toml` 원소 변형 60개 넘게, `lints.toml`·`allowlist.toml` 변형 240개 넘게 모두 어떤 검사를 바꾼다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 대상 crate는 `domain`, `application`, `order-execution`이다. 포트 crate는 `BrokerTrade`를 정의하는 `order-execution` 하나다 | architecture.md 3.3: 포트는 `application`이 정의하고 `BrokerTrade`만 `order-execution`이 정의한다 | `approval-command`까지 대상 | architecture.md 3.3 |
| `harness/raw-types/crates.toml`에 모든 작업 공간 crate를 `target`, `exception`(경계 어댑터), `outside`(대상도 어댑터도 아님) 중 하나로 적는다. 분류되지 않은 crate나 작업 공간에 없는 crate가 있으면 검사가 실패한다 | 새 crate가 조용히 검사 밖에 놓이지 않는다. 예외 crate 목록을 지우면 실제 작업 공간 검사가 실패한다 | 대상만 적기 | harness.md 3.4, 7.1 |
| 경계 어댑터는 `broker-toss-schema`, `broker-toss`, `broker-toss-trade`, `sources`, `ai-*`, `store-sqlite`, `secrets-keychain`, `platform-macos`, `mcp-server`, `src-tauri`다 | architecture.md 3.1의 `adapters/`, 인바운드 어댑터 `mcp-server`, IPC DTO를 가진 `src-tauri` | - | architecture.md 3.1 |
| 원시 타입 이름: `String`, `str`, `char`, `OsString`, `OsStr`, `CString`, `CStr`, `PathBuf`, `Path`, 정수 12개, `NonZero`와 `NonZeroI*`·`NonZeroU*` 12개, `f32`, `f64`, `Decimal`, `bool`. 타입 경로의 마지막 이름으로 판정하고 제네릭 인자, 참조, 튜플, 배열, 슬라이스, `dyn`·`impl` 경계, 함수 포인터 안까지 내려간다. `Vec<u8>`은 `u8`로 걸린다 | "이들을 담은 `Option`·`Vec`·튜플"을 감싸는 타입 종류와 무관하게 적용한다(`HashMap`, `Cow`, `Arc`도 포함). 문자열·정수와 같은 값인 OS 문자열·경로·`NonZero`도 원시 값이다 | 문서에 적힌 이름만 | harness.md 3.4 |
| 보는 곳: 자유 함수·메서드·트레이트 메서드·외부 함수의 매개변수와 반환, 모든 항목의 제네릭 경계와 `where` 절과 const 제네릭, 구조체·열거형 변형·공용체 필드, `const`·`static`, `impl` 머리(자기 타입과 트레이트 인자), 연관 타입 값과 경계. 함수 본문의 지역 변수와 클로저는 보지 않지만 본문 안의 항목은 본다 | 지역 계산은 3.4의 "조건식 안에서 생기는 값"과 같다 | 시그니처와 필드만 | harness.md 3.4 |
| 타입 별칭은 오른쪽이 무엇이든 모두 실패한다(`raw-type-alias`) | "타입 별칭은 전용 타입이 아니다" | 원시 타입을 가리키는 별칭만 | harness.md 3.4 |
| newtype: 이름 있는 튜플 구조체이고 필드가 하나이며 그 필드에 `pub`·`pub(…)`이 없고 필드 타입이 `bool`을 뺀 원시 타입 이름 하나(제네릭 인자 없음) 또는 `Vec<u8>`인 것. 그 필드만 검사에서 빠진다 | `bool` newtype은 열거형 규칙을 피하는 길이다. `Option<String>` 같은 묶음은 값 하나가 아니다 | 필드 타입 제한 없음 | harness.md 3.4 |
| 생성자: 같은 파일에 정의된 newtype의 고유 `impl` 안에서 이름이 `new` 또는 `try_new`이고, `self`가 없고, 입력이 하나이며, 반환이 `Self`·그 이름·`Result<Self, E>`·`Option<Self>`인 함수. 입력만 검사에서 빠지고 `E`는 검사한다 | 가장 엄격한 해석. `from_cents` 같은 다른 이름, `From<u64>`·`TryFrom<Decimal>`·`FromStr` 구현은 생성자로 보지 않는다 | 이름 제한 없음, 트레이트 변환 허용 | harness.md 3.4 |
| 접근자: 같은 newtype의 고유 `impl` 안에서 이름이 `as_`로 시작하고 입력이 `&self` 하나(`&mut self`, `self`, 타입을 적은 `self` 제외)인 메서드. 반환의 원시 타입은 통과하지만 `bool`은 실패한다 | 주문 토큰의 "`&mut` 접근자 없음"(architecture.md 4.3)과 맞춘다 | `&mut self` 허용 | harness.md 3.4 |
| 트레이트 구현의 `bool` 예외는 (`cmp`, `PartialEq`, `eq`·`ne`)와 (`cmp`, `PartialOrd`, `lt`·`le`·`gt`·`ge`) 여섯 메서드뿐이다. 트레이트 경로가 이름 하나(그 crate가 같은 이름의 트레이트를 정의하지 않을 때)이거나 `std::cmp::…`·`core::cmp::…`일 때만 적용한다. 그 메서드에서도 `bool`이 아닌 원시 타입은 실패한다 | 표준이 시그니처를 정한 `bool` 반환 메서드 중 구현할 일이 있는 것만 연다. `Ord::cmp`는 `Ordering`을 돌려주므로 따로 열 필요가 없다 | 모든 외부 트레이트 구현 통과 | harness.md 3.4 |
| 테스트 코드(`#[cfg(test)]`가 붙은 `mod`와 그 파일, cargo의 `test`·`bench` 대상)는 보지 않는다. 대상 crate의 lib·bin·example 대상 루트에서 `mod` 선언을 따라 파일을 찾는다 | 테스트는 리터럴로 값을 만든다. 시그니처가 공개 API로 새지 않는다. 모듈 트리를 따르므로 테스트 모듈이 아닌 파일을 `_tests` 이름으로 숨길 수 없다 | 폴더의 모든 `.rs` | harness.md 3.3 |
| 항목 위치의 매크로(`foo! { … }`)는 내용을 항목으로 해석해 같은 검사를 하고, 해석되지 않으면 `raw-type-opaque-macro`로 실패한다. 식·문장 위치의 매크로는 항목으로 해석될 때만 본다 | 매크로가 만든 시그니처를 이름 없이 통과시키지 않는다 | 매크로 무시 | - |
| `expect`로 끌 수 있는 린트: `harness/disable-control/lints.toml`의 `disableable`(clippy 0.1.99의 `clippy::pedantic`·`nursery`·`cargo` 소속)에 있고, 루트 `[workspace.lints]`에 개별로 적혀 있지 않은 것. 개별로 적힌 것은 수준(`allow`·`deny`·`forbid`)과 무관하게 실패한다 | `fallible_impl_from`처럼 그룹 소속이지만 개별 `forbid`인 린트, 개별 `allow`라 끌 이유가 없는 린트가 빠진다 | 그룹 소속만 보기 | harness.md 2.1, ADR 0005 (c) |
| `lints.toml`은 clippy 버전 문자열, 모든 그룹 이름(rustc 13개, clippy 10개), 세 그룹 소속 린트를 담는다. 테스트 `lint_catalogue_matches_pinned_clippy`가 `rustup which clippy-driver`로 고정 툴체인의 `-W help`와 `--version`을 읽어 같은지 보고, 다르면 새로 만든 파일 내용을 출력한다 | 목록을 손으로만 관리하지 않는다. 툴체인을 올리면 테스트가 바뀐 목록을 알려 준다 | 실행할 때마다 clippy에서 읽기(검사가 툴체인 출력 형식에 묶인다), 손으로 관리 | 브리프 |
| 린트 자리에 그룹 이름(`warnings`, `unused`, `clippy::all`, `clippy::pedantic` 등 `groups`의 이름)이 있으면 `disable:lint-group`, 끌 수 없는 린트면 `disable:not-disableable` | - | - | harness.md 2.1 |
| 허용 목록 `harness/disable-control/allowlist.toml`의 항목은 (파일, `let` 문, 린트)다. 문은 `let`부터 `;`까지의 토큰을 비교하고 속성은 빼고 본다. 목록에 없는 파일, `let`이 아닌 문은 목록 읽기가 실패한다 | `main.rs` 안의 다른 문, 함수 전체, 다른 파일의 같은 문, 다른 린트에는 통하지 않는다 | 파일 단위 허용 | ADR 0005 |
| `expect`를 붙일 수 있는 곳: 함수(자유·메서드·트레이트·외부), `let` 문, 블록이 아닌 식 문장, 구조체 필드, 열거형 변형, 하위 항목이 없는 단일 항목(`struct`, `enum`, `union`, `const`, `static`, `type`, `use`, `extern crate`, 연관 상수·타입, 외부 정적·타입). 그 밖은 `disable:placement:<곳>`: `module`, `impl`, `trait`(트레이트 별칭 포함), `foreign-block`, `macro`(항목·문장 매크로 호출, 매크로 입력 안의 모든 `expect`), `block`(`{…}`, `if`, `match`, `loop`, `while`, `for`, `unsafe`, `async`, `const`, 레이블로 시작하는 식 문장), `inner`(`#![expect]` 전부), `other`(매치 갈래, 매개변수, 식 안 등) | "함수, 문장, 필드, 단일 항목"의 가장 엄격한 해석. 구조체는 필드를 담지만 필드는 항목이 아니므로 단일 항목이다. 블록 식 문장은 여러 문장을 한꺼번에 끈다. 매크로는 입력을 어떻게 펼칠지 알 수 없다 | 구조체·열거형도 금지(필드 이름 계열 pedantic 린트를 끌 길이 없어진다) | harness.md 2.1, tasks.md H-40 |
| 검사 대상 파일은 작업 공간 멤버 폴더의 모든 `.rs`(다른 멤버 폴더와 `target_directory` 제외)다. 생성 경로도 뺀다는 규칙을 두지 않는다 | 끄기는 어디에 있든 보고돼야 한다 | ADR 0008처럼 생성 경로 제외 | harness.md 2.1 |
| `disable-report`는 `| 종류 | 파일 | 줄 | 규칙 | 이유 |` 표와 `disable-report: 끄기 N건 (종류별 건수)` 줄을 출력한다. Rust 행의 종류는 `rust-expect`이고 `expect` 하나의 린트마다 한 행이다. TS 수집이 같은 `Row`에 종류 `eslint-disable-next-line`으로 행을 더하면 같은 표와 건수에 들어간다(`disable_report_table_takes_other_kinds`) | harness.md 2.1이 TS 끄기를 같은 보고에 넣는다 | 언어별 명령 | harness.md 2.1 |
| `just check`는 `source-check` 뒤에 `raw-types`, `disable-check`를 돌고, nextest 뒤 마지막에 `disable-report`를 돈다 | "`just check` 끝에 개수를 보여 준다" | - | harness.md 2.1 |
| xtask의 새 외부 의존은 없다. `syn`의 기본 기능과 `proc-macro2`의 `span-locations`(`Span::source_text`)만 쓴다 | 허용 목록(deny.toml, arch-check 규칙)을 바꾸지 않는다 | `quote` 추가 | architecture.md 3.2 |

## 결과

- 지금 대상 crate는 비어 있어 실제 작업 공간은 통과한다. 대상 crate에 코드를 쓰면 `as_*` 이름의 접근자와 `new`/`try_new` 생성자만 원시 타입을 받고 내보낼 수 있다. `From<u64>`·`FromStr`·`Display`가 아닌 표준 트레이트로 원시 타입을 주고받는 구현은 실패하므로 생성자로 바꾼다.
- 원리상 `raw-types`가 놓치는 것:
  - 제네릭 매개변수에 원시 타입을 넣는 호출(`Port<T>`의 `T`에 `u64`를 넣는 쪽은 호출부이고, 호출부가 대상 밖 crate이거나 함수 본문이면 보지 않는다)
  - 이름으로 판정하므로 별칭으로 가져온 원시 타입(`use std::string::String as Text;`의 `Text`)과 같은 이름의 사용자 타입(사용자 정의 `Path`, `Decimal`은 오탐)
  - 외부 매크로·derive가 만든 시그니처, 식·문장 위치 매크로가 만드는 항목, 해석되는 매크로 입력을 다르게 펼치는 매크로
  - `impl Trait` 반환과 `dyn Trait` 뒤의 실제 타입(`-> impl Display`가 `String`을 감춘다), 다른 crate가 정의한 연관 타입(`T::Item`), 트레이트 객체의 연관 타입 기본값
  - 대상 밖 crate(어댑터, `approval-command`, `scheduler` 등)의 포트 구현 안에서 원시 타입을 쓰는 것. 포트 시그니처가 전용 타입이므로 경계 밖으로는 나가지 못한다
  - 원자 타입(`AtomicBool`, `AtomicU64`), `Duration`처럼 목록에 없는 표준 값 타입
- 원리상 `disable-check`가 놓치는 것: 매크로가 펼쳐서 만드는 `expect`(입력에 없는 것), 허용된 함수에 붙인 `expect`가 함수 본문 전체를 덮는 것(함수 단위 허용이 문서의 결정), 토큰 비교라서 허용 목록 문의 의미가 같아도 표기가 다르면 실패하는 것.
- 테스트 `lint_catalogue_matches_pinned_clippy`는 `rustup`과 고정 툴체인의 `clippy-driver`가 있어야 돈다. 툴체인을 올리면 이 테스트가 새 목록을 출력하며 실패하고, 목록 갱신은 보호 파일 변경이므로 사용자 서명 커밋으로 들어간다.
- TS 끄기(`eslint-disable-next-line`) 수집은 H-40의 TS 쪽 작업이 `Row`를 만들어 `render_report`에 더한다.
- 바뀌면 다시 볼 조건: Rust·clippy 버전을 올릴 때, `[workspace.lints]`에 세 그룹 소속 린트를 개별로 적을 때, 대상 crate가 늘거나 포트가 다른 crate로 옮겨 갈 때, `generate_context!` 문이 바뀔 때.
