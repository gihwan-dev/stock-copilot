# ADR 초안: T-11 주문 사전 조회(매수가능금액·매도가능수량·수수료·매수 유의사항)

- 상태: 초안 (메인 병합 시 ADR 번호 부여)
- 관련 작업: T-11 (`docs/tasks.md`), D-34 (ADR 0064), D-57 (ADR 0160), T-21 (ADR 0150), A-20

## 결정

`crates/adapters/broker-toss`에 토스증권 Open API의 주문 사전 읽기(GET) 4종을 구현해 `BrokerRead` 포트에 연결한다. 쓰기 메서드는 두지 않으며, 금액·수량·비율은 부동소수점이나 문자열 절삭 없이 `NumericInput`·`Money::parse`·`Quantity::parse`·`Ratio::parse`·`CostRate::try_new`로 정확히 변환한다. 요청과 응답의 계좌·종목·통화·시장이 어긋나거나, 스펙에 없는 미지 열거형·누락 필수 값·비정상 날짜 범위가 내려오면 추정하지 않고 닫힌 실패로 반환한다.

### 엔드포인트 → 도메인 타입

| 엔드포인트 | 계좌 헤더 | 제한기 그룹 | 반환 도메인 타입 | 요청↔응답 대조 및 변환 규칙 |
|---|---|---|---|---|
| `GET /api/v1/buying-power?currency={KRW\|USD}` | `X-Tossinvest-Account` | `OrderInfo` (3/s) | `BuyingPower` | 응답 `currency`가 요청 통화와 다르면 `InvalidResponse(Validation)`. KRW는 정규화 후 정수 금액(`scale == 0`)만 허용(`.00`은 허용, `.5` 거부). 음수 매수가능금액(미수 등)은 `BuyingPower::new`로 보존. 호출자 시계 `observed_at` 결합 |
| `GET /api/v1/sellable-quantity?symbol={symbol}` | `X-Tossinvest-Account` | `OrderInfo` (3/s) | `SellableQuantityFact` | 호출자가 `MasterInstrumentFacts`에서 해결한 `ResolvedOrderInstrument`·요청 계좌·호출자 시계 `observed_at`을 응답 `sellableQuantity`와 결합(스펙 응답에는 `symbol`·시각 필드가 없음). 국내 소수 수량·음수 수량 거부. D-57 `calculate_sell_draft_quantity`에 직접 공급 가능 |
| `GET /api/v1/commissions` | `X-Tossinvest-Account` | `OrderInfo` (3/s) | `CommissionRateFact` | 배열의 모든 행을 끝까지 검증. 미지 `marketCountry`, 중복 시장, 음수·1 초과·소수 파싱 불가 `rate`, 비유효 `startDate`/`endDate`, `endDate < startDate`, 요청 시장 누락 시 모두 닫힌 실패. 계좌별 수수료 차이를 보존하기 위해 계좌 간 공유 캐시를 두지 않음 |
| `GET /api/v1/stocks/{symbol}/warnings` | 없음 | `Stock` (5/s) | `StockWarnings` | 경로 세그먼트에 요청 심볼을 인코딩해 조회(계좌 헤더 미전송). 최대 256건 상한. 각 항목의 `warningType`을 알려진 8개 값과 `Unknown(MasterCode)`로 변환하고 `exchange`·`startDate`·`endDate`(`endDate >= startDate`)를 어댑터 경계에서 검증. 동일 경고는 `StockWarnings::from_lookup_result`가 중복 없는 집합으로 보존. 호출자 시계 `observed_at` 결합 |

`buying_power`, `sellable_quantity`, `commission_rate`, `stock_warnings`는 모두 `TossAccountRead`와 `TossInstrumentRead` 양쪽에서 동일 경로로 동작한다(`sellable_quantity`·`stock_warnings`는 호출자가 이미 마스터에서 해결한 `&ResolvedOrderInstrument`를 인자로 받는다).

### 상황 → 결과

| 상황 | 결과 |
|---|---|
| `buying_power` KRW `"5000000"`, `"5000000.00"`, `"-12500"` / USD `"1234.56"`, `"-12.75"` | `Ok(BuyingPower)` (정확한 `Money` 보존, 음수 허용, 호출자 `observed_at` 결합) |
| `buying_power` 응답 통화 불일치(요청 KRW, 응답 USD), 미지 통화, KRW 비정수(`"1000.5"`), 과학표기·초과 자릿수 | `Err(InvalidResponse(Validation \| Schema))` |
| `sellable_quantity` 정상 KR 정수(`"11"`, `"11.0"`, `"0"`) / US 소수(`"10.5"`) | `Ok(SellableQuantityFact)` (계좌·`ResolvedOrderInstrument`·호출자 `observed_at` 결합, D-34 `check_sellable_quantity`·D-57 `calculate_sell_draft_quantity` 호환) |
| `sellable_quantity` KR 소수 수량(`"10.5"`), 음수(`"-1"`), 과학표기, 30자 초과·비문자열 | `Err(InvalidResponse(Validation \| Schema))` |
| `commission_rate` 정상 KR/US 수수료율 (`"0.00015"`, `"0.001"`) 및 유효기간(`startDate`/`endDate`) | `Ok(CommissionRateFact)` (정확한 `CostRate`, 계좌별 독립 조회) |
| `commission_rate` 미지 `marketCountry`, 중복 시장, 요청 시장 누락, `rate > 1`, 음수, `endDate < startDate` | `Err(InvalidResponse(Validation))` |
| `stock_warnings` 빈 배열 / 알려진 경고 8종 / 미지 경고 코드 / 256건 상한 이내 | `Ok(StockWarnings)` (`Unknown(MasterCode)` 보존, `check_stock_warnings` 매수 차단·매도 허용, `X-Tossinvest-Account` 미전송) |
| `stock_warnings` 비유효 `exchange`·날짜 형식·`endDate < startDate`·257건 초과 | `Err(InvalidResponse(Validation))` |
| `account-not-found` (`buying-power` HTTP 404; `sellable-quantity`·`commissions` HTTP 400 또는 404) | `Err(AccessDenied(AccessDenial::AccountNotFound))` |
| `stock-not-found` (HTTP 404) | `Err(NotFound(MissingData::Stock))` |
| `401 token-revoked` / `429 rate-limit-exceeded` / `500 internal-error` | 기존 `error_mapping`·토큰 재발급·레이트 리밋 재시도·대기 계약 그대로 적용 |

## 정의 대기

기존 도메인 타입에 없는 OpenAPI 필드는 임의로 도메인을 넓히지 않고 어댑터 경계에서 파싱·검증까지만 수행한다. 아래 항목은 상위 유스케이스 요구가 확정될 때 결정한다.

| # | 항목 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1 | `CommissionRateFact`에 계좌 ID·관측 시각 포함 여부 | **A(현행)**: 기존 `CommissionRateFact`를 유지하고 호출 유스케이스가 계좌·시각 컨텍스트를 결합한다.<br>**B**: 도메인에 `AccountCommissionRateFact` 래퍼를 추가해 계좌 ID와 `observed_at`을 봉인한다. | A는 기존 포트·도메인 변경 없이 단순하지만 유스케이스에서 계좌 간 혼용 방지를 별도 검증해야 한다. B는 계좌별 우대 수수료 혼용을 타입으로 차단한다. |
| 2 | `StockWarnings`의 항목별 `exchange`·`startDate`·`endDate` 도메인 노출 여부 | **A(현행)**: 어댑터에서 형식·순서를 검증하고 기존 `StockWarnings` 집합만 반환한다(ADR 0064 보수 차단 유지).<br>**B**: 항목별 거래소·기간을 담은 경고 상세 타입을 도메인에 추가한다. | A는 매수 차단 판정에 필요한 정보를 최소한으로 유지한다. B는 추후 화면에서 경고 시작일·종료일 안내가 필요할 때 활용할 수 있다. |
| 3 | `/api/v1/commissions` 응답에 미지원 신규 `marketCountry`(예: `JP`) 행이 추가될 때 처리 | **A(현행)**: 배열 전체를 엄격 검증해 미지 `marketCountry`가 섞이면 닫힌 실패한다.<br>**B**: 요청한 `KR`/`US` 행만 검증하고 미지원 타 시장 행은 무시한다. | A는 스펙 변경을 즉시 감지해 잘못된 수수료 적용을 막지만, 토스가 타 국가 시장을 추가하면 앱 갱신 전까지 수수료 조회가 실패할 수 있다. B는 타 국가 추가에 내성이 생기지만 응답 오류 감지가 약해진다. |
| 4 | 비거래·정지 종목의 `sellable_quantity` 조회 경계 | **A(현행)**: 호출자가 `ResolvedOrderInstrument`(`order_gate_facts().instrument_for(OrderSide::Sell)`)로 해결한 종목만 허용해 주문 불가 종목을 차단한다(D-57과 일치).<br>**B**: ADR 0150의 T-05·A-06 인계와 함께 읽기용 종목 식별 타입을 분리한다. | A는 정지·폐지 종목의 매도 초안 생성을 원천 차단한다. B는 정지 종목 보유 현황 화면에서 참고용 수량을 별도 표기할 때만 필요하다. |

## 실측 대기

실제 키가 필요한 사용자 실측(T-13)에서 아래 항목을 확인한다.

- `account-not-found` 오류의 실제 HTTP 상태 코드가 엔드포인트마다 404(`/api/v1/buying-power`)와 400(`/api/v1/sellable-quantity`, `/api/v1/commissions`)으로 다르게 내려오는지, 아니면 하나로 통일되는지? (현재 어댑터는 스펙에 맞춰 `sellable-quantity`·`commissions`에서 400·404 모두 `AccessDenied(AccountNotFound)`로 매핑하고 타 엔드포인트는 기존 404 계약을 유지)
- 실제 계좌에서 `cashBuyingPower`가 음수(미수금 발생 등)로 반환되는 경우가 있는지, KRW 금액에 소수점(`.00` 또는 원 미만)이 포함되는지?
- `/api/v1/commissions`가 실제 계좌에서 현재 유효 수수료 1건만 시장별로 내려주는지, 아니면 과거 만료 이력이나 복수 행을 같은 `marketCountry`로 함께 내려주는지?
- `/api/v1/stocks/{symbol}/warnings`가 같은 `warningType`을 거래소(`exchange`)나 적용 기간이 다른 복수 행으로 반환하는 경우가 있는지?

## 기존 테스트 변경

`crates/adapters/broker-toss/tests/fx_market.rs`의 `capabilities_match_implemented_market_endpoints`에서 `BrokerCapability::SellableQuantity`를 미지원 목록에서 지원 목록으로 이동했다(기계적 고정 목록 갱신, 단언 강도 유지).

- `test-diff` 정확 지적 문자열: `broker-toss::fx_market tests::capabilities_match_implemented_market_endpoints: test-diff:modified:broker-toss::fx_market tests::capabilities_match_implemented_market_endpoints`

| 파일 | 기준 SHA-256 | 변경 후 SHA-256 |
|---|---|---|
| `crates/adapters/broker-toss/tests/fx_market.rs` | `ba96bbf37fb0a0d47ba0945b53e897308a26d2fe08a5282f5e7653f1bca7508b` | `70b433b55c68838fc67409fbd37a8434c84a786c60ddea1cedb6b8c4d2868630` |

## 인계

- **메인 병합 시 ADR 번호 부여**: 이 초안 파일(`docs/adr/t-11-order-info-and-warnings.md`)에 정식 ADR 번호를 부여할 때 `node harness/codex-hooks/config.mjs --write`와 `node harness/claude-hooks/hashes.mjs --write`로 보호 설정을 동기화한다.
- **T-13 실측**: 위 "실측 대기" 4개 질문의 실측 결과에 따라 오류 상태 매핑, 수수료 복수 이력 필터링, 중복 경고 처리 규칙을 보완한다.

## 변이 검증 기록

격리 복사본(`target/T-11/`, 실행 직후 삭제)에서 T-11 신규 구현 분기 11건을 컴파일 가능한 변형으로 실행해 11/11 검출을 확인했다(생존 0, 컴파일 불가 0).

| 번호 | 파일 | 실제 변형 | 검출한 테스트 |
|---|---|---|---|
| T11-01 | `crates/adapters/broker-toss/src/order_info_read.rs` | `buying_power` 응답 통화 일치 조건 반전 | `buying_power_rejects_currency_mismatch_krw_fraction_bad_numbers_and_maps_errors` |
| T11-02 | `crates/adapters/broker-toss/src/order_info_read.rs` | `buying_power` KRW 소수부 거부 검사 제거 | `buying_power_rejects_currency_mismatch_krw_fraction_bad_numbers_and_maps_errors` |
| T11-03 | `crates/adapters/broker-toss/src/order_info_read.rs` | `buying_power` KRW 요청 쿼리 통화를 USD로 변조 | `buying_power_preserves_exact_krw_usd_negative_and_account_header` |
| T11-04 | `crates/adapters/broker-toss/src/order_info_read.rs` | `buying_power` 계좌 헤더 값을 타 계좌로 변조 | `buying_power_preserves_exact_krw_usd_negative_and_account_header` |
| T11-05 | `crates/adapters/broker-toss/src/order_info_read.rs` | `sellable_quantity` 계좌 헤더 전송 누락 | `sellable_quantity_binds_account_instrument_clock_and_feeds_d34` |
| T11-06 | `crates/adapters/broker-toss/src/order_info_read.rs` | `commission_rate` 중복 KR 시장 거부 비활성화 | `commission_rate_rejects_unknown_market_duplicates_invalid_rates_dates_and_missing_market` |
| T11-07 | `crates/adapters/broker-toss/src/order_info_read.rs` | `commission_rate` 중복 US 시장 거부 비활성화 | `commission_rate_rejects_unknown_market_duplicates_invalid_rates_dates_and_missing_market` |
| T11-08 | `crates/adapters/broker-toss/src/order_info_read.rs` | `commission_rate` 미지 `marketCountry`를 KR로 허용 | `commission_rate_rejects_unknown_market_duplicates_invalid_rates_dates_and_missing_market` |
| T11-09 | `crates/adapters/broker-toss/src/order_info_read.rs` | `stock_warnings` `startDate > endDate` 검증 반전 | `stock_warnings_preserves_known_and_unknown_without_account_header` |
| T11-10 | `crates/adapters/broker-toss/src/order_info_read.rs` | `stock_warnings` 256건 상한 완화(`> 257`) | `stock_warnings_enforces_256_item_limit_boundary` |
| T11-11 | `crates/adapters/broker-toss/src/order_info_read.rs` | `stock_warnings` 256건 경계 오류(`>= 256`) | `stock_warnings_enforces_256_item_limit_boundary` |

(리뷰 종료 후 `just mutants-daily` 결과 추가)
