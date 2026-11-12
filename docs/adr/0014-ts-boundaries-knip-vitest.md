# ADR 0014: TS 폴더 경계, 미사용·순환 검사, Vitest (H-09, H-11의 Vitest 부분)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-09, H-11(Vitest 부분)
- 반영한 문서: 없음 (architecture.md 5장·5.2, harness.md 1.1, 2.1, 4.2, 4.3, 6.1, 6.3, 6.8, ADR 0009, ADR 0012를 따르고 어긋나거나 문서가 정하지 않은 곳만 적는다)

## 맥락

harness 4.2 "경계" 행은 `boundaries/dependencies`, `boundaries/no-unknown-files`, `boundaries/no-unknown-dependencies`로 architecture 5장 폴더 의존 표를 강제하고, 다른 요소의 파일은 `index.ts`로만, `@tauri-apps/api`는 `shared/ipc`에서만 가져오게 한다. ADR 0009는 `zod`를 `shared/ipc`와 `shared/lib`의 브랜드 파일로 묶었다. harness 4.3은 `eslint-plugin-boundaries` 7.4.2, `dependency-cruiser` 18.7.2(순환, `check-full`), `knip` 6.45.1, Vitest 5.1.0을 정했다. 실측 S-19는 boundaries의 폐기 예정 경고를 `--max-warnings 0`이 잡는지다.

## 실측

환경: macOS 26, Node 24.22.0, pnpm 12.18.0, ESLint 10.14.0, typescript-eslint 8.72.2, eslint-plugin-boundaries 7.4.2, knip 6.45.1, Vitest 5.1.0, Vite 8.3.8.

| 항목 | 명령 | 결과 |
|---|---|---|
| 고정 버전 설치 | `pnpm -C apps/desktop add -D --save-exact eslint-plugin-boundaries@7.4.2 knip@6.45.1 dependency-cruiser@18.7.2 vitest@5.1.0` | 모두 있다. Vitest 피어 `vite ^8`이 맞았다 |
| dependency-cruiser 잠금 파일 | `node harness/ts-deps/check.mjs` | `dependency-cruiser@18.7.2`와 전이 의존 `watskeburt@6.0.0`의 `engines: {node: ^22\|\|^24\|\|>=26}`(공백 없는 `\|\|`)가 보호된 잠금 파일 줄 문법(ADR 0009) 밖이라 실패. 설치를 되돌렸다 |
| S-19 | 앱 `src`에 `boundaries/no-unknown`, `element-types`, `entry-point`, 정책의 `rules` 옵션을 켜고 ESLint API로 검사 | 경고는 `console.warn`으로만 나오고 `warningCount`·`errorCount`는 0이다. **`--max-warnings 0`은 폐기 예정 경고를 잡지 못한다** |
| 경계 검사 기준 경로 | `cwd`가 다른 ESLint 인스턴스(앱, 위반 예제)에서 같은 설정 | boundaries는 `process.cwd()`를 기준으로 삼아 위반 예제(`cwd`가 `harness/eslint-violations`)에서 모든 파일이 무시됐다. `boundaries/root-path` 설정이 필요했다 |
| 외부·바깥 의존 | 기본 설정 | 외부 모듈(`not-local`)과 요소 밖 로컬 파일(`unknown-local`)은 `dependencies` 규칙이 건너뛴다. `checkAllOrigins: true`, `checkUnknownLocals: true`로 평가하게 했다 |
| 접미사 일치 | `src/features/nested/src/app/x.ts`에서 다른 기능 가져오기 | 기본 `partialMatch`(경로 끝만 맞춤)에서는 이 파일이 `app` 요소로 분류되어 기능 간 가져오기가 통과했다. 모든 요소에 `partialMatch: false` |
| 같은 요소 안 정책 | 같은 요소 파일끼리의 허용 정책을 지우고 예제 검사 | 결과가 같다. 같은 요소 안 의존은 규칙이 평가하지 않는다(`checkInternals` 기본값). 정책에서 뺐다 |
| `typeof import("…")` | 타입 자리의 `import()` | boundaries가 의존으로 잡는다 |
| `import x = require("…")` | TS 가져오기 대입 | boundaries가 의존으로 잡는다(`no-require-imports`도) |
| `"src/…"` 맨 이름 가져오기 | `export { a } from "src/features/other"` | boundaries는 외부 패키지 `src`로 보고 통과시킨다. `tsc`는 `paths`·`baseUrl`이 없어 TS2307로 실패한다 |
| knip 범위 | `project`를 `src`만 두기 | 설정 파일이 `project` 밖이면 그 가져오기가 의존 사용으로 잡히지 않아 ESLint 플러그인이 미사용으로 보고됐다. 설정·테스트 스크립트를 `entry`로 두자 해결됐다(진입 파일의 내보내기는 보고하지 않는다) |
| knip 기본 제외 | `get-included-issue-types.js` | `nsExports`, `nsTypes`, `cycles`는 기본으로 빠지고 `cycles`의 기본 수준은 `warn`이다. `include`에 넣고 `rules`를 `error`로 두자 순환 예제가 실패했다. `dependencies`를 넣으면 `devDependencies`, `optionalPeerDependencies`가 함께 들어간다 |
| knip 앱 결과 | `pnpm -C apps/desktop exec knip --treat-config-hints-as-errors` | `src/shared/lib/money.ts`의 `Decimal` 타입 내보내기가 쓰이지 않았다. `export`만 지웠다 |
| Vitest JSON 결과 | `vitest run --reporter=json --outputFile.json=…` | `it.skip`·`{ skip: true }`·`skipIf(true)`·`runIf(false)`·`describe.skip`·`context.skip()`은 `skipped`, `it.todo`는 `todo`, `describe.todo`는 `skipped`로 나온다. 테스트가 없는 테스트 파일은 실패로 끝나지 않는다. `allowOnly: false`에서 `.only`는 실패(종료 코드 1)다 |
| 속도 | 각 검사 | 앱 `eslint src` 약 2초, 경계 정책 변형 테스트 약 10초, knip 예제 묶음 약 4초, Vitest 예제 묶음 약 9초 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 경계 표는 `apps/desktop/eslint/boundaries.mjs`의 `ELEMENTS`, `ENTRY_FILE`, `ELEMENT_DEPENDENCIES`, `PACKAGE_LOCATIONS`이고 `appConfig(목록, 경계 표)`가 받는다. `allowlists.mjs`에 넣지 않았다 | 기존 테스트가 `allowlists.mjs`의 내보내기를 정확히 고정한다 | `allowlists.mjs`에 추가(기존 테스트 변경) | ADR 0012 |
| 요소: `src/app`, `src/features/*`(`featureName` 캡처), `src/shared/{ds,ipc,platform,lib}`, 모두 `partialMatch: false`. 정책: 기본 `disallow`, 요소별 허용 방향은 architecture 5장 표 그대로이고 대상은 `index.ts`(`fileInternalPath`)만, 외부 패키지는 허용하되 `PACKAGE_LOCATIONS`의 패키지는 그 경로의 파일에서만, Node 내장 모듈과 요소 밖 로컬 파일은 금지 | 문서 표. "마지막에 맞는 정책이 이긴다" | R-08 초안(`ds`가 `ds`, `ipc`가 `ipc`를 허용하는 줄은 같은 요소 안이라 필요 없다) | architecture 5장, R-08 7.2 |
| 패키지 위치 표는 패키지마다 허용 경로 목록 한 줄이다: `@tauri-apps/api` → `src/shared/ipc/**`, `zod` → `src/shared/ipc/**`, `src/shared/lib/brand.ts`. UI 라이브러리(DS-09)는 같은 표에 한 줄씩 더한다 | 브리프 3 | 패키지마다 별도 규칙 | 브리프 |
| `zod`의 `shared/lib` 허용은 `brand.ts` 하나다. 테스트가 이 파일이 원시 타입 허용 목록에 있는지 확인한다 | "브랜드 파일(원시 타입 허용 목록의 파일)"을 원시 타입 목록 중 브랜드 파일로 읽었다. `money.ts`·`format.ts`는 Zod를 쓰지 않고, 셋 다 넣으면 두 원소를 고정할 예제가 없다 | 원시 타입 목록의 `shared/lib` 파일 셋 | ADR 0009, 해석 |
| 경계 위반 메시지는 정책 템플릿으로 `<from>에서 <to>의 <파일> 가져오기 금지…`, 패키지·Node 내장·요소 밖 파일은 각각 고유 메시지. 예제 판정 키는 `boundaries/<규칙> :: <메시지>`다 | 방향 금지와 깊은 가져오기를 키로 가른다(ADR 0009 판정 방식) | 규칙 ID만 | ADR 0009 |
| `boundaries/root-path`는 앱 설정에서 `apps/desktop`(절대 경로), 위반 예제 실행기(`lint.mjs`)에서 예제 폴더로 덮어쓴다. 기존 테스트의 "같은 경로의 유효 설정이 같다"는 `rules`와 `linterOptions`만 비교하므로 그대로 통과한다 | 실측(기준 경로) | 환경 변수 `ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH` | 실측 |
| S-19 대응: 테스트가 (1) 앱 파일의 boundaries 규칙이 v7 이름 세 개뿐이고 모두 `error`인지, 폐기 예정 이름이 없는지 (2) `eslint --max-warnings 0 src`가 아무것도 출력하지 않는지 (3) 폐기 예정 규칙은 콘솔 경고만 내고 통과한다는 실측 자체를 확인한다 | `--max-warnings 0`이 못 잡는다 | `--max-warnings 0`에 기대기 | 실측 |
| 기존 위반 예제 54개에 새 경계 키를 더했다(대부분 `shared/lib/brand.ts`·`money.ts` 깊은 가져오기, 기능의 `zod`, `app`의 `@tauri-apps/api`). 지운 키는 없다. 아직 없는 규칙을 끄던 `undisableable-boundaries.ts`는 "쓰이지 않는 끄기"를 기대하게 바꾸고, 실제 위반이 있는 `undisableable-boundaries-real.ts`·`-unknown.ts`를 더했다 | ADR 0012의 같은 처리 | 예제 파일 내용 변경 | ADR 0012 |
| 경계 변형 테스트는 타입 정보 없는 가벼운 설정(같은 `boundaries.mjs` 함수)으로 돈다. 표의 원소, 요소, `partialMatch`, 패키지 경로, 정책, 옵션 하나를 바꾸거나 규칙을 끄거나 `warn`으로 내리면 어떤 예제가 실패해야 한다 | 타입 인식 설정으로는 변형 30여 개가 너무 느리다. 기본 설정과 가벼운 설정의 경계 키가 같은지도 확인한다 | 전체 설정으로 변형 | 실측 |
| 순환 검사: dependency-cruiser 대신 knip 6.45.1의 `cycles` 이슈 형식을 켰다. knip이 `just check`에 있어 순환도 `just check`에서 실패한다 | 새 의존성 없이 순환을 막는다. 메인 에이전트가 대체를 받아들였다(2026-11-12 수정 브리프) | dependency-cruiser + 잠금 파일 문법 완화(보호 파일 `harness/ts-deps/rules.mjs` 변경) | 실측 |
| knip 설정 `apps/desktop/knip.json`: `entry`는 `src/shared/*/index.ts`, `eslint/**/*.mjs`, `tests/*.test.mjs`(`main.tsx`는 Vite 플러그인이, `vitest.config.mjs`·`eslint.config.mjs`는 각 플러그인이 찾는다), `project`는 `src/**/*.{ts,tsx}`, 이슈 형식 전부를 `include`·`error`. 무시 목록은 `ignoreDependencies`(`@tauri-apps/cli` — 손으로 부르는 Tauri CLI, `@typescript/native` — `justfile`이 부르는 `tsc`)와 `ignoreBinaries`(`just` — 하네스 테스트가 부른다). 앱 실행은 `--treat-config-hints-as-errors` | knip은 `justfile`을 읽지 못한다. 무시 원소와 `entry` 원소 하나를 지우면 앱 검사가 실패하는지 테스트한다 | 저장소 루트 작업 공간 설정(보호된 하네스 스크립트의 미사용 내보내기가 범위 밖에서 보고된다) | R-08 7.5, 실측 |
| `shared/*/index.ts`를 진입점으로 둔다 | 공유 요소는 앞으로 쓸 공개 API를 지금 갖고 있다(`shared/lib`의 브랜드·금액·`Result`). 진입점이 아니면 지금 코드 전부가 미사용으로 실패한다 | 공개 API까지 미사용 검사(`includeEntryExports`) | 실측 |
| Vitest 설정 `apps/desktop/vitest.config.mjs`: `include: src/**/*.test.ts(x)`, `allowOnly: false`, `passWithNoTests: true`. 판정은 `harness/vitest-check/check.mjs`가 JSON 결과로 한다: `skipped`·`pending`·`todo` 결과나 대기 합계가 하나라도 있으면 실패, 실패·종료 코드 0 아님은 실패, `src`에 테스트 파일이 있는데 통과한 테스트가 0개면 실패, 테스트 파일이 없으면 통과 | 브리프 7. 테스트 파일이 없을 때만 0개를 허용하려면 `passWithNoTests`가 필요하고 판정은 실행기가 한다 | `passWithNoTests: false`(지금 테스트 파일이 없는 상태가 실패) | 브리프 |
| 앱에 첫 테스트 `src/shared/lib/result.test.ts`를 더했다 | 실제 앱 실행 경로(파일 1개 이상, 0개 실행 판정, 앱 린트·`tsc` 통과)를 확인한다 | 테스트 없음 | 브리프 7 |
| TS 커버리지 기준은 강제하지 않는다 | harness 6.1 UI 행은 "로딩·빈 상태·오류·정상 상태 테스트"만, 6.3은 Rust 대상만 적었다. **문서에 TS 커버리지 수치 기준이 없다** | 임의 수치 | harness 6.1, 6.3 |
| 예제 묶음: 경계는 `harness/eslint-violations`(`경계` 행), knip은 `harness/knip-check/cases/<이슈 형식>/`(앱 `knip.json`을 그대로 `--config`로 쓴다), Vitest는 `harness/vitest-check/cases/<형태>/`(앱 설정을 그대로 쓴다). 각 묶음은 설정이 기대값과 정확히 같은지, 폴더 목록이 기대 목록과 같은지 확인한다 | harness 6.8 | - | 브리프 9 |

## 리뷰 반영 (2026-11-12)

| 결정 | 근거(재현된 우회) |
|---|---|
| `src` 안의 심볼릭 링크를 금지한다. `harness/source-tree/check.mjs`가 `lstat`으로 `apps/desktop/src`를 훑어 링크(파일·폴더)마다 실패를 보고하고, `just check`에서 `eslint src`보다 먼저 돈다(ESLint는 폴더 링크를 만나면 `EISDIR`로 죽는다). 예제는 실행 중에 임시 폴더에 만든다(파일 링크, 폴더 링크, 깊은 폴더 링크, 링크 없음). 저장소에는 링크 예제를 둘 수 없다(`xtask source-check`가 저장소 안의 모든 심볼릭 링크를 실패시킨다) | `src` 안의 링크로 다른 기능의 내부 파일을 끌어와도 모든 검사가 통과했다 |
| 기능 내부 구조: `features/<이름>/` 바로 아래에는 `ui/`, `model/`, `index.ts`, `messages.ts`만(`FEATURE_ENTRIES`, `local/forbidden-file`의 `featureLayout`). 요소 `feature-ui`(`src/features/*/ui`), `feature-model`(`src/features/*/model`)을 나누고 같은 기능 안에서 `index.ts` → `ui`·`model`, `ui` → `model`, `ui` → `messages.ts`만 허용한다(`FEATURE_DEPENDENCIES`). `model` → `ui`, `model` → `messages.ts`, `ui` → 자기 `index.ts`는 실패 | architecture 5장 `ui/ · model/ · index.ts`. `messages.ts`는 harness 4.2의 문구 파일 이름 규칙. `ui`·`model` 방향은 문서에 없어 브리프대로 `ui → model`만 |
| 패키지 위치 표에 `react-dom` → `src/app/main.tsx` 한 줄 | `react-dom/client`는 진입 파일에서만 |
| `zod` 위치 예제: 위반 예제 `src/shared/lib/money.ts`가 `zod`를 다시 내보내 실패한다. 표를 `src/shared/lib/*.ts`로 넓히면 예제가 실패하는 변형 테스트 | `money.ts`의 `zod`가 고정되지 않았다 |
| `guard/no-import-meta`(`import.meta` 전체), `guard/no-jsx-spread`(`shared/**` 밖의 JSX 스프레드), `guard/no-module-mocks`(`vi.mock`·`vi.doMock`·`vi.spyOn` 호출과 구조 분해, `shared/ipc/testing` 밖), `guard/no-test-skip`에 `each`·`for` 표가 비어 있지 않은 배열 리터럴이 아니면 실패, `local/test-file-boundary`(테스트 파일의 내보내기, 테스트 파일 가져오기). 모두 끌 수 없다 | `import.meta.glob`, `{...props}`로 `ref`·`children` 넘기기, 모듈 가짜, 빈 표, 테스트 파일을 통로로 쓰기 |
| `justfile`에 `unexport ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH`. 테스트가 `justfile`을 가져오는 임시 `justfile`에서 변수가 지워지는지 확인한다(기존 `TAURI_CONFIG` 테스트 방식) | 환경 변수가 `boundaries/root-path` 설정보다 앞선다 |
| 다른 규칙의 대조 예제(`features/<이름>/*.ts`의 `allowed` 등)는 이제 기능 구조와 깊은 가져오기로 실패한다. 같은 내용을 유효한 위치(`model/`, `ui/`, `features/greeting/messages.ts`)에 두고 `shared/lib/index.ts`로 가져와 문제 0개를 기대하는 예제를 더했다. 기존 예제는 지우지 않고 새 키를 더했다(126개에 `featureLayout`, 7개에 `react-dom`). `check.mjs`가 "행마다 문제 0개 대조 예제가 있다"를 확인한다 | 대조 예제가 경계 오류를 기대하고 있었다 |
| 운영 코드 기준 knip: `knip.json`의 `project`에 `!`를 붙이고, 개발용 진입점(`!` 없는 `entry`)을 뺀 설정으로 `--strict`(운영 모드를 포함한다)를 한 번 더 돈다. 결과는 보호된 `harness/knip-check/production-allowlist.json`의 정확한 항목(와일드카드 없음, 항목마다 이유)과 같아야 하고, 목록에 남았지만 보고되지 않는 항목도 실패한다. 예제 `harness/knip-check/production-cases/`(테스트에서만 쓰는 내보내기, 테스트에서만 쓰는 파일, `shared/*/index.ts`의 미사용 내보내기, 문제 없음) | 테스트에서만 쓰이는 운영 코드와 `shared/*/index.ts`의 아무 내보내기가 통과했다 |
| 지금 운영 기준 knip이 보고하는 것(뼈대, "첫 사용처가 생길 때까지"): 의존성 `zod`, 파일 `src/shared/ds/index.ts`, `src/shared/ipc/index.ts`, `src/shared/lib/index.ts`, `brand.ts`, `messages.ts`, `money.ts`, `result.ts`. 파일 단위로 보고되므로 그 안의 내보내기는 첫 사용처가 생긴 뒤에 하나씩 판정된다 | 실측 |
| `--include-entry-exports`, `--production`은 넣지 않았다 | `--strict`가 운영 모드를 켠다(`create-options.js`). 운영 설정에는 개발용 진입점이 없어 `--include-entry-exports`를 빼도 예제 결과가 같았다. Vite가 `index.html`에서 찾는 `main.tsx`의 내보내기는 보지 않는다 |
| Vitest: `expect.requireAssertions: true`(단언 없는 테스트 실패), 0개 실행 판정을 테스트 파일마다(통과한 테스트가 없는 `*.test.ts(x)` 파일이 하나라도 있으면 실패). 예제 `no-assertion`, `empty-each`, `empty-file-beside` | 단언 없는 테스트와 빈 표가 통과했다 |
| `test.fails`·`it.fails`는 린트(`guard/no-test-skip`, 기존 예제)로만 막는다 | Vitest JSON 결과에서 `fails` 테스트는 `passed`라 실행 결과로 가를 수 없다 |
| 잠금 파일 줄 문법(`harness/ts-deps/rules.mjs`, 메인 승인)을 넓혔다: `||` 주변의 선택적 공백, 공백으로 이은 비교식(`^14 <16`, 이어지는 조각은 `<>=^~`나 숫자로 시작), 쉼표로 이은 여러 키의 흐름 맵. 맨 이름 끝의 `:`는 이제 실패한다. 흐름 맵이 넓어진 대신 `packages`의 모든 `resolution`이 sha512 `integrity` 하나인지 따로 확인한다(`tarball`이 계속 실패) | dependency-cruiser처럼 정상 패키지가 막혔다 |

### 끌 수 있는 금지와 끌 수 없는 금지 (ADR 0009 표에 더함)

| 끌 수 없음 | 끌 수 있음 |
|---|---|
| `boundaries/dependencies`, `boundaries/no-unknown-files`, `boundaries/no-unknown-dependencies`(`boundaries/*`, 이미 목록에 있다). 패키지 위치 제한도 `boundaries/dependencies`라 끌 수 없다. 리뷰 반영으로 `guard/no-import-meta`, `guard/no-jsx-spread`, `guard/no-module-mocks`, `local/test-file-boundary`를 더했다(기존 테스트의 고정 목록에 네 이름을 더했다. 지운 원소 없음) | 새로 더한 것 없음 |

## 결과

- `just check`의 TS 단계에 `harness/knip-check/check.mjs`, `harness/vitest-check/check.mjs`를 더했다. 테스트는 `apps/desktop/tests/{boundaries,knip,vitest}.test.mjs`.
- 원리상 못 막는 것: `index.ts`가 요소 안의 모든 것을 다시 내보내면 깊은 가져오기 금지는 이름만 남는다(리뷰 항목). 같은 요소 안(`ui` 안, `model` 안, 같은 `shared/*` 안)의 세부 방향은 평가하지 않는다. knip `cycles`는 타입만 가져오는 순환(`import type`)을 잡지 못한다. `"src/…"` 맨 이름과 경로 별칭은 린트가 외부 패키지로 보고 통과시킨다(`KNOWN_GAP` `src/features/gaps/bare-src-gap.ts`, `tsc`와 tsconfig 고정 테스트가 막는다). Vite `import.meta.glob`은 boundaries가 보지 못해 `guard/no-import-meta`로 막는다. 동적 `import()`는 `guard/no-dynamic-import`가 막는다. 환경 변수 `ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH`는 설정보다 앞서지만, 잘못된 값이면 모든 파일이 요소 밖이 되어 `no-unknown-files`로 실패한다. knip: 진입 파일(`shared/*/index.ts`, 설정·테스트 스크립트)의 미사용 내보내기, `subpaths`(예제 없음). Vitest: 테스트 안에서 단언을 하지 않는 테스트, 테스트 파일이 아닌 곳에 둔 테스트.
- 범위 밖으로 남긴 것: `apps/desktop/vitest.config.mjs`와 `knip.json`은 스크립트 린트 명령(기존 테스트가 줄을 고정한다)에 들어 있지 않다. `harness/tsc-violations`에 맨 이름 가져오기 예제를 더하지 않았다. `shared/ipc/testing`을 별도 요소로 나눌지는 U-04 몫이다(지금은 `ipc`의 내부라 다른 요소의 테스트가 가져오면 깊은 가져오기로 실패한다).
- 기존 `KNOWN_GAP` 예제 `src/features/raw/local-value-gap.ts`는 깊은 가져오기 때문에 이제 `GAP?`로 출력된다. 빈틈이 닫힌 것이 아니라 덤 오류다.
- 뒤 작업으로 넘긴 것: H-29 — TS 테스트 파일의 삭제·약화 검사. DS 작업 — CSS 가져오기에 `vite/client` 타입이 필요해질 때 `import.meta` 금지와 함께 다시 본다(필요한 곳은 파일 허용 목록). DS-09 — UI 라이브러리 위치(패키지 위치 표에 한 줄씩). DS-15·DS-07·U-04 — 브라우저 모드, Storybook, Testing Library·`mockCommands`. H-29 — `vitest list` 개수 비교.
- 성능: TS 하네스 테스트는 `node --test`가 파일마다 병렬로 돌려 가장 느린 기존 `local-allowlists`(약 30초)가 벽시계 시간을 정한다. 이 작업의 테스트(`boundaries` 약 15초, `knip`·`vitest` 각 약 10초)는 그보다 짧아 줄이지 않았다.
- 바뀌면 다시 볼 조건: boundaries가 경고를 ESLint 메시지로 내게 되면 S-19 테스트를 바꾼다. knip이 `cycles`를 기본 포함하거나 형식 이름을 바꿀 때.
