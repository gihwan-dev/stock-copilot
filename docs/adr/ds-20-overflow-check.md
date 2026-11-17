# DS-20 넘침 실측과 부하 자료

- 상태: 초안

## 결정

실제 DOM의 크기와 사각형을 재는 검사는 `shared/ds/testing/overflow.ts`에 둔다. 검사 결과는 일곱 확인의 종류와 요소를 함께 반환하며 검사를 선택적으로 끄는 제품 API는 두지 않는다. Storybook의 부하 이야기는 공개 목록과 대조해 누락을 실패시킨다. DS-12가 이 대조와 실제 배치 검사를 `ds-check`로 흡수한다.

구현은 `gpt-6.1-terra`의 max 설정으로 이 작업 사본에서 수행한다. 2026-11-17 사용자는 구현에 이 모델을 허용하고 독립 적대적 리뷰와 비판에 관한 기존 절차는 유지하기로 결정했다.

부하 내용의 문자열 문구는 별도의 테스트용 `testing/messages.ts`에 두고, 그 값과 큰 수·행·문단·출처의 자료 묶음을 `testing/stress.ts`에서 제공한다. 검사 함수와 두 자료 파일을 보호 경로에 등록한다. 앱 제품 코드가 테스트 폴더를 가져오는 경로는 추가하지 않는다. Knip 일반 검사에는 실제 브라우저 예제 파일을 정확한 테스트 진입점으로 등록하고, 운영 코드에 닿지 않는 다섯 테스트 전용 파일은 운영 검사 허용 목록에 각각의 정확한 경로와 이유를 등록한다. 와일드카드나 기존 파일에 대한 예외는 추가하지 않는다.

## 의존성

2026-11-17 사용자는 harness 4.4의 Vitest 브라우저 모드 실측을 위해 `@vitest/browser-playwright` 하나를 설치된 Vitest와 같은 정확한 버전 5.1.0의 개발 의존성으로 추가하고, 그 필수 전이 의존성과 잠금·허용 등록 갱신을 승인했다. 제공자는 기존 `playwright` 1.65.0과 DS-15 이미지의 Chromium을 사용한다. 실행에 쓰인 기존 경로는 `/ms-playwright/chromium-1254/chrome-linux-arm64/chrome`, 버전은 158.0.8127.18이다. 브라우저 바이너리는 추가로 받지 않는다.

이번 잠금에 새로 추가된 제공자와 패키지의 설치 메타데이터상 라이선스는 모두 MIT다.

| 패키지 | 버전 | 역할 |
|---|---|---|
| @vitest/browser-playwright | 5.1.0 | 승인된 개발 의존성 |
| @vitest/browser | 5.1.0 | 제공자의 브라우저 실행 기반 |
| @vitest/ui | 5.1.0 | 브라우저 기반의 필수 전이 |
| @vitest/utils | 5.1.0 | 브라우저 기반의 필수 전이 |
| @vitest/pretty-format | 5.1.0 | utils의 필수 전이 |
| @blazediff/core | 1.10.0 | 브라우저 기반의 필수 전이 |
| @polka/url | 1.0.0-next.29 | sirv의 필수 전이 |
| fflate | 0.8.3 | UI의 필수 전이 |
| mrmime | 2.0.1 | sirv의 필수 전이 |
| pathe | 2.0.3 | UI의 필수 전이 |
| pngjs | 7.0.0 | 브라우저 기반의 필수 전이 |
| sirv | 3.0.2 | 브라우저 기반의 필수 전이 |
| tinyrainbow | 3.2.0 | 제공자 등의 필수 전이 |
| totalist | 3.0.1 | sirv의 필수 전이 |

이미 잠겨 있던 전이 항목도 재사용한다: `@vitest/mocker` 5.1.0(MIT), `magic-string` 1.4.5(MIT), `ws` 8.23.0(MIT), `flatted` 3.4.4(ISC), `convert-source-map` 2.0.0(MIT), `playwright`와 `playwright-core` 1.65.0(각각 Apache-2.0). 제공자의 peer인 Vitest와 Playwright도 새 직접 의존성으로 추가하지 않는다. macOS에서 Linux 컨테이너용 모듈을 준비할 때 이미 잠긴 `@rolldown/binding-linux-arm64-gnu` 1.2.18의 SHA-512를 대조한다. 이는 Chromium 다운로드가 아니다.

## 확인과 독립 예제

| 확인 | 의도적으로 만든 예제 |
|---|---|
| 문서 스크롤 | 창보다 넓은 문서, 창보다 긴 문서 |
| 허가 없는 넘침 | 고정 높이의 긴 글, `min-width: 0`이 없는 flex 자식 |
| 조상에 잘림 | `overflow: hidden` 조상 밖으로 나온 팝오버 |
| 말줄임 전체 글자 | 전체 글자 툴팁이 없는 `data-truncate` |
| 같은 방향 스크롤 중첩 | 세로 뷰포트 안의 세로 뷰포트 |
| 창 밖 겹침 요소 | 창 오른쪽 밖으로 나간 포털 |
| 겹친 조작 요소 | 같은 부모의 서로 겹친 버튼 |

부하 자료는 40자의 공백 없는 한글 종목명, 긴 영문 낱말, 50자리 숫자, 조 단위 금액, 소수 8자리 수량, 200행 식별자, 20개 문단, 30개 출처와 빈 값이다. 각 공개 컴포넌트 이야기의 특성에 맞게 사용한다.

## 측정 범위

DS-15의 고정 `linux/arm64` Playwright 이미지와 그 안의 Chromium에서 Vitest 브라우저 모드를 사용한다. 시각 회귀와 같은 글꼴·시간대·색상 조건을 쓰고 시계와 난수를 고정한다. 모든 이야기의 960×640 창에서 일반 폭과 142px 콘텐츠 폭을, 각각 기본 글자 크기와 125%로 잰다. 브라우저 실행 컨테이너는 외부 네트워크를 차단한다. 로컬에 고정 이미지가 준비된 환경에서 `node harness/ds-overflow/launch.mjs`로 실행하며, 위반이 하나라도 있으면 종료 코드가 0이 아니다. 사용자 화면의 여러 창 크기와 실제 Tauri 창은 이 작업의 컴포넌트 이야기 측정에 포함되지 않는다.

macOS의 실제 WKWebView에서는 화면을 띄우지 않는 테스트 창에 일곱 판정의 위반 예제와 정상 예제를 그려 측정했다. `node harness/ds-overflow/wk-measure.mjs`로 재실행한다. 이는 WKWebView 엔진의 예제 검증이며 실제 Tauri 앱과 모든 이야기에서의 합격을 뜻하지 않는다.

차트의 화면 읽기용 표가 문서 전체 높이를 늘리는 문제는 시각적 차트와 공개 props를 바꾸지 않는 래퍼로 수정했다. 표의 행 수와 문서 스크롤을 확인하는 브라우저 회귀 테스트를 추가했다. 기존 시각 기준 검사에는 차트가 포함되지 않으므로 그 검사의 통과를 차트의 픽셀 동일성 근거로 쓰지 않는다.

### 이야기 측정 결과

1,032개 테마 이야기를 네 조건씩 측정한 결과, 158개 이야기의 512개 조건에서 하나 이상의 판정이 나왔다. 이 수는 서로 독립된 결함 수가 아니다. 허가 없는 넘침 512조건, 문서 스크롤 86조건, 잘림 72조건이며 한 조건에서 여러 판정이 겹칠 수 있다. 실행기는 결과가 있을 때 실패한다. 글꼴 반올림이나 테스트용 배치의 영향도 포함될 수 있으므로 각 측정치를 실제 사용자 화면의 결함으로 단정하지 않는다.

| 컴포넌트 | 측정에서 나온 현상 |
|---|---|
| Card, SegmentedControl, StatusBar | 좁은 폭의 내용 넘침과 일부 문서 스크롤 |
| Text, Heading, Button, TextField, Select, Checkbox, Switch, Disclosure, Badge | 요소의 내용 크기가 클라이언트 크기를 넘음 |
| Table, EmptyState, Markdown | 긴 행·내용 등에서 문서 스크롤 또는 내용 넘침 |
| Dialog | 조상 경계에 잘림과 내용 넘침 |
| Money, Quantity, PriceChange | 숫자 표시의 내용 넘침 |
| LineChart, AreaChart, DonutChart, Treemap | 일부 이야기에서 내용 넘침·문서 스크롤, 선·영역 차트의 경우 잘림도 측정 |
| ChatInput | 입력 영역의 내용 넘침 |

차트의 화면 읽기용 500행 표가 문서 높이를 늘리던 문제는 별도 회귀 예제에서 수정 후 통과했다. 나머지 측정은 현재 부하 이야기의 크기 및 배치 조건에서 나온 것이며, 겉모습이나 공개 계약을 바꿔야 해결되는 경우를 이 작업에서 숨기거나 허용하지 않았다. DS-12가 정식 실행 경로를 통합할 때 원인별로 확인하고, 해당 컴포넌트 작업에서 배치 결정을 해야 한다.

## 인계

- DS-12: 공개 목록과 부하 이야기의 누락 검사 및 모든 이야기의 실제 배치 검사를 정식 `ds-check`로 통합한다.
- DS-11: 실제 스크롤 컨테이너와 좁은 격자 항목이 추가될 때 해당 배치에서도 같은 검사를 실행한다.

## 손 변형

각 변형은 임시 복사본에서 컴파일한 뒤 고정 Chromium의 Vitest 브라우저 모드에서 29개 테스트를 실제로 실행했다. 기준본은 29개가 통과했고, 표의 컴파일 가능한 변형 23개가 각각 하나 이상의 테스트에서 검출됐다. 일곱 확인을 하나씩 끈 별도 실험에서는 해당 확인의 예제만 통과로 바뀌고 나머지 확인의 예제는 계속 실패했다. 허가 없는 넘침은 긴 고정 높이 글과 좁아지지 않는 flex 자식 두 예제로 확인했다.

변경분 Rust 변이 명령 `just mutants-diff`는 프런트 빌드 후 종료 0이었으나 실행할 Rust 변경 변이가 없어 0개를 선택했다. 이것을 변이 검출 통과로 계산하지 않는다.

| 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|
| document-filter: `.filter(hasOverflow)` → `.filter(() => false)` | isolates document horizontal scroll | 검출 |
| scroll-width: `element.scrollWidth > element.clientWidth` → `element.scrollWidth < element.clientWidth` | isolates document horizontal scroll | 검출 |
| scroll-height: `element.scrollHeight > element.clientHeight` → `element.scrollHeight < element.clientHeight` | isolates long text in a fixed height | 검출 |
| clip-left: `inner.left < outer.left` → `inner.left >= outer.left` | isolates popover clipped by ancestor | 검출 |
| clip-right: `inner.right > outer.right` → `inner.right <= outer.right` | detects clipping at the right edge | 검출 |
| clip-top: `inner.top < outer.top` → `inner.top >= outer.top` | detects clipping at the top edge | 검출 |
| clip-bottom: `inner.bottom > outer.bottom` → `inner.bottom <= outer.bottom` | detects clipping at the bottom edge | 검출 |
| window-left: `bounds.left < 0` → `bounds.left >= 0` | isolates popover clipped by ancestor | 검출 |
| window-right: `bounds.right > doc.documentElement.clientWidth` → `bounds.right <= doc.documentElement.clientWidth` | isolates popover clipped by ancestor | 검출 |
| window-top: `bounds.top < 0` → `bounds.top >= 0` | isolates popover clipped by ancestor | 검출 |
| window-bottom: `bounds.bottom > doc.documentElement.clientHeight` → `bounds.bottom <= doc.documentElement.clientHeight` | isolates popover clipped by ancestor | 검출 |
| overlap-left: `a.left < b.right` → `a.left >= b.right` | isolates overlapping sibling controls | 검출 |
| overlap-right: `b.left < a.right` → `b.left >= a.right` | isolates overlapping sibling controls | 검출 |
| overlap-top: `a.top < b.bottom` → `a.top >= b.bottom` | isolates overlapping sibling controls | 검출 |
| overlap-bottom: `b.top < a.bottom` → `b.top >= a.bottom` | isolates overlapping sibling controls | 검출 |
| tooltip-title: `title === text` → `title !== text` | isolates truncation without full text tooltip | 검출 |
| nested-scroll: `hasVerticalAncestor(element)` → `false` | isolates nested vertical scroll viewports | 검출 |
| viewport-allowance: `!isViewport(element)` → `true` | isolates document horizontal scroll | 검출 |
| truncate-allowance: `!element.hasAttribute("data-truncate");` → `true;` | isolates truncation without full text tooltip | 검출 |
| clip-viewport-allowance: `parent === null &#124;&#124; isViewport(parent)` → `parent === null` | isolates nested vertical scroll viewports | 검출 |
| tooltip-nonempty: `text !== "" &&` → `text === "" &&` | accepts a tooltip containing the complete truncated text | 검출 |
| vertical-overflow-kind: `["auto", "scroll", "overlay"].includes(overflow)` → `["never"].includes(overflow)` | isolates nested vertical scroll viewports | 검출 |
| chat-exception: `!element.matches("textarea.chat-input")` → `true` | permits the ChatInput textarea as the documented nested-scroll exception | 검출 |
