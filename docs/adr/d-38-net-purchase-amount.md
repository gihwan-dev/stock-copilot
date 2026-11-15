# D-38 당일 순매수 체결 금액

- 상태: 초안
- 구현 기준: `54668ffd6a0cd0e49338067a4b9ef54ded752fc5`

## 근거와 결정

ADR 0052는 보유 종목 일간 손익을 “오늘 주식 평가 − 전일 주식 평가 − 당일 순매수 체결 금액”으로 정의하고, 순매수를 “매수 체결 금액에서 매도 체결 금액을 뺀 값”으로 정의한다. 흐름 구간은 `(전일 평가 시각, 현재 평가 시각]`이다. ADR 0064는 “D-38은 토스의 filledAmount를 사용하며 averageFilledPrice × filledQuantity로 대신하지 않는다”고 정한다. filledAt은 최종 체결 시각이며 총액과 수량은 누적값이다. 결제일·transactionTime은 사용하지 않는다. 수수료·세금 포함 총액이라는 계약이 없으므로 별도 가감 없이 원 통화의 총 체결 금액을 사용한다.

`net_purchase_amount`는 D-34의 계좌 봉인 `BrokerOrderSet`, 요청 계좌, D-31과 같은 `Timestamp` 쌍, 선택적인 `ClosedOrderEvidence`, `NetPurchaseScope`를 받는다. 종목별 또는 통화별 합계를 계산한다. 서로 다른 통화를 더하지 않는다. 앱 주문인지 판별하는 필터가 없어 조회로 들어온 외부 주문도 포함한다. BTreeMap의 주문 ID 순서로 계산하여 입력 순열과 오류 선택이 결정적이다. 중간 합계의 정밀도 손실·넘침은 불완전이며 상쇄를 추측하지 않는다.

두 평가 시각은 엄격히 증가해야 한다. 체결 시각은 주문 시각 이상·조회 시각 이하여야 한다. 최종 체결이 시작 시각 이하이면 제외한다. 구간에 최종 체결이 있어도 주문 시각이 시작 시각 이하이면 누적 체결을 나눌 증거가 없으므로 불완전이다. 최종 체결이 종료 이후여도 구간 안 체결 몫을 알 수 없어 불완전이다. 주문 상태가 FILLED여도 누적 이전 체결은 구별할 수 없어 같은 규칙이다. PARTIAL_FILLED라도 주문부터 최종 체결까지 모두 구간 안이면 총액을 계산할 수 있다. 시각·총액이 빠진 행은 0 체결이라고 추정하지 않는다. 다른 종목·통화의 행은 해당 범위 계산에 영향을 주지 않는다.

`NetPurchaseAmount`의 필드는 비공개 `Result<Money, NetPurchaseError>`다. 완전한 경우에만 `amount()`에서 `Ok(Money)`를 얻는다. 불완전에서 금액을 생성하거나 0으로 바꾸는 API가 없다. 오류는 입력값을 담지 않는다. 결과와 증거의 Debug는 원문을 가린다. 원시 타입·불린 타입·새 의존성·산술 허용 목록·주석·검사 끄기는 추가하지 않는다.

## 완전성 증거

`ClosedOrderEvidence`는 비공개 필드와 `pub(crate)` 생성자만 가진다. 계좌가 봉인된 주문 집합 전체와 두 평가 시각을 결합한다. 계산 시 전체 집합·구간의 동일성을 비교하고 조회 시각이 구간 끝 이상인지 확인한다. 다른 행 집합·계좌·평가 구간의 증거를 재사용할 수 없다. OPEN coverage는 CLOSED 완전성 증거가 아니다. 계산 안에서 만드는 비교 대상은 입력 증거를 대체하지 않는다. 외부 crate는 새 증거를 생성할 수 없다.

현재 증거는 T-15의 실제 요청 범위·페이지 종료를 검증하는 구현이 아니다. T-15가 내부 생성 경로를 연결하기 전에는 제품 호출자가 증거를 얻을 수 없어 불완전이다. 테스트는 crate 내부의 가상 사실만 사용한다. 어댑터를 공개 생성자로 우회하지 않는다.

## 불완전 이유

| 이유 | 의미 |
|---|---|
| MissingTotal | 해당 체결의 filledAmount 부재 |
| UnallocatableExecution | 시작 전에 낸 주문 또는 종료 뒤 최종 체결의 누적 총액을 구간별 배분 불가 |
| IncompleteQuery | 증거 부재·집합/구간 불일치 또는 평가 종료보다 이른 조회 |
| MissingExecutionTime | 해당 종목·통화 주문의 최종 체결 시각 부재 |
| AccountMismatch | 요청 계좌와 봉인된 집합 계좌 불일치. 행 혼입은 기존 D-34 생성 단계에서 거부 |
| WrongPeriod | 동일하거나 역전된 평가 시각 |
| InvalidExecutionTime | 주문 전 체결 또는 조회 후 체결 |
| Arithmetic(MoneyError) | 정확 덧셈·뺄셈의 넘침·정밀도 실패 |
| Snapshot(SnapshotError) | 전일 누락·잘못된 기간·불완전 주식 평가 등 기존 D-31 오류 |

## 일간 손익 연결과 예제

새 자유 함수 `holdings_daily_profit_with_orders`는 기존 `validated_previous`, `ValuationSnapshot::stock_value`, `Money`를 사용한다. 반환값은 `Result<Money, NetPurchaseError>`이며 순매수의 불완전을 그대로 전달한다. 기존 `HoldingsDailyProfit`의 제공값 선택·대조 구현과 impl·기존 테스트는 수정하지 않는다. A-08은 기존 제공값 선택과 새 계산값을 결합해야 하며, 계산값을 토스 제공값으로 위장하지 않는다. 인자 수 검사에 맞춰 계좌/집합과 평가 시각을 각각 쌍으로 전달한다.

| 예제 | 전일 평가 | 현재 평가 | 순매수 | 일간 손익 | 근거 |
|---|---:|---:|---:|---:|---|
| USD, 매매 없음 | 7,000 | 6,500 | 0 | -500 USD | ADR 0052가 가리키는 R-11 예제 6과 기존 D-31 테스트의 원 통화 값 |
| KRW, 매수·매도 | 1,400 | 1,700 | 157 − 31 = 126 | 174 KRW | ADR 0052 식을 독립 계산한 추가 경계 예제 |
| USD 금액 주문 | 해당 없음 | 해당 없음 | 43.21 USD | 해당 없음 | 주문금액 99·보고수량 0·체결수량 3인 누적 총액을 그대로 사용 |
| 증거 없음 | 임의 | 임의 | 불완전 | 불완전 | 0 치환 금지 |

`design.md` 2.4는 화면의 오늘 손익을 명시하지만 수치 예제를 제시하지 않는다. 원화 계좌 변동 -1,250,000은 환율 변화를 포함한 별도 양이므로 위 USD 주식 손익을 현재 환율로만 환산하여 비교하지 않는다.

## 검사

테스트는 새 모듈 아래에만 추가했다. 4,096회 독립 정수 기준 계산과 입력 역순 대조, 양쪽 경계·같은 시각·상쇄·음수 순매수·통화와 종목 분리·금액 주문·누적 경계 걸침·총액/시각/증거 누락·증거 집합/구간 불일치·계좌 혼입·넘침·일간 손익 불완전 전파를 검증한다. 좁은 검사: `cargo test -p domain --lib net_purchase`, `cargo clippy -p domain --all-targets --locked`, `just source-check raw-types`.

자동 변이 검사는 지시에 따라 실행하지 않는다. 손 변형은 `mktemp -d`에 HEAD 아카이브와 새 소스를 복사하여 하나씩 원본에 적용하고 실제 `cargo test -p domain --lib net_purchase`를 실행한다. 컴파일 성공과 테스트 실패를 각각 확인하며, 복구한 원본도 다시 실행한다. 실행별 로그는 작업 사본의 `target/D-38/`에 둔다.

## 인계

| 작업 | 의무 |
|---|---|
| T-15 | 실제 CLOSED 조회 범위·계좌·모든 페이지 종료·격리 없음 확인 후 domain 내부 생성 경로 연결. orderedAt 조회 필터만으로 filledAt 구간의 완전성을 주장하지 않음. 시작 전에 낸 주문과 미종료 부분 체결의 누락도 검토. 잘못된 행을 버리고 증거를 만들지 않음 |
| T-15 | 누적 경계를 나눌 수 있는 개별 체결 증거가 확보되면 별도 계약으로 확장. 금액 주문 수량과 외부 주문 포함을 실제 응답으로 확인 |
| A-16 | 실제 직전 평가 스냅샷과 같은 계좌의 주문 집합·증거를 선택. D-31 스냅샷 자체에 계좌 키가 없어 호출 계층에서 결합 |
| A-08 | 새 계산 함수의 성공/불완전과 기존 토스 제공값·출처를 결합하여 대조. 불완전은 0이나 일치로 표시하지 않음 |

## 손 변형 실행 결과

24개 모두 컴파일 성공 후 테스트 실패로 검출됐다. 생존 0, 컴파일 실패 0. 원본 복구 후 같은 테스트가 통과했다. 외부 integration test에서 증거 생성자를 호출한 별도 컴파일 실험은 E0624로 거부되어 봉인도 확인했다.

| 번호 | 바꾼 것 | 잡은 테스트 (대표) |
|---|---|---|
| 1 | 계좌 비교 반전 | `boundaries_totals_sides_and_scope` |
| 2 | 동일 평가 시각 허용 | `evidence_account_and_period_are_bound` |
| 3 | 조회 시각 경계 제외 | `equal_order_fill_and_observation_times_are_valid` |
| 4 | 조회 시각 비교 반전 | `amount_order_native_currency_and_precision_failures` |
| 5 | 증거 비교 반전 | `amount_order_native_currency_and_precision_failures` |
| 6 | 종목 비교 반전 | `boundaries_totals_sides_and_scope` |
| 7 | 통화 비교 반전 | `equal_order_fill_and_observation_times_are_valid` |
| 8 | 시장 통화 반전 | `equal_order_fill_and_observation_times_are_valid` |
| 9 | 제외 행 처리 반전 | `equal_order_fill_and_observation_times_are_valid` |
| 10 | 주문 전 체결 경계 거부 | `equal_order_fill_and_observation_times_are_valid` |
| 11 | 주문 전 체결 비교 반전 | `boundaries_totals_sides_and_scope` |
| 12 | 조회 동시 체결 거부 | `equal_order_fill_and_observation_times_are_valid` |
| 13 | 조회 후 체결 비교 반전 | `amount_order_native_currency_and_precision_failures` |
| 14 | 잘못된 체결 시각 AND | `incomplete_and_period_evidence` |
| 15 | 왼쪽 경계 포함 | `boundaries_totals_sides_and_scope` |
| 16 | 오른쪽 경계 제외 | `equal_order_fill_and_observation_times_are_valid` |
| 17 | 시작 동시 주문 허용 | `incomplete_and_period_evidence` |
| 18 | 경계 걸침 AND | `incomplete_and_period_evidence` |
| 19 | 매수 부호 반전 | `amount_order_native_currency_and_precision_failures` |
| 20 | 매도 부호 반전 | `boundaries_totals_sides_and_scope` |
| 21 | 손익 순매수 부호 반전 | `daily_formula_and_adr52_native_currency_example` |
| 22 | 손익 전일 평가 부호 반전 | `daily_formula_and_adr52_native_currency_example` |
| 23 | 평가 시각 뒤바꿈 | `daily_connection_propagates_incomplete` |
| 24 | 불완전 총액 오류 바꿈 | `incomplete_and_period_evidence` |
