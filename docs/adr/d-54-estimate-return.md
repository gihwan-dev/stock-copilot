# D-54 추정별 수익률과 보정 이력

- 상태: 초안
- 기준: c40882363b8163d1942bbe00e8ce4e66710a3f85
- 근거: D-54, D-23, D-27, A-15, design 6.6, ADR 0041·0051·0072·0136

## 결정

`EstimateEntryPrice`는 거래나 신호 없이 제출과 가격 관측을 묶는다. 종목 ID와 시장·코드, 통화가 과제와 일치하고 관측 시각이 제출 시각과 정확히 같을 때만 만든다. 가격 사실은 원래 제출을 보존한다. 정확한 시각의 자료가 없을 때 다른 가격으로 대체하는 정책은 정하지 않는다. 기존 제출 원장에는 이 관측이 없으며 이 타입을 추가해도 과거 자료가 생기는 것은 아니다.

`EstimateCashFlows`는 입력한 한 주의 진입·청산 가격과 해당 한 주에 대응하는 양수 또는 0의 매수·매도 비용으로 `(청산가 - 매도 비용 - 진입가 - 매수 비용) / (진입가 + 매수 비용)`을 계산하는 산술 부품이다. 가격은 양수인 기존 `Price`, 비용은 `Money`를 사용하고 음수 비용과 통화 불일치를 거부한다. 비용이 청산가를 넘어도 산술 결과를 임의로 자르지 않는다. 결과의 음수는 손실이고, 비용이 커지면 수익률은 높아지지 않는다. 이 부품의 입력 가격과 비용이 특정 판정에 대한 올바른 증거인지는 생성자가 증명하지 않으며, 이 부품만으로 판정별 수익률이 완성됐다고 표시해서는 안 된다.

`BenchmarkCashFlows`는 두 가격의 `(종료가 - 시작가) / 시작가`를 계산한다. 초과 수익은 위 순수익률에서 이를 뺀다. 두 벤치마크 가격과 종목 가격·비용의 통화가 같고 비교 구간의 양 끝 시각이 같아야 한다. 입력 구간 일치 검사는 가격 출처와 실제 가격 관측 시각의 진위를 증명하지 않는다. 어떤 지수를 언제 관측할지와 장중 자료가 없을 때의 정책은 아래 인계로 남긴다.

금액 덧셈·뺄셈은 기존 정확 산술을 사용하고 넘침과 정밀도 손실을 오류로 둔다. 나눗셈은 기존 `RatioQuotient`를 사용하여 유한하게 정확한 값과 근삿값을 구별한다. 근삿값이 하나라도 포함된 초과 수익도 근삿값으로 남긴다. 예를 들어 2/3은 `0.6666666666666666666666666667`, −2/3은 `-0.6666666666666666666666666667`로 나온다. 근삿값의 후속 빼기에서 정밀도 손실이 생기면 잘라서 성공시키지 않는다. 표시용 반올림이나 보정 집계용 12자리 양자화를 이 산술에 섞지 않는다.

보정 이력은 기존 전체 사실 65,536건 한도를 유지한다. `Calibration::capacity`는 다음 사실 추가 가능 여부를 `Available` 또는 `Exhausted`로 나타낸다. 상한에서 `with_fact`는 복사·변경 전에 `TooManyFacts`를 반환하며 기존 사실과 표는 그대로다. A-15는 기존 ADR 0136대로 사유를 기록하고 새 추정을 미판정으로 둔다. 다른 모델·기간 조합에는 각각 독립 상한이 적용된다. 이 상태는 자동으로 해소되지 않는다. 이는 새 구간이나 이력 폐기를 승인하는 결정이 아니다.

## 정의 대기와 제안

| 항목 | 선택지와 제안 | 사용자에게 보이는 영향 |
|---|---|---|
| 제출 전 발생한 목표·손절 | 제안: 제출 전에 판정이 끝난 추정은 수익률 산출 보류. 대안: 과제 기준 시각부터 가상 거래하거나 제출부터 새로 목표·손절과 만기를 정한다 | 전자는 수익률이 보이지 않는다. 과거 기준은 실제 제출 이후보다 높거나 낮게 보일 수 있고, 새 기준은 다른 사건의 수익률이 된다 |
| 제출 가격 부재 | 제안: 정확한 관측이 없으면 보류. 대안: 제출 직전의 최신 가격 또는 다음 거래 가격을 쓴다 | 보류는 숫자가 없다. 대체 가격이 높으면 매수 수익률이 낮아지고 낮으면 높아진다 |
| 원 판정의 기준가와 가상 진입 | 제안: 원 판정은 수정하지 않고 두 가격·시각 차이가 해소될 때까지 결과 결합을 보류. 대안: 원 과제의 목표·손절을 유지하거나 제출 가격을 기준으로 재산출한다 | 진입가와 경계가 달라지면 도달 여부·청산 시각·수익률이 어느 방향으로든 달라진다 |
| 목표·손절·만기 가격과 수정 계수 | 제안: 같은 수정 기준의 완전한 봉과 도달 증거가 있을 때만 계산. 대안: 원 판정의 목표·손절 경계, 갭 시가 및 만기 종가를 채택하거나 제출 이후 별도 가상 청산을 채택한다 | 시가 갭·분할·동일 봉 양쪽 도달에서 결과와 수익률이 높거나 낮아질 수 있다. 일봉만 있으면 장중 시각을 특정할 수 없다 |
| 비용의 근거와 적용 | 제안: 비용 자료가 없으면 보류. 대안: ADR 0072의 비례 요율을 가상 한 주에 적용하거나 실제 체결 비용을 해당 수량에 배분한다 | 비용을 크게 잡을수록 수익률이 낮다. 실제 비용은 거래하지 않은 추정에 없고 최소 수수료·원 단위 절사는 수량에 의존한다 |
| 벤치마크 | 제안: 실제 진입·청산 시각과 통화가 맞는 가격이 없으면 보류. 대안: ADR 0072의 거래일 종가 근사를 명시하고 사용한다 | 종가 근사는 장중 움직임에 따라 초과 수익이 높거나 낮아질 수 있다. 국내 가격지수의 배당 제외는 별도 표시가 필요하다 |
| 조합별 상한 이후 | 제안: 현재처럼 사유를 남기고 닫힌 실패를 유지하면서 전체 이력과 비중첩 선택 의미를 보존하는 확장 저장을 설계한다. 대안: 명시적 새 구간으로 전환하고 이전 구간을 따로 보존한다 | 닫힌 실패는 이후 표본이 보정에 반영되지 않는다. 새 구간만 쓸 때 성공률·표본 수가 과거 전체 기준보다 높거나 낮아질 수 있다 |

## 인계

| 받는 작업 | 내용 |
|---|---|
| A-15, S-18 | 제출 시점 관측의 저장·복원과 결측, 상한 도달 사유·미판정 보존 및 판정과 보정의 원자적 갱신을 연결한다. 가격·비용·벤치마크 정책 확정 후에만 산술 부품을 판정과 결합한다 |
| D-27, D-29 | 과제 기준 시각과 제출 시각이 다른 추정의 판정·가상 청산 의미, 경계·수정 기준·동시 도달 및 결과 증거를 정한다 |
| E-08, E-91 | 상한 이후 전체 이력 집계 또는 명시적 구간의 의미, 기간·통화가 맞는 벤치마크와 근사·누락 표시를 확정한다 |
| S-23 | 분할·배당락과 가격 자료의 과거 보존·완전성을 실측한다 |

## 검증

프로젝트 신뢰와 세션 훅 정의 신뢰의 실제 적용은 확인하지 못했다. 시작 시 frozen 설치 및 lefthook 설치는 성공했다. 최초 테스트는 새 API 부재로 컴파일 실패한 다음 구현 뒤 통과했다. Git 훅과 세션 훅 적용은 구별한다.

## 손 변형

각 변형은 분리된 임시 복사본에서 하나씩 적용하고 해당 단위 또는 통합 시험을 실행했다. 결과는 아래 표와 같다.

| 번호 | 바꾼 것 | 검출한 시험 |
|---|---|---|
| 1 | 제출 종목 일치 조건 반전 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 2 | 제출 시각 일치 조건 반전 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 3 | 제출 가격 통화 검사 반전 | `estimate_return::tests::d54_submission_price_rejects_other_instruments_and_currencies` |
| 4 | 청산 가격 통화 검사 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods` |
| 5 | 매수 비용 통화 검사 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods` |
| 6 | 매도 비용 통화 검사 반전 | `domain/tests/estimate_return.rs tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 7 | 매수 비용 음수 조건 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 8 | 매도 비용 음수 조건 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 9 | 음수 비용 양쪽 동시 검사로 완화 | `domain/tests/estimate_return.rs tests::d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods` |
| 10 | 매수 비용을 지출에서 뺌 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 11 | 매도 비용을 수취액에 더함 | `domain/tests/estimate_return.rs tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 12 | 수익의 지출과 수취 순서 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods` |
| 13 | 투자 지출 대신 수취액을 분모로 사용 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 14 | 벤치마크 통화 비교 조건 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 15 | 벤치마크 구간 일치 조건 반전 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 16 | 벤치마크 양끝 통화 검사 반전 | `domain/tests/estimate_return.rs tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 17 | 벤치마크 변화분 부호 반전 | `domain/tests/estimate_return.rs tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 18 | 벤치마크 시작가 대신 종료가를 분모로 사용 | `domain/tests/estimate_return.rs tests::d54_cash_flow_errors_propagate_at_each_arithmetic_boundary` |
| 19 | 초과수익에서 벤치마크를 더함 | `domain/tests/estimate_return.rs tests::d54_overflow_is_not_turned_into_a_return` |
| 20 | 보정 이력 상한 등호 누락 | `estimate_return::tests::d54_capacity_reports_full_without_changing_existing_evidence` |

검출 20건, 컴파일 실패 1건(검출 수에서 제외), 생존 0건이다. 제출 시각 접근자 변형은 const 함수에서 비const 함수를 호출해 컴파일되지 않았다.
