# ADR 초안: DS-02 내부 스타일 검사

- 상태: 적대적 리뷰 수정
- 기준: b9fc68fe37d8b6496e5f1a0987e6bd790a1e6654
- 범위: shared/ds 내부, 비화면 shared 폴더의 렌더링 경계. 컴포넌트는 만들지 않는다.

## 규칙 대응

| R-14 # | 금지 | 강제 | 위반 | 통과 |
| --- | --- | --- | --- | --- |
| 1 | 생성 테마 밖 클래스 | no-unknown-classes, cwd·entryPoint 고정 | bg-red-500 | bg-primary |
| 2 | 임의 값·속성·변형, 변수 축약, 분수·불투명도 | no-restricted-classes의 닫힌 집합 | p-(--x), p-[13px] | p-space-4 |
| 3 | 숫자 직접 입력, 토큰 의미 혼용 | 같은 집합 | z-50, w-row-height | z-modal, h-row-height |
| 4 | dark 변형 | no-restricted-classes | dark:bg-primary | hover:bg-primary |
| 5 | 허용 파일 밖 가격색 | PRICE_COLOR_FILES, 파일별 no-restricted-classes | text-up | text-ink |
| 6 | 동적 className, 객체 속성·팩터리·DOM 우회 | static-classes, no-concatenated-classes, no-style-object, no-element-factory, no-dom-styling | 변수, 템플릿, JSX spread, createElement, classList | 문자열 className |
| 7 | 허용 파일 밖 inline style, DOM 스타일 변경 | INLINE_STYLE_FILES, DOM_STYLE_FILES, no-inline-style, no-dom-styling | style prop, el.style | 토큰 className |
| 8 | 색·단위 리터럴, 소문자 요소의 표현 속성 | no-style-literals, no-presentation-props | red, 1rem, width={13}, stroke | 토큰 className, d, viewBox |
| 15 | 수기 CSS 파일 | 기존 local/forbidden-file + CSS 파일 경계 검사. 생성물은 gen-tokens --check | custom.css의 모든 내용 | 고정 theme.css, 재생성 동일성 검사를 통과한 tokens.css |

모든 새 규칙은 끄기 불가 목록에 있다. 가격색·style·DOM 스타일 예외는 ds-allowlists.mjs의 보호된 파일 목록이며 처음에는 모두 비어 있다. 목록에 등록한 파일만 예외가 되고 다른 파일은 계속 실패하는 테스트를 둔다.

## DOM과 렌더링 경계

shared/ds에서 classList, className, style, setAttribute, setAttributeNS, toggleAttribute, setProperty, cssText, animate, insertAdjacentHTML, innerHTML, outerHTML, adoptedStyleSheets, insertRule, CSSStyleSheet, attributeStyleMap 멤버 접근을 막는다. 리터럴 계산 키와 구조 분해도 본다. 동적 계산 멤버는 기존 no-computed-member에 더해 DS 규칙도 막는다. style·link JSX 태그 및 dangerouslySetInnerHTML은 의도한 DS 규칙으로 금지한다.

createElement·cloneElement·jsx·jsxs·jsxDEV 참조는 별칭 import와 객체 멤버도 금지한다. 객체의 className·class·style 키는 값이 리터럴이어도 금지한다. 계산 키와 Object.fromEntries도 금지한다. JSX spread는 숨은 className·style 전달 경로여서 DS 안에서는 이전 shared 폴더 예외를 좁힌다. 기존 spread-allowed.tsx가 DS에서 오류를 기대하도록 바꾼 것은 이 결정의 결과다. DS 밖 shared에서는 spread 규칙 자체는 여전히 꺼져 있지만 JSX 렌더링이 별도 규칙으로 금지된다. 비화면 shared의 객체 spread 통과 대조군과, JSX spread가 오직 새 JSX 경계 규칙으로 실패하는 대조군을 둔다.

architecture 5장의 역할에 따라 shared/lib·shared/ipc·shared/platform에서는 JSX와 요소 팩터리를 금지한다. platform의 DOM 구독·Canvas 어댑터는 JSX를 렌더하지 않는다. features·app의 화면 코드 금지는 DS-09다. 그 두 폴더에는 이번 DS 규칙을 적용하지 않으며, DS-09 전에는 DS 전용 금지에 대해 지금은 무방비다.

동적 미터 폭을 ref로 넣는 예외는 열지 않는다. 닫힌 단계 유틸리티 또는 코어 좌표를 받는 전용 미터·차트 컴포넌트의 필요와 정책은 DS-03·DS-06에서 정한다. 측정용 파일은 DOM_STYLE_FILES에 사용자 승인으로 추가한다.

## 리터럴과 표현 속성

CSS 이름 색 148개와 transparent·currentColor를 모두 예약한다. 목록은 기존 설치 패키지 color-name 1.1.4(MIT)에 있는 표준 이름 집합으로 확인했다. currentColor의 일반 문자열 예외는 열지 않는다. red라는 문자열을 DS 안의 다른 의미로 재사용하는 것도 실패한다. UI 문구는 UiText/messages 계층의 책임이며 DS에 색 이름을 상태값이나 문구로 겸용하지 않는다. bg-transparent 클래스는 design/components/bundle.css의 ghost 버튼·IconButton·Callout 투명 배경과 일치하므로 닫힌 키워드 예외로 유지한다.

CSS 색 함수(rgb·hsl·hwb·lab·lch·oklab·oklch·color·color-mix·light-dark·device-cmyk·contrast-color), CSS 길이·각도·시간·해상도 단위와 calc·min·max·clamp를 검사한다. 단위만 있는 문자열과 템플릿 조각도 검사해 "1"+"3"+"px", String(13)+"px", `${n}px`를 막는다.

소문자 HTML·SVG 요소에서는 보호된 PRESENTATION_PROPS의 표현 속성을 값과 무관하게 금지한다. camelCase와 하이픈 표기를 동일하게 본다. fill·stroke·strokeWidth·width·height·rx·ry·r·opacity·색·정렬·글자 모양·transform 등은 클래스만 쓴다. d·viewBox는 경로·좌표계라 허용한다. 대문자 컴포넌트의 size={13} 같은 숫자 props는 지금 이 규칙이 막지 않는다. 공개 props는 DS-10, Recharts stroke·strokeWidth와 좌표 등은 DS-06의 보호된 허용 파일 목록에서 정한다. 간접 데이터의 의미를 DS-02가 추론한다는 주장은 하지 않는다.

## 클래스 의미와 후속 범위

크기 토큰의 이름·usage가 폭이면 w/min-w/max-w, 높이면 h/min-h/max-h만 만든다. 아이콘·점은 정사각형이므로 w/h/size를 허용한다. control-pad-sm은 수평 padding, gap-hairline은 padding/gap에만 쓴다. 축이 분류되지 않는 새 크기 토큰은 오류로 드러낸다. w-row-height·h-sidebar-width·w-space-4·p-sidebar-width는 금지다.

min-w-0·min-h-0은 reset 뒤 생성되지 않는다. 허용 목록에서는 제거했다. harness 4.4 #26의 자식 래퍼에 필요한 0 최소 크기는 DS-11·DS-19가 생성 정책(@utility 또는 토큰)을 결정해야 한다. 생성기·theme.css를 여기서 바꾸지 않는다.

길이 토큰 w/h는 DS-02에서 의미 검증까지만 한다. DS-19가 #21의 파일별 길이 제한을 추가해야 하며 그 전에는 전역 허용이다. absolute·overflow는 DS-02에서 필요가 없어 먼저 닫은 상태다. 현재 클래스 집합이 DS-19의 파일별 제한까지 구현했다고 해석하지 않는다.

className은 문자열 리터럴 또는 정적인 cn 인자만 받는다. 현재 승인된 cn 구현이 없어 로컬 선언·매개변수·import로 바인딩된 cn 호출은 거부한다. 예제의 미해결 cn은 문법 대조군이며 실제 앱에서는 타입 검사도 통과해야 한다.

## 스냅샷과 CSS 검사 축소

입력은 코드용 토큰의 의미별 조합, 닫힌 키워드·변형, DS-13 인계 반례다. DS-13 utility-cases.json은 대표 대응 검사이며 이 목록을 복사하지 않는다. 스냅샷 출력을 허용 목록으로 역참조하지 않는다.

실제 Tailwind compile의 결과를 PostCSS로 파싱해 중첩된 모든 규칙 선택자와 부모 선택자·at-rule 문맥을 수집한다. Tailwind 자체 candidatesToCss 결과로 각 후보의 생성됨/안 됨도 고정한다. 허용 후보가 생성되지 않으면 스냅샷 일치와 별개로 실패한다. 반례 후보는 안 만들어져도 그 상태를 기록한다. hover·motion-reduce·window-md·@content-medium·space-x의 생성, print·dark·group-hover 추가와 min-h-0·p-0·animate-spin 추가가 스냅샷 차이로 드러나는 테스트가 있다. 최종 입력은 후보 8,202개, CSS 규칙 8,208개다. 재생성은 check.mjs의 snapshot을 실행해 utilities.json에 기록한다.

수기 CSS는 local/forbidden-file이 이미 파일 자체를 거부한다. theme.css는 정확한 진입 문법 검사, tokens.css는 DS-13 gen-tokens --check의 생성물 동일성 검사 대상이다. 이 둘에 일반 스타일 규칙을 억지로 적용하지 않는다. 이전의 cssProblems와 “임의 함수를 막는다”는 주장은 폐기했다. rgb·rotate·theme·attr·image-set·important·Tailwind 지시문 등의 수기 CSS는 내용과 무관하게 파일 경계에서 거부한다. tokens.css 자체가 바뀌는 경우는 기존 DS-13 재생성 비교가 검증한다.

stylelint 18.0.0과 stylelint-declaration-strict-value 1.13.0을 제거했다. PostCSS 8.5.32(MIT)는 기존 전이 버전을 정확히 고정해 직접 개발 의존성으로 올렸다. 새 버전의 전이 패키지는 추가하지 않았다. better-tailwindcss 4.11.0은 유지한다. 버전·cwd·entryPoint의 실제 호환성 검사는 유지한다. 없는 진입점과 reset 없는 CSS에서는 bg-red-500이 통과하지만 tokens.css 단독은 검출한다는 실측도 유지한다.

## 검증과 알려진 경계

기존 클래스 예제에 리뷰 반례를 더한 파일+기대 결과 예제는 110개다. 새 경계 규칙별 격리 설정에서 켜면 검출, 끄면 통과를 확인한다. 실제 앱 ESLint 설정을 읽는 임시 프로젝트에서는 원본 tsconfig를 상속하고 TS 7 tsc와 TS 6 API 모두로 리뷰 반례를 타입 검사한다. 같은 코드가 의도한 새 규칙으로 실패하고, 토큰 클래스와 d/viewBox 대조군은 두 검사 모두 통과한다. shared/lib Box를 DS에서 가져오는 우회도 실제 설정에서 실패한다.

기존 master 테스트의 단언 삭제·약화는 하지 않는다. eslint-config.test.mjs의 끌 수 없는 규칙 목록에는 새 이름만 추가한다. 기존 concat-key-gap 예제의 정확한 보고 목록에는 새 DOM 규칙 오류를 추가한다. DS의 spread 예제 기대값 추가는 앞 구현의 결정과 이번 승인을 따른다. 대조군은 비화면 shared 경계에서도 따로 검사한다.

아직 구현하지 않은 것은 DS-09 화면 금지, DS-10 공개 props, DS-06 차트 라이브러리 숫자 props의 파일 제한, DS-11·DS-19의 레이아웃 최소 크기 및 파일별 넘침 제한이다. 승인 목록은 전부 비어 있다. 코드의 임의 계산으로 조립한 문자열 의미나 외부 라이브러리 내부 동작까지 분석하지 않는다.
