# D-53: 매수 수량 계산 가격의 종목 일치

- 상태: 초안
- 관련 작업: D-53, D-25, D-51, D-52

## 결정

신호 평가와 매수 수량 계산은 `PriceObservation::check_task_instrument`를 함께 호출한다. 관측의 시장·심볼이 과제 기준 관측과 같고 내부 ID가 과제 변동성의 ID와 같을 때만 통과한다. ID가 없으면 일치로 볼 수 없다. 수량 계산은 불일치에 `NoBuyQuantity::InvalidPriceInstrument`를 반환한다. 통화·시각·환율의 기존 검사와 그 사유는 유지한다. 발생 이력과 과제 시간 검사는 이전 순서를 유지하므로 그 단계에서 이미 실패한 경우에는 그 사유가 먼저 나온다.

이 함수는 crate 내부이며 D-51의 공개 서명은 바뀌지 않는다. 시장·심볼과 내부 ID가 모두 일치해야 한다는 D-51 판정을 그대로 공유해 수량 계산과 신호 평가가 서로 다른 종목을 가격으로 채택하지 않게 한다. 두 단계에서 쓰는 가격 자체가 같거나 같은 시점이라는 뜻은 아니다.

## 인계

| 받는 작업 | 계약과 남은 검증 |
|---|---|
| A-13, A-14 | 종목 마스터에서 내부 ID가 있는 관측을 수량 계산에 전달하고 `InvalidPriceInstrument`를 보존한다. 내부 ID 발급·영속 대응의 실제 사실성은 D-52 계약과 호출 경계에서 보장한다. |

## 검증과 한계

로컬 순수 함수만 검증하며 실제 계좌와 API는 사용하지 않는다. 프로젝트 신뢰와 이 세션의 훅 신뢰·적용 여부는 확인할 수 없다. Git 훅과 실행한 검사는 리뷰 기록에 별도로 남긴다.

## 손 변형

| 번호 | 변형 | 실행 결과 |
|---|---|---|
| 1 | 종목 비교 반전 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 2 | 내부 ID 비교 반전 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 3 | 종목과 ID를 OR로 결합 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 4 | 종목 비교 제거 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected |
| 5 | ID 비교 제거 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 6 | 종목 시장만 비교 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected |
| 7 | 종목 심볼만 비교 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected |
| 8 | 심볼 비교 반전 | 검출: buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 9 | 시장 비교 반전 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 10 | ID 없음 허용 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected |
| 11 | ID가 있기만 하면 허용 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 12 | ID 없음만 허용 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 13 | 판정 성공과 실패 교환 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 14 | 실패 분기를 성공으로 변경 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 15 | 성공 분기를 실패로 변경 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 16 | 수량 계산 판정 반전 | 검출: buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 17 | 수량 계산 판정 생략 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 18 | 수량 계산 사유를 가격·환율 오류로 변경 | 검출: buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 19 | 수량 계산 사유를 판정 불가로 변경 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_us_price_requires_the_same_internal_id, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 20 | 한국 시장의 수량 판정 생략 | 검출: buy_quantity::tests::d53_rejects_same_symbol_price_with_different_internal_id, buy_quantity::tests::d53_missing_id_and_changed_market_or_symbol_are_rejected, buy_quantity::tests::d53_matching_prices_preserve_sizing_and_nonmatching_ids_never_plan |
| 21 | 미국 시장의 수량 판정 생략 | 검출: buy_quantity::tests::d53_us_price_requires_the_same_internal_id |
| 22 | 신호 가격 판정 반전 | 검출: signal_decision::tests::d51_buy_target_price_rejects_a_quote_without_an_instrument_id, signal_decision::tests::d51_buy_target_price_missing_stale_and_boundary, signal_decision::tests::d51_buy_target_price_rejects_wrong_currency_instrument_and_id |
| 23 | 신호 가격 종목 판정 생략 | 검출: signal_decision::tests::d51_buy_target_price_rejects_a_quote_without_an_instrument_id, signal_decision::tests::d51_buy_target_price_rejects_wrong_currency_instrument_and_id |
| 24 | 신호 가격 불일치를 오래됨으로 변경 | 검출: signal_decision::tests::d51_buy_target_price_rejects_a_quote_without_an_instrument_id, signal_decision::tests::d51_buy_target_price_rejects_wrong_currency_instrument_and_id |
