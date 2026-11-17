# ADR 0122: DS-19 스크롤·넘침 정적 규칙

- 상태: 확정 (2026-11-17)

- 기준: 994c25a191e0ab08e2c8e9f1f26cfa2de7be3f4d

## 메인의 추가 책임 결정

2차 독립 리뷰·비판 뒤 메인이 두 책임 배치를 확정했다. 이 결정은 기존 코드를 통과시키기 위한 임의 허용 확대가 아니라 각 컴포넌트 자신의 크기와 기존 단일 프리미티브 사용의 설계 결정이다. 53405097의 DialogDescription·chart-loading.tsx 이동을 되돌리고 원래 EmptyState·Skeleton 파일에 렌더링을 복원한다. chart-loading 파일·전체 #21 권한·Knip 유예 등록은 제거한다.

#21의 추가 허용은 정확한 (파일, 토큰) 집합이다. EmptyState.tsx는 max-w-dialog-width 하나, Skeleton.tsx는 skeleton-line·h-chart-height-sm·h-chart-height·h-chart-height-lg만 허용한다. 이 치수는 레이아웃 배치가 아니라 컴포넌트 자신의 고유 크기이고 모두 기존 디자인 토큰이다. 다른 폭·높이 토큰과 임의 값은 계속 거부한다. Markdown의 prose-max 결정은 그대로 유지한다. 일반 길이 허용 목록과 좁은 파일·토큰 집합 양쪽에 정확한 집합 대조를 두어 추가 항목 변형을 검출한다.

IconButton.tsx는 기존 Tooltip.Positioner 사용 하나만 예외로 둔다. 정확한 @base-ui/react/tooltip의 Tooltip 이름 가져오기와 JSX의 한 Positioner만 허용하며 별칭·네임스페이스·재내보내기·변수 추출·두 번째 Positioner는 거부한다. 일반 absolute·fixed·sticky 클래스, 다른 Positioner 및 ScrollArea 권한은 부여하지 않는다. 이 예외도 정확한 집합 대조로 고정한다. 나머지 Base UI 스크롤·겹침 계열은 문서의 파일 책임 안에서만 가져올 수 있고 재내보내기는 막는다. 현재 일반 위치 목록에 없는 Select·Combobox 계열을 미리 풀지 않는다.

3차 리뷰 뒤 같은 원칙을 적용한 추가 고유 치수 예외는 Card.tsx의 min-h-toolbar-height 하나와 Icon.tsx의 size-icon-sm·size-icon-md·size-icon-lg 세 개다. Card의 머리줄과 아이콘 자신의 기존 크기를 보존하는 정확한 쌍이며 두 파일은 일반 #21 목록에서 제거한다. 다른 폭·높이·임의 값은 거부한다. 다른 단일 프리미티브 사용 예외는 추가하지 않았다. 2차 이후의 막힘 판정은 이 메인 결정으로 해소됐다.

## 결정

기존 better-tailwindcss/no-restricted-classes에 harness 4.4 #18~24의 파일별 제한을 더한다. 화면의 ScrollArea 가져오기와 다시 내보내기는 기존 ESLint no-restricted-imports로 제한한다. 새 검사 도구와 의존성은 없다. CSS는 기존 ds-style의 생성된 theme 바이트 일치와 별도 수기 CSS 금지를 유지한다. 임의 CSS 선언은 모두 거부하므로 속성별 stylelint 예외보다 엄격하다. tokens.css는 기존 토큰 재생성 검사가 맡는다.

메인 결정: 현재 긴 글 최대 폭의 책임 위치는 shared/ds/Markdown.tsx다. architecture 5.1이 긴 글을 prose-max에서 멈추도록 요구하고 현재 Markdown이 그 계약을 지므로 #21에서 prose-max 토큰 하나만 예외로 둔다. markdown-content의 정확한 생성 선언과 max-w-prose-max만 허용하며, 임의 폭·다른 max-width 토큰·w-prose-max·h-prose-max는 거부한다. Markdown의 제품 소스와 폭 제한은 변경하지 않는다. 이는 메인의 설계 결정에 따른 예외이며 기존 코드를 통과시키기 위한 임의 확대가 아니다.

허용 목록은 ds-overflow.mjs의 정확한 파일 경로다. 컨트롤에는 버튼 변형 파일·아이콘·입력·선택·토글·펼침을, 머리줄에는 Toolbar·StatusBar·Card를 등록한다. 차트의 내부 표는 일반 위치 권한을 받지 않고 기존 sr-only만 정확히 허용한다. ScrollArea만 스크롤 생성 클래스를 허용한다. 음수 여백·translate·scale의 파일 허용 목록은 비어 있다. 없는 파일 이름은 미래 DS-11의 문서 지정 위치이며 폴더 예외로 풀지 않는다. Text의 말줄임 허용은 별도의 nowrap 클래스까지 허용하지 않는다. 여러 줄 말줄임은 기존 DS-02의 닫힌 토큰 집합에서 계속 거부한다.

사용자 정의 유틸리티도 제한 대상이다. dialog-overlay는 #20, chat-input·markdown-content·skeleton-line은 #21에 포함한다. sr-only의 위치 지정은 기존 차트 대체 표의 정확한 파일에 한정한다. 의미별 클래스 표는 작성 파일과 소비 파일 양쪽에서 토큰 집합·가격색·넘침 제한을 확인한다. 일반 변수·템플릿·계산된 cn 인자와 clsx 호출은 기존 static-classes가 거부한다. 인라인 끄기도 기존 no-restricted-disable 등록으로 거부한다.

## 기존 코드 조정

EmptyState.tsx·Skeleton.tsx·Dialog.tsx의 제품 소스는 기준 994c25a1의 내용으로 복원한다. DOM·문구·클래스·치수·공개 props를 보존한다. 닫힌 내부 도우미의 필요성을 주장했던 1차 수정의 근거는 2차 평가에서 기각했으며 위의 메인 결정을 최종 근거로 삼는다. IconButton의 제품 소스도 변경하지 않는다.

## 1차 독립 리뷰와 비판 반영

Opus의 EmptyState 폭 삭제 지적은 수용해 위와 같이 기존 폭을 복원했다. 범용 Clip의 통과 가능성은 기존 검사 한계로 기록하되, 정상 DS 조립 전체를 막는 내보내기 일괄 제한은 적용하지 않는다. LENGTH_FILES가 원래 문서상 가진 길이 권한과 Markdown에 추가한 prose-max 단일 예외를 구분한다. 메인 결정은 다른 모든 길이 허용 파일에서 prose-max를 금지하라는 뜻으로 확대하지 않는다.

HTML 기본 동작은 기존 no-presentation-props를 보강한다. native dialog, popover 계열 속성, cols·rows·wrap=off를 거부한다. textarea는 architecture가 명시한 ChatInput 파일에서만 쓰고 soft·hard 줄바꿈만 허용한다. pre는 기존 chat-content 줄바꿈을 명시한 경우를 허용한다. 기존 Markdown·ChatInput과 타입 검사에 통과하는 태그 별칭도 대조·위반 자료로 검증한다. 모든 pre·textarea를 일괄 금지하지 않는다. 실제 브라우저의 최종 크기·넘침은 DS-20이 검증한다.

의미 유틸리티는 designUtilities의 CSS 선언에서 #18~24 분류를 유도한다. 기존 이름을 정책 정규식에 손으로 반복하지 않는다. 합성 유틸리티를 실제 Tailwind로 컴파일한 뒤 속성·정책 연결을 검사해 새로운 이름의 overflow-y:auto·position:absolute·고정 길이·화면 단위·nowrap·음수 여백·translate를 검증한다. text-nowrap·whitespace-pre·음수 space-x/y도 개별 패턴과 독립 증인에 추가한다. 테마에서 생성되지 않는 위치 0 키워드 다섯 개는 제거한다. 기존 unknown 검사와 닫힌 토큰 집합은 유지한다.

## 2차 이후 검사 보강

ChartTable은 일반 위치 목록에서 빼고 sr-only에만 좁은 예외를 적용한다. absolute·fixed·sticky는 원시 클래스와 의미 표 모두 거부한다. native select의 multiple 속성을 거부하고 단일 선택은 허용한다. 타입 검사에 통과하는 태그 별칭도 검증한다.

의미 유틸리티에서 inline-size·block-size와 min/max 변형은 물리 크기와 같은 #21로, text-wrap-mode:nowrap은 #23으로, vi·vb와 동적 변형은 #22로 분류한다. 행렬 변환과 음수 text-indent는 translate·음수 여백과 같이 내용을 영역 밖으로 이동시킬 수 있어 #24에 대응한다. clip-path와 contain:paint/strict/content는 내용을 잘라낼 수 있어 #18에 대응한다. 현재 생성 유틸리티에 이 누락 선언이 있다는 주장이 아니라 보호된 유틸리티가 향후 바뀔 때의 검사 보강이다. 안전한 선언 대조군과 실제 Tailwind 컴파일 결과를 함께 검사한다.

허용 집합의 추가 항목을 검출하는 정확한 대조 회귀를 더했다. 2차 수정에서 chart-loading 추가 항목을 실패시켰고, 메인 결정 뒤 그 권한과 파일을 제거했다. 일반 목록·좁은 토큰 예외·단일 프리미티브 예외를 각각 정확히 대조한다.

POSITION 집합의 회귀만 독립 실행하는 OS 임시 복사본에서 원본 대조군 통과를 확인하고, FutureHelper.tsx를 허용 목록에 추가하는 문법상 유효한 변형을 실제 실행했다. 새 정확한 집합 단언이 검출했다. 전체 회귀의 chart-loading 실패를 통과로 바꾼 실험이 아니며 기존 49개와 구분한다.

## 3차 리뷰와 비판 뒤 결정

Base UI는 shared/ds에서만 가져올 수 있도록 기존 패키지 위치 경계를 강화한다. shared/lib에서 UI 프리미티브를 내보내고 lib/index를 거쳐 DS로 되가져오는 경로도 원본 외부 의존 지점에서 거부한다. 직접 가져오기뿐 아니라 재내보내기 전체 사슬을 기존 컴포넌트·props를 보존한 앱 전체 컴파일과 린트로 대조한다.

NumberField의 ScrubAreaCursor와 Slider의 Thumb는 설치된 1.9.0 소스에서 fixed·absolute 위치를 만든다. Drawer에도 가상 키보드 측정을 위한 fixed probe가 있다. 현재 승인된 DS 소유자가 없는 세 공개 계열은 임의로 소유자를 배정하지 않고 거부한다. 공개 exports 목록만으로 위험을 자동 결정하지 않는다. 각 공개 진입점의 실제 mjs 구현에 있는 위치·스크롤 선언과 anchor helper를 위험 목록과 대조하는 독립 감사와 별도의 직접 JSX 증인을 추가한다. 이 감사가 임의 라이브러리의 모든 런타임 동작을 증명한다는 뜻은 아니다.

popover·popoverTarget·popoverTargetAction은 태그 종류에 관계없이 표현 경계에서 거부한다. Base UI Button이 네이티브 속성을 전달하는 경로를 별도 타입·린트 증인으로 검증한다. 저장소의 고정 Linux arm64 Playwright 컨테이너에서 실제 Button을 렌더하고 popover를 열어 Chromium 158.0.8127.18의 open=true·position=fixed·overflow=auto·inset=0px을 측정했다. 이는 WKWebView나 실제 내용 스크롤의 실측을 대신하지 않는다.

화면의 광범위한 ScrollArea 이름 제한은 유지한다. namespace/export-all에서도 사실인 설명으로 진단 문구를 바꾸고 기존 기대 진단의 개수와 규칙을 유지한다. 3차의 재현 지적은 f581ec35b3f37f40eaa790256ecd55733d09f2d7 → 7ebabd6f7253974088a3ead2e11ae2bd112545c5에서 수정하고 회귀·손 변형으로 검증했다. 지시문에 따라 4차 독립 리뷰는 하지 않는다. 최신 master c22117c94038c27d252fdf220d88919605afde54를 정상 --no-ff 병합으로 받으며 마지막 훅 전체 검사가 통과해야 완료다.

## 검증

시작 frozen 설치와 lefthook 설치는 성공했다. 프로젝트 신뢰·세션 훅 신뢰와 현재 세션 적용은 확인할 수 없다. 지시문의 예외에 따라 진행하며 Git 커밋 훅을 사용한다. 실제 키·계좌·API 및 홈 설정은 다루지 않았다.

harness/eslint-violations/ds19-overflow.json은 #18~25의 위반·통과 대조군이다. DS19 policy 테스트는 실제 설정과 번호별 단독 제한을 각각 실행해 기존 토큰 금지에 가려진 누락도 검출한다. CSS 경계 테스트는 각 속성과 화면 단위를 위반 자료로 검사한다. 별도 타입 인식 앱 테스트는 템플릿·도우미·변수·로컬 및 가져온 의미별 클래스 표·인라인 끄기를 검증하고, 가져온 표 반례는 실제 tsc 통과 뒤 ESLint 거부를 확인한다.

DS04 컴포넌트 회귀 12개와 새 우회 검사 1개가 통과했다. 시각 검사는 현재 등록된 Button·Card·Money·TextField의 라이트·다크 8쌍에서 바이트 일치를 확인했다. 이 기준에 없는 EmptyState·Skeleton 전체의 시각 동등성을 증명하지는 않는다. 제품 조정 뒤 최종 결과와 독립 리뷰는 리뷰 기록에 추가한다. 새 DOM·CSS 회귀는 긴 설명의 max-width, 정상·넓은 Dialog 폭, Skeleton 줄 수·원래 높이 클래스를 검사하고 원본 디자인 토큰의 480px·12px과 실제 생성 CSS를 대조한다. 이것은 브라우저 픽셀 비교를 대신한다고 주장하지 않는다.

## 승인된 기존 테스트 변경과 보호 파일

| 파일                                              | 테스트·자료                                        | 전 → 후                                                                                          | 승인 |
| ------------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ---- |
| apps/desktop/tests/boundaries.test.mjs            | 패키지 위치 고정 목록                              | 기존 항목 유지 + Base UI의 DS 위치 제한 1개                                                      | (c)  |
| apps/desktop/tests/eslint-config.test.mjs         | 끌 수 없는 규칙 고정 목록                          | 기존 항목 유지 + no-restricted-imports 1개                                                       | (c)  |
| apps/desktop/tests/ds-semantic-mutations.test.mjs | 변형 모듈 import 준비                              | 기존 경로 해석 유지 + 새 ds-overflow.mjs URL 해석                                                | (d)  |
| apps/desktop/tests/ds04-style.test.mjs            | DS04 style 통과 자료 준비                          | Probe에 모든 클래스를 배치 → Dialog·Skeleton의 정확한 허용 위치. 모든 단언과 기존 위반 자료 유지 | (d)  |
| harness/ds-style/cases.json                       | review-ref.tsx                                     | 기존 세 진단 유지 + no-restricted-classes                                                        | (c)  |
| harness/eslint-violations/cases.json              | src/app/comments/undisableable-namespace-import.ts | 기존 진단 유지 + ScrollArea 또는 모듈 전체 가져오기 제한 진단                                    | (c)  |
| harness/eslint-violations/cases.json              | src/app/syntax/export-all.ts                       | 기존 진단 유지 + ScrollArea 또는 모듈 전체 가져오기 제한 진단                                    | (c)  |
| harness/eslint-violations/cases.json              | src/features/imports/namespace.ts                  | 기존 진단 유지 + ScrollArea 또는 모듈 전체 가져오기 제한 진단                                    | (c)  |
| harness/eslint-violations/cases.json              | src/features/money/generic-key.ts                  | 기존 진단 유지 + ScrollArea 또는 모듈 전체 가져오기 제한 진단                                    | (c)  |
| harness/eslint-violations/cases.json              | src/features/react/effect-namespace.ts             | 기존 진단 유지 + ScrollArea 또는 모듈 전체 가져오기 제한 진단                                    | (c)  |

기존 기대 목록 추가 8항목(c)과 준비 변경 2항목(d), 총 10항목이다. 새 DS19 테스트·새 사례 등록은 기존 테스트 변경 수에 넣지 않는다. 첫 정상 훅의 test-diff 도구 지적은 0건이며 훅 예외를 사용하지 않았다.

ESLint 설정·규칙 파일은 이번 정적 제한 등록에 필요하다. boundaries.mjs의 Base UI 패키지 위치 추가는 DS 밖 UI 재내보내기를 막는 검사 강화다. 새 ds-overflow-elements.mjs와 ds-overflow-utilities.mjs는 기존 규칙의 네이티브 속성 및 의미 유틸리티 분석이고, ds-overflow-primitives.mjs는 기존 표현 속성 규칙에 연결하는 Base UI 책임 경계 분석이다. chart-loading.tsx의 추가·유예는 최종 변경에서 제거했다. 고정 자료와 Node 테스트는 규칙별 증인 등록이다. Codex 설정·sandbox fragment·Claude 해시는 기존 생성기로 갱신한다. 기존 금지 삭제와 검사 끄기 추가는 없다.

## 인계

- DS-11: 레이아웃 컴포넌트(Page 등)가 들어오면 긴 글 폭 책임을 그쪽으로 옮길지 재검토한다. 현재 책임은 Markdown.tsx이며 prose-max 하나만 허용한다.
- DS-11 또는 이후 Tooltip DS 작업: 공개 Tooltip 컴포넌트가 생기면 IconButton의 위치 책임을 그쪽으로 옮기고 Tooltip.Positioner 단일 사용 예외를 지운다. EmptyState·Skeleton의 고유 치수는 현재 원래 파일의 정확한 토큰 집합으로 유지한다.
- DS-11 또는 후속 DS 작업: Card·Icon의 고유 크기 네 쌍은 원래 파일에서만 유지하고 레이아웃 책임 변경 시 좁은 집합을 재검토한다. NumberField·Slider·Drawer를 도입할 때 실제 위험 부품의 소유자를 먼저 정하고 범용 권한으로 풀지 않는다.
- DS-20: 실제 문서·요소 넘침, 잘림·스크롤 중첩·창 밖 요소·조작 요소 겹침을 렌더링으로 검증한다. 정적 검사가 이 실측을 대신하지 않는다.
- DS-12: 현재 4개 컴포넌트의 시각 기준을 전체 공개 목록으로 확대한다. EmptyState·Skeleton도 포함해야 한다.

## 손 변형

아래 49개는 53405097까지의 실행 결과다. 2차 이후 미커밋 변경에 대한 최종 변이 결과를 뜻하지 않는다. 문법 검사에 실패하거나 실행 오류가 난 변형은 검출 수에 넣지 않는다.

OS 임시 디렉터리에 검사 코드·자료를 복사하고 원본 대조군 통과를 확인한 뒤, 각 변형에 node --check와 ds19-policy.test.mjs를 실제 실행했다. 최종 26개 모두 문법 검사를 통과했고 해당 테스트의 단언 실패로 검출됐다. 중복 화면 단위 패턴 때문에 동등했던 초기 변형은 중복 분기를 제거한 뒤 다시 실행했으며 검출 수에 중복 합산하지 않는다.

| 변형             | 바꾼 것                                                                              | 잡은 테스트          | 결과 |
| ---------------- | ------------------------------------------------------------------------------------ | -------------------- | ---- |
| filter-invert    | `!files.includes(file)` → `files.includes(file)`                                     | ds19-policy.test.mjs | 검출 |
| filter-drop      | `!files.includes(file)` → `false`                                                    | ds19-policy.test.mjs | 검출 |
| overflow-prefix  | `overflow(?:-[xy])?-.+` → `neverflow(?:-[xy])?-.+`                                   | ds19-policy.test.mjs | 검출 |
| truncate         | `&#124;truncate&#124;` → `&#124;truncation&#124;`                                    | ds19-policy.test.mjs | 검출 |
| line-clamp       | `line-clamp-.+` → `line-limit-.+`                                                    | ds19-policy.test.mjs | 검출 |
| ellipsis         | `text-ellipsis` → `text-ellipses`                                                    | ds19-policy.test.mjs | 검출 |
| scroll-auto      | `(?:auto&#124;scroll&#124;overlay)` → `(?:automatic&#124;scroll&#124;overlay)`       | ds19-policy.test.mjs | 검출 |
| scroll-scroll    | `(?:auto&#124;scroll&#124;overlay)` → `(?:auto&#124;scrolling&#124;overlay)`         | ds19-policy.test.mjs | 검출 |
| scroll-overlay   | `(?:auto&#124;scroll&#124;overlay)` → `(?:auto&#124;scroll&#124;overlaid)`           | ds19-policy.test.mjs | 검출 |
| absolute         | `absolute&#124;fixed` → `relative&#124;fixed`                                        | ds19-policy.test.mjs | 검출 |
| fixed            | `absolute&#124;fixed` → `absolute&#124;relative`                                     | ds19-policy.test.mjs | 검출 |
| sticky           | `&#124;sticky&#124;` → `&#124;relative&#124;`                                        | ds19-policy.test.mjs | 검출 |
| inset            | `inset(?:-[xy])?` → `inside(?:-[xy])?`                                               | ds19-policy.test.mjs | 검출 |
| top              | `&#124;top&#124;bottom` → `&#124;upper&#124;bottom`                                  | ds19-policy.test.mjs | 검출 |
| length-h         | `(?:h&#124;w&#124;size)` → `(?:q&#124;w&#124;size)`                                  | ds19-policy.test.mjs | 검출 |
| length-w         | `(?:h&#124;w&#124;size)` → `(?:h&#124;q&#124;size)`                                  | ds19-policy.test.mjs | 검출 |
| length-size      | `(?:h&#124;w&#124;size)` → `(?:h&#124;w&#124;sizing)`                                | ds19-policy.test.mjs | 검출 |
| viewport-screen  | `(?:h&#124;w&#124;size)-screen` → `(?:h&#124;w&#124;size)-screens`                   | ds19-policy.test.mjs | 검출 |
| viewport-height  | `v[hw]` → `v[w]`                                                                     | ds19-policy.test.mjs | 검출 |
| nowrap           | `"whitespace-nowrap"` → `"whitespace-normal"`                                        | ds19-policy.test.mjs | 검출 |
| margin           | `-m[xytrblse]?` → `-n[xytrblse]?`                                                    | ds19-policy.test.mjs | 검출 |
| translate        | `(?:translate&#124;scale)` → `(?:translation&#124;scale)`                            | ds19-policy.test.mjs | 검출 |
| scale            | `(?:translate&#124;scale)` → `(?:translate&#124;scaling)`                            | ds19-policy.test.mjs | 검출 |
| prose-file       | `file === PROSE_FILE` → `file !== PROSE_FILE`                                        | ds19-policy.test.mjs | 검출 |
| prose-token      | `markdown-content&#124;max-w-prose-max` → `markdown-content&#124;max-w-dialog-width` | ds19-policy.test.mjs | 검출 |
| prose-escape-all | `id === 21 && file === PROSE_FILE` → `id === 21`                                     | ds19-policy.test.mjs | 검출 |

## 1차 수정의 손 변형과 검증

수정한 코드의 임시 복사본에서 정책 변형 26개를 다시 실행해 모두 검출했고, 의미별 클래스 표의 넘침 제한 삭제 1개도 다시 검출했다. 아래 추가 22개는 원본 대조군 통과 뒤 실행했다. JS는 node --check, TSX는 실제 tsc 통과를 확인하고, 테스트의 AssertionError로 검출된 경우만 센다. 합계 49개이며 Rust 변이 선택 결과와 구분한다. 추가 22개 중 dialog-normal-width는 같은 파일의 기존 Dialog 표현을 지키는 회귀 강화다.

초기 dialog-normal-width 변형은 EmptyState 전용 검사만으로 실행했을 때 생존했다. 정상·넓은 Dialog의 폭 회귀를 더하고 정확한 설명 폭 변형도 따로 둔 뒤 재실행해 둘 다 검출했다. 준비 중 모듈 누락으로 실행하지 못한 실험과 중복 화면 단위 분기의 동등 변형은 집계하지 않는다.

| 변형                       | 바꾼 것                                                                                             | 잡은 테스트                           | 결과 |
| -------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------- | ---- |
| semantic-overflow-link     | 의미별 클래스 표의 파일 정책 검사 삭제                                                              | ds19-bypass.test.mjs의 semantic table | 검출 |
| native-dialog              | `names.includes("dialog")` → `false`                                                                | ds19-native.test.mjs                  | 검출 |
| native-pre                 | `names.includes("pre") && !wrappingClass(node)` → `false`                                           | ds19-native.test.mjs                  | 검출 |
| native-textarea            | `projectPath(ctx, ctx.filename) !== "src/shared/ds/ChatInput.tsx"` → `false`                        | ds19-native.test.mjs                  | 검출 |
| pre-wrap-control           | `.includes("chat-content")` → `.includes("text-body")`                                              | ds19-native.test.mjs                  | 검출 |
| native-popover             | `"popover",` → `"not-popover",`                                                                     | ds19-native.test.mjs                  | 검출 |
| native-cols                | `"cols",` → `"columnCount",`                                                                        | ds19-native.test.mjs                  | 검출 |
| native-rows                | `"rows",` → `"rowCount",`                                                                           | ds19-native.test.mjs                  | 검출 |
| native-wrap                | `!["soft", "hard"].includes(node.value?.value)` → `false`                                           | ds19-native.test.mjs                  | 검출 |
| native-tag-filter          | `nativeTags(ctx, node.parent.name).length === 0` → `nativeTags(ctx, node.parent.name).length !== 0` | ds19-native.test.mjs                  | 검출 |
| utility-policy-link        | `declarationPolicies(property, value).includes(id)` → `false`                                       | ds19-native.test.mjs                  | 검출 |
| utility-auto               | `/^overflow(?:-[xy])?$/u` → `/^neverflow(?:-[xy])?$/u`                                              | ds19-native.test.mjs                  | 검출 |
| utility-position           | `property === "position"` → `false`                                                                 | ds19-native.test.mjs                  | 검출 |
| utility-white-space        | `property === "white-space"` → `false`                                                              | ds19-native.test.mjs                  | 검출 |
| utility-margin             | `/^margin(?:-.+)?$/u.test(property)` → `false`                                                      | ds19-native.test.mjs                  | 검출 |
| utility-transform          | `property === "transform"` → `false`                                                                | ds19-native.test.mjs                  | 검출 |
| dialog-normal-width        | `max-w-dialog-width` → `max-w-full`                                                                 | ds19-design.test.mjs                  | 검출 |
| description-width          | `text-body-sm max-w-dialog-width` → `text-body-sm max-w-full`                                       | ds19-design.test.mjs                  | 검출 |
| line-height-token          | `skeleton-line rounded-sm` → `pt-space-1 pb-space-2 rounded-sm`                                     | ds19-design.test.mjs                  | 검출 |
| block-height               | `props.height === "chart-height-lg"` → `props.height === "chart-height-sm"`                         | ds19-design.test.mjs                  | 검출 |
| line-count                 | `props.lines === 6` → `props.lines === 3`                                                           | ds19-design.test.mjs                  | 검출 |
| intrinsic-alias            | `typeFlags.StringLiteral` → `typeFlags.NumberLiteral`                                               | ds19-bypass.test.mjs                  | 검출 |
| description-child-contract | `readonly children: UiText` → `readonly children: ReactElement &#124; UiText`                       | ds19-bypass.test.mjs                  | 검출 |

chart-loading.tsx의 Knip 유예는 메인 결정에 따라 최종 차이에서 제거했다. 제품 코드의 치수·DOM은 기준 상태와 같으며 새 구현 파일은 ESLint 책임 경계 분석에만 있다.

## 메인 결정 반영 후 손 변형

현재 구현을 OS 임시 디렉터리에 복사하고 원본 policy·native·primitives 테스트의 통과를 확인한 뒤 아래 32개를 실제 실행했다. 모든 변형은 node --check에 통과했고 지정된 테스트의 AssertionError로 검출됐다. 생존·실행 오류는 0이다. TypeScript 제품 소스는 기준과 같고 이번 표는 JavaScript 정책의 분기·경계·연산자 변형이다. 이전 49개와 합쳐 최신 대상의 수치로 보고하지 않는다.

| 변형                     | 파일·바꾼 것                                                                                                                         | 잡은 테스트              | 결과 |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------ | ---- |
| policy-file-filter       | ds-overflow.mjs: !files.includes(file) → files.includes(file)                                                                        | ds19-policy.test.mjs     | 검출 |
| overflow-prefix          | ds-overflow.mjs: overflow(?:-[xy])?-.+ → neverflow(?:-[xy])?-.+                                                                      | ds19-policy.test.mjs     | 검출 |
| truncate                 | ds-overflow.mjs: &#124;truncate&#124; → &#124;truncation&#124;                                                                       | ds19-policy.test.mjs     | 검출 |
| scroll-auto              | ds-overflow.mjs: (?:auto&#124;scroll&#124;overlay) → (?:automatic&#124;scroll&#124;overlay)                                          | ds19-policy.test.mjs     | 검출 |
| scroll-scroll            | ds-overflow.mjs: (?:auto&#124;scroll&#124;overlay) → (?:auto&#124;scrolling&#124;overlay)                                            | ds19-policy.test.mjs     | 검출 |
| absolute                 | ds-overflow.mjs: absolute&#124;fixed → relative&#124;fixed                                                                           | ds19-policy.test.mjs     | 검출 |
| fixed                    | ds-overflow.mjs: absolute&#124;fixed → absolute&#124;relative                                                                        | ds19-policy.test.mjs     | 검출 |
| sticky                   | ds-overflow.mjs: &#124;sticky&#124; → &#124;relative&#124;                                                                           | ds19-policy.test.mjs     | 검출 |
| length-height            | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:q&#124;w&#124;size)                                                                     | ds19-policy.test.mjs     | 검출 |
| length-width             | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:h&#124;q&#124;size)                                                                     | ds19-policy.test.mjs     | 검출 |
| length-size              | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:h&#124;w&#124;sizing)                                                                   | ds19-policy.test.mjs     | 검출 |
| viewport-screen          | ds-overflow.mjs: (?:screen&#124;[sdl]?v → (?:screens&#124;[sdl]?v                                                                    | ds19-policy.test.mjs     | 검출 |
| nowrap                   | ds-overflow.mjs: (?:nowrap&#124;pre) → (?:neverwrap&#124;pre)                                                                        | ds19-policy.test.mjs     | 검출 |
| negative-margin          | ds-overflow.mjs: -m[xytrblse]? → -n[xytrblse]?                                                                                       | ds19-policy.test.mjs     | 검출 |
| translate                | ds-overflow.mjs: (?:translate&#124;scale) → (?:translation&#124;scale)                                                               | ds19-policy.test.mjs     | 검출 |
| scale                    | ds-overflow.mjs: (?:translate&#124;scale) → (?:translate&#124;scaling)                                                               | ds19-policy.test.mjs     | 검출 |
| chart-table-exception    | ds-overflow.mjs: id === 20 && CHART_TABLE_FILES.includes(file) → false                                                               | ds19-policy.test.mjs     | 검출 |
| exact-length-file        | ds-overflow.mjs: path === file → path !== file                                                                                       | ds19-policy.test.mjs     | 검출 |
| empty-width-token        | ds-overflow.mjs: ["max-w-dialog-width"] → ["max-w-prose-max"]                                                                        | ds19-policy.test.mjs     | 검출 |
| skeleton-extra-token     | ds-overflow.mjs: "skeleton-line", → "skeleton-line", "h-control-height",                                                             | ds19-policy.test.mjs     | 검출 |
| native-select            | ds-overflow-elements.mjs: key === "multiple" → key === "multitude"                                                                   | ds19-native.test.mjs     | 검출 |
| logical-size             | ds-overflow-utilities.mjs: width&#124;height&#124;inline-size&#124;block-size → width&#124;height&#124;inline-length&#124;block-size | ds19-native.test.mjs     | 검출 |
| nowrap-declaration       | ds-overflow-utilities.mjs: "text-wrap-mode" → "text-wrapping-mode"                                                                   | ds19-native.test.mjs     | 검출 |
| clipping                 | ds-overflow-utilities.mjs: property === "clip-path" → false                                                                          | ds19-native.test.mjs     | 검출 |
| matrix-transform         | ds-overflow-utilities.mjs: translate&#124;scale&#124;matrix → translate&#124;scale&#124;mtrx                                         | ds19-native.test.mjs     | 검출 |
| primitive-boundary       | ds-overflow-primitives.mjs: !owned(entry) &#124;&#124; node.type !== "ImportDeclaration" → false                                     | ds19-primitives.test.mjs | 검출 |
| primitive-root           | ds-overflow-primitives.mjs: source === "@base-ui/react" → false                                                                      | ds19-primitives.test.mjs | 검출 |
| primitive-dynamic        | ds-overflow-primitives.mjs: node.source.value.startsWith("@base-ui/react") → false                                                   | ds19-primitives.test.mjs | 검출 |
| tooltip-import-alias     | ds-overflow-primitives.mjs: node.specifiers[0].local.name === "Tooltip" → true                                                       | ds19-primitives.test.mjs | 검출 |
| tooltip-value-extraction | ds-overflow-primitives.mjs: node.name === "Tooltip" → false                                                                          | ds19-primitives.test.mjs | 검출 |
| quoted-root-import       | ds-overflow-primitives.mjs: specifier.imported?.name ?? specifier.imported?.value → specifier.imported?.name                         | ds19-primitives.test.mjs | 검출 |
| tooltip-count            | ds-overflow-primitives.mjs: ++positioners > 1 → ++positioners > 2                                                                    | ds19-primitives.test.mjs | 검출 |

## 3차 수정 최종 손 변형

3차 수정의 현재 소스를 OS 임시 복사본에 옮긴 뒤 policy·native·primitives·review-three 원본 테스트 네 파일의 통과를 확인했다. 아래 40개 모두 문법 검사에 통과하고 지정 테스트의 AssertionError로 검출됐다. 생존·실행 오류 0이며 이전 대상의 변형 수와 합산하지 않는다.

| 변형                     | 파일·바꾼 것                                                                                                                              | 잡은 테스트                | 결과 |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ---- |
| policy-file-filter       | ds-overflow.mjs: !files.includes(file) → files.includes(file)                                                                             | ds19-policy.test.mjs       | 검출 |
| overflow-prefix          | ds-overflow.mjs: overflow(?:-[xy])?-.+ → neverflow(?:-[xy])?-.+                                                                           | ds19-policy.test.mjs       | 검출 |
| truncate                 | ds-overflow.mjs: &#124;truncate&#124; → &#124;truncation&#124;                                                                            | ds19-policy.test.mjs       | 검출 |
| scroll-auto              | ds-overflow.mjs: (?:auto&#124;scroll&#124;overlay) → (?:automatic&#124;scroll&#124;overlay)                                               | ds19-policy.test.mjs       | 검출 |
| scroll-scroll            | ds-overflow.mjs: (?:auto&#124;scroll&#124;overlay) → (?:auto&#124;scrolling&#124;overlay)                                                 | ds19-policy.test.mjs       | 검출 |
| absolute                 | ds-overflow.mjs: absolute&#124;fixed → relative&#124;fixed                                                                                | ds19-policy.test.mjs       | 검출 |
| fixed                    | ds-overflow.mjs: absolute&#124;fixed → absolute&#124;relative                                                                             | ds19-policy.test.mjs       | 검출 |
| sticky                   | ds-overflow.mjs: &#124;sticky&#124; → &#124;relative&#124;                                                                                | ds19-policy.test.mjs       | 검출 |
| length-height            | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:q&#124;w&#124;size)                                                                          | ds19-policy.test.mjs       | 검출 |
| length-width             | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:h&#124;q&#124;size)                                                                          | ds19-policy.test.mjs       | 검출 |
| length-size              | ds-overflow.mjs: (?:h&#124;w&#124;size) → (?:h&#124;w&#124;sizing)                                                                        | ds19-policy.test.mjs       | 검출 |
| viewport-screen          | ds-overflow.mjs: (?:screen&#124;[sdl]?v → (?:screens&#124;[sdl]?v                                                                         | ds19-policy.test.mjs       | 검출 |
| nowrap                   | ds-overflow.mjs: (?:nowrap&#124;pre) → (?:neverwrap&#124;pre)                                                                             | ds19-policy.test.mjs       | 검출 |
| negative-margin          | ds-overflow.mjs: -m[xytrblse]? → -n[xytrblse]?                                                                                            | ds19-policy.test.mjs       | 검출 |
| translate                | ds-overflow.mjs: (?:translate&#124;scale) → (?:translation&#124;scale)                                                                    | ds19-policy.test.mjs       | 검출 |
| scale                    | ds-overflow.mjs: (?:translate&#124;scale) → (?:translate&#124;scaling)                                                                    | ds19-policy.test.mjs       | 검출 |
| chart-table-exception    | ds-overflow.mjs: id === 20 && CHART_TABLE_FILES.includes(file) → false                                                                    | ds19-policy.test.mjs       | 검출 |
| exact-length-file        | ds-overflow.mjs: path === file → path !== file                                                                                            | ds19-policy.test.mjs       | 검출 |
| empty-width-token        | ds-overflow.mjs: ["max-w-dialog-width"] → ["max-w-prose-max"]                                                                             | ds19-policy.test.mjs       | 검출 |
| skeleton-extra-token     | ds-overflow.mjs: "skeleton-line", → "skeleton-line", "h-control-height",                                                                  | ds19-policy.test.mjs       | 검출 |
| native-select            | ds-overflow-elements.mjs: key === "multiple" → key === "multitude"                                                                        | ds19-native.test.mjs       | 검출 |
| logical-size             | ds-overflow-utilities.mjs: width&#124;height&#124;inline-size&#124;block-size → width&#124;height&#124;inline-length&#124;block-size      | ds19-native.test.mjs       | 검출 |
| nowrap-declaration       | ds-overflow-utilities.mjs: "text-wrap-mode" → "text-wrapping-mode"                                                                        | ds19-native.test.mjs       | 검출 |
| clipping                 | ds-overflow-utilities.mjs: property === "clip-path" → false                                                                               | ds19-native.test.mjs       | 검출 |
| matrix-transform         | ds-overflow-utilities.mjs: translate&#124;scale&#124;matrix → translate&#124;scale&#124;mtrx                                              | ds19-native.test.mjs       | 검출 |
| primitive-boundary       | ds-overflow-primitives.mjs: !owned(entry) &#124;&#124; node.type !== "ImportDeclaration" → false                                          | ds19-primitives.test.mjs   | 검출 |
| primitive-root           | ds-overflow-primitives.mjs: source === "@base-ui/react" → false                                                                           | ds19-primitives.test.mjs   | 검출 |
| primitive-dynamic        | ds-overflow-primitives.mjs: node.source.value.startsWith("@base-ui/react") → false                                                        | ds19-primitives.test.mjs   | 검출 |
| tooltip-import-alias     | ds-overflow-primitives.mjs: node.specifiers[0].local.name === "Tooltip" → true                                                            | ds19-primitives.test.mjs   | 검출 |
| tooltip-value-extraction | ds-overflow-primitives.mjs: node.name === "Tooltip" → false                                                                               | ds19-primitives.test.mjs   | 검출 |
| quoted-root-import       | ds-overflow-primitives.mjs: specifier.imported?.name ?? specifier.imported?.value → specifier.imported?.name                              | ds19-primitives.test.mjs   | 검출 |
| tooltip-count            | ds-overflow-primitives.mjs: ++positioners > 1 → ++positioners > 2                                                                         | ds19-primitives.test.mjs   | 검출 |
| lib-package-boundary     | boundaries.mjs: "@base-ui/react": ["src/shared/ds/**"], →                                                                                 | ds19-review-three.test.mjs | 검출 |
| lib-package-widening     | boundaries.mjs: "@base-ui/react": ["src/shared/ds/**"] → "@base-ui/react": ["src/shared/{ds,lib}/**"]                                     | ds19-review-three.test.mjs | 검출 |
| unowned-number-field     | ds-overflow-primitives.mjs: ["NumberField", "number-field", null], →                                                                      | ds19-review-three.test.mjs | 검출 |
| unowned-slider           | ds-overflow-primitives.mjs: ["Slider", "slider", null], →                                                                                 | ds19-review-three.test.mjs | 검출 |
| unowned-drawer           | ds-overflow-primitives.mjs: ["Drawer", "drawer", null], →                                                                                 | ds19-review-three.test.mjs | 검출 |
| popover-wrapper          | ds-overflow-elements.mjs: ["popover", "popoverTarget", "popoverTargetAction"].includes(key) → false                                       | ds19-review-three.test.mjs | 검출 |
| card-extra-size          | ds-overflow.mjs: ["min-h-toolbar-height"] → ["min-h-toolbar-height", "h-chart-height"]                                                    | ds19-policy.test.mjs       | 검출 |
| icon-extra-size          | ds-overflow.mjs: ["size-icon-sm", "size-icon-md", "size-icon-lg"] → ["size-icon-sm", "size-icon-md", "size-icon-lg", "max-w-content-max"] | ds19-policy.test.mjs       | 검출 |
