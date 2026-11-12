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
| 속도 | `just check` 전체 | 71초(첫 실행). 그중 `eslint src`는 약 2초, 위반 예제는 약 2초 |
| `rootDir` | `rootDir: "src"`인 프로젝트에서 `src` 밖 `.ts`를 가져오기 | TS 7·TS 6 모두 TS6059. 같은 프로그램에 구문 오류가 있으면 TS 6은 TS6059를 내지 않아, 위반 예제를 별도 프로젝트(`harness/tsc-violations/root-dir`)로 둔다 |
| 예제의 React 타입 | 예제 폴더에서 `react`를 가져오기 | `harness/` 아래에서는 `node_modules`를 찾지 못해 React 예제에 `no-unsafe-*`가 덤으로 붙었다. 예제 `tsconfig.json`의 `paths`를 앱의 `node_modules/@types/*`, `node_modules/*` 순서로 두면 앱과 같은 타입이 잡혔다(반대 순서는 `react`의 JS 파일에서 멈춘다) |
| `packageManager` 해시와 잠금 파일 | 해시를 넣고 `pnpm install --frozen-lockfile` | 잠금 파일 변화 없이 통과 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 설정은 `apps/desktop/eslint.config.mjs`(JS)와 보조 파일 `apps/desktop/eslint/guard-plugin.mjs`(지시문 형태 규칙과 내장 규칙 별칭), `apps/desktop/eslint/bans.mjs`(금지 목록). 검사 대상은 `src/**` | TS 설정 파일은 `jiti` 의존성이나 불안정 플래그가 필요하다. 설정에는 타입 자리가 없어 원시 타입 금지와 부딪히지 않는다. 기존 하네스 스크립트도 `.mjs`다 | `eslint.config.ts` + `jiti` | 실측 |
| 인라인 지시문: `noInlineConfig: false`, `reportUnusedDisableDirectives`·`reportUnusedInlineConfigs: "error"`, `eslint-comments/no-use`(`eslint-disable-next-line`만), `require-description`, `no-unlimited-disable`, `no-restricted-disable`, `no-duplicate-disable`, 보조 규칙 `guard/disable-directive-form` | harness 2.1이 한 형태를 허용한다. 줄 주석·`next-line`·규칙 하나·이유를 한 번에 보는 기존 규칙이 없어 `SourceCode#getDisableDirectives()`로 규칙을 만들었다 | R-08의 `noInlineConfig: true`(허용 형태까지 막는다) | harness 2.1, 실측 |
| 금지 하나에 규칙 이름 하나. `no-restricted-syntax`·`-globals`·`-properties`·`-imports`를 `guard/<금지>` 이름으로 따로 등록한다(아래 표). 핵심 `no-restricted-globals`에는 암묵 전역(`event`, `name`, `length`, `status`, `globalThis`, `window`, `self`)만 남긴다 | 끄기 한 줄이 여러 금지를 함께 풀지 못하게 한다. `no-restricted-disable`은 이름으로만 막는다 | `no-restricted-syntax` 하나에 모두 넣기 | 리뷰, harness 2.1 |
| 금지 목록의 항목마다 고유 메시지를 둔다. 예제 판정은 "보고된 (규칙, 항목 메시지) 집합이 기대 집합과 정확히 같음"이다. `check.mjs`는 유효 설정에서 모든 목록 항목을 모아 항목마다 그것을 기대하는 예제가 있는지 확인한다 | 항목 하나를 지우거나 새 항목을 예제 없이 넣으면 실패한다. 덤 오류도 드러난다 | 규칙 ID 포함 여부만 비교(첫 판) | 리뷰 |
| 프리셋이 `warn`으로 둔 규칙은 내보내기 전에 모두 `error`로 올린다 | harness 4.2 "모두 오류로 둔다" | `--max-warnings 0`에만 기대기 | 실측 |
| 위반 예제는 `harness/eslint-violations/src/**`(앱 `src`와 같은 폴더 구조), `cases.json`(예제별 기대 집합, `rows`에 행별 필수 규칙), 실행기 `check.mjs`. 예제 폴더 `tsconfig.json`은 앱 설정을 상속하고 `rootDir`, `include`, 모듈 탐색 `paths`(앱의 `node_modules`)만 둔다. 실행기가 이 내용을 고정한다 | 앱 설정 파일을 그대로 읽어 규칙을 복사하지 않는다. 같은 타입 환경이라 React 예제에 덤 오류가 없다 | 예제를 `apps/desktop/src`에 잠깐 쓰기, 가상 경로 `lintText`(타입 인식 실패), `node_modules` 심볼릭 링크(`.gitignore`가 보호 파일) | 브리프, 실측 |
| 앱 `tsconfig`에 `rootDir: "src"` | `src` 밖 `.ts`를 가져오면 `tsc`가 실패한다 | ESLint 경계 규칙만(H-09) | 리뷰 |
| 금지 확장자 파일(`.js`·`.jsx`·`.mjs`·`.cjs`·`.mts`·`.cts`·`.d.ts`)은 `files` 블록으로 검사 대상에 넣고 `guard/forbidden-extension`으로 실패시킨다. 그 블록은 `noInlineConfig: true`. ESLint가 보지 않는 파일(`.css` 등)은 테스트가 `src` 아래 모든 파일이 TS 검사 대상인지 확인한다 | `local/forbidden-file`은 H-08 몫이다 | H-08까지 비워 두기 | harness 4.2, R-08 2.2 |
| 효과 훅(`useEffect`, `useLayoutEffect`, `useInsertionEffect`, `useImperativeHandle`)과 `useSyncExternalStore`는 가져오기가 아니라 이름(식별자·문자열·템플릿) 자체를 막는다. `react` 기본·네임스페이스·`{ default as R }` 가져오기도 막는다 | 별칭, 다시 내보내기, `R["useEffect"]`가 모두 이름을 거친다 | `no-restricted-imports`의 `importNames` | 실측 |
| `ref`에는 식별자만(함수·호출·조건식·문자열 금지), JSX 스프레드 안의 `ref` 금지. `shared/ds`와 `shared/platform` 허용 목록 파일 밖 전체 | ref 콜백은 마운트 효과 우회다 | `features`의 화살표 함수만 | 리뷰 |
| 브라우저 전역(`document`, `navigator`, `location`, `localStorage`, `sessionStorage`, `indexedDB`)은 `shared/ds`와 `shared/platform` 허용 목록 파일 밖에서 금지. `src/app/main.tsx`의 `document.getElementById`는 `shared/platform/index.ts`의 `rootElement`로 옮겼다 | 리뷰 13번 | - | 리뷰 |
| `open`은 13번 목록이 아니라 네트워크 쪽 `guard/no-global-object`에 넣어 `shared/ds`에서도 금지한다 | 12번(네트워크)과 13번(전역)이 겹친다. 더 엄격한 쪽을 택했다 | `shared/ds`에서 허용 | 리뷰 |
| 숫자 전역(`Number`, `Math`, `parseFloat`, `parseInt`, `BigInt`)은 `features`, `app`, `shared/ds`(차트 허용 목록 제외)에서 참조 자체를 금지. 단항 `+`는 모든 파일에서 금지 | `const N = Number`, `const { round } = Math` 별칭 | 호출·멤버 선택자 | 리뷰 |
| 스키마 `.parse()`·`.parseAsync()`는 멤버 이름(`s.parse`, `s["parse"]`, `` s[`parse`] ``)과 구조 분해(`const { parse } = s`)를 막는다. `JSON.parse`는 따로 `guard/no-json-parse` | 리뷰 11번 | 호출 선택자만 | 리뷰 |
| 테스트 건너뛰기: 어떤 객체든 `.skip`·`.only`·`.todo`·`.skipIf`·`.runIf`(계산된 이름, 구조 분해 포함), 테스트 함수의 옵션 객체 `{ skip | only | todo | fails | concurrent }`, 테스트 객체의 `.fails`·`.concurrent`, `xit`·`xtest`·`xdescribe`·`fit`·`fdescribe` 식별자 | 리뷰 7번 | 정해진 객체 이름만 | 리뷰 |
| 네트워크: `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `sendBeacon`, `defaultView`, `Worker`, `SharedWorker`, `RTCPeerConnection`, `WebTransport`, `importScripts`(식별자와 문자열 속 단어), react-dom `preload`·`preinit`·`preconnect`·`prefetchDNS`·`preloadModule`·`preinitModule`(가져오기·다시 내보내기·멤버). 전역 객체 `globalThis`, `window`, `self`, `top`, `parent`, `frames`, `opener`, `open`, `eval`, `Function` | 리뷰 12번 | - | 리뷰 |
| 테스트·하네스 스크립트(`apps/desktop/tests/**/*.mjs`, `apps/desktop/eslint/**/*.mjs`, `apps/desktop/*.mjs`, `harness/**/*.mjs`, 위반 예제 제외)는 `apps/desktop/eslint.scripts.config.mjs`의 가벼운 블록(JS 권장 규칙, 테스트 건너뛰기, 지시문 형태)으로 저장소 루트에서 검사한다. `no-unused-vars`는 나머지 형제(`const { a: _, ...rest }`)를 허용한다 | 기존 테스트가 이 형태로 속성을 뺀다. `src/**/*.test.ts(x)`는 앱 설정을 그대로 받는다(테스트로 확인) | 앱 설정 공유(타입 인식 규칙이 JS에 맞지 않는다) | 리뷰 16번 |
| `prefer-immutable-types`는 매개변수에만 | harness 4.2 "매개변수만" | R-08 초안 | harness 4.2 |
| 예외는 파일 허용 목록으로만: `shared/platform`(처음 목록 4개), 타입 술어(`shared/lib/money.ts`), `throw`(빈 목록), 차트 숫자(빈 목록). `@tauri-apps/api/mocks`는 문서대로 `shared/ipc/testing` 폴더에서만 허용 | harness 4.2 | - | harness 4.2 |
| 추가 플러그인 버전: `@eslint/js` 10.0.1, `eslint-plugin-react-hooks` 7.1.1, `@eslint-react/eslint-plugin` 5.30.1, `@eslint-community/eslint-plugin-eslint-comments` 4.8.1 | harness 4.3에 없고 R-08 6.1에서 돌린 버전 | 최신판 | R-08 6.1 |
| TS 의존성 허용 목록 `harness/ts-deps/allowlist.json`(보호 경로), 검사 `harness/ts-deps/check.mjs`. 매니페스트마다 절별로 이름, 설치 대상(`target`), 이유를 적는다. 값은 `x.y.z`(대상이 같은 이름) 또는 `npm:<target>@x.y.z`만 통과하고 `file:`·`link:`·git·URL·`workspace:`·범위는 실패. 최상위 키 목록, `packageManager`(sha512 해시 필수), `pnpm-workspace.yaml`·`.npmrc` 내용 일치, `.pnpmfile.*`·하위 `.npmrc` 없음, 잠금 파일 최상위 키, `packages`의 `resolution`이 sha512 `integrity` 하나, `importers`의 specifier가 매니페스트 값과 정확히 같음 | 이름만 보면 `"react": "npm:left-pad@1.3.0"`이 통과했다(리뷰 2~4번) | 이름만 | 리뷰, harness 2장 |
| `packageManager`는 `pnpm@12.18.0+sha512.9d78…beb3`(npm `dist.integrity`의 16진수). 기존 테스트 `toolchain.test.mjs`의 단언을 "정확한 버전 + sha512 해시"로 바꿨다(메인 에이전트 승인) | ADR 0006 남은 일 | - | 승인 |
| 잠금 파일과 `pnpm-workspace.yaml`은 줄 단위 문법(2칸 들여쓰기 매핑, `- ` 목록, 작은따옴표 값, `@`로 시작할 때만 따옴표 친 키, `{}`·한 쌍 흐름 매핑·흐름 목록, `---` 두 번)으로만 읽고, 문법 밖의 줄(탭, 큰따옴표, 앵커·별칭, 주석, 블록 스칼라, 세 번째 문서 등)이 하나라도 있으면 실패한다. `importers`의 `version`은 정확한 레지스트리 버전(peer 접미사 허용)이고 허용 목록의 대상과 맞아야 하며, 그 `packages:` 항목이 sha512 `integrity` 하나를 가져야 한다. 첫 문서는 pnpm 고정만 허용 | 정규식 줄 읽기가 실제 YAML 해석과 어긋나거나 `version: link:` 등이 통과할 수 있었다(자동 보안 리뷰 2건). 모르는 형태는 통과가 아니라 실패로 둔다 | YAML 파서 의존성 추가 | 리뷰 |

### 끌 수 있는 금지와 끌 수 없는 금지

| 끌 수 없음 (`no-restricted-disable`에 이름으로 나열) | 끌 수 있음 (2.1 형태) |
|---|---|
| `guard/no-effect-hooks`, `guard/no-sync-external-store`, `guard/no-ref-callback` | `guard/no-enum`, `guard/no-namespace`, `guard/no-declare`, `guard/no-overload` |
| `guard/no-type-predicate` | `guard/no-default-case`, `guard/no-default-export`, `guard/no-export-all`, `guard/no-dynamic-import` |
| `guard/no-number-conversion`, `guard/no-unary-plus` | `guard/no-throwing-parse`, `guard/no-json-parse` |
| `guard/no-reflect`, `guard/no-prototype`, `guard/no-call-apply-bind`, `guard/no-own-symbols` | `guard/no-react-namespace`, `guard/no-react-subpath`, `guard/no-tauri-mocks` |
| `guard/no-network`, `guard/no-global-object` | `guard/no-object-mutation`, `guard/no-proxy`, `no-restricted-globals`(암묵 전역) |
| `guard/no-test-skip`, `guard/forbidden-extension`, `guard/no-dom-globals` | 그 밖의 typescript-eslint·functional·React 규칙 |
| `guard/disable-directive-form`, `@typescript-eslint/ban-ts-comment`, `@eslint-community/eslint-comments/{no-use, require-description, no-unlimited-disable, no-restricted-disable, no-duplicate-disable}` | |
| `local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`, `boundaries/*`(뒤 작업이 만든다) | |

## 결과

- `just check`의 TS 단계: `check-node` → `pnpm install --frozen-lockfile` → `harness/ts-deps/check.mjs` → Prettier → `tsc` → `eslint --max-warnings 0 src`(캐시 없음) → `harness/eslint-violations/check.mjs` → 스크립트 린트 → `vite build` → tsc 위반 예제(`root-dir` 프로젝트 포함) → 하네스 테스트.
- 알려진 빈틈(`KNOWN_GAP`, 막히면 `GAP?`로 출력): 이름 있는 함수를 `ref`에 넘기기, 던지는 내장 함수(`new URL`), 문자열을 조합해 만든 속성 이름(`` document[`${a}${b}`] ``). 문자열 조합 키는 리뷰 항목이다. `{...props}`처럼 식별자를 펼쳐 `ref`를 넘기는 것도 막지 못한다. `// eslint-disable-next-line a, -- r`은 ESLint가 지시문을 해석하다 죽어 실패로 끝나므로 그대로 둔다.
- 뒤 작업으로 넘긴 것: H-08 — `local/no-comments`(지시문이 아닌 줄 주석, `///` 지시문, JSX 주석), `local/forbidden-file`(허용 목록 밖 `shared/platform` 파일의 존재 자체, 대문자 확장자 `.TS`·`.JS`), `local/no-money-coercion`, `.catch`·두 인자 `.then` 금지와 그 파일 허용 목록. U-03 — CSP `connect-src`·`img-src`·`worker-src`와 외부 이동 거부. DS-10 — DS 컴포넌트의 `ref` prop은 `RefObject`만. Vitest 설정 작업 — 건너뛴 테스트 0건 확인과 `allowOnly: false`. H-38 — `~~x`, `x | 0`, 산술.
- pnpm 9.12.3의 버전 전환은 `packageManager`의 해시를 확인하지 않는다. 해시를 확인하는 것은 corepack과 이 검사뿐이다.
- 바뀌면 다시 볼 조건: typescript-eslint가 TS 7 백엔드를 지원할 때, `@eslint-community/eslint-comments`에 규칙 개수 제한이 생길 때, H-08의 `local/forbidden-file`이 생길 때(`guard/forbidden-extension`을 합칠지 판단).
