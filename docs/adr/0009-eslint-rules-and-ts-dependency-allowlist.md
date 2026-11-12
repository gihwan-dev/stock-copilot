# ADR 0009: ESLint 규칙 설정과 TS 의존성 허용 목록 (H-07, H-24, H-10)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-07, H-24, H-10
- 반영한 문서: 없음 (harness.md 2.1, 4.2, 4.3, 6.8과 R-08 7.2를 따르고, 어긋나는 곳만 여기 적는다)

## 맥락

harness.md 4.2는 ESLint 규칙을 행별로 정하고 설정 초안을 R-08 7.2로 가리킨다. R-08 초안은 `noInlineConfig: true`로 모든 인라인 지시문을 막았지만, 그 뒤 사용자 결정(harness 2.1)으로 `// eslint-disable-next-line <규칙 하나> -- <이유>` 한 형태는 허용하게 되었다. 로컬 규칙(`local/*`)은 H-08·H-38, 경계(`boundaries/*`)는 H-09의 몫이라 이 작업에서는 없다. ADR 0006은 `packageManager`의 무결성 해시를 H-10으로 넘겼다.

## 실측

환경: macOS 26 (Apple Silicon), Node 24.22.0, pnpm 12.18.0, ESLint 10.14.0, typescript-eslint 8.72.2(TS 6.0.2 API).

| 항목 | 명령 | 결과 |
|---|---|---|
| 고정 버전 설치 | `pnpm -C apps/desktop add -D --save-exact eslint@10.14.0 typescript-eslint@8.72.2 eslint-plugin-functional@10.0.1 @eslint/js@10.0.1 eslint-plugin-react-hooks@7.1.1 @eslint-react/eslint-plugin@5.30.1 @eslint-community/eslint-plugin-eslint-comments@4.8.1` | 모두 npm에 있고 피어 범위가 맞았다. 최소 공개 기간 예외는 생기지 않았다 |
| 문서의 규칙 이름 | 설치한 플러그인의 `rules` 목록 | harness 4.2와 R-08 7.2의 규칙 이름이 모두 있었다. 이름이 바뀐 규칙은 없다 |
| TS 설정 파일 | ESLint 10.14.0 `config-loader.js` | `eslint.config.ts`는 `jiti`(새 의존성)나 `unstable_native_nodejs_ts_config` 플래그가 있어야 읽는다 |
| 가상 경로 `lintText` | `apps/desktop/src/...` 경로로 디스크에 없는 예제를 `lintText` | 타입 인식 규칙에서 "was not found by the project service" 파싱 오류 |
| 예제 폴더를 같은 설정으로 | `new ESLint({ cwd: harness/eslint-violations, overrideConfigFile: apps/desktop/eslint.config.mjs })` | `overrideConfigFile`이면 기준 경로가 `cwd`라서 `src/**` 경로 조건이 예제의 `src/**`에 그대로 맞는다. 예제 경로와 앱 경로의 유효 설정(`rules`, `linterOptions`)이 177개 경로 모두 같았다 |
| 인라인 지시문 | `/* eslint-disable */`, `/* eslint x: off */`, `// eslint-disable-line`, `/* global */`, 규칙 여러 개, 이유 없음 | `@eslint-community/eslint-comments`에는 "지시문 하나에 규칙 하나"를 강제하는 규칙이 없다. 줄 주석 `// eslint-disable`(next-line이 아닌 것)은 ESLint가 지시문으로 보지 않는다 |
| `noInlineConfig: true`인 파일의 지시문 | `.js`에 `// eslint-disable-next-line` | 지시문이 무시되고 "has no effect" 경고가 난다 |
| 끌 수 없는 규칙을 끄기 | `no-restricted-disable` | 위반은 지시문 때문에 사라지지만 `no-restricted-disable`가 지시문 줄에서 오류를 낸다 |
| 프리셋의 `warn` 규칙 | 위반 예제 | `functional/prefer-tacit`, `@eslint-react/no-unnecessary-use-prefix`가 경고로 남았다 |
| `packageManager` 해시 | `pnpm@12.18.0+sha512.<틀린 해시>`로 바꾸고 빈 `PNPM_HOME`에서 `pnpm --version` | 전역 pnpm 9.12.3의 버전 전환이 해시를 확인하지 않고 12.18.0을 받아 실행했다(`parsePackageManager`가 `+` 뒤를 버린다) |
| 속도 | `just check` 전체 | 71초. 그중 `eslint src`는 약 2초, 위반 예제 177개는 약 1.5초 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 설정은 `apps/desktop/eslint.config.mjs`(JS)와 보조 파일 `apps/desktop/eslint/guard-plugin.mjs`. 검사 대상은 `src/**` | TS 설정 파일은 `jiti` 의존성이나 불안정 플래그가 필요하다. 설정에는 타입 자리가 없어 원시 타입 금지와 부딪히지 않는다. 기존 하네스 스크립트도 `.mjs`다 | `eslint.config.ts` + `jiti` | 실측 |
| 인라인 지시문: `noInlineConfig: false`, `reportUnusedDisableDirectives`·`reportUnusedInlineConfigs: "error"`, `eslint-comments/no-use`(`eslint-disable-next-line`만), `require-description`, `no-unlimited-disable`, `no-restricted-disable`, `no-duplicate-disable`, 그리고 보조 규칙 `guard/disable-directive-form` | harness 2.1이 한 형태를 허용한다. 줄 주석·`next-line`·규칙 하나·이유를 한 번에 보는 기존 규칙이 없어 `SourceCode#getDisableDirectives()`로 15줄짜리 규칙을 만들었다 | R-08의 `noInlineConfig: true`(허용 형태까지 막는다) | harness 2.1, 실측 |
| 금지 확장자 파일(`.js`·`.jsx`·`.mjs`·`.cjs`·`.mts`·`.cts`·`.d.ts`)은 `files` 블록으로 검사 대상에 넣고 `guard/forbidden-extension`(`Program` 선택자)으로 실패시킨다. 그 블록은 `noInlineConfig: true` | `local/forbidden-file`은 H-08 몫이다. 이 블록이 없으면 `.jsx`·`.mts`·`.cts`는 검사되지 않는다. 지시문으로 끌 수 없게 한다 | H-08까지 비워 두기 | harness 4.2, R-08 2.2 |
| ESLint가 아예 보지 않는 파일(`.css`, 확장자 없음 등)은 `apps/desktop/tests/eslint-config.test.mjs`가 `src` 아래 모든 파일이 TS 검사 대상인지(`.gitkeep` 제외) 확인한다 | ESLint는 `files`에 맞지 않는 파일을 조용히 건너뛴다 | 설정만으로 막기(불가) | R-08 2.2 |
| 네트워크·테스트 건너뛰기·금지 확장자는 내장 규칙(`no-restricted-syntax`, `no-restricted-globals`)을 다른 이름(`guard/no-network`, `guard/no-global-object`, `guard/no-test-skip`, `guard/forbidden-extension`)으로 등록해 쓴다 | `no-restricted-disable`은 규칙 이름으로만 막는다. `no-restricted-syntax`를 통째로 끌 수 없게 하면 끌 수 있어야 하는 금지까지 묶인다 | `no-restricted-syntax` 하나에 모두 넣기 | harness 2.1 |
| 끌 수 없는 규칙 목록: `local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`, `boundaries/*`, `guard/*`, `@typescript-eslint/ban-ts-comment`, `@eslint-community/eslint-comments/*`. 디자인 시스템 규칙은 그 작업이 더한다 | harness 2.1 목록에 이 설정의 이름을 맞췄다. 지시문 규칙 자체(`eslint-comments/*`, `guard/disable-directive-form`)를 끄는 우회도 막는다 | 2.1 목록 그대로(지시문 규칙을 끌 수 있다) | harness 2.1 |
| 프리셋이 `warn`으로 둔 규칙은 설정을 내보내기 전에 모두 `error`로 올린다 | harness 4.2 "모두 오류로 둔다". 심각도를 `warn`으로 낮춰도 효과가 없다 | `--max-warnings 0`에만 기대기 | 실측 |
| 위반 예제는 `harness/eslint-violations/src/**`(앱 `src`와 같은 폴더 구조)와 `cases.json`(예제별 기대 규칙·메시지 조각, `rows`에 harness 4.2 행별 필수 규칙), 실행기 `check.mjs`. 예제 폴더 `tsconfig.json`은 앱 설정을 상속하고 `include`만 둔다 | harness 6.8의 `harness/violations/src`·`harness/lint-cases.json` 대신 브리프의 경로를 따랐다. 앱 설정 파일을 그대로 읽어 규칙을 복사하지 않는다 | 예제를 `apps/desktop/src`에 잠깐 써서 검사(실제 소스 폴더를 건드린다), 가상 경로 `lintText`(타입 인식 실패) | 브리프, 실측 |
| 기대값은 "그 규칙이 오류로 보고됨"이고, `no-restricted-*`처럼 한 규칙에 여러 금지가 있으면 메시지 조각까지 맞춘다. 테스트가 규칙마다 `warn`·`off`로 덮어써 해당 예제가 실패하는지, 메시지 하나를 빼면 실패하는지 확인한다 | 규칙 하나를 끄거나 선택자 하나를 지우면 드러나야 한다 | 규칙 ID만 비교 | harness 6.8 |
| `prefer-immutable-types`는 매개변수에만(`variables`, `returnTypes`는 `None`) | harness 4.2 "매개변수만" | R-08 초안(전역 `ReadonlyShallow` + 반환만 `None`) | harness 4.2 |
| `useSyncExternalStore`는 `features`만이 아니라 `shared/platform` 허용 목록 파일 밖 전체에서 금지 | architecture.md 5장 "허용 목록 파일에서만" | `features`에서만 | architecture.md 5장 |
| `react`의 `default` 가져오기를 `importNames`로도 막는다. `Literal` 이름(`R["useEffect"]`)도 효과 훅 선택자에 넣는다 | `import { default as R } from "react"`가 기본·네임스페이스 가져오기 금지를 피했다 | - | 실측 |
| 숫자 변환 금지는 `features`와 `shared/ds`(차트 허용 목록은 빈 목록)에 건다. `Number.*`, `BigInt.*`, `new Number()`도 막는다. 단항 `+`는 R-08처럼 모든 파일에서 막는다 | harness 4.2 "숫자" 행. `Number.parseFloat`는 호출 이름 선택자를 피한다 | `features`만 | harness 4.2, 실측 |
| `no-restricted-properties`에 `Object.getOwnPropertyDescriptor(s)`, `no-restricted-globals`에 `Reflect`·`Proxy`를 더한다 | 불투명 금액의 symbol 키를 꺼내는 다른 길이다. `const r = Reflect`는 속성 규칙을 피한다 | - | harness 4.2 "불변" |
| 테스트 건너뛰기: 어떤 객체든 `.skip`·`.only`·`.todo`·`.skipIf`·`.runIf`(계산된 이름 포함), `describe/it/test/suite/bench`의 `.fails`·`.concurrent`, `xit`·`fit` 계열 호출 | Vitest 문맥 객체의 `ctx.skip()`도 건너뛰기다 | 정해진 객체 이름만 | 실측 |
| 예외는 파일 허용 목록으로만: `shared/platform`(처음 목록 4개: 효과 훅·`useSyncExternalStore`·`no-expression-statements`·ref 함수), 타입 술어(`shared/lib/money.ts`), `throw`(빈 목록), 차트 숫자(빈 목록). `@tauri-apps/api/mocks`는 문서대로 `shared/ipc/testing` 폴더에서만 허용 | harness 4.2 | - | harness 4.2 |
| 추가 플러그인 버전: `@eslint/js` 10.0.1, `eslint-plugin-react-hooks` 7.1.1, `@eslint-react/eslint-plugin` 5.30.1, `@eslint-community/eslint-plugin-eslint-comments` 4.8.1 | harness 4.3에는 없고 R-08 6.1에서 실제로 돌린 버전이다 | 최신판 | R-08 6.1 |
| TS 의존성 허용 목록은 `harness/ts-deps/allowlist.json`(보호 경로), 검사는 `harness/ts-deps/check.mjs`. 작업 공간 매니페스트(`pnpm-workspace.yaml`에서 읽음)마다 직접 의존을 절(section)별로 이름과 이유로 적는다. 목록 밖 이름, 다른 절, 목록 밖 최상위 키(`pnpm`, `overrides`, `scripts` 등), 목록 밖 매니페스트, 잠금 파일의 `importers`가 작업 공간과 다르면 실패 | Rust의 `bans.allow`처럼 이름과 이유. 버전은 기존 테스트가 정확한 버전을 강제하고 잠금 파일이 무결성을 가진다 | 이름과 버전을 함께 적기(올릴 때마다 보호 파일 변경) | harness 2장 |

## 결과

- `just check`의 TS 단계: `check-node` → `pnpm install --frozen-lockfile` → `harness/ts-deps/check.mjs` → Prettier → `tsc` → `eslint --max-warnings 0 src`(캐시 없음) → `harness/eslint-violations/check.mjs` → `vite build` → tsc 위반 예제 → 하네스 테스트.
- 알려진 빈틈(`KNOWN_GAP`, 막히면 `GAP?`로 출력): 이름 있는 함수를 `ref`에 넘기기, 던지는 내장 함수(`new URL`). 문자열을 이어 붙여 만든 계산된 속성 이름(`document["default" + "View"]`)도 막지 못한다.
- H-08로 넘기는 것: 일반 주석 금지(`local/no-comments`: 지시문이 아닌 `// eslint-disable` 줄 주석, `///`, JSX 주석, 2.1 형태 밖 주석이 지금은 통과한다), `local/forbidden-file`(허용 목록 밖 `shared/platform` 파일의 존재 자체를 실패시키기. 지금은 그 파일에 일반 규칙이 걸릴 뿐이다), `local/no-money-coercion`.
- 남은 일(막힘): `packageManager`의 무결성 해시. 기존 테스트 `apps/desktop/tests/toolchain.test.mjs`의 "pnpm is pinned to an exact version"이 `^pnpm@\d+\.\d+\.\d+$`와 `pnpm@<버전>` 일치를 단언해 해시를 넣으면 실패한다. 기존 테스트 변경은 사용자 서명 커밋으로만 한다. 넣을 값은 `pnpm@12.18.0+sha512.9d7882d5dc82147ecfc4e062aa34834e0ad0296710a1837a1115ae012127d36b7109e321ac1598a1dd0a65597681b1e99917e12ed0d749eae91187943363beb3`(npm `dist.integrity`의 16진수)이고, 그때 `allowlist.json`의 `packageManager`를 같은 값으로 바꾸면 검사가 해시를 요구한다. 위 실측대로 pnpm 9.12.3의 버전 전환은 해시를 확인하지 않으므로, 해시를 실제로 확인하는 것은 corepack이나 이 검사뿐이다.
- 바뀌면 다시 볼 조건: typescript-eslint가 TS 7 백엔드를 지원할 때, `@eslint-community/eslint-comments`에 규칙 개수 제한이 생길 때(보조 규칙을 뺀다), H-08의 `local/forbidden-file`이 생길 때(`guard/forbidden-extension`을 합칠지 판단).
