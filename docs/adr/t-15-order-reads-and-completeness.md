# ADR 초안: T-15 주문 목록·상세 조회와 종료 주문 완전성 증거

- 상태: 초안 (메인 병합 시 ADR 번호 부여)
- 관련 작업: T-15 (`docs/tasks.md`), D-34 (ADR 0064), D-38, T-21 (ADR 0150), A-20, T-13(실측)

## 결정

`crates/adapters/broker-toss`에 토스증권 Open API의 주문 읽기(GET) 3종(`open_orders`, `closed_orders`, `order`)을 구현하고, `crates/domain`에 종료 주문 조회의 완전성 증거(`ClosedOrderCompletenessProof`·`ProvenClosedOrderSet`·`ClosedOrderPageCollector`)와 미체결 조회 수집기(`OpenOrderCollector`)를 정의한다. 주문 생성·정정·취소 등 쓰기 메서드는 두지 않으며, `broker-toss`는 GET과 토큰 발급만 유지한다(H-22 준수).

### 엔드포인트 → 도메인 타입

| 엔드포인트 | 계좌 헤더 | 제한기 그룹 | 반환 도메인 타입 | 핵심 규칙 |
|---|---|---|---|---|
| `GET /api/v1/orders?status=OPEN` | `X-Tossinvest-Account` | `OrderHistory` (5/s) | `BrokerOrderSet` | `symbol`·`from`·`to` 무필터 조회. 각 행에 요청 계좌·마스터 종목·호출자 시계 `observed_at` 결합. `hasNext == true`, 종료 상태 혼입, 미래 시각, 식별 불가·거부 행이 하나라도 있으면 `OpenOrderCoverage::Incomplete` |
| `GET /api/v1/orders?status=CLOSED&limit=100[&cursor=...]` | `X-Tossinvest-Account` | `OrderHistory` (5/s) | `ClosedOrderSet` (+ `ClosedOrderCompletenessProof`) | 무필터 페이지 끝까지 수집(최대 8192페이지·4096행, 거부·중복 행 포함 전체 읽은 행 기준). `invalid-cursor` 400 시 1회 처음부터 재시작, `orders: [] && hasNext: true` 빈 페이지 연속은 즉시 차단. `ClosedOrderPageCollector`가 완전성 증거를 봉인해 D-38에 전달 |
| `GET /api/v1/orders/{orderId}` | `X-Tossinvest-Account` | `OrderHistory` (5/s) | `BrokerOrder` | 요청 `orderId`와 응답 `orderId` 대조(불일치 시 `InvalidResponse(Validation)`). 미래 `orderedAt`·`filledAt`·`canceledAt` 거부. `order-not-found`는 `ReadEndpoint::Order`에서만 `NotFound(MissingData::Order)`로 매핑 |

### 완전성 증거, D-38 소비, 봉인 경계와 한계 (`crates/domain/src/order_query_proof.rs`, `net_purchase.rs`)

1. **타입·상태 머신 봉인**:
   - `ClosedOrderCompletenessProof`, `ProvenClosedOrderSet`, `ClosedOrderQueryScope`, `OpenOrderQueryScope`의 모든 필드는 비공개이며 구조체 리터럴 생성(`E0451`)과 필드 변조(`E0616`)가 불가하다. `ClosedOrderSet::from_verified_proof`는 `pub(crate)`로 봉인한다(`E0624`).
   - `ClosedOrderPageCollector`는 다음 조건을 **모두** 만족할 때만 `ClosedOrderCompleteness::Complete`를 만든다: (a) 무필터(`OrderDateFilter::Unfiltered` + `OrderSymbolFilter::AllSymbols`), (b) `OrderPageBoundary::LastPage` 도달(`OrderPageExhaustion::Exhausted`) 및 `LastPage` 이후 추가 행/페이지 없음, (c) 페이지/행 상한·커서 반복·중간 실패 없음, (d) 거부·식별 불가·`PARTIAL_FILLED`·중복 ID·타 계좌·미래 시각(`orderedAt`/`filledAt`/`canceledAt > observed_at`)·0체결 모순 행 없음, (e) `observed_at >= evaluation_window.end()`.
   - 하나라도 어기면 `ClosedOrderCompleteness::Incomplete(MissingPages | RequestFailed | RejectedRows)`만 생성한다.
   - `OpenOrderCollector`는 필터 조회(`symbol`/`from`/`to`), 페이지 잔여(`hasNext == true`), 종료 상태 혼입, 완료 후 행 추가, 미래 시각·식별 불가·거부 행이 있으면 `OpenOrderCoverage::Incomplete`만 생성한다.
2. **D-38 소비 계약**:
   - `crates/domain`에 `proven_net_purchases(&ProvenClosedOrderSet, period)`와 `proven_net_purchase_amount(&ProvenClosedOrderSet, period, scope)`를 추가한다. 증명 없는 `ClosedOrderSet`을 넘기면 컴파일 실패(`E0308`)한다.
   - 기존 `net_purchases`/`net_purchase_amount`의 `validate_query`도 `orders.completeness_proof()`가 첨부돼 있으면(`broker-toss::closed_orders`는 항상 첨부) 기존 `covered` 검사를 중복 수행하지 않고 `proof.verify_for_period(period)` 단일 경로로 완전성과 평가 구간 포함 여부를 판정한다.
   - 기존 `ClosedOrderSet::new`는 기존 D-38 테스트 4개 파일(`net_purchase.rs`, `net_purchase_closed.rs`, `net_purchase_daily.rs`, `application/tests/ports.rs`)과 `BrokerRead::closed_orders` 반환형 호환(`test-diff` 보호) 때문에 당분간 유지한다(제거 이행은 인계에 기록).
3. **어댑터 신뢰 경계와 크레이트 간 호출 제한 한계(ADR 0064·H-64 선례)**:
   - `docs/architecture.md`에 따라 `crates/domain`은 작업 공간 의존성이 없고 `broker-toss → domain` 단방향이다. 따라서 Rust 가시성만으로는 `broker-toss`에만 열린 수집기 진입점을 `domain`에 둘 수 없다(기존 `CompleteTransactionResponse::after_last_page_without_quarantine`, `BrokerOrderSet::from_open_lookup_result`, `SellableQuantityFact::from_lookup_result`와 동일).
   - `ClosedOrderPageCollector::start`와 `OpenOrderCollector::start`는 어댑터 신뢰 경계로 두며, `ai-*`·`mcp-*`·`application` 등 타 크레이트의 호출을 금지하는 `crates/xtask` `arch-check` 규칙 추가는 보호 경로 변경이 필요하므로 ADR 0064 68행·H-64 선례에 따라 하네스 후속 작업으로 인계한다.
   - 컴파일 경계 테스트(`crates/domain/tests/t15_completeness_boundary.rs`)는 (a) 직접 구조체 생성·필드 변조·`from_verified_proof` 호출 차단(`E0451`/`E0616`/`E0624`)과 (b) `proven_net_purchase_amount`에 미증명 `ClosedOrderSet` 전달 차단(`E0308`)을 검증한다.

### 행 파싱과 금액·수량·마스터 결합 (`crates/adapters/broker-toss/src/order_read/row.rs`)

- 금액·가격·수량은 `NumericInput`·`Money::parse`·`Price::parse`·`Quantity::parse`로 정확히 변환한다(부동소수점·문자열 절삭 금지).
- 미지 통화·미지 주문 방향·음수 체결금액·음수 `commission`/`tax`·`canceledAt < orderedAt`·`filledAt < orderedAt`·체결 수량 0인데 `filledAt`/`averageFilledPrice`/0이 아닌 `filledAmount`가 존재하는 모순·호출자 시계 `observed_at`보다 미래인 `orderedAt`/`filledAt`/`canceledAt`은 모두 거부한다.
- `execution.commission`, `execution.tax`, `execution.settlementDate`는 스펙대로 형식·부호를 검증한다(도메인 `BrokerOrder` 노출 여부는 정의 대기).
- 각 주문 행은 `MasterInstrumentFacts`에서 `order_gate_facts().instrument_for(side)`로 종목을 해결하고 요청 계좌와 호출자 시계 `observed_at`을 결합한다.
  - 단건 `order()`에서 마스터 미등록은 `TossInstrumentRead`에서 `MasterNotReady(MissingInstrument)`, 기본 `TossAccountRead`에서는 기존 `holding_read` 호환 계약대로 `InvalidResponse(Validation)`을 반환한다.
  - 목록 `open_orders()`·`closed_orders()`에서 식별 불가 행은 목록 전체를 날리거나 조용히 빼지 않고 거부 행으로 기록해 결과 집합을 `Incomplete`로 표시한다.
- 첫 페이지 이전의 스키마·빈 페이지+`hasNext`·인증·네트워크 실패는 외부 `Err(PortError)`로 반환하고, 중간 페이지(1페이지 이상 유효 행 수집 후)의 실패·빈 페이지+`hasNext`는 이미 수집한 행과 함께 `Incomplete(RequestFailed)`로 돌려준다.
- `evaluation_window.start() >= observed_at`인 미래·동시각 평가 구간 요청은 `TimeInterval`이 성립할 수 없으므로 네트워크 호출 없이 `InvalidRequest(UnsupportedQuery)`로 즉시 거부한다.

### 상황 → 결과

| 상황 | 결과 |
|---|---|
| `open_orders` 무필터·미체결만·마스터 전부 식별·`hasNext == false`·비미래 시각 | `Ok(BrokerOrderSet)` (`OpenOrderCoverage::Complete`) |
| `open_orders` 식별 불가 행(폐지/미등록)·종료 상태 혼입·미래 시각·`hasNext == true` | `Ok(BrokerOrderSet)` (`OpenOrderCoverage::Incomplete`, `opposite_open_order`가 안전 차단) |
| `open_orders` 빈 페이지 + `hasNext == true` 또는 스키마 손상 | `Err(InvalidResponse(Validation \| Schema))` |
| `closed_orders` 무필터·페이지 끝 도달·전 행 식별·`observed_at >= window.end()` | `Ok(ClosedOrderSet)` (`Complete` + `ClosedOrderCompletenessProof`, D-38 검증 통과) |
| `closed_orders` `invalid-cursor` 400 후 1회 처음부터 재시작 성공 | `Ok(ClosedOrderSet)` (`Complete`) |
| `closed_orders` 커서 반복, 4096행/8192페이지 상한 도달, `observed_at < window.end()` | `Ok(ClosedOrderSet)` (`Incomplete(MissingPages)`) |
| `closed_orders` 2페이지 이후 HTTP 500/네트워크 실패 또는 빈 페이지+`hasNext: true` | `Ok(ClosedOrderSet)` (`Incomplete(RequestFailed)`, 수집된 유효 행 보존) |
| `closed_orders` 1페이지 HTTP 500/401/429 실패 또는 빈 페이지+`hasNext: true` | `Err(PortError)` (빈 성공 집합 대체 금지) |
| `closed_orders` `PARTIAL_FILLED` 혼입, 식별 불가 행, 0체결 모순, 음수 수수료/세금, 미래 시각 | `Ok(ClosedOrderSet)` (`Incomplete(RejectedRows)`) |
| `closed_orders` 평가 구간 이전 `orderedAt`·구간 내 `filledAt` 주문 | 무필터로 보존해 D-38이 `UnallocatableExecution`으로 안전 판정 |
| `closed_orders` 미래·동시각 평가 구간(`window.start() >= observed_at`) | 네트워크 호출 없이 `Err(InvalidRequest(UnsupportedQuery))` |
| `order` 정상 KR 수량 주문 / US 시장가 금액 주문(`orderAmount != null`) | `Ok(BrokerOrder)` (요청 계좌·`orderId` 대조, 금액 주문은 `BrokerOrderSize::Amount`로 `reported_quantity` 원문 보존) |
| `order` 응답 `orderId` 불일치, 0체결 `averageFilledPrice`, 음수 `commission`/`tax`, 미래 시각 | `Err(InvalidResponse(Validation))` |
| `order` `order-not-found` (HTTP 404) | `Err(NotFound(MissingData::Order))` (`ReadEndpoint::Order` 한정; 타 엔드포인트는 기존 `Validation` 유지) |
| `order` 종목 마스터 미등록 | `TossInstrumentRead`는 `Err(MasterNotReady(MissingInstrument))`, 기본 `TossAccountRead`는 기존 `Legacy` 계약 `Err(InvalidResponse(Validation))` |

## 정의 대기

| # | 항목 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1 | `closed_orders`의 `Complete` 선언에 실측 기반 보관기간/미지원 호가 인증 게이트 추가 여부 | **A(현행)**: 무필터 페이지 끝 도달·무실패·전 행 식별로 `Complete`를 부여하고 T-13 실측 결과에 따라 보강한다.<br>**B**: T-21 `InstrumentMasterAuthority`처럼 외부 완전성 인증 세션 도입 전까지 기본 `Incomplete`로 고정한다. | A는 합성 테스트와 D-38 연동이 바로 동작하지만, 실제 서버에 미문서 보관기간 제한이 있으면 T-13 실측 뒤 제한이 필요하다. B는 실측 전까지 자동 순매수 한도 계산이 항상 불완전으로 막힌다. |
| 2 | `CLOSED` 조회에 포함된 `PARTIAL_FILLED` 행의 도메인 처리 | **A(현행)**: ADR 0075·0082대로 거부 행으로 취급해 `Incomplete(RejectedRows)`로 닫힌 실패한다.<br>**B**: 부분 체결 후 종료(취소/만료) 주문을 별도 종료 상태로 도메인에 확장한다. | A는 부분 체결 종료 주문이 있으면 해당 평가 구간의 순매수 자동 계산이 보수적으로 보류된다. B는 실측으로 토스 상태 의미가 확인된 뒤 정확히 계산할 수 있다. |
| 3 | `execution.commission`·`tax`·`settlementDate`의 도메인 보존 여부 | **A(현행)**: 어댑터에서 형식·비음수 검증만 수행하고 ADR 0064 범위대로 `BrokerOrder`에 노출하지 않는다.<br>**B**: `BrokerOrder`에 수수료·세금·결제일 필드를 추가한다. | A는 도메인 표면을 단순하게 유지한다. B는 향후 O-11/O-26 감사·정산 표시에서 실제 체결 수수료가 필요할 때 확장한다. |
| 4 | 과거 상장폐지·정지 종목의 주문 읽기 식별 타입 분리 | **A(현행)**: `MasterInstrumentFacts::order_gate_facts().instrument_for(side)`를 사용해 비거래 종목 주문을 식별 불가(`Incomplete(RejectedRows)`)로 처리한다.<br>**B**: ADR 0064·0150 인계에 따라 읽기 전용 종목 식별 타입을 분리한다. | A는 주문 게이트 완화 없이 보수적으로 동작하지만 과거 폐지 종목 체결 이력이 계좌에 남아 있으면 종료 주문 집합이 `Incomplete`가 된다. B는 과거 체결 이력과 신규 주문 허용 게이트를 분리할 수 있다. |

## 실측 대기

실제 키가 필요한 사용자 실측(T-13)에서 아래 질문을 확인한다.

- 미국 시장가 금액 매수(`orderAmount != null`)의 실제 응답에서 `quantity`와 `execution.filledQuantity`는 어떤 값(`"0"`, 실제 체결 수량, 또는 `null`)으로 내려오는가?
- 무필터 `GET /api/v1/orders?status=CLOSED`가 계좌 개설 이후 전체 종료 주문을 페이지 끝까지 반환하는가, 아니면 문서에 없는 기간 제한(예: 최근 90일/1년)이 존재하는가?
- `CLOSED` 페이지네이션에서 `orders: []`이면서 `hasNext: true`인 정상 페이지가 실제로 내려오는 경우가 있는가? (현재 어댑터는 무한 커서 순회를 막기 위해 즉시 오류/불완전 처리)
- 체결·취소 직후 주문이 `OPEN`에서 `CLOSED`로 이동하는 지연 시간은 어느 정도이며, 부분 체결 뒤 취소·만료된 주문의 `CLOSED` 응답 `status`가 `PARTIAL_FILLED`인가 `CANCELED`인가?
- `execution.filledAmount`가 수수료·세금을 제외한 순체결금액인가, 포함한 총액인가? (D-38 순매수 금액 계산 기준)
- 토스 Open API가 지원하지 않는 호가·주문 유형이 `OPEN`/`CLOSED`/`GET /orders/{orderId}` 응답에서 누락되는 범위는 어디까지인가?
- 실제 `orderId`의 문자 집합과 최대 길이는 얼마인가? (경로 세그먼트 상한 `RouteId` 128바이트 vs `BrokerOrderId` 256바이트)

## 인계

- **하네스 후속(H-64 선례, `crates/xtask` `arch-check`)**: `ClosedOrderPageCollector::start`와 `OpenOrderCollector::start` 호출 위치를 `crates/adapters/broker-toss`와 보호된 테스트 픽스처(`testkit-ports`)로 제한해 `ai-*`·`mcp-*`·`application` 등 타 크레이트의 직접 호출을 컴파일/정적 검사에서 차단한다.
- **D-38·포트 이행 후속(기존 테스트 변경 승인 필요)**: 메인 승인으로 기존 4개 테스트 파일(`net_purchase.rs`, `net_purchase_closed.rs`, `net_purchase_daily.rs`, `application/tests/ports.rs`)과 `BrokerRead::closed_orders` 반환형을 `ProvenClosedOrderSet`으로 이행한 뒤 `ClosedOrderSet::new` 공개 생성자를 제거해 미증명 `ClosedOrderSet`의 D-38 유입 경로를 완전히 닫는다.
- **메인 병합 시 ADR 번호 부여**: 이 초안 파일(`docs/adr/t-15-order-reads-and-completeness.md`)에 정식 ADR 번호를 부여할 때 `node harness/codex-hooks/config.mjs --write`와 `node harness/claude-hooks/hashes.mjs --write`로 보호 설정을 동기화한다.
- **T-13 실측**: 위 "실측 대기" 7개 질문의 실측 결과를 바탕으로 금액 주문 수량 처리, 보관기간/미지원 호가 완전성 게이트, `PARTIAL_FILLED` 종료 주문 계약을 확정한다.
- **T-05·A-06 / 읽기 전용 식별**: 상장폐지·거래정지 종목의 과거 주문이 `closed_orders`를 영구 `Incomplete`로 만들지 않도록 읽기 전용 종목 식별 분리를 함께 검토한다.

## 변이 검증 기록

격리 복사본(`target/T-11/`, 실행 직후 삭제)에서 T-15 신규 구현 및 도메인 완전성 증거 분기 15건을 컴파일 가능한 변형으로 실행해 15/15 검출을 확인했다(생존 0, 컴파일 불가 0).

| 번호 | 파일 | 실제 변형 | 검출한 테스트 |
|---|---|---|---|
| T15-01 | `crates/adapters/broker-toss/src/error_mapping.rs` | `order-not-found`의 `ReadEndpoint::Order` `NotFound` 매핑 제거 | `order_detail_preserves_unknown_metadata_and_maps_order_not_found` |
| T15-02 | `crates/adapters/broker-toss/src/error_mapping.rs` | `order-not-found`를 타 엔드포인트에도 `NotFound`로 확장 | `order_detail_preserves_unknown_metadata_and_maps_order_not_found` |
| T15-03 | `crates/adapters/broker-toss/src/order_read.rs` | `order` 상세 응답 `orderId` 불일치 검사 제거 | `order_detail_rejects_id_mismatch_unsafe_ids_missing_master_and_bad_fields` |
| T15-04 | `crates/adapters/broker-toss/src/order_read.rs` | `open_orders` `hasNext`/`nextCursor` 불완전 판정 제거 | `open_orders_marks_incomplete_on_unidentified_closed_pagination_and_errors` |
| T15-05 | `crates/adapters/broker-toss/src/order_read.rs` | `open_orders` 빈 페이지+`hasNext` 거부 제거 | `open_orders_marks_incomplete_on_unidentified_closed_pagination_and_errors` |
| T15-06 | `crates/adapters/broker-toss/src/order_read.rs` | `closed_orders` 미래 평가 구간 선차단 제거 | `closed_orders_preserves_old_order_for_d38_and_handles_early_or_future_windows` |
| T15-07 | `crates/adapters/broker-toss/src/order_read.rs` | `closed_orders` `invalid-cursor` 1회 재시작 제거 | `closed_orders_handles_cursor_restart_and_repeated_cursor` |
| T15-08 | `crates/adapters/broker-toss/src/order_read.rs` | `closed_orders` 커서 반복 감지 제거 | `closed_orders_handles_cursor_restart_and_repeated_cursor` |
| T15-09 | `crates/adapters/broker-toss/src/order_read.rs` | `closed_orders` 4096행 상한 완화(`4097`) | `closed_orders_marks_row_limit_incomplete_and_shares_order_history_rate_limit` |
| T15-10 | `crates/adapters/broker-toss/src/order_read.rs` | `closed_orders` 중간 실패를 `LastPage`(`Complete`)로 위조 | `closed_orders_handles_mid_failure_page1_failure_and_partial_filled` |
| T15-11 | `crates/adapters/broker-toss/src/order_read/row.rs` | 주문 행 `canceledAt < orderedAt` 검증 제거 | `order_detail_rejects_id_mismatch_unsafe_ids_missing_master_and_bad_fields` |
| T15-12 | `crates/adapters/broker-toss/src/order_read/row.rs` | 주문 행 `filledAt < orderedAt` 검증 제거 | `order_detail_rejects_id_mismatch_unsafe_ids_missing_master_and_bad_fields` |
| T15-13 | `crates/domain/src/order_query_proof.rs` | `ClosedOrderPageCollector` 필터 조회 불완전 강제 제거 | `closed_collector_forbids_complete_on_filters` |
| T15-14 | `crates/domain/src/order_query_proof.rs` | `ClosedOrderPageCollector` 페이지 끝 도달 요구 제거 | `closed_collector_forbids_complete_on_limits_and_request_failure` |
| T15-15 | `crates/domain/src/order_query_proof.rs` | `OpenOrderCollector` 필터 조회 `Complete` 금지 제거 | `open_collector_enforces_unfiltered_scope_and_closed_mix` |

### 자동 변경분 변이 (`just mutants-daily 6b8cabf3b4cab2521f5489725ffc0c63de2c6e83 0/1`, 대상 `389503b8`)

- 1차 실행(`0c2e174a`): `120 mutants tested in 21m: 5 missed, 75 caught, 40 unviable` (`order_query_proof.rs` 미검출 5건)
- 미검출 5건 단독 결함 격리 테스트 보강(`389503b8`) 후 최종 실행: `120 mutants tested in 19m: 80 caught, 40 unviable` / `mutants: 위반 없음` (미검출 0, 타임아웃 0)
