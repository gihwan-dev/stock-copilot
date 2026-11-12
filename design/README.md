개인용 주식 AI 비서 데스크톱 앱(Tauri + React)의 디자인 시스템이에요. 숫자를 빠르고 정확하게 읽히게 하는 것이 첫 번째 목표이고, 사용자가 느끼기에 아주 단순해야 해요. AI는 종목별 확률과 근거를 내는 조수이고, 신호와 계산은 앱의 코드가 해요.

## 원칙

1. **숫자가 주인공이에요.** 장식보다 숫자의 정렬, 자릿수, 기준 시각이 먼저예요. 모든 숫자는 세로 정렬이 맞아요(IBM Plex Sans KR의 숫자는 기본이 고정폭이에요).
2. **빨강·파랑은 가격 방향과 신호 카드의 매수·매도 버튼에만 써요.** `up`(빨강)은 상승·이익, `down`(파랑)은 하락·손실이에요. 오류, 일반 버튼, 차트 계열, 확률 막대에는 쓰지 않아요.
3. **계산과 반올림은 Rust 코어, 화면은 표시만 해요.** 평가금액·손익·수익률·비중·환산·쏠림·적중률은 모두 코어가 계산하고 표시 자릿수까지 반올림해서 소수 문자열로 넘겨요. 컴포넌트는 부호, 쉼표, 단위만 붙여요. 잘못된 값은 0이 아니라 `—`로 보여요.
4. **AI는 확률과 근거만 내고, 신호는 앱 코드가 띄워요.** 이 앱의 확률은 “기간 안에 **손절가에 닿기 전에 목표가에 먼저 닿을** 확률”(1–99% 정수)이에요. 오를 확률이 아니에요. 목표가·손절가·기간은 코드가 정하고, AI는 그 조건에서의 확률과 근거·출처만 내요. 앱 코드가 그 확률을 적중 기록으로 조정하고, 코드가 계산한 매수·매도 임계값과 비교해 신호를 만들어요. 수량·금액·수수료·비중도 코드가 계산해 SignalCard로 보여줘요. AI는 주문을 제안하지 않아요. 판정 보류 같은 판단도 코어가 내리고 화면은 보여주기만 해요.
5. **주문은 신호 카드의 버튼과 macOS 확인 창으로만 나가요.** 매수·매도 버튼은 SignalCard에만 있어요. 누르면 앱 밖의 macOS 확인 창(Touch ID)이 뜨고, 승인해야 코어가 주문을 넣고 기록해요. 앱 안에 따로 “최종 확인 화면”은 없어요. 주문 정정·취소 UI는 만들지 않아요. 토스증권 앱에서 해요.
6. **내부 장치 이름을 화면에 드러내지 않아요.** MCP 도구 이름, API 오류 코드, 내부 모듈 이름은 로그에만 남기고 화면에는 사용자 말로 써요. 예: “보유 종목을 확인하는 중”, “토스에 연결하지 못했어요. 허용 IP가 바뀌었을 수 있어요.”

## 화면 구성

화면은 **하나**예요. 탭이나 화면 전환 메뉴가 없어요.

- **AppShell**: 왼쪽 종목 목록(보유·관심 종목과 오늘 등락) · 가운데 본문 · 오른쪽 AI 채팅.
- **본문 기본은 “현황”**: 위에 현황(총평가, 통화별 평가, 오늘 손익, 보유 종목, 자산배분), 아래 “오늘”에 신호·공시·브리핑 카드가 최근 것부터 쌓여요.
- **종목을 고르면** 같은 본문 자리에 종목 상세가 열리고, 머리줄의 뒤로 가기(“현황으로”)로 돌아와요.
- **성적표**는 종목 상세와 같은 방식으로 본문 자리에 열리는 보조 화면이에요(뒤로 가기 “현황으로”). **설정**은 머리줄 버튼으로 여는 Dialog예요.
- **첫 실행**에는 토스 키 붙여넣기와 허용 IP 등록 안내만 있는 화면이 나와요.
- **상태 띠(StatusBar)** 는 문제가 있을 때만 머리줄 아래에 한 문장으로 나타나요.
- **설정 기본 항목은 세 개**예요: 토스 연결, 투자 성향(기간 · 위험 성향, 제외 · 비중 상한 요약), 주문 켜기. 나머지는 “고급” 펼침(Disclosure) 안의 다섯 묶음(AI, 뉴스·데이터, 주문, 투자 원칙, 시스템)에 있어요. 매수·매도 임계값은 코드가 계산해서 입력란이 없고, 기록만 하는 기간은 판정된 추정이 30건 쌓이면 자동으로 끝나서 시스템 > 상태에 읽기 전용 한 줄로 보여줘요.

## 말투와 문구

- 해요체로 써요. “동기화했어요”, “IP를 등록해주세요”. “~하십시오”나 반말은 쓰지 않아요.
- 한 문장엔 한 가지만. 오류 문구는 무슨 일인지 → 어떻게 고치는지 순서로 써요.
- AI 근거는 단정하지 않아요. “오를 거예요” 대신 “근거는 A, B예요. 다만 C 때문에 변동이 클 수 있어요.”
- 신호 문구는 앱이 계산한 사실을 말해요: “매수 신호예요. 3개월 안에 손절가 67,500원보다 목표가 79,000원에 먼저 닿을 확률이 44%로, 매수 기준 41%보다 높아요.” 매도는 “…먼저 닿을 확률이 15%로, 매도 기준 18%보다 낮아요.” 매매를 권하거나 묻지 않아요(“살까요?” 금지).
- 버튼은 동사로 끝내요: “연결하기”, “IP 복사”, “매수 주문 넣기”.
- 확률 조정은 “적중 기록 반영”이라고 써요.
- 이모지는 쓰지 않아요. 방향은 ▲ ▼ –, 주의는 `!`, 실패는 `×`.

## 숫자 표기

### 코어와 화면의 약속

- 값은 **문자열만** 받아요. 형식: `^-?(0|[1-9]\d*)(\.\d+)?$` (`"1234.50"`, `"-0.00"`, `"0.0312"`). 숫자 타입, `"1,234"`, `"1e5"`, `"NaN"`, 빈 문자열, `null`, `undefined`는 모두 잘못된 값이에요.
- 잘못된 값은 그 자리에 주황 `—`(“값 오류”)로 보여주고 `StockCopilot.onFormatError(fn)`·`stockcopilot:format-error` 이벤트로 올려요. 앱은 로그에 남기고 실패 상태를 보여줘요.
- 반올림은 코어가 해요. 원화 0자리(평균단가는 2자리까지), 달러 1달러 이상 2자리·미만 4자리, 등락률 2자리, 비중 1자리, 주식 수는 토스가 준 그대로.
- 확률은 1–99 정수 문자열(`Probability`). 0%·100%는 쓰지 않아요.
- 방향은 받은 문자열로 정해요. `"-0.00"`은 보합(–)이에요.

### 표기 규칙

- 원화 `84,312,500원`, 달러 `$182.34`, 주식 수 `0.5주`.
- 큰 금액 요약: `format.krwShort` → `1조 2,345억 6,789만원`. 만 단위 아래를 반올림하는 유일한 근사 표기라 요약 문장에만 쓰고 표에는 쓰지 않아요.
- 등락은 부호 필수: `+1,700원 (+2.43%)`, `−850원 (−1.12%)`. 빼기는 `−`(U+2212).
- 통화가 섞이면 행마다 원래 통화로 보여주고, 원화 환산 합계엔 `≈`와 환율·기준 시각을 붙여요(토스 환율은 참고용).
- 실시간이 아닌 값엔 기준 시각을 붙여요.

## 색

- 바탕 `bg`, 카드·표·패널 `surface`, 입력창·표 머리·사용자 말풍선·“앱이 계산한 사실” 구역 `surface-sunken`, 팝오버·대화 상자 `surface-raised` + `shadow-popover`.
- 본문 `ink`, 라벨 `ink-muted`, 시각·출처 `ink-subtle`. 셋 다 `bg`·`surface`·`surface-sunken` 위 4.5:1 이상이에요(라이트·다크 모두).
- 카드 구분은 그림자가 아니라 `line` 테두리(`border-hairline`). 입력창·스위치·체크박스·확률 막대 테두리는 `line-strong`(3:1 이상).
- `primary`(제이드 그린): 주요 액션, 선택, 링크, AI 비서와 “AI 근거” 구역 테두리, 확률 막대 채움.
- 경고와 오류는 모두 `alert`(주황)이에요. 단계는 모양으로 구분해요(아래 표).
- 차트 계열 `chart-1`…`chart-6`: 홀수는 진하고 짝수는 밝게 번갈아서 이웃 계열이 밝기로 구분돼요. 7개 이상은 코어가 ‘기타’로 묶어요. 가격 방향(캔들, 트리맵)만 `up`·`down`(과 `-soft`)을 써요.

| 단계 | 언제 | 모양 |
|---|---|---|
| 알림 `info` | 휴장, 동기화 완료 | `surface-sunken` 바탕, 회색 `i` |
| 주의 `warning` | 쏠림, 오래된 데이터, 근거가 외부 출처뿐 | `alert-soft` 바탕, 테두리만 있는 `!`, 제목이 “주의”로 시작 |
| 실패 `error` | 토스 연결 실패, 토큰 폐기, 주문 거절·실패, 값 오류 | `alert-soft` + `alert` 테두리, 꽉 찬 `×`, 제목이 “실패”로 시작, `role="alert"`, 조치 버튼 |

## 타이포그래피

- IBM Plex Sans KR(`sans`, 400·500·600·700)과 IBM Plex Mono(`mono`, 400·500)를 `fonts/`의 woff2로 번들해요. 앱은 외부 요청 없이 이 파일만 써요. 라이선스는 SIL OFL 1.1(`fonts/OFL.txt`).
- 크기·줄 높이·굵기는 `text-*`, `leading-*`, `weight-*` 토큰이고 타입 스타일과 같은 값이에요. 본문 `body`(14px), 카드 설명 `body-sm`, 라벨 `label`, 출처 `caption`, 코드 `code`.
- 숫자: `num-display`(총 평가, 화면당 하나) → `num-lg` → `num-md` → `num-sm`.
- Plex Sans KR엔 `≈` 글리프가 없어서 그 글자만 시스템 글꼴로 보여요.

## 간격과 격자

- 간격은 **4px 기준**이에요: `space-1`(4) … `space-12`(48). 예외는 `gap-hairline`(2px, 붙은 막대 사이)와 테두리 굵기(`border-hairline` 1px, `border-mark` 1.5px, `border-focus` 2px)뿐이에요.
- bundle.css의 모든 길이·글자 크기·굵기는 토큰이에요. 새 수치가 필요하면 토큰부터 만들어요.
- 격자: 12열(`grid-columns`), 열 간격 `space-6`. GridItem은 **3·4·6·8·9·12열만** 써요.
- 본문 폭 구간(Grid 자신의 폭, 컨테이너 기준): narrow 640–899px, medium 900–1199px(`bp-content-medium`), wide 1200px 이상(`bp-content-wide`). 본문은 여백(`space-6`) 안쪽 폭이 `content-max`(1200px)에서 멈추고 가운데 정렬돼요. 그래서 Grid가 wide 구간에 닿을 수 있어요(창 1920px 예시). 긴 글은 `prose-max`(720px).

## 레이아웃

- 창 최소 `window-min-width`×`window-min-height`(960×640). 창 전체는 스크롤하지 않고 본문과 채팅이 각자 스크롤해요.
- 창 폭 구간(AppShell이 판정):
  - 1280px(`bp-window-lg`) 이상: 종목 목록 `sidebar-width` · 본문 · 채팅 `chat-width`(`chat-width-min`–`chat-width-max`).
  - 1080–1279px: 채팅이 본문 위 오버레이(`z-panel`)가 되고 기본으로 닫혀요. 머리줄의 “AI 비서 열기”로 열어요.
  - 1080px(`bp-window-md`) 미만: 종목 목록이 `sidebar-collapsed`(56px) 레일로 접혀요. 레일엔 이름 앞 두 글자와 ▲▼만 보이고(툴팁에 전체), 펼치기 버튼을 누르면 목록 전체가 본문 위에 펼쳐져요. 종목을 고르거나 Esc를 누르면 닫혀요.
- 화면 코드는 **레이아웃 컴포넌트와 토큰 이름만**으로 배치해요: AppShell, Page, Section, Grid·GridItem, Stack, Inline, Split, Card, Toolbar, ScrollArea, Divider. 이 컴포넌트들은 `className`·`style`을 받지 않고, 간격·폭은 토큰 이름(`gap: 'space-4'`, `size: 'chat-width-min'`)으로만 받아요. 토큰이 아닌 값은 무시하고 오류로 올려요.
- 겹침 순서: `z-sticky` < `z-panel` < `z-popover` < `z-modal` < `z-toast`.

## 상태와 움직임

- 각 영역은 네 상태를 가져요: **로딩**(Skeleton, 같은 모양으로 자리 잡기) · **빈**(EmptyState: 무엇이 없는지 + 다음에 할 일) · **오류**(실패 Callout + 다시 시도, 화면 전체 문제면 StatusBar 한 줄) · **정상**.
- 포커스는 `focus-ring` `border-focus` 실선, `focus-offset` 띄워서. 클릭할 수 있는 건 모두 키보드로도 돼요.
- 반복 애니메이션은 쓰지 않아요. Skeleton은 반짝이지 않고, Spinner는 돌지 않는 고리와 문구예요. 시세 변경 깜빡임(`sc-flash-up`/`down`)은 `duration-flash` 동안 한 번만.
- 움직임 줄이기(`prefers-reduced-motion: reduce`): 깜빡임과 전환을 모두 꺼요.

## 아이콘

- Lucide(ISC 라이선스) 45개를 `assets/Icons`에 넣었고, 같은 모양을 `Icon` 컴포넌트가 인라인으로 그려 글자색을 따라요. 원본 SVG는 `stroke="currentColor"`라서 `<img>`로 쓰면 검정(`ink` 라이트 값과 비슷)으로 보여요.
- 목록과 쓰임은 `assets/Icons/README.md`에 있어요. 목록 밖 아이콘이 필요하면 먼저 여기에 추가해요.
- 방향 기호(▲ ▼ –)는 아이콘이 아니라 글자예요. 로고는 아직 없어서 앱 이름은 글자로 써요.

## 컴포넌트

- 모든 컴포넌트는 `className`·`style`을 받지 않아요. Button은 `variant`, `size`, `onClick`, `disabled`, `type`, `aria-label`만 받아요.
- 레이아웃: AppShell, Page, Section, Grid, GridItem, Stack, Inline, Split, Card, Toolbar, ScrollArea, Divider
- 기본: Text, Heading, Icon, IconButton, Button, SegmentedControl, Badge
- 입력: TextField(일반·비밀 값), Select, Switch, Checkbox, ChatInput
- 겹침: Disclosure, Dialog, Popover, Menu, Tooltip
- 피드백: Callout, StatusBar, EmptyState, Skeleton, Spinner, Toast
- 숫자: Money, Percent, Quantity, Probability, PriceChange, Metric
- 표: Table, HoldingsTable, ScorecardTable
- 차트: LineChart, AreaChart, DonutChart, Treemap, CandlestickChart, ReliabilityChart, Sparkline, AllocationBar
- 제품: SignalCard, ProbabilityMeter, BriefingCard, DisclosureCard, StockList, StockListItem, ChatPanel, ChatMessage, Markdown
- 화면 예시(Screens): 주 화면 1920·1440·1180·1000, 첫 실행, 설정, 종목 상세, 성적표. 각 예시의 “화면 상태” 전환으로 로딩·빈·오류·정상을 볼 수 있어요.

전역 이름은 `window.StockCopilot`이에요. 각 컴포넌트 README에 넘길 값과 하지 말아야 할 것이 있어요.
