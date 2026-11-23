# ADR 초안 D-57: 매도 초안의 기본 수량 규칙과 A-14 매도 경로 연결

- 상태: 초안 (2026-11-24, 번호는 메인 병합 때 부여)
- 관련 작업: D-57 (`docs/tasks.md`), A-14 (ADR 0152), D-26 (ADR 0076), D-34

## 배경과 결정

ADR 0152(A-14)는 매도 신호를 평가·보존하면서도 기본 수량 규칙이 확정되지 않아 `DraftProposalError::SellDraftUnavailable`로 초안 생성을 보류했다. 메인은 D-57에서 다음을 확정했다.

1. 매도 초안의 기본 수량 목표는 보유 수량 전량(`held_quantity`)이며 상한은 매도가능수량(`sellable_quantity`)이다. 즉 기본 수량은 `min(보유 수량, 매도가능수량)`이다. 조회 시차로 `매도가능수량 > 보유 수량`인 경우에도 보유 수량 전량을 넘기지 않는다(ADR 0076 예제 `보유 7, 매도가능 19 → 7주`와 일관).
2. 매도가능수량이 `0`이거나 알 수 없으면(`None`) 수량을 추정하지 않고 초안을 만들지 않으며 닫힌 사유를 남긴다.
3. 승인 전에 사용자가 수량을 줄일 수 있다. 사용자가 지정한 수량은 `1` 이상 기본 수량 이하의 시장 수량 단위(지정가 정수 1주 단위)만 허용한다. 기본 수량보다 늘리는 것은 허용하지 않는다.
4. 계산과 판정은 `crates/domain`에서 부동소수점 없이 기존 도메인 수량·시각·식별자 타입(`Quantity`, `WholeOrderQuantity`, `SellableQuantityFact`, `SellSignal`, `PortfolioValuation`, `AccountId`, `Timestamp`)만으로 수행한다.
5. `crates/application`의 `DraftProposal`에서 `SellDraftUnavailable` 조기 반환을 제거하고 매도 초안 생성 경로를 연결한다. 단일 초안, 선택 계좌 대조·대체, 스냅숏·보정 원장 일치, 범위 밖 폐기·재추정 예약, 닫힌·불명 장 보존, 만료 후 재생성, 저장 직전 재판정과 시계 역행 거부는 매수와 동일하게 적용한다.

## ADR 0076(D-26)과의 관계 및 A-14 인계 대체 근거

확정 ADR 0076(D-26)은 A-14에 `calculate_sell_quantity` → `bind_sell_limit` → `sell_order_into_intent(CalculatedSellOrder, ClientOrderId)` 경로를 인계했다. 그러나 D-57의 A-14 매도 초안 생성 경로에서는 해당 인계를 `SellDraftQuantityPlan` + `application::sell_limit_intent`로 대체(supersede)한다. 기술적 근거는 세 가지다.

1. **내부 종목 ID 대조 누락**: D-26의 `SellQuantityPlan`·`bind_sell_limit`는 `ResolvedOrderInstrument`(심볼·시장)만 비교하고 내부 `InstrumentId`를 대조하지 않는다. ADR 0152(A-14)는 심볼 재사용·마스터 불일치 차단을 위해 내부 `InstrumentId`·심볼·시장 전체 일치를 요구하며, 매수 경로(`buy_limit_intent`)도 동일 계약을 따른다.
2. **미국 소수 보유 처리 충돌**: D-26의 `calculate_sell_quantity(..., SellQuantityMode::Whole)`은 미국 소수 보유(`10.5주`)를 조용히 내림(`10주`)해 성공시킨다. 반면 D-57은 문서 미결정인 소수점 주식 처리를 "정의 대기"로 두고 안전 기본값(초안 미생성: `FractionalQuantityUnsupported`)을 요구한다.
3. **초안 계약 필드 부재**: D-26은 `SellSignal` 결합, 보유·매도가능수량 60초 신선도 및 저장 직전 재판정(`check_facts_at`), `Option<SellableQuantityFact>`의 미확인 사유 구분, 잠긴 수량(`locked_quantity`), 사용자 승인 전 수량 축소(`reduce_quantity`)를 표현하지 않는다.

**공존과 정본 범위**:
- A-14 매도 지정가 초안 생성과 승인 전 수량 축소 검증은 **D-57(`SellDraftQuantityPlan` + `sell_limit_intent`)**을 정본으로 사용한다.
- 기존 D-26(`SellQuantityPlan`, `bind_sell_market`, `MarketOrderPolicy`)은 기존 테스트 호환과 향후 고급 설정 기반 시장가·미국 소수 시장가 경로(A-17/O-14)의 검토 자산으로 유지한다. O-01/O-04에서 정의 대기 항목(미국 소수·시장가)이 확정될 때 D-26과 D-57의 최종 통합 또는 분리 범위를 정리한다.

## 도메인 설계 (`crates/domain/src/sell_draft_quantity.rs`)

`BuyQuantityPlan`과 대칭되는 봉인된 타입 집합을 추가한다.

- `sell_signal_matches_reassessed(stored, reassessed) -> CheckResult`:
  - `DraftProposal::prepare_open`이 최신 잔고를 다시 조회해 재평가할 때 사용한다.
  - `SellSignal`의 값 동등성(`PartialEq`, `observed_at` 포함)은 기존 도메인 의미론을 보존하되, 재평가 일치 판정에서는 `facts`와 `held_quantity`가 같고 `reassessed.observed_at() >= self.observed_at()`이면 동일 매도 신호의 최신 보유 관측으로 통과시킨다.
  - 보유 수량이 달라지거나(`held_quantity` 불일치), 평가 사실이 달라지거나, 과거 보유 관측으로 역행(`observed_at < self.observed_at()`)하면 실패(`DraftProposalError::SignalChanged`)한다.
  - 이 규칙으로 장 닫힘→열림 초안 생성, 만료(10분) 후 최신 잔고·시세 재생성, 60초 이내 최신 잔고 재조회가 매수와 동일하게 동작한다.
- `SellDraftQuantityFacts<'facts>`: 계좌(`AccountId`), 포트폴리오 평가(`PortfolioValuation`), 선택적 매도가능수량(`Option<&SellableQuantityFact>`)을 묶는다. `Debug`는 내부 값을 노출하지 않는다.
- `calculate_sell_draft_quantity(signal, facts, now, calendars) -> SellDraftQuantityDecision`:
  1. `SignalTiming::check`로 신호 유효기간·제출 시각·캘린더를 확인한다 (`NoSellDraftQuantity::SignalUnavailable`).
  2. 포트폴리오에서 신호 종목의 보유를 찾는다. 없거나 수량이 `0`이면 `NoSellDraftQuantity::NotHeld`.
  3. `SignalTiming::check_holding`과 `holding.observed_at() >= signal.observed_at()`로 보유 사실의 최신성(60초 이내, 미래 거부, 과제 생성·신호 관측 이전 거부)을 확인한다 (`NoSellDraftQuantity::StaleHolding`).
  4. 보유 수량이 `signal.held_quantity()`와 다르면 `NoSellDraftQuantity::HoldingChanged`.
  5. 매도가능수량 사실이 없으면 `NoSellDraftQuantity::SellableQuantityUnknown`.
  6. 매도가능수량의 계좌·종목이 요청 계좌·신호 종목과 다르면 `NoSellDraftQuantity::DifferentFact`.
  7. 매도가능수량 관측 시각이 미래이거나 `signal.observed_at()` 이전(이는 `task.created_at()` 이전도 함께 차단)이거나 `now` 기준 60초(`SELLABLE_QUANTITY_MAX_AGE`)를 1ns라도 넘기면 `NoSellDraftQuantity::StaleSellableQuantity`.
  8. 매도가능수량이 `0`이면 `NoSellDraftQuantity::ZeroSellableQuantity`.
  9. 보유 수량과 매도가능수량 모두 정수 1주 단위(`WholeOrderQuantity`, 소수점 없는 양의 정수)인지 확인한다. 소수점 수량이 있으면 안전 기본값으로 초안을 만들지 않고 `NoSellDraftQuantity::FractionalQuantityUnsupported`.
  10. `sellable < held`이면 기본 수량은 `sellable`, 제약은 `SellQuantityConstraint::SellableQuantityCap`, 잠긴 수량(`locked_quantity`)은 `held - sellable`이다. 그렇지 않으면(`held <= sellable`) 기본 수량은 `held`, 제약은 `SellQuantityConstraint::FullHolding`, 잠긴 수량은 `0`이다.
- `SellDraftQuantityPlan`:
  - 필드는 비공개이며 `Debug`는 `"SellDraftQuantityPlan(..)"`만 출력한다.
  - `reduce_quantity(&self, requested: Quantity) -> Result<Self, SellDraftQuantityAdjustmentError>`:
    - 소수점이 있는 수량(`0.5`, `1.5` 등)은 `InvalidQuantityUnit`.
    - `0`(또는 `0.00`)은 `BelowMinimumUnit`.
    - 현재 계획 수량(`self.quantity()`)을 초과하면 `ExceedsAllowedQuantity`(안전 기본값: 기본 수량 초과뿐 아니라 이미 줄인 계획 객체에서 다시 늘리는 것도 단조 감소로 제한; 원래 기본 수량 범위 안 재선택이 필요하면 초기 계획에서 호출하거나 아래 정의 대기 결정에 따라 조정).
    - 통과하면 새 계획을 반환하며, `requested < default_quantity`이면 `SellQuantityAdjustment::Reduced`, `requested == default_quantity`이면 `SellQuantityAdjustment::Default`를 기록한다.
  - `check_facts_at(&self, now, calendars) -> Result<(), NoSellDraftQuantity>`: 저장 직전 시점(`write_at`)에 신호 유효기간·보유 관측 60초·매도가능수량 관측 60초가 여전히 유효한지 재확인한다.

## 애플리케이션 연결 (`crates/application`)

- `sell_limit_intent.rs`: `buy_limit_intent.rs`와 대칭되는 `sell_limit_intent(plan, limit, account, id) -> Result<OrderIntent, SellLimitIntentError>`를 추가한다. 지정가 방향(`OrderSide::Sell`, exhaustive `match`), 계좌(`plan.account_id() == account`), 내부 종목 ID·심볼·시장 일치를 모두 확인한 뒤 `OrderTerms::KrLimit` / `OrderTerms::UsLimit`로 `OrderIntent`를 만든다. 매수 지정가와 달리 매도 수량은 주문 가격에 종속되지 않으므로 가격 변동에 따른 수량 재계산은 요구하지 않는다(지정가 호가 단위·상하한·5초 신선도는 `calculate_limit_price`가 검증한다).
- `draft_proposal.rs`:
  - `DraftProposalInputs`에 `pub sellable_quantity: Option<SellableQuantityFact>`를 추가한다.
  - `DraftProposalError::SellDraftUnavailable`를 제거하고 `SellQuantity(NoSellDraftQuantity)`와 `SellIntent(SellLimitIntentError)`로 대체한다.
  - `prepare_open`의 재평가 비교를 exhaustive `match`로 바꾸어, 매수는 기존과 동일하게 전체 동등성을 확인하고 매도는 `sell_signal_matches_reassessed`로 최신 보유 관측을 허용하되 수량·사실 변경 및 과거 관측 역행을 `SignalChanged`로 차단한다.
  - `finish_attempt`의 매도 조기 반환을 제거하고, `DraftCandidate`를 `Buy(Box<Proposal<Draft>>)` / `Sell { proposal, plan }` 열거형으로 분리해(RV-2 준수) 저장 직전 `recheck`와 `proposal.check_current(write_at, observation)` 직후 매도 후보가 빠짐없이 `plan.check_facts_at(write_at, calendars)`를 거치도록 강제한다.
  - 기존 `check_identity`의 `MasterOrderGateFacts::instrument_for(OrderSide::Sell)` 경로로 국내 정리매매(`LiquidationTrading`) 종목의 매도 초안은 허용하고, 거래정지·상장폐지(`NotListed`)·마스터 불일치는 거부한다.

## 상황 → 결과 표

| 상황 | `DraftProposal` 결과 | 도메인 직접 호출(`calculate_sell_draft_quantity`) / 비고 |
|---|---|---|
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 17주 | 매도 초안 생성 (`17주`, `FullHolding`, `locked=0`) | 최우선 매수호가 내림 지정가 |
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 11주 | 매도 초안 생성 (`11주`, `SellableQuantityCap`, `locked=6`) | 매도가능수량 상한 적용 |
| 열린 장, 범위 안 매도 신호, 보유 7주·매도가능 19주 | 매도 초안 생성 (`7주`, `FullHolding`, `locked=0`) | 보유 전량 초과 금지(결정 1·ADR 0076) |
| 평가 후 최신 잔고 재조회(동일 보유 17주, `t_new >= t_sig`, 60초 이내) | 매도 초안 생성 | 장 닫힘→열림·만료 재생성 포함 |
| 평가 후 보유 수량 변동(`17→15`) 또는 과거 관측 역행(`task.created_at() <= t_old < t_sig`) | 초안 미생성 (`SignalChanged`) | 선행 재평가(`prepare_open`)의 `sell_signal_matches_reassessed`가 차단. 도메인 직접 호출 시 수량 불일치는 `HoldingChanged`, 신호 관측 이전은 `StaleHolding` |
| 평가 후 보유 없음 또는 보유 0주 | 초안 미생성 (`SignalUnavailable(UnheldLowProbability)`) | 선행 재평가(`prepare_open`)에서 미보유 판정. 도메인 직접 호출 시 이중 방어 `NotHeld` |
| 평가 후 보유 관측이 60초 초과·미래·과제 생성 이전 | 초안 미생성 (`SignalUnavailable(Unavailable(StaleHolding))`) | 선행 재평가(`prepare_open`)에서 낡은/미래 잔고 차단. 도메인 직접 호출 시 `StaleHolding` |
| 선행 재평가 통과 후 계산(`calculated_at`) 또는 저장 직전(`write_at`) 사이에 보유 관측 60초 초과·시계 역행 | 초안 미생성 (`SellQuantity(StaleHolding)`) | `calculate_sell_draft_quantity` 및 `plan.check_facts_at(write_at)` 저장 직전 재판정이 차단 |
| 매도가능수량 미확인(`None`) | 초안 미생성 (`SellQuantity(SellableQuantityUnknown)`) | 신호 유지, 수량 추정 금지 |
| 매도가능수량 `0` | 초안 미생성 (`SellQuantity(ZeroSellableQuantity)`) | 신호 유지 |
| 매도가능수량 60초 초과·미래·신호 관측 이전 | 초안 미생성 (`SellQuantity(StaleSellableQuantity)`) | 정확히 60초는 허용, 60초+1ns 거부. 저장 직전 재판정에서도 검증 |
| 매도가능수량 계좌 또는 종목 불일치 | 초안 미생성 (`SellQuantity(DifferentFact)`) | 타 계좌·타 종목 사실 혼용 금지 |
| 국내·미국 소수점 보유 또는 미국 소수점 매도가능수량 | 초안 미생성 (`SellQuantity(FractionalQuantityUnsupported)`) | 안전 기본값(정의 대기) |
| 사용자 축소 `1 <= q < 기본 수량` (정수) | 도메인 허용 (`quantity = q`, `Reduced`) | 기본·잠긴 수량 기준 보존 |
| 사용자 축소 `q == 기본 수량` (정수) | 도메인 허용 (`quantity = q`, `Default`) | 축소 없음 상태 유지 |
| 사용자 축소 `q == 0` | 도메인 거부 (`BelowMinimumUnit`) | 최소 1주 규칙 |
| 사용자 축소 소수점(`0.5`, `1.5` 등) | 도메인 거부 (`InvalidQuantityUnit`) | 지정가 정수 수량 단위 규칙 |
| 사용자 수량 증가(`q > 현재 허용 수량`) | 도메인 거부 (`ExceedsAllowedQuantity`) | 기본 초과 및 축소 후 재증가 차단 |
| 매도 신호가 추정 가격 범위 밖 | 매도 신호 폐기 + 재추정 예약 | 기존 A-14 계약 유지 |
| 닫힌 장 / 불명 장의 매도 신호 | `MarketClosed` / `MarketUnknown` | 신호 유지, 초안 시도 0회 |
| 매수→매도 또는 매도→매수 방향 전환 | 이전 초안 대체 후 새 방향 초안 생성 | 단일 활성 초안 계약 |
| 계좌 전환 또는 구 계좌 지연 요청 | 새 계좌 전환 시 구 초안 대체, 구 계좌 요청 `AccountChanged` | U-22·ADR 0152 계약 |
| 국내 정리매매 종목의 매도 신호 | 매도 초안 생성 허용 | 매수는 차단, 청산 매도만 허용 |

## 정의 대기 (문서가 정하지 않은 선택지와 사용자 영향)

문서와 메인 결정이 아직 정하지 않은 세부 항목은 임의로 확정하지 않고, 안전한 쪽(초안을 만들지 않음 또는 더 엄격한 상한)을 기본값으로 구현·인계한 뒤 아래 선택지를 제안한다.

| 항목 | 현재 기본값(안전 쪽) | 선택지 | 사용자 영향 |
|---|---|---|---|
| 미국 소수점 주식 보유·매도가능수량(예: `10.5주`, `0.5주`) | `FractionalQuantityUnsupported`로 초안 미생성 | **A(현재 기본)**: 소수점 보유·매도가능수량은 자동 초안을 만들지 않고 사유 표시<br>**B**: 정수 부분(`floor`)만 지정가 매도 초안으로 만들고 잔여 소수 수량을 안내<br>**C**: 소수 매도가능수량을 미국 시장가 매도(`UsMarketSell`) 초안으로 생성(고급 설정 연동) | **A**: 의도치 않은 부분 청산·시장가 전환이 없으나 소수 보유자는 직접 매도해야 함<br>**B**: 대부분 수량을 지정가로 빠르게 청산 가능하나 잔여 소수 주식이 계좌에 남음<br>**C**: 전량 청산 가능하나 지정가 원칙(설계 6.3)과 어긋나고 슬리피지 위험이 생김 |
| 일부만 매도 가능한 잠긴 수량(`locked_quantity > 0`)의 화면 표시와 초안 메타데이터 저장 | 도메인 계획에 `held_quantity`, `sellable_quantity`, `locked_quantity`, `constraint`를 노출하고 초안 수량은 `sellable_quantity`로 제한(`Proposal<Draft>` 스키마는 미변경) | **A(권장 기본)**: 초안 카드에 "보유 N주 중 매도 가능 M주(미체결·잠김 K주 제외)" 한 줄 안내(저장소 메타데이터 보관 또는 표시 시 조회)<br>**B**: 초안 카드에는 주문 수량 M주만 표시하고 상세 펼침에만 보유·잠김 수량 표시<br>**C**: 잠긴 수량이 있으면 초안 자체를 만들지 않고 미체결 주문 확인을 먼저 요구 | **A**: 사용자가 전량이 아닌 이유를 한눈에 이해(단순함 원칙 P-8의 필요한 결과만 표시), S-22 저장·조회 계약 확장 필요<br>**B**: 카드가 단순하나 사용자가 전량 매도되는 것으로 오해할 수 있음<br>**C**: 중복 주문 위험은 최저이나 일부 미체결이 걸린 종목의 매도 초안이 막힘 |
| 승인 직전(O-04) 재계산 시 '늘리기 금지' 상한 기준 | **A(안전 기본 인계)**: 초안 만료(기본 10분) 내 승인·전송 직전 최신 잔고·매도가능수량을 재조회(관측 신선도 한도 60초)하되 `min(초안 카드 수량, 최신 매도가능수량, 최신 보유 수량)`을 상한으로 고정(`Proposal<Draft>`에는 카드 시점 주문 수량만 저장) | **A(현재 기본·안전 상한)**: `min(초안 카드 수량, O-04 최신 매도가능수량, 최신 보유 수량)`을 상한으로 고정해 미체결 취소 등으로 매도가능이 늘어도 카드 수량 초과 금지<br>**B**: O-04 최신 `calculate_sell_draft_quantity` 기본 수량까지 허용(카드에 보인 11주→최신 17주 확대 허용) | **A**: 사용자가 처음 본 초안 카드 수량을 넘어 주문되는 일이 없어 예측 가능·안전<br>**B**: 승인 전 미체결을 취소한 사용자가 새 초안을 기다리지 않고 전량 매도 가능하나 카드 표시 수량과 달라짐 |
| 이미 축소한 계획에서 기본 수량 이하로의 재증가(입력 정정) 허용 여부 | 단조 감소(`requested <= self.quantity()`, 이미 줄인 계획 객체에서 재증가 거부; 원래 범위 내 재선택은 초기 계획에서 호출) | **A(현재 기본)**: 계획 객체 단위 단조 감소 유지(승인 UI 입력 변경은 초기 계획 기준 재계산)<br>**B**: `reduce_quantity` 상한을 `self.default_quantity()`로 열어 축소된 계획에서도 `1..=default_quantity` 재조정 허용 | **A**: 한 번 줄인 계획 객체가 우연히 커지지 않는 방어적 불변식 유지<br>**B**: UI 상태 보관 객체 하나로 11→3→8 입력 정정을 직접 처리 가능 |
| 매도가능수량 관측 신선도 한도 및 보유 관측과의 순서 | `60초`(`SELLABLE_QUANTITY_MAX_AGE`) 및 `sellable.observed_at() >= signal.observed_at()`(보유와 병렬 조회 허용) | **A(현재 기본)**: 60초 한도 + `>= signal.observed_at()`(두 브로커 API 병렬 조회 허용, O-04 재조회가 최종 방어)<br>**B**: `sellable.observed_at() >= holding.observed_at()` 순차 조회까지 강제<br>**C**: 호가와 동일한 5초로 강화 | **A**: 병렬 잔고 조회가 실패하지 않고 1분 초과·신호 이전 낡은 값 차단<br>**B**: 보유 관측 직후 수동 주문 반영에 유리하나 병렬 조회 불가로 지연·실패 증가<br>**C**: 잔고 API 지연·캐시 주기에서 잦은 초안 실패 가능 |
| `EvaluateSignals` 주기적 재평가 시 관측 시각만 바뀐 매도 신호의 초안 유지 여부 | `SellSignal::PartialEq`에 `observed_at`이 포함되어 `SignalTransition::Updated`로 분류(기존 도메인 의미론 보존; S-22 구현 전 결정 필요) | **A(권장 기본)**: `signal_transition`과 S-22에서 매도 신호의 `facts`·`held_quantity`가 같고 `observed_at`만 전진한 경우 `Unchanged`로 취급해 기존 활성 매도 초안·리비전 유지<br>**B**: `observed_at` 갱신마다 `Updated`로 리비전을 올려 매도 초안을 새로 생성·대체 | **A**: 매수와 동일하게 단일 활성 초안이 재평가마다 깜박이거나 중복 알림(`ProposalCreated`)을 내지 않음(도메인 `signal_transition` + S-22/S-18 동시 합의 필요)<br>**B**: 매도 초안이 재평가 주기마다 갱신·대체되어 알림 피로와 승인 중 충돌 위험 증가 |
| 사용자가 수량을 줄여 승인한 뒤 남은 매도가능수량의 재제안 허용 여부 | 매도에는 매수의 기간 H 반복 제한이 없어 현재 도메인 계산상 이전 초안 소비 후 남은 매도가능수량(예: 11주 중 5주 승인 후 잔여 6주)으로 새 초안 생성 가능(O-04/S-22 미연결; 미결정 시 안전 기본 정책 확정 필요) | **A(안전 기본 후보)**: 동일 매도 신호(같은 제출·보유 평가 기준)에서 1회 축소 승인한 뒤에는 다음 AI 제출 전까지 잔여 수량을 자동 재제안하지 않음<br>**B**: 미체결이 걸리지 않은 남은 매도가능수량에 대해 다음 초안 실행에서 잔여 수량 초안을 자동 재제안(항상 사람 승인 필요, 매도가능수량 상한 유지) | **A**: 사용자가 일부만 팔기로 선택한 의사를 존중하고 원치 않는 반복 매도 제안을 방지(단, 추가 매도는 직접 하거나 다음 추정 대기)<br>**B**: 분할 매도 시 남은 물량도 빠르게 다시 승인 가능하나, 일부만 팔고 나머지를 보유하려는 사용자에게 반복 카드 노출 |

## 인계

- **O-04 / U-xx (승인 전 수량 축소 UI 및 승인 직전 재검증)**:
  - 사용자가 승인 화면에서 수량을 줄일 때 도메인의 `SellDraftQuantityPlan::reduce_quantity`를 통해 검증한다.
  - 승인·전송 직전(초안 만료 기본 10분 안 언제든, 60초는 관측 신선도 한도)에는 최신 보유·매도가능수량을 다시 조회해 재계산한다. 위 "정의 대기"의 상한 기준이 확정되기 전에는 **안전 기본값 A(`min(초안 카드 수량, 최신 매도가능수량, 최신 보유 수량)`)**를 적용해 카드에 보인 수량을 초과하지 않게 한다.
  - 사용자가 수량을 줄여 승인한 뒤 남은 매도가능수량의 자동 재제안 여부(위 정의 대기 7행)도 O-04 소비 기록·S-22와 함께 확정한다.
- **S-22 / S-18 (저장소 및 신호 갱신)**:
  - 잠긴 수량 화면 표시(선택지 A/B)가 채택되면 `Proposal<Draft>` 또는 연관 메타데이터에 보유·매도가능·잠긴 수량과 제약 상태를 보존할지 조회 시 산출할지 저장소 계약을 함께 정한다.
  - 현재 `SellSignal`은 `observed_at`을 값 동등성(`PartialEq`)에 포함하므로, 실제 DB 저장소가 `SignalTransition::Unchanged`에서 리비전을 유지하더라도 보유 관측 시각만 새로워진 `EvaluateSignals` 재실행은 `SignalTransition::Updated`가 되어 기존 매도 초안을 대체한다. 위 정의 대기 6행(권장 기본 A: `facts`·`held_quantity` 동일 시 `Unchanged` 유지)을 도메인 `signal_transition`과 S-22/S-18에서 함께 확정한다.
- **어댑터(`DraftProposalData` 구현체)**:
  - `DraftProposalData::inputs`는 매도 신호의 `current.signal.observed_at()`보다 **이전이 아닌(`observed_at >= stored.observed_at()`) 60초 이내 최신 보유 관측(`evaluation.valuation`)과 최신 매도가능수량(`sellable_quantity`)**을 공급해야 한다. 보유 기본 5분 캐시 등 `stored.observed_at()` 이전의 오래된 스냅숏을 반환하면(특히 저장 `Conflict` 재시도 직후 평가가 더 최신 스냅숏을 커밋한 경우) `sell_signal_matches_reassessed`와 `check_sellable_freshness`가 `SignalChanged`·`StaleHolding`·`StaleSellableQuantity`로 닫힌 실패한다.
  - 선택 계좌의 최신 `BrokerRead::sellable_quantity`를 조회해 `DraftProposalInputs.sellable_quantity`로 공급한다.
  - 네트워크·인증·응답 오류 등 **조회 실패는 반드시 `Err(SystemPortError)`**로 반환해 `DraftProposalError::Inputs`가 원인을 보존하도록 한다(ADR 0152 준수). 조회 실패를 `None`으로 삼키지 않는다.
  - `sellable_quantity: None`은 엔드포인트 미제공·미지원 또는 매수 경로 생략처럼 매도가능수량을 알 수 없는 경우에만 사용하며, 이 경우 도메인이 수량을 추정하지 않고 `SellableQuantityUnknown`으로 닫힌 실패한다.
- **정의 대기 결정 반영**: 위 표의 7개 항목에 대한 사용자·메인 결정이 내려지면 ADR을 확정 상태로 갱신한다.

## 손 변형 실제 실행 (28건, 2차 `ffec2025` 기준)

별도 작업 복사본(`target/D-57/mut-copy`, 실행 뒤 삭제)에서 D-57의 핵심 규칙 28곳을 하나씩 변형해 컴파일 및 테스트 실행으로 검출을 확인했다. 28/28 모두 컴파일 통과 후 테스트가 검출(미검출 0, 컴파일 실패 0).

| # | 파일 | 변형 내용 | 검출 테스트 |
|---|---|---|---|
| M-01 | `crates/domain/src/domain_time.rs` | `SELLABLE_QUANTITY_MAX_AGE` `60s -> 61s` | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-02 | `crates/domain/src/domain_time.rs` | `SELLABLE_QUANTITY_MAX_AGE` `60s -> 59s` | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-03 | `crates/domain/src/quantity.rs` | `whole_excess`: `saturating_sub -> saturating_add` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-04 | `crates/domain/src/quantity.rs` | `whole_excess`: `.max(0_i128)` 제거 | `property_default_quantity_and_reduction_match_independent_reference` |
| M-05 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `scale() != 0` → `> 1` | `reduce_quantity_rejects_zero_fractions_and_handles_single_share` |
| M-06 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `requested > self.quantity` → `>=` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-07 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `requested > self.quantity` → `> self.default_quantity` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-08 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `< self.default_quantity` → `<=` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-09 | `crates/domain/src/sell_draft_quantity.rs` | 기본 수량: `sellable < held` → `<=` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-10 | `crates/domain/src/sell_draft_quantity.rs` | 기본 수량: `sellable < held` → `>` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-11 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: 보유 0주 거부 제거 | `sell_draft_rejects_missing_zero_and_changed_holdings` |
| M-12 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: `holding.observed_at() < signal.observed_at()` 제거 | `sell_draft_rejects_stale_future_and_pre_signal_holdings` |
| M-13 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: `holding.quantity() != signal.held_quantity()` 제거 | `sell_draft_rejects_missing_zero_and_changed_holdings` |
| M-14 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 계좌 ID 일치 검사 제거 | `sell_draft_rejects_unknown_zero_and_mismatched_sellable` |
| M-15 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 종목 ID 일치 검사 제거 | `sell_draft_rejects_unknown_zero_and_mismatched_sellable` |
| M-16 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 매도가능수량 0 거부 제거 | `property_default_quantity_and_reduction_match_independent_reference` |
| M-17 | `crates/domain/src/sell_draft_quantity.rs` | `check_sellable_freshness`: `observed_at < signal.observed_at()` 제거 | `sell_draft_enforces_sellable_60s_freshness_and_future_rejection` |
| M-18 | `crates/domain/src/sell_draft_quantity.rs` | `check_sellable_freshness`: `elapsed > MAX_AGE` → `>=` | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-19 | `crates/domain/src/sell_draft_quantity.rs` | `check_facts_at`: 보유 신선도 재검증 생략 | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-20 | `crates/domain/src/sell_draft_quantity.rs` | `check_facts_at`: 매도가능수량 신선도 재검증 생략 | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-21 | `crates/domain/src/sell_draft_quantity.rs` | `sell_signal_matches_reassessed`: `held_quantity` 비교 제거 | `sell_signal_matches_reassessed_accepts_fresher_observation_and_rejects_changes` |
| M-22 | `crates/domain/src/sell_draft_quantity.rs` | `sell_signal_matches_reassessed`: `observed_at >=` → `>` | `sell_signal_matches_reassessed_accepts_fresher_observation_and_rejects_changes` |
| M-23 | `crates/domain/src/sell_draft_quantity.rs` | `sell_signal_matches_reassessed`: `observed_at >=` 검사 제거 | `sell_signal_matches_reassessed_accepts_fresher_observation_and_rejects_changes` |
| M-24 | `crates/application/src/sell_limit_intent.rs` | `OrderSide::Buy` 허용 | `sell_limit_intent_validates_direction_account_and_instrument` |
| M-25 | `crates/application/src/sell_limit_intent.rs` | `plan.account_id() != account` 검사 제거 | `sell_limit_intent_validates_direction_account_and_instrument` |
| M-26 | `crates/application/src/sell_limit_intent.rs` | `instrument.market() != market` 검사 제거 | `sell_limit_intent_rejects_market_mismatch_and_builds_us_reduced_order` |
| M-27 | `crates/application/src/draft_proposal.rs` | 저장 직전 `plan.check_facts_at` 결과 무시 | `pre_store_recheck_rejects_stale_holding_and_local_clock_rollback` |
| M-28 | `crates/application/src/draft_proposal.rs` | `signal_matches` 매도 분기를 `old == fresh`로 되돌림 | `closed_to_open_market_sell_signal_creates_draft_with_fresh_holding` |
