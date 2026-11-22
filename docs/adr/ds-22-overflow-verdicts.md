# DS-22 부하 이야기 넘침 판정

- 상태: 초안

## 배경

DS-20의 고정 Chromium 실행에서 1,034개 테마 이야기 중 174개 이야기의 544조건에 판정이 있었다. 한 조건에 여러 판정이 겹칠 수 있다. 이번 변경은 기존 이야기와 부하 자료를 줄이지 않는다.

## 결정

- 메인 결정에 따라 편집 가능한 한 줄 텍스트 입력의 가로 내용 스크롤과 편집 가능한 textarea의 실제로 스크롤 가능한 세로 내용 초과만 해당 축의 내용 크기 판정에서 면제한다. 읽기 전용·비활성 입력은 면제하지 않는다. 텍스트 이외 입력 타입, textarea 가로 초과, 스크롤할 수 없는 textarea 세로 초과, 컨트롤 상자와 주변 문구의 넘침은 계속 검사한다. ADR 0129의 의도된 입력 내부 스크롤 인계를 해소한다.
- IBM Plex Sans KR의 텍스트 노드에서 실제 줄 사각형이 첫 줄 위와 마지막 줄 아래에 각각 최대 1px만 뻗고 글줄과 모든 관련 자르기 경계가 창 안에 있는 경우, 이 글꼴 지표만 내용 크기 오탐에서 제외한다. 여러 줄도 각 사각형을 검사한다. 다른 축의 초과, 자르는 조상, 1px 실제 자식 넘침과 가상 요소는 면제하지 않는다. body 스크롤 잠금이 뷰포트에 전파되는 경우의 고정 대화 상자는 실제 뷰포트 경계로 판정한다.
- 화면 읽기용으로 완전히 숨긴 표는 정확한 표·래퍼 구조와 계산된 크기·자르기 스타일을 모두 확인하고 해당 표의 내용 크기와 잘림만 제외한다. 나머지 판정은 그대로 실행한다. 보이는 가짜 표식, 부분 clip-path, 표 밖의 실제 넘침과 숨긴 비표 요소는 계속 보고한다.
- EmptyState, TextField, Checkbox, Switch, Heading, Select의 텍스트는 기존 status-sentence 유틸리티로 줄을 바꾼다. 공개 속성이나 토큰을 바꾸지 않고 긴 글을 자르지 않는다. 버튼 높이, 숫자 줄바꿈과 표·차트의 스크롤 배치처럼 겉모습의 결정이 필요한 항목은 고치지 않는다.
- just ds-check는 카탈로그·브라우저 타입·보류 대조 회귀와 고정 브라우저 전체 실행을 차례로 수행한다. 보류 표의 이야기 ID, 폭, 글자 크기와 판정 종류·요소가 관찰과 정확히 일치해야만 성공한다. 보류가 늘거나 줄거나 판정이 바뀌면 실패한다. 보류 일치는 잔여 UI 문제가 없다는 뜻이 아니다.

## 측정과 분류

| 단계                              | 판정된 이야기·조건   | 의미                                                                                |
| --------------------------------- | -------------------- | ----------------------------------------------------------------------------------- |
| DS-20 기준 재실행                 | 174개 이야기·544조건 | 종류별 조건은 허가 없는 넘침 544, 문서 스크롤 104, 잘림 64로 서로 겹친다            |
| 입력 내용 스크롤 수정             | 528조건              | ChatInput 16조건이 사라졌다. TextField의 입력 판정은 다른 판정과 겹친 조건도 있었다 |
| 첫 글꼴·줄바꿈 수정               | 310조건              | 여러 판정이 한 조건에 겹치므로 원인별 감소 수를 단순 합산하지 않는다                |
| 표·여러 줄 글꼴·대화 상자 수정 후 | 106개 이야기·224조건 | 고정 브라우저 전체 실행에서 보류 표와 정확히 일치한다                               |

## 보류 표

다음 표의 {light,dark}는 두 테마를 각각 뜻한다. 각 이야기의 조건과 판정 종류·대상 요소의 정확한 목록은 harness/ds-overflow/pending.json에 있으며 실행기가 양방향으로 대조한다. 이 항목은 해소된 것이 아니라 메인 결정 대기다.

| 이야기                | ID                                    | 조건(폭/글자 크기)                                 | 이유                                                                                             |
| --------------------- | ------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 레이아웃/Card         | 레이아웃-card--{light,dark}-0         | narrow/100%, narrow/125%                           | 카드의 줄바꿈 없는 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 카드 배치 결정이 필요하다.     |
| 레이아웃/Card         | 레이아웃-card--{light,dark}-5         | narrow/100%, narrow/125%                           | 긴 카드 동작 버튼 문구가 고정 높이를 넘어 버튼 높이와 배치 결정이 필요하다.                      |
| 기본/Button           | 기본-button--{light,dark}-12          | narrow/100%, narrow/125%                           | 긴 버튼 문구가 기존 고정 높이를 넘어 버튼 높이와 배치의 결정이 필요하다.                         |
| 기본/TextField        | 기본-textfield--{light,dark}-4        | narrow/100%, narrow/125%, normal/100%, normal/125% | 비밀 입력의 붙여넣기 버튼이 고정 높이 또는 좁은 폭을 넘어 입력과 버튼의 배치 결정이 필요하다.    |
| 기본/TextField        | 기본-textfield--{light,dark}-5        | narrow/100%, narrow/125%, normal/100%, normal/125% | 비밀 입력의 붙여넣기 버튼이 고정 높이 또는 좁은 폭을 넘어 입력과 버튼의 배치 결정이 필요하다.    |
| 피드백/Table          | 피드백-table--{light,dark}-0          | narrow/100%, narrow/125%                           | 표의 너비 또는 높이가 최소 영역을 넘어 표의 스크롤과 배치 결정이 필요하다.                       |
| 피드백/Table          | 피드백-table--{light,dark}-4          | narrow/100%, narrow/125%, normal/100%, normal/125% | 표의 너비 또는 높이가 최소 영역을 넘어 표의 스크롤과 배치 결정이 필요하다.                       |
| 숫자/Money            | 숫자-money--{light,dark}-0            | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/Money            | 숫자-money--{light,dark}-1            | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/Money            | 숫자-money--{light,dark}-2            | narrow/125%                                        | 줄바꿈을 금지한 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/Money            | 숫자-money--{light,dark}-3            | narrow/125%                                        | 줄바꿈을 금지한 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/Money            | 숫자-money--{light,dark}-6            | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 금액이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/Percent          | 숫자-percent--{light,dark}-4          | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 백분율이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.           |
| 숫자/Quantity         | 숫자-quantity--{light,dark}-1         | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 수량이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다.             |
| 숫자/PriceChange      | 숫자-pricechange--{light,dark}-0      | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 등락 금액과 비율이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다. |
| 숫자/PriceChange      | 숫자-pricechange--{light,dark}-1      | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 등락 금액과 비율이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다. |
| 숫자/PriceChange      | 숫자-pricechange--{light,dark}-2      | narrow/125%                                        | 줄바꿈을 금지한 등락 금액과 비율이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다. |
| 숫자/PriceChange      | 숫자-pricechange--{light,dark}-3      | narrow/100%, narrow/125%                           | 줄바꿈을 금지한 등락 금액과 비율이 최소 영역보다 넓어 가로 스크롤을 포함한 배치 결정이 필요하다. |
| 차트/LineChart        | 차트-linechart--{light,dark}-0        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/LineChart        | 차트-linechart--{light,dark}-1        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/LineChart        | 차트-linechart--{light,dark}-4        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/LineChart        | 차트-linechart--{light,dark}-6        | narrow/100%, narrow/125%                           | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/AreaChart        | 차트-areachart--{light,dark}-0        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/AreaChart        | 차트-areachart--{light,dark}-1        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/AreaChart        | 차트-areachart--{light,dark}-4        | narrow/125%                                        | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/AreaChart        | 차트-areachart--{light,dark}-6        | narrow/100%, narrow/125%                           | 차트와 축·안내가 최소 창 높이를 넘어 차트가 놓일 스크롤 영역과 배치 결정이 필요하다.             |
| 차트/DonutChart       | 차트-donutchart--{light,dark}-0       | narrow/100%, narrow/125%                           | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/DonutChart       | 차트-donutchart--{light,dark}-1       | narrow/125%                                        | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/DonutChart       | 차트-donutchart--{light,dark}-3       | narrow/125%                                        | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/DonutChart       | 차트-donutchart--{light,dark}-4       | narrow/100%, narrow/125%, normal/125%              | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/DonutChart       | 차트-donutchart--{light,dark}-6       | narrow/100%, narrow/125%                           | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/Treemap          | 차트-treemap--{light,dark}-0          | narrow/100%, narrow/125%                           | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/Treemap          | 차트-treemap--{light,dark}-1          | narrow/125%                                        | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/Treemap          | 차트-treemap--{light,dark}-3          | narrow/125%                                        | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/Treemap          | 차트-treemap--{light,dark}-4          | narrow/100%, narrow/125%, normal/125%              | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 차트/Treemap          | 차트-treemap--{light,dark}-6          | narrow/100%, narrow/125%                           | 차트와 대체 표가 최소 폭 또는 창 높이를 넘어 스크롤 영역과 배치 결정이 필요하다.                 |
| 콘텐츠/Markdown       | 콘텐츠-markdown--{light,dark}-2       | narrow/100%, narrow/125%, normal/100%, normal/125% | 긴 근거 본문이 최소 창 높이를 넘어 본문이 놓일 스크롤 영역과 배치 결정이 필요하다.               |
| 콘텐츠/Markdown       | 콘텐츠-markdown--{light,dark}-3       | narrow/100%, narrow/125%, normal/100%, normal/125% | 긴 근거 본문이 최소 창 높이를 넘어 본문이 놓일 스크롤 영역과 배치 결정이 필요하다.               |
| 기본/Checkbox         | 기본-checkbox--{light,dark}-2         | narrow/100%, narrow/125%, normal/100%, normal/125% | 선택된 체크 표시가 기존 고정 크기 안에 들지 않아 표시기 크기와 정렬 결정이 필요하다.             |
| 기본/Checkbox         | 기본-checkbox--{light,dark}-3         | narrow/100%, narrow/125%, normal/100%, normal/125% | 선택된 체크 표시가 기존 고정 크기 안에 들지 않아 표시기 크기와 정렬 결정이 필요하다.             |
| 기본/SegmentedControl | 기본-segmentedcontrol--{light,dark}-0 | narrow/100%, narrow/125%                           | 고정 높이의 선택 버튼과 긴 선택지가 좁은 폭 또는 높이를 넘어 배치 결정이 필요하다.               |
| 기본/SegmentedControl | 기본-segmentedcontrol--{light,dark}-1 | narrow/100%, narrow/125%                           | 고정 높이의 선택 버튼과 긴 선택지가 좁은 폭 또는 높이를 넘어 배치 결정이 필요하다.               |
| 기본/SegmentedControl | 기본-segmentedcontrol--{light,dark}-2 | narrow/100%, narrow/125%                           | 고정 높이의 선택 버튼과 긴 선택지가 좁은 폭 또는 높이를 넘어 배치 결정이 필요하다.               |
| 기본/SegmentedControl | 기본-segmentedcontrol--{light,dark}-3 | narrow/100%, narrow/125%, normal/100%, normal/125% | 고정 높이의 선택 버튼과 긴 선택지가 좁은 폭 또는 높이를 넘어 배치 결정이 필요하다.               |
| 기본/Disclosure       | 기본-disclosure--{light,dark}-2       | narrow/100%, narrow/125%                           | 긴 제목이 펼침 버튼의 고정 높이를 넘어 높이와 제목 배치의 결정이 필요하다.                       |
| 피드백/Badge          | 피드백-badge--{light,dark}-6          | narrow/100%, narrow/125%                           | 줄바꿈하지 않는 긴 배지가 최소 영역보다 넓어 배지 표시 정책의 결정이 필요하다.                   |
| 피드백/StatusBar      | 피드백-statusbar--{light,dark}-0      | narrow/100%, narrow/125%                           | 좁은 폭에서 상태 문구와 동작 버튼의 공간이 부족해 배치 결정이 필요하다.                          |
| 피드백/StatusBar      | 피드백-statusbar--{light,dark}-2      | narrow/100%, narrow/125%                           | 좁은 폭에서 상태 문구와 동작 버튼의 공간이 부족해 배치 결정이 필요하다.                          |
| 피드백/StatusBar      | 피드백-statusbar--{light,dark}-4      | narrow/100%, narrow/125%                           | 좁은 폭에서 상태 문구와 동작 버튼의 공간이 부족해 배치 결정이 필요하다.                          |
| 피드백/StatusBar      | 피드백-statusbar--{light,dark}-10     | narrow/100%, narrow/125%                           | 좁은 폭에서 상태 문구와 동작 버튼의 공간이 부족해 배치 결정이 필요하다.                          |
| 피드백/Dialog         | 피드백-dialog--{light,dark}-4         | narrow/100%, narrow/125%, normal/100%, normal/125% | 긴 대화 상자 제목과 동작 문구가 기존 머리글과 버튼 높이를 넘어 배치 결정이 필요하다.             |
| 피드백/EmptyState     | 피드백-emptystate--{light,dark}-0     | narrow/125%                                        | 좁은 영역의 빈 상태 설명과 여백이 최소 창 높이를 넘어 배치와 스크롤 결정이 필요하다.             |
| 피드백/EmptyState     | 피드백-emptystate--{light,dark}-5     | narrow/100%, narrow/125%                           | 긴 빈 상태 동작 버튼 문구가 고정 높이를 넘고 내용이 창보다 길어 배치 결정이 필요하다.            |

## 인계

- DS-11: 표·차트·긴 본문과 줄바꿈 없는 숫자가 놓일 스크롤 영역 및 좁은 폭의 배치를 결정한다.
- DS-12: 정식 공개 목록 잠금과 화면 상태 검사에 ds-check를 연결할 때 보류 표를 그대로 통과 판정으로 오해하지 않는다.
- 메인: 고정 높이 버튼, 체크 표시기, 상태 띠, 배지 및 대화 상자의 긴 문구 처리와 보류 표의 각 표시 결정을 검토한다.

## 검증

고정 브라우저 회귀에는 각 입력 축의 허용·비허용, 레이블·도움말·오류 문구, 실제 500행 접근성 표, 거짓 숨김 표식, 실제 1px 넘침, 스크롤 잠금된 대화 상자와 paint 자르기 반례가 있다. 입력 자체 스크롤의 독립 사례 8건, 실제 두 차트의 숨긴 표 2건, 여러 줄 글꼴과 대화 상자의 글꼴 사례, 긴 제목·선택 문구 사례는 수정 전 실패를 확인했다. ds-check는 406초에 종료 0이었고 전체 1,034개 이야기의 224조건이 보류 표와 일치했다. 이 실행의 브라우저 예제는 118개였다. 이후 비활성 입력과 표시된 textarea 반례를 더하고 EmptyState의 기존 클래스 단언을 유지하도록 수정한 뒤, 별도 실행한 브라우저 예제 120개와 DS-19 기존 테스트가 통과했다. EmptyState가 포함된 840번 배치의 34조건도 보류 표와 다시 일치했다.

## 승인된 기존 테스트 단언

메인이 ds20-catalog.browser.ts의 두 단언 변경을 명시적으로 승인했다. 두 테스트의 값 일치, 스크롤 크기 비교, 일치 건수와 나머지 기존 단언은 유지했고 실제 레이블·도움말 넘침 단언을 추가했다.

| 테스트                                                                  | 전                                                         | 후            |
| ----------------------------------------------------------------------- | ---------------------------------------------------------- | ------------- |
| measures the real ChatInput draft and keeps its nested exception narrow | textarea 자체의 세로 스크롤을 unauthorized-overflow로 보고 | 보고하지 않음 |
| reports native TextField overflow without assigning a viewport marker   | input 자체의 가로 스크롤을 unauthorized-overflow로 보고    | 보고하지 않음 |

파일의 변경 전 SHA-256은 4986090e1f8158163d8df3883e73ea97a5d106f75bf785324dda0c6a482c7ca0, 변경 후는 3b4b65318ac228f1337b5a08e36a7939bee822073f2df1a72238e264bbcb86e3이다. H-55의 test-diff는 Rust 파일만 검사하고 승인 경로에도 rs 확장자만 허용하므로 이 TypeScript 파일의 정확한 test-diff 지적 문자열은 존재하지 않으며 assertion_change 행을 등록할 수 없다. 메인의 명시적 승인과 H-55 기계 승인 경로는 별개다.

## 손 변형

1~28번은 mktemp -d 복사본에서 TypeScript 컴파일을 통과한 뒤 고정 Chromium의 해당 회귀를 실제로 실행했다. 기준 복사본의 전체 예제가 먼저 통과했다. 29~37번은 별도 복사본에서 node --check와 보류 대조 Node 테스트를 실제로 실행했다. 병렬 변이는 사용하지 않았다.

| 번호 | 바꾼 것                             | 잡은 테스트                                                                                     | 결과 |
| ---- | ----------------------------------- | ----------------------------------------------------------------------------------------------- | ---- |
| 1    | input text 타입 제외                | overflow measurements accepts the editable text input's own horizontal scrolling                | 검출 |
| 2    | input search 타입 제외              | overflow measurements accepts the editable search input's own horizontal scrolling              | 검출 |
| 3    | input tel 타입 제외                 | overflow measurements accepts the editable tel input's own horizontal scrolling                 | 검출 |
| 4    | input url 타입 제외                 | overflow measurements accepts the editable url input's own horizontal scrolling                 | 검출 |
| 5    | input email 타입 제외               | overflow measurements accepts the editable email input's own horizontal scrolling               | 검출 |
| 6    | input password 타입 제외            | overflow measurements accepts the editable password input's own horizontal scrolling            | 검출 |
| 7    | 읽기 전용 input 제한 제거           | overflow measurements still detects a readonly input's overflowing content                      | 검출 |
| 8    | 비활성 input 제한 제거              | overflow measurements still detects a disabled input's overflowing content                      | 검출 |
| 9    | textarea auto 스크롤 제외           | overflow measurements accepts an editable textarea's own vertical scrolling with auto           | 검출 |
| 10   | textarea scroll 스크롤 제외         | overflow measurements accepts an editable textarea's own vertical scrolling with scroll         | 검출 |
| 11   | textarea 뷰포트 표식 면제 확대      | overflow measurements still detects horizontal textarea overflow even with a viewport marker    | 검출 |
| 12   | 숨긴 표 position을 fixed로 변경     | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 13   | 숨긴 표 overflow-x를 visible로 변경 | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 14   | 숨긴 표 overflow-y를 visible로 변경 | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 15   | 숨긴 표 clip 사각형 변경            | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 16   | 숨긴 표 clip-path 비율 변경         | overflow measurements keeps the full hidden LineChart table without false clipping verdicts     | 검출 |
| 17   | 숨긴 래퍼 너비 경계 변경            | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 18   | 숨긴 래퍼 높이 경계 변경            | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 19   | 숨긴 래퍼 선택자 변경               | overflow measurements does not report the clipped content of a visually hidden accessible table | 검출 |
| 20   | 글꼴 이름 변경                      | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 21   | 글꼴 1px 경계 변경                  | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 22   | 글꼴 텍스트 노드 개수 변경          | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 23   | 글꼴 상단 경계 변경                 | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 24   | 글꼴 하단 경계 변경                 | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 25   | 글꼴 첫 줄 방향 반전                | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 26   | 글꼴 마지막 줄 방향 반전            | overflow measurements accepts the visible one-pixel font metric for an unconstrained line       | 검출 |
| 27   | body 뷰포트 전파 제거               | overflow measurements accepts a visible font metric in a fixed dialog with body scroll locked   | 검출 |
| 28   | 조상 자르기 검사 제거               | overflow measurements still detects the font metric when an ancestor clips the text             | 검출 |
| 29   | 새 판정 대조 제거                   | only the exact pending conditions and findings pass                                             | 검출 |
| 30   | 사라진 보류 대조 제거               | only the exact pending conditions and findings pass                                             | 검출 |
| 31   | 보류 이유 확인 제거                 | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
| 32   | 조건 중복 검사 제거                 | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
| 33   | 요소 이름 대조 제거                 | only the exact pending conditions and findings pass                                             | 검출 |
| 34   | 판정 종류 검증 제거                 | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
| 35   | 폭 조건 검증 제거                   | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
| 36   | 글자 크기 조건 검증 제거            | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
| 37   | 빈 판정 허용                        | missing reasons, invalid conditions, and duplicate conditions fail                              | 검출 |
