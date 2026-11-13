# DS-13: 토큰 생성과 대비 검사

- 상태: 초안
- 입력: `apps/desktop/design-tokens/tokens.json` (ADR 0023)

## 결정

생성·재생성 비교는 Rust `xtask gen-tokens`와 `xtask gen-tokens --check`가 한다. 색 대비는 `harness/ds-tokens/contrast.mjs`, `just tokens-contrast`가 한다. tasks의 `xtask tokens-contrast`에서 달라지는 결정이다. Rust workspace의 끌 수 없는 `float_arithmetic = forbid`와 기존 테스트를 유지한다. R-14의 대비 실측도 스크립트로 수행했다. Node 스크립트용 ESLint는 숫자 연산·Number를 금지하지 않는다. 문서 본문 수정은 메인이 병합할 때 한다.

생성기는 기존 xtask JSON 경계 파서를 재사용하고 새 crate를 추가하지 않는다. px→rem은 정수 십진수 연산으로 정확히 16으로 나눈다. 생성 결과를 메모리에서 다시 만들고 파일 바이트와 비교한다. 검사 모드는 파일을 쓰지 않는다. 출력은 저장소에 고정된 Prettier를 표준 입력 경계로 실행해 정규화한 뒤 비교한다. 따라서 gen-tokens도 시작 절차의 pnpm 설치가 필요하다. 입력 유효성·디자인 원본 일치는 기존 DS-01 검사가 담당하며, 생성기도 누락·형식 오류는 실패시킨다.

## 대응 규칙

| 입력                    | CSS 변수                                                       | 유틸리티 예                                           | 단위                                                   |
| ----------------------- | -------------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------ |
| color 이름              | `--color-<이름>`                                               | `bg-surface`, `text-ink`                              | 원값, 같은 테마의 참조는 CSS var                       |
| spacing `space-*`       | `--spacing-space-*`                                            | `p-space-4`                                           | rem                                                    |
| radius `radius-*`       | `--radius-*`                                                   | `rounded-md`                                          | rem                                                    |
| shadow `shadow-popover` | `--shadow-popover: var(--sc-shadow-popover)`                   | `shadow-popover`                                      | 원값을 테마별 `--sc-shadow-popover`에 둠               |
| size의 일반 길이        | `--spacing-<이름>`                                             | `w-sidebar-width`, `h-control-height`, `size-icon-sm` | rem                                                    |
| size `border-*`         | `--border-width-*`                                             | `border-hairline`                                     | px, 별도 정적 @utility                                 |
| size `focus-offset`     | `--sc-focus-offset`                                            | `outline-offset-focus`                                | px, 별도 정적 @utility                                 |
| size `bubble-max-width` | `--spacing-bubble-max-width`                                   | `max-w-bubble-max-width`                              | 80% 보존                                               |
| duration `duration-*`   | `--transition-duration-*`                                      | `duration-fast`                                       | ms                                                     |
| easing `ease-*`         | `--ease-*`                                                     | `ease-out`                                            | 원값                                                   |
| zIndex `z-*`            | `--z-index-*`                                                  | `z-modal`                                             | 단위 없음                                              |
| opacity `opacity-*`     | `--opacity-*`                                                  | `opacity-disabled`                                    | 단위 없음                                              |
| layout 일반 길이        | `--spacing-<이름>`                                             | `max-w-content-max`                                   | rem                                                    |
| layout `bp-window-*`    | `--breakpoint-window-*`                                        | `window-md:*`                                         | rem; CSS viewport 구간                                 |
| layout `bp-content-*`   | `--container-content-*`                                        | `@content-medium:*`                                   | rem; CSS container 구간                                |
| layout `grid-columns`   | `--sc-grid-columns`                                            | `grid-cols-layout`                                    | 정수; `repeat(var(--sc-grid-columns), minmax(0, 1fr))` |
| text `text-*`           | `--text-size-*`                                                | `text-size-xs`                                        | rem                                                    |
| text `leading-*`        | `--leading-*`                                                  | `leading-xs`                                          | rem                                                    |
| text `weight-*`         | `--font-weight-*`                                              | `font-semibold`                                       | 정수                                                   |
| text `tracking-*`       | `--tracking-*`                                                 | `tracking-display`                                    | rem                                                    |
| type.families           | `--font-sans`, `--font-mono`                                   | `font-sans`                                           | 원값                                                   |
| type.groups.styles      | `--text-<이름>` 및 부속 line-height/font-weight/letter-spacing | `text-body`                                           | rem, 굵기는 정수                                       |

원시 text-sm은 디자인의 토큰 이름이므로 TS 유니온에는 남긴다. 하지만 기본 Tailwind의 text-sm과 혼동하지 않도록 CSS는 text-size-sm이다. --text-text-xs 같은 중복 접두어도 만들지 않는다. 기본 테마를 `@theme { --*: initial; }`로 지우고 `bg-red-500`, `p-5`, `text-sm`은 생성하지 않는다. 숫자를 직접 받는 Tailwind 내장 클래스의 전면 금지는 후속 DS-02다.

폭·높이·아이콘은 간격 namespace를 공유해 w/h/size에서 쓴다. border는 길이 namespace에 섞지 않는다. bp는 JS 전용으로 만들지 않는다. CSS media/container 구간으로 생성하며 TS 파일에는 값이 없다. JS 기반 창 동작이 필요하면 후속 platform 어댑터가 계산된 CSS 변수에서 읽는다. grid-columns는 px로 변환하지 않는다.

토큰의 테마 공통값도 테마별 덮어쓰기로 생성해 입력 변경을 놓치지 않는다. 명시적 data-theme가 시스템 설정보다 우선한다. 시스템 다크는 `:root:not([data-theme])`, 수동 다크는 `:root[data-theme="dark"]`로 덮어쓴다. 그림자는 Tailwind의 인라인 전개를 막기 위해 원값 변수를 한 단계 둔다.

## 글꼴과 진입점

글꼴 경로는 design 기준으로 검증하고 생성 CSS에서 `../../../../../design/fonts/...`로 참조한다. Vite가 로컬 woff2를 해시 자산으로 번들한다. 복사본을 만들지 않아 원본·OFL 보호 범위를 유지한다. 외부 글꼴 요청은 없다. 앱 HTML이 shared/ds/theme.css를 읽고 이 파일이 Tailwind와 생성 tokens.css를 가져온다. 화면 코드에서 CSS를 가져오지 않는다. 생성 CSS에는 JSX용 ESLint 대신 재생성 동일성 검사를 적용한다.

## 대비 표와 계산

보호 파일 `harness/ds-tokens/contrast-pairs.json`에 필수 조합을 둔다. 이름과 최소 기준의 닫힌 기대 조합을 테스트에서 독립적으로 고정하며, 각 원소 삭제·변경을 거부한다. 본문 ink 3종 및 가격색은 bg/surface/surface-sunken 위 4.5, line-strong과 focus-ring은 같은 배경 위 3, 의미 색의 soft 위 글자와 on-* 버튼 글자는 4.5, 차트 6종은 surface 위 3이다. 장식 line과 overlay는 표에 넣지 않는다.

sRGB 채널은 255로 나누고 0.04045 이하에서 /12.92, 나머지는 ((c+0.055)/1.055)^2.4를 적용한다. 상대 휘도는 0.2126R+0.7152G+0.0722B, 대비는 (밝은 휘도+0.05)/(어두운 휘도+0.05)다. 통과 비교는 반올림 전 값으로 한다. 표시는 소수 여섯 자리다.

알파 전경은 불투명 배경 위에서 각 sRGB 채널을 aF+(1-a)B로 먼저 합성한 뒤 선형화한다. 알파 배경은 underlay 토큰이 명시돼야 하며 그 토큰은 불투명해야 한다. 배경이 불투명해질 근거가 없으면 실패한다. 현재 필수 조합은 모두 불투명하다. 미달 조합은 예외 없이 실패하고 디자인 원본을 수정하지 않는다.

## 실측과 검증

실제 Tailwind 4.3.5 컴파일은 138개 토큰 이름 모두 기대한 클래스를 생성했다. 기본 테마를 먼저 읽은 뒤 생성 테마를 적용하는 조건에서 bg-red-500, p-5, text-sm은 모두 생성되지 않았다. --*: initial 제거 변형은 이 검사를 실패시킨다.

### Tailwind 4.3.5 토큰별 실측

| 그룹     | 토큰              | CSS 변수                      | 유틸리티                 | 실제 생성 |
| -------- | ----------------- | ----------------------------- | ------------------------ | --------- |
| color    | bg                | `--color-bg`                  | `bg-bg`                  | 통과      |
| color    | surface           | `--color-surface`             | `bg-surface`             | 통과      |
| color    | surface-raised    | `--color-surface-raised`      | `bg-surface-raised`      | 통과      |
| color    | surface-sunken    | `--color-surface-sunken`      | `bg-surface-sunken`      | 통과      |
| color    | line              | `--color-line`                | `bg-line`                | 통과      |
| color    | line-strong       | `--color-line-strong`         | `bg-line-strong`         | 통과      |
| color    | ink               | `--color-ink`                 | `bg-ink`                 | 통과      |
| color    | ink-muted         | `--color-ink-muted`           | `bg-ink-muted`           | 통과      |
| color    | ink-subtle        | `--color-ink-subtle`          | `bg-ink-subtle`          | 통과      |
| color    | primary           | `--color-primary`             | `bg-primary`             | 통과      |
| color    | primary-hover     | `--color-primary-hover`       | `bg-primary-hover`       | 통과      |
| color    | primary-soft      | `--color-primary-soft`        | `bg-primary-soft`        | 통과      |
| color    | on-primary        | `--color-on-primary`          | `bg-on-primary`          | 통과      |
| color    | up                | `--color-up`                  | `bg-up`                  | 통과      |
| color    | up-soft           | `--color-up-soft`             | `bg-up-soft`             | 통과      |
| color    | on-up             | `--color-on-up`               | `bg-on-up`               | 통과      |
| color    | up-hover          | `--color-up-hover`            | `bg-up-hover`            | 통과      |
| color    | down              | `--color-down`                | `bg-down`                | 통과      |
| color    | down-soft         | `--color-down-soft`           | `bg-down-soft`           | 통과      |
| color    | on-down           | `--color-on-down`             | `bg-on-down`             | 통과      |
| color    | down-hover        | `--color-down-hover`          | `bg-down-hover`          | 통과      |
| color    | alert             | `--color-alert`               | `bg-alert`               | 통과      |
| color    | alert-soft        | `--color-alert-soft`          | `bg-alert-soft`          | 통과      |
| color    | on-alert          | `--color-on-alert`            | `bg-on-alert`            | 통과      |
| color    | focus-ring        | `--color-focus-ring`          | `bg-focus-ring`          | 통과      |
| color    | chart-1           | `--color-chart-1`             | `bg-chart-1`             | 통과      |
| color    | chart-2           | `--color-chart-2`             | `bg-chart-2`             | 통과      |
| color    | chart-3           | `--color-chart-3`             | `bg-chart-3`             | 통과      |
| color    | chart-4           | `--color-chart-4`             | `bg-chart-4`             | 통과      |
| color    | chart-5           | `--color-chart-5`             | `bg-chart-5`             | 통과      |
| color    | chart-6           | `--color-chart-6`             | `bg-chart-6`             | 통과      |
| color    | overlay           | `--color-overlay`             | `bg-overlay`             | 통과      |
| spacing  | space-1           | `--spacing-space-1`           | `p-space-1`              | 통과      |
| spacing  | space-2           | `--spacing-space-2`           | `p-space-2`              | 통과      |
| spacing  | space-3           | `--spacing-space-3`           | `p-space-3`              | 통과      |
| spacing  | space-4           | `--spacing-space-4`           | `p-space-4`              | 통과      |
| spacing  | space-5           | `--spacing-space-5`           | `p-space-5`              | 통과      |
| spacing  | space-6           | `--spacing-space-6`           | `p-space-6`              | 통과      |
| spacing  | space-7           | `--spacing-space-7`           | `p-space-7`              | 통과      |
| spacing  | space-8           | `--spacing-space-8`           | `p-space-8`              | 통과      |
| spacing  | space-9           | `--spacing-space-9`           | `p-space-9`              | 통과      |
| spacing  | space-10          | `--spacing-space-10`          | `p-space-10`             | 통과      |
| spacing  | space-11          | `--spacing-space-11`          | `p-space-11`             | 통과      |
| spacing  | space-12          | `--spacing-space-12`          | `p-space-12`             | 통과      |
| radius   | radius-sm         | `--radius-sm`                 | `rounded-sm`             | 통과      |
| radius   | radius-md         | `--radius-md`                 | `rounded-md`             | 통과      |
| radius   | radius-lg         | `--radius-lg`                 | `rounded-lg`             | 통과      |
| radius   | radius-pill       | `--radius-pill`               | `rounded-pill`           | 통과      |
| shadow   | shadow-popover    | `--shadow-popover`            | `shadow-popover`         | 통과      |
| size     | sidebar-width     | `--spacing-sidebar-width`     | `w-sidebar-width`        | 통과      |
| size     | sidebar-collapsed | `--spacing-sidebar-collapsed` | `w-sidebar-collapsed`    | 통과      |
| size     | chat-width        | `--spacing-chat-width`        | `w-chat-width`           | 통과      |
| size     | content-min       | `--spacing-content-min`       | `w-content-min`          | 통과      |
| size     | row-height        | `--spacing-row-height`        | `h-row-height`           | 통과      |
| size     | row-height-dense  | `--spacing-row-height-dense`  | `h-row-height-dense`     | 통과      |
| size     | control-height    | `--spacing-control-height`    | `h-control-height`       | 통과      |
| size     | control-height-sm | `--spacing-control-height-sm` | `h-control-height-sm`    | 통과      |
| size     | control-pad-sm    | `--spacing-control-pad-sm`    | `w-control-pad-sm`       | 통과      |
| size     | chat-width-min    | `--spacing-chat-width-min`    | `w-chat-width-min`       | 통과      |
| size     | chat-width-max    | `--spacing-chat-width-max`    | `w-chat-width-max`       | 통과      |
| size     | toolbar-height    | `--spacing-toolbar-height`    | `h-toolbar-height`       | 통과      |
| size     | list-item-height  | `--spacing-list-item-height`  | `h-list-item-height`     | 통과      |
| size     | icon-sm           | `--spacing-icon-sm`           | `w-icon-sm`              | 통과      |
| size     | icon-md           | `--spacing-icon-md`           | `w-icon-md`              | 통과      |
| size     | icon-lg           | `--spacing-icon-lg`           | `w-icon-lg`              | 통과      |
| size     | dot-size          | `--spacing-dot-size`          | `w-dot-size`             | 통과      |
| size     | bar-height        | `--spacing-bar-height`        | `h-bar-height`           | 통과      |
| size     | meter-height      | `--spacing-meter-height`      | `h-meter-height`         | 통과      |
| size     | gap-hairline      | `--spacing-gap-hairline`      | `w-gap-hairline`         | 통과      |
| size     | border-hairline   | `--border-width-hairline`     | `border-hairline`        | 통과      |
| size     | border-mark       | `--border-width-mark`         | `border-mark`            | 통과      |
| size     | border-focus      | `--border-width-focus`        | `border-focus`           | 통과      |
| size     | focus-offset      | `--sc-focus-offset`           | `outline-offset-focus`   | 통과      |
| size     | dialog-width      | `--spacing-dialog-width`      | `w-dialog-width`         | 통과      |
| size     | dialog-width-wide | `--spacing-dialog-width-wide` | `w-dialog-width-wide`    | 통과      |
| size     | popover-min-width | `--spacing-popover-min-width` | `w-popover-min-width`    | 통과      |
| size     | tooltip-max-width | `--spacing-tooltip-max-width` | `w-tooltip-max-width`    | 통과      |
| size     | chart-height-sm   | `--spacing-chart-height-sm`   | `h-chart-height-sm`      | 통과      |
| size     | chart-height      | `--spacing-chart-height`      | `h-chart-height`         | 통과      |
| size     | chart-height-lg   | `--spacing-chart-height-lg`   | `h-chart-height-lg`      | 통과      |
| size     | chat-input-min    | `--spacing-chat-input-min`    | `w-chat-input-min`       | 통과      |
| size     | chat-input-max    | `--spacing-chat-input-max`    | `w-chat-input-max`       | 통과      |
| size     | bubble-max-width  | `--spacing-bubble-max-width`  | `max-w-bubble-max-width` | 통과      |
| duration | duration-fast     | `--transition-duration-fast`  | `duration-fast`          | 통과      |
| duration | duration-base     | `--transition-duration-base`  | `duration-base`          | 통과      |
| duration | duration-flash    | `--transition-duration-flash` | `duration-flash`         | 통과      |
| easing   | ease-out          | `--ease-out`                  | `ease-out`               | 통과      |
| easing   | ease-in           | `--ease-in`                   | `ease-in`                | 통과      |
| zIndex   | z-sticky          | `--z-index-sticky`            | `z-sticky`               | 통과      |
| zIndex   | z-panel           | `--z-index-panel`             | `z-panel`                | 통과      |
| zIndex   | z-popover         | `--z-index-popover`           | `z-popover`              | 통과      |
| zIndex   | z-modal           | `--z-index-modal`             | `z-modal`                | 통과      |
| zIndex   | z-toast           | `--z-index-toast`             | `z-toast`                | 통과      |
| opacity  | opacity-disabled  | `--opacity-disabled`          | `opacity-disabled`       | 통과      |
| opacity  | opacity-area      | `--opacity-area`              | `opacity-area`           | 통과      |
| layout   | window-min-width  | `--spacing-window-min-width`  | `w-window-min-width`     | 통과      |
| layout   | window-min-height | `--spacing-window-min-height` | `h-window-min-height`    | 통과      |
| layout   | bp-window-md      | `--breakpoint-window-md`      | `window-md:block`        | 통과      |
| layout   | bp-window-lg      | `--breakpoint-window-lg`      | `window-lg:block`        | 통과      |
| layout   | bp-content-medium | `--container-content-medium`  | `@content-medium:block`  | 통과      |
| layout   | bp-content-wide   | `--container-content-wide`    | `@content-wide:block`    | 통과      |
| layout   | content-max       | `--spacing-content-max`       | `w-content-max`          | 통과      |
| layout   | prose-max         | `--spacing-prose-max`         | `w-prose-max`            | 통과      |
| layout   | grid-columns      | `--sc-grid-columns`           | `grid-cols-layout`       | 통과      |
| text     | text-xs           | `--text-size-xs`              | `text-size-xs`           | 통과      |
| text     | text-sm           | `--text-size-sm`              | `text-size-sm`           | 통과      |
| text     | text-md           | `--text-size-md`              | `text-size-md`           | 통과      |
| text     | text-base         | `--text-size-base`            | `text-size-base`         | 통과      |
| text     | text-lg           | `--text-size-lg`              | `text-size-lg`           | 통과      |
| text     | text-xl           | `--text-size-xl`              | `text-size-xl`           | 통과      |
| text     | text-2xl          | `--text-size-2xl`             | `text-size-2xl`          | 통과      |
| text     | text-3xl          | `--text-size-3xl`             | `text-size-3xl`          | 통과      |
| text     | leading-xs        | `--leading-xs`                | `leading-xs`             | 통과      |
| text     | leading-sm        | `--leading-sm`                | `leading-sm`             | 통과      |
| text     | leading-md        | `--leading-md`                | `leading-md`             | 통과      |
| text     | leading-base      | `--leading-base`              | `leading-base`           | 통과      |
| text     | leading-lg        | `--leading-lg`                | `leading-lg`             | 통과      |
| text     | leading-xl        | `--leading-xl`                | `leading-xl`             | 통과      |
| text     | leading-3xl       | `--leading-3xl`               | `leading-3xl`            | 통과      |
| text     | weight-regular    | `--font-weight-regular`       | `font-regular`           | 통과      |
| text     | weight-medium     | `--font-weight-medium`        | `font-medium`            | 통과      |
| text     | weight-semibold   | `--font-weight-semibold`      | `font-semibold`          | 통과      |
| text     | weight-bold       | `--font-weight-bold`          | `font-bold`              | 통과      |
| text     | tracking-display  | `--tracking-display`          | `tracking-display`       | 통과      |
| font     | sans              | `--font-sans`                 | `font-sans`              | 통과      |
| font     | mono              | `--font-mono`                 | `font-mono`              | 통과      |
| type     | num-display       | `--text-num-display`          | `text-num-display`       | 통과      |
| type     | num-lg            | `--text-num-lg`               | `text-num-lg`            | 통과      |
| type     | num-md            | `--text-num-md`               | `text-num-md`            | 통과      |
| type     | num-sm            | `--text-num-sm`               | `text-num-sm`            | 통과      |
| type     | title-lg          | `--text-title-lg`             | `text-title-lg`          | 통과      |
| type     | title-md          | `--text-title-md`             | `text-title-md`          | 통과      |
| type     | title-sm          | `--text-title-sm`             | `text-title-sm`          | 통과      |
| type     | body              | `--text-body`                 | `text-body`              | 통과      |
| type     | body-sm           | `--text-body-sm`              | `text-body-sm`           | 통과      |
| type     | label             | `--text-label`                | `text-label`             | 통과      |
| type     | caption           | `--text-caption`              | `text-caption`           | 통과      |
| type     | code              | `--text-code`                 | `text-code`              | 통과      |

### 필수 대비 38개 조합 실측

반올림 전 값으로 판정한 76건 모두 통과했다. 아래 표는 표시만 소수 여섯 자리로 반올림했다.

| 전경 / 배경                  |    라이트 |      다크 | 최소 |
| ---------------------------- | --------: | --------: | ---: |
| ink / bg                     | 16.265309 | 16.390612 |  4.5 |
| ink / surface                | 17.766475 | 15.172797 |  4.5 |
| ink / surface-sunken         | 15.146074 | 16.852920 |  4.5 |
| ink-muted / bg               |  6.331259 |  8.035710 |  4.5 |
| ink-muted / surface          |  6.915587 |  7.438660 |  4.5 |
| ink-muted / surface-sunken   |  5.895598 |  8.262362 |  4.5 |
| ink-subtle / bg              |  5.099310 |  6.048666 |  4.5 |
| ink-subtle / surface         |  5.569938 |  5.599253 |  4.5 |
| ink-subtle / surface-sunken  |  4.748420 |  6.219273 |  4.5 |
| up / bg                      |  4.896405 |  6.332869 |  4.5 |
| up / surface                 |  5.348307 |  5.862340 |  4.5 |
| up / surface-sunken          |  4.559478 |  6.511492 |  4.5 |
| down / bg                    |  5.531939 |  6.967917 |  4.5 |
| down / surface               |  6.042495 |  6.450204 |  4.5 |
| down / surface-sunken        |  5.151280 |  7.164452 |  4.5 |
| line-strong / bg             |  3.866640 |  3.961301 |    3 |
| line-strong / surface        |  4.223502 |  3.666978 |    3 |
| line-strong / surface-sunken |  3.600572 |  4.073032 |    3 |
| focus-ring / bg              |  5.397315 |  9.152731 |    3 |
| focus-ring / surface         |  5.895447 |  8.472687 |    3 |
| focus-ring / surface-sunken  |  5.025919 |  9.410890 |    3 |
| on-primary / primary         |  5.895447 |  9.410890 |  4.5 |
| primary / primary-soft       |  5.143363 |  6.736600 |  4.5 |
| on-up / up                   |  5.348307 |  6.511492 |  4.5 |
| up / up-soft                 |  4.685523 |  5.188654 |  4.5 |
| on-down / down               |  6.042495 |  7.164452 |  4.5 |
| down / down-soft             |  5.332624 |  5.600308 |  4.5 |
| on-alert / alert             |  5.417584 |  9.407126 |  4.5 |
| alert / alert-soft           |  4.784667 |  6.686582 |  4.5 |
| on-primary / primary-hover   |  7.752211 | 10.988868 |  4.5 |
| on-up / up-hover             |  6.928904 |  7.763271 |  4.5 |
| on-down / down-hover         |  7.618611 |  8.892067 |  4.5 |
| chart-1 / surface            |  7.744737 |  5.938279 |    3 |
| chart-2 / surface            |  3.850976 | 10.943505 |    3 |
| chart-3 / surface            |  7.834626 |  4.657394 |    3 |
| chart-4 / surface            |  3.652286 | 12.910817 |    3 |
| chart-5 / surface            |  9.398131 |  4.035790 |    3 |
| chart-6 / surface            |  3.847299 | 11.077851 |    3 |

### 생성기 소스 변형 검증

독립된 target 임시 패키지에 생성기와 새 테스트를 복사하고 cargo nextest로 각 변형을 실행했다. 원본은 모두 통과하고 아래 29개 변형은 컴파일 가능한 상태에서 테스트 실패로 검출했다. 빌드 실패를 검출 성공으로 세지 않았다. 임시 패키지는 제거했다.

| 무력화한 대응 규칙 | 판정               |
| ------------------ | ------------------ |
| reset              | 테스트 실패로 검출 |
| color              | 테스트 실패로 검출 |
| spacing            | 테스트 실패로 검출 |
| size               | 테스트 실패로 검출 |
| layout             | 테스트 실패로 검출 |
| radius             | 테스트 실패로 검출 |
| shadow             | 테스트 실패로 검출 |
| duration           | 테스트 실패로 검출 |
| easing             | 테스트 실패로 검출 |
| zIndex             | 테스트 실패로 검출 |
| opacity            | 테스트 실패로 검출 |
| text-prefix        | 테스트 실패로 검출 |
| leading            | 테스트 실패로 검출 |
| weight             | 테스트 실패로 검출 |
| tracking           | 테스트 실패로 검출 |
| breakpoint         | 테스트 실패로 검출 |
| container          | 테스트 실패로 검출 |
| grid               | 테스트 실패로 검출 |
| percentage         | 테스트 실패로 검출 |
| border             | 테스트 실패로 검출 |
| focus-offset       | 테스트 실패로 검출 |
| type-size          | 테스트 실패로 검출 |
| type-leading       | 테스트 실패로 검출 |
| type-weight        | 테스트 실패로 검출 |
| type-tracking      | 테스트 실패로 검출 |
| font-family        | 테스트 실패로 검출 |
| font-path          | 테스트 실패로 검출 |
| system-theme       | 테스트 실패로 검출 |
| manual-theme       | 테스트 실패로 검출 |

### 새 의존성과 등록

| 직접 의존성       | 버전  | 기능                                     | 라이선스 |
| ----------------- | ----- | ---------------------------------------- | -------- |
| tailwindcss       | 4.3.5 | 앱 CSS에서 가져오는 테마·유틸리티 컴파일 | MIT      |
| @tailwindcss/vite | 4.3.5 | 개발 의존성, Vite CSS 처리               | MIT      |

새 crate는 없다. 새 npm 전이 의존성은 @tailwindcss/node·oxide·플랫폼별 oxide 4.3.5, enhanced-resolve 5.26.2, graceful-fs 4.2.12, tapable 2.3.3이다. graceful-fs는 BlueOak-1.0.0, 나머지는 MIT다. 기존 잠금 파일에 있던 Vite·lightningcss 등의 버전은 바꾸지 않았다. ts-deps에는 두 직접 의존성만 추가한다. Vite 설정 위치는 기존 configFiles 허용 목록에 파일 하나만 추가한다. CSS 허용은 tokens.css·theme.css 두 경로만 추가하며 다른 CSS 금지는 유지한다. Knip에는 생성 타입 진입점과 실제 CSS 진입점을 추가한다. tokens.ts의 운영 소비자는 후속 DS-10이므로 생산 코드 검사에 사유 있는 항목 하나를 추가하며 첫 사용처가 생기면 stale 검사로 제거를 요구한다. 새 gen-tokens --check·contrast 검사를 just check와 check-list 기대값에 등록한다.

재생성은 보호 파일에 대한 수동 편집 허용이 아니다. 생성 CSS와 TS는 생성기 쓰기 모드만 쓴다. 글꼴 6개와 design/fonts/OFL.txt는 Vite가 번들에 포함한다. 시스템 테마와 수동 테마의 실제 WKWebView 검증은 이번 정적·컴파일 검증에 포함하지 않았다.
