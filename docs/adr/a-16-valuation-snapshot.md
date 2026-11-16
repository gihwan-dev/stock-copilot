# A-16 일별 평가 기록

- 상태: 초안

## 결정

선택 계좌의 최신 Complete/Partial 포트폴리오를 기존 PortfolioRepository에서 읽고 기존 Portfolio.evaluate와 ValuationSnapshot.observe로 평가한다. Missing/Failed/AccessBlocked는 오류로 반환한다. 저장된 SyncRecord 시각을 평가·기록 시각으로 보존하고 Clock.now보다 미래이면 거부한다. observe는 하나의 시각만 받으므로 recorded_at도 원래 자료 시각이며 실제 append 시각을 따로 지어내지 않는다. 현재 시각으로 가격 기준을 바꾸지 않는다.

통화별 가격 검증 필요 여부는 보유와 보고 평가의 존재로만 정한다. 해당 통화 보유가 있거나 ReportedPortfolio.valuation().amount(currency)가 Some이면 CurrentSession을 요구한다. 둘 다 없으면 기존 domain이 계산하는 정확한 0이므로 AwaitingSession/HolidayCarry도 보존한다. 보고 USD null은 domain의 빈 통화 의미를 유지한다. 금액·수량의 0 판정을 application에서 계산하지 않는다. Some으로 보고된 0이나 0수량 보유의 추가 생략 정책은 D-31의 전용 계약이 필요하다.

실제 16:00 KST에서 한국은 마감되고 미국은 AwaitingSession이어도 빈 계좌·국내 전용 계좌·보고 USD가 없는 국내 계좌는 저장된다. 미국 달력의 당일을 전일로 바꾸지 않는다. 양 시장 모두 휴장이면 기존 SnapshotCalendar 생성의 ClosedDay를 유지한다. 보유가 없는 통화와 가격 증거 없는 비0 관측값을 같은 경우로 취급하지 않는다.

증거 없는 비0 AwaitingSession/HolidayCarry 관측은 PriceBasisDefinitionPending으로 저장·발행 없이 반환한다. 장중 값을 직전 종가로 위장하지 않는다. 각 Holding과 보고 요약의 관측 시각이 SyncRecord 시각보다 미래이면 거부하고, 기존 calendar.price_date가 계산한 관측 가격일이 스냅샷 가격일과 달라도 거부한다. 보고 요약은 실제 valuation이 제공된 통화만 대조한다. 날짜·금액 계산을 application에서 다시 하지 않는다.

append 성공 후 ValuationSnapshotTaken(account)를 한 번 발행한다. AlreadyRecorded도 그대로 보고하고 한 번 발행한다. 저장 실패는 원 SystemPortError를 반환하고 발행하지 않는다. 구독 실패는 DeliveryReport로 보존하며 저장을 되돌리거나 재발행하지 않는다. 영속 이벤트 전달·동시 일별 중복 정책은 추가하지 않는다.

## 기준별 충족과 정의 대기

| 기준 | 상태 | 근거와 남은 계약 |
|---|---|---|
| 기존 가격 계약으로 안전한 일별 기록 생성·저장 | 충족 | 실제 거래 시간대의 16:00 KST 빈 계좌·국내 전용·USD null 보고 계좌, 휴장 빈 계좌를 테스트 |
| 저장 뒤 계좌 이벤트 한 번 | 충족 | 중복 기록·저장 실패·구독 실패 및 정의 대기 거부의 호출 횟수 검증 |
| 원래 관측 시각과 환율 근거 보존 | 충족 | 미래 거부·개별 보유 가격일 대조·51USD×Mid1300=66300KRW·Approximate |
| 빠진 평가일 목록 계산 | 정의 대기 | 기록 이력과 시장별 달력에서 누락 평가일을 산출하는 domain 정책 함수가 없음 |
| 빠진 날 재계산 연결 | 정의 대기 | recalculate·SnapshotInstrumentHistory::new·candles·transactions/latest·valuation/history는 존재하나 누락일 계획, 종목·기업행사 이력 및 대조 일치 현금 원장 공급 계약이 완비되지 않음 |
| 채우지 못한 날의 영속 기록 | 정의 대기 | append는 ValuationSnapshot만 받음. 달력 실패처럼 스냅샷 자체를 만들 수 없는 날짜·사유의 기록 타입·포트가 없음 |
| 비0 장중·휴장 이월 관측의 정확한 가격 기준 | 정의 대기 | 현재가에 직전 확정 종가 증거가 없음. 임의로 허용하거나 재계산 출처로 바꾸지 않음 |
| 가짜 테스트 위치·의존 규칙 | 충족 | testkit-ports/tests에 가짜 테스트. application에는 가짜 없는 순수 검증 테스트만 있음 |

이 변경은 A-16 전체 기능의 완료를 뜻하지 않는다. 미지원 입력을 건너뛰는 자동 복구 API는 제공하지 않는다.

## 인계

| 받는 작업 | 남은 일 |
|---|---|
| D-31 | 누락 평가일 계획과 가격 증거·통화별 확정 0 입력 판정·관측 시각·실제 기록 시각 계약 |
| A-16 | 선행 계약 뒤 누락일 조회·일봉/거래내역 재계산·부족일 기록 연결 |
| S-18 | 관측/재계산/채우지 못함의 계좌·평가일별 저장과 중복 정책 |
| J-11 | 16:00 실행·시작 시 복구·시장별 달력 공급. 미래/과거 WrongPeriod의 원인별 처리가 필요하면 계약 확정 뒤 구분 |
| A-06/D-31 | 개별 관측 시각과 요약 시각의 정합 및 사용 가능한 가격 증거 계약 |

## 리뷰 결정

3회차의 새 Opus5.5와 새 Codex 비판을 거쳤다. 원래 시각 바꿔치기, FX 테스트 공백, 장중 값과 직전 종가 기준 불일치, 미사용 통화까지 막는 과잉 거부를 고쳤다. 실제 16:00 빈/국내 전용 입력의 과잉 거부만 3차 차단으로 확정했으며 임의 비0 이월 관측까지 허용해야 한다는 주장은 기각했다. 미래/과거 오류 합류의 J-11 오동작은 불확실로 남긴다. AGENTS12 직접 위반과 evaluate(None)의 동등 변형을 결함 검출 실패로 보는 근거는 기각했다.

사용자가 정한 최대3회차 예외에 따라 3차 확정 지적 수정 뒤 4차 리뷰는 하지 않는다. 수정 범위·회귀·변이와 검사 결과는 docs/reviews/A-16.md에 기록한다.

## 검증 경계

실제 키·계좌·API·주문 전송 없음. 실제 거래 시간대와 2026-11-26 휴장 입력을 공개 생성자에 넣은 테스트이며 실서버 달력 검증이 아니다. 프로젝트·훅 신뢰와 세션 적용은 확인 불가, Git lefthook 설치는 확인했다. 새 domain 타입·함수·포트·가짜 메서드·의존성·주석·검사 끄기는 없다. 보호 변경은 새 ADR 경로를 반영한 공식 설정·해시 생성물뿐이다. 기존 master 테스트 변경 승인0건이다.

## 손 변형 실제 실행

최종 소스를 mktemp 복사본과 사본별 새 빌드 출력에서 실행했다. application 순수 테스트와 testkit-ports 통합 테스트를 매 변형마다 모두 실행했다. 기준·원복 대조군이 통과했고 서로 다른35개가 모두 컴파일 후 검출됐다. 생존·시간초과·컴파일불가0개다. 공유 빌드 출력에서 기준이 불일치한 실행과 컴파일되지 않은 초기 변형은 집계에서 제외했다. 결과는 target/A-16/hand-results.json과 hand-*.log, 소스 해시는 hand-source-proof.json이다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | 저장소 오류 전달 1을 다음 오류로 변경: InvalidData | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 2 | 저장소 오류 전달 1을 다음 오류로 변경: RepositoryIntegrity | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 3 | 저장소 오류 전달 1을 다음 오류로 변경: SecretIntegrity | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 4 | 저장소 오류 전달 1을 다음 오류로 변경: Unavailable(crate::RetryDelay::Unknown) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 5 | 저장소 오류 전달 2을 다음 오류로 변경: InvalidData | tests::market_snapshot_and_append_errors_are_preserved |
| 6 | 저장소 오류 전달 2을 다음 오류로 변경: RepositoryIntegrity | tests::market_snapshot_and_append_errors_are_preserved |
| 7 | 저장소 오류 전달 2을 다음 오류로 변경: SecretIntegrity | tests::market_snapshot_and_append_errors_are_preserved |
| 8 | 저장소 오류 전달 2을 다음 오류로 변경: Unavailable(crate::RetryDelay::Unknown) | tests::market_snapshot_and_append_errors_are_preserved |
| 9 | 저장소 오류 전달 3을 다음 오류로 변경: InvalidData | tests::market_snapshot_and_append_errors_are_preserved |
| 10 | 저장소 오류 전달 3을 다음 오류로 변경: RepositoryIntegrity | tests::market_snapshot_and_append_errors_are_preserved |
| 11 | 저장소 오류 전달 3을 다음 오류로 변경: SecretIntegrity | tests::market_snapshot_and_append_errors_are_preserved |
| 12 | 저장소 오류 전달 3을 다음 오류로 변경: Unavailable(crate::RetryDelay::Unknown) | tests::market_snapshot_and_append_errors_are_preserved |
| 13 | MissingPortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Snapshot(SnapshotError::MissingCash) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 14 | MissingPortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 15 | MissingPortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Repository(SystemPortError::new(crate::SystemPort::Repository, crate::SystemFailure::InvalidData)) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 16 | UnavailablePortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Snapshot(SnapshotError::MissingCash) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 17 | UnavailablePortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 18 | UnavailablePortfolio 오류를 다음 값으로 변경: ValuationSnapshotError::Repository(SystemPortError::new(crate::SystemPort::Repository, crate::SystemFailure::InvalidData)) | tests::missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 19 | 이벤트 계좌를 999로 변경 | tests::records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure |
| 20 | 이벤트 종류 변경 | tests::records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure, tests::real_16kst_empty_account_stores_without_us_price_evidence, tests::real_holidays_preserve_safe_empty_snapshots_and_closed_day_contract, tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, tests::observation_at_clock_boundary_is_allowed, tests::real_16kst_reported_domestic_valuation_with_absent_usd_is_supported, tests::real_16kst_domestic_account_stores_without_us_price_evidence |
| 21 | 원자료 시각 대신 Clock 시각 사용 | tests::old_holding_price_cannot_be_relabelled_by_a_newer_sync_record, tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, tests::records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure, tests::stale_and_future_records_cannot_be_relabelled_as_current_observations |
| 22 | 미래 거부에 동일 시각 포함 | valuation_snapshot::tests::observation_is_preserved_before_or_at_now_and_rejected_in_the_future, valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::real_16kst_domestic_account_stores_without_us_price_evidence, tests::observation_at_clock_boundary_is_allowed, tests::real_16kst_reported_domestic_valuation_with_absent_usd_is_supported, tests::reported_usd_without_us_holdings_still_requires_close_evidence, tests::real_holidays_preserve_safe_empty_snapshots_and_closed_day_contract, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync, tests::real_intraday_and_nonzero_carry_prices_still_require_evidence, tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, tests::real_16kst_empty_account_stores_without_us_price_evidence |
| 23 | 미래 비교 방향 반전 | valuation_snapshot::tests::observation_is_preserved_before_or_at_now_and_rejected_in_the_future, valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::either_market_awaiting_close_is_pending_without_storage_or_event, tests::market_snapshot_and_append_errors_are_preserved, tests::preclose_record_is_definition_pending_without_append, tests::old_holding_price_cannot_be_relabelled_by_a_newer_sync_record, tests::records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync, tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, tests::stale_and_future_records_cannot_be_relabelled_as_current_observations |
| 24 | 스냅샷 환율 제거 | tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence |
| 25 | KRW 가격 기준 검사 삭제 | tests::either_market_awaiting_close_is_pending_without_storage_or_event, tests::real_intraday_and_nonzero_carry_prices_still_require_evidence |
| 26 | USD 가격 기준 검사 삭제 | tests::preclose_record_is_definition_pending_without_append, tests::reported_usd_without_us_holdings_still_requires_close_evidence, tests::either_market_awaiting_close_is_pending_without_storage_or_event, tests::real_intraday_and_nonzero_carry_prices_still_require_evidence |
| 27 | 보유 미래 시각 검사 삭제 | valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::old_holding_price_cannot_be_relabelled_by_a_newer_sync_record |
| 28 | 가격일 불일치 비교 반전 | valuation_snapshot::tests::price_date_must_match_in_both_directions, valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::observation_at_clock_boundary_is_allowed, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync, tests::real_16kst_domestic_account_stores_without_us_price_evidence, tests::real_16kst_reported_domestic_valuation_with_absent_usd_is_supported, tests::foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, tests::old_holding_price_cannot_be_relabelled_by_a_newer_sync_record |
| 29 | 보유와 보고가 둘 다 있어야 가격이 필요하도록 변경 | valuation_snapshot::tests::price_requirement_uses_holdings_and_reported_valuation_presence_per_currency, tests::real_intraday_and_nonzero_carry_prices_still_require_evidence, tests::reported_usd_without_us_holdings_still_requires_close_evidence, tests::preclose_record_is_definition_pending_without_append, tests::either_market_awaiting_close_is_pending_without_storage_or_event |
| 30 | 보고 평가 존재 판정 반전 | valuation_snapshot::tests::price_requirement_uses_holdings_and_reported_valuation_presence_per_currency, tests::real_16kst_empty_account_stores_without_us_price_evidence, tests::real_16kst_domestic_account_stores_without_us_price_evidence, tests::real_16kst_reported_domestic_valuation_with_absent_usd_is_supported, tests::real_holidays_preserve_safe_empty_snapshots_and_closed_day_contract, tests::reported_usd_without_us_holdings_still_requires_close_evidence, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync |
| 31 | 보유 통화 선택 비교 반전 | valuation_snapshot::tests::price_requirement_uses_holdings_and_reported_valuation_presence_per_currency, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync, tests::real_16kst_domestic_account_stores_without_us_price_evidence, tests::either_market_awaiting_close_is_pending_without_storage_or_event, tests::real_16kst_reported_domestic_valuation_with_absent_usd_is_supported, tests::real_intraday_and_nonzero_carry_prices_still_require_evidence |
| 32 | 보고 평가 대신 매수 원금 사용 | valuation_snapshot::tests::price_requirement_uses_holdings_and_reported_valuation_presence_per_currency, tests::reported_usd_without_us_holdings_still_requires_close_evidence |
| 33 | 보고 시각을 동기화 시각으로 교체 | valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync |
| 34 | 보고 시각 대조 대상 통화 존재 판정 반전 | tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync |
| 35 | 보고 미래 시각 검사 삭제 | valuation_snapshot::tests::holding_and_report_times_are_validated_independently, tests::reported_valuation_time_cannot_be_relabelled_by_a_newer_sync |

최종 변경분 자동 변이는12개 중9개 검출·3개 컴파일불가·생존0·시간초과0으로 통과했다. 컴파일불가를 검출로 세지 않는다.
