# D-25 매수 수량 계획

- 상태: 초안
- 사용자 확인 대상: 아래 비중·표본 부족 축소율·기록 자원 상한.
- 기준: 21deb1514b4c4aaefc15b9b18dc974f851e2cef4.

## 비중 초안

| 위험 성향 | 한 번에 사는 비중 | 같은 모델·기간 판정 30건 미만 |
|---|---|---|
| 보수 | 1% | 0.5% |
| 중립 | 2% | 1% |
| 공격 | 3% | 1.5% |

문서에는 작은 비중만 요구하고 숫자는 없다. 종목 주의 10%·상한 20%보다 작게 정했고 표본 부족에서는 절반으로 줄인다. 설정이나 사용자 절차를 추가하지 않는다. SignalEstimate의 봉인된 operation을 사용하며 호출자가 표본 수나 보수 운용 여부를 넣지 않는다. EligibleBeforeE91는 실제 보정 완료가 아니다.

총자산은 D-05의 Complete AssetEstimate만 받는다. 총자산 × 비중은 경제적으로 근사인 목표다. floor_target 한 함수에서 원래 Decimal의 정수 계수와 10의 거듭제곱으로 비중을 곱한 유리수를 원 단위로 내린다. Decimal의 근사 나눗셈 결과를 금액으로 재포장하지 않는다. 계수 최대 96비트 × 3, 분모 최대 10^28 × 200은 i128에 들어간다. 음수·다른 통화는 판정 불가다.

## 순서와 닫힌 결과

| 순서 | 입력 | 적용 | 근거 |
|---|---|---|---|
| 1 | 총자산·위험 성향 | 목표 원화 금액 내림 | design 6.3 한 번에 사는 비중, ADR 0029 |
| 2 | BuySignal의 PolicyBuyLimit | compare_krw_cost로 최대 정수 수량 탐색 | ADR 0048·0057, 한도 금액 접근 금지 |
| 3 | 해당 통화 BuyingPower | 거래 통화 비용 이하 | design 6.3 미국은 달러만 |
| 4 | CommissionRateFact | 비용+통화 최소 단위로 올린 수수료 이하 | ADR 0050, D-22 비용 경로 |
| 5 | SignalOperation | 표본 부족이면 절반 비중 목표로 추가 축소 | ORD-8·open-issues 1.4 |
| 6 | 최종 비용·수량 | check_buy가 Pass일 때만 계획 | ADR 0057 |

각 탐색은 단조 비용으로 상한을 최대 96회 좁힌 뒤 상한 포함 이분 탐색을 최대 96회 한다. 정수 수량 전체 0..Decimal::MAX를 다룬다. 최대 수량이 0이면 계획을 만들지 않는다. 산술 표현 손실은 판정 불가다. 큰 탐색 후보의 범위 초과는 비용 상한 밖으로 처리한다.

BuyQuantityDecision은 Plan(Box<BuyQuantityPlan>)/None(NoBuyQuantity)이다. into_plan은 계획만 추출한다. 계획 생성자는 비공개이고 양수 WholeOrderQuantity·원래 BuySignal·PriceObservation·OrderFxRates·예상 금액·수수료·결정 제약을 보존한다. 계획·사실 묶음·기록 Debug는 금액을 출력하지 않는다. 오류에는 입력값이 없다.

이유는 InsufficientBuyingPower, NoDollarsExchangeRequired, ZeroPolicyLimit, BelowMinimumUnit, RepeatedSignal, InvalidPriceOrFx, Indeterminate의 닫힌 목록이다. 달러가 0이면 원화 잔액과 관계없이 환전 안내 이유다. 표현 손실·비용 미상·달력 부족·현재 원칙 재검사 실패는 판정 불가다.

## 주문 방식과 후속 경계

ADR 0032대로 수량 지정을 기본으로 하고 국내·미국 매수 모두 정수 주만 계획한다. 소수 수량 매수는 토스 수량 주문에서 허용하지 않는다. 금액 지정 시장가로 자동 전환하지 않는다. 별도 최소 $1을 만들지 않는다. 이 계획은 지정가·OrderIntent·Proposal·승인·주문 포트를 만들지 않는다. D-26이 최신 호가로 가격을 정하고 O-01이 잔고·비용·원칙을 다시 검사한다. 계획의 비용은 관측 가격 기준 Estimated다.

미국 원화 비교에는 D-32 OrderFxRates와 OrderExpectedAmount를 재사용한다. 두 USD/KRW 환율의 종류·기간·상식 범위·괴리 검사를 모두 통과한 뒤 큰 환율로 평가한다. 환산된 경제적 근사 금액은 원화 제약 비교에만 사용하고 달러 수량·비용 계산에 역산하지 않는다.

D-22의 buy_quantity_fee는 새 pub(crate) 자유 함수다. 요율의 시장·시점 검사를 통과해야 하며 기존 정확 곱과 ceil_fee를 재사용한다. 매수 시 별도 세금·최소액 부담의 완전성은 T·O 작업이 공급해야 한다. 현재 계좌 수수료 사실 이상의 부과금은 이 함수로 인증하지 못한다.

D-26·O-01은 calculate_buy_quantity, BuyQuantityFacts, BuyQuantityPlan의 읽기 접근자를 소비한다. D-36 병합 뒤 BuyingPower 입력을 실패를 표현하는 현금 관측으로 바꾼다. 현금·시세 조회의 진위와 최신 잔고 재확인은 어댑터·O-01 책임이다.

## 반복과 쏠림 연결

BuySignalRecord::from_signal은 D-24의 실제 BuySignal, 발생 시각, 거래소 캘린더만 받는다. D-27 signal_task_interval을 공유하므로 첫 과제 생성부터 그 과제의 거래일 만기까지 [시작, 만기)다. 승인·거래 여부를 입력받지 않으므로 첫 신호가 주문으로 이어지지 않아도 제한한다. 같은 종목은 시장+심볼이다. 호출 시각에 아직 기간 안인 동일 종목 기록이 하나라도 있으면 반복 제한이다. 미래 기록과 65,536건 초과는 판정 불가다. 입력 순서가 결과를 바꾸지 않는다. 저장·원자적 중복 방지는 S·E 작업이다. 기록을 빠뜨린 목록의 완전성을 domain이 인증하지 않는다.

concentration_for_policy는 D-07 Warning/Clear/NotApplicable/Unavailable을 D-12 네 상태에 일대일 연결한다. 쏠림은 경고이며 매수를 차단하는 새 원칙으로 바꾸지 않는다.

## 미해결

비중 표와 절반 축소율은 사용자 확인 대상이다. 현금 관측의 최대 허용 나이, 실제 최소 주문 금액, 수수료 외 매수 부담금, 시장가 금액 주문의 명시적 제품 경로는 후속 작업과 실측으로 정한다. 시세는 기존 D-14의 5초 신선도와 종목·통화 일치를 검사한다. 기록 상한 65,536은 업무 상한이 아닌 자원 초안이다. 프로젝트·훅 신뢰의 런타임 적용은 입증하지 못했으며 직접 검사·Git 훅 결과로 검증한다. 새 의존성·검사 끄기는 없다.

## 테스트 대응

- 목표 금액 내림: target_is_always_floored_against_independent_rational (4,096회).
- 이분 탐색 최대 정수 수량: bounded_search_returns_largest_whole_quantity (4,096회).
- 공개 경로 최대 수량·0과 주문 없음·최종 원칙 통과: plans_are_maximal_and_final_policy_passes_independent_integer_reference (4,096회).
- 모든 제약의 동시 최대값과 최종 원칙: all_constraints_match_rational_maximum_and_one_more_share_fails (4,096회).
- 원화 잔고와 별개인 미국 달러 상한: us_cash_cap_is_independent_of_krw_when_target_is_nonbinding (4,096회). 총자산 자체는 원화 현금을 포함하므로 목표 비중 금액까지 원화와 무관하다는 주장은 하지 않는다.
- 기록 순서·시장과 심볼·만기·미래 기록: history_identity_expiry_future_and_arbitrary_order_are_enforced (순서 4,096회).
- 원칙 여유·표본 부족 0주: policy_limit_and_zero_limit_are_binding_and_conservative_zero_is_no_order.
- 수수료·달러 부족: commission_cash_and_policy_limit_reduce_quantity, cash_and_fee_can_each_prevent_the_minimum_whole_share, no_dollars_requires_exchange_even_with_large_krw_balance.
- 사실 부적합·정밀도·현금 관측: invalid_price_fx_calendar_and_fee_are_closed_failures, future_cash_incomplete_assets_decimal_prices_and_fx_overflow_fail_closed, fee_validity_precision_and_wrong_currency_observations_are_rejected.
- 쏠림 네 상태: all_four_concentration_states_are_preserved.

테스트 우선 절차는 산술 두 속성 테스트에서 없는 함수의 E0432 실패를 확인했다. 나머지 통합·경계 테스트는 구현 뒤 보강했으므로 전체 완료 기준을 모두 먼저 실패시킨 절차는 충족하지 못했다. 실행 검사 결과와 변형 검출 결과는 최종 보고에 남긴다.

## 병합과 검증 범위 보완

H-43이 작업 중 로컬 master에 들어와 792a35b5da51e9a8fff146793a172d1c2e210fc1을 fast-forward로 반영했다. 자동 변이는 이 기준으로 한 번 실행했으나 테스트 전용 파일명과 조건부 lib 등록이 crate 전체 지원 입력으로 판정되어 91파일·2,830개로 확장됐다. 20분에 중단했고 당시 처리 184개는 검출 127·컴파일 불가 57·생존 0·개별 시간 초과 0이었다. D-25 새 파일 완료 건수는 0이므로 자동 변이 통과가 아니다. 테스트를 buy_quantity.rs의 표준 내부 테스트 모듈로 옮기고 lib에는 조건 없는 새 모듈 등록과 재수출만 남겼다. 자동 실행은 지시한 한 번을 유지하고 손 변형으로 새 분기를 보완한다.

탐색 정수는 Decimal의 96비트 계수 범위 안이다. 양 끝 합에 1을 더해도 2^97보다 작고, 상한을 두 배 해도 2^97보다 작아 i128을 넘지 않는다. 이분 탐색의 low < high에서 올림 중점은 1 이상이라 중점−1도 음수가 아니다. 목표의 계수×비중 최대 3도 i128보다 작다. 이 증명 안의 wrapping 정수 연산에는 실제 wrap이 없으며 Decimal에는 wrapping 연산을 쓰지 않는다. Quantity 최댓값은 quantity.rs의 새 내부 자유 함수가 검증된 상수만 만든다.

S·E는 현재 신호를 제외한 과거 기록을 조회하고 중복 판정·현재 신호 기록을 원자적으로 처리해야 한다. 계획이 없거나 사용자가 승인하지 않아도 첫 D-24 매수 신호를 저장한다. 누락된 기록 목록의 완전성을 domain이 보증하지 않는다.

최종 손 변형은 새 분기 21개가 모두 컴파일 후 nextest 종료 100으로 검출됐다. 반복 제한 호출 제거의 최초 형태는 dead_code로 컴파일되지 않아 제외했고, 호출을 유지하면서 결과를 무시하는 동등 변형으로 다시 검출했다. 가격·미래 현금·달러 없음·원칙 한도·0수량·수량 축소·보수 운용·최종 check_buy·환산 넘침·등호·기록 상한/미래/시장/심볼/만기·쏠림 네 상태를 포함한다. 모두 원본 복구했다. 커버리지 검사는 586개 테스트 통과·영역 위반 0건이다. 손 변형 자료는 d25-manual-otjs4szj/final-results.json과 개별 로그로 남겼다. 전체 just check 최종 결과와 SHA는 구현 보고에 기록한다.
