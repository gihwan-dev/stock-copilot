# ADR 초안: DS-02 내부 스타일 검사

- 상태: 마지막 리뷰 수정
- 기준: f5296086e036d5415090d18be14e7f12f482aa04
- 범위: shared/ds 스타일, shared의 DOM·렌더링 경계, HTML·CSS 빌드 진입점. 실제 컴포넌트는 DS-03 이후다.

## 규칙 대응

| R-14 # | 강제 | 위반 | 통과 |
| --- | --- | --- | --- |
| 1 | no-unknown-classes, cwd·entryPoint 고정 | bg-red-500 | bg-primary |
| 2 | no-restricted-classes의 닫힌 집합 | p-(--x), p-[13px] | p-space-4 |
| 3 | 같은 집합 + 디자인 사용 속성으로 유도한 길이 클래스 | z-50 | z-modal, w-control-height |
| 4 | dark 변형 금지 | dark:bg-primary | hover:bg-primary |
| 5 | 보호된 가격색 파일 목록 | text-up | text-ink |
| 6 | 정적 클래스·객체 키·팩터리·DOM 타입 경계 | 변수 className, 객체 props, 함수형 ref | 리터럴 className, RefObject 전달 |
| 7 | 보호된 인라인 style·DOM 스타일 예외 | style prop, DOM 변경 | 토큰 className |
| 8 | 전체 CSS 값 리터럴, 표현 속성과 보호된 아이콘 기하 파일 | red, 13px, 일반 DS의 width={13} | lang=fr, 앵커 #add, 승인 아이콘 경로의 숫자 기하 |
| 15 | CSS 파일 경계, theme 정확 비교, DS-13 재생성 동일성 | 수기 CSS | 검증된 theme.css·tokens.css |

## DOM: 이름 목록에서 값 접근 경계로

shared/ds에서 guard/no-dom-globals·guard/no-ref-callback을 다시 켠다. shared/platform 전체 예외에서도 이 두 규칙을 빼고 DOM_ACCESS_FILES의 파일별 예외로 대체한다. 타입 인식 ds/no-dom-values는 shared 전체에 적용한다.

실제 TypeScript lib.dom 선언에 속한 전역 값·생성자를 금지한다. DOM 타입과 그 유니온·제네릭의 멤버 접근 및 DOM 객체 구조 분해를 금지한다. RefObject.current, 이벤트의 currentTarget·target·nativeEvent도 이 경계에 걸린다. 구조적 타입 좁히기로 DOM 타입 이름을 지운 ref·이벤트 우회도 회귀로 막는다. DOM 값의 식별자 읽기·다른 타입으로의 재전달도 금지하고 RefObject를 JSX ref로 전달하는 자리만 구별한다. 계산 키와 구조 분해도 같은 진입 경계로 검사한다. current 읽기는 금지하고, target 계열은 원시 데이터 값인 경우만 허용해 업무 데이터의 target 문자열과 구분한다. 함수형 ref는 인라인·로컬 식별자·가져온 함수 모두 금지하고, 식별자로 전달하는 비호출 RefObject만 허용한다. 공개 ref props 계약은 DS-10에서도 확인한다.

따라서 setHTMLUnsafe, Range/createContextualFragment, DOMParser, attachShadow, createElementNS, CSSStyleSheet 삭제·교체, Animation/KeyframeEffect, removeAttribute, part.add를 하나씩 이름 목록에 더하지 않는다. 기존 no-dom-styling의 클래스·스타일·HTML 멤버 목록은 2차 방어로 유지한다. window·globalThis 같은 기존 전역 금지도 유지한다.

새 DS DOM 접근 예외는 비어 있다. 현재 DOM_ACCESS_FILES의 유일한 파일은 src/shared/platform/index.ts다. 이 파일의 rootElement는 document.getElementById로 부팅 루트를 찾는 기존 어댑터다. architecture 5장 및 ADR 0009에서 app의 DOM 전역 접근을 이 파일로 옮긴 결정에 해당하므로 존치한다. 그 밖의 기존 shared 파일에는 새 DOM 접근 예외가 필요하지 않았다.

포커스·측정의 새 요구는 보호된 파일 목록 변경을 승인받아야 한다. 폴더 예외를 만들거나 새 파일이 자동으로 권한을 상속하게 하지 않는다. 동적 미터 폭은 ref로 스타일을 넣는 예외를 열지 않고 DS-03·DS-06에서 닫힌 단계 값이나 전용 컴포넌트 계약으로 결정한다.

## 요소와 렌더링 경계

shared/ds 밖 shared에서는 JSX와 요소 팩터리를 금지한다. DS에서도 createElement·cloneElement·jsx 직접 참조, 계산 키와 fromEntries로 만든 스타일 props, JSX spread를 막는다. spread로 className·style이 숨어 들어오므로 DS의 이전 shared 예외를 좁힌 결정이다. 비화면 shared의 객체 spread는 계속 허용한다.

SMIL animate·animateTransform·animateMotion·set, style·link, font·center·b·i·u, iframe·object·embed·foreignObject를 금지한다. design/components의 실제 컴포넌트에서 이 렌더링 요소는 쓰이지 않는다. Cover preview의 style은 보호된 디자인 문서이며 앱 코드가 아니다. em·strong·s는 의미 요소로 구별하고 금지하지 않는다. dangerouslySetInnerHTML도 금지한다.

features·app의 화면 코드 금지는 DS-09다. 이 두 폴더에 DS 전용 규칙이 아직 적용되지 않았다는 범위는 유지한다.

## 아이콘: 스타일과 기하

Icons README의 Lucide 24×24 격자·2px 선·currentColor와 실제 SVG 규약을 기준으로 한다.

| 구분 | 정책 |
| --- | --- |
| 색·선 스타일 | 닫힌 fill-none, stroke-current, stroke-icon, stroke-linecap-round, stroke-linejoin-round 클래스 |
| 선 굵기 | stroke-icon은 생성 토큰 --border-width-focus를 참조한다. 현재 값 2px이며 두 테마에서 Lucide의 2px와 일치하는지 별도 검사한다. 일치하지 않으면 DS-03에서 전용 토큰을 결정해야 한다. 숫자 stroke-2는 열지 않는다 |
| 경로 | d·points·viewBox 문자열은 보호된 아이콘 경로 파일에서만 |
| 위치·반경 | x/y/cx/cy/r/rx/ry, x1/y1/x2/y2는 같은 파일에서 숫자 리터럴만 |
| 도형 크기 | rect/circle/ellipse/line의 width·height는 같은 파일에서 숫자 리터럴만 |
| 일반 DS 파일 | 기하 속성을 포함해 표현 속성 금지. 토큰 클래스만 사용 |

ICON_GEOMETRY_FILES는 현재 비어 있다. 통과 예제는 임시 프로젝트에 경로 파일을 지정해 아이콘 하나를 그리고 실제 ESLint와 tsc를 통과한다. 앱의 아이콘 컴포넌트와 실제 승인 파일 추가는 DS-03이다. theme.css에는 토큰 선 굵기와 round 선 스타일의 정적 utility 및 디자인에서 사용한 특수 길이 조합만 둔다. 정확한 내용은 themeSource와 기존 진입점 테스트로 고정한다.

## 디자인으로 유도하는 축

bundle.css를 PostCSS로 읽어 모든 var(--token) 사용의 속성·선택자를 수집한다. 길이 속성에서 클래스의 축을 유도하고, 같은 선택자가 같은 토큰으로 width와 height를 지정하면 size를 허용한다. 이름이나 한국어 usage의 추측으로 허용 축을 정하지 않는다.

따라서 w-control-height·size-control-height·min-w-gap-hairline을 포함해 실제 디자인이 쓰는 길이 조합을 허용한다. padding·margin 같은 축약 속성은 해당 계열의 축 클래스들로 펼친다. border 길이처럼 Tailwind spacing namespace에 없는 값은 생성 토큰 변수를 참조하는 닫힌 정적 utility로 만든다. 각 유도 클래스가 허용 목록에 있으며 실제 Tailwind에서 생성되는지 검사한다. 아래 표는 같은 추출기로 생성한 전체 var 사용 표다. 글자·색·그림자 및 복합 transform 등은 길이 축 유도 대상과 구분한다.

min-w-0·min-h-0의 생성 정책은 DS-11·DS-19에 남긴다. DS-19가 파일별 길이·배치·넘침 제한을 추가하기 전까지 이 집합은 토큰 의미와 실제 생성 여부를 검증하는 범위다.

## 값 검사와 오탐 처리

값을 trim한 뒤 전체 CSS 색·길이·각도·시간·해상도 값 또는 공백 구분 값 목록과 일치시킨다. hex는 정확히 3·4·6·8자리 전체 값만이다. CSS 함수도 값 전체와 맞춘다. 템플릿은 raw 대신 cooked 값을 사용하고, 리터럴 + 및 String(리터럴)의 단위 조합도 검사한다.

fr·span·em·s·PT1S는 CSS 숫자+단위 값이 아니므로 허용한다. href/xlinkHref의 #add는 앵커 문맥이므로 허용한다. 문맥 없는 #add는 실제 3자리 hex와 구별할 수 없어 색으로 본다. 앞뒤 공백의 red 및 이스케이프된 red/13px는 금지한다. Intl.NumberFormat의 직접 옵션 객체는 style 키 금지에서 구별한다. 실제 형식 변환은 shared/lib/format으로 유도한다.

concat·fromCharCode·atob 등 임의 계산의 결과는 분석하지 않는다. 그렇더라도 className 동적 전달과 DOM 값 접근은 별도 경계가 막는다. 외부 차트 컴포넌트의 숫자·간접 데이터 props는 DS-06·DS-10의 보호된 계약에서 정한다.

## HTML·CSS·빌드 진입점

index.html의 doctype·요소·속성·유일한 theme 링크·module script를 정확한 문자열로 고정한다. style·외부 stylesheet 링크·속성이 추가되면 실패한다. index.html은 agent-writable 밖이고 생성된 Codex 권한에서도 읽기 전용인 기존 보호 경로다.

Vite default export는 defineConfig의 정적 객체여야 하고 CSS/postcss 키·계산 키·객체 spread로 CSS 설정을 주입할 수 없다. 기존 Tailwind Vite 플러그인과 글꼴 라이선스 번들은 유지한다.

수기 CSS는 파일 자체를 금지한다. theme는 검증된 utility 정의를 포함한 정확 비교, tokens.css는 DS-13 gen-tokens --check다. stylelint 의존성은 다시 들이지 않는다. PostCSS 8.5.32와 better-tailwindcss 4.11.0을 유지한다.

스냅샷은 디자인 유도 클래스·닫힌 키워드·변형·DS-13 반례를 입력으로 실제 Tailwind를 컴파일한다. 중첩된 모든 CSS 규칙·at-rule 문맥 및 후보별 생성 여부를 고정한다. 허용 후보의 미생성은 스냅샷 일치와 별개로 실패한다. DS-13 utility-cases.json이나 스냅샷 출력을 허용 목록으로 역참조하지 않는다.

## 검증·기존 테스트 변경

리뷰의 DOM API 반례와 ref/이벤트 경로는 실제 앱 ESLint 설정을 읽는 임시 프로젝트에서 tsc를 통과한 뒤 새 DOM 규칙으로 실패한다. RefObject 전달·아이콘 경로·언어/태그 유니온·앵커·Intl 옵션은 같은 설정에서 통과한다. 각 규칙의 격리 비활성화 검사는 유지한다.

기존 eslint-config 테스트의 DS DOM 허용 기대를 금지 기대로 강화한다. ds-generation의 정확한 theme 기대값에는 검증된 utility 정의를 추가한다. 기존 예제의 추가 오류를 기대 목록에 더한다. 기존 테스트의 삭제·약화나 검사 끄기는 없다.

## 전체 디자인 var 사용과 유도 클래스

| 토큰 | 실제 CSS 속성 | 유도한 길이 클래스 |
| --- | --- | --- |
| alert | background, border, border-bottom, border-color, color | 길이 축 아님 |
| alert-soft | background | 길이 축 아님 |
| bar-height | height | h-bar-height |
| bg | background | 길이 축 아님 |
| border-focus | border, border-left, border-width, outline, outline-offset, width | w-border-focus |
| border-hairline | border, border-bottom, border-left, border-right, border-top, bottom, box-shadow, height, left, margin-left, stroke-width, top, width | bottom-border-hairline, h-border-hairline, left-border-hairline, ml-border-hairline, size-border-hairline, top-border-hairline, w-border-hairline |
| border-mark | border, stroke-width | 길이 축 아님 |
| bubble-max-width | max-width | max-w-bubble-max-width |
| chart-1 | background, fill, stroke | 길이 축 아님 |
| chart-2 | background, fill, stroke | 길이 축 아님 |
| chart-3 | background, fill, stroke | 길이 축 아님 |
| chart-4 | background, fill, stroke | 길이 축 아님 |
| chart-5 | background, fill, stroke | 길이 축 아님 |
| chart-6 | background, fill, stroke | 길이 축 아님 |
| chat-input-max | max-height | max-h-chat-input-max |
| chat-input-min | min-height | min-h-chat-input-min |
| chat-width | width | w-chat-width |
| chat-width-max | max-width | max-w-chat-width-max |
| chat-width-min | min-width, width | min-w-chat-width-min, w-chat-width-min |
| content-max | max-width | max-w-content-max |
| content-min | min-width | min-w-content-min |
| control-height | height, min-height, width | h-control-height, min-h-control-height, size-control-height, w-control-height |
| control-height-sm | height, width | h-control-height-sm, size-control-height-sm, w-control-height-sm |
| control-pad-sm | padding | p-control-pad-sm, pb-control-pad-sm, pe-control-pad-sm, pl-control-pad-sm, pr-control-pad-sm, ps-control-pad-sm, pt-control-pad-sm, px-control-pad-sm, py-control-pad-sm |
| dialog-width | max-width | max-w-dialog-width |
| dialog-width-wide | max-width | max-w-dialog-width-wide |
| dot-size | height, width | h-dot-size, size-dot-size, w-dot-size |
| down | background, color, fill, stroke | 길이 축 아님 |
| down-hover | background | 길이 축 아님 |
| down-soft | background, fill | 길이 축 아님 |
| duration-fast | transition | 길이 축 아님 |
| duration-flash | animation | 길이 축 아님 |
| ease-out | animation, transition | 길이 축 아님 |
| focus-offset | outline-offset | 길이 축 아님 |
| focus-ring | outline | 길이 축 아님 |
| font-mono | font-family | 길이 축 아님 |
| font-sans | font-family | 길이 축 아님 |
| gap-hairline | gap, letter-spacing, margin, min-width, padding, stroke-dasharray, stroke-width, text-underline-offset, transform, width | gap-gap-hairline, gap-x-gap-hairline, gap-y-gap-hairline, m-gap-hairline, mb-gap-hairline, me-gap-hairline, min-w-gap-hairline, ml-gap-hairline, mr-gap-hairline, ms-gap-hairline, mt-gap-hairline, mx-gap-hairline, my-gap-hairline, p-gap-hairline, pb-gap-hairline, pe-gap-hairline, pl-gap-hairline, pr-gap-hairline, ps-gap-hairline, pt-gap-hairline, px-gap-hairline, py-gap-hairline, w-gap-hairline |
| grid-columns | grid-template-columns | 길이 축 아님 |
| icon-lg | grid-template-columns, height, width | h-icon-lg, size-icon-lg, w-icon-lg |
| icon-md | height, transform, width | h-icon-md, size-icon-md, w-icon-md |
| icon-sm | height, width | h-icon-sm, size-icon-sm, w-icon-sm |
| ink | background, color, fill | 길이 축 아님 |
| ink-muted | background, border-left, color, fill, stroke | 길이 축 아님 |
| ink-subtle | color, fill | 길이 축 아님 |
| leading-3xl | line-height | 길이 축 아님 |
| leading-base | line-height | 길이 축 아님 |
| leading-lg | line-height | 길이 축 아님 |
| leading-md | height, line-height | 길이 축 아님 |
| leading-sm | line-height | 길이 축 아님 |
| leading-xl | line-height | 길이 축 아님 |
| leading-xs | line-height, margin-bottom, top | 길이 축 아님 |
| line | border, border-bottom, border-bottom-color, border-left, border-right, border-top, box-shadow, stroke | 길이 축 아님 |
| line-strong | border, border-color, border-left, stroke | 길이 축 아님 |
| list-item-height | height | h-list-item-height |
| meter-height | height | h-meter-height |
| on-alert | color | 길이 축 아님 |
| on-down | color | 길이 축 아님 |
| on-primary | background, border, color | 길이 축 아님 |
| on-up | color | 길이 축 아님 |
| opacity-area | fill-opacity | 길이 축 아님 |
| opacity-disabled | opacity | 길이 축 아님 |
| overlay | background | 길이 축 아님 |
| popover-min-width | grid-template-columns, min-width | min-w-popover-min-width |
| primary | background, border, border-color, border-right-color, border-top-color, color | 길이 축 아님 |
| primary-hover | background | 길이 축 아님 |
| primary-soft | background | 길이 축 아님 |
| prose-max | max-width | max-w-prose-max |
| radius-lg | border-radius | 길이 축 아님 |
| radius-md | border-radius | 길이 축 아님 |
| radius-pill | border-radius | 길이 축 아님 |
| radius-sm | border-radius | 길이 축 아님 |
| row-height | height | h-row-height |
| row-height-dense | height | h-row-height-dense |
| shadow-popover | box-shadow | 길이 축 아님 |
| sidebar-collapsed | left, width | left-sidebar-collapsed, w-sidebar-collapsed |
| sidebar-width | width | w-sidebar-width |
| space-1 | bottom, gap, height, left, margin-left, margin-top, padding, stroke-dasharray, text-underline-offset, top | bottom-space-1, gap-space-1, gap-x-space-1, gap-y-space-1, h-space-1, left-space-1, ml-space-1, mt-space-1, p-space-1, pb-space-1, pe-space-1, pl-space-1, pr-space-1, ps-space-1, pt-space-1, px-space-1, py-space-1, top-space-1 |
| space-10 | padding | p-space-10, pb-space-10, pe-space-10, pl-space-10, pr-space-10, ps-space-10, pt-space-10, px-space-10, py-space-10 |
| space-2 | gap, margin, margin-top, padding, padding-top, right, top, width | gap-space-2, gap-x-space-2, gap-y-space-2, m-space-2, mb-space-2, me-space-2, ml-space-2, mr-space-2, ms-space-2, mt-space-2, mx-space-2, my-space-2, p-space-2, pb-space-2, pe-space-2, pl-space-2, pr-space-2, ps-space-2, pt-space-2, px-space-2, py-space-2, right-space-2, top-space-2, w-space-2 |
| space-3 | gap, margin, margin-top, padding, padding-bottom, padding-left | gap-space-3, gap-x-space-3, gap-y-space-3, m-space-3, mb-space-3, me-space-3, ml-space-3, mr-space-3, ms-space-3, mt-space-3, mx-space-3, my-space-3, p-space-3, pb-space-3, pe-space-3, pl-space-3, pr-space-3, ps-space-3, pt-space-3, px-space-3, py-space-3 |
| space-4 | gap, margin, padding | gap-space-4, gap-x-space-4, gap-y-space-4, m-space-4, mb-space-4, me-space-4, ml-space-4, mr-space-4, ms-space-4, mt-space-4, mx-space-4, my-space-4, p-space-4, pb-space-4, pe-space-4, pl-space-4, pr-space-4, ps-space-4, pt-space-4, px-space-4, py-space-4 |
| space-5 | gap, margin, margin-bottom, margin-top, padding, padding-left | gap-space-5, gap-x-space-5, gap-y-space-5, m-space-5, mb-space-5, me-space-5, ml-space-5, mr-space-5, ms-space-5, mt-space-5, mx-space-5, my-space-5, p-space-5, pb-space-5, pe-space-5, pl-space-5, pr-space-5, ps-space-5, pt-space-5, px-space-5, py-space-5 |
| space-6 | bottom, gap, max-width, padding, right | bottom-space-6, gap-space-6, gap-x-space-6, gap-y-space-6, max-w-space-6, p-space-6, pb-space-6, pe-space-6, pl-space-6, pr-space-6, ps-space-6, pt-space-6, px-space-6, py-space-6, right-space-6 |
| space-8 | padding | p-space-8, pb-space-8, pe-space-8, pl-space-8, pr-space-8, ps-space-8, pt-space-8, px-space-8, py-space-8 |
| surface | background, color, fill, stroke | 길이 축 아님 |
| surface-raised | background | 길이 축 아님 |
| surface-sunken | background, fill | 길이 축 아님 |
| text-2xl | font-size | 길이 축 아님 |
| text-3xl | font-size | 길이 축 아님 |
| text-base | font-size | 길이 축 아님 |
| text-lg | font-size | 길이 축 아님 |
| text-md | font-size | 길이 축 아님 |
| text-sm | font-size, height | 길이 축 아님 |
| text-xl | font-size | 길이 축 아님 |
| text-xs | font-size | 길이 축 아님 |
| toolbar-height | height, min-height | h-toolbar-height, min-h-toolbar-height |
| tooltip-max-width | max-width | max-w-tooltip-max-width |
| tracking-display | letter-spacing | 길이 축 아님 |
| up | background, color, fill, stroke | 길이 축 아님 |
| up-hover | background | 길이 축 아님 |
| up-soft | background, fill | 길이 축 아님 |
| weight-bold | font-weight | 길이 축 아님 |
| weight-medium | font-weight | 길이 축 아님 |
| weight-regular | font-weight | 길이 축 아님 |
| weight-semibold | font-weight | 길이 축 아님 |
| window-min-height | min-height | min-h-window-min-height |
| window-min-width | min-width | min-w-window-min-width |
| z-modal | z-index | 길이 축 아님 |
| z-panel | z-index | 길이 축 아님 |
| z-popover | z-index | 길이 축 아님 |
| z-sticky | z-index | 길이 축 아님 |
| z-toast | z-index | 길이 축 아님 |
