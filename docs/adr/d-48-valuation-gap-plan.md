# D-48 평가 누락일 계획과 가격 증거

- 상태: 초안

## 근거

I-09, ADR 0052·0038·0028·0112, D-48. domain만 수정하며 application 연결은 A-24다.

## 결정

- 평가일은 기존 SnapshotCalendar와 동일하게 한 시장이라도 세션이 있는 날이다. 주말도 공급 달력상 세션이 있으면 평가일이며 요일로 휴장을 추정하지 않는다. 두 시장 모두 휴장이면 제외한다.
- 마지막 평가일은 제외하고 KST 오늘은 포함한다. 오늘 장 마감 전에도 계획에 포함하며 실제 가격 확정 여부는 기존 달력과 재계산기가 판단한다. 미래의 마지막 날짜는 WrongPeriod다. 오늘과 같으면 빈 계획이다. 계획은 복구 후보이며 저장 허가가 아니다. A-24/J-11은 시작 시 후보 계산과 16:00 일별 확정 저장을 구별하고, 오전 후보가 당일 정규 기록을 막지 않도록 연결한다.
- 호출자는 (마지막 날짜, KST 오늘]의 모든 달력일에 대해 한국·미국 달력 쌍 또는 unavailable(날짜, SnapshotError)를 날짜순으로 공급한다. 한국 today가 KST 평가일이고 미국 today는 서버가 공급하는 현지 거래일이다. domain은 미국 현지 날짜를 추측하지 않는다. 중복·역순·누락·초과 입력은 CalendarMismatch이며 임의 휴장으로 치환하지 않는다. 미국 행의 실제 날짜 매핑과 달력 출처·최신성은 공급자 책임이다. 응답을 얻지 못하거나 매핑을 확인하지 못하면 unavailable로 명시한다. 실패 행의 ClosedDay 사유도 확인된 양국 휴장과 구별해 계획에 남긴다.
- 계획은 봉인된 날짜 집합이다. resolve는 달력이 있는 계획일만 채움 함수를 호출하고, 달력 획득 실패는 입력 사유를 그대로 남겨 모든 계획일에 Result<ValuationSnapshot, SnapshotError>를 하나씩 만든다. 결제일이 다른 성공값은 WrongPeriod다. 시장별 날짜 또는 평가 시각의 가격일·가격 기준이 계획 달력과 다르면 CalendarMismatch다. 부분 스냅샷은 기존 완전성 정보를 그대로 보존한다. 외부 조회 실패는 A-24가 기존 SnapshotError로 보수적으로 매핑하며 상세 포트 오류의 영속 정책은 S-18에서 정한다.
- 가격 증거는 기존 양수 Price를 재사용한다. SessionClose·HolidayCarry·AwaitingSession은 확정 종가의 출처 주장이고 Intraday는 장중 관측이다. closing_price는 통화·가격일·기존 SnapshotPriceBasis를 모두 대조하며 장중 값을 종가로 승인하지 않는다. 증거 생성은 경계의 사실 주장으로, 이 타입이 시세 제공자 진위를 인증하지 않는다. 실제 관측 평가 연결은 A-24가 가격과 보유·보고값의 동일성까지 검증해야 한다.
- 가격 0은 자료 없음/유효 종가로 허용하지 않는다(기존 Price 규칙). Option<Money>의 None은 자료 없음이고 Some(0)은 명시적 0 평가다. 음수도 NonZero로 보존하며 평가의 유효성은 기존 평가 계약이 판단한다. presence는 완전성 증거가 아니므로 0 요약만으로 보유·가격 누락 검사를 생략하지 않는다. 0주·대기 행사는 기존 D-31을 따른다.
- 새 의존성·주석·검사 끄기는 없다. 기존 테스트는 수정하지 않는다. 첫 회귀 실행은 구현과 테스트 추가 후 실행했으므로 구현 전 red 증거는 없다.

## 공개 계약

| 타입·함수 | 역할 |
|---|---|
| ValuationCalendarDay::new / unavailable | 확인된 시장별 달력 또는 획득 실패 날짜·사유 |
| ValuationGapPlan::new / dates / resolve | 완전한 달력 범위 검증, 누락일 계획, 전 날짜 결과 |
| ValuationDayResult::date / result | 채움 또는 실패 사유 |
| ValuationPriceEvidence::closing_price | 종가·장중·휴장 이월·미완료 장 구분과 종가 검증 |
| ValuationAmountPresence / valuation_amount_presence | 자료 없음과 명시적 0/비0 구분 |

## 인계

| 작업 | 남은 일 |
|---|---|
| A-24 | 계획 공급·재계산 연결, 과거 종목·행사 이력과 대조 현금 원장 공급. 가격 증거와 실제 보유/요약 값의 동일성 검증, 관측/기록 시각 계약 연결. 오전 복구 후보를 당일 확정 기록으로 저장해 16:00 기록을 막지 않을 것 |
| S-18 | 날짜별 실패 사유와 성공 기록의 계좌별 추가 저장·중복 정책 |
| J-11 | 서버 달력의 날짜별 한국 KST/미국 현지 매핑·공급·16:00 예약 |

A-16 정의 대기: 누락일 목록은 구현 가능, 날짜별 실패 domain 표현은 구현 가능하나 영속 포트는 S-18, 가격 기준은 표현·검증 가능하나 application 연결은 A-24. 빠진 날 전체 재계산은 이력·현금 공급까지 필요하므로 D-48만으로 완료되지 않는다.

## 검증

프로젝트·세션 훅 신뢰 적용은 확인 불가. Git lefthook 설치와 실제 커밋 검사를 별도로 검증한다. 실제 키·계좌·API 없음.

## 손 변형

수정 후 just mutants-diff 994c25a191e0ab08e2c8e9f1f26cfa2de7be3f4d는18개 중10개 검출·8개 컴파일불가·생존0·시간초과0으로 통과했다. 컴파일불가는 검출로 세지 않는다.

초기 구현의 손 변형 실행과 이후 모듈 이동 실행은 작업 사본 target/D-48/hand-before-review/에 보존한다. 첫 리뷰에서 지적된 옛 모듈명을 수정하고, 수정 소스에서 다시 실행한 결과로 아래 표를 갱신한다. 컴파일 불가와 미실행을 검출로 세지 않는다. 집중 회귀 명령은 cargo nextest run --locked -p domain valuation_gap valuation_price_evidence이며 현재12개 통과다. 첫 구현 커밋62213bcc의 정상 훅 전체 검사와 커버리지도 통과했다. 비추적 target 자료는 리뷰 격리 사본에 자동 복사되지 않으며, 그 부재로 실행 여부를 판정하지 않는다.

수정 소스를 mktemp 복사본에서 실행한28개 중26개가 컴파일 후 검출됐고2개는 컴파일 불가로 집계에서 제외했다. 생존0개다. 각 변형의 로그와 마지막 변형 상태/원복 소스의 SHA-256 증거는 작업 사본 target/D-48/hand-*.log, hand-results.json, hand-proof.json에 있다. 사본은 마지막 변형 상태를 검증한 뒤 원복했고 cargo test --locked -p domain --lib valuation_ 대조군55개가 통과했다. 아래 테스트 이름은 현재 모듈과 일치한다.

R6 회귀 returned_snapshot_must_use_planned_market_dates_and_price_bases를 수정 전62213bcc 사본에 추가하면 실제 실패하고, 수정 소스의 nextest에서는 통과한다. 이는 수정 후 수행한 기준 판 대조이며 구현 전 테스트 실행으로 소급하지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 결과 |
|---|---|---|

| 1 | 미래 허용 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 2 | 오늘 거부 | valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries |
| 3 | 초과일 반전 | valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 4 | 연속성 반전 | valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 5 | 연속성 2일 | valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 6 | 마지막 날짜 반전 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 7 | 휴장만 선택 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 8 | 실패일을 휴장으로 제외 | valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes |
| 9 | 실패 사유 소실 | 컴파일 불가(집계 제외) |
| 10 | 실패일 날짜 교체 | 컴파일 불가(집계 제외) |
| 11 | 결제일 비교 반전 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date |
| 12 | 시장별 날짜 비교 반전 | valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date |
| 13 | USD 가격 검증 누락 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
| 14 | KRW 가격 검증 누락 | valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases |
| 15 | 가격 기준 비교 반전 | valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days |
| 16 | 날짜 결과 필드 교체 | valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 17 | 계획 날짜 고정 | valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 18 | 날짜 진행 누락 | valuation_gap::tests::midnight_and_calendar_coverage_are_exact, valuation_gap::tests::results_keep_success_failure_and_wrong_snapshot_date, valuation_gap::tests::consecutive_midweek_holidays_are_not_missing_evaluations, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday, valuation_gap::tests::returned_snapshot_must_use_planned_market_dates_and_price_bases, valuation_gap::tests::weekend_holidays_year_end_and_last_date_boundaries, valuation_gap::tests::unavailable_calendar_dates_keep_reasons_without_skipping_other_days, valuation_gap::tests::unavailable_and_closed_dates_have_distinct_complete_outcomes, valuation_gap::tests::every_open_date_has_exactly_one_result |
| 19 | 현재 종가를 이월로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 20 | 휴장을 미완료로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 21 | 미완료를 현재로 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 22 | 통화 비교 반전 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 23 | 증거 비교 반전 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 24 | 증거 조건 AND | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session, valuation_gap::tests::holiday_carry_preserves_last_close_without_accepting_intraday |
| 25 | 장중 거부 사유 교체 | valuation_price_evidence::tests::closing_evidence_requires_exact_basis_date_currency_and_completed_session |
| 26 | 자료 없음을 0으로 | valuation_price_evidence::tests::zero_price_is_invalid_but_explicit_zero_valuation_is_present |
| 27 | 0을 비0으로 | valuation_price_evidence::tests::zero_price_is_invalid_but_explicit_zero_valuation_is_present |
| 28 | 비0을 0으로 | valuation_price_evidence::tests::zero_price_is_invalid_but_explicit_zero_valuation_is_present |
