# DS-03 기본 컴포넌트

- 상태: 구현·적대적 리뷰 수정
- 최초 기준: `4c46c32e9239384b24ca0e13869b118fe2a3a3c7`
- 이번 리뷰 대상: `13a86d78dc73be83dd672d92ba48bbfd67e49529`
- 결정: 2026-11-14 사용자 지시. 공개 계약은 유지하며 내부 구현 오탐을 좁힌다. 기존 테스트의 정확한 기대값과 승인된 규칙 수정만 한다.
- 범위: 글자·버튼·입력·선택·토글·고급 펼침·아이콘. 카드·표·대화 상자·숫자·차트·레이아웃·마크다운·채팅 입력은 해당 후속 DS 작업이 맡는다.

## 대상과 디자인 대응

각 경로의 README·preview와 공통 `design/components/bundle.js`, `bundle.css`, `index.d.ts`를 근거로 한다. 불리언 상태는 architecture 5.3의 닫힌 유니온, 크기는 생성 토큰 이름으로 옮긴다.

| 컴포넌트         | 산출물 경로                         | 변형·상태                                                                                              | 등록 수 |
| ---------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------ | ------- |
| Heading          | design/components/Heading/          | level 1·2·3, title-lg·md·sm                                                                            | 3       |
| Text             | design/components/Text/             | body·body-sm·label·caption·code × ink·ink-muted·ink-subtle·primary·alert; p·span·div                   | 75      |
| Button           | design/components/Button/           | primary·secondary·ghost × control-height·control-height-sm × enabled·disabled; hover·focus·Enter·Space | 12      |
| IconButton       | design/components/IconButton/       | 두 control 크기, 기본·pressed·expanded·disabled·tooltip right                                          | 10      |
| TextField        | design/components/TextField/        | text·secret, hint·error·disabled·빈 값·값 있음, 보기·가리기·붙여넣기                                   | 6       |
| Select           | design/components/Select/           | native 목록, 기본·hint·error·disabled·선택                                                             | 4       |
| Checkbox         | design/components/Checkbox/         | checked·unchecked × enabled·disabled, 설명·Space                                                       | 4       |
| Switch           | design/components/Switch/           | on·off × enabled·disabled, 설명·Space                                                                  | 4       |
| SegmentedControl | design/components/SegmentedControl/ | 2–5개 선택지, 세 선택 상태, 화살표 포커스·Space·빈 선택 방지                                           | 3       |
| Disclosure       | design/components/Disclosure/       | open·closed, 제어·비제어, Enter·Space·aria-controls·동일 화살표 90도 회전                              | 2       |
| Icon             | design/components/Icon/             | 45개 이름 × icon-sm·icon-md·icon-lg × 장식·의미                                                        | 270     |

합계 393개 등록 항목에 light·dark를 명시한다. 공개 `shared/ds/index.ts`에는 컴포넌트만 둔다. DS-07은 별도 진입점 `shared/ds/stories.ts`의 `baseComponentStories`와 각 `.stories.tsx`에 Storybook 실행 환경을 붙인다. Knip의 이야기 진입점에는 운영 표시 `!`를 붙이지 않는다.

## 공개 계약과 범위 축소

- className·style·rest·임의 HTML 속성 전달이 없다. 표시 문구는 UiText, 입력 값은 별도의 InputValue다. 공개 불리언·이벤트·DOM 값·콜백 ref가 없다.
- Text.variant·tone, Button·IconButton.size, Icon.size는 생성 토큰의 해당 분류만 받는다. 컴포넌트별 실제 TypeScript 컴파일로 다른 분류와 열린 변형을 거부한다.
- design/README.md의 “매수·매도 버튼은 SignalCard에만 있어요”에 따라 공개 Button에서 buy·sell과 가격색 구현·허용 항목을 제거한다. SignalCard 작업이 내부 전용 버튼을 만들고 공개 목록에는 내보내지 않는다. buy·sell 호출의 컴파일 실패를 고정한다.
- TextField README에는 onCommit이 없다. 해당 공개 prop을 제거하고 조합 중 Enter는 외부 확정 콜백 대신 폼의 잘못된 제출을 경계에서 막는다. IconButton의 tooltip hidden, Heading의 id도 사용자 결정대로 제거하고 타입 검사에서 거부한다.
- Select README는 “운영체제 기본 목록을 써서 키보드·화면 낭독기가 그대로 동작해요”를 명시하므로 native select를 유지한다. label은 htmlFor로 select만 가리킨다. 도움말·오류·옵션은 접근 이름에 들어가지 않고 테스트에서 이름 전체가 정확히 일치해야 한다.

## DS-10 규칙 보정

ADR 0040의 JSX 문맥 타입 검사만 좁힌다. 실제 선언이 `node_modules/@base-ui/react`인 내부 콜백의 라이브러리 문맥 이벤트 인자는 공개 props가 아니다. 공개 컴포넌트 props의 이벤트·DOM·중첩·토큰 분류·ref·rest 검사와 일반 컴포넌트의 JSX 문맥 검사는 유지한다.

추가로 내부 Base UI 콜백의 매개변수를 가공 없이 공개 콜백에 그대로 전달하면 거부한다. `props.onPick(details)`와 값/콜백의 단순 별칭·구조 분해 전달을 검사한다. `details.reason`처럼 추출한 닫힌 값은 통과한다. `base-checkbox`, `base-switch`, `base-disclosure`, `base-segment`, `base-input`에 세부 객체 전달 위반 예제를 붙였다. 기존 아홉 통과 대조군과 DS-10 손 변형 15개의 검출을 유지한다. 클래스 조건식의 모든 리터럴 분기 검사도 유지한다.

## DOM 경계와 누출 검사

DS-09 ADR의 mountApp 방식으로 DOM 소비를 별도 경계에 둔다. `shared/platform/index.ts`는 mountApp(app): void만 내보낸다. 새 `shared/platform/input.ts`는 다음 세 함수만 내보낸다.

| 함수            | 입력                                       | 반환·바깥으로 전달하는 값                                              |
| --------------- | ------------------------------------------ | ---------------------------------------------------------------------- |
| inputEventValue | React 입력·native 선택 ChangeEvent         | Result<InputValue, BrandError>                                         |
| inputCommit     | React KeyboardEvent, idle·composing·ending | commit·ignore; 조합 Enter이면 preventDefault                           |
| pasteInputValue | 결과 수신 콜백                             | void; 콜백에는 Result<InputValue, BrandError 또는 닫힌 ClipboardError> |

TextField.tsx·Select.tsx 두 파일만 input.ts를 가져올 수 있다. 화면·다른 DS 파일·platform/index.ts의 재수출은 boundaries 위반 예제로 고정한다. 플러그인의 기본값은 같은 요소 안의 의존을 검사하지 않으므로 checkInternals를 켜고 기존 내부 의존은 그대로 허용한 다음 input.ts 예외만 좁힌다. 이 옵션·각 정책 제거 변형도 검출한다.

DOM 접근·platform 파일 허용 목록에 input.ts 정확한 경로 하나를 추가한다. 거절 변환 허용 목록은 index.ts에서 input.ts로 옮긴다. keyCode 229 읽기는 경계의 작은 구조 타입으로 한정하고 원시 타입 허용 목록에 input.ts만 추가한다. 방문 집합과 깊이 상한을 유지하며 DOM 반환은 허용하지 않는다. ds-screen-bootstrap은 mountApp뿐 아니라 세 입력 함수의 반환을 모두 검사한다.

`ds/no-dom-exports`의 멤버 생략은 TypeScript의 lib.es*.d.ts만 해당한다. DOM lib과 React 선언은 생략하지 않는다. 타입 인자를 먼저 검사하고 반환된 객체의 콜백 인자도 탐색한다. 최상위 입력 경계 함수의 입력 이벤트는 소비할 수 있지만 반환값·중첩 반환 콜백으로 누출할 수 없다. 호출 가능한 값 자체에 붙은 속성과 타입 인자는 기존처럼 검사한다. 함수 부가 속성의 DOM 타입도 별도 회귀로 고정한다. SyntheticEvent<never, never> 반환과 DOMAttributes<never> 반환, DOM Promise는 거부하고 값 Promise는 통과하는 실제 컴파일·린트 대조군을 둔다. 깊이 초과는 실패한다.

## 입력과 접근성

- 체크·스위치·세그먼트는 Base UI 값 콜백을 쓴다. Button·Input·Tooltip·Collapsible도 문서의 Base UI 기반이다.
- isComposing 또는 keyCode 229이면 Enter를 확정으로 보지 않는다. compositionend 직후 첫 keydown도 ending 상태에서 막는다. 다음 일반 Enter는 제출할 수 있다. 실제 WKWebView 이벤트 순서와 한글 입력기는 DS-16이 실측한다.
- 비밀 입력은 기본 가림, 값이 있을 때만 보기 버튼이다. 사용자 지우기와 외부 제어 값 비우기 모두 가림 상태로 되돌린다. TextField README의 비밀 입력 동작과 SEC-1·SEC-11의 노출 최소화에 따라 autocomplete=off, spellcheck=false, autocapitalize=none을 둔다. 값 변경에 따른 로컬 상태 보정은 조건부 렌더 재시도로 하며 effect를 추가하지 않는다.
- React 19.3.1의 제어 Input은 jsdom에서 value 프로퍼티뿐 아니라 value 속성에도 가짜 비밀 값을 동기화했다. 테스트가 이 사실을 확인한다. DOM에 값이 없다고 주장하지 않는다. 로그·저장소로 값을 보내는 기능은 추가하지 않았다. 실제 WebView·비밀 관리·노출 범위 검증은 S-02·DS-16 후속이다.
- 클립보드 없음·거절은 닫힌 오류 값으로 바꾸고 현재 값을 보존한다. 비활성 입력·체크·스위치·버튼·붙여넣기·선택에서 바깥 콜백이 불리지 않음을 검사한다. SegmentedControl·Disclosure에는 산출물에 없는 비활성 prop을 만들지 않는다.

## 스타일 근거

- design README·design-system.json·글자 산출물에 keep-all·anywhere 명시가 없어 break-keep·wrap-anywhere는 추가하지 않는다. 이 간극은 후속 디자인 결정으로 남긴다.
- 기존 border-transparent, w-switch, border-t-hairline은 bundle.css의 투명 테두리·토큰 복합 폭·상단 테두리에서 왔다.
- Disclosure README의 90도 회전과 bundle.css 236행에 따라 rotate-disclosure 고정 유틸리티 하나를 추가한다. 아이콘 자체를 교체하지 않고 같은 chevron-right를 회전한다. 회전은 즉시 바뀌므로 움직임 줄이기에서도 애니메이션이 없다. 실제 전환 시간의 표현은 DS-15 검증 때 평가한다.
- themeSource로 theme.css와 생성 클래스 스냅샷을 맞춘다. @theme·토큰 JSON·라이트/다크 값은 바꾸지 않는다. 토큰 정의·대비는 기존 독립 검사로 검증한다.
- 아이콘의 45개 기하는 산출물 bundle.js의 Lucide 경로다. ISC 고지는 public/Icons-LICENSE.txt에 둔다. 기하 허용 파일 네 개는 유지한다.

## 브라우저 공급 결정과 검증 경계

harness 4.3은 Testing Library를, R-15 2.4는 “jsdom에서 돌리거나 Vitest browser mode(Playwright 제공자)에서 돌린다”를 허용한다. harness 6.6은 실제 시각 회귀를 태그 고정 Playwright 컨테이너로 정한다. 따라서 just check의 컴포넌트 동작 검사는 Node + Testing Library + jsdom으로 실행한다. jsdom에 없는 MessageChannel은 Node의 표준 구현을 연결하고 정리할 때 포트를 모두 닫는다. user-event는 Testing Library의 키보드·Tab·Space·Enter·화살표·포커스·선택 조작 도구다. 설치 스크립트나 브라우저 다운로드가 필요하지 않다.

Playwright 운영·개발 의존성과 Chromium 실행 도우미·실제 SVG 기하 테스트를 제거한다. DOM 테스트는 393개 이야기의 두 테마 등록과 DOM 의미를 검사하며 실제 786개 렌더링 이미지·색·레이아웃 검증이라고 주장하지 않는다. 사용자 리뷰의 “806개”는 삭제된 buy·sell·tooltip hidden 10개 항목까지 포함한 이전 수치다.

DS-15에 넘기는 검증: 실제 CSS의 hover·focus 링 표시, 두 테마의 버튼 높이·색·비활성 색, 현재 786개 이야기 조합의 실제 렌더링·시각 회귀, 45개 아이콘의 SVG 기하·stroke·크기·Tab 제외. DS-07은 Storybook 실행 환경, DS-20은 부하·넘침, DS-16은 WKWebView 실측을 맡는다. jsdom의 native Select에 ArrowDown·Enter를 전달하지만 OS 목록의 실제 키보드 선택 확정은 이 환경에서 검증할 수 없어 DS-15·DS-16에 넘긴다. 기본 검사는 PLAYWRIGHT_BROWSERS_PATH를 빈 임시 폴더로 지정하고 일반·CI 전체 검사를 실행한다.

## 규칙과 기존 테스트 변경

- ds-dom: React 선언 생략 제거, ES 표준 라이브러리만 생략, 반환 객체의 콜백 인자 검사, 승인된 입력 어댑터 경로 이동.
- boundaries: checkInternals와 기존 내부 관계 보존 정책, input.ts 전역 거부와 두 소비 파일만 허용.
- ds-public-props: 내부 콜백의 무가공 공개 전달 거부. 공개 props 규칙·DS-10 위반·변형은 유지.
- 허용 목록: input.ts의 platform·DOM·원시 입력 타입·거절 변환, Button 가격색 예외 삭제. 검사 끄기 없음.
- boundaries.test.mjs: 옵션 제거 변형 목록에 checkInternals 한 항목 추가.
- ds-generation.test.mjs: 고정 theme 문자열에 rotate-disclosure 선언만 추가.
- ds-internal-review.test.mjs: PRICE_COLOR_FILES를 다시 빈 목록으로, DOM_ACCESS_FILES에 input.ts만 추가.
- ds-screen-bootstrap.test.mjs: index의 mountApp 단독 기대 복구, input의 세 반환형을 모두 검사하도록 추가. 기존 DOM·화면 금지 단언 유지.
- local-allowlists.test.mjs: platform·raw에 input.ts 추가, promise 예외를 index.ts에서 input.ts로 이동.
- knip.test.mjs: 이야기 진입점 정확한 경로 추가. production 기대는 도달 관계 변화에 따라 95개에서 48개의 정확한 항목으로 조정한다. 전체 lib가 다시 운영에서 미사용이므로 예전 export/type 유예를 파일 단위 실측 항목으로 교체하고 input.ts·stories.ts만 추가한다. 와일드카드 없음. 실제 전체 목록은 harness/knip-check/production-allowlist.json이다.
- ds-final-review.test.mjs의 기하 네 파일 기대는 변경하지 않았다. 새 DS-03 테스트는 이 브랜치에서 수정 가능하다는 승인대로 동작 환경·공개 계약을 맞춘다.

## 최종 직접 의존성

기존 저장소의 React 등 의존성은 유지하고 DS-03에서 사용하는 추가 의존성은 다음과 같다.

| 이름                        | 고정 버전 | 용도                                | 근거                                     | 라이선스 |
| --------------------------- | --------- | ----------------------------------- | ---------------------------------------- | -------- |
| @base-ui/react              | 1.9.0     | 운영, 기반 컴포넌트                 | architecture 5.2                         | MIT      |
| @testing-library/react      | 16.3.3    | 개발, 렌더·쿼리·act                 | harness 4.3                              | MIT      |
| @testing-library/user-event | 14.6.10   | 개발, Testing Library 키보드·포인터 | harness 4.3, 사용자 리뷰 4               | MIT      |
| jsdom                       | 31.0.1    | 개발, DOM 환경                      | R-15 2.4, harness 4.3·6.6, 사용자 리뷰 4 | MIT      |

jsdom·user-event는 안정판을 정확히 고정한다. 추가 기능·설치 스크립트·브라우저 설치는 없다. 이번 수정에서 추가된 직접·전이 패키지 전체는 다음과 같으며 preinstall/install/postinstall이 모두 없다. decimal.js는 jsdom의 테스트 환경 의존성이며 제품 금액 계산에는 사용하지 않는다.

| 패키지                           | 버전    | 라이선스      |
| -------------------------------- | ------- | ------------- |
| @exodus/bytes                    | 1.16.0  | MIT           |
| @testing-library/user-event      | 14.6.10 | MIT           |
| bcp-47-match                     | 2.0.3   | MIT           |
| bcp-47                           | 2.1.2   | MIT           |
| data-urls                        | 8.0.0   | MIT           |
| decimal.js                       | 10.6.0  | MIT           |
| entities                         | 9.0.0   | BSD-2-Clause  |
| html-encoding-sniffer            | 7.0.0   | MIT           |
| is-alphabetical                  | 2.0.1   | MIT           |
| is-alphanumerical                | 2.0.1   | MIT           |
| is-decimal                       | 2.0.1   | MIT           |
| is-potential-custom-element-name | 1.0.1   | MIT           |
| jsdom                            | 31.0.1  | MIT           |
| lru-cache                        | 11.5.3  | BlueOak-1.0.0 |
| parse5-parser-stream             | 8.1.1   | MIT           |
| parse5                           | 8.1.1   | MIT           |
| saxes                            | 6.0.0   | ISC           |
| tldts-core                       | 7.5.1   | MIT           |
| tldts                            | 7.5.1   | MIT           |
| tough-cookie                     | 6.1.0   | BSD-3-Clause  |
| tr46                             | 7.0.0   | MIT           |
| undici                           | 8.12.4  | MIT           |
| w3c-xmlserializer                | 6.0.0   | MIT           |
| webidl-conversions               | 8.0.1   | BSD-2-Clause  |
| whatwg-mimetype                  | 5.0.0   | MIT           |
| whatwg-url                       | 17.3.0  | MIT           |
| xml-name-validator               | 5.0.0   | Apache-2.0    |
| xmlchars                         | 2.2.0   | MIT           |

## 검사 기록과 미해결

입력/공개 계약/반환 타입 회귀, 아홉 내부 대조군과 다섯 무가공 전달 위반, 컴포넌트 타입, DS-10 손 변형 15개와 jsdom 동작 검사를 실행한다. 실제 브라우저 검증은 위 후속 범위다. 수용한 로컬 master는 `1cd405c9f7ff11be0a26d1eb892766463aaf722a`다. 병합 커밋을 만들기 전에도 조상인 리뷰 대상 `13a86d78dc73be83dd672d92ba48bbfd67e49529`를 전체 검사 기준으로 쓴다. master의 crates/domain 소스와 이번 병합 소스의 차이는 없다.

남은 제품 결정은 한글 줄바꿈 산출물의 부재다. SignalCard 전용 매수·매도 버튼, Storybook 실행 환경, 실제 시각·넘침·WKWebView 검증은 각 후속 DS 작업 범위이며 이번 공개 API에 미완성 대체 기능을 남기지 않는다.
