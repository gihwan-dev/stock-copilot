# ADR 0003: 하네스 도구와 강제 방식

- 날짜: 2026-11-11
- 상태: 확정 (open-issues 4.1의 사용자 확정과 리서치 R-08, R-09, R-10, R-14, R-15 결과)
- 반영 문서: [harness.md](../harness.md) v0.2, [workflow.md](../workflow.md) v0.2, `.claude/agents/*.md`

## 사용자가 정한 원칙 (바꾸지 않는다)

| 원칙 | 출처 |
|---|---|
| 구현은 항상 새 Opus 5.5 서브에이전트. 끝나면 새 Opus 5.5 서브에이전트로 적대적 리뷰 → 비판 → 메인 평가 → 수정 | AGENTS.md |
| 주석 금지, 검사 끄기 금지, `useEffect` 계열 금지, TypeScript를 Rust처럼 | AGENTS.md, harness.md v0.1 |
| 화면 코드에서 HTML 태그·`className`·`style` 금지, 디자인 시스템 공개 props 규칙, 공개 목록 잠금 | harness.md 4.4 |

## 결정

### 공통·도구 버전

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| Rust 툴체인을 `rust-toolchain.toml`에 1.99.1로 고정 | 이 PC의 Rust 1.88은 Tauri 2.12(1.90 이상)를 못 쓴다. 1.99.1에서 린트 초안 전체를 검증했다 | 사용자 PC의 기본 툴체인 사용 | open-issues I-17, R-09, R-05 |
| 타입 검사는 TS 7.0.2, 린트는 TS 6.0 API. npm 별칭으로 함께 설치 | TS 7에는 프로그램 API가 없고 typescript-eslint는 `<6.1.0`만 지원한다. 위반 예제 결과가 두 버전에서 같았다. TS 7이 6.5배 빠르다 | TS 6 단독, TS 7 단독 | open-issues I-16, R-08 2.1 |
| 전체 `just check`는 로컬 macOS와 macOS CI(`main` 푸시와 밤)가 맡는다. Linux CI는 서명 검사와 플랫폼 중립 crate 묶음(보호된 제외 목록)만 돈다 (2026-11-11 리뷰로 고침) | macOS 러너는 Linux보다 분당 약 10배 비싸다. macOS 전용 crate가 있고 `cfg`와 `[target]` 표를 금지하므로 Linux에서 작업 공간 전체를 빌드할 수 없다 | 푸시마다 macOS, 사용자 Mac의 자체 호스팅 러너(토스 키가 있는 PC에서 에이전트 코드를 돌리게 된다), Linux에서 전체 `just check`(v0.2 초안) | R-15 2.6, reviews/DESIGN-v0.2 #5 |
| git 훅은 just + lefthook. lefthook의 `ai:` 기능은 쓰지 않는다 | 단일 바이너리, `assert_lefthook_installed`. `ai:`는 보호 파일을 다시 쓰게 된다 | pre-commit, husky, `core.hooksPath` | R-10 Q7 |

### Rust

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 핵심 린트는 개별 `forbid`, 그룹(pedantic, nursery, cargo)은 `deny` | 코드 속 속성과 `RUSTFLAGS`로 끌 수 없는 것은 `forbid`뿐이다. 그룹 `forbid`는 개별 예외를 무효로 만든다 | `deny`만, 그룹 `forbid` | R-09 Q1, Q2 |
| 검사 끄기를 세 겹으로 막는다: 개별 `forbid` + xtask 토큰 검사 + 환경 변수 고정 | 안쪽 `#![allow]`, `#[expect]`, `cfg(not(clippy))`, `--cap-lints`, `CLIPPY_CONF_DIR`가 clippy를 통과했다 | clippy 설정만 | R-09 Q13 |
| 주석 검사는 `ra-ap-rustc_lexer`(정확한 버전 고정) + `proc-macro2` | 문자열 안의 `//`를 오탐하지 않고 문서 주석과 `#[doc]`까지 잡는다 | 정규식, `syn`(매크로 본문 속 속성을 못 본다) | R-09 Q8, Q13 |
| 작업 공간 의존 방향은 `xtask arch-check` 단독 | cargo-deny는 작업 공간 crate와 전이 경로를 막지 못했다 | cargo-deny `wrappers` | R-09 Q3 |
| 서드파티 허용 목록은 cargo-deny 0.20.2만 | 허용 목록, 라이선스, 취약점, 빌드 스크립트를 한 도구로, 운영 비용이 낮다 | cargo-vet | R-09 Q10 |
| 테스트 실행은 cargo-nextest 0.9.151만 | 네트워크 차단과 필터를 한곳에서 건다 | `cargo test` | R-09 Q9 |
| 속성 테스트 proptest 1.12.0, 변이 테스트 cargo-mutants 27.1.0(`--cap-lints=true`, 푸시 훅은 변경분, 밤은 전체) | 엄격 린트 아래에서 동작 확인. `--cap-lints` 없이는 정상 변이체가 unviable이 된다 | quickcheck, bolero | R-09 Q6, R-15 2.6 |
| 커버리지 cargo-llvm-cov 0.9.1, crate별 `-p` + `--fail-under-file-lines` | 안정 툴체인에서 동작. 분기 커버리지는 nightly가 필요하다 | 분기 커버리지 | R-09 Q7 |
| 미사용 의존성 cargo-machete 0.9.2 | rustc `unused_crate_dependencies`는 dev 의존을 오탐했다 | rustc 린트 | R-09 3장 |
| 금액 `rust_decimal`, `checked_*`만 | Copy, 할당 없음, 28자리. 연산자 panic은 `arithmetic_side_effects`가 막는다 | bigdecimal, fastnum | R-09 Q5 |
| 토스 모델은 typify 0.8.0으로 `components.schemas`만 생성해 보호된 별도 crate에 둔다 | 3.1 원본을 받고 컴파일된다. 생성물의 `#![allow]`와 문서 주석을 격리한다 | progenitor(실패), oas3-gen(컴파일 오류), openapi-generator(Java 필요, 중첩 `Option`) | R-09 Q11 |
| 현재 시각과 환경 변수 읽기 금지는 clippy가 아니라 xtask가 검사하고, 보호된 파일 허용 목록(시계 어댑터 하나, 조립 지점)을 둔다. 서드파티 crate의 현재 시각 함수와 `var_os`도 막는다 (2026-11-11 리뷰) | clippy 목록은 파일별 예외를 둘 수 없고 예외에는 끄기 속성이 필요하다 | clippy `disallowed-methods`만(v0.2 초안) | reviews/DESIGN-v0.2 #28 |
| `unsafe_code`는 `forbid`. `platform-macos` 하나에만 보호된 예외를 둘 수 있고, 둘지는 O-07의 ADR에서 정한다 (2026-11-11 리뷰) | macOS 본인 확인 호출에 `unsafe`가 필요할 수 있다 | 예외 없음(v0.2 초안), crate마다 예외 | reviews/DESIGN-v0.2 #27 |

### TypeScript와 디자인 시스템

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 린터는 ESLint 10.14.0 + typescript-eslint 8.72.2 하나 | 필요한 플러그인과 타입 인식 로컬 규칙이 모두 도는 것은 ESLint뿐이다 | oxlint(JS 플러그인 alpha, 타입 인식 사용자 규칙 불가), Biome(타입 추론 부분적) | R-08 2.3 |
| 불변·실패 처리는 eslint-plugin-functional 10.0.1 | `immutable-data`, `no-expression-statements(ignoreVoid)`는 타입 정보가 필요하다 | `no-restricted-syntax`만 | R-08 3장 |
| `Result`는 직접 정의한 판별 유니온 | 무시 탐지를 범용 규칙으로 얻었다. `switch` 빠짐 검사와 맞물린다 | neverthrow(`_unsafeUnwrap`이 던진다), Effect | R-08 2.4 |
| 금액·수량·가격·비율은 비공개 symbol 키의 불투명 객체 | 문자열 브랜드는 `parseFloat(a) + Number(b)`를 `tsc`가 통과시켰다 | 문자열 브랜드 | open-issues I-15, R-08 2.9 |
| 원시 타입 금지: TS는 타입 자리의 `string`·`number`·`bigint`를 `local/no-raw-types`로, Rust는 `domain`·`application`·포트의 원시 타입을 `xtask raw-types`로 막는다. 화면 문구도 `UiText` 브랜드 | 사용자 결정(2026-11-12). 값이 섞이는 실수(종목 코드 자리에 계좌 번호, 개수 자리에 금액)를 컴파일 단계에서 막는다. 문구가 `messages.ts`에 모여 금지어 검사 범위가 좁아진다 | 금액·식별자만 전용 타입, 리뷰로만 확인 | 사용자 결정 |
| 불린 금지: Rust `bool`, TS `boolean`을 시그니처·필드·props에 쓰지 않고 열거형(TS는 리터럴 유니온)으로 쓴다 | 사용자 결정(2026-11-12). 불린은 세 번째 경우를 더할 수 없고 호출부에서 뜻이 드러나지 않는다 | 불린 매개변수만 금지 | 사용자 결정 |
| 화면 문구는 `messages.ts`의 문구 목록(키마다 언어별 문장)에서만 만든다 | 원시 타입 금지의 결과이고, 다국어를 바로 붙일 수 있다. 금지어 검사 범위도 이 파일들로 좁아진다 | 화면 코드에 문구 직접 쓰기 | 사용자 결정 |
| 검사 끄기는 한 줄·규칙 하나·이유 필수 형태만 허용한다(TS `eslint-disable-next-line`, Rust `#[expect(…, reason)]`). `forbid` 린트와 안전 관련 TS 규칙은 끌 수 없다. 모든 끄기는 `xtask disable-report`에 나오고 리뷰어가 판정한다 | 사용자 결정(2026-11-12). 규칙을 지킬 방법이 없는 곳에서 작업이 멈추지 않게 한다 | 끄기 전면 금지 | 사용자 결정 |
| 스크롤은 `shared/ds`의 `ScrollArea`(Base UI ScrollArea) 하나로만 만들고 막대는 내용 위에 겹쳐 띄운다. `overflow`·`position`·고정 크기·화면 단위는 허용 목록 파일에만 둔다. 모든 이야기와 화면을 부하 내용과 여러 창 크기로 그려 넘침·잘림을 DOM에서 재는 검사를 `ds-check`에 넣는다 | 사용자 결정(2026-11-12). 넘침은 린트만으로 못 막고 실제로 그려야 드러난다. 겹치는 막대는 배치를 밀지 않는다 | 기본 스크롤 막대, 시각 회귀 이미지만으로 확인 | 사용자 결정, R-14 |
| 규칙 예외는 폴더가 아니라 파일 허용 목록 (`shared/platform`, 타입 술어, `throw`) | 폴더 예외는 파일을 옮겨 규칙을 피할 수 있다 | 폴더 단위 덮어쓰기 | R-08 2.2 |
| `throw` 예외 목록은 비운다: 오류 경계는 `react-error-boundary`, 라우터 `redirect`는 반환, `queryFn`은 `Promise<Result>` | 클래스와 `throw`를 앱 코드에 들이지 않는다 | `shared/platform`에서 `throw`·클래스 허용 | R-08 2.5, 2.6 |
| 경계는 eslint-plugin-boundaries 7.4.2, 순환은 dependency-cruiser 18.7.2(`check-full`), 미사용은 knip 6.45.1 | 실측으로 위반을 잡았다. 편집 직후 ESLint 한 번으로 피드백이 온다 | dependency-cruiser 단독, `import/no-cycle` | R-08 2.7, 2.8 |
| 스키마는 Zod 4.6.7, `safeParse`만 | 실측 | Valibot, ArkType | R-08 2.9 |
| 쿼리·라우터는 TanStack Query 5.107.1 + TanStack Router 1.170.46 | 예제에서 타입 검사와 린트 통과 | SWR, React Router | R-08 2.6 |
| 테스트는 Vitest 5.1.0 + Testing Library + tauri-specta `mockCommands` | 가짜 IPC가 생성된 명령 타입과 컴파일 단계에서 어긋나지 않는다 | `mockIPC` 직접 사용 | R-15 2.2 |
| 토큰은 Tailwind v4 `@theme` + `--*: initial`. 클래스 규칙(`better-tailwindcss` 4.11.0)과 stylelint는 `shared/ds` 안에만 | 토큰 밖 클래스가 생성되지 않는다. 화면 코드는 `className` 자체가 금지라 클래스 규칙이 필요 없다 | CSS 변수 + CSS Modules, Panda CSS, vanilla-extract | R-14 2.2, harness.md 4.4 |
| R-14가 `shared/ds`에서 풀자고 한 규칙 3(숫자 유틸리티), 5(가격 색), 7(인라인 `style`)은 보호된 허용 목록으로만 푼다 | 4.4가 R-14보다 엄격하다는 원칙을 따른다 | 폴더 전체 예외 | harness.md 4.4 |

### 테스트

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 테스트 네트워크 차단은 세 겹: `TossEndpoint` 주입과 주소 문자열 위치 제한, 루프백 전용 resolver, OS 수준 `sandbox-exec`(cargo `runner`) | runner 아래에서 외부 연결은 실패, 루프백은 성공했다. 자식 프로세스도 물려받는다 | nextest wrapper script(실험 기능. runner가 nextest에서 동작하지 않으면 이것으로 바꾼다) | R-15 2.7, R-09 Q9 |
| `order-execution`, 주문 게이트, `domain`의 신호·주문 계산 모듈은 커버리지 100%. 방어 분기는 타입으로 없애고, 남는 것은 보호된 예외 목록 | 100%와 방어 분기가 충돌한다. 코드로 커버리지를 빼는 수단은 unstable이라 막혀 있다 | "95% + 변이 생존 0"으로 낮추기 | open-issues I-13, R-09 Q7 |
| 주문 경로 고정 테스트는 `tauri::test` 목 런타임 + 제품과 같은 `build_app` | Touch ID는 자동화할 수 없어 이 계층이 유일한 자동 고정 지점이다 | WebDriver E2E | R-15 2.0 |
| 가짜 본인 확인은 `UserPresence` 포트(`application`)의 가짜로 `testkit`에, 가짜 증권사는 `trade-fakes`에 둔다. `testkit`은 `order-execution`에 의존하지 않는다 | `testkit`이 `order-execution`에 의존하면 주문 차단 구조와 충돌한다. `order-execution` 안의 `cfg(test)` 모듈은 `src-tauri` 경로 고정 테스트가 쓸 수 없고, `cfg(test)` 밖의 `cfg`와 `[features]`는 금지다 | `order-execution` 안의 테스트 전용 모듈, `testkit`이 `order-execution`에 의존 | open-issues I-14, ADR 0002 23번 |
| 시각 회귀 기준 환경은 태그 고정 Playwright 컨테이너(linux/arm64). 비교 엔진은 harness.md 6.6(실측 S-16, DS-15)에서 | 같은 컨테이너는 바이트까지 같았고 호스트 macOS와는 글꼴부터 달랐다 | macOS 호스트 기준 | R-15 2.5 |
| E2E는 밤 macOS 연기 테스트만. 자동화 플러그인은 별도 바이너리 `e2e-app`에서만 | 플러그인이 HTTP로 앱을 조종할 수 있다 | `debug_assertions` 분기 등록, CrabNebula(유료) | R-15 2.1 |

### 에이전트 하네스

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 보호 경로는 권한 `deny` + `PreToolUse` 허용 목록 훅 + 엄격 샌드박스 `denyWrite`(`failIfUnavailable`, `allowUnsandboxedCommands: false`) + CI 서명·허용 목록 검사. 방어의 중심은 로컬 겹이고 CI는 푸시 뒤의 사후 탐지다 | 셸 간접 쓰기는 샌드박스만 막았다. 샌드박스는 거부 목록이라 정확한 허용 목록은 CI가 맡는다 | 거부 규칙만, 훅만, 별도 사용자·컨테이너(비용) | open-issues I-11, R-10 Q2, 3.1 |
| 에이전트 쓰기 허용 목록에 `Cargo.lock`, `pnpm-lock.yaml`, IPC 생성물을 넣는다. `docs/open-issues.md`와 이미 있는 `docs/adr/*`는 보호한다(새 ADR 추가는 허용) (2026-11-11 리뷰) | 잠금 파일이 없으면 의존성을 추가하는 작업이 모두 멈춘다. 내용은 의존성 허용 목록 검사가 본다. 확정 결정 문서를 에이전트가 바꾸지 못하게 한다 | 잠금 파일과 IPC 생성물 보호, 결정 문서 비보호(v0.2 초안) | reviews/DESIGN-v0.2 #2, #3, A2 |
| 하네스 훅은 정의 파일이 아니라 `.claude/settings.json`에 두고 `agent_type`으로 역할을 나눈다 | 정의 파일 훅은 신뢰 대화상자 이후에만, `-p`에서는 돌지 않는다 | 정의 파일 안 훅 | R-10 Q1 |
| 모델은 정의의 `claude-opus-5-5` + `PreToolUse(Agent)`가 `model` 인자 거부 + `resolvedModel` 기록 | 호출 인자가 정의의 모델보다 우선한다 | 정의만, `CLAUDE_CODE_SUBAGENT_MODEL_FORCE`(내장 서브에이전트까지 한 모델로 묶는다) | R-10 Q3 |
| 포크·재개·중첩·백그라운드 금지 (`CLAUDE_CODE_FORK_SUBAGENT=0`, `Agent(fork)`·`SendMessage` 거부, 깊이 1, 백그라운드 끔) | 포크와 재개는 이전 맥락을 물려받는다. 백그라운드면 결과 훅이 시작 시점에 불린다 | 정의 파일 문구로 부탁 | R-10 Q3, Q10 |
| 종료 검사 3회는 훅이 `agent_id`별로 직접 센다. 메인은 결과 훅의 상태로 `unchecked`를 확인한다 | 내장 상한 8은 도구 호출마다 초기화된다. 훅 타임아웃 시 종료가 허용된다 | 내장 상한 사용 | R-10 Q10 |
| 리뷰어 격리는 `isolation: worktree` + 대상 SHA `checkout --detach` + `SubagentStart/Stop` 원본 상태 비교 훅 | 격리된 리뷰어가 셸로 원본에 썼다. 비교 훅으로 리뷰어가 스스로 되돌렸다 | 저장소 밖 작업 사본에서 `claude -p`로 띄우기(구조 변경이 커서 보류) | open-issues I-12, R-10 Q11 |
| 리뷰어에게는 브리프와 기준·대상 SHA만 준다. 리뷰어는 등급만 매기고 판정하지 않는다 | PR 설명 조작으로 리뷰를 97% 속였다는 연구. 평가자가 스스로 설득해 승인한 실패 사례 | 구현자 보고서나 메인의 변경 요약 제공 | R-10 Q4, Q5 |
| 사람 승인은 Secure Enclave SSH 키(Secretive, Touch ID) 서명. `commit.gpgsign=false`. CI는 푸시마다 기준 커밋(푸시 이전 `main`)의 서명자·허용 목록으로 검증하고 실패하면 알린다(사후 탐지). GitHub 규칙 세트와 브랜치 보호에 기대지 않는다. 푸시는 메인 에이전트만 한다. 병합은 사람 (2026-11-11 리뷰로 고침) | 에이전트는 키를 쓸 수 없다. 푸시된 쪽에서 신뢰 자료를 바꿔 통과하는 길을 막는다. 무료 요금제의 비공개 저장소에서는 규칙 세트와 브랜치 보호를 쓸 수 없다(open-issues S-21) | 커밋 메시지 표시, 모든 커밋 서명, 규칙 세트 "서명 커밋 필수", `pull_request_target`과 "PR 필수" 규칙 세트(v0.2 초안) | R-10 Q9, reviews/DESIGN-v0.2 #4, #7 |
| 기존 테스트 삭제·변경은 `xtask test-diff`와 테스트 수 비교로 막고, 사용자 서명 커밋만 통과 | Claude 계열 편법의 79% 넘게가 테스트 수정이었다 | 리뷰에만 의존 | R-10 Q6 |
| 하네스 상태 파일은 `~/.local/state/stock-copilot-harness` | 샌드박스 안에서 쓸 수 없는 곳이라 위조되지 않는다 | `.git/`, `$TMPDIR` | R-10 Q2 |
| macOS 관리 설정으로 샌드박스를 관리자 필수로 만드는 것은 보류 (사용자 선택) | 이 PC의 모든 프로젝트에 적용된다 | - | R-10 Q9 |

## 결과

- 도구로 막지 못하는 우회는 harness.md 9장 "리뷰에서 확인할 항목"으로 모았다. 리뷰어가 항목별 확인 결과를 쓴다.
- 실측이 남은 것은 harness.md 10장에 있다. 샌드박스 빌드(S-03)가 깨지면 보호 경로 구성(위 첫 행)을 다시 검토한다.
- 형식 도구(Prettier 또는 Biome 형식 기능)는 리서치가 비교하지 않아 이 ADR에서 정하지 않았다.
