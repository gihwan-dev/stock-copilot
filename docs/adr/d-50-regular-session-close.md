# D-50: 일별 종가의 정규장 종료 기준

- 상태: 초안

## 근거

I-09, R-11, ADR 0028·0052·0112·0118·0127. R-11과 ADR 0112는 16:00 KST에 한국 주식의 당일 종가를 쓰는 것을 전제하고, 일봉 종가는 정규장 종가다.

## 결정

일봉 종가와 평가 가격 증거의 "장 종료"는 정규장 종료로 판정한다. 연장 거래(시간외 단일가, 애프터마켓, 프리마켓)는 그날의 일봉 종가를 미완료로 만들지 않는다. 종료 시각과 같은 순간부터 확정으로 선택한다.

`SessionInformation`은 이미 시장별 정규장·프리마켓·애프터마켓과 미국 데이마켓을 명시적으로 구분한다. 기존 생성자의 뜻과 공개 타입·함수 서명은 바꾸지 않는다. 평가 달력은 이 구분에서 정규장 종료 시각만 뽑는다. 같은 날 세션이 하나도 없을 때만 휴장으로, 연장 거래만 있고 정규장이 없는 행은 종가를 판단할 근거가 없는 것으로 처리한다. 직전일도 정규장 종료를 확인해야 한다.

`MarketDay::price` 하나가 통화별 가격일과 가격 기준을 함께 결정한다. D-31 스냅샷과 D-48 가격 증거는 `SnapshotCalendar::price`를 통해 이를 공유하고 D-49 재계산은 두 경로를 모두 이용한다. `Intraday` 증거는 연장 거래 중에도 종가로 받지 않는다. `SessionClose`와 일봉의 내용은 제공자의 주장이고 이 타입만으로 실제 출처나 종가의 진실성을 인증하지 않는다.

관측 포트폴리오는 종가 증거를 포함하지 않는다. 그 통화의 당일 달력에 프리·데이·애프터 등 연장 거래가 있으면 비0 보유 또는 비0 보고 평가액을 `ValuationSnapshot::observe`에서 `MissingPrice`로 거부한다. 연장 거래가 끝난 뒤에도 현재가가 정규장 종가였음을 추정할 수 없어 동일하게 거부한다. 빈 계좌, 정확한 0, 미사용 통화는 유지한다. 이 판단은 정규장 종가의 완료 판정이나 일봉 재계산의 가격일을 변경하지 않는다. 기존 정규장만 있는 달력의 관측 계약은 유지하지만 그 경우에도 제공 가격의 출처를 인증하는 것은 아니다. 관측 값과 확정 일별 종가의 증거 연결은 A-16으로 인계한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| T-08 | 실제 장 운영 응답에서 정규장·프리·애프터·미국 데이마켓을 해당 필드로 구분하고, 거래소별 운영 시간·휴장·반일장과 일봉 OHLC의 실제 의미를 실측한다. `crates/adapters/broker-toss/src/market_read/candles.rs`의 `assemble`은 현재 최종 세션 종료의 최댓값을 일봉 확정 시각으로 사용한다. 합성 달력에서 16:00과 19:59:59는 `InProgress`, 20:00은 `Finalized`다. 이 범위 밖 어댑터를 후속 작업에서 정규장 계약과 맞추고 종료 직전·정각·연장 거래 중·최종 종료를 검증한다. 누락과 확인된 휴장을 구분한다. |
| A-16 | 관측 보유·보고값에 정규장 종가의 종목별 가격·날짜·출처 증거를 연결한다. 현행 연장 거래 달력의 비0 관측 거부를 근거 없이 제거하지 않는다. 정규장만 있는 기존 관측 경로도 원본 값의 진실성을 스스로 인증하지 않는다. 현재 가드는 당일 세션만 보므로 전일에 연장 거래가 있었고 당일에 없으면 domain 관측에서 전일 값의 출처를 확인하지 않는다. 해당 시장이 사용된 관측의 `AwaitingSession`·`HolidayCarry`는 현재 application이 저장하지 않으며, 이 비대칭도 증거 연결 시 정리한다. |
| A-26 | 실제 일봉의 정규장 종가와 증거의 출처·가격·날짜를 확인해 D-49 입력에 공급한다. `InProgress`를 `Finalized`로 바꾸거나 장중·연장 거래 체결가를 종가 주장으로 바꾸지 않는다. 필요한 자료가 없으면 날짜별 실패와 재시도를 보존하고 달력 최신성을 확인한다. |

현행 연결의 제약은 ADR 0112의 국내 전용 계좌 16:00 저장 계약과 함께 해소해야 한다. 합성 달력에 한국 정규장 09:00~15:30과 애프터마켓 15:30~20:00이 있으면 비0 관측은 16:00에 거부된다. 같은 시각에 위 어댑터가 수집한 당일 일봉은 `InProgress`이므로 양수 보유의 완전한 D-49 입력으로 사용할 수 없다. 빈 계좌 등 정확한 0은 별도 계약을 따르고, 나중에 수집한 봉이나 확정 일봉과 일치하는 종가 증거를 갖춘 domain 입력은 결과가 다를 수 있다. 실제 API 일봉의 구성과 실제 관측값의 출처는 확인하지 않았다.

## 검증

구현 전 `regular_close` 테스트에서 당일 가격 선택, 반일장 종료, 연장 거래 추가 불변 속성이 실패했다. D-31 직접 재계산과 D-49 입력을 함께 검증하는 테스트도 전일 선택으로 실패했다. 구현 후 관련 테스트는 통과했다.

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

관측 가드는 별도 복사본에서 29개 변형을 실행했다. 최초 실행에서 컴파일된 25개 중 24개를 검출했고 원화 보고만 있는 사례를 빠뜨려 1개가 생존했다. 해당 테스트를 추가해 그 변형을 다시 실행한 결과 실패했다. 나머지 4개는 미사용 변수 등으로 컴파일되지 않아 검출에서 제외한다. 최초 정상·원복 대조군은 통과했다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 결과 |
|---|---|---|
| 1 | 연장 거래 가드 조건 반전 | valuation_snapshot::tests::accessors_preserve_provenance_and_individual_toss_daily_reports |
| 2 | 연장 거래 대신 정규장만 가드 | valuation_snapshot::tests::accessors_preserve_provenance_and_individual_toss_daily_reports |
| 3 | 보유 종목 통화 조건 반전 | valuation_snapshot::tests::extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not |
| 4 | 보유 수량 0 판정 반전 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 5 | 보유 수량을 항상 0으로 처리 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 6 | 보유 수량을 항상 비0으로 처리 | valuation_snapshot::tests::extended_observed_exact_zero_and_cash_only_remain_available |
| 7 | 개별 보유 보고액 부호 판정 반전 | valuation_snapshot::tests::extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not |
| 8 | 개별 보유 보고액을 항상 0으로 | 컴파일 불가, 집계 제외 |
| 9 | 개별 보유 보고액을 항상 비0으로 | 컴파일 불가, 집계 제외 |
| 10 | 포트폴리오 보고액 부호 판정 반전 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 11 | 포트폴리오 보고액을 항상 0으로 | 컴파일 불가, 집계 제외 |
| 12 | 포트폴리오 보고액을 항상 비0으로 | 컴파일 불가, 집계 제외 |
| 13 | 보유 수량과 보고액을 둘 다 요구 | valuation_snapshot::tests::extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not |
| 14 | 보유와 요약 보고 모두 비0이어야 거부 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 15 | 통화 불일치라도 비0 보유를 거부 | valuation_snapshot::tests::extended_observed_exact_zero_and_cash_only_remain_available |
| 16 | 종가 근거 누락 대신 잘못된 봉 오류 | valuation_snapshot::tests::extended_observed_nonzero_price_requires_close_evidence_even_after_last_session |
| 17 | 원화만 두 번 검사 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 18 | 달러만 두 번 검사 | valuation_snapshot::tests::extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not |
| 19 | 보고 통화를 원화로 고정 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 20 | 보고 통화를 달러로 고정 | 처음 생존, extended_observed_krw_summary_without_holdings_requires_evidence 추가 후 재검출 |
| 21 | 개별 보고가 없을 때도 보고를 가정 | valuation_snapshot::tests::extended_observed_exact_zero_and_cash_only_remain_available |
| 22 | 요약 보고가 없을 때도 보고를 가정 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |
| 23 | 정규장을 연장 거래로 판정 | valuation_snapshot::tests::accessors_preserve_provenance_and_individual_toss_daily_reports |
| 24 | 애프터마켓만 연장 거래로 판정 | valuation_snapshot_calendar::regular_close_tests::observation_market_sessions_detect_pre_day_and_after_trading |
| 25 | 프리마켓만 연장 거래로 판정 | valuation_snapshot::tests::extended_observed_nonzero_price_requires_close_evidence_even_after_last_session |
| 26 | 데이마켓만 연장 거래로 판정 | valuation_snapshot::tests::extended_observed_nonzero_price_requires_close_evidence_even_after_last_session |
| 27 | 연장 거래가 없으면 연장 거래 있다고 판정 | valuation_snapshot::tests::accessors_preserve_provenance_and_individual_toss_daily_reports |
| 28 | 원화 관측에 달러 세션 사용 | valuation_snapshot::tests::extended_observed_nonzero_price_requires_close_evidence_even_after_last_session |
| 29 | 달러 관측에 원화 세션 사용 | valuation_snapshot::tests::extended_observed_reported_value_and_each_currency_require_evidence |

컴파일되지 않은 보고액 변형 4개는 변수를 유지하는 식으로 다시 작성했다. 원본 대조군의 평가 테스트 98개가 통과했고, 새 식 4개는 모두 컴파일됐다. 개별 보유 보고액을 항상 비0으로 만드는 변형은 처음 생존해 0주·개별 보고액 0 사례를 추가했다. 새 테스트는 이 변형에서 실패하고 원본에서 통과했다. 기존의 컴파일 불가 4개는 여전히 원래 집계에서 제외한다.

| 대체 변형 | 결과 |
|---|---|
| 개별 보유 보고액을 항상 0으로 | `extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not`에서 검출 |
| 개별 보유 보고액을 항상 비0으로 | 처음 생존, `extended_observed_zero_holding_with_zero_report_remains_available` 추가 후 검출 |
| 포트폴리오 보고액을 항상 0으로 | `extended_observed_reported_value_and_each_currency_require_evidence` 등에서 검출 |
| 포트폴리오 보고액을 항상 비0으로 | `extended_observed_reported_zero_is_allowed_but_nonzero_holding_report_is_not`에서 검출 |
