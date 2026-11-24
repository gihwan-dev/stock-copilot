# ADR DS-23: 넘침 판정 보류 224조건 해소

- 상태: 초안

## 배경

DS-22(ADR 0149)는 카탈로그 전체 1,034개 테마 이야기(4,136조건) 중 겉모습과 스크롤 배치 결정이 필요한 106개 이야기·224조건을 `harness/ds-overflow/pending.json`에 명시적 보류로 남겼다. DS-23은 메인이 확정한 다섯 가지 겉모습·배치 결정과 결정 밖 5개 컴포넌트 방침을 반영해 보류 조건을 해소하고 `pending.json`을 비운다. 넘침 검사기(`apps/desktop/src/shared/ds/testing/overflow.ts`, `overflow-geometry.ts`)는 완화하지 않는다.

## 결정

- **DS-11의 `ScrollArea` 선행 구현**: `apps/desktop/src/shared/ds/ScrollArea.tsx`를 DS-23에서 선행 구현한다. `docs/architecture.md` 5.1의 `ScrollArea` 조항에 맞춰 받는 값은 방향(`direction: "horizontal" | "vertical" | "both"`)과 자식만 받으며, 카탈로그 전용 분기나 여백 용도 prop을 두지 않는다. Base UI `@base-ui/react/scroll-area`의 `Root`·`Viewport`·`Content`·`Scrollbar`·`Corner`를 감싸며, `BaseScrollArea.Content`의 `ResizeObserver`로 뷰포트 크기가 변하지 않는 동적 콘텐츠 증가·글꼴 로드에서도 막대와 키보드 `tabIndex` 상태가 갱신된다. 네이티브 스크롤 막대는 Base UI가 숨기고 절대 위치 오버레이 막대(`scroll-track-x`, `scroll-track-y`)만 내용 초과 시 표시하므로 막대 유무로 내용 폭이 달라지지 않는다. 가로 뷰포트는 `pt-gap-hairline px-gap-hairline pb-space-2`로 막대 높이만큼 아래 여유를 항상 확보해 겹쳐 뜨는 막대가 글자·숫자 하단을 가리지 않으며, 세로 뷰포트의 `Content`는 `scroll-content-y`(`min-width: 0 !important; overflow-x: clip`)를 적용해 비활성 가로 축 넘침만으로 빈 키보드 탭 정지점(`tabIndex=0`)이 생기지 않으면서도 실제 가로 넘침은 `clipped`로 계속 검출된다. `Viewport`에 `data-scroll-viewport`와 `position: relative`, 안쪽 초점 링(`focus-visible:scroll-outline-inset`)을 둔다. DS-11의 나머지 레이아웃 컴포넌트는 만들지 않으며 `src/shared/ds/index.ts`에도 노출하지 않는다.
- **결정 ① 숫자 표시**: `Numbers.tsx`(`Money`, `Percent`, `Quantity`, `Probability`, `PriceChange`, `ChartNumber`, `Unavailable`)는 순수 인라인 한 줄(`tabular-nums whitespace-nowrap w-max`)을 유지하고 줄바꿈·말줄임을 금지한다. 담는 쪽(`Card`, `Table`, `Dialog`, `ChartTable`, `ChartAxes`, `ChartFrame` 탐색 상태 읽기, 카탈로그 숫자 슬롯)이 가로 스크롤 영역을 제공한다. `Card` 본문은 `BaseScrollArea.Content`가 내부 래퍼를 `fit-content`로 확장해 중첩 래퍼 안의 긴 숫자도 `unauthorized-overflow` 없이 가로 스크롤하고, 내부에 표 등이 놓여도 이중 가로 스크롤 막대가 겹치지 않는다.
- **결정 ② 버튼**: `button-variants.tsx`의 고정 높이(`h-control-height`, `h-control-height-sm`)를 최소 높이(`min-h-control-height`, `min-h-control-height-sm`)와 `py-space-1 status-sentence max-w-full text-center`로 전환해 긴 문구가 여러 줄로 감싸이며 높이가 확장된다.
- **결정 ③ 비밀 입력(`TextField`)**: 입력 줄에 `flex-wrap`을 두고 입력·보기 토글 묶음에 `flex-1 wrap-slot`을 부여해 넓은 폭에서는 한 줄을 유지하고 좁은 폭에서는 붙여넣기 버튼이 입력 아래로 내려간다.
- **결정 ④ 표(`Table`)**: 외곽 래퍼에 `max-w-full scroll-flex`를 두고 내부 `<table>`을 가로 `ScrollArea`로 감싼다.
- **결정 ⑤ 세로 문서 스크롤·대화 상자·차트**: 컴포넌트는 자체 세로 스크롤을 갖지 않아 중첩 세로 스크롤 금지 원칙을 유지한다. 카탈로그 스테이지(`.storybook/catalog.mjs`, `.storybook/preview.mjs`)가 `AppShell` 본문을 모방해 높이 고정 세로 `ScrollArea`를 제공하고(내부 콘텐츠 래퍼에 `py-space-3`), 단독 숫자 스토리(`Money`, `Percent`, `Quantity`, `Probability`, `PriceChange`) 슬롯에는 가로 `ScrollArea`를 제공한다. 유일한 컴포넌트 자체 세로 스크롤 예외는 `Dialog`로, `BaseDialog.Viewport`를 `flex items-center justify-center`로 두고 `Popup`에 `max-h-full scroll-flex`를 지정해 머리·바닥을 고정한 채 본문만 `ScrollArea direction="both"`로 감싸 세로 스크롤과 함께 본문 안 긴 숫자의 가로 스크롤도 잘림 없이 지원한다(본문 자식은 `p-space-5 flex flex-col gap-space-4` 래퍼에 둔다). 차트는 세로 스크롤을 갖지 않으며 `DonutChart`·`Treemap`의 대체 표, `ChartAxes`의 최소·최대 숫자(최대 숫자는 `@content-medium` 이상에서 우측 끝 정렬 유지), `ChartFrame`의 탐색 상태 숫자만 가로 `ScrollArea`로 처리한다. `ChartFrame` 탐색 상태는 라벨 줄바꿈과 숫자 단독 가로 스크롤을 분리하고, 탐색 버튼에서 상태 스크롤 영역으로 키보드 초점이 이동할 때 선택 항목을 유지해 키보드만으로도 넘친 숫자를 끝까지 스크롤할 수 있게 한다. `LineChart`·`AreaChart`의 500점 숨긴 접근성 표(`sr-only`)는 유지하고 `scroll-viewport-*`의 `position: relative`로 `sr-only`가 스크롤 영역 밖으로 빠져 문서 스크롤을 유발하지 않게 막는다.
- **결정 밖 5개 컴포넌트(전부 권장안 A)**:
  - `Checkbox`: `Base.Indicator`를 가운데 정렬(`inline-flex items-center justify-center`)하고 내부 전용 `CheckMark`(`Icon.tsx`, `size-space-3`)를 써서 외곽 `size-icon-sm`과 폼 밀도를 유지한다.
  - `Badge`: `whitespace-nowrap`을 제거하고 `status-sentence max-w-full`로 좁은 폭 줄바꿈을 허용해 정보 잘림을 없앤다.
  - `SegmentedControl`: `ToggleGroup`에 `flex-wrap max-w-full`, 각 `Toggle`에 `min-h-control-height-sm py-space-1 status-sentence max-w-full`을 적용한다.
  - `StatusBar`: 좌우 여백을 `px-space-4`로 조정하고 `flex-wrap`과 `wrap-slot`으로 좁은 폭에서 동작 버튼이 문구 아래로 내려가게 하며, 상태 아이콘을 `wrap-slot` 안의 문구 옆에 두어 좁은 폭에서도 아이콘이 단독 줄로 떨어지지 않게 한다(`region.parentElement.parentElement` 구조와 live region 노드 유지).
  - `Disclosure`: `min-h-toolbar-height py-space-2 text-left`와 제목 `status-sentence flex-1`을 적용한다.
- **새 디자인 토큰 0개**: `tokens.json`은 변경하지 않았다. 기존 토큰만 조합한 시맨틱 유틸리티 9개(`scroll-viewport-x`, `scroll-viewport-y`, `scroll-viewport-both`, `scroll-outline-inset`, `scroll-track-x`, `scroll-track-y`, `scroll-content-y`, `scroll-flex`, `wrap-slot`)를 생성기 원본(`ds-design-usage.mjs`)에 정의하고 `theme.css`와 `utilities.json`을 재생성했다. `scroll-viewport-*`·`scroll-track-*`·`scroll-content-y`는 `ds-overflow-utilities.mjs`·`ds-overflow.mjs` 규칙으로 `ScrollArea.tsx`에만, `wrap-slot`은 길이 소유 파일에만 허용된다.
- **넘침 판정 불변 및 보류 표 비우기**: `overflow.ts`와 `overflow-geometry.ts`는 수정하지 않았으며, 1,034개 이야기·4,136조건 전수 측정에서 위반 0건을 확인한 뒤 `harness/ds-overflow/pending.json`을 `[]`로 비웠다.

## 검토 후 버린 대안

- **`ScrollArea`에 `layout` 용도 prop(`card`/`dialog`/`stage`/`numeric`)을 두는 안**: 카탈로그 전용 분기가 제품 코드에 남아 RV-15와 `architecture.md` 5.1("받는 값: 방향")을 어기므로 1차 리뷰 반영 시 제거하고 `direction` 단일 계약으로 되돌렸다.
- **`Dialog` 본문을 `direction="vertical"`(`overflow-x: hidden`)로 두는 안**: 주문 승인 대화 상자 등에 `<Money />`·`<Quantity />`를 직접 넣을 때 좁은 폭에서 숫자가 조용히 잘리는 결함이 Chrome에서 재현되어 `direction="both"`로 전환했다.
- **`src/shared/platform/input.ts`에 `ScrollArea`·`ChartFrame`용 새 DOM 유틸리티를 추가하는 안**: `input.ts` 내보내기 목록과 `ds-dom.mjs`·`boundaries.mjs` 허용 목록이 `ds-screen-bootstrap.test.mjs`에 잠겨 있어, 보호 경계를 넓히지 않고 `ScrollArea`의 `scroll-content-y`(`overflow-x: clip`)와 `ChartFrame`의 순수 React 상태 복원으로 해결했다.
- **숫자 컴포넌트 자체를 가로 `ScrollArea`로 감싸는 안**: 인라인 숫자마다 별도 스크롤 컨테이너와 탭 정지점이 증식하므로, 메인 결정 #4대로 숫자 컴포넌트는 순수 인라인 한 줄을 유지하고 담는 컨테이너가 가로 `ScrollArea`를 제공하도록 고정했다.

## 224조건 해소 표

`{light,dark}`는 라이트·다크 두 테마를 뜻한다. 모든 53개 묶음(106개 이야기·224조건)이 해소되어 잔여 보류는 0건이다.

| 이야기                | ID                                      | 전(조건) | 해소 방식                                                                                           | 후(조건) |
| --------------------- | --------------------------------------- | -------: | --------------------------------------------------------------------------------------------------- | -------: |
| 레이아웃/Card         | `레이아웃-card--{light,dark}-0`         |        4 | Card 본문 가로 `ScrollArea` + 숫자 `w-max` + 스테이지 세로 `ScrollArea`                             |        0 |
| 레이아웃/Card         | `레이아웃-card--{light,dark}-5`         |        4 | Card 머리글 `flex-wrap` + 버튼 `min-h-control-height-sm` + 스테이지 세로 `ScrollArea`               |        0 |
| 기본/Button           | `기본-button--{light,dark}-12`          |        4 | 버튼 `min-h-control-height*` + 여러 줄 감싸기 + 스테이지 세로 `ScrollArea`                          |        0 |
| 기본/TextField        | `기본-textfield--{light,dark}-4`        |        8 | 비밀 입력 `flex-wrap` + `wrap-slot` + 버튼 `min-h-control-height-sm`                                |        0 |
| 기본/TextField        | `기본-textfield--{light,dark}-5`        |        8 | 비밀 입력 `flex-wrap` + `wrap-slot` + 버튼 `min-h-control-height-sm`                                |        0 |
| 피드백/Table          | `피드백-table--{light,dark}-0`          |        4 | Table 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                                |        0 |
| 피드백/Table          | `피드백-table--{light,dark}-4`          |        8 | Table 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                                |        0 |
| 숫자/Money            | `숫자-money--{light,dark}-0`            |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Money            | `숫자-money--{light,dark}-1`            |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Money            | `숫자-money--{light,dark}-2`            |        2 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Money            | `숫자-money--{light,dark}-3`            |        2 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Money            | `숫자-money--{light,dark}-6`            |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Percent          | `숫자-percent--{light,dark}-4`          |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/Quantity         | `숫자-quantity--{light,dark}-1`         |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/PriceChange      | `숫자-pricechange--{light,dark}-0`      |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/PriceChange      | `숫자-pricechange--{light,dark}-1`      |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/PriceChange      | `숫자-pricechange--{light,dark}-2`      |        2 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 숫자/PriceChange      | `숫자-pricechange--{light,dark}-3`      |        4 | 숫자 인라인 한 줄(`w-max`) 유지 + 카탈로그 숫자 슬롯 가로 `ScrollArea`                              |        0 |
| 차트/LineChart        | `차트-linechart--{light,dark}-0`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/LineChart        | `차트-linechart--{light,dark}-1`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/LineChart        | `차트-linechart--{light,dark}-4`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/LineChart        | `차트-linechart--{light,dark}-6`        |        4 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/AreaChart        | `차트-areachart--{light,dark}-0`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/AreaChart        | `차트-areachart--{light,dark}-1`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/AreaChart        | `차트-areachart--{light,dark}-4`        |        2 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/AreaChart        | `차트-areachart--{light,dark}-6`        |        4 | 스테이지 세로 `ScrollArea`(`position: relative`) + `ChartAxes`/`ChartNavigation` 가로 `ScrollArea`  |        0 |
| 차트/DonutChart       | `차트-donutchart--{light,dark}-0`       |        4 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/DonutChart       | `차트-donutchart--{light,dark}-1`       |        2 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/DonutChart       | `차트-donutchart--{light,dark}-3`       |        2 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/DonutChart       | `차트-donutchart--{light,dark}-4`       |        6 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/DonutChart       | `차트-donutchart--{light,dark}-6`       |        4 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/Treemap          | `차트-treemap--{light,dark}-0`          |        4 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/Treemap          | `차트-treemap--{light,dark}-1`          |        2 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/Treemap          | `차트-treemap--{light,dark}-3`          |        2 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/Treemap          | `차트-treemap--{light,dark}-4`          |        6 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 차트/Treemap          | `차트-treemap--{light,dark}-6`          |        4 | `ChartTable` 가로 `ScrollArea` + 스테이지 세로 `ScrollArea`                                         |        0 |
| 콘텐츠/Markdown       | `콘텐츠-markdown--{light,dark}-2`       |        8 | 스테이지 세로 `ScrollArea`(중첩 세로 스크롤 금지 유지)                                              |        0 |
| 콘텐츠/Markdown       | `콘텐츠-markdown--{light,dark}-3`       |        8 | 스테이지 세로 `ScrollArea`(중첩 세로 스크롤 금지 유지)                                              |        0 |
| 기본/Checkbox         | `기본-checkbox--{light,dark}-2`         |        8 | `Base.Indicator` 가운데 정렬 + 내부 `CheckMark`(`size-space-3`)                                     |        0 |
| 기본/Checkbox         | `기본-checkbox--{light,dark}-3`         |        8 | `Base.Indicator` 가운데 정렬 + 내부 `CheckMark`(`size-space-3`)                                     |        0 |
| 기본/SegmentedControl | `기본-segmentedcontrol--{light,dark}-0` |        4 | `ToggleGroup` `flex-wrap` + `Toggle` `min-h-control-height-sm`·줄바꿈                               |        0 |
| 기본/SegmentedControl | `기본-segmentedcontrol--{light,dark}-1` |        4 | `ToggleGroup` `flex-wrap` + `Toggle` `min-h-control-height-sm`·줄바꿈                               |        0 |
| 기본/SegmentedControl | `기본-segmentedcontrol--{light,dark}-2` |        4 | `ToggleGroup` `flex-wrap` + `Toggle` `min-h-control-height-sm`·줄바꿈                               |        0 |
| 기본/SegmentedControl | `기본-segmentedcontrol--{light,dark}-3` |        8 | `ToggleGroup` `flex-wrap` + `Toggle` `min-h-control-height-sm`·줄바꿈 + 스테이지 세로 `ScrollArea`  |        0 |
| 기본/Disclosure       | `기본-disclosure--{light,dark}-2`       |        4 | `min-h-toolbar-height` + 왼쪽 정렬 + 제목 `status-sentence flex-1`                                  |        0 |
| 피드백/Badge          | `피드백-badge--{light,dark}-6`          |        4 | `whitespace-nowrap` 제거 + `status-sentence max-w-full`                                             |        0 |
| 피드백/StatusBar      | `피드백-statusbar--{light,dark}-0`      |        4 | `px-space-4` + `flex-wrap` + `wrap-slot`(아이콘 동행) + 버튼 최소 높이                              |        0 |
| 피드백/StatusBar      | `피드백-statusbar--{light,dark}-2`      |        4 | `px-space-4` + `flex-wrap` + `wrap-slot`(아이콘 동행) + 버튼 최소 높이                              |        0 |
| 피드백/StatusBar      | `피드백-statusbar--{light,dark}-4`      |        4 | `px-space-4` + `flex-wrap` + `wrap-slot`(아이콘 동행) + 버튼 최소 높이                              |        0 |
| 피드백/StatusBar      | `피드백-statusbar--{light,dark}-10`     |        4 | `px-space-4` + `flex-wrap` + `wrap-slot`(아이콘 동행) + 버튼 최소 높이 + 스테이지 세로 `ScrollArea` |        0 |
| 피드백/Dialog         | `피드백-dialog--{light,dark}-4`         |        8 | `Dialog` `Viewport` `flex` + `Popup` `max-h-full` + 고정 머리·바닥 + 본문 양방향 `ScrollArea`       |        0 |
| 피드백/EmptyState     | `피드백-emptystate--{light,dark}-0`     |        2 | 스테이지 세로 `ScrollArea`가 문서 세로 초과를 흡수(126px/125% 본문 높이 659px → 뷰포트 내 스크롤)   |        0 |
| 피드백/EmptyState     | `피드백-emptystate--{light,dark}-5`     |        4 | 버튼 `min-h-control-height-sm` + 스테이지 세로 `ScrollArea`                                         |        0 |
| **합계**              | **53개 묶음(106개 이야기)**             |  **224** | **전원 해소**                                                                                       |    **0** |

## 보호 파일 및 승인된 기존 테스트 변경

메인의 명시적 승인 범위 안에서만 변경했고 검사 규칙은 느슨하게 하지 않았다.

- 보호 생성기·허용 목록·생성물:
  - `apps/desktop/eslint/ds-design-usage.mjs` / `apps/desktop/src/shared/ds/theme.css` / `harness/ds-style/utilities.json`: 새 토큰 없이 시맨틱 유틸리티 9개 추가 및 재생성.
  - `apps/desktop/eslint/ds-classes.mjs`: `min-h-control-height-sm`, `size-space-3`, `flex-wrap`, `h-full`, `max-h-full`, `w-max`와 새 유틸리티 9개 등록.
  - `apps/desktop/eslint/ds-overflow.mjs`: `LENGTH_EXCEPTIONS`에 `Icon.tsx`(`size-space-3`)와 `ScrollArea.tsx`(`scroll-track-x`, `scroll-track-y`, `scroll-content-y`)만 한정 추가.
  - `harness/ds-catalog/generation.mjs` / `apps/desktop/.storybook/{catalog.mjs,preview.mjs}`: 스테이지 세로 `ScrollArea`와 숫자 슬롯 가로 `ScrollArea` 반영 후 생성기로 재생성.
  - `harness/knip-check/production-allowlist.json`: `src/shared/ds/ScrollArea.tsx` 1건 추가.
  - `harness/ds-overflow/pending.json`: 해소된 224조건 제거(`[]`).
- 승인된 기존 테스트 값 갱신(검증 성질 유지):
  - `apps/desktop/tests/ds-components-interaction.test.mjs`, `apps/desktop/tests/ds21-actions.test.mjs`: `h-control-height*` → `min-h-control-height*`.
  - `apps/desktop/tests/knip.test.mjs`: 기대 목록에 `src/shared/ds/ScrollArea.tsx` 1건 추가.
  - `apps/desktop/tests/ds19-policy.test.mjs`: `LENGTH_EXCEPTIONS` 기대값에 위 두 파일의 4개 유틸리티 반영.
  - `apps/desktop/tests/ds07-render.test.mjs`: `<section>`과 `[data-scroll-viewport]` 존재를 함께 확인한 뒤 `[data-catalog-content]`를 원본 렌더와 대조.
  - `apps/desktop/tests/ds-generation.test.mjs`: 재생성된 `theme.css` 전체 문자열 갱신.
- 추가 테스트(기존 줄 삭제·수정 없음):
  - `apps/desktop/tests/ds23-layout-scroll.test.mjs`: 신규 단위·정책 테스트 4건.
  - `apps/desktop/tests/ds20-overflow.browser.ts`: 기존 183건 무변경, DS-23 브라우저 회귀 20건 추가(총 203건).

## 검증

- 로컬 macOS Chrome(Vitest Browser Mode, `960×640`, `ko-KR`, 묶은 IBM Plex Sans KR/Mono 글꼴):
  - `apps/desktop/tests/ds20-overflow.browser.ts`: 203/203 통과(기존 183건 + DS-23 회귀 20건: 숫자 한 줄 유지·가로 스크롤, 초과 시에만 막대·`tabIndex=0` 표시 및 내용 폭 불변, 동적 콘텐츠 증가 시 `BaseScrollArea.Content` 갱신, 단방향 세로 `ScrollArea` 비활성 축 넘침 시 `tabIndex=-1` 유지 및 `clipped` 검출, 스테이지가 실제 가로 넘침·중첩 세로 스크롤을 가리지 않음, 비밀 입력 붙여넣기 줄바꿈, Dialog 머리·바닥 고정 및 본문 세로·가로 스크롤, 버튼 확장·상태 띠 아이콘 동행 및 동작 줄바꿈·체크박스 테두리 내부 정렬, 포커스된 LineChart 상태 숫자 키보드 탭 스크롤 및 이탈 초기화, ChartAxes 최대값 우측 정렬, 가로 스크롤 막대와 글자 비겹침, Card 중첩 래퍼 및 내부 스크롤 비이중화).
  - `apps/desktop/tests/ds20-catalog.browser.ts`: 전체 1,034개 이야기(4,136조건) 전수 실행 결과 8/8 통과, 오류 0건, 위반 0건, `comparePending([], pending.json)` 불일치 0건.
- 단위·정책·정적 검사: `ds23-layout-scroll.test.mjs`(4건), `ds06-charts.test.mjs`(7건), `ds22-overflow-verdict.test.mjs`(5건), `ds19-review-three.test.mjs`(2건), `ds-screen-bootstrap.test.mjs`, `ds-style`, `ds-tokens`, `tokens-contrast`, `ds-catalog`, `knip-check`, `vitest-check`, `eslint --max-warnings 0`, `tsc`, `vite build`, `just check-ts`, `just check-fast-static`, `just test-diff`, `just disable-report-since`, `just hooks-installed`.
- 공식 고정 Docker Playwright 컨테이너(`just ds-check`, `just visual-update`, `just visual-check`)는 실행 기기에 Docker가 없어 미실행이며 원격 CI 및 메인의 Docker 환경에서 최종 확인한다.

## 손 변형

별도 임시 복사본(`mktemp -d target/DS-23/mut-*`)에서 기준 복사본의 `tsc`와 테스트 통과를 먼저 확인한 뒤, DS-23 새·수정 코드의 분기·클래스·규칙 34건을 하나씩 변형해 컴파일 통과 및 검출 여부를 직접 실행했다(34/34 검출, 생존 0, 컴파일 실패 0).

| 번호 | 바꾼 것                                                                                       | 잡은 검사/테스트               | 결과 |
| ---: | --------------------------------------------------------------------------------------------- | ------------------------------ | ---- |
|    1 | `ds-design-usage.mjs`: `scroll-viewport-x`의 `position: relative` 제거                        | `harness/ds-style/check.mjs`   | 검출 |
|    2 | `ds-design-usage.mjs`: `scroll-viewport-x`의 `overflow-y: hidden !important` 제거             | `harness/ds-style/check.mjs`   | 검출 |
|    3 | `ds-design-usage.mjs`: `scroll-viewport-y`의 `overflow-x: hidden !important` 제거             | `harness/ds-style/check.mjs`   | 검출 |
|    4 | `ds-design-usage.mjs`: `scroll-viewport-both`의 `overflow-x: auto !important` 제거            | `harness/ds-style/check.mjs`   | 검출 |
|    5 | `ds-design-usage.mjs`: `scroll-content-y`의 `overflow-x: clip` 제거                           | `harness/ds-style/check.mjs`   | 검출 |
|    6 | `ScrollArea.tsx`: `viewportClasses.horizontal`을 `scroll-viewport-y`로 변경                   | `ds23-layout-scroll.test.mjs`  | 검출 |
|    7 | `ScrollArea.tsx`: `data-scroll-viewport` 표식 제거                                            | `ds23-layout-scroll.test.mjs`  | 검출 |
|    8 | `ScrollArea.tsx`: 가로 뷰포트의 `pb-space-2`(막대 여유) 제거                                  | `ds23-layout-scroll.test.mjs`  | 검출 |
|    9 | `ScrollArea.tsx`: 가로 뷰포트의 `focus-visible:scroll-outline-inset` 제거                     | `ds23-layout-scroll.test.mjs`  | 검출 |
|   10 | `ScrollArea.tsx`: `BaseScrollArea.Content`를 일반 `div`로 교체                                | `ds23-layout-scroll.test.mjs`  | 검출 |
|   11 | `ScrollArea.tsx`: 세로 콘텐츠의 `scroll-content-y` 제거                                       | `ds23-layout-scroll.test.mjs`  | 검출 |
|   12 | `Numbers.tsx`: `Unavailable` 숫자 스팬의 `w-max` 제거                                         | `ds23-layout-scroll.test.mjs`  | 검출 |
|   13 | `Card.tsx`: 본문 `ScrollArea` 방향을 `vertical`로 변경                                        | `ds23-layout-scroll.test.mjs`  | 검출 |
|   14 | `Table.tsx`: 표 `ScrollArea` 방향을 `vertical`로 변경                                         | `ds23-layout-scroll.test.mjs`  | 검출 |
|   15 | `ChartTable.tsx`: 도넛·트리맵 표 `ScrollArea` 방향을 `vertical`로 변경                        | `ds23-layout-scroll.test.mjs`  | 검출 |
|   16 | `ChartTable.tsx`: 추이 차트 숨긴 표를 `ScrollArea`로 감쌈                                     | `ds23-layout-scroll.test.mjs`  | 검출 |
|   17 | `ChartTable.tsx`: `ChartAxes` 최소 숫자 `ScrollArea` 방향을 `vertical`로 변경                 | `ds23-layout-scroll.test.mjs`  | 검출 |
|   18 | `ChartTable.tsx`: `ChartAxes` 최대 값의 `@content-medium:justify-end` 제거                    | `ds23-layout-scroll.test.mjs`  | 검출 |
|   19 | `ChartTable.tsx`: `ChartStatusBody` 숫자 `ScrollArea` 방향을 `vertical`로 변경                | `ds23-layout-scroll.test.mjs`  | 검출 |
|   20 | `ChartFrame.tsx`: 상태 영역 `focus` 시 `remembered` 복원 제거                                 | `ds23-layout-scroll.test.mjs`  | 검출 |
|   21 | `button-variants.tsx`: 기본 버튼 `min-h-control-height`를 `h-control-height`로 되돌림         | `ds23-layout-scroll.test.mjs`  | 검출 |
|   22 | `button-variants.tsx`: 작은 버튼 `min-h-control-height-sm`을 `h-control-height-sm`으로 되돌림 | `ds23-layout-scroll.test.mjs`  | 검출 |
|   23 | `TextField.tsx`: 비밀 입력 줄의 `flex-wrap` 제거                                              | `ds23-layout-scroll.test.mjs`  | 검출 |
|   24 | `TextField.tsx`: 입력 묶음의 `wrap-slot` 제거                                                 | `ds23-layout-scroll.test.mjs`  | 검출 |
|   25 | `Dialog.tsx`: 본문 `both` `ScrollArea`를 `vertical` 전용으로 되돌림                           | `ds23-layout-scroll.test.mjs`  | 검출 |
|   26 | `Dialog.tsx`: `Popup`의 `max-h-full` 제거                                                     | `ds23-layout-scroll.test.mjs`  | 검출 |
|   27 | `Icon.tsx`: `CheckMark`의 `size-space-3`을 `size-icon-sm`으로 변경                            | `ds23-layout-scroll.test.mjs`  | 검출 |
|   28 | `Badge.tsx`: `whitespace-nowrap` 복원                                                         | `ds23-layout-scroll.test.mjs`  | 검출 |
|   29 | `SegmentedControl.tsx`: `min-h-control-height-sm`을 `h-control-height-sm`으로 되돌림          | `ds23-layout-scroll.test.mjs`  | 검출 |
|   30 | `StatusBar.tsx`: 좌우 여백을 `px-space-6`으로 되돌림                                          | `ds23-layout-scroll.test.mjs`  | 검출 |
|   31 | `StatusBar.tsx`: 아이콘을 `wrap-slot` 바깥으로 되돌림                                         | `ds23-layout-scroll.test.mjs`  | 검출 |
|   32 | `Disclosure.tsx`: `min-h-toolbar-height`를 `h-toolbar-height`로 되돌림                        | `ds23-layout-scroll.test.mjs`  | 검출 |
|   33 | `ds-overflow.mjs`: `ScrollArea.tsx` 길이 예외 제거                                            | `ds19-policy.test.mjs`         | 검출 |
|   34 | `harness/ds-catalog/generation.mjs`: 스테이지 `ScrollArea` 방향을 `horizontal`로 변경         | `harness/ds-catalog/check.mjs` | 검출 |

## 인계

- **DS-11**: `src/shared/ds/ScrollArea.tsx`는 DS-23에서 선행 구현했다. DS-11은 나머지 레이아웃 컴포넌트(`Page`, `Section`, `Grid`, `GridItem`, `Stack`, `Inline`, `Split`, `Toolbar`, `Divider`) 구현, `ScrollArea`의 `src/shared/ds/index.ts` 정식 공개 노출 여부 및 컨테이너 쿼리 기준 화면을 이어받는다.
- **DS-12**: 공개 목록 잠금 시 이번에 겉모습과 배치가 바뀐 컴포넌트(`Button`, `Card`, `Money`, `TextField`, `Table`, `Dialog`, `Checkbox`, `Badge`, `SegmentedControl`, `StatusBar`, `Disclosure`, `ChartTable`, `ScrollArea`)의 기준 화면 범위와 긴 금액을 포함한 `Dialog` 부하 이야기·중첩 가로 스크롤 정책 검증을 함께 점검한다.
- **메인**: `docs/tasks.md`의 DS-11 행에 `ScrollArea` 선행 구현 사실을 반영하고, Docker 환경에서 `just ds-check`·`just visual-update`·`just visual-check`를 실행해 공식 Linux arm64 넘침 0건 확인과 시각 기준 화면(`harness/visual/baselines/{Button,Card,Money,TextField}-{light,dark}-chromium-linux-arm64.png`, `measurement.json`) 갱신 및 보호 경로 변경분 사용자 서명 커밋을 처리한다.
