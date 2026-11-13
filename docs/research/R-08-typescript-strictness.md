# R-08 TypeScript 엄격 도구 구성

조사일: 2026-11-11. 모든 버전과 사실은 이 날짜에 확인한 것이다. 실험은 작은 예제 프로젝트(아래 "실행 검증")에서 실제로 돌렸다.

## 1. 요약

1. 타입 검사는 **TypeScript 7.0.2(`tsc`, Go 네이티브)** 로 하고, 타입 정보를 쓰는 린트는 **TypeScript 6.0.x API + typescript-eslint 8.72.2** 로 한다. 이유: TS 7.0에는 프로그램용 API가 없고 typescript-eslint의 지원 범위가 `typescript >=4.8.4 <6.1.0`이다. 두 버전은 npm 별칭으로 함께 설치한다(TS 팀 공식 권장 방식).
2. 강제하려는 항목은 **ESLint + typescript-eslint + eslint-plugin-functional + eslint-plugin-boundaries + 로컬 규칙 3개(주석 금지, 금액 산술 금지, 금지 파일)** 로 모두 막을 수 있었다. 위반 예제 94개 중 의도한 91개가 실패하고, 알려진 빈틈 3개만 통과했다. `tsc` 위반 예제 21개는 TS 7과 TS 6에서 같은 오류 코드로 실패했다.
3. 빈틈 없이 막으려면 설계 몇 곳을 바꿔야 한다. 금액은 문자열 브랜드가 아니라 **비공개 symbol 키를 가진 불투명 객체**로 표현해야 한다(문자열 브랜드는 `parseFloat`와 `+`를 `tsc`가 통과시킨다). `shared/platform`은 **파일 허용 목록**으로 잠가야 한다. `useSyncExternalStore`, `call`/`apply`/`bind`, `prototype`, `declare`, 오버로드도 함께 막아야 한다.
4. 무시된 Result는 Result 전용 린트 없이 막힌다. `functional/no-expression-statements`(`ignoreVoid`), `no-floating-promises`(`ignoreVoid: false`), `no-void`, `no-unused-vars`를 함께 켜면 직접 만든 Result에도 neverthrow에도 똑같이 걸린다. 그래서 **Result는 직접 정의한 판별 유니온**을 추천한다.
5. oxlint(타입 인식, tsgolint)는 빠르지만 eslint-plugin-functional·boundaries·타입 정보를 쓰는 사용자 규칙을 아직 돌릴 수 없다. Biome은 자체 타입 추론의 범위가 부분적이다. 지금은 **ESLint 하나를 기준**으로 삼는다.

## 2. 질문별 답

### 2.1 TypeScript 최신 버전의 엄격 옵션 전체

현재 버전 (npm 레지스트리, 2026-11-11 확인):

| 패키지 | 버전 | 날짜 |
|---|---|---|
| `typescript` latest | 7.0.2 | 2026-07-08 |
| `typescript` 6.x 마지막 | 6.0.3 | 2026-04-16 |
| `typescript` beta | 7.1.0-beta | 2026-10-20 |
| `@typescript/typescript6` (호환 패키지, `tsc6` 실행 파일과 6.0 API) | 6.0.2 (내부 컴파일러는 6.0.3로 보고됨) | 2026-07 |

- TS 7.0은 "API를 함께 제공하지 않는다"고 발표문에 적혀 있다. typescript-eslint처럼 API가 필요한 도구는 6.0을 함께 설치하고 별칭(`"typescript": "npm:@typescript/typescript6@^6.0.2"`, `"@typescript/native": "npm:typescript@^7.0.2"`)으로 쓰라고 권장한다. [TS 7.0 발표]
- TS 6.0부터 `strict`의 기본값이 `true`다. `noUncheckedSideEffectImports`의 기본값도 `true`이고, `types`의 기본값은 `[]`다. `esModuleInterop`, `allowSyntheticDefaultImports`, `alwaysStrict`는 끌 수 없다. TS 7.0에서는 `baseUrl`, `moduleResolution: node10`, `target: es5`, `downlevelIteration`이 제거됐다. [TS 6.0 발표], [TS 7.0 발표]
- TS 7.1 beta는 새 API를 제공하지만 새 엄격 옵션은 추가하지 않았다. [TS 7.1 beta 발표]

tsconfig 참조 문서의 "Type Checking" 분류 20개 옵션과, 그 밖에 엄격성에 영향을 주는 옵션은 다음과 같다. "검증" 열은 아래 tsc 위반 예제에서 TS 7.0.2와 6.0.3 모두 해당 오류 코드로 실패한 것을 뜻한다. [tsconfig 참조]

| 옵션 | 잡는 것 | `strict`에 포함 | 초안 | 검증 |
|---|---|---|---|---|
| `strict` | 아래 9개를 한 번에 켬. 앞으로 생기는 엄격 검사도 여기에 추가됨 | - | 켬 | - |
| `alwaysStrict` | 모든 파일을 엄격 모드로 해석 (6.0부터 끌 수 없음) | 예 | 켬 | - |
| `noImplicitAny` | 추론되지 않은 암묵적 `any` | 예 | 켬 | TS7006 |
| `noImplicitThis` | 타입이 `any`인 `this` | 예 | 켬 | - (`this`는 린트로 금지) |
| `strictBindCallApply` | `bind`/`call`/`apply` 인자 타입 | 예 | 켬 | - (린트로 금지) |
| `strictBuiltinIteratorReturn` | 내장 반복자의 `TReturn`을 `any` 대신 `undefined`로 | 예 | 켬 | - |
| `strictFunctionTypes` | 함수 매개변수 반변성 검사. **메서드 단축 문법에는 적용되지 않음** | 예 | 켬 | 린트(`method-signature-style`)로 보완 |
| `strictNullChecks` | `null`·`undefined` 분리 | 예 | 켬 | TS18047 |
| `strictPropertyInitialization` | 클래스 필드 초기화 | 예 | 켬 | - (클래스 금지) |
| `useUnknownInCatchVariables` | `catch (e)`의 `e`를 `unknown`으로 | 예 | 켬 | TS18046 |
| `noUncheckedIndexedAccess` | 인덱스 접근 결과에 `undefined` 추가 | 아니오 | 켬 | TS2322 |
| `exactOptionalPropertyTypes` | `a?: T`에 `undefined`를 넣는 것 금지 | 아니오 | 켬 | TS2375 |
| `noImplicitReturns` | 일부 경로에서 반환 누락 | 아니오 | 켬 | TS7030 |
| `noImplicitOverride` | `override` 없는 재정의 | 아니오 | 켬 | TS4114 |
| `noFallthroughCasesInSwitch` | `case` 흘러내림 | 아니오 | 켬 | TS7029 |
| `noPropertyAccessFromIndexSignature` | 인덱스 시그니처를 점 표기로 접근 | 아니오 | 켬 | TS4111 |
| `noUnusedLocals` | 미사용 지역 변수·가져오기 | 아니오 | 켬 | TS6133 |
| `noUnusedParameters` | 미사용 매개변수 | 아니오 | 켬 | TS6133 |
| `allowUnreachableCode: false` | 도달 불가 코드 (기본값은 경고 수준 제안) | 아니오 | `false` | TS7027 |
| `allowUnusedLabels: false` | 미사용 레이블 | 아니오 | `false` | TS7028 |
| `noUncheckedSideEffectImports` | 해석되지 않는 부수 효과 가져오기 (6.0부터 기본 `true`) | 아니오 | 켬 | TS2882 |
| `erasableSyntaxOnly` | `enum`, `namespace`, 매개변수 속성 등 지울 수 없는 문법 | 아니오 | 켬 | TS1294 |
| `verbatimModuleSyntax` | 타입 전용 가져오기에 `type` 표기 강제 | 아니오 | 켬 | TS1484 |
| `isolatedModules` | 파일 단위 변환에서 깨지는 문법 | 아니오 | 켬 | - |
| `forceConsistentCasingInFileNames` | 파일 이름 대소문자 불일치 | 아니오 | 켬 | - |
| `libReplacement: false` | `@typescript/lib-*`로 표준 타입이 바뀌는 것 (6.0부터 기본 `false`) | 아니오 | `false` | - |
| `types: []` | `@types/*` 전역 타입 자동 포함 (6.0부터 기본 `[]`) | 아니오 | `[]` | - |
| `skipLibCheck: false` | 의존성 `.d.ts`의 오류도 검사 | 아니오 | `false` | 예제의 React·Zod·TanStack 타입은 통과 |
| `allowJs: false`, `checkJs: false` | JS 파일 혼입 | 아니오 | `false` | - |
| `isolatedDeclarations` | 내보내는 선언에 명시적 타입 강제 | 아니오 | **끔** | 같은 효과를 ESLint `explicit-module-boundary-types`로 얻었다. 이 옵션은 `declaration`/`composite`가 필요해 `noEmit` 앱 설정과 맞지 않는다 |
| `noErrorTruncation` | 오류 메시지의 타입 생략 방지 (진단 품질용) | 아니오 | 켬 | - |

harness.md 4.1과 비교하면 빠진 옵션은 `noUncheckedSideEffectImports`, `forceConsistentCasingInFileNames`, `libReplacement: false`, `types: []`다. `strictBuiltinIteratorReturn`은 `strict`에 포함되지만, 초안은 기본값이 바뀌어도 꺼지지 않도록 명시한다.

### 2.2 각 항목을 강제하는 규칙

모든 항목을 실제 위반 예제로 확인했다. "위반 예제" 열의 이름은 아래 실행 결과의 이름과 같다.

| 강제할 것 | 규칙 | 위반 예제 | 알려진 빈틈과 대응 |
|---|---|---|---|
| `any` 금지 | `@typescript-eslint/no-explicit-any`, `strict-type-checked`의 `no-unsafe-*` 전부 | any | 라이브러리가 `any`를 돌려주는 경우는 `no-unsafe-*`가 사용 지점에서 잡는다 (tauri 예제에서 `no-unsafe-assignment` 발생) |
| 타입 단언 금지 | `consistent-type-assertions: { assertionStyle: "never" }`, `no-unsafe-type-assertion` | as, angle, double-as | `as const`는 이 규칙이 항상 허용한다 (공식 문서). 예제에서도 통과. `satisfies`도 통과 |
| `!` 단언 금지 | `no-non-null-assertion` | non-null | - |
| 타입 술어·단언 함수 금지 | `no-restricted-syntax`의 `TSTypePredicate` | type-predicate, asserts | 템플릿 리터럴 타입(`` `${number}` ``)으로 좁힐 방법이 없어 `shared/lib/money.ts` 한 곳은 예외가 필요했다 (2.9 참고) |
| 오버로드·`declare` 금지 | `no-restricted-syntax`의 `TSDeclareFunction`, `[declare=true]`, `TSModuleDeclaration` | overload, declare, namespace | 오버로드는 구현과 다른 시그니처를 내보낼 수 있고, `declare`는 없는 값을 있는 것처럼 꾸밀 수 있다. 처음 목록에는 없던 우회로다 |
| `throw` 금지 | `functional/no-throw-statements` | throw | 라이브러리와 내장 함수(`new URL`, `decodeURIComponent`)가 던지는 것은 막을 수 없다 (빈틈 throwing-builtin-gap) |
| `try/catch` 금지 | `functional/no-try-statements` | try | - |
| `Promise.reject` 금지 | `functional/no-promise-reject` | promise-reject | - |
| 무시된 Result 금지 | `functional/no-expression-statements: { ignoreVoid: true }` | ignored-result, neverthrow-ignored | 반환값이 `void`가 아닌 호출을 문장으로 쓰면 오류다. Result 종류와 상관없이 걸린다 |
| 변수에 받아 버리는 Result | `@typescript-eslint/no-unused-vars` (무시 패턴 없음), TS `noUnusedLocals` | ignored-result-assign | `_` 접두사로도 피할 수 없게 `ignorePattern`을 두지 않는다 |
| 무시된 Promise 금지 | `no-floating-promises: { ignoreVoid: false, checkThenables: true }`, `no-misused-promises` | floating-promise, neverthrow-async-ignored | 기본값 `ignoreVoid: true`는 `void p`를 허용하므로 반드시 끈다 |
| `void`로 버리기 | `no-void` | void-promise | - |
| `let` 금지 | `functional/no-let`, `no-var`, `prefer-const` | let, var | - |
| 변경 금지 | `functional/immutable-data`, `no-param-reassign: { props: true }`, `no-plusplus`, `no-restricted-properties`(`Object.assign` 등), `functional/type-declaration-immutability`, `functional/prefer-immutable-types`(매개변수만) | push, prop-assign, sort, delete, map-set-chain, ref-assign, object-assign, param-reassign, plusplus, mutable-type, mutable-param | `Array.prototype.push.call(xs, 1)`은 `immutable-data`가 잡지 못했다. `prototype` 접근과 `call`/`apply`/`bind`를 `no-restricted-syntax`로 막아 해결했다 (proto-push-call) |
| 클래스 금지 | `functional/no-classes`, `functional/no-class-inheritance` | class, class-expr | 클래스 식도 잡는다 |
| `this` 금지 | `functional/no-this-expressions` | this | 객체 리터럴 메서드 안의 `this`도 잡는다 |
| `enum`·`namespace` 금지 | TS `erasableSyntaxOnly`(TS1294) + `no-restricted-syntax` | enum, namespace | 컴파일러와 린트 양쪽에서 실패한다 |
| 빠짐없는 `switch` | `switch-exhaustiveness-check: { allowDefaultCaseForExhaustiveSwitch: false, considerDefaultExhaustiveForUnions: false }`, `no-restricted-syntax`의 `SwitchCase[test=null]`, TS `noFallthroughCasesInSwitch` | switch-missing, switch-default | `if`/삼항 사슬로 유니온을 나누는 경우는 빠짐 검사가 없다. 명시적 반환 타입과 `noImplicitReturns`가 반환 누락은 잡는다. 이 규칙의 `defaultCaseCommentPattern`(주석으로 default 생략 표시)은 주석 금지로 쓸 수 없게 된다 |
| 모든 주석 금지 | 로컬 규칙 `local/no-comments` (`sourceCode.getAllComments()`를 전부 보고) | comment-line, comment-block, comment-jsx, triple-slash | JSX 안의 `{/* */}`, `///` 지시문도 잡는다. 내장 규칙은 없다. 로컬 규칙은 15줄이다 |
| `@ts-ignore` 계열 금지 | `ban-ts-comment` (네 지시문 모두 `true`) + 주석 금지 | ts-ignore, ts-expect-error, ts-nocheck | `tsc`는 린트와 상관없이 `@ts-ignore`를 따르므로 린트 실패가 유일한 방어선이다 |
| 린트 끄기 지시문 금지 | `linterOptions.noInlineConfig: true`, `reportUnusedDisableDirectives: "error"`, `reportUnusedInlineConfigs: "error"`, `@eslint-community/eslint-comments/no-use` | eslint-disable-line, eslint-disable-file, eslint-inline-config | `noInlineConfig` 상태에서 지시문을 쓰면 ESLint가 "has no effect" 경고를 내고, `--max-warnings 0`이면 실패한다 |
| `useEffect` 계열 금지 | `no-restricted-imports`(`importNames`), `no-restricted-syntax`(식별자·멤버 이름, `react` 기본·네임스페이스 가져오기) | effect-named, effect-alias, effect-namespace, effect-default, effect-reexport | **추가로 막아야 했던 우회로**: `react/index.js` 같은 하위 경로 가져오기(react-subpath), `useSyncExternalStore`의 `subscribe`에서 부수 효과 실행(sync-store-in-feature) |
| 지정 폴더 예외 | `shared/platform`에 대한 덮어쓰기 블록 + **파일 허용 목록** (`local/forbidden-file`) | new-platform-file | 폴더만 예외로 두면 에이전트가 파일을 `shared/platform`으로 옮겨 규칙을 피할 수 있다. 허용 목록은 보호 파일인 설정 안에 있다 |
| 폴더 간 의존 | `boundaries/dependencies`, `boundaries/no-unknown-files`, `boundaries/no-unknown-dependencies` | cross-feature, deep-import, lib-to-ipc, unknown-folder | 2.7 참고 |
| `@tauri-apps/api`는 `shared/ipc`만 | `no-restricted-imports` 패턴 | tauri-outside-ipc | - |
| 금액 산술 금지 | 로컬 타입 인식 규칙 `local/no-money-coercion` + 불투명 금액 타입(`tsc` TS2362·TS2345) + `restrict-plus-operands`, `no-base-to-string` | money-plus, money-number, money-template, money-compare, opaque-money-minus | 문자열 브랜드로 만들면 `tsc`가 `parseFloat(a) + Number(b)`와 `a + b`를 통과시킨다 (string-brand-gap). 2.9 참고 |
| `features`의 숫자 계산 | `no-restricted-syntax`(`Number`·`parseFloat`·`parseInt`·`BigInt` 호출, `Math.*`, 단항 `+`) | number-in-feature | - |
| 디자인 토큰 밖 값 | `no-restricted-syntax`: `style` 속성, 색상 리터럴, 크기 리터럴, `className`의 임의 값 `[...]`, 템플릿 문자열 안의 색·크기, 기본 HTML 컨트롤 | style-prop, hex-color, px-literal, arbitrary-class, raw-button | 문자열을 쪼개 합치면(`` `${hash}ff0000` ``) 리터럴 검사를 피한다 (hex-concat-gap). CSS 파일 검사와 Tailwind 클래스 허용 목록은 R-14에서 정한다 (후보: `eslint-plugin-better-tailwindcss` 4.11.0의 미등록 클래스 규칙, 미검증) |
| 그 밖 | `no-eval`, `no-new-func`, `no-restricted-globals`(`globalThis`, `window` 등), `JSON.parse` 금지, `.parse()` 금지(`safeParse`만), 기본 내보내기·`export *`·동적 `import` 금지, `no-nested-ternary`, `functional/no-loop-statements`, `explicit-function-return-type`, `strict-boolean-expressions`(모든 허용 옵션 `false`), `method-signature-style: property`, 테스트 `.skip`/`.only`/`.todo` 금지, 복잡도 상한 | eval, function-ctor, global-eval, json-parse, schema-parse, default-export, dynamic-import, nested-ternary, loop, missing-return-type, strict-boolean, method-shorthand, test-skip | 메서드 단축 문법은 `strictFunctionTypes`를 피해 매개변수를 이변적으로 만든다. 그래서 `method-signature-style`이 필요하다 |
| `.js`·`.d.ts` 혼입 | 로컬 규칙 `local/forbidden-file`을 해당 확장자에 건다 | js-file, dts-file | ESLint 기본 설정은 `.js`를 규칙 없이 통과시키고, `files`에 맞지 않는 `.ts`는 아예 검사하지 않는다 (공식 문서). 그래서 확장자 차단 블록이 필요하다 |

로컬 규칙을 만드는 난이도: 주석 금지와 금지 파일은 각각 15줄 안팎이다. 금액 산술 금지는 타입 검사기(`parserServices.getTypeAtLocation`)를 써서 60줄 정도다. 모두 ESLint의 사용자 플러그인 API만 쓴다. 전문은 부록에 있다.

### 2.3 타입 인식 린트의 속도와 도구별 상태

측정 (Apple Silicon Mac, Node 24.22.0. 예제의 기능 폴더를 200번 복제해 619개 파일로 만든 합성 프로젝트):

| 작업 | 시간(실측) |
|---|---|
| `tsc` 7.0.2 전체 검사 | 0.54초 |
| `tsc6` 6.0.3 전체 검사 | 3.50초 |
| ESLint 전체 (위 설정, 캐시 없음) | 7.1초 |
| ESLint 파일 1개 (PostToolUse 훅 상황) | 1.6초 |
| ESLint `--cache` 두 번째 실행 | 1.0초. 단, 아래 이유로 쓰지 않는다 |
| oxlint 1.93 `--type-aware` (typescript 규칙 7개만) | 0.38초 |

- `TIMING`으로 본 무거운 규칙: `react-hooks/static-components`, `@typescript-eslint/no-deprecated`, `functional/prefer-immutable-types`, `no-misused-promises` 순이었다.
- typescript-eslint 공식 문서: 타입 인식 린트 시간은 빌드 시간과 비슷하다고 보면 된다. 느린 원인의 대부분은 타입 검사 API 호출이다. `projectService`를 쓰면 넓은 `include`도 따로 조정할 필요가 없다. [ts-eslint 성능]
- ESLint `--cache`는 파일 단위 캐시라서 다른 파일의 타입이 바뀐 것을 반영하지 못한다. typescript-eslint는 타입 인식 규칙과 함께 쓰는 것을 권하지 않는다. [ts-eslint FAQ]
- 결론: `just check`에서 전체 ESLint를 돌려도 수천 파일 규모까지는 수십 초 안쪽일 것으로 보인다(선형 외삽이며 미검증). 편집 직후 훅은 바뀐 파일만 검사한다.

| 도구 | 타입 인식 | 사용자 규칙 | 이 하네스에 필요한 플러그인 | 상태 |
|---|---|---|---|---|
| ESLint 10.14.0 + typescript-eslint 8.72.2 | 예. TS 6.0 API 사용 | JS 플러그인, 타입 검사기 사용 가능 | functional, boundaries, react-hooks, eslint-react 모두 동작 (실측) | 안정. TS 7 백엔드(`projectService.EXPERIMENTAL_backend: 'tsgo'`, TS 7.1 대상)는 2026-11-10 main에 병합됐지만 latest 8.72.2에는 없다 (canary 8.72.3-alpha.7) [PR #12803] |
| oxlint 1.93.0 + oxlint-tsgolint 7.0.2006 | 예. typescript-go 기반, "TS 7.0 이상 필요". typescript-eslint 타입 인식 규칙 61개 중 59개 지원 [oxlint 타입 인식] | JS 플러그인 **alpha**. 타입 인식 규칙은 JS 플러그인에서 지원하지 않는다 [oxlint JS 플러그인] | functional·boundaries는 지원 목록에 없음 (미확인). 로컬 금액 규칙은 불가 | 실측: `consistent-type-assertions`, `no-floating-promises`, `strict-boolean-expressions`(`allowString: false`), `switch-exhaustiveness-check`, `restrict-plus-operands`가 위반 예제를 잡았다. 인라인 끄기 지시문을 끄는 옵션은 문서에서 확인하지 못했다 (미확인) |
| Biome 2.6.6 | 자체 추론. TS 컴파일러를 쓰지 않는다. `noFloatingPromises`가 이상적으로 잡아야 할 경우의 약 85%를 탐지 (2.1 기준) [Biome 2.1] | GritQL 플러그인(진단·재작성, 의미 모델 없음), JS 플러그인 Developer Preview [Biome 플러그인] | 해당 없음 | 엄격성의 기준으로 쓰기에는 부족하다. 형식 도구로만 후보 |

### 2.4 Result 타입 선택지

| 선택지 | 버전·유지 | 무시된 Result 탐지 | 비동기 조합 | 학습 비용 | AI 일관성 |
|---|---|---|---|---|---|
| 직접 정의한 판별 유니온 (`{ ok: true, value } \| { ok: false, error }`) | 코드 15줄 (`shared/lib/result.ts`) | `no-expression-statements` + `no-unused-vars` + `no-floating-promises`로 탐지 (실측) | `Promise<Result<T, E>>`를 그대로 쓴다. `switch`/`if (r.ok)`로 좁힌다 | 가장 낮다 | 높다. 문법이 하나뿐이고 `switch-exhaustiveness-check`와 맞물린다 |
| neverthrow 8.2.0 | 마지막 릴리스 2025-02-21, 저장소 마지막 푸시 2026-02-14, 열린 이슈 93개 (GitHub API) | 범용 규칙으로 탐지 (실측: 동기 `Result`, `ResultAsync` 모두). 전용 `eslint-plugin-neverthrow` 1.1.4는 2022년판이고 `tsutils` 3에 의존한다 (ESLint 10 동작은 미확인) | `ResultAsync`, `andThen` 사슬 | 중간 | 중간. 메서드 사슬과 `_unsafeUnwrap`(던짐)이 있어 별도 금지가 필요하다 (neverthrow-unsafe-unwrap 빈틈) |
| Effect 4.1.4 | 4.0.0이 2026-10-01 출시. 활발 | `Effect` 값은 실행하지 않으면 아무 일도 안 일어난다. 무시 탐지는 범용 규칙에 의존 | 가장 강력하다 (동시성, 재시도, 자원) | 높다. 런타임과 생태계 전체를 들여온다 | 낮다. 규칙이 많아 에이전트가 섞어 쓰기 쉽다 |
| better-result 3.0.1, true-myth 9.4.0, @swan-io/boxed 3.2.1 | npm에서 버전 확인. 그 밖의 평가는 하지 않음 | 범용 규칙에 의존 | 라이브러리마다 다름 | 중간 | 미확인 |

추천: **직접 정의**. 무시 탐지는 라이브러리와 무관하게 범용 규칙으로 얻었다. TS 쪽에는 계산이 없고(architecture.md 1.3) IPC 결과를 표시하는 정도라서 사슬 조합의 이득이 작다. 판별 유니온은 `switch` 빠짐 검사와 그대로 맞물린다.

틀렸다고 판명될 조건: `features`의 비동기 흐름이 여러 단계로 깊어져(예: IPC 결과 셋을 조합한 뒤 다시 호출) `if (!r.ok) return r;` 반복이 함수 길이 상한에 자주 걸리면, `mapResult`/`andThen` 몇 개를 `shared/lib`에 추가한다. 그래도 부족하면 neverthrow를 다시 검토한다.

### 2.5 `useEffect` 없이 React 앱 만들기

React 공식 문서 "You Might Not Need an Effect"의 기준은 "외부 시스템이 없으면 Effect가 필요 없다"이다. Effect가 필요한 경우로는 외부 시스템 동기화, 데이터 가져오기, 외부 저장소 구독을 들고, 구독에는 `useSyncExternalStore`를 권한다. [React: Effect]

| 필요 | 대체 수단 | 비고 |
|---|---|---|
| 데이터 읽기 | TanStack Query `useQuery(queryOptions)` (`features/*/model`) | 예제에서 동작. `queryFn`은 `Promise<Result>`를 돌려주고 거절하지 않는다 (2.6) |
| 코어 이벤트 구독 | `shared/platform/useCoreEvent` (`useSyncExternalStore`) | 예제에서 동작. `useSyncExternalStore`는 `features`에서 금지한다 (`subscribe` 안에서 아무 부수 효과나 실행할 수 있기 때문) |
| 코어 이벤트 → 쿼리 갱신 | `shared/platform`의 구독 하나가 `queryClient.invalidateQueries`를 부른다 | 앱 전체에 한 번만 있으면 된다 |
| 파생 값 | 렌더링 중 계산, 필요하면 `useMemo`. 상태 초기화는 `key` | React 문서 권장 |
| 사용자 동작 | 이벤트 핸들러, `useMutation` | 비동기 결과를 버려야 할 때는 `shared/platform/detach`만 쓴다 |
| 화면 진입 동작 | 라우트 `loader`/`beforeLoad` | TanStack Router의 `redirect()`는 던지지 않고 반환할 수도 있다 [TanStack redirect] |
| 문서 제목 | `shared/platform/useDocumentTitle` 또는 React 19의 `<title>` 끌어올리기 | 예제는 전자를 썼다. 후자는 미검증 |
| 포커스 | `autoFocus`, 또는 ref 콜백 | ref 콜백은 붙을 때 노드를 받고 정리 함수를 돌려줄 수 있다 (React 19) [React: ref 콜백] |
| 스크롤 | ref 콜백에서 `scrollIntoView`, 또는 이벤트 핸들러 | - |
| 타이머·간격, 창 크기, 온라인 상태 | `shared/platform`의 `useSyncExternalStore` 훅 | - |
| 비React 위젯(차트 등) | ref 콜백과 정리 함수, 또는 `shared/platform`의 전용 훅 | 차트 라이브러리는 R-14에서 정한다 |

대체가 안 되는 경우:

- 렌더링 결과를 외부 시스템에 계속 맞춰야 하는데 DOM 노드가 없는 경우(예: 소리, 전역 단축키 등록). `shared/platform`의 허용 목록 파일로만 처리한다.
- 오류 경계: "현재 함수 컴포넌트로 오류 경계를 만들 방법이 없다." React는 `react-error-boundary`를 권한다. [React: Component] 클래스 금지를 지키려면 이 의존성을 허용 목록에 넣는다.
- **빈틈**: ref 콜백은 붙을 때 실행되므로 사실상 마운트 Effect로 쓸 수 있다. 예제에서 ref 콜백 안의 IPC 호출은 린트를 통과했다 (ref-callback-effect-gap). `react-hooks` 7의 컴파일러 규칙(`refs`, `purity`)도 잡지 않았다. ref 콜백 본문을 `shared/ds` 컴포넌트로 한정하는 규칙은 로컬 규칙으로 만들 수 있지만 이번에는 만들지 않았다. 지금은 리뷰 항목으로 둔다.

### 2.6 쿼리 계층과 라우터 (Tauri IPC 기준)

| 선택지 | Tauri IPC에서의 장점 | 단점 | 하네스와의 충돌 |
|---|---|---|---|
| TanStack Query 5.107.1 | 전송 방식과 무관하다. `queryFn`에 IPC 함수를 그대로 넣는다. `invalidateQueries`로 코어 이벤트와 연결한다 | 오류 상태를 쓰려면 "`queryFn`이 던지거나 거절된 Promise를 돌려줘야 한다" [TanStack Query 함수] | 던지기 금지와 충돌한다. **`queryFn`은 `Promise<Result>`를 돌려주고 거절하지 않는다.** 화면은 `status`와 `data.ok`를 둘 다 본다 (예제). 재시도는 `Err`에 대해 동작하지 않으므로 코어(Rust)가 맡는다 |
| SWR | 단순하다 | 이번에 검증하지 않았다 | 미확인 |
| 직접 만든 저장소 + `useSyncExternalStore` | 의존성이 없다 | 캐시, 중복 요청 제거, 무효화를 직접 만들어야 한다 | 없음 |
| TanStack Router 1.170.46 | 타입 안전 경로와 검색 매개변수, `validateSearch`에 스키마 사용, `loader`, `errorComponent` [TanStack 데이터 로딩] | 파일 기반 경로 생성기를 쓰면 생성 파일이 생긴다 (보호 경로 목록에 등록) | `redirect`를 반환으로 쓸 수 있다. `notFound()`를 던지지 않고 쓸 수 있는지는 미확인 |
| React Router 8.5.1 | 널리 쓰인다 | 이번에 검증하지 않았다 | 미확인 |

추천: **TanStack Query + TanStack Router**. 둘 다 예제에서 타입 검사와 린트를 통과했다(Router는 설치만 했고 경로 구성은 실행하지 않았다). 데스크톱 앱에는 주소창이 없으므로 메모리 또는 해시 기록을 쓰는 것이 안전해 보인다. Tauri에서 브라우저 기록 모드로 새로 고침했을 때의 동작은 공식 문서에서 확인하지 못했다 (미확인. R-05/R-15에서 확인).

틀렸다고 판명될 조건: Tauri 이벤트 스트림(채팅 토큰)을 쿼리 캐시로 다루기 어렵다고 드러나면, 스트림은 `shared/platform`의 구독 훅으로만 받고 쿼리는 스냅샷 읽기에만 쓴다.

### 2.7 폴더 의존 규칙 도구

| 도구 | 실측 | 장점 | 단점 |
|---|---|---|---|
| eslint-plugin-boundaries 7.4.2 | 기능 간 가져오기, `index.ts`를 거치지 않는 깊은 가져오기, `lib → ipc`, 요소 밖 파일을 모두 잡았다 | 편집 직후 ESLint 한 번으로 피드백이 온다. 경로 캡처(`featureName`)로 "같은 기능 안에서만" 같은 규칙을 표현한다. 파일 분류 밖 파일을 금지한다(`no-unknown-files`) | v7에서 `entry-point`, `no-unknown`, `mode`, 레거시 템플릿이 폐기 예정이 되었다. 실행 중 경고로 확인했고 새 문법으로 옮겼다. 정책은 마지막에 맞는 것이 이긴다 [boundaries 정책] |
| dependency-cruiser 18.7.2 | 기능 간 가져오기(`$1` 역참조)와 고아 파일을 잡았다. 26개 모듈에 0.97초 | 순환 의존(`circular`), 고아, 그래프 시각화. ESLint와 독립된 두 번째 검사 | 별도 설정 파일과 실행이 필요하다 |
| typescript-eslint 문서가 권하는 `import/no-cycle` 계열 | 미실행 | - | 느려서 CI에서만 돌리라고 권한다 [ts-eslint 성능] |

추천: **eslint-plugin-boundaries를 기본으로** 쓰고, **dependency-cruiser는 `just check-full`에서 순환 의존 검사용으로만** 쓴다. 고아 파일 검사는 knip과 겹치므로 knip에 맡긴다.

### 2.8 미사용 코드·의존성 검사

- **knip 6.45.1**. 실측으로 미사용 파일, 미사용 내보내기와 타입, 미사용 의존성을 모두 찾았다(심어 둔 `unused.ts`, `orphan`, 쓰지 않는 `valibot`·`neverthrow`). 설정의 `classMembers`는 v6에서 알 수 없는 항목이라는 경고가 났다. 클래스를 금지했으므로 필요 없다.
- TS `noUnusedLocals`/`noUnusedParameters`와 `@typescript-eslint/no-unused-vars`가 파일 안을, knip이 파일 사이를 맡는다.
- knip이 ESLint 플러그인을 devDependency로 인식하려면 `eslint.config.mjs`가 프로젝트 안에 있어야 한다. 실험에서는 복사하지 않아 미사용으로 보고됐다.

### 2.9 경계 파싱 스키마와 브랜드 타입

| 라이브러리 | 버전 | 브랜드 | 던지지 않는 파싱 | 비고 |
|---|---|---|---|---|
| Zod | 4.6.7 | `.brand<"X">()`. 정적 표시만 하고 런타임에는 영향이 없다. 4.2부터 방향(`in`/`out`/`inout`) 지정 [Zod 브랜드] | `safeParse` (`parse`는 던짐) | 예제에서 `safeParse`와 `transform`으로 금액을 만드는 데 썼다 |
| Valibot | 1.5.0 | `v.brand("X")` [Valibot brand] | `safeParse` (공식 문서의 `brand` 페이지에서는 확인하지 못했다) | 번들이 작다. 데스크톱 앱에서는 이점이 작다 |
| ArkType | 2.2.9 | 미조사 | 미조사 | - |

핵심 결과는 **브랜드를 어떤 타입에 붙이느냐**였다.

- `string & { [brand]: "DecimalString" }`(Zod `.brand()`도 같은 모양): 여전히 `string`이라서 `parseFloat(a) + Number(b)`와 `a + b`가 TS 7과 TS 6 모두에서 **오류 없이 통과한다** (string-brand-gap).
- 비공개 `unique symbol` 키를 가진 객체(`{ currency; [decimalKey]: \`${number}\` }`): `a - b`(TS2362), `parseFloat(a)`(TS2345)는 컴파일러가 막는다. 남는 `Number(a)`, `` `${a}` ``, `a > b`, `a + "원"`은 로컬 규칙 `no-money-coercion`이 막는다. 키를 내보내지 않으므로 `shared/lib` 밖에서는 값을 꺼낼 수 없다. 꺼낼 수 있는 우회로인 `Object.getOwnPropertySymbols`와 `Reflect`는 금지한다.
- 이렇게 하면 `Intl.NumberFormat.format`이 받는 `` `${number}` `` 타입으로 좁히는 곳 하나(`shared/lib/money.ts`의 타입 술어)가 필요하다. 이 파일은 예외 목록으로 관리한다.

추천: **Zod 4, `safeParse`만 사용**(`.parse()` 호출은 린트로 금지). 금액·수량·가격처럼 계산을 막아야 하는 값은 **불투명 객체 타입**으로 만든다. 종목 코드나 ID처럼 산술과 무관한 값은 Zod 브랜드로 충분하다. Rust에서 생성되는 IPC 타입(R-05)과 스키마가 어긋나지 않도록, 스키마의 출력 타입을 생성 타입에 맞춰 검사하는 방법은 R-05 결과를 받은 뒤 정한다 (미확인).

### 2.10 규칙이 실제로 작동하는지 검증하는 방법

예제 프로젝트에 다음 네 가지를 만들어 돌렸다. 모두 `just check`에 넣을 수 있다.

1. **위반 예제 + 목록** (`harness/violations/src/**`, `harness/lint-cases.json`): 실제 `src`와 같은 폴더 구조로 두어 경로별 규칙(`features`, `shared/platform`)이 그대로 적용된다. 각 예제에 "반드시 보고돼야 하는 규칙 ID"를 적는다. 실행기는 ESLint Node API(`lintFiles`)로 검사하고 해당 규칙이 없으면 실패한다. 통과해야 하는 예제(`as const`, `satisfies`)는 "문제 0개"를 기대한다. 알려진 빈틈은 `KNOWN_GAP`으로 표시해, 나중에 막히면 `GAP?`로 드러나게 한다. 예제 파일에는 주석을 쓸 수 없으므로 기대값은 목록 파일에 둔다.
2. **tsc 위반 예제** (`harness/tsc-violations/*.ts`): 파일마다 기대 오류 코드를 정하고 `tsc`와 `tsc6` 두 컴파일러로 확인한다.
3. **유효 설정 검사** (`ESLint#calculateConfigForFile`): 대표 파일마다 핵심 규칙의 심각도가 `error`이고 `noInlineConfig`가 `true`인지 확인한다. 나중에 덮어쓰기 블록이 규칙을 몰래 끄는 것을 막는다.
4. **실제 코드의 무결성**: `eslint --max-warnings 0 src`와 `tsc`가 깨끗해야 한다. 규칙이 너무 엄격해서 정상 코드를 쓸 수 없는 상태도 이것으로 드러난다.

로컬 규칙의 단위 테스트는 ESLint `RuleTester`가 표준이다(이번에는 1번으로 대신했다).

### 2.11 흔한 우회 수법과 막는 규칙

| 우회 | 막는 것 | 실측 |
|---|---|---|
| `x as unknown as T`, `<T>x` | `consistent-type-assertions: never` | 막힘 |
| `x is T` 술어로 거짓 좁히기, `asserts` | `TSTypePredicate` 금지 | 막힘 |
| 오버로드로 거짓 시그니처 | `TSDeclareFunction` 금지 | 막힘 |
| `declare const x: T`로 없는 값 꾸미기, `declare module`/`declare global`, `.d.ts` 추가 | `declare` 금지, `TSModuleDeclaration` 금지, `.d.ts` 금지 파일 | 막힘 |
| `JSON.parse`의 `any`, 스키마 `.parse()`의 예외 | `no-restricted-properties`, `.parse` 호출 금지, `no-unsafe-*` | 막힘 |
| `// @ts-expect-error 긴 설명`, `/* eslint-disable */`, `/* eslint rule: off */` | 주석 금지, `ban-ts-comment`, `noInlineConfig`, `eslint-comments/no-use` | 막힘 |
| `void promise`, `const _ = result` | `no-void`, `no-floating-promises(ignoreVoid: false)`, `no-unused-vars`(무시 패턴 없음) | 막힘 |
| `import * as R from "react"; R.useEffect`, `import React`, 별칭 가져오기, 다시 내보내기, `react/index.js` 하위 경로 | `no-restricted-imports`, `no-restricted-syntax` | 막힘 |
| `useSyncExternalStore`의 `subscribe`로 효과 실행 | `features`에서 가져오기 금지 | 막힘 |
| 규칙이 느슨한 폴더(`shared/platform`)로 파일 옮기기 | 파일 허용 목록 | 막힘 |
| `.js`/`.mjs` 파일로 TS 규칙 피하기 | `allowJs: false`, 확장자 금지 블록 | 막힘 |
| 요소 밖 폴더(`src/random/`) 만들기 | `boundaries/no-unknown-files` | 막힘 |
| `Array.prototype.push.call`, `fn.apply`, `__proto__` | `prototype`·`call`·`apply`·`bind` 금지 | 막힘 |
| `Function("...")`, `globalThis.eval` | `no-new-func`, `no-restricted-globals` | 막힘 |
| `new Map(m).set(...)`, `ref.current = x`, `delete o.a` | `immutable-data` | 막힘 |
| 메서드 단축 문법으로 이변 매개변수 | `method-signature-style: property` | 막힘 |
| 문자열 조각을 합쳐 색상 값 만들기 | 없음 | **빈틈** (R-14에서 CSS 토큰 검사와 함께 다룬다) |
| ref 콜백을 마운트 Effect처럼 쓰기 | 없음 | **빈틈** (리뷰 항목, 로컬 규칙 후보) |
| 던지는 내장 함수·라이브러리 호출 (`new URL`, `decodeURIComponent`, neverthrow `_unsafeUnwrap`) | 없음. 후보: 이름 목록으로 금지, `shared/lib`에 Result 감싸개 제공 | **빈틈** |
| 설정 파일·예외 목록 자체 수정 | 보호 경로와 CI 서명 검사 (harness.md 7.1) | 이 리서치 범위 밖 |
| 의존성으로 효과 훅 들여오기 (`react-use` 등) | 의존성 허용 목록 (harness.md 2장) | 이 리서치 범위 밖 |

## 3. 선택지 비교표와 추천

| 결정 | 선택지 | 추천 | 이유 | 틀렸다고 판명될 조건 |
|---|---|---|---|---|
| 컴파일러 | TS 7.0.2 단독 / TS 6.0.3 단독 / 7 + 6 별칭 | **7(`tsc`) + 6(린트 API)** | TS 7은 실측으로 6.5배 빠르다. 린트는 6이 필요하다. 위반 예제 결과가 두 버전에서 같았다 | typescript-eslint stable이 TS 7.1 백엔드를 지원하면 6을 뺀다. 두 버전이 같은 코드에 다른 결과를 내는 사례가 나오면 6 단독으로 되돌린다 |
| 린터 | ESLint+typescript-eslint / oxlint / Biome | **ESLint** | 필요한 플러그인과 타입 인식 로컬 규칙이 모두 동작하는 것은 ESLint뿐이다 | oxlint JS 플러그인이 타입 정보와 functional·boundaries를 지원하고 끄기 지시문을 끄는 옵션이 생기면 옮긴다 |
| 불변·함수형 규칙 | eslint-plugin-functional 10.0.1 / `no-restricted-syntax`만 | **functional** | `immutable-data`, `no-expression-statements(ignoreVoid)`는 타입 정보가 있어야 해서 문법 규칙으로 대신할 수 없다 | 오탐이 잦아 정상 코드가 길어지면 `prefer-immutable-types`부터 끈다 |
| Result | 직접 / neverthrow / Effect | **직접** | 2.4 | 2.4 |
| 경계 | boundaries / dependency-cruiser | **boundaries + (순환만) dependency-cruiser** | 2.7 | boundaries v7의 정책 문법이 다시 깨지는 변경이 생기면 dependency-cruiser로 옮긴다 |
| 미사용 | knip / TS만 | **knip** | 실측 | - |
| 스키마 | Zod / Valibot / ArkType | **Zod 4 (`safeParse`만)** | 실측. 에이전트가 가장 익숙하다는 점도 이유지만 이 점은 검증하지 않았다 | IPC 타입 생성 도구(R-05)가 다른 스키마 생성을 지원하면 그쪽을 따른다 |
| 금액 표현 | 문자열 브랜드 / 불투명 객체 | **불투명 객체 + 로컬 규칙** | 2.9의 실측 | 직렬화 경계(IPC 생성 타입)와 맞추는 비용이 크면 문자열 브랜드 + 로컬 규칙만으로 하고 `features`의 숫자 변환 금지를 유지한다 |
| 쿼리·라우터 | TanStack Query+Router / SWR / React Router | **TanStack Query + TanStack Router** | 2.6 | 2.6 |
| 형식 도구 | Prettier / Biome 형식 기능 | 이번에 비교하지 않음 | - | - |

## 4. 위험과 미확인 항목

위험:

- **두 컴파일러 동시 사용**: `tsc`(7)와 린트(6)의 타입 판단이 어긋날 수 있다. TS 7은 `stableTypeOrdering`을 항상 켠다. 6.0의 같은 옵션은 검사를 최대 25% 느리게 한다고 발표문에 적혀 있다. 이번 예제에서는 차이가 없었다. [TS 6.0 발표]
- **typescript-eslint의 TS 7 지원 시점**: TS 7.0 지원 요청 이슈 #12518은 "not planned"로 닫혔다. tsgo 백엔드는 실험 옵션으로 main에 막 들어갔고 TS 7.1을 대상으로 한다. 지원이 들어올 때 규칙 결과가 바뀔 수 있으므로 위반 예제 테스트로 확인한다.
- **eslint-plugin-boundaries v7의 폐기 예정 문법**: 오래된 예제(`element-types`, `entry-point`)를 따라 쓰면 경고만 나고 동작한다. `--max-warnings 0`이 이 경고를 잡는지는 확인하지 않았다. 이 경고는 ESLint 메시지가 아니라 콘솔 출력이었다.
- **`skipLibCheck: false`**: 의존성을 추가하다가 라이브러리 `.d.ts`의 오류로 막힐 수 있다. 그때는 의존성을 바꾸거나 ADR로 결정한다.
- **엄격함과 생산성**: `prefer-immutable-types`를 반환 타입에 적용하면 `ReactElement` 반환마다 오류가 났다. 그래서 매개변수에만 적용했다. 실제 화면 규모에서 오탐 비율은 미측정이다.
- **`functional/no-expression-statements` 예외**: `shared/platform`에서는 이 규칙을 끄므로, 그 안의 코드는 리뷰 강도를 높여야 한다. 파일 허용 목록 변경에는 사람의 승인이 필요하다.

미확인:

- oxlint에서 인라인 끄기 지시문을 끄는 설정이 있는지, eslint-plugin-functional·boundaries가 oxlint JS 플러그인으로 도는지.
- `eslint-plugin-neverthrow` 1.1.4가 ESLint 10에서 동작하는지 (실행하지 않았다).
- Tauri에서 브라우저 기록 라우팅으로 새로 고침했을 때의 동작.
- TanStack Router `notFound()`를 던지지 않고 쓸 수 있는지.
- SWR, React Router 8, ArkType, Valibot의 실제 동작 (설치·평가하지 않았다).
- 수천 파일 규모의 ESLint 시간 (619개 파일 실측을 선형으로 외삽한 값뿐이다).
- Tailwind 사용 여부와 `eslint-plugin-better-tailwindcss`의 토큰 강제 수준 (R-14).
- 에이전트가 이 설정에서 실제로 얼마나 자주 막히는지 (R-10에서 시험 작업으로 측정할 일).

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·위치 | 현재 | 바꿀 내용 |
|---|---|---|
| harness.md 4.1 | 옵션 목록 | `noUncheckedSideEffectImports`, `forceConsistentCasingInFileNames`, `libReplacement: false`, `types: []`, `noErrorTruncation`을 추가한다. strict 계열 9개는 명시한다. 컴파일러는 "TS 7(`tsc`) + TS 6 API(린트용) 별칭 설치"로 적는다 |
| harness.md 4장 표 "강한 타입 변환" | `as`, `any`, 타입 술어 금지 | 오버로드, `declare`, `namespace`/`declare global`, `.d.ts`, `JSON.parse`, 스키마 `.parse()` 금지를 추가한다. 타입 술어 예외 파일 목록(현재 `shared/lib/money.ts` 하나)을 보호 파일로 둔다 |
| harness.md 4장 표 "newtype" | 브랜드 타입 | 계산을 막아야 하는 값(`Money`, `Quantity`, `Price`, `Ratio`)은 비공개 symbol 키를 가진 **불투명 객체**로 바꾼다. 문자열 브랜드는 ID·종목 코드에만 쓴다 |
| harness.md 4.2 "실패 처리" | "반환된 `Result` 미사용 금지" | 구현 수단으로 `functional/no-expression-statements(ignoreVoid)`, `no-void`, `no-floating-promises(ignoreVoid:false, checkThenables)`, 무시 패턴 없는 `no-unused-vars`를 적는다. 비동기 결과를 버리는 유일한 수단은 `shared/platform/detach`다 |
| harness.md 4.2 "불변" | 후보 `eslint-plugin-functional` | 확정한다. 추가로 `prototype`·`call`·`apply`·`bind` 금지, `Object.assign` 등 금지, `method-signature-style: property`를 적는다 |
| harness.md 4.2 "React" | `useEffect` 3종 금지 | `useSyncExternalStore`(`features`), `react` 기본·네임스페이스 가져오기, `react/*` 하위 경로 가져오기 금지를 추가한다. `shared/platform`은 폴더 예외가 아니라 **파일 허용 목록** 예외로 바꾼다. ref 콜백 효과는 리뷰 항목으로 둔다 |
| harness.md 4.2 "숫자" | 금액에 `Number()` 등 금지 | 로컬 타입 인식 규칙 `no-money-coercion`(산술·비교·템플릿·`Number`/`String`/`parseFloat` 변환)으로 구현한다. `features`에서는 `Math.*`, 단항 `+`, 숫자 변환 호출을 일괄 금지한다 |
| harness.md 4.2 "주석" | 로컬 규칙, `noInlineConfig` | `reportUnusedDisableDirectives: "error"`, `reportUnusedInlineConfigs: "error"`, `@eslint-community/eslint-comments/no-use`, `ban-ts-comment`(4종)을 추가한다. `switch-exhaustiveness-check`의 주석 표시 기능은 쓰지 않는다 |
| harness.md 4.2 추가 행 | 없음 | "파일 종류: `src`에는 `.ts`/`.tsx`만. `.js`·`.mjs`·`.cjs`·`.d.ts`는 금지 파일 규칙으로 실패" |
| harness.md 4.2 끝 문단 | "오류 경계와 라우터가 요구하는 `throw`·클래스는 `shared/platform`에서 목록으로 허용, R-08에서 확정" | 확정: 오류 경계는 `react-error-boundary` 의존성으로 해결하고 클래스를 직접 쓰지 않는다. 라우터 `redirect`는 반환 형태로 쓴다. TanStack Query `queryFn`은 `Promise<Result>`를 돌려주고 거절하지 않는다. 이렇게 하면 `throw` 예외 목록은 비어 있다. `notFound()`는 미확인이다 |
| harness.md 4.3 | 후보들 | 경계는 `eslint-plugin-boundaries`, 순환 검사는 `dependency-cruiser`(`check-full`), 미사용 검사는 `knip`, 스키마는 Zod 4로 확정한다. 형식 도구와 테스트 도구는 미정으로 둔다 |
| harness.md 6장 "하네스 자체" | 규칙마다 위반 예제 | 구조를 적는다: `harness/violations/src/**`(실제 폴더 구조와 같게) + 기대 규칙 목록 JSON, tsc 위반 예제 + 기대 오류 코드(두 컴파일러), 유효 설정 검사, `KNOWN_GAP` 표시, 로컬 규칙 `RuleTester` |
| harness.md 7.1 보호 경로 | 린트 설정 등 | `eslint/`(로컬 규칙), `harness/`(위반 예제와 목록), 예외 파일 목록(`shared/platform` 허용 목록, 타입 술어 허용 목록), `knip.json`, `.dependency-cruiser.cjs`, 라우터 생성 파일 경로를 추가한다 |
| harness.md 1장 | `--max-warnings 0` | ESLint `--cache`를 쓰지 않는다고 적는다 (타입 인식 규칙과 맞지 않음). 편집 직후 훅은 바뀐 파일만 검사한다 (실측 약 1.6초) |
| architecture.md 5장 표 | 의존 규칙 문장 | 허용 의존을 표로 확정한다: `app → features(index만), shared/*` / `features/x → features/x 내부, ds, ipc, platform, lib` / `ds → lib` / `ipc → lib` / `platform → ipc, lib` / `lib → 없음`. 다른 요소의 파일은 `index.ts`로만 가져온다. 이 분류에 속하지 않는 파일은 만들 수 없다 |
| architecture.md 5장 "효과 금지" | `shared/platform`에서만 | "`shared/platform`의 허용 목록 파일에서만"으로 바꾼다 |
| architecture.md 5장 "상태" | 쿼리 캐시 | TanStack Query(`features/*/model`에 `queryOptions`)와 TanStack Router(메모리 또는 해시 기록)로 적는다. 코어 이벤트에 따른 무효화는 `shared/platform`의 구독 하나에서 한다 |
| architecture.md 3.6 | IPC 오류에 타입 | `shared/ipc`에서 Tauri 호출의 거절을 `then(ok, err)` 한 곳에서 `Result`로 바꾼다. 응답은 Zod `safeParse`로 검증한 뒤 도메인 표시 타입이 된다 |
| architecture.md 8장 | R-08 확정 항목 | "TS 린트·Result·경계 검사 도구"를 이 문서로 확정하고 ADR로 옮긴다. 쿼리·라우터는 R-08에서 추천, 컴포넌트·차트는 R-14 |

## 6. 실행 검증

예제 위치: `<세션 임시 폴더>/research/R-08/proj` (임시 폴더. 저장소에는 넣지 않았다).

### 6.1 설치한 버전 (`package.json`)

```json
{
  "type": "module",
  "devDependencies": {
    "typescript": "npm:@typescript/typescript6@6.0.2",
    "@typescript/native": "npm:typescript@7.0.2",
    "eslint": "10.14.0",
    "@eslint/js": "10.0.1",
    "typescript-eslint": "8.72.2",
    "eslint-plugin-functional": "10.0.1",
    "eslint-plugin-boundaries": "7.4.2",
    "eslint-plugin-react-hooks": "7.1.1",
    "@eslint-react/eslint-plugin": "5.30.1",
    "@eslint-community/eslint-plugin-eslint-comments": "4.8.1",
    "dependency-cruiser": "18.7.2",
    "knip": "6.45.1",
    "oxlint": "1.93.0",
    "oxlint-tsgolint": "7.0.2006",
    "@types/react": "19.3.0",
    "@types/react-dom": "19.3.0"
  },
  "dependencies": {
    "react": "19.3.1",
    "react-dom": "19.3.1",
    "zod": "4.6.7",
    "@tanstack/react-query": "5.107.1",
    "@tanstack/react-router": "1.170.46"
  }
}
```

`npx tsc -v` → `Version 7.0.2`, `npx tsc6 -v` → `Version 6.0.3`. Node 24.22.0, npm 11.19.1.

예제 `src`는 architecture.md 5장의 구조를 따른다: `app/`(App, main), `features/dashboard/{ui,model,index.ts}`, `features/chat`, `shared/{ds,ipc,platform,lib}`. IPC는 Tauri 없이 가짜 `invokeRaw`로 흉내 냈다.

### 6.2 결과

```
$ npx tsc -p .            → 종료 코드 0 (TS 7.0.2)
$ npx tsc6 -p .           → 종료 코드 0 (TS 6.0.3)
$ npx eslint --max-warnings 0 src   → 문제 0개, 종료 코드 0
$ node harness/check-lint-cases.mjs → 94/94 cases behaved as expected
$ node harness/check-tsc-cases.mjs  → PASS 42개 (21개 예제 × 2개 컴파일러), FAIL 0개
$ node harness/check-effective-config.mjs → effective config OK
```

린트 위반 예제 결과 (발췌. `got`은 실제로 보고된 규칙 전부):

```
PASS | as | expect @typescript-eslint/consistent-type-assertions | got @typescript-eslint/consistent-type-assertions, @typescript-eslint/no-unsafe-type-assertion
PASS | as-const-allowed | expect clean | got clean
PASS | ignored-result | expect functional/no-expression-statements | got functional/no-expression-statements
PASS | void-promise | expect no-void | got @typescript-eslint/no-floating-promises, functional/no-expression-statements, no-void
PASS | comment-jsx | expect local/no-comments | got local/no-comments
PASS | eslint-disable-file | expect local/no-comments | got (fatal) '/* eslint-disable */' has no effect because you have 'noInlineConfig' setting in your config., @eslint-community/eslint-comments/no-use, @typescript-eslint/explicit-module-boundary-types, @typescript-eslint/no-explicit-any, local/no-comments
PASS | effect-namespace | expect no-restricted-syntax | got no-restricted-imports, no-restricted-syntax
PASS | new-platform-file | expect local/forbidden-file | got local/forbidden-file
PASS | cross-feature | expect boundaries/dependencies | got boundaries/dependencies
PASS | deep-import | expect boundaries/dependencies | got boundaries/dependencies
PASS | money-number | expect local/no-money-coercion | got @typescript-eslint/no-import-type-side-effects, functional/prefer-tacit, local/no-money-coercion, no-restricted-syntax
PASS | react-subpath | expect no-restricted-imports | ...
PASS | sync-store-in-feature | expect no-restricted-imports | ...
PASS | proto-push-call | expect no-restricted-syntax | ...
PASS | js-file | expect local/forbidden-file | got local/forbidden-file
PASS | neverthrow-ignored | expect functional/no-expression-statements | got functional/no-expression-statements
GAP  | hex-concat-gap | expect KNOWN_GAP | got clean
GAP  | ref-callback-effect-gap | expect KNOWN_GAP | got clean
GAP  | throwing-builtin-gap | expect KNOWN_GAP | got clean
GAP  | neverthrow-unsafe-unwrap | expect KNOWN_GAP | got clean
```

(`(fatal)` 표시는 ESLint가 규칙 ID 없이 낸 경고다. `--max-warnings 0`이면 실패한다.)

tsc 위반 예제 결과 (TS 7.0.2와 6.0.3이 같았다):

```
catch-unknown.ts: TS18046        enum.ts: TS1294              exact-optional.ts: TS2375
fallthrough.ts: TS7029           implicit-any.ts: TS7006      implicit-override.ts: TS4114
implicit-return.ts: TS7030       index-signature-dot.ts: TS4111  namespace.ts: TS1294
null-check.ts: TS18047           opaque-money-minus.ts: TS2362 TS2363
opaque-money-parsefloat.ts: TS2345  param-property.ts: TS1294 TS6138
side-effect-import.ts: TS2882    unchecked-index.ts: TS2322   unreachable.ts: TS7027
unused-label.ts: TS7028          unused-local.ts: TS6133      unused-param.ts: TS6133
verbatim.ts: TS1484              string-brand-gap.ts: (오류 없음 = 빈틈 확인)
```

실험 중 고친 것 (초안이 처음부터 맞지는 않았다):

1. `Intl.NumberFormat.format`이 `string`을 받지 않아(TS2769) 금액의 내부 값을 `` `${number}` ``로 바꾸고, 좁히는 타입 술어 하나를 `shared/lib/money.ts` 예외로 두었다.
2. `invalidateQueries(...).then(...)`을 문장으로 쓰면 `no-expression-statements`에 걸려 `shared/platform/detach`를 만들었다.
3. `Array.prototype.push.call`이 `immutable-data`를 통과해 `prototype`·`call`·`apply`·`bind` 금지를 추가했다.
4. `type-declaration-immutability`의 기본 설정은 일반 타입 이름에 적용되지 않아 `identifiers: "^.+$"`를 지정했다.
5. `prefer-immutable-types`가 반환 타입(`ReactElement`)마다 오류를 내서 `returnTypes: { enforcement: "None" }`로 바꿨다.
6. boundaries의 `mode: "file"` 요소는 같은 폴더 파일끼리도 외부 의존으로 취급해 `index.ts`의 다시 내보내기를 막았다. 그래서 폴더 요소 + 금지 파일 블록 방식으로 바꿨다.

## 7. 설정 초안 (검증한 그대로)

### 7.1 `tsconfig.json`

```json
{
  "compilerOptions": {
    "target": "es2024",
    "lib": ["es2024", "dom", "dom.iterable"],
    "module": "esnext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "types": [],
    "noEmit": true,

    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "strictBindCallApply": true,
    "strictPropertyInitialization": true,
    "strictBuiltinIteratorReturn": true,
    "noImplicitThis": true,
    "useUnknownInCatchVariables": true,
    "alwaysStrict": true,

    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "noPropertyAccessFromIndexSignature": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "allowUnreachableCode": false,
    "allowUnusedLabels": false,
    "noUncheckedSideEffectImports": true,

    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "erasableSyntaxOnly": true,
    "forceConsistentCasingInFileNames": true,
    "libReplacement": false,

    "allowJs": false,
    "checkJs": false,
    "skipLibCheck": false,
    "noErrorTruncation": true
  },
  "include": ["src"]
}
```

실제 앱에서는 Vite 타입(`vite/client`)을 `types`에 넣어야 할 수 있다. 이 부분은 미검증이다. 위반 예제 폴더는 이 파일을 상속하고 `include`만 넓힌다(`{ "extends": "../../tsconfig.json", "compilerOptions": { "rootDir": "../.." }, "include": ["src", "../../src"] }`).

### 7.2 `eslint.config.mjs`

```js
import js from "@eslint/js";
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import eslintReact from "@eslint-react/eslint-plugin";
import boundaries from "eslint-plugin-boundaries";
import functional from "eslint-plugin-functional";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import local from "./eslint/local-plugin.mjs";

const TS_FILES = ["**/*.ts", "**/*.tsx"];
const PLATFORM = ["**/shared/platform/**"];
const IPC = ["**/shared/ipc/**"];
const LIB = ["**/shared/lib/**"];
const FEATURES = ["**/features/**"];
const TYPE_PREDICATE_ALLOWLIST = ["**/shared/lib/money.ts"];
const PLATFORM_FILE_ALLOWLIST = [
  "**/shared/platform/index.ts",
  "**/shared/platform/detach.ts",
  "**/shared/platform/useCoreEvent.ts",
  "**/shared/platform/useDocumentTitle.ts",
];

const effectHookNames = ["useEffect", "useLayoutEffect", "useInsertionEffect"];
const effectHookPattern = "/^use(Layout|Insertion)?Effect$/";

const syntax = {
  enum: { selector: "TSEnumDeclaration", message: "enum 금지. 문자열 리터럴 유니온을 쓴다." },
  namespace: { selector: "TSModuleDeclaration", message: "namespace·declare module·declare global 금지." },
  declare: { selector: ":matches(VariableDeclaration, FunctionDeclaration, TSTypeAliasDeclaration, TSInterfaceDeclaration)[declare=true]", message: "declare 금지. 존재하지 않는 값을 타입으로 꾸며낼 수 있다." },
  overload: { selector: "TSDeclareFunction", message: "함수 오버로드 금지. 구현 시그니처가 거짓말을 할 수 있다." },
  typePredicate: { selector: "TSTypePredicate", message: "타입 술어(x is T)와 단언 함수(asserts x) 금지. 스키마 파싱을 쓴다." },
  defaultCase: { selector: "SwitchCase[test=null]", message: "switch의 default 팔 금지. 모든 경우를 나열한다." },
  defaultExport: { selector: "ExportDefaultDeclaration", message: "기본 내보내기 금지." },
  exportAll: { selector: "ExportAllDeclaration", message: "export * 금지. 내보낼 이름을 나열한다." },
  dynamicImport: { selector: "ImportExpression", message: "동적 import 금지." },
  unaryPlus: { selector: "UnaryExpression[operator='+']", message: "단항 + 변환 금지." },
  throwingParse: { selector: "CallExpression[callee.property.name=/^(parse|parseAsync)$/]", message: "예외를 던지는 parse 금지. safeParse로 Result를 만든다." },
  reactNamespace: { selector: "ImportDeclaration[source.value='react'] > :matches(ImportNamespaceSpecifier, ImportDefaultSpecifier)", message: "react는 이름 있는 가져오기만 쓴다." },
  effectMember: { selector: `MemberExpression[property.name=${effectHookPattern}]`, message: "useEffect 계열은 shared/platform에서만." },
  effectIdentifier: { selector: `Identifier[name=${effectHookPattern}]`, message: "useEffect 계열은 shared/platform에서만." },
  prototype: { selector: "MemberExpression[property.name=/^(prototype|__proto__)$/]", message: "prototype 접근 금지." },
  callApply: { selector: "CallExpression[callee.property.name=/^(call|apply|bind)$/]", message: "call·apply·bind 금지." },
  testFocus: { selector: "MemberExpression[object.name=/^(describe|it|test|suite|bench)$/][property.name=/^(skip|only|todo|fails|skipIf|runIf|concurrent)$/]", message: "테스트 건너뛰기·집중 금지." },
  numericCoercion: { selector: "CallExpression[callee.name=/^(Number|parseFloat|parseInt|BigInt)$/]", message: "features에서 숫자 변환 금지. 코어가 보낸 표시용 숫자를 쓴다." },
  mathObject: { selector: "MemberExpression[object.name='Math']", message: "features에서 계산 금지." },
  styleProp: { selector: "JSXAttribute[name.name='style']", message: "인라인 스타일 금지. shared/ds 토큰을 쓴다." },
  rawControl: { selector: "JSXOpeningElement[name.name=/^(button|input|select|textarea|a|img)$/]", message: "features에서 기본 HTML 컨트롤 금지. shared/ds 컴포넌트를 쓴다." },
  colorLiteral: { selector: "Literal[value=/#[0-9a-fA-F]{3,8}\\b|\\b(rgba?|hsla?|oklch|oklab|lab|lch|color)\\(/]", message: "색상 값 직접 사용 금지. 디자인 토큰을 쓴다." },
  sizeLiteral: { selector: "Literal[value=/\\b\\d+(\\.\\d+)?(px|rem|em|vh|vw|pt)\\b/]", message: "크기 값 직접 사용 금지. 디자인 토큰을 쓴다." },
  arbitraryClass: { selector: "JSXAttribute[name.name='className'] Literal[value=/\\[[^\\]]*\\]/]", message: "임의 값 클래스 금지. 토큰 클래스를 쓴다." },
  colorTemplate: { selector: "TemplateElement[value.raw=/#[0-9a-fA-F]{3,8}\\b|\\d+px\\b/]", message: "템플릿 문자열 안의 색상·크기 값 금지." },
};

const base = [
  syntax.enum, syntax.namespace, syntax.declare, syntax.overload, syntax.typePredicate,
  syntax.defaultCase, syntax.defaultExport, syntax.exportAll, syntax.dynamicImport,
  syntax.unaryPlus, syntax.throwingParse, syntax.reactNamespace, syntax.testFocus,
  syntax.prototype, syntax.callApply,
];
const effects = [syntax.effectMember, syntax.effectIdentifier];
const featureOnly = [
  syntax.numericCoercion, syntax.mathObject, syntax.styleProp, syntax.rawControl,
  syntax.colorLiteral, syntax.sizeLiteral, syntax.arbitraryClass, syntax.colorTemplate,
];
const withoutPredicate = (list) => list.filter((entry) => entry !== syntax.typePredicate);

const restrictedImportPaths = (allowEffects) => [
  ...(allowEffects ? [] : [{ name: "react", importNames: [...effectHookNames, "useSyncExternalStore"], message: "useEffect 계열과 useSyncExternalStore는 shared/platform에서만." }]),
];
const restrictedImportPatterns = (allowTauri) => [
  ...(allowTauri ? [] : [{ group: ["@tauri-apps/*"], message: "Tauri 호출은 shared/ipc에서만." }]),
  { group: ["**/index", "**/index.ts", "**/index.tsx"], message: "index 경로를 직접 쓰지 않는다. 폴더를 가져온다." },
  { group: ["react/*", "react-dom/*", "!react-dom/client"], message: "react 하위 경로 가져오기 금지." },
];

const restrictedSyntax = (list) => ["error", ...list];
const restrictedImports = ({ allowEffects, allowTauri }) => [
  "error",
  { paths: restrictedImportPaths(allowEffects), patterns: restrictedImportPatterns(allowTauri) },
];

export default defineConfig(
  {
    ignores: ["node_modules/**", "dist/**", "eslint/**", "harness/**/*.mjs", "*.config.*"],
  },
  {
    linterOptions: {
      noInlineConfig: true,
      reportUnusedDisableDirectives: "error",
      reportUnusedInlineConfigs: "error",
    },
  },
  {
    files: TS_FILES,
    extends: [
      js.configs.recommended,
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
      functional.configs.externalTypeScriptRecommended,
      functional.configs.recommended,
      functional.configs.stylistic,
      eslintReact.configs["strict-type-checked"],
      reactHooks.configs.flat["recommended-latest"],
    ],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: {
      local,
      boundaries,
      "@eslint-community/eslint-comments": eslintComments,
    },
    settings: {
      "import/resolver": { node: { extensions: [".ts", ".tsx"] } },
      "boundaries/legacy-templates": false,
      "boundaries/elements": [
        { type: "app", pattern: "app" },
        { type: "feature", pattern: "features/*", capture: ["featureName"] },
        { type: "ds", pattern: "shared/ds" },
        { type: "ipc", pattern: "shared/ipc" },
        { type: "platform", pattern: "shared/platform" },
        { type: "lib", pattern: "shared/lib" },
      ],
    },
    rules: {
      "local/no-comments": "error",
      "local/no-money-coercion": "error",
      "@eslint-community/eslint-comments/no-use": ["error", { allow: [] }],

      "@typescript-eslint/ban-ts-comment": ["error", {
        "ts-check": true, "ts-expect-error": true, "ts-ignore": true, "ts-nocheck": true,
        minimumDescriptionLength: 100000,
      }],
      "@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
      "@typescript-eslint/no-non-null-assertion": "error",
      "@typescript-eslint/no-explicit-any": ["error", { fixToUnknown: false, ignoreRestArgs: false }],
      "@typescript-eslint/explicit-function-return-type": ["error", {
        allowExpressions: false, allowTypedFunctionExpressions: true, allowHigherOrderFunctions: false,
        allowDirectConstAssertionInArrowFunctions: false, allowConciseArrowFunctionExpressionsStartingWithVoid: false,
      }],
      "@typescript-eslint/explicit-module-boundary-types": "error",
      "@typescript-eslint/strict-boolean-expressions": ["error", {
        allowString: false, allowNumber: false, allowNullableObject: false,
        allowNullableBoolean: false, allowNullableString: false, allowNullableNumber: false,
        allowNullableEnum: false, allowAny: false,
      }],
      "@typescript-eslint/switch-exhaustiveness-check": ["error", {
        allowDefaultCaseForExhaustiveSwitch: false,
        considerDefaultExhaustiveForUnions: false,
        requireDefaultForNonUnion: false,
      }],
      "@typescript-eslint/no-floating-promises": ["error", { ignoreVoid: false, ignoreIIFE: false, checkThenables: true }],
      "@typescript-eslint/no-misused-promises": "error",
      "@typescript-eslint/no-unused-vars": ["error", {
        args: "all", caughtErrors: "all", ignoreRestSiblings: false, reportUsedIgnorePattern: true,
      }],
      "@typescript-eslint/method-signature-style": ["error", "property"],
      "@typescript-eslint/consistent-type-definitions": ["error", "type"],
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/consistent-type-exports": "error",
      "@typescript-eslint/no-import-type-side-effects": "error",
      "@typescript-eslint/prefer-readonly": "error",
      "@typescript-eslint/no-unsafe-type-assertion": "error",
      "@typescript-eslint/strict-void-return": "error",

      "functional/no-let": "error",
      "functional/no-loop-statements": "error",
      "functional/no-classes": "error",
      "functional/no-class-inheritance": "error",
      "functional/no-this-expressions": "error",
      "functional/no-throw-statements": "error",
      "functional/no-try-statements": "error",
      "functional/no-promise-reject": "error",
      "functional/immutable-data": ["error", { ignoreClasses: false, ignoreImmediateMutation: false }],
      "functional/no-expression-statements": ["error", { ignoreVoid: true, ignoreSelfReturning: false }],
      "functional/no-conditional-statements": "off",
      "functional/no-mixed-types": "off",
      "functional/no-return-void": "off",
      "functional/functional-parameters": ["error", { enforceParameterCount: false }],
      "functional/prefer-immutable-types": ["error", {
        enforcement: "ReadonlyShallow",
        ignoreInferredTypes: true,
        returnTypes: { enforcement: "None" },
      }],
      "functional/type-declaration-immutability": ["error", {
        rules: [{ identifiers: "^.+$", immutability: "ReadonlyShallow", comparator: "AtLeast" }],
        ignoreInterfaces: false,
      }],
      "functional/readonly-type": ["error", "keyword"],

      "no-var": "error",
      "prefer-const": "error",
      "no-param-reassign": ["error", { props: true }],
      "no-plusplus": "error",
      "no-void": "error",
      "no-nested-ternary": "error",
      "no-console": "error",
      "no-alert": "error",
      "no-debugger": "error",
      "no-eval": "error",
      "no-new-func": "error",
      "no-labels": "error",
      "no-sequences": "error",
      "eqeqeq": ["error", "always"],
      "no-restricted-globals": ["error", "event", "name", "length", "status", "globalThis", "window", "self"],
      "no-restricted-properties": ["error",
        { object: "Object", property: "assign" },
        { object: "Object", property: "defineProperty" },
        { object: "Object", property: "defineProperties" },
        { object: "Object", property: "setPrototypeOf" },
        { object: "Object", property: "getOwnPropertySymbols" },
        { object: "Reflect" },
        { object: "Proxy" },
        { object: "JSON", property: "parse" },
      ],
      "complexity": ["error", 8],
      "max-depth": ["error", 3],
      "max-params": ["error", 3],
      "max-lines": ["error", { max: 250, skipBlankLines: false, skipComments: false }],
      "max-lines-per-function": ["error", { max: 60, skipBlankLines: false, skipComments: false, IIFEs: true }],

      "@eslint-react/no-array-index-key": "error",

      "boundaries/no-unknown-files": "error",
      "boundaries/no-unknown-dependencies": "error",
      "boundaries/dependencies": ["error", {
        default: "disallow",
        policies: [
          { from: { element: { type: "app" } }, allow: { to: { element: { types: ["feature", "ds", "ipc", "platform", "lib"] } } } },
          { from: { element: { type: "feature" } }, allow: { to: { element: { type: "feature", captured: { featureName: "{{ from.element.captured.featureName }}" } } } } },
          { from: { element: { type: "feature" } }, allow: { to: { element: { types: ["ds", "ipc", "platform", "lib"] } } } },
          { from: { element: { type: "ds" } }, allow: { to: { element: { types: ["ds", "lib"] } } } },
          { from: { element: { type: "ipc" } }, allow: { to: { element: { types: ["ipc", "lib"] } } } },
          { from: { element: { type: "platform" } }, allow: { to: { element: { types: ["platform", "ipc", "lib"] } } } },
          { from: { element: { type: "lib" } }, allow: { to: { element: { type: "lib" } } } },
          { disallow: { to: { element: { types: ["feature", "ds", "ipc", "platform", "lib"], fileInternalPath: "!index.ts" } } } },
          { allow: { dependency: { relationship: { to: "internal" } } } },
        ],
      }],
      "no-restricted-syntax": restrictedSyntax([...base, ...effects]),
      "no-restricted-imports": restrictedImports({ allowEffects: false, allowTauri: false }),
    },
  },
  {
    files: FEATURES,
    rules: { "no-restricted-syntax": restrictedSyntax([...base, ...effects, ...featureOnly]) },
  },
  {
    files: PLATFORM,
    rules: {
      "no-restricted-syntax": restrictedSyntax(base),
      "no-restricted-imports": restrictedImports({ allowEffects: true, allowTauri: false }),
      "functional/no-expression-statements": "off",
      "functional/immutable-data": "off",
    },
  },
  {
    files: IPC,
    rules: { "no-restricted-imports": restrictedImports({ allowEffects: false, allowTauri: true }) },
  },
  {
    files: LIB,
    rules: { "local/no-money-coercion": "off" },
  },
  {
    files: PLATFORM,
    ignores: PLATFORM_FILE_ALLOWLIST,
    rules: { "local/forbidden-file": "error" },
  },
  {
    files: ["**/*.d.ts"],
    rules: { "local/forbidden-file": "error" },
  },
  {
    files: ["**/*.{js,jsx,mjs,cjs,mts,cts}"],
    ignores: ["eslint/**", "harness/**/*.mjs", "*.config.*"],
    plugins: { local },
    rules: { "local/forbidden-file": "error" },
  },
  {
    files: TYPE_PREDICATE_ALLOWLIST,
    rules: { "no-restricted-syntax": restrictedSyntax(withoutPredicate(base).concat(effects)) },
  },
);
```

설정 작성 시 주의할 점: flat config에서는 같은 규칙을 뒤 블록에 다시 쓰면 앞의 옵션을 **합치지 않고 통째로 바꾼다**. 그래서 `no-restricted-syntax`와 `no-restricted-imports`는 함수로 배열을 조립해 블록마다 전체 목록을 넘긴다. 이 동작은 유효 설정 검사(2.10의 3번)로 지킨다.

### 7.3 `eslint/local-plugin.mjs`

```js
const noComments = {
  meta: {
    type: "problem",
    docs: { description: "Disallow every comment, including directives and JSX comments" },
    schema: [],
    messages: { comment: "주석 금지: 설명은 이름, 타입, 테스트, docs/에 쓴다." },
  },
  create(context) {
    return {
      Program() {
        for (const comment of context.sourceCode.getAllComments()) {
          context.report({ loc: comment.loc, messageId: "comment" });
        }
      },
    };
  },
};

const typeContainsBrandKey = (checker, type, brandName) => {
  if (type.isUnion() || type.isIntersection()) {
    return type.types.some((part) => typeContainsBrandKey(checker, part, brandName));
  }
  return checker
    .getPropertiesOfType(type)
    .some((symbol) => symbol.escapedName.toString().includes(brandName));
};

const arithmeticOperators = new Set(["+", "-", "*", "/", "%", "**", "<", ">", "<=", ">="]);
const numericCoercions = new Set(["Number", "parseFloat", "parseInt", "BigInt", "String"]);

const noMoneyCoercion = {
  meta: {
    type: "problem",
    docs: { description: "Disallow arithmetic, comparison and numeric coercion on branded money values" },
    schema: [{ type: "object", properties: { brandKey: { type: "string" } }, additionalProperties: false }],
    messages: { money: "금액 값에 산술·비교·숫자 변환 금지. 코어가 계산한 값을 shared/lib 형식 함수로만 표시한다." },
  },
  create(context) {
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const brandKey = context.options[0]?.brandKey ?? "decimalKey";
    const isMoney = (node) =>
      typeContainsBrandKey(checker, services.getTypeAtLocation(node), brandKey);
    return {
      BinaryExpression(node) {
        if (arithmeticOperators.has(node.operator) && (isMoney(node.left) || isMoney(node.right))) {
          context.report({ node, messageId: "money" });
        }
      },
      UnaryExpression(node) {
        if ((node.operator === "+" || node.operator === "-") && isMoney(node.argument)) {
          context.report({ node, messageId: "money" });
        }
      },
      CallExpression(node) {
        if (
          node.callee.type === "Identifier" &&
          numericCoercions.has(node.callee.name) &&
          node.arguments.some((argument) => isMoney(argument))
        ) {
          context.report({ node, messageId: "money" });
        }
      },
      TemplateLiteral(node) {
        if (node.expressions.some((expression) => isMoney(expression))) {
          context.report({ node, messageId: "money" });
        }
      },
    };
  },
};

const forbiddenFile = {
  meta: {
    type: "problem",
    docs: { description: "Report files whose extension is outside the TypeScript source allowlist" },
    schema: [],
    messages: { file: "src에는 .ts·.tsx 파일만 둔다 (.d.ts 포함 금지)." },
  },
  create(context) {
    return {
      Program(node) {
        context.report({ node, messageId: "file" });
      },
    };
  },
};

export default {
  meta: { name: "local" },
  rules: {
    "no-comments": noComments,
    "no-money-coercion": noMoneyCoercion,
    "forbidden-file": forbiddenFile,
  },
};
```

`no-money-coercion`은 금액 타입의 비공개 symbol 키 이름(`decimalKey`)을 속성 이름에서 찾는다. 이름 문자열에 기대므로, 실제 구현에서는 키 이름을 보호 파일에 고정하거나 타입 별칭 이름으로 판정하도록 바꾸는 편이 안전하다.

### 7.4 금액 타입 (`shared/lib/money.ts`, 예외 파일)

```ts
import { err, ok, type Result } from "./result";

const decimalKey: unique symbol = Symbol("decimal");

export type Currency = "KRW" | "USD";

type DecimalLiteral = `${number}`;

export type Money = {
  readonly currency: Currency;
  readonly [decimalKey]: DecimalLiteral;
};

export type MoneyParseError =
  | { readonly kind: "malformed-decimal"; readonly input: string }
  | { readonly kind: "unknown-currency"; readonly input: string };

const decimalPattern = /^-?(0|[1-9]\d*)(\.\d+)?$/u;

const isDecimalLiteral = (input: string): input is DecimalLiteral =>
  decimalPattern.test(input);

const parseCurrency = (input: string): Result<Currency, MoneyParseError> => {
  switch (input) {
    case "KRW":
      return ok("KRW");
    case "USD":
      return ok("USD");
  }
  return err({ kind: "unknown-currency", input });
};

export const parseMoney = (
  amount: string,
  currency: string,
): Result<Money, MoneyParseError> => {
  const parsedCurrency = parseCurrency(currency);
  if (!parsedCurrency.ok) {
    return parsedCurrency;
  }
  return isDecimalLiteral(amount)
    ? ok({ currency: parsedCurrency.value, [decimalKey]: amount })
    : err({ kind: "malformed-decimal", input: amount });
};

const formatters: Readonly<Record<Currency, Intl.NumberFormat>> = {
  KRW: new Intl.NumberFormat("ko-KR", { style: "currency", currency: "KRW" }),
  USD: new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }),
};

export const formatMoney = (money: Money): string =>
  formatters[money.currency].format(money[decimalKey]);
```

### 7.5 그 밖의 검증된 설정

`knip.json`:

```json
{
  "$schema": "https://unpkg.com/knip@6/schema.json",
  "entry": ["src/app/main.tsx"],
  "project": ["src/**/*.{ts,tsx}"],
  "rules": { "files": "error", "exports": "error", "types": "error", "dependencies": "error", "devDependencies": "error", "unlisted": "error", "duplicates": "error", "enumMembers": "error", "nsExports": "error", "nsTypes": "error", "unresolved": "error", "binaries": "error" }
}
```

`.dependency-cruiser.cjs` (`check-full`용):

```js
module.exports = {
  forbidden: [
    { name: "no-circular", severity: "error", from: {}, to: { circular: true } },
    {
      name: "feature-isolation",
      severity: "error",
      from: { path: "^src/features/([^/]+)/" },
      to: { path: "^src/features/([^/]+)/", pathNot: "^src/features/$1/" },
    },
  ],
  options: {
    tsConfig: { fileName: "tsconfig.json" },
    tsPreCompilationDeps: true,
    doNotFollow: { path: "node_modules" },
    enhancedResolveOptions: { extensions: [".ts", ".tsx"] },
  },
};
```

하네스 실행기 `harness/check-lint-cases.mjs`의 핵심 (전문은 예제 폴더에 있다):

```js
const results = await eslint.lintFiles(cases.map((c) => c.file));
for (const c of cases) {
  const result = results.find((r) => r.filePath.endsWith(c.file));
  const rules = [...new Set(result.messages.map((m) => m.ruleId ?? `(fatal) ${m.message}`))].sort();
  const gap = c.primary === "KNOWN_GAP";
  const pass = gap || (c.primary === null ? rules.length === 0 : rules.includes(c.primary));
}
```

## 8. 출처 (모두 2026-11-11 확인)

- [TS 7.0 발표] Announcing TypeScript 7.0, Microsoft DevBlogs, 2026-07-08 — https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
- [TS 6.0 발표] Announcing TypeScript 6.0, 2026-03-23 — https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
- [TS 7.1 beta 발표] Announcing TypeScript 7.1 Beta, 2026-10-20 — https://devblogs.microsoft.com/typescript/announcing-typescript-7-1-beta/
- [tsconfig 참조] TSConfig Reference — https://www.typescriptlang.org/tsconfig/
- npm 레지스트리 (`npm view <패키지> version time peerDependencies`): typescript, @typescript/typescript6, typescript-eslint 8.72.2(`typescript >=4.8.4 <6.1.0`), eslint 10.14.0, eslint-plugin-functional 10.0.1, eslint-plugin-boundaries 7.4.2, dependency-cruiser 18.7.2, knip 6.45.1, @biomejs/biome 2.6.6, oxlint 1.93.0, oxlint-tsgolint 7.0.2006, neverthrow 8.2.0, eslint-plugin-neverthrow 1.1.4, effect 4.1.4, zod 4.6.7, valibot 1.5.0, arktype 2.2.9, @tanstack/react-query 5.107.1, @tanstack/react-router 1.170.46, react-router 8.5.1, react 19.3.1 — https://www.npmjs.com/
- typescript-eslint 이슈 #12518 "TypeScript 7.0.2 Support" (closed, not planned, 2026-07-08) — https://github.com/typescript-eslint/typescript-eslint/issues/12518
- [PR #12803] typescript-eslint "add projectService.EXPERIMENTAL_backend for TypeScript 7.1 native parser support over IPC" (2026-11-10 병합) — https://github.com/typescript-eslint/typescript-eslint/pull/12803
- typescript-eslint 이슈 #13181 (tsgo 백엔드에서 TS 6 제거, 2026-11-10 완료) — https://github.com/typescript-eslint/typescript-eslint/issues/13181
- typescript-eslint 규칙: consistent-type-assertions — https://typescript-eslint.io/rules/consistent-type-assertions/ , switch-exhaustiveness-check — https://typescript-eslint.io/rules/switch-exhaustiveness-check/
- [ts-eslint 성능] Typed Linting Performance — https://typescript-eslint.io/troubleshooting/typed-linting/performance/
- [ts-eslint FAQ] ESLint FAQs (`--cache`) — https://typescript-eslint.io/troubleshooting/faqs/eslint
- ESLint Configuration Files (`linterOptions`, 기본 검사 파일) — https://eslint.org/docs/latest/use/configure/configuration-files
- eslint-plugin-functional no-expression-statements — https://github.com/eslint-functional/eslint-plugin-functional/blob/main/docs/rules/no-expression-statements.md
- JS Boundaries: Quick start — https://www.jsboundaries.dev/docs/quick-start/ , Selectors — https://www.jsboundaries.dev/docs/selectors/ , Dependency selector — https://www.jsboundaries.dev/docs/selectors/dependency/ , [boundaries 정책] Policies — https://www.jsboundaries.dev/docs/policies/ , entry-point 이전 안내 — https://www.jsboundaries.dev/docs/rules/entry-point/
- [oxlint 타입 인식] Type-aware linting — https://oxc.rs/docs/guide/usage/linter/type-aware.html
- [oxlint JS 플러그인] JS plugins — https://oxc.rs/docs/guide/usage/linter/js-plugins.html
- [Biome 플러그인] Linter plugins — https://biomejs.dev/linter/plugins/
- [Biome 2.1] Biome v2.1 — https://biomejs.dev/blog/biome-v2-1/
- [React: Effect] You Might Not Need an Effect — https://react.dev/learn/you-might-not-need-an-effect
- [React: ref 콜백] Common components, ref callback — https://react.dev/reference/react-dom/components/common
- [React: Component] Component (Error Boundary) — https://react.dev/reference/react/Component
- [TanStack Query 함수] Query Functions — https://tanstack.com/query/latest/docs/framework/react/guides/query-functions
- [TanStack 데이터 로딩] Router Data Loading — https://tanstack.com/router/latest/docs/framework/react/guide/data-loading
- [TanStack redirect] redirect function — https://tanstack.com/router/latest/docs/api/router/redirectFunction
- [Zod 브랜드] Zod API, Branded types — https://zod.dev/api?id=branded-types
- [Valibot brand] — https://valibot.dev/api/brand/
- neverthrow 저장소 상태 (GitHub API: pushed_at 2026-02-14, open issues 93) — https://github.com/supermacro/neverthrow
- 보조 출처(블로그, 사실 확인에는 쓰지 않음): https://dev.to/herrington_darkholme/trying-out-biome-v2s-type-inference-and-knowing-its-limits-1j36
