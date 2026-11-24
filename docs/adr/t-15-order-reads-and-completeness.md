# ADR 초안: T-15 주문 목록·상세 조회와 종료 주문 완전성 증거

- 상태: 초안 (메인 병합 시 ADR 번호 부여)
- 관련 작업: T-15 (`docs/tasks.md`), D-34 (ADR 0064), D-38, T-21 (ADR 0150), A-20, T-13(실측)

## 결정

`crates/adapters/broker-toss`에 토스증권 Open API의 주문 읽기(GET) 3종(`open_orders`, `closed_orders`, `order`)을 구현하고, `crates/domain`에 종료 주문 조회의 완전성 증거(`ClosedOrderCompletenessProof`·`ClosedOrderPageCollector`)와 미체결 조회 수집기(`OpenOrderCollector`)를 봉인된 타입으로 정의한다. 주문 생성·정정·취소 등 쓰기 메서드는 두지 않으며, `broker-toss`는 GET과 토큰 발급만 유지한다(H-22 준수).

### 엔드포인트 → 도메인 타입

| 엔드포인트 | 계좌 헤더 | 제한기 | 반환 도메인 타입 | 핵심 규칙 |
|---|---|---|---|---|
| `GET /api/v1/orders?status=OPEN` | `X-Tossinvest-Account` | `ORDER_HISTORY` | `BrokerOrderSet` | `symbol`·`from`·`to` 무필터 조회. 각 행에 요청 계좌·마스터 종목·호출자 시계 `observed_at` 결합. `hasNext == true`, 종료 상태 혼입, 식별 불가·거부 행이 하나라도 있으면 `OpenOrderCoverage::Incomplete` |
| `GET /api/v1/orders?status=CLOSED&limit=100[&cursor=...]` | `X-Tossinvest-Account` | `ORDER_HISTORY` | `ClosedOrderSet` (+ `ClosedOrderCompletenessProof`) | 무필터 페이지 끝까지 수집(최대 8192페이지·4096행). `invalid-cursor` 400 시 1회 처음부터 재시작. `ClosedOrderPageCollector`가 완전성 증거를 봉인해 D-38(`net_purchases`·`net_purchase_amount`)에 전달 |
| `GET /api/v1/orders/{orderId}` | `X-Tossinvest-Account` | `ORDER_HISTORY` | `BrokerOrder` | 요청 `orderId`와 응답 `orderId` 대조(불일치 시 `InvalidResponse(Validation)`). `order-not-found`는 `ReadEndpoint::Order`에서만 `MissingData::Order`로 매핑 |

### 완전성 증거와 봉인 경계 (`crates/domain/src/order_query_proof.rs`)

종료 주문과 미체결 주문 조회에서 불완전한 결과가 `Complete`로 위조되거나 외부에서 임의로 생성되지 않도록 `crates/domain`에 봉인된 수집기와 증거 타입을 둔다.

1. **`ClosedOrderCompletenessProof` / `ProvenClosedOrderSet` / `ClosedOrderQueryScope`**:
   - 모든 필드가 비공개이며 외부 공개 생성자가 없다. `ClosedOrderSet::from_verified_proof`는 `pub(crate)`로 봉인하고, 외부 소비자는 `ClosedOrderSet::completeness_proof() -> Option<&ClosedOrderCompletenessProof>`로 읽기 전용 접근만 할 수 있다.
   - `ClosedOrderPageCollector`는 다음 조건을 **모두** 만족할 때만 `ClosedOrderCompleteness::Complete`를 만든다.
     - 날짜 필터 없음(`OrderDateFilter::Unfiltered`) 및 종목 필터 없음(`OrderSymbolFilter::AllSymbols`)
     - 서버가 `hasNext == false`로 마지막 페이지임을 보고(`OrderPageBoundary::LastPage` → `OrderPageExhaustion::Exhausted`)
     - 페이지 상한(8192)·행 상한(4096)·커서 반복·중간 페이지 실패가 없음
     - 거부된 행·마스터 식별 불가 행·`CLOSED`에 섞인 `PARTIAL_FILLED`·중복 주문 ID·다른 계좌 행이 없음
     - 호출자 관측 시각이 평가 구간 종료 시각 이상(`observed_at >= evaluation_window.end()`)
   - 하나라도 어긋나면 `ClosedOrderCompleteness::Incomplete(MissingPages | RequestFailed | RejectedRows)`로만 생성되며, D-38 `net_purchase_amount`가 즉시 `NetPurchaseError::Incomplete`로 차단한다.
2. **`OpenOrderCollector` / `OpenOrderQueryScope`**:
   - 날짜·종목 필터가 걸린 `OPEN` 조회는 페이지 끝과 무관하게 `OpenOrderCoverage::Complete`를 선언할 수 없다(`Incomplete` 강제).
   - 페이지 잔여(`hasNext == true`), 종료 상태 혼입, 식별 불가·거부 행이 있으면 `OpenOrderCoverage::Incomplete`를 만든다.
3. **컴파일 실패 경계 증명 (`crates/domain/tests/t15_completeness_boundary.rs`)**:
   - 외부 크레이트에서 `ClosedOrderCompletenessProof`, `ProvenClosedOrderSet`, `ClosedOrderQueryScope`, `OpenOrderQueryScope`의 구조체 리터럴 생성(`E0451`), `ClosedOrderSet::from_verified_proof` 직접 호출(`E0624`), 비공개 필드 변조(`E0616`)가 모두 컴파일 실패함을 검증한다.

### 행 파싱과 금액·수량·마스터 결합 (`crates/adapters/broker-toss/src/order_read/row.rs`)

- 금액·가격·수량은 `NumericInput`·`Money::parse`·`Price::parse`·`Quantity::parse`로 정확히 변환한다(부동소수점·문자열 절삭 금지).
- 미지 통화·미지 주문 방향·음수 체결금액·`canceledAt < orderedAt`·`filledAt < orderedAt`·체결 수량 0인데 평균가/체결금액 존재 등 스키마 모순은 거부한다.
- `execution.commission`, `execution.tax`, `execution.settlementDate`는 스펙대로 형식을 검증한다(도메인 `BrokerOrder` 노출 여부는 정의 대기).
- 각 주문 행은 `MasterInstrumentFacts`에서 `order_gate_facts().instrument_for(side)`로 종목을 해결하고 요청 계좌와 호출자 시계 `observed_at`을 결합한다.
  - 단건 `order()`에서 마스터 미등록은 `TossInstrumentRead`에서 `MasterNotReady(MissingInstrument)`로 반환한다.
  - 목록 `open_orders()`·`closed_orders()`에서 마스터 미등록·거래정지·폐지 등으로 식별하지 못한 행은 목록 전체를 날리거나 조용히 빼지 않고 거부 행으로 기록해 결과 집합을 `Incomplete`로 표시한다.
- 첫 페이지 이전의 스키마 오류·인증·네트워크 실패는 외부 `Err(PortError)`로 반환하고, 중간 페이지(1페이지 이상 수집 후)의 전송/서버 실패는 이미 수집한 행과 함께 `Incomplete(RequestFailed)`로 돌려준다(페이지 1 실패 시 빈 성공 집합으로 대체하지 않음).
- `evaluation_window.start() > observed_at`인 미래 평가 구간 요청은 네트워크 호출 없이 `InvalidRequest(UnsupportedQuery)`로 거부한다.

### 상황 → 결과

| 상황 | 결과 |
|---|---|
| `open_orders` 무필터·미체결만·마스터 전부 식별·`hasNext == false` | `Ok(BrokerOrderSet)` (`OpenOrderCoverage::Complete`) |
| `open_orders` 식별 불가 행(폐지/미등록)·종료 상태 혼입·`hasNext == true` | `Ok(BrokerOrderSet)` (`OpenOrderCoverage::Incomplete`, `opposite_open_order`가 안전 차단) |
| `open_orders` 빈 페이지 + `hasNext == true` 또는 스키마 손상 | `Err(InvalidResponse(Validation \| Schema))` |
| `closed_orders` 무필터·페이지 끝 도달·전 행 식별·`observed_at >= window.end()` | `Ok(ClosedOrderSet)` (`Complete` + `ClosedOrderCompletenessProof`, D-38 순매수 계산 통과) |
| `closed_orders` `invalid-cursor` 400 후 1회 처음부터 재시작 성공 | `Ok(ClosedOrderSet)` (`Complete`) |
| `closed_orders` 커서 반복, 4096행/8192페이지 상한 도달, `observed_at < window.end()` | `Ok(ClosedOrderSet)` (`Incomplete(MissingPages)`) |
| `closed_orders` 2페이지 이후 HTTP 500/네트워크 실패 | `Ok(ClosedOrderSet)` (`Incomplete(RequestFailed)`, 수집된 유효 행 보존) |
| `closed_orders` 1페이지 HTTP 500/401/429 실패 | `Err(PortError)` (빈 성공 집합 대체 금지) |
| `closed_orders` `PARTIAL_FILLED` 혼입 또는 식별 불가 행 포함 | `Ok(ClosedOrderSet)` (`Incomplete(RejectedRows)`) |
| `closed_orders` 평가 구간 이전 `orderedAt`·구간 내 `filledAt` 주문 | 무필터로 보존해 D-38이 `UnallocatableExecution`으로 안전 판정 |
| `closed_orders` 미래 평가 구간(`window.start() > observed_at`) | `Err(InvalidRequest(UnsupportedQuery))` |
| `order` 정상 KR 수량 주문 / US 시장가 금액 주문(`orderAmount != null`) | `Ok(BrokerOrder)` (요청 계좌·`orderId` 대조, 금액 주문은 `BrokerOrderSize::Amount`로 `reported_quantity` 원문 보존) |
| `order` 응답 `orderId` 불일치 | `Err(InvalidResponse(Validation))` |
| `order` `order-not-found` (HTTP 404) | `Err(NotFound(MissingData::Order))` (`ReadEndpoint::Order` 한정; 타 엔드포인트는 기존 `Validation` 유지) |
| `order` 종목 마스터 미등록 | `TossInstrumentRead`는 `Err(MasterNotReady(MissingInstrument))`, 기본 `TossAccountRead`는 기존 호환 계약 `Err(Unsupported(Instruments))` |

## 정의 대기

| # | 항목 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1 | `closed_orders`의 `Complete` 선언에 실측 기반 보관기간/미지원 호가 인증 게이트 추가 여부 | **A(현행)**: 무필터 페이지 끝 도달·무실패·전 행 식별로 `Complete`를 부여하고 T-13 실측 결과에 따라 보강한다.<br>**B**: T-21 `InstrumentMasterAuthority`처럼 외부 완전성 인증 세션 도입 전까지 기본 `Incomplete`로 고정한다. | A는 합성 테스트와 D-38 연동이 바로 동작하지만, 실제 서버에 미문서 보관기간 제한이 있으면 T-13 실측 뒤 제한이 필요하다. B는 실측 전까지 자동 순매수 한도 계산이 항상 불완전으로 막힌다. |
| 2 | `CLOSED` 조회에 포함된 `PARTIAL_FILLED` 행의 도메인 처리 | **A(현행)**: ADR 0075·0082대로 거부 행으로 취급해 `Incomplete(RejectedRows)`로 닫힌 실패한다.<br>**B**: 부분 체결 후 종료(취소/만료) 주문을 별도 종료 상태로 도메인에 확장한다. | A는 부분 체결 종료 주문이 있으면 해당 평가 구간의 순매수 자동 계산이 보수적으로 보류된다. B는 실측으로 토스 상태 의미가 확인된 뒤 정확히 계산할 수 있다. |
| 3 | `execution.commission`·`tax`·`settlementDate`의 도메인 보존 여부 | **A(현행)**: 어댑터에서 형식 검증만 수행하고 ADR 0064 범위대로 `BrokerOrder`에 노출하지 않는다.<br>**B**: `BrokerOrder`에 수수료·세금·결제일 필드를 추가한다. | A는 도메인 표면을 단순하게 유지한다. B는 향후 O-11/O-26 감사·정산 표시에서 실제 체결 수수료가 필요할 때 확장한다. |
| 4 | 과거 상장폐지·정지 종목의 주문 읽기 식별 타입 분리 | **A(현행)**: `MasterInstrumentFacts::order_gate_facts().instrument_for(side)`를 사용해 비거래 종목 주문을 식별 불가(`Incomplete(RejectedRows)`)로 처리한다.<br>**B**: ADR 0064·0150 인계에 따라 읽기 전용 종목 식별 타입을 분리한다. | A는 주문 게이트 완화 없이 보수적으로 동작하지만 과거 폐지 종목 체결 이력이 계좌에 남아 있으면 종료 주문 집합이 `Incomplete`가 된다. B는 과거 체결 이력과 신규 주문 허용 게이트를 분리할 수 있다. |

## 실측 대기

실제 키가 필요한 사용자 실측(T-13)에서 아래 질문을 확인한다.

- 미국 시장가 금액 매수(`orderAmount != null`)의 실제 응답에서 `quantity`와 `execution.filledQuantity`는 어떤 값(`"0"`, 실제 체결 수량, 또는 `null`)으로 내려오는가?
- 무필터 `GET /api/v1/orders?status=CLOSED`가 계좌 개설 이후 전체 종료 주문을 페이지 끝까지 반환하는가, 아니면 문서에 없는 기간 제한(예: 최근 90일/1년)이 존재하는가?
- 체결·취소 직후 주문이 `OPEN`에서 `CLOSED`로 이동하는 지연 시간은 어느 정도이며, 부분 체결 뒤 취소·만료된 주문의 `CLOSED` 응답 `status`가 `PARTIAL_FILLED`인가 `CANCELED`인가?
- `execution.filledAmount`가 수수료·세금을 제외한 순체결금액인가, 포함한 총액인가? (D-38 순매수 금액 계산 기준)
- 토스 Open API가 지원하지 않는 호가·주문 유형이 `OPEN`/`CLOSED`/`GET /orders/{orderId}` 응답에서 누락되는 범위는 어디까지인가?
- 실제 `orderId`의 문자 집합과 최대 길이는 얼마인가? (경로 세그먼트 상한 `RouteId` 128바이트 vs `BrokerOrderId` 256바이트)

## 인계

- **메인 병합 시 ADR 번호 부여**: 이 초안 파일(`docs/adr/t-15-order-reads-and-completeness.md`)에 정식 ADR 번호를 부여할 때 `node harness/codex-hooks/config.mjs --write`와 `node harness/claude-hooks/hashes.mjs --write`로 보호 설정을 동기화한다.
- **T-13 실측**: 위 "실측 대기" 6개 질문의 실측 결과를 바탕으로 금액 주문 수량 처리, 보관기간/미지원 호가 완전성 게이트, `PARTIAL_FILLED` 종료 주문 계약을 확정한다.
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

(리뷰 종료 후 `just mutants-daily` 결과 추가)
