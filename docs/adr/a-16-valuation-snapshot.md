# A-16 일별 평가 기록

- 상태: 초안

## 결정

기존 PortfolioRepository.latest에서 계좌의 Complete/Partial 포트폴리오를 읽고 저장된 SyncRecord.recorded_at을 관측 평가·기록 시각으로 보존한다. Clock.now보다 미래인 기록은 WrongPeriod로 거부한다. 호출자가 제공한 SnapshotCalendar와 저장된 관측 시각의 Mid 조회 결과를 domain Portfolio.evaluate와 ValuationSnapshot.observe에 전달한다. 유스케이스에서 날짜·금액 계산을 하지 않는다. Failed/AccessBlocked/기록 없음은 오류로 반환하며 오래된 성공 기록으로 대체하지 않는다. 일별 실행 시점과 달력 준비는 J-11의 책임이다. 조회 자료의 신선도 임계값은 정의하지 않는다.

ValuationRepository.append 성공 후 ValuationSnapshotTaken(account)를 한 번 낸다. AlreadyRecorded도 성공이므로 보고서에 그대로 남기고 한 번 발행한다. 저장 실패는 발행하지 않고 원 오류를 돌려준다. 구독 실패는 DeliveryReport로 보존하며 저장을 되돌리거나 재발행하지 않는다. 영속 발행 보장이나 일별 중복 정책은 추가하지 않는다. 새 포트·domain 타입·의존성·가짜 메서드는 없다.

## 기준별 충족과 정의 대기

| 기준 | 상태 | 근거와 필요한 계약 |
|---|---|---|
| 일별 평가 기록 생성·저장 | 충족 | 명시적 평가 달력, Clock 시각, 기존 domain 함수, 기존 append |
| 저장 뒤 계좌 이벤트 한 번 | 충족 | 저장 오류·구독 실패·중복 저장을 포함한 통합 테스트 |
| 빠진 날짜 목록 계산 | 정의 대기 | domain에 기록 이력·시장별 달력에서 빠진 평가일을 산출하는 함수가 없음. Timestamp 연산으로 application에서 만들지 않음 |
| 빠진 날 일봉·거래내역 재계산 | 정의 대기 | recalculate는 있으나 날짜 계획과 종목 이력·기업 행사·대조 일치 현금 원장의 공급 계약이 없음 |
| 자료 부족일을 채우지 못함으로 영속 기록 | 정의 대기 | ValuationRepository.append는 ValuationSnapshot만 받음. SnapshotCalendar 생성 실패 등 스냅샷 자체가 없는 날짜·실패 사유를 저장할 계약이 없음 |
| 가짜 테스트 위치 | 충족 | testkit-ports/tests/valuation_snapshot.rs; application/domain 개발 의존 추가 없음 |

정의 대기 부분을 건너뛰는 자동 복구 API를 제공하지 않는다. 이 변경은 A-16 전체 완료를 뜻하지 않는다.

## 인계

| 받는 작업 | 남은 일 |
|---|---|
| D-31 | 누락 평가일 계획 함수와 자료 부족일 결과 타입·재시도 기준 확정 |
| A-16 | 위 계약 이후 시작 시 누락 계획·일봉·거래내역 재계산 연결 |
| S-18 | 계좌·평가일별 중복 및 원본/재계산/채우지 못함 저장 계약 확정 |
| J-11 | 16:00 실행과 시작 시 복구, 시장별 달력 공급 연결 |

## 검증 경계

실제 키·계좌·API 없음. 프로젝트 신뢰와 훅 신뢰·세션 적용은 확인 불가이며 Git lefthook 설치만 확인했다. 기존 테스트와 보호 규칙 약화 없음. 새 ADR로 발생하는 설정·해시 생성물만 공식 생성 명령으로 갱신한다.

## 1차 리뷰 반영

새 Opus 리뷰와 새 Codex 비판에서 과거·마감 전 자료를 현재 시각으로 바꾸는 결함을 실행으로 확인했다. 자료 시각을 observe에 전달해 달력이 다른 날짜의 자료는 WrongPeriod이고 마감 전 자료는 AwaitingSession으로 보존한다. 현재 시각으로 가격 기준을 올리지 않는다. observe는 평가·기록 시각을 하나만 받으므로 이 경로의 recorded_at 역시 원래 자료 시각이다. 실제 append 실행 시각을 별도로 보존할 기존 계약은 없으며 새 domain API를 만들지 않는다.

SyncRecord 시각과 개별 HoldingObservation 시각의 정합·신선도 검증 함수는 없다. 이 변경만으로 모든 보유 가격이 동시 관측되었다고 보장하지 않으며 D-31/A-06에서 관측 시각의 일관성·가격 기준 계약을 확정해야 한다. 임계값을 추측하거나 오래된 가격을 재계산으로 위장하지 않는다.

외화 보유 3주 × 17달러 = 51달러, Mid1300에서 66300원·Approximate·환율 보존을 고정한다. observe의 fx 제거 변형은 실제 회귀이며 evaluate의 fx 제거는 이 소비 경로에서 동등해 검출 실패로 세지 않는다.

## 손 변형 실제 실행

수정 후 mktemp 복사본에서 기준·원복 테스트 통과, 서로 다른24개 모두 컴파일 후 검출, 생존·시간초과·컴파일불가0개. 각 결과는 target/A-16/hand-results.json과 hand-*.log다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | repository 1 -> InvalidData | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 2 | repository 1 -> RepositoryIntegrity | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 3 | repository 1 -> SecretIntegrity | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 4 | repository 1 -> Unavailable(crate::RetryDelay::Unknown) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 5 | repository 2 -> InvalidData | market_snapshot_and_append_errors_are_preserved |
| 6 | repository 2 -> RepositoryIntegrity | market_snapshot_and_append_errors_are_preserved |
| 7 | repository 2 -> SecretIntegrity | market_snapshot_and_append_errors_are_preserved |
| 8 | repository 2 -> Unavailable(crate::RetryDelay::Unknown) | market_snapshot_and_append_errors_are_preserved |
| 9 | repository 3 -> InvalidData | market_snapshot_and_append_errors_are_preserved |
| 10 | repository 3 -> RepositoryIntegrity | market_snapshot_and_append_errors_are_preserved |
| 11 | repository 3 -> SecretIntegrity | market_snapshot_and_append_errors_are_preserved |
| 12 | repository 3 -> Unavailable(crate::RetryDelay::Unknown) | market_snapshot_and_append_errors_are_preserved |
| 13 | MissingPortfolio -> ValuationSnapshotError::Snapshot(SnapshotError::MissingCash) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 14 | MissingPortfolio -> ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 15 | MissingPortfolio -> ValuationSnapshotError::Repository(SystemPortError::new(crate::SystemPort::Repository, crate::SystemFailure::InvalidData)) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 16 | UnavailablePortfolio -> ValuationSnapshotError::Snapshot(SnapshotError::MissingCash) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 17 | UnavailablePortfolio -> ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 18 | UnavailablePortfolio -> ValuationSnapshotError::Repository(SystemPortError::new(crate::SystemPort::Repository, crate::SystemFailure::InvalidData)) | missing_failed_blocked_and_repository_errors_do_not_write_or_publish |
| 19 | event account | records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure |
| 20 | event kind | records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure |
| 21 | observation time replaced with now | preclose_record_keeps_its_time_and_awaiting_price_basis, stale_and_future_records_cannot_be_relabelled_as_current_observations, foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure |
| 22 | future guard removes equality boundary | observation_at_clock_boundary_is_allowed |
| 23 | future guard inverted | preclose_record_keeps_its_time_and_awaiting_price_basis, foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence, stale_and_future_records_cannot_be_relabelled_as_current_observations, records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure, market_snapshot_and_append_errors_are_preserved |
| 24 | observe drops fx | foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence |

자동 변이에서 Clock 비교 연산자3개가 application 검사에 잡히지 않아, 가짜가 필요 없는 순수 시간 검증과 과거·동일·미래 단위 테스트를 추가했다. 재실행은3개 검출·2개 컴파일불가·생존0·시간초과0이다. 이 코드에서 위 손 변형24개도 다시 전부 컴파일·검출하고 원복 통과를 확인했다.
