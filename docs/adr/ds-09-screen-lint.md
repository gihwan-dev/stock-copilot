# DS-09: 화면 코드 금지

- 상태: 초안
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
| 5    | UI 라이브러리 직접 가져오기                     | ds/no-screen-imports                             | recharts, @base-ui/react                         | DS 래퍼에서 허용                                           |
| 6    | HTML 주입                                       | ds/no-dom-styling 재사용                         | dangerouslySetInnerHTML                          | DS-02에서도 금지 유지                                      |
| 7    | 스타일 리터럴·직접 형식 변환·하이픈 빼기·이모지 | ds/no-style-literals 재사용, ds/no-screen-format | 13px, Intl, toFixed, -1, 이모지                  | 스타일 값은 DS-02 금지 유지, 형식 변환은 shared/lib/format |
| 8    | 함수형 ref 및 DOM 값 접근                       | ds/no-dom-values 재사용, guard/no-ref-callback   | ref 화살표, current, currentTarget.ownerDocument | RefObject를 ref에 전달만 허용                              |

모든 위반을 DS에서 허용하지 않는다. DS-02가 이미 금지한 팩터리·DOM 접근·HTML 주입을 약화하지 않는다. DS에서 허용되는 HTML과 토큰 className으로 화면과의 차이를 검증한다.

## 값과 경계

이벤트 객체는 화면 코드로 넘기지 않는다. DS 입력 컴포넌트는 onValueChange(value) 형태의 브랜드 값을 넘긴다. 화면에서 event.target.value도 읽지 않는다. DOM 판정은 DS-02를 재사용하며 계산된 문자열의 임의 실행 결과를 분석하지 않는다.

JSX의 aria-label, aria-description, title, alt, placeholder, label, description 문구는 UiText만 허용한다. 기존 local/no-ui-text의 자식 문구 검사를 유지하고 속성에 별도 타입 검사를 더한다. key, id, variant 같은 구조 값은 이 검사 대상이 아니다.

새 최상위 폴더는 기존 boundaries/no-unknown-files로 거부된다. DS 깊은 경로는 기존 boundaries/dependencies의 index.ts 공개 진입점 규칙으로 거부된다.

## 후속 작업

DS-10은 공개 props의 className/style/rest 및 토큰·RefObject 계약, DS-03은 실제 컴포넌트, DS-11은 배치 컴포넌트, DS-19는 스크롤과 넘침을 맡는다. 임의 prop 이름에 계산된 클래스 문자열을 전달하는 의미 분석은 DS-10의 공개 계약이 함께 막아야 한다.

임시 허용 목록은 app/main.tsx의 DOM 부팅 전달 하나다. DOM 전역·스타일 금지는 유지한다. U-03에서 부팅을 DOM 값이 화면으로 나오지 않는 포트로 옮긴 뒤 목록을 비운다. 현재 소문자 JSX에 필요한 DS-03 임시 예외는 없다.

## 검증과 한계

새 규칙 6개와 재사용 규칙 5개는 각각 독립적으로 켜면 위반 예제가 실패하고 끄면 통과하는 테스트가 있다. 정적 토큰 className과 소문자 요소는 DS에서는 통과하고 두 화면 루트에서는 실패한다. 별도 임시 TypeScript 프로젝트가 실제 앱 ESLint 설정을 읽고 DOM·ref·이벤트·요소 팩터리·문구·가져온 클래스·깊은 DS 가져오기를 tsc 통과 뒤 규칙별로 거부하는지 확인한다. 새 pages/widgets/components는 기존 경계 규칙으로 실패한다. UI 문구의 UiText 통과 대조군도 있다.

남는 재현 가능한 한계: messages.ts에서 `("bg" + "-primary") as UiText`를 문구 목록에 넣고 목록 선택 함수를 통해 DS의 임의 `paint` prop으로 넘기면 화면·messages 린트와 tsc가 통과한다. 런타임 검증의 DS 테스트 대역은 DS-10 전의 느슨한 임의 prop을 모델링한다. 실제 DS가 이런 prop을 공개해도 된다는 뜻이 아니다. 문자열 조각의 계산·브랜드를 넘어선 의미까지 분석하지 않으므로 DS-10의 공개 props 계약이 필요하다. 테스트는 이 한계를 명시적으로 재현하며 닫혔다고 주장하지 않는다.

기존 테스트 변경: eslint-config의 끌 수 없는 목록에 새 6개 이름만 추가했다. ds-style의 화면 규칙 부재 기대를 DS-02 재사용 규칙 활성 기대까지 강화했다. eslint-violations의 기존 기대 집합에는 새 ds 오류만 추가하고 기존 항목은 유지했다. 새 규칙으로 기존 주석 대조군이 DOM 값 위반이 되어 문제 없는 대조 파일을 추가했다. 새 의존성·검사 끄기·crates 변경은 없다.
