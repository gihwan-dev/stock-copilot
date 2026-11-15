# ADR 0069: DS-04 기본 컴포넌트 2

- 상태: 확정 (2026-11-15)
- 기준: `503261f48fe530378274307dfda375c0eba2b9f4`

## 대상 확정

근거는 각 폴더 README·preview.html, 공통 bundle.js·bundle.css·index.d.ts와 design/README.md다.

| 대상       | 산출물 폴더 | 변형과 상태                                                         | 공개 계약                                                                                    |
| ---------- | ----------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Card       | Card        | 제목·동작 있음/없음                                                 | title, actions, children. DS-04에서 면과 머리·본문을 구현하고 DS-11에서 중복 구현하지 않는다 |
| Table      | Table       | 일반·촘촘, 빈 목록·값 있음, 문자·숫자 정렬                          | 숫자는 자식으로 받는 머리·행·칸 조합. 형식화·정렬·선택·통화 변환 없음                        |
| Badge      | Badge       | neutral·primary·alert, mono                                         | UiText와 닫힌 tone·mono                                                                      |
| StatusBar  | StatusBar   | warning·error·info, 내용 없음·동작 있음/없음                        | 내용 없으면 빈 실시간 영역만, error는 alert, 나머지는 status                                 |
| Dialog     | Dialog      | open·closed, 일반·wide, 동작 있음/없음                              | Base UI Dialog, 닫힌 이유만 onClose에 전달                                                   |
| EmptyState | EmptyState  | 제목, 설명·아이콘·동작 선택                                         | UiText와 기존 Icon 이름, 동작 요소                                                           |
| Skeleton   | Skeleton    | text(기본 3줄), block(chart-height-sm·chart-height·chart-height-lg) | 정적 막대, aria-hidden. 반복 애니메이션 없음                                                 |

Callout은 화면 안에 머무는 별도 피드백 상자라 상태 띠에 포함하지 않는다. HoldingsTable·ScorecardTable, SignalCard·BriefingCard·DisclosureCard, 숫자·차트·채팅·레이아웃 배치는 제외한다. 설정은 design/README.md와 ScreenSettings README에 따라 Dialog다. Dialog README의 성적표 설명은 최신 공통 README의 본문 보조 화면 결정이 앞선다.

## 미해결과 검증 경계

- Badge·StatusBar 유예는 사용자 후속 결정으로 해소한다. tone은 의미 변형이며 DS 안의 빠짐없는 정적 클래스 대응표로 색 토큰에 연결한다. ADR 0040의 tone 이름 기반 색 토큰 판정을 이 범위에서 좁힌다.
- 표의 데이터 columns는 DS-10의 격자 columns 검사와 충돌한다. 브리프가 허용한 복합 표·자식 계약을 사용한다. 공개 하위에도 className·style·rest·DOM 이벤트·불린을 두지 않는다.
- 스크롤은 DS-19·DS-20 및 DS-11 ScrollArea 이후다. overflow·sticky를 추가하지 않는다.
- Skeleton의 60% 마지막 줄 폭은 토큰 근거가 없어 구현하지 않는다. aria-hidden은 bundle.js의 접근성 계약을 따른다. 애니메이션이 없어 움직임 줄이기에서도 정적이다.
- DS-07 실행 환경, DS-15 실제 CSS·양 테마 시각 회귀, DS-16 WKWebView, DS-20 넘침 실측은 후속 범위다. 이번은 Node·Testing Library·jsdom 동작 검증이다.
- 프로젝트 신뢰·훅 정의의 실제 런타임 신뢰는 프로젝트 파일만으로 증명할 수 없다. 로컬 설정 hooks=true는 확인했고 실제 검사를 직접 실행한다. 사용자 신뢰 저장소를 읽거나 바꾸지 않는다.

## 구현과 검사 대응

- Card 4, Table 3, Dialog 4, EmptyState 4, Skeleton 5: 이야기 20개, 각 light·dark. 별도 `feedbackComponentStories`를 기존 이야기 진입점에서 내보낸다. 기존 `baseComponentStories` 393개와 검증 단언은 그대로다.
- Skeleton.lines는 산출물에서 실제 사용한 3·6만 지원한다(bundle.js의 6줄, preview의 3줄). 임의 숫자를 위한 원시 타입 예외나 새 브랜드 생성기를 만들지 않는다.
- Table은 `Table.Root/Head/Body/Row/Header/RowHeader/Cell`이다. `align=end`인 숫자 칸은 오른쪽, 전체 표는 tabular-nums다. 숫자는 DS-05 자식으로 전달한다. 자동 열 형식화·열 배열·행 레코드 API는 만들지 않는다. 빈 Root는 기본 또는 지정 문구를 표시한다. 머리·본문을 조립하는 경우 빈 행 조립도 호출자가 맡는다.
- Dialog는 Base UI Root·Portal·Backdrop·Viewport·Popup·Title·Close를 쓴다. 제목은 자동 ID로 연결하고 aria-modal을 명시한다. `escape-key/outside-press/close-press`에서 `escape/outside/close-button` 값만 내보낸다. 다른 라이브러리 이유를 공개하지 않는다.
- 첫 입력 초점은 input.ts의 `focusDialogInput(ref): void`에서 소비한다. 숨김·비활성 입력을 제외한 첫 입력, 없으면 버튼, 없으면 Popup을 초점 대상으로 쓴다. useEffect·DOM 반환·콜백 ref가 없다. ds-dom은 실제 input.ts 선언의 이 함수로 넘기는 ref 인자만 추가 허용한다. boundaries는 Dialog.tsx 한 소비자만 더한다. 다른 파일과 같은 이름의 가짜 함수·직접 ref.current 읽기는 새 대조군에서 계속 거부한다.
- Base UI 내부 세부 객체의 공개 전달 검사는 유지한다. DS-10은 후속 결정의 의미 변형 증명만 추가한다.
- 추가 스타일은 bundle.css 241의 fixed/inset:0을 옮긴 dialog-overlay, 277의 글줄 높이를 생성 namespace로 옮긴 skeleton-line, 244의 아래 테두리 border-b-hairline, Skeleton README가 지정한 chart-height 세 높이뿐이다. Tailwind 생성 스냅샷은 기존 snapshot 함수로 재생성했다. fixed·inset-0·임의 높이·임의 z-index는 계속 실패한다. 가격색 허용 목록은 후속 결정에서 Badge.tsx만 추가하며 토큰 값·새 의존성·검사 끄기는 없다.
- 기존 테스트 변경은 ds-generation.test.mjs의 정확한 theme 기대에 utility 네 선언 추가, ds-screen-bootstrap.test.mjs의 input 내보내기와 반환 기대에 focusDialogInput/void 추가, knip.test.mjs의 정확한 운영 유예 기대에 새 파일 여섯 개 추가뿐이다. 단언 형태·기존 기대 원소는 유지한다.
- 첫 구현의 knip 운영 유예는 기존 48개에서 새 파일 여섯 개를 추가한 54개였다. 이번에는 Badge.tsx·StatusBar.tsx·Status.stories.tsx 세 개만 더해 57개다.
- 설정은 디자인 산출물에서 Dialog를 쓴다. architecture 5.1의 별도 macOS 설정 창 결정과 산출물 사이에는 간극이 있으므로 이번 작업은 설정 화면을 조립하거나 창 방식을 바꾸지 않는다.

- Dialog.onClose가 없는 경우에는 산출물 bundle.js와 같이 닫기 버튼을 표시하지 않는다. 콜백이 없는 제어 상태는 유지하고 외부 콜백도 발생하지 않는다. 해당 회귀는 수정 전 실패·수정 후 통과를 확인한다.

## 의미 변형 후속 결정

아래는 1066a3d 시점의 결정 기록이다. 공개 방향 배지·가격색 권한과 소비 판정은 마지막 적대적 리뷰 수정 결정이 대체한다.

사용자 지시로 `tone`, `variant`, `severity`, `status` 및 해당 접미사의 의미 prop은 컴포넌트 모듈 또는 DS 타입 파일에 선언한 닫힌 문자열 리터럴 유니온을 받는다. 색 자체를 받는 textColor·bg·color의 생성 색 토큰 계약은 유지한다. 직접 생성 토큰을 받던 Text의 기존 API도 유지한다.

대응은 실제 className이 소비하는 const 객체의 리터럴 값 또는 exhaustive switch의 리터럴 반환으로 제한한다. 빠진 값·동적 값·대응표 미사용·넓힌 문자열·템플릿 타입·다른 모듈 유니온의 확장·의미와 생성 토큰 타입 혼합은 실패한다. DS-02와 같은 클래스 집합·변형·파일별 가격색 권한을 사용한다. 임의 문자열의 클래스 소비는 허용하지 않는다. Button과 Skeleton의 기존 variant도 같은 증명 조건에 맞춰 정적 대응표를 실제로 사용한다.

Badge 산출물은 up/down을 가격·수급 방향으로 명시한다. 가격색 허용 목록에는 Badge.tsx 한 파일만 더한다. 화면이 방향이 아닌 곳에 up/down 배지를 쓰는 것은 린트로 막지 못한다. 방향 값은 코어가 보낸 값으로만 쓴다(architecture 5.3). DS-05의 숫자 표시 컴포넌트가 방향 배지를 감싸는 것이 맞는지는 DS-05에서 정한다.

StatusBar는 내용이 없으면 동작까지 포함해 아무것도 그리지 않는다. 기본 warning, error는 alert 역할, warning/info는 status 역할이다. warning/error는 alert-soft·alert, info는 surface-sunken·line·ink-muted를 쓴다. 가장 심각한 문제 하나의 선택은 화면 조립의 책임이다.

후속 이야기: Badge의 다섯 tone × 두 mono 상태 10개, StatusBar의 세 tone × 동작 유무 6개·내용 없음 3개·기본 warning 1개로 10개다. 별도 statusComponentStories 진입점으로 light·dark를 등록한다. 기존 DS-03 393개와 앞선 DS-04 20개는 그대로다.

경고/오류 색은 가격 색과 다르다. 라이트 alert=#a8520a, up=#d0202d, down=#1f5bd1; 다크 alert=#f1a34a, up=#ff5d5d, down=#5e9cff이다. 토큰 자체는 수정하지 않고 두 테마의 색 차이를 테스트한다.

정적 대응표는 별도 ds-semantic-variants 검사 모듈에서 타입 출처·리터럴·키의 완전성·실제 className 소비를 함께 확인한다. 조건식 안에서도 유효한 각 대응만 허용하고 순환 별칭은 거부한다. 타입을 옮기거나 인라인 끄기로 열린 클래스 경로를 만들지 않는다. DS-10 손 변형 15개는 유지한다. 기존 변형 테스트에는 새 검사 모듈의 import 경로 해석만 추가한다.

기존 테스트 기대 변경: ds-internal-review.test.mjs 가격색 목록에 Badge.tsx 한 경로, knip.test.mjs 운영 유예 목록에 새 DS 파일 세 경로를 추가한다. DS-04 충돌 회귀 둘은 실제 컴포넌트의 모든 린트 진단이 없는 통과 대조군으로 바꾸고, DS-04 타입 행에 두 컴포넌트의 양성·음성 사례를 추가한다. 단언 형태를 유지한다. Tailwind 클래스 스냅샷은 Badge 산출물 78행의 whitespace-nowrap 한 키워드에 대해서만 재생성한다.

후속 기준은 `1e2b4f34b8dffaa679fbc5e90f08276f23d6f122`이다. 마무리 때 진행된 로컬 master `6e966736fd33f1aa9143c595835016be0fbb5dd7`까지 병합한다. crates의 병합 결과는 해당 master와 동일하며 직접 편집하지 않는다. 충돌한 권한 설정과 훅 해시는 저장소 스크립트로 다시 만든다.

최종 의미 변형 예제는 통과 12개·위반 23개다. DS 하위 타입 파일도 직접 가져올 수 있으며, 생성 토큰을 선택하는 Extract/Exclude 안의 템플릿 타입도 허용하지 않는다. 구문 전용 린트처럼 program이 없는 환경에서는 새 대응표 경로를 허용하지 않고 기존 리터럴 클래스 판정을 유지한다.

가격색 권한은 대응표·switch 선언 파일과 실제 클래스 소비 파일 양쪽에서 확인한다. Badge의 가격색 대응표를 다른 DS 파일로 가져가 쓰는 객체·switch 경로는 실패하고, 가격색 없는 공유 대응표는 통과한다. 예제에서 바꾼 의존 파일도 먼저 린트해 TypeScript의 이전 모듈 캐시가 음성 사례를 가리지 않도록 한다.

통합 검사의 기준은 수용한 master `6e966736fd33f1aa9143c595835016be0fbb5dd7`이다. 커밋 훅이 HEAD와 master의 분기점을 사용하므로, master에서 이미 수용한 application/ports 테스트 변경을 다시 수정으로 보지 않도록 master를 첫 부모로 통합 커밋을 만들고 ds-04를 fast-forward한다. 검사·기존 테스트·master 참조는 바꾸지 않는다.

## 1066a3d 적대적 리뷰 수정 결정

사용자는 이번 회차의 eslint·harness·기존 테스트 보호 경로 수정을 승인했다. 독립 리뷰 결과는 차단 0·중요 4·사소 5, 우회 51건 중 10건 통과(신규 8·기존 2)였다. 실제 수정과 검증 결과는 이 절 및 구현 보고로 구분한다.

Badge의 공개 tone은 neutral·primary·alert로 제한한다. 산출물의 up·down은 이번 공개 API에서 제외한다. 의미를 가격 방향에 대응시키는 래퍼나 잘못된 화면 사용을 정적으로 막을 수 없기 때문이다. DS-05가 코어 방향 타입만 받는 별도 방향 배지를 만들고 그 파일만 가격색을 허용한다. 현재 PRICE_COLOR_FILES는 비어 있다.

의미 prop 이름은 tone·variant·severity·status·kind·intent·appearance의 정확 일치다. size는 기존 토큰 크기 규칙을 따른다. 이름과 무관하게 문자열 리터럴 prop이 className 계산에 도달하면 같은 대응 증명을 요구한다. orderStatus처럼 클래스에 닿지 않는 데이터에는 요구하지 않는다. ColorToken은 textColor·bg·color 이름에서만 받는다. Text.tone은 기존 표시값을 유지하는 로컬 의미 유니온과 정적 대응으로 옮긴다.

소비 증명은 실제 함수의 props 매개변수와 직접 구조 분해·기본값 처리한 const 값만 추적한다. 재구성·덮어쓰기·타입만 같은 const는 인정하지 않는다. 해당 속성의 전체 값이 완전한 대응을 거쳐 JSX className에 도달해야 한다. 조건식은 양쪽 모두 증명하고 JSX를 반환하는 각 경로에 증명이 있어야 한다.

StatusBar.action은 UiText label·값 콜백 onPress·enabled/disabled만 받고 내부 Button sm으로 그린다. 문장에는 min-width:0과 긴 토큰 줄바꿈 키워드를 쓴다. 내용이 없어도 빈 status·alert 실시간 영역을 유지한다. WAI-ARIA 1.3 5.3.5는 렌더링된 빈 영역을 먼저 만들고 내용을 바꾸도록 권고한다: https://www.w3.org/TR/wai-aria-1.3/#live_region_roles . 실기기 VoiceOver·WKWebView에서의 최초 알림·역할 전환·중복 낭독은 미해결이다. 정적/ jsdom 결과를 실기기 증명으로 보지 않는다.

| 새 허용                                                                      | 근거와 제한                                                         |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| 의미 prop 정적 대응표·switch                                                 | 닫힌 타입·전체 키·리터럴 값·실제 매개변수 소비 증명                 |
| DS_KEYWORDS: chart-height 3개·dialog-overlay·skeleton-line·border-b-hairline | 앞선 DS-04 산출물 근거, 토큰 클래스만                               |
| whitespace-nowrap                                                            | Badge 산출물                                                        |
| status-sentence                                                              | StatusBar 문장의 min-width:0과 긴 토큰 줄바꿈                       |
| theme 유틸리티 4개                                                           | dialog-overlay의 position/inset 2개·skeleton-line·border-b-hairline |
| Dialog.tsx → platform/input.ts                                               | 초점 입력 경계 한 소비자, DOM 값 반환 없음                          |

| 새 금지                                   | 이유                                                      |
| ----------------------------------------- | --------------------------------------------------------- |
| Badge up·down, 비허용 파일의 가격색 대응  | 방향은 코어 타입으로 봉인하는 DS-05에 인계                |
| 경로 꼬리 위조, 변수명에 숨긴 금지 클래스 | 프로젝트 상대 경로 정확 일치·값 검사                      |
| 다른 prop·가짜 const·외부 probe로 증명    | 해당 매개변수의 해당 속성만 인정                          |
| 분기 일부만 대응, 불완전·mutable 대응표   | 모든 값과 모든 표시 경로의 정적 증명                      |
| 의미 이름 접미사 판정                     | 데이터 prop 오탐 제거, 클래스 흐름은 이름과 무관하게 검사 |
| variant 등에서 ColorToken 받기            | 색 토큰 계약은 textColor·bg·color만                       |

기존 테스트 변경 7개: ds-generation은 theme 유틸리티 기대 원소를 추가하며 기존 원소를 유지한다. ds-screen-bootstrap은 focusDialogInput/void 경계 기대만 추가한다. ds-internal-review는 가격색 허용 목록을 비워 더 엄격하게 한다. ds-public-mutations는 새 모듈 import 해석만 추가하고 15개 변형을 유지한다. ds04-contract-blockers는 해결된 충돌을 실제 설정 통과 대조군으로 바꾼다. ds04-types는 두 컴포넌트 사례를 더하고 음성 사례별 오류를 확인해 총수 단언의 구멍을 없앤다. knip은 추가 파일만 정확히 열거하고 목록 동등성 단언을 유지한다. 이번 상태 테스트·의미 예제·손 변형 검사는 브랜치에서 새로 만든 테스트를 보강한다.

knip 운영 유예는 각 파일을 처음 실제 화면이 사용하는 작업에서 제거한다. Card·Table·EmptyState·Skeleton·Badge는 최초 목록 화면 조립, Dialog는 최초 보조 대화 화면 조립, StatusBar는 머리줄/연결 문제 화면 조립이 그 시점이다. 이야기 파일은 DS-07 이야기 실행 진입점에서 실제 사용될 때 제거한다. 아직 정해지지 않은 화면의 작업 번호를 만들지 않는다.

리뷰 수정으로 기존 Text·Disclosure·Dialog·Table의 표시 분기를 빠짐없는 정적 대응표로 옮겼다. 기존 상태의 클래스·문구·접근성 동작은 테스트로 유지한다. StatusBar 문장 유틸리티 추가에 맞춰 ds-generation의 정확한 CSS 기대에 두 선언만 더한다. 기존 기대 원소와 동등성 단언은 유지한다.

색 구별 회귀 기준은 CIE76 색차 10 이상이다. alert와 neutral(ink-muted)·primary의 양 테마 전경을 확인한다. 이 수치는 실기기 색각·낭독 검증을 대신하지 않는다.

수정 회차는 9b7c74285cca8ac83d914e609962695aad58ad8b를 정상 훅으로 병합한 8adb94c를 기준으로 한다. 소비 증명은 첫 props 매개변수에 한정하며, 같은 타입의 두 번째 인자·콜백 인자·다른 객체를 전달한 대리 호출도 거부한다. 의미 변형 예제는 통과 19·위반 52개이며 전체 설정·tsc 대조를 포함한다.

기존 테스트 추가 변경: ds-components-contract의 문자열 유니온 state 스타일 통과 대조군을 정적 대응표로 옮긴다. 위반 예제와 기존 단언은 유지한다. ds-screen-bootstrap의 UI 속성 격리 대조군은 의미 이름 kind 대신 데이터 이름 level을 써 같은 UI 속성 위반 3개와 규칙을 끈 통과를 확인한다. ds-public의 중첩 color 객체 통과 대조군은 바꾸지 않고 ColorToken 판정이 객체 전체로 번지는 오탐을 고친다. 새 모듈의 DS·생성 타입 출처도 프로젝트 상대 경로로 판정한다.

생성 토큰 유니온을 임의 의미 이름으로 받거나 클래스 계산에 흘려 대응을 생략하는 것도 금지한다. 문서화된 토큰 이름은 기존 분류 검사로 보내 bg에 SpacingToken 등을 넣는 우회를 막는다. DS-03 ADR 0056의 Text.variant: TypeStyleToken 계약만 해당 분류·출처 검사로 유지한다. ColorToken은 새 결정에 따라 textColor·bg·color만 받는다. 전체 설정과 tsc를 함께 대조하는 사례는 기존 8개에 토큰 이름 우회 3개를 더한 11개다.

Text.variant의 기존 토큰 예외는 실제 생성 TypeStyleToken 출처가 있는 선언에만 적용한다. 선언이 없는 복사 Record는 이 예외를 얻지 못한다.

## 마지막 수정 회차: 보장 범위와 한계

기준은 abe39e664455b40ce7fc6e37cb9a854241b15068이다. 메인은 변형이 실제 화면에 반영되는지 구문 추적으로 완전히 증명하지 않기로 결정했다. 임의 클래스·가격색·화면 스타일 주입·공개 검사 회피는 금지하고, 변형 대응은 실수로 빠진 것을 찾는 범위로 제한한다. 앞 절의 이름과 무관한 대응 요구·전역 함수 증명은 이 결정으로 대체한다.

직접 className 표현식에 닿는 tone·variant·severity·status·kind·intent·appearance만 대응 증명을 요구한다. size 등 문서화된 토큰 prop은 기존 분류 검사를 따른다. 증명은 그 함수의 모든 return 식에서 찾고, 같은 함수의 const 식별자와 조건식 양쪽을 전개한다. 풀 수 없는 반환은 증명으로 인정하지 않는다. JSX 속성 값에 들어간 요소는 반환 트리의 요소가 아니므로 증명에서 제외한다. 오류는 원인 함수에 보고하며 다른 파일·사용처로 전파하지 않는다.

공개 요소 반환 함수는 대소문자와 관계없이 props를 검사한다. 소문자 이름으로 요소 반환 함수를 내보내는 것도 금지한다. React 타입 검사기의 해석된 반환 타입과 ColorToken 타입을 사용해 별칭·인덱스 접근·Parameters로 이름 검사를 피하지 못하게 한다. 클래스 변형 타입은 명시적 닫힌 문자열 유니온을 쓴다. keyof typeof styles와 클래스 변형의 인덱스 접근 타입 거부는 유지한다. 데이터 prop에는 클래스 대응 증명을 요구하지 않는다.

### 한계

| 리뷰의 통과 경로                                                          | 이번 판정    | 이유와 후속 검증                                         |
| ------------------------------------------------------------------------- | ------------ | -------------------------------------------------------- |
| V01 소문자 요소 반환 함수·index 재내보내기로 DOM 이벤트/ref 공개          | 차단         | 이름과 무관한 props 검사 및 소문자 함수 내보내기 금지    |
| Z16 같은 파일의 다른 함수가 증명                                          | 차단         | 직접 클래스 참조가 있는 그 함수마다 검사                 |
| Z14 반환 const 식별자                                                     | 전개 후 검사 | 같은 함수의 const만 전개, 미증명 반환은 실패             |
| Z15 반환 조건식                                                           | 양쪽 검사    | 어느 한쪽이 미증명이면 실패                              |
| Z17 풀 수 없는 반환 호출·식                                               | 차단         | 실행 결과를 추측하지 않음                                |
| Z13 data-proof 등 JSX 속성 값의 가짜 요소                                 | 차단         | 반환 요소와 자식의 className만 증명                      |
| shade: Box["color"], Parameters<typeof f>[0]                              | 차단         | 해석된 ColorToken은 textColor·bg·color 이름만 허용       |
| 목록 밖 이름 level·emphasis, 숫자 유니온, 객체 안 값으로 허용 클래스 선택 | 한계로 허용  | 임의 클래스·가격색은 별도 규칙으로 계속 금지             |
| 파생 const·별칭을 거친 분기                                               | 한계로 허용  | 완전성 추적을 요구하지 않음                              |
| Z12 조건부 형제 안에서만 존재하는 증명                                    | 한계로 허용  | 시각 결과·상태 의미는 스토리·시각 회귀·리뷰가 확인       |
| Z10 값이 같은 대응표                                                      | 허용         | warning/error 등 정당한 동일 스타일을 막지 않음          |
| tone ?? fallback의 fallback 의미                                          | 한계로 허용  | 키를 클래스로 오인하지 않으며 대응 값은 정적 클래스 검사 |

같은 Props를 받되 children만 그리는 도우미, kind의 아이콘 선택, status의 접근성 속성 사용, 다른 DS 의미 prop으로의 전달은 클래스 계산이 아니므로 증명하지 않는다. 실제 클래스 값에는 DS-02의 정적 토큰·변형·가격색 검사를 그대로 적용한다. 토큰 밖 클래스가 들어간 대응표는 변수 이름과 관계없이 실패한다. 기본 키의 오탐 억제는 정적 클래스 검증을 통과한 대응표 조회의 키 리터럴에만 적용한다.

StatusBar는 flex-none 래퍼 하나 안에 빈 status/alert 영역을 유지한다. 문장만 실시간 영역에 넣고 아이콘과 동작 버튼은 그 밖의 형제로 둔다. warning↔error 전환에서 버튼 노드와 초점을 유지하는 테스트를 추가한다. 빈 내용에서는 면·아이콘·동작을 만들지 않는다. 실기기 VoiceOver/WKWebView 낭독 확인은 계속 미해결이다.

변형 완전성과 시각 결과는 상태별 스토리 테스트, 해당 DS 작업의 시각 회귀, 리뷰가 본다. 이 린트의 통과를 화면 의미의 완전 증명으로 보고하지 않는다. Card·EmptyState·Dialog의 ReactElement 동작 슬롯은 이번 범위 밖이며 메인이 후속 작업으로 등록한다.

기존 테스트의 이전 회차 우회용 수정은 되돌린다. ds-components-contract의 state 조건식 통과 대조군과 ds-screen-bootstrap의 kind 접근성 대조군을 복원하며 단언은 유지한다. 브랜치의 의미 예제는 위 한계에 맞춰 통과·위반 판정을 명시적으로 고치고, 기존 DS-10 손 변형 15개는 유지한다.


### 마지막 회차의 검증과 변경 목록

먼저 master 257bd4c94999034745f75fb7df2c1fb545138919를 병합했고, 병합 커밋 3cdb73fcdfd3165ff493214e36adb9ff19b6b571은 정상 훅을 통과했다. 훅 생략은 사용하지 않았다. 이 회차에 의존성·토큰·가격색 허용 경로는 추가하지 않았다. Badge 공개 변형은 neutral·primary·alert다.

| 이번에 허용하거나 좁힌 것 | 경계 |
|---|---|
| 직접 클래스 계산에 쓰지 않는 의미 데이터 prop | 닫힌 값·DOM/이벤트 금지는 유지 |
| 정적 대응표 조회의 기본 키 리터럴 | 실제 대응표 전체가 정적 토큰·가격색 검사에 통과한 경우에만 Tailwind의 키 오탐을 억제 |
| 클래스 대응의 증명 요구 범위 | 위 한계 표 밖으로 추적을 확장하지 않음 |
| 요소 반환 함수의 공개 검사 | 소문자·추론된 JSX 반환·함수 선언·재내보내기까지 검사 |
| 색 토큰 계약 이름 검사 | 해석된 ColorToken의 인덱스 접근·Parameters 별칭도 검사 |
| StatusBar의 렌더링 구조 | flex 항목 하나, 문장만 실시간 영역, 버튼 노드와 초점 유지 |

button-variants의 공개 함수 이름은 ButtonVariant로 바꿔 소문자 요소 반환 함수 금지를 따른다. 화면의 Button 계약은 그대로다. 기존 검사의 한 분기를 끄는 변경은 없다.

이번 회차에 수정한 테스트는 다섯 파일이다. ds-components-contract와 ds-screen-bootstrap은 이전 회차의 우회용 대조군 수정을 원형으로 복원하고 기존 단언을 유지한다. ds-semantic-variants는 의미 예제의 명시적 한계 판정, 전체 ESLint 및 tsc 검증, 원인 위치·깨끗한 Badge/index 대조군을 검사한다. ds-semantic-mutations는 기존 16개를 유지하고 수정 분기 13개를 더해 위반 누락뿐 아니라 오탐 재발·오류 위치 변이도 잡는다. ds04-status-components는 문장만 읽기와 단일 래퍼·버튼 노드·초점 유지 단언을 더한다. DS-10의 기존 손 변형 15개는 변경하지 않았다.

의미 예제는 99개(통과 44·위반 55)이며, 이번에 추가한 28개 중 필수 위반 및 다섯 오탐 대조군은 전체 ESLint 설정과 tsc로 검증한다. 의미·공개 검사 손 변형은 29개(이번 수정 분기 13개 포함)다. 최종 전체 검사 결과는 구현 보고와 리뷰 기록에 남긴다.

전체 검사의 기존 DS-10 tone: Text 위반 예제가 의미 데이터 prop의 자유 문자열 회귀를 잡았다. 대응 증명 요구와 타입 경계를 분리해 고쳤다. 클래스에 쓰지 않는 의미 prop도 넓은 문자열·브랜드 문자열은 받지 않고 닫힌 문자열 값을 받는다. 숫자·객체 데이터의 기존 판정은 유지한다. 기존 ds-public.test.mjs 및 ds10-public.json은 수정하지 않았다. 이 경계의 위반 예제와 손 변형 증인을 추가했다.
