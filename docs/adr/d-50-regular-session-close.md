# D-50: 일별 종가의 정규장 종료 기준

- 상태: 초안

## 근거

I-09, R-11, ADR 0028·0052·0112·0118·0127. R-11과 ADR 0112는 16:00 KST에 한국 주식의 당일 종가를 쓰는 것을 전제하고, 일봉 종가는 정규장 종가다.

## 결정

일봉 종가와 평가 가격 증거의 "장 종료"는 정규장 종료로 판정한다. 연장 거래(시간외 단일가, 애프터마켓, 프리마켓)는 그날의 일봉 종가를 미완료로 만들지 않는다. 종료 시각과 같은 순간부터 확정으로 선택한다.

`SessionInformation`은 이미 시장별 정규장·프리마켓·애프터마켓과 미국 데이마켓을 명시적으로 구분한다. 기존 생성자의 뜻과 공개 타입·함수 서명은 바꾸지 않는다. 평가 달력은 이 구분에서 정규장 종료 시각만 뽑는다. 같은 날 세션이 하나도 없을 때만 휴장으로, 연장 거래만 있고 정규장이 없는 행은 종가를 판단할 근거가 없는 것으로 처리한다. 직전일도 정규장 종료를 확인해야 한다.

`MarketDay::price` 하나가 통화별 가격일과 가격 기준을 함께 결정한다. D-31 스냅샷과 D-48 가격 증거는 `SnapshotCalendar::price`를 통해 이를 공유하고 D-49 재계산은 두 경로를 모두 이용한다. `Intraday` 증거는 연장 거래 중에도 종가로 받지 않는다. `SessionClose`와 일봉의 내용은 제공자의 주장이고 이 타입만으로 실제 출처나 종가의 진실성을 인증하지 않는다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| T-08 | 실제 장 운영 응답에서 정규장·프리·애프터·미국 데이마켓을 해당 필드로 구분해 매핑하고, 거래소별 운영 시간·휴장·반일장과 정규장 종가를 제공하는 일봉의 의미를 실측한다. 누락과 확인된 휴장을 구분한다. |
| A-26 | 실제 일봉의 정규장 종가와 증거의 출처·가격·날짜를 확인해 D-49 입력에 공급한다. 장중·연장 거래 체결가를 종가 주장으로 바꾸지 않는다. 달력 최신성과 날짜별 실패를 보존한다. |

## 검증

구현 전 `regular_close` 테스트에서 당일 가격 선택, 반일장 종료, 연장 거래 추가 불변 속성이 실패했다. D-31 직접 재계산과 D-49 입력을 함께 검증하는 테스트도 전일 선택으로 실패했다. 구현 후 관련 테스트는 통과했다. 프로젝트 신뢰와 세션 훅 정의의 신뢰·현재 세션 적용은 각각 확인하지 못했다. Git lefthook 설치 성공은 세션 훅 적용 증거가 아니다. 지시문을 읽기 전 메모리 색인을 조회한 사실이 있으며 이후에는 홈 메모리를 읽지 않았다.

## 손 변형

임시 복사본에서 33개를 실행했다. 컴파일된 29개 중 28개는 첫 실행에서 검출했고 직전 종료 경계 변형 1개는 생존했다. 경계 테스트를 추가한 뒤 그 변형이 실패함을 다시 확인했다. 4개는 컴파일 오류로 검출 수에서 제외했다. 최초 원본 대조군과 원복 대조군은 통과했고, 추가 경계 테스트의 정상 대조군도 통과했다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 결과 |
|---|---|---|
| 1 | 정규장 대신 애프터마켓 선택 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
| 2 | 정규장 대신 프리마켓 선택 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 3 | 정규장 대신 데이마켓 선택 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 4 | 정규장 이외의 세션 선택 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
| 5 | 종료 대신 시작 시각 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 6 | 세션이 비어 있지 않을 때 휴장 | valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations |
| 7 | 세션이 없어도 미확인 | valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries |
| 8 | 정규장이 없으면 항상 휴장 | 컴파일 불가, 집계 제외 |
| 9 | 당일 가격일을 직전 날짜로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 10 | 직전 가격일을 당일 날짜로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 11 | 당일 종료를 직전 행에서 선택 | valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries |
| 12 | 직전 종료를 당일 행에서 선택 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 13 | 당일 종료 경계에서 미확정 | valuation_snapshot_calendar::regular_close_tests::regular_close_handles_short_session_holiday_and_missing_regular_session |
| 14 | 당일 종료 비교 반전 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 15 | 당일 종료 전후 비교 이상 | valuation_recompute_contract::tests::incomplete_market_uses_previous_final_close_not_today_intraday |
| 16 | 당일 확정 날짜를 직전으로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 17 | 당일 확정 기준을 대기로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 18 | 당일 확정 기준을 휴장 이월로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 19 | 미완료 기준을 휴장 이월로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 20 | 휴장 기준을 미완료로 | valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 21 | 정규장 누락 오류 변경 | valuation_snapshot_calendar::regular_close_tests::regular_close_handles_short_session_holiday_and_missing_regular_session |
| 22 | 직전 종료 비교 반전 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 23 | 직전 종료 경계에서 미확정 | 처음 생존, 추가 테스트 regular_close_prior_session_is_usable_at_its_exact_end에서 재검출 |
| 24 | 직전 종료를 당일 날짜에 귀속 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 25 | 직전 가격 기준을 현재 종가로 | 컴파일 불가, 집계 제외 |
| 26 | 직전 가격 기준을 미완료로 | 컴파일 불가, 집계 제외 |
| 27 | 직전 가격 기준을 휴장으로 | 컴파일 불가, 집계 제외 |
| 28 | 확인되지 않은 직전 장을 수용 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 29 | 한 시장 휴장만으로 양국 휴장 | valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 30 | 휴장 조건에서 한국 상태 반전 | valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations |
| 31 | 휴장 조건에서 미국 상태 반전 | valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days |
| 32 | 원화 달력을 미국 달력으로 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
| 33 | 달러 달력을 한국 달력으로 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
