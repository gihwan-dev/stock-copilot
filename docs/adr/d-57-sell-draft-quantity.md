# ADR 초안 D-57: 매도 초안의 기본 수량 규칙과 A-14 매도 경로 연결

- 상태: 초안 (2026-11-24, 번호는 메인 병합 때 부여)
- 관련 작업: D-57 (`docs/tasks.md`), A-14 (ADR 0152), D-26 (ADR 0076), D-34

## 배경과 결정

ADR 0152(A-14)는 매도 신호를 평가·보존하면서도 기본 수량 규칙이 확정되지 않아 `DraftProposalError::SellDraftUnavailable`로 초안 생성을 보류했다. 메인은 D-57에서 다음을 확정했다.

1. 매도 초안의 기본 수량 목표는 보유 수량 전량(`held_quantity`)이며 상한은 매도가능수량(`sellable_quantity`)이다. 즉 기본 수량은 `min(보유 수량, 매도가능수량)`이다.
2. 매도가능수량이 `0`이거나 알 수 없으면(`None`) 수량을 추정하지 않고 초안을 만들지 않으며 닫힌 사유를 남긴다.
3. 승인 전에 사용자가 수량을 줄일 수 있다. 사용자가 지정한 수량은 `1` 이상 현재 허용 수량 이하의 시장 수량 단위(지정가 정수 1주 단위)만 허용한다. 기본 수량이나 이미 줄인 수량보다 늘리는 것은 허용하지 않는다.
4. 계산과 판정은 `crates/domain`에서 부동소수점 없이 기존 도메인 수량·시각·식별자 타입(`Quantity`, `WholeOrderQuantity`, `SellableQuantityFact`, `SellSignal`, `PortfolioValuation`, `AccountId`, `Timestamp`)만으로 수행한다.
5. `crates/application`의 `DraftProposal`에서 `SellDraftUnavailable` 조기 반환을 제거하고 매도 초안 생성 경로를 연결한다. 단일 초안, 선택 계좌 대조·대체, 스냅숏·보정 원장 일치, 범위 밖 폐기·재추정 예약, 닫힌·불명 장 보존, 저장 직전 재판정과 시계 역행 거부는 매수와 동일하게 적용한다.

## 도메인 설계 (`crates/domain/src/sell_draft_quantity.rs`)

기존 D-26의 `SellQuantityPlan` / `calculate_sell_quantity`(비율 매도·미국 소수 시장가 포함)은 호환을 위해 유지하고, A-14 매도 초안 전용으로 `BuyQuantityPlan`과 대칭되는 봉인된 타입 집합을 추가한다.

- `SellDraftQuantityFacts<'facts>`: 계좌(`AccountId`), 포트폴리오 평가(`PortfolioValuation`), 선택적 매도가능수량(`Option<&SellableQuantityFact>`)을 묶는다. `Debug`는 내부 값을 노출하지 않는다.
- `calculate_sell_draft_quantity(signal, facts, now, calendars) -> SellDraftQuantityDecision`:
  1. `SignalTiming::check`로 신호 유효기간·제출 시각·캘린더를 확인한다 (`NoSellDraftQuantity::SignalUnavailable`).
  2. 포트폴리오에서 신호 종목의 보유를 찾는다. 없거나 수량이 `0`이면 `NoSellDraftQuantity::NotHeld`.
  3. `SignalTiming::check_holding`과 `holding.observed_at() >= signal.observed_at()`로 보유 사실의 최신성(60초 이내, 미래 거부, 과제 생성·신호 관측 이전 거부)을 확인한다 (`NoSellDraftQuantity::StaleHolding`).
  4. 보유 수량이 `signal.held_quantity()`와 다르면 `NoSellDraftQuantity::HoldingChanged`.
  5. 매도가능수량 사실이 없으면 `NoSellDraftQuantity::SellableQuantityUnknown`.
  6. 매도가능수량의 계좌·종목이 요청 계좌·신호 종목과 다르면 `NoSellDraftQuantity::DifferentFact`.
  7. 매도가능수량 관측 시각이 미래이거나 과제 생성 이전이거나 `now` 기준 60초(`SELLABLE_QUANTITY_MAX_AGE`)를 1ns라도 넘기면 `NoSellDraftQuantity::StaleSellableQuantity`.
  8. 매도가능수량이 `0`이면 `NoSellDraftQuantity::ZeroSellableQuantity`.
  9. 보유 수량과 매도가능수량 모두 정수 1주 단위(`WholeOrderQuantity`, 소수점 없는 양의 정수)인지 확인한다. 소수점 수량이 있으면 안전 기본값으로 초안을 만들지 않고 `NoSellDraftQuantity::FractionalQuantityUnsupported`.
  10. `sellable < held`이면 기본 수량은 `sellable`, 제약은 `SellQuantityConstraint::SellableQuantityCap`, 잠긴 수량(`locked_quantity`)은 `held - sellable`이다. 그렇지 않으면(`held <= sellable`) 기본 수량은 `held`, 제약은 `SellQuantityConstraint::FullHolding`, 잠긴 수량은 `0`이다.
- `SellDraftQuantityPlan`:
  - 필드는 비공개이며 `Debug`는 `"SellDraftQuantityPlan(..)"`만 출력한다.
  - `reduce_quantity(&self, requested: Quantity) -> Result<Self, SellDraftQuantityAdjustmentError>`:
    - 소수점이 있는 수량(`0.5`, `1.5` 등)은 `InvalidQuantityUnit`.
    - `0`(또는 `0.00`)은 `BelowMinimumUnit`.
    - 현재 계획 수량(`self.quantity()`)을 초과하면 `ExceedsAllowedQuantity` (기본 수량 초과뿐 아니라 이미 줄인 계획에서 다시 늘리는 것도 거부한다).
    - 통과하면 새 계획을 반환하며, `requested < default_quantity`이면 `SellQuantityAdjustment::Reduced`, `requested == default_quantity`이면 `SellQuantityAdjustment::Default`를 기록한다.
  - `check_facts_at(&self, now, calendars) -> Result<(), NoSellDraftQuantity>`: 저장 직전 시점(`write_at`)에 신호 유효기간·보유 관측 60초·매도가능수량 관측 60초가 여전히 유효한지 재확인한다.

## 애플리케이션 연결 (`crates/application`)

- `sell_limit_intent.rs`: `buy_limit_intent.rs`와 대칭되는 `sell_limit_intent(plan, limit, account, id) -> Result<OrderIntent, SellLimitIntentError>`를 추가한다. 지정가 방향(`OrderSide::Sell`), 계좌(`plan.account_id() == account`), 내부 종목 ID·심볼·시장 일치를 모두 확인한 뒤 `OrderTerms::KrLimit` / `OrderTerms::UsLimit`로 `OrderIntent`를 만든다. 매수 지정가와 달리 매도 수량은 주문 가격에 종속되지 않으므로 가격 상승에 따른 수량 재계산은 요구하지 않는다(지정가 호가 단위·상하한·5초 신선도는 `calculate_limit_price`가 검증한다).
- `draft_proposal.rs`:
  - `DraftProposalInputs`에 `pub sellable_quantity: Option<SellableQuantityFact>`를 추가한다.
  - `DraftProposalError::SellDraftUnavailable`를 제거하고 `SellQuantity(NoSellDraftQuantity)`와 `SellIntent(SellLimitIntentError)`로 대체한다.
  - `finish_attempt`의 매도 조기 반환을 제거하고, `make_proposal`에서 `RaisedSignal::Sell`을 `calculate_sell_draft_quantity` → `calculate_limit_price(..., OrderSide::Sell, ...)` → `sell_limit_intent` → `Proposal::new`로 연결한다.
  - 저장 직전 `recheck`와 `proposal.check_current(write_at, observation)` 직후 `plan.check_facts_at(write_at, calendars)`를 실행해 계산과 저장 사이에 보유·매도가능수량이 60초를 넘긴 경우 저장하지 않고 닫힌 실패한다.
  - 기존 `check_identity`의 `MasterOrderGateFacts::instrument_for(OrderSide::Sell)` 경로로 국내 정리매매(`LiquidationTrading`) 종목의 매도 초안은 허용하고, 거래정지·상장폐지·마스터 불일치는 거부한다.

## 상황 → 결과 표

| 상황 | 결과 | 비고 |
|---|---|---|
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 17주 | 매도 초안 생성 (`17주`, `FullHolding`, `locked=0`) | 최우선 매수호가 내림 지정가 |
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 11주 | 매도 초안 생성 (`11주`, `SellableQuantityCap`, `locked=6`) | 매도가능수량 상한 적용 |
| 열린 장, 범위 안 매도 신호, 보유 7주·매도가능 19주 | 매도 초안 생성 (`7주`, `FullHolding`, `locked=0`) | 보유 전량 초과 금지(ADR 0076 일관) |
| 매도가능수량 미확인(`None`) | 초안 미생성 (`SellQuantity(SellableQuantityUnknown)`) | 신호 유지, 수량 추정 금지 |
| 매도가능수량 `0` | 초안 미생성 (`SellQuantity(ZeroSellableQuantity)`) | 신호 유지 |
| 매도가능수량 60초 초과·미래·과제 이전 관측 | 초안 미생성 (`SellQuantity(StaleSellableQuantity)`) | 정확히 60초는 허용, 60초+1ns 거부 |
| 매도가능수량 계좌 또는 종목 불일치 | 초안 미생성 (`SellQuantity(DifferentFact)`) | 타 계좌·타 종목 사실 혼용 금지 |
| 보유 없음 또는 보유 0주 | 초안 미생성 (`SellQuantity(NotHeld)`) | 신호 평가·수량 계산 이중 방어 |
| 보유 관측 60초 초과·미래·신호 관측 이전 | 초안 미생성 (`SellQuantity(StaleHolding)`) | 저장 직전 재판정에서도 검증 |
| 보유 수량이 매도 신호 시점과 달라짐 | 초안 미생성 (`SellQuantity(HoldingChanged)`) | 재평가 전 불일치 초안 차단 |
| 국내·미국 소수점 보유 또는 미국 소수점 매도가능수량 | 초안 미생성 (`SellQuantity(FractionalQuantityUnsupported)`) | 안전 기본값(정의 대기) |
| 사용자 축소 `1 <= q < 기본 수량` (정수) | 허용 (`quantity = q`, `Reduced`) | 기본·잠긴 수량 기준 보존 |
| 사용자 축소 `q == 기본 수량` (정수) | 허용 (`quantity = q`, `Default`) | 축소 없음 상태 유지 |
| 사용자 축소 `q == 0` | 거부 (`BelowMinimumUnit`) | 최소 1주 규칙 |
| 사용자 축소 소수점(`0.5`, `1.5` 등) | 거부 (`InvalidQuantityUnit`) | 지정가 정수 수량 단위 규칙 |
| 사용자 수량 증가(`q > 현재 허용 수량`) | 거부 (`ExceedsAllowedQuantity`) | 기본 초과 및 축소 후 재증가 금지 |
| 매도 신호가 추정 가격 범위 밖 | 매도 신호 폐기 + 재추정 예약 | 기존 A-14 계약 유지 |
| 닫힌 장 / 불명 장의 매도 신호 | `MarketClosed` / `MarketUnknown` | 신호 유지, 초안 시도 0회 |
| 매수→매도 또는 매도→매수 방향 전환 | 이전 초안 대체 후 새 방향 초안 생성 | 단일 활성 초안 계약 |
| 계좌 전환 또는 구 계좌 지연 요청 | 새 계좌 전환 시 구 초안 대체, 구 계좌 요청 `AccountChanged` | U-22·ADR 0152 계약 |
| 국내 정리매매 종목의 매도 신호 | 매도 초안 생성 허용 | 매수는 차단, 청산 매도만 허용 |

## 정의 대기 (문서가 정하지 않은 선택지와 사용자 영향)

문서와 메인 결정이 아직 정하지 않은 세부 항목은 임의로 확정하지 않고, 안전한 쪽(초안을 만들지 않음 또는 보수적 상한)을 기본값으로 구현한 뒤 아래 선택지를 제안한다.

| 항목 | 현재 기본값(안전 쪽) | 선택지 | 사용자 영향 |
|---|---|---|---|
| 미국 소수점 주식 보유·매도가능수량(예: `10.5주`, `0.5주`) | `FractionalQuantityUnsupported`로 초안 미생성 | **A(현재 기본)**: 소수점 보유·매도가능수량은 자동 초안을 만들지 않고 사유 표시<br>**B**: 정수 부분(`floor`)만 지정가 매도 초안으로 만들고 잔여 소수 수량을 안내<br>**C**: 소수 매도가능수량을 미국 시장가 매도(`UsMarketSell`) 초안으로 생성(고급 설정 연동) | **A**: 의도치 않은 부분 청산·시장가 전환이 없으나 소수 보유자는 직접 매도해야 함<br>**B**: 대부분 수량을 지정가로 빠르게 청산 가능하나 잔여 소수 주식이 계좌에 남음<br>**C**: 전량 청산 가능하나 지정가 원칙(설계 6.3)과 어긋나고 슬리피지 위험이 생김 |
| 일부만 매도 가능한 잠긴 수량(`locked_quantity > 0`)의 화면 표시 | 도메인 계획에 `held_quantity`, `sellable_quantity`, `locked_quantity`, `constraint(SellableQuantityCap)`를 노출하고 초안 수량은 `sellable_quantity`로 제한 | **A**: 초안 카드에 "보유 N주 중 매도 가능 M주(미체결·잠김 K주 제외)" 한 줄 안내<br>**B**: 초안 카드에는 주문 수량 M주만 표시하고 상세 펼침에만 보유·잠김 수량 표시<br>**C**: 잠긴 수량이 있으면 초안 자체를 만들지 않고 미체결 주문 확인을 먼저 요구 | **A**: 사용자가 전량이 아닌 이유를 한눈에 이해(단순함 원칙 P-8의 필요한 결과만 표시)<br>**B**: 카드가 단순하나 사용자가 전량 매도되는 것으로 오해할 수 있음<br>**C**: 중복 주문 위험은 최저이나 일부 미체결이 걸린 종목의 매도 초안이 막힘 |
| 매도가능수량 관측 신선도 한도 | `60초`(`SELLABLE_QUANTITY_MAX_AGE`, 보유·매수가능금액과 동일) | **A(현재 기본)**: 보유·매수가능금액과 동일한 60초<br>**B**: 시세·호가와 동일한 5초로 강화<br>**C**: 조회 시점 재확인(승인 직전 O-04에서 필수 재조회) 전제로 60초 유지 | **A/C**: 초안 생성 시점의 불필요한 실패를 줄이면서 1분 초과 낡은 잔고 차단(O-04 승인 직전 재조회가 최종 방어)<br>**B**: 잔고 API 응답 지연·캐시 주기에서 잦은 초안 실패 가능 |
| `매도가능수량 > 보유 수량` 스냅숏 불일치 | `min(보유, 매도가능) = 보유 수량`(`FullHolding`, `locked=0`, ADR 0076 일관) | **A(현재 기본)**: 두 값 중 작은 보유 수량 전량으로 제한해 초과 매도 차단<br>**B**: 보유·매도가능 스냅숏 불일치로 간주해 초안 미생성 후 재조회 유도 | **A**: 체결 직후 일시적 조회 시차에서도 보수적 하한으로 초안 생성 가능하며 보유 초과 없음<br>**B**: 최신 체결분까지 한 번에 반영하려다 일시 실패가 잦아질 수 있음 |

## 인계

- **O-04 / U-xx (승인 전 수량 축소 UI 및 승인 직전 재검증)**: 사용자가 승인 화면에서 수량을 줄일 때 도메인의 `SellDraftQuantityPlan::reduce_quantity`를 통해 검증하고, 승인·전송 직전에 최신 보유·매도가능수량을 재조회해 축소 수량이 여전히 유효한지 확인한다.
- **어댑터(`DraftProposalData` 구현체)**: 선택 계좌의 최신 `BrokerRead::sellable_quantity`를 조회해 `DraftProposalInputs.sellable_quantity`로 공급한다. 조회 실패·미지원 시 추정값을 채우지 않고 `None` 또는 오류로 닫힌 실패한다.
- **정의 대기 결정 반영**: 위 표의 4개 항목에 대한 사용자·메인 결정이 내려지면 ADR을 확정 상태로 갱신한다.
