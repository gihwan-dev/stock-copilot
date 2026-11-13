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

알파 전경은 불투명 배경 위에서 각 sRGB 채널을 aF+(1-a)B로 먼저 합성한 뒤 선형화한다. 알파 배경은 현재 필수 조합에 없으므로 거부한다. 쓰이지 않는 underlay 경로는 두지 않는다. 현재 필수 조합은 모두 불투명하다. 미달 조합은 예외 없이 실패하고 디자인 원본을 수정하지 않는다.

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

### 필수 대비 조합 실측

65개 조합·두 테마를 검사한다. 현재 128건은 기준을 충족하고 두 건은 사용자 확인 대기다. 수치는 표시용으로만 반올림한다. 조합별 문장 근거의 원본은 contrast-pairs.json의 basis다.

| 전경 / 배경                |       라이트 |         다크 | 최소 (라이트/다크) | 다크 상태              |
| -------------------------- | -----------: | -----------: | ------------------ | ---------------------- |
| ink/bg                     | 16.265308799 | 16.390612080 | 4.5/4.5            | 통과                   |
| ink/surface                | 17.766474761 | 15.172796765 | 4.5/4.5            | 통과                   |
| ink/surface-sunken         | 15.146073975 | 16.852920375 | 4.5/4.5            | 통과                   |
| ink-muted/bg               |  6.331258941 |  8.035709823 | 4.5/4.5            | 통과                   |
| ink-muted/surface          |  6.915586638 |  7.438660095 | 4.5/4.5            | 통과                   |
| ink-muted/surface-sunken   |  5.895597647 |  8.262362451 | 4.5/4.5            | 통과                   |
| ink-subtle/bg              |  5.099309566 |  6.048666139 | 4.5/4.5            | 통과                   |
| ink-subtle/surface         |  5.569937579 |  5.599252888 | 4.5/4.5            | 통과                   |
| ink-subtle/surface-sunken  |  4.748420142 |  6.219272856 | 4.5/4.5            | 통과                   |
| up/bg                      |  4.896405217 |  6.332869000 | 4.5/4.5            | 통과                   |
| up/surface                 |  5.348306681 |  5.862339601 | 4.5/4.5            | 통과                   |
| up/surface-sunken          |  4.559477877 |  6.511491852 | 4.5/4.5            | 통과                   |
| down/bg                    |  5.531938974 |  6.967917221 | 4.5/4.5            | 통과                   |
| down/surface               |  6.042495435 |  6.450204016 | 4.5/4.5            | 통과                   |
| down/surface-sunken        |  5.151279816 |  7.164452037 | 4.5/4.5            | 통과                   |
| line-strong/bg             |  3.866640345 |  3.961301024 | 3/3                | 통과                   |
| line-strong/surface        |  4.223502237 |  3.666978089 | 3/3                | 통과                   |
| line-strong/surface-sunken |  3.600572324 |  4.073032197 | 3/3                | 통과                   |
| focus-ring/bg              |  5.397314940 |  9.152730593 | 3/3                | 통과                   |
| focus-ring/surface         |  5.895446613 |  8.472686709 | 3/3                | 통과                   |
| focus-ring/surface-sunken  |  5.025919419 |  9.410889547 | 3/3                | 통과                   |
| on-primary/primary         |  5.895446613 |  9.410889547 | 4.5/4.5            | 통과                   |
| primary/primary-soft       |  5.143363039 |  6.736600068 | 4.5/4.5            | 통과                   |
| on-up/up                   |  5.348306681 |  6.511491852 | 4.5/4.5            | 통과                   |
| up/up-soft                 |  4.685522958 |  5.188654170 | 4.5/4.5            | 통과                   |
| on-down/down               |  6.042495435 |  7.164452037 | 4.5/4.5            | 통과                   |
| down/down-soft             |  5.332623668 |  5.600308203 | 4.5/4.5            | 통과                   |
| on-alert/alert             |  5.417584253 |  9.407125593 | 4.5/4.5            | 통과                   |
| alert/alert-soft           |  4.784666999 |  6.686581797 | 4.5/4.5            | 통과                   |
| on-primary/primary-hover   |  7.752210775 | 10.988868451 | 4.5/4.5            | 통과                   |
| on-up/up-hover             |  6.928903503 |  7.763270566 | 4.5/4.5            | 통과                   |
| on-down/down-hover         |  7.618610998 |  8.892066943 | 4.5/4.5            | 통과                   |
| chart-1/surface            |  7.744737085 |  5.938279310 | 3/3                | 통과                   |
| chart-2/surface            |  3.850976346 | 10.943505074 | 3/3                | 통과                   |
| chart-3/surface            |  7.834625958 |  4.657393795 | 3/3                | 통과                   |
| chart-4/surface            |  3.652286048 | 12.910817002 | 3/3                | 통과                   |
| chart-5/surface            |  9.398130896 |  4.035789788 | 3/3                | 통과                   |
| chart-6/surface            |  3.847299277 | 11.077851365 | 3/3                | 통과                   |
| ink/surface-raised         | 17.766474761 | 13.925688331 | 4.5/4.5            | 통과                   |
| ink-muted/surface-raised   |  6.915586638 |  6.827249035 | 4.5/4.5            | 통과                   |
| ink-subtle/surface-raised  |  5.569937579 |  5.139029528 | 4.5/4.5            | 통과                   |
| primary/bg                 |  5.397314940 |  9.152730593 | 4.5/4.5            | 통과                   |
| primary/surface            |  5.895446613 |  8.472686709 | 4.5/4.5            | 통과                   |
| primary/surface-sunken     |  5.025919419 |  9.410889547 | 4.5/4.5            | 통과                   |
| primary/surface-raised     |  5.895446613 |  7.776285168 | 4.5/4.5            | 통과                   |
| alert/bg                   |  4.959829229 |  9.149069891 | 4.5/4.5            | 통과                   |
| alert/surface              |  5.417584253 |  8.469297996 | 4.5/4.5            | 통과                   |
| alert/surface-sunken       |  4.618537609 |  9.407125593 | 4.5/4.5            | 통과                   |
| alert/surface-raised       |  5.417584253 |  7.773174986 | 4.5/4.5            | 통과                   |
| ink/primary-soft           | 15.500001206 | 12.063831371 | 4.5/4.5            | 통과                   |
| ink/alert-soft             | 15.690880199 | 11.979050295 | 4.5/4.5            | 통과                   |
| ink-muted/primary-soft     |  6.033363550 |  5.914449551 | 4.5/4.5            | 통과                   |
| ink-muted/alert-soft       |  6.107663051 |  5.872884530 | 4.5/4.5            | 통과                   |
| ink-subtle/primary-soft    |  4.859379272 |  4.451944074 | 4.5/4.5            | 미달, 사용자 확인 대기 |
| ink-subtle/alert-soft      |  4.919221423 |  4.420657114 | 4.5/4.5            | 미달, 사용자 확인 대기 |
| up/primary-soft            |  4.666021882 |  4.661123291 | 4.5/4.5            | 통과                   |
| down/primary-soft          |  5.271652806 |  5.128531988 | 4.5/4.5            | 통과                   |
| primary/alert-soft         |  5.206702386 |  6.689257215 | 4.5/4.5            | 통과                   |
| chart-1/chart-2            |  2.011110012 |  1.842874763 | 2/1.8              | 통과                   |
| chart-2/chart-3            |  2.034451852 |  2.349705770 | 2/1.8              | 통과                   |
| chart-3/chart-4            |  2.145129340 |  2.772111952 | 2/1.8              | 통과                   |
| chart-4/chart-5            |  2.573218738 |  3.199080646 | 2/1.8              | 통과                   |
| chart-5/chart-6            |  2.442786542 |  2.744902967 | 2/1.8              | 통과                   |
| focus-ring/surface-raised  |  5.895446613 |  7.776285168 | 3/3                | 통과                   |
| line-strong/surface-raised |  4.223502237 |  3.365575561 | 3/3                | 통과                   |

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

| 직접 의존성       | 버전  | 기능                              | 라이선스 |
| ----------------- | ----- | --------------------------------- | -------- |
| tailwindcss       | 4.3.5 | 개발 의존성, 테마·유틸리티 컴파일 | MIT      |
| @tailwindcss/vite | 4.3.5 | 개발 의존성, Vite CSS 처리        | MIT      |

새 crate는 없다. 새 npm 전이 의존성은 @tailwindcss/node·oxide·플랫폼별 oxide 4.3.5, enhanced-resolve 5.26.2, graceful-fs 4.2.12, tapable 2.3.3이다. graceful-fs는 BlueOak-1.0.0, 나머지는 MIT다. 기존 잠금 파일에 있던 Vite·lightningcss 등의 버전은 바꾸지 않았다. ts-deps에는 두 직접 의존성만 추가한다. Vite 설정 위치는 기존 configFiles 허용 목록에 파일 하나만 추가한다. CSS 허용은 tokens.css·theme.css 두 경로만 추가하며 다른 CSS 금지는 유지한다. Knip에는 실제 CSS 진입점을 등록하며 생성 타입 파일은 정확한 한 경로의 files·exports·types만 제외한다. 생성 타입 진입점과 임시 production 허용 항목은 두지 않는다. 새 gen-tokens --check·contrast 검사를 just check와 check-list 기대값에 등록한다.

재생성은 보호 파일에 대한 수동 편집 허용이 아니다. 생성 CSS와 TS는 생성기 쓰기 모드만 쓴다. 글꼴 6개와 design/fonts/OFL.txt는 Vite가 번들에 포함한다. 시스템 테마와 수동 테마의 실제 WKWebView 검증은 이번 정적·컴파일 검증에 포함하지 않았다.

## 적대적 리뷰 수정 결정

Tailwind 자동 탐지는 사용하지 않는다. theme.css는 `@import "tailwindcss" source(none)`와 `@source "../../"`로 src만 읽는다. tests와 eslint 폴더에 임의 클래스 문자열을 넣은 Vite 빌드 회귀에서 해당 클래스가 출력되지 않으며 src의 p-space-4는 생성된다. theme.css는 agent-writable 제외, rules.json의 정확한 설정 위치, 잠금 조각과 Codex 생성 설정으로 보호한다.

### 독립 값 검증과 픽스처 출처

utility-cases.json은 최초 입력 tokens.json과 대응 규칙에서 작성한 클래스 실측 픽스처이며 Rust 생성물에서 값을 추출한 파일은 아니다. 이 픽스처를 값 검증의 원본으로 삼지 않는다. Node가 입력 JSON에서 독립적으로 전체 CSS 변수 기대값을 유도하고, 생성된 :root·시스템 다크·수동 다크의 변수 집합과 값을 각각 비교한다. @font-face의 family·경로·format·weight·style·display도 입력과 대조한다. CLI로 새로 생성한 임시 출력에도 같은 독립 검증을 적용해 자기 자신과의 비교만으로 통과할 수 없게 했다.

### 대비 계약 수정

contrast-pairs.json 하나에 65개 조합·근거, 제외 색과 사유, 사용자 확인 대기 목록을 둔다. 코드 안의 중복 조합은 제거하고 고정된 조합 수와 중복·미포함 색 검사를 쓴다. 표에 없는 색 토큰은 실패한다. 제외는 장식 line과 딤 overlay 두 개뿐이다. 반투명 배경용 underlay 경로는 사용처가 없어 제거했다. 전경 알파 합성은 계속 검증한다.

본문·라벨·시각 및 출처의 색 의미와 Text tone을 선택된 행 및 Callout children에 조합하는 요구를 표에 명시했다. 특히 primary-soft의 종목 코드에는 원본 CSS가 ink-muted로 올리는 구체적 예외가 이미 있다. 이를 ink-subtle의 모든 사용처가 금지된다는 전역 규칙으로 확장하지 않았다. 일반 caption·출처 조합은 아래의 확인 대기에 둔다.

| 조합                    | 테마 |         실제 대비 | 요구 | 상태                   |
| ----------------------- | ---- | ----------------: | ---: | ---------------------- |
| ink-subtle/primary-soft | dark | 4.451944073961445 |  4.5 | 미달, 사용자 확인 대기 |
| ink-subtle/alert-soft   | dark | 4.420657113909118 |  4.5 | 미달, 사용자 확인 대기 |

확인 대기 항목은 조합·테마·실측값·요구값·근거가 고정된 두 항목뿐이다. 실제 값이 바뀌거나 기준을 넘게 되면 목록이 stale하므로 실패한다. 원본 토큰은 바꾸지 않았다.

### Knip 생성 타입과 의존성 구분

tokens.ts는 정확한 경로의 files·exports·types만 제외한다. 임시 미사용 파일 허용 항목과 entry 등록은 제거했다. 첫 소비자가 ColorToken 하나만 가져와도 남은 생성 타입의 미사용이나 stale 항목이 생기지 않는다. 다른 ds 파일과 다른 파일의 unused export 검사는 유지한다.

tailwindcss는 빌드 전용 devDependencies로 옮긴다. theme.css의 빌드 시 @import만 production Knip의 unlisted 검사에서 정확한 경로로 제외한다. theme.css의 내용은 정확한 import·source 목록으로 테스트하므로 임의 패키지를 이 경로로 숨기지 못한다. runtime 배포에는 컴파일된 CSS만 포함된다. 빠졌던 @napi-rs/wasm-runtime, @emnapi/core·runtime·wasi-threads, @tybys/wasm-util, tslib, @tailwindcss/oxide-wasm32-wasi 번들 전이 의존성도 잠금 파일에 포함돼 있다(원본 버전 유지). tslib는 0BSD이고 나머지는 MIT다. 기존 graceful-fs는 BlueOak-1.0.0이다.

### R-14 4.1과의 차이

- p-4 대신 p-space-4다. 원본 space-4 이름을 보존한다. 기본 숫자 간격을 다시 열지 않는다.
- 누락됐던 ChartSeriesToken·ThemeId를 생성한다. 기존 TypeStyleToken을 유지하면서 TextStyleToken 이름도 제공한다.
- data-text-scale=90/100/115/130의 font-size 규칙을 생성한다.
- type.groups[].family를 text-* 정적 유틸리티로 적용한다. text-code는 font-mono가 함께 적용된다.
- radius-pill은 원본 999px을 rem으로 환산한다. R-14 시제품의 9999px로 디자인 값을 바꾸지 않는다.

### 메인이 고칠 문서와 DS-02 인계

harness 4.4의 “기본 테마를 지워서 토큰 밖 클래스가 생성되지 않는다”는 문장은 색·간격·글자 크기의 기본값 제거에 한정해야 한다. 다음은 실제 Tailwind 4.3.5에서 여전히 생성된다. source 한정은 tests 유입만 막고 src 안의 이 문법은 막지 않는다. DS-02에서 금지·허용 범위를 구현한다. 특히 CSS 변수 축약 (--var)는 대괄호 정규식만으로 잡히지 않는다. dark:는 prefers-color-scheme 기준이므로 data-theme 수동 선택과 어긋나 금지한다.

| 후보              | 실제 생성 | DS-02 처리                                                                                              |
| ----------------- | --------- | ------------------------------------------------------------------------------------------------------- |
| `p-[13px]`        | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `[color:red]`     | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `p-(--x)`         | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `z-50`            | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `opacity-50`      | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `duration-150`    | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `grid-cols-7`     | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `border-2`        | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `outline-2`       | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `line-clamp-3`    | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `w-1/2`           | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `w-full`          | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `h-screen`        | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `rounded-full`    | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `bg-primary/50`   | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `dark:bg-primary` | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `p-sidebar-width` | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `w-space-4`       | 생성      | 임의 값·속성·변수 축약·숫자·분수·키워드·수식어·변형·namespace 혼용을 금지하거나 보호 허용 목록으로 한정 |
| `bg-red-500`      | 미생성    | 기본 테마 제거 유지                                                                                     |
| `p-5`             | 미생성    | 기본 테마 제거 유지                                                                                     |
| `text-sm`         | 미생성    | 기본 테마 제거 유지                                                                                     |

### 디자인 bundle.css의 var 사용처 전수 대조

원본은 수정하지 않았다. 기본 표에는 각 변수의 대표 유틸리티를 적었다. 같은 spacing 공간을 폭·높이·간격이 공유한다는 점은 DS-02의 props·클래스 제한으로 다룬다.

| 원본 변수             | 사용하는 속성                                                                                                                         | 생성 대응 / 인계                                                                                                       |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `--alert`             | color, border, border-color, background, border-bottom                                                                                | `bg-alert`                                                                                                             |
| `--alert-soft`        | background                                                                                                                            | `bg-alert-soft`                                                                                                        |
| `--bar-height`        | height                                                                                                                                | `h-bar-height`                                                                                                         |
| `--bg`                | background                                                                                                                            | `bg-bg`                                                                                                                |
| `--border-focus`      | outline, outline-offset, border-width, border, border-left, width                                                                     | `border-focus` 및 `outline-focus`(굵기·실선) 추가                                                                      |
| `--border-hairline`   | border, width, height, border-top, box-shadow, border-right, border-left, border-bottom, top, left, stroke-width, bottom, margin-left | `border-hairline`                                                                                                      |
| `--border-mark`       | border, stroke-width                                                                                                                  | `border-mark`는 테두리. stroke-width 전용 클래스는 DS-06 차트에서 필요 시 추가                                         |
| `--bubble-max-width`  | max-width                                                                                                                             | `max-w-bubble-max-width`                                                                                               |
| `--chart-1`           | background, stroke, fill                                                                                                              | `bg-chart-1`                                                                                                           |
| `--chart-2`           | background, stroke, fill                                                                                                              | `bg-chart-2`                                                                                                           |
| `--chart-3`           | background, stroke, fill                                                                                                              | `bg-chart-3`                                                                                                           |
| `--chart-4`           | background, stroke, fill                                                                                                              | `bg-chart-4`                                                                                                           |
| `--chart-5`           | background, stroke, fill                                                                                                              | `bg-chart-5`                                                                                                           |
| `--chart-6`           | background, stroke, fill                                                                                                              | `bg-chart-6`                                                                                                           |
| `--chat-input-max`    | max-height                                                                                                                            | `w-chat-input-max`                                                                                                     |
| `--chat-input-min`    | min-height                                                                                                                            | `w-chat-input-min`                                                                                                     |
| `--chat-width`        | width                                                                                                                                 | `w-chat-width`                                                                                                         |
| `--chat-width-max`    | max-width                                                                                                                             | `w-chat-width-max`                                                                                                     |
| `--chat-width-min`    | min-width, width                                                                                                                      | `w-chat-width-min`                                                                                                     |
| `--content-max`       | max-width                                                                                                                             | `w-content-max`                                                                                                        |
| `--content-min`       | min-width                                                                                                                             | `w-content-min`                                                                                                        |
| `--control-height`    | height, width, min-height                                                                                                             | `h-control-height`                                                                                                     |
| `--control-height-sm` | height, width                                                                                                                         | `h-control-height-sm`                                                                                                  |
| `--control-pad-sm`    | padding                                                                                                                               | `w-control-pad-sm`                                                                                                     |
| `--dialog-width`      | max-width                                                                                                                             | `w-dialog-width`                                                                                                       |
| `--dialog-width-wide` | max-width                                                                                                                             | `w-dialog-width-wide`                                                                                                  |
| `--dot-size`          | width, height                                                                                                                         | `w-dot-size`                                                                                                           |
| `--down`              | background, color, stroke, fill                                                                                                       | `bg-down`                                                                                                              |
| `--down-hover`        | background                                                                                                                            | `bg-down-hover`                                                                                                        |
| `--down-soft`         | background, fill                                                                                                                      | `bg-down-soft`                                                                                                         |
| `--duration-fast`     | transition                                                                                                                            | `duration-fast`                                                                                                        |
| `--duration-flash`    | animation                                                                                                                             | `duration-flash`는 전환 시간. sc-flash-up/down 키프레임·animation-duration는 DS-03 이후 시세 컴포넌트에서 필요 시 추가 |
| `--ease-out`          | transition, animation                                                                                                                 | `ease-out`                                                                                                             |
| `--focus-offset`      | outline-offset                                                                                                                        | `outline-offset-focus`                                                                                                 |
| `--focus-ring`        | outline                                                                                                                               | `bg-focus-ring`                                                                                                        |
| `--font-mono`         | font-family                                                                                                                           | `font-mono`                                                                                                            |
| `--font-sans`         | font-family                                                                                                                           | `font-sans`                                                                                                            |
| `--gap-hairline`      | stroke-dasharray, gap, min-width, padding, width, margin, letter-spacing, transform, text-underline-offset, stroke-width              | `w-gap-hairline`                                                                                                       |
| `--grid-columns`      | grid-template-columns                                                                                                                 | `grid-cols-layout`                                                                                                     |
| `--icon-lg`           | width, height, grid-template-columns                                                                                                  | `w-icon-lg`                                                                                                            |
| `--icon-md`           | width, height, transform                                                                                                              | `w-icon-md`                                                                                                            |
| `--icon-sm`           | width, height                                                                                                                         | `w-icon-sm`                                                                                                            |
| `--ink`               | color, background, fill                                                                                                               | `bg-ink`                                                                                                               |
| `--ink-muted`         | color, stroke, background, fill, border-left                                                                                          | `bg-ink-muted`                                                                                                         |
| `--ink-subtle`        | color, fill                                                                                                                           | `bg-ink-subtle`                                                                                                        |
| `--leading-3xl`       | line-height                                                                                                                           | `leading-3xl`                                                                                                          |
| `--leading-base`      | line-height                                                                                                                           | `leading-base`                                                                                                         |
| `--leading-lg`        | line-height                                                                                                                           | `leading-lg`                                                                                                           |
| `--leading-md`        | line-height, height                                                                                                                   | `leading-md`                                                                                                           |
| `--leading-sm`        | line-height                                                                                                                           | `leading-sm`                                                                                                           |
| `--leading-xl`        | line-height                                                                                                                           | `leading-xl`                                                                                                           |
| `--leading-xs`        | line-height, margin-bottom, top                                                                                                       | `leading-xs`                                                                                                           |
| `--line`              | border, border-top, box-shadow, border-right, border-left, border-bottom, border-bottom-color, stroke                                 | `bg-line`                                                                                                              |
| `--line-strong`       | border-color, stroke, border, border-left                                                                                             | `bg-line-strong`                                                                                                       |
| `--list-item-height`  | height                                                                                                                                | `h-list-item-height`                                                                                                   |
| `--meter-height`      | height                                                                                                                                | `h-meter-height`                                                                                                       |
| `--on-alert`          | color                                                                                                                                 | `bg-on-alert`                                                                                                          |
| `--on-down`           | color                                                                                                                                 | `bg-on-down`                                                                                                           |
| `--on-primary`        | color, background, border                                                                                                             | `bg-on-primary`                                                                                                        |
| `--on-up`             | color                                                                                                                                 | `bg-on-up`                                                                                                             |
| `--opacity-area`      | fill-opacity                                                                                                                          | `opacity-area`                                                                                                         |
| `--opacity-disabled`  | opacity                                                                                                                               | `opacity-disabled`                                                                                                     |
| `--overlay`           | background                                                                                                                            | `bg-overlay`                                                                                                           |
| `--popover-min-width` | grid-template-columns, min-width                                                                                                      | `w-popover-min-width`                                                                                                  |
| `--primary`           | background, color, border-color, border-top-color, border-right-color, border                                                         | `bg-primary`                                                                                                           |
| `--primary-hover`     | background                                                                                                                            | `bg-primary-hover`                                                                                                     |
| `--primary-soft`      | background                                                                                                                            | `bg-primary-soft`                                                                                                      |
| `--prose-max`         | max-width                                                                                                                             | `w-prose-max`                                                                                                          |
| `--radius-lg`         | border-radius                                                                                                                         | `rounded-lg`                                                                                                           |
| `--radius-md`         | border-radius                                                                                                                         | `rounded-md`                                                                                                           |
| `--radius-pill`       | border-radius                                                                                                                         | `rounded-pill`                                                                                                         |
| `--radius-sm`         | border-radius                                                                                                                         | `rounded-sm`                                                                                                           |
| `--row-height`        | height                                                                                                                                | `h-row-height`                                                                                                         |
| `--row-height-dense`  | height                                                                                                                                | `h-row-height-dense`                                                                                                   |
| `--shadow-popover`    | box-shadow                                                                                                                            | `shadow-popover`                                                                                                       |
| `--sidebar-collapsed` | width, left                                                                                                                           | `w-sidebar-collapsed`                                                                                                  |
| `--sidebar-width`     | width                                                                                                                                 | `w-sidebar-width`                                                                                                      |
| `--space-1`           | gap, stroke-dasharray, padding, margin-left, text-underline-offset, height, top, bottom, left, margin-top                             | `p-space-1`                                                                                                            |
| `--space-10`          | padding                                                                                                                               | `p-space-10`                                                                                                           |
| `--space-2`           | padding, gap, padding-top, margin, margin-top, right, width, top                                                                      | `p-space-2`                                                                                                            |
| `--space-3`           | gap, padding, margin, margin-top, padding-left, padding-bottom                                                                        | `p-space-3`                                                                                                            |
| `--space-4`           | padding, gap, margin                                                                                                                  | `p-space-4`                                                                                                            |
| `--space-5`           | padding, padding-left, gap, margin, margin-top, margin-bottom                                                                         | `p-space-5`                                                                                                            |
| `--space-6`           | right, bottom, padding, max-width, gap                                                                                                | `p-space-6`                                                                                                            |
| `--space-8`           | padding                                                                                                                               | `p-space-8`                                                                                                            |
| `--surface`           | background, color, stroke, fill                                                                                                       | `bg-surface`                                                                                                           |
| `--surface-raised`    | background                                                                                                                            | `bg-surface-raised`                                                                                                    |
| `--surface-sunken`    | background, fill                                                                                                                      | `bg-surface-sunken`                                                                                                    |
| `--text-2xl`          | font-size                                                                                                                             | `text-size-2xl`                                                                                                        |
| `--text-3xl`          | font-size                                                                                                                             | `text-size-3xl`                                                                                                        |
| `--text-base`         | font-size                                                                                                                             | `text-size-base`                                                                                                       |
| `--text-lg`           | font-size                                                                                                                             | `text-size-lg`                                                                                                         |
| `--text-md`           | font-size                                                                                                                             | `text-size-md`                                                                                                         |
| `--text-sm`           | font-size, height                                                                                                                     | `text-size-sm`                                                                                                         |
| `--text-xl`           | font-size                                                                                                                             | `text-size-xl`                                                                                                         |
| `--text-xs`           | font-size                                                                                                                             | `text-size-xs`                                                                                                         |
| `--toolbar-height`    | height, min-height                                                                                                                    | `h-toolbar-height`                                                                                                     |
| `--tooltip-max-width` | max-width                                                                                                                             | `w-tooltip-max-width`                                                                                                  |
| `--tracking-display`  | letter-spacing                                                                                                                        | `tracking-display`                                                                                                     |
| `--up`                | background, color, stroke, fill                                                                                                       | `bg-up`                                                                                                                |
| `--up-hover`          | background                                                                                                                            | `bg-up-hover`                                                                                                          |
| `--up-soft`           | background, fill                                                                                                                      | `bg-up-soft`                                                                                                           |
| `--weight-bold`       | font-weight                                                                                                                           | `font-bold`                                                                                                            |
| `--weight-medium`     | font-weight                                                                                                                           | `font-medium`                                                                                                          |
| `--weight-regular`    | font-weight                                                                                                                           | `font-regular`                                                                                                         |
| `--weight-semibold`   | font-weight                                                                                                                           | `font-semibold`                                                                                                        |
| `--window-min-height` | min-height                                                                                                                            | `h-window-min-height`                                                                                                  |
| `--window-min-width`  | min-width                                                                                                                             | `w-window-min-width`                                                                                                   |
| `--z-modal`           | z-index                                                                                                                               | `z-modal`                                                                                                              |
| `--z-panel`           | z-index                                                                                                                               | `z-panel`                                                                                                              |
| `--z-popover`         | z-index                                                                                                                               | `z-popover`                                                                                                            |
| `--z-sticky`          | z-index                                                                                                                               | `z-sticky`                                                                                                             |
| `--z-toast`           | z-index                                                                                                                               | `z-toast`                                                                                                              |

### 리뷰 재현 변형 검증

독립 입력 기대값을 CLI의 새 생성물에도 적용했다. 아래 소스 변형은 임시 패키지에서 빌드·실행을 성공한 뒤 독립 검증의 단언 실패로 검출했다.

| 변형                 | 결과                  |
| -------------------- | --------------------- |
| manual-dark-light    | 독립 검증 실패로 검출 |
| system-light-media   | 독립 검증 실패로 검출 |
| dark-shadow-omission | 독립 검증 실패로 검출 |
| font-weight          | 독립 검증 실패로 검출 |
| font-format          | 독립 검증 실패로 검출 |

테스트 폴더 클래스 문자열 주입은 Vite 빌드 회귀, surface-raised를 ink로 바꾸는 변형·새 색 토큰·확인 대기 stale 변형은 대비 회귀, 첫 타입 소비자와 다른 dead.ts·unused export는 Knip 회귀로 고정했다. 이름·값 주입과 미해결 참조·순환·모르는 그룹은 실제 CLI에서 출력 파일이 쓰이지 않는 것까지 검증한다.

| 번들 전이 의존성      | 실제 버전 | 패키지 라이선스 |
| --------------------- | --------- | --------------- |
| @napi-rs/wasm-runtime | 1.2.2     | MIT             |
| @emnapi/core          | 1.11.3    | MIT             |
| @emnapi/runtime       | 1.11.3    | MIT             |
| @emnapi/wasi-threads  | 1.2.3     | MIT             |
| @tybys/wasm-util      | 0.10.3    | MIT             |
| tslib                 | 2.8.1     | 0BSD            |
