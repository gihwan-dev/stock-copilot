# ADR 0075: D-38 당일 순매수 체결 금액

- 상태: 확정 (2026-11-15)

- 최초 기준: `54668ffd6a0cd0e49338067a4b9ef54ded752fc5`
- 리뷰 수정 기준: `9d525f6a4eddd9e3336a3bc9ce061f17f556ad4c`. 먼저 `git merge --no-ff --no-edit master` 실행, Already up to date.
- 근거: 메인이 확정한 D-38 리뷰 수정 결정, ADR 0052·0064, design 2.4.

## 결정

ADR 0052의 보유 종목 일간 손익은 `현재 주식 평가 − 전일 주식 평가 − 순매수 체결 금액`이다. 순매수는 원 통화별 `매수 filledAmount 합 − 매도 filledAmount 합`이다. ADR 0064의 “averageFilledPrice × filledQuantity로 대신하지 않는다”를 따른다. 결제일에서 체결일을 역산하지 않는다. 앱 안팎 주문을 구분하는 필터는 없다.

단순화를 위해 `ClosedOrderEvidence`, 동치 비교, `BrokerOrderSet` 우회, 계좌 인자, 시각 튜플, `NetPurchaseAmount` 래퍼를 삭제한다. `ClosedOrderSet`은 계좌·D-34의 봉인 `BrokerOrder`들·조회가 덮은 `TimeInterval`·조회 시각·`ClosedOrderCompleteness`를 가진 새 봉인 집합이다. 공개 생성자 `new`는 계좌 일치, 종료 상태, ID 중복 없음, 행 조회 시각 일치, 조회 범위 끝 ≤ 조회 시각, 주문 시각 ≤ 조회 시각, 주문 시각 ≤ 최종 체결 시각 ≤ 조회 시각을 검증한다. 필드는 비공개이며 Debug에 원문을 넣지 않는다.

완전성은 `Complete` / `Incomplete(MissingPages | RejectedRows | RequestFailed)`다. 페이지 종료 여부는 어댑터가 관찰하여 넘기는 신뢰 경계이며 domain 생성자가 네트워크 진위를 인증하지 않는다. 계산은 Complete이고 조회 범위가 평가 구간을 덮으며 조회 시각이 평가 종료 이상일 때만 완전하다. 범위는 체결 구간에 대한 조회의 포괄 범위이며 주문 시각 필터를 그대로 체결 포괄 범위라고 주장할 수 없다. 부분 응답을 빈 Complete로 바꾸지 않는다.

D-34의 `open_state() == Closed`인 상태만 집합에 허용한다. 따라서 Pending 계열·PartialFilled·Unknown은 거부한다. ADR 0064의 CLOSED 조회에도 PartialFilled가 올 수 있다는 제한은 여전히 유효하다. 이 상태만으로 종료 사실을 증명할 수 없으므로 T-15가 임의로 Filled로 바꾸거나 버린 뒤 Complete로 선언하면 안 된다. 종료된 부분 체결은 Canceled 등 종료 상태로 확인된 행에서 누적 금액을 계산한다. 추가 종료 사실이 필요하면 T-15가 별도 계약을 확정한다.

체결 수량 0은 정확한 기여분 0이다. 시각이 없고 총액이 없거나 0이면 제외한다. 단 0수량인데 체결 시각이 있거나 비0 체결 금액이 있으면 생성 단계의 입력 불일치다. 체결 수량이 있는데 시각이 없으면 불완전이다. `ordered_at > current`인 주문은 체결 시각·총액 부재와 무관하게 제외한다. 입력 불일치는 구간·종목 필터로 숨기지 않고 생성 단계에서 거부한다.

집합은 주문 ID로 정렬한 BTreeMap이므로 입력 순서와 오류 선택이 결정적이다. 원 통화끼리만 더한다. 중간 합계의 정확 산술이 실패하면 입력 불일치로 구분하고 0으로 바꾸지 않는다. 수수료·세금을 별도 가감하지 않으며 filledAmount에 포함되는지는 T-15 실측으로 남긴다. 이는 토스 손익과 맞는지 판정에 직접 영향을 준다.

## 최종 공개 API

| API | 역할 |
|---|---|
| `ClosedOrderSet::new(account, orders, covered, observed, completeness)` | 공개 입력 검증과 봉인. `Result<ClosedOrderSet, NetPurchaseInputError>` |
| `net_purchases(&ClosedOrderSet, TimeInterval)` | 통화별 결과 묶음 `NetPurchases`. `currency`가 `Result<Money, NetPurchaseError>` 반환 |
| `net_purchase_amount(&ClosedOrderSet, TimeInterval, NetPurchaseScope)` | 종목 또는 통화별 `Result<Money, NetPurchaseError>` 직접 반환 |
| `holdings_daily_profit_with_orders(today, previous, orders, currency)` | 평가 차이에서 새 순매수 결과를 차감 |
| `compare_holdings_daily_profit_with_orders(today, previous, orders, rows, reported)` | 기존 `HoldingsDailyProfit::compare`로 토스 정본·출처를 선택하고 새 계산을 대조한 `HoldingsDailyWithOrders` 반환 |

별도 계좌 인자와 인자 수를 피하는 튜플은 없다. `TimeInterval`이 엄격히 증가하는 구간을 보장하므로 새 순매수 오류에 WrongPeriod를 두지 않는다. 기존 스냅샷의 역전된 기간 오류는 기존 SnapshotError를 입력 불일치에 보존한다. 기존 impl·기존 테스트는 바꾸지 않는다. 이 브랜치에서 추가했던 D-38 테스트만 새 설계에 맞췄다.

## 오류와 대조

`NetPurchaseError`는 두 변형이다.

| 분류 | 닫힌 이유 |
|---|---|
| `Incomplete(NetPurchaseIncomplete)` | MissingTotal, UnallocatableExecution, MissingExecutionTime, Query(조회 불완전 이유), OutsideQueryRange, ObservationTooEarly, Snapshot(기존 평가 부족), KrwSummaryNotComparable |
| `InvalidInput(NetPurchaseInputError)` | AccountMismatch, NonClosedOrder, DuplicateOrder, ObservationMismatch, InvalidTime, ZeroFillMismatch, Arithmetic(정확 산술 오류), Snapshot(역전 평가 시각) |

`OrderDailyCurrency`는 계산 결과, 기존 `HoldingsDailySelection`의 토스 금액·출처·전일 출처, `OrderDailyComparison`을 함께 보존한다. 성공한 계산과 선택값이 있으면 기존 `compare_money` 정책으로 Compared(Matches/Different)를 낸다. 계산이 불완전하거나 입력 불일치이면 토스 값은 그대로 두고 NotCompared(원인)를 낸다. 토스 제공값이 없으면 성공 계산의 대조는 NotProvided다. 기존 ADR 0052의 혼합 원화 요약 제한도 보존한다. 해외 주식이 있을 때 원화 요약은 국내 통화별 계산과 비교하지 않고 KrwSummaryNotComparable로 남긴다. 기존 `HoldingsDailyProfit` 원본도 접근 가능하다.

## 시각 조합 표

P는 전일 평가, C는 현재 평가, o는 주문, f는 최종 체결, R은 조회 시각이다. P < C는 TimeInterval이 보장한다. 생성 단계의 입력 검증이 계산 제외보다 우선한다.

| 조건 | 처리 |
|---|---|
| f < o, f > R, o > R, 조회 범위 끝 > R | 입력 불일치 InvalidTime |
| 수량 0인데 f 있음 또는 비0 총액 있음 | 입력 불일치 ZeroFillMismatch |
| R < C | 불완전 ObservationTooEarly |
| 조회 Incomplete 또는 조회 범위가 평가 구간을 덮지 않음 | 불완전 Query / OutsideQueryRange |
| o > C | 제외, 기여분 0 |
| f 없음, 체결 수량 0 | 제외, 기여분 0 |
| f 없음, 체결 수량 있음 | 불완전 MissingExecutionTime |
| f ≤ P | 제외, 기여분 0 |
| P < o ≤ f ≤ C | 총액으로 포함. o=f, f=C 허용 |
| o ≤ P < f ≤ C | 불완전 UnallocatableExecution |
| P < o ≤ C < f | 불완전 UnallocatableExecution |
| o ≤ P, f > C | 불완전 UnallocatableExecution |
| 포함할 체결의 총액 없음 | 불완전 MissingTotal |

테스트는 P=100, C=200으로 두고 o·f·R을 11개 순서 대표값(경계 전·동일·사이·이후 각 세 값 포함)으로 전수 조합한다. f가 있는 1,331개와 f가 없는 수량 0/3의 242개, 총 1,573개를 독립 판정 표와 대조한다. 추가로 양 끝 경계·조회 범위 경계·0수량 모순을 명시한 표 테스트를 둔다.

## 하루 시나리오와 예제

| 행 | 총액 | 처리 |
|---|---:|---|
| 오늘 매수 1 | 157 KRW | 포함 |
| 오늘 매수 2, f=C | 89 KRW | 포함 |
| 오늘 매도 | 31 KRW | 차감 |
| 미체결 취소, 체결 수량 0, f 없음 | 총액 없음 | 제외 |
| 평가 시각 뒤 주문 | 9,999 KRW | 제외 |
| 어제 체결 주문 | 777 KRW | 제외 |
| 독립 계산 결과 | 157 + 89 − 31 = 215 KRW | 완전. 역순 입력도 동일 |

일간 손익 예제는 KRW 평가 1,400 → 1,700, 순매수 157−31=126에서 174 KRW다. ADR 0052·R-11의 USD 평가 7,000 → 6,500, 순매수 0에서 −500 USD다. USD 토스 제공값 −500이면 Matches, −490이면 Different다. 조회 불완전이면 계산 오류와 NotCompared(조회 이유)를 보존하면서 토스 제공값 −499는 그대로 유지한다. USD 금액 주문은 보고 수량 0·체결 수량 3·주문 금액 99와 무관하게 filledAmount 43.21 USD를 쓴다.

## 검사

수정 전 회귀 테스트는 미체결 취소 주문이 MissingExecutionTime이 되어 실패함을 확인했다(`target/D-38/review-red.log`). 좁은 검사는 `cargo test -p domain --lib net_purchase`, `cargo clippy -p domain --all-targets --locked`, `just source-check raw-types`다. 독립 정수 기준 속성 테스트 4,096회 및 입력 역순을 대조한다. 새 패키지·원시 타입·주석·검사 끄기·허용 목록 변경은 없다.

손 변형은 `mktemp -d`의 HEAD 아카이브에 이번 세 모듈과 lib를 복사하여 한 번에 하나씩 적용했다. 각 변형에서 실제 `cargo test -p domain --lib net_purchase` 실행·컴파일 성공·테스트 실패를 별도로 기록한다. 로그는 `target/D-38/review-mutation-*.log`, 원본 복구 검사는 `review-restored.log`다. 자동 변이 검사는 실행하지 않는다.

## 인계

| 작업 | 책임 |
|---|---|
| T-15 | 실제 요청·응답 계좌 결합, CLOSED 마지막 페이지까지 받았는지와 누락·격리·실패 여부로 Complete/Incomplete 결정. 조회 범위가 filledAt 평가 구간을 덮는지 보장. 오래된 주문의 늦은 체결을 orderedAt 필터로 누락하지 않음. 실패/식별 불가 행을 버리고 Complete로 만들지 않음 |
| T-15 | CLOSED의 PartialFilled 응답에 추가 종료 사실이 필요한지 계약 확정. 현재 상태만으로 종료라고 추정하지 않음. 미종료 부분 체결의 일간 손익 포함 범위도 별도 검토 |
| T-15 | filledAmount의 수수료·세금 포함 여부 실측. 토스 일간 손익과의 Matches/Different 판정에 직접 영향이 있음. 금액 주문 수량·외부 주문 포함도 실측 |
| A-16 | 실제 직전 스냅샷과 ClosedOrderSet 계좌를 결합. 스냅샷 자체에 계좌 키가 없어 이 책임은 호출 계층에 있음 |
| A-08 | 새 대조 자유 함수를 호출하고 selected 토스 값·computed·comparison을 사용. Incomplete와 InvalidInput을 구분하고 NotCompared 이유를 보존. 계산 불완전을 0이나 일치로 표시하지 않음 |

## 리뷰 수정 손 변형 결과

35개 모두 컴파일 성공 후 테스트에 검출됐다. 생존 0, 컴파일 실패 0. 원본 복구 후 통과했다. 별도 외부 integration test에서도 공개 생성자로 Complete 종료 주문 집합을 만들고 완전한 0을 계산하는 경로가 통과했다(`review-public-api.log`).

| 번호 | 바꾼 것 | 잡은 테스트 (대표) |
|---|---|---|
| 1 | 범위 제외 반전 | `missing_time_total_and_query_remain_incomplete` |
| 2 | 평가 종료 동시 주문 제외 | `missing_fill_time_table_distinguishes_zero_and_nonzero_execution` |
| 3 | 평가 뒤 주문 제외 삭제 | `review_zero_fill_and_future_orders_are_exact_zero` |
| 4 | 0체결 제외 반전 | `missing_time_total_and_query_remain_incomplete` |
| 5 | 왼쪽 체결 경계 포함 | `time_order_table` |
| 6 | 시작 동시 주문 배분 허용 | `time_order_table` |
| 7 | 오른쪽 체결 경계 제외 | `time_order_table` |
| 8 | 누적 걸침 OR를 AND로 | `time_order_table` |
| 9 | 매수 부호 반전 | `scopes_native_amount_orders_and_arithmetic` |
| 10 | 매도 부호 반전 | `realistic_day_is_complete_and_order_independent` |
| 11 | 종목 비교 반전 | `scopes_native_amount_orders_and_arithmetic` |
| 12 | 통화 비교 반전 | `missing_fill_time_table_distinguishes_zero_and_nonzero_execution` |
| 13 | 시장 통화 반전 | `time_order_table` |
| 14 | 조회 동시 평가 거부 | `coverage_and_observation_boundaries` |
| 15 | 조회 시작 동시 경계 거부 | `coverage_and_observation_boundaries` |
| 16 | 조회 종료 동시 경계 거부 | `coverage_and_observation_boundaries` |
| 17 | 조회 구간 덮음 AND | `coverage_and_observation_boundaries` |
| 18 | 계좌 비교 반전 | `zero_quantity_with_execution_time_or_nonzero_total_is_inconsistent` |
| 19 | 종료 상태 비교 반전 | `missing_fill_time_table_distinguishes_zero_and_nonzero_execution` |
| 20 | 행 조회 시각 비교 반전 | `realistic_day_is_complete_and_order_independent` |
| 21 | 조회 종료 동시 범위 거부 | `zero_quantity_with_execution_time_or_nonzero_total_is_inconsistent` |
| 22 | 조회 동시 주문 거부 | `time_order_table` |
| 23 | 주문 체결 동시 시각 거부 | `every_time_ordering_including_equalities_matches_the_decision_table` |
| 24 | 조회 체결 동시 시각 거부 | `every_time_ordering_including_equalities_matches_the_decision_table` |
| 25 | 잘못된 체결 시각 AND | `invalid_time_orders_are_rejected_before_any_exclusion` |
| 26 | 0체결 모순 검증 대상 반전 | `identity_status_duplicates_and_metadata` |
| 27 | 0체결 모순 OR를 AND로 | `zero_quantity_with_execution_time_or_nonzero_total_is_inconsistent` |
| 28 | 0체결 금액 일치 비교 반전 | `zero_quantity_with_execution_time_or_nonzero_total_is_inconsistent` |
| 29 | 중복 주문 검증 반전 | `every_time_ordering_including_equalities_matches_the_decision_table` |
| 30 | 일간 손익 순매수 부호 반전 | `daily_arithmetic_failure_is_input_error_with_reported_value_retained` |
| 31 | 일간 손익 전일 평가 부호 반전 | `daily_formula_and_reported_selection_are_connected` |
| 32 | 대조 안 함 이유 삭제 | `daily_arithmetic_failure_is_input_error_with_reported_value_retained` |
| 33 | 혼합 원화 요약 방어 반전 | `daily_formula_and_reported_selection_are_connected` |
| 34 | 혼합 원화 요약 통화 뒤바꿈 | `daily_formula_and_reported_selection_are_connected` |
| 35 | 평가 시각 뒤바꿈 | `snapshot_errors_do_not_erase_reported_values` |
