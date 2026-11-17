# DS-20 넘침 실측과 부하 자료

- 상태: 초안

## 결정

`shared/ds/testing/overflow.ts`는 실제 DOM의 크기와 사각형으로 harness 4.4의 일곱 확인을 실행한다. 선택적으로 확인을 끄는 API는 없다. 레이아웃이 없는 문서는 통과가 아닌 `layout-unavailable`로 반환한다. 클리핑 기하 계산은 보호된 `overflow-geometry.ts`에 둔다. fixed 요소가 일반 overflow 조상을 벗어나는 경우, 변환된 containing block에 실제로 잘리는 경우, 문서 루트에 전파되는 body 스크롤 잠금, 테두리 안쪽 padding 경계를 각각 구분한다.

`data-scroll-viewport`는 스크롤 가능한 CSS가 함께 있어야 하고, `data-truncate`는 실제 말줄임 CSS가 함께 있어야 넘침 예외가 된다. 정적 규칙은 전자를 `ScrollArea`, 후자를 `Text`와 `Heading` 소유 파일에서만 허용한다. 말줄임 요소의 전체 글자 제공은 따로 검사한다. 표식만 붙여 검사 결과를 감추지 못하게 하기 위한 제한이다.

뷰포트의 자르기 예외는 각 축의 실제 스크롤 가능 범위에 한정한다. 스크롤이 불가능한 축과 스크롤 시작점 바깥은 잘림으로 판정하며, 오른쪽에서 왼쪽으로 흐르는 배치와 역방향 flex도 시작점을 구분한다. 화면이 없는 문서뿐 아니라 `html`이나 `body`가 숨겨진 문서도 `layout-unavailable`이다. 일반 textarea의 세로 스크롤은 중첩을 검사하고, 명시된 `textarea.chat-input`의 중첩 예외를 실제 요소에도 적용한다. 이 예외가 내용 크기 초과까지 면제하지는 않는다.

부하 값의 화면 문구는 `testing/messages.ts`, 자료는 `testing/stress.ts`, 공개 컴포넌트 이야기는 두 `Stress*.stories.tsx` 파일에 둔다. 검사와 기하 함수 및 부하 자료·이야기 여섯 파일은 정확한 경로로 보호 목록과 생성된 정책에 등록한다. 원시 DOM·타입 접근 허용은 새 검사 파일에만, Knip 운영 예외는 운영 코드에 들어가지 않는 이 여섯 테스트 전용 파일에만 정확한 경로로 추가한다. 기존 DS-19 CSS 허용 목록은 늘리지 않는다. `ds-catalog`는 공개 컴포넌트마다 보호된 부하 이야기의 존재를 확인한다. 기존 파일에서 `state: stress`만 붙인 이야기는 누락을 가리지 못한다.

보호된 부하 이야기의 각 테마 렌더는 유효한 React 요소여야 한다. 실제 브라우저에서는 양의 면적을 가진 자식 요소, 면적이 있는 텍스트 또는 열린 Dialog의 포털 중 하나가 있어야 한다. 이는 완전히 비거나 0×0인 렌더를 잡는 확인이며 모든 시각적 내용을 보증하지는 않는다. ChatInput은 실제 textarea에 보호된 초안 전체가 전달됐는지 검증한다. 폭과 글자 크기를 바꾼 후에는 두 animation frame을 기다리며, ResizeObserver로 재배치되는 예제를 별도로 검증한다.

기하 계산의 CSS position 값이 앱의 Tailwind 클래스 후보로 오인되는 것을 막기 위해 테마 진입점의 소스 검색에서 새 테스트 전용 `overflow-geometry.ts` 한 파일만 제외한다. 앱 컴포넌트나 다른 테스트 파일의 검색 범위는 바꾸지 않는다. 고정 테마 소스 스냅샷에도 이 경로를 추가했고, 기존 Vite 탐지 회귀 단언을 유지해 확인한다. 새 브라우저 테스트 타입 검사 단계는 보호된 검사 목록과 justfile 해석 스냅샷에 정확히 등록한다.

구현과 수정은 `gpt-6.1-terra`의 max 설정으로 이 작업 사본에서 직접 수행한다. 2026-11-17 사용자는 구현 모델을 이 모델로 정했고 독립 적대적 리뷰와 비판은 유지하기로 했다.

## 의존성

2026-11-17 사용자는 harness 4.4의 Vitest 브라우저 모드 실측을 위해 `@vitest/browser-playwright` 하나를 설치된 Vitest와 같은 정확한 버전 5.1.0의 개발 의존성으로 추가하고, 그 필수 전이 의존성과 잠금·허용 등록 갱신을 승인했다. 제공자는 기존 `playwright` 1.65.0과 DS-15 이미지의 Chromium을 사용한다. 실행에 쓰인 기존 경로는 `/ms-playwright/chromium-1254/chrome-linux-arm64/chrome`, 버전은 158.0.8127.18이다. 브라우저 바이너리는 추가로 받지 않는다.

이번 잠금에 새로 추가된 제공자와 패키지의 설치 메타데이터상 라이선스는 모두 MIT다.

| 패키지                     | 버전          | 역할                        | 라이선스 |
| -------------------------- | ------------- | --------------------------- | -------- |
| @vitest/browser-playwright | 5.1.0         | 승인된 개발 의존성          | MIT      |
| @vitest/browser            | 5.1.0         | 제공자의 브라우저 실행 기반 | MIT      |
| @vitest/ui                 | 5.1.0         | 브라우저 기반의 필수 전이   | MIT      |
| @vitest/utils              | 5.1.0         | 브라우저 기반의 필수 전이   | MIT      |
| @vitest/pretty-format      | 5.1.0         | utils의 필수 전이           | MIT      |
| @blazediff/core            | 1.10.0        | 브라우저 기반의 필수 전이   | MIT      |
| @polka/url                 | 1.0.0-next.29 | sirv의 필수 전이            | MIT      |
| fflate                     | 0.8.3         | UI의 필수 전이              | MIT      |
| mrmime                     | 2.0.1         | sirv의 필수 전이            | MIT      |
| pathe                      | 2.0.3         | UI의 필수 전이              | MIT      |
| pngjs                      | 7.0.0         | 브라우저 기반의 필수 전이   | MIT      |
| sirv                       | 3.0.2         | 브라우저 기반의 필수 전이   | MIT      |
| tinyrainbow                | 3.2.0         | 제공자 등의 필수 전이       | MIT      |
| totalist                   | 3.0.1         | sirv의 필수 전이            | MIT      |

이미 잠겨 있던 전이 항목도 재사용한다: `@vitest/mocker` 5.1.0(MIT), `magic-string` 1.4.5(MIT), `ws` 8.23.0(MIT), `flatted` 3.4.4(ISC), `convert-source-map` 2.0.0(MIT), `playwright`와 `playwright-core` 1.65.0(각각 Apache-2.0). 제공자의 peer인 Vitest와 Playwright도 새 직접 의존성으로 추가하지 않는다. macOS에서 Linux 컨테이너용 모듈을 준비할 때 이미 잠긴 `@rolldown/binding-linux-arm64-gnu` 1.2.18의 SHA-512를 대조한다. 이는 Chromium 다운로드가 아니다.

## 확인과 독립 예제

| 확인                  | 의도적으로 만든 예제                               |
| --------------------- | -------------------------------------------------- |
| 문서 스크롤           | 창보다 넓거나 긴 문서                              |
| 허가 없는 넘침        | 고정 높이의 긴 글, `min-width: 0`이 없는 flex 자식 |
| 조상에 잘림           | `overflow: hidden` 조상 밖으로 나온 팝오버         |
| 말줄임 전체 글자      | 전체 글자 툴팁이 없는 `data-truncate`              |
| 같은 방향 스크롤 중첩 | 세로 뷰포트 안의 세로 뷰포트                       |
| 창 밖 겹침 요소       | 창 밖으로 나간 포털과 표식이 있는 토스트           |
| 겹친 조작 요소        | 같은 부모의 서로 겹친 버튼                         |

부하 자료에는 공백 없는 한글 종목명 40자, 긴 영문 낱말, 50자리 숫자, 조 단위 금액, 소수 8자리 수량, 보유 종목 200행, 근거 20문단, 출처 30개, 빈 값이 있다. 공개 컴포넌트 30개 모두 보호된 값을 쓰는 부하 이야기를 갖는다. 선·영역 차트의 부하 이야기에는 실제 차트가 그려지도록 서로 다른 두 점을 넣는다. Percent에는 큰 숫자를 넣고 Probability에는 유효 범위의 99를 넣는다. 자유 문구 prop이 없는 Skeleton과 DirectionBadge에는 허용된 최대 줄 수 또는 방향과 함께 보호된 긴 문구를 같은 이야기에 배치한다.

## 측정 범위

DS-15의 고정 `linux/arm64` Playwright 이미지와 그 안의 Chromium에서 Vitest 브라우저 모드를 쓴다. 네트워크가 차단된 컨테이너에서 묶인 글꼴 로딩을 확인하고 시간대·시계·난수를 고정한다. 모든 이야기를 960×640 창의 일반 폭과 126px 콘텐츠 폭에서 각각 기본 글자 크기와 125%로 잰다. 126px는 좁은 본문 최소 640px에서 Page의 좌우 32px을 빼고, 12열 및 24px 간격 중 3열을 차지하는 너비다. 아직 구현되지 않은 AppShell·Grid의 실제 조합에서 이것이 절대 최솟값인지는 확인하지 못했다.

로컬에 DS-15 고정 이미지가 준비된 환경에서 `node harness/ds-overflow/launch.mjs`로 전체를 측정한다. 결과가 하나라도 있거나 실행이 불완전하면 실패한다. `node harness/ds-overflow/examples.mjs`는 실제 Chromium 판정 예제를 실행하며 `just check-full`에 연결돼 있다. 기본 `just check`와 기존 macOS CI는 이 브라우저 예제를 실행하지 않는다. 브라우저 테스트의 정적 검사는 `check-ts`에 연결돼 있다. DS-12가 정식 `ds-check`에 통합한다.

WKWebView에서는 표준 모드의 화면 없는 시험 창에서 일곱 판정의 위반 예제 8개, 정상 예제 1개와 fixed·변환 containing block·padding 경계의 기하 예제 3개를 실측했다(`node harness/ds-overflow/wk-measure.mjs`). 이는 WebKit 엔진의 합성 예제 12개에 대한 결과이며 실제 Tauri 창이나 전체 이야기에 대한 검증은 아니다. 화면 없는 WKWebView에서 hit-test로 실제 노출 여부를 확인하지는 못했다.

### 수정과 남은 측정

긴 영문 Text는 126px 칸에서 수정 전 `clientWidth=126`, `scrollWidth=525`였다. 기존에 허용된 `status-sentence`의 줄바꿈을 적용한 뒤 같은 실제 이야기에서 `scrollWidth <= clientWidth`가 됐다. 공개 props와 토큰을 바꾸지 않았다. 기본 Text의 `word-break: keep-all`은 현재 스타일에 없어 DS-11에서 나머지 기본 줄바꿈 계약을 확인해야 한다.

선·영역 차트의 화면 읽기용 500행 표가 문서 전체 높이를 늘리던 현상은 래퍼로 수정했고, 해당 500행이 남고 문서 스크롤이 없는지 브라우저에서 확인했다. 화면에서 숨긴 표 자체는 strict 검사의 잘림과 내용 크기 초과로 여전히 보고된다. 기존 시각 기준 검사 8쌍은 일치했지만 이 기준에 차트가 없어 차트 픽셀 동일성의 근거로 삼지 않는다.

1,034개 테마 이야기를 네 조건씩 측정한 결과, 174개 이야기의 544조건에서 하나 이상의 판정이 나왔다. 허가 없는 넘침 544조건, 문서 스크롤 104조건, 잘림 64조건이며 한 조건의 판정은 중복될 수 있다. 실행기는 종료 코드 1을 반환했다. Dialog의 정상 fixed 팝업이 짧은 스크롤 잠금 body에 잘린다고 오판하던 현상은 별도 실제 hit-test와 회귀 예제로 수정했다. 따라서 이전의 Dialog 잘림 수치를 컴포넌트 결함으로 해석하지 않는다.

일부 글꼴 단계의 짧은 글은 고정 Chromium에서 `clientHeight=16`, `scrollHeight=17`이고, 글자 사각형은 줄 상자 위아래로 1px씩 뻗었다. harness는 이 수치 비교를 요구하므로 허용 오차로 통과시키지 않았다. 시각적 잘림이 있다는 결론은 별도 확인되지 않았다. 반면 화면 읽기용 차트 표는 의도적으로 숨긴 영역이지만 현행 문구에는 예외가 없어 그대로 보고한다. 이런 결과는 실제로 보이는 결함 수와 같지 않다.

측정에 포함된 컴포넌트는 Card, Text, Heading, Button, TextField, Select, Checkbox, Switch, SegmentedControl, Disclosure, Table, Badge, StatusBar, Dialog, EmptyState, Money, Percent, Quantity, PriceChange, LineChart, AreaChart, DonutChart, Treemap, Markdown, ChatInput이다. 그 외 공개 컴포넌트는 이 네 조건에서 판정되지 않았다. 남은 각 판정의 화면상 영향이나 공개 계약 변경 필요성은 이 측정만으로 확정하지 않는다.

## 인계

- DS-12: 공개 목록과 보호된 부하 이야기 누락 검사 및 모든 이야기의 실제 배치 검사를 정식 `ds-check`에 통합한다. 기존 macOS CI의 Docker 가용성과 고정 이미지 준비도 함께 확인한다. 화면 테스트를 `src/app/*.test.tsx`에서 직접 검사 함수로 연결하는 시도는 경계·DOM 규칙이 거부했으므로 규칙을 넓히지 않는 브라우저 테스트 진입 경로를 정한다.
- DS-11: 실제 AppShell·Grid 폭과 Text의 한글 줄바꿈 계약을 확인하고, Base UI ScrollArea의 양축 계산값에서 실제 방향을 구분한다. 새 포털 토스트는 `data-overflow-overlay`로 검사 대상임을 명시하고 실제 열린 상태를 검증한다. `overflow-wrap: anywhere`가 적용된 Text와 별도로 레이아웃 래퍼의 최소 너비·높이 계약을 판별할 예제를 마련한다.
- DS-12와 해당 컴포넌트 작업: 접근성용으로 숨긴 표와 글꼴 지표에서 발생하는 엄격한 판정, 입력 컨트롤의 의도된 내부 스크롤에 대한 내용 크기 판정, 나머지 실제 화면상 영향을 각각 확인한다. 실제 긴 ChatInput과 TextField의 내부 스크롤은 내용 크기 초과로 보고된다. 이 작업에서는 그 판정을 면제하거나 허용 목록을 넓혀 통과시키지 않았다.

## 손 변형

각 변형은 `mktemp -d` 복사본에서 엄격 타입 검사로 컴파일한 뒤 고정 Chromium의 Vitest 브라우저 모드에서 실행했다. 기준 66개 테스트가 통과했고, 컴파일 가능한 변형 42개가 모두 검출됐다. 일곱 확인을 하나씩 끈 별도 실험에서는 해당 확인의 독립 예제만 통과로 바뀌고 나머지 예제는 해당 확인에서 계속 실패했다. 허가 없는 넘침은 두 예제다. 숨겨진 body의 첫 변형은 타입 검사에서 컴파일되지 않아 검출 수에 넣지 않았고, 아래 표에는 별도로 컴파일해 검출한 대체 변형을 적었다.

`just mutants-diff`는 프런트 빌드 후 종료 0이었으나 Rust 변경 변이가 없어 0개를 선택했다. 이것을 변이 검출 통과로 계산하지 않는다.

| 바꾼 것                                                                                                                                                                                                                                | 잡은 테스트                                                                                                                         | 결과 |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---- |
| document-filter: `.filter(hasOverflow)` → `.filter(() => false)`                                                                                                                                                                       | isolates document horizontal scroll; detects document vertical scroll independently                                                 | 검출 |
| scroll-width: `element.scrollWidth > element.clientWidth` → `element.scrollWidth < element.clientWidth`                                                                                                                                | isolates flex child without min-width zero; distinguishes a flex minimum width from content that can wrap                           | 검출 |
| scroll-height: `element.scrollHeight > element.clientHeight` → `element.scrollHeight < element.clientHeight`                                                                                                                           | isolates long text in a fixed height; detects document vertical scroll independently                                                | 검출 |
| clip-left: `inner.left < left` → `inner.left >= left`                                                                                                                                                                                  | isolates popover clipped by ancestor; detects clipping at the left edge                                                             | 검출 |
| clip-right: `inner.right > right` → `inner.right <= right`                                                                                                                                                                             | detects clipping at the right edge; does not clip a visible fixed dialog at a short scroll-locked body                              | 검출 |
| clip-top: `inner.top < top` → `inner.top >= top`                                                                                                                                                                                       | detects clipping at the top edge; does not clip a visible fixed dialog at a short scroll-locked body                                | 검출 |
| clip-bottom: `inner.bottom > bottom` → `inner.bottom <= bottom`                                                                                                                                                                        | detects clipping at the bottom edge; does not clip a visible fixed dialog at a short scroll-locked body                             | 검출 |
| window-left: `bounds.left < 0` → `bounds.left >= 0`                                                                                                                                                                                    | isolates popover clipped by ancestor; detects an overlay beyond the left edge                                                       | 검출 |
| window-right: `bounds.right > doc.documentElement.clientWidth` → `bounds.right <= doc.documentElement.clientWidth`                                                                                                                     | isolates popover clipped by ancestor; isolates portal outside the window                                                            | 검출 |
| window-top: `bounds.top < 0` → `bounds.top >= 0`                                                                                                                                                                                       | isolates popover clipped by ancestor; detects an overlay beyond the top edge                                                        | 검출 |
| window-bottom: `bounds.bottom > doc.documentElement.clientHeight` → `bounds.bottom <= doc.documentElement.clientHeight`                                                                                                                | isolates popover clipped by ancestor; detects an overlay beyond the bottom edge                                                     | 검출 |
| overlap-left: `a.left < b.right` → `a.left >= b.right`                                                                                                                                                                                 | isolates overlapping sibling controls; accepts contained controls and reachable viewport content                                    | 검출 |
| overlap-right: `b.left < a.right` → `b.left >= a.right`                                                                                                                                                                                | isolates overlapping sibling controls; accepts contained controls and reachable viewport content                                    | 검출 |
| overlap-top: `a.top < b.bottom` → `a.top >= b.bottom`                                                                                                                                                                                  | isolates overlapping sibling controls; detects overlapping controls on the left side                                                | 검출 |
| overlap-bottom: `b.top < a.bottom` → `b.top >= a.bottom`                                                                                                                                                                               | isolates overlapping sibling controls; detects overlapping controls on the left side                                                | 검출 |
| tooltip-title: `title === text` → `title !== text`                                                                                                                                                                                     | isolates truncation without full text tooltip; accepts a tooltip containing the complete truncated text                             | 검출 |
| nested-scroll: `hasVerticalAncestor(element)` → `false`                                                                                                                                                                                | isolates nested vertical scroll viewports; detects vertical nesting through an intermediate horizontal viewport                     | 검출 |
| viewport-allowance: `!isViewport(element)` → `true`                                                                                                                                                                                    | isolates document horizontal scroll; isolates nested vertical scroll viewports                                                      | 검출 |
| truncate-allowance: `!isTruncated(element)` → `true`                                                                                                                                                                                   | isolates truncation without full text tooltip; accepts a tooltip containing the complete truncated text                             | 검출 |
| tooltip-nonempty: `text !== "" &&` → `text === "" &&`                                                                                                                                                                                  | accepts a tooltip containing the complete truncated text; accepts the complete text from an associated tooltip                      | 검출 |
| vertical-overflow-kind: `["auto", "scroll", "overlay"].includes(overflow)` → `["never"].includes(overflow)`                                                                                                                            | isolates nested vertical scroll viewports; detects vertical nesting through an intermediate horizontal viewport                     | 검출 |
| chat-exception: `!element.matches("textarea.chat-input")` → `true`                                                                                                                                                                     | permits the ChatInput textarea as the documented nested-scroll exception; detects nested native textarea scrolling except ChatInput | 검출 |
| switch-control: `,[role='switch']` → ``                                                                                                                                                                                                | detects an overlapping switch and button under the same parent                                                                      | 검출 |
| layout-unavailable: `doc.documentElement.clientWidth === 0 &#124;&#124;     doc.documentElement.clientHeight === 0` → `false`                                                                                                          | reports a document without a layout viewport                                                                                        | 검출 |
| viewport-marker: `if (!element.hasAttribute("data-scroll-viewport")) return false;` → `if (!element.hasAttribute("data-scroll-viewport")) return true;`                                                                                | isolates long text in a fixed height; isolates flex child without min-width zero                                                    | 검출 |
| truncate-marker: `if (!element.hasAttribute("data-truncate")) return false;` → `if (!element.hasAttribute("data-truncate")) return true;`                                                                                              | isolates long text in a fixed height; isolates flex child without min-width zero                                                    | 검출 |
| viewport-styles: `return [style.overflowX, style.overflowY].some((value) =>     ["auto", "scroll", "overlay"].includes(value),   );` → `return [style.overflowX, style.overflowY].some((value) =>     ["never"].includes(value),   );` | isolates document horizontal scroll; isolates nested vertical scroll viewports                                                      | 검출 |
| ellipsis-style: `style.textOverflow === "ellipsis"` → `style.textOverflow !== "ellipsis"`                                                                                                                                              | isolates truncation without full text tooltip; accepts a tooltip containing the complete truncated text                             | 검출 |
| fixed-escape: `if (position === "fixed") return "fixed";` → `if (position === "fixed") return "none";`                                                                                                                                 | does not clip a fixed element at an unrelated overflow ancestor; matches clipping behavior for a size container                     | 검출 |
| containing-block: `if (establishesContainingBlock(style)) return "none";` → `if (establishesContainingBlock(style)) return pending;`                                                                                                   | detects a fixed element clipped by its transformed containing block                                                                 | 검출 |
| padding-edge: `bounds.left + parent.clientLeft` → `bounds.left`                                                                                                                                                                        | detects clipping at the padding edge inside a border                                                                                | 검출 |
| unscrollable-axis: `if (!axis.canScroll) return inner.start < clip.start &#124;&#124; inner.end > clip.end;` → `if (!axis.canScroll) return false;`                                                                                    | detects clipping on the non-scrollable axis of a viewport; detects vertical clipping in a horizontally scrollable viewport          | 검출 |
| scroll-start-reach: `axis.offset - minimum` → `0`                                                                                                                                                                                      | accepts horizontal content that can be reached by scrolling; accepts reachable right-to-left scroll content                         | 검출 |
| scroll-end-reach: `maximum - axis.offset` → `0`                                                                                                                                                                                        | isolates nested vertical scroll viewports; accepts contained controls and reachable viewport content                                | 검출 |
| scroll-reverse-origin: `axis.reverse ? -axis.extent : 0` → `0`                                                                                                                                                                         | accepts reachable right-to-left scroll content; accepts reachable content in a reversed horizontal flex viewport                    | 검출 |
| scroll-direction: `style.direction === "rtl"` → `style.direction !== "rtl"`                                                                                                                                                            | accepts horizontal content that can be reached by scrolling; accepts reachable right-to-left scroll content                         | 검출 |
| scroll-row-reverse: `reversedFlex(style, "row-reverse")` → `false`                                                                                                                                                                     | accepts reachable content in a reversed horizontal flex viewport; accepts reachable reversed flex content in right-to-left layout   | 검출 |
| scroll-column-reverse: `reversedFlex(style, "column-reverse")` → `false`                                                                                                                                                               | accepts reachable content in a reversed vertical flex viewport                                                                      | 검출 |
| native-textarea-viewport: `&#124;&#124; element.matches("textarea")` → `&#124;&#124; false`                                                                                                                                            | detects nested native textarea scrolling except ChatInput                                                                           | 검출 |
| scrollable-axis: `const isScrollable = (value: string): boolean =>   ["auto", "scroll", "overlay"].includes(value);` → `const isScrollable = (value: string): boolean =>   ["never"].includes(value);`                                 | isolates nested vertical scroll viewports; accepts contained controls and reachable viewport content                                | 검출 |
| hidden-root: `getComputedStyle(doc.documentElement).display === "none"` → `false`                                                                                                                                                      | reports a hidden document root as unavailable layout                                                                                | 검출 |
| hidden-body: `getComputedStyle(body).display === "none"` → `getComputedStyle(body).display === "no-such-display"`                                                                                                                      | reports a hidden document body as unavailable layout                                                                                | 검출 |
