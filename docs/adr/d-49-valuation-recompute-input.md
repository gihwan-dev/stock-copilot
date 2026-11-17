# D-49 과거 평가 재계산 입력
- 상태: 초안

## 근거

I-09, design 4.3, ADR 0052·0112·0118·0121. 구현은 2026-11-17 사용자 결정에 따라 Codex `gpt-6.1-terra`(max)가 수행한다. 구현 모델 허용 규칙의 H-50 갱신은 master의 732b58d1bab83bef0aa9a7affa51277316fbd651에 반영됐다. 적대적 리뷰와 독립 비판의 모델은 변경하지 않는다.

## 결정

- 평가 기준 시각은 평가일 16:00 KST 하나로 만든다. 이 시각은 같은 날짜 07:00 UTC이며 출력은 UTC 오프셋으로 표현한다. KST 투영 시 날짜와 16:00이 보존된다. 입력의 평가 시각이 이 절대 시각과 다르면 거부한다. 기록 시각과 환율은 기존 SnapshotObservation을 사용한다.
- 기존 결제 행·SnapshotInstrumentHistory·SnapshotLedgers와 종목별 ValuationPriceEvidence를 함께 전달한다. ValuationHistoryEvidence는 이력과 증거를 묶고 DatedSnapshotLedgers는 현금 적용일과 기존 원장을 묶는다. 새 종목·가격·원장 도메인 타입은 만들지 않고 기존 로트 재구성 및 스냅샷 계산 규칙을 재사용한다. 현금 적용일은 평가일과 같아야 하며, 양 통화의 대조 원장과 일치하는 확정 기간이 그 날짜를 포함해야 한다. 기간의 끝은 현금 적용일이나 원장 관측일과 같다는 뜻이 아니다. 기존 balance_at으로 평가 시각 이후의 현금 효과를 되돌리므로 더 긴 대조 기간도 이용할 수 있고, 서로 다른 날짜의 스냅샷이 성과 계산에 필요한 공통 기간 토큰을 보존할 수 있다.
- 양수 보유 수량은 해당 종목의 증거가 있어야 한다. 기존 정확 곱셈이 정밀도 손실을 오류로 거부하므로 성공한 평가가 0인 경우에만 0주로 판정하며, 매우 작은 양수의 곱이 0으로 잘리지 않는 회귀를 둔다. 0주는 대기 기업 행사가 없다면 증거와 봉이 없어도 0으로 평가할 수 있다. 봉이 결과에 남는 경우 증거 유무와 관계없이 종목·통화·일봉·확정 상태·원주가를 검사한다. 증거가 있으면 실제 종가, 가격일과 시장 상태도 대조한다. 중복 증거와 알 수 없는 종목은 거부한다. 가격 증거는 제공자의 사실 주장이지 출처의 진실성을 인증하지 않는다.
- 외화 주식 평가액과 음수 현금이 상쇄되더라도 주식 환산과 계좌 환산을 각각 확인해 환율 누락을 거부한다.
- 휴장 이월 및 미완료 장에서는 기존 ADR 0052·0118의 직전 확정 종가를 사용한다. 미완료 장의 현재 값을 확정 종가로 취급하지 않는다. 누락·불완전 계산을 0으로 치환하지 않고 SnapshotError로 돌려 ValuationDayResult에 담을 수 있게 한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| A-26 | 과거 보유·행사·대조 원장·거래·가격 증거·환율과 현금 적용일을 공급하고, ValuationRecomputeInput::new와 ValuationSnapshot::recompute의 검증 경로로 날짜별 성공·실패를 기록한다. 자료 출처와 실제 이력 완전성을 확인한다. 기존 recalculate 직접 호출은 D-49의 가격 증거 검증 경로가 아니다. |
| D-31 / D-48 / T-08 | 한국 16시 평가의 종가 기준을 확정한다. 정규 09:00–15:30, 애프터 15:30–20:00인 합성 달력에서는 기존 D-31이 마지막 세션 종료를 기다려 전일 종가를 선택하고 D-48 증거도 이를 따른다. 반면 R-11은 16시 한국의 당일 종가를 사용하고 ADR 0112는 한국이 마감됐다고 설명한다. 일봉의 정규장·애프터 포함 범위와 실제 토스 달력은 확인되지 않았다. 기존 계약을 임의로 바꾸지 않으며, 충돌 해결 전 D-49의 해당 조건은 완료로 판정하지 않는다. |
| S-18 | 날짜별 결과 및 기존 성공 스냅샷 정본의 저장·조회와 재시도·원자성 정책을 구현한다. |
| J-11 | 시장별 달력의 날짜 매핑·출처·최신성을 검증하고 16:00 예약 및 시작 시 복구를 연결한다. |
| H-50 | 구현·수정 모델 허용 규칙 갱신이 master에 반영됐다. |

## 검증

프로젝트 신뢰와 세션 훅 신뢰·적용은 이 세션에서 확인하지 못했다. 홈 설정과 자격 증명을 읽거나 수정하지 않았다. Git lefthook은 설치했다.

정상 커밋 훅 재시도 중 기존 생성물 테스트의 자식 상태가 null로 실패한 뒤 해당 테스트 단독 실행은 통과했다. 다른 실행에서는 기존 commit-guard 테스트가 기대한 오류 문구를 받지 못해 전체 테스트가 중단됐고, 해당 테스트는 현재 작업 사본과 별도 작업 사본에서 각각 단독 통과했다. 두 실패의 당시 종료 신호·실제 오류는 기록되지 않아 원인은 미확정이다. 이 실패 실행은 전체 검사 통과로 세지 않는다.

## 손 변형

임시 복사본에서 36개를 실행해 컴파일된 33개를 모두 검출했고 3개는 컴파일 불가로 제외했다. 생존은 없었다. 원본 대조군과 원복 뒤 대조군을 실행했고, 복사본의 변경 소스는 원본과 일치했다. 아래는 각 실제 실행의 첫 실패 테스트다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 생존 |
|---|---|---|
| 1 | 기준 시각을 15시 KST로 | daily_recomputations_share_the_period_needed_for_performance |
| 2 | 기준 시각을 17시 KST로 | each_ledger_period_and_observation_time_is_checked_at_construction |
| 3 | 평가일을 한국 대신 미국 달력에서 선택 | us_market_date_can_differ_from_kst_valuation_date |
| 4 | 기준 시각 일치 비교 반전 | equivalent_instant_with_previous_local_date_is_canonicalized |
| 5 | 현금 적용일 검사를 항상 통과 | mismatched_ledger_date_is_rejected_when_created |
| 6 | 현금 적용일 비교 반전 | even_a_zero_position_cannot_label_an_incomplete_candle_as_a_close |
| 7 | 기준 시각과 현금 적용일 검사의 OR를 AND로 | each_ledger_period_and_observation_time_is_checked_at_construction |
| 8 | 생성자 검증 제거 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 9 | 중복 종목 증거 검사 반전 | corporate_action_is_applied_only_from_its_date |
| 10 | 가격 증거 통화를 원화로 고정 | a_later_reconciled_period_rewinds_cash_to_the_valuation_day |
| 11 | 가격 증거 통화를 달러로 고정 | first_evaluation_after_consecutive_holidays_is_not_skipped |
| 12 | 증거 판정 시각을 기록 시각으로 | incomplete_market_uses_previous_final_close_not_today_intraday |
| 13 | 봉의 종목 비교 반전 | first_evaluation_after_consecutive_holidays_is_not_skipped |
| 14 | 증거와 실제 종가 비교 반전 | us_market_date_can_differ_from_kst_valuation_date |
| 15 | 일봉 기간 비교 반전 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 16 | 확정 상태 비교 반전 | even_a_zero_position_cannot_label_an_incomplete_candle_as_a_close |
| 17 | 원주가 조정 여부 비교 반전 | recalculated_us_stock_and_cash_match_d31_expected_values |
| 18 | 봉 부재 오류 변경 | zero_position_cannot_attach_evidence_without_a_candle |
| 19 | 0주 판정 반전 | evidence_must_match_the_actual_candle_currency_date_and_kind |
| 20 | 보유 종목 증거 존재 판정 반전 | evidence_must_match_the_actual_candle_currency_date_and_kind |
| 21 | 누락 검사의 AND를 OR로 | recalculated_us_stock_and_cash_match_d31_expected_values |
| 22 | 환산 완전성 검사 제거 | ledger_period_must_cover_the_requested_cash_date |
| 23 | 대조 원장 입력 제거 | 컴파일 불가 (미사용 필드 또는 변수, 검출 건수 제외) |
| 24 | 종목 이력 제거 | 컴파일 불가 (미사용 필드 또는 변수, 검출 건수 제외) |
| 25 | 알 수 없는 증거 종목 오류 바꿈 | constructor_rejects_missing_evidence_unknown_id_and_missing_fx |
| 26 | 가격 증거 누락 오류 바꿈 | evidence_must_match_the_actual_candle_currency_date_and_kind |
| 27 | 기준 시각 불일치 오류 바꿈 | each_ledger_period_and_observation_time_is_checked_at_construction |
| 28 | 주식 환산 완전성 검사 제거 | offsetting_usd_cash_does_not_hide_missing_stock_fx |
| 29 | 봉 종목 검사를 항상 통과 | zero_position_rejects_wrong_instrument_adjustment_and_period |
| 30 | 원주가 검사를 항상 통과 | zero_without_evidence_rejects_unfinished_nondaily_or_adjusted_candles |
| 31 | 일봉 검사를 항상 통과 | zero_position_rejects_wrong_instrument_adjustment_and_period |
| 32 | 기준 시각의 오프셋 정규화 생략 | equivalent_instant_with_previous_local_date_is_canonicalized |
| 33 | 증거 없는 봉의 검증 제거 | 컴파일 불가 (미사용 필드 또는 변수, 검출 건수 제외) |
| 34 | 증거 없는 봉의 검증 대상 반전 | zero_without_evidence_accepts_absence_and_rejects_wrong_identity_or_currency |
| 35 | 봉 통화 검사를 항상 통과 | zero_without_evidence_accepts_absence_and_rejects_wrong_identity_or_currency |
| 36 | 증거가 있는 봉의 검증 제거 | even_a_zero_position_cannot_label_an_incomplete_candle_as_a_close |
