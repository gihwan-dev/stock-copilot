# ADR 초안 D-57: 매도 초안의 기본 수량 규칙과 A-14 매도 경로 연결

- 상태: 초안 (2026-11-24, 번호는 메인 병합 때 부여)
- 관련 작업: D-57 (`docs/tasks.md`), A-14 (ADR 0152), D-26 (ADR 0076), D-34, D-56 (ADR 0159)

## 배경과 결정

ADR 0152(A-14)는 매도 신호를 평가·보존하면서도 기본 수량 규칙이 확정되지 않아 `DraftProposalError::SellDraftUnavailable`로 초안 생성을 보류했다. 메인은 D-57과 후속 결정에서 다음을 확정했다.

1. 매도 초안의 기본 수량 목표는 보유 수량 전량(`held_quantity`)이며 상한은 매도가능수량(`sellable_quantity`)이다. 즉 기본 수량은 `min(보유 수량, 매도가능수량)`이다. 조회 시차로 `매도가능수량 > 보유 수량`인 경우에도 보유 수량 전량을 넘기지 않는다(ADR 0076 예제 `보유 7, 매도가능 19 → 7주`와 일관).
2. 매도가능수량이 `0`이거나 알 수 없으면(`None`) 수량을 추정하지 않고 초안을 만들지 않으며 닫힌 사유를 남긴다.
3. 승인 전에 사용자가 수량을 줄일 수 있고, 입력을 고칠 때 `1..=기본 수량` 범위 안에서는 다시 늘릴 수 있다(지정가 정수 1주 단위만 허용). 기본 수량 초과만 거부한다.
4. 계산과 판정은 `crates/domain`에서 부동소수점 없이 기존 도메인 수량·시각·식별자 타입(`Quantity`, `WholeOrderQuantity`, `SellableQuantityFact`, `SellSignal`, `PortfolioValuation`, `AccountId`, `Timestamp`)만으로 수행한다.
5. `crates/application`의 `DraftProposal`에서 `SellDraftUnavailable` 조기 반환을 제거하고 매도 초안 생성 경로를 연결한다. 단일 초안, 선택 계좌 대조·대체, 스냅숏·보정 원장 일치, 범위 밖 폐기·재추정 예약, 닫힌·불명 장 보존, 만료 후 재생성, 저장 직전 재판정과 시계 역행 거부, 그리고 **D-56(ADR 0159)의 저장 신호 ↔ 라이브 신호 대조(매도만 의도적으로 허용한 보유 관측 시각 전진 `>= stored.observed_at()` 외에는 매수와 동일한 강도)·`StoredEstimateEvidence::verify_matches_task`·`DraftRestoreEvidence::capture`·현재 마스터 이중 검증(`DraftStoreCandidate`, `DraftCheck.master`, `draft(id, master)`)**을 적용하며 매도 전용 우회 경로를 두지 않는다.

## ADR 0076(D-26)과의 관계 및 A-14 인계 대체 근거

확정 ADR 0076(D-26)은 A-14에 `calculate_sell_quantity` → `bind_sell_limit` → `sell_order_into_intent(CalculatedSellOrder, ClientOrderId)` 경로를 인계했다. 그러나 D-57의 A-14 매도 초안 생성 경로에서는 해당 인계를 `SellDraftQuantityPlan` + `application::sell_limit_intent`로 대체(supersede)한다. 기술적 근거는 세 가지다.

1. **내부 종목 ID 대조 누락**: D-26의 `SellQuantityPlan`·`bind_sell_limit`는 `ResolvedOrderInstrument`(심볼·시장)만 비교하고 내부 `InstrumentId`를 대조하지 않는다. ADR 0152(A-14)는 심볼 재사용·마스터 불일치 차단을 위해 내부 `InstrumentId`·심볼·시장 전체 일치를 요구하며, 매수 경로(`buy_limit_intent`)도 동일 계약을 따른다.
2. **미국 소수 보유 처리 충돌**: D-26의 `calculate_sell_quantity(..., SellQuantityMode::Whole)`은 미국 소수 보유(`10.5주`)를 조용히 내림(`10주`)해 성공시킨다. 반면 D-57은 메인 결정 1에 따라 소수점 주식은 초안을 만들지 않고 닫힌 사유(`FractionalQuantityUnsupported`)를 남긴다.
3. **초안 계약 필드 부재**: D-26은 `SellSignal` 결합, 보유·매도가능수량 60초 신선도 및 저장 직전 재판정(`check_facts_at`), `Option<SellableQuantityFact>`의 미확인 사유 구분, 잠긴 수량(`locked_quantity`), 사용자 승인 전 수량 조정(`reduce_quantity`)을 표현하지 않는다.

**공존과 정본 범위**:
- A-14 매도 지정가 초안 생성과 승인 전 수량 조정 검증은 **D-57(`SellDraftQuantityPlan` + `sell_limit_intent`)**을 정본으로 사용한다.
- 기존 D-26(`SellQuantityPlan`, `bind_sell_market`, `MarketOrderPolicy`)은 기존 테스트 호환과 향후 고급 설정 기반 시장가·미국 소수 시장가 경로(A-17/O-14)의 검토 자산으로 유지한다. O-01/O-04에서 시장가·소수점 매도 경로가 도입될 때 D-26과 D-57의 최종 통합 또는 분리 범위를 정리한다.

## 도메인 설계 (`crates/domain`)

`BuyQuantityPlan`·`calculate_buy_quantity_with_history`·`HistoricalSignal`과 대칭되는 봉인된 타입과 함수를 둔다.

- `HistoricalSignal::matches_live(&self, live: &RaisedSignal) -> CheckResult` (`signal_restore.rs`):
  - D-56의 저장된 `HistoricalSignal` ↔ 재평가 라이브 `RaisedSignal` 대조를 기준으로 삼고, 매도에서만 필요한 관측 시각 전진 허용을 도메인에 둔다.
  - 매수(`Buy`)는 D-56 그대로 `HistoricalSignal::from_signal(live) == *self`로 완전 일치 여부를 판정한다.
  - 매도(`Sell`)는 `sell_signal_matches_reassessed(&stored, fresh)`에 위임해 `facts`와 `held_quantity`가 같고 `fresh.observed_at() >= stored.observed_at()`이면 통과, 변동·역행은 실패시킨다.
  - `HistoricalSignal::sell_signal`은 `pub(crate)`로 봉인해 `crates/domain` 밖으로 노출하지 않으며 `crates/domain/tests/d57_compile_boundary.rs`에서 컴파일 경계(`E0624`·`E0308`)를 고정한다.
- `signal_transition` (`signal_decision.rs`, 메인 결정 6):
  - `SellSignal::PartialEq`는 D-56 복원·오프셋 검증의 정확성을 위해 그대로 보존하되, `signal_transition`(및 `signal_transition_from_history`)에서 이전·이후 모두 매도일 때 `match sell_signal_matches_reassessed(old_sell, new_sell)`로 `Passed`이면(관측 시각만 전진) `SignalTransition::Unchanged`, `Failed | Unknown`이면 `SignalTransition::Updated`를 반환한다.
  - 저장소(실제 SQLite 저장소·메모리 가짜·공통 계약 시나리오)에서 `Unchanged`일 때 리비전과 기존 활성 초안을 유지하는 연결은 S-22에 인계한다.
- `SellDraftQuantityFacts<'facts>` (`sell_draft_quantity.rs`): 계좌(`AccountId`), 포트폴리오 평가(`PortfolioValuation`), 선택적 매도가능수량(`Option<&SellableQuantityFact>`)을 묶는다. `Debug`는 내부 값을 노출하지 않는다.
- `calculate_sell_draft_quantity(signal, facts, now, calendars)` 및 `calculate_sell_draft_quantity_with_history(live, stored, facts, now, calendars) -> SellDraftQuantityDecision`:
  - `DraftProposal`은 `calculate_sell_draft_quantity_with_history`를 사용해 라이브 `SellSignal`로 보유·수량을 계산하되, 저장된 `HistoricalSignal::Sell`이 `live`와 일치하는지 확인하고 병렬 브로커 조회를 깨뜨리지 않도록 매도가능수량 하한 시각(`min_sellable_observed_at`)을 저장된 신호의 관측 시각(`stored.observed_at()`)으로 둔다(메인 결정 5).
  1. `SignalTiming::check`로 신호 유효기간·제출 시각·캘린더를 확인한다 (`NoSellDraftQuantity::SignalUnavailable`).
  2. 포트폴리오에서 신호 종목의 보유를 찾는다. 없거나 수량이 `0`이면 `NoSellDraftQuantity::NotHeld`.
  3. `SignalTiming::check_holding`과 `holding.observed_at() >= signal.observed_at()`로 보유 사실의 최신성(60초 이내, 미래 거부, 과제 생성·신호 관측 이전 거부)을 확인한다 (`NoSellDraftQuantity::StaleHolding`).
  4. 보유 수량이 `signal.held_quantity()`와 다르면 `NoSellDraftQuantity::HoldingChanged`.
  5. 매도가능수량 사실이 없으면 `NoSellDraftQuantity::SellableQuantityUnknown`.
  6. 매도가능수량의 계좌·종목이 요청 계좌·신호 종목과 다르면 `NoSellDraftQuantity::DifferentFact`.
  7. 매도가능수량 관측 시각이 미래이거나 `min_sellable_observed_at` 이전(이는 `task.created_at()` 이전도 함께 차단)이거나 `now` 기준 60초(`SELLABLE_QUANTITY_MAX_AGE`)를 1ns라도 넘기면 `NoSellDraftQuantity::StaleSellableQuantity`.
  8. 매도가능수량이 `0`이면 `NoSellDraftQuantity::ZeroSellableQuantity`.
  9. 보유 수량과 매도가능수량 모두 정수 1주 단위(`WholeOrderQuantity`, 소수점 없는 양의 정수)인지 확인한다. 소수점 수량이 있으면 초안을 만들지 않고 `NoSellDraftQuantity::FractionalQuantityUnsupported`.
  10. `sellable < held`이면 기본 수량은 `sellable`, 제약은 `SellQuantityConstraint::SellableQuantityCap`, 잠긴 수량(`locked_quantity`)은 `held - sellable`이다. 그렇지 않으면(`held <= sellable`) 기본 수량은 `held`, 제약은 `SellQuantityConstraint::FullHolding`, 잠긴 수량은 `0`이다.
- `SellDraftQuantityPlan`:
  - 필드는 비공개이며 `Debug`는 `"SellDraftQuantityPlan(..)"`만 출력한다.
  - `reduce_quantity(&self, requested: Quantity) -> Result<Self, SellDraftQuantityAdjustmentError>` (메인 결정 4):
    - 소수점이 있는 수량(`0.5`, `1.5` 등)은 `InvalidQuantityUnit`.
    - `0`(또는 `0.00`)은 `BelowMinimumUnit`.
    - `self.default_quantity()`를 초과하면 `ExceedsAllowedQuantity`. 이미 줄인 계획에서도 사용자가 입력을 고칠 수 있도록 `1..=default_quantity` 범위 안에서는 다시 늘릴 수 있다.
    - 통과하면 새 계획을 반환하며, `requested < default_quantity`이면 `SellQuantityAdjustment::Reduced`, `requested == default_quantity`이면 `SellQuantityAdjustment::Default`를 기록한다.
  - `check_facts_at(&self, now, calendars) -> Result<(), NoSellDraftQuantity>`: 저장 직전 시점(`write_at`)에 신호 유효기간·보유 관측 60초·매도가능수량 관측 60초가 여전히 유효한지 재확인한다.

## 애플리케이션 연결 (`crates/application`)

- `sell_limit_intent.rs`: `buy_limit_intent.rs`와 대칭되는 `sell_limit_intent(plan, limit, account, id) -> Result<OrderIntent, SellLimitIntentError>`를 추가한다. 지정가 방향(`OrderSide::Sell`, exhaustive `match`), 계좌(`plan.account_id() == account`), 내부 종목 ID·심볼·시장 일치를 모두 확인한 뒤 `OrderTerms::KrLimit` / `OrderTerms::UsLimit`로 `OrderIntent`를 만든다. 매수 지정가와 달리 매도 수량은 주문 가격에 종속되지 않으므로 가격 변동에 따른 수량 재계산은 요구하지 않는다(지정가 호가 단위·상하한·5초 신선도는 `calculate_limit_price`가 검증한다).
- `draft_proposal.rs`:
  - `DraftProposalInputs`에 `pub sellable_quantity: Option<SellableQuantityFact>`를 추가한다.
  - `DraftProposalError::SellDraftUnavailable`를 제거하고 `SellQuantity(NoSellDraftQuantity)`와 `SellIntent(SellLimitIntentError)`로 대체한다.
  - `prepare_open`은 D-56의 `live_signal` 재평가 뒤 `current.signal.matches_live(&live_signal)`을 exhaustive `match`로 대조한다(`CheckResult::Failed | CheckResult::Unknown => SignalChanged`).
  - `finish_attempt`는 매수·매도 구분 없이 `check_existing_draft`(현재 마스터 포함 `DraftCheck`) → `make_proposal` → `recheck` → `check_current`(+ 매도는 `plan.check_facts_at`) → `store_candidate`(`DraftStoreCandidate { proposal, evidence }` + `DraftCheck { .., master }`)를 거친다.
  - `make_buy_proposal`과 `make_sell_proposal` 모두 동일한 `capture_draft_evidence(&proposal, &prepared.live_signal, &prepared.inputs)`를 호출해 `StoredEstimateEvidence::verify_matches_task`와 `DraftRestoreEvidence::capture`를 강제한다(매도 전용 우회 경로 없음).
  - 국내 정리매매(`LiquidationTrading`) 매도 초안은 `EstimateRestoreEvidence.side = OrderSide::Sell`인 청산 매도 과제와 매도 주문 게이트(`instrument_for(OrderSide::Sell)`)로 생성·복원된다. 단, 정리매매 지정 전에 `EstimateRestoreEvidence.side = OrderSide::Buy`로 발급·저장된 기존 과제는 D-56의 `EstimateRestoreEvidence::replay(LiveOrderGate)` 계약상 `LiquidationBuy`로 닫힌 실패(초안 생성 거부 및 `EvaluateSignals` 재평가 무효화)하며, 이는 우회가 아닌 D-56/E-01 과제 발급 방향 계약 범위로 후속 인계한다.
  - 저장소 현재 마스터가 거래정지·상장폐지·상장예정·미지 증권 유형·한국 거래 상태 누락으로 전환되면 낡은 호출자 마스터로도 매도 초안 생성·`current_draft`·`store_current_draft`·`draft` 조회가 모두 닫힌 실패(`RepositoryIntegrity`)한다.

## 상황 → 결과 표

| 상황 | `DraftProposal` 결과 | 도메인 직접 호출(`calculate_sell_draft_quantity`) / 비고 |
|---|---|---|
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 17주 | 매도 초안 생성 (`17주`, `FullHolding`, `locked=0`) | 최우선 매수호가 내림 지정가, `DraftRestoreEvidence` 저장 |
| 열린 장, 범위 안 매도 신호, 보유 17주·매도가능 11주 | 매도 초안 생성 (`11주`, `SellableQuantityCap`, `locked=6`) | 매도가능수량 상한 적용 |
| 열린 장, 범위 안 매도 신호, 보유 7주·매도가능 19주 | 매도 초안 생성 (`7주`, `FullHolding`, `locked=0`) | 보유 전량 초과 금지(결정 1·ADR 0076) |
| 평가 후 최신 잔고 재조회(동일 보유 17주, `t_new >= t_sig`, 60초 이내) | 매도 초안 생성 | 장 닫힘→열림·만료 재생성 및 병렬 조회(`t_sig <= t_sellable < t_holding`) 포함 |
| 평가 후 보유 수량 변동(`17→15`) 또는 과거 관측 역행(`task.created_at() <= t_old < t_sig`) | 초안 미생성 (`SignalChanged`) | 선행 재평가(`prepare_open`)의 `HistoricalSignal::matches_live`가 차단. 도메인 직접 호출 시 `HoldingChanged` / `StaleHolding` |
| 평가 후 보유 없음 또는 보유 0주 | 초안 미생성 (`SignalUnavailable(UnheldLowProbability)`) | 선행 재평가(`prepare_open`)에서 미보유 판정. 도메인 직접 호출 시 이중 방어 `NotHeld` |
| 평가 후 보유 관측이 60초 초과·미래·과제 생성 이전 | 초안 미생성 (`SignalUnavailable(Unavailable(StaleHolding))`) | 선행 재평가(`prepare_open`)에서 낡은/미래 잔고 차단. 도메인 직접 호출 시 `StaleHolding` |
| 선행 재평가 통과 후 `calculated_at`·`write_at` 사이 보유·매도가능 60초 초과 | 초안 미생성 (`SellQuantity(StaleHolding)` / `SellQuantity(StaleSellableQuantity)`) | `calculate_sell_draft_quantity_with_history` 및 `plan.check_facts_at(write_at)` 저장 직전 재판정이 차단 |
| 선행 재평가 통과 후 로컬 시계 역행(`calculated_at < submitted_at` 또는 `write_at < calculated_at`) | `SellQuantity(SignalUnavailable(Unavailable(SubmissionFromFuture)))` / `DraftStatus(ClockRollback)` | `write_at` 역행은 `proposal.check_current(write_at, observation)`이 차단하며 저장소 역행과 동일하게 닫힌 실패 |
| 만료 전 활성 매도 초안 존재 상태에서 매도가능수량 감소·`0`·미확인(`None`) 전환 | 기존 매도 초안 재사용 (`Existing`) | A-14 매수 경로와 동일하게 `check_existing_draft`가 선행하며, 승인 직전 최종 수량 상한은 결정 3·O-04에서 재조회·검증 |
| 매도가능수량 미확인(`None`, 활성 초안 없음) | 초안 미생성 (`SellQuantity(SellableQuantityUnknown)`) | 신호 유지, 수량 추정 금지 |
| 매도가능수량 `0` (활성 초안 없음) | 초안 미생성 (`SellQuantity(ZeroSellableQuantity)`) | 신호 유지 |
| 매도가능수량 60초 초과·미래·저장 신호 관측 이전 | 초안 미생성 (`SellQuantity(StaleSellableQuantity)`) | 정확히 60초는 허용, 60초+1ns 거부. 저장 직전 재판정에서도 검증 |
| 매도가능수량 계좌 또는 종목 불일치 | 초안 미생성 (`SellQuantity(DifferentFact)`) | 타 계좌·타 종목 사실 혼용 금지 |
| 국내·미국 소수점 보유 또는 미국 소수점 매도가능수량 | 초안 미생성 (`SellQuantity(FractionalQuantityUnsupported)`) | 메인 결정 1 |
| 사용자 조정 `1 <= q < 기본 수량` (정수, 축소 후 재증가 포함) | 도메인 허용 (`quantity = q`, `Reduced`) | 기본·잠긴 수량 기준 보존(메인 결정 4) |
| 사용자 조정 `q == 기본 수량` (정수, 축소 후 복원 포함) | 도메인 허용 (`quantity = q`, `Default`) | 축소 없음 상태 복원(메인 결정 4) |
| 사용자 조정 `q == 0` / 소수점(`0.5`, `1.5`) / 기본 수량 초과(`q > default`) | 도메인 거부 (`BelowMinimumUnit` / `InvalidQuantityUnit` / `ExceedsAllowedQuantity`) | 최소 1주·정수 단위·기본 수량 상한 |
| 복원 근거 누락(`restoration: None`) 또는 추정 근거 불일치 | `Evaluation(RestorationUnavailable)` / `Restoration(Estimate(_))` | D-56 매수와 동일 강도, 매도 우회 없음 |
| 저장소 현재 마스터 비거래 전환(거래정지·상장폐지·상장예정·미지 유형·한국 상세 누락) 뒤 낡은 마스터로 생성·조회 | `Storage(RepositoryIntegrity)` / `draft`·`current_draft`·`store_current_draft` 모두 거부 | D-56 이중 마스터 검증, 기존 유효 초안 미훼손 |
| 매도 신호가 추정 가격 범위 밖 | 매도 신호 폐기 + 재추정 예약 | 기존 A-14 계약 유지 |
| 닫힌 장 / 불명 장의 매도 신호 | `MarketClosed` / `MarketUnknown` | 신호 유지, 초안 시도 0회 |
| 매수→매도 또는 매도→매수 방향 전환 | 이전 초안 대체 후 새 방향 초안 생성 | 단일 활성 초안 계약 |
| 계좌 전환 또는 구 계좌 지연 요청 | 새 계좌 전환 시 구 초안 대체, 구 계좌 요청 `AccountChanged` | U-22·ADR 0152 계약 |
| 국내 정리매매 종목의 매도 신호(`OrderSide::Sell` 과제 근거) | 매도 초안 생성·복원 조회 허용 | 매수는 차단, 청산 매도만 허용. 정리매매 전 `OrderSide::Buy` 근거 과제는 D-56 계약상 `LiquidationBuy`로 닫힌 실패(D-56/E-01 후속 인계) |

## 메인 결정 (확정)

1차·2차 리뷰에서 정의 대기로 올렸던 7개 항목에 대해 메인이 다음 결정을 확정했다.

| # | 항목 | 메인 결정 | 구현 및 인계 범위 |
|---|---|---|---|
| 1 | 미국·국내 소수점 주식 보유·매도가능수량 | **A**: 초안 미생성·사유 표시(`FractionalQuantityUnsupported`) | D-57 도메인 구현·테스트 완료 |
| 2 | 잠긴 수량(`locked_quantity > 0`) 화면 표시와 초안 메타데이터 저장 | **A**: 초안 카드에 보유·매도가능·잠김 수량 한 줄 안내 | 도메인 계획에 `held_quantity`·`sellable_quantity`·`locked_quantity`·`constraint` 노출 완료. 저장·화면 계약은 **S-22·U 작업 인계** |
| 3 | 승인 직전(O-04) 재계산 시 '늘리기 금지' 상한 기준 | **A**: 초안 만료(기본 10분) 내 승인·전송 직전 최신 잔고·매도가능수량 재조회(관측 신선도 60초) 후 `min(초안 카드 수량, 최신 매도가능수량, 최신 보유 수량)` 상한 고정 | **O-04 인계** (도메인 카드 수량 상한 결합 함수 추가 포함) |
| 4 | 이미 축소한 계획에서 기본 수량 이하 재증가(입력 정정) | **B**: 사용자가 입력을 고칠 수 있어야 하므로 `1..=기본 수량` 범위 안에서는 다시 늘릴 수 있고, 기본 수량 초과만 거부 | D-57 도메인 `SellDraftQuantityPlan::reduce_quantity` 구현·예제·속성 테스트 완료 |
| 5 | 매도가능수량 관측 신선도 한도 및 보유 관측과의 순서 | **A**: 60초(`SELLABLE_QUANTITY_MAX_AGE`), 저장 신호 관측 시각 이후(`>= stored.observed_at()`, 두 브로커 API 병렬 조회 허용) | D-57 도메인 `calculate_sell_draft_quantity_with_history`·`check_facts_at` 구현·테스트 완료 |
| 6 | `EvaluateSignals` 재평가 시 관측 시각만 바뀐 매도 신호의 초안 유지 | **A**: `facts`·`held_quantity`가 같고 `observed_at`만 전진하면 같은 초안 유지(`SignalTransition::Unchanged`) | D-57 도메인 `signal_transition` 판정 구현·테스트 완료. 저장소(SQLite·메모리 가짜·공통 계약 시나리오) 연결은 **S-22 인계** |
| 7 | 사용자가 수량을 줄여 승인한 뒤 남은 매도가능수량의 자동 재제안 | **A**: 동일 매도 신호에서 1회 축소 승인한 뒤에는 다음 AI 제출 전까지 잔여 수량을 자동 재제안하지 않음 | 규칙 확정 기록 완료. 소비 기록 기반 재제안 억제 구현은 **O-04 인계** |

## 인계

- **O-04 / U-xx (승인 전 수량 조정 UI, 승인 직전 재검증 및 축소 승인 후 억제)**:
  - 사용자가 승인 화면에서 수량을 줄이거나 `1..=기본 수량` 안에서 고칠 때 도메인의 `SellDraftQuantityPlan::reduce_quantity`로 검증한다(메인 결정 4).
  - 현재 `Proposal<Draft>`는 `SellDraftQuantityPlan`을 보관하지 않고 `SellDraftQuantityPlan::reduce_quantity`의 상한은 자기 계획의 `default_quantity`다. 따라서 **승인·전송 직전**(초안 만료 기본 10분 안 언제든, 60초는 관측 신선도 한도)에는 O-04가 최신 보유·매도가능수량으로 재계산한 계획만 바로 쓰지 말고, `Proposal<Draft>`의 저장된 카드 수량(`HistoricalWholeQuantity`)과 최신 `SellDraftQuantityPlan`을 도메인 내부에서 결합하는 보조 함수(예: `SellDraftQuantityPlan::cap_to_draft`)를 도입해 **메인 결정 3(`min(초안 카드 수량, 최신 매도가능수량, 최신 보유 수량)`)**을 상한으로 강제한다.
  - **메인 결정 7**에 따라 사용자가 수량을 줄여 승인·소비한 매도 신호에서는 다음 AI 제출 전까지 남은 매도가능수량을 자동 재제안하지 않도록 O-04 소비 기록과 연계한다(체결로 보유 수량이 바뀐 뒤 재평가가 `Updated`로 흐르는 경로 포함).
- **S-22 / S-18 / U 작업 (저장소·메모리 가짜·공통 계약·카드 표시)**:
  - **메인 결정 2**에 따라 잠긴 수량(`locked_quantity > 0`)이 있을 때 초안 카드에 "보유 N주 중 매도 가능 M주(미체결·잠김 K주 제외)" 한 줄 안내를 표시하도록 저장소 메타데이터 또는 조회 계약을 정의한다.
  - **메인 결정 6**에 따라 도메인 `signal_transition`이 관측 시각만 전진한 매도 신호를 `SignalTransition::Unchanged`로 반환한다. 현재 `MemorySignalEvaluationRepository`(`crates/testkit-ports/src/memory/signal_repository.rs`)는 `Unchanged` 커밋에도 리비전을 올려 기존 초안을 대체하므로, S-22 실제 저장소와 함께 **메모리 가짜 및 공통 계약 시나리오(`crates/testkit-ports/src/contracts/scenarios.rs`)**에서 `Unchanged` 커밋 시 신호 리비전과 기존 활성 매도 초안을 유지하도록 연결한다.
  - D-56(ADR 0159)과 동일하게 저장소 `store_current_draft`·`current_draft`·`draft`는 매도 초안에도 저장소 현재 마스터와 호출자 마스터 이중 검증 및 `DraftRestoreEvidence` 검증을 적용한다.
- **D-56 후속 / E-01·A-12 (정리매매 전환 시 기존 매수 측 근거 과제 처리)**:
  - 정상 거래 상태에서 기본 `EstimateRestoreEvidence.side = OrderSide::Buy`로 발급·평가된 종목에 매도 신호가 저장된 뒤 해당 종목이 정리매매(`LiquidationTrading`)로 지정되면, D-56의 `EstimateRestoreEvidence::replay(LiveOrderGate)`가 과제 근거의 `side: Buy`로 현재 마스터를 검사해 `LiquidationBuy`로 닫힌 실패한다. D-56 후속 또는 E-01·A-12에서 (a) 실거래 복원이 생성하려는 신호·초안 방향(`OrderSide`)으로 현재 마스터 게이트를 검사하도록 확장하거나 (b) 정리매매 진입 시 `OrderSide::Sell` 전용 청산 과제를 새로 발급하는 정책을 확정한다.
- **어댑터(`DraftProposalData` 구현체)**:
  - `DraftProposalData::inputs`는 저장된 매도 신호의 `observed_at`보다 이전이 아닌(`observed_at >= stored.observed_at()`) 60초 이내 최신 보유 관측(`evaluation.valuation`)과 최신 매도가능수량(`sellable_quantity`)을 공급한다(보유와 매도가능수량의 병렬 조회는 허용). 보유 기본 5분 캐시 등 `stored.observed_at()` 이전 스냅숏을 반환하면(`Conflict` 재시도 직후 포함) `SignalChanged`·`StaleHolding`·`StaleSellableQuantity`로 닫힌 실패한다.
  - D-56과 동일하게 매도 초안에도 `evaluation.restoration`(`SignalRestorationBasis`)을 반드시 공급한다.
  - 선택 계좌의 최신 `BrokerRead::sellable_quantity` 조회 실패는 반드시 `Err(SystemPortError)`로 반환해 `DraftProposalError::Inputs`가 원인을 보존하도록 하며 `None`으로 삼키지 않는다. `None`은 엔드포인트 미제공·미지원 또는 매수 경로 생략에만 사용한다.
- **메인 병합 시 ADR 번호 부여**:
  - 이 초안 파일(`docs/adr/d-57-sell-draft-quantity.md`)에 정식 ADR 번호를 부여해 파일 이름을 바꿀 때 `node harness/codex-hooks/config.mjs --write`와 `node harness/claude-hooks/hashes.mjs --write`를 함께 실행해 `.codex/config.toml`과 `harness/claude-hooks/hashes.json`을 동기화한다.

## 변이 검증 기록

### 1. 자동 변경분 변이 (`just mutants-daily 8a1b8bbce95efeda6d740647c214e17109c2214d 0/1`, 대상 `e2ec66e3`)

- 결과 원문: `82 mutants tested in 31m: 49 caught, 33 unviable` / `mutants: 위반 없음` (미검출 0, 타임아웃 0)

### 2. 병합 후 신규·변경 규칙 손 변형 (11건, 대상 `e2ec66e3`)

격리 복사본(`target/D-57/mut-copy`, 실행 뒤 삭제)에서 D-56 병합과 메인 결정 4·5·6으로 새로 도입·변경된 규칙 11곳을 변형해 컴파일 및 테스트 실행으로 11/11 검출을 확인했다(미검출 0, 컴파일 실패 0).

| # | 파일 | 변형 내용 | 검출 테스트 |
|---|---|---|---|
| P-01 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `> self.default_quantity` → `>= self.default_quantity` | `reduce_quantity_enforces_kr_and_us_bounds` |
| P-02 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `> self.default_quantity` → `> self.quantity` (결정 4B 재증가 차단) | `reduce_quantity_enforces_kr_and_us_bounds` |
| P-03 | `crates/domain/src/sell_draft_quantity.rs` | `calculate_sell_draft_quantity_with_history`: `stored_sell.facts() != live.facts()` 제거 | `with_history_rejects_mismatched_and_regressed_stored_signals` |
| P-04 | `crates/domain/src/sell_draft_quantity.rs` | `calculate_sell_draft_quantity_with_history`: `stored_sell.held_quantity() != live.held_quantity()` 제거 | `with_history_accepts_same_observation_time_and_rejects_stored_live_holding_drift` |
| P-05 | `crates/domain/src/sell_draft_quantity.rs` | `calculate_sell_draft_quantity_with_history`: `live.observed_at() < stored_sell.observed_at()` → `<=` | `with_history_accepts_same_observation_time_and_rejects_stored_live_holding_drift` |
| P-06 | `crates/domain/src/sell_draft_quantity.rs` | `calculate_sell_draft_quantity_with_history`: 하한 `stored_sell.observed_at()` → `live.observed_at()` (결정 5A 병렬 조회) | `with_history_allows_parallel_sellable_after_stored_signal_and_rejects_mismatches` |
| P-07 | `crates/domain/src/signal_restore.rs` | `HistoricalSignal::matches_live`: 매수 분기 `Self::from_signal(live) == *self` → `true` | `historical_signal_matches_live_checks_buy_sell_and_cross_direction` |
| P-08 | `crates/domain/src/signal_restore.rs` | `HistoricalSignal::matches_live`: 매도 분기를 `&stored.0 == fresh`로 축소 | `historical_signal_matches_live_checks_buy_sell_and_cross_direction` |
| P-09 | `crates/domain/src/signal_decision.rs` | `signal_transition`: 매도 `Passed => Unchanged` → `Updated(value)` (결정 6A) | `d57_sell_transition_keeps_unchanged_on_forward_observation_and_updates_on_change` |
| P-10 | `crates/application/src/draft_proposal.rs` | `check_identity`: `current.signal.side()` → `OrderSide::Buy` (정리매매 매도 게이트) | `sell_draft_allows_liquidation_trading_and_rejects_suspended_master` |
| P-11 | `crates/application/src/draft_proposal.rs` | `prepare_open`: `current.signal.matches_live(&live_signal)` → `CheckResult::Passed` | `buy_signal_reassessment_rejects_changed_buy_limit` |

### 3. 병합 전 유효 유지 손 변형 (24건, 초기 2차 `ffec2025` 기준 28건 중 4건 대체)

초기 28건 중 결정 4B·5A와 D-56 `HistoricalSignal::matches_live` 도입으로 대체된 4건(`M-06`, `M-07`, `M-17`, `M-28`)은 위 `P-01`, `P-02`, `P-06`, `P-08`, `P-11`로 교체됐으며, 나머지 24건은 코드와 테스트가 그대로 유지되어 유효하다(현행 유효 누적 35/35 검출).

| # | 파일 | 변형 내용 | 검출 테스트 |
|---|---|---|---|
| M-01 | `crates/domain/src/domain_time.rs` | `SELLABLE_QUANTITY_MAX_AGE` `60s -> 61s` | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-02 | `crates/domain/src/domain_time.rs` | `SELLABLE_QUANTITY_MAX_AGE` `60s -> 59s` | `check_facts_at_revalidates_signal_holding_and_sellable_freshness` |
| M-03 | `crates/domain/src/quantity.rs` | `whole_excess`: `saturating_sub -> saturating_add` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-04 | `crates/domain/src/quantity.rs` | `whole_excess`: `.max(0_i128)` 제거 | `property_default_quantity_and_reduction_match_independent_reference` |
| M-05 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `scale() != 0` → `> 1` | `reduce_quantity_rejects_zero_fractions_and_handles_single_share` |
| M-08 | `crates/domain/src/sell_draft_quantity.rs` | `reduce_quantity`: `< self.default_quantity` → `<=` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-09 | `crates/domain/src/sell_draft_quantity.rs` | 기본 수량: `sellable < held` → `<=` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-10 | `crates/domain/src/sell_draft_quantity.rs` | 기본 수량: `sellable < held` → `>` | `property_default_quantity_and_reduction_match_independent_reference` |
| M-11 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: 보유 0주 거부 제거 | `sell_draft_rejects_missing_zero_and_changed_holdings` |
| M-12 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: `holding.observed_at() < signal.observed_at()` 제거 | `sell_draft_rejects_stale_future_and_pre_signal_holdings` |
| M-13 | `crates/domain/src/sell_draft_quantity.rs` | `verified_holding`: `holding.quantity() != signal.held_quantity()` 제거 | `sell_draft_rejects_missing_zero_and_changed_holdings` |
| M-14 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 계좌 ID 일치 검사 제거 | `sell_draft_rejects_unknown_zero_and_mismatched_sellable` |
| M-15 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 종목 ID 일치 검사 제거 | `sell_draft_rejects_unknown_zero_and_mismatched_sellable` |
| M-16 | `crates/domain/src/sell_draft_quantity.rs` | `verified_sellable`: 매도가능수량 0 거부 제거 | `property_default_quantity_and_reduction_match_independent_reference` |
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
