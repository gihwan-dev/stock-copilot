# ADR 0012: 로컬 ESLint 규칙, 파일 허용 목록, 브랜드 기반 (H-08, H-38)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-08, H-38
- 반영한 문서: 없음 (harness.md 2.1, 4.2, 6.8, architecture.md 5장·5.3, ADR 0003, ADR 0009를 따르고 어긋나거나 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness 4.2는 로컬 규칙 4개(`local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`, `local/no-raw-types`), `.catch`·두 인자 `.then` 금지, 파일 허용 목록 표, `UiText`·`messages.ts`를 정한다. ADR 0009는 이 규칙들을 H-08로 넘겼다. Rust 쪽 `raw-types` 리뷰에서 "금지 이름 목록" 방식이 별칭으로 뚫렸으므로, TS 쪽은 표기된 이름이 아니라 타입 검사기가 푼 타입으로 판정해야 한다.

## 실측

환경: macOS 26, Node 24.22.0, ESLint 10.14.0, typescript-eslint 8.72.2(TS 6.0.2 API), TS 7.0.2, Zod 4.6.7.

| 항목 | 명령 | 결과 |
|---|---|---|
| Zod 고정 버전 | `pnpm view zod@4.6.7`, `pnpm -C apps/desktop add --save-exact zod@4.6.7` | 있음. 잠금 파일에 이미 전이 의존으로 있어 3줄만 바뀌었다 |
| Zod 브랜드 모양 | `zod/v4/core/core.d.cts` | `output & { [$brand]: { [k in N]: true } }`. `$brand`는 `unique symbol`이다. 브랜드 값은 파싱(`safeParse`)이나 단언으로만 생긴다 |
| `src/**/*` 블록 | 임시 폴더에서 `eslint src` | `src/**/*`만 있는 블록은 디렉터리 순회 대상을 넓히지 않아 `.css`, `x.TSX`가 검사되지 않았다. `src/**/*.*`, `src/**/.*`, `src/**/[!.]*`는 검사됐다 |
| 확장자 대소문자 | 같은 실험 | `src/**/*.ts`는 `x.TS`에 맞지 않는다(대소문자 구분). 대문자 확장자 파일은 위 블록으로만 잡힌다 |
| `node_modules` 기본 무시 | `src/features/node_modules/evil.ts`를 두고 `eslint src` | 통과했다(검사 안 됨). `ignores`에 `!src/**/node_modules/`를 넣자 "not found by the project service"로 실패했다 |
| `Readonly<string & 브랜드>` | `messages.ts`에서 `Readonly<z.output<…>>` | 문자열과 객체의 교차를 매핑해 문자열 성질이 사라졌다(TS2352). 브랜드 객체에만 `Readonly`를 씌운 `string & Readonly<$brand<N>>`는 `functional/type-declaration-immutability`와 단언 둘 다 통과했다 |
| 라이브러리 기본 타입 인자 | `(): ReactElement` | `ReactElement`의 기본 인자(`unknown`, `string \| …`)가 원시 타입으로 잡혔다. 선언된 기본값과 같은 인자는 건너뛰도록 했다 |
| 튜플 색인 키 | `["buy", "sell"] as const` 내보내기 | 튜플의 숫자 색인 키(`number`)가 원시 타입으로 잡혔다. 색인 키는 프로젝트에서 선언한 타입에서만 본다 |
| 금액 tsc | `harness/tsc-violations/money`를 `tsc`, `tsc6`로 | `parseFloat(a) + Number(b)`는 TS2345, `a + b`는 TS2365, `a - b`는 TS2362·TS2363, `Quantity`를 `Money`로는 TS2739. 두 컴파일러가 같았다. 문자열 브랜드는 모두 통과했다 |
| 속도 | `node --test apps/desktop/tests/*.test.mjs` | 허용 목록 변형 테스트(원소마다 전체 예제 검사)가 약 20초를 더한다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 로컬 규칙은 `apps/desktop/eslint/local-plugin.mjs`와 `eslint/rules/*.mjs`. 설정 본문은 `eslint/app-config.mjs`의 `appConfig(목록)`이고 `eslint.config.mjs`는 `eslint/allowlists.mjs`를 넘겨 부르기만 한다 | 테스트가 허용 목록 원소 하나를 빼거나 더한 설정을 만들어 예제가 실패하는지 본다 | 설정 파일 안에 목록 상수(첫 판) | 브리프 10 |
| `local/no-comments`: `getAllComments()`의 모든 주석(줄, 블록, 문서, JSX, `///`, `#!`)을 보고하고, `guard/disable-directive-form`과 같은 판정(`isAllowedDirective`)을 통과한 `eslint-disable-next-line` 줄 주석만 뺀다 | harness 2.1의 한 형태만 남긴다 | 지시문을 따로 두고 주석만 보기 | harness 2.1 |
| `local/no-money-coercion`: 금액은 "`shared/lib/money.ts`에서 선언한 `unique symbol` 키를 가진 타입"(제네릭 제약 포함)으로 판정한다. 막는 것: 모든 이항 연산(`=== undefined`/`null` 비교만 허용), 복합 대입, 단항 `+ - ~ !`, 증감, `switch`, 템플릿, `Number`·`String`·`Boolean`·`BigInt`·`Symbol`·`parseFloat`·`parseInt`·`*.stringify` 호출, 매개변수가 `any`·`unknown`·`object`·`{}`인 호출(`Object.getOwnPropertySymbols`, `Reflect.ownKeys`, `console.*` 등), `money.ts`에 선언되지 않은 멤버(`toString`, `valueOf`), 계산된 멤버, 구조 분해의 계산된 키·나머지, 키 상수 참조. 키 읽기는 `money.ts`와 `format.ts`만 | R-08 초안은 키 이름 문자열로 판정했다(문서가 스스로 위험하다고 적음). 선언 파일로 판정하면 이름을 바꿔도 피할 수 없다 | 타입 별칭 이름, 키 이름 문자열 | R-08 7.3 |
| 금액 타입마다 키가 따로다(`moneyKey`, `quantityKey`, `priceKey`, `ratioKey`, `probabilityKey`). 키는 `format.ts`가 읽도록 내보내고, 다른 파일의 참조는 위 규칙이 막는다 | 키 하나면 `Quantity`와 `Ratio`가 구조적으로 같아 서로 대입된다 | 키 하나 + 종류 필드 | ADR 0003 |
| `local/no-raw-types`: (1) 표기 — `string`·`number`·`bigint`·`boolean`·`unknown`·`any`·`object` 키워드, `true`·`false` 리터럴 타입, `String`·`Number`·`BigInt`·`Boolean`·`Object` 참조. (2) 해석 — 타입 참조, `typeof`, `keyof`·`readonly` 연산자, 색인 접근, 조건부, 매핑, `import()` 타입, 템플릿 리터럴 타입, 타입 리터럴을 검사기로 풀어 원시·열린 타입이 나오면 실패. 유니온은 구성원마다, 교차는 "원시 + Zod `$brand` 객체"일 때만 통과(`string & {}` 실패), 객체는 타입 인자·별칭 인자(선언된 기본값 제외)·색인 값과, 프로젝트에서 선언한 타입이면 속성·호출 시그니처까지 본다. `{}`(빈 익명 객체)와 `Object`는 열린 타입이다. (3) 추론 — 이 파일이 선언해 내보내는 값의 타입(함수면 모든 시그니처의 매개변수와 반환)과, 반환이 JSX인 함수의 매개변수(props) | 별칭, 가져온 별칭, `ReturnType<typeof f>`, `` `${number}` ``, `keyof`, 제네릭 기본값, `unknown`·`{}` 뒤에 숨기를 표기 검사만으로는 못 막는다 | 금지 이름 목록 | 브리프 6, Rust 리뷰 |
| `unknown`도 타입 자리에서 금지한다(허용 목록 파일 밖) | harness 4장 "`unknown`은 경계에서 Zod로 파싱". 받는 쪽이 `unknown`이면 원시 값이 그대로 들어온다 | `unknown` 허용 | harness 4장 |
| 다시 내보내기(`export { x } from`)는 원래 파일에서 판정한다 | `shared/lib/index.ts`가 허용 목록 파일(`brand.ts`)의 `parseTicker(input: unknown)`을 내보낼 수 있어야 한다 | 모든 내보내기를 그 자리에서 판정 | 실측 |
| 브랜드 타입은 `string & Readonly<z.core.$brand<"N">>` 꼴로 `shared/lib/brand.ts`에 둔다(`UiText` 포함). 생성은 Zod 스키마 + `safeParse`(경계 값)나 단언(코드가 정한 값: `format.ts`의 `DisplayText`, `messages.ts`의 `UiText`) | Zod 브랜드 값은 파싱이나 단언으로만 생긴다. 문구와 표시 문자열은 실패할 수 없는데 `safeParse`의 실패 분기를 채울 값이 없다 | `messages.ts`도 `safeParse`(실패 분기에 넣을 `UiText`가 없다), 불투명 객체 문구(React가 그리지 못한다) | 실측, 브리프 7·8 |
| `local/no-brand-creation`(새 규칙): 모드 `forbid`(기본) — Zod `.brand`(멤버·구조 분해), 대상이 브랜드인 단언, Zod 함수에 브랜드 타입 인자(`z.custom<Ticker>()`), `$brand`·`BRAND` 참조 실패. 모드 `brand-file`(원시 타입 허용 목록 파일) — 이름이 리터럴인 `.brand<"N">()`와 브랜드 대상 단언만, `UiText`는 실패. 모드 `messages`(`src/**/messages.ts`) — 문자열 리터럴 타입 값을 `UiText`로 단언하는 것과 `.brand<"UiText">()`만. 이 두 모드의 파일에서는 `consistent-type-assertions`·`no-unsafe-type-assertion`을 끄고 이 규칙이 단언을 맡는다 | harness 4.2 "브랜드는 `shared/lib/brand`의 생성 함수와 `shared/ipc`의 Zod 스키마로만", "`UiText`는 `messages.ts`에서만" | `no-raw-types`에 합치기(한 줄 끄기가 두 금지를 함께 풀게 된다) | ADR 0009 "금지 하나에 규칙 하나" |
| `local/no-ui-text`(새 규칙): 한글이 든 문자열 리터럴·템플릿 조각·JSX 글자는 `messages.ts` 밖 어디서나 실패. `app`·`features`·`shared/ds`(`.ts`·`.tsx`)에서는 공백이 아닌 JSX 글자와 JSX 자식 자리의 문자열·템플릿 식도 실패. 공백뿐인 JSX 글자, 리터럴 유니온 값(`'buy'`), 키·경로 같은 영문 문자열은 통과 | 영문 키와 영문 문구를 문법으로 가를 수 없다. 화면 문구는 한국어라 한글 판정이 가장 단순하다. 영문 문구가 DS props로 들어가는 길은 props 타입(`UiText`)이 `tsc`에서 막는다 | 모든 문자열 리터럴 금지(`key`, 경로, 리터럴 유니온 값까지 막힌다) | 브리프 8 |
| `messages.ts`는 이름 규칙(`src/**/messages.ts`)이다. 목록은 `{ 키: { ko: "문장" } } as const`이고 `text(key, locale)`이 `UiText`를 돌려준다. `Locale`은 지금 `"ko"`뿐 | 브리프 8 "이름 규칙", 다국어는 `Locale`에 값을 더해 붙인다 | 고정 파일 하나 | 브리프 8 |
| `local/forbidden-file`: 옵션 `folders`(지금은 `src/shared/platform` → 플랫폼 허용 목록)와 `checkExtension`. TS 블록에서는 폴더 판정만, `.ts`·`.tsx`가 아닌 모든 파일(`src/**/*.*`, `src/**/.*`, `src/**/[!.]*`에서 TS 파일과 기존 금지 확장자 블록을 뺀 것)은 빈 파서로 읽어 확장자 판정까지 한다. 그 블록은 `noInlineConfig: true`. 빈 `.gitkeep`만 통과 | `src/features/.gitkeep`이 있어야 폴더 검사 테스트가 통과한다. 내용이 있으면 실패 | 기존 `guard/forbidden-extension`과 합치기(기존 예제·테스트를 바꿔야 한다. 뒤 작업에서 판단) | ADR 0009 |
| `src` 안의 `node_modules` 폴더도 검사한다(`ignores`에 `!src/**/node_modules/`) | ESLint 기본 무시로 그 안의 파일이 검사 없이 통과했다 | - | 실측 |
| `.catch`·두 인자 `.then`은 `guard/no-promise-catch`(목록 규칙, 항목 2개: `catch` 멤버·구조 분해, 인자 2개 이상이거나 펼침 인자가 있는 `then`). 허용 목록 `PROMISE_REJECTION_ALLOWLIST` = `src/shared/ipc/call.ts`, `src/shared/platform/detach.ts` | 기존 목록 규칙 틀(항목마다 예제, 항목 하나 지우면 실패)을 그대로 쓴다 | 로컬 규칙 | ADR 0009 |
| IPC 호출 어댑터는 `shared/ipc/call.ts`의 `settle(promise): Promise<Result<T, IpcFailure>>`. Tauri `invoke` 연결은 하지 않는다 | 브리프 5, U-04·H-13 몫 | - | 브리프 |
| 원시 타입 허용 목록: `shared/lib/brand.ts`, `shared/lib/money.ts`, `shared/lib/format.ts`, `shared/ipc/schema.ts`, `shared/ipc/bindings.ts`(뒤 둘은 아직 없음) | harness 4.2 표 | - | harness 4.2 |
| 예제 판정 키에 로컬 규칙의 메시지도 넣는다(`local/<규칙> :: <메시지>`). 메시지마다 예제가 있어야 한다 | 같은 규칙의 다른 금지(예: `UiText`는 `messages.ts`에서만 / 이름 없는 브랜드)를 가른다 | 규칙 ID만 | ADR 0009 판정 방식 |
| `Result`는 `{ kind: "ok", value } \| { kind: "err", error }`(`shared/lib/result.ts`) | 불린 판별자(`ok: true`)는 `true` 리터럴 타입 금지에 걸린다 | R-08 초안의 `ok` 불린 | harness 4.2 |

### 끌 수 있는 금지와 끌 수 없는 금지 (ADR 0009 표에 더함)

| 끌 수 없음 (`no-restricted-disable`) | 끌 수 있음 |
|---|---|
| `local/no-comments`, `local/no-money-coercion`, `local/forbidden-file`, `guard/no-promise-catch` (브리프 9) | 없음 |
| `local/no-raw-types`, `local/no-brand-creation`, `local/no-ui-text` — 모두 파일 허용 목록(원시 타입 목록, `messages.ts`)으로 예외를 주므로 harness 2.1 원칙에 따라 끌 수 없다 | |

## 결과

- 기존 테스트 변경: `apps/desktop/tests/eslint-config.test.mjs`의 끌 수 없는 규칙 고정 목록에 위 4개 이름을 더했다(지우거나 바꾼 원소 없음). `cases.json`의 기존 예제 148개에 새 규칙의 키를 더했다(대부분 `unknown` 반환과 한글 리터럴). 아직 없는 규칙을 끄려던 예제 3개(`undisableable-{no-comments,money,forbidden-file}.ts`)는 "규칙 없음" 보고 대신 "쓰이지 않는 끄기"를 기대하게 바꿨다. 예제 파일 내용은 `src/shared/lib/money.ts`에 금액 타입을 덧붙인 것 말고 바꾸지 않았다.
- 기존 `KNOWN_GAP` 예제 3개는 `unknown` 때문에 이제 `GAP?`로 출력된다. 빈틈이 닫힌 것이 아니라 덤 오류다. 예제를 고치는 것은 기존 테스트 변경이라 두었다.
- 원리상 못 막는 것(`KNOWN_GAP`과 리뷰 항목):
  - `no-raw-types`: 내보내지 않는 지역 값의 추론 타입(`const s = plainValue`), 추론된 제네릭 인자(`identity(plainValue)`), 라이브러리 타입의 속성(`ComponentProps<"input">`의 `string` 속성), 깊이 10을 넘는 중첩, 허용 목록 파일이 내보내는 원시 타입 값(그 값을 다시 내보내거나 타입 자리에 쓰면 잡힌다). `symbol`은 막지 않는다.
  - `no-money-coercion`: 금액을 담은 배열·객체를 통째로 바꾸기(`String([m])`; `join`은 `no-base-to-string`이 잡는다), 허용된 키 읽기 파일(`format.ts`) 안에서 꺼낸 십진수 문자열의 계산.
  - `no-ui-text`: 영문 문구(JSX 속성 `placeholder="Search"`, 라이브러리 props). DS props를 `UiText`로 받으면 `tsc`가 막는다.
  - `no-brand-creation`: 허용 목록 파일 안의 단언은 대상 브랜드만 본다(값이 맞는지는 리뷰).
  - `forbidden-file`: 폴더 허용 목록은 지금 `shared/platform` 하나다. 폴더 구조 전체는 H-09(`boundaries/no-unknown-files`).
- 정상 구현을 막을 수 있는 곳: `ReactNode`(문자열·숫자·불린 포함)를 props·반환에 쓸 수 없다. `children`은 `ReactElement`나 `UiText`로 받는다. 서드파티 컴포넌트의 `string` props를 그대로 노출하는 래퍼는 원시 타입 허용 목록에 `shared/ds` 파일을 사용자 승인으로 넣어야 한다(harness 4.2). `key`·`id` 같은 문자열 prop에는 브랜드 값이나 리터럴을 넘긴다. 이벤트 핸들러가 받는 `e.target.value`는 값으로는 쓸 수 있지만 함수 시그니처·상태 타입에 `string`을 적을 수 없어, 입력 값은 `shared/lib/brand`로 파싱해 브랜드로 바꿔야 한다. `NonNullable<T>`(`T & {}`)도 빈 객체로 잡힌다.
- 뒤 작업으로 넘긴 것: 만·억 표기와 `원` 접미(`format.ts`; 단위 글자는 `messages.ts`에서 받아야 한다), `shared/ipc/schema.ts`·`bindings.ts`(H-13), Tauri `invoke` 연결(U-04), 폴더 경계·knip(H-09), DS 규칙(DS-02·09·10), `xtask disable-report`의 TS 끄기(H-40), `guard/forbidden-extension`과 `local/forbidden-file` 통합.
