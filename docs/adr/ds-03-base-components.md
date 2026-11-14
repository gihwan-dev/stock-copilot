# DS-03 기본 컴포넌트

- 상태: 구현
- 기준: 4c46c32e9239384b24ca0e13869b118fe2a3a3c7
- 결정: 2026-11-14 사용자 후속 지시에 따라 기존 기대값 확장과 내부 구현 오탐 수정을 승인받음
- 범위: 글자·버튼·입력·선택·토글·고급 펼침·아이콘. 카드·표·배지·대화 상자·숫자·차트·레이아웃·마크다운·채팅 입력은 구현하지 않음

## 대상과 디자인 대응

산출물 경로는 모두 `design/components/<컴포넌트>/`의 README·preview와 공통 bundle.js·bundle.css·index.d.ts다. 불리언 props는 architecture 5.3에 따라 닫힌 유니온으로 옮겼다. 의미상 크기 별칭은 DS-10 계약에 맞춰 생성 토큰 이름으로 받는다.

| 컴포넌트         | 변형과 상태                                                                                                     | 등록 항목 |
| ---------------- | --------------------------------------------------------------------------------------------------------------- | --------- |
| Heading          | level 1·2·3 → title-lg·title-md·title-sm, 기본 제목                                                             | 3         |
| Text             | body·body-sm·label·caption·code × ink·ink-muted·ink-subtle·primary·alert × p·span·div                           | 75        |
| Button           | primary·secondary·ghost·buy·sell × control-height·control-height-sm × enabled·disabled, hover·focus·Enter·Space | 20        |
| IconButton       | control-height·control-height-sm, 기본·pressed·expanded·disabled·tooltip right·tooltip hidden                   | 12        |
| TextField        | text·secret, 기본·hint·error·disabled·비밀 값 있음·빈 값, 보기·가리기·붙여넣기·한글 조합 Enter                  | 6         |
| Select           | native 목록, 기본·hint·error·disabled, 선택 값 변경                                                             | 4         |
| Checkbox         | checked·unchecked × enabled·disabled, 설명·키보드                                                               | 4         |
| Switch           | on·off × enabled·disabled, 설명·키보드                                                                          | 4         |
| SegmentedControl | 2–5개 선택지, 세 예시 선택 상태, 화살표 포커스·Space 선택·선택 해제 방지                                        | 3         |
| Disclosure       | open·closed, 제어·비제어, Enter·Space·aria-controls                                                             | 2         |
| Icon             | 산출물의 45개 이름 × icon-sm·icon-md·icon-lg × 장식·의미                                                        | 270       |

총 403개 등록 항목에 light·dark를 명시한다. hover·focus는 강제 상태 prop을 발명하지 않고 브라우저 조작으로 검사한다. Button의 buy·sell은 산출물대로 SignalCard 전용 사용 계약이며 이번에 주문·승인 경로를 붙이지 않는다.

## 공개 계약

- className·style·rest·임의 HTML 속성 전달이 없다. children 문구·label·hint·error·placeholder는 UiText다.
- Text.variant는 생성 TypeStyleToken 중 다섯 글자 스타일만, tone은 ColorToken의 다섯 글자 색만 받는다. 디자인의 default·muted·subtle 의미는 ink·ink-muted·ink-subtle로 대응한다.
- Button·IconButton.size는 생성 SizeToken의 control-height·control-height-sm만, Icon.size는 icon-sm·icon-md·icon-lg만 받는다. 컴포넌트마다 실제 TypeScript 컴파일로 다른 분류를 거부한다.
- ref prop을 새로 발명하지 않는다. 콜백 ref는 기존 전역 규칙과 타입 검사에서 계속 거부한다.
- 입력·선택 값은 UiText와 다른 InputValue 브랜드다. 만드는 곳은 기존 shared/lib/brand.ts이고 빈 값과 safeParse 기반 생성 함수를 제공한다. 이벤트나 DOM 객체는 외부 콜백에 전달하지 않는다.
- TextField의 onCommit(value)는 초기 작업의 조합 중 Enter 확정 콜백 완료 기준에 따른 값 전용 API다. 다른 새 제품 변형·상태를 발명하지 않는다.

## DS-10과 내부 스타일 오탐 수정

ADR 0040의 JSX 문맥 타입 결정을 다음처럼 좁힌다.

1. 내보낸 컴포넌트의 공개 props 탐색·토큰 분류·이벤트·DOM·중첩·ref·rest 검사는 그대로 유지한다.
2. JSX 대상 심볼의 실제 선언이 node_modules/@base-ui/react에 있는 내부 호출에서는 라이브러리의 문맥 타입을 공개 계약으로 검사하지 않는다. 내부 콜백 매개변수에 라이브러리 이벤트가 있어도 허용한다. 실제 데이터 값과 스타일·숫자 검사, 일반 로컬 컴포넌트의 문맥 숫자 검사는 유지한다.
3. 외부 props가 이벤트·HTMLElement를 받으며 내부 콜백이 그대로 전달하는 예제는 공개 props 검사에서 계속 실패한다. 기존 DS-10 위반 예제와 15개 손 변형을 유지한다. 기존 충돌 재현 9개는 안전한 형태의 통과 대조군으로 바꾸고 원래 직접 DOM 접근·산출물 밖 클래스는 계속 거부한다.
4. 선택적 UiText 속성에서 undefined·null은 속성 생략으로 허용한다. 자유 문자열과 UiText가 아닌 선택적 문자열은 계속 거부한다.
5. className의 두 분기가 모두 문자열 리터럴인 단일 조건식을 허용한다. 기존 cn의 정적 분기와 같은 유한 집합이며 동적 변수·문자열 연결·임의 호출·토큰 밖 클래스는 계속 거부한다. 오류 테두리 상태를 이 형태로 표현한다.
6. 크기·기하 파일을 넘기는 Icon 구현은 유지한다. optional label은 정의된 분기에서만 aria-label에 전달한다. target 아이콘의 내부 키만 targetIcon으로 두어 일반 객체의 target 키를 잡는 DOM 규칙과 충돌하지 않게 했다.

## 스타일 산출물 근거

- design/README.md, design/design-system.json, Text·Heading 산출물에서 keep-all·anywhere 명시를 찾지 못했다. 사용자 결정 3에 따라 break-keep·wrap-anywhere는 추가하지 않는다. Text·Heading에 말줄임 prop도 추가하지 않는다. architecture 5.1과 디자인의 간극은 미해결로 남긴다.
- bundle.css .sc-btn의 transparent 테두리에 해당하는 border-transparent만 정적 키워드에 추가한다.
- bundle.css .sc-switch의 calc(var(--icon-md) * 2)에 해당하는 w-switch와 .sc-disc의 토큰 상단 테두리에 해당하는 border-t-hairline을 정적 유틸리티로 추가한다. 새 숫자·색·토큰을 만들지 않고 생성 토큰 변수만 사용한다.
- themeSource로 theme.css를 다시 만들고 생성 클래스 스냅샷을 갱신한다. tokens.ts·tokens.css·@theme 토큰 정의는 바꾸지 않는다. 라이트·다크 토큰 정의는 기존 토큰 검사와 실제 브라우저 렌더링으로 확인한다.
- 아이콘은 패키지의 다른 버전으로 교체하지 않고 산출물 bundle.js의 45개 Lucide 경로를 그대로 쓴다. ISC 고지를 public/Icons-LICENSE.txt에 함께 넣는다. 기하 허용 파일은 Icon.tsx와 icon-paths-0/1/2.tsx 네 개뿐이다.

## 값 전용 입력 경계

DOM 접근 파일은 기존 shared/platform/index.ts 하나를 유지한다. 기존 mountApp(app): void를 유지하고 아래 세 함수만 더한다.

| 함수            | 받는 것                            | 내보내는 것                                                                   |
| --------------- | ---------------------------------- | ----------------------------------------------------------------------------- |
| inputEventValue | React 입력·native 선택 ChangeEvent | Result<InputValue, BrandError>                                                |
| inputCommit     | React 입력 KeyboardEvent           | commit 또는 ignore                                                            |
| pasteInputValue | 값 전용 결과 수신 콜백             | void. 콜백에는 Result<InputValue, BrandError 또는 닫힌 ClipboardError>만 전달 |

문자열 읽기·isComposing 읽기·클립보드 읽기는 이 파일에서만 한다. clipboard가 없거나 읽기가 거절되면 닫힌 실패 값으로 바꾼다. 양쪽 Promise 결과를 처리하며 이 파일만 기존 거절 변환 허용 목록에 더한다. 비밀 값의 붙여넣기 trim은 디자인 bundle.js의 동작을 따른다. TextField는 compositionstart·compositionend의 닫힌 상태와 경계의 isComposing 판정을 함께 확인한다.

DS에서 platform으로 가져오는 예외는 TextField.tsx·Select.tsx에서 platform/index.ts로 향하는 두 파일만 허용한다. DOM 읽기 규칙은 실제 승인 경계 함수로 직접 전달하는 이벤트 인자만 구별하며, 객체의 target/currentTarget/nativeEvent 읽기·가짜 동명 함수·DOM 객체 반환을 허용하지 않는다.

DOM 출력 검사는 Promise 등의 표준 라이브러리 내부 호출 시그니처를 무한히 순회하지 않도록 고쳤다. 타입 인자·중첩 객체의 DOM 여부를 먼저 검사하고 표준 라이브러리의 구현 멤버를 생략한다. 깊이 한계는 거부로 처리한다. 값 Promise 통과와 Promise 안의 DOM 거부를 실제 컴파일·린트 대조군으로 고정한다.

Select README는 운영체제 기본 목록을 명시한다. 이 근거에 따라 Base UI Select로 바꾸지 않고 native select의 값만 같은 경계에서 읽는다. 체크·스위치·세그먼트는 Base UI 값 콜백을 쓴다. Button·Input·Tooltip·Collapsible도 Base UI 기반이다.

## 검증과 후속 작업

새 Node 테스트는 36개다: 기존 충돌 통과 대조군 9개, Icon 기하·테마 렌더 2개, Icon 타입 1개, 나머지 열 컴포넌트 타입 10개, 공개 계약·경계 대조군 2개, 키보드·비활성·오류·조합·클립보드·테마별 목록 렌더 등 12개다. 기존 DS-10 손 변형 15개를 모두 계속 검출한다.

403개 등록 항목을 양 테마에서 실제 렌더링해 806개 조합의 상태 의미를 검사한다. 별도로 Icon의 SVG 기하 540개 조합을 디자인 원본에서 독립적으로 읽은 값과 대조한다. Button 높이·색·hover·disabled 색은 디자인 토큰에서 독립 기대값을 읽는다. 키보드는 Tab·Enter·Space·화살표를 포함하고 값·이벤트의 타입 경계를 별도로 검증한다. Select는 native picker의 Tab/화살표 입력과 브라우저 선택 값 확정을 검사하며 실제 OS 메뉴 내부나 WKWebView 실측을 주장하지 않는다.

Storybook 설치·실행 환경은 DS-07이 붙인다. 상태별 이야기 자료는 컴포넌트별 .stories.tsx와 baseComponentStories 목록으로 제공한다. default export를 발명하지 않는다. 시각 회귀 기준 환경·이미지, 부하 자료·넘침 검사, 실제 WKWebView는 DS-15·DS-20·DS-16 범위다.

## 기존 테스트·유예 목록 변경

단언의 형태나 기존 위반 기대를 약하게 바꾸지 않는다.

- ds-final-review.test.mjs: 빈 기하 목록 기대에 승인된 네 파일만 추가한다.
- knip.test.mjs: production 유예 목록의 정확한 항목 기대를 실제 상태에 맞춘다. 공개 DS 파일·이야기·도우미는 화면의 첫 사용처 전까지 파일별로 유예하며 와일드카드는 없다.
- 입력 경계가 lib 공개 진입점을 처음 참조하여 기존 lib 파일 전체·Zod 의존성 유예 6개가 더 이상 보고되지 않는다. 이 유예를 제거하고 아직 화면이 쓰지 않는 기존 API를 export/type 단위로 세분화한다. 새로운 미사용 API는 계속 실패한다. 최종 목록은 95개이며 아래에 전부 기록한다.
- ds-screen-bootstrap.test.mjs: mountApp 단독 공개 목록 기대를 네 함수로 확장하고 실제 소스의 새 의존성을 scratch에 복사한다. mountApp의 void 반환·DOM 반환 금지·화면 DOM 접근 금지 단언은 유지한다.
- local-allowlists.test.mjs: 거절 변환 목록에 platform/index.ts 한 파일을 추가한다.
- 기존 platform/index.ts 위반 예제에 정상적인 두 결과 Promise 변환 함수만 추가하여 새 허용 항목을 제거하면 반드시 실패하게 한다. 기존 기대 []는 그대로다.

- ds-internal-review.test.mjs의 보호 목록 기대도 같은 기하 네 파일과 가격색 button-variants.tsx 한 파일로 고정한다. 빈 inline style·DOM style 목록과 단일 DOM 접근 파일 단언은 유지한다.
- ds-generation.test.mjs의 고정 theme 문자열에 디자인 근거가 있는 w-switch·border-t-hairline 두 선언만 더한다.
- DS-02 dynamic-ternary 예제는 한 분기가 변수인 실제 동적 코드로 고쳐 기존 거부 기대를 유지한다. 두 리터럴 분기의 통과와 토큰 밖 클래스 거부는 별도 대조군으로 고정한다.
- 기존 경계 변형 하네스에 TextField·Select의 승인된 platform 가져오기 대조군과 다른 DS 파일의 거부 예제를 더한다. 새 정책을 지워도 반드시 실패한다.

## 새 의존성

| 직접 의존성            | 고정 버전 | 분류 | 문서 근거·라이선스                       |
| ---------------------- | --------- | ---- | ---------------------------------------- |
| @base-ui/react         | 1.9.0     | 운영 | architecture 5.2, MIT                    |
| @testing-library/react | 16.3.3    | 개발 | harness 4.3, MIT                         |
| playwright             | 1.65.0    | 개발 | architecture 5.2·harness 6.6, Apache-2.0 |

별도 선택 기능은 켜지 않는다. Playwright 1.65.1은 최소 공개 기간 정책에 걸려 예외를 제거하고 정책을 통과하는 직전 안정판 1.65.0을 유지했다. pnpm-workspace.yaml의 변경과 새 인라인 검사 끄기는 없다.

잠금 파일에 추가된 직접·전이 패키지의 라이선스다.

| 패키지                  | 버전    | 라이선스   |
| ----------------------- | ------- | ---------- |
| @babel/runtime          | 7.29.14 | MIT        |
| @babel/runtime          | 8.1.2   | MIT        |
| @base-ui/react          | 1.9.0   | MIT        |
| @base-ui/utils          | 0.5.0   | MIT        |
| @floating-ui/core       | 1.8.0   | MIT        |
| @floating-ui/dom        | 1.8.0   | MIT        |
| @floating-ui/react-dom  | 2.1.9   | MIT        |
| @floating-ui/utils      | 0.2.12  | MIT        |
| @testing-library/dom    | 10.4.2  | MIT        |
| @testing-library/react  | 16.3.3  | MIT        |
| @types/aria-query       | 5.0.4   | MIT        |
| ansi-regex              | 5.0.1   | MIT        |
| ansi-styles             | 5.2.0   | MIT        |
| aria-query              | 5.3.0   | Apache-2.0 |
| dequal                  | 2.0.3   | MIT        |
| dom-accessibility-api   | 0.5.16  | MIT        |
| lz-string               | 1.5.0   | MIT        |
| playwright-core         | 1.65.0  | Apache-2.0 |
| playwright              | 1.65.0  | Apache-2.0 |
| pretty-format           | 27.5.1  | MIT        |
| react-is                | 17.0.2  | MIT        |
| reselect                | 5.3.0   | MIT        |
| use-sync-external-store | 1.7.0   | MIT        |

## 검사 기록

`just base=4c46c32e9239384b24ca0e13869b118fe2a3a3c7 check`가 통과했다. Rust 작업 공간 534개·커버리지 대상 268개 테스트와 구조·test-diff·hooks-installed 검사가 통과했고 새 검사 끄기는 0개다.

## 정확한 production 유예 항목

- `files src/shared/ds/index.ts src/shared/ds/index.ts`
- `files src/shared/ipc/index.ts src/shared/ipc/index.ts`
- `dependencies package.json @tauri-apps/api`
- `files src/shared/ipc/bindings.ts src/shared/ipc/bindings.ts`
- `files src/shared/ipc/call.ts src/shared/ipc/call.ts`
- `files src/shared/ipc/schema.ts src/shared/ipc/schema.ts`
- `files src/shared/ipc/testing/fakes.ts src/shared/ipc/testing/fakes.ts`
- `files src/shared/ipc/testing/mockBindings.ts src/shared/ipc/testing/mockBindings.ts`
- `files src/shared/ipc/testing/responses.ts src/shared/ipc/testing/responses.ts`
- `dependencies package.json @base-ui/react`
- `exports src/shared/lib/brand.ts emptyInputValue`
- `exports src/shared/lib/brand.ts parseCount`
- `exports src/shared/lib/brand.ts parseEntityId`
- `exports src/shared/lib/brand.ts parseTicker`
- `exports src/shared/lib/brand.ts parseTimestamp`
- `exports src/shared/lib/index.ts emptyInputValue`
- `exports src/shared/lib/index.ts ok`
- `exports src/shared/lib/index.ts parseCount`
- `exports src/shared/lib/index.ts parseEntityId`
- `exports src/shared/lib/index.ts parseMoney`
- `exports src/shared/lib/index.ts parsePrice`
- `exports src/shared/lib/index.ts parseProbability`
- `exports src/shared/lib/index.ts parseQuantity`
- `exports src/shared/lib/index.ts parseRatio`
- `exports src/shared/lib/index.ts parseTicker`
- `exports src/shared/lib/index.ts parseTimestamp`
- `exports src/shared/lib/index.ts text`
- `exports src/shared/lib/messages.ts text`
- `exports src/shared/lib/money.ts parseMoney`
- `exports src/shared/lib/money.ts parsePrice`
- `exports src/shared/lib/money.ts parseProbability`
- `exports src/shared/lib/money.ts parseQuantity`
- `exports src/shared/lib/money.ts parseRatio`
- `exports src/shared/platform/index.ts inputCommit`
- `exports src/shared/platform/index.ts inputEventValue`
- `exports src/shared/platform/index.ts pasteInputValue`
- `files src/shared/ds/Base.stories.tsx src/shared/ds/Base.stories.tsx`
- `files src/shared/ds/Button.stories.tsx src/shared/ds/Button.stories.tsx`
- `files src/shared/ds/Button.tsx src/shared/ds/Button.tsx`
- `files src/shared/ds/Checkbox.stories.tsx src/shared/ds/Checkbox.stories.tsx`
- `files src/shared/ds/Checkbox.tsx src/shared/ds/Checkbox.tsx`
- `files src/shared/ds/Disclosure.stories.tsx src/shared/ds/Disclosure.stories.tsx`
- `files src/shared/ds/Disclosure.tsx src/shared/ds/Disclosure.tsx`
- `files src/shared/ds/Heading.stories.tsx src/shared/ds/Heading.stories.tsx`
- `files src/shared/ds/Heading.tsx src/shared/ds/Heading.tsx`
- `files src/shared/ds/Icon.stories.tsx src/shared/ds/Icon.stories.tsx`
- `files src/shared/ds/Icon.tsx src/shared/ds/Icon.tsx`
- `files src/shared/ds/IconButton.stories.tsx src/shared/ds/IconButton.stories.tsx`
- `files src/shared/ds/IconButton.tsx src/shared/ds/IconButton.tsx`
- `files src/shared/ds/SegmentedControl.stories.tsx src/shared/ds/SegmentedControl.stories.tsx`
- `files src/shared/ds/SegmentedControl.tsx src/shared/ds/SegmentedControl.tsx`
- `files src/shared/ds/Select.stories.tsx src/shared/ds/Select.stories.tsx`
- `files src/shared/ds/Select.tsx src/shared/ds/Select.tsx`
- `files src/shared/ds/Switch.stories.tsx src/shared/ds/Switch.stories.tsx`
- `files src/shared/ds/Switch.tsx src/shared/ds/Switch.tsx`
- `files src/shared/ds/Text.stories.tsx src/shared/ds/Text.stories.tsx`
- `files src/shared/ds/Text.tsx src/shared/ds/Text.tsx`
- `files src/shared/ds/TextField.stories.tsx src/shared/ds/TextField.stories.tsx`
- `files src/shared/ds/TextField.tsx src/shared/ds/TextField.tsx`
- `files src/shared/ds/button-variants.tsx src/shared/ds/button-variants.tsx`
- `files src/shared/ds/control-types.ts src/shared/ds/control-types.ts`
- `files src/shared/ds/icon-paths-0.tsx src/shared/ds/icon-paths-0.tsx`
- `files src/shared/ds/icon-paths-1.tsx src/shared/ds/icon-paths-1.tsx`
- `files src/shared/ds/icon-paths-2.tsx src/shared/ds/icon-paths-2.tsx`
- `files src/shared/ds/icon-paths.tsx src/shared/ds/icon-paths.tsx`
- `files src/shared/ds/story-data.ts src/shared/ds/story-data.ts`
- `types src/shared/lib/brand.ts Count`
- `types src/shared/lib/brand.ts EntityId`
- `types src/shared/lib/brand.ts Ticker`
- `types src/shared/lib/brand.ts Timestamp`
- `types src/shared/lib/index.ts Count`
- `types src/shared/lib/index.ts Currency`
- `types src/shared/lib/index.ts DecimalParseError`
- `types src/shared/lib/index.ts EntityId`
- `types src/shared/lib/index.ts Err`
- `types src/shared/lib/index.ts Locale`
- `types src/shared/lib/index.ts MessageKey`
- `types src/shared/lib/index.ts Money`
- `types src/shared/lib/index.ts Ok`
- `types src/shared/lib/index.ts Price`
- `types src/shared/lib/index.ts Probability`
- `types src/shared/lib/index.ts Quantity`
- `types src/shared/lib/index.ts Ratio`
- `types src/shared/lib/index.ts Ticker`
- `types src/shared/lib/index.ts Timestamp`
- `types src/shared/lib/index.ts UiText`
- `types src/shared/lib/messages.ts Locale`
- `types src/shared/lib/messages.ts MessageKey`
- `types src/shared/lib/money.ts Currency`
- `types src/shared/lib/money.ts DecimalParseError`
- `types src/shared/lib/money.ts Money`
- `types src/shared/lib/money.ts Price`
- `types src/shared/lib/money.ts Probability`
- `types src/shared/lib/money.ts Quantity`
- `types src/shared/lib/money.ts Ratio`
