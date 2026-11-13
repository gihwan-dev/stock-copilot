# ADR 0036: DS-09 화면 코드 금지

- 상태: 확정 (2026-11-14)
- 기준: 88566415e2503aee40d19e74b5917c02f2b8e5e4
- 범위: apps/desktop/src/app 및 features 전체

## 완료 기준 원문

harness 4.4 화면 코드 금지 1~8마다 위반 예제 실패. 같은 코드가 `shared/ds`에서는 통과 화면 코드가 DOM에 닿는 길(`event.currentTarget.ownerDocument`, DOM `ref`의 값으로 계산)이 없음. JSX 속성 문자열 문구(`aria-label`, `title`)는 `UiText`만 (ADR 0012).

## 먼저 정하는 대응

| 번호 | 금지                                            | 규칙                                             | 위반 예제                                        | shared/ds 대조                                             |
| ---- | ----------------------------------------------- | ------------------------------------------------ | ------------------------------------------------ | ---------------------------------------------------------- |
| 1    | 소문자 HTML/SVG 및 문자열 태그 별칭             | ds/no-screen-elements                            | div, svg, const Tag="div"                        | div 허용                                                   |
| 2    | 요소 팩터리                                     | ds/no-element-factory 재사용                     | createElement, cloneElement, jsx                 | DS-02에서도 금지 유지                                      |
| 3    | 클래스·스타일 속성과 객체 키                    | ds/no-screen-props, ds/no-style-object 재사용    | className="bg-primary", class, style             | 리터럴 토큰 className 허용                                 |
| 4    | CSS·CSS-in-JS·클래스 도우미                     | ds/no-screen-imports, ds/no-screen-classes       | CSS import, cn, styled                           | DS의 토큰 클래스 허용, 수기 CSS 금지 유지                  |
| 5    | UI 라이브러리 직접 가져오기                     | boundaries/dependencies                          | recharts, @base-ui/react                         | DS 래퍼에서 허용                                           |
| 6    | HTML 주입                                       | ds/no-dom-styling 재사용                         | dangerouslySetInnerHTML                          | DS-02에서도 금지 유지                                      |
| 7    | 스타일 리터럴·직접 형식 변환·하이픈 빼기·이모지 | ds/no-style-literals 재사용, ds/no-screen-format | 13px, Intl, toFixed, -1, 이모지                  | 스타일 값은 DS-02 금지 유지, 형식 변환은 shared/lib/format |
| 8    | 함수형 ref 및 DOM 값 접근                       | ds/no-dom-values 재사용, guard/no-ref-callback   | ref 화살표, current, currentTarget.ownerDocument | RefObject를 ref에 전달만 허용                              |

모든 위반을 DS에서 허용하지 않는다. DS-02가 이미 금지한 팩터리·DOM 접근·HTML 주입을 약화하지 않는다. DS에서 허용되는 HTML과 토큰 className으로 화면과의 차이를 검증한다.

## 값과 경계

이벤트 객체는 화면 코드로 넘기지 않는다. DS 입력 컴포넌트는 onValueChange(value) 형태의 브랜드 값을 넘긴다. 화면에서 event.target.value도 읽지 않는다. DOM 판정은 DS-02를 재사용하며 계산된 문자열의 임의 실행 결과를 분석하지 않는다.

JSX의 사용자 문구 속성은 아래 WAI-ARIA 기반 닫힌 목록에 따라 UiText만 허용한다. 기존 local/no-ui-text의 자식 문구 검사를 유지하고 속성에 별도 타입 검사를 더한다. key, id, variant 같은 구조 값은 이 검사 대상이 아니다.

새 최상위 폴더는 기존 boundaries/no-unknown-files로 거부된다. DS 깊은 경로는 기존 boundaries/dependencies의 index.ts 공개 진입점 규칙으로 거부된다.

## 후속 작업

DS-10은 공개 props의 className/style/rest 및 토큰·RefObject 계약, DS-03은 실제 컴포넌트, DS-11은 배치 컴포넌트, DS-19는 스크롤과 넘침을 맡는다. 임의 prop 이름에 계산된 클래스 문자열을 전달하는 의미 분석은 DS-10의 공개 계약이 함께 막아야 한다.

임시 허용 파일 목록은 비어 있다. main.tsx는 ReactElement를 mountApp에 넘기기만 한다. createRoot와 DOM 루트 조회는 기존 DOM 허용 파일 shared/platform/index.ts 안에서 수행하고 반환은 void다. ds/no-dom-exports가 DOM 허용 파일의 공개 값과 함수 반환값(중첩 함수·객체 포함)을 검사한다. 공개 export가 mountApp 하나인지도 타입 검사기로 고정한다.

## 검증과 한계

화면 규칙과 DOM export·문구 규칙에는 독립적으로 켜면 위반 예제가 실패하고 끄면 통과하는 테스트가 있다. 정적 토큰 className과 소문자 요소는 DS에서는 통과하고 두 화면 루트에서는 실패한다. 별도 임시 TypeScript 프로젝트가 실제 앱 ESLint 설정을 읽고 DOM·ref·이벤트·요소 팩터리·문구·멤버 태그·깊은 DS 가져오기를 tsc 통과 뒤 규칙별로 거부하는지 확인한다. 새 pages/widgets/components는 기존 경계 규칙으로 실패한다. UI 문구의 UiText 통과 대조군도 있다.

문구 목록의 `("bg" + "-primary") as UiText`를 임의 paint prop으로 전달하는 것은 데이터 전달 단계에서 허용한다. DS-02의 static-classes가 DS에서 그 prop을 className으로 쓰는 것을 거부하므로 현재 실제 스타일 누출 경로가 아니다. 이 DS 소비 거부를 테스트로 고정한다. DS-10은 문자열 prop을 클래스로 쓰지 않는 계약과 이벤트 객체를 공개 prop으로 내보내지 않는 계약을 유지해야 한다.

## 적대적 리뷰 수정

- 화면 외부 패키지는 기존 boundaries 정책의 허용 목록으로 닫는다: react(React 조립), @tanstack/react-router(화면·URL 상태), @tanstack/react-query(서버 상태), react-error-boundary(클래스를 쓰지 않는 오류 경계). 근거는 architecture 5장·5.2와 harness 4.2다. vitest는 harness 4장의 테스트 도구로서 *.test.ts 및 *.test.tsx에서만 허용한다. react-dom은 platform/index.ts에서만 허용한다. 멤버 표현식 JSX 태그는 화면에서 전부 금지한다. CSS·도우미는 ds/no-screen-imports가, 패키지 위치는 boundaries가 담당해 중복 판정하지 않는다.
- 클래스 문자열 검사는 2차 방어다. 단일 단어·토큰·리터럴 유니온은 허용하고 JSX 속성의 타입을 클래스 값으로 분석하지 않는다. 여러 유틸리티 토큰의 공백 목록, 변형 접두사, 임의 값과 변수 축약만 거부한다. 생성 tokens.ts AST에서 모든 문자열 토큰을 읽어 통과 대조군으로 사용한다.
- UiText 속성 검사는 화면과 shared/ds 모두에 적용한다. [WAI-ARIA 1.3](https://www.w3.org/TR/wai-aria-1.3/)의 자유 문자열 문구 속성인 aria-label, aria-description, aria-valuetext, aria-roledescription, aria-placeholder, aria-braillelabel, aria-brailleroledescription, aria-colindextext, aria-rowindextext와 HTML title, alt, placeholder, label 및 description, summary, abbr, prompt, standby를 닫힌 목록으로 둔다. aria-labelledby·describedby 등 ID 참조, keyshortcuts 등 기계 문법, 상태 토큰·숫자는 문구가 아니므로 제외한다. input의 button·submit·reset 종류에서 value는 버튼 문구이므로 UiText를 요구한다. 입력·선택의 데이터 value는 이 문구 검사 대상이 아니다.
- 하이픈 음수와 이모지는 messages.ts의 목록 값에서 검사한다. 음수 문구는 U+2212를 사용한다. Emoji_Presentation, 국기 지역 표시자, keycap 시퀀스, 명시적인 VS16 이모지 표현을 금지한다. 기본 텍스트 표현인 ©는 허용하고 ©+VS16은 이모지로 거부한다. 문구 키는 이 검사 대상이 아니다.
- 형식 변환은 Intl, toLocaleString, toLocaleDateString, toLocaleTimeString, toFixed, toExponential, toPrecision을 화면에서 금지한다.
- console은 DOM 자체를 조작하지 않아도 lib.dom이 제공하는 호스트 객체라는 기존 경계 판정을 유지한다. no-console을 별칭으로 우회하는 경로도 다시 열지 않는다. 허용하는 끄기 주석 대조군은 DOM과 무관한 debugger 문장으로 바꿔 오류 0개를 유지한다.
- 금지 2·6·8은 DS-02도 금지한다. 따라서 모든 위반이 DS에서 통과한다는 뜻으로 완료 기준을 해석하지 않고, HTML·토큰 클래스처럼 DS에서 허용되는 코드를 대조군으로 삼는 의도된 차이를 유지한다.

기존 테스트 변경: boundaries의 react-dom 허용 위치 기대를 platform/index.ts로 옮겼다. eslint-config의 끌 수 없는 목록에 DOM export·문구 값 규칙을 더하고 DS 속성 규칙의 순서를 맞췄다. allowed-form.ts는 DOM과 무관한 debugger로 바꿔 기대 오류 0개를 복구했다. 부팅 예제와 이를 참조하는 예제는 rootElement에서 mountApp API로 이름을 맞췄다. 속성 문구와 목록 밖 패키지를 새로 거부하는 기존 gap 예제의 정확한 보고 목록을 강화했다. 새 의존성·새 인라인 검사 끄기·crates 직접 수정은 없다.
