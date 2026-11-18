# D-51 신호 평가의 도메인 계약
- 상태: 초안
- 관련 작업: D-51, A-13, A-27, A-15, S-07, S-18, E-06, E-91

## 결정

모델과 기간은 함께 `CalibrationKey`다. 다른 키의 `for_combination`과 `calibrate`, 다른 키의 제출·원장 위치·스냅숏 조합은 `MismatchedCombination`이다. 정상 빈 표는 조합이 일치하는 완전한 스냅숏이며 판정 0건의 보수 운용이다. E-91 전 보정 확률은 종전처럼 원래 AI 확률이다.

보정표는 `CalibrationSnapshot`으로 `CalibrationLedgerHead`를 함께 보존한다. 위치는 조합별 판정 원장의 불투명한 단조 증가 버전이다. 위치 0도 정상적인 빈 원장을 나타낼 수 있다. 저장 계층이 조합별 이력 전체와 일관된 위치를 읽고 판정 사실을 누락 없이 반영했음을 보증한다. domain은 시계를 읽거나 과거 발생 시각을 표 생성 시각으로 추정하지 않는다. 스냅숏이 없으면 `MissingCalibration`, 요청한 최신 위치와 반영 위치가 다르면 방향과 무관하게 `StaleCalibration`이다. 미래 위치도 불일치로 거부한다. 요청한 최신 위치 자체가 없으면 평가가 불가능하다. 준비 후 이력이 늘어나는 경우를 막기 위해 `SignalDecision`에서도 현재 원장 위치를 비교한다. 원장 사실의 진위와 판정부터 저장까지의 원자성은 domain이 증명하지 못한다.

표 또는 최신 위치가 없거나 오래되면 매도도 판정하지 않는다. E-91 전 확률은 원 확률이지만 보수 운용에 쓸 표본 구간도 보정 정보에 의존하므로, 정보가 없는 매도를 별도 예외로 허용하지 않는 결정이다. 그 결과 저장소가 일시적으로 갱신되거나 조회되지 않으면 유효한 매도 제안도 중단될 수 있다. 최초 빈 표의 저장과 일관된 원장 조회가 필요하다. 평가 단계에서 최신 위치가 없을 때의 `MissingCalibration`은 준비 단계의 표 부재와 다른 타입·단계의 사유이며, 조회 실패와 위치 미생성은 그 자체로 구분하지 않는다.

매수 조건이 충족된 뒤 독립적인 대상 가격을 확인한다. 관측이 없으면 `MissingTargetPrice`, 종목·내부 종목 ID·통화가 다르면 `InvalidTargetPrice`, 기준 관측 이전이거나 미래 또는 5초 이상 지난 관측이면 `StaleTargetPrice`다. 경계는 D-14의 `PriceObservation` 계약과 같다. 관측 유형은 실제 호가 관측 타입이며 수정 일봉을 대신 받지 않는다. 가격은 보유 평가에 포함된 가격과 독립적이다. 매도 및 매수 조건을 충족하지 않는 판정은 대상 가격이 없어도 기존 사유를 유지한다.

내부 ID가 없는 시세 관측은 대상 종목을 확인할 수 없으므로 매수 판정에서 `InvalidTargetPrice`로 거부한다. A-13은 종목 마스터의 공개 `price_observation`으로 내부 ID를 포함해 전달한다.

전이는 같은 내부 종목 ID와 시장·심볼을 확인한다. 같은 종목에서 방향이 같으면 하나의 발생이고, 제출 시각·과제·확률·근거·한도·보유 사실이 달라져도 내용 갱신이다. 값도 같으면 유지다. 다른 종목을 한 전이에 섞으면 오류다. 신호 없음 판정에는 종목 키가 없으므로 호출자가 같은 종목 평가라는 사실을 보장해야 한다. 발생은 `SignalRaised`, 해제는 `SignalCleared`, 방향 변경은 해제 다음 발생 순서이며, 유지와 내용 갱신은 이벤트가 없다. 이벤트의 발행은 원자 저장 성공 뒤 A-13의 책임이다.

시세 부재·보정 정보 불일치 같은 일시적 판정 불가도 현재 신호를 해제한다. 오래된 신호를 주문 가능한 상태로 남기지 않기 위한 보수적 선택이다. 이후 입력이 회복되어 새 발생이 생기면 이전 매수 발생의 H 구간이 끝나기 전에는 반복 제한에 걸릴 수 있다. 일시 중단 중 발생을 이어 가는 별도 정책은 현재 전이·저장·주문 계약에 없으며, 이를 도입한다면 기존 발생 ID와 H를 보존하는 동시에 주문을 중지하고 회복 시 입력을 다시 검증해야 한다.

매수 발생은 별도 `BuySignalOccurrenceId`와 종목, 실제 발생 시각 및 그 시각부터 계산한 H 구간을 가진다. `CurrentBuySignal`은 현재 신호와 해당 발생 기록을 묶어 수량 계산에 넘긴다. 현재 ID의 동일한 기록은 제외하고, 같은 ID의 내용이 다르면 판정 불가다. 다른 ID의 이전 기록은 해제 뒤에도 남아 H 안의 재발생을 제한한다. 같은 방향 내용 갱신은 기존 발생 기록을 보존한다. ID의 고유성과 현재 신호에 실제 연결된 발생 기록은 저장소의 책임이며, domain 생성자는 저장소가 제공한 사실의 진위를 인증하지 않는다.

반복 제한에서 같은 내부 종목 ID 또는 같은 시장·심볼이면 같은 종목으로 본다. 코드 변경과 분리된 내부 ID가 유지되는 심볼 변경도 제한하기 위해 ADR 0060의 시장·심볼 비교를 보수적으로 보강한다. 내부 ID가 바뀌어도 같은 시장·심볼은 기존처럼 제한한다.

## 인계

| 받는 작업 | 계약과 남은 검증 |
|---|---|
| A-27, S-07, S-18 | 종목별 현재 신호와 발생 ID·원본 발생 시각·H 구간의 원자 저장 및 해제 후 이력 보존. 동일 ID 충돌 거부와 현재 발생의 정확한 연결. 제출 버전·현재 상태·보정 원장 위치를 저장 시 같은 직렬화 경계에서 검사하고 경쟁하는 제출·평가·폐기를 검증 |
| A-15, A-27, S-18 | 모델·기간별 원장의 단조 버전과 완전한 집계 및 최초 정상 빈 표를 실제 저장. 표와 반영 위치를 원자적으로 갱신하고 최신 원장 위치와 함께 조회 |
| A-13 | 최신 제출·출처 원장·보유·대상 시세·보정 스냅숏·원장 위치를 읽고 원자 저장 뒤 전이 이벤트를 순서대로 발행. 도메인 오류와 판정 불가 사유를 소비자에게 보존. 저장 실패 시 이벤트를 내지 않음 |
| A-13, A-14, E-06, U-09, U-14 | 같은 방향 내용 갱신을 새 발생 알림 없이 화면이 재조회하는 경로, 입력 변경·만기·재시작 재평가와 제안 만료·가격 범위 폐기의 차이, 경쟁과 이벤트 유실 복구를 정의·검증 |
| S-05, S-09, M-02, AI-14, E-02 | 제출에 연결된 출처 원장의 완전한 조회와 사실 출처를 보장 |
| E-91 | 별도 승인된 보정 방법을 도입하고 기존 원래 확률 반환을 대체 |

## 검증과 한계

현재 문서는 초안이며 로컬 순수 함수의 계약만 다룬다. 저장·직렬화·포트·실계좌·네트워크·화면 갱신은 이 작업의 검증 범위가 아니다. 프로젝트 신뢰와 현재 세션 훅 적용은 확인하지 못했다. Git 훅과 개별 검사 결과는 리뷰 기록과 최종 보고에 기록한다.

## 손 변형

| 번호 | 변형 | 실행 결과 |
|---|---|---|
| 1 | 조합 키 불일치 반전 | 검출: calibration::tests::d51_matching_empty_table_is_conservative_and_mismatch_is_error |
| 2 | 표와 반영 위치의 키 검사 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 3 | 제출과 원장 키 검사 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 4 | 표와 최신 원장 키 검사 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 5 | 보정표 반영 위치 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 6 | 표 부재를 오래됨으로 변경 | 검출: signal_decision::tests::d51_calibration_snapshot_distinguishes_missing_empty_stale_and_wrong_key |
| 7 | 판정 시 원장 키 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 8 | 판정 시 최신 위치 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 9 | 가격 부재 사유 변경 | 검출: signal_decision::tests::d51_buy_target_price_missing_stale_and_boundary |
| 10 | 대상 가격 종목 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 11 | 대상 가격 통화 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 12 | 대상 가격 내부 ID 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 13 | 대상 가격 기준 시각 비교 반전 | 검출: signal_decision::tests::d51_buy_target_price_missing_stale_and_boundary |
| 14 | 대상 가격 신선도 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 15 | 발생 이벤트를 해제 이벤트로 변경 | 검출: signal_decision::tests::d51_transition_table_emits_expected_events |
| 16 | 방향 변경 이벤트 순서 반전 | 검출: signal_decision::tests::d51_transition_table_emits_expected_events |
| 17 | 내용 동일성 비교 반전 | 검출: signal_decision::tests::d51_transition_table_emits_expected_events, signal_decision::tests::d51_transition_updates_content_without_a_new_event |
| 18 | 전이 종목 ID 비교 반전 | 검출: signal_decision::tests::d51_transition_property_preserves_same_direction_occurrence, signal_decision::tests::d51_transition_table_emits_expected_events, signal_decision::tests::d51_transition_updates_content_without_a_new_event |
| 19 | 현재 발생 종목 비교 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_existing_occurrence_rejects_an_expired_signal, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates |
| 20 | 자기 발생 제외 조건 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates, buy_quantity::tests::d51_update_preserves_occurrence_and_horizon |
| 21 | 발생 ID 충돌 검사 반전 | 검출: buy_quantity::tests::d51_current_occurrence_is_excluded_but_prior_occurrence_blocks, buy_quantity::tests::d51_history_exclusion_is_independent_of_order_and_duplicates, buy_quantity::tests::d51_history_rejects_conflicting_identity_and_expires_at_horizon_end |
| 22 | 반복 제한 종료 등호 추가 | 검출: buy_quantity::tests::d51_history_rejects_conflicting_identity_and_expires_at_horizon_end |
| 23 | 현재 발생의 미래 시각 비교 반전 | 검출: buy_quantity::tests::d51_history_rejects_conflicting_identity_and_expires_at_horizon_end, buy_quantity::tests::d51_update_preserves_occurrence_and_horizon |
| 24 | 내용 갱신에서 새 신호 누락 | 컴파일 불가 |
| 25 | 내용 갱신에서 새 신호 폐기 | 검출: buy_quantity::tests::d51_update_preserves_occurrence_and_horizon |
| 26 | 제출과 원장 키 비교 삭제 | 검출: signal_decision::tests::d51_submission_rejects_a_consistent_snapshot_for_another_model |
| 27 | 전이의 내부 종목 ID 비교 삭제 | 검출: signal_decision::tests::d51_transition_rejects_same_symbol_with_a_different_instrument_id |
| 28 | 현재 발생의 내부 종목 ID 비교 삭제 | 검출: signal_decision::tests::d51_current_occurrence_rejects_same_symbol_with_a_different_instrument_id |
| 29 | 대상 시세의 ID 부재를 허용 | 검출: signal_decision::tests::d51_buy_target_price_rejects_a_quote_without_an_instrument_id |
| 30 | 이력의 내부 종목 ID 비교 삭제 | 검출: buy_quantity::tests::d51_history_blocks_the_same_instrument_after_a_symbol_change |
| 31 | 이력의 시장·심볼 비교 삭제 | 검출: signal_decision::tests::d51_history_blocks_same_symbol_with_a_different_instrument_id |

초기 코드의 복사본에서 컴파일된 24개를 각각 실제 실행해 모두 검출했다. 24번은 컴파일되지 않아 개수에 넣지 않았다. 복원 후 D-51 테스트 17개가 다시 통과했다. 1차 리뷰 뒤에는 각 비교의 삭제 변이 26~28을 별도 복사본에서 하나씩 실행해 추가된 회귀 테스트가 각각 검출했다. 2차 리뷰 수정 뒤 변이 29~31도 각각 검출했고 복원 후 관련 테스트 4개가 다시 통과했다.
