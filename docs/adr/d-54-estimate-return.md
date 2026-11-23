# D-54 추정별 수익률과 보정 이력

- 상태: 초안
- 기준: c40882363b8163d1942bbe00e8ce4e66710a3f85
- 근거: D-54, D-23, D-27, A-15, design 6.6, ADR 0041·0051·0072·0136

## 결정

`EstimateEntryPrice`는 거래나 신호 없이 발급 ID가 보존된 `EstimateSubmissionRecord`와 정확히 같은 시각의 가격 관측을 묶는다. 종목 ID·시장·코드와 통화가 과제와 일치해야 한다. 같은 내용의 두 제출도 발급 ID로 구별하며, 결과가 이 ID와 제출·판정을 함께 보존한다. 기존 제출 원장에는 이 관측이 없으므로 과거 자료가 자동으로 생기지는 않는다.

`estimate_return`은 제출, 기존 판정, 봉·시장 달력·기업 행사·가격 수정 기준, 비용 사실과 벤치마크를 결합한다. 원 과제와 같은 입력으로 판정을 재계산해 원래 판정과 일치하는지 확인하고, 신호나 체결 기록 없이도 같은 경로를 쓴다. 판정의 원래 기준가와 목표·손절 경계를 유지하며 진입에는 제출 관측가를 쓴다. 성공은 목표 경계, 손절은 손절 경계, 만료는 만기 봉의 종가를 쓴다. 분할은 원 경계에 전체 수정 계수를 적용하고 제출가에는 제출 이후의 계수만 적용한다. ADR 0051에 따라 행사 기준일의 시장 현지일 시작에 행사가 적용되므로 제출 당일의 행사는 제출가에 다시 적용하지 않는다. 정확히 표현할 수 없는 수정 가격, 수정 기준보다 늦은 청산, 누락되거나 서로 다른 봉·판정·기업 행사, 식별할 수 없는 달력은 숫자 대신 사유를 돌려준다.

제출 전에 판정됐거나 같은 분봉·일봉에서 제출 전후 순서를 확인할 수 없는 경우, 제출가가 이미 원 목표·손절 경계를 벗어난 경우는 결합을 보류한다. 제출이 정규장 시작보다 이른 같은 날의 일봉 도달은 제출 이후로 확인할 수 있으므로 계산한다. 일봉만으로 확인된 도달은 장중 시각을 만들지 않고 거래일을 보존하며, 분봉의 시각은 실제 체결 시각이 아닌 봉 종료 경계다. 제출 시각의 표기 오프셋과 관계없이 시장 현지 관측일을 사용한다. 정규장 밖의 과제 기준 관측은 ADR 0051의 원 판정처럼 다음 정규장에서 평가를 시작하면서 제출 가격을 진입가로 보존한다. 갭으로 시가가 경계를 넘어선 경우와 양쪽 경계를 같은 봉에서 건드린 모호한 판정도 아래의 정의 대기로 둔다.

비용은 같은 과제·만기·시장의 `SignalReturnCost`에 묶인 매수·매도 비례 요율을 수정된 가상 한 주에 적용한다. 순수익률은 `(청산가 - 매도 비용 - 진입가 - 매수 비용) / (진입가 + 매수 비용)`이다. 임의의 가격·비용·기간을 받아 추정 수익률을 만드는 생성자는 공개하지 않는다. 비용 사실이 없거나 과제와 다르면 수익률을 임의로 만들지 않는다. 수량에 따른 최소 수수료·절사는 이 비례 비용의 범위 밖이다.

초과 수익은 순수익률에서 `SignalBenchmark`의 제출 거래일~청산 거래일 수익률을 뺀다. 양쪽 자료의 통화와 실제 정규장 종료 시각을 대조한다. 제출과 청산이 서로 다른 거래일의 정규장 종료 시각이면 `Exact`, 그 외 장중 또는 같은 날 비교면 `DailyCloseApproximation`을 별도 정렬 표지로 남긴다. 벤치마크가 없거나 불일치하면 초과 수익만 오류로 두고 순수익률은 보존한다. 벤치마크 연결 수익률은 기존 `PerformanceValue`에 수치 정밀도 표지가 없으므로 초과 수익의 수치 정확도는 근사로 보존한다. 정렬 표지와 수치 정확도는 서로 다른 정보다.

금액 덧셈·뺄셈은 기존 정확 산술을 사용하고 넘침과 정밀도 손실을 오류로 둔다. 나눗셈은 기존 `RatioQuotient`로 유한하게 정확한 값과 근삿값을 구별한다. 예를 들어 2/3은 `0.6666666666666666666666666667`, −2/3은 `-0.6666666666666666666666666667`로 나온다. 표시용 반올림이나 보정 집계용 12자리 양자화를 이 산술에 섞지 않는다.

보정 이력은 기존 전체 사실 65,536건 한도를 유지한다. `Calibration::capacity`는 다음 사실 추가 가능 여부를 `Available` 또는 `Exhausted`로 나타낸다. 상한에서 `with_fact`는 복사·변경 전에 `TooManyFacts`를 반환하며 기존 사실과 표는 그대로다. A-15는 기존 ADR 0136대로 사유를 기록하고 새 추정을 미판정으로 둔다. 판정과 이벤트가 저장되지 않아 해당 추정은 확정 판정 표본으로 성적표에 기여하지 못한다. 모델·기간 조합별 상한이며 자동 해소되거나 과거 이력을 삭제하지 않는다. 용량 상한 자체는 기존 보정표와 원장 위치가 일치하는 신호를 별도로 차단하지 않는다. 다른 신호 조건은 계속 적용된다.

## 정의 대기와 제안

| 항목 | 선택지와 제안 | 사용자에게 보이는 영향 |
|---|---|---|
| 제출 전 발생한 목표·손절 또는 제출 가격이 이미 경계 밖인 경우 | 제안: 결합 보류. 대안: 과제 기준 시각부터 가상 거래하거나 제출부터 경계를 다시 만든다 | 보류는 수익률이 없다. 과거 기준 또는 새 경계는 실제 제출 이후보다 높거나 낮게 보일 수 있다 |
| 제출 가격 부재 | 제안: 정확한 관측이 없으면 보류. 대안: 제출 직전의 최신 가격 또는 다음 거래 가격을 쓴다 | 대체 가격이 높으면 매수 수익률이 낮아지고 낮으면 높아진다 |
| 제출 시각과 같은 분봉 또는 장중 제출일의 일봉 | 제안: 순서를 확인할 수 없으면 보류. 대안: 해당 봉의 시가 또는 종가를 사용한다 | 실제 앞뒤 순서에 따라 수익률이 어느 쪽으로든 달라진다 |
| 갭 시가와 같은 봉의 양쪽 도달 | 제안: 갭과 모호한 판정은 보류. 대안: 갭 시가 또는 원 경계와 분봉 우선순위를 정한다 | 갭 시가가 목표보다 높거나 손절보다 낮으면 해당 경계보다 이익 또는 손실이 커질 수 있다 |
| 제출일의 벤치마크 종가 결측 | 제안: 순수익률을 유지하고 초과 수익만 보류. 대안: 직전 또는 다음 거래일 종가를 대체한다 | 대체 시각의 지수에 따라 초과 수익이 높거나 낮아진다 |
| 비용과 벤치마크의 확장 | 제안: 검증된 비례 요율과 같은 통화의 종가 근사를 사용한다. 대안: 수량별 최소 비용과 장중 벤치마크를 확정한다 | 최소 비용은 수익률을 낮출 수 있고 장중 지수는 종가 근사보다 높거나 낮을 수 있다. 국내 가격지수의 배당 제외는 별도 표시가 필요하다 |
| 조합별 상한 이후 | 제안: 현재의 사유 있는 미판정을 유지하면서 전체 이력과 비중첩 선택 의미를 보존하는 확장 저장을 설계한다. 대안: 명시적 새 구간으로 전환하고 이전 구간을 따로 보존한다 | 닫힌 실패는 새 판정 표본과 성적표 반영이 멈춘다. 새 구간의 성공률·표본 수는 과거 전체 기준보다 높거나 낮아질 수 있다 |

## 인계

| 받는 작업 | 내용 |
|---|---|
| A-15, S-18 | 제출 시점 관측·비용·완전한 봉과 기업 행사 사실을 저장·복원하고 이 도메인 수익률 경로에 연결한다. 상한 사유와 미판정을 보존한다 |
| E-08, E-91 | 보류 사유와 수치·거래일 정렬의 근사 표시, 상한 이후 전체 이력 또는 명시적 구간의 의미를 확정한다 |
| S-23 | 분할·배당락과 가격 자료의 과거 보존·완전성을 실측한다 |

## 검증

프로젝트 신뢰와 세션 훅 정의 신뢰의 실제 적용은 확인하지 못했다. 시작 시 frozen 설치 및 lefthook 설치는 성공했다. 판정 결합 회귀 테스트는 새 API가 없을 때 컴파일 실패했고 구현 뒤 통과했다. Git 훅과 세션 훅 적용은 구별한다.

## 1차 구현의 손 변형

다음은 최초 구현 커밋 `350375687adfae959acdbe9ef31ef7936b79e39f`의 실행 기록이다. 각 변형은 분리된 임시 복사본에서 하나씩 적용하고 해당 시험을 실행했다. 이후 산술 시험은 공개 API를 닫으면서 내부로 옮겼다. 이 표는 수정 후 변이 검사의 결과를 뜻하지 않는다.

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

## 행사일 보강 전 후보의 손 변형

격리된 `mktemp -d` 복사본에서 각 변형을 하나씩 적용하고 해당 시험을 실제 실행했다. 복사본의 결과 로그에서 20·21번의 설명은 각각 다른 요율 사용으로 기록됐으나, 적용된 식은 해당 요율을 0으로 만드는 변형이다. 아래 표는 실행된 식을 정확히 적었다. 이 후보에서는 32건 모두 컴파일 후 테스트 실패로 검출했고 생존·컴파일 실패는 없었다. 이후 ADR 0051의 행사일 기준과 정규장 밖 관측 처리를 반영했으므로 이 표를 최종 코드의 결과로 취급하지 않는다.

| 번호 | 바꾼 것 | 검출한 시험 |
|---|---|---|
| 1 | 제출 종목 검사를 반전 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 2 | 제출 시각에서 이후 관측을 허용 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 3 | 제출 통화 검사를 반전 | `estimate_return::tests::d54_submission_price_rejects_other_instruments_and_currencies` |
| 4 | 판정의 수정 사실 불일치 조건을 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 5 | 판정 재계산의 결과 비교를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 6 | 성공 청산을 손절로 표기 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 7 | 손절 청산을 성공으로 표기 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 8 | 만기 청산을 성공으로 표기 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 9 | 같은 분봉의 제출 이후 검사 반전 | `estimate_return::tests::d54_return_rejects_earlier_or_partially_observed_exit_and_gap` |
| 10 | 일봉 판정과 제출 날짜의 선후 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 11 | 청산까지 수정 기준을 덮는지 반전 | `estimate_return::tests::d54_adjustment_must_cover_exit_evidence` |
| 12 | 기업 행사와 제출일 일치 검사를 반전 | `estimate_return::tests::d54_split_adjusts_entry_once_and_rejects_mismatched_actions` |
| 13 | 제출가에 과제의 전체 수정 계수 사용 | `estimate_return::tests::d54_split_adjusts_entry_once_and_rejects_mismatched_actions` |
| 14 | 제출가 목표 경계 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 15 | 제출가 손절 경계 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 16 | 갭 가격 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 17 | 비용의 과제 일치 검사를 반전 | `estimate_return::tests::d54_unrelated_cost_and_missing_benchmark_are_separate_errors` |
| 18 | 비용 만기 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 19 | 비용 시장 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 20 | 매수 비용률을 0으로 바꿈 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 21 | 매도 비용률을 0으로 바꿈 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 22 | 벤치마크 실제 종료 시각 비교를 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 23 | 벤치마크의 서로 다른 날짜 조건을 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 24 | 벤치마크 청산 종료 시각 조건을 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 25 | 초과 수익에서 벤치마크를 더함 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 26 | 근사값이 섞여도 정확으로 표기 | `estimate_return::arithmetic_tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 27 | 근사 수익률을 0으로 바꿈 | `estimate_return::arithmetic_tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 28 | 매수 비용을 지출에서 뺌 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 29 | 매도 비용을 수취액에 더함 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 30 | 수익 차감 순서를 반전 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 31 | 수익률 분모를 수취액으로 바꿈 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 32 | 보정 이력 상한에서 등호 제거 | `estimate_return::tests::d54_capacity_reports_full_without_changing_existing_evidence` |

## 행사일·정규장 보강 후 손 변형

최종 후보를 `mktemp -d` 복사본에 반영하고 각 변형을 하나씩 실행했다. 35건 모두 컴파일 후 해당 시험이 실패해 검출됐다. 생존·컴파일 실패·시간 초과는 없었다.

| 번호 | 바꾼 것 | 검출한 시험 |
|---|---|---|
| 1 | 제출 종목 검사를 반전 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 2 | 제출 시각에서 이후 관측을 허용 | `estimate_return::tests::d54_submission_price_preserves_submission_and_rejects_other_times` |
| 3 | 제출 통화 검사를 반전 | `estimate_return::tests::d54_submission_price_rejects_other_instruments_and_currencies` |
| 4 | 판정의 수정 사실 불일치 조건을 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 5 | 판정 재계산의 결과 비교를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 6 | 성공 청산을 손절로 표기 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 7 | 손절 청산을 성공으로 표기 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 8 | 만기 청산을 성공으로 표기 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 9 | 같은 분봉의 제출 이후 검사 반전 | `estimate_return::tests::d54_return_rejects_earlier_or_partially_observed_exit_and_gap` |
| 10 | 일봉 판정과 제출 날짜의 선후 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 11 | 청산까지 수정 기준을 덮는지 반전 | `estimate_return::tests::d54_adjustment_must_cover_exit_evidence` |
| 12 | 제출일 행사를 제출가에 다시 적용 | `estimate_return::tests::d54_split_adjusts_entry_once_and_rejects_mismatched_actions` |
| 13 | 제출가에 과제의 전체 수정 계수 사용 | `estimate_return::tests::d54_split_adjusts_entry_once_and_rejects_mismatched_actions` |
| 14 | 제출가 목표 경계 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 15 | 제출가 손절 경계 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 16 | 갭 가격 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 17 | 비용의 과제 일치 검사를 반전 | `estimate_return::tests::d54_unrelated_cost_and_missing_benchmark_are_separate_errors` |
| 18 | 비용 만기 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 19 | 비용 시장 일치 검사를 반전 | `estimate_return::tests::d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity` |
| 20 | 매수 비용률을 0으로 바꿈 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 21 | 매도 비용률을 0으로 바꿈 | `estimate_return::tests::d54_maturity_uses_submission_price_and_proportional_costs` |
| 22 | 벤치마크 실제 종료 시각 비교를 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 23 | 벤치마크의 서로 다른 날짜 조건을 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 24 | 벤치마크 청산 종료 시각 조건을 반전 | `estimate_return::tests::d54_benchmark_alignment_uses_submission_instead_of_task_reference` |
| 25 | 초과 수익에서 벤치마크를 더함 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 26 | 근사값이 섞여도 정확으로 표기 | `estimate_return::arithmetic_tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 27 | 근사 수익률을 0으로 바꿈 | `estimate_return::arithmetic_tests::d54_nonterminating_division_is_marked_approximate_and_direction_is_retained` |
| 28 | 매수 비용을 지출에서 뺌 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 29 | 매도 비용을 수취액에 더함 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 30 | 수익 차감 순서를 반전 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 31 | 수익률 분모를 수취액으로 바꿈 | `estimate_return::arithmetic_tests::d54_return_examples_preserve_sign_cost_and_excess` |
| 32 | 보정 이력 상한에서 등호 제거 | `estimate_return::tests::d54_capacity_reports_full_without_changing_existing_evidence` |
| 33 | 같은 날 일봉 도달의 개장 전후 조건 반전 | `estimate_return::tests::d54_preopen_submission_precedes_a_same_day_daily_exit` |
| 34 | 정규장 밖 과제를 신호 날짜 규칙으로 계산 | `estimate_return::tests::d54_preopen_submission_uses_next_regular_session` |
| 35 | 벤치마크 기준일을 제출 표기 오프셋으로 계산 | `estimate_return::tests::d54_minute_exit_and_entry_calendar_evidence` |
