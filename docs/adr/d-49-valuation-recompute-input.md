# D-49 과거 평가 재계산 입력
- 상태: 초안

## 근거

I-09, design 4.3, ADR 0052·0112·0118·0121. 구현은 2026-11-17 사용자 결정에 따라 Codex `gpt-6.1-terra`(max)가 수행한다. 구현 모델 허용 규칙의 문서 갱신은 H-50에 인계한다. 적대적 리뷰와 독립 비판의 모델은 변경하지 않는다.

## 결정

- 평가 기준 시각은 평가일 16:00 KST 하나로 만든다. 이 시각은 같은 날짜 07:00 UTC이며 출력은 UTC 오프셋으로 표현한다. KST 투영 시 날짜와 16:00이 보존된다. 입력의 평가 시각이 이 절대 시각과 다르면 거부한다. 기록 시각과 환율은 기존 SnapshotObservation을 사용한다.
- 기존 결제 행·SnapshotInstrumentHistory·SnapshotLedgers와 종목별 ValuationPriceEvidence를 함께 전달한다. ValuationHistoryEvidence는 이력과 증거를 묶으며 중복된 종목·가격 도메인 타입을 새로 정의하지 않는다. 기존 로트 재구성 및 스냅샷 계산 규칙을 재사용한다. 원장의 확정 기간은 평가일을 끝으로 해야 하며 양 통화 모두 기존 대조 및 행 일치 검사를 통과해야 한다.
- 양수 보유 수량은 해당 종목의 증거가 있어야 한다. 기존 정확 곱셈이 정밀도 손실을 오류로 거부하므로 성공한 평가가 0인 경우에만 0주로 판정하며, 매우 작은 양수의 곱이 0으로 잘리지 않는 회귀를 둔다. 0주는 대기 기업 행사가 없다면 증거 없이 0으로 평가할 수 있다. 증거가 있으면 그 종목의 실제 일봉 종가, 통화, 가격일과 시장 상태를 대조한다. 중복 증거와 알 수 없는 종목은 거부한다. 가격 증거는 제공자의 사실 주장이지 출처의 진실성을 인증하지 않는다.
- 외화 주식 평가액과 음수 현금이 상쇄되더라도 주식 환산과 계좌 환산을 각각 확인해 환율 누락을 거부한다.
- 휴장 이월 및 미완료 장에서는 기존 ADR 0052·0118의 직전 확정 종가를 사용한다. 미완료 장의 현재 값을 확정 종가로 취급하지 않는다. 누락·불완전 계산을 0으로 치환하지 않고 SnapshotError로 돌려 ValuationDayResult에 담을 수 있게 한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| A-26 | 과거 보유·행사·대조 원장·거래·가격 증거·환율을 공급하고 날짜별 성공·실패를 기록한다. 자료 출처와 실제 이력 완전성을 확인한다. |
| S-18 | 날짜별 결과 및 기존 성공 스냅샷 정본의 저장·조회와 재시도·원자성 정책을 구현한다. |
| J-11 | 시장별 달력 매핑과 16:00 예약 및 시작 시 복구를 연결한다. |
| H-50 | 구현·수정 모델 허용 규칙을 갱신한다. |

## 검증

프로젝트 신뢰와 세션 훅 신뢰·적용은 이 세션에서 확인하지 못했다. 홈 설정과 자격 증명을 읽거나 수정하지 않았다. Git lefthook은 설치했다.

정상 커밋 훅 재시도 중 기존 생성물 테스트의 자식 상태가 null로 실패한 뒤 해당 테스트 단독 실행은 통과했다. 다른 실행에서는 기존 commit-guard 테스트가 기대한 오류 문구를 받지 못해 전체 테스트가 중단됐고, 해당 테스트는 현재 작업 사본과 별도 작업 사본에서 각각 단독 통과했다. 두 실패의 당시 종료 신호·실제 오류는 기록되지 않아 원인은 미확정이다. 이 실패 실행은 전체 검사 통과로 세지 않는다.

## 손 변형

임시 복사본에서 29개를 실행해 컴파일된 26개를 모두 검출했고 3개는 컴파일 불가로 제외했다. 생존은 없었다. 원본 대조군과 원복 뒤 대조군을 실행했고, 복사본의 변경 소스는 원본과 일치했다. 아래는 각 실제 실행의 첫 실패 테스트다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 생존 |
|---|---|---|
| 1 | 기준 시각을 15시 KST로 | evidence_must_match_the_actual_candle_currency_date_and_kind |
| 2 | 기준 시각을 17시 KST로 | unreconciled_cash_cannot_form_an_input |
| 3 | 평가일을 한국 대신 미국 달력에서 선택 | us_market_date_can_differ_from_kst_valuation_date |
| 4 | 기준 시각 일치 비교 반전 | recalculated_us_stock_and_cash_match_d31_expected_values |
| 5 | 원장 평가일 검사 제거 | 컴파일 불가 (검출 건수 제외) |
| 6 | KR 원장 끝 날짜 비교 반전 | tiny_nonzero_quantity_and_price_cannot_turn_precision_loss_into_zero |
| 7 | USD 원장 끝 날짜 비교 반전 | corporate_action_is_applied_only_from_its_date |
| 8 | 원장 날짜 불일치 OR를 AND로 | each_ledger_date_and_observation_time_is_checked_at_construction |
| 9 | 생성자 검증 제거 | unreconciled_cash_cannot_form_an_input |
| 10 | 중복 종목 증거 검사 반전 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 11 | 가격 증거 통화를 원화로 고정 | recalculated_us_stock_and_cash_match_d31_expected_values |
| 12 | 가격 증거 통화를 달러로 고정 | first_evaluation_after_consecutive_holidays_is_not_skipped |
| 13 | 증거 판정 시각을 기록 시각으로 | incomplete_market_uses_previous_final_close_not_today_intraday |
| 14 | 봉의 종목 비교 반전 | first_evaluation_after_consecutive_holidays_is_not_skipped |
| 15 | 증거와 실제 종가 비교 반전 | incomplete_market_uses_previous_final_close_not_today_intraday |
| 16 | 일봉 기간 비교 반전 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 17 | 확정 상태 비교 반전 | incomplete_market_uses_previous_final_close_not_today_intraday |
| 18 | 원주가 조정 여부 비교 반전 | corporate_action_is_applied_only_from_its_date |
| 19 | 봉 부재를 통과시킴 | zero_position_cannot_attach_evidence_without_a_candle |
| 20 | 0주 판정 반전 | evidence_must_match_the_actual_candle_currency_date_and_kind |
| 21 | 보유 종목 증거 존재 판정 반전 | us_market_date_can_differ_from_kst_valuation_date |
| 22 | 누락 검사의 AND를 OR로 | recalculated_us_stock_and_cash_match_d31_expected_values |
| 23 | 환산 완전성 검사 제거 | unreconciled_cash_cannot_form_an_input |
| 24 | 대조 원장 입력 제거 | 컴파일 불가 (검출 건수 제외) |
| 25 | 종목 이력 제거 | 컴파일 불가 (검출 건수 제외) |
| 26 | 알 수 없는 증거 종목 오류 바꿈 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 27 | 가격 증거 누락 오류 바꿈 | failed_recompute_is_a_day_result_with_its_reason |
| 28 | 기준 시각 불일치 오류 바꿈 | each_ledger_date_and_observation_time_is_checked_at_construction |
| 29 | 주식 환산 완전성 검사 제거 | offsetting_usd_cash_does_not_hide_missing_stock_fx |
