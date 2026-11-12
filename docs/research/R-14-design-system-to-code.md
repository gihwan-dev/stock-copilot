# R-14 디자인 시스템을 코드로 옮기기

- 조사일: 2026-11-11 (아래 "확인" 날짜는 모두 이 날이다)
- 조사 환경: macOS (Apple Silicon), Node 24.22.0 (ICU 78.3, CLDR 48.0), Playwright 1.65.0 WebKit (UA `Version/27.2 Safari/605.1.15`)
- 조사 대상 산출물: 사용자가 Claude Design으로 만든 "Stock Copilot" 디자인 시스템 첫 버전(2026-11-11). `README.md`, `design-system.json`, `tokens.json`, `components/bundle.css`, `components/bundle.js`, `components/index.d.ts`, `components/*/README.md`
- 실험 코드: `/private/tmp/claude-501/-Users-choegihwan--t3-worktrees-sure-t3-7f34ca39-5542c437df4f301a/d672a99d-9dd8-4133-b4ab-9b52494be3de/scratchpad/research/R-14/` (저장소 밖. `scripts/`에 생성기·대비 검사·Intl 검사, `exp/`에 Tailwind·ESLint·stylelint·WebKit 실험, `headless/`에 무스타일 라이브러리 소스 검사, `font/`에 글꼴 검사)
- 방법: `anthropic-skills:deep-research` 스킬은 조사 서브에이전트를 띄우는 방식이라 이번 작업의 "서브에이전트 금지" 제약과 맞지 않았다. 그래서 웹 검색·공식 문서 읽기·npm 레지스트리 조회·로컬 실험을 직접 했다.
- 버전은 2026-11-11에 `npm view`로 레지스트리에서 확인했다. 표의 날짜는 그 버전이 게시된 날이다.

## 1. 요약

1. Claude Design 산출물은 **사람이 읽는 규칙(README) + 자체 형식의 `tokens.json` + 미리보기용 컴포넌트 번들(전역 `window.StockCopilot`, React 18, ES5 스타일 JS)**이다. 공식 문서에는 토큰 형식 명세가 없다. 그래서 그대로 옮길 수 있는 것은 `tokens.json`뿐이다. 컴포넌트는 금액을 JS 부동소수점으로 계산하고 하네스 규칙(인라인 스타일, 인덱스 키, 계산)을 어기므로 **명세로만 쓰고 다시 짠다**.
2. 토큰은 **Tailwind v4 `@theme`(4.3.5)에서 `--*: initial`로 기본 테마를 지우고 `tokens.json`에서 생성**하는 방식을 추천한다. 이렇게 하면 토큰 밖 클래스(`bg-red-500`, `p-5`, `text-sm`)는 아예 생성되지 않는다. `eslint-plugin-better-tailwindcss` 4.11.0의 `no-unknown-classes`와 `no-restricted-classes`가 임의 값, 불투명도 수식어, `dark:`까지 린트 오류로 잡는 것을 실행해서 확인했다(15건 전부 검출). 다만 숫자를 그대로 받는 유틸리티(`opacity-45`, `z-10`, `duration-600` 등)는 테마를 지워도 살아남으므로 따로 막아야 한다.
3. 컴포넌트 기반은 **Base UI 1.9.0**을 추천한다. Combobox가 있고 IME 조합 처리 코드가 들어 있다(소스에서 확인). Radix에는 Combobox가 없다. 차트는 **Recharts 3.10.1**(선·영역·도넛·트리맵. SVG라서 CSS 변수가 바로 먹는다)과 **Lightweight Charts 5.3.0**(캔들 전용)을 함께 쓰기를 추천한다. Canvas는 `var()` 색을 해석하지 못한다는 것을 WebKit에서 확인했다.
4. 숫자 표기는 **Rust가 준 십진수 문자열을 `Intl.NumberFormat`에 문자열 그대로 넘기면 부동소수점 없이 정확히 형식화된다**(Safari 15.4+. 실험으로 확인). 다만 ko-KR의 빼기 기호는 하이픈(U+002D)이므로 U+2212로 바꿔야 한다. 만·억 혼합 표기와 `원` 접미는 직접 만들어야 한다.
5. 바꿔야 할 설계: 디자인 README의 "비중·수익률·손익은 컴포넌트가 계산"과 "주문 버튼은 UI에 없다"는 architecture 1·5장과 design 6장(승인 흐름)과 충돌한다. 디자인 쪽을 고쳐야 한다(Remix). 또한 harness 4.2 디자인 시스템 규칙을 구체 규칙 목록(아래 3.4)으로 바꾸고, `tokens.json → tokens.css/tokens.ts` 생성기를 `xtask`에 넣고, 그 생성물을 보호 경로로 지정해야 한다.

## 2. 질문별 답

### 2.1 Claude Design 산출물의 형식과 토큰 추출

**공식 문서로 확인한 것** (확인 2026-11-11)

- Claude Design 프로젝트의 내보내기 메뉴: `.zip` 내려받기, PDF, PPTX, Google Slides, 독립 HTML, 다른 도구로 보내기, "Handoff to Claude Code"(로컬 코딩 에이전트로 보내기 / Claude Code Web으로 보내기). 문서는 각 형식의 내용을 설명하지 않는다 ([Get started with Claude Design](https://support.claude.com/en/articles/14604416-get-started-with-claude-design)).
- 디자인 시스템은 "colors, typography, components, and layout patterns"를 담는다. 옮겨 온 시스템은 Claude Code를 포함한 어느 대화에서나 쓸 수 있는 artifact가 된다. 수정은 "Remix"로 한다. `/design-sync`는 React 컴포넌트로 된 기존 시스템의 "tokens and components"를 읽어 Claude Design으로 가져온다 ([Set up your design system](https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design)).
- Claude Code의 `/design`은 디자인을 코드베이스로 가져오거나, 코드를 라이브 프로토타입으로 내보내거나, 처음부터 끝까지 만든다 ([Get started](https://support.claude.com/en/articles/14604416-get-started-with-claude-design), [Claude Design 소개](https://claude.com/design), [블로그 2026-06-17](https://claude.com/blog/claude-design-stays-on-brand-for-daily-work)).
- **토큰 파일 형식, 번들 형식, 핸드오프 묶음의 구조는 공식 문서에 없다(미확인).** 핸드오프 묶음이 gzip tar라는 등의 이야기는 제3자 블로그에만 있다 ([pilot-shell](https://pilot-shell.com/blog/claude-design-handoff), [claudelab](https://claudelab.net/en/articles/claude-ai/claude-design-claude-code-handoff-production-guide)). 그래서 아래 분석은 실제로 받은 파일을 기준으로 한다.

**실제 산출물의 구조** (파일을 직접 읽어 확인)

| 파일 | 내용 | 코드로 옮길 때의 쓰임 |
|---|---|---|
| `design-system.json` | 메타데이터. `"v": 3`, `"layout": "files"`, `namespace: "StockCopilot"`, 라이브러리 `react 18`, `react-dom 18` | 버전 추적용. 코드에는 쓰지 않는다 |
| `README.md` | 원칙, 말투, 숫자 표기, 색 용도, 글자, 레이아웃, 상태·움직임, 아이콘 | 린트 규칙과 컴포넌트 명세의 근거 |
| `tokens.json` | 자체 형식 `version: 1`. `color.themes`(light/dark), `color.tokens[]`(`name`, 테마별 `value`, `usage`), `type.families`/`type.groups[].styles[]`, `spacing`/`radius`/`shadow`/`size`의 `tokens[]` | **생성기의 입력.** 기계가 읽을 수 있는 유일한 원본 |
| `components/bundle.js` | 첫 줄 `/* @ds-bundle: {"format":4,...} */`. IIFE가 `window.React`를 읽고 `window.StockCopilot`에 컴포넌트 10개와 `format`을 붙인다 | 동작 명세로만 쓴다. 가져오지 않는다 |
| `components/bundle.css` | `sc-` 접두 클래스. `var(--bg)` 같은 변수를 참조하지만 **변수 정의는 들어 있지 않다.** Google Fonts `@import` | 값과 상태(hover, focus, disabled)를 읽는 참고 자료 |
| `components/index.d.ts` | props 타입 | props 설계의 출발점. 단, 금액이 `number`라서 그대로 쓸 수 없다 |
| `components/*/README.md` | 쓰는 쪽이 넘길 값과 금지 사항 | 컴포넌트 테스트 항목 |

- `bundle.css`가 쓰는 변수 이름(`--bg`, `--space-3`, `--font-sans`)과 `tokens.json`의 이름이 일치한다. CSS 변수는 Claude Design 미리보기 환경이 `tokens.json`으로 만들어 넣는 것으로 보이지만, 그 규칙은 **미확인**이다.
- `tokens.json`에는 **움직임(motion) 토큰이 없다**. README에 "시세가 바뀐 행은 600ms 동안 깜빡"이라는 글만 있다. `type.fonts`도 빈 배열이다.
- 색 참조 문법은 `{primary}` 하나뿐이다(`focus-ring`). 단위는 전부 `px`이다. `radius-pill`은 `999px`이다.
- 이 파일은 W3C DTCG 형식(2025.10에 첫 안정판. [발표](https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/), [Format 2025.10](https://www.designtokens.org/tr/2025.10/))이 아니다. DTCG를 읽는 Style Dictionary 5.8.0 같은 도구를 쓰려면 변환 단계가 하나 더 필요하다.

**산출물 컴포넌트를 그대로 쓸 수 없는 이유** (`bundle.js`를 직접 읽어 확인)

| 위치 | 문제 | 어기는 규칙 |
|---|---|---|
| `HoldingsTable` | `cost = r.avgPrice * r.qty`, `pl / cost * 100`, 합계와 비중을 JS `number`로 계산 | architecture 1.3, 5장 "계산 금지", design ACC-1·2 |
| `AllocationBar` | 비중을 `it.value / total * 100`으로 계산하고 '기타'를 합산하고 쏠림을 판정 | 같음. 쏠림 판정은 `domain`의 `Concentration`이 해야 한다 |
| `format.money/pct/num` | `Number(n).toLocaleString`, `Math.abs` | harness 4.2 "금액 타입에 `Number()`·산술 금지" |
| `PriceChange` | 방향을 `rate`의 부호로 TS에서 판정 | 계산 금지 (방향도 코어가 보내야 한다) |
| `HoldingsTable` | 표 하나에 통화 하나(`currency` prop 하나). README는 "섞이면 원화로 환산한 값을 넣으라"고 한다 | ACC-3·6 (종목별 평가액은 원래 통화로 정확해야 한다) |
| `AllocationBar` | `style: { flexGrow: r.pct }` | 인라인 스타일 금지 |
| `ChatMessage` | `key: i`(배열 인덱스), `href`를 받아 `target=_blank`로 그대로 연다 | 인덱스 키 금지, R-16(링크는 코어가 확인 후 연다) |
| `HoldingsTable` | `<tr onClick>`에 키보드 접근과 역할이 없다 | 접근성 |
| 전체 | `var`, `for...in`, 객체 변경 | 불변 규칙 |
| 없음 | 주문 승인, 제안 카드, 캔들·선·도넛·트리맵 차트, 입력·선택·토글·탭·대화 상자·빈 상태·뼈대 로딩 | tasks DS-03 ~ DS-06의 범위를 못 채운다 |

**토큰을 뽑아내는 방법 (추천)**

1. 원본 보관: 받은 산출물 전체를 저장소 `design/claude-design/`에 그대로 둔다(tasks DS-00). 사람만 바꾸는 보호 경로로 지정한다.
2. 생성: `xtask gen-tokens`가 `design/claude-design/tokens.json`을 읽어 두 파일을 만든다. ① `apps/desktop/src/shared/ds/tokens.css`(Tailwind `@theme`와 테마별 덮어쓰기) ② `apps/desktop/src/shared/ds/tokens.ts`(토큰 이름 유니온 타입). 값은 넣지 않고 이름만 넣는다. 생성기는 Rust로 짜면 Node 의존성을 늘리지 않는다. 입력이 작은 자체 형식이라 Style Dictionary를 쓸 이유가 약하다.
3. 생성기의 검증: 모르는 키가 있으면 실패. 모든 색에 모든 테마 값이 있어야 한다. `{참조}`가 풀려야 한다. 단위는 `px`만 받는다. 이름은 `^[a-z0-9-]+$`만 받는다.
4. 검사: CI가 다시 생성해서 저장소 파일과 다르면 실패한다(harness 5장과 같은 방식).
5. 대비 검사: 필수 색 조합 표(보호 파일)로 WCAG 대비를 계산해서, 기준에 못 미치면 실패한다.

실험: Node로 만든 생성기 시제품 `scripts/gen-tokens.mjs`(46줄)로 실제 `tokens.json`을 변환했고, Tailwind 4.3.5로 컴파일하고 WebKit에서 렌더링했다. 결과는 4장 대응표에 정리했다.

**`tokens.json` 대비 주장 검증** (`scripts/contrast.py`, WCAG 2 상대 휘도 공식)

- `usage`와 README에 적힌 대비 주장 52개(테마마다 26개)는 반올림 오차 안에서 모두 맞다. 예를 들어 라이트 `primary`/`surface`는 5.8954인데 "5.9 이상"이라고 적혀 있고, 라이트 `focus-ring`/`bg`는 5.3973인데 "5.4 이상"이라고 적혀 있다. 본문 글자색은 모두 4.5:1 이상이다.
- 라이트 `chart-6`/`surface`는 3.74, `chart-2`는 4.30이다. 비텍스트 기준 3:1(WCAG 1.4.11)은 넘는다.
- `up`과 `down`의 대비는 라이트 1.13, 다크 1.10으로 밝기가 거의 같다. README의 "색만으로 방향을 전하지 말고 ▲▼를 함께"가 필수라는 뜻이다.
- `chart-1`의 값은 `primary`와 같다(라이트·다크 모두). 그래서 "가장 큰 비중 계열"과 "AI·주요 액션 표시색"이 시각적으로 같다. 디자인을 그대로 둘지는 사용자 결정이다.
- `focus-ring`은 라이트 `surface-sunken` 위에서 5.03이다. README의 "모든 면에서 5:1 이상"은 맞다.

### 2.2 토큰 표현 방식 비교와 린트 강제

| 기준 | CSS 변수만 (+ CSS Modules) | **Tailwind v4 `@theme`** | Panda CSS 2.5.0 (`strictTokens`) | vanilla-extract 1.21.3 |
|---|---|---|---|---|
| 토큰 밖 값이 "존재하지 않게" 되는가 | 아니다. 아무 값이나 쓸 수 있다 | **된다.** `--*: initial`로 기본 테마를 지우면 토큰 밖 클래스가 생성되지 않는다(실험) | 타입 오류가 난다. 단 `[123px]` 탈출 문법이 있다 ([문서](https://panda-css.com/docs/concepts/writing-styles)) | 아니다. `style()`에 원시 값을 쓸 수 있다 |
| 린트 도구 | stylelint `declaration-strict-value`. `var()` 이름이 실제 토큰인지는 따로 검사해야 한다 | `eslint-plugin-better-tailwindcss` `no-unknown-classes` + `no-restricted-classes`(정규식) | 탈출 문법을 막는 린트를 직접 만들어야 한다 | 직접 만들어야 한다 |
| 빈틈 | `calc(13px)` 같은 함수형 값이 통과한다(실험) | 임의 값 `[..]`, 불투명도 `/50`, 숫자를 받는 유틸리티 → 정규식으로 막는다 | 탈출 문법 | 많다 |
| 다크 테마 | 변수 덮어쓰기 | 변수 덮어쓰기(실험). 그림자는 별도 처리가 필요하다(아래) | 조건부 토큰 | 테마 계약 |
| features 코드의 형태 | `className={styles.x}` + CSS 파일 | `className="bg-surface p-4"` | `css({...})` | `.css.ts` |
| 생성 단계 | 없음 | 있음(빌드 시 CSS 생성) | 있음(codegen) | 있음 |

**추천: Tailwind v4 `@theme`.** 이유: (1) 토큰 밖 값이 빌드 단계에서 존재하지 않게 되고, 같은 목록을 읽는 ESLint 규칙이 저장 직후에 알려 준다. 방어가 두 겹이다. (2) features에 CSS 파일이 생기지 않으므로 stylelint 대상이 `shared/ds`로 줄어든다. (3) 클래스 이름이 토큰 이름과 1:1이라 AI가 README의 토큰 이름을 그대로 쓰면 된다.

**실험 1: 무엇이 생성되는가** (Tailwind 4.3.5 CLI, `exp/out.css`, `exp/out2.css`)

- 생성되지 않음: `bg-red-500`, `p-5`, `text-sm`, `text-xs`, `font-bold`, `font-semibold`, `rounded-xl`, `shadow-md`, `ease-out`, `tracking-wide`, `blur-sm`, `animate-pulse`, `aspect-video`, `inset-0`
- 생성됨 (막아야 함): 임의 값 `p-[13px]`, `text-[#ff0000]`, `w-[232px]`, `bg-[var(--color-up)]`, 불투명도 수식어 `bg-surface/50`, `m-px`, `dark:` 변형, 그리고 숫자를 그대로 받는 `opacity-45`, `z-10`, `border-2`, `outline-2`, `duration-600`, `delay-100`, `leading-none`, `rounded-full`, `grid-cols-3`, `w-1/2`, `translate-x-4`
- 생성되고 써도 됨: `bg-surface`, `text-ink`, `p-4`, `gap-6`, `text-body`(크기·줄높이·굵기가 함께 들어간다), `rounded-lg`, `shadow-popover`, `h-control-height`, `w-sidebar-width`, `font-mono`, `tabular-nums`, `w-full`, `flex-1`

**실험 2: 린트가 잡는가** (`exp/eslint.config.mjs`, ESLint 9.39.5 + typescript-eslint 8.72.2 + better-tailwindcss 4.11.0)

```js
"better-tailwindcss/no-unknown-classes": "error",
"better-tailwindcss/no-restricted-classes": ["error", { restrict: [
  { pattern: "\\[.*\\]", message: "임의 값 금지: 토큰 클래스를 쓰세요" },
  { pattern: "/\\d+$", message: "불투명도 수식어 금지: *-soft 토큰을 쓰세요" },
  { pattern: "^(?:.*:)?[a-z-]+-px$", message: "px 키워드 금지" }
] }],
"no-restricted-syntax": ["error",
  { selector: "JSXAttribute[name.name='style']", message: "인라인 스타일 금지" },
  { selector: "Literal[value=/#[0-9a-fA-F]{3,8}\\b|\\brgba?\\(|\\boklch\\(/]", message: "색상 리터럴 금지" }
]
```

결과: 위반 예제 `Bad.tsx`·`Bad2.tsx`에서 **15건 전부 오류**(`bg-red-500`, `p-5`, `text-sm`, `font-bold`, `rounded-xl`, `shadow-md`는 unknown, 임의 값 4건, `/50`, `m-px`, 색상 리터럴 2건, 인라인 스타일 1건). 정상 예제 `Ok.tsx`는 오류 0건.

알려진 빈틈 (실험 `Bad3.tsx`):
- `` className={`p-${pad}`} ``처럼 동적으로 만든 클래스는 위 규칙으로 잡히지 않는다. 그런데 `better-tailwindcss/no-concatenated-classes`가 이것을 잡았다.
- `const k = "text-[#f00]"; <span className={k}>`처럼 변수에 담은 문자열은 기본 설정에서 검사되지 않는다. `className`의 값을 "문자열 리터럴 또는 `cn()` 호출"로 제한하는 `no-restricted-syntax` 규칙을 추가하니 잡혔다.
- `entryPoint` 설정으로 실제 CSS 진입점을 읽어야 `no-unknown-classes`가 사용자 테마를 안다. 설정을 빠뜨리면 기본 테마로 판정할 수 있으므로, 하네스 자체 테스트(위반 예제가 실패하는지)로 지킨다.

**실험 3: shared/ds의 CSS** (stylelint 18.0.0 + stylelint-declaration-strict-value 1.13.0)

`color-no-hex`와 `declaration-strict-value`(color, background, padding, margin, gap, font-size, border-radius, box-shadow 등)로 `#ff0000`, `13px` 4건을 검출했다. `var(--없는-이름)`과 `calc(13px)`는 통과하므로, `var()` 이름이 생성된 토큰 목록에 있는지는 `xtask`로 검사해야 한다.

**다크 테마와 그림자 (실험 4)**

- `@theme`(inline이 아닌 것)에 둔 색은 유틸리티가 `var(--color-surface)`를 참조한다. 그래서 `:root[data-theme="dark"]`에서 변수만 바꾸면 다크 테마가 된다. WebKit에서 `bg-surface`가 라이트 `rgb(255,255,255)`, 다크 `rgb(22,25,28)`로 나오는 것을 확인했다.
- **함정**: `--shadow-popover`를 `@theme`에 값으로 두면 Tailwind가 그림자 값을 풀어서 유틸리티 안에 직접 넣는다(`--tw-shadow: 0 4px 16px var(--tw-shadow-color, rgba(...))`). 그래서 다크 덮어쓰기가 먹지 않는다. 테마별 원값을 `--sc-shadow-popover`에 두고 `@theme { --shadow-popover: var(--sc-shadow-popover) }`로 참조하게 하니 WebKit 다크에서 `rgba(0,0,0,0.5) 0 4px 16px`가 나왔다.
- Tailwind 공식 문서: `@theme`의 네임스페이스, `--color-*: initial`, `--*: initial`, `@theme inline`, `@theme static` ([Theme variables](https://tailwindcss.com/docs/theme), 확인 2026-11-11).

### 2.3 무스타일 컴포넌트 기반 비교

| 기준 | Radix (`radix-ui` 1.8.1, 2026-10-12) | **Base UI (`@base-ui/react` 1.9.0, 2026-10-09)** | React Aria Components 1.22.1 (2026-10-09) | Ark UI 5.40.0 |
|---|---|---|---|---|
| 라이선스 | MIT | MIT | Apache-2.0 | MIT |
| 안정 버전 | 1.x | 1.0.0이 2025-12-11 ([릴리스 노트](https://base-ui.com/react/overview/releases/v1-0-0.md)). 이후 1.9까지 나옴 | 1.x | 5.x |
| 이 앱에 필요한 부품 | Dialog, Popover, Tooltip, Tabs, Select, Switch, ToggleGroup, Menu, ScrollArea, Toast는 있다. **Combobox와 NumberField는 없다** | 위 전부 + **Combobox, Autocomplete, NumberField**, Meter, Toolbar, Drawer, CSPProvider (패키지 폴더로 확인) | 위 전부 + ComboBox, NumberField, Table, GridList, DatePicker. ko-KR 번역 문자열 포함(`react-aria/i18n/ko-KR.mjs` 등 116개 파일) | 대부분 있음 (Zag 상태 기계) |
| 한글 IME 처리 (설치한 패키지 소스에서 확인) | 텍스트 입력 부품이 없어서 해당 없음. `isComposing`·`composition` 처리 코드 0건 | `ComboboxInput`이 `onCompositionStart/End`로 조합 중 값을 따로 들고, `event.which === 229`를 검사한다. `useListNavigation`과 `useDismiss`에도 처리가 있다 | `useKeyboard`가 `nativeEvent.isComposing`이면 키 처리를 건너뛴다. `useAutocomplete`는 `insertCompositionText`를 정방향 입력으로 본다 | 미확인 |
| useEffect 금지 규칙과의 충돌 | 없음. 라이브러리 내부의 효과는 린트 대상이 아니다 | 없음 | 없음 | 없음 |
| 스타일 연결 | `data-state` 속성 | `data-*` 속성, `className` 함수, `render` prop | `className` 함수, `data-*` 속성 | `data-*` 속성 |
| 설치 크기 (node_modules, 참고) | 5.7MB | 20MB | react-aria 37MB + rac 10MB | 미측정 |

- 효과 금지 규칙과의 관계: 세 라이브러리 모두 내부에서 효과 훅을 쓰지만, 린트는 `apps/desktop/src`만 검사하므로 충돌하지 않는다. 다만 **React 19의 정리 함수를 돌려주는 ref 콜백은 사실상 효과다.** `features`에서 `ref={(el) => ...}` 화살표 함수를 금지하고 `shared/platform`에서만 허용해야 한다. 그렇지 않으면 차트 같은 명령형 라이브러리를 features에서 직접 붙이는 우회로가 생긴다.
- CSP: Base UI는 일부 부품이 인라인 `<style>`·`<script>`를 넣는다고 문서에 적혀 있다. `CSPProvider`로 nonce를 주거나 인라인 `<style>` 생성을 끌 수 있다 (`@base-ui/react/docs/react/utils/csp-provider.md`). R-16 초안의 CSP는 `style-src 'self'`이므로 실제 화면에서 막히는 부품이 있는지 확인해야 한다(미확인).
- 한글 IME의 남은 위험: 채팅 입력창의 "Enter로 보내기"는 직접 만드는 부분이다. `e.nativeEvent.isComposing` 검사가 기본이다 ([classmethod](https://dev.classmethod.jp/en/articles/react-ime-composition-pitfalls/)). 한국어 IME는 조합 중 Enter가 "조합 확정"과 "줄바꿈"을 함께 일으키는 경우가 있다고 W3C에 보고되어 있다 ([w3c/uievents#202](https://github.com/w3c/uievents/issues/202)). Playwright는 WebKit에서 OS IME를 흉내 내지 못하므로 **WKWebView에서의 실제 동작은 미확인**이다. 손으로 확인할 목록에 넣는다(3.5).

**추천: Base UI.** 종목 검색 Combobox와 수량 NumberField가 필요하고, 그 둘에 IME 처리가 들어 있다. API가 Radix와 비슷해서 예제가 많다. React Aria는 접근성과 국제화가 가장 강하지만 크기와 API 표면이 커서 AI가 일관되게 쓰기 어렵다. 이것이 대안 1순위다.

### 2.4 금융 차트 라이브러리 비교

| 기준 | **Recharts 3.10.1** (MIT) | **Lightweight Charts 5.3.0** (Apache-2.0) | ECharts 6.1.0 (Apache-2.0) | visx 4.0.0 (MIT) |
|---|---|---|---|---|
| 그리는 방식 | SVG, 선언형 React | Canvas, 명령형 | Canvas 또는 SVG, 명령형 | SVG 기본 부품 |
| 선·영역 | 있음 | 있음 | 있음 | 직접 조립 |
| 캔들 | 없음 (막대에 사용자 모양을 넣어 만들어야 함) | **있음** (`upColor`, `downColor`, `wickUpColor`, `wickDownColor`) ([문서](https://tradingview.github.io/lightweight-charts/docs)) | 있음 | 직접 조립 |
| 도넛·트리맵 | 있음 (Pie, Treemap) | 없음 | 있음 | 직접 조립 |
| 큰 데이터 | SVG라서 수천 점 단위가 한계로 알려져 있다 (이 앱에서는 미측정) | 금융 시계열에 특화 | 샘플링·대용량 모드 | 직접 구현 |
| 토큰 색 | **`fill="var(--color-chart-2)"`나 `fill-chart-2` 클래스가 그대로 먹는다** (WebKit에서 확인: 라이트 `rgb(163,112,16)`, 다크 `rgb(227,179,74)`). 테마를 바꿔도 다시 그릴 필요가 없다 | Canvas는 `var()`를 해석하지 못한다 (WebKit에서 `fillStyle = "var(...)"`가 무시됨을 확인). `getComputedStyle`로 실제 색을 읽고 테마가 바뀌면 옵션을 다시 넣어야 한다 | Canvas는 같은 문제. v6에 `setTheme` 동적 전환이 있다 ([v6 기능](https://echarts.apache.org/handbook/en/basics/release-note/v6-feature/)) | 클래스로 지정 |
| 접근성 | 3.0부터 `accessibilityLayer`가 기본으로 켜짐 ([3.0 이전 안내](https://github.com/recharts/recharts/wiki/3.0-migration-guide)) | 문서에 언급 없음 → 텍스트 요약을 따로 둔다 | aria 모듈 (v6 문서에서는 이번에 미확인) | 직접 구현 |
| useEffect 금지와의 관계 | features에서 바로 쓸 수 있다 | `shared/platform` 래퍼 하나가 필요하다 (ref 콜백과 정리) | 같음 | 없음 |
| 기타 | - | 라이선스가 TradingView 표기를 요구한다(공개 웹페이지 기준. 개인용 앱은 해당 범위가 미확인). 기본 로고 표기를 유지하면 된다 | 하나로 전부 된다 | 만들 것이 많다 |

**추천: Recharts(선·영역·도넛·트리맵) + Lightweight Charts(캔들만).** 차트 대부분이 CSS 변수로 토큰 색을 바로 쓰고, 효과 없이 features에서 선언형으로 쓸 수 있다. 캔들은 종목 상세(U-13) 한 곳뿐이고 design.md 1장이 트레이딩 차트를 비목표로 두었으므로, 명령형 래퍼 하나로 가둔다. 라이브러리를 하나만 원하면 ECharts 6가 대안이다(전부 되지만, 모든 차트가 명령형 래퍼와 색 해석 단계를 거쳐야 한다).

한국 관례 적용:
- 상승 `up`(빨강)·하락 `down`(파랑)은 캔들 `upColor/downColor`와 손익 선에만 쓴다. 자산배분 계열색은 `chart-1`…`chart-6`만 쓴다(README 원칙 2).
- Lightweight Charts에는 `up`·`down`·`line`·`ink-subtle`의 해석된 값을 넘긴다. 테마 변경은 `shared/platform`의 구독 훅(`matchMedia('(prefers-color-scheme: dark)')` + `data-theme` 변경)으로 받는다.
- 차트 좌표용 숫자는 코어가 표시 전용으로 보낸 값만 쓴다(architecture 5장). 방향(상승/하락)도 코어가 보낸다.

**토큰 빈틈**: `tokens.json`에는 차트 축, 눈금선, 십자선, 툴팁, 거래량 막대 색이 없다. 우선 `line`(눈금선), `ink-subtle`(축 글자), `surface-raised`+`shadow-popover`(툴팁)로 대응하고, 디자인 Remix 때 `chart-grid`, `chart-axis`, `chart-crosshair`를 추가하자고 요청한다.

### 2.5 한국 금융 앱의 숫자 표기

**규칙 (디자인 README + 어문 규범 + 실험)**

| 항목 | 규칙 | 근거 |
|---|---|---|
| 원화 | `84,312,500원`. 세 자리 쉼표, 소수 없음, `원`을 붙여 씀 | 디자인 README. 한글 맞춤법은 아라비아 숫자 뒤 `원`을 붙여 쓰는 것을 허용하고, 금액은 변조를 막으려고 붙여 쓰는 관례가 있다 ([국립국어원](https://www.korean.go.kr/nkview/nknews/200406/71_13.html)) |
| 달러 | `$182.34` (소수 둘째 자리) | 디자인 README |
| 주식 수 | `300주`. 소수점 주식은 `0.123456주`처럼 자릿수를 잘라 버리지 않는다 | 디자인 README, R-11 |
| 등락 | 부호 필수 `+1,700원 (+2.43%)`, `−850원 (−1.12%)`. 빼기는 U+2212 | 디자인 README |
| 비중·등락률 | 비중은 소수 첫째(`48.0%`), 등락률은 둘째(`2.43%`) | 디자인 README |
| 만·억 | 수를 한글로 적을 때는 만 단위로 띄어 쓴다(`12억 3456만 7898`). 한글 맞춤법 제44항 | [국립국어원](https://www.korean.go.kr/nkview/nknews/200406/71_13.html) |
| 색과 기호 | 상승 빨강 ▲, 하락 파랑 ▼, 보합 `ink-muted` – | 디자인 README, design UX-1 |
| 고정 폭 숫자 | `tabular-nums` | 디자인 README |
| 기준 시각 | 실시간이 아닌 값에 "15:30 기준" | 디자인 README, design ACC-4 |

토스증권 앱의 실제 표기(만·억을 어디서 쓰는지, `+` 표시 여부)는 앱을 직접 열어 보지 못해서 **미확인**이다.

**실험: `Intl.NumberFormat` (Node 24.22.0, ICU 78.3)** (`scripts/intl.mjs`, `scripts/intl2.mjs`)

```
문자열 "12345678901234567.89" → "12,345,678,901,234,567.89"   (정확)
숫자   12345678901234567.89  → "12,345,678,901,234,568"       (부동소수점 손실)
signDisplay exceptZero  1700 → "+1,700",  -850 → "-850" (U+002D 하이픈)
compact 84312500 → "8431만",  1234567890123 → "1.2조"
currency KRW → "₩84,312,500",  USD(ko-KR) → "US$182.34",  USD(en-US) → "$182.34"
percent 문자열 "0.024349" (exceptZero, 2자리) → "+2.43%"
문자열 "-0.001", 2자리 → "0" (반올림 뒤 부호가 사라짐)
"0.3" percent → "30%", 숫자 0.1+0.2 → "30.000000000000004%"
```

- 문자열을 넘기면 정확한 십진수로 형식화된다는 것은 MDN에도 적혀 있다 ([MDN format](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/NumberFormat/format)). browser-compat-data 기준 Safari 15.4부터 지원한다(`format.number_parameter-string_decimal`, `roundingMode`, `signDisplay: negative`). macOS의 WKWebView는 Safari와 같은 엔진이므로 지원 범위 안이다. 다만 **실제 Tauri WKWebView에서 같은 결과가 나오는지는 미확인**이다(실험은 Node와 Playwright WebKit에서 했다).
- R-11이 정한 대로 비율은 소수 비율(`0.1516`)로 오므로, `style: "percent"`를 문자열에 적용하면 곱셈 없이 정확하다.
- 따라서 `shared/lib/format`은 **산술 없이** 다음만 한다: `Intl.NumberFormat(...).formatToParts(decimalString)` → `minusSign`을 U+2212로 바꾼다 → `원`/`$`/`주`/`%`를 붙인다. 만·억 혼합 표기(`8,431만 2,500원`)는 Intl에 없으므로, 정수부 문자열을 뒤에서 4자리씩 자르는 문자열 처리로 만든다(숫자 변환 없음).
- 글꼴 검사 (`scripts/feat.py`, fonttools): fontsource의 IBM Plex Sans KR 라틴 하위 집합(400·600)에는 GSUB `tnum` 기능이 없다. 대신 숫자 0–9의 폭이 모두 600으로 같아서 원래 고정 폭이다. `+`와 `−`(U+2212)도 600으로 숫자와 같지만, 하이픈(U+002D)은 399다. **U+2212를 써야 부호가 있는 숫자의 세로 정렬이 맞는다.** ▲(U+25B2)와 ▼(U+25BC)는 라틴 하위 집합에 없고 한국어 하위 집합(113번, 110번)에 있다. 폰트를 로컬로 묶을 때 이 하위 집합을 빼먹으면 대체 글꼴로 그려진다.
- **결정 필요**: 실제 값이 `-0.001%`이고 2자리로 반올림하면 글자는 `0.00%`인데 코어가 보낸 방향이 `down`이면 파란 ▼가 붙는다. 방향을 "표시 단위로 반올림한 값" 기준으로 정할지 코어 쪽에서 정해야 한다(R-11과 함께).

### 2.6 채팅 화면

구조 (추천):

1. **코어가 구조화된 스트림을 보낸다.** R-05가 추천한 요청별 `tauri::ipc::Channel<T>`로, 아래처럼 판별 유니온을 보낸다(타입은 Rust에서 생성).
   `TextDelta { text }` · `ToolCallStarted { id, tool, summary }` · `ToolCallFinished { id, status }` · `Card { kind: Proposal | Holding | Quote, id }` · `Sources { items }` · `Done { usage }` · `Failed { kind }`
2. **카드는 마크다운에서 파싱하지 않는다.** AI가 쓴 글에서 "제안 카드"를 찾아내면, 프롬프트 주입으로 가짜 카드를 만들 수 있다. 카드는 MCP 도구(`propose_order` 등)가 실제로 만든 객체의 ID로만 그리고, 내용은 그 ID로 코어에서 다시 읽는다. 그러면 화면의 숫자는 코어 계산값이 된다(ACC-1). 이 카드 컴포넌트는 제안함의 카드와 같은 것을 쓴다(tasks O-12).
3. **도구 호출 표시**: 디자인의 `ChatMessage.tools`(primary-soft 배지, mono)를 그대로 쓴다. 진행 중에는 `ToolCallStarted.summary`("보유 종목 조회 중")를 보여 준다(PERF-2).
4. **본문 마크다운**: R-16과 맞춰 `react-markdown` 10.1.0을 쓴다. 원시 HTML은 끈다(`rehype-raw` 없음). `img`는 링크 글자로 바꾸고, `a`는 코어 명령 `open_external`을 부르는 DS 컴포넌트로 바꾼다. 스트리밍 중 덜 닫힌 마크다운이 깜빡이는 문제는 Streamdown 2.7.1이 `parseIncompleteMarkdown`(remend)으로 해결한다. 하지만 기본 rehype 플러그인에 `rehype-raw`가 들어 있고, 링크·이미지 접두어를 기본으로 모두 허용하며, 기본 스타일이 Tailwind 기본 테마 클래스에 기대고 있다 ([llms.txt](https://streamdown.ai/llms.txt)). `--*: initial`을 쓰면 그 클래스가 사라지므로 `components`를 전부 바꿔 끼워야 한다. 그래서 기본은 react-markdown이고, 깜빡임이 실제로 문제가 될 때 Streamdown을 검토한다.
5. 디자인의 `ChatMessage.sources[].href`를 `<a target=_blank>`로 바로 여는 방식은 쓰지 않는다(R-16).
6. AI 답변 속 숫자 대조 표시(ACC-1)와 "앱 계산" 배지(디자인 Badge README)는 코어가 대조 결과를 함께 보내야 표시할 수 있다. 메시지 형식에 `NumberCheck` 이벤트를 둘지는 R-12에서 정한다.

### 2.7 컴포넌트 목록 화면과 시각 회귀 테스트

| 기준 | **Storybook 10.6.7** (2026-11-10) | Ladle 5.1.1 (2025-11-04) | Playwright만 (1.65.0) |
|---|---|---|---|
| 목록 화면 | 있음 | 있음 (가벼움) | 없음 (직접 페이지를 만들어야 함) |
| 이야기를 테스트로 | Vitest 애드온이 이야기를 실제 브라우저 컴포넌트 테스트로 바꾼다. Vite 기반 프레임워크와 Vitest 3.0+가 필요하다 ([문서](https://storybook.js.org/docs/writing-tests/integrations/vitest-addon)) | 없음 | - |
| 접근성 검사 | a11y 애드온 (컴포넌트 테스트와 함께 돈다) | 애드온 있음 | axe 직접 연결 |
| 시각 회귀 | 공식 Visual tests 애드온은 Chromatic(클라우드)이다 | 없음 | `toHaveScreenshot` |
| 유지 상태 | 활발 | 1년간 새 릴리스 없음 | 활발 |

시각 회귀는 **Vitest 5.1.0 브라우저 모드의 `toMatchScreenshot`**(Playwright 공급자, WebKit)을 Storybook 이야기에 붙이는 구성을 추천한다. 기준 이미지 이름에 브라우저와 플랫폼이 들어가고(`*-webkit-darwin.png`), 비교기는 pixelmatch이고, 허용 오차는 직접 정해야 한다(기본값 없음). 문서도 "통제된 같은 환경에서 돌릴 때 가장 믿을 만하다"고 한다 ([Vitest 시각 회귀](https://vitest.dev/guide/browser/visual-regression-testing)). Tauri에 맞추는 점:

- WKWebView와 가장 가까운 것은 Playwright WebKit이다. 하지만 **같은 엔진 빌드는 아니다**(Playwright의 WebKit은 별도 빌드). 최종 화면은 사용자가 실제 앱으로 대조해 승인한다(tasks DS-08).
- 기준 이미지는 macOS에서만 만든다(로컬과 CI 모두 macOS 러너. R-15). 글꼴은 woff2를 저장소에 묶는다(fontsource `@fontsource/ibm-plex-sans-kr` 5.3.0, OFL-1.1. 원본 `bundle.css`의 Google Fonts `@import`는 R-16 CSP의 `font-src 'self'`에 막힌다).
- 이야기마다 라이트·다크 두 장을 찍는다(`data-theme` 고정). 600ms 깜빡임 같은 움직임은 `prefers-reduced-motion`으로 끄고 찍는다.
- 기준 이미지 경로는 보호 경로로 둔다. 갱신은 사용자 서명 커밋만 받는다(harness 7.1).

### 2.8 밝은·어두운 화면, 글자 크기 조절

- 테마: 기본은 시스템을 따른다(`@media (prefers-color-scheme: dark) :root:not([data-theme])`). 설정에서 고르면 `<html data-theme="light|dark">`를 쓴다. 생성된 `tokens.css`가 두 경우를 모두 담는다(실험 4에서 WebKit 확인: 시스템 다크 + `data-theme="light"` → 라이트 값). Tailwind의 `dark:` 변형은 기본이 `prefers-color-scheme`이라 `data-theme`와 어긋나므로 금지한다(토큰이 바뀌므로 필요도 없다).
- Tauri 창 테마: `WindowConfig.theme`은 "초기 창 테마. 기본값은 시스템 테마. Windows와 macOS 10.14+에서만 구현"이다 ([Tauri 설정](https://v2.tauri.app/reference/config/)). 제목 표시줄과 WebView를 맞추는 런타임 API는 R-05 범위이고, 이번에는 미확인이다.
- 글자 크기: 생성기가 `px`를 `rem`(÷16)으로 바꾼다. 실험 생성물에서 `text-body`는 `0.875rem`/`1.375rem`, `row-height`는 `2.5rem`이 되었다. 설정의 "글자 크기"(예: 90/100/115/130%)는 `<html data-text-scale="115">`로 두고, 생성된 CSS가 `html[data-text-scale="115"] { font-size: 115% }` 같은 규칙을 갖게 한다. 행 높이와 컨트롤 높이도 같이 커져서 글자가 잘리지 않는다. 경계선 1px은 그대로 둔다.
- 대안: `zoomHotkeysEnabled`는 macOS에서 Cmd ± 로 20%씩, 20~1000% 확대하는 폴리필을 넣고, `webview:allow-set-webview-zoom` 권한이 필요하다([Tauri 설정](https://v2.tauri.app/reference/config/)). 화면 전체를 확대하므로 간단하지만, 권한을 하나 더 열고 1px 선까지 커진다. 그래서 rem 방식을 기본으로 한다.
- macOS의 시스템 글자 크기(접근성 설정)가 WKWebView에 전달되는지는 미확인이다.
- `prefers-reduced-motion`에서는 깜빡임 애니메이션을 끈다.

### 2.9 디자인이 바뀌었을 때 다시 맞추는 절차

1. 사용자가 Claude Design에서 Remix하고 내보낸 파일로 `design/claude-design/`을 교체한다. 보호 경로이므로 사용자 서명 커밋으로만 바뀐다.
2. `xtask gen-tokens`를 실행한다. `tokens.css`와 `tokens.ts`가 바뀌고, 토큰 이름이 사라지면 `tokens.ts` 유니온 타입이 줄어들어 **그 토큰을 쓰던 코드가 컴파일 오류**가 난다. 없어진 클래스는 `no-unknown-classes` 린트 오류가 난다.
3. 대비 검사와 생성물 차이 검사가 돈다.
4. 컴포넌트 명세의 변경은 `components/*/README.md`와 `index.d.ts`의 차이로 본다. 차이마다 작업 브리프를 하나씩 만든다(사람의 판단이 필요하다).
5. 시각 회귀가 바뀐 이야기를 보여 준다. 사용자가 실제 앱 화면과 대조하고 새 기준 이미지를 서명 커밋으로 승인한다.
6. 디자인의 컴포넌트 코드(`bundle.js`)는 저장소 코드에 들어가지 않으므로 "동기화"할 대상이 아니다. 계산이 들어간 새 컴포넌트가 오면 4번에서 코어 DTO로 바꾸는 작업을 낸다.
7. 반대 방향(코드 → Claude Design)은 `/design-sync`로 할 수 있다(공식 문서). 다만 그 결과가 `tokens.json`을 어떤 형식으로 다시 쓰는지는 미확인이다. 원본은 하나만 둔다. Claude Design의 `tokens.json`이 원본이고, 코드는 생성물이다.

## 3. 선택지 비교표와 추천

### 3.1 추천 요약

| 항목 | 추천 | 대안 | 추천이 틀렸다고 판명될 조건 |
|---|---|---|---|
| 토큰 방식 | Tailwind 4.3.5 `@theme` + `--*: initial`, `tokens.json`에서 생성 | CSS 변수 + CSS Modules + stylelint | `no-unknown-classes`가 생성 테마를 못 읽는 경우가 반복되거나, 정규식 금지 목록이 계속 늘어서 관리가 어려워질 때 |
| 생성기 | `xtask gen-tokens` (Rust, serde_json) | Style Dictionary 5.8.0 (DTCG로 바꾼 뒤) | Claude Design이 DTCG로 내보내기 시작하면 Style Dictionary가 더 싸다 |
| 컴포넌트 기반 | Base UI 1.9.0 | React Aria Components 1.22.1 | WKWebView에서 Base UI Combobox의 한글 입력이 깨지고 React Aria는 되는 경우. CSP 때문에 필요한 부품이 동작하지 않는 경우 |
| 차트 | Recharts 3.10.1 + Lightweight Charts 5.3.0(캔들만) | ECharts 6.1.0 하나 | 일봉 수천 점 선 차트에서 Recharts가 PERF 기준(대시보드 2초)을 못 맞출 때 |
| 채팅 마크다운 | react-markdown 10.1.0 (원시 HTML 끔) | Streamdown 2.7.1 (보안 옵션을 조이고 컴포넌트를 전부 교체) | 스트리밍 중 덜 닫힌 문법의 깜빡임이 사용성 문제로 확인될 때 |
| 목록·시각 회귀 | Storybook 10.6.7 + Vitest 5.1.0 브라우저 모드(WebKit) `toMatchScreenshot` | Playwright `toHaveScreenshot` + 직접 만든 목록 페이지 | Storybook과 Vitest 버전 맞물림 때문에 하네스가 자주 깨질 때 |
| 아이콘 | lucide-react 1.72.0 (디자인 README 추천) | - | - |

### 3.2 디자인 시스템 전용 린트 규칙 목록 (DS-02 초안)

`features/**`에 적용한다. `shared/ds/**`는 3, 5, 7번만 예외로 풀어 준다.

| # | 규칙 | 도구 | 확인 |
|---|---|---|---|
| 1 | 토큰 밖 Tailwind 클래스 금지 | `better-tailwindcss/no-unknown-classes` (`entryPoint`=생성된 테마를 담은 CSS) | 실험에서 6건 검출 |
| 2 | 임의 값 `[...]`(임의 변형 `[&>p]:` 포함), 불투명도 수식어 `/N`, `-px` 금지 | `better-tailwindcss/no-restricted-classes` 정규식 | 실험에서 6건 검출 |
| 3 | 숫자를 그대로 받는 유틸리티 금지: `opacity-N`, `z-N`, `duration-N`, `delay-N`, `border-N`, `outline-N`, `stroke-N`, `leading-none`, `rounded-full`, `rounded-none`, `grid-cols-N` (허용 목록에 있는 것만 예외) | 같은 규칙, 허용 목록 방식 정규식 | 생성되는 것을 실험으로 확인. 금지 정규식 자체는 미실행 |
| 4 | `dark:` 변형 금지 (테마는 토큰이 바꾼다) | 같은 규칙 `^dark:` | 생성되는 것을 확인 |
| 5 | 가격 색 전용: `(text\|bg\|border\|fill\|stroke)-(up\|down)(-soft)?`는 `shared/ds`의 가격 컴포넌트에서만 쓴다 | 같은 규칙을 features에만 적용 | 미실행 |
| 6 | 동적 클래스 금지. `className`은 문자열 리터럴이나 `cn()`만 | `better-tailwindcss/no-concatenated-classes` + `no-restricted-syntax` | 실험에서 검출 |
| 7 | 인라인 `style` 금지 | `no-restricted-syntax` | 실험에서 검출 |
| 8 | 색 리터럴(`#hex`, `rgb(`, `oklch(`)과 `\d+px` 문자열 금지 | `no-restricted-syntax` (Literal, TemplateElement) | 색은 실험에서 검출 |
| 9 | 기본 HTML 요소 금지: `button`, `input`, `select`, `textarea`, `a`, `table`, `svg`, `img` | `no-restricted-syntax` `JSXOpeningElement[name.name=/^(...)$/]` | 미실행 |
| 10 | 라이브러리 직접 가져오기 금지: `@base-ui/*`, `recharts`, `lightweight-charts`, `react-markdown`, `lucide-react` | `no-restricted-imports` (features) | 미실행 |
| 11 | ref 콜백 화살표 함수 금지 (효과 우회) | `no-restricted-syntax` `JSXAttribute[name.name='ref'] > JSXExpressionContainer > ArrowFunctionExpression` | 미실행 |
| 12 | `Intl`, `toLocaleString`, `toFixed` 금지. 형식 변환은 `shared/lib/format`만 | `no-restricted-globals`, `no-restricted-properties` | 미실행 |
| 13 | 하이픈 빼기 금지: JSX 텍스트의 `-\d` | `no-restricted-syntax` `JSXText[value=/-\d/]` | 미실행 |
| 14 | 이모지 금지 (README 말투 규칙) | `no-restricted-syntax` 유니코드 정규식 | 미실행 |
| 15 | `shared/ds`의 CSS: `color-no-hex`, `declaration-strict-value`, `var()` 이름이 생성 토큰 목록에 있어야 함 | stylelint 18 + `xtask` | 앞의 둘은 실험에서 검출 |
| 16 | 생성물(`tokens.css`, `tokens.ts`)과 시각 회귀 기준 이미지는 손으로 고치지 않음 | 보호 경로 + 재생성 차이 검사 | - |
| 17 | 토큰 대비 기준 | `xtask tokens-contrast` (필수 조합 표는 보호 파일) | 시제품 실행 |

## 4. `tokens.json` → 코드 토큰 대응표

생성기 시제품의 실제 출력(`exp/src/tokens.css`)을 기준으로 정리했다. 클래스 예시는 Tailwind 4.3.5로 실제로 생성되는 것을 확인했다.

### 4.1 규칙

| `tokens.json` 경로 | CSS 변수 (Tailwind 테마 키) | 값 변환 | 다크 | 클래스 예 | `tokens.ts` |
|---|---|---|---|---|---|
| `color.tokens[name]` | `--color-<name>` | 그대로. `{x}` → `var(--color-x)` | `:root[data-theme="dark"]`와 `prefers-color-scheme: dark`에서 덮어씀 | `bg-<name>`, `text-<name>`, `border-<name>`, `fill-<name>`, `stroke-<name>`, `outline-<name>` | `ColorToken` 유니온 |
| `color.tokens[chart-N]` | `--color-chart-N` | 같음 | 같음 | `fill-chart-1` | `ChartSeriesToken` (Canvas 차트가 `getComputedStyle`로 읽을 이름) |
| `type.families.<k>` | `--font-<k>` | 그대로 | - | `font-sans`, `font-mono` | - |
| `type.groups[].styles[name]` | `--text-<name>`, `--text-<name>--line-height`, `--text-<name>--font-weight`, `--text-<name>--letter-spacing` | px → rem | - | `text-<name>` 하나로 크기·줄높이·굵기·자간이 함께 적용 | `TextStyleToken` |
| `spacing.tokens[space-N]` | `--spacing-N` | px → rem | - | `p-N`, `gap-N`, `m-N`, `px-N`, `translate-x-N` (N ∈ 1,2,3,4,6,8만) | - |
| `size.tokens[name]` | `--spacing-<name>` | px → rem | - | `w-sidebar-width`, `w-chat-width`, `h-row-height`, `h-control-height` | - |
| `radius.tokens[radius-k]` | `--radius-k` | px → rem. `999px` → `9999px` | - | `rounded-sm`, `rounded-md`, `rounded-lg`, `rounded-pill` | - |
| `shadow.tokens[name]` | 원값 `--sc-<name>` (테마별) + `@theme { --<name>: var(--sc-<name>) }` | 그대로 | `--sc-<name>`를 덮어씀 | `shadow-popover` | - |
| `color.themes` | - | - | - | - | `ThemeId = 'light' \| 'dark'` |

### 4.2 전체 대응 (값은 라이트 / 다크)

**색** (`--color-*`)

| 토큰 | 라이트 | 다크 | 주 클래스 | 용도 (요약) |
|---|---|---|---|---|
| `bg` | `#f5f5f2` | `#0e1012` | `bg-bg` | 앱 바탕 |
| `surface` | `#ffffff` | `#16191c` | `bg-surface` | 카드, 표, 채팅 패널 |
| `surface-raised` | `#ffffff` | `#1d2125` | `bg-surface-raised` | 팝오버, 메뉴, 툴팁 |
| `surface-sunken` | `#ededea` | `#0a0c0d` | `bg-surface-sunken` | 입력창, 표 머리, 사용자 말풍선, hover |
| `line` | `#e3e3de` | `#272c31` | `border-line` | 카드 테두리, 행 구분선 |
| `line-strong` | `#8a8c86` | `#6b737b` | `border-line-strong` | 입력창, secondary 버튼 |
| `ink` | `#17181a` | `#eceef0` | `text-ink` | 본문, 숫자 |
| `ink-muted` | `#575a60` | `#a3a9af` | `text-ink-muted` | 라벨, 보합 |
| `ink-subtle` | `#66686e` | `#8b9298` | `text-ink-subtle` | 시각, 출처, 차트 축(제안) |
| `primary` | `#0b7258` | `#3dc9a3` | `bg-primary`, `text-primary` | 주요 액션, 선택, 링크, AI |
| `primary-hover` | `#095e49` | `#5fd6b5` | `hover:bg-primary-hover` | primary hover |
| `primary-soft` | `#e3f3ee` | `#11312a` | `bg-primary-soft` | 도구 호출 칩, 선택된 행 |
| `on-primary` | `#ffffff` | `#0a0c0d` | `text-on-primary` | primary 위 글자 |
| `up` | `#d0202d` | `#ff5d5d` | `text-up` (`shared/ds` 가격 컴포넌트만) | 상승·이익 |
| `up-soft` | `#fdeced` | `#3a1a1c` | `bg-up-soft` (같음) | 상승 배지, 깜빡임 |
| `down` | `#1f5bd1` | `#5e9cff` | `text-down` (같음) | 하락·손실 |
| `down-soft` | `#ebf1fd` | `#16253f` | `bg-down-soft` (같음) | 하락 배지, 깜빡임 |
| `alert` | `#a8520a` | `#f1a34a` | `text-alert` | 경고·오류 |
| `alert-soft` | `#fbefe2` | `#3a2914` | `bg-alert-soft` | Callout(alert) 바탕 |
| `on-alert` | `#ffffff` | `#0a0c0d` | `text-on-alert` | alert 위 글자 |
| `focus-ring` | `var(--color-primary)` | (primary를 따라감) | `focus-visible:outline-focus-ring` | 포커스 링 |
| `chart-1` | `#0b7258` | `#3dc9a3` | `fill-chart-1`, `bg-chart-1` | 자산배분 1번(= primary와 같은 값) |
| `chart-2` | `#a37010` | `#e3b34a` | `fill-chart-2` | 2번 |
| `chart-3` | `#5b6470` | `#8e99a6` | `fill-chart-3` | 3번 |
| `chart-4` | `#8a5a9e` | `#c08fd6` | `fill-chart-4` | 4번 |
| `chart-5` | `#1f827a` | `#6fd6cf` | `fill-chart-5` | 5번 |
| `chart-6` | `#8a846f` | `#cfc8b0` | `fill-chart-6` | 6번, 기타 |
| `overlay` | `rgba(23, 24, 26, 0.4)` | `rgba(0, 0, 0, 0.6)` | `bg-overlay` | 모달 딤 |

`bg-bg`는 어색하지만 이름을 1:1로 유지해야 Claude Design 문서와 코드를 대조하기 쉬우므로 바꾸지 않는다.

**글자** (`--font-*`, `--text-*`)

| 토큰 | 크기 / 줄높이 / 굵기 / 자간 (px) | 생성값 (rem) | 클래스 |
|---|---|---|---|
| `sans` | IBM Plex Sans KR, Apple SD Gothic Neo, Malgun Gothic, system-ui | - | `font-sans` |
| `mono` | IBM Plex Mono, ui-monospace, SF Mono, Menlo | - | `font-mono` |
| `num-display` | 32 / 40 / 600 / −0.5 | 2 / 2.5 / 600 / −0.03125 | `text-num-display tabular-nums` |
| `num-lg` | 22 / 28 / 600 | 1.375 / 1.75 | `text-num-lg` |
| `num-md` | 14 / 20 / 500 | 0.875 / 1.25 | `text-num-md` |
| `num-sm` | 12 / 16 / 500 | 0.75 / 1 | `text-num-sm` |
| `title-lg` | 20 / 28 / 600 | 1.25 / 1.75 | `text-title-lg` |
| `title-md` | 16 / 24 / 600 | 1 / 1.5 | `text-title-md` |
| `title-sm` | 14 / 20 / 600 | 0.875 / 1.25 | `text-title-sm` |
| `body` | 14 / 22 / 400 | 0.875 / 1.375 | `text-body` |
| `body-sm` | 13 / 20 / 400 | 0.8125 / 1.25 | `text-body-sm` |
| `label` | 12 / 16 / 500 | 0.75 / 1 | `text-label` |
| `caption` | 11 / 16 / 400 | 0.6875 / 1 | `text-caption` |
| `code` | 12 / 18 / 400 (mono) | 0.75 / 1.125 | `font-mono text-code` (패밀리는 따로 지정) |

`type.groups[].family`(Mono 그룹의 `mono`)는 Tailwind의 `--text-*`에 들어가지 않는다. DS의 `Code` 컴포넌트가 `font-mono`를 함께 붙인다.

**간격·크기·반경·그림자**

| 토큰 | 원값 | 테마 키 | 클래스 |
|---|---|---|---|
| `space-1` … `space-8` | 4, 8, 12, 16, 24, 32px | `--spacing-1/2/3/4/6/8` | `p-4`, `gap-6` 등 (5, 7은 없음) |
| `sidebar-width` | 232px | `--spacing-sidebar-width` (14.5rem) | `w-sidebar-width` |
| `chat-width` | 384px | `--spacing-chat-width` (24rem) | `w-chat-width` |
| `row-height` | 40px | `--spacing-row-height` (2.5rem) | `h-row-height` |
| `control-height` | 32px | `--spacing-control-height` (2rem) | `h-control-height` |
| `radius-sm/md/lg` | 4, 8, 12px | `--radius-sm/md/lg` | `rounded-sm/md/lg` |
| `radius-pill` | 999px | `--radius-pill` (9999px) | `rounded-pill` |
| `shadow-popover` | 라이트 `0 4px 16px rgba(23,24,26,.12), 0 0 0 1px rgba(23,24,26,.06)` / 다크 `0 4px 16px rgba(0,0,0,.5), 0 0 0 1px rgba(255,255,255,.06)` | `--sc-shadow-popover` → `--shadow-popover` | `shadow-popover` |

### 4.3 토큰에 없어서 디자인에 추가를 요청할 값

`bundle.css`가 토큰 없이 직접 쓴 값들이다. 이 값을 코드로 옮기면 3.2의 규칙에 걸린다.

| 값 (bundle.css) | 쓰는 곳 | 제안 토큰 |
|---|---|---|
| 28px 높이 | `sc-btn-sm`, 세그먼트 버튼 (README에는 "sm은 28px"라고 글로만 있음) | `control-height-sm` |
| 32px 행 | `sc-table-dense` ("촘촘 모드 32px") | `row-height-dense` |
| 13px / 600 / 줄높이 1 | 버튼 글자 | `button` 글자 스타일 (또는 `body-sm`+굵기) |
| 10px 좌우 여백 | `sc-btn-sm` | `space-2`나 `space-3`으로 맞출지 결정 |
| 2px | 세그먼트 안쪽 여백·간격, 배분 막대 간격, Callout 제목 아래 | `space-0.5` |
| 12px / 8px / 20px | 배분 막대 높이, 점, Callout 아이콘 | `bar-height`, `dot-size`, `icon-sm`(16)·`icon-md`(20) |
| 150px | 범례 격자 최소 폭 | `legend-min` 또는 레이아웃에서 결정 |
| `opacity: .45` | disabled | `opacity-disabled` |
| 2px 실선, 2px 간격 | 포커스 링 | `focus-ring-width`, `focus-ring-offset` |
| 1.5 / 1 | 스파크라인 선 두께, 기준선 | `stroke-chart`, `stroke-baseline` |
| 600ms | 시세 깜빡임 (README) | `duration-flash`, 이징 하나. Tailwind `--animate-flash-up/down`으로 생성 |
| 없음 | 차트 축·눈금·십자선 | `chart-grid`, `chart-axis`, `chart-crosshair` |
| 없음 | z 순서 (팝오버, 모달, 토스트) | `z-popover`, `z-modal` |
| 없음 | 창 최소 폭, 채팅 폭 범위 320–560px | `chat-width-min/max`, `window-min-width` |

## 5. 위험과 미확인 항목

**위험**

- 디자인 README와 설계가 충돌한다. ① 원칙 3 "비중·수익률·손익은 컴포넌트와 앱 코드가 계산"과 HoldingsTable·AllocationBar README의 "컴포넌트가 계산"은 architecture 1.3·5장 "TS는 계산하지 않는다"와 충돌한다. ② 원칙 4와 Button README의 "매수·매도 주문 버튼은 만들지 않는다"는 design 6장(사용자 승인 → WebView 밖 본인 확인) 및 tasks O-08·O-12의 승인 화면과 충돌한다. 그대로 두면 AI 구현자가 디자인 문서를 근거로 계산 코드를 넣거나 승인 UI를 빼먹을 수 있다.
- `--*: initial`은 Tailwind 기본 클래스에 기대는 제3자 컴포넌트(Streamdown 기본 스타일 등)의 스타일을 없앤다. 제3자 컴포넌트는 반드시 DS 래퍼로 감싸서 스타일을 다시 입힌다.
- `no-unknown-classes`는 `entryPoint` 설정에 의존한다. 설정이 틀리면 조용히 통과할 수 있으므로 하네스 자체 테스트(위반 예제가 실패하는지)가 필수다.
- 정규식 금지 목록(3.2의 3번)은 Tailwind가 숫자를 받는 유틸리티를 새로 추가하면 빈틈이 생긴다. Tailwind는 정확한 버전으로 고정하고, 올릴 때 "생성되는 클래스 목록" 스냅숏 테스트를 다시 돌린다.
- 시각 회귀는 Playwright WebKit 기준이다. Tauri의 WKWebView와 글꼴 렌더링이 다를 수 있다.
- Lightweight Charts와 ECharts 같은 Canvas 차트는 색을 실행 중에 해석해야 하므로, 테마를 바꿀 때 다시 그리지 않으면 이전 색이 남는다.
- `chart-1`이 `primary`와 같은 색이라, "AI가 낸 것"과 "가장 큰 비중"이 같은 색으로 보인다.

**미확인**

- Claude Design의 토큰·번들·핸드오프 묶음 형식에 대한 공식 명세. `bundle.css`의 변수 정의를 누가 어떻게 넣는지.
- `/design-sync`가 코드에서 Claude Design으로 보낼 때 `tokens.json`을 어떤 형식으로 쓰는지.
- WKWebView에서 Base UI Combobox와 직접 만든 채팅 입력창의 한글 IME 동작(조합 중 Enter, 조합 확정 직후 Enter).
- 실제 Tauri WKWebView에서의 `Intl.NumberFormat` 문자열 십진수 결과(Node와 Playwright WebKit에서만 확인).
- macOS 시스템 글자 크기 설정이 WKWebView에 전달되는지.
- Base UI 부품 중 R-16 CSP(`style-src 'self'`)에 막히는 것이 있는지.
- 토스증권 앱의 실제 만·억, 부호, 등락률 표기.
- Recharts의 큰 시계열 성능(이 앱 규모에서 측정하지 않음).
- ECharts 6의 aria 모듈 현재 상태(v6 기능 문서에서는 확인하지 못함).
- Lightweight Charts 라이선스의 표기 의무가 배포하지 않는 개인용 앱에도 적용되는지.
- `-0.001%` 같이 반올림 뒤 0이 되는 값의 방향(색·기호) 규칙.

## 6. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서 | 항목 | 바꿀 내용 |
|---|---|---|
| Claude Design 산출물 (사용자 Remix) | README 원칙 3, HoldingsTable·AllocationBar README, `index.d.ts` | "컴포넌트가 계산"을 "코어가 계산한 값을 받아 표시"로 바꾼다. props를 `number`에서 코어 DTO(십진수 문자열 + 통화 + 코어가 정한 방향)로 바꾼다. 셀마다 통화를 갖게 해서 한 표에 원화와 달러가 함께 나오게 한다 |
| Claude Design 산출물 | README 원칙 4, Button README | "주문 버튼 없음"을 "AI에게 주문 권한 없음. 사용자의 승인 버튼은 제안 카드에만 있고, 누르면 WebView 밖 확인 창이 뜬다"로 바꾼다. 제안 카드와 승인·거절 컴포넌트를 추가한다 |
| Claude Design 산출물 | `tokens.json` | 4.3의 빠진 토큰(움직임, 높이 sm·dense, 2px 간격, 아이콘 크기, disabled 불투명도, 포커스 링 두께, 차트 축·눈금, z 순서)을 추가한다 |
| architecture.md | 5장 마지막 문단, 8장 표 | "컴포넌트 기반: Base UI, 차트: Recharts + Lightweight Charts(캔들), 토큰: Tailwind v4 생성 테마"로 확정 (ADR 하나) |
| architecture.md | 5장 `shared/` 배치 | `shared/ds/tokens.css`, `shared/ds/tokens.ts`가 생성물이라고 명시한다. `shared/platform`에 테마·글자 크기 구독 훅과 Canvas 차트 래퍼를 둔다 |
| architecture.md | 5장 규칙 표 | "차트 숫자"에 "방향(상승·하락·보합)도 코어가 보낸다"를 더한다. "채팅 카드는 마크다운에서 파싱하지 않고 코어 이벤트의 ID로만 그린다"를 더한다 |
| harness.md | 4.2 "디자인 시스템" 행 | 3.2의 규칙 목록으로 바꾼다. "React" 행에 "ref 콜백 화살표 함수 금지(shared/platform 예외)"를 더한다 |
| harness.md | 4.3 도구 표 | `eslint-plugin-better-tailwindcss`, stylelint + `stylelint-declaration-strict-value`, Storybook, Vitest 브라우저 모드(WebKit) 추가 |
| harness.md | 2장 생성 파일, 7.1 보호 파일 | `design/`, `shared/ds/tokens.css`, `shared/ds/tokens.ts`, 시각 회귀 기준 이미지, 대비 검사 조합 표를 보호 경로에 넣는다 |
| harness.md | 3.3 도구 표 | `xtask gen-tokens`, `xtask tokens-contrast` 추가 |
| tasks.md | DS-01 | 완료 기준에 "생성기 + 재생성 차이 검사 + 대비 검사"를 넣는다. 범위에 "움직임"을 더한다(토큰이 없으므로 디자인 추가가 먼저다) |
| tasks.md | DS-05 | 완료 기준에 "십진수 문자열 입력, 산술 없음, U+2212, 만·억 혼합 표기, 2.5의 실험 값과 일치"를 넣는다 |
| tasks.md | DS-06 | Recharts와 Lightweight Charts로 나눈다. Canvas 차트의 테마 전환 테스트를 넣는다 |
| tasks.md | DS-07 | Storybook + `toMatchScreenshot`(WebKit, 라이트·다크) |
| tasks.md | 신규 | "제안 카드·승인 컴포넌트"(디자인 추가 후), "글꼴 woff2 로컬 묶음(한국어 하위 집합의 ▲▼ 포함)", "한글 IME 수동 확인 목록(WKWebView)" |
| design.md | UX-1 | "빼기는 U+2212, 등락은 부호 필수, 만·억은 요약 카드에서만, 표는 전체 자릿수"처럼 구체화한다 |
| design.md | 7.8 신규 | 테마(시스템 따름 + 수동 선택), 글자 크기 단계, `prefers-reduced-motion` 존중 |
| R-16 | CSP | `font-src 'self'`(Google Fonts 금지)와 Base UI `CSPProvider` 확인 항목을 추가한다 |

## 7. 출처 목록

모두 2026-11-11에 확인했다.

**Claude Design**
- Get started with Claude Design — https://support.claude.com/en/articles/14604416-get-started-with-claude-design
- Set up your design system in Claude Design — https://support.claude.com/en/articles/14604397-set-up-your-design-system-in-claude-design
- Claude Design 제품 페이지 — https://claude.com/design
- Claude Design now stays on brand for daily work (2026-06-17, 2026-09 갱신) — https://claude.com/blog/claude-design-stays-on-brand-for-daily-work
- (보조, 제3자) pilot-shell — https://pilot-shell.com/blog/claude-design-handoff , claudelab — https://claudelab.net/en/articles/claude-ai/claude-design-claude-code-handoff-production-guide

**토큰과 스타일**
- Tailwind CSS Theme variables — https://tailwindcss.com/docs/theme
- tailwindcss 4.3.5 (npm, 2026-11-06) — https://www.npmjs.com/package/tailwindcss
- eslint-plugin-better-tailwindcss 4.11.0 (2026-11-09) — https://github.com/schoero/eslint-plugin-better-tailwindcss , 규칙 소스 `lib/rules/no-restricted-classes.js`
- stylelint 18.0.0 (2026-11-03), stylelint-declaration-strict-value 1.13.0 (2026-11-11) — npm
- Panda CSS writing styles (`strictTokens`) — https://panda-css.com/docs/concepts/writing-styles , @pandacss/dev 2.5.0 (2026-11-02)
- vanilla-extract 1.21.3 (2026-10-08) — npm
- Style Dictionary 5.8.0 (2026-11-03) — npm
- DTCG 2025.10 첫 안정판 발표 — https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/ , 명세 — https://www.designtokens.org/tr/2025.10/

**컴포넌트 기반**
- Base UI v1.0.0 릴리스 노트 (2025-12-11) — https://base-ui.com/react/overview/releases/v1-0-0.md , @base-ui/react 1.9.0 (2026-10-09) 패키지 소스와 `docs/react/utils/csp-provider.md`
- radix-ui 1.8.1 (2026-10-12) — npm, 패키지 소스
- react-aria-components 1.22.1 (2026-10-09) — npm, 패키지 소스 (`react-aria/dist/private/interactions/useKeyboard.mjs`, `autocomplete/useAutocomplete.mjs`)
- @ark-ui/react 5.40.0 (2026-10-29) — npm
- W3C UI Events #202 (한국어 IME Enter) — https://github.com/w3c/uievents/issues/202
- React IME 함정 (classmethod) — https://dev.classmethod.jp/en/articles/react-ime-composition-pitfalls/

**차트**
- Lightweight Charts 문서 — https://tradingview.github.io/lightweight-charts/docs , 5.3.0 (2026-11-10)
- ECharts 6 새 기능 — https://echarts.apache.org/handbook/en/basics/release-note/v6-feature/ , 6.1.0 (2026-05-19)
- Recharts 3.0 이전 안내 — https://github.com/recharts/recharts/wiki/3.0-migration-guide , 3.10.1 (2026-07-25)
- visx 4.0.0 (2026-06-11), uPlot 1.6.32 (2025-03-14), Highcharts 13.1.1 (상용 라이선스) — npm

**숫자 표기**
- MDN Intl.NumberFormat.prototype.format — https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/NumberFormat/format
- MDN browser-compat-data `Intl/NumberFormat.json` — https://github.com/mdn/browser-compat-data/blob/main/javascript/builtins/Intl/NumberFormat.json
- 국립국어원, 수의 띄어쓰기(한글 맞춤법 제44항)와 금액 표기 — https://www.korean.go.kr/nkview/nknews/200406/71_13.html
- @fontsource/ibm-plex-sans-kr 5.3.0 (OFL-1.1) — npm, 패키지 안 글꼴 파일

**채팅**
- Streamdown 문서 — https://streamdown.ai/docs , https://streamdown.ai/llms.txt , 2.7.1 (2026-10-19)
- react-markdown 10.1.0 — https://github.com/remarkjs/react-markdown

**목록·시각 회귀·Tauri**
- Storybook Vitest 애드온 — https://storybook.js.org/docs/writing-tests/integrations/vitest-addon , storybook 10.6.7 (2026-11-10)
- Vitest 시각 회귀 테스트 — https://vitest.dev/guide/browser/visual-regression-testing , vitest 5.1.0 (2026-11-02)
- Ladle 5.1.1 (2025-11-04), Playwright 1.65.0 (2026-11-05) — npm
- Tauri 2 설정 참조 (`WindowConfig.theme`, `zoomHotkeysEnabled`) — https://v2.tauri.app/reference/config/

**저장소 내부**
- docs/harness.md 2·4·5·7장, docs/architecture.md 1·5장, docs/design.md 6·7장, docs/tasks.md DS, docs/research/R-05, R-11, R-16
