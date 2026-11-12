# SignalCard

매수·매도 신호 하나예요. AI는 “기간 안에 손절가에 닿기 전에 목표가에 먼저 닿을 확률”과 근거만 내고, 앱 코드가 그 확률을 적중 기록으로 조정해 임계값과 비교해서 신호를 띄워요. 매수·매도 버튼은 이 카드에만 있어요.

- 제공: `stock`(`{name, ticker, market}`), `side`(`buy`·`sell`), `time`, `price`(현재가), `targetPrice`·`targetRate`(목표가와 현재가 대비 등락률), `stopPrice`·`stopRate`(손절가와 등락률), `horizon`(“3개월”, “5일”), `horizonEnd`(끝나는 날), `quantity`, `amount`, `costs`(수수료·세금), `weightAfter`(주문 후 비중), `currency`, `aiProbability`(AI가 낸 확률), `adjustedProbability`(적중 기록을 반영한 확률), `baseProbability`(기준 확률), `buyThreshold`, `sellThreshold`, `history`(`{hitRate, count, judged}`), `reasons`(마크다운), `sources`, `state`, `reason`, `doneAt`, `onOrder`, `onRetry`. 숫자는 모두 코어가 준 문자열, 확률은 1–99 정수.
- 두 구역을 눈에 띄게 나눠요.
  - **앱이 계산한 사실**(`surface-sunken` 바탕): 맨 위 한 줄에 현재가 · 목표가 · 손절가 · 기간, 그 아래 확률 막대(적중 기록 반영 확률, 기준 확률, 매수·매도 기준), AI 확률과 반영 확률, 수량·금액·비용·비중·과거 적중률.
  - **AI 근거**(`primary` 테두리): 근거와 출처.
- 상태(`state`):
  - `default`: 매수(빨강)·매도(파랑) 버튼. 누르면 앱 밖 macOS 확인 창(Touch ID)이 떠요.
  - `ordersOff`: 주문 기능 꺼짐. 버튼 없음.
  - `zeroQty`: 수량 0. 버튼 없이 `reason` 한 문장.
  - `externalOnly`: 근거가 외부 출처뿐. 주의 Callout과 버튼.
  - `recordOnly`: 기록만 하는 기간(판정된 추정이 30건 쌓이면 코어가 끝내요). 버튼 없음.
  - `pending`: Touch ID 승인 대기.
  - `filled`: 체결됨(`doneAt`).
  - `rejected`: 거절·실패. 실패 Callout과 `reason`.
- `history.judged`가 `false`면 과거 적중률 대신 “판정 보류 (n건)”을 보여줘요. 판정 여부는 코어가 정해요.
- 출처가 없으면 “출처 없음”을 `alert` 색으로 보여줘요.
- 주문 정정·취소 버튼은 두지 않아요. 토스증권 앱에서 해요.
