# DS-19: 스크롤·넘침 정적 규칙

- 상태: 초안
- 기준: 994c25a191e0ab08e2c8e9f1f26cfa2de7be3f4d

## 결정

기존 better-tailwindcss/no-restricted-classes에 harness 4.4 #18~24의 파일별 제한을 더한다. 화면의 ScrollArea 가져오기와 다시 내보내기는 기존 ESLint no-restricted-imports로 제한한다. 새 검사 도구와 의존성은 없다. CSS는 기존 ds-style의 생성된 theme 바이트 일치와 별도 수기 CSS 금지를 유지한다. 임의 CSS 선언은 모두 거부하므로 속성별 stylelint 예외보다 엄격하다. tokens.css는 기존 토큰 재생성 검사가 맡는다.

메인 결정: 현재 긴 글 최대 폭의 책임 위치는 shared/ds/Markdown.tsx다. architecture 5.1이 긴 글을 prose-max에서 멈추도록 요구하고 현재 Markdown이 그 계약을 지므로 #21에서 prose-max 토큰 하나만 예외로 둔다. markdown-content의 정확한 생성 선언과 max-w-prose-max만 허용하며, 임의 폭·다른 max-width 토큰·w-prose-max·h-prose-max는 거부한다. Markdown의 제품 소스와 폭 제한은 변경하지 않는다. 이는 메인의 설계 결정에 따른 예외이며 기존 코드를 통과시키기 위한 임의 확대가 아니다.

허용 목록은 ds-overflow.mjs의 정확한 파일 경로다. 컨트롤에는 버튼 변형 파일·아이콘·입력·선택·토글·펼침을, 머리줄에는 Toolbar·StatusBar·Card를 등록한다. 차트의 내부 표는 Table과 같은 위치 지정 범주로 취급한다. ScrollArea만 스크롤 생성 클래스를 허용한다. 음수 여백·translate·scale의 파일 허용 목록은 비어 있다. 없는 파일 이름은 미래 DS-11의 문서 지정 위치이며 폴더 예외로 풀지 않는다. Text의 말줄임 허용은 별도의 nowrap 클래스까지 허용하지 않는다. 여러 줄 말줄임은 기존 DS-02의 닫힌 토큰 집합에서 계속 거부한다.

사용자 정의 유틸리티도 제한 대상이다. dialog-overlay는 #20, chat-input·markdown-content·skeleton-line은 #21에 포함한다. sr-only의 위치 지정은 기존 차트 대체 표의 정확한 파일에 한정한다. 의미별 클래스 표는 작성 파일과 소비 파일 양쪽에서 토큰 집합·가격색·넘침 제한을 확인한다. 일반 변수·템플릿·계산된 cn 인자와 clsx 호출은 기존 static-classes가 거부한다. 인라인 끄기도 기존 no-restricted-disable 등록으로 거부한다.

## 기존 코드 조정

EmptyState의 직접 max-w-dialog-width를 제거한다. Skeleton에 길이 예외를 추가하지 않는다. 차트 자리 표시의 세 높이 렌더링은 기존 ChartFrame.tsx의 내부 ChartPlaceholder로 옮기며 공개 index에는 추가하지 않는다. 텍스트 뼈대의 skeleton-line 높이는 pt-space-1 + pb-space-2로 바꾼다. 원래 text-size-sm과 같은 12px이며 빈 장식 요소의 자식 수·aria-hidden을 보존한다. 컨트롤·머리줄 범주와 메인이 정한 Markdown 외에 임의 길이 허용 파일을 추가하지 않는다.

## 검증

시작 frozen 설치와 lefthook 설치는 성공했다. 프로젝트 신뢰·세션 훅 신뢰와 현재 세션 적용은 확인할 수 없다. 지시문의 예외에 따라 진행하며 Git 커밋 훅을 사용한다. 실제 키·계좌·API 및 홈 설정은 다루지 않았다.

harness/eslint-violations/ds19-overflow.json은 #18~25의 위반·통과 대조군이다. DS19 policy 테스트는 실제 설정과 번호별 단독 제한을 각각 실행해 기존 토큰 금지에 가려진 누락도 검출한다. CSS 경계 테스트는 각 속성과 화면 단위를 위반 자료로 검사한다. 별도 타입 인식 앱 테스트는 템플릿·도우미·변수·로컬 및 가져온 의미별 클래스 표·인라인 끄기를 검증하고, 가져온 표 반례는 실제 tsc 통과 뒤 ESLint 거부를 확인한다.

DS04 컴포넌트 회귀 12개와 새 우회 검사 1개가 통과했다. 시각 검사는 현재 등록된 Button·Card·Money·TextField의 라이트·다크 8쌍에서 바이트 일치를 확인했다. 이 기준에 없는 EmptyState·Skeleton 전체의 시각 동등성을 증명하지는 않는다. 제품 조정 뒤 최종 결과와 독립 리뷰는 리뷰 기록에 추가한다.

## 승인된 기존 테스트 변경과 보호 파일

| 파일                                              | 항목                          | 전 → 후                                                                                                             | 승인 |
| ------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------- | ---- |
| apps/desktop/tests/eslint-config.test.mjs         | 끌 수 없는 규칙 고정 목록     | 기존 항목 유지 + no-restricted-imports 1개                                                                          | (c)  |
| apps/desktop/tests/ds-semantic-mutations.test.mjs | 변형 모듈 import 준비         | 기존 경로 해석 유지 + 새 ds-overflow.mjs의 절대 URL 해석                                                            | (d)  |
| apps/desktop/tests/ds04-style.test.mjs            | DS04 style 통과 자료의 준비   | Probe 한 파일에 모든 클래스를 배치 → Dialog·차트 파일 등 해당 허용 위치로 나누어 검사. 기존 단언과 위반 자료는 유지 | (d)  |
| harness/ds-style/cases.json                       | review-ref.tsx 고정 진단 목록 | 기존 세 진단 → 기존 세 진단 + no-restricted-classes                                                                 | (c)  |

ESLint 설정·규칙 파일은 이번 정적 제한 등록에 필요하다. 고정 자료와 Node 테스트는 규칙별 증인 등록이다. Codex 설정·sandbox fragment·Claude 해시는 기존 생성기로 갱신한다. 기존 금지 삭제와 검사 끄기 추가는 없다.

## 인계

- DS-11: 레이아웃 컴포넌트(Page 등)가 들어오면 긴 글 폭 책임을 그쪽으로 옮길지 재검토한다. 현재 책임은 Markdown.tsx이며 prose-max 하나만 허용한다.
- DS-20: 실제 문서·요소 넘침, 잘림·스크롤 중첩·창 밖 요소·조작 요소 겹침을 렌더링으로 검증한다. 정적 검사가 이 실측을 대신하지 않는다.
- DS-12: 현재 4개 컴포넌트의 시각 기준을 전체 공개 목록으로 확대한다. EmptyState·Skeleton도 포함해야 한다.

## 손 변형

최종 실행 결과를 아래 표에 기록한다. 문법 검사에 실패하거나 실행 오류가 난 변형은 검출 수에 넣지 않는다.

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
