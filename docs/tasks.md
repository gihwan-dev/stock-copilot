# 작업 단위 (v0.2, 2026-11-11)

- 작업 하나는 서브에이전트 한 번의 실행으로 끝나는 크기다. 한 번의 리뷰로 다 볼 수 있어야 한다 (완료 기준 3~5개, [workflow.md](workflow.md) 2.1).
- 모든 작업은 [workflow.md](workflow.md) 절차를 거친다. 공통 완료 기준: `just check` 통과, 리뷰 절차 완료, `docs/reviews/<작업 ID>.md` 기록.
- 표의 "완료 기준"은 그 작업에만 해당하는 기준이다. 괄호 안은 근거 요구사항이나 문서 절이다.
- 상태: `대기` `진행` `리뷰` `완료` `막힘`. R 묶음만 `완료`이고 나머지는 전부 `대기`라서 열을 생략했다. 시작하면 상태 열을 추가한다.
- "(담당: 사용자)"인 작업은 에이전트가 하지 않는다. "(사용자 승인)"은 보호 파일([harness.md](harness.md) 7.1)을 만들거나 바꾸는 작업이라 사용자 서명 커밋이 있어야 들어간다. H-20 잠금 전의 하네스 작업은 보호 파일을 직접 만든다. H-20은 로컬 잠금(권한 규칙, 훅, 샌드박스)이고 CI와 서명 키를 기다리지 않는다.
- 실측 ID는 [open-issues.md](open-issues.md) 4.3의 것이다. 작업 묶음 S(저장소)와 번호가 겹치므로 이 문서에서는 항상 "실측 S-nn"으로 쓴다.
- v0.1에서 없어진 작업: U-12(시장 현황 화면, 주 화면 하나 원칙), N-15(뉴스 화면), O-02(`propose_order`), O-10(거절 사유를 다음 제안에 반영. 이제는 반영하지 않는다), O-16(아이디어), G-02(`get_policy`, `get_proposal_history`), G-03·G-04(제안 성과와 성적표. E 묶음으로 옮김).

## 진행 순서

```
R 리서치 (완료)
T-00 토스 연결 확인 (사용자) ·············································· T-13 실측 녹화 (사용자)
H-16·H-26·H-27 CI, H-28 서명 키 (사용자) ·········· 나란히 진행. 도메인 작업을 막지 않는다
H 하네스 ─▶ H-20 잠금 (사용자 승인) ─┬─▶ D 도메인 ─▶ A 유스케이스 ─┬─▶ T 토스 ──┐
                                    │                              └─▶ S 저장소 ─┤
DS-00 디자인 (사용자) ─▶ DS 디자인 시스템 ───────────────────────────────────────┤
                                                                                ▼
                         U 앱 상주 · 첫 실행 · 자체 점검 · 주 화면 · 설정 기본 3항목
                                                                                ▼
                         M MCP ─▶ AI 러너 (분석 실행 · 수집 실행) ─▶ C 채팅
                                                                                ▼
                         N 뉴스·공시 · 웹 조사 ─▶ J 예약 작업 ─▶ E 추정 · 신호 · 판정 · 성적표
                                                                                ▼
            ┌──────────── E-90 추정만 기록하는 기간 (사용자) ────────────┐
            │ 주문 기능 없음. 신호 카드에 주문 버튼 없음. 판정 30건 이상 │
            └─────────────────────────────┬──────────────────────────────┘
O 주문 개발 (E와 함께 진행, 스위치는 꺼진 채) ┤
                                          ▼
                     E-91·E-92 보정·세부 값 ─▶ O-15 소액 실주문 (사용자) ─▶ 사용자가 주문 켜기
                                          ▼
                     G 심층 연구 · 유망 종목 · 비교 · 성능 점검 (AI-09 Codex는 이 뒤)
```

## R. 리서치

프롬프트는 [research.md](research.md)에 있다. 남은 실측은 open-issues 4.3의 ID로 각 작업에 연결했다.

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| R-01 | 토스증권 Open API | 완료. [R-01](research/R-01-toss-open-api.md) | - |
| R-02 | Claude Code 런타임 | 완료. [R-02](research/R-02-claude-code-runtime.md) | - |
| R-03 | Codex CLI 런타임 | 완료. [R-03](research/R-03-codex-cli-runtime.md) | - |
| R-04 | Rust MCP 서버 | 완료. [R-04](research/R-04-rust-mcp-server.md) | - |
| R-05 | Tauri 구조 | 완료. [R-05](research/R-05-tauri-architecture.md) | - |
| R-06 | SQLite 저장소 | 완료. [R-06](research/R-06-sqlite-store.md) | - |
| R-07 | 뉴스 소스 | 완료. [R-07](research/R-07-news-sources.md) | - |
| R-08 | TypeScript 엄격성 | 완료. [R-08](research/R-08-typescript-strictness.md) | - |
| R-09 | Rust 엄격성 | 완료. [R-09](research/R-09-rust-strictness.md) | - |
| R-10 | 에이전트 하네스 | 완료. [R-10](research/R-10-agent-harness.md) | - |
| R-11 | 포트폴리오 계산 | 완료. [R-11](research/R-11-portfolio-math.md) | - |
| R-12 | AI 분석 설계 | 완료. [R-12](research/R-12-ai-analysis-design.md) | - |
| R-13 | 법률과 약관 | 완료. [R-13](research/R-13-legal-and-terms.md) | - |
| R-14 | 디자인 시스템과 코드 | 완료. [R-14](research/R-14-design-system-to-code.md) | - |
| R-15 | Tauri 테스트 | 완료. [R-15](research/R-15-tauri-testing.md) | - |
| R-16 | 로컬 보안 | 완료. [R-16](research/R-16-local-security.md) | - |
| R-90 | 리서치 결과와 사용자 결정을 design·architecture·harness·workflow·tasks에 반영 | v0.2 문서가 서로 어긋나지 않고 open-issues 4.3의 실측 ID를 가리킨다. 사용자 확인 대기 | R-01 ~ R-16 |

## H. 하네스

하네스 작업은 보호 파일을 만든다. 이 단계에서만 에이전트가 보호 파일을 쓸 수 있고, H-20에서 잠근다. 위반 예제는 하네스 자체 테스트([harness.md](harness.md) 6.8)에 들어간다.

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| H-01 | Rust 뼈대: 루트 가상 작업 공간, `rust-toolchain.toml`, [architecture.md](architecture.md) 3.1의 crate 전부를 빈 crate로(`approval-command`, `broker-toss-trade`, `broker-toss-schema`, `platform-macos`, `mcp-bridge`, `trade-fakes`, `e2e-app` 포함), `src-tauri` lib(`build_app`)와 bin(`main`, `gen-bindings`) 분리, `justfile`의 `check`·`check-full`·`lint-file` 자리 | 빈 저장소에서 `just check` 통과. 저장소 안의 `rustc --version`이 1.99.1 (harness 1.3, I-17). `cargo metadata`의 작업 공간 crate 목록이 architecture 3.1과 같음. 빈 Tauri 앱이 루트 `target/`으로 빌드되고 실측 S-12 결과를 ADR에 기록 | - |
| H-21 | TS 뼈대: pnpm 작업 공간, Node 고정, TS 7.0.2와 린트용 TS 6 API 별칭 설치, `apps/desktop/src`의 architecture 5장 폴더 | `just check`가 `tsc` 검사를 포함해 통과. `tsc` 7.0.2와 TS 6 API가 함께 설치됨 (I-16, harness 1.3). 고정과 다른 Node로 돌리면 `just check` 실패 | H-01 |
| H-02 | Rust 린트: `[workspace.lints]`, `clippy.toml`, `rustfmt.toml`, `just check`의 환경 고정 | harness 3.1의 `forbid` 린트마다 위반 예제 실패. 그 린트에 `#[allow]`·`#[expect]`를 붙여도 실패 (3.2 ①). `RUSTFLAGS`·`CLIPPY_CONF_DIR`를 설정한 채 `just check`를 돌려도 같은 위반이 실패 (1.1) | H-01 |
| H-03 | Rust 서드파티 허용 목록 (`deny.toml`) | 목록에 없는 crate(전이 포함)를 추가하면 실패. 목록에 없는 서드파티 빌드 스크립트 실패. 라이선스·취약점 검사 동작 (harness 2장, 3.5) | H-01 |
| H-04 | `xtask arch-check` 그래프 검사: 허용 간선(일반·빌드), 개발 의존 표, 전이 금지, 외부 crate 위치, 제품 bin 그래프 | architecture 3.2 표마다 위반 예제 실패: `src-tauri → order-execution` 직접 의존, `mcp-server`에서 `broker-toss-trade`로 가는 전이, `testkit → order-execution`, 제품 bin 일반 그래프의 `trade-fakes`·`testkit`·자동화 플러그인, `domain`의 허용 밖 외부 crate (architecture 4.6) | H-01 |
| H-22 | `xtask` 소스 위치 검사: 재수출 금지, `impl UserPresence` 위치, 토스 쓰기 메서드·주문 경로 조합과 `tossinvest.com` 위치 | `approval-command`·`broker-toss-trade`·`trade-fakes`의 `pub use` 재수출 예제 실패. `platform-macos`·`testkit` 밖의 `impl UserPresence` 실패. `UserPresence`를 `use … as`로 다른 이름으로 가져온 뒤 구현한 예제와 다시 내보낸 예제 실패. `broker-toss-trade` 밖의 쓰기 메서드와 주문·조건주문 경로 조합 실패(토큰 발급은 통과, `broker-toss`의 주문 조회 GET은 통과). `main.rs`·`toss-smoke` 밖의 `tossinvest.com` 실패 (architecture 3.2, harness 3.5, 6.2) | H-04 |
| H-05 | `xtask` 토큰 검사 1: 주석(문서 주석, `#[doc]` 포함), 금지 속성, 금지 매크로, `domain`의 `f32`/`f64` | harness 3.2 표의 속성·매크로마다 위반 예제 실패. 매크로 호출 안의 속성도 잡음. 문자열 안의 `//`는 오탐하지 않음. 생성 경로 목록의 파일만 제외 (harness 2장) | H-01 |
| H-23 | `xtask` 매니페스트·파일 검사: `[lints]` 상속, `[target]`·`[profile]`·`[patch]`·`[replace]`·`[features]`, `cfg(test)` 밖의 `cfg`와 `cfg_attr`, 작업 공간 `build.rs`·proc-macro, `crates/` 아래 설정 파일, 테스트 건너뛰기 | crate의 `[lints]` 절을 지운 예제 실패 (R-09 Q2). 표마다 위반 예제 실패. `apps/desktop/src-tauri/build.rs`만 통과. `#[ignore = "…"]`, `cfg(test)` 분기 예제 실패 (harness 2장, 3.2) 저장소 `.cargo/config.toml`의 `[env] CLIPPY_CONF_DIR`·`CLIPPY_ARGS`, `[alias] clippy` 예제 실패 (ADR 0005 알려진 빈틈). | H-05 |
| H-06 | `tsconfig` 엄격 설정과 형식 도구 ADR | harness 4.1의 옵션이 모두 켜짐. `harness/tsc-violations`의 기대 오류 코드가 TS 7과 TS 6에서 같음. 형식 도구(Prettier 또는 Biome 형식 기능) ADR. 실측 S-18 결과 기록 | H-21 |
| H-07 | ESLint 1: 타입, 거짓 타입, 실패 처리, 불변 규칙 | harness 4.2의 해당 행 규칙마다 위반 예제 실패. `Array.prototype.push.call`과 `Reflect` 우회 예제 실패. `as const`, `satisfies` 예제는 문제 0개 | H-21 |
| H-24 | ESLint 2: 문법, `switch`, React 효과 훅, 주석 지시문, 숫자, 파일 종류, 네트워크, 테스트 건너뛰기 | harness 4.2의 해당 행 규칙마다 위반 예제 실패. `useEffect` 별칭·다시 내보내기 우회 실패. 허용 형태가 아닌 인라인 설정 주석은 실패 (harness 2.1). `.js` 파일이 `src`에 있으면 실패 | H-07 |
| H-38 | ESLint 로컬 규칙 `local/no-raw-types`와 브랜드 기반 (`shared/lib/brand.ts`) | 타입 자리의 `string`·`number`·`bigint`·`boolean`, `Array<string>`, `Record<string, T>`, 추론으로 원시 타입이 새는 내보낸 함수와 props 예제가 실패. 리터럴 유니온은 통과. 화면 코드의 문자열 리터럴 문구와 JSX 글자가 실패. `messages.ts`가 문구 키마다 언어별 문장을 가짐(한국어만 채움). 브랜드 생성 함수가 허용 목록 파일 밖에서 만들어지면 실패. `messages.ts` 밖에서 `UiText`를 만들면 실패. `RuleTester` 테스트 (harness 4.2) | H-07 |
| H-39 | `xtask raw-types` | `domain`·`application`·포트 crate의 시그니처·필드에 `String`, `&str`, 정수, 부동소수점, `Decimal`, `Option<String>`, `Vec<u64>`, `bool`(매개변수·반환·필드), 타입 별칭 예제가 실패. 표준 트레이트 구현의 `bool`은 통과. newtype 정의와 생성자·접근자는 통과. 예외 crate 목록이 보호 경로에 있음 (harness 3.4) | H-04 |
| H-40 | 허용하는 검사 끄기와 `xtask disable-report` | TS: `eslint-disable-next-line <규칙> -- <이유>`만 통과. 파일·블록 단위, 이유 없음, 규칙 여러 개, 끌 수 없는 규칙을 끄는 예제가 실패. Rust: 항목 단위 `#[expect(린트, reason)]`만 통과. `#[allow]`, `#![expect]`, 이유 없는 `expect`, `forbid` 린트를 끄려는 예제가 실패. `disable-report`가 모든 끄기를 파일·규칙·이유와 함께 출력 (harness 2.1) Rust에서 끌 수 있는 것은 `pedantic`·`nursery`·`cargo` 그룹 소속 린트뿐: 개별 `deny` 린트와 `disallowed_*`에 붙은 `#[expect]` 예제가 실패하고, 허용 목록의 `main.rs` `generate_context!` 문장만 통과 (harness 2.1, ADR 0005). `#[expect]`의 린트 자리에 그룹 이름(`warnings`, `clippy::pedantic`, `clippy::all` 등)을 쓴 예제 실패. `mod`·`impl`에 붙여 여러 항목을 한꺼번에 끄는 `#[expect]` 예제 실패(함수, 문장, 필드, 단일 항목에만 허용). | H-03, H-24 |
| H-08 | ESLint 로컬 규칙(`local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`)과 파일 허용 목록 | 로컬 규칙마다 `RuleTester` 테스트. 허용 목록 밖 파일을 `shared/platform`으로 옮기면 실패 (폴더 예외 우회 예제). `throw` 허용 목록이 비어 있음 (harness 4.2) `.catch`와 두 인자 `.then`이 허용 목록 파일(`shared/ipc` 호출 어댑터, `shared/platform/detach`) 밖에서 실패. `/// <reference … />` 지시문 실패. 대문자 확장자(`x.JS`, `x.TSX`) 파일 실패 (ADR 0009). | H-24 |
| H-09 | TS 폴더 경계(`eslint-plugin-boundaries`), 미사용 검사(`knip`), 순환 검사(`dependency-cruiser`) | architecture 5장 폴더 의존 위반, `index.ts` 밖 가져오기, `shared/ipc` 밖의 `@tauri-apps/api` 가져오기가 실패. 순환은 `just check-full`에서 실패. 실측 S-19 결과 기록 `src` 밖 파일을 가져오는 예제가 실패. `zod` 가져오기는 `shared/ipc`와 `shared/lib`의 브랜드 파일에서만 (ADR 0009). | H-07 |
| H-10 | TS 의존성 허용 목록 검사 | 목록에 없는 패키지를 추가하면 실패. 목록 파일이 보호 경로에 있음 (harness 2장) `packageManager` 필드에 무결성 해시가 있음 (ADR 0006). | H-21 |
| H-11 | 테스트 실행기와 커버리지: nextest, cargo-llvm-cov 파일별 기준, 보호된 커버리지 예외 목록, Vitest | 기준 미달 예제 실패: `domain` 95%, `order-execution`·주문 게이트·`domain` 신호·주문 계산 모듈 100% (harness 6.1). 예외 목록 밖의 실행되지 않은 줄이 있으면 실패 (6.3). Rust 테스트가 nextest로만 돎 Vitest: 건너뛴 테스트와 `todo`가 0건인지 실행 결과로 확인하고 `allowOnly: false` (ADR 0009). | H-02 |
| H-29 | `xtask test-diff`와 테스트 수 비교 | 기존 Rust `#[test]`·TS 테스트 블록의 삭제와 본문 변경 예제 실패. 새 테스트 추가는 통과. `nextest list`·`vitest list` 개수가 줄면 실패 (harness 7.7) TS 테스트 파일(`*.test.ts(x)`)의 삭제와 본문 변경도 실패 (ADR 0014). 하네스 테스트 스크립트(`*.test.mjs`)의 삭제와 변경도 실패(지금 `test-diff`는 Rust 테스트만 봄) (ADR 0017). | H-11 |
| H-30 | `xtask lint-file`과 `just lint-file` | 바뀐 파일 하나의 형식과 린트를 검사하고 위반이면 0이 아닌 종료와 메시지. Rust와 TS 파일 모두. 훅이 `target/` 산출물을 직접 부르지 않음 (harness 1.1, 7.2) | H-05, H-07 |
| H-31 | SQL 검사 | `store-sqlite` SQL 상수의 금액 열에 `sum`, `avg`, `ORDER BY`, 산술을 쓴 위반 예제 실패 (harness 3.5, ACC-2) | H-05 |
| H-32 | `xtask migration-check` | git에서 이미 적용된 마이그레이션 파일을 고친 예제 실패. 번호 공백 실패 (architecture 6.3) | H-01 |
| H-37 | `xtask` 시각·환경 변수 검사 | `SystemTime::now`, `Instant::now`, `env::var`, `env::var_os`와 서드파티 crate의 현재 시각 함수 위반 예제가 보호된 파일 허용 목록(시계 어댑터 하나, 조립 지점) 밖에서 실패하고 목록 안에서는 통과. 별칭 가져오기로 부른 예제도 실패 (harness 3.1, architecture 3.2) | H-05 |
| H-33 | `xtask capability-check` | 픽스처에서 생성된 명령 목록과 capability 명령 허용 항목이 어긋나면 실패. `build.rs`가 명령 목록을 손으로 적은 예제 실패. architecture 3.6의 금지 권한이 있으면 실패. `tauri.conf.json`에 capability 명시가 없으면 실패 (SEC-10) 빈 `capabilities` 목록은 명시로 보지 않음: `capabilities: []`이거나 키가 없으면 실패 (ADR 0004). | H-01 |
| H-12 | 위반 예제를 하나의 하네스 자체 테스트로 묶기: `harness/lint-cases.json`, `KNOWN_GAP`, 유효 설정 검사, 실제 코드 무결성 | 규칙 하나를 끄면 자체 테스트가 실패. 덮어쓰기 블록으로 규칙을 몰래 끄는 예제 실패(`calculateConfigForFile`). `eslint --max-warnings 0 src`와 `tsc`가 깨끗함 (harness 6.8) | H-02 ~ H-11, H-22 ~ H-24, H-29 ~ H-33, H-37 ~ H-40 |
| H-13 | Rust→TS 타입 생성과 차이 검사 | `gen-bindings`로 `bindings.ts`, `MockCommandHandlers`, 명령 목록 파일, capability 명령 허용 항목을 생성. `build.rs`가 명령 목록 파일을 읽음. 명령을 하나 추가해도 보호 파일 변경이 0건. Rust 타입을 바꾸고 다시 생성하지 않으면 `just check`의 `git diff --exit-code`로 실패 (harness 5장, architecture 3.6) `invoke_handler`에 등록된 명령 = 생성된 명령 목록(`ipc/commands.txt`) = 권한 묶음(`permissions/commands.toml`)을 대조하고 어긋나면 실패. `build.rs`는 `harness/`의 기대 본문과 정확히 같아야 함 (ADR 0011). | H-01, H-21 |
| H-14 | git 훅 (lefthook) | 커밋 훅이 `just check`, 푸시 훅이 `just check-full`을 돈다. 위반 커밋 거부. 훅이 설치되지 않은 상태를 `assert_lefthook_installed`가 잡음 (harness 1.2) 커밋 훅은 `just base=HEAD check`, 푸시 훅은 푸시 전 원격 SHA를 기준으로 돎. 훅의 검사는 훅이 소유한 작업 사본 밖 target 폴더에서 돌거나 먼저 작업 공간 crate를 정리함: 소스를 고친 뒤 mtime을 되돌린 예제가 실패 (ADR 0013). | H-12 |
| H-15 | Claude Code 편집·셸 훅과 권한 규칙: 권한 `deny`, `PreToolUse`(Edit\|Write\|NotebookEdit, Bash), `PostToolUse`(Edit\|Write), `ConfigChange`, `harness/agent-writable.txt` | 역할별 허용 목록 밖 편집 거부(메인은 `docs/**`만, 리뷰어는 없음). 심볼릭 링크와 작업 사본 밖 경로 거부. harness 7.3의 금지 셸 명령마다 거부. 린트 실패 시 `decision: "block"`. `agent-writable.txt`에 `Cargo.lock`, `pnpm-lock.yaml`, IPC 생성물이 있고 `docs/open-issues.md`와 기존 `docs/adr/*`는 없음(새 ADR 추가는 허용). 실측 S-17 결과 기록 (harness 7.1 ~ 7.3) 셸 훅이 `git update-index --assume-unchanged/--skip-worktree/--cacheinfo`, `.git/info/` 쓰기, `LEFTHOOK*` 환경 변수, `CARGO_HOME` 설정, `--no-verify`, `core.hooksPath` 덮어쓰기를 거부. 편집 훅이 하위 `.gitignore` 생성을 거부. 서브에이전트의 `git merge`·`cherry-pick`·`revert`·`rebase` 통제. 편집 훅은 `just lint-file`의 종료 코드로 판정 (ADR 0015). | H-30 |
| H-25 | 서브에이전트 훅과 통제 설정: `PreToolUse`(Agent), `PostToolUse`(Agent), `SubagentStart`, `SubagentStop` 2종, 환경 변수, `SendMessage`·`Agent(fork)` 거부 | 허용 밖 종류와 `model`·`isolation`·`run_in_background` 인자 거부. implementer가 3번 막힌 뒤 "막힘:"으로 종료 허용. 리뷰어가 원본에 쓰면 종료 막힘. 상태 파일이 없으면 `unchecked`. `.claude/settings.json`의 하네스 훅이 9종 (harness 7.3 ~ 7.5) `SubagentStart`가 브리프의 기준 커밋 SHA를 작업 사본 밖 상태 파일에 기록하고 `SubagentStop`이 그 SHA로 `just check`를 돎. SHA가 HEAD의 조상이 아니면 실패. 새 기준은 시작 시점의 기준 브랜치 끝이거나 상태가 `passed`인 앞 단계의 대상 SHA여야 함 (ADR 0013). | H-15 |
| H-35 | 엄격 샌드박스와 실측 S-03 | `sandbox.enabled`, `failIfUnavailable: true`, `allowUnsandboxedCommands: false`, `denyWrite`. `python3 -c "open(...)"`로 보호 경로 쓰기 실패. 샌드박스 안에서 `cargo`, `pnpm`, `tauri` 빌드와 테스트가 통과하거나 필요한 `allowWrite`가 기록됨. `excludedCommands`가 생기면 ADR (harness 7.2) `CARGO_HOME`·상위 폴더의 cargo 설정 파일과 `PATH` 앞의 가짜 `cargo`로 검사를 끄는 예제가 샌드박스에서 막힘 (ADR 0005). `sandbox-exec`는 바깥 프로필과 글자까지 같을 때만 중첩됨: 샌드박스 안에서 `just check`의 nextest가 통과하거나, 테스트 네트워크 차단을 샌드박스 네트워크 정책으로 대체하는 ADR (ADR 0013). `node_modules/`·`target/`의 서드파티 산출물과 `PATH` 앞 폴더 쓰기 차단 (ADR 0015). 샌드박스 `denyWrite`가 `harness/**`, `.claude/{settings*.json,hooks,agents}`, `~/.gitconfig`, `~/.config/git`, `~/.claude`, `~/.rustup/settings.toml`을 포함. 하드 링크, 인터프리터 쓰기, 스크립트 파일 실행, 다시 연결한 작업 사본 경로를 통한 보호 경로 쓰기가 실패하는 실측 예제 (ADR 0017). | H-15 |
| H-36 | Claude Code 업그레이드 때 도는 하네스 회귀 테스트 | R-10 "실행 검증 요약"의 실험 12개가 스크립트 하나로 돈다. 하나라도 기대와 다르면 실패하고 CLI 버전을 기록. 사용자가 업그레이드 뒤 돌리는 방법이 workflow.md 4장에 적힘 (harness 1.3, 6.8) | H-25, H-35 |
| H-16 | CI 작업 흐름: Linux 푸시, `main` 푸시와 밤 macOS, 밤 Linux | Linux 푸시마다 플랫폼 중립 crate 묶음(보호된 제외 목록에 없는 crate)의 검사·테스트와 컨테이너 시각 회귀가 돈다. 제외 목록 밖 crate가 Linux에서 빌드되지 않으면 실패. macOS CI가 `main` 푸시와 밤에 전체 `just check`를 돈다. 밤 Linux에 전체 변이 테스트와 AI 고정 자료의 CLI 버전 점검. Linux 테스트 네트워크 차단(`--network none` 또는 `unshare -n`)과 실측 S-15 결과 기록 (harness 1.2, 6.2). 도메인 작업과 나란히 진행 밤 작업에서 `just mutants-all`. 깨끗한 빌드. 기준은 푸시 전 `main` SHA (ADR 0013). 깨끗한 checkout에서 `lefthook install` 뒤 `just base=<원격 SHA> check`. 병합 커밋과 fast-forward로 들어온 범위도 검사 (ADR 0015). | H-14 |
| H-26 | 보호 경로 서명 검사 CI (푸시 뒤 사후 탐지) | 푸시마다 기준 커밋(푸시 이전 `main`)의 `allowed_signers`·`agent-writable.txt`·검증 스크립트로 검사하고 실패하면 알림. 서명 없는 보호 경로 변경과 기존 테스트 변경 실패. 같은 푸시에서 서명자 목록을 바꿔도 실패. `Cargo.toml`·`package.json`의 의존성 절 밖 변경 실패 (harness 7.6, 7.7). 도메인 작업과 나란히 진행 기준 커밋의 허용 목록으로 경로를 판정. pre-commit이 돌지 않은 커밋도 서명 검사로 잡음. `package.json`·`Cargo.toml`은 의존성 절만 (ADR 0017). | H-16, H-28, H-29 |
| H-27 | 리뷰 기록 검사 CI | tasks.md에서 `완료`인 작업마다 `docs/reviews/<작업 ID>.md`가 없으면 실패 (workflow 3장) | H-16, H-19 |
| H-17 | 변이 테스트 구성 | `--cap-lints=true`, `--test-tool nextest`. 대상은 `domain`(신호·주문 계산 포함), `order-execution`, `application`의 주문 게이트 모듈. 일부러 약한 테스트를 넣으면 생존 변이로 실패. `#[mutants::skip]` 실패. 생존 허용 목록과 `.cargo/mutants.toml`이 보호 경로 (harness 6.4) 새 사본에 `apps/desktop/dist`가 없어도 변이 테스트가 돎 (ADR 0006). `source-check`가 `.cargo/mutants.toml`만 추가로 허용하고 그 밖의 `.cargo` 항목은 계속 실패 (ADR 0008). | H-11 |
| H-18 | 테스트 네트워크 차단 OS 겹 (macOS) | `.cargo/config.toml` runner의 `sandbox-exec`. 외부 연결이 실패해야 통과하는 테스트와 루프백 가짜 서버 테스트가 nextest로 돎. 실측 S-14 결과 기록. runner가 안 되면 nextest wrapper로 바꾸는 ADR. Linux 쪽은 H-16 (harness 6.2) `source-check`가 루트 `.cargo/config.toml`만 허용하고 `target.<triple>.runner` 밖의 키(`[env]`, `[alias]`, `build.*`, `rustflags`, `linker`, `[unstable]`, `include`)가 있으면 실패 (ADR 0008). | H-11 |
| H-19 | ADR·브리프·리뷰 기록 양식 | `docs/adr/0000-template.md`, 브리프 양식(workflow 2.1), 리뷰 기록 양식(workflow 3장) | - |
| H-28 | 사람 승인용 서명 키 (담당: 사용자). 다른 작업을 막지 않는다 | Secretive SSH 키(Touch ID), 저장소 `gpg.format=ssh`·`commit.gpgsign=false`, `harness/allowed_signers`에 공개키 (harness 7.6) | - |
| H-20 | 로컬 잠금 (사용자 승인) | `agent-writable.txt` 기준으로 권한 `deny`·편집 훅·샌드박스가 켜짐. 에이전트의 보호 경로 편집과 셸 쓰기가 거부됨. 에이전트의 `Cargo.lock`·`pnpm-lock.yaml`·IPC 생성물 쓰기는 통과. GitHub 규칙 세트와 CI에 기대지 않음 (harness 7.2, 7.6) `lefthook.yml`, 모든 폴더의 `.gitignore`, `.git/hooks`의 삭제·변경 거부 (ADR 0015). 잠금을 켤 때(`harness/claude-hooks/lock.txt`) 훅 파일 해시를 사용자 서명 커밋의 값과 비교 (ADR 0017). | H-01 ~ H-15, H-17 ~ H-19, H-21 ~ H-25, H-29 ~ H-40 |

## DS. 디자인 시스템

H-20 뒤에 `shared/ds/index.ts`, 토큰 생성물, 허용 목록, 기준 화면이 보호 파일이 된다. 그래서 공개 컴포넌트를 추가하는 작업은 모두 사용자 승인이다 ([harness.md](harness.md) 4.4 "공개 목록 잠금").

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| DS-00 | Claude Design으로 화면 디자인 (담당: 사용자) | 산출물이 `design/`에 있음. 주 화면 하나(현황, 카드, 채팅, 종목 목록, 상태 띠)와 보조 화면, 설정 기본·고급이 있음 (design 2.4, 8장). 레이아웃 토큰(architecture 5.1의 추가 요청 목록)이 있음 (open-issues U-05). 화면 작업에 필요한 컴포넌트가 모두 있음: 입력, 대화 상자, 차트, 신호·주문 제안 카드, 마크다운 표시, 채팅 입력, 성적표의 신뢰도 도표(산점도) 등. 목록은 DS-03 ~ DS-06, DS-17, DS-18과 대조한다. "컴포넌트가 비중·손익 계산", "주문 버튼 없음"을 고침 (architecture 5.2) | - |
| DS-01 | 토큰 추출: 색, 글자, 간격(4px 기준), 반경, 그림자, 레이아웃, 겹침 순서 | `tokens.json`에 라이트·다크 값이 있음. architecture 5.1의 간격·구간·크기 토큰 이름이 모두 있음 | DS-00 |
| DS-13 | `xtask gen-tokens`, `xtask tokens-contrast` (사용자 승인) | `tokens.json`에서 `tokens.css`·`tokens.ts`를 생성하고 Tailwind `@theme`에 `--*: initial`. 재생성 차이가 있으면 실패. 대비 조합 표를 어기면 실패 (harness 4.4 R-14 #16, #17) | DS-01, H-20 |
| DS-02 | 디자인 시스템 린트 1: `shared/ds` 내부 (사용자 승인) | harness 4.4 내부 스타일 규칙(R-14 #1~8, 15)마다 위반 예제 실패. `no-unknown-classes`의 `entryPoint`를 틀리게 둔 예제가 실패로 드러남. Tailwind "생성되는 클래스 목록" 스냅샷 | DS-13 |
| DS-09 | 디자인 시스템 린트 2: 화면 코드 금지 (사용자 승인) | harness 4.4 화면 코드 금지 1~8마다 위반 예제 실패. 같은 코드가 `shared/ds`에서는 통과 화면 코드가 DOM에 닿는 길(`event.currentTarget.ownerDocument`, DOM `ref`의 값으로 계산)이 없음. JSX 속성 문자열 문구(`aria-label`, `title`)는 `UiText`만 (ADR 0012). | H-20 |
| DS-10 | 디자인 시스템 린트 3: 공개 props (사용자 승인) | harness 4.4 #9~11 위반 예제 실패. 토큰 이름이 사라지면 쓰던 코드가 컴파일 오류 (#10) DS 컴포넌트의 `ref` prop 타입은 `RefObject`만(콜백 ref를 받지 않음, 타입 테스트) (ADR 0009). | DS-09, DS-13 |
| DS-03 | 기본 컴포넌트 1 (사용자 승인): 글자, 버튼, 입력, 선택, 토글, 펼침(고급 설정용), 아이콘. 디자인 산출물에 있는 것만 | 컴포넌트마다 상태별 테스트와 목록 화면 항목 | DS-10, DS-13 |
| DS-04 | 기본 컴포넌트 2 (사용자 승인): 카드, 표, 배지, 상태 띠, 대화 상자, 빈 상태, 뼈대 로딩 | 같음 | DS-10, DS-13 |
| DS-05 | 숫자 표시 컴포넌트 (사용자 승인): 금액, 등락률, 수량, 확률, 만·억 표기, 상승·하락 색 | 십진수 문자열을 부동소수점 없이 형식화, 빼기는 U+2212 (architecture 5.3). 방향은 코어가 보낸 값으로만. R-11·R-14 표기 예제와 일치 (UX-1). 반올림 뒤 0이 되는 값의 방향 규칙이 D-01의 ADR과 같음 표시 함수(`shared/lib/format.ts`)는 받은 자릿수를 그대로 쓰고 부호·쉼표·단위만 붙임. `Intl` 기본값의 반올림을 쓰지 않음. 0으로 보이는 음수의 부호 처리 (ADR 0012). | DS-10, DS-13, D-01 |
| DS-06 | 차트 컴포넌트 (사용자 승인): 선, 영역, 도넛, 트리맵 (Recharts) | 토큰 색만 사용, 빈 데이터 처리. 좌표는 코어가 보낸 십진수 문자열이고 숫자 변환은 차트 컴포넌트 파일(보호된 허용 목록) 안에서만 함. 테마 값은 속성으로 받음 (architecture 5장, harness 4.2) | DS-10, DS-13 |
| DS-18 | 마크다운 표시와 채팅 입력 (사용자 승인) | react-markdown 래퍼: 원시 HTML 끔, 이미지는 링크 글자, 링크는 콜백 속성으로만 열고 링크로 만들지 여부는 속성으로 받음(수집 목록 판정은 화면 코드와 코어). 채팅 입력: 한글 조합 중 Enter로 보내지 않음. 상태별 테스트와 목록 화면 항목 (SEC-10, architecture 5장) 자유 입력 문장용 브랜드(채팅 입력)를 `shared/lib/brand.ts`에 추가 (ADR 0012). | DS-10, DS-13 |
| DS-07 | 컴포넌트 목록 화면 (Storybook) | 모든 공개 컴포넌트가 목록에 있음. 이야기마다 라이트·다크 | DS-03 ~ DS-06, DS-18 |
| DS-15 | 시각 회귀 기준 환경과 실측 S-16 | 태그 고정 Playwright 컨테이너, 묶은 웹 글꼴, 애니메이션·캐럿 끔, 고정 시계·난수. 같은 컨테이너에서 두 번 찍은 결과가 바이트까지 같음. 호스트에서 만든 `-darwin` 기준 파일이 있으면 실패. 비교 엔진 ADR (harness 6.6) | DS-07 |
| DS-11 | 레이아웃 컴포넌트 (사용자 승인): `Page`, `Section`, `Grid`, `GridItem`, `Stack`, `Inline`, `Split`, `Card`, `Toolbar`, `ScrollArea`, `Divider` | 받는 값이 architecture 5.1 표와 같음. 격자 열 수는 3·4·6·8·9·12만 컴파일됨. 본문 폭 구간(컨테이너 쿼리)별 기준 화면. `ScrollArea`는 Base UI ScrollArea 래퍼이고 막대가 폭을 차지하지 않음(막대 유무로 내용 폭이 같음). 자식 래퍼의 `min-width: 0` 기준 예제가 넘침 검사를 통과 | DS-10, DS-15, DS-20 |
| DS-19 | 스크롤·넘침 정적 규칙 (사용자 승인) | harness 4.4 #18~25마다 위반 예제 실패: 허용 목록 밖의 `overflow-*`·`truncate`, `absolute`·`fixed`, 길이를 준 `h-*`·`w-*`, `h-screen`, `whitespace-nowrap`, 음수 여백, 화면 코드의 `ScrollArea` 가져오기. 허용 목록 파일에서는 통과 | DS-02, DS-09 |
| DS-20 | 넘침 검사 함수와 부하 자료 (사용자 승인) | `shared/ds/testing`의 넘침 검사가 harness 4.4의 일곱 확인을 함. 일부러 넘치게 만든 예제(고정 높이 안의 긴 글, `min-width` 없는 flex 자식, 조상에 잘린 팝오버, 겹친 세로 스크롤, 겹친 버튼)가 각각 실패. 부하 자료 파일이 보호 경로에 있음. 부하 이야기가 없는 컴포넌트는 `ds-check` 실패 | DS-15, DS-19 |
| DS-08 | `AppShell` (사용자 승인): 사이드바(종목 목록 자리), 본문, 채팅 패널, 창 폭별 동작, 창 최소 크기 960 × 640 | 사이드바에 화면 전환 메뉴가 없음 (design 2.4, P-10). 창 폭 세 구간의 기준 화면을 사용자가 디자인과 대조해 승인 (architecture 5.1) | DS-03, DS-04, DS-11 |
| DS-12 | `xtask ds-check`와 공개 목록 잠금 (사용자 승인) | 컴포넌트마다 목록 항목, 라이트·다크 기준 화면, 상태 테스트 중 하나라도 없으면 실패. 에이전트의 `shared/ds/index.ts` 편집이 거부됨 (harness 4.4 #12). 부하 이야기와 넘침 검사 통과도 확인 | DS-07, DS-15, DS-20 |
| DS-17 | 캔들 차트와 Canvas 테마 래퍼 (사용자 승인) | Lightweight Charts는 `shared/ds` 래퍼로만 쓰임. `shared/platform`의 허용 목록 파일이 테마를 읽어 넘기고 테마가 바뀌면 다시 그림 (architecture 5.2) | DS-06, U-05 |
| DS-16 | 실측 S-05 (담당: 사용자): 실제 WKWebView 창의 한글 입력과 숫자 형식 | 한글 조합 입력과 `Intl` 결과를 기록. 다르면 DS-05 수정 작업을 추가 | DS-05, U-04 |

## D. 도메인 (`crates/domain`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| D-01 | `Money`, `Currency`, 반올림 규칙, 시간 타입 crate ADR | 통화가 다른 `Money`의 더하기가 컴파일되지 않거나 오류. `checked_*` 넘침이 오류로 나옴(속성 테스트). 반올림은 표시용 함수에만 있음. 시간 타입 crate와 반올림 뒤 0인 등락의 방향 규칙 ADR (architecture 3.4, 5.3, 8장) | H-20 |
| D-02 | `Quantity`, `Price`, `Ratio` | 음수·0·소수점 수량 규칙 테스트. 생성자 밖에서 만들 수 없음 | D-01 |
| D-19 | `Probability` | 0~1 십진수. AI 입력은 1~99 정수 퍼센트만 파싱되고 0, 100, 소수, 범위 밖은 생성 실패 (design 6.1, ACC-2) | D-01 |
| D-03 | 식별자: `Symbol`(영숫자), `InstrumentId`, `Market`, `AccountId`, `ClientOrderId` | 잘못된 형식은 생성 실패. 영숫자 종목 코드 통과(6자리 숫자를 가정하지 않음). `AccountId`의 표시값은 뒤 4자리 (SEC-7) | H-20 |
| D-04 | `FxRate{kind, as_of, source}`와 통화 환산 | 환율 없이 환산 불가. 합산 기본은 `Mid`. 쓴 환율 종류와 시각이 결과에 남음 (ACC-3) | D-01 |
| D-05 | `Holding`, `Portfolio` 평가 | R-11 예제 값과 일치. 원화 환산 총액과 총자산에 근사 표시가 붙음 (ACC-6, I-05) | D-02 ~ D-04 |
| D-06 | `Allocation`, `SectorAssignment` | 종목·섹터·국가·통화 비중. 합이 정확히 100%가 되는 표시 반올림 규칙 테스트 | D-05 |
| D-07 | `Concentration` | R-11 예제 값과 일치. 유효 종목 수 5 미만 경고 (design 8장) | D-06 |
| D-08 | `Transaction`, `TransactionKind`(`Unknown` 변형), `SettledRowSet` | 같은 행의 개수까지 맞추는 멀티셋. 사라진 행은 지우지 않고 사라진 시각만 표시. 미결제→결제 전환과 국내 취소 표현 (I-06) | D-02, D-03 |
| D-28 | `Lot`, `CostBasisMethod`, `CorporateAction` | 분할, 병합, 무상증자, 주식배당, 코드 변경, 상장폐지 예제가 R-11과 일치 | D-08 |
| D-30 | `CashLedger`(신뢰 상태 포함), `ExternalFlow` | 거래내역으로 재구성한 현금이 대조용으로만 쓰임. 신뢰 상태가 낮으면 표시 값에 쓰이지 않음 (I-05) | D-08 |
| D-11 | `Quote`, `Candle`(수정주가), `Volatility` | 형식 검증. 변동성 계산이 R-11 예제와 일치 | D-02, D-03 |
| D-10 | `MarketSession`: 장 상태 판정 | 휴장일, 서머타임 경계 테스트. 시각은 인자로만 받음 | D-03 |
| D-31 | `ValuationSnapshot`과 빠진 날 재계산 | 일별 평가에 환율이 포함됨. 일봉과 거래내역으로 다시 계산한 값이 예제와 일치 (I-09) | D-05, D-08, D-11 |
| D-09 | `Performance`(TWR, MWR), `BenchmarkSeries` | R-11 예제 값과 일치. 1년 미만은 연율화하지 않음. 변동성 60거래일·베타 52주 미만은 "표본 부족" (ACC-8) | D-30, D-31 |
| D-20 | `InvestorProfile{Horizon, RiskAppetite, Exclusions, WeightCaps}`와 기본값 | 기본값이 몇 주~몇 달, 중립, 제외 없음, 종목 20%(10%부터 주의)·섹터 40% (design 6.2, P-4). 몇 시간 단위 기간은 기본으로 선택할 수 없음 | D-03 |
| D-12 | `Policy`, `PolicyViolation` 검사 | 원칙마다 위반·통과 테스트. 비중 상한 위반이 수량 축소 또는 신호 차단으로 나옴 (design 6.2) | D-06, D-20 |
| D-21 | 추정 과제 만들기 (`EstimateTask`)와 위험 성향별 (a, b) 초안 ADR | T = a·σ√H, S = b·σ√H가 투자 성향과 변동성만으로 정해짐(AI 입력 없음). 기간 제한을 넣은 기준 확률 p₀가 리뷰 기록 DESIGN-v0.2 2절의 확인값(T 10%/S 5%, σ 2%, 20일 → 24.75%, σ 1% → 3.15%, T 20%/S 10%, σ 2%, 120일 → 30.3%)과 소수 둘째 자리까지 일치. 초안 (a, b)의 p₀가 20~40% 안. 변동성 표본 60거래일 미만이면 과제 없음. 기준가와 시각이 고정됨 (open-issues 1.1, 1.2) | D-10, D-11, D-19, D-20 |
| D-22 | `TradeCost`, `Margin`, `Thresholds` | open-issues 1.2 예 표의 손익분기, 매수 임계값, 매도 임계값을 표의 자릿수까지 재현. 손익분기 = p₀ + c/(T+S), 매도 임계값 = p₀ − 여유분이고 5% 아래로 내려가지 않음(속성 테스트). `c/(T+S)`가 5%p를 넘으면 과제 없음. 판정 30건 미만이면 여유분 15%p를 강제하고 설정 값으로 바꿀 수 없음(속성 테스트, ORD-8) | D-21 |
| D-23 | `Calibration` 적중표와 보정 적용 틀 | 모델·기간 조합별 판정 수와 구간별 적중을 집계. 판정 수는 같은 종목에서 기간이 겹치지 않는 추정만 셈(겹치는 예제가 한 건). 30건 미만 조합은 보정하지 않고 보수 운용 표시. 모델이 바뀌면 새 모델의 건수로 다시 셈. 보정 함수는 E-91 전까지 원래 값을 그대로 돌려줌 (design 6.2) | D-19 |
| D-24 | `SignalDecision`: 신호 판정 | 보정한 p와 임계값으로 매수·매도·없음. 매도는 보유 종목에만. 보유하지 않은 종목의 낮은 p는 신호 없음(공매도 없음). 제외 대상과 출처 없는 추정은 신호 없음 (ACC-5, design 6.2) | D-22, D-23 |
| D-13 | `OrderIntent`와 검증 규칙 | 수량 지정이 기본. 금액 지정은 미국 주식 시장가에서만. 소수점 수량은 토스 스펙이 허용하는 조합만 (design 6.3) | D-02, D-03 |
| D-25 | 매수 수량 계산 | 한 번에 사는 비중(ADR 초안)으로 금액을 정하고 비중 상한, 해당 통화 매수가능금액, 30건 전 보수 운용으로 깎은 뒤 수량으로 바꿈. 미국 주식은 달러 매수가능금액만 봄. 수량이 0이면 주문 없음과 이유(예: 달러 없음 → 환전 안내)를 돌려줌. 같은 종목 매수 신호는 기간 H 안에 두 번째부터 없음 (design 6.3, ORD-8) | D-12, D-24 |
| D-26 | 매도 수량과 지정가 계산 | 매도 수량은 보유 수량과 매도가능수량 안. 지정가는 최신 호가와 호가 단위로 정함. 시장가는 설정이 켜졌을 때만 (design 6.3) | D-13, D-25 |
| D-14 | `Proposal<Draft>`, `Proposal<Validated>`, `ValidationReport`, 만료 | 잘못된 전이가 컴파일되지 않음. `Validated`는 `Clone`·직렬화가 없음. 만료 10분과 변동성 기준 가격 범위(σ√H 비례, 계수 초안 ADR) 이탈 테스트 (design 6.5, architecture 4.3) | D-13 |
| D-32 | `OrderSummary`와 1억 원 판정 | 요약 필드가 architecture 4.4 순서 5의 목록과 같음. 달러 주문은 `rate`와 `midRate` 중 큰 값으로 환산하고 1억 원의 80% 이상이면 고액 확인 필요로 판정 (design 6.5) | D-04, D-14 |
| D-27 | `EstimateOutcome` 판정 | 고가가 목표에 닿으면 성공, 저가가 손절에 닿으면 실패, 기간이 끝나면 실패. 기준가를 같은 수정 계수로 맞춤(기준 시각 뒤 액면분할 예제가 즉시 손절이 아님). 기준일과 목표·손절에 모두 닿은 날은 1분봉으로 판정하고 1분봉으로도 모르면 실패. 몇 시간 단위 기간은 1분봉만. 같은 봉 입력에 항상 같은 결과(속성 테스트) (design 6.6, ACC-9) | D-11, D-21 |
| D-15 | `NewsItem`, `Disclosure`(키 `(source, receipt_no)`), `Cluster`, `CollectedItem`(비신뢰) | URL 정규화 규칙 테스트. `CollectedItem`은 출처 없이 만들 수 없음 | D-03 |
| D-16 | `Thesis`: 반증 가능한 주장 목록과 변경 이력 | 이력 불변. 노트 전체 상태는 주장 판정을 코드가 집계. 출처가 웹뿐인 변경은 "확인 필요" (R-12 2.2, 3.3) | D-15 |
| D-17 | `Scorecard` 1: 구간별 적중률, Brier 점수, 신뢰도 도표 자료, 판정 보류, 비교 수 | R-12 방법론 예제와 일치. 30건 미만이면 "판정 보류". 모델별·기간별로 따로 집계 (design 6.6, AI-5) | D-23, D-27 |
| D-29 | `Scorecard` 2: 신호 수익률, 벤치마크 비교, 신뢰구간 | 수수료·세금을 반영한 신호 수익률. 가상 청산 규칙(목표·손절가, 그 가격을 넘겨 시작한 날은 시가, 만기는 종가)마다 예제와 일치. 같은 기간·같은 통화 벤치마크. 날짜 단위로 묶은 부트스트랩 95% 구간(난수 시드는 인자)이 0을 포함하면 "우위 확인 안 됨" (design 6.6) | D-09, D-17, D-22 |
| D-18 | `DomainEvent` | architecture 3.5의 예시를 닫힌 목록으로 확정해 문서에 반영하고 코드와 일치 | D-05, D-14, D-24, D-27 |

## A. 유스케이스 (`crates/application`, `crates/testkit`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| A-01 | 포트 정의: `BrokerRead`, `MarketInfo`, `BrokerCapabilities` | architecture 3.3과 일치. `BrokerTrade`는 여기 없음(`order-execution`이 정의) | D-05, D-10, D-11 |
| A-02 | 포트 정의: 저장소, `AuditLog`, `SecretStore`, `Clock`, `Notifier`, `UserPresence` | architecture 3.3과 일치. `UserPresence`의 요청은 주문 요약과 위험 설정 변경 두 종류. 비밀 값 타입에 `Debug`·`Display`·`Serialize`·`Clone`이 없음 (SEC-11) | D-18 |
| A-03 | 포트 정의: `NewsSource`(메타데이터 포함), `Fundamentals`, `MacroSeries`, `AiProvider`(`RunSpec`에 실행 종류) | architecture 3.3과 일치 | D-15 |
| A-04 | `testkit` 1: 포트 가짜, `SteppedClock`, `ScriptedPresence` (사용자 승인: 본인 확인 가짜는 보호 파일) | `testkit`의 의존 그래프에 `order-execution`이 없음(arch-check). `ScriptedPresence`가 대본대로 `Confirmed`·`Denied`·`Unavailable`을 돌려주고 호출 횟수를 기록 (I-14, architecture 3.2) | A-01 ~ A-03 |
| A-11 | `testkit` 2: 가짜 AI CLI 실행 파일 | 녹화한 stream-json 재생 모드와 대본 MCP 클라이언트 모드. 실행 파일 경로 주입으로만 바뀌고 `PATH`를 바꾸지 않음 (harness 6.1) | A-03 |
| A-05 | 오류 분류 (사용자 조치, 대기, 버그) | 모든 포트 오류가 셋 중 하나로 분류. 오류 값에 비밀과 요청·응답 본문이 없음 (architecture 3.8) | A-01 |
| A-06 | `SyncPortfolio` | 성공, 일부 실패, 접근 차단 경로 테스트 | A-04 |
| A-07 | `SyncTransactions` | 결제일 기준 최근 14일 창을 매번 다시 받아 같은 행의 개수까지 맞춤. 사라진 행은 지우지 않고 표시. 하루 1회 전체 기간 검사 (I-06) | A-04, D-08 |
| A-08 | `GetPortfolioView`, `GetAllocationView` | 표시용 값이 전부 코어 계산. 환산에 쓴 환율 종류와 시각 포함 (ACC-3). 현금 자리는 통화별 매수가능금액, 총자산은 근사 표시 (ACC-6, I-05) | A-06 |
| A-09 | `GetMarketOverview` | 오래된 데이터 표시 (ACC-4). 미국 지수·금리는 토스 ETF 근사인지 미 재무부·FRED 값인지 표시 (I-18) | A-04 |
| A-10 | 이벤트 버스 | 구독자 추가가 기존 코드 수정 없이 됨. 페이로드는 ID와 종류만 (architecture 3.5) | D-18 |
| A-12 | `SubmitEstimate` | 코드가 만들지 않은 `task_id`와 다른 실행에 배정된 과제는 거부. 한 실행이 받는 추정 수는 그 실행에 배정된 과제 수 이하이고, 같은 과제의 두 번째 제출은 거부(테스트). 출처 없는 추정에 "출처 없음". 제출된 추정은 거래 여부와 상관없이 전부 기록(누락 0, ACC-9). AI-6의 메타데이터 기록 | A-04, D-21 |
| A-13 | `EvaluateSignals` | 보정 → 임계값 비교 → `SignalRaised`·`SignalCleared`. 같은 종목의 새 추정이 오면 신호를 다시 계산하고 이전 추정은 판정 대상으로 남음 (design 6.1, 6.2) | A-12, D-24 |
| A-14 | `DraftProposal` | 장이 열려 있으면 `Proposal<Draft>`를 만듦. 닫혀 있으면 신호만 남김. 장이 열린 뒤 추정 기준가에서 변동성 기준 범위 안이면 만들고, 아니면 신호를 버리고 재추정 예약. 만료된 제안의 신호는 남고, 다시 만들면 최신가를 씀 (design 6.4, 6.5) | A-13, D-14, D-26 |
| A-15 | `ResolveEstimates` | 기간이 끝났거나 목표·손절에 닿은 추정을 전부 판정(거래하지 않은 것 포함). `EstimateResolved`. 보정표 갱신. 판정이 끝난 기록을 다시 바꾸지 않음 (ACC-9) | A-12, D-23, D-27 |
| A-16 | `TakeValuationSnapshot`과 빠진 날 재계산 | 일별 평가 기록. 앱이 꺼져 있던 날을 다음 시작 때 일봉과 거래내역으로 채움 (I-09) | A-06, D-31 |
| A-17 | 위험 설정 변경 유스케이스 | 주문 켜기, 시장가 허용, 몇 시간 단위 기간 허용, 비중 상한 완화, 위험 성향을 더 공격적으로 바꾸기는 `UserPresence`의 위험 설정 확인이 `Confirmed`일 때만 바뀜. 그 밖의 결과에서는 설정이 그대로. 주문용 본인 확인을 요청하지 않음 (SEC-12, P-9) | A-02, A-04 |
| A-19 | 자체 점검 판정 | design 2.6의 8개 점검을 순서대로 판정해 사용자가 할 일이 있는 문제 하나만 돌려줌. 모두 정상이면 없음. 할 일이 없는 상태(6, 7번)와 뉴스 소스 실패는 상태 띠 대상이 아니고 고급 > 상태 기록으로만 감 (P-7, UX-5, REL-5) | A-05 |

## T. 토스 어댑터 (`adapters/broker-toss*`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| T-00 | 토큰·계좌·보유 주식을 직접 호출해 연결 확인 (담당: 사용자). 하네스 작업과 함께 진행 | 허용 IP 등록 완료, 응답이 스펙과 같은 형태. 실측 S-01 중 허용 IP 개수와 토큰 유효 시간 기록 | - |
| T-01 | `broker-toss-schema`: OpenAPI `components.schemas`에서 `typify`로 생성 (사용자 승인: 생성 경로) | 스펙을 바꾸면 생성물 차이가 드러남. 이 crate만 주석·속성 검사에서 빠짐. 생성 타입이 `broker-toss`·`broker-toss-trade` 밖으로 나가지 않음 (harness 5장) | H-20 |
| T-18 | `TossEndpoint` 주입과 HTTP 클라이언트 | 실제 주소 생성자는 `main.rs`에서만 보임. 제품 클라이언트 `https_only`. 테스트 클라이언트는 루프백만 허용하는 resolver. `broker-toss`의 공개 API에 GET과 토큰 발급만 (harness 6.2, architecture 4.2) | T-01, A-01 |
| T-20 | 계약 테스트 틀 | 로컬 가짜 HTTP 서버에 녹화 응답을 올림. 녹화 자료가 토스 스펙과 어긋나면 실패 (harness 6.1) | T-18 |
| T-02 | 토큰 관리: 발급, 보관, 만료 전 갱신, 단일 토큰 | 동시 요청에서 발급이 한 번만. 갱신 순서가 API-3과 같음. `token-revoked` 재발급은 1회만 (API-3) | T-18, A-02 |
| T-03 | 속도 제한기 | 그룹별 한도의 60%. 응답 머리말의 한도 값을 따름. 429는 `Retry-After` 뒤 지수 백오프 (API-1, API-2) | T-18 |
| T-04 | 오류 매핑 | 스펙의 모든 오류 코드와 토큰 발급 OAuth 오류가 A-05 분류로 매핑. `requestId`와 `referenceId`를 둘 다 받음 (architecture 3.8) | A-05, T-01 |
| T-05 | 계좌 목록, 보유 주식 | 녹화 응답 계약 테스트 | T-02 ~ T-04, T-20 |
| T-06 | 거래내역: 커서 페이징 | 커서 오류 시 처음부터 재조회 | T-05 |
| T-07 | 현재가, 캔들(일봉, 1분봉) | 200개 제한과 과거 방향 페이징. `adjusted`(수정주가) 값을 명시해 요청 | T-05 |
| T-08 | 환율, 장 운영 정보, 국내 지수, 미국 지수 대용 ETF 시세 | 계약 테스트. 환율 `kind` 구분 (I-18) | T-05 |
| T-09 | 종목 마스터, 섹터 | 전체 종목 조회의 초당 1회 한도 준수 | T-05 |
| T-10 | 랭킹, 수급 동향 | 계약 테스트 | T-05 |
| T-11 | 매수가능금액, 매도가능수량, 수수료, 매수 유의사항 | 계약 테스트 | T-05 |
| T-15 | 주문 목록·상세 조회 (읽기, `broker-toss`의 GET) | 계약 테스트 | T-05 |
| T-12 | 연결 확인 명령 `crates/toss-smoke` (읽기 전용) | 앱이 켜져 있으면 실행 거부(토큰이 하나뿐). 키는 표준 입력으로만 받고 키체인·파일·로그에 저장하지 않음(인자와 환경 변수로 받는 경로 없음). 에이전트는 가짜 서버와 가짜 키로만 검증 | T-05 ~ T-08 |
| T-13 | 실제 키로 `toss-smoke` 실행, 응답 녹화 (담당: 사용자) | 실측 S-01 나머지(평균단가 방식, 거래내역 조회 기간, 1억 판정 환율), 실측 S-23(배당락 전후 수정주가, 1분봉 과거 조회 범위)과 ACC-6 대조 항목 기록. 녹화본은 저장소 밖 | T-00, T-12 |
| T-14 | 녹화 응답 비식별화 도구와 픽스처 반영 | 계좌번호와 실제 금액이 픽스처에 0건 (MNT-3) | T-13 |
| T-16 | `broker-toss-trade`: 주문 생성만 | `ApprovedOrder` 없이는 호출할 수 없음. `clientOrderId`와 `confirmHighValueOrder`를 실음. 정정·취소·조건주문 경로를 구현하지 않음. 쓰기 메서드와 주문 경로 조합이 이 crate에만 있음(H-22 검사) (design 6.5) 범용 HTTP 요청(메서드를 고를 수 있는 것)은 `broker-toss-trade`에만 있음. 읽기 쪽은 GET과 토큰 발급만 노출하는 HTTP crate를 따로 둘지 ADR로 정함. `sources`가 토스 자격 증명·토큰을 받지 못함을 조립 경로 테스트로 고정 (ADR 0007). | O-03, T-15 |
| T-17 | 웹소켓: 시세, 본인 주문 이벤트 | 재연결 후 주문 재조회, 연결 유지 신호, 계정당 2연결 규칙, 토큰 만료 처리 | T-02 |
| T-19 | 토스 스펙 변경 감지 | 정규화 해시, 공식 `versions.json`, 파일 `Last-Modified`를 비교. 바뀌면 고급 > 상태에 기록하고 생성 타입 차이를 확인하기 전까지 새 필드에 의존하지 않음 (API-7, I-08) | T-01 |

## S. 저장소와 비밀 (`adapters/store-sqlite`, `adapters/secrets-keychain`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| S-01 | DB 연결과 암호화 | SQLCipher 원시 키, `TEMP_STORE=2`, WAL 암호화, 전용 쓰기 스레드. 빌드 산출물이 `cipher_provider = commoncrypto` (SEC-6, architecture 6.1) | A-02 |
| S-20 | SQL 검사 구조 바꾸기 (사용자 승인: `xtask`, 새 의존 `sqlparser`) | SQL은 `store-sqlite`의 `src/sql/*.rs`에 있는 `pub const X: &str = "리터럴";`에만 있고 그 파일에서는 매크로를 못 씀. `rusqlite` 실행 메서드(`execute`, `execute_batch`, `prepare`, `prepare_cached`, `query_row`, `pragma_*`)의 SQL 인자는 그 상수 이름만. 상수는 SQL 파서로 문장 하나로 읽힘. DDL은 마이그레이션에만. 금액 열 집합은 마이그레이션 `CREATE TABLE`의 정규 십진수 CHECK 열에서 뽑고 CHECK로 못 가리는 열만 보호 표에 둠. 금액 열은 허용 위치(`SELECT` 목록의 맨 열, `INSERT` 값, `SET col = ?`, `WHERE col = ?`, CHECK)에만 나옴. 별칭·CTE·함수·연산자·인덱스 우회 예제가 실패. 마이그레이션 잠금 파일에 올리는 절차를 ADR로 정함 (ADR 0011) | H-31, S-15 |
| S-15 | 마이그레이션 실행기와 원장 | SHA-256 원장. 해시 불일치·누락·번호 공백이면 열기 실패. 스키마 스냅샷 테스트. 모든 테이블 STRICT, 금액 열 CHECK. SQL 상수마다 `prepare` 테스트 (architecture 6.3) | S-01, H-32 |
| S-02 | 보유 스냅샷 저장소 | 왕복 테스트. 장중 스냅샷 30일 뒤 일별 하나로 축소 | S-15 |
| S-03 | 거래내역 저장소 (원본 + 정규화) | 원본에서 정규화를 다시 만들 수 있음. 14일 멀티셋과 사라진 행 표시 반영 (I-06) | S-15, D-08 |
| S-04 | 시세·환율 캐시 | 왕복 테스트. 환율 `kind`, 출처, 시각 보존 | S-15 |
| S-05 | 뉴스·공시 저장소와 검색 | 앱 측 바이그램 FTS5. 2글자 질의와 조사 붙은 단어 검색 테스트 | S-15 |
| S-06 | 논거 노트 저장소 | 이력 보존 | S-15, D-16 |
| S-07 | 신호·제안 저장소 | `Proposal<Draft>`, `ValidationReport`, 소비 기록(제안 ID 고유 제약). `Validated`를 저장하거나 복원하는 함수가 없음 (architecture 4.3) | S-15, D-14 |
| S-18 | 추정 저장소: 과제, 추정, 판정, 보정표, 일별 평가 | 추가만 됨. 판정이 끝난 기록의 수정이 실패 (ACC-9) | S-15, D-27, D-31 |
| S-08 | 감사 로그 1: 추가 전용 | 수정·삭제 금지 트리거, 연속 순번 트리거, 연결 단위 authorizer, DEFENSIVE, 해시 체인. 수정·삭제 시도가 실패 | S-15 |
| S-09 | AI 작업·대화·MCP 호출 원장 | 왕복 테스트. MCP 호출 원장은 추가만 | S-15 |
| S-10 | 설정 저장소 (버전 있는 스키마) | 이전 버전 설정을 읽어 올림. 토스 스펙 해시 보관 | S-15 |
| S-13 | 서명 준비 (담당: 사용자) | Apple 개발자 멤버십 확인(open-issues U-02). 팀 ID와 프로비저닝 프로파일 준비. 멤버십이 없으면 그 사실을 기록 | - |
| S-14 | 서명 구성과 키 분실 ADR (사용자 승인: `tauri.conf.json`) | 서명 빌드의 키체인 접근 그룹 설정. ad-hoc 개발 빌드는 별도 서비스 이름과 가짜 키만 씀 (SEC-1). ADR: DB 키 분실 시 복구 경로, 머리값 고정 시점, 멤버십이 없을 때 로그인 키체인으로 낮추는 방식 (architecture 6.1). 실측 S-17용 최소 시험 빌드(키체인 왕복과 두 확인 창만)를 서명해 만들 수 있음 `gen-bindings` 바이너리를 앱 묶음에서 뺌(example 대상으로 옮기기 등) (ADR 0016). | S-13, H-20 |
| S-17 | 실측 S-02 (담당: 사용자). 키체인 구현 전에 한다 | S-14의 서명한 최소 시험 빌드에서 키체인 왕복, 네이티브 요약 창, macOS 본인 확인 창이 동작함. 이유 문자열 길이 한도 기록. 동작하지 않으면 S-12와 O-07을 시작하지 않고 ADR로 대안을 정함 | S-14 |
| S-12 | 키체인 `SecretStore` | 데이터 보호 키체인, 접근 그룹, `ThisDeviceOnly`. 개발 빌드에서는 실제 서비스 이름에 닿지 않음. 가짜 비밀 문자열이 로그·오류·SQLite에 0건 (SEC-1, SEC-11) | A-02, S-14, S-17 |
| S-16 | 감사 로그 2: HMAC과 머리값 | HMAC 키는 키체인. 마지막 순번과 해시를 키체인에 고정. 중간 수정, 삭제, 꼬리 잘라내기, 머리값 되돌리기를 각각 탐지 (ORD-3, harness 6.9) | S-08, S-12 |
| S-11 | 일일 백업과 7일 보관 | 쓰기 연결의 온라인 백업 → `cipher_integrity_check`, `quick_check`, 해시 체인 검증 → 원자적 이름 변경. 복구 테스트 (REL-6) | S-16 |

## U. 앱 상주, 첫 실행, 주 화면 (`apps/desktop`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| U-01 | 조립 루트: `build_app<R: Runtime>(builder, Ports, ctx)`, 수명 주기 | `main.rs`는 실제 어댑터 생성과 `build_app` 호출만 담음(RV-6). `src-tauri`가 `order-execution`에 직접 의존하지 않음(arch-check). 앱 시작 시 저장된 데이터로 주 화면 자료를 먼저 냄 (PERF-1) | A-04, S-01 |
| U-02 | IPC 계약: 명령·이벤트 틀, 오류 타입, 생성 | tauri-specta `ErrorHandlingMode::Result`. 입력 DTO `deny_unknown_fields`. TS에서 문자열 오류를 받지 않음. 큰 정수 타입은 생성이 거부 (architecture 3.6) 입력 DTO의 `deny_unknown_fields`가 specta derive와 함께 끌 수 없는 린트에 걸리는 문제를 정함(ADR 0016에서 넘어옴). | H-13, U-01 |
| U-03 | Tauri 권한 최소화 | 앱 ACL(`AppManifest::commands`), capability `main` 하나, 금지 권한 없음, CSP 명시, `freezePrototype`. `xtask capability-check` 통과 (SEC-10) CSP에 `connect-src`·`img-src`·`worker-src`를 명시하고 외부 주소가 없음. Tauri의 탐색 처리로 외부 주소 이동과 새 창 열기를 거부(테스트). `freezePrototype` 켬. capability 목록을 명시 (ADR 0004, 0009). | U-02, H-33 |
| U-18 | 메뉴 막대 상주와 단일 실행 | 창을 닫으면 숨기고 수집·예약 작업이 계속 돎. 트레이 아이콘, `Accessory`. 두 번째 실행은 기존 창을 띄우고 끝남 (REL-7, architecture 2.1) | U-01 |
| U-20 | 자식 프로세스 정리 | 실행 중인 프로세스 그룹 ID를 기록하고 앱 시작 때 남은 그룹을 정리. `ExitRequested`에서 SIGTERM 뒤 SIGKILL. 진행 중인 감사 로그 쓰기가 끝날 때까지 종료를 미룸. 실측 S-11 결과 기록 (architecture 2.3) | U-01, S-09 |
| U-04 | 프런트 뼈대: 라우터, 쿼리 계층, `shared/ipc` | IPC 가짜(`mockCommands`)로 화면 테스트 가능. 응답과 오류를 Zod `safeParse`로 검증. `queryFn`이 거절하지 않음 (architecture 5장) IPC 호출 어댑터(`shared/ipc/call.ts`)는 IPC 명령 호출에만 묶여 거절을 `Result`로 바꿈. 임의 Promise를 받는 범용 도우미를 내보내지 않음 (ADR 0012). IPC 호출에 시간 제한. 운영 knip 무시 목록에서 IPC 항목을 뺌 (ADR 0016). | U-02, DS-08 |
| U-05 | `shared/platform`: 코어 이벤트 구독 훅, `detach` | 이벤트가 오면 쿼리를 무효화하고 다시 읽음. 효과 훅이 허용 목록 파일에만 있음 `detach`는 정해진 종류의 작업만 받고 임의 Promise의 거절을 삼키는 범용 도우미가 아님 (ADR 0012). 이벤트 API(`emit`, `listen`)의 허용 파일을 `harness/capability-check/webview-apis.toml`에 추가(사용자 승인) (ADR 0016). | U-04 |
| U-23 | 설정 화면: 기본 3항목과 고급 틀 | 기본에는 토스 연결, 투자 성향, 주문 켜기만 보임. 나머지는 고급 아래 (UX-7, P-6). 선택 키가 없으면 "키를 넣으면 켜집니다" 한 줄 (P-5). 설정은 머리줄 설정 버튼, 메뉴 막대 아이콘, 창 메뉴(⌘,)에서 별도 macOS 창으로 열림. 주 창이 macOS 기본 제목 표시줄을 씀 (design 2.4, architecture 5.1) | U-04 |
| U-24 | 위험 설정 확인 창 (`platform-macos`, 네이티브 창만) | 네이티브 확인 창에서 확인해야 A-17이 통과. 이 경로에서 macOS 본인 확인 호출 0회 (SEC-12, P-9). 창 내용은 코어가 만든 문구 | A-17, U-01 |
| U-06 | 첫 실행 1: 토스 키 붙여넣기 | 키를 키체인에 저장하고 바로 토큰 발급 시도. 키가 WebView 상태와 로그에 남지 않음 (design 2.5, SEC-1) | U-04, S-12, T-05 |
| U-07 | 첫 실행 2: 허용 IP | `access_denied`·`ip-not-allowed`에서 현재 공인 IP와 복사 버튼, 등록 위치 한 문장. 현재 공인 IP는 이 화면이 떠 있을 때만 공개 IP 확인 서비스에 물어 얻음(그 밖에는 요청 0건). 서비스 선택 ADR, 실측 S-22(IPv4·IPv6) 결과 기록. 화면이 열려 있는 동안 1분에 1회 이하로 다시 시도하고 통과하면 다음으로 (REL-1, design 5.1) | U-06 |
| U-22 | 계좌 선택 | 종합매매 계좌 중 첫 계좌를 자동 선택. 계좌가 여럿이면 고급 > 시스템에서 바꿀 수 있음. 첫 실행에 계좌를 고르는 화면이 없음 (design 2.5, UX-4) | U-06, U-23 |
| U-08 | 동기화 스케줄 | 보유·매수가능금액 5분과 주문 직후 즉시, 거래내역 30분(14일 창)과 하루 1회 전체. 장중 여부는 토스 장 운영 정보로 (design 5.2) | A-06, A-07, J-01 |
| U-09 | 주 화면 현황 | 총평가(근사 표시), 통화별 평가, 보유 종목 표, 오늘 손익. 로딩·빈·오류·정상 네 상태 테스트 (design 2.4) | U-05, A-08, DS-05 |
| U-10 | 주 화면 자산배분과 쏠림 | 같음 | U-09, DS-06 |
| U-29 | 종목 목록 (사이드바) | 보유·관심 종목의 이름과 오늘 등락. 누르면 그 종목 상세가 본문에 열림. 화면 전환 메뉴 항목이 없음 (design 2.4, P-10) | U-09, J-09 |
| U-11 | 거래내역 보조 화면 | 사라진 행 표시. 네 상태 테스트 | U-05, A-07 |
| U-13 | 종목 상세 보조 화면: 시세, 차트, 내 보유 | 네 상태 테스트. 종목 목록과 채팅이 그대로 보임 | T-07, U-29, DS-17 |
| U-21 | 자체 점검과 상태 띠 | 문제가 있을 때만 한 문장이 보이고 여럿이면 앞 순서 하나만. 정상이면 0개 (UX-5). 문장에 내부 장치 이름 없음 | A-19, U-05, DS-04 |
| U-27 | 고급 > 상태 | 점검 기록과 자세한 사유, 사용자가 할 일이 없는 상태(AI 실행 환경, AI 사용 한도의 재개 시각), 소스별 마지막 성공 시각, AI 사용량 (REL-4, REL-5) | A-19, U-23 |
| U-28 | 첫 실행 종단 테스트 | 가짜 토스로 키 붙여넣기와 허용 IP 확인 두 조작만으로 주 화면에 닿음. 터미널 명령을 요구하는 화면 0개 (UX-4, P-1, P-3) | U-07, U-09, U-22 |
| U-14 | 알림 (`Notifier`) | 새 신호, 보유 종목 긴급 공시, 브리핑 완료가 macOS 알림으로. 종류별 켜기·끄기 (UX-3) | A-02, U-01 |
| U-19 | 로그인 시 자동 시작 | 고급 > 시스템의 스위치. 기본 켬. 처음 설치한 뒤 재부팅하면 앱이 메뉴 막대에 떠 있음 (design 8장, architecture 2.1, P-4) | U-18, U-23 |
| U-15 | 고급 > 시스템: 백업, 내보내기, 전체 삭제 | 전체 삭제 후 키체인에도 남지 않음 | S-11, S-12, U-23 |
| U-16 | 오프라인 동작 | 네트워크를 끊은 상태에서 저장된 데이터 조회와 대화 기록 열람 테스트 (REL-2) | U-09, C-03 |
| U-17 | 고지 문구 고정 표시 (WebView 쪽) | AI가 쓴 글이 나오는 곳, 신호 카드 머리, 채팅 첫머리의 데이터 전송 안내, 앱 정보 화면에 고정 문구 (UX-2, design 2.5) | C-03, E-06 |
| U-25 | UX-6 금지어 테스트 (사용자 승인: 금지어·예외 목록) | 화면 코드의 한국어 문자열과 코어가 보내는 화면 문구에서 금지어 0건. 예외는 고급 > 상태 화면 파일뿐 (UX-6, harness 6.9) | U-21, U-27 |
| U-30 | 실측 S-10 (담당: 사용자): App Nap | 창을 숨긴 채 8시간 이상 둔 뒤 예약 작업마다 예정 시각과 실제 실행 시각의 차이를 기록. 차이가 5분을 넘는 작업이 하나라도 있으면 대응 작업을 추가 | U-18, J-01 |
| U-31 | 수집·예약 작업이 UI를 멈추지 않음 (PERF-4) | 가짜 수집 작업과 가짜 예약 작업이 끝나지 않고 막혀 있는 동안에도 IPC 조회 명령이 응답함(테스트). 코어의 수집·예약 작업이 Tauri 메인 스레드에서 돌지 않음 | U-01, J-01 |

## M. MCP 서버 (`crates/mcp-server`, `crates/mcp-bridge`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| M-01 | 서버 뼈대, 실행별 소켓, 토큰 인증 | `rmcp` `=3.9.0`. 0700 폴더 안 0600 소켓, 경로 104바이트 이내. 토큰 상수 시간 비교와 실행 ID 묶기. uid·부모 PID·실행 파일 보조 확인. 열린 TCP 포트 0개 (SEC-3) | A-04 |
| M-07 | `mcp-bridge` 실행 파일 | stdio와 소켓 사이 바이트 전달, 첫 줄 토큰. 작업 공간 crate 의존 0개(arch-check). 앱 번들에 들어감 (I-01) | M-01 |
| M-02 | 도구 틀: 스키마 검증, 페이징, 오류, 호출 원장 | 응답 3만 자 이하와 `limit`·`cursor`. 입력 오류는 `isError` 구조화 오류. 읽기 도구 `readOnlyHint`. `tools/list` 정렬이 결정적. 모든 호출이 원장에 남음 (I-04, R-04 Q4) | M-01, S-09 |
| M-08 | `untrusted_text` 정규화 | 제어·폭 0·양방향 문자 제거, 링크 평문화, 길이 상한. 도구 설명·오류·필드 이름에 외부 텍스트 없음 (SEC-4) | M-02 |
| M-09 | 서버 계약 테스트 | 프로세스 안 duplex 테스트, 두 프로토콜 세대의 JSON 줄 스냅샷, `tools/list` 골든 파일 (harness 6.1) | M-02 |
| M-03 | 포트폴리오 도구: `get_portfolio`, `get_allocation`, `get_transactions`, `get_performance` | 계좌번호 뒤 4자리만 (SEC-7). 현금 자리는 매수가능금액. 표본 부족 표시 | M-02, A-08 |
| M-04 | 시장 도구: `get_quote`, `get_candles`, `get_market_overview`, `get_rankings`, `get_sectors`, `get_investor_flow` | 수집 시각 포함 (ACC-4). 미국 값의 근사·공식 표시 | M-02, A-09, T-07 ~ T-10 |
| M-05 | MCP 쪽 주문 0건 고정 | 레지스트리의 모든 도구를 무작위 인자로 불러도 가짜 `BrokerTrade` 호출 0. `tools/list`에 주문·승인·설정·키 도구 없음. 도구를 추가하면 자동으로 대상 (SEC-5, ORD-1) | M-03 |
| M-06 | `read_article` | 본문 읽기가 켜진 언론사 RSS 소스의 도메인에서 온 주소만 받음. 수집 실행이 웹에서 가져온 주소(출처 `web`), 수집 목록에 없는 주소, 본문 읽기가 꺼진 소스는 거부(경로 고정 테스트, harness 6.9 SEC-4). 본문은 한 번 전달하고 저장하지 않음 (design 3.1, 5.4) | M-08, N-09, N-17 |

## AI. AI 러너 (`adapters/ai-claude-code`, `adapters/ai-codex`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| AI-01 | 실행 이벤트 모델 `AiEvent` | architecture 3.3의 변형과 모르는 type을 담는 `Unknown`. 두 제공자의 녹화 출력을 같은 모델로 표현 | A-03 |
| AI-02 | Claude Code 어댑터: 실행, 스트림 파싱 | 가짜 CLI로 녹화 스트림 재생 테스트. 모르는 필드를 견딤 | AI-01, A-11 |
| AI-13 | Claude Code 찾기와 로그인 상태 | 알려진 설치 위치와 로그인 셸 PATH에서 찾음. 미설치·미로그인을 자체 점검 4·5번으로 보고 (design 2.6). 실측 S-08 결과 기록 | AI-02 |
| AI-03 | 실행 명령과 환경 | 환경 변수는 허용 목록만(`USER` 포함, `ANTHROPIC_API_KEY` 없음, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`). 0700 빈 작업 폴더. 파일 디스크립터 상속 없음. 실행 종류별 인자가 스냅샷과 같음: 분석은 `--tools ""`, 수집은 `--tools WebSearch`와 빈 MCP 설정 (SEC-2, SEC-13, design 4.1) | AI-02 |
| AI-11 | 사전 점검 | 첫 메시지 `/usage`로 init을 받아 design 4.1의 검사 항목을 확인. 하나라도 다르면 실제 요청 0건(가짜 CLI 대본으로 항목마다 테스트). 분석은 앱 서버 `connected`, 수집은 빈 목록 (SEC-2) | AI-03 |
| AI-17 | 실행 하나의 수명 | 새 프로세스 그룹, `kill_on_drop`, stdin 유지. 끝나면 토큰 폐기, 소켓·작업 폴더·CLI가 만든 기록 폴더 삭제 (architecture 2.3, I-04) | AI-03, M-01 |
| AI-18 | `sandbox-exec` 추가 겹 | 시작 때 적용 여부를 스스로 점검. 적용되지 않아도 안전 조건(SEC-2의 나머지)이 그대로 성립 (SEC-2) | AI-17 |
| AI-12 | 실측 S-04와 SEC-2 실제 확인 (담당: 사용자) | 사용자 훅·플러그인·스킬·프로젝트 CLAUDE.md가 깔린 환경에서 init의 `skills`·`slash_commands`가 빈 목록이고 사용자 훅 실행 0건. 사용자 메모리·전역 CLAUDE.md 차단, 샌드박스 안 구독 인증, SEC-13 항목 기록 | AI-11, AI-18 |
| AI-04 | 채팅 프로세스 유지와 대화 다시 넣기, 모델·추론 강도 | 채팅과 예약 작업 모두 `--no-session-persistence`. 채팅은 대화 동안 프로세스 하나를 유지하고, 프로세스가 끝나거나 앱을 다시 켜면 앱이 저장한 대화를 새 프로세스에 다시 넣음(가짜 CLI 테스트). `--resume`을 쓰지 않음. 실행이 끝난 뒤 CLI 기록 파일 0개. 실제로 답한 모델을 기록 (AI-6, design 4.1) | AI-02 |
| AI-05 | 한도 초과 감지와 재개 시각 | `rate_limit_event`와 `usage_limit_reached`. 한도 초과는 오류가 아닌 대기 상태이고 `resetsAt`에 재개 (REL-4) | AI-02 |
| AI-06 | 작업 큐 | 채팅이 시작되면 예약 작업이 단계 경계에서 SIGINT로 멈추고 나중에 이어 감 (AI-1). 예약 작업 한도 비율 기본 70% (AI-2). 동시에 채팅 1개와 예약 작업 1개, 실행마다 고유 세션 ID와 토큰 (AI-7) | AI-05 |
| AI-07 | 프롬프트 저장소 | `prompts/<작업>/<버전>.md` 앞머리 검증. 검증기는 앱의 프롬프트 로더(`application`)에 두고, `just check`의 테스트가 모든 프롬프트 파일에 돌린다. `xtask`나 보호 경로에 두지 않으므로 사용자 승인 작업이 아니다. 프롬프트 ID·버전·해시가 작업 기록에 남음. 본문에 제공자별 도구 이름이 있으면 실패 (design 4.1, AI-6) | H-20 |
| AI-08 | 사용량 기록과 조회 | 작업별 토큰과 소요 시간 (AI-3) | AI-06, S-09 |
| AI-14 | 수집 실행 실행기 | `--tools WebSearch`, 빈 MCP 설정, `--json-schema` 구조화 출력. 결과는 비신뢰 `CollectedItem`(출처 `web`). 프롬프트 입력에 수량·금액·계좌가 0건 (design 4.4, architecture 2.2) | AI-11, D-15 |
| AI-15 | 실측 S-07 (담당: 사용자): 수집 실행의 웹 읽기 | 격리를 유지하면서 웹 읽기를 살리는 옵션 조합의 시험 결과 기록. 메인 에이전트가 결과로 ADR 작성(조합 사용, 검색 결과만 사용, 앱이 대신 가져오는 도구 중 하나) | AI-14 |
| AI-16 | CLI 버전 재점검 | 버전이 바뀌면 사전 점검을 다시 하고 통과할 때만 AI를 씀. 실패하면 AI만 끄고 고급 > 상태에 사유를 남김(주 화면 상태 띠 0개, design 2.6의 6번). 확인한 버전 목록 기록 (REL-8) | AI-11, A-19 |
| AI-10 | 고급 > AI | 작업별 모델과 추론 강도, 예약 작업, 한도 비율, 사용량, Claude 설정 위치, 학습 설정 바로가기. 제공자 선택은 보이지 않음. 모델을 바꿔도 기록이 그대로 (AI-4, design 8장) | AI-06, U-23 |
| AI-09 | Codex 어댑터 (뒤로 미룸: G 뒤) | `codex app-server` 구동. 실측 S-09가 통과하기 전에는 켤 수 없음. Claude 어댑터와 같은 계약 테스트 통과 (design 4.1) | AI-01, AI-12 |

## C. 채팅

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| C-01 | `SendChatMessage`와 세션 관리 | 분석 실행으로 돎(웹 도구 없음). 대화 원본은 앱 SQLite. 가짜 제공자로 스트림 테스트 (design 4.2) | AI-06, M-03 |
| C-09 | 미리 띄운 채팅 실행 | 사전 점검을 끝낸 채 대기. 첫 글자 시간에 사전 점검과 MCP 연결 대기가 들어가지 않음 (PERF-2) | C-01, AI-11 |
| C-02 | 채팅 IPC: 요청별 `Channel<ChatChunk>` | `TextDelta`부터 `Failed`까지 변형. 창이 새로 고쳐지면 저장분을 읽고 다시 구독. 첫 글자까지 시간 측정 (PERF-2, architecture 3.5) | C-01, U-02 |
| C-03 | 채팅 패널 UI | 스트리밍, 도구 호출 중에는 무엇을 조회하는지 사용자 말로 표시(내부 이름 없음). 네 상태 테스트 | C-02, DS-08, DS-18 |
| C-04 | 화면 맥락 전달 | 보고 있는 종목·기간이 프롬프트에 포함됨 | C-03 |
| C-11 | `open_external` 명령 | `https`만 엶. 수집 목록에 있는 주소와 앱이 정한 고정 주소만 열고 나머지는 거부 (SEC-10) | U-02 |
| C-05 | 답변 안의 카드 렌더링 | 카드는 코어 이벤트 ID로만 그리고 마크다운에서 파싱하지 않음. 원시 HTML 끔, 이미지는 링크 글자. 수집 목록에 있는 주소만 `open_external` 링크가 되고, 보유 내역을 쿼리로 실은 주소 같은 나머지 주소는 글자로만 보임(테스트) (SEC-10) | C-03, C-11, DS-18 |
| C-06 | 숫자 대조 | 답변 속 숫자가 같은 실행의 도구 원장과 다르면 "검증 안 됨" (ACC-1) | C-05 |
| C-07 | 출처 표시 | 출처 종류 구분, 없으면 "출처 없음" (ACC-5) | C-05 |
| C-10 | 채팅의 웹 조사 | 사용자가 웹 검색을 시키면 `request_web_research`로 수집 실행 결과만 받아 옴. 사용자가 직접 쓴 문장으로 조사할 때는 저장된 사용자 메시지 원문이 그대로 수집 실행에 들어감(AI가 바꾼 문장이 들어가지 않음, 테스트). 화면에는 "찾아봤습니다"와 결과만 (design 4.2, P-8) | C-01, N-18 |

## N. 뉴스·공시·지표 수집과 웹 조사 (`adapters/sources`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| N-01 | 소스 레지스트리와 공통 요청 계층 | 소스당 동시 1건, 최소 간격 5초 (API-4). 조건부 GET은 지원 소스만. 소스별 요청 머리말 (API-6) | A-03 |
| N-02 | 언론사·거시 RSS: 한국경제, 매일경제, 연합뉴스, 이데일리, 한국은행, 연준 | 녹화 응답 테스트, 깨진 피드 처리 | N-01 |
| N-17 | 소스별 AI 본문 읽기 설정 | 기본 꺼짐. 연합뉴스는 켤 수 없음. 꺼진 소스는 제목과 요약만 AI에 들어감 (design 5.4) | N-02 |
| N-03 | DART 공시 목록 (키 선택) | 정정 공시 처리. 키가 없으면 이 보강만 빠지고 나머지는 동작 (P-5) | N-01 |
| N-04 | DART 종목 코드 대응표 | 대응 실패 종목 목록화 | N-03 |
| N-05 | KIND 오늘의 공시 RSS (주 소스) | 회사명→코드 대응, 앱 시작 때 하루치(500건) 따라잡기. 실측 S-20 결과 기록. 막히면 DART RSS와 DART 목록으로 바꾸는 경로 테스트 | N-01, T-09 |
| N-06 | Google 뉴스 검색 RSS (보유 종목 보조) | 제목과 링크만 표시, AI에는 제목만, 7일 뒤 삭제. 네이버 검색 API는 없음 (design 5.1) | N-01 |
| N-07 | 미국: Yahoo 헤드라인 RSS, SEC EDGAR | Yahoo는 브라우저형 User-Agent. SEC는 연락처가 없으면 요청 0건, 있으면 초당 10회 이하 (API-6) | N-01 |
| N-08 | DART 재무제표 (`Fundamentals`) | 녹화 응답 테스트. 키가 없으면 재무 기능만 빠짐 | N-04 |
| N-16 | 미국 지표 (`MacroSeries`): 미 재무부 일별 국채 금리 XML, FRED | 녹화 응답 테스트. FRED 키가 없으면 토스 ETF 근사만 보임 (I-18, P-5) | A-03, N-01 |
| N-09 | 중복 제거와 사건 묶음 | 정규화 URL이 같으면 같은 항목. 같은 종목 48시간 안 제목 2-gram Jaccard 0.45 이상을 대표 기사와 비교. 다른 회사 기사가 섞이는 예제가 묶이지 않음 (design 5.4) | D-15, S-05 |
| N-10 | 종목 매칭 | design 5.4의 규칙(영문 단어 경계, 한글 경계와 조사, 일반 명사 종목명, 그룹명 접두)마다 맞아야 할 예제와 걸러야 할 예제를 고정 자료로 두고 전부 기대대로 판정. 빠른 모델로 넘기는 것은 규칙으로 판정하지 못한 예제뿐이고, 고정 자료에서 그 수를 기록 | T-09, N-09 |
| N-11 | 수집 스케줄 | design 5.2 표와 일치. 장중 판정은 토스 장 운영 정보. DART 일일 한도 50%에서 주기를 늘림 (API-5) | N-02 ~ N-07, N-16, D-10, J-01 |
| N-12 | AI를 깨우는 트리거 | 긴급·높음 공시, 하루 ±5%, 1시간 5건. 정정 공시는 원 등급에서 한 단계 낮춤 (design 5.3) | N-11, A-10 |
| N-13 | MCP 도구: `search_news`, `get_disclosures` | 외부 텍스트가 `untrusted_text`로만 나감 (SEC-4) | N-09, M-08 |
| N-19 | 웹 검색어 검사 (SEC-9) | 두 자리 이상 숫자(연도·분기 허용 목록 제외), URL·도메인 형태, 금액·수량·계좌 단위어가 든 주제어 거부. 보유 수량·금액·계좌번호를 섞은 속성 테스트에서 통과 0건. 거부와 통과가 감사 로그에 남음 (harness 6.9) | A-02 |
| N-18 | `request_web_research` 도구 | 앱이 아는 종목(보유, 관심, 코드가 뽑은 후보)과 코드가 정한 주제 목록의 항목만 받음. 자유 문장 인자가 스키마에 없음. 호출 빈도 제한. 코드가 만든 검색어는 N-19를 통과해야 함. 사용자 문장 모드는 저장된 사용자 메시지 원문만 넘김. 수집 실행을 따로 띄워 결과만 `untrusted_text`로 돌려줌 (design 3.1, 4.4, SEC-9) | AI-14, M-08, N-19 |
| N-20 | 종목별 웹 수집 예약 | 스케줄러가 코드가 정한 주제로 수집 실행을 띄움. 결과는 출처 `web` (architecture 2.2) | N-18, J-01 |
| N-21 | 중요 공시 카드 | 보유 종목 긴급·높음 공시가 주 화면 카드로 (design 2.4) | N-12, U-09 |
| N-14 | 고급 > 뉴스·데이터 | 소스 켜기·끄기, 본문 읽기, 선택 키, SEC 연락처, 주기, AI를 깨우는 기준, 종목 약칭 표. 소스별 마지막 성공 시각은 고급 > 상태 (REL-5) | N-11, N-17, U-23 |

## J. 예약 작업 (`crates/scheduler`)

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| J-01 | 스케줄러 코어 | 짧은 주기로 깨어 `Clock`과 비교(`Instant` 장시간 대기 없음, RV-12). 레지스트리 등록. 작업별 놓친 작업 정책. 고정 시계로 여러 날을 건너뛰어도 한 번만 따라잡음 (REL-3) | A-10, AI-06 |
| J-09 | 관심 종목 관리 | 수집·감시·추정 대상에 반영 | U-05 |
| J-02 | 논거 노트 MCP 도구: `get_thesis`, `update_thesis` | 주장 단위 차분으로 쓰고 이력이 남음. 출처가 웹뿐인 변경은 "확인 필요"로 보류 (design 3.1) | D-16, S-06, M-08 |
| J-03 | 아침 브리핑 작업과 프롬프트 | 08:20, 수집 → 분석. 출처 포함. 결과는 주 화면 카드. 가짜 제공자 테스트 (design 4.3) | J-01, N-13, N-20 |
| J-04 | 마감 리뷰 작업 | 변동 분해는 코드, 원인 후보는 AI | J-03 |
| J-05 | 미국장 전 점검 작업 | 개장 시각을 토스 장 운영 정보에서 읽음 | J-03 |
| J-07 | 브리핑 카드와 지난 브리핑 보조 화면 | 네 상태 테스트 | J-03, U-09 |
| J-08 | 논거 노트 보조 화면 | 현재 노트와 이력. 네 상태 테스트 | J-02, U-13 |
| J-11 | 일별 평가 스냅샷 작업 | 16:00 기록, 빠진 날 재계산 (I-09, design 4.3) | J-01, A-16, S-18 |
| J-12 | 추정 판정 작업 | 국내·미국 마감 후 A-15를 돌림. 앱이 꺼져 있던 기간도 다음 시작 때 판정 (REL-7) | J-01, A-15, S-18 |
| J-13 | 장중 스냅샷 축소와 `incremental_vacuum` | 30일 지난 장중 스냅샷이 일별 하나로 (architecture 6.3) | J-01, S-02 |
| J-14 | 토스 스펙 변경 감시 작업 | T-19를 주기적으로 돌림 (I-08) | J-01, T-19 |
| J-15 | 백업 작업 | 하루 한 번 S-11 (REL-6) | J-01, S-11 |
| J-06 | 장중 감시 | 트리거가 오면 논거 재판정과 재추정. 같은 사건으로 중복 실행되지 않음 | J-02, N-12, E-05 |
| J-10 | 예약 작업 설정 | 고급 > AI 안에서 켜기·끄기와 시각 변경. 실행 기록은 고급 > 상태 | J-01, AI-10 |

## E. 확률 추정, 신호, 판정, 성적표

실제 돈을 넣기 전에 주문 기능 없이 추정만 기록하는 기간(E-90)을 둔다 (open-issues 1.4, 실측 S-06, design 10장 4단계).

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| E-01 | `get_estimate_task` 도구와 과제 저장 | 사건 정의(+T%, −S%, H), 기준 확률, 기준가와 시각을 코드가 만들고 저장. 과제는 코드가 정한 대상(보유, 관심, 코드가 뽑은 후보)에만 있음. 도구 입력은 `task_id`뿐이고 종목을 받지 않음(`tools/list` 골든 파일). AI 입력이 과제 내용에 영향을 주지 않음 (design 3.1, 6.1, SEC-4) | D-21, D-22, M-02, S-18 |
| E-02 | `submit_estimate` 도구 | 입력은 `task_id`, p(1~99 정수), 근거, 반대 논거, 출처뿐. 방향·수량·가격 같은 추가 필드는 거부 (ORD-4). 주문을 만들지 않음. 숫자는 도구 결과 참조로만 받고 원장에 없으면 거부 (ACC-1) | E-01, A-12 |
| E-03 | `get_profile`, `get_scorecard` 도구 | 판정 결과와 적중률을 코드 계산값으로 줌. 30건 미만이면 판정 보류 표시 | M-02, D-17, D-20 |
| E-04 | 추정 프롬프트 | 기준 확률을 먼저 보여 주고 근거가 있을 때만 움직이게 함. 반대 논거 하나 이상, 근거마다 출처. 프롬프트 파일 형식 통과 (design 6.1, R-12 2.8) | AI-07, E-02 |
| E-05 | 종목 추정 순환 작업 (단일 호출) | 보유·관심 종목을 하나씩 돌며 추정. 예약 작업 한도 비율 안에서만. 장이 닫힌 시간의 추정도 기록하고 신호 계산 (design 4.3) | E-04, J-01, J-09 |
| E-06 | 신호 카드 (주 화면) | 코드 계산 사실(현재가, 수량, 금액, 수수료·세금, 주문 후 비중, AI가 낸 p와 실제 적중률을 반영한 p, 임계값, 구간 적중률과 건수)과 AI 근거를 나눠 표시. 화면 문구에 "보정"이 없음(UX-6). 근거가 외부 출처뿐이면 경고하고, 시세·포트폴리오 도구 참조는 출처로 세지 않음 (SEC-4). 주문 기능이 꺼져 있으면 주문 버튼이 없음 (ORD-5). 수량이 0이면 버튼 없이 이유 한 문장 (design 6.3) | A-13, U-09, DS-04 |
| E-07 | 장이 닫혔을 때의 카드 | "장 열리면 확인" 카드, 버튼 없음. 장이 열린 뒤 A-14 결과로 바뀜 (design 6.4) | A-14, E-06 |
| E-10 | 넘기기 | 사유를 한 줄 적을 수 있고 기록만 함. 다음 추정 입력에 사유가 들어가지 않음(테스트) (design 6.5) | E-06, S-07 |
| E-11 | 신호 알림 | 새 신호가 macOS 알림으로 (UX-3) | E-06, U-14 |
| E-08 | 성적표 보조 화면 | 구간별 적중률과 건수, Brier 점수와 신뢰도 도표, 신호 수익률(가상 청산)과 벤치마크, 신뢰구간, 비교 수. 모델별·기간별. 30건 미만은 판정 보류. 화면 문구에 "보정"이 없음 (design 6.6, UX-6) | D-29, U-05, DS-06 |
| E-09 | 투자 성향 설정 (기본 설정) | 기간, 위험 성향, 제외 유형·섹터, 비중 상한과 기본값. 몇 시간 단위 기간, 비중 상한 완화, 위험 성향을 더 공격적으로 바꾸기는 위험 설정 확인 창을 거침 (SEC-12, design 6.2) | D-20, U-23, U-24 |
| E-90 | 추정만 기록하는 기간 (담당: 사용자, 실측 S-06) | 주문 기능을 끈 채 운용. 판정된 추정이 30건 이상 쌓이고 성적표가 표시됨. 사용자가 성적표를 보고 계속할지 기록 | E-05, E-08, J-12 |
| E-91 | 보정 방법 ADR과 구현 (사용자 승인: 기존 테스트 변경) | E-90 자료로 보정 방법을 정해 D-23에 적용. 방법은 표본이 적을 때 기준 확률 쪽으로 당기는 방식이어야 함(표본 1건 예제에서 결과가 기준 확률에 가까움). 구간별 적중은 표시용. 보정 전후 Brier 점수를 기록. 30건 미만 조합은 계속 보수 운용 (design 6.2) | E-90 |
| E-92 | 신호·수량 세부 값 ADR 조정 (사용자 승인: 기존 테스트 변경) | 위험 성향별 (a, b)와 여유분, 한 번에 사는 비중, 매도 비율, 지정가 산정식, 가격 이탈 범위 계수를 E-90 자료로 조정. 조정한 (a, b)의 p₀가 20~40% 안 (design 6.2, 9.2) | E-90 |

## O. 주문

O 묶음은 E와 함께 개발할 수 있다. 주문 기능 스위치는 처음에 꺼져 있고(ORD-5), 사용자는 E-90과 O-15를 거친 뒤에 켠다.

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| O-01 | 주문 게이트 검증 (사실 수집과 `domain::validate`) | 종목, 장 시간, 통화별 매수가능금액·매도가능수량, 같은 종목 반대 방향 미체결(국내만), 매수 유의사항, 원칙. 항목마다 통과·실패 테스트. 줄 커버리지 100% (design 6.4, harness 6.1) | D-12, D-14, D-26, T-11 |
| O-03 | `order-execution` 타입: `BrokerTrade`, `UserApproval`, `ApprovedOrder` | `UserApproval`이 `Proposal<Validated>`를 소유. crate 밖에서 두 타입을 만들거나 복제·`Default`·비공개 메서드 호출하는 코드가 컴파일되지 않음(trybuild). 직렬화·`&mut` 접근자 없음 (architecture 4.3) | A-02, D-14 |
| O-17 | `approve_and_submit` | `UserPresence::confirm`이 `Confirmed`일 때만 `ApprovedOrder`가 생김. 토큰은 `self` 소비로만 넘어감. Secure Enclave 서명 자리 (architecture 4.3) | O-03, A-04 |
| O-22 | 주문 횟수 상한 (ORD-6) | 전체 시간당 주문 건수 상한 값 ADR(토스 어뷰징 예보다 충분히 낮게). 넘으면 승인 요청 거부와 이유 한 문장 | A-02, S-10 |
| O-04 | `approval-command` 1: architecture 4.4의 1~5 | 확인 창 하나와 거절 후 재요청 제한, 요청 횟수 기록. 스위치와 횟수 상한. `proposal_id`로 읽고 만료 확인. 만료됐으면 최신가로 다시 만들고 추정 기준가에서 변동성 기준 범위 밖이면 거부. 최신 시세·잔고로 다시 `validate`. `OrderSummary` 계산 | O-01, O-17, O-22, D-32, S-07, A-14 |
| O-18 | `approval-command` 2: architecture 4.4의 6~10 | 본인 확인 뒤 값이 허용 범위를 넘으면 전송 0건(다시 계산하지 않음). 한 트랜잭션으로 소비 기록과 전송 전 기록, 실패하면 전송 0건. 결정적 `clientOrderId`. 응답 기록과 `OrderSubmitted` (ORD-2, ORD-3) | O-04, S-08 |
| O-19 | 결과를 모르는 주문 처리 | 10분 안에는 같은 키·같은 내용으로만 재요청. 10분 뒤와 `maintenance` 뒤에는 재전송 0건이고 조회·이벤트로 대조. 하나로 정해지지 않으면 "확인 불가"와 같은 종목 새 주문 차단 (ORD-2, I-07) | O-18, T-15 |
| O-05 | 감사 로그 연결 | 추정, 신호, 검증 결과, 승인 요청, 본인 확인 결과, 넘기기, 전송 전 기록, 응답, 위험 설정 변경이 각각 1건씩 남음 (ORD-3) | A-17, E-10, O-18, S-16 |
| O-25 | 토스 주문 내역 대조 | 감사 로그의 전송 기록과 토스 주문 조회를 대조해 불일치를 탐지하고 고급 > 상태에 표시 (I-10) | O-05, T-15 |
| O-06 | 주문 기능 스위치 | 처음 설치하면 꺼짐. 켜기는 위험 설정 확인 창을 거침. 꺼져 있으면 승인 요청이 거부되고 카드에 버튼이 없음 (ORD-5, SEC-12) | A-17, S-10, U-24 |
| O-07 | 주문 본인 확인 (`platform-macos`) | `rfd` 요약 창에 코어가 계산한 요약과 고지 문구(UX-2). `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`를 `spawn_blocking`에서 호출, 재사용 0초. 이유 문자열에 종목·방향·수량. 창은 한 번에 하나 (SEC-8). `unsafe` 처리 ADR: 안전한 래퍼를 쓸지 `platform-macos`에만 보호된 예외를 둘지(예외면 사용자 승인) (architecture 4.5) | A-02, U-24, S-17 |
| O-08 | 신호 카드의 주문 버튼과 승인 IPC | WebView는 `request_order_approval(proposal_id)`만 부름. 다른 필드는 역직렬화 거부. 버튼은 스위치가 켜지고 장이 열려 있고 제안이 있을 때만. 수량·가격을 바꾸는 버튼 없음 (SEC-5, design 6.5) | E-06, O-06, O-18, U-03 |
| O-09 | 고액 주문 확인 | 1억 원의 80% 이상이면 처음부터 최종 확인(네이티브 요약 창)에서 고액 확인을 받고 `confirmHighValueOrder`를 붙임. 그래도 토스가 고액 확인으로 거절하면 그 주문은 실패이고 재전송 0건, 이유 한 문장. 본인 확인부터 전송까지 1초 이내 측정 (PERF-3, design 6.5) | O-07, O-18, T-16 |
| O-11 | 체결 추적과 주문 내역 보기 | 미체결 주문 상태 갱신과 주문 내역 보조 화면(보기만, 정정·취소 버튼 없음). 주문 직후 보유 즉시 동기화. 네 상태 테스트 (design 6.5) | O-18, T-15, T-17 |
| O-26 | 수수료·세금 추정 표시와 확정값 교체 (ACC-7) | 신호 카드, 요약 창, 주문 내역의 수수료·세금은 "추정"으로 표시. 체결·결제 뒤 거래내역의 `commission`·`tax`·`netAmount`로 바뀌고 "추정" 표시가 사라짐(녹화 응답 테스트) | O-11, A-07 |
| O-12 | 채팅 안의 신호 카드 | 주 화면 카드와 같은 컴포넌트, 코어 ID로만 그림 | C-05, O-08 |
| O-14 | 고급 > 주문 | 시장가 허용, 카드 만료 시간, 몇 시간 단위 기간 허용, 주문 횟수 상한. 기본 꺼짐·10분·꺼짐. 위험 설정은 확인 창을 거침 (design 8장, SEC-12) | O-06, U-23 |
| O-13 | 경로 고정 테스트 1 (사용자 승인) | 제품과 같은 `build_app`에 `ScriptedPresence`와 `RecordingBroker`. architecture 4.6의 (a), (b), (c), (f), (g), (h) 통과. 실측 S-13 결과 기록 | O-08, U-03 |
| O-20 | 경로 고정 테스트 2 (사용자 승인) | architecture 4.6의 (d), (e), (i) ~ (n) 통과. 본인 확인을 무시하게 바꾼 변이에서 (a)가 실패 (harness 6.4, 6.5) | A-17, J-01, M-05, O-13, O-22 |
| O-21 | E2E 연기 테스트 (사용자 승인) | `e2e-app`에만 자동화 플러그인. 가짜 본인 확인(항상 거부)과 가짜 증권사로 조립해 승인 버튼만 누르면 주문 0건. 밤 macOS 작업에 등록 (harness 6.7) | H-16, O-13 |
| O-15 | 소액 실주문 검증 (담당: 사용자) | 성적표를 본 뒤 주문을 켜고 소액 지정가 주문으로 신호 → 승인 → 실제 Touch ID → 체결 → 감사 로그 → 토스 대조를 확인 (design 10장 5단계, L9) | E-90, O-09, O-11, O-20, O-21, O-25, S-17 |

## G. 성장 기능

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| G-01 | 고급 > 투자 원칙 | 자유 서술 원칙, 현금 최소 비중, 쏠림 경고 기준. 게이트와 AI 맥락에 반영 (design 8장) | D-12, U-23 |
| G-05 | 심층 연구 1: 단계 0~2 | 증거 묶음의 모든 항목에 ID와 수집 시각. 검색 계획은 종목명과 주제어만(SEC-9). 수집 결과는 출처 `web` (design 4.3) | E-04, N-18 |
| G-10 | 심층 연구 2: 단계 3~4 | 긍정론과 부정론이 서로 보지 않고 같은 증거로 쓰임. 주장 검증은 초안을 보지 않고 원장과 대조 | G-05 |
| G-11 | 심층 연구 3: 단계 5~6 | 결론은 노트 차분과 `submit_estimate`. 원칙 검사, 출처 경고, 개수 제한, 노트 상태 집계. 단계 경계마다 저장해 채팅이 끼어들어도 이어 감 | G-10, J-02 |
| G-06 | 심층 연구 예약 | 한도가 남을 때 주로 야간, 예약 작업 한도 비율 안에서만 (AI-2) | AI-06, G-11 |
| G-07 | 유망 종목 발굴 | 후보는 코드가 랭킹·섹터·수급에서 뽑음. 시가총액 구간별 할당과 순서 섞기. 출처 포함 (design 4.3) | G-11, T-10 |
| G-08 | 주간 점검 | 토요일 오전. 쏠림과 원칙 위반 점검, 성적표 갱신. 결과는 주 화면 카드 | E-08, G-01 |
| G-12 | 짝지은 비교 | 같은 입력으로 단일 호출과 파이프라인(또는 다른 모델)을 함께 돌려 기록만 함. 예산 안 표본. 성적표의 비교 수에 반영 (AI-5) | E-05, G-11 |
| G-13 | 프롬프트 재생 세트 | 지난 실행의 증거 묶음을 얼려 프롬프트 변경 때 스키마 통과율, 출처 포함률, 숫자 불일치율을 비교 (MNT-4) | E-04 |
| G-09 | 성능 점검 | 측정 기록 파일에 PERF-1(시작부터 주 화면 표시까지 초), PERF-2(채팅 첫 글자까지 초), PERF-3(본인 확인 통과부터 전송까지 초), PERF-5(대기 10분 평균 CPU %와 최대 메모리 MB)를 각 10회 이상 재서 남기고, 모두 design 7.4의 수치 이하. PERF-4는 U-31의 테스트 통과로 판정 | O-15, U-31 |
